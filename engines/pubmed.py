"""PubMed adapter — biomedical literature search via NCBI E-utilities.

Free, public API. No auth required.
Docs: https://www.ncbi.nlm.nih.gov/books/NBK25501/
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
class PubMedAdapter(EngineAdapter):
    """PubMed biomedical literature search."""

    name = "pubmed"
    display_name = "PubMed"
    env_prefix = "ENGINE_PUBMED"
    engine_type = "api"
    categories = ["science", "reference", "medical", "health"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://eutils.ncbi.nlm.nih.gov/entrez/eutils")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        headers = {
            "User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)",
        }
        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                # Step 1: ESearch — get PMIDs matching the query
                esearch_resp = await client.get(
                    f"{base_url}/esearch.fcgi",
                    params={
                        "db": "pubmed",
                        "term": query,
                        "retmax": max_results,
                        "retmode": "json",
                        "sort": "relevance",
                    },
                    headers=headers,
                )
                esearch_resp.raise_for_status()
                esearch_data = esearch_resp.json()

                id_list = esearch_data.get("esearchresult", {}).get("idlist", [])
                if not id_list:
                    latency = (time.monotonic() - start_time) * 1000
                    return AdapterResponse(results=[], status=EngineStatus.OK, latency_ms=latency)

                # Step 2: ESummary — get details for each PMID
                esummary_resp = await client.get(
                    f"{base_url}/esummary.fcgi",
                    params={
                        "db": "pubmed",
                        "id": ",".join(id_list),
                        "retmode": "json",
                    },
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000
                esummary_resp.raise_for_status()
                summary_data = esummary_resp.json()

                results = []
                result_map = summary_data.get("result", {})
                for idx, pmid in enumerate(id_list):
                    article = result_map.get(pmid, {})
                    title = article.get("title", "")
                    source = article.get("source", "")
                    pub_date = article.get("pubdate", "")
                    authors = article.get("authors", [])
                    author_names = [a.get("name", "") for a in authors[:3]]
                    author_str = ", ".join(author_names)
                    content_parts = [source]
                    if author_str:
                        content_parts.append(author_str)
                    content = " — ".join(content_parts)

                    results.append(
                        SearchResult(
                            url=f"https://pubmed.ncbi.nlm.nih.gov/{pmid}/",
                            title=title,
                            content=content[:500],
                            engine=self.name,
                            position=idx + 1,
                            published_date=pub_date if pub_date else None,
                            score=1.0,
                        ),
                    )

                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except httpx.HTTPStatusError as exc:
            latency = (time.monotonic() - start_time) * 1000
            if exc.response.status_code == 429:
                return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
                latency_ms=latency,
            )
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
                latency_ms=latency,
            )
