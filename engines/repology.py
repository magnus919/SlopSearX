"""Repology adapter — multi-repository package search.

Free, public JSON API. No auth required.
Docs: https://repology.org/api-docs/
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
class RepologyAdapter(EngineAdapter):
    """Multi-repository package search via Repology."""

    name = "repology"
    display_name = "Repology"
    env_prefix = "ENGINE_REPOLOGY"
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
        base_url = cfg.get("base_url", "https://repology.org/api/v1")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        headers = {"User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)"}
        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    f"{base_url}/project/{query}",
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 404:
                    return AdapterResponse(results=[], status=EngineStatus.OK, latency_ms=latency)

                resp.raise_for_status()
                data = resp.json()

                # Repology returns a list of package dicts or a dict keyed by repo name
                packages = data if isinstance(data, list) else [data]
                packages = packages[:max_results]

                results = []
                for idx, pkg in enumerate(packages):
                    name = pkg.get("name", "")
                    repo = pkg.get("repo", "")
                    vers = pkg.get("visible_version", "") or pkg.get("version", "")
                    summary = pkg.get("summary", "") or ""
                    licenses = pkg.get("licenses", [])
                    status = pkg.get("status", "")

                    content = f"[{repo}] {name}"
                    if vers:
                        content += f" {vers}"
                    if summary:
                        content += f" — {summary}"
                    if licenses:
                        content += f" | License: {', '.join(licenses)}"

                    results.append(
                        SearchResult(
                            url=f"https://repology.org/project/{name}/",
                            title=f"{name} — {repo}",
                            content=content[:500],
                            engine=self.name,
                            position=idx + 1,
                            score=1.0 if status == "newest" else 0.5,
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
