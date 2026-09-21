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

AVAILABILITY_RESPONSE = {
    "archived_snapshots": {
        "closest": {
            "available": True,
            "status": "200",
            "timestamp": "20240102123456",
            "url": "http://web.archive.org/web/20240102123456/https://whitehouse.gov/",
        }
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
            result = await adapter.search("whitehouse.gov")

        assert result.status == EngineStatus.OK
        assert "cdx/search/cdx" in captured["url"]
        assert "whitehouse.gov" in captured["url"]

    async def test_wayback_returns_snapshots(self, adapter):
        def _handler(r):
            return httpx.Response(200, json=CDX_RESPONSE)

        async with MockHTTP(_handler):
            result = await adapter.search("whitehouse.gov")

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
            result = await adapter.search("whitehouse.gov")

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
            result = await adapter.search("whitehouse.gov")

        assert result.status == EngineStatus.TIMEOUT

    async def test_cdx_timeout_falls_back_to_one_validated_closest_snapshot(self, adapter):
        calls = []

        def _handler(r):
            calls.append(str(r.url))
            if "cdx/search/cdx" in str(r.url):
                raise httpx.TimeoutException("timeout", request=r)
            return httpx.Response(200, json=AVAILABILITY_RESPONSE)

        async with MockHTTP(_handler):
            result = await adapter.search("whitehouse.gov")

        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert result.results[0].url == "https://web.archive.org/web/20240102123456/https://whitehouse.gov/"
        assert "closest snapshot" in result.error_message
        assert "not a CDX history" in result.error_message
        assert len(calls) == 2
        assert "wayback/available" in calls[1]

    async def test_availability_reports_no_snapshot_without_fabricating_history(self, adapter):
        def _handler(r):
            if "cdx/search/cdx" in str(r.url):
                return httpx.Response(500)
            return httpx.Response(200, json={"archived_snapshots": {}})

        async with MockHTTP(_handler):
            result = await adapter.search("whitehouse.gov")

        assert result.status == EngineStatus.OK
        assert result.results == []
        assert result.error_message == "No archived snapshot available"

    async def test_availability_false_is_an_honest_empty_result(self, adapter):
        def _handler(r):
            if "cdx/search/cdx" in str(r.url):
                return httpx.Response(500)
            return httpx.Response(
                200,
                json={
                    "archived_snapshots": {"closest": {"available": False, "status": "404", "timestamp": "", "url": ""}}
                },
            )

        async with MockHTTP(_handler):
            result = await adapter.search("whitehouse.gov")

        assert result.status == EngineStatus.OK
        assert result.results == []
        assert result.error_message == "No archived snapshot available"

    async def test_availability_malformed_json_is_error(self, adapter):
        def _handler(r):
            if "cdx/search/cdx" in str(r.url):
                return httpx.Response(500)
            return httpx.Response(200, content=b"not-json")

        async with MockHTTP(_handler):
            result = await adapter.search("whitehouse.gov")

        assert result.status == EngineStatus.ERROR
        assert result.results == []

    @pytest.mark.parametrize(
        "closest",
        [
            {
                "available": True,
                "status": "200",
                "timestamp": "20240102123456",
                "url": "https://evil.example/web/20240102123456/https://whitehouse.gov/",
            },
            {
                "available": True,
                "status": "200",
                "timestamp": "20240102123456",
                "url": "http://web.archive.org/web/20240102123455/https://whitehouse.gov/",
            },
            {
                "available": True,
                "status": "200",
                "timestamp": "20240102123456",
                "url": "http://web.archive.org/web/20240102123456/https://other.example/",
            },
        ],
    )
    async def test_availability_rejects_untrusted_snapshot_url(self, adapter, closest):
        def _handler(r):
            if "cdx/search/cdx" in str(r.url):
                return httpx.Response(500)
            return httpx.Response(200, json={"archived_snapshots": {"closest": closest}})

        async with MockHTTP(_handler):
            result = await adapter.search("whitehouse.gov")

        assert result.status == EngineStatus.ERROR
        assert result.results == []

    async def test_availability_blocked_is_preserved(self, adapter):
        def _handler(r):
            if "cdx/search/cdx" in str(r.url):
                raise httpx.TimeoutException("timeout", request=r)
            return httpx.Response(403)

        async with MockHTTP(_handler):
            result = await adapter.search("whitehouse.gov")

        assert result.status == EngineStatus.BLOCKED
        assert result.results == []

    async def test_availability_timeout_is_preserved(self, adapter):
        def _handler(r):
            raise httpx.TimeoutException("timeout", request=r)

        async with MockHTTP(_handler):
            result = await adapter.search("whitehouse.gov")

        assert result.status == EngineStatus.TIMEOUT
        assert result.results == []


class TestInternetArchiveAdapterHelpers:
    def test_is_domain_query_detects_domain(self):
        from engines.internetarchive import _is_domain_query

        assert _is_domain_query("example.com") is True
        assert _is_domain_query("sub.example.org") is True

    def test_is_domain_query_rejects_text(self):
        from engines.internetarchive import _is_domain_query

        assert _is_domain_query("python programming") is False
        assert _is_domain_query("hello world") is False
