"""Tests for the Google scrape adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401
from slopsearx.adapter import EngineStatus, discover_engines, list_engines
from tests.test_adapters import MockHTTP

SAMPLE_HTML = """
<html><body>
<div class="g">
  <h3><a href="https://example.com/page1">Result One</a></h3>
  <div><span class="st">Description of result one.</span></div>
</div>
<div class="g">
  <h3><a href="https://example.com/page2">Result Two</a></h3>
  <div><span class="st">Description of result two.</span></div>
</div>
</body></html>
"""

CHALLENGE_HTML = '<html><body><div class="g-recaptcha">verify you are human</div></body></html>'


@pytest.fixture
def adapter():
    return discover_engines({"google": {"enabled": True}})["google"]


class TestGoogleAdapterRegistration:
    def test_adapter_registered(self):
        assert "google" in list_engines()

    def test_adapter_categories(self):
        cls = list_engines()["google"]
        assert "general" in cls.categories


class TestGoogleAdapterSearch:
    async def test_search_returns_results(self, adapter):
        def _handler(r):
            return httpx.Response(200, text=SAMPLE_HTML)

        async with MockHTTP(_handler):
            result = await adapter.search("test query")

        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert result.results[0].title == "Result One"
        assert result.results[0].url == "https://example.com/page1"

    async def test_search_empty_html(self, adapter):
        def _handler(r):
            return httpx.Response(200, text="<html></html>")

        async with MockHTTP(_handler):
            result = await adapter.search("test query")

        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    async def test_search_challenge_page(self, adapter):
        def _handler(r):
            return httpx.Response(200, text=CHALLENGE_HTML)

        async with MockHTTP(_handler):
            result = await adapter.search("test query")

        assert result.status == EngineStatus.BLOCKED
        assert len(result.results) == 0

    async def test_search_rate_limited(self, adapter):
        def _handler(r):
            return httpx.Response(429)

        async with MockHTTP(_handler):
            result = await adapter.search("test query")

        assert result.status == EngineStatus.RATE_LIMITED

    async def test_search_blocked(self, adapter):
        def _handler(r):
            return httpx.Response(503)

        async with MockHTTP(_handler):
            result = await adapter.search("test query")

        assert result.status == EngineStatus.BLOCKED

    async def test_search_timeout(self, adapter):
        def _handler(r):
            raise httpx.TimeoutException("timeout", request=r)

        async with MockHTTP(_handler):
            result = await adapter.search("test query")

        assert result.status == EngineStatus.TIMEOUT

    async def test_search_error(self, adapter):
        def _handler(r):
            return httpx.Response(500)

        async with MockHTTP(_handler):
            result = await adapter.search("test query")

        assert result.status == EngineStatus.ERROR

    async def test_search_sends_headers(self, adapter):
        captured = {}

        def _handler(r):
            captured["ua"] = r.headers.get("User-Agent", "")
            return httpx.Response(200, text=SAMPLE_HTML)

        async with MockHTTP(_handler):
            await adapter.search("test query")

        assert "Mozilla" in captured["ua"]
