"""UniProt adapter — protein sequence and function search.

Free, public REST API. No auth required.
Docs: https://www.uniprot.org/help/api_queries
"""

from __future__ import annotations

import re
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

_SIMPLE_QUERY_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{1,63}$")
_FIELD_QUERY_RE = re.compile(
    r"\b(?P<field>accession|gene|id)\s*:\s*(?P<value>[A-Za-z0-9][A-Za-z0-9._-]*)",
    re.IGNORECASE,
)
# UniProtKB accessions are six or ten characters in their canonical forms.
# The optional version suffix is accepted for matching, but the API result's
# primary accession remains the stable URL identifier.
_ACCESSION_RE = re.compile(r"^(?:[A-Z][0-9][A-Z0-9]{4}|[A-Z][0-9][A-Z0-9]{8})(?:-\d+)?$", re.IGNORECASE)
_MAX_CANDIDATE_OVERFETCH = 40


def _query_plan(query: str) -> tuple[str, str | None, str | None]:
    """Return an upstream query and the exact-match intent, if any.

    UniProt's bare search is full-text.  For a single identifier-like term,
    retain that broad search while explicitly adding the supported accession
    or gene field.  This keeps related proteins available while giving the
    adapter enough evidence to rank exact records ahead of them.  Existing
    fielded/compound queries are passed through unchanged.
    """
    stripped = query.strip()
    field_match = _FIELD_QUERY_RE.fullmatch(stripped)
    if field_match:
        field = field_match.group("field").lower()
        value = field_match.group("value")
        intent = "identifier" if field in {"accession", "id"} else "gene"
        return stripped, intent, value

    if not _SIMPLE_QUERY_RE.fullmatch(stripped):
        return query, None, None

    if _ACCESSION_RE.fullmatch(stripped):
        return f"({stripped}) OR (accession:{stripped})", "identifier", stripped
    return f"({stripped}) OR (gene:{stripped})", "gene", stripped


def _gene_names(protein: dict[str, Any]) -> list[str]:
    """Extract all explicitly supplied gene-name values from a record."""
    names: list[str] = []
    for gene in protein.get("genes", []) or []:
        if not isinstance(gene, dict):
            continue
        for key in ("geneName", "orderedLocusName", "orfName"):
            value = gene.get(key, {}) or {}
            if isinstance(value, dict) and isinstance(value.get("value"), str):
                names.append(value["value"])
        synonyms = gene.get("synonyms", []) or []
        for synonym in synonyms:
            if isinstance(synonym, dict) and isinstance(synonym.get("value"), str):
                names.append(synonym["value"])
    return names


def _exact_rank(
    protein: dict[str, Any],
    intent: str | None,
    value: str | None,
) -> int:
    """Return a stable relevance tier: accession, gene, then related."""
    if not value:
        return 2
    needle = value.casefold()
    accession = str(protein.get("primaryAccession", ""))
    entry_id = str(protein.get("uniProtkbId", ""))
    if intent == "identifier" and needle in {accession.casefold(), entry_id.casefold()}:
        return 0
    if intent == "gene" and any(name.casefold() == needle for name in _gene_names(protein)):
        return 1
    return 2


def _candidate_size(max_results: int) -> int:
    """Return a bounded provider window used before presentation truncation."""
    return max_results + min(max_results * 2, _MAX_CANDIDATE_OVERFETCH)


@register_engine
class UniProtAdapter(EngineAdapter):
    """UniProt protein search."""

    name = "uniprot"
    display_name = "UniProt"
    env_prefix = "ENGINE_UNIPROT"
    engine_type = "api"
    categories = ["science", "reference", "biology", "medical"]

    # -- Declared capability metadata (audited, issue 185) --
    supported_result_types = ("text",)
    failure_classes = ("rate_limited", "error", "timeout")
    cost_class = "free"

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
        candidate_size = _candidate_size(max_results)

        headers = {
            "User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)",
            "Accept": "application/json",
        }
        start_time = time.monotonic()
        upstream_query, intent, intent_value = _query_plan(query)

        try:
            async with self.http_client(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    base_url,
                    params={
                        "query": upstream_query,
                        "size": candidate_size,
                        "format": "json",
                    },
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()
                data = resp.json()

                results = []
                proteins = data.get("results", [])
                ranked_proteins = sorted(
                    enumerate(proteins[:candidate_size]),
                    key=lambda item: (_exact_rank(item[1], intent, intent_value), item[0]),
                )
                for position, (_upstream_position, protein) in enumerate(ranked_proteins[:max_results], start=1):
                    primary_accession = protein.get("primaryAccession", "")
                    uni_id = protein.get("uniProtkbId", "")
                    description = protein.get("proteinDescription", {})
                    rec_name = description.get("recommendedName", {}) or {}
                    full_name = rec_name.get("fullName", {}).get("value", "") or ""
                    organism = protein.get("organism", {}) or {}
                    organism_name = organism.get("scientificName", "")
                    gene_names = _gene_names(protein)
                    gene_name = gene_names[0] if gene_names else ""

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
                            position=position,
                            score=3.0 - _exact_rank(protein, intent, intent_value),
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
