"""Unit tests for engine adapters with httpx mocking."""

from __future__ import annotations

from unittest.mock import patch

import httpx
import pytest

import engines  # noqa: F401 — trigger @register_engine
from slopsearx.adapter import EngineStatus, discover_engines

# ---------------------------------------------------------------------------
# Helper: mock HTTP transport for adapters that create httpx clients inline
# ---------------------------------------------------------------------------


class MockHTTP:
    """Context manager that patches httpx.AsyncClient to return a mock client.

    Usage::

        async with MockHTTP(lambda r: httpx.Response(200, json=...)):
            result = await adapter.search("query")
    """

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


# ---------------------------------------------------------------------------
# Brave adapter
# ---------------------------------------------------------------------------


class TestBraveAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"brave": {"enabled": True, "api_key": "test-key"}})
        return instances["brave"]

    @pytest.fixture
    def sample_response(self) -> dict:
        return {
            "web": {
                "results": [
                    {
                        "url": "https://example.com/page1",
                        "title": "Test Page 1",
                        "description": "Description of test page 1",
                        "thumbnail": {"src": "https://example.com/thumb1.jpg"},
                    },
                    {
                        "url": "https://example.com/page2",
                        "title": "Test Page 2",
                        "description": "Description of test page 2",
                    },
                ],
            },
        }

    async def test_search_returns_results(self, adapter, sample_response):
        def _handler(r):
            return httpx.Response(200, json=sample_response)
        async with MockHTTP(_handler):
            result = await adapter.search("test query")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert result.results[0].url == "https://example.com/page1"
        assert result.results[0].thumbnail == "https://example.com/thumb1.jpg"
        assert result.results[1].url == "https://example.com/page2"

    async def test_search_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("test")
        assert result.status == EngineStatus.RATE_LIMITED
        assert result.results == []

    async def test_search_blocked(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(403)):
            result = await adapter.search("test")
        assert result.status == EngineStatus.BLOCKED
        assert result.results == []

    async def test_search_timeout(self, adapter):
        async with MockHTTP(lambda r: (_ for _ in ()).throw(httpx.TimeoutException("timeout"))):
            with patch("httpx.AsyncClient", side_effect=httpx.TimeoutException("timeout")):
                result = await adapter.search("test")
        assert result.status == EngineStatus.TIMEOUT
        assert result.results == []

    async def test_search_missing_api_key(self):
        instances = discover_engines({"brave": {"enabled": True, "api_key": ""}})
        adapter = instances["brave"]
        result = await adapter.search("test")
        assert result.status == EngineStatus.ERROR
        assert "API key not configured" in (result.error_message or "")


# ---------------------------------------------------------------------------
# DuckDuckGo adapter
# ---------------------------------------------------------------------------


class TestDuckDuckGoAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"duckduckgo": {"enabled": True}})
        return instances["duckduckgo"]

    @pytest.fixture
    def sample_html(self) -> str:
        return """
        <html><body>
        <div class="result">
            <div class="result__a">
                <span class="result__url">https://example.com/page1</span>
                <h2 class="result__title">Test Page 1</h2>
            </div>
            <div class="result__snippet">Snippet for page 1</div>
        </div>
        <div class="result">
            <div class="result__a">
                <span class="result__url">https://example.com/page2</span>
                <h2 class="result__title">Test Page 2</h2>
            </div>
            <div class="result__snippet">Snippet for page 2</div>
        </div>
        </body></html>
        """

    def test_parse_html(self, adapter, sample_html):
        results = adapter._parse_html(sample_html, "test", 10)
        assert len(results) == 2
        assert results[0].url == "https://example.com/page1"
        assert results[0].content == "Snippet for page 1"

    def test_parse_html_respects_max_results(self, adapter, sample_html):
        results = adapter._parse_html(sample_html, "test", 1)
        assert len(results) == 1

    def test_parse_html_empty(self, adapter):
        results = adapter._parse_html("<html></html>", "test", 10)
        assert results == []

    def test_captcha_detection(self, adapter):
        html = '<html><body><div class="challenge">verify you are human</div></body></html>'
        results = adapter._parse_html(html, "test", 10)
        assert results == []

    def test_captcha_indicators_match(self, adapter):
        assert adapter._is_challenge_page("ddg_sl_ challenge page")
        assert adapter._is_challenge_page("hcaptcha challenge")
        assert adapter._is_challenge_page("cf-browser-verification")
        assert not adapter._is_challenge_page("normal search results content")

    async def test_search_happy_path(self, adapter):
        def _handler(r):
            return httpx.Response(
                200,
                content=b'<html><body><div class="result">'
                b'<div class="result__a"><span class="result__url">https://x.com</span></div>'
                b'<div class="result__snippet">content</div></div></body></html>',
            )
        async with MockHTTP(_handler):
            result = await adapter.search("test")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1

    async def test_search_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("test")
        assert result.status == EngineStatus.RATE_LIMITED

    async def test_search_captcha_blocked(self, adapter):
        def _handler(r):
            return httpx.Response(200, content=b'<html><body>hcaptcha challenge</body></html>')
        async with MockHTTP(_handler):
            result = await adapter.search("test")
        assert result.status == EngineStatus.OK  # no error, just empty
        assert len(result.results) == 0


# ---------------------------------------------------------------------------
# Google adapter
# ---------------------------------------------------------------------------


class TestGoogleAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"google": {"enabled": True}})
        return instances["google"]

    @pytest.fixture
    def sample_html(self) -> str:
        return """
        <html><body>
        <div class="g">
            <a href="https://example.com/page1"><h3>Test Page 1</h3></a>
            <span class="st">Snippet for page 1</span>
        </div>
        <div class="g">
            <a href="https://example.com/page2"><h3>Test Page 2</h3></a>
            <div class="VwiC3b">Snippet for page 2</div>
        </div>
        </body></html>
        """

    def test_parse_html(self, adapter, sample_html):
        results = adapter._parse_html(sample_html, "test", 10)
        assert len(results) == 2
        assert results[0].url == "https://example.com/page1"

    def test_parse_html_respects_max_results(self, adapter, sample_html):
        results = adapter._parse_html(sample_html, "test", 1)
        assert len(results) == 1

    def test_parse_html_empty(self, adapter):
        results = adapter._parse_html("<html></html>", "test", 10)
        assert results == []

    def test_captcha_detection(self, adapter):
        html = '<html><body><div class="g-recaptcha">unusual traffic</div></body></html>'
        results = adapter._parse_html(html, "test", 10)
        assert results == []

    def test_captcha_indicators_match(self, adapter):
        assert adapter._is_challenge_page("recaptcha challenge")
        assert adapter._is_challenge_page("unusual traffic from your network")
        assert adapter._is_challenge_page("g-recaptcha")
        assert not adapter._is_challenge_page("normal search results")


# ---------------------------------------------------------------------------
# Wikipedia adapter
# ---------------------------------------------------------------------------


class TestWikipediaAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"wikipedia": {"enabled": True}})
        return instances["wikipedia"]

    @pytest.fixture
    def opensearch_response(self) -> dict:
        return [
            "test query",
            ["Test Page 1", "Test Page 2"],
            ["https://en.wikipedia.org/wiki/Test_Page_1", "https://en.wikipedia.org/wiki/Test_Page_2"],
            ["snippet 1", "snippet 2"],
        ]

    @pytest.fixture
    def rich_query_response(self) -> dict:
        return {
            "batchcomplete": "",
            "query": {
                "pages": {
                    "123": {
                        "pageid": 123,
                        "ns": 0,
                        "title": "Test Page 1",
                        "extract": "This is a rich extract for test page 1 with more detailed content.",
                        "thumbnail": {
                            "source": "https://upload.wikimedia.org/wikipedia/test1.jpg",
                            "width": 300,
                            "height": 300,
                        },
                    },
                    "456": {
                        "pageid": 456,
                        "ns": 0,
                        "title": "Test Page 2",
                        "extract": "Extract for test page 2 with additional context about the topic.",
                    },
                },
            },
        }

    def _two_call_handler(self, responses: list):
        """Create a request handler that returns responses in sequence."""
        calls = []

        def handler(request: httpx.Request) -> httpx.Response:
            calls.append(request)
            idx = min(len(calls) - 1, len(responses) - 1)
            return responses[idx]

        return handler

    async def test_search_two_stage(self, adapter, opensearch_response, rich_query_response):
        handler = self._two_call_handler(
            [
                httpx.Response(200, json=opensearch_response),
                httpx.Response(200, json=rich_query_response),
            ],
        )
        async with MockHTTP(handler):
            result = await adapter.search("test query")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert result.results[0].title == "Test Page 1"
        assert result.results[0].thumbnail == "https://upload.wikimedia.org/wikipedia/test1.jpg"
        assert "rich extract" in result.results[0].content

    async def test_search_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("test")
        assert result.status == EngineStatus.RATE_LIMITED

    async def test_search_opensearch_only_no_results(self, adapter):
        """When opensearch returns no titles, return empty results."""
        def _handler(r):
            return httpx.Response(200, json=["test query", [], [], []])
        async with MockHTTP(_handler):
            result = await adapter.search("test query")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    async def test_opensearch_then_missing_pages(self, adapter, opensearch_response):
        """When rich query returns only missing pages, return empty results."""
        rich_empty = {
            "batchcomplete": "",
            "query": {"pages": {"-1": {"missing": "", "title": "Missing"}}},
        }
        handler = self._two_call_handler(
            [
                httpx.Response(200, json=opensearch_response),
                httpx.Response(200, json=rich_empty),
            ],
        )
        async with MockHTTP(handler):
            result = await adapter.search("test query")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0
