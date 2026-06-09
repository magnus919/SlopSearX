"""Tests for Valkey-backed SearchCache."""

from __future__ import annotations

from slopsearx.cache import SearchCache, _ttl_for_query, cache_key


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


class TestTTL:
    """Category-based TTL logic."""

    def test_news_short_ttl(self) -> None:
        assert _ttl_for_query(["news"]) == 60
        assert _ttl_for_query(["tech", "news"]) == 60

    def test_general_default_ttl(self) -> None:
        assert _ttl_for_query(["general"]) == 300
        assert _ttl_for_query([]) == 300
        assert _ttl_for_query(None) == 300

    def test_case_insensitive(self) -> None:
        assert _ttl_for_query(["News"]) == 60
        assert _ttl_for_query(["NEWS"]) == 60


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

    def test_env_var_empty(self, monkeypatch) -> None:
        monkeypatch.setenv("VALKEY_URL", "")
        cache = SearchCache()
        assert not cache.is_connected
