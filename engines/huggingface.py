"""HuggingFace adapter — model, dataset, and paper discovery."""

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
class HuggingFaceAdapter(EngineAdapter):
    name = "huggingface"
    display_name = "HuggingFace"
    env_prefix = "ENGINE_HUGGINGFACE"
    engine_type = "api"
    categories = ["general", "science", "huggingface:datasets", "huggingface:papers"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if (early := await self._check_rate_limit()):
            return early

        cfg = self.config
        token = cfg.get("api_key") or ""
        base_url = cfg.get("base_url", "https://huggingface.co/api")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 5)
        categories = (params or {}).get("categories", []) or ["general"]

        # Determine sub-mode from categories
        if "huggingface:datasets" in categories:
            endpoint = f"{base_url}/datasets"
        elif "huggingface:papers" in categories:
            endpoint = f"{base_url}/papers"
        else:
            # Default: models
            endpoint = f"{base_url}/models"

        params_dict: dict[str, Any] = {
            "search": query,
            "limit": max_results,
        }

        headers = {
            "User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)",
        }
        if token:
            headers["Authorization"] = f"Bearer {token}"

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(endpoint, params=params_dict, headers=headers)
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                resp.raise_for_status()

                items = resp.json()
                results = self._parse_items(items, endpoint)
                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR, error_message=str(exc), latency_ms=latency,
            )

    def _parse_items(self, items: list[dict[str, Any]], endpoint: str) -> list[SearchResult]:
        """Parse HF API results into SearchResult list."""
        results: list[SearchResult] = []

        is_models = "/models" in endpoint
        is_datasets = "/datasets" in endpoint

        for idx, item in enumerate(items):
            if is_models:
                model_id = item.get("modelId", item.get("id", ""))
                url = f"https://huggingface.co/{model_id}" if model_id else ""
                title = model_id
                pipeline = item.get("pipeline_tag") or ""
                library = item.get("library_name") or ""
                downloads = item.get("downloads", 0)
                likes = item.get("likes", 0)
                desc = item.get("description") or ""

                content_parts = []
                if desc:
                    content_parts.append(desc[:200])
                tag_parts = []
                if pipeline:
                    tag_parts.append(pipeline)
                if library:
                    tag_parts.append(library)
                tag_parts.append(f"▼{downloads}")
                tag_parts.append(f"♥{likes}")
                content_parts.append(" | ".join(tag_parts))

                cat = pipeline or "models"

                results.append(
                    SearchResult(
                        url=url,
                        title=title,
                        content=" — ".join(content_parts),
                        engine=self.name,
                        position=idx + 1,
                        score=float(likes),
                        category=cat,
                    ),
                )

            elif is_datasets:
                ds_id = item.get("id", "")
                url = f"https://huggingface.co/datasets/{ds_id}" if ds_id else ""
                title = ds_id
                downloads = item.get("downloads", 0)
                likes = item.get("likes", 0)
                desc = item.get("description") or ""

                content_parts = []
                if desc:
                    content_parts.append(desc[:200])
                content_parts.append(f"▼{downloads} | ♥{likes}")

                results.append(
                    SearchResult(
                        url=url,
                        title=title,
                        content=" — ".join(content_parts),
                        engine=self.name,
                        position=idx + 1,
                        score=float(likes),
                        category="datasets",
                    ),
                )

            else:
                # Papers
                paper_id = item.get("id", "")
                url = item.get("url", f"https://huggingface.co/papers/{paper_id}" if paper_id else "")
                title = item.get("title", paper_id)
                upvotes = item.get("upvotes", 0)
                author = (item.get("authors") or [{}])[0].get("name", "") if item.get("authors") else ""
                published = item.get("publishedAt") or None

                content = f"▲ {upvotes}"
                if author:
                    content += f" | {author}"

                results.append(
                    SearchResult(
                        url=url,
                        title=title,
                        content=content,
                        engine=self.name,
                        position=idx + 1,
                        score=float(upvotes),
                        published_date=published,
                        category="papers",
                    ),
                )

        return results
