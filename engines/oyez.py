"""Oyez adapter — US Supreme Court case search.

Free, public REST API. No auth required.
Docs: https://www.oyez.org/api-docs
"""

from __future__ import annotations

import time
from typing import Any
from urllib.parse import unquote, urlparse

import httpx

from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult, register_engine

_DEFAULT_SEARCH_URL = "https://beta-search.oyez.org/elasticsearch_index_scotus_nodes/_search"
_OYEZ_HOSTS = {"api.oyez.org", "www.oyez.org"}
_SEARCH_FIELDS = (
    "field_docket_number^3",
    "field_additional_docket_numbers^3",
    "title^2",
    "field_court_term",
    "field_first_party",
    "field_second_party",
    "field_facts_of_the_case:value",
    "field_question:value",
    "field_conclusion:value",
    "field_biography:value",
    "field_citation:field_volume",
    "field_citation:field_page",
    "field_citation:field_year^0.2",
)


def _search_body(query: str, max_results: int) -> dict[str, Any]:
    """Build the query used by Oyez's public search frontend.

    Oyez's official frontend uses Elasticsearch ``multi_match`` over these
    fields.  Keeping this shape lets Oyez assign relevance scores, including
    fuzzy alternatives, instead of pretending that the REST ``/cases``
    endpoint's unsupported ``name`` parameter is a title search.
    """
    return {
        "size": max_results,
        "query": {
            "multi_match": {
                "query": query,
                "type": "cross_fields",
                "fields": list(_SEARCH_FIELDS),
            },
        },
    }


def _case_url(value: Any) -> str:
    """Return the public Oyez case URL without dropping term/docket identity."""
    raw = value.strip() if isinstance(value, str) else ""
    if not raw:
        return ""

    parsed = urlparse(raw)
    if parsed.scheme or parsed.netloc:
        if parsed.scheme != "https" or (parsed.hostname or "").casefold() not in _OYEZ_HOSTS:
            return ""
    elif not parsed.path.startswith("/"):
        return ""

    parts = [unquote(part) for part in parsed.path.split("/") if part]
    if len(parts) != 3 or parts[0] != "cases" or any("/" in part for part in parts[1:]):
        return ""
    return f"https://www.oyez.org/cases/{parts[1]}/{parts[2]}"


def _records(data: Any) -> list[tuple[dict[str, Any], Any]]:
    """Extract source records and source relevance scores from either API shape."""
    cases: Any = []
    if isinstance(data, dict) and isinstance(data.get("hits"), dict):
        hits = data["hits"].get("hits", [])
        if not isinstance(hits, list):
            return []
        records: list[tuple[dict[str, Any], Any]] = []
        for hit in hits:
            if not isinstance(hit, dict) or not isinstance(hit.get("_source"), dict):
                continue
            records.append((hit["_source"], hit.get("_score")))
        return records

    if isinstance(data, list):
        cases = data
    elif isinstance(data, dict):
        cases = data.get("results", data.get("data", []))
    if not isinstance(cases, list):
        return []
    return [(case, None) for case in cases if isinstance(case, dict)]


def _first_text(value: Any) -> str:
    """Extract a usable string from a scalar or source list."""
    values = value if isinstance(value, list) else [value]
    for item in values:
        if isinstance(item, str) and item.strip():
            return item.strip()
    return ""


@register_engine
class OyezAdapter(EngineAdapter):
    """US Supreme Court case search via Oyez API."""

    name = "oyez"
    display_name = "Oyez (SCOTUS)"
    env_prefix = "ENGINE_OYEZ"
    engine_type = "api"
    categories = ["reference", "legal"]

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
        base_url = cfg.get("base_url")
        search_url = cfg.get("search_url") or _DEFAULT_SEARCH_URL
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        start_time = time.monotonic()

        try:
            async with self.http_client(timeout=timeout_ms / 1000.0) as client:
                if cfg.get("search_url"):
                    resp = await client.post(search_url, json=_search_body(query, max_results))
                elif isinstance(base_url, str) and base_url.strip():
                    # Preserve an explicit REST endpoint override for local
                    # deployments and deterministic legacy fixtures.
                    resp = await client.get(
                        f"{base_url.rstrip('/')}/cases",
                        params={
                            "page": 1,
                            "per_page": max_results,
                            "order": "desc",
                            "sort": "decision_date",
                        },
                    )
                else:
                    resp = await client.post(search_url, json=_search_body(query, max_results))
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 404:
                    return AdapterResponse(results=[], status=EngineStatus.OK, latency_ms=latency)

                resp.raise_for_status()
                data = resp.json()

                results: list[SearchResult] = []
                for case, source_score in _records(data):
                    name = _first_text(case.get("name")) or _first_text(case.get("title"))
                    term = _first_text(case.get("term")) or _first_text(case.get("field_court_term"))
                    decision_date = _first_text(case.get("decision_date"))
                    href_candidates = (
                        _first_text(case.get("search_api_url")),
                        _first_text(case.get("url")),
                        _first_text(case.get("href")),
                    )
                    public_url = next(
                        (
                            candidate
                            for candidate in (_case_url(value) for value in href_candidates)
                            if candidate
                        ),
                        "",
                    )
                    if not public_url:
                        continue

                    description = _first_text(case.get("description")) or _first_text(
                        case.get("field_case_description"),
                    )
                    content = description or (f"Supreme Court term: {term}" if term else "")
                    if decision_date:
                        content = f"Decided: {decision_date}" + (f" | {content}" if content else "")

                    try:
                        score = float(source_score) if source_score is not None else 1.0
                    except (TypeError, ValueError):
                        score = 1.0

                    results.append(
                        SearchResult(
                            url=public_url,
                            title=name,
                            content=content[:500] if content else "U.S. Supreme Court case",
                            engine=self.name,
                            position=len(results) + 1,
                            published_date=decision_date if decision_date else None,
                            score=score,
                        ),
                    )
                    if len(results) >= max_results:
                        break

                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except httpx.HTTPStatusError as exc:
            latency = (time.monotonic() - start_time) * 1000
            if exc.response.status_code == 429:
                return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
            return AdapterResponse(results=[], status=EngineStatus.ERROR, error_message=str(exc), latency_ms=latency)
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
                latency_ms=latency,
            )
