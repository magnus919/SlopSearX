"""Tests for the Stack Exchange adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401
from slopsearx.adapter import EngineStatus, discover_engines, list_engines
from tests.test_adapters import MockHTTP

SAMPLE_RESPONSE = {
    "items": [
        {
            "title": "How to sort a list in Python?",
            "link": "https://stackoverflow.com/questions/123/sort-list",
            "score": 42,
            "creation_date": 1700000000,
            "body_markdown": "You can use the sorted() function or list.sort() method.",
        },
        {
            "title": "Async/await best practices",
            "link": "https://stackoverflow.com/questions/456/async-await",
            "score": 15,
            "creation_date": 1700001000,
        },
    ]
}


@pytest.fixture
def adapter():
    return discover_engines({"stackexchange": {"enabled": True, "api_key": ""}})["stackexchange"]


class TestStackExchangeAdapterRegistration:
    def test_adapter_registered(self):
        assert "stackexchange" in list_engines()

    def test_adapter_categories(self):
        cls = list_engines()["stackexchange"]
        assert "stackexchange:code" in cls.categories


class TestStackExchangeAdapterSearch:
    async def test_search_returns_results(self, adapter):
        def _handler(r):
            return httpx.Response(200, json=SAMPLE_RESPONSE)

        async with MockHTTP(_handler):
            result = await adapter.search("python sort")

        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert result.results[0].title == "How to sort a list in Python?"
        assert "stackoverflow.com" in result.results[0].url

    async def test_search_defaults_to_stackoverflow(self, adapter):
        captured = {}

        def _handler(r):
            captured["url"] = str(r.url)
            return httpx.Response(200, json={"items": []})

        async with MockHTTP(_handler):
            await adapter.search("test", {"categories": ["general"]})

        assert "site=stackoverflow" in captured["url"]

    async def test_search_serverfault_site(self, adapter):
        captured = {}

        def _handler(r):
            captured["url"] = str(r.url)
            return httpx.Response(200, json={"items": []})

        async with MockHTTP(_handler):
            await adapter.search("test", {"categories": ["stackexchange:serverfault"]})

        assert "site=serverfault" in captured["url"]

    async def test_search_code_site(self, adapter):
        captured = {}

        def _handler(r):
            captured["url"] = str(r.url)
            return httpx.Response(200, json={"items": []})

        async with MockHTTP(_handler):
            await adapter.search("test", {"categories": ["stackexchange:code"]})

        assert "site=stackoverflow" in captured["url"]

    async def test_search_empty_results(self, adapter):
        def _handler(r):
            return httpx.Response(200, json={"items": []})

        async with MockHTTP(_handler):
            result = await adapter.search("nothing")

        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    async def test_search_rate_limited(self, adapter):
        """400 from SE indicates rate limited."""

        def _handler(r):
            return httpx.Response(400)

        async with MockHTTP(_handler):
            result = await adapter.search("test")

        assert result.status == EngineStatus.RATE_LIMITED

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

    async def test_search_http_status_error_sanitized(self):
        """HTTPStatusError (500) must not leak API key in error_message."""
        instances = discover_engines({"stackexchange": {"enabled": True, "api_key": "test-se-key-secret"}})
        adapter = instances["stackexchange"]

        def _handler(r):
            return httpx.Response(500)

        async with MockHTTP(_handler):
            result = await adapter.search("test")
        assert result.status == EngineStatus.ERROR
        assert result.error_message is not None
        assert "test-se-key-secret" not in result.error_message

    async def test_search_broad_exception_sanitized(self):
        """Broad handler sanitizes error_message when exception contains a URL."""
        instances = discover_engines({"stackexchange": {"enabled": True, "api_key": "test-se-key-secret"}})
        adapter = instances["stackexchange"]

        async with MockHTTP(
            lambda r: (_ for _ in ()).throw(
                RuntimeError(
                    "https://api.stackexchange.com/2.3/search?order=desc&sort=relevance&intitle=test&site=stackoverflow&pagesize=10&key=test-se-key-secret"
                )
            )
        ):
            result = await adapter.search("test")
        assert result.status == EngineStatus.ERROR
        assert result.error_message is not None
        assert "test-se-key-secret" not in result.error_message

    async def test_uses_x_api_key_header(self):
        """StackExchange sends API key as X-API-Key header, not query param."""
        captured_headers = {}
        captured_url = ""

        def _handler(r):
            nonlocal captured_url
            captured_headers.update(dict(r.headers))
            captured_url = str(r.url)
            return httpx.Response(200, json={"items": []})

        instances = discover_engines({"stackexchange": {"enabled": True, "api_key": "se-header-test"}})
        async with MockHTTP(_handler):
            await instances["stackexchange"].search("test")

        assert captured_headers.get("x-api-key") == "se-header-test"
        assert "key=se-header-test" not in captured_url
        assert "&key=" not in captured_url

    async def test_empty_key_still_works_free_tier(self):
        """Empty API key works for free tier — no header sent, no crash."""
        captured_headers = {}

        def _handler(r):
            captured_headers.update(dict(r.headers))
            return httpx.Response(200, json={"items": []})

        instances = discover_engines({"stackexchange": {"enabled": True, "api_key": ""}})
        async with MockHTTP(_handler):
            result = await instances["stackexchange"].search("test")
        assert result.status == EngineStatus.OK
        assert "x-api-key" not in captured_headers

    async def test_whitespace_key_treated_as_no_key(self):
        """Whitespace-only key treated as absent — no header sent."""
        captured_headers = {}

        def _handler(r):
            captured_headers.update(dict(r.headers))
            return httpx.Response(200, json={"items": []})

        instances = discover_engines({"stackexchange": {"enabled": True, "api_key": "   "}})
        async with MockHTTP(_handler):
            result = await instances["stackexchange"].search("test")
        assert result.status == EngineStatus.OK
        assert "x-api-key" not in captured_headers
