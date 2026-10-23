"""Actual source config observations over existing offline MockTransport only."""

import asyncio
import hashlib
import json
import math
import runpy
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
BASE = runpy.run_path(str(ROOT / "docs/rust-rewrite/a06/oracle/export_source.py"))
VALUES = [
    ("false", False, "reachable scalar"),
    ("true", True, "reachable scalar"),
    ("zero", 0, "reachable scalar"),
    ("one", 1, "reachable scalar"),
    ("negative", -1, "reachable scalar"),
    ("integer", 3, "reachable scalar"),
    ("float_zero", 0.0, "reachable scalar"),
    ("fraction", 2.5, "reachable scalar"),
    ("text_number", "2", "reachable scalar"),
    ("text_empty", "", "reachable scalar"),
    ("text_bad", "bad", "reachable scalar"),
    ("null", None, "direct-source-only; F06 rejects null"),
    ("list", [], "direct-source-only; F06 rejects list"),
    ("map", {}, "direct-source-only; F06 rejects map"),
    ("nan", math.nan, "reachable envFloat; nonfinite YAML rejected"),
    ("inf", math.inf, "reachable envFloat; nonfinite YAML rejected"),
    ("negative_inf", -math.inf, "reachable envFloat; nonfinite YAML rejected"),
]


def encode(value):
    if isinstance(value, float) and not math.isfinite(value):
        return {"$source_nonfinite_float": str(value)}
    if isinstance(value, dict):
        return {key: encode(item) for key, item in value.items()}
    if isinstance(value, list):
        return [encode(item) for item in value]
    return value


async def main():
    records = []
    for engine in BASE["TYPES"]:
        body = (
            BASE["FEED"]
            if engine == "arxiv"
            else {("results" if engine == "openalex" else "data"): [{"title": f"paper-{index}"} for index in range(3)]}
        )
        for field in ["max_results", "timeout_ms"]:
            for name, value, reachability in VALUES:
                for malformed in [False, True] if field == "max_results" else [False]:
                    case = {
                        "id": f"{engine}.config.{field}.{name}." + ("malformed" if malformed else "valid"),
                        "engine": engine,
                        "query": "offline config",
                        "config": {field: value},
                        "responses": [
                            BASE["reply"]("<feed" if engine == "arxiv" else "!", 200)
                            if malformed
                            else BASE["reply"](body)
                        ],
                    }
                    record = await BASE["capture"](case)
                    record["native_reachability"] = reachability
                    records.append(encode(record))
    out = {
        "schema_version": 1,
        "plan_id": "A06",
        "python": sys.version,
        "scope": "actual source full adapter observations; MockTransport does not qualify real socket timeout behavior",
        "encoding": (
            "Only actual nonfinite floats use $source_nonfinite_float; observation encoding, "
            "never native expected normalization. latency_ms omitted by existing exporter; sets sorted."
        ),
        "source_sha256": {
            name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest()
            for name in [
                "engines/arxiv.py",
                "engines/openalex.py",
                "engines/semanticscholar.py",
                "slopsearx/config.py",
                "docs/rust-rewrite/a06/oracle/export_source.py",
            ]
        },
        "records": records,
    }
    print(json.dumps(out, ensure_ascii=True, allow_nan=False, indent=2))


if __name__ == "__main__":
    asyncio.run(main())
