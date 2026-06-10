"""npm adapter — npm registry package search.

Free, public JSON API. No auth required.
Docs: https://github.com/npm/registry/blob/master/docs/REGISTRY-API.md
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
class NpmAdapter(EngineAdapter):
    """npm registry package search."""

    name = "npm"
    display_name = "npm"
    env_prefix = "ENGINE_NPM"
    engine_type = "api"
    categories = ["general", "it", "reference", "packages"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://registry.npmjs.org/-/v1/search")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        headers = {"User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)"}
        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    base_url,
                    params={"text": query, "size": max_results},
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()
                data = resp.json()

                results = []
                objects = data.get("objects", [])
                for idx, obj in enumerate(objects[:max_results]):
                    pkg = obj.get("package", {})
                    name = pkg.get("name", "")
                    version = pkg.get("version", "")
                    description = pkg.get("description", "") or ""
                    publisher = pkg.get("publisher", {}).get("username", "")
                    downloads = obj.get("score", {}).get("detail", {}).get("popularity", 0)

                    content = description
                    if publisher:
                        content += f" — Published by {publisher}"

                    results.append(
                        SearchResult(
                            url=f"https://www.npmjs.com/package/{name}",
                            title=f"{name}@{version}",
                            content=content[:500],
                            engine=self.name,
                            position=idx + 1,
                            score=float(downloads) * 1000 if downloads else 0.0,
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
