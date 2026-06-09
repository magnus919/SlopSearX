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


# ---------------------------------------------------------------------------
# arXiv adapter
# ---------------------------------------------------------------------------


class TestArxivAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"arxiv": {"enabled": True}})
        return instances["arxiv"]

    @pytest.fixture
    def sample_atom(self) -> str:
        return """<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <entry>
    <id>http://arxiv.org/abs/2401.12345</id>
    <title>Attention Is All You Need</title>
    <summary>A novel transformer architecture for sequence transduction tasks.</summary>
    <published>2024-01-15T00:00:00Z</published>
    <author><name>Vaswani et al.</name></author>
  </entry>
  <entry>
    <id>http://arxiv.org/abs/2402.67890</id>
    <title>BERT: Pre-training of Deep Bidirectional Transformers</title>
    <summary>Pre-training technique for natural language understanding.</summary>
    <published>2024-02-20T00:00:00Z</published>
    <author><name>Devlin et al.</name></author>
  </entry>
</feed>"""

    async def test_search_returns_results(self, adapter, sample_atom):
        async with MockHTTP(lambda r: httpx.Response(200, content=sample_atom.encode())):
            result = await adapter.search("transformer")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert result.results[0].title == "Attention Is All You Need"
        assert "arxiv.org/abs/2401.12345" in result.results[0].url
        assert result.results[0].published_date == "2024-01-15T00:00:00Z"
        assert result.results[1].title == "BERT: Pre-training of Deep Bidirectional Transformers"

    async def test_search_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("test")
        assert result.status == EngineStatus.RATE_LIMITED
        assert result.results == []

    async def test_search_empty_feed(self, adapter):
        empty_feed = """<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom"></feed>"""
        async with MockHTTP(lambda r: httpx.Response(200, content=empty_feed.encode())):
            result = await adapter.search("nonexistent")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0


# ---------------------------------------------------------------------------
# GitHub adapter
# ---------------------------------------------------------------------------


class TestGitHubAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"github": {"enabled": True, "api_key": "test-token"}})
        return instances["github"]

    @pytest.fixture
    def repo_response(self) -> dict:
        return {
            "items": [
                {
                    "html_url": "https://github.com/user/repo1",
                    "full_name": "user/repo1",
                    "name": "repo1",
                    "description": "A test repository",
                    "language": "Python",
                    "stargazers_count": 42,
                    "topics": ["ai", "search"],
                },
                {
                    "html_url": "https://github.com/user/repo2",
                    "full_name": "user/repo2",
                    "name": "repo2",
                    "description": "Another test repo",
                    "language": "Rust",
                    "stargazers_count": 17,
                    "topics": ["cli"],
                },
            ],
        }

    @pytest.fixture
    def issues_response(self) -> dict:
        return {
            "items": [
                {
                    "html_url": "https://github.com/user/repo/issues/1",
                    "title": "Bug: something breaks",
                    "state": "open",
                    "body": "When I do X, Y breaks",
                    "labels": [{"name": "bug"}],
                    "created_at": "2024-01-10T00:00:00Z",
                },
            ],
        }

    async def test_search_repos(self, adapter, repo_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=repo_response)):
            result = await adapter.search("test repo")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert result.results[0].url == "https://github.com/user/repo1"
        assert "★ 42" in result.results[0].content
        assert result.results[0].score == 42.0

    async def test_search_missing_token(self):
        instances = discover_engines({"github": {"enabled": True, "api_key": ""}})
        adapter = instances["github"]
        result = await adapter.search("test")
        assert result.status == EngineStatus.ERROR
        assert "token not configured" in (result.error_message or "").lower()

    async def test_search_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(403, content=b'{"message":"rate limit exceeded"}')):
            result = await adapter.search("test")
        assert result.status == EngineStatus.RATE_LIMITED
        assert result.results == []

    async def test_search_422_graceful(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(422, content=b'{"message":"code search limited"}')):
            result = await adapter.search("test")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0


# ---------------------------------------------------------------------------
# Hacker News adapter
# ---------------------------------------------------------------------------


class TestHackerNewsAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"hackernews": {"enabled": True}})
        return instances["hackernews"]

    @pytest.fixture
    def sample_response(self) -> dict:
        return {
            "hits": [
                {
                    "title": "Show HN: A new search engine",
                    "url": "https://example.com/search",
                    "objectID": "12345",
                    "points": 150,
                    "author": "testuser",
                    "created_at": "2024-01-15T00:00:00Z",
                    "num_comments": 30,
                },
                {
                    "title": "Ask HN: Best tools for 2024?",
                    "url": None,
                    "objectID": "67890",
                    "points": 75,
                    "author": "asker",
                    "created_at": "2024-01-14T00:00:00Z",
                    "num_comments": 0,
                },
            ],
        }

    async def test_search_returns_results(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("search engine")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert result.results[0].title == "Show HN: A new search engine"
        assert "▲ 150" in result.results[0].content
        assert "30 comments" in result.results[0].content
        assert result.results[0].score == 150.0

    async def test_search_story_without_url_uses_hn_link(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("ask")
        assert result.status == EngineStatus.OK
        # Second result has url=None → should use HN item link
        assert "news.ycombinator.com/item?id=67890" in result.results[1].url

    async def test_search_empty_results(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(200, json={"hits": []})):
            result = await adapter.search("xyznonexistent")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    async def test_search_timeout(self, adapter):
        async with MockHTTP(lambda r: (_ for _ in ()).throw(httpx.TimeoutException("timeout"))):
            result = await adapter.search("test")
        assert result.status == EngineStatus.TIMEOUT


# ---------------------------------------------------------------------------
# Semantic Scholar adapter
# ---------------------------------------------------------------------------


class TestSemanticScholarAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"semanticscholar": {"enabled": True}})
        return instances["semanticscholar"]

    @pytest.fixture
    def sample_response(self) -> dict:
        return {
            "data": [
                {
                    "paperId": "abc123",
                    "title": "Deep Learning in Natural Language Processing",
                    "url": "https://www.semanticscholar.org/paper/abc123",
                    "abstract": (
                        "A comprehensive survey of deep learning techniques "
                        "for NLP including transformers and attention mechanisms."
                    ),
                    "citationCount": 250,
                    "publicationDate": "2023-06-15",
                    "externalIds": {"ArXiv": "2306.12345", "DOI": "10.1234/example"},
                    "authors": [{"name": "Alice Smith"}, {"name": "Bob Jones"}],
                },
                {
                    "paperId": "def456",
                    "title": "Graph Neural Networks",
                    "url": "https://www.semanticscholar.org/paper/def456",
                    "abstract": "A review of graph neural network architectures and applications.",
                    "citationCount": 180,
                    "publicationDate": "2023-03-10",
                    "externalIds": {},
                    "authors": [{"name": "Carol Wang"}],
                },
            ],
        }

    async def test_search_returns_results(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("deep learning")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert "Deep Learning" in result.results[0].title
        assert "Cited by: 250" in result.results[0].content
        assert result.results[0].published_date == "2023-06-15"
        assert result.results[0].score == 250.0

    async def test_search_includes_arxiv_id(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("deep learning")
        assert "arXiv: 2306.12345" in result.results[0].content

    async def test_search_sends_api_key_when_configured(self):
        instances = discover_engines({"semanticscholar": {"enabled": True, "api_key": "s2-fake-key"}})
        adapter = instances["semanticscholar"]

        header_checks = {}

        def _handler(r):
            header_checks["api_key"] = r.headers.get("x-api-key", "")
            return httpx.Response(200, json={"data": []})

        async with MockHTTP(_handler):
            await adapter.search("test")
        assert header_checks.get("api_key") == "s2-fake-key"

    async def test_search_empty(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(200, json={"data": []})):
            result = await adapter.search("nonexistent")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    async def test_search_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("test")
        assert result.status == EngineStatus.RATE_LIMITED


# ---------------------------------------------------------------------------
# HuggingFace adapter
# ---------------------------------------------------------------------------


class TestHuggingFaceAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"huggingface": {"enabled": True}})
        return instances["huggingface"]

    @pytest.fixture
    def model_response(self) -> list[dict]:
        return [
            {
                "modelId": "mistralai/Mistral-7B-v0.1",
                "pipeline_tag": "text-generation",
                "library_name": "transformers",
                "downloads": 500000,
                "likes": 1200,
                "description": "Mistral 7B base model",
            },
            {
                "modelId": "openai-community/gpt2",
                "pipeline_tag": "text-generation",
                "library_name": "transformers",
                "downloads": 300000,
                "likes": 800,
                "description": "GPT-2 language model",
            },
        ]

    async def test_search_returns_models(self, adapter, model_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=model_response)):
            result = await adapter.search("llm")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert result.results[0].title == "mistralai/Mistral-7B-v0.1"
        assert "huggingface.co/mistralai/Mistral-7B-v0.1" in result.results[0].url
        assert "♥1200" in result.results[0].content
        assert result.results[0].score == 1200.0

    async def test_search_empty(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(200, json=[])):
            result = await adapter.search("nonexistent-model")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    async def test_search_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("test")
        assert result.status == EngineStatus.RATE_LIMITED

    async def test_search_sends_token_when_configured(self):
        instances = discover_engines({"huggingface": {"enabled": True, "api_key": "hf-fake-token"}})
        adapter = instances["huggingface"]

        header_checks = {}

        def _handler(r):
            header_checks["auth"] = r.headers.get("Authorization", "")
            return httpx.Response(200, json=[])

        async with MockHTTP(_handler):
            await adapter.search("test")
        assert "hf-fake-token" in header_checks.get("auth", "")
