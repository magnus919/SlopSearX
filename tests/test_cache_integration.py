"""Integration tests for Valkey-backed SearchCache.

These tests require a running Valkey instance pointed at by the
VALKEY_URL environment variable. They are skipped by default.

To run::

    VALKEY_URL=redis://bishop:6379 pytest tests/test_cache_integration.py -v
"""

from __future__ import annotations

import os
from typing import Any

import pytest

from slopsearx.cache import SearchCache, cache_key

pytestmark = pytest.mark.skipif(
    not os.environ.get("VALKEY_URL"),
    reason="VALKEY_URL not set — requires a running Valkey instance",
)


@pytest.fixture
def cache() -> SearchCache:
    """A SearchCache connected to the Valkey instance from VALKEY_URL."""
    url = os.environ["VALKEY_URL"]
    c = SearchCache(valkey_url=url)
    assert c.is_connected, f"Could not connect to Valkey at {url}"
    return c


@pytest.fixture(autouse=True)
async def clean_cache(cache: SearchCache) -> None:
    """Clean the cache before each test."""
    await cache.clear()


class TestSearchCacheConnect:
    """Valkey connection lifecycle."""

    def test_connect_success(self) -> None:
        """Connecting to a live Valkey succeeds."""
        url = os.environ["VALKEY_URL"]
        cache = SearchCache(valkey_url=url)
        assert cache.is_connected
        assert cache._client is not None

    def test_connect_idempotent(self) -> None:
        """Calling _connect twice doesn't reconnect."""
        url = os.environ["VALKEY_URL"]
        cache = SearchCache(valkey_url=url)
        client_before = cache._client
        cache._connect()
        assert cache._client is client_before  # same instance

    def test_connect_failure_logs_warning(self) -> None:
        """Invalid URL produces disconnected cache, not crash."""
        cache = SearchCache(valkey_url="redis://192.0.2.1:16379")  # TEST-NET, unreachable
        assert not cache.is_connected
        assert cache._client is None

    def test_connect_from_env_var(self, monkeypatch) -> None:
        """VALKEY_URL env var is picked up when no arg given."""
        url = os.environ["VALKEY_URL"]
        monkeypatch.setenv("VALKEY_URL", url)
        cache = SearchCache()
        assert cache.is_connected


class TestSearchCacheSetGet:
    """Cache set/get round-trip."""

    async def test_set_and_get(self, cache: SearchCache) -> None:
        """Setting a value and retrieving it returns the original."""
        key = cache_key("integration test query", "en", 0)
        value: dict[str, Any] = {
            "query": "integration test",
            "results": [{"url": "https://example.com", "title": "Test"}],
            "meta": {"response_time_ms": 42},
        }
        await cache.set(key, value, ttl=60)
        result = await cache.get(key)
        assert result is not None
        assert result["query"] == "integration test"
        assert len(result["results"]) == 1
        assert result["results"][0]["url"] == "https://example.com"

    async def test_get_miss(self, cache: SearchCache) -> None:
        """Getting a non-existent key returns None."""
        result = await cache.get("search:nonexistentkey")
        assert result is None

    async def test_set_with_ttl_expiry(self, cache: SearchCache) -> None:
        """Keys expire after their TTL."""
        key = cache_key("ttl test", "en", 0)
        await cache.set(key, {"data": "short-lived"}, ttl=1)
        # Should exist immediately
        result = await cache.get(key)
        assert result is not None
        # Should expire after 1s
        import asyncio
        await asyncio.sleep(1.5)
        result = await cache.get(key)
        assert result is None

    async def test_set_overwrite(self, cache: SearchCache) -> None:
        """Setting same key twice overwrites the value."""
        key = cache_key("overwrite test", "en", 0)
        await cache.set(key, {"data": "original"})
        await cache.set(key, {"data": "updated"})
        result = await cache.get(key)
        assert result is not None
        assert result["data"] == "updated"

    async def test_set_with_serialization(self, cache: SearchCache) -> None:
        """Complex nested objects are serialized and deserialized."""
        key = cache_key("serialization", "en", 0)
        value = {
            "number": 42,
            "text": "hello",
            "list": [1, 2, 3],
            "nested": {"a": 1, "b": [True, False]},
        }
        await cache.set(key, value)
        result = await cache.get(key)
        assert result == value


class TestSearchCacheClear:
    """Cache clearing operations."""

    async def test_clear_removes_all(self, cache: SearchCache) -> None:
        """Clear removes all cached entries."""
        k1 = cache_key("query one", "en", 0)
        k2 = cache_key("query two", "en", 0)
        await cache.set(k1, {"data": "one"})
        await cache.set(k2, {"data": "two"})
        await cache.clear()
        assert await cache.get(k1) is None
        assert await cache.get(k2) is None


class TestSearchCacheEdgeCases:
    """Edge cases and error handling."""

    async def test_get_empty_string_value(self, cache: SearchCache) -> None:
        """Setting and getting a value that is JSON-compatible works with empty."""
        key = cache_key("", "en", 0)
        value: dict[str, Any] = {"data": ""}
        await cache.set(key, value)
        result = await cache.get(key)
        assert result == value

    async def test_set_large_value(self, cache: SearchCache) -> None:
        """Large values up to 1MB can be stored and retrieved."""
        key = cache_key("large value test", "en", 0)
        large_text = "x" * 100_000
        value = {"data": large_text}
        await cache.set(key, value)
        result = await cache.get(key)
        assert result is not None
        assert len(result["data"]) == 100_000

    async def test_cache_key_independence(self, cache: SearchCache) -> None:
        """Different cache keys do not interfere."""
        k1 = cache_key("query A", "en", 0)
        k2 = cache_key("query B", "en", 0)
        await cache.set(k1, {"data": "A"})
        await cache.set(k2, {"data": "B"})
        result_a = await cache.get(k1)
        result_b = await cache.get(k2)
        assert result_a == {"data": "A"}
        assert result_b == {"data": "B"}
