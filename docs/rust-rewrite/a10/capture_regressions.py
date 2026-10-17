"""Optional G08 original-only A10 regression capture. Never a native gate input producer."""

from __future__ import annotations

# ruff: noqa: E402 -- explicit admission/network guards precede backend imports
# Pure admission and runtime guards MUST precede backend/third-party imports.
import argparse
import sys
import unicodedata

parser = argparse.ArgumentParser()
parser.add_argument("--allow-original-oracle", action="store_true")
parser.add_argument("--output", required=True)
args = parser.parse_args()
if not args.allow_original_oracle:
    raise RuntimeError("original oracle requires explicit --allow-original-oracle; native gates never invoke it")
if sys.version_info[:3] != (3, 14, 7) or unicodedata.unidata_version != "16.0.0":
    raise RuntimeError("original oracle requires Python3.14.7 / Unicode16.0.0")

import socket


def denied(*_args, **_kwargs):
    raise RuntimeError("A10 original oracle denies all real network/DNS operations")


socket.socket.connect = denied
socket.socket.connect_ex = denied
socket.socket.sendto = denied
socket.create_connection = denied
socket.getaddrinfo = denied

# This process writes exclusively-created migration evidence, never source inputs.
import dataclasses
import functools
import hashlib
import importlib
import importlib.metadata
import json
import math
import os
import subprocess
from pathlib import Path

os.environ["PYTHONDONTWRITEBYTECODE"] = "1"
sys.dont_write_bytecode = True
os.environ["PYTEST_DISABLE_PLUGIN_AUTOLOAD"] = "1"
for key in list(os.environ):
    if key.startswith("ENGINE_") or key.lower() in {"http_proxy", "https_proxy", "all_proxy"}:
        os.environ.pop(key)
root = Path(__file__).resolve().parents[3]
if root.name != "slopsearx-a10" or not (root / "engines/cve.py").is_file():
    raise RuntimeError("run only from the isolated A10 worktree with producer at docs/rust-rewrite/a10")
sys.path.insert(0, str(root))
output = Path(args.output)
stream = output.open("x", encoding="utf-8")
source_paths = [
    f"{directory}/{prefix}{engine}.py"
    for engine in ("cve", "nvd", "epss", "exploitdb", "mitreattack")
    for directory, prefix in (("engines", ""), ("tests", "test_"))
]
source_hashes = {p: hashlib.sha256((root / p).read_bytes()).hexdigest() for p in source_paths}
producer_sha = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()

import httpx
import pytest

records = []
reports = []
current_test = ""
active = None
counter = {}


def safe(value):
    if dataclasses.is_dataclass(value):
        return safe(dataclasses.asdict(value))
    if isinstance(value, dict):
        return {str(k): safe(v) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return [safe(v) for v in value]
    if isinstance(value, (set, frozenset)):
        return sorted(safe(v) for v in value)
    if isinstance(value, float) and not math.isfinite(value):
        return {"__original_nonfinite_number__": repr(value)}
    if value is None or isinstance(value, (str, int, float, bool)):
        return value
    if hasattr(value, "value"):
        return safe(value.value)
    raise RuntimeError(f"unreviewed capture type: {type(value).__name__}")


def identity(engine, operation):
    key = (current_test, engine, operation)
    number = counter.get(key, 0)
    counter[key] = number + 1
    return f"{current_test}::{engine}.{operation}::{number}"


original_mock = httpx.MockTransport.handle_async_request


async def observed_mock(self, request):
    global active
    import base64

    request_row = {
        "method": request.method,
        "url": str(request.url),
        "headers": list(request.headers.multi_items()),
        "body_base64": base64.b64encode(request.content).decode("ascii"),
    }
    row = {"request": request_row}
    if active is not None:
        active["exchanges"].append(row)
    try:
        response = await original_mock(self, request)
        body = await response.aread()
        row["response"] = {
            "status": response.status_code,
            "headers": list(response.headers.multi_items()),
            "body_base64": base64.b64encode(body).decode("ascii"),
        }
        return response
    except Exception as error:
        row["exception"] = {"class": type(error).__name__, "message": str(error)}
        raise


httpx.MockTransport.handle_async_request = observed_mock

classes = {
    "cve": "CVEAdapter",
    "nvd": "NVDAdapter",
    "epss": "EPSSAdapter",
    "exploitdb": "ExploitDBAdapter",
    "mitreattack": "MitreAttackAdapter",
}
for engine, class_name in classes.items():
    cls = getattr(importlib.import_module("engines." + engine), class_name)
    original_search = cls.search

    def wrap_search(original, engine):
        @functools.wraps(original)
        async def wrapped(self, query, params=None):
            global active
            row = {
                "id": identity(engine, "search"),
                "source_test": current_test,
                "engine": engine,
                "operation": "search",
                "query": query,
                "params": safe(params),
                "config": safe(self.config),
                "exchanges": [],
            }
            previous = active
            active = row
            try:
                result = await original(self, query, params)
                value = safe(result)
                value.pop("latency_ms", None)
                row["output"] = value
                return result
            except Exception as error:
                row["exception"] = {"class": type(error).__name__, "message": str(error)}
                raise
            finally:
                records.append(row)
                active = previous

        return wrapped

    cls.search = wrap_search(original_search, engine)
    for method_name in list(vars(cls)):
        if not method_name.startswith(("_parse_", "_extract_", "_format_", "_cvss_", "_best_")):
            continue
        original_method = getattr(cls, method_name)
        if not callable(original_method):
            continue

        def wrap_method(original, engine, operation):
            @functools.wraps(original)
            def wrapped(self, *values, **kwargs):
                row = {
                    "id": identity(engine, operation),
                    "source_test": current_test,
                    "engine": engine,
                    "operation": operation,
                    "arguments": safe(values),
                    "kwargs": safe(kwargs),
                    "nested_in_search": active is not None,
                }
                try:
                    result = original(self, *values, **kwargs)
                    row["output"] = safe(result)
                    return result
                except Exception as error:
                    row["exception"] = {"class": type(error).__name__, "message": str(error)}
                    raise
                finally:
                    records.append(row)

            return wrapped

        setattr(cls, method_name, wrap_method(original_method, engine, method_name))


class Capture:
    def pytest_runtest_setup(self, item):
        global current_test
        current_test = item.nodeid

    def pytest_runtest_logreport(self, report):
        reports.append({"nodeid": report.nodeid, "when": report.when, "outcome": report.outcome})


exit_code = None
try:
    exit_code = int(
        pytest.main(
            [
                "-o",
                "addopts=",
                "-p",
                "pytest_asyncio.plugin",
                "-p",
                "no:cacheprovider",
                "-q",
                *["tests/test_" + engine + ".py" for engine in classes],
            ],
            plugins=[Capture()],
        )
    )
finally:
    after = {p: hashlib.sha256((root / p).read_bytes()).hexdigest() for p in source_paths}
    result = {
        "schema_version": 1,
        "purpose": "immutable optional original regression observations; never runtime/native implementation",
        "head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
        "producer_sha256": producer_sha,
        "python": sys.version,
        "unicode": unicodedata.unidata_version,
        "packages": {name: importlib.metadata.version(name) for name in ("httpx", "pytest", "pytest-asyncio", "lxml")},
        "source_sha256": source_hashes,
        "source_inputs_unchanged": after == source_hashes,
        "pytest_exit": exit_code,
        "reports": reports,
        "records": records,
        "real_network_allowed": False,
        "latency_normalization": "only search latency_ms removed; no other result normalization",
    }
    json.dump(result, stream, ensure_ascii=True, allow_nan=False, indent=2)
    stream.write("\n")
    stream.close()
    if after != source_hashes:
        raise RuntimeError("immutable original inputs changed")
if exit_code != 0:
    raise RuntimeError(f"original source regression execution failed: {exit_code}")
