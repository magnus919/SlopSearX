"""PubMed adapter — biomedical literature search via NCBI E-utilities.

Free, public API. No auth required.
Docs: https://www.ncbi.nlm.nih.gov/books/NBK25501/
"""

from __future__ import annotations

import asyncio
import math
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
from slopsearx.payload import DOMAIN_SCIENCE, build_payload


@register_engine
class PubMedAdapter(EngineAdapter):
    """PubMed biomedical literature search."""

    name = "pubmed"
    display_name = "PubMed"
    env_prefix = "ENGINE_PUBMED"
    engine_type = "api"
    categories = ["science", "reference", "medical", "health"]

    # -- Declared capability metadata (audited, issue 185) --
    supported_result_types = ("text",)
    failure_classes = ("rate_limited", "error", "timeout")
    cost_class = "free"

    # The shared service uses timeout_ms as the whole adapter dispatch budget.
    # Reserve a small amount for cleanup and service bookkeeping before the
    # sequential ESearch -> ESummary pipeline starts its final stage.
    AGGREGATE_TIMEOUT_FRACTION = 0.9

    @staticmethod
    def _id_list(payload: Any) -> list[str]:
        """Validate and return the PMID list from an ESearch JSON payload."""
        if not isinstance(payload, dict):
            raise ValueError("ESearch payload must be an object")
        search_result = payload.get("esearchresult")
        if not isinstance(search_result, dict):
            raise ValueError("ESearch payload is missing esearchresult")
        id_list = search_result.get("idlist")
        if not isinstance(id_list, list):
            raise ValueError("ESearch payload is missing idlist")
        if any(not isinstance(pmid, str) or not pmid for pmid in id_list):
            raise ValueError("ESearch idlist contains an invalid PMID")
        return id_list

    @staticmethod
    def _text_field(article: dict[str, Any], name: str) -> str:
        """Read an optional ESummary text field without fabricating values."""
        value = article.get(name, "")
        if value is None:
            return ""
        if not isinstance(value, str):
            raise ValueError(f"ESummary field {name} must be text")
        return value

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
        aggregate_timeout_ms = cfg.get("aggregate_timeout_ms")

        headers = {
            "User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)",
        }
        start_time = time.monotonic()

        try:
            if isinstance(timeout_ms, bool):
                raise ValueError("timeout_ms must be a positive finite number")
            request_timeout_ms = float(timeout_ms)
            if not math.isfinite(request_timeout_ms) or request_timeout_ms <= 0:
                raise ValueError("timeout_ms must be a positive finite number")

            if aggregate_timeout_ms is None:
                aggregate_timeout_ms = request_timeout_ms * self.AGGREGATE_TIMEOUT_FRACTION
            elif isinstance(aggregate_timeout_ms, bool):
                raise ValueError("aggregate_timeout_ms must be a positive finite number")
            else:
                aggregate_timeout_ms = float(aggregate_timeout_ms)
            if not math.isfinite(aggregate_timeout_ms) or aggregate_timeout_ms <= 0:
                raise ValueError("aggregate_timeout_ms must be a positive finite number")
            if aggregate_timeout_ms > request_timeout_ms:
                raise ValueError("aggregate_timeout_ms cannot exceed timeout_ms")

            request_timeout = request_timeout_ms / 1000.0
            deadline = start_time + aggregate_timeout_ms / 1000.0
            async with self.http_client(timeout=request_timeout) as client:

                async def bounded_get(url: str, **kwargs: Any) -> httpx.Response:
                    remaining = min(request_timeout, deadline - time.monotonic())
                    if remaining <= 0:
                        raise TimeoutError("PubMed aggregate deadline exceeded")
                    # Keep the client-level timeout for compatibility while
                    # bounding each stage by the remaining aggregate budget.
                    async with asyncio.timeout(remaining):
                        return await client.get(url, timeout=remaining, **kwargs)

                # Step 1: ESearch — get PMIDs matching the query
                esearch_resp = await bounded_get(
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

                id_list = self._id_list(esearch_data)
                if not id_list:
                    latency = (time.monotonic() - start_time) * 1000
                    return AdapterResponse(results=[], status=EngineStatus.OK, latency_ms=latency)

                # Step 2: ESummary — get details for each PMID
                esummary_resp = await bounded_get(
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

                if not isinstance(summary_data, dict):
                    raise ValueError("ESummary payload must be an object")
                result_map = summary_data.get("result")
                if not isinstance(result_map, dict):
                    raise ValueError("ESummary payload is missing result")

                results: list[SearchResult] = []
                for idx, pmid in enumerate(id_list):
                    # NCBI can omit a summary for an otherwise valid UID;
                    # preserve the successful partial response without
                    # emitting a fabricated blank article.
                    if pmid not in result_map:
                        continue
                    article = result_map[pmid]
                    if not isinstance(article, dict):
                        raise ValueError(f"ESummary entry for PMID {pmid} must be an object")

                    title = self._text_field(article, "title")
                    source = self._text_field(article, "source")
                    pub_date = self._text_field(article, "pubdate")
                    authors = article.get("authors", [])
                    if not isinstance(authors, list):
                        raise ValueError(f"ESummary authors for PMID {pmid} must be a list")
                    # The payload carries the FULL author list (fidelity); the
                    # display content string stays capped at the first three.
                    author_names = []
                    for author in authors:
                        if not isinstance(author, dict):
                            raise ValueError(f"ESummary author for PMID {pmid} must be an object")
                        name = self._text_field(author, "name")
                        if name:
                            author_names.append(name)
                    author_str = ", ".join(author_names[:3])
                    content_parts = [source]
                    if author_str:
                        content_parts.append(author_str)
                    content = " — ".join(content_parts)

                    payload = build_payload(
                        DOMAIN_SCIENCE,
                        "publication",
                        {
                            "publication_id": pmid or None,
                            "pmid": pmid or None,
                            "journal": source or None,
                            "authors": author_names or None,
                        },
                        engine=self.name,
                    )

                    results.append(
                        SearchResult(
                            url=f"https://pubmed.ncbi.nlm.nih.gov/{pmid}/",
                            title=title,
                            content=content[:500],
                            engine=self.name,
                            position=idx + 1,
                            published_date=pub_date if pub_date else None,
                            score=1.0,
                            payload=payload,
                        ),
                    )

                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except (httpx.TimeoutException, TimeoutError):
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
