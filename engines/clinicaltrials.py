"""ClinicalTrials.gov adapter — clinical trial search.

Free, public API. No auth required.
Docs: https://clinicaltrials.gov/data-api
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
class ClinicalTrialsAdapter(EngineAdapter):
    """ClinicalTrials.gov study search."""

    name = "clinicaltrials"
    display_name = "ClinicalTrials.gov"
    env_prefix = "ENGINE_CLINICALTRIALS"
    engine_type = "api"
    categories = ["general", "medical", "health", "science"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://clinicaltrials.gov/api/v2/studies")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    base_url,
                    params={
                        "query.term": query,
                        "pageSize": max_results,
                        "format": "json",
                        "sort": "@relevance",
                    },
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()
                data = resp.json()

                results = []
                studies = data.get("studies", [])
                for idx, study in enumerate(studies[:max_results]):
                    protocol = study.get("protocolSection", {})
                    id_module = protocol.get("identificationModule", {})
                    status_module = protocol.get("statusModule", {})
                    design_module = protocol.get("designModule", {})
                    conditions_module = protocol.get("conditionsModule", {})

                    nct_id = id_module.get("nctId", "")
                    brief_title = id_module.get("briefTitle", "")
                    overall_status = status_module.get("overallStatus", "")
                    phase = design_module.get("phases", [None])
                    phase_str = phase[0] if phase else ""
                    conditions = conditions_module.get("conditions", [])
                    conditions_str = "; ".join(conditions) if conditions else ""

                    content_parts = []
                    if overall_status:
                        content_parts.append(overall_status)
                    if phase_str:
                        content_parts.append(phase_str)
                    if conditions_str:
                        content_parts.append(conditions_str)
                    content = " — ".join(content_parts)

                    results.append(
                        SearchResult(
                            url=f"https://clinicaltrials.gov/study/{nct_id}",
                            title=brief_title,
                            content=content[:500],
                            engine=self.name,
                            position=idx + 1,
                            score=1.0,
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
