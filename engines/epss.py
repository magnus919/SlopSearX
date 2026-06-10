"""FIRST EPSS adapter — Exploit Prediction Scoring System.

Free API, no key required. Returns the probability that a CVE will be
exploited in the next 30 days as a score between 0 and 1.
API docs: https://www.first.org/epss/api
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

_CVE_ID_PATTERN = re.compile(r"(CVE-\d{4}-\d{4,})", re.IGNORECASE)


@register_engine
class EPSSAdapter(EngineAdapter):
    """FIRST EPSS — exploit probability scoring for CVEs."""

    name = "epss"
    display_name = "FIRST EPSS (Exploit Prediction)"
    env_prefix = "ENGINE_EPSS"
    engine_type = "api"
    categories = ["security", "threat-intel"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if (early := await self._check_rate_limit()):
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://api.first.org/data/v1/epss")
        timeout_ms = cfg.get("timeout_ms", 10_000)

        match = _CVE_ID_PATTERN.search(query)
        if not match:
            return AdapterResponse(results=[], status=EngineStatus.OK)

        cve_id = match.group(1).upper()

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(base_url, params={"cve": cve_id})
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                if resp.status_code == 403:
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                resp.raise_for_status()

                data = resp.json()
                results = self._parse_epss(data.get("data", []), cve_id)
                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR, error_message=str(exc), latency_ms=latency,
            )

    def _parse_epss(self, epss_data: list[dict[str, Any]], cve_id: str) -> list[SearchResult]:
        if not epss_data:
            return []

        entry = epss_data[0]
        score = float(entry.get("epss", 0))
        percentile = float(entry.get("percentile", 0))
        epss_date = entry.get("date", "")

        # Build a human-readable probability description
        prob_level = "Very Low"
        if score >= 0.9:
            prob_level = "Critical"
        elif score >= 0.7:
            prob_level = "High"
        elif score >= 0.4:
            prob_level = "Moderate"
        elif score >= 0.1:
            prob_level = "Low"

        content_parts = [
            f"EPSS Score: {score:.5f} ({prob_level})",
            f"Percentile: {percentile:.2f}",
            f"Date: {epss_date[:10] if epss_date else 'N/A'}",
        ]

        return [
            SearchResult(
                url=f"https://www.first.org/epss/api?cve={cve_id}",
                title=f"{cve_id} — EPSS {score:.5f}",
                content=" | ".join(content_parts),
                engine=self.name,
                position=1,
                score=score * 1000,
            ),
        ]
