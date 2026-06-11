"""Tests for the Internet Archive adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401
from slopsearx.adapter import EngineStatus, discover_engines, list_engines
from tests.test_adapters import MockHTTP

CDX_RESPONSE = [
    ["urlkey", "timestamp", "original", "mimetype", "statuscode", "digest", "length"],
    ["com,example)/", "20250101000000", "https://example.com/page1", "text/html", "200", "abc", "1234"],
    ["com,example)/", "20250201000000", "https://example.com/page2", "text/html", "200", "def", "5678"],
]

ARCHIVE_RESPONSE = {
    "response": {
        "docs": [
            {
                "identifier": "book123",
                "title": "The Great Book",
                "description": "A fantastic book about everything.",
                "downloads": 5000,
                "date": "2024-01-15",
            },
            {
                "identifier": "book456",
                "title": "Another Book",
                "description": "A sequel with more content.",
                "downloads": 1200,
                "date": "2023-06-01",
            },
        ]
    }
}


@pytest.fixture
def adapter():
    return discover_engines({"internetarchive": {"enabled": True}})["internetarchive"]


class TestInternetArchiveAdapterRegistration:
    def test_adapter_registered(self):
        assert "internetarchive" in list_engines()

    def test_adapter_categories_excludes_general(self):
        cls = list_engines()["internetarchive"]
        assert "general" not in cls.categories
        assert "web:archive" in cls.categories
        assert "historical" in cls.categories


class TestInternetArchiveWaybackSearch:
    async def test_domain_query_routes_to_wayback(self, adapter):
        captured = {}

        def _handler(r):
            captured["url"] = str(r.url)
            return httpx.Response(200, json=CDX_RESPONSE)

        async with MockHTTP(_handler):
            result = await adapter.search("example.com")

        assert result.status == EngineStatus.OK
        assert "cdx/search/cdx" in captured["url"]
        assert "example.com" in captured["url"]

    async def test_wayback_returns_snapshots(self, adapter):
        def _handler(r):
            return httpx.Response(200, json=CDX_RESPONSE)

        async with MockHTTP(_handler):
            result = await adapter.search("example.com")

        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert "web.archive.org" in result.results[0].url
        assert "20250101" in result.results[0].url

    async def test_domain_query_rejects_regular_text(self, adapter):
        """Regular text queries should not route to Wayback."""
        captured = {}

        def _handler(r):
            captured["url"] = str(r.url)
            return httpx.Response(200, json=ARCHIVE_RESPONSE)

        async with MockHTTP(_handler):
            result = await adapter.search("python programming")

        assert result.status == EngineStatus.OK
        assert "advancedsearch.php" in captured["url"]


class TestInternetArchiveGeneralSearch:
    async def test_general_search_returns_results(self, adapter):
        def _handler(r):
            return httpx.Response(200, json=ARCHIVE_RESPONSE)

        async with MockHTTP(_handler):
            result = await adapter.search("books about history")

        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert result.results[0].title == "The Great Book"
        assert "archive.org/details/book123" in result.results[0].url

    async def test_general_search_content_from_description(self, adapter):
        def _handler(r):
            return httpx.Response(200, json=ARCHIVE_RESPONSE)

        async with MockHTTP(_handler):
            result = await adapter.search("books")

        assert result.results[0].content == "A fantastic book about everything."

    async def test_general_search_empty_results(self, adapter):
        def _handler(r):
            return httpx.Response(200, json={"response": {"docs": []}})

        async with MockHTTP(_handler):
            result = await adapter.search("nothing")

        assert result.status == EngineStatus.OK
        assert len(result.results) == 0


class TestInternetArchiveErrors:
    async def test_wayback_error(self, adapter):
        def _handler(r):
            return httpx.Response(500)

        async with MockHTTP(_handler):
            result = await adapter.search("example.com")

        assert result.status == EngineStatus.ERROR

    async def test_general_error(self, adapter):
        def _handler(r):
            return httpx.Response(500)

        async with MockHTTP(_handler):
            result = await adapter.search("books")

        assert result.status == EngineStatus.ERROR

    async def test_timeout(self, adapter):
        def _handler(r):
            raise httpx.TimeoutException("timeout", request=r)

        async with MockHTTP(_handler):
            result = await adapter.search("example.com")

        assert result.status == EngineStatus.ERROR


class TestInternetArchiveAdapterHelpers:
    def test_is_domain_query_detects_domain(self):
        from engines.internetarchive import _is_domain_query

        assert _is_domain_query("example.com") is True
        assert _is_domain_query("sub.example.org") is True

    def test_is_domain_query_rejects_text(self):
        from engines.internetarchive import _is_domain_query

        assert _is_domain_query("python programming") is False
        assert _is_domain_query("hello world") is False
