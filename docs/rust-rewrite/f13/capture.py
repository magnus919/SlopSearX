"""Temporary offline source oracle; G08 retires executable after migration."""

import argparse
import asyncio
import copy
import datetime
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path
from unittest.mock import patch

parser = argparse.ArgumentParser()
parser.add_argument("--source-root", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--reverse-record", type=Path)
args = parser.parse_args()
sys.path.insert(0, str(args.source_root))
from slopsearx.adapter import SearchResult  # noqa: E402 - explicit pinned source root
from slopsearx.cache import SearchCache, _answer_cache_key, _ttl_for_query, cache_key  # noqa: E402
from slopsearx.service import (  # noqa: E402
    ScopeDecision,
    SearchRequest,
    SearchResponse,
    SearchService,
    search_response_from_payload,
    search_response_to_payload,
)
from slopsearx.snapshot import SnapshotStore  # noqa: E402

SOURCES = [
    "slopsearx/cache.py",
    "slopsearx/snapshot.py",
    "slopsearx/service.py",
    "slopsearx/filters.py",
    "slopsearx/artifacts.py",
    "slopsearx/payload.py",
    "tests/test_cache.py",
    "tests/test_mcp_snapshots.py",
    "tests/test_cache_integration.py",
]
PINS = {
    "slopsearx/cache.py": {
        "git_blob": "354dbc179c4ca6ebc4ac9f85dd66a40d07ba3c09",
        "sha256": "88155d2060f19bcb7740c9498f8f82378e26c4c60ac4d013bee9913eb333c089",
    },
    "slopsearx/snapshot.py": {
        "git_blob": "fe0e62b9872b3d01cef6a8f843d9720f2cd50a42",
        "sha256": "85c88b483635003a4a395f40cf48eb83a564b8dd3965ed8a139ba65c1ea88377",
    },
    "slopsearx/service.py": {
        "git_blob": "8b45ce71efc3c7cc4a72b962968e146a63e135be",
        "sha256": "aed455ccbefcfee5ffcf099876dc7230cead27527313269edc0f69b504c20922",
    },
    "slopsearx/filters.py": {
        "git_blob": "e1a1cac35c5fcbbf2e6fb7728241953aa8047669",
        "sha256": "f0edd2f633cc9a42c49660ca74b225732cbc82937a15ad58a5b0dbeb76e103a9",
    },
    "slopsearx/artifacts.py": {
        "git_blob": "2cea4d16eb901b4c0ac0a2f5c0795add57f1f869",
        "sha256": "61aad828f61008623cc23d57b3a51c2c40e591ebfeece8c4065ec9d5a7470678",
    },
    "slopsearx/payload.py": {
        "git_blob": "efa149d48bae401f7a0a9df6e50c8c614adecbb5",
        "sha256": "a44a95655566f9fadaf62da1bab8c975f71f3ead6c3cc57f48c411a9c093c40d",
    },
    "tests/test_cache.py": {
        "git_blob": "8830f11cd664d68dd169c4e6deaba48b9fc1106c",
        "sha256": "bb3fec26f9a49c70c44ffbe0995488b9a90c03e66cd8bf51f9f6d347ee503f58",
    },
    "tests/test_mcp_snapshots.py": {
        "git_blob": "783f710689c16cdf12d51cc443d1ad30efb87b0c",
        "sha256": "d0eef9212e77a2365342e7303f62e33f27d855f011dae81c0109288088d4e698",
    },
    "tests/test_cache_integration.py": {
        "git_blob": "f4b7eb88f5693f2d1dfcc76de48de1e5d8c2b3c7",
        "sha256": "44e2809b7f3f73421441b33f0675e530cff91d579357580363c5600bcb2bb1d7",
    },
}


class Store:
    is_connected = True

    def __init__(self):
        self.values = {}
        self.ttls = []

    async def get(self, key):
        return copy.deepcopy(self.values.get(key))

    async def set(self, key, value, ttl=300):
        self.values[key] = copy.deepcopy(value)
        self.ttls.append(ttl)


async def capture():
    observations = []

    def add(kind, name, inputs, expected):
        observations.append({"kind": kind, "id": name, "input": inputs, "expected": expected})

    for i, query in enumerate(
        ["q", " q ", "\u001cq\u001f", "Q", "C++", "C#", "café", "😀", "a%20b", "a b", "a|b", "a:b", ""]
    ):
        add("key", f"query-{i}", {"query": query}, cache_key(query))
    for name, fields in [
        ("scope-none", {}),
        ("scope-empty", {"engines": [], "categories": []}),
        ("scope-sort", {"engines": ["b", "a"], "categories": ["z", "a"]}),
        ("scope-sorted", {"engines": ["a", "b"], "categories": ["a", "z"]}),
        ("scope-duplicates", {"engines": ["a", "a"]}),
        ("scope-single", {"engines": ["a"]}),
        ("language", {"language": "fr"}),
        ("safe", {"safesearch": 2}),
        ("page", {"pageno": 2}),
        ("media", {"media_type": "image"}),
        ("absolute-date", {"date_from": "2026-01-01", "date_to": "2026-01-31"}),
        ("relative-day", {"time_range": "day", "time_range_anchor": "2026-03-01"}),
        ("relative-next-day", {"time_range": "day", "time_range_anchor": "2026-03-02"}),
        ("relative-month", {"time_range": "month", "time_range_anchor": "2026-03-01"}),
        ("invalid-range", {"time_range": "invalid", "time_range_anchor": "2026-03-01"}),
        ("empty-range", {"time_range": ""}),
    ]:
        actual = dict(fields)
        if "time_range_anchor" in actual:
            actual["time_range_anchor"] = datetime.date.fromisoformat(actual["time_range_anchor"])
        add("key", name, {"query": "q", **fields}, cache_key("q", **actual))
    for i, q in enumerate(["q", " q ", "Q", "café", "a%20b", "a b"]):
        add("answer-key", f"answer-{i}", {"query": q}, _answer_cache_key(q))
    for i, (normal, partial_ttl, categories, partial) in enumerate(
        [
            ("3600", "30", [], False),
            ("3600", "30", ["news"], False),
            ("3600", "30", ["notnews"], False),
            ("120", "30", ["NEWS"], False),
            ("3600", "30", [], True),
            ("10", "30", [], True),
            ("3600", "300", ["news"], True),
            ("3_600", "3_0", [], True),
            (" ٣٦٠٠ ", "٣٠", [], False),
            ("0", "30", [], False),
            ("-1", "30", [], False),
            ("1.0", "30", [], False),
            ("3600", "301", [], True),
            ("3600", "0", [], True),
            ("3600", "0", [], False),
        ]
    ):
        env = {"SEARCH_CACHE_TTL_SECONDS": normal, "SEARCH_CACHE_PARTIAL_TTL_SECONDS": partial_ttl}
        with patch.dict(os.environ, env):
            try:
                expected = {"ttl": _ttl_for_query(categories, partial=partial)}
            except ValueError:
                expected = {"error": "ValueError"}
        add(
            "ttl",
            f"ttl-{i}",
            {"normal": normal, "partial_ttl": partial_ttl, "categories": categories, "partial": partial},
            expected,
        )
    canonical = SearchResponse(
        query="q",
        query_id="ssx-q",
        results=[
            SearchResult(
                url=f"https://example.com/{i}",
                title=f"Title{i}",
                content="full content",
                engine="a",
                engines={"b", "a"},
                payload={"data": {"nested": ["original"]}},
            )
            for i in range(3)
        ],
        scope=ScopeDecision(selected_engines=["a"]),
        engine_outcomes=[],
        suggestions=["suggest"],
        answers=[{"nested": {"value": "original"}}],
        infoboxes=[{"urls": [{"title": "original"}]}],
        empty_engines=[["b", "empty"]],
    )
    payload = search_response_to_payload(canonical)
    for i, (include, maximum) in enumerate(
        [
            (["results"], 1),
            (["results", "suggestions"], 2),
            (["results", "suggestions", "engine_status", "diagnostics"], None),
            ([], 0),
            ([], -1),
            ([], 10),
        ]
    ):
        response = search_response_from_payload(copy.deepcopy(payload))
        view = SearchService._view_for_request(SearchRequest("q", include=set(include), max_results=maximum), response)
        add(
            "view",
            f"view-{i}",
            {"canonical": payload, "include": include, "max_results": maximum},
            search_response_to_payload(view),
        )
    store = Store()
    snapshots = SnapshotStore(store, tenant="tenant-a", ttl_seconds=10)

    class UUID:
        hex = "0123456789abcdef0123456789abcdef"

    with (
        patch("slopsearx.snapshot.time.time", return_value=100.0),
        patch("slopsearx.snapshot.uuid.uuid4", return_value=UUID()),
    ):
        sid = await snapshots.create("q", "ssx-q", canonical.results, canonical.scope)
    stored = copy.deepcopy(store.values[f"mcp:snapshot:tenant-a:{sid}"])
    add(
        "snapshot-create",
        "snapshot-create",
        {"tenant": "tenant-a", "ttl": 10, "now": 100.0, "id": sid},
        {
            "payload": stored,
            "physical_ttl": store.ttls[0],
            "key": snapshots._key(sid),
            "result_id": snapshots.result_id(sid, 2),
        },
    )
    variants = [
        ("live", {}, 109.0),
        ("equal", {}, 110.0),
        ("expired", {}, 110.001),
        ("other-tenant", {"tenant": "tenant-b"}, 109),
        ("nan-expiry", {"expires_at": "NaN"}, 109),
        ("inf-created", {"created_at": "Infinity"}, 109),
        ("bad-results", {"results": [1]}, 109),
        ("bad-scope", {"scope": [1]}, 109),
        ("bad-total", {"total": "bad"}, 109),
        ("legacy", {"expires_at": None}, 10000),
        ("legacy-engines", {"total": 1, "results": [{"url": "u", "engine": "a", "engines": "{'b', 'a'}"}]}, 109),
    ]
    for name, changes, now in variants:
        raw = copy.deepcopy(stored)
        raw.update(changes)
        if name == "legacy":
            raw.pop("expires_at")
            raw.pop("lineage")
            raw.pop("ranking_explanation")
            raw.pop("created_at")
        store.values[snapshots._key(sid)] = raw
        with patch("slopsearx.snapshot.time.time", return_value=now):
            result = await snapshots.read(sid)
        value = {
            "state": "unavailable"
            if result.unavailable
            else "expired"
            if result.expired
            else "found"
            if result.snapshot
            else "missing"
        }
        if result.expired:
            value["expires_at"] = result.expires_at
        if result.snapshot:
            s = result.snapshot
            value.update(
                {
                    "id": s.snapshot_id,
                    "total": s.total,
                    "tenant": s.tenant,
                    "created_at": s.created_at,
                    "expires_at": s.expires_at,
                    "ranking": s.ranking_explanation,
                    "lineage_available": s.lineage_available,
                    "results": [sorted(r.engines) for r in s.results],
                }
            )
        add("snapshot-read", name, {"payload": raw, "tenant": "tenant-a", "now": now, "id": sid}, value)

    async def lost_read(key):
        return None

    store.get = lost_read
    result = await snapshots.read(sid)
    add("source-quirk", "connected-read-loss", {}, {"unavailable": result.unavailable, "snapshot": None})

    class BrokenClient:
        async def setex(self, *unused):
            raise ConnectionError("synthetic")

    cache = SearchCache("")
    cache._connected = True
    cache._client = BrokenClient()
    with (
        patch("slopsearx.snapshot.uuid.uuid4", return_value=UUID()),
        patch("slopsearx.snapshot.time.time", return_value=100),
    ):
        issued = await SnapshotStore(cache, tenant="tenant-a", ttl_seconds=10).create("q", "ssx-q", [], ScopeDecision())
    add("source-quirk", "failed-write-publishes-id", {}, {"id": issued})
    from types import SimpleNamespace

    from slopsearx.service import _routing_cache_digest, _scope_cache_key

    base_cap = {
        "enabled": True,
        "auth_class": "none",
        "auth_configured": True,
        "cost_class": "free",
        "circuit_open": False,
        "last_known_status": "ok",
        "last_known_status_stale": False,
    }
    for i, (change, bound) in enumerate(
        [
            ({}, 0),
            ({"auth_configured": False}, 0),
            ({"cost_class": "paid"}, 0),
            ({"circuit_open": True}, 0),
            ({"last_known_status": "error"}, 0),
            ({"last_known_status_stale": True}, 0),
            ({}, 1),
            ({"last_known_status": "error"}, 1),
            ({"last_known_status_stale": True}, 1),
            ({"enabled": False}, 0),
        ]
    ):
        cap = SimpleNamespace(**(base_cap | change))
        catalog = SimpleNamespace(get=lambda name: cap)
        budget = SimpleNamespace(max_engines=bound, max_cost_class="paid", coverage_target="balanced")
        ctx = SimpleNamespace(
            routing_budget=budget,
            catalog=catalog,
            active_engines={"a": None},
            sensitive_engines={"s"},
            tier1_engines={"a"},
            ranking_strategy="presence",
            rerank_provider=None,
        )
        digest = _routing_cache_digest(ctx)
        add("routing", f"routing-{i}", {"cap": base_cap | change, "max_engines": bound}, digest)
    for i, (timeout, suggestions) in enumerate([(None, True), (100, True), (100, False), (0, False)]):
        req = SearchRequest("q", interactive_timeout_ms=timeout, generate_suggestions=suggestions)
        add(
            "composed-key",
            f"composed-{i}",
            {
                "query": "q",
                "routing_digest": "routing:fixed",
                "interactive_timeout_ms": timeout,
                "generate_suggestions": suggestions,
            },
            _scope_cache_key(req, "routing:fixed"),
        )

    class Client:
        def __init__(self, data=None):
            self.data = data
            self.writes = []

        async def get(self, key):
            return self.data

        async def setex(self, *values):
            self.writes.append(values)

    for i, raw in enumerate(
        [None, "{}", "[]", "null", "true", "1", '"text"', "{", '{"_error":true,"timestamp":100}', json.dumps(payload)]
    ):
        c = SearchCache("")
        c._connected = True
        c._client = Client(raw)
        result = await c.get("key")
        add("cache-read", f"cache-read-{i}", {"raw": raw}, result)
    client = Client()
    c = SearchCache("")
    c._connected = True
    c._client = client
    with patch("slopsearx.cache.time.time", return_value=100):
        await c.set_error("negative")
    await c.set_answer(" q ", {"answer": "value"})
    add("cache-write", "separate-negative-answer", {}, {"writes": [list(x) for x in client.writes]})
    c._connected = False
    client.writes.clear()
    await c.set_error("negative")
    add("cache-write", "negative-disconnected", {}, {"writes": client.writes})
    for name, change in [
        ("mismatched-id", {"snapshot_id": "snap-other"}),
        ("omitted-id", None),
        ("omitted-query-id", {"query_id": None}),
        ("inconsistent-total", {"total": 99}),
        ("backward-expiry", {"expires_at": 90}),
    ]:
        raw = copy.deepcopy(stored)
        if name == "omitted-id":
            raw.pop("snapshot_id")
        elif name == "omitted-query-id":
            raw.pop("query_id")
        else:
            raw.update(change)
        legacy = Store()
        legacy.values[f"mcp:snapshot:tenant-a:{sid}"] = raw
        with patch("slopsearx.snapshot.time.time", return_value=109):
            result = await SnapshotStore(legacy, tenant="tenant-a", ttl_seconds=10).read(sid)
        expected = {"state": "expired" if result.expired else "found" if result.snapshot else "missing"}
        if result.snapshot:
            expected.update(
                {
                    "id": result.snapshot.snapshot_id,
                    "query_id": result.snapshot.query_id,
                    "total": result.snapshot.total,
                }
            )
        add("snapshot-integrity", name, {"payload": raw, "id": sid, "tenant": "tenant-a", "now": 109}, expected)
    for name, change in [
        ("null-total-empty", {"total": None, "results": []}),
        ("bad-jev-scores", {"scope": {"jev_scores": [1]}}),
    ]:
        raw = copy.deepcopy(stored)
        raw.update(change)
        legacy = Store()
        legacy.values[f"mcp:snapshot:tenant-a:{sid}"] = raw
        with patch("slopsearx.snapshot.time.time", return_value=109):
            result = await SnapshotStore(legacy, tenant="tenant-a", ttl_seconds=10).read(sid)
        add(
            "snapshot-read",
            name,
            {"payload": raw, "id": sid, "tenant": "tenant-a", "now": 109},
            {"state": "found" if result.snapshot else "missing"},
        )

    class NegativeCache:
        async def get(self, key):
            return {"_error": True, "timestamp": 100}

    service = object.__new__(SearchService)
    service._ctx = SimpleNamespace(cache=NegativeCache())
    service._ranking_explanation = "tier_then_cross_engine_presence"
    request = SearchRequest("current query", include={"results"}, max_results=1)
    with patch("slopsearx.service.generate_query_id", return_value="ssx-current"):
        response = await service._read_cache(request, "routing:fixed")
    add(
        "negative-view",
        "negative-view",
        {
            "query": "current query",
            "query_id": "ssx-current",
            "ranking_explanation": service._ranking_explanation,
            "include": ["results"],
            "max_results": 1,
        },
        {
            "payload": search_response_to_payload(response),
            "cached": response.cached,
            "cached_error": response.cached_error,
        },
    )
    for i, timestamp in enumerate([None, "bad", {}, "100", 100.5]):

        class MetadataCache:
            async def get(self, key):
                return {"_error": True, "timestamp": timestamp}

        service._ctx = SimpleNamespace(cache=MetadataCache())
        with patch("slopsearx.service.generate_query_id", return_value="ssx-current"):
            response = await service._read_cache(request, "routing:fixed")
        add(
            "negative-metadata",
            f"negative-metadata-{i}",
            {"timestamp": timestamp},
            {"cached": response.cached, "cached_error": response.cached_error},
        )
    for i, value in enumerate([[], None, "text", 1, True]):
        client = Client()
        c = SearchCache("")
        c._connected = True
        c._client = client
        await c.set_answer("q", value)
        client.data = client.writes[-1][2]
        read = await c.get_answer("q")
        add(
            "answer-nonobject",
            f"answer-nonobject-{i}",
            {"value": value},
            {"written": json.loads(client.writes[-1][2]), "read": read},
        )
    for count in [10000, 10001]:
        bounded_store = Store()
        bounded = SnapshotStore(bounded_store, tenant="tenant-a", ttl_seconds=10)
        results = [SearchResult(url="", title="", content="", engine="") for _ in range(count)]
        with (
            patch("slopsearx.snapshot.uuid.uuid4", return_value=UUID()),
            patch("slopsearx.snapshot.time.time", return_value=100),
        ):
            handle = await bounded.create("q", "ssx-q", results, ScopeDecision())
        with patch("slopsearx.snapshot.time.time", return_value=109):
            read = await bounded.read(handle)
        add(
            "storage-bounds",
            f"source-count-{count}",
            {"count": count},
            {"created": handle is not None, "read_total": read.snapshot.total, "physical_ttl": bounded_store.ttls[0]},
        )
    client = Client()
    c = SearchCache("")
    c._connected = True
    c._client = client
    large = {"answers": ["x" * (4 * 1024 * 1024 + 1)]}
    await c.set("search:v2:oversized", large)
    add(
        "storage-bounds",
        "source-bytes-over-4mib",
        {"value_bytes": 4 * 1024 * 1024 + 1},
        {"written": len(client.writes) == 1, "serialized_bytes": len(client.writes[0][2].encode())},
    )
    import dataclasses

    for name, scope in [
        (
            "legacy-scope-iterables",
            {
                "selected_engines": "npm",
                "resolved_categories": {"news": 1, "general": 2},
                "warnings": "ab",
                "jev_added_engines": "gh",
            },
        ),
        ("legacy-scope-unicode", {"selected_engines": "é😀", "warnings": {"warning-a": 1, "warning-b": 2}}),
        (
            "legacy-scope-falsy",
            {"selected_engines": False, "resolved_categories": 0, "warnings": None, "jev_added_engines": ""},
        ),
        ("legacy-scope-true", {"selected_engines": True}),
        ("legacy-scope-number", {"warnings": 1}),
    ]:
        raw = copy.deepcopy(stored)
        raw["scope"] = scope
        legacy = Store()
        legacy.values[f"mcp:snapshot:tenant-a:{sid}"] = raw
        with patch("slopsearx.snapshot.time.time", return_value=109):
            result = await SnapshotStore(legacy, tenant="tenant-a", ttl_seconds=10).read(sid)
        expected = {"state": "found" if result.snapshot else "missing"}
        if result.snapshot:
            expected["scope"] = dataclasses.asdict(result.snapshot.scope)
        add("snapshot-scope", name, {"payload": raw, "id": sid, "now": 109}, expected)
    for i, lineage in enumerate([None, False, 0, True, 7, "abc", {"x": 1}, [{"relation": "legacy"}, 7, "x"]]):
        raw = copy.deepcopy(stored)
        raw["lineage"] = lineage
        legacy = Store()
        legacy.values[f"mcp:snapshot:tenant-a:{sid}"] = raw
        with patch("slopsearx.snapshot.time.time", return_value=109):
            result = await SnapshotStore(legacy, tenant="tenant-a", ttl_seconds=10).read(sid)
        expected = {"state": "found" if result.snapshot else "missing"}
        if result.snapshot:
            expected["lineage"] = result.snapshot.lineage
            expected["lineage_available"] = result.snapshot.lineage_available
        add("snapshot-lineage", f"legacy-lineage-{i}", {"payload": raw, "id": sid, "now": 109}, expected)
    return observations


pins = {
    p: {
        "git_blob": subprocess.check_output(["git", "-C", str(args.source_root), "hash-object", "--", p])
        .decode()
        .strip(),
        "sha256": hashlib.sha256((args.source_root / p).read_bytes()).hexdigest(),
    }
    for p in SOURCES
}
if PINS and pins != PINS:
    raise RuntimeError("F13 accepted source drift; independent review required")
with patch.dict(os.environ, {}, clear=True):
    observations = asyncio.run(capture())
out = {
    "schema_version": 1,
    "plan_id": "F13",
    "source_commit": "35d97ccf8c9b2d8246373b2024cc8d7a457705a3",
    "source_files": pins,
    "cases": observations,
}
if args.reverse_record:
    import dataclasses

    from slopsearx.service import search_result_to_dict
    from slopsearx.snapshot import _snapshot_from_payload

    record_bytes = args.reverse_record.read_bytes()
    snapshot = _snapshot_from_payload(json.loads(record_bytes))
    out["reverse_read"] = {
        "native_record_sha256": hashlib.sha256(record_bytes).hexdigest(),
        "projection": {
            "snapshot_id": snapshot.snapshot_id,
            "query": snapshot.query,
            "query_id": snapshot.query_id,
            "tenant": snapshot.tenant,
            "total": snapshot.total,
            "created_at": snapshot.created_at,
            "expires_at": snapshot.expires_at,
            "ranking_explanation": snapshot.ranking_explanation,
            "lineage": snapshot.lineage,
            "lineage_available": snapshot.lineage_available,
            "scope": dataclasses.asdict(snapshot.scope),
            "results": [search_result_to_dict(r) for r in snapshot.results],
        },
    }
args.output.write_text(json.dumps(out, ensure_ascii=False, indent=2, allow_nan=False) + "\n")
print(len(observations), hashlib.sha256(args.output.read_bytes()).hexdigest())
