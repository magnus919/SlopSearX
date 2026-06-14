"""
SlopSearX — Durable Query Audit Trail.

Records every search query and its dispatch results in a Valkey stream
for operational analysis, debugging, and capacity planning.

Schema::

    query_audit:{YYYY-MM-DD} → Stream
      Field                        Value
      query                        "search query text"
      client_ip                    "10.0.0.1"
      timestamp                    "2026-06-14T12:34:56.789Z"
      engines                      "brave,wikipedia,duckduckgo"
      engines_ok                   2
      engines_error                1
      engines_timeout              0
      total_results                15
      latency_ms                   1234

Each daily stream has a 90-day TTL (7_776_000 seconds) and is capped
at ~10,000 entries to bound memory usage.

Graceful degradation: Valkey unavailability is non-fatal — the call
is a no-op.
"""

from __future__ import annotations

import logging
import time
from typing import Any

from slopsearx.adapter import EngineStatus

logger = logging.getLogger(__name__)

_AUDIT_STREAM_MAXLEN = 10_000
_AUDIT_TTL = 7_776_000  # 90 days, matching engine_stats TTL


class QueryAuditLogger:
    """Fire-and-forget query audit trail writer.

    Each call to ``record_query`` writes a field-value pair to a Valkey
    stream keyed by date (``query_audit:{YYYY-MM-DD}``). The stream is
    capped at ``MAXLEN ~10000`` and auto-expires after 90 days.

    Graceful degradation: Valkey failure is a no-op.
    """

    def __init__(self, cache: Any = None) -> None:
        self._cache = cache

    def _stream_key(self) -> str:
        """Build the daily stream key for audit entries.

        Example: ``query_audit:2026-06-14``
        """
        today = time.strftime("%Y-%m-%d")
        return f"query_audit:{today}"

    async def record_query(
        self,
        query: str,
        client_ip: str,
        engine_results: dict[str, Any],
        latency_ms: float,
    ) -> None:
        """Record a search query in the audit trail.

        This is an async fire-and-forget call. The actual Valkey
        stream write happens via an awaited call to avoid blocking
        the request path.

        Args:
            query: The raw search query string.
            client_ip: The requesting client's IP address.
            engine_results: Dict mapping engine name to AdapterResponse-like
                objects. Each object must have ``status`` (EngineStatus),
                ``results`` (list), and ``latency_ms`` (float).
            latency_ms: Total request latency in milliseconds.
        """
        if self._cache is None or not self._cache.is_connected:
            return

        client = getattr(self._cache, "_client", None)
        if client is None:
            return

        # Derive aggregated dispatch statistics from engine results
        engine_names: list[str] = []
        engines_ok = 0
        engines_error = 0
        engines_timeout = 0
        total_results = 0
        for name, resp in engine_results.items():
            engine_names.append(name)
            if isinstance(resp, dict):
                result_list = resp.get("results", [])
                status = resp.get("status")
            else:
                result_list = resp.results
                status = resp.status
            total_results += len(result_list) if result_list else 0
            if status == EngineStatus.OK:
                engines_ok += 1
            elif status == EngineStatus.TIMEOUT:
                engines_timeout += 1
            elif status in (EngineStatus.ERROR, EngineStatus.BLOCKED, EngineStatus.RATE_LIMITED):
                engines_error += 1

        fields: dict[str, str] = {
            "query": query,
            "client_ip": client_ip,
            "timestamp": _iso_timestamp(),
            "engines": ",".join(engine_names),
            "engines_ok": str(engines_ok),
            "engines_error": str(engines_error),
            "engines_timeout": str(engines_timeout),
            "total_results": str(total_results),
            "latency_ms": str(round(latency_ms, 1)),
        }

        stream_key = self._stream_key()

        try:
            # XADD stream_key MAXLEN ~10000 * field1 val1 field2 val2 ...
            pipe = client.pipeline()
            pipe.execute_command(
                "XADD",
                stream_key,
                "MAXLEN",
                "~",
                str(_AUDIT_STREAM_MAXLEN),
                "*",
                *[item for pair in fields.items() for item in pair],
            )
            pipe.expire(stream_key, _AUDIT_TTL)
            await pipe.execute()
        except Exception as exc:
            logger.debug("Audit write error: %s", exc)


def _iso_timestamp() -> str:
    """Return an ISO 8601 timestamp string with milliseconds."""
    t = time.time()
    secs = int(t)
    millis = int((t - secs) * 1000)
    # Generate ISO-ish format without external dependencies
    struct = time.gmtime(secs)
    return "{:04d}-{:02d}-{:02d}T{:02d}:{:02d}:{:02d}.{:03d}Z".format(
        struct.tm_year,
        struct.tm_mon,
        struct.tm_mday,
        struct.tm_hour,
        struct.tm_min,
        struct.tm_sec,
        millis,
    )
