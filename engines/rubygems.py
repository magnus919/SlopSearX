"""RubyGems adapter — Ruby gem package registry search.

Free, public JSON API. No auth required.
Docs: https://guides.rubygems.org/rubygems-org-api/
"""

from __future__ import annotations

import time
from typing import Any

import httpx

from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult, register_engine


@register_engine
class RubyGemsAdapter(EngineAdapter):
    """RubyGems package search."""

    name = "rubygems"
    display_name = "RubyGems"
    env_prefix = "ENGINE_RUBYGEMS"
    engine_type = "api"
    categories = ["general", "it", "reference", "packages"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if (early := await self._check_rate_limit()):
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://rubygems.org/api/v1/search.json")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    base_url,
                    params={"query": query},
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()
                data = resp.json()

                results = []
                gems = data if isinstance(data, list) else data.get("results", [])
                for idx, gem in enumerate(gems[:max_results]):
                    name = gem.get("name", "")
                    version = gem.get("version", "")
                    info = gem.get("info", "") or gem.get("description", "") or ""
                    downloads = gem.get("downloads", 0) or 0
                    authors = gem.get("authors", "")

                    content = info
                    if authors:
                        content = f"{info} — by {authors}" if info else f"by {authors}"
                    if downloads:
                        content += f" | Downloads: {downloads:,}"

                    results.append(
                        SearchResult(
                            url=f"https://rubygems.org/gems/{name}",
                            title=f"{name} {version}",
                            content=content[:500],
                            engine=self.name,
                            position=idx + 1,
                            score=float(downloads) / 1000.0 if downloads else 0.0,
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
            return AdapterResponse(results=[], status=EngineStatus.ERROR, error_message=str(exc), latency_ms=latency)
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
                latency_ms=latency,
            )
