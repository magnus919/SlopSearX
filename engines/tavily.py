"""Tavily Search API adapter.

Tavily is a commercial search API built for retrieval-augmented agents. Its
result records carry a ``content`` field — an extracted passage from the
indexed page — and the response can optionally carry an ``answer``, which is
model-authored prose synthesized from those results.

Three deliberate boundaries hold this adapter inside SlopSearX's search-only
contract (``docs/RETRIEVAL_HANDOFF.md`` §1, §9):

1. **No page bodies.** ``include_raw_content`` is never requested, and a
   ``raw_content`` field is ignored if the upstream ever returns one
   unasked. SlopSearX returns leads, not captured page content; fetching
   and extracting a page belongs to the downstream retriever.
2. **No generated prose.** ``include_answer`` is never requested and the
   ``answer`` field is ignored. Only ``results[].content`` — an extracted
   passage — reaches ``SearchResult.content``, so a snippet is never a
   synthesized claim. (Should a maintainer later want the answer, it
   belongs on the separate ``AdapterResponse.answers`` channel with an
   explicit model-authored label, behind an operator-visible switch;
   ``EngineEntry`` has no field for one today, and a knob that cannot be
   reached from configuration would be dead code.)
3. **Bounded cost and payload.** The search depth is pinned to ``basic``,
   the snippet length is a fixed bound, and the result count is clamped to a
   ceiling, so no configuration can escalate per-request credit spend.

Operator-tunable settings are the standard engine fields —
``ENGINE_TAVILY_MAX_RESULTS``, ``ENGINE_TAVILY_TIMEOUT_MS``,
``ENGINE_TAVILY_BASE_URL``, ``ENGINE_TAVILY_ENABLED``,
``ENGINE_TAVILY_CATEGORIES`` — resolved through the layered config in
``slopsearx/config.py``. The cost-shaping choices below are deliberately
*not* configurable: they are the safety bounds this adapter is reviewed
against.

The engine requires ``ENGINE_TAVILY_API_KEY``. Without it the adapter is
inert: registered and visible in the capability catalog, reporting
``auth_class="required"`` / ``auth_configured=false``, and hard-excluded
from cost/coverage routing (``slopsearx/routing.py``), so an operator opts
in by supplying a credential rather than opting out of a default. A key that
is not printable ASCII — a stray newline from a secret file, say — is refused
before it reaches the wire rather than being echoed back in a transport error;
see :func:`_is_wire_safe_credential`.

Tier 2 (specialised) by default, per the tier governance in
``CONTRIBUTING.md``.

Filter declaration: ``date_from``/``date_to``/``time_range`` are consumed
and forwarded upstream — Tavily's ``time_range`` vocabulary
(``day``/``week``/``month``/``year``) is identical to SearXNG's, so the
mapping is direct. ``supported_filters`` records that consumption;
``enforced_filters`` stays **empty** until the enforcement layer has been
audited against live responses, because an unenforced filter must never be
reported as enforced (``AGENTS.md`` §8).
"""

from __future__ import annotations

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
from slopsearx.filters import DateFilterError, publication_date_bounds

_DEFAULT_BASE_URL = "https://api.tavily.com"
_SEARCH_PATH = "/search"
_DEFAULT_TIMEOUT_MS = 8_000
_DEFAULT_MAX_RESULTS = 10
# Hard ceiling on results requested per search — a spend bound, since Tavily
# bills per API credit.
_MAX_RESULTS_CEILING = 20
# Bound applied to each result snippet before it enters the normalized model.
# 500 matches the snippet bound the other API adapters truncate to.
_SNIPPET_CHARS = 500
_ERROR_MESSAGE_LIMIT = 200
# Bounds on the sibling fields copied straight from the upstream record. Only
# ``content`` was bounded before; a degenerate response could otherwise write
# multi-megabyte titles and URLs into the shared cache and snapshot store.
_TITLE_CHARS = 500
_PUBLISHED_DATE_CHARS = 64
# Practical maximum URL length. A longer value is not a usable fetch target,
# so the record is dropped rather than truncated into a different URL.
_MAX_URL_CHARS = 2_048

# Pinned search tier. ``basic`` is the depth whose credit cost is documented
# and the cheaper of the two documented depths — ``advanced`` costs more per
# request. The vendor SDK also accepts ``fast`` and ``ultra-fast``, whose
# credit costs are not published, so they are not selected either. The tier is
# pinned rather than configurable so no deployment can escalate spend.
_SEARCH_DEPTH = "basic"

# SlopSearX category → Tavily ``topic``. Only exact counterparts are mapped;
# an unmapped category sends no ``topic`` and uses Tavily's default.
_CATEGORY_TOPIC: dict[str, str] = {"news": "news", "finance": "finance"}

# Tavily's relative-window vocabulary, which matches SearXNG's exactly.
_TIME_RANGES: frozenset[str] = frozenset({"day", "week", "month", "year"})


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


def _topic(categories: Any) -> str | None:
    """Map the requested categories to a Tavily ``topic``, or ``None``."""
    if not isinstance(categories, list):
        return None
    for category in categories:
        if isinstance(category, str) and category in _CATEGORY_TOPIC:
            return _CATEGORY_TOPIC[category]
    return None


@register_engine
class TavilyAdapter(EngineAdapter):
    """Tavily Search API adapter."""

    name = "tavily"
    display_name = "Tavily Search API"
    env_prefix = "ENGINE_TAVILY"
    engine_type = "api"
    categories = ["general", "news"]

    # -- Declared capability metadata (audited, issue 185) --
    # Only ``text`` is populated: the adapter never requests Tavily's
    # generated answer, so no answer/infobox/correction/media field is
    # produced.
    supported_result_types = ("text",)
    # Relative and absolute publication windows are consumed and forwarded
    # upstream. This records consumption only — see the module docstring on
    # why ``enforced_filters`` stays empty until a live audit.
    supported_filters = {"date_from": True, "date_to": True, "time_range": True}
    failure_classes = ("rate_limited", "blocked", "error", "timeout", "unavailable")
    # Tavily issues a monthly free credit allowance and bills beyond it.
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
            "max_results": max_results,
            "search_depth": _SEARCH_DEPTH,
            # Credit-bearing extras are pinned off. Generated answers, page
            # bodies and image search are never requested.
            "include_answer": False,
            "include_raw_content": False,
            "include_images": False,
        }
        topic = _topic(params.get("categories"))
        if topic is not None:
            body["topic"] = topic
        time_range = params.get("time_range")
        if isinstance(time_range, str) and time_range in _TIME_RANGES:
            body["time_range"] = time_range
        start, end = publication_date_bounds(params.get("date_from"), params.get("date_to"))
        if start is not None:
            body["start_date"] = start.isoformat()
        if end is not None:
            body["end_date"] = end.isoformat()
        return f"{base_url}{_SEARCH_PATH}", body

    def _classify(self, status_code: int, latency_ms: float) -> AdapterResponse | None:
        """Map a non-success HTTP status to a classified response, or ``None``.

        This mapping is SlopSearX's, not a restatement of vendor semantics.
        ``429`` is a throttle. ``432``/``433`` are plan and usage-limit
        exhaustion, which the vendor SDK groups with ``403`` as a forbidden
        condition — an account state an operator must act on rather than a
        throttle that clears on its own — so they classify as ``BLOCKED``.
        """
        if status_code == 429:
            return AdapterResponse(
                results=[], status=EngineStatus.RATE_LIMITED, error_message="rate limited", latency_ms=latency_ms
            )
        if status_code in (432, 433):
            return AdapterResponse(
                results=[],
                status=EngineStatus.BLOCKED,
                error_message="plan or usage limit exceeded",
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
        """Parse a Tavily search response body into normalized results.

        Every field is defensively typed: a malformed item, a non-string URL
        or a missing field yields a skipped or empty-valued entry rather than
        an exception. A record without a usable URL is dropped — it is not a
        lead a downstream retriever can act on. ``raw_content`` and the
        top-level ``answer`` are ignored even when present, so neither a page
        body nor generated prose can become a snippet.
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
            content = item.get("content")
            published = item.get("published_date")
            published_date = published[:_PUBLISHED_DATE_CHARS] if isinstance(published, str) and published else None
            results.append(
                SearchResult(
                    url=url,
                    title=title[:_TITLE_CHARS] if isinstance(title, str) else "",
                    content=content[:_SNIPPET_CHARS] if isinstance(content, str) else "",
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
        """Execute a Tavily search. Never raises — errors are classified."""
        started = time.monotonic()
        search_params = params or {}

        api_key = self._resolve_api_key()
        if not api_key:
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message="Tavily API key not configured",
            )
        if not _is_wire_safe_credential(api_key):
            # Never send it: the transport would echo the whole header value
            # back in an exception message. See _is_wire_safe_credential.
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message="Tavily API key is malformed (must be printable ASCII without whitespace)",
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
            "Authorization": f"Bearer {api_key}",
        }
        limit = int(body["max_results"])

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
