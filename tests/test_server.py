"""Tests for FastAPI server — /search and /health endpoints."""

from __future__ import annotations

import dataclasses

import pytest
from fastapi.testclient import TestClient

import engines  # noqa: F401 — triggers @register_engine
from slopsearx.adapter import (
    AdapterResponse,
    EngineAdapter,
    EngineStatus,
    SearchResult,
    register_engine,
)
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


# ---------------------------------------------------------------------------
# /search endpoint
# ---------------------------------------------------------------------------


class TestSearchEndpoint:
    """GET /search endpoint."""

    def test_basic_search(self, client: TestClient) -> None:
        """Basic search returns JSON with results."""
        response = client.get("/search", params={"q": "test query"})

        assert response.status_code == 200
        data = response.json()
        assert data["query"] == "test query"
        assert data["number_of_results"] == 3
        assert len(data["results"]) == 3
        assert "meta" in data

    def test_missing_query(self, client: TestClient) -> None:
        """Missing q parameter returns 400."""
        response = client.get("/search")

        assert response.status_code == 400
        data = response.json()
        assert data["error"] == "query_required"

    def test_empty_query(self, client: TestClient) -> None:
        """Empty q parameter returns 400."""
        response = client.get("/search", params={"q": ""})

        assert response.status_code == 400
        data = response.json()
        assert data["error"] == "query_required"

    def test_whitespace_only_query(self, client: TestClient) -> None:
        """Whitespace-only query returns 400."""
        response = client.get("/search", params={"q": "   "})

        assert response.status_code == 400

    def test_yaml_format(self, client: TestClient) -> None:
        """format=yaml returns YAML+Markdown response."""
        response = client.get("/search", params={"q": "test", "format": "yaml"})

        assert response.status_code == 200
        assert "text/vnd.yaml+markdown" in response.headers["content-type"]
        assert "test" in response.text
        assert "## Results Summary" in response.text

    def test_json_format_default(self, client: TestClient) -> None:
        """format=json is the default."""
        response = client.get("/search", params={"q": "test"})

        assert response.status_code == 200
        assert "application/json" in response.headers["content-type"]

    def test_unresponsive_engine(self, client: TestClient) -> None:
        """Error from engine is reported in unresponsive_engines."""
        response = client.get("/search", params={"q": "error"})

        assert response.status_code == 503  # all engines unresponsive
        data = response.json()
        assert len(data["unresponsive_engines"]) == 1
        assert data["unresponsive_engines"][0][0] == "mocktest"

    def test_suggestions_always_present(self, client: TestClient) -> None:
        """suggestions field is always present (may be empty)."""
        response = client.get("/search", params={"q": "test"})

        data = response.json()
        assert "suggestions" in data
        assert isinstance(data["suggestions"], list)

    def test_meta_fields(self, client: TestClient) -> None:
        """meta.* extension fields are present."""
        response = client.get("/search", params={"q": "test"})

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

        disabled_response = client.get("/search", params={"q": "diagnostic-disabled"})

        assert "empty_engines" not in disabled_response.json()["meta"]

        server_mod._empty_scrape_diagnostics_enabled = True

        response = client.get("/search", params={"q": "diagnostic-enabled"})

        assert response.status_code == 200
        data = response.json()
        assert data["unresponsive_engines"] == []
        assert data["meta"]["empty_engines"] == [["emptyscrape", "successful scrape returned no results"]]

    def test_engines_filter(self, client: TestClient) -> None:
        """engines parameter filters which engines to use."""
        response = client.get("/search", params={"q": "test", "engines": "mocktest"})

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
        response = client.get("/search", params={"q": "leak_exception"})

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


# ---------------------------------------------------------------------------
# /health endpoint
# ---------------------------------------------------------------------------


class TestHealthEndpoint:
    """GET /health endpoint."""

    def test_health_ok(self, client: TestClient) -> None:
        """Health check returns status with per-engine info."""
        response = client.get("/health")

        assert response.status_code == 200
        data = response.json()
        assert "status" in data
        assert "version" in data
        assert "engines" in data
        assert "mocktest" in data["engines"]
        # Mock engine health uses search("healthcheck") which returns
        # normal results, so status should be OK
        assert data["engines"]["mocktest"]["status"] == "ok"

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
