"""PyPI adapter — Python Package Index search via HTML scraping.

Free, public search. No auth required.
Parses search results from https://pypi.org/search/
"""

from __future__ import annotations

import time
from typing import Any

import httpx
from lxml import html

from slopsearx.adapter import (
    AdapterResponse,
    EngineAdapter,
    EngineStatus,
    SearchResult,
    register_engine,
)


@register_engine
class PyPIAdapter(EngineAdapter):
    """PyPI package search via HTML scraping."""

    name = "pypi"
    display_name = "PyPI"
    env_prefix = "ENGINE_PYPI"
    engine_type = "api"
    categories = ["general", "it", "reference", "packages"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://pypi.org/search/")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        headers = {"User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)"}
        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    base_url,
                    params={"q": query, "page": 1},
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()

                results = self._parse_search_results(resp.text, max_results)
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

    def _parse_search_results(self, html_text: str, max_results: int) -> list[SearchResult]:
        """Parse PyPI search result HTML into SearchResult list."""
        tree = html.fromstring(html_text)
        results = []

        snippets = tree.cssselect(".package-snippet")
        for idx, snippet in enumerate(snippets[:max_results]):
            name_el = snippet.cssselect(".package-snippet__name")
            version_el = snippet.cssselect(".package-snippet__version")
            desc_el = snippet.cssselect(".package-snippet__description")

            name = name_el[0].text_content().strip() if name_el else ""
            version = version_el[0].text_content().strip() if version_el else ""
            description = desc_el[0].text_content().strip() if desc_el else ""

            results.append(
                SearchResult(
                    url=f"https://pypi.org/project/{name}/",
                    title=f"{name} {version}",
                    content=description[:500] if description else f"Python package: {name}",
                    engine=self.name,
                    position=idx + 1,
                    score=1.0,
                ),
            )

        return results
