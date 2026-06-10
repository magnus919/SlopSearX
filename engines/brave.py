"""Brave Search API adapter."""

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
class BraveAdapter(EngineAdapter):
    name = "brave"
    display_name = "Brave Search API"
    env_prefix = "ENGINE_BRAVE"
    engine_type = "api"
    categories = ["general", "news", "science", "images"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if (early := await self._check_rate_limit()):
            return early

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

        params_dict: dict[str, Any] = {
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

                # Extract answer-box content from Brave's mixed/answer sections
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
                            answers.append({
                                "url": item.get("url", ""),
                                "title": item.get("title", ""),
                                "content": desc[:500],
                            })

        # Check mixed section for inline answers
        mixed = data.get("mixed")
        if isinstance(mixed, dict):
            for entry in mixed.get("main", []):
                if isinstance(entry, dict) and entry.get("type") == "answer":
                    ans_data = entry.get("data", {})
                    if isinstance(ans_data, dict):
                        desc = ans_data.get("description", "") or ""
                        if desc:
                            answers.append({
                                "url": ans_data.get("url", ""),
                                "title": ans_data.get("title", ""),
                                "content": desc[:500],
                            })

        return answers

    def _parse_results(self, raw: list[dict[str, Any]], query: str) -> list[SearchResult]:
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
