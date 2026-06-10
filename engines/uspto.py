"""USPTO Patents adapter — US patent search via PatentsView API.

Free, public API. No auth required.
Docs: https://patentsview.org/apis/api-endpoints
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
class USPTOAdapter(EngineAdapter):
    """US patent search via PatentsView API."""

    name = "uspto"
    display_name = "USPTO Patents"
    env_prefix = "ENGINE_USPTO"
    engine_type = "api"
    categories = ["general", "reference", "legal", "it"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://api.patentsview.org/patents/query")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        headers = {
            "User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)",
            "Content-Type": "application/json",
            "Accept": "application/json",
        }
        start_time = time.monotonic()

        try:
            query_body = {
                "q": {
                    "_text_any": {"patent_title": query},
                },
                "f": [
                    "patent_id",
                    "patent_title",
                    "patent_date",
                    "patent_type",
                    "patent_abstract",
                    "patent_year",
                ],
                "o": {"per_page": max_results, "page": 1},
            }

            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.post(
                    base_url,
                    json=query_body,
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()
                data = resp.json()

                results = []
                patents = data.get("patents", [])
                for idx, patent in enumerate(patents[:max_results]):
                    patent_id = patent.get("patent_id", "")
                    title = patent.get("patent_title", "")
                    abstract = patent.get("patent_abstract", "") or ""
                    patent_date = patent.get("patent_date", "")
                    patent_type = patent.get("patent_type", "")

                    content = abstract[:300] if abstract else ""
                    if patent_type:
                        content = f"[{patent_type}] {content}".strip()

                    results.append(
                        SearchResult(
                            url=f"https://patents.google.com/patent/US{patent_id}/" if patent_id else "",
                            title=title,
                            content=content[:500],
                            engine=self.name,
                            position=idx + 1,
                            published_date=patent_date if patent_date else None,
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
