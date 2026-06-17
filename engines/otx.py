"""AlienVault OTX adapter — Open Threat Exchange intelligence.

Requires ENGINE_OTX_API_KEY.
Free tier: unlimited for community members.
API docs: https://otx.alienvault.com/api
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

_IP_PATTERN = re.compile(r"\b(?:\d{1,3}\.){3}\d{1,3}\b")
_HASH_PATTERN = re.compile(r"\b[a-fA-F0-9]{32}\b|\b[a-fA-F0-9]{40}\b|\b[a-fA-F0-9]{64}\b")
_CVE_PATTERN = re.compile(r"CVE-\d{4}-\d{4,}", re.IGNORECASE)
_DOMAIN_PATTERN = re.compile(r"\b(?:[a-zA-Z0-9-]+\.)+[a-zA-Z]{2,}\b")


@register_engine
class OTXAdapter(EngineAdapter):
    """AlienVault OTX — community threat intelligence."""

    name = "otx"
    display_name = "AlienVault OTX"
    env_prefix = "ENGINE_OTX"
    engine_type = "api"
    categories = ["security", "threat-intel"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        api_key = cfg.get("api_key") or ""
        base_url = cfg.get("base_url", "https://otx.alienvault.com/api/v1")
        timeout_ms = cfg.get("timeout_ms", 10_000)
        max_results = cfg.get("max_results", 10)

        if not api_key:
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message="OTX API key not configured (set ENGINE_OTX_API_KEY)",
            )

        headers = {
            "Accept": "application/json",
            "X-OTX-API-KEY": api_key,
        }

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                # Detect indicator type
                ip_match = _IP_PATTERN.search(query)
                hash_match = _HASH_PATTERN.search(query)
                cve_match = _CVE_PATTERN.search(query)

                if ip_match:
                    indicator_type = "IPv4"
                    indicator = ip_match.group(0)
                elif hash_match:
                    indicator_type = "file"
                    indicator = hash_match.group(0)
                elif cve_match:
                    indicator_type = "CVE"
                    indicator = cve_match.group(0).upper()
                else:
                    # Keyword search across pulses
                    indicator_type = "pulses"
                    indicator = ""

                if indicator_type == "pulses":
                    resp = await client.get(
                        f"{base_url}/pulses/subscribed",
                        headers=headers,
                        params={"q": query, "limit": max_results},
                    )
                else:
                    resp = await client.get(
                        f"{base_url}/indicators/{indicator_type}/{indicator}/general",
                        headers=headers,
                    )

                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                if resp.status_code == 403:
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                resp.raise_for_status()

                data = resp.json()
                if indicator_type == "pulses":
                    results = self._parse_pulses(data.get("results", [])[:max_results])
                else:
                    results = self._parse_indicator(data, indicator)

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

    def _parse_indicator(self, data: dict[str, Any], indicator: str) -> list[SearchResult]:
        base_score = data.get("base_score", 0)
        pulse_count = data.get("pulse_info", {}).get("count", 0) if isinstance(data.get("pulse_info"), dict) else 0
        tags = data.get("tags", [])
        country = data.get("country_code", "")
        asn = data.get("asn", "")
        reputation = data.get("reputation", 0)

        content_parts = []
        if base_score:
            content_parts.append(f"Score: {base_score}")
        if pulse_count:
            content_parts.append(f"Pulses: {pulse_count}")
        if tags:
            content_parts.append(f"Tags: {', '.join(tags[:5])}")
        if reputation:
            content_parts.append(f"Reputation: {reputation}")
        if country:
            content_parts.append(country)
        if asn:
            content_parts.append(asn)

        return [
            SearchResult(
                url=f"https://otx.alienvault.com/indicator/ip/{indicator}",
                title=indicator,
                content=" | ".join(content_parts) if content_parts else "OTX indicator result",
                engine=self.name,
                position=1,
                score=float(base_score) if base_score else 0.0,
            ),
        ]

    def _parse_pulses(self, pulses: list[dict[str, Any]]) -> list[SearchResult]:
        results: list[SearchResult] = []
        for i, pulse in enumerate(pulses):
            name = pulse.get("name", "")
            desc = pulse.get("description", "")[:300]
            tags = pulse.get("tags", [])
            created = pulse.get("created", "")
            ioc_count = pulse.get("indicator_count", 0)

            content_parts = [desc] if desc else []
            if tags:
                content_parts.append(f"Tags: {', '.join(tags[:5])}")
            if ioc_count:
                content_parts.append(f"IOCs: {ioc_count}")

            pulse_id = pulse.get("id", "")
            results.append(
                SearchResult(
                    url=f"https://otx.alienvault.com/pulse/{pulse_id}" if pulse_id else "",
                    title=name,
                    content=" | ".join(content_parts),
                    engine=self.name,
                    position=i + 1,
                    published_date=created[:10] if created and len(created) >= 10 else None,
                ),
            )
        return results
