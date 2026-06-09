"""Wikipedia API adapter."""

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
class WikipediaAdapter(EngineAdapter):
    name = "wikipedia"
    display_name = "Wikipedia"
    env_prefix = "ENGINE_WIKIPEDIA"
    engine_type = "api"

    async def search(
        self,
        query: str,
        params: dict | None = None,
    ) -> AdapterResponse:
        cfg = self.config
        base_url = cfg.get("base_url", "https://en.wikipedia.org/w/api.php")
        timeout_ms = cfg.get("timeout_ms", 3_000)
        max_results = cfg.get("max_results", 3)

        api_params = {
            "action": "query",
            "list": "search",
            "srsearch": query,
            "srlimit": max_results,
            "format": "json",
            "origin": "*",
        }

        headers = {"User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)"}

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(base_url, params=api_params, headers=headers)
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                resp.raise_for_status()

                data = resp.json()
                search_results = data.get("query", {}).get("search", [])
                results = self._parse_results(search_results, query)
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
            page_title = item.get("title", "")
            page_url = f"https://en.wikipedia.org/wiki/{page_title.replace(' ', '_')}"
            # Strip HTML tags from snippet
            import re as _re

            snippet = _re.sub(r"<.*?>", "", item.get("snippet", ""))

            results.append(
                SearchResult(
                    url=page_url,
                    title=page_title,
                    content=snippet,
                    engine=self.name,
                    position=i + 1,
                ),
            )
        return results
