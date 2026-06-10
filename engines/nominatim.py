"""Nominatim adapter — OpenStreetMap geocoding search.

Free, public API. No auth required, but usage policy applies (max 1 req/sec).
Docs: https://nominatim.org/release-docs/latest/api/Search/
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
class NominatimAdapter(EngineAdapter):
    """OpenStreetMap geocoding search via Nominatim."""

    name = "nominatim"
    display_name = "Nominatim (OSM)"
    env_prefix = "ENGINE_NOMINATIM"
    engine_type = "api"
    categories = ["general", "geography", "reference"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://nominatim.openstreetmap.org/search")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        headers = {
            "User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)",
            "Accept": "application/json",
        }
        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    base_url,
                    params={
                        "q": query,
                        "format": "json",
                        "limit": max_results,
                        "addressdetails": "1",
                    },
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()
                data = resp.json()

                results = []
                for idx, place in enumerate(data[:max_results]):
                    display_name = place.get("display_name", "")
                    place_type = place.get("type", "")
                    category = place.get("category", "")
                    osm_type = place.get("osm_type", "")
                    osm_id = place.get("osm_id", "")
                    importance = place.get("importance", 0) or 0

                    content_parts = [display_name]
                    if category or place_type:
                        content_parts.append(f"[{category}/{place_type}]")
                    content = " — ".join(content_parts) if content_parts else "Location"

                    pid = place.get("place_id", "")
                    result_url = (
                        f"https://www.openstreetmap.org/{osm_type}/{osm_id}"
                        if osm_type and osm_id
                        else f"https://nominatim.openstreetmap.org/details.php?place_id={pid}"
                    )
                    results.append(
                        SearchResult(
                            url=result_url,
                            title=display_name[:100] + ("..." if len(display_name) > 100 else ""),
                            content=content[:500],
                            engine=self.name,
                            position=idx + 1,
                            score=float(importance) if importance else 0.5,
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
