"""Internet Archive adapter — web archives, books, audio, video.

Free, public API. No auth required.
API docs: https://archive.org/advancedsearch.php
"""

from __future__ import annotations

import urllib.parse

from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult, register_engine


@register_engine
class InternetArchiveAdapter(EngineAdapter):
    """Internet Archive search — Wayback Machine, books, media.

    Deliberately excludes "general" from categories. Default-disabled
    in config. Operators must explicitly enable for targeted queries.
    """

    name = "internetarchive"
    display_name = "Internet Archive"
    env_prefix = "ENGINE_INTERNETARCHIVE"
    engine_type = "api"
    categories = ["reference", "web:archive", "historical"]

    async def search(
        self,
        query: str,
        params: dict | None = None,
    ) -> AdapterResponse:
        import httpx

        cfg = self.config
        timeout_ms = cfg.get("timeout_ms", 10_000)  # IA can be slow
        max_results = cfg.get("max_results", 10)
        base_url = cfg.get("base_url", "https://archive.org")

        url = (
            f"{base_url}/advancedsearch.php"
            f"?q={urllib.parse.quote(query)}"
            f"&output=json"
            f"&rows={max_results}"
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

        docs = data.get("response", {}).get("docs", [])
        results = []
        for doc in docs[:max_results]:
            identifier = doc.get("identifier", "")
            results.append(
                SearchResult(
                    url=f"https://archive.org/details/{identifier}" if identifier else "",
                    title=doc.get("title", ""),
                    content=(doc.get("description") or "")[:500],
                    engine=self.name,
                    score=float(doc.get("downloads", 0)) / 10000.0,
                    published_date=doc.get("date"),
                )
            )

        return AdapterResponse(results=results, status=EngineStatus.OK)
