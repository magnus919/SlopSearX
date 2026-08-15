"""Tests for the structured filter-enforcement report (feature: filter-enforcement-report).

Covers VAL-SEARCH-008, VAL-FILTER-001..007, VAL-FILTER-016, VAL-FILTER-017,
VAL-FILTER-024, VAL-TARGET-009, VAL-SPEC-009, VAL-SPEC-010: every search tool
returns a machine-readable top-level ``enforcement`` object keyed by filter
name, each entry carrying ``{requested, status, reason, enforced_by}`` with a
closed status vocabulary (enforced|partially_enforced|unsupported|rejected).
No adapter consumes language/time_range/safesearch today, so those report
``unsupported``; strict SafeSearch fails closed as ``rejected``.
"""

from __future__ import annotations

from typing import Any

import pytest

import engines  # noqa: F401 — triggers @register_engine to populate registry
from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult
from slopsearx.capabilities import CapabilityCatalog, MCPPolicy, load_mcp_policy
from slopsearx.config import load_config
from slopsearx.mcp import tools as t
from slopsearx.mcp.state import McpState, set_state
from slopsearx.research import ResearchJobRunner, ResearchJobStore
from slopsearx.service import AppContext, SearchService
from slopsearx.snapshot import SnapshotStore

# The five search-capable tools, as (name, factory).
SEARCH_TOOLS = ("slopsearx_search", "slopsearx_search_targeted", "slopsearx_search_jobs")


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
        supported_filters: dict[str, bool] | None = None,
    ) -> None:
        super().__init__()
        self.name = name
        self._status = status
        self._count = count
        self.categories = list(categories or ["general"])
        self.supported_filters = supported_filters or {}
        self.calls = 0

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        self.calls += 1
        if self._status != EngineStatus.OK:
            return AdapterResponse(results=[], status=self._status, error_message="simulated failure", latency_ms=1.0)
        return AdapterResponse(
            results=[
                SearchResult(
                    url=f"https://{self.name}{i}.com",
                    title=f"{self.name} result {i}",
                    content=f"Content for {self.name} result {i}.",
                    engine=self.name,
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
) -> McpState:
    engine_names = engine_names or ["wikipedia", "brave", "duckduckgo"]
    engines_map = _make_engines(engine_names)
    policy = policy or load_mcp_policy(config_path=None)

    ctx = AppContext(
        active_engines=engines_map,
        router=None,
        cache=_FakeStore(),
        tier1_engines=set(engine_names),
        sensitive_engines=policy.sensitive_engines,
    )
    catalog = CapabilityCatalog(config=load_config())
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


ENFORCEMENT_STATUSES = {"enforced", "partially_enforced", "unsupported", "rejected"}
FIXED_KEYS = {"requested", "status", "reason", "enforced_by"}


def _assert_valid_entry(entry: dict[str, Any]) -> None:
    """Assert one entry conforms to the fixed schema (VAL-FILTER-007)."""
    assert set(entry.keys()) == FIXED_KEYS, entry
    assert entry["status"] in ENFORCEMENT_STATUSES, entry
    assert isinstance(entry["reason"], str) and entry["reason"], entry
    assert isinstance(entry["enforced_by"], list), entry


def _assert_report(report: dict[str, Any]) -> None:
    assert isinstance(report, dict), report
    for entry in report.values():
        _assert_valid_entry(entry)


def _enable_jobs(state: McpState) -> None:
    state.policy.enabled_tools["jobs"] = True
    state.ctx.active_engines = _make_engines(["greenhouse", "ashby", "lever"])


def _enable_science(state: McpState) -> None:
    state.policy.enabled_tools["science"] = True
    state.ctx.active_engines = _make_engines(["arxiv", "semanticscholar", "openalex"])


def _enable_security(state: McpState) -> None:
    state.policy.enabled_tools["security"] = True
    state.ctx.active_engines = _make_engines(["cve", "nvd", "epss", "vulncheck", "exploitdb"])


# ---------------------------------------------------------------------------
# VAL-SEARCH-008 / VAL-FILTER-002 / VAL-FILTER-003 / VAL-FILTER-004
# ---------------------------------------------------------------------------


class TestCoreFilters:
    async def test_enforcement_object_present_with_language_time_range(self, state: McpState) -> None:
        """VAL-SEARCH-008 — language/time_range appear in the top-level enforcement object."""
        result = await t.slopsearx_search("hello", language="de", time_range="week")

        assert "error" not in result
        report = result["enforcement"]
        _assert_report(report)
        assert "language" in report
        assert "time_range" in report
        assert report["language"]["requested"] == "de"
        assert report["time_range"]["requested"] == "week"

    async def test_time_range_unsupported_with_reason(self, state: McpState) -> None:
        """VAL-FILTER-002 — time_range reports unsupported with a reason."""
        result = await t.slopsearx_search("hello", time_range="month")
        entry = result["enforcement"]["time_range"]
        assert entry["status"] == "unsupported"
        assert entry["reason"]

    async def test_language_unsupported_with_reason(self, state: McpState) -> None:
        """VAL-FILTER-003 — non-default language reports unsupported with a reason."""
        result = await t.slopsearx_search("hello", language="de")
        entry = result["enforcement"]["language"]
        assert entry["status"] == "unsupported"
        assert entry["reason"]

    async def test_moderate_safesearch_unsupported_not_enforced(self, state: McpState) -> None:
        """VAL-FILTER-004 — moderate safesearch reports unsupported, not enforced."""
        result = await t.slopsearx_search("hello", safesearch="moderate")
        entry = result["enforcement"]["safesearch"]
        assert entry["status"] == "unsupported"
        assert entry["requested"] == "moderate"
        assert entry["status"] != "enforced"

    async def test_strict_safesearch_rejected_and_no_dispatch(self, state: McpState) -> None:
        """VAL-FILTER-005 — strict safesearch fails closed; no engine is dispatched."""
        for engine in state.ctx.active_engines.values():
            assert engine.calls == 0
        result = await t.slopsearx_search("hello", safesearch="strict")

        assert "error" in result
        assert result["error"]["code"] == "safesearch_unenforced"
        assert result["error"]["message"]
        assert "results" not in result
        assert "engine_outcomes" not in result
        for engine in state.ctx.active_engines.values():
            assert engine.calls == 0

    async def test_strict_safesearch_rejected_on_targeted(self, state: McpState) -> None:
        """VAL-FILTER-017 / VAL-TARGET-009 — strict safesearch fails closed on the targeted path too."""
        result = await t.slopsearx_search_targeted("hello", engines=["brave"], safesearch="strict")

        assert result["error"]["code"] == "safesearch_unenforced"
        assert "results" not in result
        assert state.ctx.active_engines["brave"].calls == 0

    async def test_unenforced_filters_never_report_enforced(self, state: McpState) -> None:
        """VAL-FILTER-006 — language/time_range/moderate-safesearch are never 'enforced'."""
        result = await t.slopsearx_search("hello", language="fr", time_range="year", safesearch="moderate")
        report = result["enforcement"]
        for key in ("language", "time_range", "safesearch"):
            assert report[key]["status"] != "enforced"

    async def test_closed_vocabulary_and_reason_present(self, state: McpState) -> None:
        """VAL-FILTER-007 — status is closed enum and every entry carries a reason."""
        result = await t.slopsearx_search("hello", language="es", time_range="day", safesearch="moderate")
        _assert_report(result["enforcement"])

    async def test_default_search_returns_enforcement_object(self, state: McpState) -> None:
        """VAL-FILTER-024 — a default search still returns the enforcement object."""
        result = await t.slopsearx_search("hello")
        assert "enforcement" in result
        _assert_report(result["enforcement"])


# ---------------------------------------------------------------------------
# VAL-FILTER-001 / VAL-FILTER-016 — identical shape on every search tool
# ---------------------------------------------------------------------------


class TestAllTools:
    async def test_all_five_tools_return_same_report_shape(self, state: McpState) -> None:
        """VAL-FILTER-001 / VAL-FILTER-016 — report shape identical across all five tools."""
        # Generic + targeted with the same core filters.
        generic = await t.slopsearx_search("hello", language="de", time_range="week", safesearch="moderate")
        targeted = await t.slopsearx_search_targeted(
            "hello", engines=["brave"], language="de", time_range="week", safesearch="moderate"
        )
        for result in (generic, targeted):
            assert "error" not in result
            _assert_report(result["enforcement"])
            assert set(result["enforcement"].keys()) == {"language", "time_range", "safesearch"}
            for key in ("language", "time_range", "safesearch"):
                _assert_valid_entry(result["enforcement"][key])

        # Jobs with filter-like params.
        _enable_jobs(state)
        jobs = await t.slopsearx_search_jobs("Acme", location="Berlin", employment_type="full_time")
        assert "error" not in jobs
        _assert_report(jobs["enforcement"])
        assert set(jobs["enforcement"].keys()) == {"location", "employment_type"}
        for entry in jobs["enforcement"].values():
            assert entry["status"] == "unsupported"
            assert entry["reason"]
            assert entry["status"] != "enforced"

        # Security with no filter-like params — still returns the enforcement key.
        _enable_security(state)
        security = await t.slopsearx_search_security("log4j", evidence_types=["vulnerability"])
        assert "error" not in security
        assert "enforcement" in security
        _assert_report(security["enforcement"])

        # Science with date filters.
        _enable_science(state)
        science = await t.slopsearx_search_science(
            "transformers", source_types=["papers"], date_from="2024-01-01", date_to="2024-12-31"
        )
        assert "error" not in science
        _assert_report(science["enforcement"])
        assert set(science["enforcement"].keys()) == {"date_from", "date_to"}
        for entry in science["enforcement"].values():
            assert entry["status"] == "unsupported"
            assert entry["reason"]
            assert entry["status"] != "enforced"


# ---------------------------------------------------------------------------
# VAL-SPEC-009 / VAL-SPEC-010 — specialist filter-like params
# ---------------------------------------------------------------------------


class TestSpecialistFilterParams:
    async def test_jobs_location_and_employment_type_unsupported(self, state: McpState) -> None:
        """VAL-SPEC-009 — jobs location/employment_type report unsupported, never applied."""
        _enable_jobs(state)
        result = await t.slopsearx_search_jobs("Acme", location="Berlin", employment_type="full_time")

        assert "error" not in result
        report = result["enforcement"]
        assert set(report.keys()) == {"location", "employment_type"}
        assert report["location"]["status"] == "unsupported"
        assert report["location"]["requested"] == "Berlin"
        assert report["location"]["reason"]
        assert report["employment_type"]["status"] == "unsupported"
        assert report["employment_type"]["requested"] == "full_time"
        assert report["employment_type"]["reason"]

    async def test_science_date_filters_unsupported(self, state: McpState) -> None:
        """VAL-SPEC-010 — science date_from/date_to report unsupported, never applied."""
        _enable_science(state)
        result = await t.slopsearx_search_science(
            "transformers", source_types=["papers"], date_from="2024-01-01", date_to="2024-12-31"
        )

        assert "error" not in result
        report = result["enforcement"]
        assert set(report.keys()) == {"date_from", "date_to"}
        assert report["date_from"]["status"] == "unsupported"
        assert report["date_to"]["status"] == "unsupported"
        for entry in report.values():
            assert entry["reason"]
            assert entry["status"] != "enforced"


# ---------------------------------------------------------------------------
# VAL-SEARCH-008 — status consistent with supported_filters (partially_enforced path)
# ---------------------------------------------------------------------------


class TestSupportedFiltersConsistency:
    async def test_partially_enforced_when_subset_supports(self, state: McpState) -> None:
        """VAL-SEARCH-008 — status is consistent with the catalog's supported_filters."""
        # One engine declares support for time_range, one does not → partially_enforced.
        state.ctx.active_engines = {
            "brave": _MockEngine("brave", supported_filters={"time_range": True}),
            "duckduckgo": _MockEngine("duckduckgo"),
        }
        result = await t.slopsearx_search_targeted(
            "hello", engines=["brave", "duckduckgo"], time_range="week"
        )
        assert "error" not in result
        entry = result["enforcement"]["time_range"]
        assert entry["status"] == "partially_enforced"
        assert entry["enforced_by"] == ["brave"]

    async def test_enforced_when_all_selected_support(self, state: McpState) -> None:
        """VAL-SEARCH-008 — enforced when every selected engine supports the filter."""
        state.ctx.active_engines = {
            "brave": _MockEngine("brave", supported_filters={"time_range": True}),
            "duckduckgo": _MockEngine("duckduckgo", supported_filters={"time_range": True}),
        }
        result = await t.slopsearx_search_targeted("hello", engines=["brave", "duckduckgo"], time_range="week")
        assert "error" not in result
        entry = result["enforcement"]["time_range"]
        assert entry["status"] == "enforced"
        assert set(entry["enforced_by"]) == {"brave", "duckduckgo"}
