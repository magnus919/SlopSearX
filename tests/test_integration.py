"""End-to-end integration tests for the HTTP API.

These tests exercise the full request pipeline — routing, dispatching,
caching, and formatting — using FastAPI's TestClient.  No real engine API
keys are needed; the server gracefully handles engines that fail to
authenticate.
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

    def test_health_includes_timestamp(self) -> None:
        response = client.get("/health")
        assert response.status_code == 200
        data = response.json()
        assert "timestamp" in data

    def test_health_includes_engine_status(self) -> None:
        response = client.get("/health")
        assert response.status_code == 200
        data = response.json()
        assert "engines" in data
        assert isinstance(data["engines"], dict)


class TestSearchEndpoint:
    """Verify search pipeline end-to-end."""

    def test_search_json_returns_200(self) -> None:
        response = client.get("/search", params={"q": "test", "format": "json"})
        assert response.status_code == 200

    def test_search_json_has_expected_fields(self) -> None:
        response = client.get("/search", params={"q": "test", "format": "json"})
        assert response.status_code == 200
        data = response.json()
        assert "query" in data
        assert "results" in data
        assert "number_of_results" in data

    def test_search_yaml_returns_200(self) -> None:
        response = client.get("/search", params={"q": "test", "format": "yaml"})
        assert response.status_code == 200
        assert response.text  # non-empty output

    def test_search_missing_query_returns_error(self) -> None:
        response = client.get("/search")
        assert response.status_code == 422  # FastAPI validation error

    def test_search_with_category_filter(self) -> None:
        response = client.get(
            "/search", params={"q": "test", "format": "json", "categories": "general"}
        )
        assert response.status_code == 200

    def test_search_with_engine_filter(self) -> None:
        response = client.get(
            "/search",
            params={"q": "python", "format": "json", "engines": "wikipedia"},
        )
        assert response.status_code == 200


class TestConfigEndpoint:
    """Verify config discovery."""

    def test_config_returns_200(self) -> None:
        response = client.get("/config")
        assert response.status_code == 200

    def test_config_has_categories(self) -> None:
        response = client.get("/config")
        assert response.status_code == 200
        data = response.json()
        assert isinstance(data, dict)
        assert len(data) > 0


class TestServerMetadata:
    """Verify server identification and metrics."""

    def test_root_returns_200(self) -> None:
        response = client.get("/")
        assert response.status_code == 200

    def test_metrics_returns_200(self) -> None:
        response = client.get("/metrics")
        assert response.status_code == 200
        assert "slopsearx" in response.text.lower()

    def test_x_request_id_propagated(self) -> None:
        response = client.get(
            "/health", headers={"X-Request-ID": "test-trace-001"}
        )
        assert response.status_code == 200
        assert response.headers.get("X-Request-ID") == "test-trace-001"

    def test_x_request_id_generated(self) -> None:
        response = client.get("/health")
        assert response.status_code == 200
        assert "X-Request-ID" in response.headers
        assert len(response.headers["X-Request-ID"]) > 0
