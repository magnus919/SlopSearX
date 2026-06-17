"""Hacker News adapter — Algolia-powered story/comment search."""

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
class HackerNewsAdapter(EngineAdapter):
    name = "hackernews"
    display_name = "Hacker News"
    env_prefix = "ENGINE_HACKERNEWS"
    engine_type = "api"
    categories = ["general", "news"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://hn.algolia.com/api/v1/search")
        timeout_ms = cfg.get("timeout_ms", 3_000)
        max_results = cfg.get("max_results", 5)

        params_dict: dict[str, Any] = {
            "query": query,
            "hitsPerPage": max_results,
            "tags": "story",  # Only stories, not comments
        }

        headers = {
            "User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)",
        }

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(base_url, params=params_dict, headers=headers)
                latency = (time.monotonic() - start_time) * 1000

                resp.raise_for_status()
                data = resp.json()
                hits = data.get("hits", [])
                results = self._parse_hits(hits)
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

    def _parse_hits(self, hits: list[dict[str, Any]]) -> list[SearchResult]:
        """Parse HN Algolia hits into SearchResult list."""
        results: list[SearchResult] = []

        for idx, hit in enumerate(hits):
            title = hit.get("title", "")
            object_id = hit.get("objectID", "")
            points = hit.get("points", 0)
            author = hit.get("author", "")
            created_at = hit.get("created_at", "")
            url = hit.get("url") or f"https://news.ycombinator.com/item?id={object_id}"
            comment_count = hit.get("num_comments", 0)

            content = f"▲ {points}  by {author}"
            if comment_count:
                content += f"  |  {comment_count} comments"

            results.append(
                SearchResult(
                    url=url,
                    title=title,
                    content=content,
                    engine=self.name,
                    position=idx + 1,
                    score=float(points),
                    published_date=created_at,
                    category="discussion",
                ),
            )

        return results
