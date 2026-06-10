"""AbuseIPDB adapter — IP address reputation database.

Requires ENGINE_ABUSEIPDB_API_KEY.
Free tier: 1000 checks/day.
API docs: https://docs.abuseipdb.com/
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


@register_engine
class AbuseIPDBAdapter(EngineAdapter):
    """AbuseIPDB — IP reputation and abuse reporting."""

    name = "abuseipdb"
    display_name = "AbuseIPDB"
    env_prefix = "ENGINE_ABUSEIPDB"
    engine_type = "api"
    categories = ["general", "security", "threat-intel"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if (early := await self._check_rate_limit()):
            return early

        cfg = self.config
        api_key = cfg.get("api_key") or ""
        base_url = cfg.get("base_url", "https://api.abuseipdb.com/api/v2")
        timeout_ms = cfg.get("timeout_ms", 10_000)

        if not api_key:
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR,
                error_message="AbuseIPDB API key not configured (set ENGINE_ABUSEIPDB_API_KEY)",
            )

        ip_match = _IP_PATTERN.search(query)
        ip = ip_match.group(0) if ip_match else query.strip()

        headers = {
            "Accept": "application/json",
            "Key": api_key,
        }

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    f"{base_url}/check",
                    headers=headers,
                    params={"ipAddress": ip, "maxAgeInDays": "90", "verbose": ""},
                )
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                if resp.status_code == 403:
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                resp.raise_for_status()

                data = resp.json().get("data", {})
                results = self._parse_ip(data, ip)
                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR, error_message=str(exc), latency_ms=latency,
            )

    def _parse_ip(self, data: dict[str, Any], ip: str) -> list[SearchResult]:
        if not data:
            return []

        confidence = data.get("abuseConfidenceScore", 0)
        total_reports = data.get("totalReports", 0)
        last_reported = data.get("lastReportedAt", "")
        country = data.get("countryCode", "")
        isp = data.get("isp", "")
        domain = data.get("domain", "")
        usage = data.get("usageType", "")
        categories_raw = data.get("reports", []) if isinstance(data.get("reports"), list) else []

        unique_categories = set()
        for report in categories_raw:
            raw_cats = report.get("categories", []) if isinstance(report, dict) else []
            for c in raw_cats:
                unique_categories.add(str(c))

        content_parts = []
        content_parts.append(f"Abuse Confidence: {confidence}%")
        if total_reports:
            content_parts.append(f"Reports: {total_reports}")
        if unique_categories:
            content_parts.append(f"Categories: {', '.join(sorted(unique_categories)[:5])}")
        if country:
            content_parts.append(country)
        if isp:
            content_parts.append(isp)
        if domain:
            content_parts.append(domain)
        if usage:
            content_parts.append(usage)
        if last_reported:
            content_parts.append(f"Last reported: {last_reported[:10]}")

        return [
            SearchResult(
                url=f"https://www.abuseipdb.com/check/{ip}",
                title=ip,
                content=" | ".join(content_parts),
                engine=self.name,
                position=1,
                score=float(confidence) / 100.0,
            ),
        ]
