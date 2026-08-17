"""Tests for the live capability catalog feature (feature: capability-catalog).

Covers VAL-CAP-001..014, VAL-CAP-016, VAL-CROSS-009, and the dispatched-scope
filter-enforcement resolution required by the feature description.

The catalog is generated from the live adapter registry (never prose), exposes
the full per-engine capability matrix (sensitivity, supported filters, result
types, failure classes, cost class, last-known status/freshness), and the
capability/routing resources are read-only, cacheable, and leak no secrets.
"""

from __future__ import annotations

from typing import Any

import pytest

import engines  # noqa: F401 — triggers @register_engine to populate the registry
from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult, list_engines
from slopsearx.capabilities import (
    DEFAULT_SENSITIVE_ENGINES,
    INTENT_PROFILES,
    CapabilityCatalog,
    MCPPolicy,
    load_mcp_policy,
)
from slopsearx.config import load_config
from slopsearx.mcp import resources as r
from slopsearx.mcp import tools as t
from slopsearx.mcp.state import McpState, set_state
from slopsearx.research import STRATEGIES, ResearchJobRunner, ResearchJobStore
from slopsearx.service import AppContext, SearchService
from slopsearx.snapshot import SnapshotStore

# Every catalog entry must carry at least these fields (VAL-CAP-002).
REQUIRED_CAP_FIELDS = {
    "name",
    "display_name",
    "type",
    "categories",
    "enabled",
    "sensitive",
    "supported_filters",
    "supported_result_types",
    "failure_classes",
    "cost_class",
    "last_known_status",
    "last_known_status_at",
    "auth",
}
SUPPORTED_FILTER_KEYS = {"language", "time_range", "safesearch", "pagination"}
RESULT_TYPE_VOCAB = {"text", "answers", "corrections", "infoboxes", "media", "structured"}
FAILURE_CLASS_VOCAB = {"ok", "rate_limited", "blocked", "error", "timeout", "auth_required", "unavailable"}
AUTH_CLASSES = {"none", "optional", "required", "unknown"}
ENFORCEMENT_STATUSES = {"enforced", "partially_enforced", "unsupported", "rejected"}


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
        count: int = 2,
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
    catalog = CapabilityCatalog(
        config=load_config(),
        adapters=engines_map,
        sensitive_engines=policy.sensitive_engines,
    )
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


def _entry(result: dict[str, Any], name: str) -> dict[str, Any]:
    return next(e for e in result["engines"] if e["name"] == name)


def _assert_cap_entry(entry: dict[str, Any]) -> None:
    assert REQUIRED_CAP_FIELDS <= set(entry), entry
    assert isinstance(entry["enabled"], bool)
    assert isinstance(entry["sensitive"], bool)
    sf = entry["supported_filters"]
    assert set(sf) == SUPPORTED_FILTER_KEYS, sf
    assert all(isinstance(v, bool) for v in sf.values())
    assert entry["supported_result_types"], entry
    assert set(entry["supported_result_types"]) <= RESULT_TYPE_VOCAB, entry
    assert entry["failure_classes"], entry
    assert set(entry["failure_classes"]) <= FAILURE_CLASS_VOCAB, entry
    assert entry["cost_class"] is None or isinstance(entry["cost_class"], str)
    assert isinstance(entry["last_known_status"], str)
    assert "last_known_status_at" in entry
    auth = entry["auth"]
    assert set(auth) == {"class", "configured"}
    assert auth["class"] in AUTH_CLASSES
    assert isinstance(auth["configured"], bool)


# ---------------------------------------------------------------------------
# VAL-CAP-001 / VAL-CAP-002 / VAL-CAP-003 / VAL-CAP-004
# ---------------------------------------------------------------------------


class TestCatalogMatrix:
    async def test_discoverable_and_matches_registry(self, state: McpState) -> None:
        """VAL-CAP-001 — with include_disabled=true the engine set equals the live registry."""
        result = await t.slopsearx_list_capabilities(include_disabled=True)
        assert isinstance(result["engines"], list)
        assert result["count"] == len(result["engines"])
        names = {e["name"] for e in result["engines"]}
        assert names == set(list_engines()), "catalog must match the live registry exactly"

    async def test_every_entry_carries_full_matrix(self, state: McpState) -> None:
        """VAL-CAP-002 — every entry exposes the full per-engine capability matrix."""
        result = await t.slopsearx_list_capabilities(include_disabled=True)
        assert result["count"] >= 51
        for entry in result["engines"]:
            _assert_cap_entry(entry)

    async def test_auth_class_and_configured_never_secret(self, state: McpState) -> None:
        """VAL-CAP-003 — auth is class + boolean; no credential material anywhere."""
        result = await t.slopsearx_list_capabilities()
        blob = str(result)
        assert "api_key" not in blob
        for entry in result["engines"]:
            auth = entry["auth"]
            assert set(auth) == {"class", "configured"}
            assert auth["class"] in AUTH_CLASSES
            assert isinstance(auth["configured"], bool)
            assert "key" not in auth

    async def test_sensitive_classification_matches_policy(self, state: McpState) -> None:
        """VAL-CAP-004 — sensitive is present; default set is reflected; no engine lacks it."""
        result = await t.slopsearx_list_capabilities(include_disabled=True)
        by_name = {e["name"]: e["sensitive"] for e in result["engines"]}
        for name, sensitive in by_name.items():
            assert name in by_name
            assert isinstance(sensitive, bool)
        assert by_name["hibp"] is True
        assert by_name["dehashed"] is True
        for name in ("hibp", "dehashed"):
            assert by_name[name] is True
        for name, sensitive in by_name.items():
            expected = name in set(DEFAULT_SENSITIVE_ENGINES)
            assert sensitive == expected, f"{name} sensitive mismatch"

    async def test_operator_sensitive_override_reflected(self) -> None:
        """VAL-CAP-004 — an operator-configured sensitive set is reflected in the catalog."""
        policy = load_mcp_policy(config_path=None)
        policy.sensitive_engines = {"cve", "nvd"}
        state_obj = _build_state(engine_names=["cve", "nvd", "wikipedia"], policy=policy)
        set_state(state_obj)
        try:
            result = await t.slopsearx_list_capabilities()
        finally:
            set_state(None)
        by_name = {e["name"]: e["sensitive"] for e in result["engines"]}
        assert by_name["cve"] is True
        assert by_name["nvd"] is True
        assert by_name["wikipedia"] is False


class TestAuditedCapabilityDeclarations:
    """Issue 185 — the runtime catalog exposes the audited capability matrix.

    The ``state`` fixture overrides ``wikipedia``/``brave``/``duckduckgo``
    with mocks; every other registry engine is surfaced from its real class
    declarations, so these assertions prove the tool/resource wire the
    audited adapter metadata through end to end.
    """

    async def test_tool_exposes_audited_declarations_for_registry_engines(self, state: McpState) -> None:
        """Representative domain families report distinct declarations via the tool."""
        result = await t.slopsearx_list_capabilities(include_disabled=True)
        by_name = {e["name"]: e for e in result["engines"]}
        # Security: keyed engines declare a freemium cost class.
        assert by_name["shodan"]["cost_class"] == "freemium"
        assert by_name["virustotal"]["cost_class"] == "freemium"
        # Science: free scholarly indexes with honest failure classes.
        assert by_name["openalex"]["cost_class"] == "free"
        assert by_name["openalex"]["failure_classes"] == ["error"]
        # Media: TMDB returns media thumbnails and needs a key.
        assert "media" in by_name["tmdb"]["supported_result_types"]
        assert by_name["tmdb"]["cost_class"] == "freemium"
        # Jobs: free, text-only ATS boards.
        assert by_name["greenhouse"]["cost_class"] == "free"
        assert by_name["greenhouse"]["supported_result_types"] == ["text"]
        # General/web: registry-backed engines carry the audited matrix.
        assert by_name["openlibrary"]["cost_class"] == "free"
        assert by_name["openlibrary"]["supported_result_types"] == ["text", "media"]
        assert by_name["openlibrary"]["supported_filters"]["safesearch"] is False

    async def test_tool_exposes_audited_declarations_for_disabled_engine(self, state: McpState) -> None:
        """VAL-CAP-002/007 — a disabled engine still exposes its audited matrix."""
        result = await t.slopsearx_list_capabilities(include_disabled=True)
        entry = _entry(result, "internetarchive")
        assert entry["enabled"] is False
        assert entry["cost_class"] == "free"
        assert entry["supported_result_types"] == ["text"]
        assert entry["failure_classes"] == ["error"]
        assert entry["supported_filters"]["safesearch"] is False

    async def test_tool_declared_filters_are_not_enforcement_claims(self, state: McpState) -> None:
        """A declaration is a boolean hint; the report never invents enforcement."""
        result = await t.slopsearx_list_capabilities(include_disabled=True)
        for entry in result["engines"]:
            assert set(entry["supported_filters"]) == SUPPORTED_FILTER_KEYS
            assert all(isinstance(v, bool) for v in entry["supported_filters"].values())

    def test_resource_exposes_audited_capabilities_for_registry_engine(self, state: McpState) -> None:
        """The per-engine resource shows the audited cost/result/failure matrix."""
        content = r.render_engine_capability("shodan")
        assert "cost class: freemium" in content
        assert "supported result types: text" in content
        assert "rate_limited" in content and "blocked" in content

    def test_full_capabilities_resource_shows_declared_cost_classes(self, state: McpState) -> None:
        """The full-catalog resource renders declared cost classes and result types."""
        content = r.render_capabilities()
        assert "## internetarchive (disabled) — Internet Archive" in content
        assert "## tmdb — TMDB" in content
        assert "cost class: freemium" in content
        # Registry-backed engines render their declared result-type matrix.
        assert "supported result types: text, media" in content


# ---------------------------------------------------------------------------
# VAL-CAP-005 / VAL-CAP-006 / VAL-CAP-007 / VAL-CAP-008 / VAL-CAP-011
# ---------------------------------------------------------------------------


class TestCapabilityFiltering:
    async def test_family_filter_returns_only_matching(self, state: McpState) -> None:
        """VAL-CAP-005 — family='science' returns entries whose categories include science."""
        result = await t.slopsearx_list_capabilities(family="science")
        assert result["filter"]["family"] == "science"
        assert result["count"] == len(result["engines"])
        assert result["count"] > 0
        assert all("science" in e["categories"] for e in result["engines"])

    async def test_category_filter_returns_only_matching(self, state: McpState) -> None:
        """VAL-CAP-006 — category='general' returns entries whose categories include general."""
        result = await t.slopsearx_list_capabilities(category="general")
        assert result["filter"]["category"] == "general"
        assert result["count"] == len(result["engines"])
        assert result["count"] > 0
        assert all("general" in e["categories"] for e in result["engines"])

    async def test_include_disabled_controls_visibility(self, state: McpState) -> None:
        """VAL-CAP-007 — default excludes disabled; include_disabled=true includes them."""
        default = await t.slopsearx_list_capabilities()
        assert all(e["enabled"] is True for e in default["engines"])

        with_disabled = await t.slopsearx_list_capabilities(include_disabled=True)
        assert any(e["enabled"] is False for e in with_disabled["engines"])
        assert len(with_disabled["engines"]) > len(default["engines"])

    async def test_include_auth_requirements_false_omits_auth(self, state: McpState) -> None:
        """VAL-CAP-008 — include_auth_requirements=false omits the auth key entirely."""
        result = await t.slopsearx_list_capabilities(include_auth_requirements=False)
        assert result["engines"]
        assert all("auth" not in e for e in result["engines"])

    async def test_unknown_family_and_category_empty_no_error(self, state: McpState) -> None:
        """VAL-CAP-011 — unknown family/category yields count=0, empty list, no error."""
        for kw in ({"family": "nonexistent"}, {"category": "nonexistent"}):
            result = await t.slopsearx_list_capabilities(**kw)
            assert result["count"] == 0
            assert result["engines"] == []
            assert "error" not in result


# ---------------------------------------------------------------------------
# VAL-CAP-009 / VAL-CAP-010 / VAL-CAP-012 / VAL-CAP-016
# ---------------------------------------------------------------------------


class TestCapabilityResources:
    def test_single_engine_resource_describes_engine_and_agrees(self, state: McpState) -> None:
        """VAL-CAP-009 — the capabilities/brave resource agrees with the catalog entry."""
        content = r.render_engine_capability("brave")
        cap = state.catalog.get("brave")
        assert cap is not None
        assert "brave" in content
        assert cap.display_name in content
        assert "auth class:" in content
        assert "sensitive" in content.lower()
        assert "supported filters" in content.lower()
        assert "supported result types" in content.lower()

    def test_unknown_engine_resource_lists_valid_names(self, state: McpState) -> None:
        """VAL-CAP-010 — unknown engine resource states Unknown engine and enumerates valid names."""
        content = r.render_engine_capability("no_such_engine")
        assert "Unknown engine" in content
        assert "Valid engines" in content
        # It should actually list some real engines.
        for name in ("wikipedia", "brave"):
            assert name in content

    def test_routing_profiles_list_all_intents_and_strategies(self, state: McpState) -> None:
        """VAL-CAP-012 — routing-profiles lists every intent and the research strategies section."""
        content = r.render_routing_profiles()
        for intent in INTENT_PROFILES:
            assert f"## {intent}" in content, f"intent '{intent}' missing from routing-profiles"
        assert "# Research strategies" in content
        for strategy in STRATEGIES:
            assert f"- {strategy}" in content

    def test_resources_are_readonly_deterministic_and_leak_no_secrets(self, state: McpState) -> None:
        """VAL-CAP-016 — capability resources are idempotent and expose no secrets/config dumps."""
        for render in (r.render_capabilities, r.render_routing_profiles):
            first = render()
            second = render()
            assert first == second  # deterministic / safe to cache
            assert "api_key" not in first.lower()
            assert "os.environ" not in first.lower()
            assert "MCP_AUTH_TOKEN" not in first.upper() or "MCP_AUTH_TOKEN" not in first

    def test_full_capabilities_resource_lists_all(self, state: McpState) -> None:
        """The full catalog resource covers every registered engine."""
        content = r.render_capabilities()
        for name in list(list_engines().keys()):
            assert name in content

    # -- Operational diagnostics resource (feature: operational-diagnostics) --

    def test_health_resource_matches_status_tool(self, state: McpState) -> None:
        """VAL-DIAG-010 — the health resource reports the same version/valkey/count/bounds as the tool."""
        content = r.render_health_summary()
        diag = t.service_diagnostics(state)
        assert f"- version: {diag['version']}" in content
        assert f"- contract version: {diag['contract_version']}" in content
        assert f"- valkey connected: {diag['valkey']['connected']}" in content
        assert f"- valkey fail-closed: {diag['valkey']['fail_closed']}" in content
        assert f"- active engines: {diag['active_engines']}" in content
        assert f"- cache connected: {diag['cache_connected']}" in content
        assert f"- snapshots available: {diag['snapshots_available']}" in content
        assert f"max_query_length={diag['policy_bounds']['max_query_length']}" in content
        assert f"max_results={diag['policy_bounds']['max_results']}" in content

    def test_health_resource_leaks_no_secrets_or_environment(self, state: McpState) -> None:
        """VAL-DIAG-011/012/013 — the health resource is redacted: no secrets, env, or metrics dump."""
        state.policy.auth_token = "sentinel-token-abc"
        content = r.render_health_summary()
        assert "sentinel-token-abc" not in content
        assert "api_key" not in content.lower()
        assert "os.environ" not in content
        assert "MCP_AUTH_TOKEN" not in content.upper()
        assert "# HELP" not in content and "# TYPE" not in content


# ---------------------------------------------------------------------------
# VAL-CAP-013 / VAL-CAP-014 / VAL-CROSS-009 — scope preview
# ---------------------------------------------------------------------------


class TestScopePreview:
    async def test_preview_performs_no_search_and_no_rate_limit(self, state: McpState) -> None:
        """VAL-CAP-013 — preview returns routing fields and performs no search / no rate limit."""
        for engine in state.ctx.active_engines.values():
            assert engine.calls == 0
        result = await t.slopsearx_explain_search_scope("hello", intent="web")
        assert "error" not in result
        assert "selected_engines" in result
        assert "excluded_engines" in result
        assert "routing_reason" in result
        assert "matched_topic" in result
        assert "warnings" in result
        assert "results" not in result
        assert "cursor" not in result
        for engine in state.ctx.active_engines.values():
            assert engine.calls == 0  # no engine is ever queried

    async def test_preview_honors_explicit_engine_override(self, state: McpState) -> None:
        """VAL-CAP-014 — engines=['brave'] selects brave; unknown engine is rejected."""
        ok = await t.slopsearx_explain_search_scope("hello", engines=["brave"])
        assert "error" not in ok
        assert ok["selected_engines"] == ["brave"]

        bad = await t.slopsearx_explain_search_scope("hello", engines=["no_such_engine"])
        assert bad["error"]["code"] == "invalid_scope"
        assert "valid_alternatives" in bad["error"]

    async def test_preview_matches_executed_scope(self, state: McpState) -> None:
        """VAL-CROSS-009 — preview selected_engines matches the executed search scope."""
        preview = await t.slopsearx_explain_search_scope("hello", intent="auto")
        search = await t.slopsearx_search("hello", intent="auto")
        assert "error" not in preview
        assert "error" not in search
        assert set(preview["selected_engines"]) == set(search["scope"]["selected_engines"])
        assert preview["routing_reason"] == search["scope"]["routing_reason"]


# ---------------------------------------------------------------------------
# Feature description — filter-enforcement resolves against the dispatched scope
# ---------------------------------------------------------------------------


class TestEnforcementAgainstDispatchedScope:
    async def test_category_route_enforcement_uses_dispatched_scope(self) -> None:
        """The enforcement report resolves supported_filters against dispatched engines, not all active."""
        engines_map = {
            "a_sci": _MockEngine("a_sci", categories=["science"], supported_filters={"time_range": True}),
            "b_sci": _MockEngine("b_sci", categories=["science"]),
            "c_news": _MockEngine("c_news", categories=["news"], supported_filters={"time_range": True}),
        }
        state_obj = _build_state(engine_names=["a_sci", "b_sci", "c_news"], engines_map=engines_map)
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("hello", categories=["science"], time_range="week")
        finally:
            set_state(None)

        assert "error" not in result
        # Dispatched scope is the two science engines only.
        assert set(result["scope"]["selected_engines"]) == {"a_sci", "b_sci"}
        entry = result["enforcement"]["time_range"]
        # c_news supports time_range but was NOT dispatched → it must not appear as enforcing.
        assert entry["enforced_by"] == ["a_sci"]
        assert entry["status"] == "partially_enforced"

    async def test_category_route_all_dispatch_support_yields_enforced(self) -> None:
        """When every dispatched engine supports the filter, status is enforced (not all-active unsupported)."""
        engines_map = {
            "a_sci": _MockEngine("a_sci", categories=["science"], supported_filters={"time_range": True}),
            "b_sci": _MockEngine("b_sci", categories=["science"], supported_filters={"time_range": True}),
        }
        state_obj = _build_state(engine_names=["a_sci", "b_sci"], engines_map=engines_map)
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("hello", categories=["science"], time_range="week")
        finally:
            set_state(None)

        assert "error" not in result
        entry = result["enforcement"]["time_range"]
        assert entry["status"] == "enforced"
        assert set(entry["enforced_by"]) == {"a_sci", "b_sci"}

    async def test_explicit_engine_enforcement_still_uses_selected_scope(self) -> None:
        """Targeted enforcement remains computed against the selected engines only."""
        engines_map = {
            "wikipedia": _MockEngine("wikipedia", supported_filters={"language": True}),
            "brave": _MockEngine("brave"),
            "duckduckgo": _MockEngine("duckduckgo", supported_filters={"language": True}),
        }
        state_obj = _build_state(engine_names=["wikipedia", "brave", "duckduckgo"], engines_map=engines_map)
        set_state(state_obj)
        try:
            result = await t.slopsearx_search_targeted("hello", engines=["wikipedia", "brave"], language="de")
        finally:
            set_state(None)

        assert "error" not in result
        entry = result["enforcement"]["language"]
        assert entry["status"] == "partially_enforced"
        assert entry["enforced_by"] == ["wikipedia"]
        assert "duckduckgo" not in entry["enforced_by"]
