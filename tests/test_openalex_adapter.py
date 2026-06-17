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
            "doi": "10.1234/test",
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
        assert result.results[0].score == 0.45
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

        assert result.status == EngineStatus.ERROR
