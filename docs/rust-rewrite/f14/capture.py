"""Developer-only F14 actual Python source capture; retire under G08."""

from __future__ import annotations

import argparse
import asyncio
import datetime
import hashlib
import json
import math
import os
import subprocess
import sys
from pathlib import Path
from unittest.mock import patch
from zoneinfo import ZoneInfo


async def capture(root: Path) -> dict:
    sys.path.insert(0, str(root))
    from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult
    from slopsearx.config import Config
    from slopsearx.filters import time_range_window
    from slopsearx.jev import JevRoutingDecision
    from slopsearx.router import QueryRouter
    from slopsearx.service import AppContext, SearchRequest, SearchService, build_context, search_response_to_payload
    from slopsearx.suggest import SuggestionService, _suggest_cache_key

    class Engine(EngineAdapter):
        categories = ["general"]

        def __init__(self, name, status="ok"):
            super().__init__()
            self.name = name
            self.status = EngineStatus(status)
            self.calls = []

        async def search(self, query, params=None):
            self.calls.append([query, params])
            return AdapterResponse(
                results=[
                    SearchResult(
                        url=f"https://example.test/{self.name}/{i}",
                        title=f"{self.name} {i}",
                        content="body",
                        engine=self.name,
                    )
                    for i in range(3)
                ]
                if self.status == EngineStatus.OK
                else [],
                status=self.status,
                latency_ms=5.0,
                error_message=None if self.status == EngineStatus.OK else "fixture failure",
                answers=[{"answer": self.name}],
                corrections=[self.name],
                infoboxes=[{"name": self.name}],
            )

    cases = []
    specs = [
        ("complete", {}, ["ok", "ok"]),
        ("partial", {}, ["ok", "error"]),
        ("all-error", {}, ["error", "timeout"]),
        ("blocked", {}, ["blocked", "rate_limited"]),
        ("view-one", {"max_results": 1, "include": []}, ["ok", "ok"]),
        ("view-zero", {"max_results": 0}, ["ok", "ok"]),
        ("explicit-order", {"engines": ["wikipedia", "brave"]}, ["ok", "ok"]),
        ("duplicate", {"engines": ["brave", "brave"]}, ["ok", "ok"]),
        ("category", {"categories": ["general"]}, ["ok", "ok"]),
        ("no-suggestions", {"generate_suggestions": False}, ["ok", "ok"]),
    ]
    for name, params, statuses in specs:
        engines = {n: Engine(n, status) for n, status in zip(["brave", "wikipedia"], statuses, strict=True)}
        request = SearchRequest(query=" café 🐙 ", **{k: set(v) if k == "include" else v for k, v in params.items()})
        response = await SearchService(AppContext(active_engines=engines)).search(request)
        output = search_response_to_payload(response)
        output["query_id"] = "<query-id>"
        output["response_time_ms"] = 0
        cases.append(
            {
                "id": name,
                "kind": "service",
                "request": params,
                "statuses": statuses,
                "output": output,
                "calls": {n: e.calls for n, e in engines.items()},
            }
        )

    class Router:
        def cache_identity(self):
            return "f14-reference-advice"

        async def route(self, *args, **kwargs):
            return JevRoutingDecision(engines=["pubmed"], scores={"pubmed": 0.9}, latency_ms=0.0)

    engines = {n: Engine(n) for n in ["brave", "pubmed"]}
    response = await SearchService(
        AppContext(active_engines=engines, router=QueryRouter({"enabled": False}), jev_router=Router())
    ).search(SearchRequest(query=" café 🐙 "))
    output = search_response_to_payload(response)
    output["query_id"] = "<query-id>"
    output["response_time_ms"] = 0
    cases.append(
        {
            "id": "jev-promoted",
            "kind": "jev-service",
            "output": output,
            "calls": {n: e.calls for n, e in engines.items()},
        }
    )

    for zone, stamp in [
        ("America/New_York", "2026-03-08T04:59:59+00:00"),
        ("America/New_York", "2026-03-08T05:00:00+00:00"),
        ("America/New_York", "2026-03-08T06:59:59+00:00"),
        ("America/New_York", "2026-03-08T07:00:00+00:00"),
        ("America/New_York", "2026-11-01T05:59:59+00:00"),
        ("America/New_York", "2026-11-01T06:00:00+00:00"),
        ("Asia/Tokyo", "2026-03-08T15:00:00+00:00"),
        ("UTC", "2026-03-08T00:00:00+00:00"),
    ]:
        instant = datetime.datetime.fromisoformat(stamp)
        date = instant.astimezone(ZoneInfo(zone)).date()
        window = time_range_window("day", date)
        cases.append(
            {
                "id": f"calendar-{len(cases)}",
                "kind": "calendar",
                "zone": zone,
                "epoch": int(instant.timestamp()),
                "date": date.isoformat(),
                "window": [d.isoformat() for d in window],
            }
        )

    class EmptyCache:
        async def connect(self):
            pass

    settings_cases = [
        {},
        {"MAX_CONCURRENT_ENGINES": "bad", "PER_CLIENT_REQUESTS": "bad", "PER_CLIENT_WINDOW_SECONDS": "bad"},
        {"MAX_CONCURRENT_ENGINES": "0", "FEATURE_EMPTY_SCRAPE_DIAGNOSTICS": "TRUE"},
        {"MAX_CONCURRENT_ENGINES": "-999999999999999999999999999999999999"},
        {"MAX_CONCURRENT_ENGINES": " +１２ ", "PER_CLIENT_REQUESTS": "٣.٥", "PER_CLIENT_WINDOW_SECONDS": "1_0"},
        {"FAIL_CLOSED": " YeS ", "FEATURE_EMPTY_SCRAPE_DIAGNOSTICS": " true "},
        {"FAIL_CLOSED": "on", "FEATURE_EMPTY_SCRAPE_DIAGNOSTICS": "1"},
        {"PER_CLIENT_REQUESTS": "nan"},
        {"PER_CLIENT_REQUESTS": "inf"},
        {"PER_CLIENT_REQUESTS": "-1"},
        {"PER_CLIENT_WINDOW_SECONDS": "0"},
        {"PER_CLIENT_WINDOW_SECONDS": "0.0001"},
        {"PER_CLIENT_WINDOW_SECONDS": "0.001"},
        {"MAX_CONCURRENT_ENGINES": "9999999999999999999999999999999999"},
    ]
    for env in settings_cases:
        with (
            patch.dict(os.environ, env, clear=True),
            patch("slopsearx.service.SearchCache", EmptyCache),
            patch("slopsearx.service.discover_engines", return_value={}),
            patch("slopsearx.service.load_config", return_value=Config()),
        ):
            context = await build_context()
        rate, window = context.client_rate_window._default_rate, context.client_rate_window._window
        cases.append(
            {
                "id": f"settings-{len(cases)}",
                "kind": "settings",
                "input": env,
                "output": {
                    "max_parallel_engines": str(context.engine_semaphore._value),
                    "rate": rate if math.isfinite(rate) else str(rate),
                    "window": window if math.isfinite(window) else str(window),
                    "fail_closed": context.client_rate_window._fail_closed,
                    "diagnostics": context.empty_scrape_diagnostics_enabled,
                },
            }
        )

    class Response:
        status_code = 200

        def __init__(self, body):
            self.body = body

        def json(self):
            return self.body

    class Client:
        def __init__(self, *args, **kwargs):
            pass

        async def __aenter__(self):
            return self

        async def __aexit__(self, *args):
            pass

        async def get(self, url, **kwargs):
            return Response(body)

    for provider, body in [
        ("brave", {"results": [{"q": "one"}, {"q": "two"}]}),
        ("brave", ["one", 4, None, {"other": [True]}]),
        ("brave", {"results": [{"q": "valid"}, {"q": None}]}),
        ("brave", {"results": [{"q": 4}]}),
        ("brave", {"results": {}}),
        ("ddg", ["query", ["one", "", False, 0, None, 4, {"x": True}]]),
        ("ddg", ["query", []]),
        ("ddg", {}),
    ]:
        service = SuggestionService("synthetic")
        with patch("slopsearx.suggest.httpx.AsyncClient", Client):
            result = await (service._fetch_brave("query") if provider == "brave" else service._fetch_ddg("query"))
        cases.append(
            {"id": f"suggest-{len(cases)}", "kind": "suggest", "provider": provider, "input": body, "output": result}
        )
    for query in ["", " café 🐙 ", "ABC", "\x1cABC\x1f", "İ"]:
        cases.append(
            {
                "id": f"suggest-key-{len(cases)}",
                "kind": "suggest-key",
                "query": query,
                "output": _suggest_cache_key(query),
            }
        )

    class CountingWindow:
        calls = 0

        async def acquire(self, *args, **kwargs):
            self.calls += 1
            return False

    class CountingCache:
        reads = 0
        writes = 0

        async def get(self, *args, **kwargs):
            self.reads += 1
            return None

        async def set(self, *args, **kwargs):
            self.writes += 1

    for suffix, names in [("empty", []), ("sensitive-only", ["hibp"])]:
        engines = {name: Engine(name) for name in names}
        window, cache = CountingWindow(), CountingCache()
        context = AppContext(active_engines=engines, cache=cache, client_rate_window=window)
        response = await SearchService(context).search(SearchRequest(query="availability", client_identifier="blocked"))
        output = search_response_to_payload(response)
        output["query_id"] = "<query-id>"
        output["response_time_ms"] = 0
        cases.append(
            {
                "id": f"availability-{suffix}",
                "kind": "availability",
                "engines": names,
                "output": output,
                "quota_calls": window.calls,
                "cache_reads": cache.reads,
                "cache_writes": cache.writes,
                "engine_calls": {name: len(engine.calls) for name, engine in engines.items()},
            }
        )

    # Append-only extension: preserve all historical 48 case objects above.
    import httpx

    for query in ["q*~", " café 🐙 +%/?&= ", "a\t\n\x00", "!*'()_-."]:
        for provider in ["brave", "ddg-fallback"]:
            seen = []

            def handle(request):
                seen.append(request.url.query.decode("ascii"))
                if request.url.host == "api.search.brave.com":
                    return httpx.Response(
                        200, json={"results": [{"q": "x"}]} if provider == "brave" else {"results": []}
                    )
                return httpx.Response(200, json=["query", ["x"]])

            async with httpx.AsyncClient(transport=httpx.MockTransport(handle)) as client:
                with patch("slopsearx.suggest.httpx.AsyncClient") as mocked:
                    mocked.return_value.__aenter__.return_value = client
                    output = await SuggestionService("synthetic-key").fetch(query)
            cases.append(
                {
                    "id": f"suggest-wire-{len(cases)}",
                    "kind": "suggest-wire",
                    "query": query,
                    "provider": provider,
                    "wire_queries": seen,
                    "output": output,
                }
            )

    sources = [
        "slopsearx/service.py",
        "slopsearx/suggest.py",
        "slopsearx/adapter.py",
        "slopsearx/cache.py",
        "slopsearx/snapshot.py",
        "slopsearx/filters.py",
        "slopsearx/merger.py",
        "slopsearx/scholarly.py",
        "slopsearx/router.py",
        "slopsearx/capabilities.py",
        "slopsearx/ratelimit.py",
        "slopsearx/payload.py",
        "slopsearx/publication_metadata.py",
        "slopsearx/routing.py",
        "slopsearx/jev.py",
        "slopsearx/rerank.py",
        "slopsearx/stats.py",
        "slopsearx/audit.py",
        "slopsearx/metrics.py",
        "slopsearx/config.py",
        "slopsearx/logging.py",
        "tests/test_service.py",
        "tests/test_concurrency.py",
        "tests/test_interactive_deadline.py",
    ]
    return {
        "schema_version": 1,
        "plan_id": "F14",
        "python": sys.version.split()[0],
        "provenance": (
            "Actual Python source execution with synthetic local inputs; no upstream recordings or provider calls. "
            "Native runtime tests separately exercise private loopback HTTP and Valkey."
        ),
        "source_commit": subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip(),
        "source_files": {
            p: {
                "sha256": hashlib.sha256((root / p).read_bytes()).hexdigest(),
                "git_blob": subprocess.check_output(
                    ["git", "-C", str(root), "hash-object", "--", p], text=True
                ).strip(),
            }
            for p in sources
        },
        "normalizations": ["query_id", "response_time_ms"],
        "cases": cases,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = asyncio.run(capture(args.source_root.resolve()))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
