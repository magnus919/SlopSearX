"""FRED adapter — Federal Reserve Economic Data search.

Free API. Requires ENGINE_FRED_API_KEY env var (free tier available at fred.stlouisfed.org).
Docs: https://fred.stlouisfed.org/docs/api/fred/

NOTE: FRED API only supports query-param authentication (api_key=XXX).
The api_key query param is unavoidable for upstream requests, but error
messages are sanitized to prevent key leakage.
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
class FredAdapter(EngineAdapter):
    """Federal Reserve Economic Data search."""

    name = "fred"
    display_name = "FRED"
    env_prefix = "ENGINE_FRED"
    engine_type = "api"
    categories = ["finance", "reference", "economics"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://api.stlouisfed.org/fred/series/search")
        api_key = cfg.get("api_key") or cfg.get("ENGINE_FRED_API_KEY", "")
        if not api_key:
            from os import environ

            api_key = environ.get("ENGINE_FRED_API_KEY", "")
        api_key = (api_key or "").strip()
        if not api_key:
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message="FRED API key not configured. Set ENGINE_FRED_API_KEY env var.",
            )

        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    base_url,
                    params={
                        "api_key": api_key,
                        "file_type": "json",
                        "search_text": query,
                        "limit": max_results,
                        "order_by": "popularity",
                        "sort_order": "desc",
                    },
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()
                data = resp.json()

                results = []
                series = data.get("seriess", [])
                for idx, s in enumerate(series[:max_results]):
                    series_id = s.get("id", "")
                    title = s.get("title", "")
                    observation_start = s.get("observation_start", "")
                    units = s.get("units", "")
                    frequency = s.get("frequency", "")
                    seasonal_adjustment = s.get("seasonal_adjustment", "")
                    popularity = s.get("popularity", 0) or 0
                    notes = (s.get("notes", "") or "")[:200]

                    content_parts = []
                    if frequency:
                        content_parts.append(frequency)
                    if units:
                        content_parts.append(units)
                    if seasonal_adjustment:
                        content_parts.append(seasonal_adjustment)
                    if notes:
                        content_parts.append(notes)
                    content = " — ".join(content_parts) if content_parts else "Economic data series"

                    results.append(
                        SearchResult(
                            url=f"https://fred.stlouisfed.org/series/{series_id}",
                            title=f"{title} ({series_id})",
                            content=content[:500],
                            engine=self.name,
                            position=idx + 1,
                            published_date=observation_start if observation_start else None,
                            score=float(popularity),
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
