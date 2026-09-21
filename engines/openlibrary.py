"""Open Library adapter — book search.

Free, public JSON API. No auth required.
Docs: https://openlibrary.org/developers/api
"""

from __future__ import annotations

import re
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

_WORK_KEY_RE = re.compile(r"^(?:/)?works/(OL\d+W)$")
_EDITION_KEY_RE = re.compile(r"^(?:/)?books/(OL\d+M)$")
_BARE_WORK_KEY_RE = re.compile(r"^(OL\d+W)$")
_BARE_EDITION_KEY_RE = re.compile(r"^(OL\d+M)$")
_ISBN_RE = re.compile(r"^[0-9Xx]{10,13}$")


def _openlibrary_key_url(value: Any, kind: str) -> str | None:
    """Return a canonical Open Library URL for a validated record key."""
    if not isinstance(value, str):
        return None

    value = value.strip()
    if kind == "work":
        match = _WORK_KEY_RE.fullmatch(value) or _BARE_WORK_KEY_RE.fullmatch(value)
        if match:
            return f"https://openlibrary.org/works/{match.group(1)}"
    elif kind == "edition":
        match = _EDITION_KEY_RE.fullmatch(value) or _BARE_EDITION_KEY_RE.fullmatch(value)
        if match:
            return f"https://openlibrary.org/books/{match.group(1)}"
    return None


def _openlibrary_isbn_url(values: Any) -> str | None:
    """Return an ISBN URL for the first safe ISBN in a provider field."""
    if isinstance(values, str):
        values = [values]
    if not isinstance(values, (list, tuple)):
        return None

    for value in values:
        if not isinstance(value, (str, int)):
            continue
        isbn = str(value).replace("-", "").replace(" ", "").strip()
        if _ISBN_RE.fullmatch(isbn):
            return f"https://openlibrary.org/isbn/{isbn}"
    return None


def _result_url(doc: dict[str, Any]) -> str | None:
    """Build a stable, item-specific URL from an Open Library document.

    Work keys are the canonical identity when present. Edition keys and ISBNs
    provide progressively narrower identifiers for records without a work key.
    Documents without one of these stable identifiers are omitted by the
    adapter rather than represented by a non-canonical search URL.
    """
    work_url = _openlibrary_key_url(doc.get("key"), "work")
    if work_url:
        return work_url

    edition_keys = doc.get("edition_key")
    if isinstance(edition_keys, str):
        edition_keys = [edition_keys]
    if isinstance(edition_keys, (list, tuple)):
        for edition_key in edition_keys:
            edition_url = _openlibrary_key_url(edition_key, "edition")
            if edition_url:
                return edition_url

    isbn_url = _openlibrary_isbn_url(doc.get("isbn"))
    if isbn_url:
        return isbn_url

    return None


@register_engine
class OpenLibraryAdapter(EngineAdapter):
    """Open Library book search."""

    name = "openlibrary"
    display_name = "Open Library"
    env_prefix = "ENGINE_OPENLIBRARY"
    engine_type = "api"
    categories = ["books", "reference"]

    # -- Declared capability metadata (audited, issue 185) --
    supported_result_types = ("text", "media")
    failure_classes = ("rate_limited", "error", "timeout")
    cost_class = "free"

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
            async with self.http_client(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    base_url,
                    params={"q": query, "limit": max_results},
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()
                data = resp.json()

                results: list[SearchResult] = []
                docs = data.get("docs", [])
                for doc in docs[:max_results]:
                    result_url = _result_url(doc)
                    if result_url is None:
                        continue

                    title = doc.get("title", "")
                    author = doc.get("author_name", [None])
                    author_name = author[0] if author else ""
                    year = doc.get("first_publish_year", "")
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
                            url=result_url,
                            title=title,
                            content=content[:500],
                            engine=self.name,
                            position=len(results) + 1,
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
