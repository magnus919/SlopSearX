"""
SlopSearX — Search Query Suggestions.

Fetches search suggestions from engine suggest APIs and caches them
in Valkey. Primary provider: Brave Suggest API. Fallback: DuckDuckGo
suggest API. Gracefully degrades to an empty list when all providers
are unavailable.
"""

from __future__ import annotations

import hashlib
import logging
from typing import Any, cast

import httpx

logger = logging.getLogger(__name__)

_BRAVE_SUGGEST_URL = "https://api.search.brave.com/res/v1/web/suggest"
_DDG_SUGGEST_URL = "https://ac.duckduckgo.com/ac/"

_SUGGEST_CACHE_TTL = 1800  # 30 minutes — suggestions change slowly


def _suggest_cache_key(query: str) -> str:
    """Build deterministic cache key for a suggestion query."""
    norm = query.lower().strip()
    digest = hashlib.sha256(norm.encode()).hexdigest()
    return f"suggest:{digest}"


class SuggestionService:
    """Fetches search suggestions from engine suggest APIs.

    Uses Brave Suggest API as primary, DuckDuckGo as fallback.
    Results are cached in Valkey for 30 minutes.
    """

    def __init__(
        self,
        brave_api_key: str = "",
        cache: Any = None,
    ) -> None:
        self._brave_api_key = brave_api_key
        self._cache = cache

    async def fetch(self, query: str) -> list[str]:
        """Fetch suggestions for a query, using cache if available.

        Args:
            query: The search query string.

        Returns:
            List of suggestion strings, or empty list on failure.
        """
        if not query.strip():
            return []

        # Check cache
        if self._cache is not None and self._cache.is_connected:
            ck = _suggest_cache_key(query)
            cached = await self._cache.get(ck)
            if cached is not None:
                return cast("list[str]", cached.get("suggestions", []))

        # Try Brave first, then DDG
        suggestions = await self._fetch_brave(query)
        if not suggestions:
            suggestions = await self._fetch_ddg(query)

        # Cache result (even empty — avoids repeated failed lookups)
        if self._cache is not None and self._cache.is_connected:
            ck = _suggest_cache_key(query)
            await self._cache.set(ck, {"suggestions": suggestions}, ttl=_SUGGEST_CACHE_TTL)

        return suggestions

    async def _fetch_brave(self, query: str) -> list[str]:
        """Fetch suggestions from Brave Suggest API."""
        if not self._brave_api_key:
            return []

        headers = {
            "Accept": "application/json",
            "Accept-Encoding": "gzip",
            "X-Subscription-Token": self._brave_api_key,
        }

        try:
            async with httpx.AsyncClient(timeout=3.0) as client:
                resp = await client.get(
                    _BRAVE_SUGGEST_URL,
                    params={"q": query, "count": 5},
                    headers=headers,
                )
                if resp.status_code != 200:
                    return []
                data = resp.json()

                # Brave suggest returns {"results": [{"q": "..."}, ...]}
                # or a flat list of strings depending on API version
                results = data.get("results", []) if isinstance(data, dict) else data
                if isinstance(results, list):
                    return [
                        item["q"] if isinstance(item, dict) and "q" in item else str(item)
                        for item in results
                    ]
                return []

        except Exception as exc:
            logger.debug("Brave suggest error: %s", exc)
            return []

    async def _fetch_ddg(self, query: str) -> list[str]:
        """Fetch suggestions from DuckDuckGo suggest API.

        DDG suggest returns: ["query", ["sug1", "sug2", ...]]
        """
        try:
            async with httpx.AsyncClient(timeout=3.0) as client:
                resp = await client.get(
                    _DDG_SUGGEST_URL,
                    params={"q": query, "type": "list"},
                )
                if resp.status_code != 200:
                    return []
                data = resp.json()

                # DDG returns [query_string, [suggestions_list]]
                if isinstance(data, list) and len(data) >= 2 and isinstance(data[1], list):
                    return [str(s) for s in data[1] if s]

                return []

        except Exception as exc:
            logger.debug("DDG suggest error: %s", exc)
            return []
