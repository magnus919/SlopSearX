"""Brave Search API adapter."""

from __future__ import annotations

import os
import time
from typing import Any
from urllib.parse import urlparse, urlunparse

import httpx

from slopsearx.adapter import (
    AdapterResponse,
    EngineAdapter,
    EngineStatus,
    SearchResult,
    register_engine,
)

# Maps SearXNG category → Brave endpoint path segment
_CATEGORY_ENDPOINT: dict[str, str] = {
    "images": "images",
    "news": "news",
    "videos": "videos",
    "video": "videos",
}


def _resolve_endpoint(categories: list[str]) -> str:
    """Map a list of categories to a Brave API endpoint segment.

    Returns the endpoint path segment (``"web"``, ``"images"``,
    ``"news"``, or ``"videos"``).  If no category matches a
    known Brave endpoint, falls back to ``"web"``.
    """
    for cat in categories:
        if cat in _CATEGORY_ENDPOINT:
            return _CATEGORY_ENDPOINT[cat]
    return "web"


def _build_endpoint_url(base_url: str, endpoint: str) -> str:
    """Derive a Brave API endpoint URL from the configured web search URL.

    Replaces the ``/web/`` path segment in *base_url* with
    ``/{endpoint}/``.  If *base_url* doesn't contain ``/web/``,
    constructs the URL from the API origin.
    """
    if "/web/" in base_url:
        return base_url.replace("/web/", f"/{endpoint}/")
    # Fallback: construct from the API origin
    parsed = urlparse(base_url)
    new_path = f"/res/v1/{endpoint}/search"
    return urlunparse(parsed._replace(path=new_path))


@register_engine
class BraveAdapter(EngineAdapter):
    name = "brave"
    display_name = "Brave Search API"
    env_prefix = "ENGINE_BRAVE"
    engine_type = "api"
    categories = ["general", "news", "science", "images"]

    def __init__(self, config: dict | None = None, **kwargs):
        cfg = config or {}
        # Load API key from environment if not in config
        if not cfg.get("api_key") and self.env_prefix:
            env_key = os.environ.get(f"{self.env_prefix}_API_KEY", "")
            if env_key:
                cfg["api_key"] = env_key
        super().__init__(cfg, **kwargs)

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        search_params = params or {}
        cfg = self.config
        api_key = cfg.get("api_key") or ""
        # Fallback: load API key from environment variable if not in config
        if not api_key and self.env_prefix:
            env_key = os.environ.get(f"{self.env_prefix}_API_KEY", "")
            if env_key:
                cfg["api_key"] = env_key
                api_key = env_key
        base_url = cfg.get("base_url", "https://api.search.brave.com/res/v1/web/search")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        if not api_key:
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message="Brave API key not configured",
            )

        # Resolve endpoint via category routing (feature-flagged)
        categories: list[str] = search_params.get("categories", ["general"])
        category_routing_enabled = cfg.get("_feature_brave_category_routing", True)
        if category_routing_enabled:
            endpoint = _resolve_endpoint(categories)
        else:
            endpoint = "web"
        endpoint_url = _build_endpoint_url(base_url, endpoint)

        headers = {
            "Accept": "application/json",
            "Accept-Encoding": "gzip",
            "X-Subscription-Token": api_key,
        }

        params_dict: dict[str, Any] = {
            "q": query,
            "count": max_results,
            "safesearch": "off",
        }

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(endpoint_url, headers=headers, params=params_dict)
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                if resp.status_code == 403:
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                resp.raise_for_status()

                data = resp.json()

                # Parse results based on endpoint format
                if endpoint == "web":
                    web_results = (data.get("web", {}) if isinstance(data.get("web"), dict) else {}).get("results", [])
                    results = self._parse_web_results(web_results)
                elif endpoint == "images":
                    raw = data.get("results", [])
                    results = self._parse_image_results(raw)
                else:
                    # news, videos — results at top level
                    raw = data.get("results", [])
                    results = self._parse_web_results(raw)

                # Extract answer-box content from Brave's mixed/answer sections
                # (only available from web endpoint responses)
                answers = self._parse_answers(data)

                return AdapterResponse(
                    results=results,
                    status=EngineStatus.OK,
                    latency_ms=latency,
                    answers=answers,
                )

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

    def _parse_answers(self, data: dict[str, Any]) -> list[dict[str, Any]]:
        """Extract answer-box style results from Brave response.

        Brave's ``infobox`` and ``mixed`` sections can contain
        answer-type content (summaries, entity descriptions) that
        map to the SearXNG ``answers`` field.
        """
        answers: list[dict[str, Any]] = []

        # Check infobox (entity/summary answers)
        infobox = data.get("infobox")
        if isinstance(infobox, dict):
            infobox_results = infobox.get("results", [])
            if isinstance(infobox_results, list):
                for item in infobox_results:
                    if isinstance(item, dict):
                        desc = item.get("description", "") or ""
                        if desc:
                            answers.append(
                                {
                                    "url": item.get("url", ""),
                                    "title": item.get("title", ""),
                                    "content": desc[:500],
                                }
                            )

        # Check mixed section for inline answers
        mixed = data.get("mixed")
        if isinstance(mixed, dict):
            for entry in mixed.get("main", []):
                if isinstance(entry, dict) and entry.get("type") == "answer":
                    ans_data = entry.get("data", {})
                    if isinstance(ans_data, dict):
                        desc = ans_data.get("description", "") or ""
                        if desc:
                            answers.append(
                                {
                                    "url": ans_data.get("url", ""),
                                    "title": ans_data.get("title", ""),
                                    "content": desc[:500],
                                }
                            )

        return answers

    def _parse_web_results(self, raw: list[dict[str, Any]]) -> list[SearchResult]:
        """Parse web/news/video result items into SearchResult objects.

        Used for both web endpoint (``data.web.results``) and
        news/video endpoints (``data.results``), which share the
        same item field schema.
        """
        results: list[SearchResult] = []
        for i, item in enumerate(raw):
            thumbnail = item.get("thumbnail", {})
            results.append(
                SearchResult(
                    url=item.get("url", ""),
                    title=item.get("title", ""),
                    content=item.get("description", ""),
                    engine=self.name,
                    position=i + 1,
                    thumbnail=thumbnail.get("src") if isinstance(thumbnail, dict) else None,
                ),
            )
        return results

    def _parse_image_results(self, raw: list[dict[str, Any]]) -> list[SearchResult]:
        """Parse image result items into SearchResult objects.

        Image items use ``page_url`` for the link and ``thumbnail.src``
        for the image source. The ``title`` field carries alt-text.
        """
        results: list[SearchResult] = []
        for i, item in enumerate(raw):
            thumbnail = item.get("thumbnail", {})
            img_src = thumbnail.get("src") if isinstance(thumbnail, dict) else None
            results.append(
                SearchResult(
                    url=item.get("page_url", item.get("url", "")),
                    title=item.get("title", ""),
                    content=item.get("description", ""),
                    engine=self.name,
                    position=i + 1,
                    thumbnail=img_src,
                    img_src=img_src,
                ),
            )
        return results
