"""Tests for Valkey-backed SearchCache."""

from __future__ import annotations

import inspect
from unittest.mock import AsyncMock, MagicMock

import pytest

from slopsearx.cache import SearchCache, _answer_cache_key, _ttl_for_query, cache_key, normalize_query


class TestNormalizeQuery:
    """Query normalization for deterministic cache keys."""

    def test_lower_and_strip(self) -> None:
        assert normalize_query("  Hello World  ") == "hello world"

    def test_url_decode(self) -> None:
        assert normalize_query("hello+world") == "hello world"
        assert normalize_query("search%20query") == "search query"
        assert normalize_query("a%2Bb") == "a+b"  # %2B → +

    def test_trailing_punctuation_stripped(self) -> None:
        assert normalize_query("hello.") == "hello"
        assert normalize_query("hello!") == "hello"
        assert normalize_query("hello?") == "hello"
        assert normalize_query("hello,") == "hello"
        assert normalize_query("hello;") == "hello"
        assert normalize_query("hello:") == "hello"
        assert normalize_query("hello?!.") == "hello"

    def test_collapse_spaces(self) -> None:
        assert normalize_query("hello    world") == "hello world"
        assert normalize_query("  hello   world  ") == "hello world"

    def test_full_normalization_chain(self) -> None:
        assert normalize_query("  Hello+World%21  ") == "hello world"


class TestCacheKey:
    """Cache key construction."""

    def test_deterministic(self) -> None:
        k1 = cache_key("test query", "en", 0)
        k2 = cache_key("test query", "en", 0)
        assert k1 == k2
        assert k1.startswith("search:")

    def test_case_insensitive(self) -> None:
        k1 = cache_key("Hello World", "en", 0)
        k2 = cache_key("hello world", "en", 0)
        assert k1 == k2

    def test_different_language(self) -> None:
        k1 = cache_key("test", "en", 0)
        k2 = cache_key("test", "fr", 0)
        assert k1 != k2

    def test_different_safesearch(self) -> None:
        k1 = cache_key("test", "en", 0)
        k2 = cache_key("test", "en", 1)
        assert k1 != k2

    def test_normalized_equivalence(self) -> None:
        """Queries that differ only in casing/punctuation produce the same key."""
        k1 = cache_key("Hello World!", "en", 0)
        k2 = cache_key("hello world", "en", 0)
        assert k1 == k2

    def test_url_encoded_equivalence(self) -> None:
        """URL-encoded and decoded forms produce the same key."""
        k1 = cache_key("hello+world", "en", 0)
        k2 = cache_key("hello world", "en", 0)
        assert k1 == k2

    def test_trailing_punctuation_equivalence(self) -> None:
        """Same query with/without trailing punctuation produces the same key."""
        k1 = cache_key("python programming.", "en", 0)
        k2 = cache_key("python programming", "en", 0)
        assert k1 == k2


class TestAnswerCacheKey:
    """Answer-level cache key construction."""

    def test_prefix(self) -> None:
        key = _answer_cache_key("test query")
        assert key.startswith("answer:")

    def test_normalized(self) -> None:
        k1 = _answer_cache_key("Hello World!")
        k2 = _answer_cache_key("hello world")
        assert k1 == k2

    def test_no_language_dependence(self) -> None:
        """Answer key only depends on query, not language."""
        q = "test query"
        # Compare with cache_key to show different structure
        k = _answer_cache_key(q)
        assert k.startswith("answer:")
        assert "|" not in k  # no language/safesearch

    def test_different_queries_different_keys(self) -> None:
        assert _answer_cache_key("python") != _answer_cache_key("rust")


class TestTTL:
    """Category-based TTL logic."""

    def test_news_short_ttl(self) -> None:
        assert _ttl_for_query(["news"]) == 300
        assert _ttl_for_query(["tech", "news"]) == 300

    def test_general_default_ttl(self) -> None:
        assert _ttl_for_query(["general"]) == 3600
        assert _ttl_for_query([]) == 3600
        assert _ttl_for_query(None) == 3600

    def test_case_insensitive(self) -> None:
        assert _ttl_for_query(["News"]) == 300
        assert _ttl_for_query(["NEWS"]) == 300


class TestSearchCacheDisconnected:
    """SearchCache graceful degradation when Valkey unavailable."""

    def test_default_not_connected(self) -> None:
        cache = SearchCache(valkey_url="")
        assert not cache.is_connected

    async def test_get_returns_none(self) -> None:
        cache = SearchCache(valkey_url="")
        result = await cache.get("any_key")
        assert result is None

    async def test_set_noop(self) -> None:
        cache = SearchCache(valkey_url="")
        await cache.set("key", {"data": "test"})

    async def test_clear_noop(self) -> None:
        cache = SearchCache(valkey_url="")
        await cache.clear()

    def test_env_var_empty(self, monkeypatch: "pytest.MonkeyPatch") -> None:
        monkeypatch.setenv("VALKEY_URL", "")
        cache = SearchCache()
        assert not cache.is_connected

    async def test_connect_empty_url_noop(self) -> None:
        """Connect with empty URL does nothing."""
        cache = SearchCache(valkey_url="")
        await cache.connect()
        assert not cache.is_connected
        assert cache._client is None

    async def test_close_noop_when_not_connected(self) -> None:
        """Close is a no-op when not connected."""
        cache = SearchCache(valkey_url="")
        await cache.close()
        assert cache._client is None


class TestNegativeCacheDisconnected:
    """Negative caching graceful degradation."""

    async def test_set_error_noop_when_disconnected(self) -> None:
        cache = SearchCache(valkey_url="")
        await cache.set_error("some_key")
        # No exception is success

    async def test_set_error_noop_without_client(self) -> None:
        cache = SearchCache(valkey_url="")
        cache._connected = True  # pretend connected
        cache._client = None
        await cache.set_error("some_key")
        # No exception is success


class TestAnswerCacheDisconnected:
    """Answer caching graceful degradation."""

    async def test_get_answer_noop_when_disconnected(self) -> None:
        cache = SearchCache(valkey_url="")
        result = await cache.get_answer("test query")
        assert result is None

    async def test_set_answer_noop_when_disconnected(self) -> None:
        cache = SearchCache(valkey_url="")
        await cache.set_answer("test query", {"data": "test"})
        # No exception is success


class TestCacheAsyncConformance:
    """M3-006: All I/O methods are async def."""

    def test_connect_is_async(self) -> None:
        assert inspect.iscoroutinefunction(SearchCache.connect)

    def test_get_is_async(self) -> None:
        assert inspect.iscoroutinefunction(SearchCache.get)

    def test_set_is_async(self) -> None:
        assert inspect.iscoroutinefunction(SearchCache.set)

    def test_clear_is_async(self) -> None:
        assert inspect.iscoroutinefunction(SearchCache.clear)

    def test_close_is_async(self) -> None:
        assert inspect.iscoroutinefunction(SearchCache.close)

    def test_set_error_is_async(self) -> None:
        assert inspect.iscoroutinefunction(SearchCache.set_error)

    def test_get_answer_is_async(self) -> None:
        assert inspect.iscoroutinefunction(SearchCache.get_answer)

    def test_set_answer_is_async(self) -> None:
        assert inspect.iscoroutinefunction(SearchCache.set_answer)


class TestSearchCacheDefaults:
    """Default TTL values from env or hardcoded defaults."""

    def test_default_ttl_when_no_env(self) -> None:
        cache = SearchCache(valkey_url="")
        assert cache._default_ttl == 3600
        assert cache._negative_ttl == 60
        assert cache._answer_ttl == 3600

    def test_env_var_ttl_overrides(self, monkeypatch: "pytest.MonkeyPatch") -> None:
        monkeypatch.setenv("SEARCH_CACHE_TTL_SECONDS", "7200")
        monkeypatch.setenv("SEARCH_CACHE_NEGATIVE_TTL_SECONDS", "120")
        cache = SearchCache(valkey_url="")
        assert cache._default_ttl == 7200
        assert cache._negative_ttl == 120
        assert cache._answer_ttl == 7200


class TestSearchCacheMocked:
    """SearchCache operations with mocked Valkey client."""

    def _make_mocks(self) -> tuple[MagicMock, MagicMock]:
        """Create a properly configured mock cache with a connected client."""
        mock_client = MagicMock()
        mock_client.ping = AsyncMock()
        mock_client.get = AsyncMock()
        mock_client.setex = AsyncMock()
        mock_client.close = AsyncMock()
        mock_client.flushdb = AsyncMock()

        cache = SearchCache(valkey_url="")
        cache._connected = True
        cache._client = mock_client
        return mock_client, cache

    async def test_set_error_stores_with_negative_ttl(self) -> None:
        mock_client, cache = self._make_mocks()
        await cache.set_error("search:abc123")
        # Should use _negative_ttl (60 by default)
        mock_client.setex.assert_called_once()
        args = mock_client.setex.call_args
        assert args[0][0] == "search:abc123"  # key
        assert args[0][1] == 60  # TTL = _negative_ttl
        payload = args[0][2]
        assert "_error" in payload

    async def test_set_error_custom_ttl(self) -> None:
        mock_client, cache = self._make_mocks()
        await cache.set_error("search:abc123", ttl=30)
        args = mock_client.setex.call_args
        assert args[0][1] == 30

    async def test_get_answer_uses_answer_prefix(self) -> None:
        mock_client, cache = self._make_mocks()
        mock_client.get.return_value = None
        result = await cache.get_answer("test query")
        assert result is None
        # Should look up answer:{sha256}
        key = mock_client.get.call_args[0][0]
        assert key.startswith("answer:")
        assert len(key) > len("answer:")  # has digest suffix

    async def test_set_answer_uses_answer_prefix_and_default_ttl(self) -> None:
        mock_client, cache = self._make_mocks()
        await cache.set_answer("test query", {"data": "test"})
        mock_client.setex.assert_called_once()
        args = mock_client.setex.call_args
        key = args[0][0]
        assert key.startswith("answer:")
        assert args[0][1] == 3600  # default _answer_ttl

    async def test_set_answer_custom_ttl(self) -> None:
        mock_client, cache = self._make_mocks()
        await cache.set_answer("test query", {"data": "test"}, ttl=300)
        args = mock_client.setex.call_args
        assert args[0][1] == 300

    async def test_set_error_client_exception_logged(self) -> None:
        """set_error does not propagate Valkey exceptions."""
        mock_client, cache = self._make_mocks()
        mock_client.setex.side_effect = RuntimeError("Valkey error")
        await cache.set_error("some_key")
        # No exception is success

    async def test_get_answer_client_exception_returns_none(self) -> None:
        """get_answer does not propagate Valkey exceptions."""
        mock_client, cache = self._make_mocks()
        mock_client.get.side_effect = RuntimeError("Valkey error")
        result = await cache.get_answer("test query")
        assert result is None

    async def test_set_answer_client_exception_logged(self) -> None:
        """set_answer does not propagate Valkey exceptions."""
        mock_client, cache = self._make_mocks()
        mock_client.setex.side_effect = RuntimeError("Valkey error")
        await cache.set_answer("test query", {"data": "test"})
        # No exception is success
