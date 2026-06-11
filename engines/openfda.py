"""openFDA adapter — FDA drug, device, and food recall data.

Free, public JSON API. No auth required.
Docs: https://open.fda.gov/apis/
"""

from __future__ import annotations

import time
from typing import Any

import httpx

from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult, register_engine


@register_engine
class OpenFDAAdapter(EngineAdapter):
    """FDA drug and device data search via openFDA."""

    name = "openfda"
    display_name = "openFDA"
    env_prefix = "ENGINE_OPENFDA"
    engine_type = "api"
    categories = ["medical", "health", "science", "government"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if (early := await self._check_rate_limit()):
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://api.fda.gov/drug/label.json")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    base_url,
                    params={"search": query, "limit": max_results},
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()
                data = resp.json()

                results = []
                items = data.get("results", [])
                for idx, item in enumerate(items[:max_results]):
                    openfda = item.get("openfda", {}) or {}
                    brand_name = (openfda.get("brand_name") or [None])[0] or ""
                    generic_name = (openfda.get("generic_name") or [None])[0] or ""
                    manufacturer = (openfda.get("manufacturer_name") or [None])[0] or ""
                    substance = (openfda.get("substance_name") or [None])[0] or ""
                    purpose = (item.get("purpose") or [""])[0] or ""
                    indications = (item.get("indications_and_usage") or [""])[0] or ""

                    title = brand_name or generic_name or f"FDA drug: {query}"
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

                    url_suffix = f"?query={query}" if not brand_name else f"#{brand_name.lower().replace(' ','-')}"

                    results.append(
                        SearchResult(
                            url=f"https://open.fda.gov/drug/label/{url_suffix}",
                            title=title,
                            content=content[:500],
                            engine=self.name,
                            position=idx + 1,
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
