"""Stack Exchange API adapter — Stack Overflow + 180+ Q&A sites.

Free API, 10K req/day with key, 300/day without.
API docs: https://api.stackexchange.com/docs/search
"""

from __future__ import annotations

import urllib.parse
from typing import Any

from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult, register_engine


@register_engine
class StackExchangeAdapter(EngineAdapter):
    """Stack Exchange search across Stack Overflow + all SE sites."""

    name = "stackexchange"
    display_name = "Stack Exchange"
    env_prefix = "ENGINE_STACKEXCHANGE"
    engine_type = "api"
    categories = [
        "general",
        "reference",
        "science",
        "stackexchange:code",
        "stackexchange:serverfault",
    ]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        import httpx

        if (early := await self._check_rate_limit()):
            return early

        cfg = self.config
        api_key = cfg.get("api_key", "")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        # Determine site from sub-category
        site = self._site_from_categories((params or {}).get("categories", []))
        base_url = cfg.get("base_url", "https://api.stackexchange.com/2.3")

        url = (
            f"{base_url}/search"
            f"?order=desc&sort=relevance"
            f"&intitle={urllib.parse.quote(query)}"
            f"&site={site}"
            f"&pagesize={max_results}"
        )
        if api_key:
            url += f"&key={api_key}"

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000) as client:
                resp = await client.get(url)
                if resp.status_code == 400:
                    return AdapterResponse(
                        results=[],
                        status=EngineStatus.RATE_LIMITED,
                        error_message="Stack Exchange rate limited (no API key)",
                    )
                resp.raise_for_status()
                data = resp.json()
        except Exception as exc:
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
            )

        items = data.get("items", [])
        results = []
        for item in items[:max_results]:
            results.append(
                SearchResult(
                    url=item.get("link", ""),
                    title=item.get("title", ""),
                    content=item.get("body_markdown", "")[:500] if "body_markdown" in item else "",
                    engine=self.name,
                    score=float(item.get("score", 0)) / 100.0,  # normalize
                    published_date=_iso_from_unix(item.get("creation_date")),
                )
            )

        return AdapterResponse(results=results, status=EngineStatus.OK)

    @staticmethod
    def _site_from_categories(categories: list[str]) -> str:
        """Map category to Stack Exchange site parameter."""
        for cat in categories:
            if cat == "stackexchange:serverfault":
                return "serverfault"
            if cat == "stackexchange:code":
                return "stackoverflow"
        return "stackoverflow"  # default


def _iso_from_unix(ts: int | None) -> str | None:
    """Convert Unix timestamp to ISO 8601."""
    if ts is None:
        return None
    import datetime

    return datetime.datetime.fromtimestamp(ts, tz=datetime.timezone.utc).isoformat()
