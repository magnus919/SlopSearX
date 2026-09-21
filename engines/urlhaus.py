"""URLhaus adapter — malware URL and payload tracking.

Query by URL, host, or MD5 hash to find malware distribution sites. URLhaus
requires an Auth-Key; the adapter forwards it without putting the credential
in the request URL.
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

_INVALID_INPUT_STATUSES = {"invalid_host", "invalid_url", "invalid_md5", "invalid_sha256"}


@register_engine
class URLhausAdapter(EngineAdapter):
    """URLhaus — malware URL and payload tracking."""

    name = "urlhaus"
    display_name = "URLhaus (abuse.ch)"
    env_prefix = "ENGINE_URLHAUS"
    engine_type = "api"
    categories = ["security", "threat-intel"]

    # -- Declared capability metadata (audited, issue 185) --
    supported_result_types = ("text",)
    failure_classes = ("rate_limited", "blocked", "error", "timeout", "unavailable")
    cost_class = "free"

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        cfg = self.config
        auth_key = cfg.get("api_key") or ""
        if not auth_key:
            return AdapterResponse(
                results=[],
                status=EngineStatus.UNAVAILABLE,
                error_message="URLhaus requires ENGINE_URLHAUS_API_KEY",
            )

        if early := await self._check_rate_limit():
            return early

        base_url = cfg.get("base_url", "https://urlhaus-api.abuse.ch")
        timeout_ms = cfg.get("timeout_ms", 10_000)

        start_time = time.monotonic()
        try:
            async with self.http_client(timeout=timeout_ms / 1000.0) as client:
                # Determine query type
                normalized_query = query.strip()
                if not normalized_query:
                    latency = (time.monotonic() - start_time) * 1000
                    return AdapterResponse(
                        results=[],
                        status=EngineStatus.ERROR,
                        error_message="URLhaus query is empty",
                        latency_ms=latency,
                    )

                url_match = _URL_PATTERN.search(query)
                ip_match = _IP_PATTERN.match(query)
                is_hash = bool(re.match(r"^[a-fA-F0-9]{32}$", normalized_query))

                if url_match:
                    # URL lookup
                    query_kind = "url"
                    endpoint = f"{base_url}/v1/url/"
                    payload = {"url": url_match.group(0)}
                elif is_hash:
                    # Hash (MD5) lookup
                    query_kind = "payload"
                    endpoint = f"{base_url}/v1/payload/"
                    payload = {"md5_hash": normalized_query.lower()}
                elif ip_match:
                    # Host (IP) lookup
                    query_kind = "host"
                    endpoint = f"{base_url}/v1/host/"
                    payload = {"host": ip_match.group(0)}
                else:
                    # Treat as hostname — must pass validation
                    first_word = normalized_query.split()[0]
                    if not _HOSTNAME_PATTERN.match(first_word):
                        latency = (time.monotonic() - start_time) * 1000
                        return AdapterResponse(
                            results=[],
                            status=EngineStatus.ERROR,
                            error_message="URLhaus query is not a supported URL, host, or MD5 hash",
                            latency_ms=latency,
                        )
                    query_kind = "host"
                    endpoint = f"{base_url}/v1/host/"
                    payload = {"host": first_word}

                headers = {"Accept": "application/json"}
                if auth_key:
                    headers["Auth-Key"] = str(auth_key)

                resp = await client.post(endpoint, data=payload, headers=headers)
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                if resp.status_code == 401:
                    return AdapterResponse(
                        results=[],
                        status=EngineStatus.BLOCKED,
                        error_message="URLhaus authentication required",
                        latency_ms=latency,
                    )
                if resp.status_code == 403:
                    return AdapterResponse(
                        results=[],
                        status=EngineStatus.BLOCKED,
                        error_message="URLhaus request blocked",
                        latency_ms=latency,
                    )
                if 400 <= resp.status_code < 500:
                    return AdapterResponse(
                        results=[],
                        status=EngineStatus.ERROR,
                        error_message=f"URLhaus rejected request (HTTP {resp.status_code})",
                        latency_ms=latency,
                    )
                resp.raise_for_status()

                data = resp.json()
                if not isinstance(data, dict):
                    raise ValueError("URLhaus response must be a JSON object")
                results = self._parse_response(data, query_kind)
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

    def _parse_response(self, data: dict[str, Any], query_kind: str) -> list[SearchResult]:
        query_status = data.get("query_status")
        if not isinstance(query_status, str) or not query_status:
            raise ValueError("URLhaus response is missing query_status")

        if query_status == "no_results":
            return []
        if query_status in _INVALID_INPUT_STATUSES:
            raise ValueError(f"URLhaus rejected invalid input ({query_status}) for {query_kind} query")
        if query_status != "ok":
            raise ValueError(f"URLhaus returned unexpected query_status: {query_status}")

        # URL lookup response. URLhaus documents ``id``; retain ``url_id``
        # compatibility with older fixtures/responses.
        if query_kind == "url":
            if not ("id" in data or "url_id" in data):
                raise ValueError("URLhaus URL response is missing its identifier")
            return [self._parse_single_url(data)]

        # Host/payload response — array of URLs.
        urls = data.get("urls")
        if not isinstance(urls, list) or not urls:
            raise ValueError("URLhaus response is missing a non-empty urls list")
        if not all(isinstance(item, dict) for item in urls):
            raise ValueError("URLhaus response contains a malformed URL record")
        return [self._parse_single_url(item) for item in urls[:10]]

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
            url=url or f"https://urlhaus.abuse.ch/url/{item.get('url_id', item.get('id', ''))}",
            title=title,
            content=" | ".join(content_parts),
            engine=self.name,
            position=1,
            published_date=first_seen[:10] if first_seen else None,
        )
