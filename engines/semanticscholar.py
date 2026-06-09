"""Semantic Scholar adapter — scientific literature search with citation data."""

from __future__ import annotations

import time

import httpx

from slopsearx.adapter import (
    AdapterResponse,
    EngineAdapter,
    EngineStatus,
    SearchResult,
    register_engine,
)


@register_engine
class SemanticScholarAdapter(EngineAdapter):
    name = "semanticscholar"
    display_name = "Semantic Scholar"
    env_prefix = "ENGINE_SEMANTICSCHOLAR"
    engine_type = "api"

    async def search(
        self,
        query: str,
        params: dict | None = None,
    ) -> AdapterResponse:
        cfg = self.config
        api_key = cfg.get("api_key") or ""
        base_url = cfg.get("base_url", "https://api.semanticscholar.org/graph/v1/paper/search")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 5)

        fields = "title,url,abstract,citationCount,publicationDate,externalIds,authors"
        params_dict: dict = {
            "query": query,
            "limit": max_results,
            "fields": fields,
        }

        headers = {
            "User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)",
        }
        if api_key:
            headers["x-api-key"] = api_key

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(base_url, params=params_dict, headers=headers)
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                resp.raise_for_status()

                data = resp.json()
                papers = data.get("data", [])
                results = self._parse_papers(papers)
                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR, error_message=str(exc), latency_ms=latency,
            )

    def _parse_papers(self, papers: list[dict]) -> list[SearchResult]:
        """Parse Semantic Scholar paper results into SearchResult list."""
        results: list[SearchResult] = []

        for idx, paper in enumerate(papers):
            title = paper.get("title", "")
            url = paper.get("url", "")
            abstract = paper.get("abstract") or ""
            citation_count = paper.get("citationCount", 0)
            pub_date = paper.get("publicationDate") or None
            external_ids = paper.get("externalIds") or {}
            authors_raw = paper.get("authors", []) or []
            author_names = [a.get("name", "") for a in authors_raw if isinstance(a, dict)]
            author_str = ", ".join(author_names[:3])
            if len(author_names) > 3:
                author_str += " et al."

            # Build content from abstract + citation + author metadata
            content_parts = []
            if abstract:
                clean = abstract.strip()[:300]
                if len(abstract) > 300:
                    clean += "…"
                content_parts.append(clean)
            if author_str:
                content_parts.append(f"By: {author_str}")
            content_parts.append(f"Cited by: {citation_count}")

            # Add arXiv ID if present for cross-referencing
            arxiv_id = external_ids.get("ArXiv")
            if arxiv_id:
                content_parts.append(f"arXiv: {arxiv_id}")

            results.append(
                SearchResult(
                    url=url,
                    title=title,
                    content=" | ".join(content_parts),
                    engine=self.name,
                    position=idx + 1,
                    score=float(citation_count),
                    published_date=pub_date,
                ),
            )

        return results
