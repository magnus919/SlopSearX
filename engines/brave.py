"""Brave Search API adapter."""

from __future__ import annotations

import time

import httpx

from slopsearx.adapter import (
    AdapterResponse,
    EngineAdapter,
    EngineStatus,
    SearchResult,
    register_engine,
)


@register_engine
class BraveAdapter(EngineAdapter):
    name = "brave"
    display_name = "Brave Search API"
    env_prefix = "ENGINE_BRAVE"
    engine_type = "api"

    async def search(
        self,
        query: str,
        params: dict | None = None,
    ) -> AdapterResponse:
        cfg = self.config
        api_key = cfg.get("api_key") or ""
        base_url = cfg.get("base_url", "https://api.search.brave.com/res/v1/web/search")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        if not api_key:
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message="Brave API key not configured",
            )

        headers = {
            "Accept": "application/json",
            "Accept-Encoding": "gzip",
            "X-Subscription-Token": api_key,
        }

        params_dict: dict = {
            "q": query,
            "count": max_results,
            "safesearch": "off",
        }

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(base_url, headers=headers, params=params_dict)
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                if resp.status_code == 403:
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                resp.raise_for_status()

                data = resp.json()
                web_results = (data.get("web", {}) if isinstance(data.get("web"), dict) else {}).get("results", [])
                results = self._parse_results(web_results, query)
                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
                latency_ms=latency,
            )

    def _parse_results(self, raw: list[dict], query: str) -> list[SearchResult]:
        results: list[SearchResult] = []
        for i, item in enumerate(raw):
            results.append(
                SearchResult(
                    url=item.get("url", ""),
                    title=item.get("title", ""),
                    content=item.get("description", ""),
                    engine=self.name,
                    position=i + 1,
                    thumbnail=item.get("thumbnail", {}).get("src") if isinstance(item.get("thumbnail"), dict) else None,
                ),
            )
        return results
