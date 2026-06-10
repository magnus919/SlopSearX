"""Censys adapter — internet asset discovery.

Requires ENGINE_CENSYS_API_KEY and ENGINE_CENSYS_API_SECRET.
Free tier: 250 queries/month.
API docs: https://docs.censys.io/api-v2/
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
class CensysAdapter(EngineAdapter):
    """Censys — internet asset and certificate discovery."""

    name = "censys"
    display_name = "Censys"
    env_prefix = "ENGINE_CENSYS"
    engine_type = "api"
    categories = ["it", "security"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if (early := await self._check_rate_limit()):
            return early

        cfg = self.config
        api_id = cfg.get("api_key") or ""
        api_secret = cfg.get("api_secret") or ""
        base_url = cfg.get("base_url", "https://search.censys.io/api/v2")
        timeout_ms = cfg.get("timeout_ms", 10_000)
        max_results = cfg.get("max_results", 10)

        if not api_id or not api_secret:
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR,
                error_message=(
                    "Censys API credentials not configured"
                    " (set ENGINE_CENSYS_API_KEY and ENGINE_CENSYS_API_SECRET)"
                ),
            )

        headers = {"Accept": "application/json"}

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    f"{base_url}/hosts/search",
                    headers=headers,
                    auth=(api_id, api_secret),
                    params={"q": query, "per_page": max_results},
                )
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                if resp.status_code == 403:
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                resp.raise_for_status()

                data = resp.json()
                result_field = data.get("result", {})
                hits = result_field.get("hits", []) if isinstance(result_field, dict) else []
                results = self._parse_hits(hits)
                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR, error_message=str(exc), latency_ms=latency,
            )

    def _parse_hits(self, hits: list[dict[str, Any]]) -> list[SearchResult]:
        results: list[SearchResult] = []
        for i, hit in enumerate(hits):
            ip = hit.get("ip", "")
            location = hit.get("location", {}) or {}
            services = hit.get("services", []) or []
            asn = location.get("asn", "")
            country = location.get("country", "")
            city = location.get("city", "")

            service_str = ", ".join(
                f"{s.get('service_name', '')}:{s.get('port', '')}" for s in services[:5]
                if s.get('service_name')
            ) if services else ""

            content_parts = []
            if service_str:
                content_parts.append(service_str)
            if asn:
                content_parts.append(asn)
            if city and country:
                content_parts.append(f"{city}, {country}")
            elif country:
                content_parts.append(country)

            results.append(
                SearchResult(
                    url=f"https://search.censys.io/hosts/{ip}" if ip else "",
                    title=ip,
                    content=" | ".join(content_parts) if content_parts else "Host result",
                    engine=self.name,
                    position=i + 1,
                ),
            )
        return results
