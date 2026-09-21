"""Reddit adapter — Reddit Data API search.

Reddit may allow anonymous JSON access from some networks, but its current
access guidance requires a valid OAuth token (or a logged-in session) when
requests originate from hosted-service IP ranges.  This adapter never falls
back to browser automation or scraping: an optional OAuth bearer token is
used when configured, and an upstream block is reported honestly otherwise.

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

    # -- Declared capability metadata (audited, issue 185) --
    supported_result_types = ("text", "media")
    failure_classes = ("rate_limited", "blocked", "error", "timeout", "unavailable")
    cost_class = "free"
    _DEFAULT_BASE_URL = "https://www.reddit.com"

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        # ``AppContext`` passes ``dataclasses.asdict(EngineEntry)`` into the
        # adapter.  An operator who configures only ``api_key`` therefore
        # supplies ``base_url=""`` explicitly, which must mean "use the
        # built-in default" rather than producing a relative request URL.
        base_url = str(cfg.get("base_url") or self._DEFAULT_BASE_URL).rstrip("/")
        token = str(cfg.get("access_token") or cfg.get("api_key") or "").strip()
        # Reddit's documented API host for OAuth-authenticated requests is
        # oauth.reddit.com. Preserve an explicitly configured test/operator
        # endpoint, but route the built-in public default through OAuth when a
        # token is present.
        if token and base_url == self._DEFAULT_BASE_URL:
            base_url = "https://oauth.reddit.com"
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)
        search_params = params or {}

        # Sub-category routing
        categories = search_params.get("categories", [])
        subreddit = search_params.get("subreddit", "all")

        if "reddit:subreddit" in categories:
            endpoint = f"{base_url}/r/{subreddit}/search"
        else:
            endpoint = f"{base_url}/search"

        # Keep the legacy public JSON extension for anonymous requests. The
        # OAuth endpoint uses the documented API path and raw_json explicitly.
        if not token:
            endpoint += ".json"

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
        if token:
            headers["Authorization"] = f"Bearer {token}"

        start_time = time.monotonic()
        try:
            async with self.http_client(timeout=timeout_ms / 1000.0) as client:
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
                    if token:
                        message = (
                            "Reddit blocked the authenticated request; verify OAuth approval, scope, and access policy"
                        )
                    else:
                        message = (
                            "Reddit blocked unauthenticated API access; set ENGINE_REDDIT_API_KEY to an approved "
                            "OAuth access token for hosted/provider IPs"
                        )
                    return AdapterResponse(
                        results=[],
                        status=EngineStatus.BLOCKED,
                        latency_ms=latency,
                        error_message=message,
                    )
                if resp.status_code == 401:
                    return AdapterResponse(
                        results=[],
                        status=EngineStatus.UNAVAILABLE,
                        latency_ms=latency,
                        error_message="Reddit rejected the OAuth access token; configure a valid approved token",
                    )
                resp.raise_for_status()

                try:
                    data = resp.json()
                except ValueError:
                    return AdapterResponse(
                        results=[],
                        status=EngineStatus.ERROR,
                        latency_ms=latency,
                        error_message="Reddit returned invalid JSON",
                    )

                if not isinstance(data, dict) or not isinstance(data.get("data"), dict):
                    return AdapterResponse(
                        results=[],
                        status=EngineStatus.ERROR,
                        latency_ms=latency,
                        error_message="Reddit returned an invalid listing shape",
                    )
                children = data["data"].get("children")
                if not isinstance(children, list):
                    return AdapterResponse(
                        results=[],
                        status=EngineStatus.ERROR,
                        latency_ms=latency,
                        error_message="Reddit returned an invalid listing children field",
                    )
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
                published = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(int(created_utc)))

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
