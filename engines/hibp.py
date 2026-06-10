"""Have I Been Pwned (HIBP) adapter — breach and credential exposure search.

Requires ENGINE_HIBP_API_KEY.
Free tier for community/OSS use.
API docs: https://haveibeenpwned.com/API/v3
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

_EMAIL_PATTERN = re.compile(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b")


@register_engine
class HIBPAdapter(EngineAdapter):
    """Have I Been Pwned — breach and credential exposure search."""

    name = "hibp"
    display_name = "Have I Been Pwned"
    env_prefix = "ENGINE_HIBP"
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
        api_key = cfg.get("api_key") or ""
        base_url = cfg.get("base_url", "https://haveibeenpwned.com/api/v3")
        timeout_ms = cfg.get("timeout_ms", 10_000)

        if not api_key:
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR,
                error_message="HIBP API key not configured (set ENGINE_HIBP_API_KEY)",
            )

        email_match = _EMAIL_PATTERN.search(query)
        account = email_match.group(0) if email_match else query.strip().lower()

        headers = {
            "hibp-api-key": api_key,
            "User-Agent": "SlopSearX/0.1.0",
            "Accept": "application/vnd.haveibeenpwned.v3+json",
        }

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    f"{base_url}/breachedaccount/{account}",
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                if resp.status_code == 403:
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                if resp.status_code == 404:
                    return AdapterResponse(results=[], status=EngineStatus.OK, latency_ms=latency)
                resp.raise_for_status()

                data = resp.json()
                results = self._parse_breaches(data if isinstance(data, list) else [])
                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR, error_message=str(exc), latency_ms=latency,
            )

    def _parse_breaches(self, breaches: list[dict[str, Any]]) -> list[SearchResult]:
        results: list[SearchResult] = []
        for i, b in enumerate(breaches):
            name = b.get("Name", "")
            domain = b.get("Domain", "")
            breach_date = b.get("BreachDate", "")
            pwn_count = b.get("PwnCount", 0)
            data_classes = b.get("DataClasses", [])
            description = b.get("Description", "")[:300]

            content_parts = [description] if description else []
            if domain:
                content_parts.append(f"Domain: {domain}")
            if pwn_count:
                content_parts.append(f"Accounts: {pwn_count:,}")
            if data_classes:
                content_parts.append(f"Data: {', '.join(data_classes[:5])}")

            results.append(
                SearchResult(
                    url=f"https://haveibeenpwned.com/breaches/{name}",
                    title=name,
                    content=" | ".join(content_parts),
                    engine=self.name,
                    position=i + 1,
                    score=float(pwn_count) / 1_000_000 if pwn_count else 0.0,
                    published_date=breach_date if len(breach_date) >= 10 else None,
                ),
            )
        return results
