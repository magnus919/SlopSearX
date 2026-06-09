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

    with TestClient(app) as tc:
        # Set mock engine AFTER startup runs (which calls discover_engines)
        server_mod._active_engines = {
            "mocktest": _MockEngine(),
        }
        yield tc

    # Restore original state
    server_mod._active_engines = original_engines


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
