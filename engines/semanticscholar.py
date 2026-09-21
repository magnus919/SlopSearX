"""Semantic Scholar adapter — scientific literature search with citation data."""

from __future__ import annotations

import datetime as _dt
import math
import time
from email.utils import parsedate_to_datetime
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
from slopsearx.ratelimit import (
    DEFAULT_UPSTREAM_COOLDOWN_SECONDS,
    MAX_UPSTREAM_COOLDOWN_SECONDS,
    MIN_UPSTREAM_COOLDOWN_SECONDS,
)


def _parse_retry_after(value: str | None, *, now: float | None = None) -> float:
    """Return a bounded Retry-After delay, falling back safely when invalid."""
    fallback = DEFAULT_UPSTREAM_COOLDOWN_SECONDS
    if not value:
        return fallback

    raw = value.strip()
    try:
        seconds = float(raw)
    except (TypeError, ValueError):
        seconds = math.nan
    if math.isfinite(seconds) and seconds >= 0:
        return min(MAX_UPSTREAM_COOLDOWN_SECONDS, max(MIN_UPSTREAM_COOLDOWN_SECONDS, seconds))

    try:
        retry_at = parsedate_to_datetime(raw)
        if retry_at.tzinfo is None:
            retry_at = retry_at.replace(tzinfo=_dt.UTC)
        remaining = retry_at.timestamp() - (time.time() if now is None else now)
    except (TypeError, ValueError, OverflowError, IndexError):
        return fallback
    if not math.isfinite(remaining) or remaining <= 0:
        return fallback
    return min(MAX_UPSTREAM_COOLDOWN_SECONDS, max(MIN_UPSTREAM_COOLDOWN_SECONDS, remaining))


@register_engine
class SemanticScholarAdapter(EngineAdapter):
    name = "semanticscholar"
    display_name = "Semantic Scholar"
    env_prefix = "ENGINE_SEMANTICSCHOLAR"
    engine_type = "api"
    categories = ["science", "reference"]

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
        api_key = cfg.get("api_key") or ""
        base_url = cfg.get("base_url", "https://api.semanticscholar.org/graph/v1/paper/search")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 5)

        fields = "title,url,abstract,citationCount,publicationDate,externalIds,authors"
        params_dict: dict[str, Any] = {
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
            async with self.http_client(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(base_url, params=params_dict, headers=headers)
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    cooldown = _parse_retry_after(resp.headers.get("Retry-After"))
                    limiter = self.rate_limiter
                    set_cooldown = getattr(limiter, "set_upstream_cooldown", None)
                    if set_cooldown is not None:
                        try:
                            await set_cooldown(self.name, cooldown)
                        except Exception:  # noqa: BLE001 - cooldown must not break adapter classification
                            pass
                    return AdapterResponse(
                        results=[],
                        status=EngineStatus.RATE_LIMITED,
                        error_message=f"upstream rate limited; cooldown {cooldown:.0f}s",
                        latency_ms=latency,
                    )
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
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
                latency_ms=latency,
            )

    def _parse_papers(self, papers: list[dict[str, Any]]) -> list[SearchResult]:
        """Parse Semantic Scholar paper results into SearchResult list."""
        results: list[SearchResult] = []

        for idx, paper in enumerate(papers):
            title = paper.get("title", "")
            url = paper.get("url", "")
            abstract = paper.get("abstract") or ""
            citation_count = paper.get("citationCount", 0)
            pub_date = paper.get("publicationDate") or None
            raw_external_ids = paper.get("externalIds")
            external_ids = raw_external_ids if isinstance(raw_external_ids, dict) else {}
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
                    payload=build_payload(
                        DOMAIN_SCIENCE,
                        "publication",
                        {
                            "doi": external_ids.get("DOI") or None,
                            "pmid": external_ids.get("PubMed") or None,
                            "pmcid": external_ids.get("PubMedCentral") or None,
                        },
                        engine=self.name,
                    ),
                ),
            )

        return results
