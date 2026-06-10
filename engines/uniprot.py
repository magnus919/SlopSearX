"""UniProt adapter — protein sequence and function search.

Free, public REST API. No auth required.
Docs: https://www.uniprot.org/help/api_queries
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
class UniProtAdapter(EngineAdapter):
    """UniProt protein search."""

    name = "uniprot"
    display_name = "UniProt"
    env_prefix = "ENGINE_UNIPROT"
    engine_type = "api"
    categories = ["general", "science", "reference", "biology", "medical"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://rest.uniprot.org/uniprotkb/search")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        headers = {
            "User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)",
            "Accept": "application/json",
        }
        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    base_url,
                    params={
                        "query": query,
                        "size": max_results,
                        "format": "json",
                    },
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()
                data = resp.json()

                results = []
                proteins = data.get("results", [])
                for idx, protein in enumerate(proteins[:max_results]):
                    primary_accession = protein.get("primaryAccession", "")
                    uni_id = protein.get("uniProtkbId", "")
                    description = protein.get("proteinDescription", {})
                    rec_name = description.get("recommendedName", {}) or {}
                    full_name = rec_name.get("fullName", {}).get("value", "") or ""
                    organism = protein.get("organism", {}) or {}
                    organism_name = organism.get("scientificName", "")
                    gene = protein.get("genes", [None])
                    gene_name = gene[0].get("geneName", {}).get("value", "") if gene and gene[0] else ""

                    content_parts = [organism_name] if organism_name else []
                    if gene_name:
                        content_parts.append(f"Gene: {gene_name}")
                    content = " — ".join(content_parts) if content_parts else "Protein"

                    label = full_name or uni_id or primary_accession
                    results.append(
                        SearchResult(
                            url=f"https://www.uniprot.org/uniprotkb/{primary_accession}/entry",
                            title=f"{primary_accession} — {label}" if full_name else label,
                            content=content[:500],
                            engine=self.name,
                            position=idx + 1,
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
