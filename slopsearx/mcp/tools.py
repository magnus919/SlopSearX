"""MCP tool implementations for SlopSearX.

Each function is a plain async callable (FastMCP-free) so the logic is
testable without an MCP runtime; ``slopsearx.mcp.server`` registers
them with FastMCP. Tools return JSON-serializable dicts — the typed
envelope described in docs/MCP_SERVER_DESIGN.md §3.
"""

from __future__ import annotations

import dataclasses
import datetime as _dt
import hashlib
import json
import time
from typing import Any

from pydantic import StrictInt

from slopsearx.adapter import OBSERVED_STATUS_VOCAB, SUPPORTED_MEDIA_TYPES
from slopsearx.capabilities import INTENT_PROFILES, build_engine_health, resolve_intent
from slopsearx.filters import (
    DateFilterError,
    enforcement_entry,
    engine_filter_layer,
    publication_date_bounds,
    resolve_filter_enforcement,
)
from slopsearx.mcp.entity_projection import ENTITY_CONTRACT, ENTITY_VERSION, entity_groups
from slopsearx.mcp.result_serialization import (
    CONTENT_UNAVAILABLE_NOTE as CONTENT_UNAVAILABLE_NOTE,
)
from slopsearx.mcp.result_serialization import (
    NON_VERIFICATION_NOTE as NON_VERIFICATION_NOTE,
)

# Compatibility exports for callers that imported these helpers from tools.
from slopsearx.mcp.result_serialization import (
    RANKING_EXPLANATION as RANKING_EXPLANATION,
)
from slopsearx.mcp.result_serialization import (
    RETRIEVAL_HANDOFF_CONTRACT as RETRIEVAL_HANDOFF_CONTRACT,
)
from slopsearx.mcp.result_serialization import (
    RETRIEVAL_HANDOFF_VERSION as RETRIEVAL_HANDOFF_VERSION,
)
from slopsearx.mcp.result_serialization import (
    SNIPPET_LENGTH as SNIPPET_LENGTH,
)
from slopsearx.mcp.result_serialization import (
    _media_triage as _media_triage,
)
from slopsearx.mcp.result_serialization import (
    _payload_for_record as _payload_for_record,
)
from slopsearx.mcp.result_serialization import (
    _payload_inline as _payload_inline,
)
from slopsearx.mcp.result_serialization import (
    _payload_size as _payload_size,
)
from slopsearx.mcp.result_serialization import (
    _result_record as _result_record,
)
from slopsearx.mcp.result_serialization import (
    _result_to_dict as _result_to_dict,
)
from slopsearx.mcp.result_serialization import (
    _retrieval_card as _retrieval_card,
)
from slopsearx.mcp.result_serialization import (
    _retrieval_handoff as _retrieval_handoff,
)
from slopsearx.mcp.result_serialization import (
    _source_engines as _source_engines,
)
from slopsearx.mcp.retrieval_url import (
    RETRIEVAL_DEPRECATED_SITE_LOCAL_V6 as RETRIEVAL_DEPRECATED_SITE_LOCAL_V6,
)
from slopsearx.mcp.retrieval_url import (
    RETRIEVAL_PORT_MAX as RETRIEVAL_PORT_MAX,
)
from slopsearx.mcp.retrieval_url import (
    RETRIEVAL_SIXTOFOUR_V6 as RETRIEVAL_SIXTOFOUR_V6,
)
from slopsearx.mcp.retrieval_url import (
    RETRIEVAL_URL_STATUS_AMBIGUOUS as RETRIEVAL_URL_STATUS_AMBIGUOUS,
)
from slopsearx.mcp.retrieval_url import (
    RETRIEVAL_URL_STATUS_MISSING as RETRIEVAL_URL_STATUS_MISSING,
)
from slopsearx.mcp.retrieval_url import (
    RETRIEVAL_URL_STATUS_NON_HTTP as RETRIEVAL_URL_STATUS_NON_HTTP,
)
from slopsearx.mcp.retrieval_url import (
    RETRIEVAL_URL_STATUS_OK as RETRIEVAL_URL_STATUS_OK,
)
from slopsearx.mcp.retrieval_url import (
    RETRIEVAL_URL_STATUS_UNSAFE as RETRIEVAL_URL_STATUS_UNSAFE,
)
from slopsearx.mcp.retrieval_url import (
    RETRIEVAL_URL_STATUSES as RETRIEVAL_URL_STATUSES,
)
from slopsearx.mcp.retrieval_url import (
    UNSAFE_RETRIEVAL_SCHEMES as UNSAFE_RETRIEVAL_SCHEMES,
)
from slopsearx.mcp.retrieval_url import (
    _ip_literal_candidates as _ip_literal_candidates,
)
from slopsearx.mcp.retrieval_url import (
    _ipv4_component_value as _ipv4_component_value,
)
from slopsearx.mcp.retrieval_url import (
    _retrieval_url as _retrieval_url,
)
from slopsearx.mcp.retrieval_url import (
    _whatwg_ipv4_literal as _whatwg_ipv4_literal,
)
from slopsearx.mcp.state import McpState, current_tenant, get_state
from slopsearx.ratelimit import ValkeySlidingWindow
from slopsearx.research import (
    JobStillRunningError,
    LeaseLostError,
    ResearchJob,
    ResearchQuery,
    generate_job_id,
    is_retryable_query,
    plan_research_queries,
    summarize_coverage,
)
from slopsearx.research_budget import ResearchMutationError, budget_summary, initialize_budget
from slopsearx.service import (
    QueryValidationError,
    RateLimitExceededError,
    ScopeDecision,
    ScopeResolver,
    SearchRequest,
)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

VALID_INTENTS = tuple(INTENT_PROFILES)
VALID_MEDIA_TYPES = SUPPORTED_MEDIA_TYPES
VALID_SAFESEARCH = ("off", "moderate", "strict")
VALID_FRESHNESS = ("prefer_cache", "prefer_fresh", "no_preference")
VALID_STRATEGIES = ("triangulate", "broad", "fresh", "counterevidence")

# The sensitive-engine grant. This is the SINGLE grant that permits a
# sensitive engine to be dispatched on any path (schema pin); the
# specialist grants never grant sensitive access by themselves.
SENSITIVE_GRANT = "MCP_TARGETED_SENSITIVE_ALLOWED"

# Intent → required specialist grant. Generic search reaches these
# intents through the shared policy gate, so a disabled specialist grant
# also blocks the corresponding intent (VAL-SPEC-017/018).
GRANT_ENV = {
    "jobs": "MCP_GRANT_JOBS",
    "security": "MCP_GRANT_SECURITY",
    "science": "MCP_GRANT_SCIENCE",
}
INTENT_GRANTS: dict[str, str] = {
    "jobs": "jobs",
    "security": "security",
    "science": "science",
}

# Strict SafeSearch is all-or-nothing: the whole selected scope must enforce
# it upstream, so a single non-enforcing engine fails the request closed.
# The rejection message is derived from the actual per-engine layers, never
# from a static claim, so mixed scopes are described accurately.

# Evidence-type → engine profile for the security search tool.
EVIDENCE_TYPE_ENGINES: dict[str, list[str]] = {
    "vulnerability": ["cve", "nvd", "epss", "vulncheck", "exploitdb"],
    "exposure": ["shodan", "censys", "crtsh", "urlhaus", "abuseipdb", "intelx", "dehashed"],
    "reputation": ["otx", "greynoise", "abuseipdb", "virustotal", "hibp"],
    "malware": ["virustotal", "urlhaus", "abuseipdb"],
    "threat_intel": ["otx", "intelx", "greynoise", "mitreattack"],
    "exploit": ["exploitdb", "cve", "nvd"],
}

# Source-type → engine profile for the science search tool.
SOURCE_TYPE_ENGINES: dict[str, list[str]] = {
    "papers": ["arxiv", "semanticscholar", "openalex"],
    "scholarly_index": ["semanticscholar", "openalex"],
    "biomedical": ["pubmed", "clinicaltrials", "openfda"],
    "chemistry": ["pubchem"],
    "datasets": ["huggingface"],
    "general_reference": ["wikipedia", "brave"],
}

# The MCP contract version for the operational diagnostics surface. This is
# distinct from the service (package) version so agents can negotiate schema
# changes independently of releases (design §7, decision 15).
MCP_CONTRACT_VERSION = "1.0"

# Closed set of status classes used to aggregate engine health. The final
# ``unknown`` bucket captures engines never observed by a search outcome
# (VAL-DIAG-006). Aligned with ``slopsearx.adapter.OBSERVED_STATUS_VOCAB`` so
# the MCP status surface and HTTP /health share one vocabulary (issue 190).
ENGINE_STATUS_CLASSES: tuple[str, ...] = OBSERVED_STATUS_VOCAB

# Engine-health note: /health never actively probes external APIs; health is
# observed passively through search outcomes (unchanged product behavior).
HEALTH_PASSIVE_NOTE = "/health does not actively probe external APIs; use search outcomes for passive engine health"

JOBS_ADAPTERS = ("greenhouse", "ashby", "lever")

JOBS_LIMITATION_NOTE = (
    "current ATS adapters return title, URL, location, salary/department where available; "
    "they provide no full job descriptions and no cross-ATS global search"
)

SECURITY_LIMITATION_NOTE = (
    "results are search findings, not a complete security assessment; "
    "absence from the selected engines does not mean absence of a vulnerability or exposure"
)

SCIENCE_LIMITATION_NOTE = (
    "source provenance and engine coverage are reported, but peer-review status, "
    "study quality, and citation completeness are not inferred from search results"
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _error(code: str, message: str, *, field: str | None = None, **extra: Any) -> dict[str, Any]:
    """Build a structured tool error envelope."""
    error: dict[str, Any] = {"code": code, "message": message}
    if field:
        error["field"] = field
    error.update(extra)
    return {"error": error}


def _client_identifier(state: McpState) -> str:
    del state
    return f"mcp:{current_tenant()}"


def _validate_query(query: str, state: McpState) -> dict[str, Any] | None:
    if not query or not query.strip():
        return _error("invalid_input", "query is required", field="query")
    if len(query) > state.policy.max_query_length:
        return _error(
            "invalid_input",
            f"query exceeds the maximum length of {state.policy.max_query_length} characters",
            field="query",
            max_length=state.policy.max_query_length,
        )
    return None


def _validate_engines(state: McpState, engines: list[str]) -> dict[str, Any] | None:
    """Validate an explicit engine list; returns an error dict or None."""
    known = state.catalog.known_names()
    unknown = [name for name in engines if name not in known]
    inactive = [
        name
        for name in engines
        if name in known and not bool(state.catalog.get(name) and state.catalog.get(name).enabled)  # type: ignore[union-attr]
    ]
    if unknown or inactive:
        problems = [f"{name} (unknown)" for name in unknown] + [f"{name} (inactive)" for name in inactive]
        valid = sorted(name for name in known if bool(state.catalog.get(name) and state.catalog.get(name).enabled))  # type: ignore[union-attr]
        return _error(
            "invalid_scope",
            "unknown or inactive engines: " + ", ".join(problems),
            field="engines",
            valid_alternatives=valid,
        )
    return None


def _enforce_policy(
    state: McpState,
    engines: list[str],
    *,
    field: str = "engines",
) -> dict[str, Any] | None:
    """One shared, fail-closed policy gate: engine validation + sensitive block.

    Every search-capable path (generic, targeted, jobs, security,
    science) and the scope-preview tool reach this before any engine
    dispatch. Returns an error envelope or ``None`` to proceed.

    A mixed sensitive + non-sensitive list fails closed atomically: the
    whole request is rejected, naming the sensitive engines in the
    structured ``error.engines`` field.
    """
    error = _validate_engines(state, engines)
    if error:
        return error
    sensitive = [name for name in engines if name in state.policy.sensitive_engines]
    if sensitive and not state.policy.targeted_sensitive_allowed:
        return _error(
            "tool_disabled",
            "sensitive engines are unreachable without the sensitive-engine grant "
            f"({SENSITIVE_GRANT}=1): {', '.join(sorted(sensitive))}",
            field=field,
            engines=sensitive,
            grant=SENSITIVE_GRANT,
        )
    return None


def _safesearch_value(safesearch: str) -> int:
    return {"off": 0, "moderate": 1, "strict": 2}[safesearch]


def _filter_warnings(state: McpState, selected_engines: list[str], language: str, time_range: str | None) -> list[str]:
    """Honest warnings for filter parameters no adapter enforces.

    Gated on the resolved enforcement status so the prose never contradicts
    the machine-readable report: a filter that any selected adapter enforces
    (``enforced``/``partially_enforced``) must not be described as "not
    consumed by any adapter". The report is derived via
    :func:`_core_filter_enforcement` against the same scope the search will
    dispatch.
    """
    report = _core_filter_enforcement(
        state, selected_engines, language=language, time_range=time_range, safesearch="off"
    )
    warnings: list[str] = []
    if report.get("language", {}).get("status") == "unsupported":
        warnings.append(f"language '{language}' is not consumed by any adapter")
    if report.get("time_range", {}).get("status") == "unsupported":
        warnings.append(f"time_range '{time_range}' is not consumed by any adapter")
    return warnings


def _safesearch_warning(state: McpState, selected_engines: list[str], safesearch: str) -> list[str]:
    """Moderate SafeSearch is best-effort; the enforcement report is authoritative.

    Gated on the resolved enforcement status like :func:`_filter_warnings`:
    when every selected adapter enforces moderate SafeSearch (the report says
    ``enforced``), the prose must not contradict the report, so no warning is
    emitted. Resolved via :func:`_core_filter_enforcement` against the same
    scope the search will dispatch.
    """
    if safesearch != "moderate":
        return []
    report = _core_filter_enforcement(state, selected_engines, language="en", time_range=None, safesearch=safesearch)
    if report.get("safesearch", {}).get("status") == "enforced":
        return []
    return ["moderate safesearch is requested but may not be enforced by every adapter"]


def _preview_selected_engines(
    state: McpState,
    query: str,
    categories: list[str] | None,
    engines: list[str] | None,
    media_type: str | None = None,
) -> list[str]:
    """Preview the engine scope that would execute for a request (no dispatch).

    Mirrors the service's scope resolution exactly (same active engines,
    router, tier-1 set, sensitive set, capability catalog, routing budget,
    and media-type constraint) so mandatory-constraint checks can fail closed
    *before* any engine is dispatched. The real ``query`` is passed through
    so auto-intent topic routing previews the same scope the service will
    dispatch — an empty-query preview would fall back to the tier-1 set and
    mask non-conforming topic scopes. The catalog comes from the shared
    context (wired by the server lifespan) so the preview and the executed
    scope agree on cost/coverage-aware automatic routing (issue 192), and the
    media-type constraint keeps the media intents' scope consistent (issue
    188).
    """
    resolver = ScopeResolver(
        active_engines=state.ctx.active_engines,
        router=state.ctx.router,
        tier1_engines=state.ctx.tier1_engines,
        sensitive_engines=state.ctx.sensitive_engines,
        catalog=state.ctx.catalog,
        budget=state.ctx.routing_budget,
    )
    return resolver.explain(
        SearchRequest(query=query, categories=categories, engines=engines, media_type=media_type)
    ).selected_engines


def _strict_safesearch_satisfiable(state: McpState, selected_engines: list[str]) -> bool:
    """Whether every selected engine enforces strict SafeSearch upstream.

    Strict SafeSearch is only satisfiable when the entire dispatched scope
    declares upstream safesearch enforcement; a single non-enforcing engine
    makes the request fail closed.
    """
    if not selected_engines:
        return False
    return all(
        engine_filter_layer(state.ctx.active_engines.get(name), "safesearch") == "upstream" for name in selected_engines
    )


def _safesearch_rejection(state: McpState, selected_engines: list[str]) -> dict[str, Any]:
    """Fail-closed strict SafeSearch rejection with the machine-readable report.

    The ``enforcement`` object carries the ``rejected`` status (the closed
    vocabulary's fail-closed member), while the error envelope names the
    selected engines that could not satisfy the constraint.

    Strict SafeSearch is all-or-nothing — every selected engine must enforce
    it upstream. The reason/message name the non-enforcing subset so a mixed
    scope (some engines enforce, some do not) is never described as "no
    adapter enforces".
    """
    non_enforcing = sorted(
        name
        for name in selected_engines
        if engine_filter_layer(state.ctx.active_engines.get(name), "safesearch") != "upstream"
    )
    reason = (
        "strict SafeSearch is not enforced by "
        + (", ".join(non_enforcing) or "any selected engine")
        + "; strict results cannot be guaranteed — use 'off' or 'moderate'"
    )
    entry = enforcement_entry("strict", "rejected", reason, [])
    return {
        "error": {
            "code": "safesearch_unenforced",
            "message": reason,
            "field": "safesearch",
            "selected_engines": list(selected_engines),
        },
        "enforcement": {"safesearch": entry},
    }


# ---------------------------------------------------------------------------
# Structured filter-enforcement report
# ---------------------------------------------------------------------------


def _core_filter_enforcement(
    state: McpState,
    selected_engines: list[str],
    *,
    language: str,
    time_range: str | None,
    safesearch: str,
) -> dict[str, Any]:
    """Structured enforcement report for language/time_range/safesearch.

    Resolved against the dispatched engine scope via the shared
    :func:`slopsearx.filters.resolve_filter_enforcement`, so every search
    path reports the same vocabulary, reasons, and layer-qualified
    ``enforced_by`` tokens.

    Strict SafeSearch is handled before dispatch (scope-aware fail-closed
    rejection) and therefore never reaches here; moderate SafeSearch is
    resolved like any other best-effort filter.
    """
    report: dict[str, Any] = {}
    if language and language != "en":
        report["language"] = resolve_filter_enforcement(
            selected_engines, "language", language, state.ctx.active_engines
        )
    if time_range:
        report["time_range"] = resolve_filter_enforcement(
            selected_engines, "time_range", time_range, state.ctx.active_engines
        )
    if safesearch in ("moderate", "strict"):
        report["safesearch"] = resolve_filter_enforcement(
            selected_engines, "safesearch", safesearch, state.ctx.active_engines
        )
    return report


def _routing_scope_block(scope: ScopeDecision) -> dict[str, Any]:
    """The machine-readable routing explanation attached to a scope decision.

    Exposed on both the search envelope's ``scope`` block and the
    ``slopsearx_explain_search_scope`` tool so preview and executed scope
    agree (issue 192). Every exclusion carries a ``stage`` (policy | auth |
    health | budget), and ``routing`` reports whether the deterministic
    fallback ran, whether configured budget bounds shaped the mix, and any
    coverage-for-cost/availability trade-offs.

    On explicit-engine scopes the cost/coverage pass is bypassed by design
    (issue 192: explicit source scope is preserved verbatim), so the block
    carries only ``applied: false``. Without that discriminator a
    ``fallback: false, budget_applied: false`` block would misread as "the
    budget was evaluated and did not bite".
    """
    if scope.routing_rule == "explicit engine":
        return {"applied": False}
    return {
        "applied": True,
        "fallback": bool(scope.routing_fallback),
        "budget_applied": bool(scope.routing_budget_applied),
        "tradeoffs": [{"kind": t.kind, "detail": t.detail} for t in scope.routing_tradeoffs],
    }


def _excluded_engines_list(scope: ScopeDecision) -> list[dict[str, str]]:
    """Machine-readable excluded-engines list with reasons and stages."""
    return [{"engine": e.engine, "reason": e.reason, "stage": e.stage} for e in scope.excluded_engines]


def _envelope(
    state: McpState,
    response: Any,
    *,
    requested_intent: str,
    warnings: list[str],
    cursor: str | None,
    include_suggestions: bool,
    total: int,
    enforcement: dict[str, Any] | None = None,
    include_payload: bool = False,
) -> dict[str, Any]:
    """Build the standard search envelope from a SearchResponse.

    Surfaces every piece of evidence the service produced — answers,
    corrections, infoboxes, suggestions, per-engine outcomes, empty engines,
    excluded engines, aggregate count, and pagination signal — so no
    available evidence is silently discarded (envelope recovery).
    """
    excluded_engines = _excluded_engines_list(response.scope)
    scope = {
        "requested_intent": requested_intent,
        "resolved_categories": response.scope.resolved_categories,
        "selected_engines": response.scope.selected_engines,
        "routing_reason": response.scope.routing_rule,
        "excluded_engines": excluded_engines,
        "routing": _routing_scope_block(response.scope),
    }
    if response.all_unresponsive:
        return _error(
            "all_engines_failed",
            "interactive deadline reached before any engine responded"
            if response.deadline_exceeded
            else "every selected engine failed to respond",
            deadline_exceeded=response.deadline_exceeded,
            query_id=response.query_id,
            scope=scope,
            engine_outcomes=[
                {"engine": o.engine, "status": o.status, "result_count": o.result_count, "message": o.message}
                for o in response.engine_outcomes
            ],
            retry_guidance=(
                "increase or omit interactive_timeout_ms and retry"
                if response.deadline_exceeded
                else "check the engine outcomes, adjust scope, and retry"
            ),
        )
    return {
        "query": response.query,
        "results": [
            _result_to_dict(
                result,
                result_id=(state.snapshots.result_id(cursor, index) if cursor else None),
                include_payload=include_payload,
            )
            for index, result in enumerate(response.results)
        ],
        "scope": scope,
        "answers": response.answers,
        "corrections": response.corrections,
        "infoboxes": response.infoboxes,
        "empty_engines": [{"engine": entry[0], "reason": entry[1]} for entry in response.empty_engines],
        "enforcement": enforcement or {},
        "engine_outcomes": [
            {
                "engine": o.engine,
                "status": o.status,
                "result_count": o.result_count,
                "latency_ms": o.latency_ms,
                "message": o.message,
            }
            for o in response.engine_outcomes
        ],
        "meta": {
            "query_id": response.query_id,
            "cached": response.cached,
            "cached_error": response.cached_error,
            "response_time_ms": response.response_time_ms,
            "partial": response.partial,
            "deadline_exceeded": response.deadline_exceeded,
            "ranking": response.ranking_explanation,
            "cursor": cursor,
            "suggestions": response.suggestions if include_suggestions else [],
            "total": total,
            "has_more": total > len(response.results),
        },
        "warnings": warnings + response.scope.warnings,
    }


async def _run_search(
    state: McpState,
    request: SearchRequest,
    requested_intent: str,
    warnings: list[str],
    include_suggestions: bool,
    max_results: int | None = None,
    enforcement: dict[str, Any] | None = None,
    core_filters: dict[str, Any] | None = None,
    include_payload: bool = False,
) -> dict[str, Any]:
    """Execute one search through the service and build the envelope.

    The full ranked set is captured as an immutable snapshot (for
    pagination); ``max_results`` is a presentation bound applied to the
    returned page only.

    When ``enforcement`` is not supplied and ``core_filters`` is, the
    filter-enforcement report is resolved against the **dispatched/executed
    scope** (``response.scope.selected_engines``), never against all active
    engines. This keeps the report consistent with the engine set that
    actually ran once per-engine ``supported_filters`` are declared.
    """
    try:
        response = await state.service.search(request)
    except QueryValidationError as exc:
        rejected = _error("invalid_input", str(exc), field=exc.field)
        if exc.field != "query":
            rejected["enforcement"] = {exc.field: enforcement_entry(getattr(request, exc.field), "rejected", str(exc))}
        return rejected
    except RateLimitExceededError:
        return _error("rate_limited", "too many requests; please retry later")

    if enforcement is None and core_filters is not None:
        enforcement = _core_filter_enforcement(state, response.scope.selected_engines, **core_filters)

    if request.date_from is not None or request.date_to is not None:
        enforcement = dict(enforcement or {})
        for name, value in (("date_from", request.date_from), ("date_to", request.date_to)):
            if value is not None:
                entry = resolve_filter_enforcement(
                    response.scope.selected_engines, name, value, state.ctx.active_engines
                )
                enforcement[name] = entry
                if entry["status"] != "enforced":
                    warnings = warnings + [entry["reason"]]

    # Capture the full ranked set as an immutable snapshot for pagination,
    # then present the bounded page. ``total`` is the aggregate captured
    # count (meta.total), independent of the max_results page bound.
    total = len(response.results)
    cursor = await state.snapshots.for_tenant(current_tenant()).create(
        response.query,
        response.query_id,
        response.results,
        response.scope,
        ranking_explanation=response.ranking_explanation,
    )
    if cursor is None:
        warnings = warnings + ["snapshot store unavailable — pagination cursor not created"]
    if max_results is not None and max_results > 0:
        response.results = response.results[:max_results]
    return _envelope(
        state,
        response,
        requested_intent=requested_intent,
        warnings=warnings,
        cursor=cursor,
        include_suggestions=include_suggestions,
        total=total,
        enforcement=enforcement,
        include_payload=include_payload,
    )


def _deadline_iso(deadline: float) -> str:
    return _dt.datetime.fromtimestamp(deadline, tz=_dt.timezone.utc).isoformat()


def _expires_iso(expires_at: float | None) -> str | None:
    """Render a snapshot expiry epoch as an ISO 8601 UTC timestamp."""
    if expires_at is None:
        return None
    return _dt.datetime.fromtimestamp(expires_at, tz=_dt.timezone.utc).isoformat()


# ---------------------------------------------------------------------------
# Core search
# ---------------------------------------------------------------------------


async def slopsearx_search(
    query: str,
    intent: str = "auto",
    categories: list[str] | None = None,
    engines: list[str] | None = None,
    language: str = "en",
    time_range: str | None = None,
    safesearch: str = "off",
    media_type: str | None = None,
    max_results: int | None = None,
    include: list[str] | None = None,
    freshness: str = "no_preference",
    interactive_timeout_ms: int | None = None,
) -> dict[str, Any]:
    """Search across SlopSearX engines with intent-based routing.

    - intent: one of auto, web, news, science, reference, code, social,
      historical, jobs, security, medical, finance, packages, media, images,
      videos, legal, geography. auto uses query-topic routing with tier-1
      fallback.
    - categories: explicit category OR-filter (overridden by engines).
    - engines: explicit engine list (overrides everything; must be known).
    - media_type: image | video. Constrains the dispatched scope to engines
      that advertise the requested media type; when no selected engine
      advertises it, the coverage gap is reported explicitly.
    - safesearch: off | moderate | strict. strict fails closed because no
      adapter enforces it.
    - interactive_timeout_ms: optional 1–30000 ms engine/suggestion wait budget;
      may return incomplete coverage. Omit for full configured engine deadlines.
    - freshness: prefer_cache | prefer_fresh | no_preference.
    - include: subset of results, suggestions, engine_status, diagnostics,
      payload. When ``payload`` is included, compact result cards inline the
      domain payload only when it is within ``PAYLOAD_MAX_PERSIST_BYTES``
      (the snapshot persistence bound); otherwise cards inline a payload only
      when it is small. The full payload is available via
      slopsearx_read_result when it is within the persistence bound.
    Returns results, scope, engine outcomes, and a pagination cursor.
    """
    state = get_state()
    if interactive_timeout_ms is not None and (
        type(interactive_timeout_ms) is not int or not 1 <= interactive_timeout_ms <= 30000
    ):
        return _error(
            "invalid_input",
            "interactive_timeout_ms must be an integer between 1 and 30000",
            field="interactive_timeout_ms",
        )

    if intent != "auto" and intent not in VALID_INTENTS:
        return _error(
            "invalid_input",
            f"unknown intent '{intent}'",
            field="intent",
            valid_alternatives=list(VALID_INTENTS),
        )
    if safesearch not in VALID_SAFESEARCH:
        return _error("invalid_input", "safesearch must be off, moderate, or strict", field="safesearch")
    if media_type is not None and media_type not in VALID_MEDIA_TYPES:
        return _error(
            "invalid_input",
            f"unknown media_type '{media_type}'",
            field="media_type",
            valid_alternatives=list(VALID_MEDIA_TYPES),
        )
    if freshness not in VALID_FRESHNESS:
        return _error(
            "invalid_input",
            f"unknown freshness '{freshness}'",
            field="freshness",
            valid_alternatives=list(VALID_FRESHNESS),
        )

    error = _validate_query(query, state)
    if error:
        return error

    # Resolve intent → scope (explicit inputs win over the profile).
    resolved_categories, resolved_engines, resolved_media_type, requested_intent, warnings = _resolve_scope(
        state, intent, categories, engines, media_type
    )
    if isinstance(resolved_categories, dict):  # error envelope
        return resolved_categories

    # Shared policy gate: sensitive engines (and specialist intents) are
    # fail-closed on the generic explicit-engine/profile path too.
    if resolved_engines is not None:
        policy_error = _enforce_policy(state, resolved_engines, field="engines")
        if policy_error:
            return policy_error

    # Preview the scope that will execute so strict SafeSearch and the filter
    # warnings are resolved against the same engine set the report will name.
    selected = _preview_selected_engines(state, query, resolved_categories, resolved_engines, resolved_media_type)

    # A media-intent search that resolves to no engines reports the coverage
    # gap explicitly instead of surfacing a misleading "all engines failed".
    if resolved_media_type is not None and not selected:
        return _error(
            "media_coverage_gap",
            f"no selected engine advertises media type '{resolved_media_type}'",
            field="media_type",
            media_type=resolved_media_type,
        )

    # Mandatory strict SafeSearch fails closed before dispatch when the
    # selected scope cannot satisfy it.
    if safesearch == "strict" and not _strict_safesearch_satisfiable(state, selected):
        return _safesearch_rejection(state, selected)

    safesearch_warning = _safesearch_warning(state, selected, safesearch)

    include_set = set(include) if include is not None else {"results", "engine_status"}
    max_results = _bounded_max_results(state, max_results)

    request = SearchRequest(
        query=query,
        interactive_timeout_ms=interactive_timeout_ms,
        categories=resolved_categories,
        engines=resolved_engines,
        language=language,
        time_range=time_range,
        safesearch=_safesearch_value(safesearch),
        media_type=resolved_media_type,
        include=include_set,
        freshness=freshness,
        client_identifier=_client_identifier(state),
    )
    warnings = warnings + _filter_warnings(state, selected, language, time_range) + safesearch_warning

    # Structured filter-enforcement report is resolved inside ``_run_search``
    # against the dispatched/executed scope (response.scope.selected_engines),
    # not against all active engines, so it stays consistent with the engine
    # set that actually ran.
    return await _run_search(
        state,
        request,
        requested_intent,
        warnings,
        include_suggestions="suggestions" in include_set,
        max_results=max_results,
        core_filters={"language": language, "time_range": time_range, "safesearch": safesearch},
        include_payload="payload" in include_set,
    )


def _resolve_scope(
    state: McpState,
    intent: str,
    categories: list[str] | None,
    engines: list[str] | None,
    media_type: str | None = None,
) -> tuple[list[str] | dict[str, Any] | None, list[str] | None, str | None, str, list[str]]:
    """Resolve intent/scope precedence.

    Returns (categories, engines, media_type, requested_intent, warnings); a
    dict as the first element signals an error envelope.
    """
    # Media intents (images/videos) advertise their media type on the intent
    # profile. An explicit media_type wins; otherwise the intent's media type
    # carries through the explicit-engine and explicit-category branches so a
    # media intent never silently degrades into a text search when combined
    # with an explicit scope (issue-188 review).
    profile = INTENT_PROFILES.get(intent)
    resolved_media_type = (
        media_type
        if media_type is not None
        else (profile.media_types[0] if profile is not None and profile.media_types else None)
    )

    if engines:
        error = _validate_engines(state, engines)
        if error:
            return error, None, resolved_media_type, intent, []
        return None, engines, resolved_media_type, intent, []

    if categories:
        return categories, None, resolved_media_type, intent, []

    if intent == "auto":
        return None, None, resolved_media_type, "auto", []

    if profile is None:
        return (
            _error("invalid_input", f"unknown intent '{intent}'", field="intent"),
            None,
            resolved_media_type,
            intent,
            [],
        )
    required_grant = INTENT_GRANTS.get(intent)
    if required_grant is not None and not state.policy.tool_enabled(required_grant):
        return (
            _error(
                "tool_disabled",
                f"intent '{intent}' requires the {required_grant} grant ({GRANT_ENV[required_grant]}=1)",
                field="intent",
                grant=GRANT_ENV[required_grant],
            ),
            None,
            resolved_media_type,
            intent,
            [],
        )
    if profile.engines:
        engines_list = [name for name in profile.engines if name in state.catalog.known_names()]
        return None, engines_list, resolved_media_type, intent, [f"intent profile '{intent}' selected explicit engines"]
    if profile.media_types:
        return (
            None,
            None,
            resolved_media_type,
            intent,
            [f"intent profile '{intent}' selected media type '{resolved_media_type}'"],
        )
    return profile.categories, None, resolved_media_type, intent, [f"intent profile '{intent}' selected categories"]


def _bounded_max_results(state: McpState, requested: int | None) -> int:
    if requested is None or requested < 1:
        return state.policy.max_results
    return min(requested, state.policy.max_results)


# ---------------------------------------------------------------------------
# Targeted search
# ---------------------------------------------------------------------------


async def slopsearx_search_targeted(
    query: str,
    engines: list[str],
    language: str = "en",
    time_range: str | None = None,
    safesearch: str = "off",
    max_results: int | None = None,
) -> dict[str, Any]:
    """Search only the named engines (deliberate, auditable scope).

    Requires at least one known, active engine; unknown or inactive
    engines produce an error listing valid alternatives. Sensitive
    engines require the operator grant MCP_TARGETED_SENSITIVE_ALLOWED=1.
    """
    state = get_state()
    if not engines:
        return _error("invalid_input", "engines is required and must list at least one engine", field="engines")

    error = _validate_query(query, state)
    if error:
        return error
    error = _enforce_policy(state, engines, field="engines")
    if error:
        return error
    if safesearch not in VALID_SAFESEARCH:
        return _error("invalid_input", "safesearch must be off, moderate, or strict", field="safesearch")

    # Mandatory strict SafeSearch fails closed before dispatch when the
    # selected scope cannot satisfy it.
    if safesearch == "strict" and not _strict_safesearch_satisfiable(state, engines):
        return _safesearch_rejection(state, list(engines))

    safesearch_warning = _safesearch_warning(state, list(engines), safesearch)

    request = SearchRequest(
        query=query,
        engines=engines,
        language=language,
        time_range=time_range,
        safesearch=_safesearch_value(safesearch),
        include={"results", "engine_status"},
        client_identifier=_client_identifier(state),
    )
    warnings = _filter_warnings(state, list(engines), language, time_range) + safesearch_warning
    return await _run_search(
        state,
        request,
        "explicit engine",
        warnings,
        include_suggestions=False,
        max_results=_bounded_max_results(state, max_results),
        core_filters={"language": language, "time_range": time_range, "safesearch": safesearch},
    )


# ---------------------------------------------------------------------------
# Jobs
# ---------------------------------------------------------------------------


async def slopsearx_search_jobs(
    company: str,
    keywords: list[str] | None = None,
    location: str | None = None,
    employment_type: str | None = None,
    sources: list[str] | None = None,
    max_results: int | None = None,
) -> dict[str, Any]:
    """Search ATS job boards for a named company.

    Builds the internal "… at <company>" query the job adapters
    understand and targets the ATS boards explicitly. Requires the
    jobs grant (MCP_GRANT_JOBS=1).
    """
    state = get_state()
    if not state.policy.tool_enabled("jobs"):
        return _error("tool_disabled", "slopsearx_search_jobs requires the jobs grant (MCP_GRANT_JOBS=1)")

    if not company or not company.strip():
        return _error("invalid_input", "company is required", field="company")

    sources = list(sources) if sources else list(JOBS_ADAPTERS)
    error = _enforce_policy(state, sources, field="sources")
    if error:
        return error

    title = " ".join(keywords or []).strip()
    query = f"{title} at {company.strip()}" if title else f"jobs at {company.strip()}"
    warnings = [JOBS_LIMITATION_NOTE]
    if location:
        warnings.append(f"location '{location}' is not consumed by current adapters")
    if employment_type:
        warnings.append(f"employment_type '{employment_type}' is not consumed by current adapters")

    # Structured filter-enforcement report for jobs-specific filter params.
    enforcement: dict[str, Any] = {}
    if location:
        enforcement["location"] = enforcement_entry(
            location, "unsupported", "location is not consumed by current adapters"
        )
    if employment_type:
        enforcement["employment_type"] = enforcement_entry(
            employment_type, "unsupported", "employment_type is not consumed by current adapters"
        )

    request = SearchRequest(
        query=query,
        engines=sources,
        include={"results", "engine_status"},
        client_identifier=_client_identifier(state),
    )
    return await _run_search(
        state,
        request,
        "jobs",
        warnings,
        include_suggestions=False,
        max_results=_bounded_max_results(state, max_results),
        enforcement=enforcement,
    )


# ---------------------------------------------------------------------------
# Security
# ---------------------------------------------------------------------------


async def slopsearx_search_security(
    query: str,
    evidence_types: list[str] | None = None,
    engines: list[str] | None = None,
    max_results: int | None = None,
) -> dict[str, Any]:
    """Security and threat-intelligence search.

    evidence_types resolve to engine profiles: vulnerability, exposure,
    reputation, malware, threat_intel, exploit. Requires the security
    grant (MCP_GRANT_SECURITY=1).
    """
    state = get_state()
    if not state.policy.tool_enabled("security"):
        return _error("tool_disabled", "slopsearx_search_security requires the security grant (MCP_GRANT_SECURITY=1)")

    error = _validate_query(query, state)
    if error:
        return error

    evidence_types = evidence_types or ["vulnerability"]
    selected: list[str] = []
    for evidence_type in evidence_types:
        if evidence_type not in EVIDENCE_TYPE_ENGINES:
            return _error(
                "invalid_input",
                f"unknown evidence_type '{evidence_type}'",
                field="evidence_types",
                valid_alternatives=sorted(EVIDENCE_TYPE_ENGINES),
            )
        for name in EVIDENCE_TYPE_ENGINES[evidence_type]:
            if name not in selected:
                selected.append(name)

    if engines:
        selected = list(engines)
    else:
        selected = [name for name in selected if name in state.catalog.known_names()]

    # Shared policy gate: a sensitive engine reached via explicit engines
    # OR an evidence-type profile is blocked unless the sensitive grant is set.
    policy_field = "engines" if engines else "evidence_types"
    policy_error = _enforce_policy(state, selected, field=policy_field)
    if policy_error:
        return policy_error

    request = SearchRequest(
        query=query,
        engines=selected,
        include={"results", "engine_status"},
        client_identifier=_client_identifier(state),
    )
    return await _run_search(
        state,
        request,
        "security",
        [SECURITY_LIMITATION_NOTE, f"resolved evidence_types: {', '.join(evidence_types)}"],
        include_suggestions=False,
        max_results=_bounded_max_results(state, max_results),
        enforcement={},
    )


# ---------------------------------------------------------------------------
# Science
# ---------------------------------------------------------------------------


async def slopsearx_search_science(
    query: str,
    source_types: list[str] | None = None,
    date_from: str | None = None,
    date_to: str | None = None,
    engines: list[str] | None = None,
    max_results: int | None = None,
) -> dict[str, Any]:
    """Research-oriented search.

    source_types resolve to engine profiles: papers, scholarly_index,
    biomedical, chemistry, datasets, general_reference. Requires the
    science grant (MCP_GRANT_SCIENCE=1).
    """
    state = get_state()
    if not state.policy.tool_enabled("science"):
        return _error("tool_disabled", "slopsearx_search_science requires the science grant (MCP_GRANT_SCIENCE=1)")

    error = _validate_query(query, state)
    if error:
        return error

    source_types = source_types or ["papers"]
    selected: list[str] = []
    for source_type in source_types:
        if source_type not in SOURCE_TYPE_ENGINES:
            return _error(
                "invalid_input",
                f"unknown source_type '{source_type}'",
                field="source_types",
                valid_alternatives=sorted(SOURCE_TYPE_ENGINES),
            )
        for name in SOURCE_TYPE_ENGINES[source_type]:
            if name not in selected:
                selected.append(name)

    if engines:
        selected = list(engines)
    else:
        selected = [name for name in selected if name in state.catalog.known_names()]

    # Shared policy gate: a sensitive engine reached via explicit engines
    # OR a source-type profile is blocked unless the sensitive grant is set.
    policy_field = "engines" if engines else "source_types"
    policy_error = _enforce_policy(state, selected, field=policy_field)
    if policy_error:
        return policy_error

    try:
        publication_date_bounds(date_from, date_to)
    except DateFilterError as exc:
        rejected = {
            name: enforcement_entry(value, "rejected", str(exc))
            for name, value in (("date_from", date_from), ("date_to", date_to))
            if value is not None
        }
        error_response = _error("invalid_input", str(exc), field=exc.field)
        error_response["enforcement"] = rejected
        return error_response

    warnings = [SCIENCE_LIMITATION_NOTE, f"resolved source_types: {', '.join(source_types)}"]
    enforcement: dict[str, Any] = {}

    request = SearchRequest(
        query=query,
        date_from=date_from,
        date_to=date_to,
        engines=selected,
        include={"results", "engine_status"},
        client_identifier=_client_identifier(state),
    )
    return await _run_search(
        state,
        request,
        "science",
        warnings,
        include_suggestions=False,
        max_results=_bounded_max_results(state, max_results),
        enforcement=enforcement,
    )


# ---------------------------------------------------------------------------
# Capability discovery and scope explanation
# ---------------------------------------------------------------------------


async def slopsearx_list_capabilities(
    family: str | None = None,
    category: str | None = None,
    include_disabled: bool = False,
    include_auth_requirements: bool = True,
) -> dict[str, Any]:
    """List the live engine catalog with categories, auth classes, caveats.

    Generated from the runtime registry and effective configuration —
    never from prose documentation. Auth requirements describe whether
    an engine needs credentials; actual key values are never exposed.
    """
    state = get_state()
    caps = state.catalog.all() if include_disabled else state.catalog.enabled()
    if category:
        caps = [cap for cap in caps if category in cap.categories]
    if family:
        caps = [cap for cap in caps if family in cap.categories]

    engines_out: list[dict[str, Any]] = []
    for cap in caps:
        entry: dict[str, Any] = {
            "name": cap.name,
            "display_name": cap.display_name,
            "type": cap.engine_type,
            "categories": cap.categories,
            "subcategories": cap.subcategories,
            "enabled": cap.enabled,
            "sensitive": cap.sensitive,
            "supported_filters": cap.supported_filters,
            "enforced_filters": cap.enforced_filters,
            "supported_result_types": cap.supported_result_types,
            "supported_media_types": cap.supported_media_types,
            "failure_classes": cap.failure_classes,
            "cost_class": cap.cost_class or None,
            "last_known_status": cap.last_known_status,
            "last_known_status_at": cap.last_known_status_at,
            "last_known_status_stale": cap.last_known_status_stale,
            "circuit_open": cap.circuit_open,
            "circuit_consecutive_errors": cap.circuit_consecutive_errors,
            "scope_hints": cap.scope_hints,
            "caveats": cap.caveats,
        }
        if include_auth_requirements:
            entry["auth"] = {"class": cap.auth_class, "configured": cap.auth_configured}
        engines_out.append(entry)

    return {
        "engines": engines_out,
        "count": len(engines_out),
        "filter": {"family": family, "category": category, "include_disabled": include_disabled},
    }


async def slopsearx_explain_search_scope(
    query: str,
    intent: str = "auto",
    categories: list[str] | None = None,
    engines: list[str] | None = None,
    media_type: str | None = None,
) -> dict[str, Any]:
    """Dry-run routing preview: which engines would run and why.

    Executes no searches and spends no rate limits. Useful to correct
    scope before dispatching. ``media_type`` (image | video) constrains the
    preview to engines that advertise the requested media type.
    """
    state = get_state()
    error = _validate_query(query, state)
    if error:
        return error
    if media_type is not None and media_type not in VALID_MEDIA_TYPES:
        return _error(
            "invalid_input",
            f"unknown media_type '{media_type}'",
            field="media_type",
            valid_alternatives=list(VALID_MEDIA_TYPES),
        )

    resolved_categories, resolved_engines, resolved_media_type, requested_intent, warnings = _resolve_scope(
        state, intent, categories, engines, media_type
    )
    if isinstance(resolved_categories, dict):
        return resolved_categories

    # Scope preview must match execution: sensitive engines and specialist
    # intents are fail-closed here exactly as they are on the search path.
    if resolved_engines is not None:
        policy_error = _enforce_policy(state, resolved_engines, field="engines")
        if policy_error:
            return policy_error

    resolver = ScopeResolver(
        active_engines=state.ctx.active_engines,
        router=state.ctx.router,
        tier1_engines=state.ctx.tier1_engines,
        sensitive_engines=state.policy.sensitive_engines,
        catalog=state.ctx.catalog,
        budget=state.ctx.routing_budget,
    )
    decision = resolver.explain(
        SearchRequest(
            query=query,
            categories=resolved_categories,
            engines=resolved_engines,
            media_type=resolved_media_type,
        )
    )
    return {
        "selected_engines": decision.selected_engines,
        "excluded_engines": _excluded_engines_list(decision),
        "routing": _routing_scope_block(decision),
        "routing_reason": decision.routing_rule,
        # Backward-compatible alias so preview and executed scope agree on
        # the routing_reason field (schema pin) while older consumers that
        # read routing_rule keep working.
        "routing_rule": decision.routing_rule,
        "matched_topic": decision.matched_topic,
        "requested_intent": requested_intent,
        "media_type": resolved_media_type,
        "warnings": warnings + decision.warnings,
    }


def _engine_health_by_class(state: McpState) -> dict[str, Any]:
    """Aggregate enabled-engine health into per-status-class integer counts.

    Health is observed passively: every engine's last-known status defaults to
    ``unknown`` until a search outcome records otherwise, so unobserved engines
    land in the ``unknown`` bucket (never a fabricated ``ok``).
    """
    counts: dict[str, Any] = {cls: 0 for cls in ENGINE_STATUS_CLASSES}
    for cap in state.catalog.enabled():
        status = cap.last_known_status if cap.last_known_status in counts else "unknown"
        counts[status] += 1
    counts["note"] = HEALTH_PASSIVE_NOTE
    return counts


def _enabled_grants(state: McpState) -> dict[str, Any]:
    """The enabled specialist grants by name (never any token/key value).

    Specialist grants are jobs/security/science/research. Disabled grants are
    present as ``False`` so agents can tell absent from disabled; the
    sensitive-engine grant is exposed as a boolean only.
    """
    enabled = sorted(name for name, flag in state.policy.enabled_tools.items() if flag)
    specialist = {name: bool(flag) for name, flag in sorted(state.policy.enabled_tools.items())}
    return {
        "enabled": enabled,
        "specialist": specialist,
        "targeted_sensitive_allowed": bool(state.policy.targeted_sensitive_allowed),
    }


def service_diagnostics(state: McpState, *, now: str | None = None) -> dict[str, Any]:
    """Build the curated, non-secret operational diagnostics schema.

    Shared by ``slopsearx_get_service_status`` and the ``slopsearx://health/
    summary`` resource so both report the same authoritative values
    (VAL-DIAG-010). Deliberately excludes credentials, raw audit, environment,
    and unrestricted metrics.
    """
    ctx = state.ctx
    window = ctx.client_rate_window
    valkey_connected: bool | None = False
    fail_closed = False
    if isinstance(window, ValkeySlidingWindow):
        valkey_connected = window._connected
        fail_closed = window._fail_closed

    cache_available = bool(ctx.cache is not None and ctx.cache.is_connected)
    snapshots_available = bool(state.snapshots.available)
    job_store_available = bool(state.job_store.available)
    engine_count = len(state.catalog.enabled())
    durable_research = bool(state.job_store.durable)

    # Per-engine observed-health detail, derived with the same builder used by
    # HTTP /health so the two surfaces agree on status vocabulary, freshness
    # timestamps, and the distinct circuit/auth signals (issue 190).
    engine_details: dict[str, Any] = {}
    for cap in state.catalog.enabled():
        adapter = ctx.active_engines.get(cap.name)
        engine_details[cap.name] = build_engine_health(adapter, cap)

    causes: list[str] = []
    if not valkey_connected:
        causes.append("Valkey unavailable")
    if not cache_available:
        causes.append("cache unavailable")
    if not snapshots_available:
        causes.append("snapshot store unavailable")
    if not job_store_available:
        causes.append("research job store unavailable")

    return {
        "version": state.version,
        "contract_version": MCP_CONTRACT_VERSION,
        "valkey": {"connected": valkey_connected, "fail_closed": fail_closed},
        "cache_connected": cache_available,
        "snapshots_available": snapshots_available,
        "job_store_available": job_store_available,
        "active_engines": engine_count,
        "router_enabled": bool(ctx.router is not None and ctx.router.enabled),
        "engine_health": _engine_health_by_class(state),
        "engines": engine_details,
        "grants": _enabled_grants(state),
        "research_execution": {
            "mode": "durable_leased" if durable_research else "degraded",
            "worker_id": state.runner.worker_id,
            "lease_ttl_seconds": state.runner.lease_ttl,
            "poll_interval_seconds": state.runner.poll_interval,
            "max_concurrent_jobs": state.runner.max_concurrent_jobs,
            "note": (
                "research jobs are claimed under an exclusive Valkey lease and "
                "reclaimed by another replica on lease expiry"
                if durable_research
                else "Valkey unavailable — research jobs are not executed (no shared job store)"
            ),
        },
        "policy_bounds": {
            "max_query_length": state.policy.max_query_length,
            "max_results": state.policy.max_results,
            "snapshot_ttl_seconds": state.policy.snapshot_ttl_seconds,
            "job_max_queries": state.policy.job_max_queries,
            "job_max_engines_per_query": state.policy.job_max_engines_per_query,
            "job_max_results": state.policy.job_max_results,
            "job_default_deadline_seconds": state.policy.job_default_deadline_seconds,
            "job_lease_ttl_seconds": state.policy.job_lease_ttl_seconds,
            "job_poll_interval_seconds": state.policy.job_poll_interval_seconds,
            "job_max_concurrent_jobs": state.policy.job_max_concurrent_jobs,
        },
        "degradation": {
            "operational": not causes,
            "summary": "fully operational" if not causes else "degraded",
            "causes": causes,
        },
        "freshness": now or _dt.datetime.now(_dt.timezone.utc).isoformat(),
    }


async def slopsearx_get_service_status() -> dict[str, Any]:
    """Operational status: liveness, versions, Valkey, engine inventory.

    Returns the curated, non-secret diagnostics schema shared with
    ``slopsearx://health/summary``. /health does not actively probe external
    APIs — engine health is observed passively through search outcomes.
    """
    state = get_state()
    return {"status": "ok", **service_diagnostics(state)}


# ---------------------------------------------------------------------------
# Snapshot reads
# ---------------------------------------------------------------------------


async def slopsearx_read_results(
    cursor: str,
    page: int = 1,
    max_results: int | None = None,
) -> dict[str, Any]:
    """Read a stable page from a captured search snapshot.

    Never re-runs the query — pages come from the captured evidence.
    cursor values are server-issued; arbitrary URLs are not accepted.
    """
    state = get_state()
    if not cursor or not cursor.strip():
        return _error("invalid_input", "cursor is required", field="cursor")
    if page < 1:
        return _error("invalid_input", "page must be >= 1", field="page")

    page_size = _bounded_max_results(state, max_results)
    lookup = await state.snapshots.for_tenant(current_tenant()).read(cursor)
    if lookup.unavailable:
        return _error("store_unavailable", "snapshot store is unavailable", field="cursor")
    if lookup.expired:
        return _error(
            "expired_handle",
            "snapshot has expired",
            handle=cursor,
            expires_at=_expires_iso(lookup.expires_at),
            field="cursor",
        )
    snapshot = lookup.snapshot
    if snapshot is None:
        return _error("invalid_cursor", "unknown cursor", field="cursor")

    start = (page - 1) * page_size
    end = start + page_size
    page_results = snapshot.results[start:end]
    return {
        "query": snapshot.query,
        "cursor": cursor,
        "page": page,
        "results": [
            _result_to_dict(result, result_id=state.snapshots.result_id(cursor, start + index))
            for index, result in enumerate(page_results)
        ],
        "meta": {
            "total": snapshot.total,
            "ranking": snapshot.ranking_explanation,
            "has_more": end < snapshot.total,
            "query_id": snapshot.query_id,
        },
    }


async def slopsearx_read_entities(
    cursor: str,
    page: int = 1,
    max_results: int | None = None,
) -> dict[str, Any]:
    """Read explicit CVE and npm/PyPI release groups from a captured snapshot.

    max_results counts entities, not members; groups contain original result IDs
    for slopsearx_read_result. Unknown identities stay separate. This read-only
    view neither searches nor establishes source independence or verification.
    """
    state = get_state()
    if not cursor or not cursor.strip():
        return _error("invalid_input", "cursor is required", field="cursor")
    if page < 1:
        return _error("invalid_input", "page must be >= 1", field="page")
    page_size = _bounded_max_results(state, max_results)
    lookup = await state.snapshots.for_tenant(current_tenant()).read(cursor)
    if lookup.unavailable:
        return _error("store_unavailable", "snapshot store is unavailable", field="cursor")
    if lookup.expired:
        return _error(
            "expired_handle",
            "snapshot has expired",
            handle=cursor,
            expires_at=_expires_iso(lookup.expires_at),
            field="cursor",
        )
    if lookup.snapshot is None:
        return _error("invalid_cursor", "unknown cursor", field="cursor")
    snapshot = lookup.snapshot
    groups = entity_groups(snapshot)
    start = (page - 1) * page_size
    return {
        "contract": ENTITY_CONTRACT,
        "version": ENTITY_VERSION,
        "cursor": cursor,
        "query": snapshot.query,
        "page": page,
        "entities": groups[start : start + page_size],
        "meta": {
            "total_entities": len(groups),
            "total_results": len(snapshot.results),
            "unresolved_entities": sum(group["entity_id"] is None for group in groups),
            "has_more": start + page_size < len(groups),
            "query_id": snapshot.query_id,
            "note": "Entity identity is source-reported, not independent corroboration or verification.",
        },
    }


async def slopsearx_read_result(result_id: str) -> dict[str, Any]:
    """Expand one server-issued result ID into a full result record.

    Returns complete content (not the card snippet), media fields, every
    contributing engine, provenance, a ``content_available`` flag, and an
    explicit note that SlopSearX did not fetch or verify the linked page.
    Served from the immutable snapshot — the search is not re-executed.
    """
    state = get_state()
    if ":" not in result_id:
        return _error("invalid_result_id", "result_id must be a server-issued snapshot result ID", field="result_id")
    snapshot_id, index_str = result_id.rsplit(":", 1)
    try:
        index = int(index_str)
    except ValueError:
        return _error("invalid_result_id", "malformed result_id", field="result_id")
    if index < 0:
        return _error("invalid_result_id", "result index out of range", field="result_id")

    lookup = await state.snapshots.for_tenant(current_tenant()).read(snapshot_id)
    if lookup.unavailable:
        return _error("store_unavailable", "snapshot store is unavailable", field="result_id")
    if lookup.expired:
        return _error(
            "expired_handle",
            "snapshot has expired",
            handle=result_id,
            expires_at=_expires_iso(lookup.expires_at),
            field="result_id",
        )
    snapshot = lookup.snapshot
    if snapshot is None:
        return _error("invalid_cursor", "unknown or expired snapshot", field="result_id")
    if index >= len(snapshot.results):
        return _error("invalid_result_id", "result index out of range", field="result_id")

    return _result_record(snapshot.results[index], snapshot, result_id)


# ---------------------------------------------------------------------------
# Research jobs
# ---------------------------------------------------------------------------


def _research_dispatch_error(state: McpState, query: ResearchQuery) -> str | None:
    if not state.policy.tool_enabled("research"):
        return "research grant is disabled"
    grant = INTENT_GRANTS.get(query.intent)
    if query.requires_intent_grant and grant and not state.policy.tool_enabled(grant):
        return f"{query.intent} requires the {grant} grant"
    if not query.engines:
        return "research query has no permitted engines"
    error = _enforce_policy(state, query.engines)
    return str(error["error"]["message"]) if error else None


def bind_research_policy(state: McpState) -> None:
    """Keep recovered and retried dispatches behind the shared live policy gate."""
    state.runner.dispatch_validator = lambda query: _research_dispatch_error(state, query)


def _research_metadata(value: Any, name: str, limit: int = 512) -> str | None:
    if value is None:
        return None
    if not isinstance(value, str) or not value.strip() or len(value) > limit:
        raise ResearchMutationError("invalid_input", f"{name} must be a nonempty string of at most {limit} characters")
    return value.strip()


def _prepare_research_query(state: McpState, entry: dict[str, Any], max_engines: int) -> ResearchQuery:
    allowed = {"query", "intent", "engines", "subquestion_id", "rationale", "parent_attempt_id"}
    if not isinstance(entry, dict) or set(entry) - allowed:
        raise ResearchMutationError("invalid_input", "invalid research plan entry fields")
    text = entry.get("query")
    if not isinstance(text, str) or _validate_query(text, state):
        raise ResearchMutationError("invalid_input", "query must be a nonempty bounded string")
    intent = entry.get("intent", "web")
    if not isinstance(intent, str) or intent not in INTENT_PROFILES:
        raise ResearchMutationError("invalid_input", "unknown research intent")
    if INTENT_PROFILES[intent].media_types:
        raise ResearchMutationError("invalid_intent", "research subqueries do not support media searches")
    grant = INTENT_GRANTS.get(intent)
    if grant and not state.policy.tool_enabled(grant):
        raise ResearchMutationError("tool_disabled", f"intent {intent} requires the {grant} grant")
    engines = entry.get("engines")
    if engines is not None:
        if not isinstance(engines, list) or not engines or any(not isinstance(name, str) for name in engines):
            raise ResearchMutationError("invalid_input", "engines must be a nonempty list of names")
        error = _enforce_policy(state, engines)
        if error:
            raise ResearchMutationError(str(error["error"]["code"]), str(error["error"]["message"]))
    else:
        engines, _ = resolve_intent(intent, state.catalog)
        engines = [
            name
            for name in engines
            if name not in state.policy.sensitive_engines or state.policy.targeted_sensitive_allowed
        ]
    engines = list(dict.fromkeys(engines))[:max_engines]
    query = ResearchQuery(
        index=0,
        query=text.strip(),
        intent=intent,
        engines=engines,
        subquestion_id=_research_metadata(entry.get("subquestion_id"), "subquestion_id", 128),
        rationale=_research_metadata(entry.get("rationale"), "rationale", state.policy.max_query_length),
        parent_attempt_id=_research_metadata(entry.get("parent_attempt_id"), "parent_attempt_id", 128),
        requires_intent_grant=True,
    )
    rejection = _research_dispatch_error(state, query)
    if rejection:
        raise ResearchMutationError("tool_disabled", rejection)
    return query


def _validate_research_associations(job: ResearchJob, query: ResearchQuery) -> None:
    if query.subquestion_id is not None and query.subquestion_id not in job.subquestions:
        raise ResearchMutationError("invalid_input", "unknown subquestion_id")
    if query.parent_attempt_id is not None and not any(
        attempt.attempt_id == query.parent_attempt_id and attempt.state != "running"
        for existing in job.queries
        for attempt in existing.attempts
    ):
        raise ResearchMutationError("invalid_input", "parent_attempt_id must identify a terminal attempt in this job")


def _research_limit(value: int | None, ceiling: int, field: str) -> int:
    if value is None:
        return ceiling
    if type(value) is not int or value <= 0:
        raise ResearchMutationError("invalid_input", f"{field} must be a positive integer")
    return min(value, ceiling)


async def slopsearx_start_research(
    question: str,
    strategy: str = "triangulate",
    max_queries: StrictInt | None = None,
    max_engines_per_query: StrictInt | None = None,
    deadline: str | None = None,
    idempotency_key: str | None = None,
    initial_plan: list[dict[str, Any]] | None = None,
    subquestions: list[dict[str, str]] | None = None,
    max_attempts: StrictInt | None = None,
    max_engine_attempts: StrictInt | None = None,
    max_results: StrictInt | None = None,
) -> dict[str, Any]:
    """Start an asynchronous multi-query research job.

    Strategies: triangulate (same question across independent sources),
    broad (several source families), fresh (recent material),
    counterevidence (limits, criticism, counterexamples). Returns a job
    handle immediately; poll slopsearx_get_job for progress.
    """
    state = get_state()
    if not state.policy.tool_enabled("research"):
        return _error("tool_disabled", "slopsearx_start_research requires the research grant (MCP_GRANT_RESEARCH=1)")

    tenant = current_tenant()
    store = state.job_store.for_tenant(tenant)

    if not question or not question.strip():
        return _error("invalid_input", "question is required", field="question")
    if len(question) > state.policy.max_query_length:
        return _error(
            "invalid_input",
            f"question exceeds the maximum length of {state.policy.max_query_length} characters",
            field="question",
        )
    if strategy not in VALID_STRATEGIES:
        return _error(
            "invalid_input",
            f"unknown strategy '{strategy}'",
            field="strategy",
            valid_alternatives=list(VALID_STRATEGIES),
        )

    if idempotency_key:
        existing = await store.find_by_idempotency(idempotency_key)
        if existing is not None:
            result = _job_summary(existing)
            result["note"] = "returned existing job for idempotency_key"
            return result

    bind_research_policy(state)
    try:
        limits = {
            "queries": _research_limit(max_queries, state.policy.job_max_queries, "max_queries"),
            "attempts": _research_limit(max_attempts, state.policy.job_max_queries, "max_attempts"),
            "engines_per_query": _research_limit(
                max_engines_per_query, state.policy.job_max_engines_per_query, "max_engines_per_query"
            ),
            "engine_attempts": _research_limit(
                max_engine_attempts,
                state.policy.job_max_queries * state.policy.job_max_engines_per_query,
                "max_engine_attempts",
            ),
            "results": _research_limit(max_results, state.policy.job_max_results, "max_results"),
        }
        limits["engine_attempts"] = min(limits["engine_attempts"], limits["attempts"] * limits["engines_per_query"])
        declared = {}
        if subquestions is not None:
            if not isinstance(subquestions, list) or len(subquestions) > limits["queries"]:
                raise ResearchMutationError("invalid_input", "subquestions exceeds query budget")
            for item in subquestions:
                if not isinstance(item, dict) or set(item) != {"id", "question"}:
                    raise ResearchMutationError("invalid_input", "subquestion requires exactly id and question")
                identity = _research_metadata(item["id"], "subquestion id", 128)
                question_text = _research_metadata(
                    item["question"], "subquestion question", state.policy.max_query_length
                )
                if identity is None or question_text is None or identity in declared:
                    raise ResearchMutationError("invalid_input", "duplicate or missing subquestion")
                declared[identity] = {"question": question_text, "state": "unresolved"}
    except ResearchMutationError as exc:
        return _error(exc.code, str(exc))

    deadline_ts = _resolve_deadline(state, deadline)
    if isinstance(deadline_ts, dict):
        return deadline_ts

    try:
        if initial_plan is not None:
            if not isinstance(initial_plan, list) or not initial_plan or len(initial_plan) > limits["queries"]:
                raise ResearchMutationError("invalid_input", "initial_plan must fit the positive query budget")
            queries = [_prepare_research_query(state, entry, limits["engines_per_query"]) for entry in initial_plan]
            warnings: list[str] = []
            for index, query in enumerate(queries):
                query.index = index
                if query.parent_attempt_id is not None:
                    raise ResearchMutationError("invalid_input", "initial queries cannot reference parent attempts")
                if query.subquestion_id is not None and query.subquestion_id not in declared:
                    raise ResearchMutationError("invalid_input", "unknown subquestion_id")
            if len(queries) > limits["attempts"] or sum(len(q.engines) for q in queries) > limits["engine_attempts"]:
                raise ResearchMutationError("job_budget_exceeded", "initial plan exceeds execution budget")
        else:
            queries, warnings = plan_research_queries(
                question.strip(),
                strategy,
                limits["queries"],
                limits["engines_per_query"],
                state.catalog,
                state.policy,
            )
            for query in queries:
                # Empty template scopes are reported as failed queries, never dispatched unscoped.
                error = _enforce_policy(state, query.engines)
                if error:
                    raise ResearchMutationError(str(error["error"]["code"]), str(error["error"]["message"]))
    except ResearchMutationError as exc:
        return _error(exc.code, str(exc))
    if not queries:
        return _error("invalid_input", "; ".join(warnings) or "no queries could be planned", field="strategy")

    job = ResearchJob(
        job_id=generate_job_id(),
        question=question.strip(),
        strategy=strategy,
        queries=queries,
        warnings=warnings,
        deadline=deadline_ts,
        tenant=tenant,
        idempotency_key=idempotency_key,
        subquestions=declared,
        budget_limits=limits,
        budget_used={"attempts": 0, "engine_attempts": 0, "results": 0},
    )
    await store.save(job)
    state.runner.enqueue(job.job_id, tenant=tenant)

    result = _job_summary(job)
    if not store.available:
        result["degraded"] = True
        result["ephemeral"] = True
        result["note"] = (
            "job store unavailable — this job was not persisted and will not be "
            "executed (research jobs require Valkey); idempotency was not checked "
            "or persisted (VAL-RESEARCH-003)"
        )
    else:
        result["note"] = "job queued; in-flight engine calls are not interrupted by cancellation"
    return result


def _resolve_deadline(state: McpState, deadline: str | None) -> float | dict[str, Any]:
    """Parse/clamp the job deadline; returns a unix timestamp or error dict."""
    if deadline:
        try:
            parsed = _dt.datetime.fromisoformat(deadline.replace("Z", "+00:00"))
            ts = parsed.timestamp()
        except ValueError:
            return _error("invalid_input", "deadline must be an ISO 8601 timestamp", field="deadline")
    else:
        ts = time.time() + state.policy.job_default_deadline_seconds

    now = time.time()
    earliest = now + 60
    latest = now + 86_400  # 24h cap
    return max(earliest, min(ts, latest))


async def slopsearx_get_job(job_id: str) -> dict[str, Any]:
    """Return research job state, progress, and per-query cursors."""
    state = get_state()
    store = state.job_store.for_tenant(current_tenant())
    if not store.available:
        return _error("store_unavailable", "job store is unavailable; research jobs are not persisted", field="job_id")
    job = await store.load(job_id)
    if job is None:
        return _error("invalid_job_id", "unknown job id", field="job_id")
    result = _job_summary(job)
    result["created_at"] = _dt.datetime.fromtimestamp(job.created_at, tz=_dt.timezone.utc).isoformat()
    result["note"] = "completed queries are immutable; their cursors remain readable"
    return result


async def slopsearx_cancel_job(job_id: str) -> dict[str, Any]:
    """Best-effort cancellation of a research job.

    Stops undispatched queries; in-flight upstream calls complete and
    their evidence stays readable.
    """
    state = get_state()
    store = state.job_store.for_tenant(current_tenant())
    if not store.available:
        return _error("store_unavailable", "job store is unavailable; research jobs are not persisted", field="job_id")
    job = await store.load(job_id)
    if job is None:
        return _error("invalid_job_id", "unknown job id", field="job_id")

    if job.state in ("succeeded", "partial", "failed", "cancelled", "expired"):
        return {
            "job_id": job.job_id,
            "state": job.state,
            "note": "job already finished; completed evidence remains readable",
        }

    result_state = await store.request_cancel(job_id)
    if result_state == "running":
        return {
            "job_id": job.job_id,
            "state": "running",
            "note": "cancellation requested; the owning worker will stop undispatched "
            "queries and preserve completed evidence",
        }
    if result_state != "cancelled":
        return {
            "job_id": job.job_id,
            "state": result_state,
            "note": "best-effort cancellation requested; completed evidence remains readable",
        }
    return {
        "job_id": job.job_id,
        "state": "cancelled",
        "note": "best-effort cancellation requested; undispatched queries are stopped, "
        "in-flight engine calls were not interrupted",
    }


async def slopsearx_retry_research(job_id: str) -> dict[str, Any]:
    """Re-run only failed/empty subqueries of a research job.

    Successful subqueries are never re-executed; their snapshot cursors
    remain byte-for-byte unchanged. Each retried subquery is linked to the
    same job under a NEW attempt/cursor; the original failed/empty attempt's
    evidence stays readable. Returns a structured ``no_retryable_work``
    error when the job has no failed/empty work (VAL-RESEARCH-008/016/021).
    """
    state = get_state()
    if not state.policy.tool_enabled("research"):
        return _error("tool_disabled", "slopsearx_retry_research requires the research grant (MCP_GRANT_RESEARCH=1)")
    tenant = current_tenant()
    store = state.job_store.for_tenant(tenant)
    if not store.available:
        return _error("store_unavailable", "job store is unavailable; research jobs are not persisted", field="job_id")
    job = await store.load(job_id)
    if job is None:
        return _error("invalid_job_id", "unknown job id", field="job_id")

    # Terminal-state gate: a cancelled or already-expired job is never
    # resurrected by a retry (VAL-RESEARCH-010/011).
    if job.state in ("cancelled", "expired") or job.caller_completed:
        return {
            "job_id": job.job_id,
            "state": job.state,
            "note": "job already reached a terminal state; it was not retried",
        }

    retryable = [query for query in job.queries if is_retryable_query(query)]
    if not retryable:
        return _error(
            "no_retryable_work",
            "job has no failed or empty subqueries to retry",
            job_id=job.job_id,
            state=job.state,
            field="job_id",
        )

    attempt_counts = {query.index: len(query.attempts) for query in job.queries}
    bind_research_policy(state)
    try:
        job = await state.runner.retry(job_id, tenant=tenant)
    except JobStillRunningError:
        return {
            "job_id": job_id,
            "state": "running",
            "note": "job is still running under a live worker; retry was not started to avoid concurrent execution",
        }
    except LeaseLostError:
        return {
            "job_id": job_id,
            "state": "running",
            "note": "job was reclaimed by another worker mid-run; it will be completed by that worker",
        }
    if job is None:
        return _error("invalid_job_id", "unknown job id", field="job_id")

    if job.state == "cancelled":
        # A durable cancel flag finalized the job mid-operation: no subquery
        # was actually re-run, so surface the cancellation rather than a
        # success note.
        return {
            "job_id": job.job_id,
            "state": "cancelled",
            "note": "the job had a pending cancellation request; the retry was cancelled, not executed",
        }

    result = _job_summary(job)
    if job.state == "expired":
        # Deadline gate: a deadline-passed retry finalizes to expired instead
        # of re-running and ending in partial/failed.
        result["note"] = (
            "job deadline had already passed; retry finalized the job to expired and re-executed no subqueries"
        )
    else:
        result["retried"] = [
            query.index for query in job.queries if len(query.attempts) > attempt_counts.get(query.index, 0)
        ]
        result["note"] = "retry accounting recorded; inspect retried indices and stop_reason for executed work"
    return result


async def slopsearx_extend_research(
    job_id: str,
    query: str,
    intent: str = "web",
    engines: list[str] | None = None,
    subquestion_id: str | None = None,
    rationale: str | None = None,
    parent_attempt_id: str | None = None,
    continuation_key: str | None = None,
) -> dict[str, Any]:
    """Append and execute one bounded follow-up query to a research job.

    Appends a new subquery only while the job has remaining query budget
    (under ``job_max_queries``) and its deadline has not passed. The
    follow-up is validated and routed through the same sensitive-engine /
    specialist-grant gate as a normal subquery (VAL-RESEARCH-009/015/020).
    """
    state = get_state()
    if not state.policy.tool_enabled("research"):
        return _error("tool_disabled", "slopsearx_extend_research requires the research grant (MCP_GRANT_RESEARCH=1)")
    store = state.job_store.for_tenant(current_tenant())
    if not store.available:
        return _error("store_unavailable", "job store is unavailable; research jobs are not persisted", field="job_id")
    job = await store.load(job_id)
    if job is None:
        return _error("invalid_job_id", "unknown job id", field="job_id")

    error = _validate_query(query, state)
    if error:
        return error
    if intent not in INTENT_PROFILES:
        return _error(
            "invalid_input",
            f"unknown intent '{intent}'",
            field="intent",
            valid_alternatives=sorted(INTENT_PROFILES),
        )
    required_grant = INTENT_GRANTS.get(intent)
    if required_grant is not None and not state.policy.tool_enabled(required_grant):
        return _error(
            "tool_disabled",
            f"intent '{intent}' requires the {required_grant} grant ({GRANT_ENV[required_grant]}=1)",
            field="intent",
            grant=GRANT_ENV[required_grant],
        )
    # Research subqueries carry no media_type/categories, so a media intent
    # (images/videos) cannot be dispatched as a media search. Reject it
    # explicitly rather than silently running a text search over the
    # media-capable engines (VAL-RESEARCH-020).
    if INTENT_PROFILES[intent].media_types:
        return _error(
            "invalid_intent",
            f"intent '{intent}' selects a media type; research subqueries do not support media searches",
            field="intent",
        )

    if engines is not None and not engines:
        return _error("invalid_input", "engines must be a nonempty list when supplied", field="engines")

    # Resolve the follow-up engine scope through the shared policy gate.
    if engines:
        policy_error = _enforce_policy(state, list(engines), field="engines")
        if policy_error:
            return policy_error
        # Cap an explicit engine list to the per-query bound, in parity with
        # the intent path below, so extend can never exceed job_max_engines.
        resolved_engines = list(engines)[: state.policy.job_max_engines_per_query]
    else:
        resolved_engines, _ = resolve_intent(intent, state.catalog)
        resolved_engines = resolved_engines[: state.policy.job_max_engines_per_query]
        sensitive = [
            name
            for name in resolved_engines
            if name in state.policy.sensitive_engines and not state.policy.targeted_sensitive_allowed
        ]
        if sensitive:
            resolved_engines = [name for name in resolved_engines if name not in state.policy.sensitive_engines]

    new_query = ResearchQuery(
        index=len(job.queries),
        query=query.strip(),
        intent=intent,
        engines=list(dict.fromkeys(resolved_engines)),
        requires_intent_grant=True,
    )
    try:
        new_query.subquestion_id = _research_metadata(subquestion_id, "subquestion_id", 128)
        new_query.rationale = _research_metadata(rationale, "rationale", state.policy.max_query_length)
        new_query.parent_attempt_id = _research_metadata(parent_attempt_id, "parent_attempt_id", 128)
        new_query.continuation_key = _research_metadata(continuation_key, "continuation_key", 128)
    except ResearchMutationError as exc:
        return _error(exc.code, str(exc))
    equivalent = [
        new_query.query,
        new_query.intent,
        sorted(new_query.engines),
        new_query.subquestion_id,
        new_query.rationale,
        new_query.parent_attempt_id,
    ]
    new_query.continuation_digest = hashlib.sha256(json.dumps(equivalent).encode()).hexdigest()
    # Read-only replay remains available after completion/deadline. Current
    # caller and scope policy above still applies; no lease or dispatch needed.
    if new_query.continuation_key:
        previous = next((q for q in job.queries if q.continuation_key == new_query.continuation_key), None)
        if previous is not None:
            if previous.continuation_digest != new_query.continuation_digest:
                return _error("idempotency_conflict", "continuation_key was used for another request")
            result = _job_summary(job)
            result["note"] = "returned the current status of the previously accepted continuation"
            return result

    if time.time() >= job.deadline:
        return _error("deadline_exceeded", "job deadline has passed; cannot extend", field="query")

    # Terminal-state gate: mirror retry and refuse to append a follow-up
    # query to a job that already reached a terminal state (never resurrect
    # a cancelled/expired job).
    if job.state in ("cancelled", "expired") or job.caller_completed:
        return _error(
            "invalid_job_state",
            f"job is in terminal state '{job.state}'; cannot extend",
            job_id=job.job_id,
            state=job.state,
            field="query",
        )

    bind_research_policy(state)

    def _append_followup(target: ResearchJob) -> None:
        initialize_budget(target, state.policy)
        if target.caller_completed:
            raise ResearchMutationError("invalid_job_state", "caller already completed this job")
        _validate_research_associations(target, new_query)
        if new_query.continuation_key:
            previous = next((q for q in target.queries if q.continuation_key == new_query.continuation_key), None)
            if previous is not None:
                if previous.continuation_digest != new_query.continuation_digest:
                    raise ResearchMutationError("idempotency_conflict", "continuation_key was used for another request")
                raise ResearchMutationError("continuation_replayed", "continuation already accepted")
        if len(target.queries) >= target.budget_limits["queries"]:
            raise ResearchMutationError("job_budget_exceeded", "job query budget is exhausted")
        for key, amount in (("attempts", 1), ("engine_attempts", len(new_query.engines)), ("results", 1)):
            if target.budget_used[key] + amount > target.budget_limits[key]:
                raise ResearchMutationError("job_budget_exceeded", f"job {key} budget is exhausted")
        if len(new_query.engines) > target.budget_limits["engines_per_query"]:
            raise ResearchMutationError("job_budget_exceeded", "follow-up exceeds per-query engine budget")
        rejection = _research_dispatch_error(state, new_query)
        if rejection:
            raise ResearchMutationError("tool_disabled", rejection)
        target.queries.append(dataclasses.replace(new_query, index=len(target.queries)))

    # A job previously executed by the durable worker still carries lease
    # fields whose Valkey key was already released. Run through run_direct so
    # the stale lease is cleared before run_pending (which otherwise tries to
    # renew the missing lease and raises LeaseLostError). The follow-up is
    # applied to the freshly loaded record (never the caller's copy) so a
    # record finalized concurrently is reconciled, not clobbered. If a live
    # worker still owns the job, run_direct raises JobStillRunningError
    # instead of racing it.
    try:
        job = await state.runner.run_direct(job, mutate=_append_followup)
    except ResearchMutationError as exc:
        if exc.code == "continuation_replayed":
            current = await store.load(job_id)
            if current is not None:
                result = _job_summary(current)
                result["note"] = "returned the current status of the previously accepted continuation"
                return result
        return _error(exc.code, str(exc), field="query")
    except JobStillRunningError:
        return {
            "job_id": job_id,
            "state": "running",
            "note": "job is still running under a live worker; follow-up query was not appended or executed",
        }
    except LeaseLostError:
        return {
            "job_id": job_id,
            "state": "running",
            "note": "job was reclaimed by another worker mid-run; the follow-up query will be completed by that worker",
        }
    if job.state == "cancelled":
        # A durable cancel flag finalized the job mid-operation: the follow-up
        # was not executed, so surface the cancellation instead of a success
        # note.
        return {
            "job_id": job.job_id,
            "state": "cancelled",
            "note": "the job had a pending cancellation request; the follow-up query was cancelled, not executed",
        }
    if job.state == "expired":
        # The job's deadline lapsed during the direct run (the deadline gate
        # above passed against the loaded record, then the deadline passed
        # before/while the run claimed the job): the follow-up was not
        # appended or executed, so surface the terminal state instead of
        # claiming it was.
        return {
            "job_id": job.job_id,
            "state": "expired",
            "note": "job deadline had already passed; the follow-up query was not appended or executed",
        }
    result = _job_summary(job)
    result["note"] = "follow-up query appended and executed; prior completed evidence was preserved"
    return result


async def slopsearx_update_research(
    job_id: str,
    subquestion_states: dict[str, str],
    complete: bool = False,
    rationale: str | None = None,
) -> dict[str, Any]:
    """Record caller-declared progress or completion without judging evidence.

    State values are resolved/unresolved. Completion preserves unresolved
    questions and permanently prevents new execution for this job.
    """
    state = get_state()
    if not state.policy.tool_enabled("research"):
        return _error("tool_disabled", "research grant is required")
    store = state.job_store.for_tenant(current_tenant())
    if not store.available:
        return _error("store_unavailable", "job store is unavailable")
    job = await store.load(job_id)
    if job is None:
        return _error("invalid_job_id", "unknown job id", field="job_id")
    if not isinstance(subquestion_states, dict) or any(
        value not in ("resolved", "unresolved") for value in subquestion_states.values()
    ):
        return _error("invalid_input", "subquestion states must be resolved or unresolved")
    if type(complete) is not bool:
        return _error("invalid_input", "complete must be a boolean")
    try:
        rationale = _research_metadata(rationale, "rationale", state.policy.max_query_length)
    except ResearchMutationError as exc:
        return _error(exc.code, str(exc))
    if job.state in ("cancelled", "expired") or job.caller_completed:
        return _error("invalid_job_state", "job is already terminal")

    def update(target: ResearchJob) -> None:
        if set(subquestion_states) - set(target.subquestions):
            raise ResearchMutationError("invalid_input", "unknown subquestion id")
        initialize_budget(target, state.policy)
        for identity, value in subquestion_states.items():
            target.subquestions[identity]["state"] = value
        if complete:
            target.caller_completed = True
            target.completion_rationale = rationale
            target.stop_reason = "caller_completed"
            # Operational success here means the caller closed work, not that answers are true.
            for query in target.queries:
                if query.state in ("pending", "running"):
                    query.state = "cancelled"
            target.state = "succeeded"

    try:
        return _job_summary(await state.runner.run_direct(job, mutate=update, execute=False))
    except ResearchMutationError as exc:
        return _error(exc.code, str(exc))
    except (JobStillRunningError, LeaseLostError):
        return _error("job_busy", "job is owned by a live worker; retry the progress update later")


def _job_summary(job: ResearchJob) -> dict[str, Any]:
    """Compact job view for tool responses.

    Exposes per-query state plus per-engine coverage (each entry carrying
    ``{engine, bucket, status, result_count, failure_class}``) and the
    disjoint coverage summary per query and at the job level.
    """
    completed, total = job.progress
    job_coverage = summarize_coverage([entry for query in job.queries for entry in query.engine_coverage])
    return {
        "job_id": job.job_id,
        "state": job.state,
        "question": job.question,
        "strategy": job.strategy,
        "progress": {"completed": completed, "total": total},
        "deadline": _deadline_iso(job.deadline) if job.deadline else None,
        "idempotency_key": job.idempotency_key,
        "queries": [
            {
                "index": query.index,
                "query": query.query,
                "intent": query.intent,
                "engines": query.engines,
                "state": query.state,
                "result_count": query.result_count,
                "query_id": query.query_id,
                "cursor": query.cursor,
                "error": query.error,
                "subquestion_id": query.subquestion_id,
                "rationale": query.rationale,
                "parent_attempt_id": query.parent_attempt_id,
                "continuation_key": query.continuation_key,
                "attempts": [dataclasses.asdict(attempt) for attempt in query.attempts],
                "engine_coverage": [
                    {
                        "engine": cov.engine,
                        "bucket": cov.bucket,
                        "status": cov.status,
                        "result_count": cov.result_count,
                        "failure_class": cov.failure_class,
                    }
                    for cov in query.engine_coverage
                ],
                "coverage": summarize_coverage(query.engine_coverage).as_dict(),
                "enforcement": query.enforcement,
            }
            for query in job.queries
        ],
        "coverage": job_coverage.as_dict(),
        "warnings": job.warnings,
        "subquestions": job.subquestions,
        "unresolved_subquestions": [
            identity for identity, item in job.subquestions.items() if item["state"] == "unresolved"
        ],
        "caller_completed": job.caller_completed,
        "completion_rationale": job.completion_rationale,
        "stop_reason": job.stop_reason,
        "budgets": budget_summary(job),
    }
