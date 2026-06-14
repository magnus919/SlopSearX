"""Valkey-backed response cache.

Cache key: search:{sha256(normalized_query + language + safesearch)}
Default TTL: 3600s for general queries, 300s for news. Graceful degradation:
Valkey unavailable -> skip cache.
"""

from __future__ import annotations

import hashlib
import json
import logging
import os
import re
import time
from typing import Any, cast
from urllib.parse import unquote_plus

logger = logging.getLogger(__name__)


def normalize_query(query: str) -> str:
    """Normalize a search query for deterministic cache key construction.

    Steps:
        1. URL-decode the query (unquote_plus)
        2. Strip leading/trailing whitespace
        3. Strip trailing punctuation: . ! ? , ; :
        4. Collapse multiple whitespace characters into one
        5. Lower-case and strip leading/trailing whitespace (final safeguard)
    """
    norm = unquote_plus(query)
    norm = norm.strip()
    norm = norm.rstrip(".!?,;:")
    norm = re.sub(r"\s+", " ", norm)
    return norm.lower().strip()


def cache_key(query: str, language: str = "en", safesearch: int = 0) -> str:
    """Build deterministic cache key from normalized query tuple."""
    norm_query = normalize_query(query)
    norm = "{}|{}|{}".format(norm_query, language, safesearch)
    digest = hashlib.sha256(norm.encode()).hexdigest()
    return "search:{}".format(digest)


def _answer_cache_key(query: str) -> str:
    """Build answer-level cache key from normalized query (no language/safesearch)."""
    norm_query = normalize_query(query)
    digest = hashlib.sha256(norm_query.encode()).hexdigest()
    return "answer:{}".format(digest)


def _ttl_for_query(categories: list[str] | None = None) -> int:
    """Determine TTL based on query category.

    News/time-sensitive queries get shorter TTL (300s).
    General queries get default TTL (3600s).
    """
    if categories and any("news" in c.lower() for c in categories):
        return 300
    return 3600


class SearchCache:
    """Valkey-backed response cache for merged search results.

    Stores serialized JSON result sets. Gracefully degrades if
    Valkey is unreachable: cache operations are no-ops on failure.
    """

    def __init__(self, valkey_url: str = "") -> None:
        self._url = valkey_url or os.environ.get("VALKEY_URL", "")
        self._client: Any = None
        self._connected = False
        self._default_ttl = int(os.environ.get("SEARCH_CACHE_TTL_SECONDS", "3600"))
        self._negative_ttl = int(os.environ.get("SEARCH_CACHE_NEGATIVE_TTL_SECONDS", "60"))
        self._answer_ttl = int(os.environ.get("SEARCH_CACHE_TTL_SECONDS", "3600"))

    async def connect(self) -> None:
        """Establish async Valkey connection."""
        if self._client is not None:
            return
        if not self._url:
            return
        try:
            import valkey.asyncio

            self._client = valkey.asyncio.Valkey.from_url(self._url)
            await self._client.ping()
            self._connected = True
            logger.info("SearchCache connected to Valkey")
        except Exception as e:
            self._connected = False
            self._client = None
            logger.warning("SearchCache: Valkey unavailable, caching disabled: %s", e)

    async def close(self) -> None:
        """Close the Valkey connection."""
        if self._client is not None:
            try:
                await self._client.close()
            except Exception:
                pass
            self._client = None
            self._connected = False

    async def get(self, key: str) -> dict[str, Any] | None:
        """Retrieve cached result set by key. Returns None on miss or error."""
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
        if not self._connected or self._client is None:
            return
        try:
            serialized = json.dumps(value, default=str)
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
