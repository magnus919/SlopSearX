"""NVD (National Vulnerability Database) adapter — CVE metadata with CVSS scores.

Free tier (no API key): ~5 requests per 30 seconds.
With API key (ENGINE_NVD_API_KEY): ~50 requests per 30 seconds.
API docs: https://nvd.nist.gov/developers/vulnerabilities
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

_CVE_ID_PATTERN = re.compile(r"CVE-\d{4}-\d{4,}", re.IGNORECASE)


@register_engine
class NVDAdapter(EngineAdapter):
    """NVD CVE search — vulnerability descriptions, CVSS scores, references."""

    name = "nvd"
    display_name = "NVD (National Vulnerability Database)"
    env_prefix = "ENGINE_NVD"
    engine_type = "api"
    categories = ["it", "security"]

    def __init__(self, config: dict[str, Any] | None = None, rate_limiter: Any = None) -> None:
        super().__init__(config, rate_limiter)
        self._has_api_key = bool(self.config.get("api_key"))

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if (early := await self._check_rate_limit()):
            return early

        cfg = self.config
        api_key = cfg.get("api_key") or ""
        base_url = cfg.get("base_url", "https://services.nvd.nist.gov/rest/json/cves/2.0")
        timeout_ms = cfg.get("timeout_ms", 10_000)
        max_results = cfg.get("max_results", 10)

        # Detect CVE ID pattern in the query
        cve_matches = _CVE_ID_PATTERN.findall(query)
        params_dict: dict[str, Any] = {"resultsPerPage": min(max_results, 100)}

        if cve_matches:
            # Direct CVE-ID lookup
            params_dict["cveIds"] = ",".join(c.upper() for c in cve_matches[:100])
        else:
            # Keyword search
            params_dict["keywordSearch"] = query
            if max_results:
                params_dict["resultsPerPage"] = min(max_results, 100)

        # Add API key if available (reduces rate limiting)
        if api_key:
            params_dict["apiKey"] = api_key

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(base_url, params=params_dict)
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(
                        results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency,
                    )
                if resp.status_code == 403:
                    return AdapterResponse(
                        results=[], status=EngineStatus.BLOCKED, latency_ms=latency,
                    )
                resp.raise_for_status()

                data = resp.json()
                vulns = data.get("vulnerabilities", [])
                results = self._parse_vulnerabilities(vulns, max_results)
                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR, error_message=str(exc), latency_ms=latency,
            )

    def _parse_vulnerabilities(
        self, vulns: list[dict[str, Any]], max_results: int,
    ) -> list[SearchResult]:
        results: list[SearchResult] = []
        for item in vulns[:max_results]:
            cve = item.get("cve", {})
            cve_id = cve.get("id", "")

            # Extract English description
            descriptions = cve.get("descriptions", [])
            desc_text = ""
            for d in descriptions:
                if d.get("lang") == "en":
                    desc_text = d.get("value", "")
                    break
            if not desc_text:
                desc_text = descriptions[0].get("value", "") if descriptions else ""

            # Extract CVSS v3.1 / v3 score (prefer newest)
            metrics = cve.get("metrics", {})
            cvss_text = self._format_cvss(metrics)

            # Extract CWE weaknesses
            weaknesses = cve.get("weaknesses", [])
            cwe_ids = []
            for w in weaknesses:
                for desc in w.get("description", []):
                    val = desc.get("value", "")
                    if val and val not in cwe_ids:
                        cwe_ids.append(val)
            cwe_text = " | ".join(cwe_ids) if cwe_ids else ""

            # Extract references
            refs = cve.get("references", [])
            ref_urls = [r.get("url", "") for r in refs if r.get("url")]

            # Build the content: description + CVSS + CWE + refs
            content_parts = [desc_text[:500]]
            if cvss_text:
                content_parts.append(cvss_text)
            if cwe_text:
                content_parts.append(cwe_text)
            if ref_urls:
                # Show first 3 reference URLs
                refs_str = "Refs: " + " | ".join(ref_urls[:3])
                if len(ref_urls) > 3:
                    refs_str += f" (+{len(ref_urls) - 3} more)"
                content_parts.append(refs_str)

            # Published date
            published_date = cve.get("published") or None
            if published_date and "T" in published_date:
                published_date = published_date.split("T")[0]

            results.append(
                SearchResult(
                    url=f"https://nvd.nist.gov/vuln/detail/{cve_id}",
                    title=cve_id,
                    content=" | ".join(content_parts),
                    engine=self.name,
                    position=len(results) + 1,
                    published_date=published_date,
                ),
            )

        return results

    def _format_cvss(self, metrics: dict[str, Any]) -> str:
        """Format CVSS metrics into a readable string.

        Prefers CVSS v4, then v3.1, then v3.0, then v2.
        Returns empty string if no metrics found.
        """
        for version_key in ("cvssMetricV40", "cvssMetricV31", "cvssMetricV30", "cvssMetricV2"):
            metric_list = metrics.get(version_key)
            if not metric_list:
                continue
            entry = metric_list[0]
            if isinstance(entry, dict):
                cvss_data = entry.get("cvssData", {}) or entry
                vector = cvss_data.get("vectorString", "")
                base_score = cvss_data.get("baseScore", "")
                severity = cvss_data.get("baseSeverity", "")
                parts = []
                if base_score is not None and base_score != "":
                    parts.append(f"CVSS {base_score}")
                if severity:
                    parts.append(f"({severity})")
                if vector:
                    parts.append(vector)
                if parts:
                    return " ".join(parts)
        return ""
