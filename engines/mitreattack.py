"""MITRE ATT&CK adapter — adversary techniques, tactics, groups, and software search.

Free API, no key required. Uses the MITRE ATT&CK website search
interface and STIX data for technique/group/software lookups.
"""

from __future__ import annotations

import re
import time
from typing import Any

import httpx
from lxml import html

from slopsearx.adapter import (
    AdapterResponse,
    EngineAdapter,
    EngineStatus,
    SearchResult,
    register_engine,
)

_TECHNIQUE_PATTERN = re.compile(r"(T\d{4}(?:\.\d{3})?)", re.IGNORECASE)
_GROUP_PATTERN = re.compile(r"(G\d{4})", re.IGNORECASE)
_SOFTWARE_PATTERN = re.compile(r"(S\d{4})", re.IGNORECASE)


@register_engine
class MitreAttackAdapter(EngineAdapter):
    """MITRE ATT&CK — adversary TTP knowledge base."""

    name = "mitreattack"
    display_name = "MITRE ATT&CK"
    env_prefix = "ENGINE_MITREATTACK"
    engine_type = "api"
    categories = ["general", "security", "reference"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if (early := await self._check_rate_limit()):
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://attack.mitre.org")
        timeout_ms = cfg.get("timeout_ms", 10_000)
        max_results = cfg.get("max_results", 10)

        # Check for ATT&CK ID patterns
        technique = _TECHNIQUE_PATTERN.search(query)
        group = _GROUP_PATTERN.search(query)
        software = _SOFTWARE_PATTERN.search(query)

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0, follow_redirects=True) as client:
                headers = {
                    "User-Agent": (
                        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
                        "(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
                    ),
                    "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
                }

                # Direct ID lookup if we found patterns
                if technique:
                    url = f"{base_url}/techniques/{technique.group(1).upper()}/"
                    resp = await client.get(url, headers=headers)
                    latency = (time.monotonic() - start_time) * 1000
                    if resp.status_code == 429:
                        return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                    if resp.status_code == 403:
                        return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                    resp.raise_for_status()
                    results = self._parse_technique_page(resp.text, technique.group(1).upper())
                    return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

                if group:
                    url = f"{base_url}/groups/{group.group(1).upper()}/"
                    resp = await client.get(url, headers=headers)
                    latency = (time.monotonic() - start_time) * 1000
                    if resp.status_code == 429:
                        return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                    if resp.status_code == 403:
                        return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                    resp.raise_for_status()
                    results = self._parse_group_page(resp.text, group.group(1).upper())
                    return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

                if software:
                    url = f"{base_url}/software/{software.group(1).upper()}/"
                    resp = await client.get(url, headers=headers)
                    latency = (time.monotonic() - start_time) * 1000
                    if resp.status_code == 429:
                        return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                    if resp.status_code == 403:
                        return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                    resp.raise_for_status()
                    results = self._parse_software_page(resp.text, software.group(1).upper())
                    return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

                # Keyword search via the site search
                resp = await client.get(
                    f"{base_url}/search",
                    headers=headers,
                    params={"q": query},
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()

                results = self._parse_search_page(resp.text, query, max_results)
                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR, error_message=str(exc), latency_ms=latency,
            )

    def _parse_technique_page(self, html_text: str, technique_id: str) -> list[SearchResult]:
        """Parse a single technique detail page."""
        if not html_text:
            return []
        tree = html.fromstring(html_text)

        title_el = tree.cssselect("h1")
        title = title_el[0].text_content().strip() if title_el else technique_id

        desc_el = tree.cssselect(".description-body p, .description p, .card-body p")
        description = desc_el[0].text_content().strip()[:500] if desc_el else ""

        # Extract tactics/platforms
        tactics_el = tree.cssselect(".tactic-badge, .tactic a, .card-body .badge")
        tactics = [t.text_content().strip() for t in tactics_el[:3] if t.text_content().strip()]

        content_parts = [description] if description else []
        if tactics:
            content_parts.append(f"Tactics: {', '.join(tactics)}")

        return [
            SearchResult(
                url=f"https://attack.mitre.org/techniques/{technique_id}/",
                title=title,
                content=" | ".join(content_parts) if content_parts else f"ATT&CK Technique {technique_id}",
                engine=self.name,
                position=1,
            ),
        ]

    def _parse_group_page(self, html_text: str, group_id: str) -> list[SearchResult]:
        """Parse a group (adversary) detail page."""
        if not html_text:
            return []
        tree = html.fromstring(html_text)

        title_el = tree.cssselect("h1")
        title = title_el[0].text_content().strip() if title_el else group_id

        desc_el = tree.cssselect(".description-body p, .description p, .card-body p")
        description = desc_el[0].text_content().strip()[:500] if desc_el else ""

        # Associated techniques
        tech_els = tree.cssselect(".technique-table a, .table a")
        techniques = [t.text_content().strip() for t in tech_els[:5] if t.text_content().strip()]

        content_parts = [description] if description else []
        if techniques:
            content_parts.append(f"Techniques: {', '.join(techniques[:5])}")

        return [
            SearchResult(
                url=f"https://attack.mitre.org/groups/{group_id}/",
                title=title,
                content=" | ".join(content_parts) if content_parts else f"ATT&CK Group {group_id}",
                engine=self.name,
                position=1,
            ),
        ]

    def _parse_software_page(self, html_text: str, software_id: str) -> list[SearchResult]:
        """Parse a software detail page."""
        if not html_text:
            return []
        tree = html.fromstring(html_text)

        title_el = tree.cssselect("h1")
        title = title_el[0].text_content().strip() if title_el else software_id

        desc_el = tree.cssselect(".description-body p, .description p, .card-body p")
        description = desc_el[0].text_content().strip()[:500] if desc_el else ""

        content_parts = [description] if description else [f"ATT&CK Software {software_id}"]

        return [
            SearchResult(
                url=f"https://attack.mitre.org/software/{software_id}/",
                title=title,
                content=" | ".join(content_parts),
                engine=self.name,
                position=1,
            ),
        ]

    def _parse_search_page(self, html_text: str, query: str, max_results: int) -> list[SearchResult]:
        """Parse the ATT&CK search results page."""
        if not html_text or "captcha" in html_text.lower():
            return []
        tree = html.fromstring(html_text)
        results: list[SearchResult] = []

        # Search result items
        items = tree.cssselect(".search-result, .result-item, .list-group-item, li.search-result")
        if not items:
            # Try alt selectors
            items = tree.cssselect("article, .card")

        for item in items[:max_results]:
            title_el = item.cssselect("a, h2, h3, h4")
            desc_el = item.cssselect("p, .description, .summary")

            title = title_el[0].text_content().strip() if title_el else ""
            link = title_el[0].get("href", "") if title_el else ""
            description = desc_el[0].text_content().strip()[:300] if desc_el else ""

            if not title:
                continue

            full_url = f"https://attack.mitre.org{link}" if link.startswith("/") else link

            results.append(
                SearchResult(
                    url=full_url or f"https://attack.mitre.org/search?q={query}",
                    title=title,
                    content=description or f"ATT&CK search result for '{query}'",
                    engine=self.name,
                    position=len(results) + 1,
                ),
            )

        return results
