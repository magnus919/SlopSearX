"""Docker Hub adapter — container image registry search.

Free, public JSON API. No auth required for anonymous search.
Docs: https://docs.docker.com/docker-hub/api/latest/
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
class DockerHubAdapter(EngineAdapter):
    """Docker Hub image search."""

    name = "dockerhub"
    display_name = "Docker Hub"
    env_prefix = "ENGINE_DOCKERHUB"
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
        base_url = cfg.get("base_url", "https://hub.docker.com/v2/repositories/library/")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        headers = {
            "User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)",
            "Accept": "application/json",
        }
        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    f"{base_url}",
                    params={"search": query, "page_size": max_results},
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()
                data = resp.json()

                results = []
                repos = data.get("results", [])
                for idx, repo in enumerate(repos[:max_results]):
                    name = repo.get("name", "")
                    description = repo.get("description", "") or ""
                    pull_count = repo.get("pull_count", 0)
                    star_count = repo.get("star_count", 0)

                    content = description
                    if pull_count:
                        content += f" — Pulls: {pull_count:,}"

                    results.append(
                        SearchResult(
                            url=f"https://hub.docker.com/_/{name}",
                            title=f"{name}",
                            content=content[:500],
                            engine=self.name,
                            position=idx + 1,
                            score=float(star_count) if star_count else float(pull_count) / 100000.0,
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
