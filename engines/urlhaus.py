"""URLhaus adapter — malware URL and payload tracking.

Free API, no key required. Query by URL, host, or MD5 hash to find
malware distribution sites.
API docs: https://urlhaus-api.abuse.ch/
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

_URL_PATTERN = re.compile(r"https?://[^\s]+", re.IGNORECASE)
_IP_PATTERN = re.compile(r"\b(?:\d{1,3}\.){3}\d{1,3}\b")
_HOSTNAME_PATTERN = re.compile(
    r"^[a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?"
    r"(\.[a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?)*$",
)


@register_engine
class URLhausAdapter(EngineAdapter):
    """URLhaus — malware URL and payload tracking."""

    name = "urlhaus"
    display_name = "URLhaus (abuse.ch)"
    env_prefix = "ENGINE_URLHAUS"
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
        base_url = cfg.get("base_url", "https://urlhaus-api.abuse.ch")
        timeout_ms = cfg.get("timeout_ms", 10_000)

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                # Determine query type
                url_match = _URL_PATTERN.search(query)
                ip_match = _IP_PATTERN.match(query)
                is_hash = bool(re.match(r"^[a-fA-F0-9]{32}$", query.strip()))

                if url_match:
                    # URL lookup
                    endpoint = f"{base_url}/v1/url/"
                    payload = {"url": url_match.group(0)}
                elif is_hash:
                    # Hash (MD5) lookup
                    endpoint = f"{base_url}/v1/payload/"
                    payload = {"md5_hash": query.strip().lower()}
                elif ip_match:
                    # Host (IP) lookup
                    endpoint = f"{base_url}/v1/host/"
                    payload = {"host": ip_match.group(0)}
                else:
                    # Treat as hostname — must pass validation
                    first_word = query.strip().split()[0]
                    if not _HOSTNAME_PATTERN.match(first_word):
                        latency = (time.monotonic() - start_time) * 1000
                        return AdapterResponse(results=[], status=EngineStatus.OK, latency_ms=latency)
                    endpoint = f"{base_url}/v1/host/"
                    payload = {"host": first_word}

                resp = await client.post(endpoint, data=payload)
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                if resp.status_code == 403:
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                resp.raise_for_status()

                data = resp.json()
                results = self._parse_response(data)
                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR, error_message=str(exc), latency_ms=latency,
            )

    def _parse_response(self, data: dict[str, Any]) -> list[SearchResult]:
        results: list[SearchResult] = []

        query_status = data.get("query_status", "")
        if query_status in ("no_results", "invalid_host", "invalid_url"):
            return []

        # URL lookup response
        if "url_id" in data:
            results.append(self._parse_single_url(data))
            return results

        # Host/payload response — array of URLs
        urls = data.get("urls", [])
        if isinstance(urls, list):
            for item in urls[:10]:
                if isinstance(item, dict):
                    results.append(self._parse_single_url(item))

        return results

    def _parse_single_url(self, item: dict[str, Any]) -> SearchResult:
        url = item.get("url", "")
        threat = item.get("threat", "")
        tags = item.get("tags", [])
        file_type = item.get("file_type", "")
        first_seen = item.get("firstseen", "")
        last_seen = item.get("lastseen", "")
        url_status = item.get("url_status", "")
        host = item.get("host", "")

        content_parts = []
        if threat:
            content_parts.append(f"Threat: {threat}")
        if tags:
            content_parts.append(f"Tags: {', '.join(tags[:5])}")
        if file_type:
            content_parts.append(f"Type: {file_type}")
        if url_status:
            content_parts.append(f"Status: {url_status}")
        if first_seen:
            content_parts.append(f"First: {first_seen[:10]}")
        if last_seen:
            content_parts.append(f"Last: {last_seen[:10]}")

        title = host or url.split("/")[2] if url else "malware URL"
        return SearchResult(
            url=url or f"https://urlhaus.abuse.ch/url/{item.get('url_id', '')}",
            title=title,
            content=" | ".join(content_parts),
            engine=self.name,
            position=1,
            published_date=first_seen[:10] if first_seen else None,
        )
