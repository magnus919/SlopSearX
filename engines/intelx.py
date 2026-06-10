"""IntelX (Intelligence X) adapter — darknet, paste, and OSINT search.

Requires ENGINE_INTELX_API_KEY.
Limited free tier (credits-based).
API docs: https://intelx.io/api-docs
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
class IntelXAdapter(EngineAdapter):
    """IntelX — darknet, paste sites, and document OSINT search."""

    name = "intelx"
    display_name = "Intelligence X (IntelX)"
    env_prefix = "ENGINE_INTELX"
    engine_type = "api"
    categories = ["security", "threat-intel"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if (early := await self._check_rate_limit()):
            return early

        cfg = self.config
        api_key = cfg.get("api_key") or ""
        base_url = cfg.get("base_url", "https://2.intelx.io")
        timeout_ms = cfg.get("timeout_ms", 15_000)
        max_results = cfg.get("max_results", 10)

        if not api_key:
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR,
                error_message="IntelX API key not configured (set ENGINE_INTELX_API_KEY)",
            )

        headers = {
            "Accept": "application/json",
            "x-key": api_key,
            "User-Agent": "SlopSearX/0.1.0",
        }

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                # Phase 1: search
                search_resp = await client.post(
                    f"{base_url}/intelligent/search",
                    headers=headers,
                    json={
                        "term": query,
                        "maxresults": max_results,
                        "target": 0,
                        "sort": 2,
                    },
                )
                latency = (time.monotonic() - start_time) * 1000

                if search_resp.status_code == 429:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                if search_resp.status_code == 403:
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                search_resp.raise_for_status()

                search_data = search_resp.json()
                search_id = search_data.get("id", "")
                if not search_id:
                    return AdapterResponse(results=[], status=EngineStatus.OK, latency_ms=latency)

                # Phase 2: fetch results (with a brief delay for the search to complete)
                import asyncio
                await asyncio.sleep(2)

                result_resp = await client.get(
                    f"{base_url}/intelligent/result",
                    headers=headers,
                    params={"id": search_id, "limit": max_results},
                )
                latency = (time.monotonic() - start_time) * 1000
                result_resp.raise_for_status()

                result_data = result_resp.json()
                records = (
                    result_data.get("records", [])
                    if isinstance(result_data, dict)
                    else result_data if isinstance(result_data, list) else []
                )
                records_list = records if isinstance(records, list) else []

                results = self._parse_records(records_list[:max_results])
                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR, error_message=str(exc), latency_ms=latency,
            )

    def _parse_records(self, records: list[dict[str, Any]]) -> list[SearchResult]:
        results: list[SearchResult] = []
        # Source type labels
        source_labels = {1: "Pastebin", 2: "Darknet", 3: "Document", 4: "Source Code", 5: "Forum"}

        for i, record in enumerate(records):
            # Handle both nested and flat structures
            if isinstance(record, dict) and "record" in record:
                record = record["record"]
            if not isinstance(record, dict):
                continue

            system_id = record.get("systemid", "")
            name = record.get("name", "")
            record_type = record.get("type", 0)
            source = source_labels.get(record_type, f"Type {record_type}")
            date = record.get("date", "")
            preview = record.get("preview", "")[:300]
            bucket = record.get("bucket", "")

            content_parts = [source]
            if preview:
                content_parts.append(preview)
            if bucket:
                content_parts.append(bucket)

            title = name if name else system_id[:32] if system_id else f"IntelX result {i + 1}"
            results.append(
                SearchResult(
                    url=f"https://intelx.io/?did={system_id}" if system_id else "",
                    title=title,
                    content=" | ".join(content_parts),
                    engine=self.name,
                    position=i + 1,
                    published_date=date[:10] if date and len(date) >= 10 else None,
                ),
            )
        return results
