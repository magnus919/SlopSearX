"""CVE Program adapter — authoritative CVE Record lookup from MITRE.

No API key required. Uses the public CVE Services API single-record
endpoint (https://cveawg.mitre.org/api/cve/{id}).

Only processes queries that contain a CVE ID (CVE-YYYY-NNNN).
Plain keyword queries return empty results gracefully — the public
CVE Services API does not expose a keyword search endpoint.
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
class CVEAdapter(EngineAdapter):
    """CVE Program lookup — authoritative CVE Record from MITRE."""

    name = "cve"
    display_name = "CVE Program (MITRE)"
    env_prefix = "ENGINE_CVE"
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
        base_url = cfg.get("base_url", "https://cveawg.mitre.org/api")
        timeout_ms = cfg.get("timeout_ms", 10_000)

        # Only respond to queries with a CVE ID
        match = _CVE_ID_PATTERN.search(query)
        if not match:
            return AdapterResponse(results=[], status=EngineStatus.OK)

        cve_id = match.group(1).upper()
        url = f"{base_url}/cve/{cve_id}"

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(url)
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(
                        results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency,
                    )
                if resp.status_code == 403:
                    return AdapterResponse(
                        results=[], status=EngineStatus.BLOCKED, latency_ms=latency,
                    )
                if resp.status_code == 404:
                    return AdapterResponse(
                        results=[], status=EngineStatus.OK, latency_ms=latency,
                    )
                resp.raise_for_status()

                data = resp.json()
                results = self._parse_cve_record(data, cve_id)
                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR, error_message=str(exc), latency_ms=latency,
            )

    def _parse_cve_record(self, data: dict[str, Any], cve_id: str) -> list[SearchResult]:
        """Parse a single CVE Record (JSON 5.2 format) into a SearchResult."""
        # Extract description from the CNA container or ADP containers
        desc_text = ""
        containers = data.get("containers", {})

        # Try cna container first
        cna = containers.get("cna", {})
        if cna:
            desc_text = self._extract_description(cna)

        # Fall back to ADP containers if no CNA description
        if not desc_text:
            adp_list = containers.get("adp", []) or []
            for adp in adp_list:
                desc_text = self._extract_description(adp)
                if desc_text:
                    break

        # Extract references
        refs = cna.get("references", []) if cna else []
        ref_urls = [r.get("url", "") for r in refs if isinstance(r, dict) and r.get("url")]

        # Extract CVSS from providerMetadata / metrics
        metrics_text = self._extract_metrics(cna)

        # Build content
        content_parts = [desc_text[:500]] if desc_text else ["(No description available)"]
        if metrics_text:
            content_parts.append(metrics_text)
        if ref_urls:
            refs_str = "Refs: " + " | ".join(ref_urls[:3])
            if len(ref_urls) > 3:
                refs_str += f" (+{len(ref_urls) - 3} more)"
            content_parts.append(refs_str)

        # Published date
        published_date = None
        if cna and "datePublic" in cna:
            published_date = cna["datePublic"]
        if not published_date and data.get("cveId") == cve_id:
            published_date = data.get("datePublished") or data.get("dateUpdated")

        return [
            SearchResult(
                url=f"https://nvd.nist.gov/vuln/detail/{cve_id}",
                title=cve_id,
                content=" | ".join(content_parts),
                engine=self.name,
                position=1,
                published_date=published_date[:10] if published_date and len(published_date) >= 10 else published_date,
            ),
        ]

    def _extract_description(self, container: dict[str, Any]) -> str:
        """Extract English description from a container."""
        descs: list[dict[str, Any]] = container.get("descriptions", []) or []
        for d in descs:
            if isinstance(d, dict) and d.get("lang") == "en":
                return str(d.get("value", ""))
        # Fall back to first description
        for d in descs:
            if isinstance(d, dict):
                return str(d.get("value", ""))
        return ""

    def _extract_metrics(self, container: dict[str, Any]) -> str:
        """Extract CVSS metrics text from a container."""
        metrics_list = container.get("metrics", []) if container else []
        parts = []
        for m in metrics_list:
            if isinstance(m, dict):
                for key in ("cvssV3_1", "cvssV3_0", "cvssV2_0"):
                    cvss = m.get(key, {})
                    if isinstance(cvss, dict):
                        vs = cvss.get("vectorString", "")
                        bs = cvss.get("baseScore", "")
                        if bs is not None and bs != "":
                            parts.append(f"CVSS {bs}")
                        if vs:
                            parts.append(vs)
                        if parts:
                            break
                if parts:
                    # Only take the first metric set
                    break
        return " | ".join(parts) if parts else ""
