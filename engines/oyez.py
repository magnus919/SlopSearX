"""Oyez adapter — US Supreme Court case search.

Free, public REST API. No auth required.
Docs: https://www.oyez.org/api-docs
"""

from __future__ import annotations

import time
from typing import Any

import httpx

from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult, register_engine


@register_engine
class OyezAdapter(EngineAdapter):
    """US Supreme Court case search via Oyez API."""

    name = "oyez"
    display_name = "Oyez (SCOTUS)"
    env_prefix = "ENGINE_OYEZ"
    engine_type = "api"
    categories = ["reference", "legal"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://api.oyez.org")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    f"{base_url}/cases",
                    params={
                        "page": 1,
                        "per_page": max_results,
                        "order": "desc",
                        "sort": "decision_date",
                        "name": query,
                    },
                )
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 404:
                    return AdapterResponse(results=[], status=EngineStatus.OK, latency_ms=latency)

                resp.raise_for_status()
                data = resp.json()

                results = []
                cases = data if isinstance(data, list) else data.get("results", data.get("data", []))
                for idx, case in enumerate(cases[:max_results]):
                    name = case.get("name", "") or case.get("title", "")
                    term = case.get("term", "")
                    decision_date = case.get("decision_date", "")
                    href = case.get("href", "")
                    oyez_id = href.rstrip("/").split("/")[-1] if href else ""

                    content = f"Supreme Court term: {term}" if term else ""
                    if decision_date:
                        content = f"Decided: {decision_date}" + (f" | {content}" if content else "")

                    results.append(
                        SearchResult(
                            url=f"https://www.oyez.org/cases/{oyez_id}" if oyez_id else href or "",
                            title=name,
                            content=content[:500] if content else "U.S. Supreme Court case",
                            engine=self.name,
                            position=idx + 1,
                            published_date=decision_date if decision_date else None,
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
            return AdapterResponse(results=[], status=EngineStatus.ERROR, error_message=str(exc), latency_ms=latency)
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
                latency_ms=latency,
            )
