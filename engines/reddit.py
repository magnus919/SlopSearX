"""Reddit adapter — public JSON search API.

Free, public JSON API. No auth or API key required.
Rate limit: ~60 req/min per IP. Respect Retry-After header.

Sub-category routing:
- ``reddit:subreddit`` → scoped to a specific subreddit.
  The subreddit name is passed via ``params["subreddit"]``
  (or defaults to "all").

Search results are filtered to exclude NSFW (over_18) items.
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
class RedditAdapter(EngineAdapter):
    name = "reddit"
    display_name = "Reddit"
    env_prefix = "ENGINE_REDDIT"
    engine_type = "api"
    categories = ["general", "social", "reddit:subreddit"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        cfg = self.config
        base_url = cfg.get("base_url", "https://www.reddit.com")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)
        search_params = params or {}

        # Sub-category routing
        categories = search_params.get("categories", [])
        subreddit = search_params.get("subreddit", "all")

        if "reddit:subreddit" in categories:
            endpoint = f"{base_url}/r/{subreddit}/search.json"
        else:
            endpoint = f"{base_url}/search.json"

        query_params: dict[str, str | int] = {
            "q": query,
            "limit": max_results,
            "sort": "relevance",
            "t": "all",
            "raw_json": 1,
        }

        headers = {
            "User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native; by /u/SlopSearX)",
            "Accept": "application/json",
        }

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(endpoint, params=query_params, headers=headers)
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(
                        results=[],
                        status=EngineStatus.RATE_LIMITED,
                        latency_ms=latency,
                        error_message="rate limited by Reddit",
                    )
                if resp.status_code == 403:
                    return AdapterResponse(
                        results=[],
                        status=EngineStatus.BLOCKED,
                        latency_ms=latency,
                        error_message="blocked by Reddit",
                    )
                resp.raise_for_status()

                data = resp.json()
                children = data.get("data", {}).get("children", [])
                results = self._parse_listing(children)
                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except Exception as exc:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
                latency_ms=latency,
            )

    def _parse_listing(self, children: list[dict[str, Any]]) -> list[SearchResult]:
        """Parse a Reddit JSON listing into SearchResult list.

        Filters out NSFW (over_18) items and non-link posts
        (self posts with no external URL are included with their
        reddit permalink).
        """
        results: list[SearchResult] = []

        for item in children:
            data = item.get("data", {})
            if not isinstance(data, dict):
                continue

            # Skip NSFW content
            if data.get("over_18", False):
                continue

            title = data.get("title", "")
            if not title:
                continue

            # Build URL — use external link if available, else permalink
            url = data.get("url", "")
            permalink = data.get("permalink", "")
            if not url or url.startswith(permalink) or not url.startswith("http"):
                url = f"https://www.reddit.com{permalink}" if permalink else ""

            # Build content snippet: selftext, or a summary
            selftext = data.get("selftext", "")
            content = selftext[:500] if selftext else ""

            # Score metadata
            score = data.get("score", 0)
            num_comments = data.get("num_comments", 0)
            author = data.get("author", "[deleted]")
            subreddit = data.get("subreddit", "")
            created_utc = data.get("created_utc", 0)

            if not content:
                content = f"Score: {score} | Comments: {num_comments} | Author: {author}"

            # ISO 8601 from epoch
            published = None
            if created_utc:
                published = time.strftime(
                    "%Y-%m-%dT%H:%M:%SZ", time.gmtime(int(created_utc))
                )

            # Thumbnail
            thumbnail = data.get("thumbnail", "")
            if not thumbnail or thumbnail in ("self", "default", "nsfw", ""):
                thumbnail = None

            results.append(
                SearchResult(
                    url=url,
                    title=title,
                    content=content,
                    engine=self.name,
                    score=float(score),
                    position=len(results) + 1,
                    category=f"social:{subreddit}" if subreddit else "social",
                    published_date=published,
                    thumbnail=thumbnail,
                ),
            )

        return results
