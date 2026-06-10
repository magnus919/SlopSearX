"""VulnCheck Community adapter — exploit intelligence and CVE timeline.

Requires ENGINE_VULNCHECK_API_KEY.
Free community tier available.
API docs: https://docs.vulncheck.com/
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

_CVE_PATTERN = re.compile(r"(CVE-\d{4}-\d{4,})", re.IGNORECASE)


@register_engine
class VulnCheckAdapter(EngineAdapter):
    """VulnCheck Community — exploit intelligence and CVE timeline."""

    name = "vulncheck"
    display_name = "VulnCheck Community"
    env_prefix = "ENGINE_VULNCHECK"
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
        base_url = cfg.get("base_url", "https://api.vulncheck.com/v3")
        timeout_ms = cfg.get("timeout_ms", 10_000)

        if not api_key:
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR,
                error_message="VulnCheck API key not configured (set ENGINE_VULNCHECK_API_KEY)",
            )

        match = _CVE_PATTERN.search(query)
        if not match:
            return AdapterResponse(results=[], status=EngineStatus.OK)

        cve_id = match.group(1).upper()
        headers = {"Accept": "application/json", "Authorization": f"Bearer {api_key}"}

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    f"{base_url}/community/cve/{cve_id}",
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                if resp.status_code == 403:
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                resp.raise_for_status()

                data = resp.json()
                results = self._parse_cve(data.get("data", {}), cve_id)
                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR, error_message=str(exc), latency_ms=latency,
            )

    def _parse_cve(self, data: dict[str, Any], cve_id: str) -> list[SearchResult]:
        if not data:
            return []

        exploit_state = data.get("exploit_state", "")
        date_added = data.get("date_added", "") or data.get("vulncheck_date_added", "")
        vendor_data = data.get("vendor_data", []) or []
        exploit_urls = data.get("exploit_urls", []) or []
        cpe_entries = data.get("cpe", []) or []

        content_parts = []
        if exploit_state:
            content_parts.append(f"Exploit State: {exploit_state}")
        if date_added:
            content_parts.append(f"Added: {date_added[:10]}")
        if vendor_data:
            vendors = set()
            for v in vendor_data[:5]:
                if isinstance(v, dict):
                    vendors.add(v.get("vendor", ""))
            if vendors:
                content_parts.append(f"Affected: {', '.join(sorted(vendors))}")
        if exploit_urls:
            content_parts.append(f"Exploits: {len(exploit_urls)} available")
        if cpe_entries:
            content_parts.append(f"CPEs: {len(cpe_entries)}")

        return [
            SearchResult(
                url=f"https://api.vulncheck.com/v3/community/cve/{cve_id}",
                title=cve_id,
                content=" | ".join(content_parts) if content_parts else "VulnCheck result",
                engine=self.name,
                position=1,
            ),
        ]
