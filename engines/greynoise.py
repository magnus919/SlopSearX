"""GreyNoise adapter — IP reputation and internet background noise classification.

Community API tier: single IP lookups without API key.
With API key: higher rate limits and bulk/quick scan endpoints.
API docs: https://docs.greynoise.io/reference/
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
class GreyNoiseAdapter(EngineAdapter):
    """GreyNoise — IP noise vs targeted classification."""

    name = "greynoise"
    display_name = "GreyNoise"
    env_prefix = "ENGINE_GREYNOISE"
    engine_type = "api"
    categories = ["security", "threat-intel"]

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
        base_url = cfg.get("base_url", "https://api.greynoise.io")
        timeout_ms = cfg.get("timeout_ms", 10_000)

        # Extract IP from query
        ip_match = _IP_PATTERN.search(query)
        if not ip_match:
            return AdapterResponse(results=[], status=EngineStatus.OK)

        ip = ip_match.group(0)

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                if api_key:
                    # Enterprise/paid API
                    headers = {
                        "Accept": "application/json",
                        "key": api_key,
                    }
                    resp = await client.get(f"{base_url}/v3/enterprise/{ip}", headers=headers)
                else:
                    # Community API — no auth needed
                    resp = await client.get(f"{base_url}/v3/community/{ip}")

                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                if resp.status_code == 403:
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                if resp.status_code == 404:
                    return AdapterResponse(results=[], status=EngineStatus.OK, latency_ms=latency)
                resp.raise_for_status()

                data = resp.json()
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
        if not data or data.get("ip") != ip:
            return []

        # Community response fields
        classification = data.get("classification", "unknown")
        noise = data.get("noise", False)
        riot = data.get("riot", False)
        tags = data.get("tags", [])
        last_seen = data.get("last_seen", "")
        name = data.get("name", "")
        link = data.get("link", "")
        bot = data.get("bot", False)

        content_parts = []
        if noise:
            content_parts.append("Noise: background scanner")
        else:
            content_parts.append("Noise: potentially targeted")
        if riot:
            content_parts.append("RIOT: benign service")
        if classification != "unknown":
            content_parts.append(f"Classification: {classification}")
        if bot:
            content_parts.append("Bot activity detected")
        if tags and isinstance(tags, list):
            content_parts.append(f"Tags: {', '.join(tags[:5])}")
        if name:
            content_parts.append(f"Name: {name}")
        if link:
            content_parts.append(link)
        if last_seen:
            content_parts.append(f"Last seen: {last_seen[:10]}")

        return [
            SearchResult(
                url=f"https://viz.greynoise.io/ip/{ip}",
                title=f"GreyNoise — {ip}",
                content=" | ".join(content_parts),
                engine=self.name,
                position=1,
            ),
        ]
