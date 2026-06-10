"""CRT.sh adapter — Certificate Transparency log search.

Free API, no key required. Searches by domain and returns matching
SSL/TLS certificates from Certificate Transparency logs.
API docs: https://crt.sh/
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
class CrtShAdapter(EngineAdapter):
    """Certificate Transparency log search via CRT.sh."""

    name = "crtsh"
    display_name = "CRT.sh (Certificate Transparency)"
    env_prefix = "ENGINE_CRTSH"
    engine_type = "api"
    categories = ["it", "security"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://crt.sh")
        timeout_ms = cfg.get("timeout_ms", 10_000)
        max_results = cfg.get("max_results", 10)

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(f"{base_url}/?output=json", params={"q": query})
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                if resp.status_code == 403:
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                resp.raise_for_status()

                data = resp.json()
                if not isinstance(data, list):
                    return AdapterResponse(results=[], status=EngineStatus.OK, latency_ms=latency)

                results = self._parse_certs(data[:max_results])
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

    def _parse_certs(self, certs: list[dict[str, Any]]) -> list[SearchResult]:
        results: list[SearchResult] = []
        for i, cert in enumerate(certs):
            common_name = cert.get("common_name", "")
            issuer = cert.get("issuer_name", "")
            sans_raw = cert.get("name_value", "")
            sans = ", ".join(s.split("\n")[0] for s in [sans_raw][:5]) if sans_raw else ""
            not_before = cert.get("not_before", "")
            not_after = cert.get("not_after", "")
            serial = cert.get("serial_number", "")

            content_parts = []
            if issuer:
                content_parts.append(f"Issuer: {issuer}")
            if sans:
                content_parts.append(f"SANs: {sans}")
            if not_before:
                content_parts.append(f"Valid: {not_before[:10]}")
            if not_after:
                content_parts.append(f"Expires: {not_after[:10]}")

            results.append(
                SearchResult(
                    url=f"https://crt.sh/?id={cert.get('id', '')}"
                    if cert.get("id")
                    else f"https://crt.sh/?q={common_name}",
                    title=common_name or f"Certificate #{cert.get('id', '')}",
                    content=" | ".join(content_parts)
                    if content_parts
                    else f"Serial: {serial[:20]}"
                    if serial
                    else "Certificate result",
                    engine=self.name,
                    position=i + 1,
                    published_date=not_before[:10] if not_before else None,
                ),
            )
        return results
