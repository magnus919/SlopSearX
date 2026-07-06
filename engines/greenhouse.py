"""Greenhouse ATS adapter — job listings from Greenhouse-hosted career pages.

Public, no-auth JSON API.
Docs: https://developers.greenhouse.io/job-board.html
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
from slopsearx.jobs_utils import extract_company


@register_engine
class GreenhouseAdapter(EngineAdapter):
    """Greenhouse job board search."""

    name = "greenhouse"
    display_name = "Greenhouse"
    env_prefix = "ENGINE_GREENHOUSE"
    engine_type = "api"
    categories = ["jobs"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        company_slug, company_name = extract_company(query)
        if company_slug is None:
            return AdapterResponse(results=[], status=EngineStatus.OK)

        cfg = self.config
        base_url = cfg.get("base_url", "https://boards-api.greenhouse.io/v1/boards")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        url = f"{base_url}/{company_slug}/jobs"
        headers = {"User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)"}
        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    url,
                    params={"content": "true"},
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 404:
                    return AdapterResponse(results=[], status=EngineStatus.OK, latency_ms=latency)

                resp.raise_for_status()
                data = resp.json()

                results = []
                jobs = data.get("jobs", [])
                for idx, job in enumerate(jobs[:max_results]):
                    title = job.get("title", "")
                    job_id = job.get("id", "")
                    absolute_url = job.get("absolute_url", f"https://boards.greenhouse.io/{company_slug}/jobs/{job_id}")
                    updated_at = job.get("updated_at")

                    locations = [loc.get("name", "") for loc in job.get("offices", [])]
                    location_str = ", ".join(locations) if locations else ""

                    salary = ""
                    metadata = job.get("metadata", [])
                    for meta in metadata:
                        if meta.get("name", "").lower() == "salary":
                            salary = meta.get("value", "")
                            break

                    content_parts = [company_name or company_slug.title()]
                    if location_str:
                        content_parts.append(location_str)
                    if salary:
                        content_parts.append(salary)

                    results.append(
                        SearchResult(
                            url=absolute_url,
                            title=title,
                            content=" | ".join(content_parts)[:500],
                            engine=self.name,
                            position=idx + 1,
                            score=0.0,
                            published_date=updated_at,
                            category="jobs",
                            tier=2,
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
