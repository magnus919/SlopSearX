"""MusicBrainz adapter — music metadata search (artists, releases, recordings).

Free, public JSON API. No auth required, usage subject to rate limits.
Docs: https://musicbrainz.org/doc/Development/XML_Web_Service/Rate_Limiting
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
)


@register_engine
class MusicBrainzAdapter(EngineAdapter):
    """MusicBrainz music metadata search."""

    name = "musicbrainz"
    display_name = "MusicBrainz"
    env_prefix = "ENGINE_MUSICBRAINZ"
    engine_type = "api"
    categories = ["general", "music", "reference"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://musicbrainz.org/ws/2")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        headers = {
            "User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)",
            "Accept": "application/json",
        }
        start_time = time.monotonic()

        try:
            # Try artist search first
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    f"{base_url}/artist/",
                    params={"query": query, "limit": max_results, "fmt": "json"},
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()
                data = resp.json()

                results = []
                artists = data.get("artists", [])
                for idx, artist in enumerate(artists[:max_results]):
                    name = artist.get("name", "")
                    mbid = artist.get("id", "")
                    type_ = artist.get("type", "")
                    country = artist.get("country", "")
                    lifespan = artist.get("life-span", {}) or {}
                    begin = lifespan.get("begin", "")

                    content_parts = []
                    if type_:
                        content_parts.append(type_)
                    if country:
                        content_parts.append(country)
                    if begin:
                        content_parts.append(f"Active since {begin}")
                    content = " — ".join(content_parts) if content_parts else "Music artist"

                    results.append(
                        SearchResult(
                            url=f"https://musicbrainz.org/artist/{mbid}",
                            title=name,
                            content=content[:500],
                            engine=self.name,
                            position=idx + 1,
                            score=1.0,
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
                error_message=str(exc),
                latency_ms=latency,
            )
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
                latency_ms=latency,
            )
