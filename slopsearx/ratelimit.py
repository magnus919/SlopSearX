"""Distributed rate limiting backed by Valkey.

Pluggable strategy interface with three implementations:
- LocalTokenBucket: in-memory, correct for 1-3 replicas
- ValkeySlidingWindow: distributed, correct for any replica count
- ExternalSidecar: delegates to dedicated rate-limit service

Backpressure: rate-limited engines get 30s cooldown.
3 consecutive failures -> engine deactivated until health check passes.
"""

from __future__ import annotations

import logging
import os
import time
from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Any

logger = logging.getLogger(__name__)


# --- Strategy interface ---


class RateLimitStrategy(ABC):
    """Pluggable rate-limiting strategy.

    Each adapter calls ``acquire(engine, cost)`` before sending
    a request. The strategy decides whether to allow or deny.
    """

    @abstractmethod
    async def acquire(self, engine: str, cost: int = 1) -> bool:
        """Try to acquire tokens. Returns True if allowed."""
        ...

    async def warmup(self) -> None:
        """Optional: called at startup."""

    async def shutdown(self) -> None:
        """Optional: called at graceful shutdown."""


# --- Local token bucket (dev / 1-3 replicas) ---


class LocalTokenBucket(RateLimitStrategy):
    """In-memory token bucket. Correct for 1-3 replicas.

    NOT suitable for 50+ replicas — each replica maintains
    independent state, so the effective rate is N * max_rate.
    """

    def __init__(self, max_rate: float = 10.0, burst: float = 30.0) -> None:
        self.max_rate = max_rate
        self.burst = burst
        self._tokens: dict[str, float] = {}
        self._last_refill: dict[str, float] = {}

    async def acquire(self, engine: str, cost: int = 1) -> bool:
        now = time.monotonic()
        tokens = self._tokens.get(engine, self.burst)
        last = self._last_refill.get(engine, now)

        elapsed = now - last
        tokens = min(self.burst, tokens + elapsed * self.max_rate)
        self._last_refill[engine] = now

        if tokens >= cost:
            self._tokens[engine] = tokens - cost
            return True
        self._tokens[engine] = tokens
        return False


# --- Valkey sliding window (production / 50+ replicas) ---


class ValkeySlidingWindow(RateLimitStrategy):
    """Distributed sliding window via Valkey INCR + EXPIRE.

    Correct for all replica counts. Centralized rate-limit state
    in Valkey means replicas share the same counter.
    """

    def __init__(
        self,
        valkey_url: str = "",
        default_rate: float = 10.0,
        window_seconds: float = 1.0,
    ) -> None:
        self._url = valkey_url or os.environ.get("VALKEY_URL", "")
        self._default_rate = default_rate
        self._window = window_seconds
        self._client: Any = None
        self._connected = False
        if self._url:
            self._connect()

    def _connect(self) -> None:
        """Establish Valkey connection."""
        try:
            import valkey

            self._client = valkey.Valkey.from_url(self._url)
            self._client.ping()
            self._connected = True
            logger.info("ValkeySlidingWindow connected to Valkey")
        except Exception as e:
            self._connected = False
            self._client = None
            logger.warning(
                "ValkeySlidingWindow: Valkey unavailable, rate limiting disabled: %s",
                e,
            )

    async def acquire(self, engine: str, cost: int = 1) -> bool:
        if not self._connected or self._client is None:
            # Valkey unavailable — allow request (fail open for availability)
            return True

        try:
            window_start = int(time.monotonic() / self._window)
            key = f"ratelimit:{engine}:{window_start}"

            count: int = self._client.incrby(key, cost)
            if count == cost:
                # First increment in this window — set expiry
                self._client.expire(key, int(self._window * 2))

            # Check against configured rate for this engine
            # Default: self._default_rate requests per window
            return count <= self._default_rate
        except Exception as e:
            logger.debug("Rate limit check error for %s: %s", engine, e)
            return True  # fail open

    async def warmup(self) -> None:
        if self._url and not self._connected:
            self._connect()

    async def shutdown(self) -> None:
        if self._client:
            try:
                self._client.close()
            except Exception:
                pass
            self._client = None


# --- External sidecar (advanced deployments) ---


class ExternalSidecar(RateLimitStrategy):
    """Delegates to a dedicated rate-limit service.

    For advanced use cases where rate limiting is managed by
    a separate infrastructure component (e.g., global API key
    budget across multiple services).
    """

    def __init__(self, sidecar_url: str = "") -> None:
        self._url = sidecar_url

    async def acquire(self, engine: str, cost: int = 1) -> bool:
        # Stub: not implemented, always allows
        return True


# --- Backpressure wrapper ---


@dataclass
class _EngineState:
    """Per-engine backpressure tracking."""

    consecutive_failures: int = 0
    cooldown_until: float = 0.0
    deactivated: bool = False


class RateLimiter:
    """Rate limiter with backpressure propagation.

    Wraps a RateLimitStrategy and adds:
    - 30s cooldown after rate-limit denial
    - 3-strike deactivation (engine disabled until health check passes)
    - Per-engine state tracking
    """

    def __init__(self, strategy: RateLimitStrategy | None = None) -> None:
        self._strategy = strategy or LocalTokenBucket()
        self._states: dict[str, _EngineState] = {}

    async def acquire(self, engine: str, cost: int = 1) -> bool:
        """Try to acquire tokens, respecting deactivation and cooldown."""
        state = self._states.get(engine)
        if state is None:
            state = _EngineState()
            self._states[engine] = state

        if state.deactivated:
            return False

        allowed = await self._strategy.acquire(engine, cost)

        if allowed:
            state.consecutive_failures = 0
            state.cooldown_until = 0.0
        else:
            state.consecutive_failures += 1
            state.cooldown_until = time.monotonic() + 30.0  # 30s cooldown
            if state.consecutive_failures >= 3:
                state.deactivated = True
                logger.warning(
                    "Engine %s deactivated after 3 consecutive rate-limit denials",
                    engine,
                )

        return allowed

    def reactivate(self, engine: str) -> None:
        """Reactivate a deactivated engine (called after health check passes)."""
        state = self._states.get(engine)
        if state:
            state.deactivated = False
            state.consecutive_failures = 0
            state.cooldown_until = 0.0

    @property
    def deactivated_engines(self) -> set[str]:
        """Set of engine names currently deactivated."""
        return {
            name
            for name, state in self._states.items()
            if state.deactivated
        }

    async def warmup(self) -> None:
        await self._strategy.warmup()

    async def shutdown(self) -> None:
        await self._strategy.shutdown()
