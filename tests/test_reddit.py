"""Tests for the Reddit adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401 — trigger @register_engine
from slopsearx.adapter import EngineStatus, discover_engines, list_engines
from tests.test_adapters import MockHTTP

# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture
def adapter():
    instances = discover_engines({"reddit": {}})
    return instances["reddit"]


@pytest.fixture
def sample_listing() -> dict:
    """A minimal Reddit search listing response with 3 posts."""
    return {
        "data": {
            "children": [
                {
                    "kind": "t3",
                    "data": {
                        "title": "Reddit Post One",
                        "url": "https://example.com/post1",
                        "selftext": "This is the content of post one.",
                        "permalink": "/r/test/comments/abc123/reddit_post_one/",
                        "score": 150,
                        "num_comments": 25,
                        "author": "user1",
                        "subreddit": "test",
                        "created_utc": 1700000000,
                        "over_18": False,
                        "thumbnail": "https://example.com/thumb1.jpg",
                    },
                },
                {
                    "kind": "t3",
                    "data": {
                        "title": "Reddit Post Two - Self Post",
                        "url": "",
                        "selftext": "",
                        "permalink": "/r/test/comments/def456/reddit_post_two/",
                        "score": 89,
                        "num_comments": 5,
                        "author": "user2",
                        "subreddit": "test",
                        "created_utc": 1700001000,
                        "over_18": False,
                        "thumbnail": "self",
                    },
                },
                {
                    "kind": "t3",
                    "data": {
                        "title": "NSFW Post Three",
                        "url": "https://example.com/nsfw",
                        "selftext": "This should be filtered out.",
                        "permalink": "/r/test/comments/ghi789/nsfw_post_three/",
                        "score": 999,
                        "num_comments": 0,
                        "author": "nsfw_user",
                        "subreddit": "test",
                        "created_utc": 1700002000,
                        "over_18": True,  # Should be filtered
                        "thumbnail": "nsfw",
                    },
                },
            ],
        },
    }


@pytest.fixture
def empty_listing() -> dict:
    """A listing with no children."""
    return {"data": {"children": [], "after": None, "before": None}}


# ---------------------------------------------------------------------------
# Adapter registration
# ---------------------------------------------------------------------------


class TestRedditAdapterRegistration:
    def test_adapter_registered(self):
        """Reddit adapter is in the engine registry."""
        assert "reddit" in list_engines()

    def test_adapter_categories(self):
        """Has correct category tags."""
        cls = list_engines()["reddit"]
        cats = cls.categories
        assert "general" in cats
        assert "social" in cats
        assert "reddit:subreddit" in cats

    def test_adapter_default_enabled(self):
        """Reddit is enabled by default (no auth needed)."""
        from slopsearx.config import _DEFAULT_ENGINES

        cfg = _DEFAULT_ENGINES["reddit"]
        assert cfg.get("enabled", True) is True


# ---------------------------------------------------------------------------
# API interaction
# ---------------------------------------------------------------------------


class TestRedditAdapterSearch:
    async def test_search_returns_results(self, adapter, sample_listing):
        """Happy path: returns parsed SearchResult list."""

        def _handler(r):
            assert "search.json" in str(r.url)
            return httpx.Response(200, json=sample_listing)

        async with MockHTTP(_handler):
            result = await adapter.search("test query")

        assert result.status == EngineStatus.OK
        assert len(result.results) == 2  # NSFW post filtered out
        assert result.results[0].title == "Reddit Post One"
        assert result.results[0].url == "https://example.com/post1"
        assert result.results[0].score == 150.0
        assert result.results[0].category == "social:test"
        assert result.results[0].thumbnail == "https://example.com/thumb1.jpg"

    async def test_search_self_post_no_url(self, adapter, sample_listing):
        """Self posts with no external URL get reddit permalink."""

        def _handler(r):
            return httpx.Response(200, json=sample_listing)

        async with MockHTTP(_handler):
            result = await adapter.search("test query")

        # The self post (index 1) has no url but has permalink
        self_post = result.results[1]
        assert self_post.url == "https://www.reddit.com/r/test/comments/def456/reddit_post_two/"
        # Content should be the fallback since selftext is empty
        assert "Score:" in self_post.content

    async def test_search_empty_results(self, adapter, empty_listing):
        """Empty listing returns empty results with OK status."""

        def _handler(r):
            return httpx.Response(200, json=empty_listing)

        async with MockHTTP(_handler):
            result = await adapter.search("test query")

        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    async def test_search_rate_limited(self, adapter):
        """429 returns RATE_LIMITED status."""

        def _handler(r):
            return httpx.Response(429)

        async with MockHTTP(_handler):
            result = await adapter.search("test query")

        assert result.status == EngineStatus.RATE_LIMITED

    async def test_search_blocked(self, adapter):
        """403 returns BLOCKED status."""

        def _handler(r):
            return httpx.Response(403)

        async with MockHTTP(_handler):
            result = await adapter.search("test query")

        assert result.status == EngineStatus.BLOCKED

    async def test_search_error(self, adapter):
        """500 returns ERROR status."""

        def _handler(r):
            return httpx.Response(500)

        async with MockHTTP(_handler):
            result = await adapter.search("test query")

        assert result.status == EngineStatus.ERROR

    async def test_search_timeout(self, adapter):
        """Timeout returns TIMEOUT status."""

        def _handler(r):
            raise httpx.TimeoutException("timeout", request=r)

        async with MockHTTP(_handler):
            result = await adapter.search("test query")

        assert result.status == EngineStatus.TIMEOUT

    async def test_search_filters_nsfw(self, adapter, sample_listing):
        """NSFW (over_18) items are filtered out."""

        def _handler(r):
            return httpx.Response(200, json=sample_listing)

        async with MockHTTP(_handler):
            result = await adapter.search("test query")

        titles = [r.title for r in result.results]
        assert "NSFW Post Three" not in titles

    async def test_search_sends_user_agent(self, adapter):
        """Request includes a proper User-Agent header."""
        headers = {}

        def _handler(r):
            headers["ua"] = r.headers.get("User-Agent", "")
            return httpx.Response(200, json={"data": {"children": []}})

        async with MockHTTP(_handler):
            await adapter.search("test query")

        assert "SlopSearX" in headers["ua"]
        assert "Reddit" not in headers["ua"]  # Should not claim to be a browser

    async def test_search_content_from_selftext(self, adapter, sample_listing):
        """Content field is populated from selftext when available."""

        def _handler(r):
            return httpx.Response(200, json=sample_listing)

        async with MockHTTP(_handler):
            result = await adapter.search("test query")

        assert result.results[0].content == "This is the content of post one."


# ---------------------------------------------------------------------------
# Sub-category routing
# ---------------------------------------------------------------------------


class TestRedditAdapterSubreddit:
    async def test_subreddit_routing(self, adapter):
        """reddit:subreddit category routes to /r/{subreddit}/search.json."""
        requested_url = ""

        def _handler(r):
            nonlocal requested_url
            requested_url = str(r.url)
            return httpx.Response(200, json={"data": {"children": []}})

        async with MockHTTP(_handler):
            await adapter.search("test query", {"categories": ["reddit:subreddit"], "subreddit": "python"})

        assert "python" in requested_url
        assert "/r/python/search.json" in requested_url

    async def test_subreddit_defaults_to_all(self, adapter):
        """No subreddit param defaults to 'all'."""
        requested_url = ""

        def _handler(r):
            nonlocal requested_url
            requested_url = str(r.url)
            return httpx.Response(200, json={"data": {"children": []}})

        async with MockHTTP(_handler):
            await adapter.search("test query", {"categories": ["reddit:subreddit"]})

        assert "/r/all/search.json" in requested_url

    async def test_general_search_no_subreddit(self, adapter):
        """General search (no reddit:subreddit) uses top-level /search.json."""

        def _handler(r):
            assert "search.json" in str(r.url)
            assert "/r/" not in str(r.url).split(".com")[1].split("/search.json")[0]
            return httpx.Response(200, json={"data": {"children": []}})

        async with MockHTTP(_handler):
            await adapter.search("test query", {"categories": ["social"]})

    async def test_subreddit_does_not_affect_general_search(self, adapter):
        """subreddit param is ignored when reddit:subreddit category not set."""
        requested_url = ""

        def _handler(r):
            nonlocal requested_url
            requested_url = str(r.url)
            return httpx.Response(200, json={"data": {"children": []}})

        async with MockHTTP(_handler):
            await adapter.search("test query", {"categories": ["general"], "subreddit": "python"})

        assert "/r/python/" not in requested_url
        assert "/search.json" in requested_url
