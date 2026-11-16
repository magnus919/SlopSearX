"""Temporary layered-config/adapter timing oracle; MockTransport, no provider calls.

Captures declared per-request timeout metadata, not real IO timing guarantees.
Native mandatory gates never execute this migration oracle; G08 retires it.
"""

import argparse  # noqa: I001 - pinned oracle import behavior
import asyncio
import dataclasses
import hashlib
import importlib.util
import json
import math  # noqa: F401 - pinned oracle import behavior
import os
from pathlib import Path
import struct
import sys
import tempfile
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT))
import httpx  # noqa: E402, I001 - pinned oracle import behavior
import yaml  # noqa: E402 - pinned oracle import behavior
import engines  # register source adapters  # noqa: E402, F401 - pinned oracle import behavior
from slopsearx.adapter import discover_engines  # noqa: E402 - pinned oracle import behavior
from slopsearx.config import load_config  # noqa: E402 - pinned oracle import behavior
from slopsearx.service import _normalize_engine_configs  # noqa: E402 - pinned oracle import behavior

spec = importlib.util.spec_from_file_location("a05_existing_export", Path(__file__).with_name("export.py"))
export = importlib.util.module_from_spec(spec)
spec.loader.exec_module(export)


def typed(value):
    if isinstance(value, float):
        return {"kind": "float", "repr": repr(value), "ieee754_be_hex": struct.pack(">d", value).hex()}
    if isinstance(value, bool):
        return {"kind": "bool", "value": value}
    if isinstance(value, int):
        return {"kind": "int", "decimal": str(value)}
    if value is None:
        return {"kind": "NoneType"}
    if isinstance(value, str):
        return {"kind": "str", "value": value}
    if isinstance(value, list):
        return {"kind": "list", "items": [typed(item) for item in value]}
    if isinstance(value, dict):
        return {"kind": "dict", "items": [[key, typed(item)] for key, item in value.items()]}
    raise RuntimeError(type(value).__name__)


async def capture(engine, label, value, env=None, missing=False):
    raw = {"enabled": True}
    if not missing:
        raw["timeout_ms"] = value
    text = yaml.safe_dump({"engines": {engine: raw}}, sort_keys=False)
    row = {
        "id": f"{engine}_layered_timeout_{label}",
        "engine": engine,
        "yaml": text,
        "environment": {} if env is None else {f"ENGINE_{engine.upper()}_TIMEOUT_MS": env},
        "requests": [],
        "response_exchanges": [],
        "source_raised": None,
    }
    adapter = None
    transport = None
    with (
        tempfile.TemporaryDirectory(prefix="a05-config-source-") as directory,
        patch.dict(os.environ, row["environment"], clear=True),
    ):
        path = Path(directory) / "config.yaml"
        path.write_text(text)
        try:
            loaded = load_config(path)
            entry = loaded.engines[engine]
            row["loaded_timeout"] = typed(entry.timeout_ms)
            normalized = _normalize_engine_configs({engine: dataclasses.asdict(entry)})
            row["normalized_config"] = typed(normalized[engine])
        except Exception as error:
            row["source_raised"] = {"phase": "layered_config", "class": type(error).__name__, "message": str(error)}
            return row

        def handler(request):
            row["requests"].append(
                {
                    "method": request.method,
                    "url": str(request.url),
                    "headers": list(request.headers.multi_items()),
                    "body_hex": request.content.hex(),
                    "timeout": typed(request.extensions.get("timeout")),
                }
            )
            home = engine == "duckduckgo" and request.method == "GET"
            body = export.HOME if home else export.PRIMARY if engine == "duckduckgo" else export.G["SAMPLE_HTML"]
            exchange = {
                "status": 200,
                "headers": [["content-type", "text/html; charset=utf-8"]],
                "body_hex": body.encode().hex(),
            }
            row["response_exchanges"].append(exchange)
            return httpx.Response(
                exchange["status"], headers=exchange["headers"], content=bytes.fromhex(exchange["body_hex"])
            )

        try:
            adapter = discover_engines(normalized)[engine]
            transport = httpx.MockTransport(handler)
            adapter.set_http_transport(transport)
            response = await adapter.search("timeout config control", {"categories": []})
            outcome = export.normalize(dataclasses.asdict(response))
            outcome.pop("latency_ms")
            outcome["status"] = response.status.value
            row["expected"] = outcome
        except Exception as error:
            row["source_raised"] = {"phase": "adapter_search", "class": type(error).__name__, "message": str(error)}
        finally:
            if adapter is not None:
                await adapter.shutdown()
            if transport is not None:
                await transport.aclose()
    return row


async def main(output, extended_only=False):
    controls = [
        ("missing", None, None, True),
        ("integer", 1000, None, False),
        ("fractional", 12.5, None, False),
        ("true", True, None, False),
        ("false", False, None, False),
        ("zero", 0, None, False),
        ("negative", -1, None, False),
        ("negative_fractional", -0.5, None, False),
        ("nan", float("nan"), None, False),
        ("infinity", float("inf"), None, False),
        ("negative_infinity", float("-inf"), None, False),
        ("text", "1000", None, False),
        ("null", None, None, False),
        ("list", [], None, False),
        ("object", {}, None, False),
        ("env_default_fractional_rejected", None, "2.5", True),
        ("env_fractional", 12.5, "2.5", False),
        ("env_bool", True, "no", False),
        ("env_text", "1000", "2.5", False),
    ]
    if extended_only:
        controls = [
            ("env_float_nan", 12.5, "nan", False),
            ("env_float_infinity", 12.5, "inf", False),
            ("env_float_negative_infinity", 12.5, "-inf", False),
            ("huge_finite", 1.0e300, None, False),
            ("outside_f06_integer", 2**80, None, False),
            ("integer_53_minus", 2**53 - 1, None, False),
            ("integer_53_plus", 2**53 + 1, None, False),
            ("integer_i64_max", 2**63 - 1, None, False),
            ("tiny_fractional", 0.0001, None, False),
        ]
    rows = [await capture(engine, *control) for engine in ["duckduckgo", "google"] for control in controls]
    sources = [
        "slopsearx/config.py",
        "slopsearx/service.py",
        "slopsearx/adapter.py",
        "engines/duckduckgo.py",
        "engines/google.py",
    ]
    payload = {
        "status": "actual layered YAML/env plus original adapters under MockTransport; no wall-time assertion",
        "source_files": [
            {"path": name, "sha256": hashlib.sha256((ROOT / name).read_bytes()).hexdigest()} for name in sources
        ],
        "python": sys.version,
        "case_count": len(rows),
        "cases": rows,
    }
    output.write_text(json.dumps(payload, indent=2, ensure_ascii=False, allow_nan=False) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--extended-only", action="store_true")
    args = parser.parse_args()
    asyncio.run(main(args.output, args.extended_only))
