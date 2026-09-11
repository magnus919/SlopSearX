"""Deterministic SearXNG HTTP API contract checks.

The fixture is a pinned, sanitized reference of the SearXNG API surface. The
tests intentionally assert required fields and behavior, not result relevance;
SlopSearX-specific metadata and formats remain additive.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Iterator, cast

import httpx
import pytest
from fastapi.testclient import TestClient

from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult
from slopsearx.server import app

FIXTURE_PATH = Path(__file__).parent / "fixtures" / "searxng_api_contract.json"
CONTRACT = json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))


class _ContractEngine(EngineAdapter):
    """A deterministic result source; no external engine traffic is allowed."""

    name = "contract"
    display_name = "Contract Engine"
    engine_type = "api"
    categories = ["general", "science"]

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        del params
        return AdapterResponse(
            results=[
                SearchResult(
                    url="https://example.test/path;param?q=one#section",
                    title=f"Contract result for {query}",
                    content="Deterministic contract result",
                    engine=self.name,
                )
            ],
            status=EngineStatus.OK,
            latency_ms=1.0,
        )


@pytest.fixture
def client(monkeypatch: pytest.MonkeyPatch) -> Iterator[TestClient]:
    """Run the HTTP surface with one deterministic local adapter."""
    import slopsearx.server as server_module

    monkeypatch.setattr(server_module, "_active_engines", {"contract": _ContractEngine()})
    monkeypatch.setattr(server_module, "_router", None)
    with TestClient(app) as test_client:
        yield test_client


def _request(client: TestClient, case: dict[str, Any]) -> httpx.Response:
    method = case["method"].lower()
    request = getattr(client, method)
    if method == "post":
        return cast(httpx.Response, request(case["path"], data=case["params"]))
    return cast(httpx.Response, request(case["path"], params=case["params"]))


@pytest.mark.parametrize("case", CONTRACT["request_matrix"], ids=lambda case: case["name"])
def test_request_matrix_matches_pinned_searxng_contract(client: TestClient, case: dict[str, Any]) -> None:
    response = _request(client, case)

    assert response.status_code == case["status"]
    assert response.headers["content-type"].startswith(case["media_type"])
    if "error" in case:
        assert response.json()["error"] == case["error"]


def test_disabled_formats_fail_closed_like_reference(client: TestClient, monkeypatch: pytest.MonkeyPatch) -> None:
    import slopsearx.server as server_module

    config = server_module._health_config()
    monkeypatch.setattr(config.search, "formats", ["html", "json"])
    for output_format, expected in CONTRACT["disabled_formats"].items():
        response = client.get("/search", params={"q": "contract", "format": output_format})
        assert response.status_code == expected["status"]
        assert response.headers["content-type"].startswith(expected["media_type"])


def test_healthz_and_config_shape_match_reference(client: TestClient) -> None:
    healthz = client.get("/healthz")
    assert healthz.status_code == 200
    assert healthz.text == "OK"

    config = client.get("/config")
    assert config.status_code == 200
    data = config.json()
    assert isinstance(data["categories"], list)
    assert CONTRACT["config"]["categories_type"] == "list"
    assert set(CONTRACT["config"]["required_top_level"]) <= data.keys()
    assert data["engines"]
    assert set(CONTRACT["config"]["required_engine_fields"]) <= data["engines"][0].keys()
    assert "api_key" not in data["engines"][0]


def test_result_fields_are_required_but_slopsearx_extensions_are_allowed(client: TestClient) -> None:
    response = client.get("/search", params={"q": "contract", "format": "json"})
    data = response.json()
    result = data["results"][0]

    assert set(CONTRACT["config"]["required_result_fields"]) <= result.keys()
    assert result["parsed_url"] == CONTRACT["config"]["parsed_url"]
    assert "meta" in data
    assert "tier" in result

    yaml_response = client.get("/search", params={"q": "contract", "format": "yaml"})
    assert yaml_response.status_code == 200
    assert yaml_response.headers["content-type"].startswith("text/vnd.yaml+markdown")


def test_dependency_dossier_grant_does_not_change_http_search(
    client: TestClient, monkeypatch: pytest.MonkeyPatch
) -> None:
    params = {"q": "contract", "format": "json"}
    monkeypatch.delenv("MCP_GRANT_DEPENDENCY_DOSSIER", raising=False)
    disabled = client.get("/search", params=params)
    monkeypatch.setenv("MCP_GRANT_DEPENDENCY_DOSSIER", "1")
    enabled = client.get("/search", params=params)

    assert disabled.status_code == enabled.status_code == 200
    disabled_data = disabled.json()
    enabled_data = enabled.json()
    disabled_data["meta"].pop("query_id")
    enabled_data["meta"].pop("query_id")
    assert disabled_data == enabled_data
