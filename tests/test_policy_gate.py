"""Tests for the one shared sensitive-engine + specialist-grant policy gate.

Covers VAL-FILTER-008..015, 018..023, VAL-CAP-015, VAL-SPEC-001/002/003,
016/017/018, and VAL-RESEARCH-017: a single fail-closed gate reached by
every search path (generic, targeted, jobs, security, science, scope
preview) and by research query planning, before any engine dispatch.

Tools are plain async callables tested directly (FastMCP-free), exactly
like tests/test_mcp_tools.py.
"""

from __future__ import annotations

from typing import Any

import pytest

import engines  # noqa: F401 — populates the engine registry
from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult
from slopsearx.capabilities import CapabilityCatalog, MCPPolicy, load_mcp_policy
from slopsearx.config import load_config
from slopsearx.mcp import tools as t
from slopsearx.mcp.state import McpState, set_state
from slopsearx.research import ResearchJobRunner, ResearchJobStore, plan_research_queries
from slopsearx.service import AppContext, SearchService
from slopsearx.snapshot import SnapshotStore

SENSITIVE_GRANT = "MCP_TARGETED_SENSITIVE_ALLOWED"


class _FakeStore:
    """In-memory key-value store."""

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
) -> McpState:
    engine_names = engine_names or ["wikipedia", "brave", "duckduckgo"]
    engines_map = _make_engines(engine_names)
    policy = policy or load_mcp_policy(config_path=None)
    ctx = AppContext(
        active_engines=engines_map,
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


def _assert_sensitive_blocked(result: dict[str, Any], sensitive: list[str], field: str) -> None:
    """Shared assertions for every blocked path (VAL-FILTER-015)."""
    assert result["error"]["code"] == "tool_disabled"
    assert SENSITIVE_GRANT in result["error"]["message"]
    assert result["error"].get("grant") == SENSITIVE_GRANT
    assert result["error"]["field"] == field
    # Structured sensitive engine names (not only in the message).
    assert set(result["error"]["engines"]) == set(sensitive)
    # Fail-closed: no search envelope, no dispatch evidence.
    assert "results" not in result
    assert "engine_outcomes" not in result
    assert "meta" not in result


# ---------------------------------------------------------------------------
# Generic explicit engines (VAL-FILTER-008, 019, 022)
# ---------------------------------------------------------------------------


class TestGenericExplicitEngines:
    async def test_sensitive_engine_blocked_without_grant(self, state: McpState) -> None:
        """VAL-FILTER-008 — generic explicit engines=['hibp'] blocked without the grant."""
        result = await t.slopsearx_search("breach", engines=["hibp"])
        _assert_sensitive_blocked(result, ["hibp"], "engines")

    async def test_dehashed_also_blocked(self, state: McpState) -> None:
        """VAL-FILTER-008 — dehashed equivalently blocked."""
        result = await t.slopsearx_search("creds", engines=["dehashed"])
        _assert_sensitive_blocked(result, ["dehashed"], "engines")

    async def test_mixed_list_fails_closed_atomically(self, state: McpState) -> None:
        """VAL-FILTER-019 — ['brave','hibp'] rejects the whole request, brave not dispatched."""
        state.ctx.active_engines["brave"] = _MockEngine("brave")
        result = await t.slopsearx_search("breach", engines=["brave", "hibp"])
        _assert_sensitive_blocked(result, ["hibp"], "engines")
        assert state.ctx.active_engines["brave"].calls == 0

    async def test_no_engine_calls_on_block(self, state: McpState) -> None:
        """VAL-FILTER-018 — a blocked request dispatches zero engines (fail-closed ordering)."""
        state.ctx.active_engines["hibp"] = _MockEngine("hibp")
        result = await t.slopsearx_search("breach", engines=["hibp"])
        _assert_sensitive_blocked(result, ["hibp"], "engines")
        assert state.ctx.active_engines["hibp"].calls == 0

    async def test_allowed_with_grant(self, state: McpState) -> None:
        """VAL-FILTER-022 — generic engines=['hibp'] proceeds with the grant."""
        state.policy.targeted_sensitive_allowed = True
        state.ctx.active_engines["hibp"] = _MockEngine("hibp")
        result = await t.slopsearx_search("breach", engines=["hibp"])
        assert "error" not in result
        assert "hibp" in result["scope"]["selected_engines"]
        assert state.ctx.active_engines["hibp"].calls == 1


# ---------------------------------------------------------------------------
# Targeted (VAL-FILTER-013, 014)
# ---------------------------------------------------------------------------


class TestTargeted:
    async def test_sensitive_engine_blocked_without_grant(self, state: McpState) -> None:
        """VAL-FILTER-014 — targeted engines=['hibp'] blocked, structured error."""
        result = await t.slopsearx_search_targeted("breach", engines=["hibp"])
        _assert_sensitive_blocked(result, ["hibp"], "engines")

    async def test_sensitive_engine_allowed_with_grant(self, state: McpState) -> None:
        """VAL-FILTER-013 — targeted engines=['hibp'] proceeds with the grant."""
        state.policy.targeted_sensitive_allowed = True
        state.ctx.active_engines["hibp"] = _MockEngine("hibp")
        result = await t.slopsearx_search_targeted("breach", engines=["hibp"])
        assert "error" not in result
        assert result["scope"]["selected_engines"] == ["hibp"]
        assert state.ctx.active_engines["hibp"].calls == 1


# ---------------------------------------------------------------------------
# Specialist paths (VAL-FILTER-010, 011, 012, 020, 021, 023)
# ---------------------------------------------------------------------------


class TestSpecialistSensitive:
    async def test_security_reputation_cannot_reach_hibp(self, state: McpState) -> None:
        """VAL-FILTER-010 — security evidence_types=['reputation'] cannot reach hibp without the grant."""
        state.policy.enabled_tools["security"] = True
        state.ctx.active_engines = _make_engines(["otx", "greynoise", "abuseipdb", "virustotal", "hibp"])
        result = await t.slopsearx_search_security("breach", evidence_types=["reputation"])
        _assert_sensitive_blocked(result, ["hibp"], "evidence_types")

    async def test_security_explicit_hibp_blocked(self, state: McpState) -> None:
        """VAL-FILTER-020 — security engines=['hibp'] blocked naming hibp, no dispatch."""
        state.policy.enabled_tools["security"] = True
        state.ctx.active_engines["hibp"] = _MockEngine("hibp")
        result = await t.slopsearx_search_security("breach", engines=["hibp"])
        _assert_sensitive_blocked(result, ["hibp"], "engines")
        assert state.ctx.active_engines["hibp"].calls == 0

    async def test_security_exposure_cannot_reach_dehashed(self, state: McpState) -> None:
        """VAL-FILTER-021 — security evidence_types=['exposure'] cannot reach dehashed."""
        state.policy.enabled_tools["security"] = True
        state.ctx.active_engines = _make_engines(
            ["shodan", "censys", "crtsh", "urlhaus", "abuseipdb", "intelx", "dehashed"]
        )
        result = await t.slopsearx_search_security("breach", evidence_types=["exposure"])
        _assert_sensitive_blocked(result, ["dehashed"], "evidence_types")

    async def test_science_explicit_dehashed_blocked(self, state: McpState) -> None:
        """VAL-FILTER-011 — science engines=['dehashed'] blocked without the grant."""
        state.policy.enabled_tools["science"] = True
        result = await t.slopsearx_search_science("creds", engines=["dehashed"])
        _assert_sensitive_blocked(result, ["dehashed"], "engines")

    async def test_jobs_sensitive_source_blocked(self, state: McpState) -> None:
        """VAL-FILTER-012 — jobs sources=['hibp'] blocked naming the sensitive engine."""
        state.policy.enabled_tools["jobs"] = True
        result = await t.slopsearx_search_jobs("Acme", sources=["hibp"])
        _assert_sensitive_blocked(result, ["hibp"], "sources")

    async def test_specialist_dispatch_with_grants(self, state: McpState) -> None:
        """VAL-FILTER-023 — jobs sources=['hibp'] and science engines=['dehashed'] proceed with grants."""
        state.policy.enabled_tools["jobs"] = True
        state.policy.enabled_tools["science"] = True
        state.policy.targeted_sensitive_allowed = True
        state.ctx.active_engines["hibp"] = _MockEngine("hibp")
        state.ctx.active_engines["dehashed"] = _MockEngine("dehashed")

        jobs = await t.slopsearx_search_jobs("Acme", sources=["hibp"])
        assert "error" not in jobs
        assert "hibp" in jobs["scope"]["selected_engines"]

        sci = await t.slopsearx_search_science("creds", engines=["dehashed"])
        assert "error" not in sci
        assert "dehashed" in sci["scope"]["selected_engines"]


# ---------------------------------------------------------------------------
# Scope preview (VAL-CAP-015)
# ---------------------------------------------------------------------------


class TestScopePreview:
    async def test_preview_hibp_matches_search(self, state: McpState) -> None:
        """VAL-CAP-015 — explain_search_scope(engines=['hibp']) returns the same tool_disabled as search."""
        search_result = await t.slopsearx_search("breach", engines=["hibp"])
        preview = await t.slopsearx_explain_search_scope("breach", engines=["hibp"])
        assert preview["error"]["code"] == "tool_disabled"
        assert preview["error"]["code"] == search_result["error"]["code"]
        assert preview["error"].get("grant") == SENSITIVE_GRANT
        assert "hibp" in preview["error"]["engines"]

    async def test_preview_security_intent_requires_grant(self, state: McpState) -> None:
        """VAL-CAP-015 — explain_search_scope(intent='security') fails closed without the grant."""
        preview = await t.slopsearx_explain_search_scope("log4j", intent="security")
        assert preview["error"]["code"] == "tool_disabled"
        assert preview["error"]["field"] == "intent"

    async def test_preview_unscoped_never_offers_sensitive(self, state: McpState) -> None:
        """VAL-CAP-015 — hibp/dehashed are absent from preview selected_engines without the grant."""
        state.ctx.active_engines = _make_engines(["wikipedia", "brave", "hibp", "dehashed"])
        preview = await t.slopsearx_explain_search_scope("hello", intent="auto")
        assert "error" not in preview
        assert "hibp" not in preview["selected_engines"]
        assert "dehashed" not in preview["selected_engines"]


# ---------------------------------------------------------------------------
# Generic search never selects sensitive on category/unscoped routes (VAL-FILTER-009)
# ---------------------------------------------------------------------------


class TestUnscopedNeverSelects:
    async def test_sensitive_absent_from_outcomes_and_results(self, state: McpState) -> None:
        """VAL-FILTER-009 — generic unscoped search never dispatches hibp/dehashed."""
        state.ctx.active_engines = _make_engines(["wikipedia", "brave", "hibp", "dehashed"])
        result = await t.slopsearx_search("hello", intent="auto")
        assert "error" not in result
        selected = result["scope"]["selected_engines"]
        assert "hibp" not in selected
        assert "dehashed" not in selected
        outcome_engines = {o["engine"] for o in result["engine_outcomes"]}
        assert "hibp" not in outcome_engines
        assert "dehashed" not in outcome_engines


# ---------------------------------------------------------------------------
# Uniform block semantics across every path (VAL-FILTER-015)
# ---------------------------------------------------------------------------


class TestUniformGate:
    async def test_all_paths_share_error_code_and_field(self, state: McpState) -> None:
        """VAL-FILTER-015 — every blocked path emits the shared code, grant, and per-tool field."""
        state.policy.enabled_tools["jobs"] = True
        state.policy.enabled_tools["security"] = True
        state.policy.enabled_tools["science"] = True

        cases = [
            (await t.slopsearx_search("q", engines=["hibp"]), "engines"),
            (await t.slopsearx_search_targeted("q", engines=["hibp"]), "engines"),
            (await t.slopsearx_search_jobs("Acme", sources=["hibp"]), "sources"),
            (await t.slopsearx_search_security("q", engines=["dehashed"]), "engines"),
            (await t.slopsearx_search_science("q", engines=["dehashed"]), "engines"),
        ]
        for result, field in cases:
            assert result["error"]["code"] == "tool_disabled"
            assert result["error"].get("grant") == SENSITIVE_GRANT
            assert result["error"]["field"] == field
            assert "engines" in result["error"] and result["error"]["engines"]


# ---------------------------------------------------------------------------
# Specialist grants are tool-scoped (VAL-SPEC-016, 017, 018)
# ---------------------------------------------------------------------------


class TestSpecialistGrantScoping:
    async def test_grants_are_tool_scoped(self, state: McpState) -> None:
        """VAL-SPEC-016 — enabling jobs does not enable security/science, and vice versa."""
        state.policy.enabled_tools["jobs"] = True
        assert (await t.slopsearx_search_security("log4j"))["error"]["code"] == "tool_disabled"
        assert (await t.slopsearx_search_science("attention"))["error"]["code"] == "tool_disabled"

        state.policy.enabled_tools = {"jobs": False, "security": True, "science": False, "research": False}
        assert (await t.slopsearx_search_jobs("Acme"))["error"]["code"] == "tool_disabled"
        assert (await t.slopsearx_search_science("attention"))["error"]["code"] == "tool_disabled"
        assert (await t.slopsearx_search_security("log4j"))["error"]["code"] != "tool_disabled"

    async def test_generic_cannot_bypass_disabled_security_grant(self, state: McpState) -> None:
        """VAL-SPEC-017 — generic intent='security' fails closed, field='intent', references the grant."""
        result = await t.slopsearx_search("log4j", intent="security")
        assert result["error"]["code"] == "tool_disabled"
        assert result["error"]["field"] == "intent"
        assert "MCP_GRANT_SECURITY" in result["error"]["message"]
        assert "results" not in result

    async def test_generic_jobs_intent_is_grant_gated(self, state: McpState) -> None:
        """VAL-SPEC-018 — generic intent='jobs' gated by MCP_GRANT_JOBS."""
        result = await t.slopsearx_search("engineer", intent="jobs")
        assert result["error"]["code"] == "tool_disabled"
        assert result["error"]["field"] == "intent"
        assert "MCP_GRANT_JOBS" in result["error"]["message"]

    async def test_generic_science_intent_is_grant_gated(self, state: McpState) -> None:
        """VAL-SPEC-018 — generic intent='science' gated by MCP_GRANT_SCIENCE."""
        result = await t.slopsearx_search("transformers", intent="science")
        assert result["error"]["code"] == "tool_disabled"
        assert result["error"]["field"] == "intent"
        assert "MCP_GRANT_SCIENCE" in result["error"]["message"]

    async def test_jobs_grant_message_names_grant(self, state: McpState) -> None:
        """VAL-SPEC-001 — jobs tool_disabled message names MCP_GRANT_JOBS."""
        result = await t.slopsearx_search_jobs("Acme")
        assert result["error"]["code"] == "tool_disabled"
        assert "MCP_GRANT_JOBS" in result["error"]["message"]
        assert "results" not in result

    async def test_security_grant_message_names_grant(self, state: McpState) -> None:
        """VAL-SPEC-002 — security tool_disabled message names MCP_GRANT_SECURITY."""
        result = await t.slopsearx_search_security("log4j")
        assert result["error"]["code"] == "tool_disabled"
        assert "MCP_GRANT_SECURITY" in result["error"]["message"]
        assert "results" not in result

    async def test_science_grant_message_names_grant(self, state: McpState) -> None:
        """VAL-SPEC-003 — science tool_disabled message names MCP_GRANT_SCIENCE."""
        result = await t.slopsearx_search_science("attention")
        assert result["error"]["code"] == "tool_disabled"
        assert "MCP_GRANT_SCIENCE" in result["error"]["message"]
        assert "results" not in result


# ---------------------------------------------------------------------------
# Research planning (VAL-RESEARCH-017)
# ---------------------------------------------------------------------------


class TestResearchPlanning:
    def test_sensitive_excluded_with_policy_warning(self, state: McpState) -> None:
        """VAL-RESEARCH-017 — planning excludes sensitive engines and reports a policy reason."""
        # The triangulate "web" query resolves the general category, which
        # includes brave; mark brave sensitive to exercise the gate.
        state.policy.sensitive_engines = {"brave"}
        catalog = state.catalog
        queries, warnings = plan_research_queries("question", "triangulate", 10, 20, catalog, state.policy)
        all_engines = {name for q in queries for name in q.engines}
        assert "brave" not in all_engines
        assert any("sensitive" in w and "MCP_TARGETED_SENSITIVE_ALLOWED" in w for w in warnings)

    def test_sensitive_included_when_granted(self, state: McpState) -> None:
        """VAL-RESEARCH-017 — with the sensitive grant, sensitive engines are not excluded for policy."""
        state.policy.sensitive_engines = {"brave"}
        state.policy.targeted_sensitive_allowed = True
        catalog = state.catalog
        queries, _ = plan_research_queries("question", "triangulate", 10, 20, catalog, state.policy)
        all_engines = {name for q in queries for name in q.engines}
        assert "brave" in all_engines
