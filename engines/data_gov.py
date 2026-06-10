"""Data.gov adapter — US government open data catalog search.

Free, public CKAN API. No auth required.
Docs: https://catalog.data.gov/api/3/
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
class DataGovAdapter(EngineAdapter):
    """US government open data search via Data.gov."""

    name = "data_gov"
    display_name = "Data.gov"
    env_prefix = "ENGINE_DATA_GOV"
    engine_type = "api"
    categories = ["general", "reference", "government"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://catalog.data.gov/api/3/action/package_search")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    base_url,
                    params={"q": query, "rows": max_results},
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()
                data = resp.json()

                results = []
                datasets = data.get("result", {}).get("results", [])
                for idx, ds in enumerate(datasets[:max_results]):
                    title = ds.get("title", "")
                    notes = ds.get("notes", "") or ""
                    organization = ds.get("organization", {}) or {}
                    org_name = organization.get("title", "") if organization else ""
                    metadata_created = ds.get("metadata_created", "")
                    resources = ds.get("resources", [])
                    resource_count = len(resources) if resources else 0
                    tags = [t.get("display_name", "") for t in ds.get("tags", [])[:5]]

                    content_parts = []
                    if org_name:
                        content_parts.append(org_name)
                    if resource_count:
                        content_parts.append(f"{resource_count} resources")
                    if tags:
                        content_parts.append(f"Tags: {', '.join(tags)}")
                    if notes:
                        content_parts.append(notes[:200])
                    content = " — ".join(content_parts) if content_parts else "Government dataset"

                    results.append(
                        SearchResult(
                            url=f"https://catalog.data.gov/dataset/{ds.get('name', '')}" if ds.get("name") else "",
                            title=title,
                            content=content[:500],
                            engine=self.name,
                            position=idx + 1,
                            published_date=metadata_created[:10] if metadata_created else None,
                            score=float(ds.get("score", 0) or 0.0),
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
