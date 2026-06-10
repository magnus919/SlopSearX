"""Shodan adapter — internet-connected device search.

Requires ENGINE_SHODAN_API_KEY for all queries.
Free tier available with usage limits.
API docs: https://developer.shodan.io/api
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
class ShodanAdapter(EngineAdapter):
    """Shodan — internet device search engine."""

    name = "shodan"
    display_name = "Shodan"
    env_prefix = "ENGINE_SHODAN"
    engine_type = "api"
    categories = ["general", "it", "security"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if (early := await self._check_rate_limit()):
            return early

        cfg = self.config
        api_key = cfg.get("api_key") or ""
        base_url = cfg.get("base_url", "https://api.shodan.io")
        timeout_ms = cfg.get("timeout_ms", 10_000)
        max_results = cfg.get("max_results", 10)

        if not api_key:
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR,
                error_message="Shodan API key not configured (set ENGINE_SHODAN_API_KEY)",
            )

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    f"{base_url}/shodan/host/search",
                    params={"key": api_key, "query": query, "limit": max_results},
                )
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                if resp.status_code == 403:
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                resp.raise_for_status()

                data = resp.json()
                results = self._parse_matches(data.get("matches", [])[:max_results])
                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR, error_message=str(exc), latency_ms=latency,
            )

    def _parse_matches(self, matches: list[dict[str, Any]]) -> list[SearchResult]:
        results: list[SearchResult] = []
        for i, m in enumerate(matches):
            ip = m.get("ip_str", "")
            port = m.get("port", "")
            org = m.get("org", "")
            hostnames = ", ".join(m.get("hostnames", [])[:3])
            product = m.get("product", "")
            version = m.get("version", "")
            transport = m.get("transport", "")
            cvss = m.get("cvss", 0)
            vulns = m.get("vulns", [])

            title = f"{ip}:{port}" if port else ip
            content_parts = []
            if product:
                content_parts.append(f"{product} {version}" if version else product)
            if transport:
                content_parts.append(transport)
            if org:
                content_parts.append(org)
            if hostnames:
                content_parts.append(hostnames)
            if cvss:
                content_parts.append(f"CVSS {cvss}")
            if vulns:
                content_parts.append(f"CVEs: {', '.join(list(vulns)[:5])}")

            results.append(
                SearchResult(
                    url=f"https://www.shodan.io/host/{ip}" if ip else "",
                    title=title,
                    content=" | ".join(content_parts) if content_parts else "Device match",
                    engine=self.name,
                    position=i + 1,
                    score=float(cvss) if cvss else 0.0,
                ),
            )
        return results
