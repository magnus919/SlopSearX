"""
SlopSearX — Per-Engine Quality Telemetry.

Stores per-engine quality metrics in Valkey for operator dashboards,
V2 ranking calibration, and silent-quality-degradation detection.

Schema::

    engine_stats:{engine_name}:{YYYY-MM-DD} → Hash
      queries: 15230
      results_returned: 142301
      errors: 234
      rate_limited: 45
      avg_latency_ms: 420
      avg_result_score: 0.73
"""

from __future__ import annotations

import logging
import time
from typing import Any

from slopsearx.adapter import EngineStatus

logger = logging.getLogger(__name__)


class EngineStatsTracker:
    """Fire-and-forget per-engine quality metrics in Valkey.

    Each call to ``record_query`` asynchronously increments counters
    in Valkey via HINCRBY and HSET. Daily key rotation is handled
    automatically by the date component of the key.

    Graceful degradation: Valkey unavailability is non-fatal — the
    call is a no-op.
    """

    def __init__(self, cache: Any = None) -> None:
        self._cache = cache

    def _daily_key(self, engine: str) -> str:
        """Build the daily stats key for an engine.

        Example: ``engine_stats:brave:2026-06-10``
        """
        today = time.strftime("%Y-%m-%d")
        return f"engine_stats:{engine}:{today}"

    async def record_query(
        self,
        engine: str,
        result_count: int,
        latency_ms: float,
        status: EngineStatus,
        avg_score: float = 0.0,
    ) -> None:
        """Record a single query's results for an engine.

        This is an async fire-and-forget call. The actual Valkey
        writes happen via an awaited async pipeline to avoid blocking
        the request path.

        Args:
            engine: Engine name (e.g. ``"brave"``).
            result_count: Number of results returned.
            latency_ms: Request latency in milliseconds.
            status: Engine status classification.
            avg_score: Average result score (0.0 if no results).
        """
        if self._cache is None or not self._cache.is_connected:
            return

        key = self._daily_key(engine)
        try:
            client = self._cache._client
            if client is None:
                return

            # Use a pipeline for atomic batch increment
            pipe = client.pipeline()
            pipe.hincrby(key, "queries", 1)
            pipe.hincrby(key, "results_returned", result_count)
            pipe.hincrby(key, "errors", 1 if status in (EngineStatus.ERROR, EngineStatus.TIMEOUT) else 0)
            pipe.hincrby(key, "rate_limited", 1 if status == EngineStatus.RATE_LIMITED else 0)
            pipe.hincrby(key, "total_latency_ms", int(latency_ms))
            pipe.hincrby(key, "total_score", int(avg_score * 1000))
            # Set TTL to 90 days so old keys auto-expire
            pipe.expire(key, 7_776_000)  # 90 days
            await pipe.execute()
        except Exception as exc:
            logger.debug("Stats write error for %s: %s", engine, exc)
