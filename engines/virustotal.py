"""VirusTotal adapter — file hash, URL, domain, and IP reputation.

Requires ENGINE_VIRUSTOTAL_API_KEY.
Free tier: 500 requests/day, 4 req/min.
API docs: https://developers.virustotal.com/reference
"""

from __future__ import annotations

import re
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

_HASH_PATTERN = re.compile(r"\b[a-fA-F0-9]{32}\b|\b[a-fA-F0-9]{40}\b|\b[a-fA-F0-9]{64}\b")
_IP_PATTERN = re.compile(r"\b(?:\d{1,3}\.){3}\d{1,3}\b")


@register_engine
class VirusTotalAdapter(EngineAdapter):
    """VirusTotal — multi-engine malware detection."""

    name = "virustotal"
    display_name = "VirusTotal"
    env_prefix = "ENGINE_VIRUSTOTAL"
    engine_type = "api"
    categories = ["security", "malware"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        api_key = cfg.get("api_key") or ""
        base_url = cfg.get("base_url", "https://www.virustotal.com/api/v3")
        timeout_ms = cfg.get("timeout_ms", 10_000)
        max_results = cfg.get("max_results", 10)

        if not api_key:
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message="VirusTotal API key not configured (set ENGINE_VIRUSTOTAL_API_KEY)",
            )

        headers = {
            "Accept": "application/json",
            "x-apikey": api_key,
        }

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    f"{base_url}/search",
                    headers=headers,
                    params={"query": query, "limit": max_results},
                )
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                if resp.status_code == 403:
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                resp.raise_for_status()

                data = resp.json()
                results = self._parse_search(data.get("data", []))
                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
                latency_ms=latency,
            )

    def _parse_search(self, items: list[dict[str, Any]]) -> list[SearchResult]:
        results: list[SearchResult] = []
        for i, item in enumerate(items):
            attrs = item.get("attributes", {})
            ioc_id = item.get("id", "")

            stats = attrs.get("last_analysis_stats", {})
            malicious = stats.get("malicious", 0)
            suspicious = stats.get("suspicious", 0)
            total = sum(stats.values()) if stats else 0

            title = ioc_id
            if attrs.get("meaningful_name"):
                title = attrs["meaningful_name"]
            elif attrs.get("type_description"):
                title = f"{attrs['type_description']}: {ioc_id[:32]}"

            content_parts = []
            if total > 0:
                content_parts.append(f"Detection: {malicious}/{total} engines ({suspicious} suspicious)")
            else:
                content_parts.append("Detection: unknown")

            if attrs.get("type_description"):
                content_parts.append(f"Type: {attrs['type_description']}")

            rep = attrs.get("reputation", 0)
            if rep:
                content_parts.append(f"Reputation: {rep}")

            content_parts.append(f"VT link: https://virustotal.com/gui/search/{ioc_id}")

            results.append(
                SearchResult(
                    url=f"https://virustotal.com/gui/search/{ioc_id}",
                    title=title,
                    content=" | ".join(content_parts),
                    engine=self.name,
                    position=i + 1,
                    score=float(malicious),
                ),
            )
        return results
