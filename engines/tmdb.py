"""TMDB adapter — movie, TV show, and person search.

Free API. Requires ENGINE_TMDB_API_KEY env var (free tier available).
Docs: https://developers.themoviedb.org/3/search
"""

from __future__ import annotations

import time
from typing import Any

import httpx

from slopsearx.adapter import (
    AdapterResponse,
    EngineAdapter,
    EngineStatus,
    SearchResult,
    register_engine,
    sanitize_url,
)


@register_engine
class TMDBAdapter(EngineAdapter):
    """TMDB movie and TV search."""

    name = "tmdb"
    display_name = "TMDB"
    env_prefix = "ENGINE_TMDB"
    engine_type = "api"
    categories = ["general", "movies", "entertainment"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://api.themoviedb.org/3/search/multi")
        api_key = cfg.get("api_key") or cfg.get("ENGINE_TMDB_API_KEY", "")
        if not api_key:
            from os import environ

            api_key = environ.get("ENGINE_TMDB_API_KEY", "")
        api_key = (api_key or "").strip()
        if not api_key:
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message="TMDB API key not configured. Set ENGINE_TMDB_API_KEY env var.",
            )

        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    base_url,
                    params={"query": query, "page": 1},
                    headers={"Authorization": f"Bearer {api_key}"},
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()
                data = resp.json()

                results = []
                for idx, item in enumerate(data.get("results", [])[:max_results]):
                    media_type = item.get("media_type", "")

                    if media_type == "movie":
                        title = item.get("title", "")
                        date = item.get("release_date", "")
                        _type = "Movie"
                    elif media_type == "tv":
                        title = item.get("name", "")
                        date = item.get("first_air_date", "")
                        _type = "TV"
                    else:
                        title = item.get("name", "") or item.get("title", "")
                        date = ""
                        _type = media_type.capitalize() if media_type else "Content"

                    overview = item.get("overview", "") or ""
                    vote = item.get("vote_average", 0) or 0
                    poster = item.get("poster_path")

                    content = f"[{_type}] "
                    if date:
                        content += f"({date[:4]}) "
                    if overview:
                        content += overview[:200]

                    thumbnail = None
                    if poster:
                        thumbnail = f"https://image.tmdb.org/t/p/w200{poster}"

                    results.append(
                        SearchResult(
                            url=f"https://www.themoviedb.org/{media_type}/{item.get('id', '')}" if media_type else "",
                            title=title,
                            content=content[:500],
                            engine=self.name,
                            position=idx + 1,
                            score=float(vote) if vote else 0.0,
                            published_date=date if date else None,
                            thumbnail=thumbnail,
                        ),
                    )

                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except httpx.HTTPStatusError as exc:
            latency = (time.monotonic() - start_time) * 1000
            if exc.response.status_code == 429:
                return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=sanitize_url(str(exc)),
                latency_ms=latency,
            )
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=sanitize_url(str(exc)),
                latency_ms=latency,
            )
