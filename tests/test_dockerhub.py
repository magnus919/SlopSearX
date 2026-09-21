"""Deterministic contract tests for the Docker Hub adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401 — trigger @register_engine
from slopsearx.adapter import EngineStatus, discover_engines


async def _search_with_response(query: str, response: dict, *, max_results: int = 10):
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(200, json=response)

    transport = httpx.MockTransport(handler)
    adapter = discover_engines({"dockerhub": {"enabled": True, "max_results": max_results}})["dockerhub"]
    adapter.set_http_transport(transport)
    try:
        result = await adapter.search(query)
    finally:
        await adapter.shutdown()
        await transport.aclose()
    return result, requests


@pytest.mark.asyncio
async def test_search_uses_documented_namespace_route_and_name_filter() -> None:
    result, requests = await _search_with_response(
        "python",
        {"results": [{"name": "python", "description": "Official Python image"}]},
    )

    assert result.status == EngineStatus.OK
    assert len(requests) == 1
    assert requests[0].url.path == "/v2/namespaces/library/repositories"
    assert dict(requests[0].url.params) == {"name": "python", "page_size": "30"}


@pytest.mark.asyncio
@pytest.mark.parametrize("query", ["python", "library/python", "docker.io/library/python:3.13"])
async def test_exact_official_name_is_promoted_without_dropping_alternatives(query: str) -> None:
    result, _ = await _search_with_response(
        query,
        {
            "results": [
                {"name": "python-tools", "description": "Python tooling"},
                {"name": "python", "description": "Official Python image"},
                {"name": "python-dev", "description": "Python development image"},
            ],
        },
    )

    assert result.status == EngineStatus.OK
    assert [item.title for item in result.results] == ["python", "python-tools", "python-dev"]
    assert [item.url for item in result.results] == [
        "https://hub.docker.com/_/python",
        "https://hub.docker.com/_/python-tools",
        "https://hub.docker.com/_/python-dev",
    ]
    assert [item.position for item in result.results] == [1, 2, 3]


@pytest.mark.asyncio
@pytest.mark.parametrize("query", ["acme/python", "docker.io/acme/python:3.13"])
async def test_namespace_qualified_query_preserves_namespace_and_identity(query: str) -> None:
    result, requests = await _search_with_response(
        query,
        {
            "results": [
                {"name": "python-tools", "namespace": "acme", "description": "Python tooling"},
                {"name": "python", "namespace": "acme", "description": "Acme Python image"},
            ],
        },
    )

    assert result.status == EngineStatus.OK
    assert requests[0].url.path == "/v2/namespaces/acme/repositories"
    assert dict(requests[0].url.params) == {"name": "python", "page_size": "30"}
    assert [item.title for item in result.results] == ["acme/python", "acme/python-tools"]
    assert [item.url for item in result.results] == [
        "https://hub.docker.com/r/acme/python",
        "https://hub.docker.com/r/acme/python-tools",
    ]


@pytest.mark.asyncio
async def test_no_exact_match_preserves_upstream_order() -> None:
    result, _ = await _search_with_response(
        "py",
        {
            "results": [
                {"name": "python-tools", "description": "Python tooling"},
                {"name": "pyroscope", "description": "Profiling"},
            ],
        },
    )

    assert [item.title for item in result.results] == ["python-tools", "pyroscope"]


@pytest.mark.asyncio
async def test_exact_match_after_original_window_is_promoted_with_bounded_overfetch() -> None:
    result, requests = await _search_with_response(
        "python",
        {
            "results": [
                {"name": "python-tools", "description": "Python tooling"},
                {"name": "python-dev", "description": "Python development image"},
                {"name": "python", "description": "Official Python image"},
            ],
        },
        max_results=2,
    )

    assert result.status == EngineStatus.OK
    assert [item.title for item in result.results] == ["python", "python-tools"]
    assert [item.position for item in result.results] == [1, 2]
    assert dict(requests[0].url.params) == {"name": "python", "page_size": "6"}


@pytest.mark.asyncio
async def test_candidate_overfetch_respects_docker_hub_page_size_cap() -> None:
    result, requests = await _search_with_response(
        "python",
        {"results": []},
        max_results=80,
    )

    assert result.status == EngineStatus.OK
    assert dict(requests[0].url.params) == {"name": "python", "page_size": "100"}


@pytest.mark.asyncio
@pytest.mark.parametrize("query", ["", "quay.io/acme/python/image"])
async def test_empty_or_malformed_query_remains_non_raising(query: str) -> None:
    result, requests = await _search_with_response(query, {"results": []})

    assert result.status == EngineStatus.OK
    assert result.results == []
    assert requests[0].url.path == "/v2/namespaces/library/repositories"
