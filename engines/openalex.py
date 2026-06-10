"""OpenAlex adapter — 250M+ scholarly works, authors, institutions.

Free, open REST API. No auth required. Polite usage: 100K/day.
API docs: https://docs.openalex.org/api-entities/works/search-works
"""

from __future__ import annotations

import urllib.parse
from typing import Any

from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult, register_engine


@register_engine
class OpenAlexAdapter(EngineAdapter):
    """OpenAlex scholarly search — works, authors, institutions."""

    name = "openalex"
    display_name = "OpenAlex"
    env_prefix = "ENGINE_OPENALEX"
    engine_type = "api"
    categories = ["general", "science", "reference"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        import httpx

        if (early := await self._check_rate_limit()):
            return early

        cfg = self.config
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)
        base_url = cfg.get("base_url", "https://api.openalex.org")

        url = (
            f"{base_url}/works"
            f"?search={urllib.parse.quote(query)}"
            f"&sort=cited_by_count:desc"
            f"&per_page={max_results}"
        )

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000) as client:
                resp = await client.get(url)
                resp.raise_for_status()
                data = resp.json()
        except Exception as exc:
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
            )

        results = []
        for work in data.get("results", [])[:max_results]:
            doi = work.get("doi", "")
            url = f"https://doi.org/{doi}" if doi else work.get("id", "")
            content = _reconstruct_abstract(work.get("abstract_inverted_index"))

            results.append(
                SearchResult(
                    url=url,
                    title=work.get("title", ""),
                    content=content[:500],
                    engine=self.name,
                    score=float(work.get("cited_by_count", 0)) / 1000.0,
                    published_date=work.get("publication_date"),
                )
            )

        return AdapterResponse(results=results, status=EngineStatus.OK)


def _reconstruct_abstract(inverted: dict[str, Any] | None) -> str:
    """Reconstruct abstract text from OpenAlex inverted index.

    Format: {"word": [positions], ...} → "word word word ..."
    """
    if not inverted:
        return ""
    # Build (position, word) pairs, sort by position, join
    pairs = []
    for word, positions in inverted.items():
        for pos in positions:
            pairs.append((pos, word))
    pairs.sort()
    return " ".join(w for _, w in pairs)
