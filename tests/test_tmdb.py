"""Tests for TMDB adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401
from slopsearx.adapter import EngineStatus, discover_engines, list_engines
from tests.test_adapters import MockHTTP

SAMPLE_MOVIE_RESPONSE = {
    "results": [
        {
            "media_type": "movie",
            "title": "Test Movie",
            "release_date": "2024-01-15",
            "overview": "A test movie about testing.",
            "vote_average": 7.5,
            "poster_path": "/testposter.jpg",
            "id": 12345,
        },
        {
            "media_type": "tv",
            "name": "Test TV Show",
            "first_air_date": "2023-06-01",
            "overview": "A test TV show.",
            "vote_average": 8.0,
            "id": 67890,
        },
    ],
}


class TestTMDBAdapterRegistration:
    def test_adapter_registered(self):
        assert "tmdb" in list_engines()


class TestTMDBAdapterSearch:
    @pytest.fixture
    def adapter(self):
        return discover_engines({"tmdb": {"enabled": True, "api_key": "test-tmdb-key"}})["tmdb"]

    async def test_search_returns_results(self, adapter):
        def _handler(r):
            return httpx.Response(200, json=SAMPLE_MOVIE_RESPONSE)

        async with MockHTTP(_handler):
            result = await adapter.search("test")

        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert result.results[0].title == "Test Movie"
        assert result.results[1].title == "Test TV Show"

    async def test_uses_bearer_auth_header(self):
        """TMDB sends API key as Authorization: Bearer header, not query param."""
        captured_headers = {}
        captured_url = ""

        def _handler(r):
            nonlocal captured_url
            captured_headers.update(dict(r.headers))
            captured_url = str(r.url)
            return httpx.Response(200, json={"results": []})

        instances = discover_engines({"tmdb": {"enabled": True, "api_key": "tmdb-bearer-test"}})
        async with MockHTTP(_handler):
            await instances["tmdb"].search("test")

        assert captured_headers.get("authorization") == "Bearer tmdb-bearer-test"
        assert "api_key=" not in captured_url

    async def test_missing_api_key_returns_not_configured(self):
        """Missing API key returns 'not configured' error."""
        instances = discover_engines({"tmdb": {"enabled": True, "api_key": ""}})
        result = await instances["tmdb"].search("test")
        assert result.status == EngineStatus.ERROR
        assert "API key not configured" in (result.error_message or "")

    async def test_empty_key_treated_as_not_configured(self):
        """Empty-string key treated as not configured."""
        instances = discover_engines({"tmdb": {"enabled": True, "api_key": ""}})
        result = await instances["tmdb"].search("test")
        assert result.status == EngineStatus.ERROR
        assert "API key not configured" in (result.error_message or "")

    async def test_whitespace_key_treated_as_not_configured(self):
        """Whitespace-only key treated as absent."""
        instances = discover_engines({"tmdb": {"enabled": True, "api_key": "   "}})
        result = await instances["tmdb"].search("test")
        assert result.status == EngineStatus.ERROR
        assert "API key not configured" in (result.error_message or "")

    async def test_http_status_error_sanitized(self):
        """HTTPStatusError must not leak API key in error_message."""
        instances = discover_engines({"tmdb": {"enabled": True, "api_key": "tmdb-secret-key-999"}})
        adapter = instances["tmdb"]

        def _handler(r):
            return httpx.Response(500)

        async with MockHTTP(_handler):
            result = await adapter.search("test")
        assert result.status == EngineStatus.ERROR
        assert result.error_message is not None
        assert "tmdb-secret-key-999" not in result.error_message

    async def test_broad_exception_sanitized(self):
        """Broad handler sanitizes error_message when exception contains a URL."""
        instances = discover_engines({"tmdb": {"enabled": True, "api_key": "tmdb-secret-key-999"}})
        adapter = instances["tmdb"]

        async with MockHTTP(lambda r: (_ for _ in ()).throw(RuntimeError(
            "https://api.themoviedb.org/3/search/multi?api_key=tmdb-secret-key-999&query=test&page=1"
        ))):
            result = await adapter.search("test")
        assert result.status == EngineStatus.ERROR
        assert result.error_message is not None
        assert "tmdb-secret-key-999" not in result.error_message

    async def test_rate_limited(self, adapter):
        def _handler(r):
            return httpx.Response(429)

        async with MockHTTP(_handler):
            result = await adapter.search("test")
        assert result.status == EngineStatus.RATE_LIMITED
        assert result.results == []

    async def test_timeout(self, adapter):
        async with MockHTTP(lambda r: (_ for _ in ()).throw(httpx.TimeoutException("timeout"))):
            result = await adapter.search("test")
        assert result.status == EngineStatus.TIMEOUT
