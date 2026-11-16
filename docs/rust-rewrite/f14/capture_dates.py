"""Developer-only actual d207 publication-date amendment capture; retire under G08."""

from __future__ import annotations

import argparse
import asyncio
import copy
import hashlib
import json
import subprocess
import sys
from pathlib import Path

REVISION = "d207d4496129d496fe8a74b040ebe1a71151b6f9"
SOURCES = [
    "slopsearx/service.py",
    "slopsearx/filters.py",
    "slopsearx/adapter.py",
    "engines/openalex.py",
    "slopsearx/formatter.py",
    "slopsearx/server.py",
    "slopsearx/mcp/tools.py",
    "tests/test_filter_semantics.py",
    "tests/test_server.py",
    "tests/test_mcp_enforcement.py",
    "tests/test_mcp_tools.py",
]


async def capture(root: Path) -> dict:
    bindings = {}
    for path in SOURCES:
        data = (root / path).read_bytes()
        blob = subprocess.check_output(["git", "-C", str(root), "hash-object", "--", path], text=True).strip()
        expected = subprocess.check_output(
            ["git", "-C", str(root), "rev-parse", f"{REVISION}:{path}"], text=True
        ).strip()
        if blob != expected:
            raise RuntimeError(f"source differs from reviewed amendment: {path}")
        bindings[path] = {"sha256": hashlib.sha256(data).hexdigest(), "git_blob": blob}
    sys.path.insert(0, str(root))
    from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult
    from slopsearx.service import AppContext, SearchRequest, SearchService, search_response_to_payload

    class Engine(EngineAdapter):
        categories = ["science"]

        def __init__(self, name, layers):
            super().__init__()
            self.name = name
            self.enforced_filters = layers
            self.calls = []

        async def search(self, query, params=None):
            self.calls.append([query, params])
            return AdapterResponse(
                status=EngineStatus.OK,
                latency_ms=5.0,
                results=[
                    SearchResult(
                        url="https://example.test/paper",
                        title="paper",
                        content="body",
                        engine=self.name,
                    )
                ],
            )

    class Cache:
        def __init__(self):
            self.payload = None
            self.reads = self.writes = 0

        async def get(self, _key):
            self.reads += 1
            return copy.deepcopy(self.payload)

        async def set(self, _key, payload, _ttl):
            self.writes += 1
            self.payload = copy.deepcopy(payload)

    class Quota:
        calls = 0

        async def acquire(self, *_args, **_kwargs):
            self.calls += 1
            return True

    specs = [
        ("from", {"engines": ["openalex"], "date_from": "2025-01-01"}, None),
        ("to", {"engines": ["openalex"], "date_to": "2025-12-31"}, None),
        ("both", {"engines": ["openalex"], "date_from": "2025-01-01", "date_to": "2025-12-31"}, None),
        ("implicit", {"date_from": "2025-01-01"}, None),
        ("other", {"engines": ["brave"], "date_from": "2025-01-01"}, None),
        ("mixed", {"engines": ["openalex", "brave"], "date_from": "2025-01-01"}, None),
        ("duplicate", {"engines": ["openalex", "openalex"], "date_from": "2025-01-01"}, None),
        ("media", {"engines": ["openalex"], "media_type": "images", "date_from": "2025-01-01"}, None),
        ("relative-empty", {"engines": ["openalex"], "time_range": "", "date_from": "2025-01-01"}, None),
        ("relative-day", {"engines": ["openalex"], "time_range": "day", "date_from": "2025-01-01"}, None),
        ("malformed-precedes-scope", {"engines": ["brave"], "time_range": "", "date_from": "bad"}, None),
        ("reversed", {"engines": ["openalex"], "date_from": "2025-12-31", "date_to": "2025-01-01"}, None),
        ("missing-layer", {"engines": ["openalex"], "date_from": "2025-01-01"}, {"date_from": "upstream"}),
        (
            "local-layer",
            {"engines": ["openalex"], "date_from": "2025-01-01"},
            {"date_from": "upstream", "date_to": "local"},
        ),
    ]
    cases = []
    for name, params, layers in specs:
        engines = {
            n: Engine(n, layers if layers is not None else {"date_from": "upstream", "date_to": "upstream"})
            for n in ["openalex", "brave"]
        }
        cache, quota = Cache(), Quota()
        service = SearchService(AppContext(active_engines=engines, cache=cache, client_rate_window=quota))
        try:
            response = await service.search(SearchRequest(query="date", client_identifier="client", **params))
            output = search_response_to_payload(response)
            output["query_id"], output["response_time_ms"] = "<query-id>", 0
        except Exception as exc:
            output = {"error": type(exc).__name__, "field": getattr(exc, "field", None), "message": str(exc)}
        cases.append(
            {
                "id": name,
                "request": params,
                "layers": layers,
                "output": output,
                "calls": {n: e.calls for n, e in engines.items()},
                "quota": quota.calls,
                "cache_reads": cache.reads,
                "cache_writes": cache.writes,
            }
        )
    for mutation in [
        "unchanged",
        "missing",
        "requested",
        "applied",
        "enforced_by",
        "engines",
        "scope",
        "scope-self-consistent",
    ]:
        engine = Engine("openalex", {"date_from": "upstream", "date_to": "upstream"})
        cache = Cache()
        service = SearchService(AppContext(active_engines={"openalex": engine}, cache=cache))
        request = SearchRequest(query="date", engines=["openalex"], date_from="2025-01-01")
        await service.search(request)
        if mutation == "missing":
            cache.payload.pop("date_filter")
        elif mutation in ["requested", "applied"]:
            cache.payload["date_filter"][mutation]["date_from"] = "2024-01-01"
        elif mutation == "enforced_by":
            cache.payload["date_filter"][mutation] = ["local:openalex"]
        elif mutation == "engines":
            cache.payload["date_filter"][mutation] = ["brave"]
        elif mutation.startswith("scope"):
            cache.payload["scope"]["selected_engines"] = ["brave"]
            if mutation == "scope-self-consistent":
                cache.payload["date_filter"]["engines"] = ["brave"]
        response = await service.search(request)
        cases.append(
            {
                "id": f"cache-{mutation}",
                "kind": "cache",
                "mutation": mutation,
                "cached": response.cached,
                "engine_calls": len(engine.calls),
                "proof": response.date_filter,
                "scope": response.scope.selected_engines,
            }
        )
    return {
        "schema_version": 1,
        "plan_id": "F14",
        "source_commit": REVISION,
        "python": sys.version.split()[0],
        "source_files": bindings,
        "provenance": "Actual amended Python source with local synthetic adapters and memory cache; no provider calls.",
        "normalizations": ["query_id", "response_time_ms"],
        "cases": cases,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.write_text(
        json.dumps(asyncio.run(capture(args.source_root.resolve())), indent=2, ensure_ascii=False) + "\n"
    )


if __name__ == "__main__":
    main()
