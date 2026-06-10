"""Wikipedia API adapter — two-stage opensearch → query pipeline."""

from __future__ import annotations

import re
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
class WikipediaAdapter(EngineAdapter):
    name = "wikipedia"
    display_name = "Wikipedia"
    env_prefix = "ENGINE_WIKIPEDIA"
    engine_type = "api"
    categories = ["general", "science", "reference"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if (early := await self._check_rate_limit()):
            return early

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

                # Check for "Did you mean" corrections from opensearch
                corrections = self._check_corrections(query, titles)

                # Stage 2: query extracts, page images, and pageprops for each title
                results, infoboxes = await self._rich_query(client, base_url, titles, headers)
                latency = (time.monotonic() - start_time) * 1000
                return AdapterResponse(
                    results=results,
                    status=EngineStatus.OK,
                    latency_ms=latency,
                    corrections=corrections,
                    infoboxes=infoboxes,
                )

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
        headers: dict[str, str],
    ) -> list[str]:
        """Stage 1: fetch quick title suggestions via opensearch."""
        params: dict[str, str | int] = {
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
        headers: dict[str, str],
    ) -> tuple[list[SearchResult], list[dict[str, Any]]]:
        """Stage 2: fetch extracts, thumbnails, and pageprops for resolved titles.

        Returns:
            Tuple of (search_results, infoboxes).
        """
        if not titles:
            return [], []

        params: dict[str, str] = {
            "action": "query",
            "titles": "|".join(titles),
            "prop": "extracts|pageimages|pageprops",
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
        infoboxes: list[dict[str, Any]] = []
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

            # Build infobox from pageprops
            pageprops = page.get("pageprops") or {}
            if isinstance(pageprops, dict) and pageprops.get("wikibase-shortdesc"):
                infoboxes.append({
                    "id": f"wiki:{title.replace(' ', '_')}",
                    "title": title,
                    "content": pageprops.get("wikibase-shortdesc", ""),
                    "img_src": thumbnail or "",
                    "url": page_url,
                    "urls": [{"title": "Wikipedia", "url": page_url}],
                })

        return results, infoboxes


    @staticmethod
    def _check_corrections(query: str, titles: list[str]) -> list[str]:
        """Detect redirect-based corrections from opensearch.

        If the first returned title differs from the query, surface
        it as a "Did you mean" correction.
        """
        if not titles:
            return []
        first = titles[0].lower().strip()
        q = query.lower().strip()
        if first != q and q not in first and first not in q:
            return [titles[0]]
        return []
