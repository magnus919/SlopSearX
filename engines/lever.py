"""Lever ATS adapter — job listings from Lever-hosted career pages.

Public, no-auth JSON API.
Docs: https://github.com/lever/postings-api
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
class LeverAdapter(EngineAdapter):
    """Lever job board search."""

    name = "lever"
    display_name = "Lever"
    env_prefix = "ENGINE_LEVER"
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
        base_url = cfg.get("base_url", "https://api.lever.co/v0/postings")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        url = f"{base_url}/{company_slug}"
        headers = {"User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)"}
        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    url,
                    params={"mode": "json"},
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 404:
                    return AdapterResponse(results=[], status=EngineStatus.OK, latency_ms=latency)

                resp.raise_for_status()
                data = resp.json()

                if not isinstance(data, list):
                    return AdapterResponse(results=[], status=EngineStatus.OK, latency_ms=latency)

                results = []
                for idx, posting in enumerate(data[:max_results]):
                    title = posting.get("text", "")
                    posting_id = posting.get("id", "")
                    hosted_url = posting.get("hostedUrl", f"https://jobs.lever.co/{company_slug}/{posting_id}")
                    categories = posting.get("categories", {}) or {}

                    location = categories.get("location", "")
                    commitment = categories.get("commitment", "")
                    team = categories.get("team", "")

                    created_at_ms = posting.get("createdAt")
                    published_date = None
                    if created_at_ms:
                        from datetime import datetime, timezone

                        published_date = datetime.fromtimestamp(created_at_ms / 1000, tz=timezone.utc).isoformat()

                    content_parts = [company_name or company_slug.title()]
                    if location:
                        content_parts.append(location)
                    if commitment:
                        content_parts.append(commitment)

                    results.append(
                        SearchResult(
                            url=hosted_url,
                            title=title,
                            content=" | ".join(content_parts)[:500],
                            engine=self.name,
                            position=idx + 1,
                            score=0.0,
                            published_date=published_date,
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
