"""Valkey-backed response cache.

Cache key: search:{sha256(normalized_query + language + safesearch)}
Default TTL: 300s. Graceful degradation: Valkey unavailable -> skip cache.
"""

from __future__ import annotations

import hashlib
import json
import logging
import os
from typing import Any

logger = logging.getLogger(__name__)


def cache_key(query: str, language: str = "en", safesearch: int = 0) -> str:
    """Build deterministic cache key from normalized query tuple."""
    norm_query = query.lower().strip()
    norm = "{}|{}|{}".format(norm_query, language, safesearch)
    digest = hashlib.sha256(norm.encode()).hexdigest()
    return "search:{}".format(digest)


def _ttl_for_query(categories: list[str] | None = None) -> int:
    """Determine TTL based on query category.

    News/time-sensitive queries get shorter TTL (60s).
    General queries get default TTL (300s).
    """
    if categories and any("news" in c.lower() for c in categories):
        return 60
    return 300


class SearchCache:
    """Valkey-backed response cache for merged search results.

    Stores serialized JSON result sets. Gracefully degrades if
    Valkey is unreachable: cache operations are no-ops on failure.
    """

    def __init__(self, valkey_url: str = "") -> None:
        self._url = valkey_url or os.environ.get("VALKEY_URL", "")
        self._client: Any = None
        self._connected = False
        if self._url:
            self._connect()

    def _connect(self) -> None:
        """Establish Valkey connection if not already connected."""
        if self._client is not None:
            return
        try:
            import valkey
            self._client = valkey.Valkey.from_url(self._url)
            self._client.ping()
            self._connected = True
            logger.info("SearchCache connected to Valkey")
        except Exception as e:
            self._connected = False
            self._client = None
            logger.warning("SearchCache: Valkey unavailable, caching disabled: %s", e)

    async def get(self, key: str) -> dict | None:
        """Retrieve cached result set by key. Returns None on miss or error."""
        if not self._connected or self._client is None:
            return None
        try:
            data = self._client.get(key)
            if data is None:
                return None
            return json.loads(data)
        except Exception as e:
            logger.debug("Cache get error: %s", e)
            return None

    async def set(self, key: str, value: dict, ttl: int = 300) -> None:
        """Store result set in cache with TTL."""
        if not self._connected or self._client is None:
            return
        try:
            serialized = json.dumps(value, default=str)
            self._client.setex(key, ttl, serialized)
        except Exception as e:
            logger.debug("Cache set error: %s", e)

    async def clear(self) -> None:
        """Clear all cached entries (admin/debug)."""
        if not self._connected or self._client is None:
            return
        try:
            self._client.flushdb()
        except Exception as e:
            logger.debug("Cache clear error: %s", e)

    @property
    def is_connected(self) -> bool:
        """Whether the cache is currently connected to Valkey."""
        return self._connected
