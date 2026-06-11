"""Open Library adapter — book search.

Free, public JSON API. No auth required.
Docs: https://openlibrary.org/developers/api
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
class OpenLibraryAdapter(EngineAdapter):
    """Open Library book search."""

    name = "openlibrary"
    display_name = "Open Library"
    env_prefix = "ENGINE_OPENLIBRARY"
    engine_type = "api"
    categories = ["books", "reference"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://openlibrary.org/search.json")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        headers = {
            "User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)",
        }
        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    base_url,
                    params={"q": query, "limit": max_results},
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()
                data = resp.json()

                results = []
                docs = data.get("docs", [])
                for idx, doc in enumerate(docs[:max_results]):
                    title = doc.get("title", "")
                    author = doc.get("author_name", [None])
                    author_name = author[0] if author else ""
                    year = doc.get("first_publish_year", "")
                    isbn = doc.get("isbn", [None])
                    isbn_str = isbn[0] if isbn else ""
                    cover_id = doc.get("cover_i")
                    edition_count = doc.get("edition_count", 0)

                    content = f"By {author_name}" if author_name else ""
                    if year:
                        content += f" ({year})" if content else f"Published {year}"
                    if edition_count:
                        content += f" — {edition_count} editions"

                    thumbnail = None
                    if cover_id:
                        thumbnail = f"https://covers.openlibrary.org/b/id/{cover_id}-M.jpg"

                    results.append(
                        SearchResult(
                            url=f"https://openlibrary.org/isbn/{isbn_str}"
                            if isbn_str
                            else f"https://openlibrary.org/search?q={query}",
                            title=title,
                            content=content[:500],
                            engine=self.name,
                            position=idx + 1,
                            score=float(doc.get("ratings_count", 0) or 0),
                            thumbnail=thumbnail,
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
