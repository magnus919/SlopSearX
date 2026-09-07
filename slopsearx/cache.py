"""Valkey-backed response cache.

Cache key: search:v2:{sha256(JSON query and scope tuple)}
Default TTL: 3600s for general queries, 300s for news. Graceful degradation:
Valkey unavailable -> skip cache.
"""

from __future__ import annotations

import asyncio
import datetime as _dt
import hashlib
import json
import logging
import os
import time
from typing import Any, cast

from slopsearx.filters import time_range_window

logger = logging.getLogger(__name__)


def normalize_query(query: str) -> str:
    """Normalize a search query for deterministic cache key construction.

    The transport already decoded the query. Preserve punctuation and literal
    percent escapes, which can distinguish programming languages and operators.
    """
    return query.strip()


def cache_key(
    query: str,
    language: str = "en",
    safesearch: int = 0,
    *,
    categories: list[str] | None = None,
    engines: list[str] | None = None,
    pageno: int = 1,
    time_range: str | None = None,
    media_type: str | None = None,
    date_from: str | None = None,
    date_to: str | None = None,
    time_range_anchor: _dt.date | None = None,
) -> str:
    """Build deterministic cache key from normalized query tuple.

    The key includes every result-affecting input. Categories, engines,
    page, time range, and media type were previously excluded, which could
    serve a cached response that did not represent the requested filters —
    two semantically different searches could share one entry. Scope inputs
    are sorted so equivalent requests produce identical keys.
    """
    norm_query = normalize_query(query)
    norm = json.dumps(
        [
            norm_query,
            language,
            safesearch,
            sorted(categories or []),
            sorted(engines or []),
            pageno,
            time_range,
            media_type,
            date_from,
            date_to,
            [bound.isoformat() for bound in time_range_window(time_range, now=time_range_anchor) or ()]
            if time_range
            else None,
        ],
        ensure_ascii=False,
        separators=(",", ":"),
    )
    digest = hashlib.sha256(norm.encode()).hexdigest()
    return f"search:v2:{digest}"


def _answer_cache_key(query: str) -> str:
    """Build answer-level cache key from normalized query (no language/safesearch)."""
    norm_query = normalize_query(query)
    digest = hashlib.sha256(norm_query.encode()).hexdigest()
    return "answer:v2:{}".format(digest)


def _cache_ttl_setting(name: str, default: int, maximum: int | None = None) -> int:
    """Read a positive whole-second lifetime, rejecting unsafe configuration."""
    try:
        value = int(os.environ.get(name, str(default)))
    except ValueError as exc:
        raise ValueError(f"{name} must be a positive integer in seconds") from exc
    if value <= 0 or (maximum is not None and value > maximum):
        bound = f" between 1 and {maximum}" if maximum is not None else " greater than zero"
        raise ValueError(f"{name} must be an integer{bound}")
    return value


def _ttl_for_query(categories: list[str] | None = None, *, partial: bool = False) -> int:
    """Honor the configured lifetime, with shorter news and degraded bounds."""
    ttl = _cache_ttl_setting("SEARCH_CACHE_TTL_SECONDS", 3600)
    if categories and any("news" in c.lower() for c in categories):
        ttl = min(ttl, 300)
    if partial:
        ttl = min(ttl, _cache_ttl_setting("SEARCH_CACHE_PARTIAL_TTL_SECONDS", 30, maximum=300))
    return ttl


class SearchCache:
    """Valkey-backed response cache for merged search results.

    Stores serialized JSON result sets. Gracefully degrades if
    Valkey is unreachable: cache operations are no-ops on failure.
    """

    def __init__(self, valkey_url: str = "") -> None:
        self._url = valkey_url or os.environ.get("VALKEY_URL", "")
        self._client: Any = None
        self._connected = False
        self._connect_lock = asyncio.Lock()
        self._retry_at = 0.0
        self._closed = False
        self._default_ttl = _cache_ttl_setting("SEARCH_CACHE_TTL_SECONDS", 3600)
        _cache_ttl_setting("SEARCH_CACHE_PARTIAL_TTL_SECONDS", 30, maximum=300)
        self._negative_ttl = int(os.environ.get("SEARCH_CACHE_NEGATIVE_TTL_SECONDS", "60"))
        self._answer_ttl = self._default_ttl

    async def connect(self) -> None:
        """Establish async Valkey connection."""
        if self._connected or not self._url or self._closed or time.monotonic() < self._retry_at:
            return
        async with self._connect_lock:
            if self._connected or self._closed or time.monotonic() < self._retry_at:
                return
            import valkey.asyncio

            client = None
            try:
                client = valkey.asyncio.Valkey.from_url(
                    self._url,
                    socket_connect_timeout=1,
                    socket_timeout=1,
                )
                await client.ping()
            except (Exception, asyncio.CancelledError) as exc:
                self._retry_at = time.monotonic() + 5
                if client is not None:
                    try:
                        await client.aclose()
                    except Exception:
                        pass
                if isinstance(exc, asyncio.CancelledError):
                    raise
                logger.warning("SearchCache: Valkey unavailable, retrying later: %s", exc)
                return
            self._client = client
            self._connected = True
            logger.info("SearchCache connected to Valkey")

    async def close(self) -> None:
        """Close the Valkey connection."""
        self._closed = True
        async with self._connect_lock:
            if self._client is not None:
                try:
                    await self._client.aclose()
                except Exception:
                    pass
                self._client = None
                self._connected = False

    async def get(self, key: str) -> dict[str, Any] | None:
        """Retrieve cached result set by key. Returns None on miss or error."""
        await self.connect()
        if not self._connected or self._client is None:
            return None
        try:
            data = await self._client.get(key)
            if data is None:
                return None
            return cast("dict[str, Any]", json.loads(data))
        except Exception as e:
            logger.debug("Cache get error: %s", e)
            return None

    async def set(self, key: str, value: dict[str, Any], ttl: int = 300) -> None:
        """Store result set in cache with TTL."""
        await self.connect()
        if not self._connected or self._client is None:
            return
        try:
            serialized = json.dumps(value)
            await self._client.setex(key, ttl, serialized)
        except Exception as e:
            logger.debug("Cache set error: %s", e)

    async def set_error(self, key: str, ttl: int | None = None) -> None:
        """Store a negative cache entry (cached error response).

        Negative entries signal that the previous attempt to serve
        this key failed. The caller (server.py) checks for the
        ``_error`` sentinel and returns 503 without dispatching.

        Args:
            key: The cache key to mark as errored.
            ttl: TTL in seconds. Falls back to ``self._negative_ttl``.
        """
        if not self._connected or self._client is None:
            return
        try:
            payload = json.dumps({"_error": True, "timestamp": int(time.time())})
            ttl = ttl if ttl is not None else self._negative_ttl
            await self._client.setex(key, ttl, payload)
        except Exception as e:
            logger.debug("Cache set_error error: %s", e)

    async def get_answer(self, query: str) -> dict[str, Any] | None:
        """Retrieve answer-level cached response for a query.

        Answer cache uses a broader key (query only, no language/
        safesearch), so the same response is returned for any variant
        of the same query string.

        Args:
            query: The raw search query string.

        Returns:
            Cached response dict, or ``None`` on miss / error.
        """
        key = _answer_cache_key(query)
        return await self.get(key)

    async def set_answer(self, query: str, value: dict[str, Any], ttl: int | None = None) -> None:
        """Store a response in the answer-level cache.

        Args:
            query: The raw search query string.
            value: The response dict to cache.
            ttl: TTL in seconds. Falls back to ``self._answer_ttl``.
        """
        key = _answer_cache_key(query)
        if ttl is None:
            ttl = self._answer_ttl
        await self.set(key, value, ttl)

    async def clear(self) -> None:
        """Clear all cached entries (admin/debug)."""
        if not self._connected or self._client is None:
            return
        try:
            await self._client.flushdb()
        except Exception as e:
            logger.debug("Cache clear error: %s", e)

    @property
    def is_connected(self) -> bool:
        """Whether the cache is currently connected to Valkey."""
        return self._connected
