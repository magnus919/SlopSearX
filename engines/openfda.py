"""openFDA adapter — FDA drug, device, and food recall data.

Free, public JSON API. No auth required.
Docs: https://open.fda.gov/apis/
"""

from __future__ import annotations

import time
from typing import Any
from urllib.parse import urlencode

import httpx

from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult, register_engine
from slopsearx.payload import DOMAIN_BIOMEDICAL, build_payload

_NAME_FIELDS = ("brand_name", "generic_name", "substance_name")
_DEFAULT_BASE_URL = "https://api.fda.gov/drug/label.json"


def _first_text(value: Any) -> str:
    """Return the first non-empty source value without fabricating one."""
    values = value if isinstance(value, list) else [value]
    for candidate in values:
        if isinstance(candidate, str) and candidate.strip():
            return candidate.strip()
    return ""


def _field_values(openfda: Any, field: str) -> list[str]:
    """Return all usable values for one harmonized openFDA name field."""
    if not isinstance(openfda, dict):
        return []
    value = openfda.get(field, [])
    values = value if isinstance(value, list) else [value]
    return [item.strip() for item in values if isinstance(item, str) and item.strip()]


def _normalized_text(value: str) -> str:
    """Normalize only case and surrounding whitespace for local ranking."""
    return value.strip().casefold()


def _query_variants(query: str) -> list[str]:
    """Cover documented case-sensitive exact fields without broadening terms."""
    normalized = query.strip()
    if not normalized:
        return []
    variants = [normalized, normalized.casefold(), normalized.upper(), normalized.title(), normalized.capitalize()]
    return list(dict.fromkeys(variants))


def _quote_term(term: str) -> str:
    """Quote an openFDA phrase and escape characters significant to the query syntax."""
    escaped = term.replace("\\", "\\\\").replace('"', '\\"')
    return f'"{escaped}"'


def _search_expression(query: str, *, exact: bool) -> str:
    """Build a provider-supported OR query over the three drug-name fields."""
    suffix = ".exact" if exact else ""
    terms = _query_variants(query) if exact else [query.strip()]
    # openFDA defines whitespace-separated field clauses as OR. Do not use
    # +AND+ here: a label need only match one of the name fields.
    return " ".join(
        f"openfda.{field}{suffix}:{_quote_term(term)}"
        for field in _NAME_FIELDS
        for term in terms
    )


def _match_rank(item: dict[str, Any], query: str) -> int:
    """Rank exact source-name matches above related and unrelated records."""
    target = _normalized_text(query)
    if not target:
        return 0
    values = [value for field in _NAME_FIELDS for value in _field_values(item.get("openfda", {}), field)]
    normalized_values = [_normalized_text(value) for value in values]
    if target in normalized_values:
        return 3
    if any(target in value or value in target for value in normalized_values):
        return 2
    target_tokens = set(target.split())
    if target_tokens and any(target_tokens <= set(value.split()) for value in normalized_values):
        return 1
    return 0


def _record_key(item: dict[str, Any]) -> str | None:
    """Use FDA identifiers for deterministic de-duplication when present."""
    for field in ("set_id", "spl_set_id", "spl_id", "id"):
        value = _first_text(item.get(field))
        if value:
            return f"{field}:{value}"
    openfda = item.get("openfda")
    if isinstance(openfda, dict):
        for field in ("spl_set_id", "spl_id"):
            value = _first_text(openfda.get(field))
            if value:
                return f"openfda.{field}:{value}"
    return None


def _record_url(item: dict[str, Any]) -> str | None:
    """Return an item-specific FDA API link, or None when identity is absent."""
    set_id = _first_text(item.get("set_id"))
    if set_id:
        search = f'set_id.exact:{_quote_term(set_id)}'
        return f"https://api.fda.gov/drug/label.json?{urlencode({'search': search, 'limit': 1})}"

    openfda = item.get("openfda")
    if isinstance(openfda, dict):
        spl_set_id = _first_text(openfda.get("spl_set_id"))
        if spl_set_id:
            search = f'openfda.spl_set_id.exact:{_quote_term(spl_set_id)}'
            return f"https://api.fda.gov/drug/label.json?{urlencode({'search': search, 'limit': 1})}"
        spl_id = _first_text(openfda.get("spl_id"))
        if spl_id:
            search = f'openfda.spl_id.exact:{_quote_term(spl_id)}'
            return f"https://api.fda.gov/drug/label.json?{urlencode({'search': search, 'limit': 1})}"

    document_id = _first_text(item.get("id"))
    if document_id:
        search = f'id.exact:{_quote_term(document_id)}'
        return f"https://api.fda.gov/drug/label.json?{urlencode({'search': search, 'limit': 1})}"

    return None


@register_engine
class OpenFDAAdapter(EngineAdapter):
    """FDA drug and device data search via openFDA."""

    name = "openfda"
    display_name = "openFDA"
    env_prefix = "ENGINE_OPENFDA"
    engine_type = "api"
    categories = ["medical", "health", "science", "government"]

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
        configured_base_url = cfg.get("base_url")
        base_url = (
            configured_base_url.strip()
            if isinstance(configured_base_url, str) and configured_base_url.strip()
            else _DEFAULT_BASE_URL
        )
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        start_time = time.monotonic()

        try:
            query = query.strip()
            if not query:
                return AdapterResponse(results=[], status=EngineStatus.OK, latency_ms=0.0)

            async with self.http_client(timeout=timeout_ms / 1000.0) as client:
                exact_data = await self._fetch(client, base_url, _search_expression(query, exact=True), max_results)
                exact_items = self._items(exact_data)
                items = exact_items
                if not items:
                    broad_data = await self._fetch(
                        client,
                        base_url,
                        _search_expression(query, exact=False),
                        max_results,
                    )
                    items = self._items(broad_data)

                results, omitted = self._build_results(items, query, max_results)
                latency = (time.monotonic() - start_time) * 1000
                diagnostic = (
                    f"omitted {omitted} openFDA label record(s) without a stable source identifier"
                    if omitted
                    else None
                )

                return AdapterResponse(
                    results=results,
                    status=EngineStatus.OK,
                    error_message=diagnostic,
                    latency_ms=latency,
                )

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

    async def _fetch(self, client: Any, base_url: str, search: str, max_results: int) -> dict[str, Any]:
        """Fetch one bounded openFDA query; a 404 is the API's empty result shape."""
        resp = await client.get(base_url, params={"search": search, "limit": max_results})
        if resp.status_code == 404:
            return {"results": []}
        resp.raise_for_status()
        data = resp.json()
        if not isinstance(data, dict):
            raise ValueError("openFDA response must be a JSON object")
        items = data.get("results", [])
        if not isinstance(items, list):
            raise ValueError("openFDA results must be a JSON array")
        return data

    def _items(self, data: dict[str, Any]) -> list[dict[str, Any]]:
        """Retain only structured source records for the normalizer."""
        return [item for item in data.get("results", []) if isinstance(item, dict)]

    def _build_results(
        self,
        items: list[dict[str, Any]],
        query: str,
        max_results: int,
    ) -> tuple[list[SearchResult], int]:
        ranked: list[tuple[int, int, str | None, dict[str, Any]]] = []
        seen: set[str] = set()
        omitted = 0
        for source_position, item in enumerate(items):
            openfda = item.get("openfda", {})
            if not isinstance(openfda, dict):
                openfda = {}
            if _record_url(item) is None:
                omitted += 1
                continue
            key = _record_key(item)
            if key is not None and key in seen:
                continue
            if key is not None:
                seen.add(key)
            ranked.append((_match_rank(item, query), source_position, key, item))

        ranked.sort(key=lambda entry: (-entry[0], entry[1]))
        results: list[SearchResult] = []
        for position, (match_rank, _source_position, _key, item) in enumerate(ranked[:max_results], start=1):
            openfda = item.get("openfda", {})
            if not isinstance(openfda, dict):
                openfda = {}
            brand_name = _first_text(openfda.get("brand_name"))
            generic_name = _first_text(openfda.get("generic_name"))
            manufacturer = _first_text(openfda.get("manufacturer_name"))
            substance = _first_text(openfda.get("substance_name"))
            purpose = _first_text(item.get("purpose"))
            indications = _first_text(item.get("indications_and_usage"))

            title = brand_name or generic_name or substance or f"FDA drug: {query}"
            content_parts = []
            if manufacturer:
                content_parts.append(manufacturer)
            if substance:
                content_parts.append(f"Active: {substance}")
            if purpose:
                content_parts.append(purpose)
            if indications:
                content_parts.append(indications[:120])
            content = " — ".join(content_parts) if content_parts else "FDA drug labeling information"

            payload = build_payload(
                DOMAIN_BIOMEDICAL,
                "drug_label",
                {
                    "brand_name": brand_name or None,
                    "generic_name": generic_name or None,
                    "manufacturer": manufacturer or None,
                    "substance": substance or None,
                    "purpose": purpose or None,
                    "indications": indications or None,
                    "set_id": _first_text(item.get("set_id")) or None,
                    "spl_set_id": _first_text(item.get("spl_set_id"))
                    or _first_text(openfda.get("spl_set_id"))
                    or None,
                    "spl_id": _first_text(item.get("spl_id")) or _first_text(openfda.get("spl_id")) or None,
                    "id": _first_text(item.get("id")) or None,
                },
                engine=self.name,
            )

            results.append(
                SearchResult(
                    url=_record_url(item) or "",
                    title=title,
                    content=content[:500],
                    engine=self.name,
                    position=position,
                    score={3: 1.0, 2: 0.75, 1: 0.5, 0: 0.25}[match_rank],
                    payload=payload,
                ),
            )
        return results, omitted
