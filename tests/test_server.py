"""Tests for FastAPI server — /search and /health endpoints."""

from __future__ import annotations

import dataclasses

import pytest
import yaml
from fastapi.testclient import TestClient

import engines  # noqa: F401 — triggers @register_engine
from slopsearx.adapter import (
    AdapterResponse,
    EngineAdapter,
    EngineStatus,
    SearchResult,
    register_engine,
)
from slopsearx.capabilities import MCPPolicy
from slopsearx.config import load_config
from slopsearx.server import app

# ---------------------------------------------------------------------------
# Test engine — mock adapter for controlled test scenarios
# ---------------------------------------------------------------------------


@register_engine
class _MockEngine(EngineAdapter):
    """Mock engine used only in tests — not registered during normal startup."""

    name = "mocktest"
    display_name = "Mock Test Engine"
    env_prefix = "ENGINE_MOCKTEST"
    engine_type = "api"
    categories = ["general", "news", "tech", "science"]

    async def search(self, query, params=None):
        if query == "error":
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message="simulated error",
            )
        if query == "timeout_sim":
            return AdapterResponse(
                results=[],
                status=EngineStatus.TIMEOUT,
                error_message="simulated timeout",
            )
        if query == "blocked":
            return AdapterResponse(
                results=[],
                status=EngineStatus.BLOCKED,
                error_message="CAPTCHA detected",
            )
        if query == "rate_limited":
            return AdapterResponse(
                results=[],
                status=EngineStatus.RATE_LIMITED,
                error_message="too many requests",
            )
        if query == "leak_exception":
            # Raise an exception with an embedded URL to test server-level sanitization
            raise RuntimeError(
                "Client error '403 Forbidden' for url 'https://api.example.com/search?key=secret-key-12345&q=test'"
            )

        # Normal response
        return AdapterResponse(
            results=[
                SearchResult(
                    url=f"https://mock{i}.com",
                    title=f"Mock Result {i}",
                    content=f"Content for mock result {i}.",
                    engine=self.name,
                )
                for i in range(3)
            ],
            status=EngineStatus.OK,
            latency_ms=42.0,
        )


class _EmptyScrapeEngine(EngineAdapter):
    """Successful scrape response with no parsed results."""

    name = "emptyscrape"
    engine_type = "scrape"
    categories = ["general"]

    async def search(self, query, params=None):
        return AdapterResponse(results=[], status=EngineStatus.OK)


class _SurrogatePayloadEngine(EngineAdapter):
    """Mock engine returning a payload containing a lone UTF-16 surrogate."""

    name = "surrogatepayload"
    engine_type = "api"
    categories = ["general"]

    async def search(self, query, params=None):
        return AdapterResponse(
            results=[
                SearchResult(
                    url="https://example.com/result",
                    title="Surrogate payload",
                    content="snippet",
                    engine=self.name,
                    engines={self.name},
                    payload={"domain": "security", "type": "vulnerability", "data": {"note": "\ud800"}},
                )
            ],
            status=EngineStatus.OK,
            latency_ms=1.0,
        )


# ---------------------------------------------------------------------------
# Test fixture: server with mock engines enabled
# ---------------------------------------------------------------------------


@pytest.fixture
def client() -> TestClient:
    """Test client with mock engine as the only active engine.

    We modify the server's _active_engines directly to control
    what engines are available during tests.
    """
    import slopsearx.server as server_mod

    # Save original state
    original_engines = dict(server_mod._active_engines)
    original_empty_scrape_diagnostics = server_mod._empty_scrape_diagnostics_enabled
    original_portal_policy = server_mod._portal_policy

    with TestClient(app) as tc:
        # Set mock engine AFTER startup runs (which calls discover_engines)
        server_mod._active_engines = {
            "mocktest": _MockEngine(),
        }
        # Disable query router so mock engines aren't filtered to Tier 1
        server_mod._router = None
        yield tc

    # Restore original state
    server_mod._active_engines = original_engines
    server_mod._empty_scrape_diagnostics_enabled = original_empty_scrape_diagnostics
    server_mod._portal_policy = original_portal_policy


# ---------------------------------------------------------------------------
# Cache hit handling (service-level tests live in test_service.py)
# ---------------------------------------------------------------------------


# ---------------------------------------------------------------------------
# /search endpoint
# ---------------------------------------------------------------------------


class TestSearchEndpoint:
    """GET /search endpoint."""

    def test_basic_search(self, client: TestClient) -> None:
        """Basic search returns JSON with results."""
        response = client.get("/search", params={"q": "test query", "format": "json"})

        assert response.status_code == 200
        data = response.json()
        assert data["query"] == "test query"
        assert data["number_of_results"] == 3
        assert len(data["results"]) == 3
        assert "meta" in data

    @pytest.mark.parametrize("path", ["/", "/search"])
    @pytest.mark.parametrize("method", ["get", "post"])
    def test_searxng_routes_and_methods(self, client: TestClient, path: str, method: str) -> None:
        """SearXNG clients can use either search route and GET or form POST."""
        request = getattr(client, method)
        kwargs = {"params": {"q": "route compatibility", "format": "json"}}
        if method == "post":
            kwargs = {"data": {"q": "route compatibility", "format": "json"}}

        response = request(path, **kwargs)

        assert response.status_code == 200
        assert response.json()["query"] == "route compatibility"

    @pytest.mark.parametrize("saved_grant", ["0", "1"])
    @pytest.mark.parametrize("receipt_grant", ["0", "1"])
    def test_additive_mcp_grants_do_not_change_searxng_json(
        self,
        client: TestClient,
        monkeypatch,
        saved_grant: str,
        receipt_grant: str,
    ) -> None:
        monkeypatch.setenv("MCP_GRANT_SAVED_SEARCHES", saved_grant)
        monkeypatch.setenv("MCP_GRANT_RETRIEVAL_RECEIPTS", receipt_grant)
        response = client.get("/search", params={"q": "compatibility", "format": "json"})
        assert response.status_code == 200
        data = response.json()
        assert data["query"] == "compatibility"
        assert data["number_of_results"] == 3
        assert all("url" in result and "title" in result for result in data["results"])

    def test_missing_query(self, client: TestClient) -> None:
        """Missing q parameter returns 400."""
        response = client.get("/search", params={"format": "json"})

        assert response.status_code == 400
        data = response.json()
        assert data["error"] == "query_required"

    def test_empty_query(self, client: TestClient) -> None:
        """Empty q parameter returns 400."""
        response = client.get("/search", params={"q": "", "format": "json"})

        assert response.status_code == 400
        data = response.json()
        assert data["error"] == "query_required"

    def test_whitespace_only_query(self, client: TestClient) -> None:
        """Whitespace-only query returns 400."""
        response = client.get("/search", params={"q": "   ", "format": "json"})

        assert response.status_code == 400

    @pytest.mark.parametrize("output_format", ["json", "yaml"])
    def test_interactive_deadline(self, client: TestClient, monkeypatch, output_format) -> None:
        import asyncio

        import slopsearx.server as server_mod
        from slopsearx.mcp.harness import InMemoryStore

        monkeypatch.setattr(server_mod, "_cache", InMemoryStore())
        original = _MockEngine.search

        async def delayed(engine, query, params=None):
            await asyncio.sleep(1)
            return await original(engine, query, params)

        monkeypatch.setattr(_MockEngine, "search", delayed)
        response = client.get(
            "/search", params={"q": "deadline", "format": output_format, "interactive_timeout_ms": 10}
        )
        assert response.status_code == 503
        data = response.json() if output_format == "json" else yaml.safe_load(response.text.split("\n---\n", 1)[0])
        assert data["meta"]["deadline_exceeded"]
        assert client.get("/search", params={"q": "deadline", "interactive_timeout_ms": 0}).status_code == 400

    def test_default_format_is_html(self, client: TestClient) -> None:
        response = client.get("/search", params={"q": "test"})

        assert response.status_code == 200
        assert "text/html" in response.headers["content-type"]
        assert "Search results for test" in response.text

    @pytest.mark.parametrize(
        ("output_format", "media_type", "marker"),
        [
            ("html", "text/html", "Search results for test"),
            ("json", "application/json", '"results"'),
            ("csv", "text/csv", "title,url,content,engine,publishedDate"),
            ("rss", "application/rss+xml", "<rss"),
            ("yaml", "text/vnd.yaml+markdown", "## Results Summary"),
        ],
    )
    def test_supported_formats(self, client: TestClient, output_format: str, media_type: str, marker: str) -> None:
        response = client.get("/search", params={"q": "test", "format": output_format})

        assert response.status_code == 200
        assert media_type in response.headers["content-type"]
        assert marker in response.text

    def test_accept_header_negotiates_json(self, client: TestClient) -> None:
        response = client.get("/search", params={"q": "test"}, headers={"Accept": "application/json"})

        assert response.status_code == 200
        assert "application/json" in response.headers["content-type"]

    def test_disabled_format_returns_403(self, client: TestClient, monkeypatch: pytest.MonkeyPatch) -> None:
        import slopsearx.server as server_mod

        config = server_mod._health_config()
        monkeypatch.setattr(config.search, "formats", ["html", "json"])

        response = client.get("/search", params={"q": "test", "format": "csv"})

        assert response.status_code == 403
        assert "text/csv" in response.headers["content-type"]

    @pytest.mark.parametrize("params", [{"pageno": 0}, {"safesearch": 3}, {"pageno": "not-an-int"}])
    def test_invalid_search_parameters_return_400(self, client: TestClient, params: dict[str, object]) -> None:
        response = client.get("/search", params={"q": "test", "format": "json", **params})

        assert response.status_code == 400
        assert response.json()["error"] == "invalid_filter"

    def test_missing_query_default_html_error(self, client: TestClient) -> None:
        response = client.get("/search")

        assert response.status_code == 400
        assert "text/html" in response.headers["content-type"]
        assert "query_required" in response.text

    def test_root_without_query_opens_portal_landing_page(self, client: TestClient) -> None:
        response = client.get("/")

        assert response.status_code == 200
        assert "text/html" in response.headers["content-type"]
        assert "Search <em>SlopSearX.</em>" in response.text
        assert 'action="/search"' in response.text
        assert "data-theme-toggle" in response.text

    def test_portal_html_responses_include_security_headers(self, client: TestClient) -> None:
        landing = client.get("/")
        assert landing.status_code == 200
        assert "default-src 'self'" in landing.headers["content-security-policy"]
        assert landing.headers["referrer-policy"] == "no-referrer"
        assert landing.headers["x-content-type-options"] == "nosniff"
        assert "camera=()" in landing.headers["permissions-policy"]

        results = client.get("/search", params={"q": "headers"})
        assert results.status_code == 200
        assert "default-src 'self'" in results.headers["content-security-policy"]

    def test_sensitive_engine_selection_is_rejected_before_dispatch(
        self, client: TestClient, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        import slopsearx.server as server_mod

        monkeypatch.setattr(
            server_mod,
            "_portal_policy",
            MCPPolicy(sensitive_engines={"mocktest"}, targeted_sensitive_allowed=False),
        )
        response = client.get("/search", params={"q": "private", "engines": "mocktest", "format": "json"})

        assert response.status_code == 403
        assert response.json() == {
            "error": "engine_restricted",
            "message": "Explicit selection of one or more sensitive sources requires operator authorization.",
            "field": "engines",
            "engines": ["mocktest"],
        }

    def test_sensitive_engine_selection_is_allowed_with_operator_grant(
        self, client: TestClient, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        import slopsearx.server as server_mod

        monkeypatch.setattr(
            server_mod,
            "_portal_policy",
            MCPPolicy(sensitive_engines={"mocktest"}, targeted_sensitive_allowed=True),
        )
        response = client.get("/search", params={"q": "private", "engines": "mocktest", "format": "json"})

        assert response.status_code == 200
        assert response.json()["number_of_results"] == 3

    def test_root_machine_format_without_query_keeps_error_contract(self, client: TestClient) -> None:
        response = client.get("/", params={"format": "json"})

        assert response.status_code == 400
        assert response.json()["error"] == "query_required"

    def test_portal_default_theme_can_be_configured(self, client: TestClient, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("SLOPSEARX_PORTAL_DEFAULT_THEME", "darker")
        response = client.get("/")

        assert response.status_code == 200
        assert 'data-default-theme="darker"' in response.text

    def test_html_results_expose_capability_aware_scope_and_pagination(self, client: TestClient) -> None:
        response = client.get(
            "/search",
            params={"q": "test", "categories": "general", "pageno": 2, "time_range": "month"},
        )

        assert response.status_code == 200
        assert "Scope and filters" in response.text
        assert 'name="categories"' in response.text
        assert 'value="general"' in response.text
        assert "Past month" in response.text
        assert "← Previous" in response.text
        assert "Try next page →" in response.text

    def test_strict_safesearch_is_rejected_before_dispatch(self, client: TestClient) -> None:
        response = client.get("/search", params={"q": "test", "safesearch": 2, "format": "json"})

        assert response.status_code == 400
        assert response.json()["error"] == "invalid_filter"
        assert response.json()["field"] == "safesearch"

    def test_yaml_format(self, client: TestClient) -> None:
        """format=yaml returns YAML+Markdown response."""
        response = client.get("/search", params={"q": "test", "format": "yaml"})

        assert response.status_code == 200
        assert "text/vnd.yaml+markdown" in response.headers["content-type"]
        assert "test" in response.text
        assert "## Results Summary" in response.text

    @pytest.mark.parametrize("formats", [("yaml", "yaml"), ("json", "yaml"), ("yaml", "json"), ("json", "json")])
    def test_cached_format(self, client: TestClient, monkeypatch, formats) -> None:
        """A shared canonical cache honors each caller's output format."""
        import slopsearx.server as server_mod
        from slopsearx.mcp.harness import InMemoryStore

        monkeypatch.setattr(server_mod, "_cache", InMemoryStore())
        calls = 0
        original = _MockEngine.search

        async def counted_search(self, query, params=None):
            nonlocal calls
            calls += 1
            return await original(self, query, params)

        monkeypatch.setattr(_MockEngine, "search", counted_search)
        urls = []
        for index, output_format in enumerate(formats):
            response = client.get("/search", params={"q": "format regression", "format": output_format})
            assert response.status_code == 200
            if output_format == "yaml":
                assert "text/vnd.yaml+markdown" in response.headers["content-type"]
                assert "## Results Summary" in response.text
                data = yaml.safe_load(response.text.split("\n---\n", 1)[0])
            else:
                assert "application/json" in response.headers["content-type"]
                data = response.json()
            assert data["meta"]["cached"] is bool(index)
            urls.append([result["url"] for result in data["results"]])
        assert urls[0] == urls[1]
        assert calls == 1

    def test_yaml_all_engines_failed(self, client: TestClient) -> None:
        response = client.get("/search", params={"q": "error", "format": "yaml"})
        assert response.status_code == 503
        assert "text/vnd.yaml+markdown" in response.headers["content-type"]

    def test_json_format_default(self, client: TestClient) -> None:
        """format=json remains available explicitly."""
        response = client.get("/search", params={"q": "test", "format": "json"})

        assert response.status_code == 200
        assert "application/json" in response.headers["content-type"]

    def test_unresponsive_engine(self, client: TestClient) -> None:
        """Error from engine is reported in unresponsive_engines."""
        response = client.get("/search", params={"q": "error", "format": "json"})

        assert response.status_code == 503  # all engines unresponsive
        data = response.json()
        assert len(data["unresponsive_engines"]) == 1
        assert data["unresponsive_engines"][0][0] == "mocktest"

    def test_suggestions_always_present(self, client: TestClient) -> None:
        """suggestions field is always present (may be empty)."""
        response = client.get("/search", params={"q": "test", "format": "json"})

        data = response.json()
        assert "suggestions" in data
        assert isinstance(data["suggestions"], list)

    def test_meta_fields(self, client: TestClient) -> None:
        """meta.* extension fields are present."""
        response = client.get("/search", params={"q": "test", "format": "json"})

        data = response.json()
        meta = data["meta"]
        assert "response_time_ms" in meta
        assert "cached" in meta
        assert "query_id" in meta
        assert "engine_status" in meta
        assert meta["query_id"].startswith("ssx-")
        assert isinstance(meta["cached"], bool)

    def test_empty_scrape_diagnostic_is_opt_in(self, client: TestClient) -> None:
        """An empty scrape is visible without being marked unresponsive."""
        import slopsearx.server as server_mod

        server_mod._active_engines = {"emptyscrape": _EmptyScrapeEngine()}
        server_mod._empty_scrape_diagnostics_enabled = False

        disabled_response = client.get("/search", params={"q": "diagnostic-disabled", "format": "json"})

        assert "empty_engines" not in disabled_response.json()["meta"]

        server_mod._empty_scrape_diagnostics_enabled = True

        response = client.get("/search", params={"q": "diagnostic-enabled", "format": "json"})

        assert response.status_code == 200
        data = response.json()
        assert data["unresponsive_engines"] == []
        assert data["meta"]["empty_engines"] == [["emptyscrape", "successful scrape returned no results"]]

    def test_engines_filter(self, client: TestClient) -> None:
        """engines parameter filters which engines to use."""
        response = client.get("/search", params={"q": "test", "engines": "mocktest", "format": "json"})

        assert response.status_code == 200
        data = response.json()
        assert data["number_of_results"] == 3

    def test_nonexistent_engine_filter(self, client: TestClient) -> None:
        """Filtering to a nonexistent engine returns 503."""
        response = client.get("/search", params={"q": "test", "engines": "nonexistent"})

        assert response.status_code == 503

    def test_dispatch_engine_sanitizes_error_message(self, client: TestClient) -> None:
        """VAL-M1-013: _dispatch_engine broad except handler sanitizes error messages.

        When an adapter raises an exception with a URL containing an API key,
        the server-level handler must sanitize it before returning to the client.
        """
        response = client.get("/search", params={"q": "leak_exception", "format": "json"})

        assert response.status_code == 503  # all engines unresponsive
        data = response.json()
        assert len(data["unresponsive_engines"]) == 1
        error_msg = data["unresponsive_engines"][0][1]
        assert "secret-key-12345" not in error_msg, f"API key found in unresponsive_engines error: {error_msg}"

    def test_query_params_preserved(self, client: TestClient) -> None:
        """Query parameters are accepted without error."""
        response = client.get(
            "/search",
            params={
                "q": "test",
                "language": "fr",
                "pageno": 2,
                "safesearch": 1,
                "time_range": "month",
                "categories": "news,tech",
            },
        )

        assert response.status_code == 200

    def test_lone_surrogate_payload_does_not_500(self, client: TestClient) -> None:
        """A payload with a lone surrogate is sanitized to the canonical form
        (U+FFFD replacement, matching the persistence boundary) and emitted —
        never a 500."""
        import slopsearx.server as server_mod
        from slopsearx.payload import payload_to_dict

        server_mod._active_engines = {"surrogatepayload": _SurrogatePayloadEngine()}
        response = client.get("/search", params={"q": "surrogate", "engines": "surrogatepayload", "format": "json"})
        assert response.status_code == 200
        data = response.json()
        raw = {"domain": "security", "type": "vulnerability", "data": {"note": "\ud800"}}
        assert data["results"][0]["payload"] == payload_to_dict(raw)


# ---------------------------------------------------------------------------
# /health endpoint
# ---------------------------------------------------------------------------


class TestHealthEndpoint:
    """GET /health endpoint."""

    def test_health_ok(self, client: TestClient) -> None:
        """Health check returns liveness plus observed (never fabricated) engine health."""
        response = client.get("/health")

        assert response.status_code == 200
        data = response.json()
        assert "status" in data
        assert "version" in data
        assert "engines" in data
        assert "mocktest" in data["engines"]
        record = data["engines"]["mocktest"]
        # Never-observed engines are explicitly unknown, never optimistically ok.
        assert record["status"] == "unknown"
        assert record["status_at"] is None
        assert record["stale"] is False
        assert record["configured"] is True
        assert "auth_class" in record
        assert "auth_configured" in record
        assert record["circuit_open"] is False
        assert record["circuit_consecutive_errors"] == 0

    def test_healthz_is_searxng_compatible(self, client: TestClient) -> None:
        response = client.get("/healthz")

        assert response.status_code == 200
        assert response.headers["content-type"].startswith("text/plain")
        assert response.text == "OK"

    def test_health_no_engines(self) -> None:
        """Health works even with no engines registered."""
        import slopsearx.server as server_mod

        original = dict(server_mod._active_engines)
        server_mod._active_engines = {}

        try:
            with TestClient(app) as client:
                server_mod._active_engines = {}
                response = client.get("/health")
                assert response.status_code == 200
                data = response.json()
                assert data["status"] == "ok"
                assert data["engines"] == {}
        finally:
            server_mod._active_engines = original


class TestEngineConfigPropagation:
    """Ensures engine config from env vars reaches adapters."""

    def test_env_var_api_key_flows_to_adapter(self, monkeypatch) -> None:
        """ENGINE_BRAVE_API_KEY env var should reach Brave adapter's config."""
        monkeypatch.setenv("ENGINE_BRAVE_API_KEY", "test-key-12345")

        # Re-discover engines with env var set
        cfg = load_config()
        engine_configs = {name: dataclasses.asdict(entry) for name, entry in cfg.engines.items()}

        # Brave config should have the API key
        assert "brave" in engine_configs
        assert engine_configs["brave"]["api_key"] == "test-key-12345"


class TestRoutingBudgetFrozen:
    """The HTTP routing budget is resolved once at startup and frozen.

    The MCP lifespan resolves its budget once at startup (``build_context``
    reads the ``ROUTING_*`` env vars a single time); the HTTP route must
    freeze the same value instead of re-reading ``os.environ`` on every
    ``/search`` request, or the two surfaces' routed scopes and cache
    digests would diverge mid-run (routing-coherence followup).
    """

    def test_runtime_env_change_does_not_alter_budget(self, monkeypatch) -> None:
        """A ROUTING_* env change after startup must not change the budget
        the HTTP routed scope/digest use."""
        import slopsearx.server as server_mod

        # Pin the startup env before the lifespan runs.
        server_mod._routing_budget_cache = None
        monkeypatch.setenv("ROUTING_MAX_ENGINES_PER_INTENT", "5")
        monkeypatch.setenv("ROUTING_MAX_COST_CLASS", "free")
        monkeypatch.setenv("ROUTING_COVERAGE_TARGET", "2")

        try:
            with TestClient(app):
                # The lifespan (``_startup``) resolved the budget from the
                # pinned env; capture that frozen value.
                frozen = server_mod._routing_budget_snapshot()
                assert frozen.max_engines == 5
                assert frozen.max_cost_class == "free"
                assert frozen.coverage_target == 2

                # A runtime ROUTING_* change must not leak into the HTTP
                # routing budget or the per-request AppContext.
                monkeypatch.setenv("ROUTING_MAX_ENGINES_PER_INTENT", "1")
                monkeypatch.setenv("ROUTING_MAX_COST_CLASS", "paid")
                monkeypatch.setenv("ROUTING_COVERAGE_TARGET", "0")

                assert server_mod._routing_budget_snapshot() == frozen
                assert server_mod._current_context().routing_budget == frozen
        finally:
            server_mod._routing_budget_cache = None


def test_invalid_date_window_reports_filter_error(monkeypatch: pytest.MonkeyPatch) -> None:
    import slopsearx.server as server_module

    engine = _MockEngine()
    engine.enforced_filters = {"time_range": "local"}
    monkeypatch.setattr(server_module, "_active_engines", {engine.name: engine})
    with TestClient(app, raise_server_exceptions=True) as client:
        monkeypatch.setattr(server_module, "_active_engines", {engine.name: engine})
        response = client.get(
            "/search",
            params={"q": "climate", "engines": engine.name, "time_range": "all", "format": "json"},
        )
    assert response.status_code == 400
    assert response.json()["error"] == "invalid_filter"
    assert response.json()["field"] == "time_range"
    assert "q" not in response.json()["message"].split()
