"""Internet Archive adapter — web archives, books, audio, video.

Free, public API. No auth required.
Wayback CDX API for web archive queries.
General advancedsearch for books/media queries.
"""

from __future__ import annotations

import re
import urllib.parse
from typing import Any

import httpx

from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult, register_engine


def _is_domain_query(query: str) -> bool:
    """Detect if query looks like a domain name (for Wayback routing)."""
    return bool(re.match(r"^[\w.-]+\.[a-z]{2,}$", query.strip(), re.IGNORECASE))


_CDX_COLUMNS = ("urlkey", "timestamp", "original")


def _valid_cdx_rows(raw: Any) -> bool:
    """A header-only CDX result is empty; malformed records are not."""
    return (
        isinstance(raw, list)
        and bool(raw)
        and isinstance(raw[0], list)
        and raw[0][:3] == list(_CDX_COLUMNS)
        and all(
            isinstance(row, list)
            and len(row) >= 3
            and isinstance(row[1], str)
            and re.fullmatch(r"\d{14}", row[1]) is not None
            and isinstance(row[2], str)
            and row[2].startswith(("http://", "https://"))
            for row in raw[1:]
        )
    )


@register_engine
class InternetArchiveAdapter(EngineAdapter):
    """Internet Archive search — Wayback Machine, books, media.

    Routes domain-looking queries to the Wayback CDX API.
    Routes everything else to the general archive advancedsearch.
    """

    name = "internetarchive"
    display_name = "Internet Archive"
    env_prefix = "ENGINE_INTERNETARCHIVE"
    engine_type = "api"
    categories = ["reference", "web:archive", "historical"]

    # -- Declared capability metadata (audited, issue 185) --
    supported_result_types = ("text",)
    failure_classes = ("error", "timeout")
    cost_class = "free"

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:

        if _is_domain_query(query):
            return await self._search_wayback(query, params)

        return await self._search_archive(query, params)

    async def _search_wayback(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        """Search Wayback Machine CDX API for domain snapshots."""
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        timeout_ms = cfg.get("timeout_ms", 10_000)
        max_results = cfg.get("max_results", 10)
        domain = query.strip()

        url = (
            f"https://web.archive.org/cdx/search/cdx"
            f"?url={urllib.parse.quote(domain)}"
            f"&output=json"
            f"&limit={max_results}"
            f"&filter=statuscode:200"
            f"&collapse=timestamp:6"  # one snapshot per month
        )

        try:
            async with self.http_client(timeout=timeout_ms / 1000) as client:
                resp = await client.get(url)
                resp.raise_for_status()
                raw = resp.json()
        except httpx.TimeoutException:
            return AdapterResponse(
                results=[],
                status=EngineStatus.TIMEOUT,
                error_message="Internet Archive CDX request timed out",
            )
        except Exception:
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message="Internet Archive CDX request failed",
            )

        if not _valid_cdx_rows(raw):
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message="Invalid Internet Archive CDX response",
            )

        # CDX returns: [urlkey, timestamp, original, mimetype, statuscode, digest, length]
        results = []
        for row in raw[1:]:  # skip header row
            if len(row) < 3:
                continue
            ts = row[1]
            original_url = row[2]
            wayback_url = f"https://web.archive.org/web/{ts}/{original_url}"
            mime = row[3] if len(row) > 3 else ""

            results.append(
                SearchResult(
                    url=wayback_url,
                    title=f"{domain} — {ts[:4]}-{ts[4:6]}-{ts[6:8]} ({mime})",
                    content=f"Archived snapshot of {original_url} from {ts[:4]}-{ts[4:6]}-{ts[6:8]}",
                    engine=self.name,
                    score=0.5,
                    published_date=f"{ts[:4]}-{ts[4:6]}-{ts[6:8]}",
                )
            )

        return AdapterResponse(results=results, status=EngineStatus.OK)

    async def _search_archive(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        """Search general Internet Archive catalog."""
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        timeout_ms = cfg.get("timeout_ms", 10_000)
        max_results = cfg.get("max_results", 10)
        base_url = cfg.get("base_url", "https://archive.org")

        url = f"{base_url}/advancedsearch.php?q={urllib.parse.quote(query)}&output=json&rows={max_results}"

        try:
            async with self.http_client(timeout=timeout_ms / 1000) as client:
                resp = await client.get(url)
                resp.raise_for_status()
                data = resp.json()
        except httpx.TimeoutException:
            return AdapterResponse(
                results=[],
                status=EngineStatus.TIMEOUT,
                error_message="Internet Archive catalog request timed out",
            )
        except Exception:
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message="Internet Archive catalog request failed",
            )

        docs = data.get("response", {}).get("docs", [])
        results = []
        for doc in docs[:max_results]:
            identifier = doc.get("identifier", "")
            results.append(
                SearchResult(
                    url=f"https://archive.org/details/{identifier}" if identifier else "",
                    title=doc.get("title", ""),
                    content=(doc.get("description") or "")[:500],
                    engine=self.name,
                    score=float(doc.get("downloads", 0)) / 10000.0,
                    published_date=doc.get("date"),
                )
            )

        return AdapterResponse(results=results, status=EngineStatus.OK)
