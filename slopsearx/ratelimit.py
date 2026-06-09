"""Distributed rate limiting backed by Valkey (stub — no-op in V1)."""

from __future__ import annotations

import asyncio
import time


class RateLimiter:
    """Distributed rate limiter backed by Valkey.

    V1 uses a local token-bucket implementation as a default.
    Valkey-backed distributed mode will follow.
    """

    def __init__(self, max_rate: float = 10.0, window_seconds: float = 1.0) -> None:
        self.max_rate = max_rate
        self.window = window_seconds
        self._tokens: float = max_rate
        self._last_refill: float = time.monotonic()

    async def acquire(self) -> bool:
        """Try to acquire a token. Returns True if allowed, False if rate-limited."""
        now = time.monotonic()
        elapsed = now - self._last_refill
        self._tokens = min(self.max_rate, self._tokens + elapsed * self.max_rate / self.window)
        self._last_refill = now

        if self._tokens >= 1.0:
            self._tokens -= 1.0
            return True
        return False

    async def wait_acquire(self) -> None:
        """Block until a token is available."""
        while not await self.acquire():
            await asyncio.sleep(0.05)
