"""Wikipedia API adapter — two-stage opensearch → query pipeline."""

from __future__ import annotations

import re
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
    categories = ["general", "science", "reference"]

    async def search(
        self,
        query: str,
        params: dict | None = None,
    ) -> AdapterResponse:
        cfg = self.config
        base_url = cfg.get("base_url", "https://en.wikipedia.org/w/api.php")
        timeout_ms = cfg.get("timeout_ms", 3_000)
        max_results = cfg.get("max_results", 3)

        headers = {"User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)"}
        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                # Stage 1: opensearch for quick title/suggestion matches
                titles = await self._opensearch(client, base_url, query, max_results, headers)
                if not titles:
                    return AdapterResponse(
                        results=[],
                        status=EngineStatus.OK,
                        latency_ms=(time.monotonic() - start_time) * 1000,
                    )

                # Stage 2: query extracts and page images for each title
                results = await self._rich_query(client, base_url, titles, headers)
                latency = (time.monotonic() - start_time) * 1000
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

    async def _opensearch(
        self,
        client: httpx.AsyncClient,
        base_url: str,
        query: str,
        limit: int,
        headers: dict,
    ) -> list[str]:
        """Stage 1: fetch quick title suggestions via opensearch."""
        params = {
            "action": "opensearch",
            "search": query,
            "limit": limit,
            "format": "json",
            "origin": "*",
        }
        resp = await client.get(base_url, params=params, headers=headers)
        # HTTP-level errors (429, 5xx) bubble up to search()'s catch-all
        resp.raise_for_status()
        data = resp.json()
        # opensearch returns [query, [titles], [urls], [snippets]]
        if len(data) >= 2 and isinstance(data[1], list):
            return [t for t in data[1] if isinstance(t, str)][:limit]
        return []

    async def _rich_query(
        self,
        client: httpx.AsyncClient,
        base_url: str,
        titles: list[str],
        headers: dict,
    ) -> list[SearchResult]:
        """Stage 2: fetch extracts and thumbnails for resolved page titles."""
        if not titles:
            return []

        params = {
            "action": "query",
            "titles": "|".join(titles),
            "prop": "extracts|pageimages",
            "exintro": "1",
            "explaintext": "1",
            "pithumbsize": "300",
            "format": "json",
            "origin": "*",
        }
        resp = await client.get(base_url, params=params, headers=headers)
        resp.raise_for_status()
        data = resp.json()

        pages = data.get("query", {}).get("pages", {})
        results: list[SearchResult] = []
        for idx, (page_id, page) in enumerate(pages.items()):
            if page_id == "-1":
                continue  # missing page
            title = page.get("title", "")
            extract = page.get("extract", "")
            thumbnail = None
            thumb_data = page.get("thumbnail")
            if isinstance(thumb_data, dict):
                thumbnail = thumb_data.get("source")

            # Clean extract
            clean = re.sub(r"\s+", " ", extract).strip()
            # Truncate to first 200 chars for snippet
            if len(clean) > 200:
                clean = clean[:200] + "…"

            page_url = f"https://en.wikipedia.org/wiki/{title.replace(' ', '_')}"
            results.append(
                SearchResult(
                    url=page_url,
                    title=title,
                    content=clean,
                    engine=self.name,
                    position=idx + 1,
                    thumbnail=thumbnail,
                ),
            )

        return results
