"""Tests for the MCP tool implementations (slopsearx.mcp.tools).

Tools are plain async callables tested directly (FastMCP-free), plus a
few integration checks through FastMCP's call_tool.
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
from slopsearx.service import SearchService
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
