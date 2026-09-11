"""Tests for the OpenAlex adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401
from slopsearx.adapter import EngineStatus, discover_engines, list_engines
from tests.test_adapters import MockHTTP

SAMPLE_RESPONSE = {
    "results": [
        {
            "id": "https://openalex.org/W123",
            "title": "A Study on Machine Learning",
            "doi": "https://doi.org/10.1234/test",
            "relevance_score": 42.5,
            "cited_by_count": 450,
            "publication_date": "2024-03-15",
            "abstract_inverted_index": {
                "Machine": [0],
                "learning": [1],
                "is": [2],
                "important": [3],
            },
        },
        {
            "id": "https://openalex.org/W456",
            "title": "Deep Learning Advances",
            "doi": None,
            "cited_by_count": 120,
            "publication_date": "2023-11-01",
            "abstract_inverted_index": None,
        },
    ]
}


@pytest.fixture
def adapter():
    return discover_engines({"openalex": {"enabled": True}})["openalex"]


class TestOpenAlexAdapterRegistration:
    def test_adapter_registered(self):
        assert "openalex" in list_engines()

    def test_adapter_categories(self):
        cls = list_engines()["openalex"]
        assert "science" in cls.categories


class TestOpenAlexAdapterSearch:
    async def test_search_returns_results(self, adapter):
        def _handler(r):
            return httpx.Response(200, json=SAMPLE_RESPONSE)

        async with MockHTTP(_handler):
            result = await adapter.search("machine learning")

        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert result.results[0].title == "A Study on Machine Learning"
        assert result.results[0].url == "https://doi.org/10.1234/test"
        assert result.results[0].score == 42.5
        assert result.results[0].published_date == "2024-03-15"

    async def test_search_content_from_abstract(self, adapter):
        def _handler(r):
            return httpx.Response(200, json=SAMPLE_RESPONSE)

        async with MockHTTP(_handler):
            result = await adapter.search("machine learning")

        assert "Machine learning is important" in result.results[0].content

    async def test_search_doi_fallback_to_id(self, adapter):
        """Results without DOI use OpenAlex ID as URL."""
        resp = {
            "results": [
                {
                    "id": "https://openalex.org/W456",
                    "title": "No DOI Work",
                    "doi": None,
                    "cited_by_count": 0,
                    "publication_date": None,
                    "abstract_inverted_index": None,
                }
            ]
        }

        def _handler(r):
            return httpx.Response(200, json=resp)

        async with MockHTTP(_handler):
            result = await adapter.search("deep learning")

        assert result.results[0].url == "https://openalex.org/W456"
        assert result.results[0].payload["data"]["openalex_id"] == "https://openalex.org/W456"

    async def test_search_empty_results(self, adapter):
        def _handler(r):
            return httpx.Response(200, json={"results": []})

        async with MockHTTP(_handler):
            result = await adapter.search("nothing")

        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    async def test_search_error(self, adapter):
        def _handler(r):
            return httpx.Response(500)

        async with MockHTTP(_handler):
            result = await adapter.search("test")

        assert result.status == EngineStatus.ERROR

    async def test_search_timeout(self, adapter):
        def _handler(r):
            raise httpx.TimeoutException("timeout", request=r)

        async with MockHTTP(_handler):
            result = await adapter.search("test")

        assert result.status == EngineStatus.TIMEOUT


@pytest.mark.parametrize(
    ("doi", "expected"),
    [
        ("https://doi.org/10.1038/nature14539", "https://doi.org/10.1038/nature14539"),
        ("http://dx.doi.org/10.1038/nature14539", "http://dx.doi.org/10.1038/nature14539"),
        ("10.1038/nature14539", "https://doi.org/10.1038/nature14539"),
        (None, "https://openalex.org/W123"),
        ("  ", "https://openalex.org/W123"),
    ],
)
async def test_doi_forms(adapter, doi, expected):
    work = {**SAMPLE_RESPONSE["results"][0], "doi": doi}
    async with MockHTTP(lambda r: httpx.Response(200, json={"results": [work]})):
        response = await adapter.search("Deep learning")
    assert response.status == EngineStatus.OK
    assert response.results[0].url == expected


@pytest.mark.parametrize("outcome", ["success", "empty", "timeout", "429", "500", "malformed"])
async def test_elapsed_latency_and_classified_outcomes(adapter, monkeypatch, outcome):
    from engines import openalex

    ticks = iter([10.0, 10.125])
    from types import SimpleNamespace

    monkeypatch.setattr(openalex, "time", SimpleNamespace(monotonic=lambda: next(ticks)))

    def handler(request):
        if outcome == "timeout":
            raise httpx.ReadTimeout("timeout", request=request)
        if outcome in ("429", "500"):
            return httpx.Response(int(outcome))
        payload = SAMPLE_RESPONSE if outcome == "success" else {"results": []}
        if outcome == "malformed":
            payload = {"results": [None]}
        return httpx.Response(200, json=payload)

    async with MockHTTP(handler):
        response = await adapter.search("test")
    assert response.latency_ms == 125
    assert (
        response.status
        == {
            "success": EngineStatus.OK,
            "empty": EngineStatus.OK,
            "timeout": EngineStatus.TIMEOUT,
            "429": EngineStatus.RATE_LIMITED,
            "500": EngineStatus.ERROR,
            "malformed": EngineStatus.ERROR,
        }[outcome]
    )


@pytest.mark.parametrize(
    "payload",
    [
        [],
        {"results": None},
        {"results": [{}, {"abstract_inverted_index": ["bad"]}]},
        {"results": [{"cited_by_count": "bad", "relevance_score": {"bad": 1}}]},
        {"results": [{"abstract_inverted_index": {"word": None}}]},
    ],
)
async def test_malformed_payload_never_raises(adapter, payload):
    async with MockHTTP(lambda r: httpx.Response(200, json=payload)):
        response = await adapter.search("test")
    assert response.status == EngineStatus.ERROR
    assert response.results == []
    assert response.latency_ms > 0


async def test_search_preserves_upstream_relevance_order(adapter):
    def handler(request):
        assert "sort" not in request.url.params
        assert request.url.params["search"] == "C++ learning"
        return httpx.Response(200, json=SAMPLE_RESPONSE)

    async with MockHTTP(handler):
        response = await adapter.search("C++ learning")
    assert [r.title for r in response.results] == [w["title"] for w in SAMPLE_RESPONSE["results"]]


async def test_local_rate_limit_is_timed(adapter):
    from unittest.mock import AsyncMock

    adapter.rate_limiter = AsyncMock()
    adapter.rate_limiter.acquire.return_value = False
    response = await adapter.search("test")
    assert response.status == EngineStatus.RATE_LIMITED
    assert response.latency_ms > 0


async def test_rate_limiter_failure_never_raises(adapter):
    from unittest.mock import AsyncMock

    adapter.rate_limiter = AsyncMock()
    adapter.rate_limiter.acquire.side_effect = RuntimeError("store unavailable")
    response = await adapter.search("test")
    assert response.status == EngineStatus.ERROR
    assert response.latency_ms > 0


@pytest.mark.parametrize(
    "bounds,expected",
    [
        ({"date_from": "2024-02-29"}, "from_publication_date:2024-02-29"),
        ({"date_to": "2024-02-29"}, "to_publication_date:2024-02-29"),
        (
            {"date_from": "2024-02-29", "date_to": "2024-02-29"},
            "from_publication_date:2024-02-29,to_publication_date:2024-02-29",
        ),
    ],
)
async def test_inclusive_date_filters_and_unproven_dates(adapter, bounds, expected):
    seen = []

    def handler(request):
        seen.append(request)
        return httpx.Response(
            200,
            json={
                "results": [
                    {"id": f"https://openalex.org/W{i}", "publication_date": date}
                    for i, date in enumerate(["2024-02-28", "2024-02-29", "2024-03-01", None, "2024", "bad"])
                ]
            },
        )

    async with MockHTTP(handler):
        result = await adapter.search("C++ & climate", bounds)
    assert result.status == EngineStatus.OK
    assert seen[0].url.params["filter"] == expected
    assert seen[0].url.params["search"] == "C++ & climate"
    dates = [r.published_date for r in result.results]
    assert "2024-02-29" in dates
    assert None not in dates and "2024" not in dates and "bad" not in dates
    if "date_from" in bounds:
        assert "2024-02-28" not in dates
    if "date_to" in bounds:
        assert "2024-03-01" not in dates


@pytest.mark.parametrize(
    "bounds",
    [
        {"date_from": "2023-02-29"},
        {"date_to": "2024-13-01"},
        {"date_from": "20240101"},
        {"date_from": ""},
        {"date_from": "2024-01-01,publication_year:2020"},
        {"date_from": "2024-03-01", "date_to": "2024-02-01"},
        {"time_range": "all"},
    ],
)
async def test_invalid_date_constraints_never_dispatch(adapter, bounds):
    seen = []
    async with MockHTTP(lambda request: seen.append(request)):
        result = await adapter.search("climate", bounds)
    assert result.status == EngineStatus.ERROR
    assert seen == []
