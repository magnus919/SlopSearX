"""Ashby ATS adapter — job listings from Ashby-hosted career pages.

Public, no-auth GraphQL API.
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

_ASHBY_QUERY = """
query ApiJobBoardWithTeams {
  jobBoard {
    jobPostings {
      id
      title
      locationName
      salaryRange
      departmentName
      updatedAt
    }
  }
}
"""


@register_engine
class AshbyAdapter(EngineAdapter):
    """Ashby job board search."""

    name = "ashby"
    display_name = "Ashby"
    env_prefix = "ENGINE_ASHBY"
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
        base_url = cfg.get("base_url", "https://jobs.ashbyhq.com")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        url = f"{base_url}/{company_slug}/api/non-user-graphql"
        headers = {
            "User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)",
            "Content-Type": "application/json",
        }
        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.post(
                    url,
                    json={"operationName": "ApiJobBoardWithTeams", "variables": {}, "query": _ASHBY_QUERY},
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 404:
                    return AdapterResponse(results=[], status=EngineStatus.OK, latency_ms=latency)

                resp.raise_for_status()
                data = resp.json()

                results = []
                job_board = (data.get("data") or {}).get("jobBoard") or {}
                postings = job_board.get("jobPostings") or []
                for idx, posting in enumerate(postings[:max_results]):
                    posting_id = posting.get("id") or ""
                    title = posting.get("title") or ""
                    location = posting.get("locationName") or ""
                    salary_range = posting.get("salaryRange") or ""
                    department = posting.get("departmentName") or ""
                    updated_at = posting.get("updatedAt")

                    posting_url = f"https://jobs.ashbyhq.com/{company_slug}/{posting_id}"

                    content_parts = [company_name or company_slug.title()]
                    if location:
                        content_parts.append(location)
                    if department:
                        content_parts.append(department)
                    if salary_range:
                        content_parts.append(salary_range)

                    results.append(
                        SearchResult(
                            url=posting_url,
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
