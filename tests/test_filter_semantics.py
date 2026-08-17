"""Deterministic tests for issue 187 — search filter semantics.

Pins the unified enforcement model for ``language``, ``time_range``,
``safesearch``, and ``pagination`` across generic, targeted, specialist, and
research paths:

- one closed status vocabulary and schema,
- "enforced" is never derived from a mere ``supported_filters`` (accepts
  parameter) declaration — only from an audited ``enforced_filters`` layer,
- strict SafeSearch fails closed before dispatch when the selected scope
  cannot satisfy it,
- ``enforced_by`` names the enforcing engines *and* the enforcement layer
  (``upstream:<engine>`` / ``local:<engine>``),
- cached / targeted / specialist / research searches preserve the same
  enforcement truth.

No live network and no Valkey — every engine is a deterministic fixture.
"""

from __future__ import annotations

import datetime as _dt
import time
from typing import Any

import pytest

import engines  # noqa: F401 — triggers @register_engine to populate registry
from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult
from slopsearx.capabilities import CapabilityCatalog, MCPPolicy, load_mcp_policy
from slopsearx.config import load_config
from slopsearx.filters import (
    ENFORCEMENT_LAYERS,
    ENFORCEMENT_STATUSES,
    engine_filter_layer,
    filter_results_by_time_range,
    published_date_within,
    resolve_filter_enforcement,
    time_range_window,
)
from slopsearx.mcp import tools as t
from slopsearx.mcp.state import McpState, set_state
from slopsearx.research import ResearchJob, ResearchJobRunner, ResearchJobStore, ResearchQuery, generate_job_id
from slopsearx.service import AppContext, SearchRequest, SearchService
from slopsearx.snapshot import SnapshotStore


class _FakeStore:
    """In-memory key-value store (SearchCache-like)."""

    def __init__(self) -> None:
        self.is_connected = True
        self._data: dict[str, dict[str, Any]] = {}

    async def get(self, key: str) -> dict[str, Any] | None:
        return self._data.get(key)

    async def set(self, key: str, value: dict[str, Any], ttl: int = 300) -> None:
        del ttl
        self._data[key] = value


class _MockEngine(EngineAdapter):
    """Parameterizable mock engine with a real registry name."""

    def __init__(
        self,
        name: str,
        status: EngineStatus = EngineStatus.OK,
        count: int = 3,
        categories: list[str] | None = None,
        enforced_filters: dict[str, str] | None = None,
        published_dates: list[str | None] | None = None,
    ) -> None:
        super().__init__()
        self.name = name
        self._status = status
        self._count = count
        self.categories = list(categories or ["general"])
        self.enforced_filters = enforced_filters or {}
        self._published_dates = published_dates or [None] * count
        self.calls = 0

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        self.calls += 1
        if self._status != EngineStatus.OK:
            return AdapterResponse(results=[], status=self._status, error_message="simulated failure", latency_ms=1.0)
        return AdapterResponse(
            results=[
                SearchResult(
                    url=f"https://{self.name}{i}.example",
                    title=f"{self.name} result {i}",
                    content=f"Content for {self.name} result {i}.",
                    engine=self.name,
                    published_date=self._published_dates[i] if i < len(self._published_dates) else None,
                )
                for i in range(self._count)
            ],
            status=EngineStatus.OK,
            latency_ms=2.0,
        )


def _make_engines(names: list[str], **kwargs: Any) -> dict[str, EngineAdapter]:
    return {name: _MockEngine(name=name, **kwargs) for name in names}


def _build_state(
    engine_names: list[str] | None = None,
    *,
    policy: MCPPolicy | None = None,
    engines_map: dict[str, EngineAdapter] | None = None,
) -> McpState:
    engine_names = engine_names or ["wikipedia", "brave", "duckduckgo"]
    engines_map = engines_map if engines_map is not None else _make_engines(engine_names)
    policy = policy or load_mcp_policy(config_path=None)

    ctx = AppContext(
        active_engines=engines_map,
        router=None,
        cache=_FakeStore(),
        tier1_engines=set(engine_names),
        sensitive_engines=policy.sensitive_engines,
    )
    catalog = CapabilityCatalog(config=load_config(), adapters=engines_map, sensitive_engines=policy.sensitive_engines)
    service = SearchService(ctx)
    store = _FakeStore()
    snapshots = SnapshotStore(store, ttl_seconds=policy.snapshot_ttl_seconds)
    job_store = ResearchJobStore(store)
    runner = ResearchJobRunner(service, job_store, snapshots, catalog, policy)
    return McpState(
        ctx=ctx,
        policy=policy,
        catalog=catalog,
        service=service,
        snapshots=snapshots,
        job_store=job_store,
        runner=runner,
        version="test",
    )


@pytest.fixture
def state() -> McpState:
    state_obj = _build_state()
    set_state(state_obj)
    yield state_obj
    set_state(None)


FIXED_KEYS = {"requested", "status", "reason", "enforced_by"}


def _assert_valid_entry(entry: dict[str, Any]) -> None:
    assert set(entry.keys()) == FIXED_KEYS, entry
    assert entry["status"] in ENFORCEMENT_STATUSES, entry
    assert isinstance(entry["reason"], str) and entry["reason"], entry
    assert isinstance(entry["enforced_by"], list), entry
    for token in entry["enforced_by"]:
        layer, _, name = token.partition(":")
        assert layer in ENFORCEMENT_LAYERS, token
        assert name, token


# ---------------------------------------------------------------------------
# Pure enforcement model
# ---------------------------------------------------------------------------


class TestEnforcementModel:
    def test_supported_filters_never_yields_enforced(self) -> None:
        """Accepting a parameter is a capability hint, never an enforcement claim."""
        adapter = _MockEngine("brave")
        adapter.supported_filters = {"time_range": True}  # noqa: SLF001
        assert engine_filter_layer(adapter, "time_range") is None

    def test_enforced_filters_classifies_layer(self) -> None:
        upstream = _MockEngine("brave", enforced_filters={"time_range": "upstream"})
        local = _MockEngine("arxiv", enforced_filters={"time_range": "local"})
        assert engine_filter_layer(upstream, "time_range") == "upstream"
        assert engine_filter_layer(local, "time_range") == "local"
        assert engine_filter_layer(_MockEngine("brave"), "time_range") is None

    def test_invalid_layer_is_not_enforcement(self) -> None:
        adapter = _MockEngine("brave", enforced_filters={"time_range": "maybe"})
        assert engine_filter_layer(adapter, "time_range") is None

    def test_pagination_model_supports_mixed_layers(self) -> None:
        """Pagination uses the same vocabulary and layer-qualified evidence."""
        adapters = {
            "github": _MockEngine("github", enforced_filters={"pagination": "upstream"}),
            "brave": _MockEngine("brave"),
        }
        entry = resolve_filter_enforcement(["github", "brave"], "pagination", 2, adapters)
        assert entry["status"] == "partially_enforced"
        assert entry["requested"] == 2
        assert entry["enforced_by"] == ["upstream:github"]

    def test_time_range_window_and_date_helpers(self) -> None:
        today = _dt.date(2026, 1, 15)
        start, end = time_range_window("week", now=today)
        assert (end - start).days == 7
        assert published_date_within("2026-01-14", "week", now=today) is True
        assert published_date_within("2025-12-01", "week", now=today) is False
        assert published_date_within(None, "week", now=today) is None
        assert published_date_within("garbage", "week", now=today) is None

    def test_filter_results_by_time_range_keeps_only_in_window(self) -> None:
        today = _dt.date.today()
        results = [
            SearchResult(url="https://a", title="fresh", content="", engine="e", published_date=today.isoformat()),
            SearchResult(
                url="https://b",
                title="stale",
                content="",
                engine="e",
                published_date=(today - _dt.timedelta(days=400)).isoformat(),
            ),
            SearchResult(url="https://c", title="undated", content="", engine="e", published_date=None),
        ]
        kept = filter_results_by_time_range(results, "day")
        assert [r.title for r in kept] == ["fresh"]


# ---------------------------------------------------------------------------
# Strict SafeSearch — scope-aware fail closed
# ---------------------------------------------------------------------------


class TestStrictSafesearch:
    async def test_strict_rejected_before_dispatch_with_report(self, state: McpState) -> None:
        """Mandatory constraint fails closed before any engine dispatch."""
        for engine in state.ctx.active_engines.values():
            assert engine.calls == 0

        result = await t.slopsearx_search("hello", safesearch="strict")

        assert result["error"]["code"] == "safesearch_unenforced"
        assert result["error"]["field"] == "safesearch"
        assert result["error"]["selected_engines"]
        assert "results" not in result
        entry = result["enforcement"]["safesearch"]
        _assert_valid_entry(entry)
        assert entry["status"] == "rejected"
        assert entry["requested"] == "strict"
        assert entry["enforced_by"] == []
        for engine in state.ctx.active_engines.values():
            assert engine.calls == 0

    async def test_strict_targeted_rejected_before_dispatch(self, state: McpState) -> None:
        result = await t.slopsearx_search_targeted("hello", engines=["brave"], safesearch="strict")
        assert result["error"]["code"] == "safesearch_unenforced"
        assert result["enforcement"]["safesearch"]["status"] == "rejected"
        assert state.ctx.active_engines["brave"].calls == 0

    async def test_strict_enforced_when_every_selected_engine_satisfies(self, state: McpState) -> None:
        state.ctx.active_engines = {
            "brave": _MockEngine("brave", enforced_filters={"safesearch": "upstream"}),
            "duckduckgo": _MockEngine("duckduckgo", enforced_filters={"safesearch": "upstream"}),
        }
        result = await t.slopsearx_search("hello", safesearch="strict")

        assert "error" not in result
        entry = result["enforcement"]["safesearch"]
        assert entry["status"] == "enforced"
        assert entry["requested"] == "strict"
        assert set(entry["enforced_by"]) == {"upstream:brave", "upstream:duckduckgo"}
        assert state.ctx.active_engines["brave"].calls == 1
        assert state.ctx.active_engines["duckduckgo"].calls == 1

    async def test_strict_rejected_when_only_subset_satisfies(self, state: McpState) -> None:
        """Strict is all-or-nothing: a single non-enforcing engine fails closed."""
        state.ctx.active_engines = {
            "brave": _MockEngine("brave", enforced_filters={"safesearch": "upstream"}),
            "duckduckgo": _MockEngine("duckduckgo"),
        }
        result = await t.slopsearx_search_targeted("hello", engines=["brave", "duckduckgo"], safesearch="strict")
        assert result["error"]["code"] == "safesearch_unenforced"
        assert result["enforcement"]["safesearch"]["status"] == "rejected"
        assert state.ctx.active_engines["brave"].calls == 0
        assert state.ctx.active_engines["duckduckgo"].calls == 0


# ---------------------------------------------------------------------------
# Enforcement layer + local post-filter
# ---------------------------------------------------------------------------


class TestEnforcementLayer:
    async def test_upstream_layer_reported(self, state: McpState) -> None:
        state.ctx.active_engines = {"arxiv": _MockEngine("arxiv", enforced_filters={"time_range": "upstream"})}
        result = await t.slopsearx_search_targeted("hello", engines=["arxiv"], time_range="week")
        entry = result["enforcement"]["time_range"]
        assert entry["status"] == "enforced"
        assert entry["enforced_by"] == ["upstream:arxiv"]

    async def test_local_layer_reported(self, state: McpState) -> None:
        state.ctx.active_engines = {"arxiv": _MockEngine("arxiv", enforced_filters={"time_range": "local"})}
        result = await t.slopsearx_search_targeted("hello", engines=["arxiv"], time_range="week")
        entry = result["enforcement"]["time_range"]
        assert entry["status"] == "enforced"
        assert entry["enforced_by"] == ["local:arxiv"]

    async def test_mixed_layers_reported(self, state: McpState) -> None:
        state.ctx.active_engines = {
            "arxiv": _MockEngine("arxiv", enforced_filters={"time_range": "local"}),
            "brave": _MockEngine("brave", enforced_filters={"time_range": "upstream"}),
        }
        result = await t.slopsearx_search_targeted("hello", engines=["arxiv", "brave"], time_range="week")
        entry = result["enforcement"]["time_range"]
        assert entry["status"] == "enforced"
        assert set(entry["enforced_by"]) == {"local:arxiv", "upstream:brave"}


class TestLocalTimeRangePostFilter:
    async def test_local_time_range_post_filters_results(self) -> None:
        today = _dt.date.today()
        engine = _MockEngine(
            "localeng",
            count=3,
            enforced_filters={"time_range": "local"},
            published_dates=[
                today.isoformat(),
                (today - _dt.timedelta(days=400)).isoformat(),
                None,
            ],
        )
        service = SearchService(AppContext(active_engines={"localeng": engine}, router=None))

        response = await service.search(SearchRequest(query="q", time_range="day"))

        assert [r.title for r in response.results] == ["localeng result 0"]
        assert engine.calls == 1

    async def test_non_local_engine_is_not_post_filtered(self) -> None:
        today = _dt.date.today()
        engine = _MockEngine(
            "localeng",
            count=2,
            published_dates=[today.isoformat(), (today - _dt.timedelta(days=400)).isoformat()],
        )
        service = SearchService(AppContext(active_engines={"localeng": engine}, router=None))

        response = await service.search(SearchRequest(query="q", time_range="day"))

        # No local declaration → the stale result is retained.
        assert len(response.results) == 2


# ---------------------------------------------------------------------------
# Consistent truth across cached / targeted / specialist / research
# ---------------------------------------------------------------------------


class TestEnforcementTruthAcrossPaths:
    async def test_cached_search_preserves_enforcement(self, state: McpState) -> None:
        first = await t.slopsearx_search("hello", language="de", time_range="week")
        assert first["meta"]["cached"] is False

        second = await t.slopsearx_search("hello", language="de", time_range="week")
        assert second["meta"]["cached"] is True
        assert second["enforcement"] == first["enforcement"]

    async def test_targeted_and_generic_share_schema(self, state: McpState) -> None:
        generic = await t.slopsearx_search("hello", language="de", time_range="week", safesearch="moderate")
        targeted = await t.slopsearx_search_targeted(
            "hello", engines=["brave"], language="de", time_range="week", safesearch="moderate"
        )
        for result in (generic, targeted):
            assert set(result["enforcement"]) == {"language", "time_range", "safesearch"}
            for entry in result["enforcement"].values():
                _assert_valid_entry(entry)

    async def test_specialist_date_filters_are_unsupported(self, state: McpState) -> None:
        state.policy.enabled_tools["science"] = True
        state.ctx.active_engines = _make_engines(["arxiv"])
        result = await t.slopsearx_search_science(
            "transformers", source_types=["papers"], date_from="2024-01-01", date_to="2024-12-31"
        )
        assert "error" not in result
        report = result["enforcement"]
        assert set(report) == {"date_from", "date_to"}
        for entry in report.values():
            _assert_valid_entry(entry)
            assert entry["status"] == "unsupported"

    async def test_research_preserves_enforcement_truth(self, state: McpState) -> None:
        job = ResearchJob(
            job_id=generate_job_id(),
            question="q",
            strategy="fresh",
            deadline=time.time() + 3600,
            queries=[
                ResearchQuery(index=0, query="q", intent="web", engines=["wikipedia", "brave"], time_range="month"),
                ResearchQuery(index=1, query="q", intent="news", engines=["duckduckgo"], time_range="day"),
            ],
        )
        await state.job_store.save(job)
        await state.runner.run_pending(job)

        finished = await state.job_store.load(job.job_id)
        assert finished is not None
        assert finished.state == "succeeded"
        for query in finished.queries:
            entry = query.enforcement["time_range"]
            _assert_valid_entry(entry)
            assert entry["status"] == "unsupported"
            # The research report matches the shared resolver for the same scope.
            expected = t._core_filter_enforcement(
                state,
                query.engines,
                language="en",
                time_range=query.time_range,
                safesearch="off",
            )["time_range"]
            assert entry == expected
