"""Tests for Valkey-backed SearchCache."""

from __future__ import annotations

import inspect
from unittest.mock import AsyncMock, MagicMock

import pytest

from slopsearx.cache import SearchCache, _answer_cache_key, _ttl_for_query, cache_key, normalize_query


class TestNormalizeQuery:
    def test_strip_boundary_whitespace(self) -> None:
        assert normalize_query("  Hello World  ") == "Hello World"

    @pytest.mark.parametrize("query", ["C++", "C#", "hello?", "a%2Bb", "a+b", '"a  b"', "Hello"])
    def test_preserve_query_syntax(self, query: str) -> None:
        assert normalize_query(query) == query


class TestCacheKey:
    """Cache key construction."""

    def test_deterministic(self) -> None:
        k1 = cache_key("test query", "en", 0)
        k2 = cache_key("test query", "en", 0)
        assert k1 == k2
        assert k1.startswith("search:")

    def test_case_preserved(self) -> None:
        k1 = cache_key("Hello World", "en", 0)
        k2 = cache_key("hello world", "en", 0)
        assert k1 != k2

    def test_different_language(self) -> None:
        k1 = cache_key("test", "en", 0)
        k2 = cache_key("test", "fr", 0)
        assert k1 != k2

    def test_different_safesearch(self) -> None:
        k1 = cache_key("test", "en", 0)
        k2 = cache_key("test", "en", 1)
        assert k1 != k2

    def test_punctuation_preserved(self) -> None:
        """Case and punctuation can carry engine-specific meaning."""
        k1 = cache_key("Hello World!", "en", 0)
        k2 = cache_key("hello world", "en", 0)
        assert k1 != k2

    def test_url_encoding_preserved(self) -> None:
        """Queries reach the cache after transport decoding."""
        k1 = cache_key("hello+world", "en", 0)
        k2 = cache_key("hello world", "en", 0)
        assert k1 != k2

    def test_trailing_punctuation_preserved(self) -> None:
        """Preserve trailing operator syntax."""
        k1 = cache_key("python programming.", "en", 0)
        k2 = cache_key("python programming", "en", 0)
        assert k1 != k2


class TestAnswerCacheKey:
    """Answer-level cache key construction."""

    def test_prefix(self) -> None:
        key = _answer_cache_key("test query")
        assert key.startswith("answer:")

    def test_normalized(self) -> None:
        k1 = _answer_cache_key("Hello World!")
        k2 = _answer_cache_key("hello world")
        assert k1 != k2

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

    def test_case_preserved(self) -> None:
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


@pytest.mark.parametrize(
    "left,right", [("C++", "C"), ("C#", "C"), ("a+b", "a b"), ("%20", " "), ("C?", "C"), ('"a  b"', '"a b"')]
)
def test_meaningful_queries_do_not_collide(left: str, right: str) -> None:
    assert cache_key(left) != cache_key(right)
    assert _answer_cache_key(left) != _answer_cache_key(right)


def test_version_and_unambiguous_scope_encoding() -> None:
    assert cache_key("C++").startswith("search:v2:")
    assert cache_key("x", categories=["a,b"]) != cache_key("x", categories=["a", "b"])


async def test_failed_startup_retries_once_after_backoff(monkeypatch) -> None:
    import valkey.asyncio

    broken = AsyncMock()
    broken.ping.side_effect = ConnectionError("offline")
    recovered = AsyncMock()
    recovered.get.return_value = '{"ok": true}'
    factory = MagicMock(side_effect=[broken, recovered])
    monkeypatch.setattr(valkey.asyncio.Valkey, "from_url", factory)
    cache = SearchCache("redis://unused:6379")
    await cache.connect()
    assert not cache.is_connected
    broken.aclose.assert_awaited_once()
    assert await cache.get("key") is None
    assert factory.call_count == 1
    cache._retry_at = 0
    assert await cache.get("key") == {"ok": True}
    assert factory.call_count == 2
    await cache.close()
    assert await cache.get("key") is None
    assert factory.call_count == 2


@pytest.mark.parametrize(
    "normal,partial,categories,expected",
    [
        (900, 30, [], 900),
        (900, 30, ["news"], 300),
        (10, 30, ["news"], 10),
    ],
)
def test_configured_normal_ttl(monkeypatch, normal, partial, categories, expected):
    monkeypatch.setenv("SEARCH_CACHE_TTL_SECONDS", str(normal))
    monkeypatch.setenv("SEARCH_CACHE_PARTIAL_TTL_SECONDS", str(partial))
    assert _ttl_for_query(categories) == expected
    assert _ttl_for_query(categories, partial=True) == min(expected, partial)


@pytest.mark.parametrize(
    "name,value",
    [
        ("SEARCH_CACHE_TTL_SECONDS", "0"),
        ("SEARCH_CACHE_TTL_SECONDS", "-1"),
        ("SEARCH_CACHE_TTL_SECONDS", "garbage"),
        ("SEARCH_CACHE_TTL_SECONDS", "1.5"),
        ("SEARCH_CACHE_PARTIAL_TTL_SECONDS", "0"),
        ("SEARCH_CACHE_PARTIAL_TTL_SECONDS", "-1"),
        ("SEARCH_CACHE_PARTIAL_TTL_SECONDS", "301"),
        ("SEARCH_CACHE_PARTIAL_TTL_SECONDS", "garbage"),
    ],
)
def test_invalid_cache_ttl_rejected_at_startup(monkeypatch, name, value):
    monkeypatch.setenv(name, value)
    with pytest.raises(ValueError, match=name):
        SearchCache()


@pytest.mark.parametrize("value", ["0", "-1", "bad", "1.5"])
def test_invalid_global_ttl_rejected_by_config(monkeypatch, value):
    from slopsearx.config import load_config

    monkeypatch.setenv("SEARCH_CACHE_TTL_SECONDS", value)
    with pytest.raises(ValueError, match="SEARCH_CACHE_TTL_SECONDS"):
        load_config()
