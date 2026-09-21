"""SEC EDGAR adapter — corporate filings search.

Free, public API. No auth required, but rate limits apply.
Docs: https://efts.sec.gov/LATEST/search-index
     https://www.sec.gov/edgar/search/
"""

from __future__ import annotations

import re
import time
from typing import Any
from urllib.parse import urlparse

import httpx

from slopsearx.adapter import (
    AdapterResponse,
    EngineAdapter,
    EngineStatus,
    SearchResult,
    register_engine,
)

_SEC_ARCHIVES_PREFIX = "/Archives/edgar/data/"
_SEC_CIK_RE = re.compile(r"\d{1,10}\Z")
_SEC_ACCESSION_RE = re.compile(r"\d{10}-\d{2}-\d{6}\Z")
_SEC_PATH_PART_RE = re.compile(r"[A-Za-z0-9._-]+\Z")


def _safe_cik(value: Any) -> str | None:
    """Return the SEC archive CIK segment when **value** is trustworthy."""
    if isinstance(value, bool):
        return None
    if isinstance(value, int):
        raw = str(value)
    elif isinstance(value, str):
        raw = value.strip()
    else:
        return None
    if not _SEC_CIK_RE.fullmatch(raw) or int(raw) == 0:
        return None
    return str(int(raw))


def _first_cik(source: dict[str, Any]) -> str | None:
    """Read a CIK from the documented scalar or list-shaped provider field."""
    for value in (source.get("cik"), source.get("ciks")):
        if isinstance(value, (list, tuple)):
            value = value[0] if value else None
        if cik := _safe_cik(value):
            return cik
    return None


def _safe_accession(value: Any) -> tuple[str, str] | None:
    """Return (hyphenated, archive) accession forms for a valid SEC ID."""
    if not isinstance(value, str):
        return None
    accession = value.strip()
    if not _SEC_ACCESSION_RE.fullmatch(accession):
        return None
    return accession, accession.replace("-", "")


def _safe_sec_archive_url(value: Any) -> str | None:
    """Accept only an HTTPS SEC archive URL supplied by the provider."""
    if not isinstance(value, str) or not value.strip():
        return None
    candidate = value.strip()
    try:
        parsed = urlparse(candidate)
        port = parsed.port
    except ValueError:
        return None
    if (
        parsed.scheme.lower() != "https"
        or parsed.hostname != "www.sec.gov"
        or port not in (None, 443)
        or parsed.username is not None
        or parsed.password is not None
        or parsed.query
        or parsed.fragment
        or not parsed.path.startswith(_SEC_ARCHIVES_PREFIX)
    ):
        return None
    path_parts = parsed.path.split("/")[4:]
    if (
        len(path_parts) < 3
        or not _safe_cik(path_parts[0])
        or not re.fullmatch(r"\d{18}\Z", path_parts[1])
        or not all(_SEC_PATH_PART_RE.fullmatch(part) for part in path_parts)
    ):
        return None
    return candidate


def _looks_like_filing(source: dict[str, Any]) -> bool:
    """Identify provider records that cannot safely use a company fallback."""
    return any(isinstance(source.get(field), str) and source[field].strip() for field in ("form_type", "filed_at"))


def _filing_url(hit: dict[str, Any], source: dict[str, Any]) -> str | None:
    """Build a safe, canonical SEC URL from fields present in an EDGAR hit.

    A filing index is distinct whenever CIK and accession are available. The
    primary document is used only when the provider also supplies a safe
    single path segment. Clearly company-level CIK-only records retain a
    useful company page. A filing-looking record without a validated SEC URL
    or filing identity is unresolved and must be omitted by the caller rather
    than receiving a guessed filing URL.
    """
    for record in (source, hit):
        for field in ("filing_url", "file_url", "url", "link", "href"):
            if url := _safe_sec_archive_url(record.get(field)):
                return url

    cik = _first_cik(source)
    accession = None
    for accession_value in (source.get("accession_no"), source.get("adsh")):
        if accession := _safe_accession(accession_value):
            break
    if cik and accession:
        hyphenated, archive_accession = accession
        primary_doc = None
        for field in ("primary_doc", "primaryDocument", "primary_document", "file_name"):
            value = source.get(field)
            if isinstance(value, str) and _SEC_PATH_PART_RE.fullmatch(value.strip()):
                primary_doc = value.strip()
                break
        if primary_doc:
            return f"https://www.sec.gov{_SEC_ARCHIVES_PREFIX}{cik}/{archive_accession}/{primary_doc}"
        return f"https://www.sec.gov{_SEC_ARCHIVES_PREFIX}{cik}/{archive_accession}/{hyphenated}-index.html"
    if cik and not _looks_like_filing(source):
        return f"https://www.sec.gov/cgi-bin/browse-edgar?action=getcompany&CIK={cik}"
    return None


@register_engine
class EdgarAdapter(EngineAdapter):
    """SEC EDGAR corporate filings search."""

    name = "edgar"
    display_name = "SEC EDGAR"
    env_prefix = "ENGINE_EDGAR"
    engine_type = "api"
    categories = ["finance", "reference"]

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
        base_url = cfg.get("base_url", "https://efts.sec.gov/LATEST/search-index")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        headers = {
            "User-Agent": "SlopSearX/0.1.0 (meta search engine; research)",
            "Accept": "application/json, text/plain, */*",
        }
        start_time = time.monotonic()

        try:
            async with self.http_client(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    base_url,
                    params={"q": query, "limit": max_results, "offset": 0},
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()
                data = resp.json()

                results: list[SearchResult] = []
                unresolved = 0
                hits = data.get("hits", {}).get("hits", [])
                for hit in hits[:max_results]:
                    if not isinstance(hit, dict):
                        unresolved += 1
                        continue
                    src = hit.get("_source", {})
                    if not isinstance(src, dict):
                        unresolved += 1
                        continue
                    filing_name = src.get("display_names", [None])
                    name = filing_name[0] if filing_name else src.get("entity_name", "")
                    form_type = src.get("form_type", "")
                    description = src.get("description", "") or ""
                    filed_at = src.get("filed_at", "")
                    url = _filing_url(hit, src)
                    if not url:
                        unresolved += 1
                        continue

                    content_parts = [form_type] if form_type else []
                    if description:
                        content_parts.append(description)
                    content = " — ".join(content_parts) if content_parts else "SEC filing"

                    results.append(
                        SearchResult(
                            url=url,
                            title=f"{name} — {form_type}" if form_type else name,
                            content=content[:500],
                            engine=self.name,
                            position=len(results) + 1,
                            published_date=filed_at[:10] if filed_at else None,
                            score=hit.get("_score", 0) or 0.0,
                        ),
                    )

                message = (
                    f"{unresolved} SEC result(s) omitted because no validated SEC URL or filing identity was available"
                    if unresolved
                    else None
                )
                return AdapterResponse(
                    results=results,
                    status=EngineStatus.OK,
                    error_message=message,
                    latency_ms=latency,
                )

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
