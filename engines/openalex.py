"""OpenAlex adapter — 250M+ scholarly works, authors, institutions.

Free, open REST API. No auth required. Polite usage: 100K/day.
API docs: https://help.openalex.org/api/filtering/
"""

from __future__ import annotations

import time
import urllib.parse
from typing import Any

import httpx

from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult, register_engine
from slopsearx.filters import publication_date_bounds, publication_date_in_bounds, time_range_window


@register_engine
class OpenAlexAdapter(EngineAdapter):
    """OpenAlex scholarly search — works, authors, institutions."""

    name = "openalex"
    display_name = "OpenAlex"
    env_prefix = "ENGINE_OPENALEX"
    engine_type = "api"
    categories = ["science", "reference"]

    # -- Declared capability metadata (audited, issue 185) --
    supported_result_types = ("text",)
    failure_classes = ("rate_limited", "error", "timeout")
    cost_class = "free"
    supported_filters = {"date_from": True, "date_to": True, "time_range": True}
    # OpenAlex applies inclusive publication-date range queries upstream.
    # Relative windows use the service's audited published_date post-filter.
    enforced_filters = {"date_from": "upstream", "date_to": "upstream", "time_range": "local"}

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:

        started = time.monotonic()
        try:
            params = params or {}
            date_from, date_to = publication_date_bounds(params.get("date_from"), params.get("date_to"))
            if params.get("time_range") and time_range_window(params["time_range"]) is None:
                raise ValueError("time_range must be day, week, month, or year")
            if early := await self._check_rate_limit():
                early.latency_ms = (time.monotonic() - started) * 1000
                return early

            cfg = self.config
            timeout_ms = cfg.get("timeout_ms", 5_000)
            max_results = cfg.get("max_results", 10)
            base_url = cfg.get("base_url", "https://api.openalex.org")
            # OpenAlex text searches default to descending relevance_score.
            url = f"{base_url}/works?search={urllib.parse.quote(query)}&per_page={max_results}"
            date_filters = []
            if date_from is not None:
                date_filters.append(f"from_publication_date:{date_from.isoformat()}")
            if date_to is not None:
                date_filters.append(f"to_publication_date:{date_to.isoformat()}")
            if date_filters:
                url += "&filter=" + urllib.parse.quote(",".join(date_filters))
            async with self.http_client(timeout=timeout_ms / 1000) as client:
                resp = await client.get(url)
                resp.raise_for_status()
                data = resp.json()

            results = []
            for work in data.get("results", [])[:max_results]:
                # Guard against malformed/upstream-inconsistent records; never
                # infer dates from publication_year, identifiers, or titles.
                if date_filters and not publication_date_in_bounds(work.get("publication_date"), date_from, date_to):
                    continue
                doi = work.get("doi")
                url = _work_url(doi, work.get("id", ""))
                content = _reconstruct_abstract(work.get("abstract_inverted_index"))
                results.append(
                    SearchResult(
                        url=url,
                        title=work.get("title") or "",
                        content=content[:500],
                        engine=self.name,
                        score=float(work.get("relevance_score") or 0),
                        published_date=work.get("publication_date"),
                    )
                )
            return AdapterResponse(
                results=results, status=EngineStatus.OK, latency_ms=(time.monotonic() - started) * 1000
            )
        except httpx.TimeoutException as exc:
            status = EngineStatus.TIMEOUT
            error_message = str(exc)
        except httpx.HTTPStatusError as exc:
            status = EngineStatus.RATE_LIMITED if exc.response.status_code == 429 else EngineStatus.ERROR
            error_message = str(exc)
        except Exception as exc:
            # Parsing and configuration errors also honor the never-raise contract.
            status = EngineStatus.ERROR
            error_message = str(exc)
        return AdapterResponse(
            results=[], status=status, error_message=error_message, latency_ms=(time.monotonic() - started) * 1000
        )


def _work_url(doi: str | None, fallback: str) -> str:
    """OpenAlex supplies DOI URLs; also accept bare DOI identifiers."""
    if not doi:
        return fallback
    doi = doi.strip()
    if doi.startswith(("https://", "http://")):
        return doi
    return f"https://doi.org/{doi}" if doi else fallback


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
