"""Wikipedia API adapter — two-stage opensearch → query pipeline."""

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

_BLOCKED_STATUS_CODES = frozenset({403})
_DIAGNOSTIC_LIMIT = 1000
_MIME_TYPE_RE = re.compile(r"[a-z0-9][a-z0-9!#$&^_.+-]*/[a-z0-9][a-z0-9!#$&^_.+-]*")


class _WikipediaStageError(Exception):
    """Safe, stage-aware failure raised while decoding a provider response."""

    def __init__(
        self,
        stage: str,
        message: str,
        *,
        status_code: int | None = None,
        failure: str = "error",
    ) -> None:
        super().__init__(message)
        self.stage = stage
        self.status_code = status_code
        self.failure = failure


def _content_type(response: httpx.Response | None) -> str:
    if response is None:
        return "unknown"
    value = response.headers.get("content-type", "unknown").split(";", 1)[0].strip().lower()
    if not value:
        return "unknown"
    return value if len(value) <= 64 and _MIME_TYPE_RE.fullmatch(value) else "invalid"


def _shape_signature(value: Any) -> str:
    """Return a body-free response-shape signature for diagnostics."""
    if isinstance(value, list):
        suffix = "+" if len(value) > _DIAGNOSTIC_LIMIT else ""
        return f"list(len={min(len(value), _DIAGNOSTIC_LIMIT)}{suffix})"
    if isinstance(value, dict):
        suffix = "+" if len(value) > _DIAGNOSTIC_LIMIT else ""
        return f"object(keys={min(len(value), _DIAGNOSTIC_LIMIT)}{suffix})"
    if value is None:
        return "null"
    return type(value).__name__


def _stage_diagnostic(
    stage: str,
    response: httpx.Response | None,
    response_shape: str,
    detail: str,
) -> str:
    status = str(response.status_code) if response is not None else "unknown"
    return (
        f"Wikipedia stage={stage} failed: status={status}; "
        f"content_type={_content_type(response)}; response_shape={response_shape}; {detail}"
    )


@register_engine
class WikipediaAdapter(EngineAdapter):
    name = "wikipedia"
    display_name = "Wikipedia"
    env_prefix = "ENGINE_WIKIPEDIA"
    engine_type = "api"
    categories = ["general", "science", "reference"]

    # -- Declared capability metadata (audited, issue 185) --
    supported_result_types = ("text", "corrections", "infoboxes", "media")
    failure_classes = ("rate_limited", "blocked", "error", "timeout")
    cost_class = "free"

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://en.wikipedia.org/w/api.php")
        timeout_ms = cfg.get("timeout_ms", 3_000)
        max_results = cfg.get("max_results", 3)

        headers = {"User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)"}
        start_time = time.monotonic()

        try:
            async with self.http_client(timeout=timeout_ms / 1000.0) as client:
                # Stage 1: opensearch for quick title/suggestion matches
                titles = await self._opensearch(client, base_url, query, max_results, headers)
                if not titles:
                    return AdapterResponse(
                        results=[],
                        status=EngineStatus.OK,
                        latency_ms=(time.monotonic() - start_time) * 1000,
                    )

                # Check for "Did you mean" corrections from opensearch
                corrections = self._check_corrections(query, titles)

                # Stage 2: query extracts, page images, and pageprops for each title
                results, infoboxes = await self._rich_query(client, base_url, titles, headers)
                latency = (time.monotonic() - start_time) * 1000
                return AdapterResponse(
                    results=results,
                    status=EngineStatus.OK,
                    latency_ms=latency,
                    corrections=corrections,
                    infoboxes=infoboxes,
                )

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
        except _WikipediaStageError as exc:
            latency = (time.monotonic() - start_time) * 1000
            if exc.failure == "timeout":
                status = EngineStatus.TIMEOUT
            elif exc.status_code == 429:
                status = EngineStatus.RATE_LIMITED
            elif exc.failure == "blocked":
                status = EngineStatus.BLOCKED
            else:
                status = EngineStatus.ERROR
            return AdapterResponse(
                results=[],
                status=status,
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

    async def _opensearch(
        self,
        client: httpx.AsyncClient,
        base_url: str,
        query: str,
        limit: int,
        headers: dict[str, str],
    ) -> list[str]:
        """Stage 1: fetch quick title suggestions via opensearch."""
        params: dict[str, str | int] = {
            "action": "opensearch",
            "search": query,
            "limit": limit,
            "format": "json",
            "origin": "*",
        }
        try:
            resp = await client.get(base_url, params=params, headers=headers)
            resp.raise_for_status()
        except httpx.TimeoutException as exc:
            raise _WikipediaStageError(
                "opensearch",
                _stage_diagnostic("opensearch", None, "unavailable", "timeout"),
                failure="timeout",
            ) from exc
        except httpx.HTTPStatusError as exc:
            raise _WikipediaStageError(
                "opensearch",
                _stage_diagnostic("opensearch", exc.response, "unparsed", "provider HTTP error"),
                status_code=exc.response.status_code,
                failure=("blocked" if exc.response.status_code in _BLOCKED_STATUS_CODES else "error"),
            ) from exc
        except httpx.HTTPError as exc:
            raise _WikipediaStageError(
                "opensearch",
                _stage_diagnostic("opensearch", None, "unavailable", "transport error"),
            ) from exc

        try:
            data = resp.json()
        except ValueError as exc:
            raise _WikipediaStageError(
                "opensearch",
                _stage_diagnostic("opensearch", resp, "malformed_json", "malformed JSON"),
            ) from exc

        # opensearch returns [query, [titles], [urls], [snippets]]
        if not isinstance(data, list) or len(data) < 2 or not isinstance(data[1], list):
            raise _WikipediaStageError(
                "opensearch",
                _stage_diagnostic("opensearch", resp, _shape_signature(data), "unexpected JSON shape"),
            )
        return [t for t in data[1] if isinstance(t, str)][:limit]

    async def _rich_query(
        self,
        client: httpx.AsyncClient,
        base_url: str,
        titles: list[str],
        headers: dict[str, str],
    ) -> tuple[list[SearchResult], list[dict[str, Any]]]:
        """Stage 2: fetch extracts, thumbnails, and pageprops for resolved titles.

        Returns:
            Tuple of (search_results, infoboxes).
        """
        if not titles:
            return [], []

        params: dict[str, str] = {
            "action": "query",
            "titles": "|".join(titles),
            "prop": "extracts|pageimages|pageprops",
            "exintro": "1",
            "explaintext": "1",
            "pithumbsize": "300",
            "format": "json",
            "origin": "*",
        }
        try:
            resp = await client.get(base_url, params=params, headers=headers)
            resp.raise_for_status()
        except httpx.TimeoutException as exc:
            raise _WikipediaStageError(
                "rich_query",
                _stage_diagnostic("rich_query", None, "unavailable", "timeout"),
                failure="timeout",
            ) from exc
        except httpx.HTTPStatusError as exc:
            raise _WikipediaStageError(
                "rich_query",
                _stage_diagnostic("rich_query", exc.response, "unparsed", "provider HTTP error"),
                status_code=exc.response.status_code,
                failure=("blocked" if exc.response.status_code in _BLOCKED_STATUS_CODES else "error"),
            ) from exc
        except httpx.HTTPError as exc:
            raise _WikipediaStageError(
                "rich_query",
                _stage_diagnostic("rich_query", None, "unavailable", "transport error"),
            ) from exc

        try:
            data = resp.json()
        except ValueError as exc:
            raise _WikipediaStageError(
                "rich_query",
                _stage_diagnostic("rich_query", resp, "malformed_json", "malformed JSON"),
            ) from exc

        if not isinstance(data, dict) or not isinstance(data.get("query"), dict):
            raise _WikipediaStageError(
                "rich_query",
                _stage_diagnostic("rich_query", resp, _shape_signature(data), "unexpected JSON shape"),
            )
        pages = data["query"].get("pages")
        if not isinstance(pages, dict):
            raise _WikipediaStageError(
                "rich_query",
                _stage_diagnostic("rich_query", resp, _shape_signature(data), "unexpected JSON shape"),
            )

        results: list[SearchResult] = []
        infoboxes: list[dict[str, Any]] = []
        for idx, (page_id, page) in enumerate(pages.items()):
            if page_id == "-1":
                continue  # missing page
            title = page.get("title", "")
            extract = page.get("extract", "")
            thumbnail = None
            thumb_data = page.get("thumbnail")
            if isinstance(thumb_data, dict):
                thumbnail = thumb_data.get("source")

            # Clean extract
            clean = re.sub(r"\s+", " ", extract).strip()
            # Truncate to first 200 chars for snippet
            if len(clean) > 200:
                clean = clean[:200] + "…"

            page_url = f"https://en.wikipedia.org/wiki/{title.replace(' ', '_')}"
            results.append(
                SearchResult(
                    url=page_url,
                    title=title,
                    content=clean,
                    engine=self.name,
                    position=idx + 1,
                    thumbnail=thumbnail,
                ),
            )

            # Build infobox from pageprops
            pageprops = page.get("pageprops") or {}
            if isinstance(pageprops, dict) and pageprops.get("wikibase-shortdesc"):
                infoboxes.append(
                    {
                        "id": f"wiki:{title.replace(' ', '_')}",
                        "title": title,
                        "content": pageprops.get("wikibase-shortdesc", ""),
                        "img_src": thumbnail or "",
                        "url": page_url,
                        "urls": [{"title": "Wikipedia", "url": page_url}],
                    }
                )

        return results, infoboxes

    @staticmethod
    def _check_corrections(query: str, titles: list[str]) -> list[str]:
        """Detect redirect-based corrections from opensearch.

        If the first returned title differs from the query, surface
        it as a "Did you mean" correction.
        """
        if not titles:
            return []
        first = titles[0].lower().strip()
        q = query.lower().strip()
        if first != q and q not in first and first not in q:
            return [titles[0]]
        return []
