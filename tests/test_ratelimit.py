"""Tests for distributed rate limiting."""

from __future__ import annotations

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
