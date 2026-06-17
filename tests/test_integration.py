"""End-to-end integration tests for the HTTP API.

These tests exercise the full request pipeline using FastAPI's TestClient.
No real engine API keys or Valkey connection are required; the server
gracefully handles missing infrastructure by returning appropriate error
responses.
"""

from __future__ import annotations

from fastapi.testclient import TestClient

from slopsearx.server import app

client = TestClient(app)


class TestHealthEndpoint:
    """Verify server liveness and health reporting."""

    def test_health_returns_200(self) -> None:
        response = client.get("/health")
        assert response.status_code == 200
        data = response.json()
        assert data["status"] == "ok"

    def test_health_has_expected_fields(self) -> None:
        response = client.get("/health")
        assert response.status_code == 200
        data = response.json()
        assert set(data.keys()) >= {"status", "version", "valkey_connected", "engines"}
        assert isinstance(data["engines"], dict)

    def test_health_valkey_reflects_environment(self) -> None:
        response = client.get("/health")
        assert response.status_code == 200
        data = response.json()
        assert isinstance(data["valkey_connected"], bool)


class TestSearchEndpoint:
    """Verify search pipeline (graceful degradation without Valkey)."""

    def test_search_returns_error_without_valkey(self) -> None:
        """Without Valkey, the cache layer returns 503 to prevent
        dispatching queries that cannot be cached."""
        response = client.get("/search", params={"q": "test", "format": "json"})
        assert response.status_code == 503

    def test_search_error_has_expected_structure(self) -> None:
        response = client.get("/search", params={"q": "test", "format": "json"})
        assert response.status_code == 503
        data = response.json()
        assert "query" in data
        assert data["query"] == "test"

    def test_search_missing_query_rejected(self) -> None:
        response = client.get("/search")
        assert response.status_code in (400, 422)

    def test_search_yaml_returns_response(self) -> None:
        response = client.get("/search", params={"q": "test", "format": "yaml"})
        assert response.status_code in (200, 503)
        assert response.text

    def test_search_with_engine_filter(self) -> None:
        response = client.get(
            "/search",
            params={"q": "python", "format": "json", "engines": "wikipedia"},
        )
        assert response.status_code in (200, 503)


class TestConfigAndMetrics:
    """Verify discovery and observability endpoints."""

    def test_config_returns_200(self) -> None:
        response = client.get("/config")
        assert response.status_code == 200

    def test_config_has_categories(self) -> None:
        response = client.get("/config")
        assert response.status_code == 200
        data = response.json()
        assert isinstance(data, dict)
        assert len(data) > 0

    def test_metrics_returns_200(self) -> None:
        response = client.get("/metrics")
        assert response.status_code == 200
        assert "slopsearx" in response.text.lower()


class TestMiddleware:
    """Verify X-Request-ID middleware."""

    def test_request_id_propagated(self) -> None:
        response = client.get("/health", headers={"X-Request-ID": "test-trace-001"})
        assert response.status_code == 200
        assert response.headers.get("X-Request-ID") == "test-trace-001"

    def test_request_id_generated(self) -> None:
        response = client.get("/health")
        assert response.status_code == 200
        assert "X-Request-ID" in response.headers
        assert len(response.headers["X-Request-ID"]) > 0
