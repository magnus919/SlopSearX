from __future__ import annotations

from slopsearx.mcp.harness import InMemoryStore
from slopsearx.portal_rate_limit import PortalRateLimiter


async def test_rate_limit_is_bounded_keyed_and_resets_by_window() -> None:
    now = [1000.0]
    store = InMemoryStore()
    limiter = PortalRateLimiter(store, key=b"k" * 32, clock=lambda: now[0])
    assert await limiter.allow("mutation", "raw-principal", limit=2, window=60)
    assert await limiter.allow("mutation", "raw-principal", limit=2, window=60)
    assert not await limiter.allow("mutation", "raw-principal", limit=2, window=60)
    assert all("raw-principal" not in key for key in store._data)
    now[0] += 60
    assert await limiter.allow("mutation", "raw-principal", limit=2, window=60)


async def test_rate_limit_fails_closed_when_store_is_unavailable() -> None:
    store = InMemoryStore()
    store.is_connected = False
    limiter = PortalRateLimiter(store, key=b"k" * 32)
    assert not await limiter.allow("read", "principal", limit=1, window=60)
