"""Optional G08 original A10 boundary oracle; native implementation never invokes it."""

from __future__ import annotations

# ruff: noqa: E402 -- pure admission and network guards precede backend imports
import argparse
import sys
import unicodedata

parser = argparse.ArgumentParser()
parser.add_argument("--allow-original-oracle", action="store_true")
parser.add_argument("--output", required=True)
parser.add_argument("--inputs", default="boundary-inputs-v1.json")
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

import asyncio
import base64
import dataclasses
import hashlib
import json
import math
import os
import subprocess
from pathlib import Path

os.environ["PYTHONDONTWRITEBYTECODE"] = "1"
sys.dont_write_bytecode = True
for key in list(os.environ):
    if key.startswith("ENGINE_") or key.lower() in {"http_proxy", "https_proxy", "all_proxy"}:
        os.environ.pop(key)
root = Path(__file__).resolve().parents[3]
if root != Path("/Volumes/tank01/magnus/git/slopsearx-a10"):
    raise RuntimeError("run only from the isolated A10 worktree")
sys.path.insert(0, str(root))


def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()


if (
    Path(args.inputs).name != args.inputs
    or not args.inputs.startswith("boundary-")
    or not args.inputs.endswith(".json")
):
    raise RuntimeError("original oracle input must name an inventoried local boundary fixture")
input_path = root / "docs/rust-rewrite/a10" / args.inputs
source_paths = [
    root / d / (prefix + e + ".py")
    for e in ("cve", "nvd", "epss", "exploitdb", "mitreattack")
    for d, prefix in [("engines", ""), ("tests", "test_")]
]
pins = {str(p.relative_to(root)): sha(p) for p in [*source_paths, input_path]}
inputs = json.loads(input_path.read_text())
ids = [r["id"] for r in inputs["cases"]]
if len(ids) != len(set(ids)):
    raise RuntimeError("duplicate original boundary identity")
stream = Path(args.output).open("x", encoding="utf-8")

import httpx

import engines  # noqa: F401
from slopsearx.adapter import discover_engines
from tests.test_adapters import MockHTTP


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
    raise RuntimeError(f"unreviewed original capture type: {type(value).__name__}")


async def main():
    records = []
    for case in inputs["cases"]:
        e = case["engine"]
        row = {
            "id": case["id"],
            "source_test": None,
            "engine": e,
            "operation": "search",
            "query": case["query"],
            "params": case["params"],
            "config": {"enabled": True, **case["config"]},
            "exchanges": [],
        }
        adapter = discover_engines({e: row["config"]})[e]
        response = case["response"]

        def handler(request):
            exchange = {
                "request": {
                    "method": request.method,
                    "url": str(request.url),
                    "headers": list(request.headers.multi_items()),
                    "body_base64": base64.b64encode(request.content).decode("ascii"),
                }
            }
            row["exchanges"].append(exchange)
            if "exception" in response:
                error = getattr(httpx, response["exception"])("synthetic offline request failure")
                exchange["exception"] = {"class": type(error).__name__, "message": str(error)}
                raise error
            result = httpx.Response(
                response["status"], headers=response["headers"], content=response["body"].encode("utf-8")
            )
            exchange["response"] = {
                "status": result.status_code,
                "headers": list(result.headers.multi_items()),
                "body_base64": base64.b64encode(result.content).decode("ascii"),
            }
            return result

        try:
            async with MockHTTP(handler):
                result = await adapter.search(case["query"], case["params"])
            output = safe(result)
            output.pop("latency_ms", None)
            row["output"] = output
        except Exception as error:  # noqa: BLE001 -- preserve original unhandled source failures
            row["exception"] = {"class": type(error).__name__, "message": str(error)}
        records.append(row)
        print("A10_ORIGINAL_BOUNDARY_COMPLETE " + case["id"], flush=True)
    return records


records = None
try:
    records = asyncio.run(main())
    after = {str(p.relative_to(root)): sha(p) for p in [*source_paths, input_path]}
    if pins != after:
        raise RuntimeError("original boundary inputs changed during capture")
    if len(records) != len(ids) or {r["id"] for r in records} != set(ids):
        raise RuntimeError("original boundary case completion mismatch")
    json.dump(
        {
            "schema_version": 1,
            "purpose": "immutable optional original boundary observations; not native acceptance",
            "head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
            "producer_sha256": sha(Path(__file__)),
            "python": sys.version.split()[0],
            "unicode": unicodedata.unidata_version,
            "source_sha256": pins,
            "source_inputs_unchanged": True,
            "records": records,
            "real_network_allowed": False,
            "latency_normalization": "remove only original search latency_ms",
        },
        stream,
        ensure_ascii=True,
        indent=2,
        allow_nan=False,
    )
    stream.write("\n")
finally:
    stream.close()
if records is None:
    raise RuntimeError("original boundary capture did not complete")
