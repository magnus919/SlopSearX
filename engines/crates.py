"""Crates.io adapter — Rust package registry search.

Free, public JSON API. No auth required.
Docs: https://doc.rust-lang.org/cargo/reference/registry-web-api.html
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
class CratesAdapter(EngineAdapter):
    """crates.io package search."""

    name = "crates"
    display_name = "crates.io"
    env_prefix = "ENGINE_CRATES"
    engine_type = "api"
    categories = ["it", "reference", "packages"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://crates.io/api/v1/crates")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        headers = {"User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)"}
        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    base_url,
                    params={"q": query, "per_page": max_results},
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()
                data = resp.json()

                results = []
                crates = data.get("crates", [])
                for idx, crate in enumerate(crates[:max_results]):
                    name = crate.get("name", "")
                    latest_version = crate.get("max_version", "")
                    description = crate.get("description", "") or ""
                    downloads = crate.get("downloads", 0)
                    recent_downloads = crate.get("recent_downloads", 0) or 0

                    content = description
                    if downloads:
                        content += f" — Total downloads: {downloads:,}"

                    results.append(
                        SearchResult(
                            url=f"https://crates.io/crates/{name}",
                            title=f"{name} v{latest_version}",
                            content=content[:500],
                            engine=self.name,
                            position=idx + 1,
                            score=float(recent_downloads) if recent_downloads else float(downloads),
                        ),
                    )

                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except httpx.HTTPStatusError as exc:
            latency = (time.monotonic() - start_time) * 1000
            if exc.response.status_code == 429:
                return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
                latency_ms=latency,
            )
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
                latency_ms=latency,
            )
