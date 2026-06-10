"""CourtListener adapter — US legal opinion search via Free Law Project.

Free, public REST API. Rate-limited without a token but functional.
Docs: https://www.courtlistener.com/help/api/
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
class CourtListenerAdapter(EngineAdapter):
    """US legal opinion search via CourtListener."""

    name = "courtlistener"
    display_name = "CourtListener"
    env_prefix = "ENGINE_COURTLISTENER"
    engine_type = "api"
    categories = ["general", "reference", "legal"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://www.courtlistener.com/api/rest/v3/opinions")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        headers = {
            "User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)",
        }

        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    base_url,
                    params={
                        "search": query,
                        "page_size": max_results,
                        "format": "json",
                        "order_by": "score",
                    },
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()
                data = resp.json()

                results = []
                opinions = data.get("results", [])
                for idx, opinion in enumerate(opinions[:max_results]):
                    title = opinion.get("caseName", "") or ""
                    court = opinion.get("court", "")
                    court_name = opinion.get("court_string", "") or court
                    date_filed = opinion.get("dateFiled", "") or ""
                    citation = opinion.get("citation", [])
                    citation_str = ", ".join(citation) if citation else ""
                    absolute_url = opinion.get("absolute_url", "")
                    plain_text = (opinion.get("plain_text", "") or "")[:300]

                    content_parts = []
                    if court_name:
                        content_parts.append(court_name)
                    if citation_str:
                        content_parts.append(citation_str)
                    if date_filed:
                        content_parts.append(date_filed)
                    if plain_text:
                        content_parts.append(plain_text)

                    results.append(
                        SearchResult(
                            url=f"https://www.courtlistener.com{absolute_url}" if absolute_url else "",
                            title=title,
                            content=" — ".join(content_parts)[:500],
                            engine=self.name,
                            position=idx + 1,
                            published_date=date_filed if date_filed else None,
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
