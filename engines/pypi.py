"""PyPI adapter — Python Package Index search via JSON API.

Free, public JSON API. No auth required.
Uses the warehouse JSON API: https://pypi.org/pypi/{name}/json
"""

from __future__ import annotations

import time
from typing import Any

import httpx

from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult, register_engine


@register_engine
class PyPIAdapter(EngineAdapter):
    """PyPI package search via JSON API."""

    name = "pypi"
    display_name = "PyPI"
    env_prefix = "ENGINE_PYPI"
    engine_type = "api"
    categories = ["it", "reference", "packages"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if (early := await self._check_rate_limit()):
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://pypi.org")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        headers = {"User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)"}
        start_time = time.monotonic()

        try:
            # Fetch package JSON directly for the query as a package name
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0, follow_redirects=True) as client:
                resp = await client.get(
                    f"{base_url}/pypi/{query}/json",
                    headers=headers,
                )
                if resp.status_code == 200:
                    data = resp.json()
                    info = data.get("info", {})
                    name = info.get("name", query)
                    version = info.get("version", "")
                    summary = info.get("summary", "") or ""
                    latency = (time.monotonic() - start_time) * 1000
                    return AdapterResponse(
                        results=[
                            SearchResult(
                                url=f"https://pypi.org/project/{name}/",
                                title=f"{name} {version}",
                                content=summary[:500] or f"Python package: {name}",
                                engine=self.name,
                                position=1,
                                score=1.0,
                            ),
                        ],
                        status=EngineStatus.OK,
                        latency_ms=latency,
                    )

                # Not found as exact package name - try searching via simple index
                return await self._search_via_simple(query, base_url, timeout_ms, max_results, headers, start_time)

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

    async def _search_via_simple(
        self,
        query: str,
        base_url: str,
        timeout_ms: int,
        max_results: int,
        headers: dict[str, str],
        start_time: float,
    ) -> AdapterResponse:
        """Fallback: query package JSON for similar names via simple index."""
        from lxml import html

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    f"{base_url}/simple/",
                    headers=headers,
                )
                resp.raise_for_status()
                latency = (time.monotonic() - start_time) * 1000

                root = html.fromstring(resp.content)
                links = root.xpath("//a")
                query_lower = query.lower().replace("-", "").replace("_", "")
                matched = []
                for a in links:
                    href = a.get("href", "")
                    name = href.strip("/").split("/")[-1] if href else ""
                    name_clean = name.lower().replace("-", "").replace("_", "")
                    if query_lower in name_clean:
                        matched.append(name)
                    if len(matched) >= max_results:
                        break

                results: list[SearchResult] = []
                for name in matched:
                    try:
                        r = await client.get(f"{base_url}/pypi/{name}/json", headers=headers)
                        if r.status_code == 200:
                            info = r.json().get("info", {})
                            results.append(
                                SearchResult(
                                    url=f"https://pypi.org/project/{name}/",
                                    title=f"{name} {info.get('version', '')}",
                                    content=(info.get("summary", "") or "")[:500],
                                    engine=self.name,
                                    position=len(results) + 1,
                                    score=1.0,
                                ),
                            )
                    except Exception:
                        continue

                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
                latency_ms=latency,
            )
