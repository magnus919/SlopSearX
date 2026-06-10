"""SEC EDGAR adapter — corporate filings search.

Free, public API. No auth required, but rate limits apply.
Docs: https://efts.sec.gov/LATEST/search-index
     https://www.sec.gov/edgar/search/
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
class EdgarAdapter(EngineAdapter):
    """SEC EDGAR corporate filings search."""

    name = "edgar"
    display_name = "SEC EDGAR"
    env_prefix = "ENGINE_EDGAR"
    engine_type = "api"
    categories = ["general", "finance", "reference"]

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
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    base_url,
                    params={"q": query, "limit": max_results, "offset": 0},
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()
                data = resp.json()

                results = []
                hits = data.get("hits", {}).get("hits", [])
                for idx, hit in enumerate(hits[:max_results]):
                    src = hit.get("_source", {})
                    filing_name = src.get("display_names", [None])
                    name = filing_name[0] if filing_name else src.get("entity_name", "")
                    form_type = src.get("form_type", "")
                    description = src.get("description", "") or ""
                    filed_at = src.get("filed_at", "")
                    cik = src.get("cik", "")

                    content_parts = [form_type] if form_type else []
                    if description:
                        content_parts.append(description)
                    content = " — ".join(content_parts) if content_parts else "SEC filing"

                    results.append(
                        SearchResult(
                            url=f"https://www.sec.gov/cgi-bin/browse-edgar?action=getcompany&CIK={cik}" if cik else "",
                            title=f"{name} — {form_type}" if form_type else name,
                            content=content[:500],
                            engine=self.name,
                            position=idx + 1,
                            published_date=filed_at[:10] if filed_at else None,
                            score=hit.get("_score", 0) or 0.0,
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
