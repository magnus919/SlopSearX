"""DeHashed adapter — credential leak and breached data search.

Requires ENGINE_DEHASHED_API_KEY (email:api_key format for basic auth).
Paid API, limited free tier.
API docs: https://dehashed.com/documentation
"""

from __future__ import annotations

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


@register_engine
class DeHashedAdapter(EngineAdapter):
    """DeHashed — credential leak and breached data search."""

    name = "dehashed"
    display_name = "DeHashed"
    env_prefix = "ENGINE_DEHASHED"
    engine_type = "api"
    categories = ["security", "threat-intel"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        api_key = cfg.get("api_key") or ""
        base_url = cfg.get("base_url", "https://dehashed.com/api/v1")
        timeout_ms = cfg.get("timeout_ms", 10_000)
        max_results = cfg.get("max_results", 10)

        if not api_key:
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message="DeHashed API key not configured (set ENGINE_DEHASHED_API_KEY)",
            )

        # DeHashed basic auth: email as username, API key as password
        # Config can store "email:api_key" or just "api_key"
        if ":" in api_key:
            email, key = api_key.split(":", 1)
        else:
            email = ""
            key = api_key

        headers = {
            "Accept": "application/json",
        }

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    f"{base_url}/search",
                    headers=headers,
                    auth=(email, key),
                    params={"q": query, "size": max_results},
                )
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                if resp.status_code == 403:
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                resp.raise_for_status()

                data = resp.json()
                entries = data.get("entries", [])[:max_results]
                results = self._parse_entries(entries, query)
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

    def _parse_entries(self, entries: list[dict[str, Any]], query_str: str = "") -> list[SearchResult]:
        results: list[SearchResult] = []
        for i, entry in enumerate(entries):
            email = entry.get("email", "")
            username = entry.get("username", "")
            password = entry.get("password", "")
            hashed_pass = entry.get("hashed_password", "")
            ip = entry.get("ip_address", "")
            name = entry.get("name", "")
            database = entry.get("database_name", "")
            breach = entry.get("obtained_from", "")

            title = email or username or name or f"Entry #{entry.get('id', i)}"
            content_parts = []

            if email:
                content_parts.append(f"Email: {email}")
            if username:
                content_parts.append(f"Username: {username}")
            # Truncate passwords for safety — show first 4 chars only
            if password:
                content_parts.append(f"Password: {password[:4]}***")
            if hashed_pass:
                content_parts.append(f"Hash: {hashed_pass[:12]}...")
            if ip:
                content_parts.append(f"IP: {ip}")
            if database:
                content_parts.append(f"DB: {database}")
            if breach:
                content_parts.append(f"Source: {breach}")

            results.append(
                SearchResult(
                    url=f"https://dehashed.com/search?q={email if email else query_str}",
                    title=title,
                    content=" | ".join(content_parts),
                    engine=self.name,
                    position=i + 1,
                ),
            )
        return results
