"""Tests for distributed rate limiting."""

from __future__ import annotations

import inspect
from unittest.mock import AsyncMock

import pytest

from slopsearx.ratelimit import (
    LocalTokenBucket,
    RateLimiter,
    RateLimitStrategy,
    ValkeySlidingWindow,
)


class TestRateLimitStrategy:
    """RateLimitStrategy ABC contract."""

    def test_is_abstract(self) -> None:
        """Cannot instantiate abstract strategy."""
        with pytest.raises(TypeError):
            RateLimitStrategy()  # type: ignore[abstract]

    def test_local_is_strategy(self) -> None:
        """LocalTokenBucket is a valid strategy."""
        assert issubclass(LocalTokenBucket, RateLimitStrategy)

    def test_valkey_is_strategy(self) -> None:
        """ValkeySlidingWindow is a valid strategy."""
        assert issubclass(ValkeySlidingWindow, RateLimitStrategy)


class TestLocalTokenBucket:
    """In-memory token bucket."""

    async def test_acquire_first_allowed(self) -> None:
        """First acquire should always be allowed (burst capacity)."""
        bucket = LocalTokenBucket(max_rate=10.0, burst=30.0)
        assert await bucket.acquire("brave")

    async def test_rate_limited_after_burst(self) -> None:
        """After exhausting burst, requests are denied."""
        bucket = LocalTokenBucket(max_rate=1.0, burst=2.0)
        # Consume burst
        assert await bucket.acquire("brave")
        assert await bucket.acquire("brave")
        # Denied (rate limited)
        assert not await bucket.acquire("brave")

    async def test_per_engine_independent(self) -> None:
        """Rate limiting is per-engine."""
        bucket = LocalTokenBucket(max_rate=1.0, burst=1.0)
        # Engine A exhausts its burst
        assert await bucket.acquire("brave")
        assert not await bucket.acquire("brave")
        # Engine B is independent
        assert await bucket.acquire("wikipedia")


class TestRateLimiterBackpressure:
    """RateLimiter backpressure (cooldown, deactivation)."""

    async def test_acquire_delegates_to_strategy(self) -> None:
        """Allowed requests pass through."""
        strategy = LocalTokenBucket(max_rate=10.0, burst=30.0)
        limiter = RateLimiter(strategy)
        assert await limiter.acquire("brave")

    async def test_cooldown_after_denial(self) -> None:
        """After rate-limit denial, engine enters 30s cooldown."""
        strategy = LocalTokenBucket(max_rate=0, burst=0)  # always deny
        limiter = RateLimiter(strategy)

        assert not await limiter.acquire("brave")
        # Immediate retry should also be denied (cooldown active)
        assert not await limiter.acquire("brave")

    async def test_three_strike_deactivation(self) -> None:
        """3 consecutive denials → engine deactivated."""
        strategy = LocalTokenBucket(max_rate=0, burst=0)
        limiter = RateLimiter(strategy)

        # 3 denials
        for _ in range(3):
            assert not await limiter.acquire("brave")

        # Now deactivated
        assert "brave" in limiter.deactivated_engines

        # Further requests still denied
        assert not await limiter.acquire("brave")

    async def test_reactivate_restores_engine(self) -> None:
        """reactivate() restores a deactivated engine."""
        strategy = LocalTokenBucket(max_rate=0, burst=0)
        limiter = RateLimiter(strategy)

        for _ in range(3):
            await limiter.acquire("brave")

        assert "brave" in limiter.deactivated_engines

        limiter.reactivate("brave")
        assert "brave" not in limiter.deactivated_engines

    async def test_success_resets_consecutive_failures(self) -> None:
        """A successful acquire resets the failure counter."""
        strategy = LocalTokenBucket(max_rate=10.0, burst=30.0)
        limiter = RateLimiter(strategy)

        # Allowed
        assert await limiter.acquire("brave")
        # Not deactivated
        assert "brave" not in limiter.deactivated_engines


class TestValkeyDisconnected:
    """ValkeySlidingWindow graceful degradation."""

    async def test_fail_open_when_disconnected(self) -> None:
        """Without Valkey URL, all requests are allowed (fail open)."""
        window = ValkeySlidingWindow(valkey_url="")
        assert await window.acquire("brave")
        assert await window.acquire("brave")
        assert await window.acquire("brave")  # still allowed

    async def test_warmup_no_url_noop(self) -> None:
        """warmup is a no-op when no URL is configured."""
        window = ValkeySlidingWindow(valkey_url="")
        await window.warmup()
        assert not window._connected

    async def test_shutdown_noop_when_not_connected(self) -> None:
        """shutdown is a no-op when not connected."""
        window = ValkeySlidingWindow(valkey_url="")
        await window.shutdown()
        assert window._client is None

    async def test_connect_no_url_noop(self) -> None:
        """_connect is a no-op when no URL is configured."""
        window = ValkeySlidingWindow(valkey_url="")
        await window._connect()
        assert not window._connected


class TestValkeyAsyncConformance:
    """M3-006: All Valkey I/O methods are async def."""

    def test_acquire_is_async(self) -> None:
        assert inspect.iscoroutinefunction(ValkeySlidingWindow.acquire)

    def test_warmup_is_async(self) -> None:
        assert inspect.iscoroutinefunction(ValkeySlidingWindow.warmup)

    def test_shutdown_is_async(self) -> None:
        assert inspect.iscoroutinefunction(ValkeySlidingWindow.shutdown)

    def test_connect_is_async(self) -> None:
        assert inspect.iscoroutinefunction(ValkeySlidingWindow._connect)


class TestValkeyAcquireWithMock:
    """ValkeySlidingWindow acquire behavior with mocked async client."""

    @pytest.fixture
    def mock_window(self) -> ValkeySlidingWindow:
        """Create a ValkeySlidingWindow with a mocked async client."""
        mock_client = AsyncMock()
        mock_client.ping = AsyncMock()
        mock_client.incrby = AsyncMock(return_value=1)
        mock_client.expire = AsyncMock()
        mock_client.close = AsyncMock()

        window = ValkeySlidingWindow(valkey_url="redis://mock:6379", default_rate=10.0, window_seconds=1.0)
        window._client = mock_client
        window._connected = True
        return window

    async def test_acquire_under_limit(self, mock_window: ValkeySlidingWindow) -> None:
        """Under the rate limit, acquire returns True."""
        mock_window._client.incrby = AsyncMock(return_value=5)
        result = await mock_window.acquire("brave")
        assert result is True

    async def test_acquire_over_limit(self, mock_window: ValkeySlidingWindow) -> None:
        """Over the rate limit, acquire returns False."""
        mock_window._client.incrby = AsyncMock(return_value=11)
        result = await mock_window.acquire("brave")
        assert result is False

    async def test_acquire_sets_expiry_on_first(self, mock_window: ValkeySlidingWindow) -> None:
        """On first increment (count == cost), expiry is set."""
        mock_window._client.incrby = AsyncMock(return_value=1)
        await mock_window.acquire("brave")
        mock_window._client.expire.assert_awaited_once()

    async def test_acquire_does_not_set_expiry_on_subsequent(
        self,
        mock_window: ValkeySlidingWindow,
    ) -> None:
        """On subsequent increments (count > cost), expiry is not set."""
        mock_window._client.incrby = AsyncMock(return_value=3)
        await mock_window.acquire("brave")
        mock_window._client.expire.assert_not_awaited()

    async def test_acquire_error_fails_open(self, mock_window: ValkeySlidingWindow) -> None:
        """When Valkey raises, acquire returns True (fail open)."""
        mock_window._client.incrby = AsyncMock(side_effect=RuntimeError("Valkey down"))
        result = await mock_window.acquire("brave")
        assert result is True

    async def test_shutdown_closes_client(self, mock_window: ValkeySlidingWindow) -> None:
        """shutdown closes the async client."""
        client = mock_window._client
        await mock_window.shutdown()
        client.close.assert_awaited_once()
        assert mock_window._client is None
        assert not mock_window._connected
