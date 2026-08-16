"""Tests for the MCP tool implementations (slopsearx.mcp.tools).

Tools are plain async callables tested directly (FastMCP-free), plus a
few integration checks through FastMCP's call_tool.
"""

from __future__ import annotations

import datetime as _dt
from typing import Any

import pytest

import engines  # noqa: F401 — triggers @register_engine to populate registry
from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult
from slopsearx.capabilities import CapabilityCatalog, MCPPolicy, load_mcp_policy
from slopsearx.config import load_config
from slopsearx.mcp import tools as t
from slopsearx.mcp.state import McpState, set_state
from slopsearx.research import ResearchJobRunner, ResearchJobStore
from slopsearx.service import EngineExclusion, EngineOutcome, ScopeDecision, SearchResponse, SearchService
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
    ) -> None:
        super().__init__()
        self.name = name
        self._status = status
        self._count = count
        self.categories = list(categories or ["general"])
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
    router: Any = None,
) -> McpState:
    engine_names = engine_names or ["wikipedia", "brave", "duckduckgo"]
    engines_map = _make_engines(engine_names)
    policy = policy or load_mcp_policy(config_path=None)
    from slopsearx.service import AppContext

    ctx = AppContext(
        active_engines=engines_map,
        router=router,
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


# ---------------------------------------------------------------------------
# slopsearx_search
# ---------------------------------------------------------------------------


class TestSearchTool:
    async def test_basic_search(self, state: McpState) -> None:
        result = await t.slopsearx_search("hello world")

        assert "error" not in result
        assert result["query"] == "hello world"
        assert result["results"]
        assert result["meta"]["cursor"] is not None
        assert result["meta"]["ranking"] == "tier_then_cross_engine_presence"
        assert result["scope"]["routing_reason"] == "all active engines"
        assert "suggestions" in result["meta"]

    async def test_unknown_intent_returns_alternatives(self, state: McpState) -> None:
        result = await t.slopsearx_search("hello", intent="bogus")

        assert result["error"]["code"] == "invalid_input"
        assert "valid_alternatives" in result["error"]

    async def test_empty_query(self, state: McpState) -> None:
        result = await t.slopsearx_search("")

        assert result["error"]["code"] == "invalid_input"
        assert result["error"]["field"] == "query"

    async def test_strict_safesearch_fails_closed(self, state: McpState) -> None:
        result = await t.slopsearx_search("hello", safesearch="strict")

        assert result["error"]["code"] == "safesearch_unenforced"

    async def test_moderate_safesearch_warns(self, state: McpState) -> None:
        result = await t.slopsearx_search("hello", safesearch="moderate")

        assert "error" not in result
        assert any("moderate safesearch" in w for w in result["warnings"])

    async def test_partial_failure_is_explicit(self, state: McpState) -> None:
        state.ctx.active_engines["wikipedia"] = _MockEngine("wikipedia", status=EngineStatus.TIMEOUT)
        result = await t.slopsearx_search("hello")

        assert result["meta"]["partial"] is True
        statuses = {o["engine"]: o["status"] for o in result["engine_outcomes"]}
        assert statuses["wikipedia"] == "timeout"

    async def test_all_failure_returns_error_envelope(self, state: McpState) -> None:
        state.ctx.active_engines = _make_engines(["wikipedia"], status=EngineStatus.ERROR)
        result = await t.slopsearx_search("hello")

        assert result["error"]["code"] == "all_engines_failed"
        assert result["error"]["query_id"].startswith("ssx-")
        assert "retry_guidance" in result["error"]

    async def test_intent_profile_resolution(self, state: McpState) -> None:
        state.ctx.active_engines = _make_engines(
            ["github", "pypi", "npm", "crates", "rubygems", "dockerhub", "repology", "stackexchange"]
        )
        result = await t.slopsearx_search("python package", intent="code")

        assert "error" not in result
        assert "github" in result["scope"]["selected_engines"]

    async def test_unsupported_filters_warn(self, state: McpState) -> None:
        result = await t.slopsearx_search("hello", language="fr", time_range="month")

        assert any("language 'fr'" in w for w in result["warnings"])
        assert any("time_range 'month'" in w for w in result["warnings"])

    async def test_max_results_bounds(self, state: McpState) -> None:
        state.policy.max_results = 4
        result = await t.slopsearx_search("hello", max_results=100)

        assert len(result["results"]) == 4

    async def test_include_engine_status_drives_engine_outcomes(self, state: McpState) -> None:
        """VAL-CORRECT-015 — engine_outcomes presence is driven by the current request's include."""
        # Warm the cache WITHOUT engine_status, then read WITH it.
        without = await t.slopsearx_search("hello", include=["results"])
        assert without["engine_outcomes"] == []

        with_status = await t.slopsearx_search("hello", include=["results", "engine_status"])
        assert with_status["meta"]["cached"] is True
        assert with_status["engine_outcomes"] != []

        # Reverse population order: warm WITH engine_status, read WITHOUT.
        with_status2 = await t.slopsearx_search("world", include=["results", "engine_status"])
        assert with_status2["engine_outcomes"] != []
        without2 = await t.slopsearx_search("world", include=["results"])
        assert without2["meta"]["cached"] is True
        assert without2["engine_outcomes"] == []

    async def test_max_results_return_own_count_no_stale_slice(self, state: McpState) -> None:
        """VAL-CORRECT-016 — each request's result count respects its own max_results."""
        small = await t.slopsearx_search("hello", max_results=2)
        assert len(small["results"]) == 2

        big = await t.slopsearx_search("hello", max_results=100)
        assert big["meta"]["cached"] is True
        assert len(big["results"]) == 9  # full captured fixture set, never capped at 2

        big2 = await t.slopsearx_search("world", max_results=100)
        assert len(big2["results"]) == 9
        small2 = await t.slopsearx_search("world", max_results=2)
        assert small2["meta"]["cached"] is True
        assert len(small2["results"]) == 2

    async def test_meta_total_and_has_more(self, state: McpState) -> None:
        """VAL-SEARCH-021 — meta.total is the full captured count; has_more reflects pagination."""
        bounded = await t.slopsearx_search("hello", max_results=2)
        assert bounded["meta"]["total"] == 9  # full fixture set (3 engines x 3, no URL overlap)
        assert bounded["meta"]["has_more"] is True

        full = await t.slopsearx_search("world", max_results=100)
        assert full["meta"]["total"] == 9
        assert full["meta"]["has_more"] is False

    async def test_empty_results_are_success_not_error(self, state: McpState) -> None:
        """VAL-SEARCH-014 — all engines ok but zero results is a success envelope with a cursor."""
        state.ctx.active_engines = _make_engines(["wikipedia"], count=0)
        result = await t.slopsearx_search("hello")

        assert "error" not in result
        assert result["results"] == []
        assert result["meta"]["cursor"] is not None
        assert result["meta"]["total"] == 0
        assert result["meta"]["has_more"] is False
        assert "empty_engines" in result

    async def test_scope_surfaces_excluded_engines(self, state: McpState) -> None:
        """VAL-SEARCH-009 — scope.excluded_engines is a machine-readable list with reasons."""
        state.ctx.active_engines = _make_engines(["wikipedia", "brave", "duckduckgo", "hibp"])
        result = await t.slopsearx_search("hello")
        assert "excluded_engines" in result["scope"]
        assert isinstance(result["scope"]["excluded_engines"], list)

    async def test_all_unresponsive_error_exposes_scope_and_outcomes(self, state: McpState) -> None:
        """VAL-SEARCH-015 — all-engines-failed error still exposes scope + per-engine outcomes."""
        state.ctx.active_engines = _make_engines(["wikipedia"], status=EngineStatus.ERROR)
        result = await t.slopsearx_search("hello")

        assert result["error"]["code"] == "all_engines_failed"
        assert "results" not in result
        assert result["error"]["scope"]["selected_engines"] == ["wikipedia"]
        assert result["error"]["scope"]["routing_reason"]
        assert result["error"]["engine_outcomes"][0]["engine"] == "wikipedia"
        assert result["error"]["engine_outcomes"][0]["status"] == "error"


# ---------------------------------------------------------------------------
# slopsearx_search_targeted
# ---------------------------------------------------------------------------


class TestEnvelopeEvidence:
    """Envelope recovery: answers/corrections/infoboxes/empty_engines/cached_error are surfaced.

    Covers VAL-SEARCH-007, the empty-engines recovery precondition, and the
    honest meta.cached_error signal. Verified at the envelope boundary with a
    populated SearchResponse so the surface behavior is pinned regardless of
    how the service populates the fields.
    """

    def _env(
        self,
        state: McpState,
        *,
        results: list[Any] | None = None,
        answers: list[dict[str, Any]] | None = None,
        corrections: list[str] | None = None,
        infoboxes: list[dict[str, Any]] | None = None,
        empty_engines: list[list[str]] | None = None,
        cached_error: bool = False,
        total: int = 0,
        cursor: str | None = "snap-x",
    ) -> dict[str, Any]:
        resp = SearchResponse(
            query="q",
            results=results or [],
            scope=ScopeDecision(selected_engines=["wikipedia"], routing_rule="explicit engine"),
            engine_outcomes=[EngineOutcome(engine="wikipedia", status="ok", result_count=0)],
            answers=answers or [],
            corrections=corrections or [],
            infoboxes=infoboxes or [],
            query_id="ssx-test",
            cached_error=cached_error,
            empty_engines=empty_engines or [],
        )
        return t._envelope(
            state,
            resp,
            requested_intent="auto",
            warnings=[],
            cursor=cursor,
            include_suggestions=False,
            total=total,
        )

    async def test_answers_corrections_infoboxes_surfaced(self, state: McpState) -> None:
        """VAL-SEARCH-007 — answers/corrections/infoboxes surface as typed fields verbatim."""
        env = self._env(
            state,
            answers=[{"answer": "42", "url": "https://x.example"}],
            corrections=["did you mean y"],
            infoboxes=[{"title": "info", "url": "https://i.example"}],
        )
        assert env["answers"] == [{"answer": "42", "url": "https://x.example"}]
        assert env["corrections"] == ["did you mean y"]
        assert env["infoboxes"] == [{"title": "info", "url": "https://i.example"}]

    async def test_answers_corrections_infoboxes_empty_when_absent(self, state: McpState) -> None:
        """VAL-SEARCH-007 — absent answers/corrections/infoboxes yield empty typed lists, never missing keys."""
        env = self._env(state)
        assert env["answers"] == []
        assert env["corrections"] == []
        assert env["infoboxes"] == []

    async def test_empty_engines_surfaced_machine_readable(self, state: McpState) -> None:
        """empty_engines from the response is surfaced as {engine, reason} objects, not dropped."""
        env = self._env(state, empty_engines=[["emptyscrape", "successful scrape returned no results"]])
        assert env["empty_engines"] == [{"engine": "emptyscrape", "reason": "successful scrape returned no results"}]

    async def test_empty_engines_empty_list_when_none(self, state: McpState) -> None:
        """empty_engines is always an empty list when the response carries none."""
        env = self._env(state)
        assert env["empty_engines"] == []

    async def test_meta_cached_error_surfaced(self, state: McpState) -> None:
        """cached_error is surfaced in meta honestly."""
        env = self._env(state, cached_error=True)
        assert env["meta"]["cached_error"] is True
        clean = self._env(state)
        assert clean["meta"]["cached_error"] is False

    async def test_meta_total_surfaced(self, state: McpState) -> None:
        """meta.total reports the aggregate captured count."""
        env = self._env(state, total=7)
        assert env["meta"]["total"] == 7

    async def test_scope_surfaces_excluded_engines_with_reasons(self, state: McpState) -> None:
        """VAL-SEARCH-009 — scope.excluded_engines carries engine + reason entries."""
        resp = SearchResponse(
            query="q",
            results=[],
            scope=ScopeDecision(
                selected_engines=["brave"],
                routing_rule="all active engines",
                excluded_engines=[EngineExclusion(engine="hibp", reason="sensitive engine excluded")],
            ),
            engine_outcomes=[EngineOutcome(engine="brave", status="ok", result_count=0)],
        )
        env = t._envelope(
            state, resp, requested_intent="auto", warnings=[], cursor=None, include_suggestions=False, total=0
        )
        assert env["scope"]["excluded_engines"] == [{"engine": "hibp", "reason": "sensitive engine excluded"}]


# ---------------------------------------------------------------------------
# slopsearx_search_targeted
# ---------------------------------------------------------------------------


class TestTargetedTool:
    async def test_requires_engines(self, state: McpState) -> None:
        result = await t.slopsearx_search_targeted("hello", engines=[])
        assert result["error"]["code"] == "invalid_input"

    async def test_unknown_engine_lists_alternatives(self, state: McpState) -> None:
        result = await t.slopsearx_search_targeted("hello", engines=["not-an-engine"])
        assert result["error"]["code"] == "invalid_scope"
        assert "valid_alternatives" in result["error"]

    async def test_sensitive_engine_requires_grant(self, state: McpState) -> None:
        result = await t.slopsearx_search_targeted("hello", engines=["hibp"])
        assert result["error"]["code"] == "tool_disabled"

    async def test_sensitive_engine_allowed_with_grant(self, state: McpState) -> None:
        state.policy.targeted_sensitive_allowed = True
        state.ctx.active_engines["hibp"] = _MockEngine("hibp")
        result = await t.slopsearx_search_targeted("hello", engines=["hibp"])
        assert "error" not in result
        assert result["scope"]["selected_engines"] == ["hibp"]

    async def test_explicit_search(self, state: McpState) -> None:
        result = await t.slopsearx_search_targeted("hello", engines=["wikipedia"])
        assert "error" not in result
        assert result["scope"]["selected_engines"] == ["wikipedia"]


# ---------------------------------------------------------------------------
# Jobs / security / science grants
# ---------------------------------------------------------------------------


class TestSpecialistGrants:
    async def test_jobs_requires_grant(self, state: McpState) -> None:
        result = await t.slopsearx_search_jobs("Anthropic")
        assert result["error"]["code"] == "tool_disabled"

    async def test_security_requires_grant(self, state: McpState) -> None:
        result = await t.slopsearx_search_security("log4j")
        assert result["error"]["code"] == "tool_disabled"

    async def test_science_requires_grant(self, state: McpState) -> None:
        result = await t.slopsearx_search_science("attention")
        assert result["error"]["code"] == "tool_disabled"

    async def test_research_requires_grant(self, state: McpState) -> None:
        result = await t.slopsearx_start_research("what is RLHF")
        assert result["error"]["code"] == "tool_disabled"

    async def test_jobs_with_grant(self, state: McpState) -> None:
        state.policy.enabled_tools["jobs"] = True
        state.ctx.active_engines = _make_engines(["greenhouse", "ashby", "lever", "brave"])
        result = await t.slopsearx_search_jobs("Anthropic", keywords=["senior", "engineer"])

        assert "error" not in result
        assert result["query"] == "senior engineer at Anthropic"
        assert any("no full job descriptions" in w for w in result["warnings"])

    async def test_jobs_requires_company(self, state: McpState) -> None:
        state.policy.enabled_tools["jobs"] = True
        result = await t.slopsearx_search_jobs("")
        assert result["error"]["code"] == "invalid_input"

    async def test_security_with_grant(self, state: McpState) -> None:
        state.policy.enabled_tools["security"] = True
        state.ctx.active_engines = _make_engines(["cve", "nvd", "epss", "vulncheck", "exploitdb"])
        result = await t.slopsearx_search_security("log4j", evidence_types=["vulnerability"])

        assert "error" not in result
        assert any("not a complete security assessment" in w for w in result["warnings"])

    async def test_security_unknown_evidence_type(self, state: McpState) -> None:
        state.policy.enabled_tools["security"] = True
        result = await t.slopsearx_search_security("log4j", evidence_types=["bogus"])
        assert result["error"]["code"] == "invalid_input"
        assert "valid_alternatives" in result["error"]

    async def test_science_with_grant(self, state: McpState) -> None:
        state.policy.enabled_tools["science"] = True
        state.ctx.active_engines = _make_engines(["arxiv", "semanticscholar", "openalex"])
        result = await t.slopsearx_search_science("transformers", source_types=["papers"])

        assert "error" not in result
        assert any("peer-review" in w for w in result["warnings"])

    async def test_science_date_range_warns(self, state: McpState) -> None:
        state.policy.enabled_tools["science"] = True
        state.ctx.active_engines = _make_engines(["arxiv"])
        result = await t.slopsearx_search_science("transformers", date_from="2024-01-01")

        assert any("date_from" in w for w in result["warnings"])

    async def test_security_intent_requires_grant_in_search(self, state: McpState) -> None:
        result = await t.slopsearx_search("log4j", intent="security")
        assert result["error"]["code"] == "tool_disabled"

    async def test_envelope_shape_consistent_across_search_tools(self, state: McpState) -> None:
        """VAL-SPEC-015 / VAL-TARGET-011 — specialist and targeted envelopes share the ordinary-search shape."""
        ordinary = await t.slopsearx_search("hello")
        ordinary_keys = set(ordinary)

        targeted = await t.slopsearx_search_targeted("hello", engines=["wikipedia"])
        assert set(targeted) == ordinary_keys

        state.policy.enabled_tools["jobs"] = True
        state.ctx.active_engines = _make_engines(["greenhouse", "ashby", "lever", "brave"])
        jobs = await t.slopsearx_search_jobs("Anthropic")
        assert set(jobs) == ordinary_keys
        # Specialist coverage fields match ordinary search.
        for env in (targeted, jobs):
            assert "engine_outcomes" in env
            assert "partial" in env["meta"]
            assert "scope" in env and "selected_engines" in env["scope"]
            assert "answers" in env and "corrections" in env and "infoboxes" in env
            assert "empty_engines" in env
            assert "total" in env["meta"] and "has_more" in env["meta"]


# ---------------------------------------------------------------------------
# Capabilities / explain / status
# ---------------------------------------------------------------------------


class TestDiscoveryTools:
    async def test_list_capabilities(self, state: McpState) -> None:
        result = await t.slopsearx_list_capabilities()
        assert result["count"] > 0
        first = result["engines"][0]
        assert "auth" in first
        assert "name" in first

    async def test_list_capabilities_hides_auth(self, state: McpState) -> None:
        result = await t.slopsearx_list_capabilities(include_auth_requirements=False)
        assert "auth" not in result["engines"][0]

    async def test_list_capabilities_filters(self, state: McpState) -> None:
        result = await t.slopsearx_list_capabilities(category="security")
        assert result["count"] > 0
        assert all("security" in e["categories"] for e in result["engines"])

    async def test_explain_scope(self, state: McpState) -> None:
        result = await t.slopsearx_explain_search_scope("hello")
        assert "error" not in result
        assert result["selected_engines"]
        assert "routing_rule" in result

    async def test_get_service_status(self, state: McpState) -> None:
        result = await t.slopsearx_get_service_status()
        assert result["status"] == "ok"
        assert result["active_engines"] > 0
        assert result["version"] == "test"

    # -- Operational diagnostics (feature: operational-diagnostics) --------

    async def test_status_reports_service_and_contract_versions(self, state: McpState) -> None:
        """VAL-DIAG-001 — status reports both service version and MCP contract version."""
        result = await t.slopsearx_get_service_status()
        assert isinstance(result["version"], str) and result["version"]
        assert isinstance(result["contract_version"], str) and result["contract_version"]
        assert result["version"] == state.version

    async def test_status_reports_no_parameters_and_ok(self, state: McpState) -> None:
        """VAL-DIAG-014 — status accepts no arguments and returns status='ok' with the schema."""
        result = await t.slopsearx_get_service_status()
        assert result["status"] == "ok"
        for key in (
            "version",
            "contract_version",
            "valkey",
            "active_engines",
            "grants",
            "engine_health",
            "cache_connected",
            "snapshots_available",
            "policy_bounds",
            "degradation",
            "freshness",
        ):
            assert key in result, f"missing schema key {key}"

    async def test_status_valkey_reported_honestly(self, state: McpState) -> None:
        """VAL-DIAG-003 — valkey.connected is false and fail_closed is reflected when no Valkey."""
        result = await t.slopsearx_get_service_status()
        assert result["valkey"]["connected"] is False
        assert "fail_closed" in result["valkey"]

    async def test_status_effective_engine_count_matches_catalog(self, state: McpState) -> None:
        """VAL-DIAG-004 — effective engine count equals the enabled catalog count."""
        result = await t.slopsearx_get_service_status()
        enabled = await t.slopsearx_list_capabilities()
        assert result["active_engines"] == enabled["count"] == len(state.catalog.enabled())
        assert result["active_engines"] > 0

    async def test_status_grants_listed_by_name_only(self, state: McpState) -> None:
        """VAL-DIAG-005 — enabled grants are listed by name; no secret value appears."""
        state.policy.enabled_tools["jobs"] = True
        state.policy.enabled_tools["science"] = True
        result = await t.slopsearx_get_service_status()
        blob = str(result)
        assert "jobs" in result["grants"]["enabled"]
        assert "science" in result["grants"]["enabled"]
        assert "security" not in result["grants"]["enabled"]
        assert "MCP_GRANT" not in blob

    async def test_status_engine_health_aggregated_by_class(self, state: McpState) -> None:
        """VAL-DIAG-006 — engine health is a machine-readable per-class integer count mapping."""
        result = await t.slopsearx_get_service_status()
        health = result["engine_health"]
        for cls in ("ok", "rate_limited", "blocked", "error", "timeout", "unknown"):
            assert isinstance(health[cls], int), f"{cls} not an integer count"
            assert health[cls] >= 0

    async def test_status_policy_bounds_reported(self, state: McpState) -> None:
        """VAL-DIAG-008 — current policy bounds are positive numbers matching policy."""
        result = await t.slopsearx_get_service_status()
        bounds = result["policy_bounds"]
        for key in (
            "max_query_length",
            "max_results",
            "snapshot_ttl_seconds",
            "job_max_queries",
            "job_max_engines_per_query",
            "job_max_results",
            "job_default_deadline_seconds",
        ):
            assert bounds[key] == getattr(state.policy, key), key
            assert isinstance(bounds[key], int) and bounds[key] > 0

    async def test_status_degradation_and_freshness(self, state: McpState) -> None:
        """VAL-DIAG-009 — degradation summary + freshness timestamp are present and recent."""
        result = await t.slopsearx_get_service_status()
        degradation = result["degradation"]
        assert "operational" in degradation and "summary" in degradation and "causes" in degradation
        # No Valkey in the fixture → honest degraded view.
        assert degradation["operational"] is False
        assert "valkey" in " ".join(degradation["causes"]).lower()
        parsed = _dt.datetime.fromisoformat(result["freshness"])
        assert (parsed - _dt.datetime.now(_dt.timezone.utc)).total_seconds() < 10

    async def test_status_and_health_resource_agree(self, state: McpState) -> None:
        """VAL-DIAG-010 — the health resource reports the same shared values as the status tool."""
        result = await t.slopsearx_get_service_status()
        health = t.service_diagnostics(state, now=_dt.datetime.now(_dt.timezone.utc).isoformat())
        assert health["version"] == result["version"] == state.version
        assert health["valkey"] == result["valkey"]
        assert health["active_engines"] == result["active_engines"]
        assert health["cache_connected"] == result["cache_connected"]
        assert health["snapshots_available"] == result["snapshots_available"]
        assert health["policy_bounds"] == result["policy_bounds"]

    async def test_status_reveals_no_credentials(self, state: McpState) -> None:
        """VAL-DIAG-011 — a configured token never appears in the status output."""
        sentinel = "super-secret-token-xyz"
        state.policy.auth_token = sentinel
        result = await t.slopsearx_get_service_status()
        blob = str(result)
        assert sentinel not in blob
        assert "api_key" not in blob.lower()

    async def test_status_reveals_no_audit_or_environment(self, state: McpState) -> None:
        """VAL-DIAG-012 — status output has no raw audit records or environment dump."""
        result = await t.slopsearx_get_service_status()
        blob = str(result)
        assert "os.environ" not in blob
        assert "audit" not in blob.lower()

    async def test_status_contains_no_unrestricted_metrics(self, state: McpState) -> None:
        """VAL-DIAG-013 — status returns only the curated summary schema, no metric-series dump."""
        result = await t.slopsearx_get_service_status()
        blob = str(result)
        assert "# HELP" not in blob and "# TYPE" not in blob
        assert "metrics" not in blob.lower()


# ---------------------------------------------------------------------------
# Snapshot reads
# ---------------------------------------------------------------------------


class TestSnapshotReads:
    async def test_read_results_pages(self, state: McpState) -> None:
        result = await t.slopsearx_search("hello")
        cursor = result["meta"]["cursor"]
        assert cursor is not None

        page1 = await t.slopsearx_read_results(cursor, page=1, max_results=2)
        assert len(page1["results"]) == 2
        assert page1["meta"]["has_more"] is True

        # 9 merged results → 5 pages of 2; the last page is partial.
        last_page = await t.slopsearx_read_results(cursor, page=5, max_results=2)
        assert len(last_page["results"]) == 1
        assert last_page["meta"]["has_more"] is False

    async def test_read_results_unknown_cursor(self, state: McpState) -> None:
        result = await t.slopsearx_read_results("snap-bogus", page=1)
        assert result["error"]["code"] == "invalid_cursor"

    async def test_read_result(self, state: McpState) -> None:
        result = await t.slopsearx_search("hello")
        cursor = result["meta"]["cursor"]
        result_id = f"{cursor}:0"
        expanded = await t.slopsearx_read_result(result_id)

        assert "error" not in expanded
        assert expanded["provenance"]["query"] == "hello"
        assert expanded["note"]

    async def test_read_result_rejects_arbitrary_input(self, state: McpState) -> None:
        result = await t.slopsearx_read_result("https://evil.example")
        assert result["error"]["code"] == "invalid_result_id"


# ---------------------------------------------------------------------------
# Uniform structured input validation
# ---------------------------------------------------------------------------


class TestInputValidation:
    """Uniform, structured input validation across all search tools.

    Covers VAL-SEARCH-011/012/013/019/020, VAL-TARGET-002/003/007, and
    VAL-SPEC-007/008: empty/over-long queries, unknown intents, unknown and
    disabled explicit engines, empty engine lists, and unknown evidence /
    source types — all rejected with the pinned error code + field before
    any engine dispatch.
    """

    async def test_empty_and_whitespace_query_rejected(self, state: McpState) -> None:
        """VAL-SEARCH-011 — empty/whitespace query returns invalid_input on 'query', no dispatch."""
        for q in ("", "   ", "\t\n"):
            result = await t.slopsearx_search(q)
            assert result["error"]["code"] == "invalid_input"
            assert result["error"]["field"] == "query"
            assert "results" not in result
            assert "meta" not in result

    async def test_overlong_query_rejected_with_max(self, state: McpState) -> None:
        """VAL-SEARCH-012 — query over max_query_length returns invalid_input with the configured maximum."""
        state.policy.max_query_length = 20
        result = await t.slopsearx_search("x" * 21)

        assert result["error"]["code"] == "invalid_input"
        assert result["error"]["field"] == "query"
        assert result["error"]["max_length"] == 20
        assert "results" not in result

    async def test_unknown_intent_rejected_with_alternatives(self, state: McpState) -> None:
        """VAL-SEARCH-013 — unknown intent returns invalid_input on 'intent' with valid alternatives."""
        result = await t.slopsearx_search("hello", intent="does_not_exist")

        assert result["error"]["code"] == "invalid_input"
        assert result["error"]["field"] == "intent"
        assert result["error"]["valid_alternatives"]
        assert "results" not in result

    async def test_generic_search_unknown_engine_rejected(self, state: McpState) -> None:
        """VAL-SEARCH-019 — generic search with unknown engine returns invalid_scope, no dispatch."""
        result = await t.slopsearx_search("hello", engines=["no_such_engine"])

        assert result["error"]["code"] == "invalid_scope"
        assert result["error"]["field"] == "engines"
        assert "no_such_engine" in result["error"]["message"]
        assert result["error"]["valid_alternatives"]
        assert "results" not in result
        assert "meta" not in result

    async def test_generic_search_disabled_engine_rejected(self, state: McpState) -> None:
        """VAL-SEARCH-020 — generic search with a known-but-disabled engine returns invalid_scope."""
        # internetarchive is registered but disabled by default config.
        result = await t.slopsearx_search("hello", engines=["internetarchive"])

        assert result["error"]["code"] == "invalid_scope"
        assert result["error"]["field"] == "engines"
        assert "inactive" in result["error"]["message"].lower()
        assert result["error"]["valid_alternatives"]
        assert "internetarchive" not in result["error"]["valid_alternatives"]
        assert "results" not in result

    async def test_targeted_unknown_engine_rejected(self, state: McpState) -> None:
        """VAL-TARGET-002 — targeted search with unknown engine returns invalid_scope with alternatives."""
        result = await t.slopsearx_search_targeted("hello", engines=["definitely_not_an_engine"])

        assert result["error"]["code"] == "invalid_scope"
        assert result["error"]["field"] == "engines"
        assert "definitely_not_an_engine" in result["error"]["message"]
        assert result["error"]["valid_alternatives"]
        assert "results" not in result

    async def test_targeted_disabled_engine_rejected(self, state: McpState) -> None:
        """VAL-TARGET-003 — targeted search with a disabled engine returns invalid_scope as inactive."""
        result = await t.slopsearx_search_targeted("hello", engines=["internetarchive"])

        assert result["error"]["code"] == "invalid_scope"
        assert result["error"]["field"] == "engines"
        assert "inactive" in result["error"]["message"].lower()
        assert result["error"]["valid_alternatives"]
        assert "internetarchive" not in result["error"]["valid_alternatives"]
        assert "results" not in result

    async def test_targeted_empty_engine_list_rejected(self, state: McpState) -> None:
        """VAL-TARGET-007 — targeted search with an empty engine list returns invalid_input, no routing."""
        result = await t.slopsearx_search_targeted("hello", engines=[])

        assert result["error"]["code"] == "invalid_input"
        assert result["error"]["field"] == "engines"
        assert "results" not in result

    async def test_security_unknown_evidence_type_rejected(self, state: McpState) -> None:
        """VAL-SPEC-007 — security search with unknown evidence_type returns invalid_input + alternatives."""
        state.policy.enabled_tools["security"] = True
        result = await t.slopsearx_search_security("log4j", evidence_types=["does_not_exist"])

        assert result["error"]["code"] == "invalid_input"
        assert result["error"]["field"] == "evidence_types"
        assert result["error"]["valid_alternatives"]
        assert "does_not_exist" in result["error"]["message"]
        assert "results" not in result

    async def test_science_unknown_source_type_rejected(self, state: McpState) -> None:
        """VAL-SPEC-008 — science search with unknown source_type returns invalid_input + alternatives."""
        state.policy.enabled_tools["science"] = True
        result = await t.slopsearx_search_science("transformers", source_types=["not_a_source"])

        assert result["error"]["code"] == "invalid_input"
        assert result["error"]["field"] == "source_types"
        assert result["error"]["valid_alternatives"]
        assert "not_a_source" in result["error"]["message"]
        assert "results" not in result


# ---------------------------------------------------------------------------
# FastMCP integration
# ---------------------------------------------------------------------------


class TestFastMCPIntegration:
    async def test_call_tool_registration(self, state: McpState) -> None:
        from slopsearx.mcp.server import create_server

        server = create_server()
        raw = await server.call_tool("slopsearx_list_capabilities", {"include_auth_requirements": False})
        assert raw is not None

    async def test_call_tool_search_returns_structured_dict(self, state: McpState) -> None:
        from slopsearx.mcp.server import create_server

        server = create_server()
        raw = await server.call_tool("slopsearx_search", {"query": "hello"})
        assert raw is not None

    async def test_instrumented_tracks_structured_errors(self) -> None:
        from slopsearx import metrics as m
        from slopsearx.mcp.server import _instrumented

        async def boom() -> dict:
            return {"error": {"code": "invalid_input"}}

        wrapped = _instrumented(boom)
        result = await wrapped()
        assert result["error"]["code"] == "invalid_input"
        rendered = m.render_metrics()
        assert "slopsearx_mcp_tool_calls_total" in rendered
        assert "slopsearx_mcp_tool_errors_total" in rendered
        assert "slopsearx_mcp_tool_latency_seconds" in rendered
