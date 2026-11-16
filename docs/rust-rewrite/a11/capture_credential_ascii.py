"""Optional G08 original-only credential ASCII boundary; never native runtime."""

from __future__ import annotations

# ruff: noqa: E402 -- explicit grant/version/output/network guards precede backend imports
import argparse
import sys
import unicodedata

parser = argparse.ArgumentParser()
parser.add_argument("--allow-original-oracle", action="store_true")
parser.add_argument("--source-root", required=True)
parser.add_argument("--inputs", required=True)
parser.add_argument("--output", required=True)
args = parser.parse_args()
if not args.allow_original_oracle:
    raise RuntimeError("explicit original-oracle grant required")
if sys.version_info[:3] != (3, 14, 7) or unicodedata.unidata_version != "16.0.0":
    raise RuntimeError("Python3.14.7/Unicode16 required")

import asyncio
import base64
import dataclasses
import hashlib
import importlib
import json
import os
import socket
import subprocess
from pathlib import Path

root = Path(args.source_root).resolve()
if root != Path("/Volumes/tank01/magnus/git/slopsearx-a11"):
    raise RuntimeError("isolated A11 source root required")
inputs_path = Path(args.inputs).resolve()
output = Path(args.output)
if output.exists():
    raise FileExistsError("exclusive original output already exists")

network_attempts = []


def denied(*_args, **_kwargs):
    network_attempts.append("real socket/DNS denied")
    raise RuntimeError("A11 credential oracle denies all real socket/DNS")


socket.socket.connect = denied
socket.socket.connect_ex = denied
socket.socket.sendto = denied
socket.create_connection = denied
socket.getaddrinfo = denied
socket.gethostbyname = denied
socket.gethostbyname_ex = denied
socket.gethostbyaddr = denied
socket.getnameinfo = denied
sys.dont_write_bytecode = True
os.environ["PYTHONDONTWRITEBYTECODE"] = "1"
fixture_environment_cleared = []
for name in list(os.environ):
    if name.startswith("ENGINE_") or name.lower() in {
        "http_proxy",
        "https_proxy",
        "all_proxy",
        "ssl_cert_file",
        "ssl_cert_dir",
        "requests_ca_bundle",
        "curl_ca_bundle",
        "sslkeylogfile",
    }:
        fixture_environment_cleared.append(name)
        os.environ.pop(name)
sys.path.insert(0, str(root))


def sha(value):
    return hashlib.sha256(value).hexdigest()


source_paths = {
    f"{directory}/{prefix}{engine}.py": root / f"{directory}/{prefix}{engine}.py"
    for engine in ["abuseipdb", "greynoise", "otx", "urlhaus"]
    for directory, prefix in [("engines", ""), ("tests", "test_")]
}
source_paths.update(
    {
        "slopsearx/adapter.py": root / "slopsearx/adapter.py",
        "tests/test_adapters.py": root / "tests/test_adapters.py",
        "pinned-httpx/_models.py": Path(sys.prefix) / "lib/python3.14/site-packages/httpx/_models.py",
        "pinned-httpx/_client.py": Path(sys.prefix) / "lib/python3.14/site-packages/httpx/_client.py",
    }
)
before = {name: sha(path.read_bytes()) for name, path in source_paths.items()}
input_before = sha(inputs_path.read_bytes())
inputs = json.loads(inputs_path.read_text())
if [case["engine"] for case in inputs["cases"]] != ["abuseipdb", "greynoise", "otx", "urlhaus"]:
    raise RuntimeError("exact four original engine cases required")
if any(case["config"].get("api_key") != "clé" for case in inputs["cases"]):
    raise RuntimeError("only the fixed synthetic Unicode credential is allowed")

import httpx
import httpx._models as httpx_models

if httpx.__version__ != "0.28.1":
    raise RuntimeError("pinned HTTPX0.28.1 required")
if Path(httpx_models.__file__).resolve() != source_paths["pinned-httpx/_models.py"].resolve():
    raise RuntimeError("unexpected HTTPX normalizer module")
classes = {
    "abuseipdb": "AbuseIPDBAdapter",
    "greynoise": "GreyNoiseAdapter",
    "otx": "OTXAdapter",
    "urlhaus": "URLhausAdapter",
}
classes = {name: getattr(importlib.import_module("engines." + name), cls) for name, cls in classes.items()}


def public(value):
    if dataclasses.is_dataclass(value):
        return public(dataclasses.asdict(value))
    if isinstance(value, dict):
        return {str(key): public(item) for key, item in value.items()}
    if isinstance(value, (tuple, list)):
        return [public(item) for item in value]
    if isinstance(value, (set, frozenset)):
        return sorted(public(item) for item in value)
    if value is None or isinstance(value, (str, int, bool, float)):
        return value
    if hasattr(value, "value"):
        return public(value.value)
    raise RuntimeError("unreviewed original capture type " + type(value).__name__)


async def capture(case):
    record = {
        "id": case["id"],
        "engine": case["engine"],
        "query": case["query"],
        "params": case["params"],
        "config": case["config"],
        "exchanges": [],
        "admission_calls": [],
        "header_normalizer_observations": [],
        "adapter_shutdown_joined": False,
        "transport_close_joined": False,
    }

    class Limiter:
        async def acquire(self, engine):
            record["admission_calls"].append(engine)
            return True

    async def exchange(request):
        response = case["response"]
        raw = json.dumps(response["json"], ensure_ascii=True, allow_nan=False, separators=(",", ":")).encode()
        record["exchanges"].append(
            {
                "request": {
                    "method": request.method,
                    "url": str(request.url),
                    "headers": list(request.headers.multi_items()),
                    "body_base64": base64.b64encode(request.content).decode(),
                },
                "response": {"status": response["status"], "body_base64": base64.b64encode(raw).decode()},
            }
        )
        return httpx.Response(response["status"], json=response["json"], request=request)

    original_normalizer = httpx_models._normalize_header_value

    def observed_normalizer(value, encoding=None):
        try:
            return original_normalizer(value, encoding)
        except UnicodeEncodeError as error:
            record["header_normalizer_observations"].append(
                {
                    "class": type(error).__name__,
                    "message": str(error),
                    "encoding": error.encoding,
                    "start": error.start,
                    "end": error.end,
                    "reason": error.reason,
                    "non_ascii_input": isinstance(value, str) and not value.isascii(),
                }
            )
            raise

    transport = httpx.MockTransport(exchange)
    adapter = None
    httpx_models._normalize_header_value = observed_normalizer
    try:
        adapter = classes[case["engine"]](dict(case["config"]), rate_limiter=Limiter())
        adapter.set_http_transport(transport)
        try:
            response = await adapter.search(case["query"], case["params"])
        except Exception as error:
            record["exception"] = {"class": type(error).__name__, "message": str(error)}
        else:
            record["output"] = public(response)
            record["output"].pop("latency_ms", None)
    finally:
        httpx_models._normalize_header_value = original_normalizer
        try:
            if adapter is not None:
                await adapter.shutdown()
                record["adapter_shutdown_joined"] = True
        finally:
            await transport.aclose()
            record["transport_close_joined"] = True
    return record


async def main():
    return [await capture(case) for case in inputs["cases"]]


records = asyncio.run(main())
after = {name: sha(path.read_bytes()) for name, path in source_paths.items()}
if before != after or sha(inputs_path.read_bytes()) != input_before:
    raise RuntimeError("immutable original source or inputs changed")
failures = []
for record in records:
    observations = record["header_normalizer_observations"]
    expected = {
        "results": [],
        "status": "error",
        "error_message": observations[0]["message"] if len(observations) == 1 else None,
        "answers": [],
        "infoboxes": [],
        "corrections": [],
        "synthetic": False,
    }
    if (
        record.get("output") != expected
        or record["exchanges"]
        or record["admission_calls"] != [record["engine"]]
        or not record["adapter_shutdown_joined"]
        or not record["transport_close_joined"]
        or len(observations) != 1
        or observations[0]["class"] != "UnicodeEncodeError"
        or observations[0]["encoding"] != "ascii"
        or not observations[0]["non_ascii_input"]
    ):
        failures.append(record["id"])
if network_attempts:
    failures.append("real-network-or-DNS-attempt")
result = {
    "schema_version": 1,
    "purpose": "Original-only exact four Unicode credential/header refusal controls; not native acceptance",
    "head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
    "producer_sha256": sha(Path(__file__).read_bytes()),
    "input_sha256": input_before,
    "source_sha256": before,
    "source_after_sha256": after,
    "source_inputs_unchanged": True,
    "python": sys.version,
    "unicode": unicodedata.unidata_version,
    "httpx_version": httpx.__version__,
    "real_network_allowed": False,
    "real_network_attempts": network_attempts,
    "fixture_environment_cleared_names": sorted(fixture_environment_cleared),
    "latency_normalization": "only search latency_ms removed",
    "normalizer_observation": (
        "Pass-through wrapper delegates to unchanged pinned HTTPX normalizer and rethrows actual UnicodeEncodeError"
    ),
    "records": records,
    "control_failures": failures,
}
with output.open("x", encoding="utf-8") as stream:
    json.dump(result, stream, ensure_ascii=True, allow_nan=False, indent=2)
    stream.write("\n")
if failures:
    raise RuntimeError("original Unicode credential controls failed: " + ", ".join(failures))
print(json.dumps({"records": len(records), "transport_calls": 0, "output_sha256": sha(output.read_bytes())}))
