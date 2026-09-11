"""Exa Search API adapter.

Exa is a commercial web-search API whose result records can carry
*highlights* — passages selected from the crawled page text. This adapter
requests highlights only, and treats them as the result snippet.

Three deliberate boundaries hold this adapter inside SlopSearX's
search-only contract (``docs/RETRIEVAL_HANDOFF.md`` §1, §9):

1. **No page bodies.** ``contents.text`` is never requested, and a ``text``
   field is ignored if the upstream ever returns one unasked. SlopSearX
   returns leads, not captured page content; fetching and extracting a page
   belongs to the downstream retriever.
2. **No generated prose.** ``contents.summary`` is never requested and a
   ``summary`` field is ignored. Highlights are passages taken from the
   page; a summary is model-authored. Only retrieved text reaches
   ``SearchResult.content``, so a snippet is never a synthesized claim.
3. **Bounded cost and payload.** The search type is pinned to Exa's
   standard tier, the highlight length is a fixed bound, and the result
   count is clamped to the largest value with a documented basis, so no
   configuration can escalate a search into Exa's expensive deep/agentic
   tiers or past its documented result ceiling.

Operator-tunable settings are the standard engine fields —
``ENGINE_EXA_MAX_RESULTS``, ``ENGINE_EXA_TIMEOUT_MS``,
``ENGINE_EXA_BASE_URL``, ``ENGINE_EXA_ENABLED``, ``ENGINE_EXA_CATEGORIES``
— resolved through the layered config in ``slopsearx/config.py``. The
cost-shaping choices below are deliberately *not* configurable: they are
the safety bounds this evaluation adapter is being reviewed against.

The engine requires ``ENGINE_EXA_API_KEY``. Without it the adapter is
inert: it is registered and visible in the capability catalog, reports
``auth_class="required"`` / ``auth_configured=false``, and is hard-excluded
from cost/coverage routing (``slopsearx/routing.py``), so an operator opts
in by supplying a credential rather than opting out of a default. A key that
is not printable ASCII — a stray newline from a secret file, say — is refused
before it reaches the wire rather than being echoed back in a transport error;
see :func:`_is_wire_safe_credential`.

Tier 2 (specialised) by default, per the tier governance in
``CONTRIBUTING.md``. Promotion to Tier 1 needs maintainer approval and
live evidence.

Filter declaration: ``date_from``/``date_to``/``time_range`` are consumed
and forwarded upstream as publication-date bounds, so ``supported_filters``
records the consumption. ``enforced_filters`` stays **empty**: enforcement
is only declared once the layer has been audited against live responses,
and an unenforced filter must never be reported as enforced
(``AGENTS.md`` §8).
"""

from __future__ import annotations

import datetime as _dt
import os
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
from slopsearx.filters import DateFilterError, publication_date_bounds, time_range_window

_DEFAULT_BASE_URL = "https://api.exa.ai"
_SEARCH_PATH = "/search"
_DEFAULT_TIMEOUT_MS = 8_000
_DEFAULT_MAX_RESULTS = 10
# Hard ceiling on results requested per search. Exa's SDK documents
# ``num_results`` as "Default: 10, Max for basic: 10", its published per-request
# price includes up to 10 results, and its schema notes that limits vary by
# search type without publishing the per-type values. 10 is therefore the
# largest count with a documented basis — both a spend bound and a bound on
# requesting a count the upstream may reject.
_MAX_RESULTS_CEILING = 10
# Maximum characters requested per highlight, and the bound applied to the
# assembled snippet. Highlights are the only content this adapter asks for,
# so this is both the payload bound and the content-unit cost bound. 500
# matches the snippet bound the other API adapters truncate to.
_SNIPPET_CHARS = 500
_HIGHLIGHT_SEPARATOR = " … "
_ERROR_MESSAGE_LIMIT = 200
# Bounds on the sibling fields copied straight from the upstream record. Only
# ``content`` was bounded before; a degenerate response could otherwise write
# multi-megabyte titles and URLs into the shared cache and snapshot store.
_TITLE_CHARS = 500
_PUBLISHED_DATE_CHARS = 64
# Practical maximum URL length. A longer value is not a usable fetch target,
# so the record is dropped rather than truncated into a different URL.
_MAX_URL_CHARS = 2_048

# Exa's standard search tier. ``deep-lite``, ``deep`` and ``deep-reasoning``
# run multi-query agentic retrieval with model synthesis at a much higher
# per-request cost, and emit synthesized ``output`` prose this adapter would
# have to discard. The tier is pinned rather than configurable so no
# deployment can escalate a routine search into that spend.
_SEARCH_TYPE = "auto"

# SlopSearX category → Exa ``category`` focus. Only mappings with an exact
# Exa counterpart are declared; an unmapped category sends no ``category``
# field, letting Exa search the open web.
_CATEGORY_FOCUS: dict[str, str] = {"news": "news"}


def _is_wire_safe_credential(api_key: str) -> bool:
    """Whether a credential can be sent as an HTTP header value at all.

    A key carrying a control character — the shape a secret file or a wrapped
    paste produces — makes h11 reject the request with an exception whose text
    embeds the **entire header value**. That text would otherwise reach
    ``error_message`` and, through it, unauthenticated ``/search`` callers. So
    a credential that is not printable ASCII never reaches the wire.
    """
    return bool(api_key) and all("\x21" <= char <= "\x7e" for char in api_key)


def _redact(message: str, secret: str) -> str:
    """Remove a secret from an error string before it is surfaced.

    Defence in depth behind :func:`_is_wire_safe_credential`: any future path
    that folds a header or URL into an exception message is covered too.
    """
    return message.replace(secret, "<redacted>") if secret else message


def _bounded_int(value: Any, default: int, minimum: int, maximum: int) -> int:
    """Coerce a configured value to an int clamped to ``[minimum, maximum]``.

    Configuration arrives from YAML and environment variables, so a
    non-numeric or out-of-range value must degrade to the safe default
    rather than reaching the wire as an unbounded request.
    """
    try:
        parsed = int(value)
    except (TypeError, ValueError, OverflowError):
        # OverflowError covers an infinite float, which YAML produces from
        # ``.inf`` — an adapter must classify that, never raise out of search().
        return default
    return max(minimum, min(maximum, parsed))


def _iso_day_start(day: _dt.date) -> str:
    """Render an inclusive lower publication bound as an RFC 3339 instant."""
    return f"{day.isoformat()}T00:00:00.000Z"


def _iso_day_end(day: _dt.date) -> str:
    """Render an inclusive upper publication bound as an RFC 3339 instant."""
    return f"{day.isoformat()}T23:59:59.999Z"


def _publication_window(params: dict[str, Any]) -> tuple[str | None, str | None]:
    """Resolve Exa publication-date bounds from the shared filter params.

    Explicit ``date_from``/``date_to`` win. When neither is supplied and a
    recognised ``time_range`` is, the relative window is expanded into the
    same absolute bounds. An unrecognised ``time_range`` yields no window —
    the adapter never fabricates a bound for a vocabulary it cannot map.

    Raises:
        DateFilterError: if explicit bounds are malformed or contradictory.
            The caller classifies this without raising.
    """
    start, end = publication_date_bounds(params.get("date_from"), params.get("date_to"))
    if start is None and end is None:
        time_range = params.get("time_range")
        if isinstance(time_range, str) and time_range:
            window = time_range_window(time_range)
            if window is not None:
                start, end = window
    return (
        _iso_day_start(start) if start is not None else None,
        _iso_day_end(end) if end is not None else None,
    )


def _category_focus(categories: Any) -> str | None:
    """Map the requested categories to an Exa ``category`` focus, or ``None``."""
    if not isinstance(categories, list):
        return None
    for category in categories:
        if isinstance(category, str) and category in _CATEGORY_FOCUS:
            return _CATEGORY_FOCUS[category]
    return None


def _snippet(item: dict[str, Any], limit: int) -> str:
    """Build the result snippet from Exa highlights only.

    Highlights are passages selected from the crawled page, so they are
    retrieved text. ``text`` (the full page body) and ``summary``
    (model-authored) are ignored even when present, keeping the snippet
    inside the search-only boundary and free of generated prose.
    """
    highlights = item.get("highlights")
    if not isinstance(highlights, list):
        return ""
    # Accumulate only up to the bound. Joining the whole array first would let
    # an upstream that ignores ``maxCharacters`` drive transient allocation and
    # CPU on the event loop by the size of the response rather than the bound.
    passages: list[str] = []
    length = 0
    for passage in highlights:
        if not isinstance(passage, str):
            continue
        stripped = passage.strip()
        if not stripped:
            continue
        passages.append(stripped[:limit])
        length += len(stripped) + (len(_HIGHLIGHT_SEPARATOR) if len(passages) > 1 else 0)
        if length >= limit:
            break
    if not passages:
        return ""
    return _HIGHLIGHT_SEPARATOR.join(passages)[:limit]


@register_engine
class ExaAdapter(EngineAdapter):
    """Exa Search API adapter (evaluation candidate — issue #272)."""

    name = "exa"
    display_name = "Exa Search API"
    env_prefix = "ENGINE_EXA"
    engine_type = "api"
    categories = ["general", "news"]

    # -- Declared capability metadata (audited, issue 185) --
    # Only ``text`` is populated: highlights become the snippet, and no
    # answer/infobox/correction/media field is ever produced.
    supported_result_types = ("text",)
    # Publication-date parameters are consumed and forwarded upstream. This
    # records consumption only — see the module docstring on why
    # ``enforced_filters`` stays empty until a live audit.
    supported_filters = {"date_from": True, "date_to": True, "time_range": True}
    failure_classes = ("rate_limited", "blocked", "error", "timeout", "unavailable")
    # Exa issues recurring monthly credits on signup and bills beyond them.
    cost_class = "freemium"

    def _resolve_api_key(self) -> str:
        """Return the configured key, falling back to the environment.

        Resolution happens per search rather than at construction so a key
        exported after engine discovery is still picked up, and the resolved
        value is memoized back onto the instance config. A whitespace-only
        value is not a credential and resolves to empty.
        """
        api_key = str(self.config.get("api_key") or "").strip()
        if not api_key and self.env_prefix:
            env_key = os.environ.get(f"{self.env_prefix}_API_KEY", "").strip()
            if env_key:
                self.config["api_key"] = env_key
                api_key = env_key
        return api_key

    def _build_request(self, query: str, params: dict[str, Any]) -> tuple[str, dict[str, Any]]:
        """Build the endpoint URL and JSON body for one search.

        Raises:
            DateFilterError: if the requested publication bounds are invalid.
        """
        cfg = self.config
        base_url = str(cfg.get("base_url") or _DEFAULT_BASE_URL).rstrip("/")
        max_results = _bounded_int(
            cfg.get("max_results", _DEFAULT_MAX_RESULTS), _DEFAULT_MAX_RESULTS, 1, _MAX_RESULTS_CEILING
        )

        body: dict[str, Any] = {
            "query": query,
            "numResults": max_results,
            "type": _SEARCH_TYPE,
            # Highlights only: no page body, no model-authored summary.
            "contents": {"highlights": {"query": query, "maxCharacters": _SNIPPET_CHARS}},
        }
        focus = _category_focus(params.get("categories"))
        if focus is not None:
            body["category"] = focus
        start, end = _publication_window(params)
        if start is not None:
            body["startPublishedDate"] = start
        if end is not None:
            body["endPublishedDate"] = end
        return f"{base_url}{_SEARCH_PATH}", body

    def _classify(self, status_code: int, latency_ms: float) -> AdapterResponse | None:
        """Map a non-success HTTP status to a classified response, or ``None``.

        ``None`` means the status is a success and parsing should proceed.
        This mapping is SlopSearX's, not a restatement of vendor semantics.
        ``402`` (Exa's documented "credits exhausted or spending budget
        exceeded") classifies as ``BLOCKED`` rather than ``RATE_LIMITED``:
        it is an account condition an operator must act on, not a throttle
        that clears on its own.
        """
        if status_code == 429:
            return AdapterResponse(
                results=[], status=EngineStatus.RATE_LIMITED, error_message="rate limited", latency_ms=latency_ms
            )
        if status_code == 402:
            return AdapterResponse(
                results=[],
                status=EngineStatus.BLOCKED,
                error_message="credit or spending budget exhausted",
                latency_ms=latency_ms,
            )
        if status_code == 401:
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR, error_message="authentication rejected", latency_ms=latency_ms
            )
        if status_code == 403:
            return AdapterResponse(
                results=[], status=EngineStatus.BLOCKED, error_message="forbidden", latency_ms=latency_ms
            )
        if status_code >= 500:
            return AdapterResponse(
                results=[],
                status=EngineStatus.UNAVAILABLE,
                error_message=f"upstream returned {status_code}",
                latency_ms=latency_ms,
            )
        if status_code >= 400:
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=f"upstream returned {status_code}",
                latency_ms=latency_ms,
            )
        return None

    def _parse_results(self, data: Any, limit: int) -> list[SearchResult]:
        """Parse an Exa search response body into normalized results.

        Every field is defensively typed: a malformed item, a non-string URL
        or a missing field yields a skipped or empty-valued entry rather than
        an exception. A record without a usable URL is dropped — it is not a
        lead a downstream retriever can act on.
        """
        if not isinstance(data, dict):
            return []
        raw = data.get("results")
        if not isinstance(raw, list):
            return []
        results: list[SearchResult] = []
        for item in raw:
            if not isinstance(item, dict):
                continue
            url = item.get("url")
            if not isinstance(url, str) or not url.strip() or len(url) > _MAX_URL_CHARS:
                continue
            title = item.get("title")
            published = item.get("publishedDate")
            published_date = published[:_PUBLISHED_DATE_CHARS] if isinstance(published, str) and published else None
            results.append(
                SearchResult(
                    url=url,
                    title=title[:_TITLE_CHARS] if isinstance(title, str) else "",
                    content=_snippet(item, _SNIPPET_CHARS),
                    engine=self.name,
                    position=len(results) + 1,
                    published_date=published_date,
                )
            )
            if len(results) >= limit:
                break
        return results

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        """Execute an Exa search. Never raises — errors are classified."""
        started = time.monotonic()
        search_params = params or {}

        api_key = self._resolve_api_key()
        if not api_key:
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message="Exa API key not configured",
            )
        if not _is_wire_safe_credential(api_key):
            # Never send it: the transport would echo the whole header value
            # back in an exception message. See _is_wire_safe_credential.
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message="Exa API key is malformed (must be printable ASCII without whitespace)",
            )

        if early := await self._check_rate_limit():
            early.latency_ms = (time.monotonic() - started) * 1000
            return early

        try:
            endpoint, body = self._build_request(query, search_params)
        except DateFilterError as exc:
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc)[:_ERROR_MESSAGE_LIMIT],
                latency_ms=(time.monotonic() - started) * 1000,
            )

        timeout_ms = _bounded_int(self.config.get("timeout_ms", _DEFAULT_TIMEOUT_MS), _DEFAULT_TIMEOUT_MS, 1, 60_000)
        headers = {
            "Accept": "application/json",
            "Content-Type": "application/json",
            "x-api-key": api_key,
        }

        limit = int(body["numResults"])
        try:
            async with self.http_client(timeout=timeout_ms / 1000.0) as client:
                resp = await client.post(endpoint, headers=headers, json=body)
                latency = (time.monotonic() - started) * 1000
                classified = self._classify(resp.status_code, latency)
                if classified is not None:
                    return classified
                data = resp.json()
            return AdapterResponse(
                results=self._parse_results(data, limit),
                status=EngineStatus.OK,
                latency_ms=latency,
            )
        except httpx.TimeoutException:
            return AdapterResponse(
                results=[],
                status=EngineStatus.TIMEOUT,
                error_message="request timed out",
                latency_ms=(time.monotonic() - started) * 1000,
            )
        except Exception as exc:  # noqa: BLE001 — adapters never raise
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=_redact(str(exc), api_key)[:_ERROR_MESSAGE_LIMIT],
                latency_ms=(time.monotonic() - started) * 1000,
            )
