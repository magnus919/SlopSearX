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
    categories = ["it", "reference", "packages"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://repology.org/api/v1/projects/")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        headers = {
            "User-Agent": (
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
            ),
        }
        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    base_url,
                    params={"search": query},
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 404:
                    return AdapterResponse(results=[], status=EngineStatus.OK, latency_ms=latency)

                resp.raise_for_status()
                data = resp.json()

                results = []
                for idx, (project_name, packages) in enumerate(data.items()):
                    if idx >= max_results:
                        break
                    if not packages:
                        continue

                    pkg = packages[0]
                    version = pkg.get("version", "")
                    summary = pkg.get("summary", "") or ""
                    repos = list({p.get("repo", "") for p in packages})
                    statuses = list({p.get("status", "") for p in packages})

                    content = f"Repos: {', '.join(repos[:5])}"
                    if summary:
                        content = f"{summary} — {content}"
                    if statuses:
                        content += f" | Status: {', '.join(statuses)}"

                    results.append(
                        SearchResult(
                            url=f"https://repology.org/project/{project_name}/",
                            title=f"{project_name} {version}",
                            content=content[:500],
                            engine=self.name,
                            position=idx + 1,
                            score=1.0 if "newest" in statuses else 0.5,
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
