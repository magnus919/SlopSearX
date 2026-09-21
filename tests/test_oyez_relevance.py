"""Deterministic relevance and source-order tests for the Oyez adapter."""

from __future__ import annotations

import dataclasses
import json
from unittest.mock import patch

import httpx
import pytest

import engines  # noqa: F401 — trigger @register_engine
from engines.oyez import _case_url
from slopsearx.adapter import EngineStatus, discover_engines
from slopsearx.config import EngineEntry
from slopsearx.formatter import format_html, format_json
from slopsearx.merger import PresenceRanker


class MockHTTP:
    """Context manager that routes the adapter's HTTP client to a fixture."""

    def __init__(self, handler):
        self.transport = httpx.MockTransport(handler)

    async def __aenter__(self):
        self.mock_client = httpx.AsyncClient(transport=self.transport)
        self.patcher = patch("httpx.AsyncClient")
        mock_class = self.patcher.start()
        mock_class.return_value.__aenter__.return_value = self.mock_client
        return self

    async def __aexit__(self, *args):
        self.patcher.stop()
        await self.mock_client.aclose()


@pytest.fixture
def adapter():
    return discover_engines({"oyez": {"enabled": True}})["oyez"]


async def test_uses_oyez_source_search_and_preserves_relevance_order(adapter):
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(
            200,
            json={
                "hits": {
                    "hits": [
                        {
                            "_score": 6.279115,
                            "_source": {
                                "title": "brown v. board of education of topeka (2)",
                                "field_court_term": "1940-1955",
                                "field_docket_number": "1",
                                "search_api_url": "https://api.oyez.org/cases/1940-1955/349us294",
                            },
                        },
                        {
                            "_score": 6.2773943,
                            "_source": {
                                "title": "brown v. board of education of topeka (1)",
                                "field_court_term": "1940-1955",
                                "field_docket_number": "1",
                                "search_api_url": "https://api.oyez.org/cases/1940-1955/347us483",
                            },
                        },
                        {
                            "_score": 5.1956644,
                            "_source": {
                                "title": "pickering v. board of education",
                                "field_court_term": "1967",
                                "field_docket_number": "510",
                                "search_api_url": "https://api.oyez.org/cases/1967/510",
                            },
                        },
                    ],
                },
            },
        )

    async with MockHTTP(handler):
        response = await adapter.search("Brown v. Board of Education")

    assert response.status is EngineStatus.OK
    assert [result.title for result in response.results] == [
        "brown v. board of education of topeka (2)",
        "brown v. board of education of topeka (1)",
        "pickering v. board of education",
    ]
    assert [result.position for result in response.results] == [1, 2, 3]
    assert [result.score for result in response.results] == [6.279115, 6.2773943, 5.1956644]
    assert all("brown v. board of education" in result.title for result in response.results[:2])
    assert response.results[0].url == "https://www.oyez.org/cases/1940-1955/349us294"
    assert response.results[1].url == "https://www.oyez.org/cases/1940-1955/347us483"

    assert len(requests) == 1
    assert requests[0].method == "POST"
    body = json.loads(requests[0].content)
    multi_match = body["query"]["multi_match"]
    assert body["size"] == 10
    assert multi_match["query"] == "Brown v. Board of Education"
    assert multi_match["type"] == "cross_fields"
    assert "title^2" in multi_match["fields"]
    assert "field_first_party" in multi_match["fields"]


async def test_adapter_does_not_reorder_raw_source_hits(adapter):
    raw_hits = [
        {
            "_score": 1.0,
            "_source": {
                "title": "fuzzy candidate",
                "url": "https://api.oyez.org/cases/1971/1",
            },
        },
        {
            "_score": 9.0,
            "_source": {
                "title": "exact candidate",
                "url": "https://api.oyez.org/cases/1954/1",
            },
        },
    ]

    async with MockHTTP(lambda request: httpx.Response(200, json={"hits": {"hits": raw_hits}})):
        response = await adapter.search("candidate")

    assert [result.title for result in response.results] == ["fuzzy candidate", "exact candidate"]
    assert [result.position for result in response.results] == [1, 2]
    assert [result.score for result in response.results] == [1.0, 9.0]


async def test_unresolved_records_are_omitted_without_empty_url_merge(adapter):
    raw_hits = [
        {
            "_score": 4.0,
            "_source": {
                "title": "identified case",
                "url": "https://api.oyez.org/cases/1954/1",
            },
        },
        {
            "_score": 3.0,
            "_source": {"title": "unresolved case", "url": "https://evil.example/cases/1954/2"},
        },
        {
            "_score": 2.0,
            "_source": {
                "title": "second identified case",
                "url": "https://api.oyez.org/cases/1967/510",
            },
        },
        {
            "_score": 1.0,
            "_source": {"title": "missing identity"},
        },
    ]

    async with MockHTTP(lambda request: httpx.Response(200, json={"hits": {"hits": raw_hits}})):
        response = await adapter.search("case")

    assert response.status is EngineStatus.OK
    assert [result.title for result in response.results] == ["identified case", "second identified case"]
    assert [result.position for result in response.results] == [1, 2]
    assert [result.url for result in response.results] == [
        "https://www.oyez.org/cases/1954/1",
        "https://www.oyez.org/cases/1967/510",
    ]

    merged = PresenceRanker().rank({"oyez": response.results}, "case")
    document = format_json(merged, "case")
    html = format_html(merged, "case")
    assert len(merged) == 2
    assert all(result.url for result in merged)
    assert [result["url"] for result in document["results"]] == [
        "https://www.oyez.org/cases/1954/1",
        "https://www.oyez.org/cases/1967/510",
    ]
    assert 'href="#"' not in html
    assert html.count('class="result-link" href="https://www.oyez.org/cases/') == 2


async def test_explicit_rest_override_keeps_legacy_case_fixture_shape(adapter):
    adapter.config["base_url"] = "https://api.example.test"
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(
            200,
            json=[
                {
                    "name": "Marbury v. Madison",
                    "term": "1801",
                    "href": "/cases/1801/10",
                },
            ],
        )

    async with MockHTTP(handler):
        response = await adapter.search("marbury")

    assert response.status is EngineStatus.OK
    assert response.results[0].url == "https://www.oyez.org/cases/1801/10"
    assert "name" not in str(requests[0].url)


async def test_empty_base_url_from_configured_startup_uses_source_search():
    configured = discover_engines({"oyez": dataclasses.asdict(EngineEntry(timeout_ms=8_000))})["oyez"]
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(200, json={"hits": {"hits": []}})

    async with MockHTTP(handler):
        response = await configured.search("Brown v. Board of Education")

    assert response.status is EngineStatus.OK
    assert requests[0].method == "POST"
    assert str(requests[0].url) == "https://beta-search.oyez.org/elasticsearch_index_scotus_nodes/_search"


@pytest.mark.parametrize(
    "value",
    [
        "https://evil.example/cases/1954/1",
        "https://api.oyez.org/not-cases/1954/1",
        "https://api.oyez.org/cases/1954",
        "cases/1954/1",
    ],
)
def test_case_url_rejects_untrusted_or_incomplete_identity(value):
    assert _case_url(value) == ""


def test_case_url_does_not_infer_identity_without_provider_url():
    assert _case_url("") == ""
