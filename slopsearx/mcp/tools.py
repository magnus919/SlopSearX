"""MCP tool implementations for SlopSearX.

Each function is a plain async callable (FastMCP-free) so the logic is
testable without an MCP runtime; ``slopsearx.mcp.server`` registers
them with FastMCP. Tools return JSON-serializable dicts — the typed
envelope described in docs/MCP_SERVER_DESIGN.md §3.
"""

from __future__ import annotations

import datetime as _dt
import time
from typing import Any

from slopsearx.adapter import SearchResult
from slopsearx.capabilities import INTENT_PROFILES
from slopsearx.mcp.state import McpState, get_state
from slopsearx.ratelimit import ValkeySlidingWindow
from slopsearx.research import (
    ResearchJob,
    generate_job_id,
    plan_research_queries,
)
from slopsearx.service import (
    QueryValidationError,
    RateLimitExceededError,
    ScopeResolver,
    SearchRequest,
)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

VALID_INTENTS = tuple(INTENT_PROFILES)
VALID_SAFESEARCH = ("off", "moderate", "strict")
VALID_FRESHNESS = ("prefer_cache", "prefer_fresh", "no_preference")
VALID_STRATEGIES = ("triangulate", "broad", "fresh", "counterevidence")

# No adapter consumes the safesearch parameter (Brave hardcodes it off),
# so strict SafeSearch can never be honestly enforced — fail closed.
SAFESEARCH_UNENFORCED_NOTE = (
    "no adapter enforces the safesearch parameter (Brave hardcodes it off); "
    "strict SafeSearch cannot be guaranteed — use 'off' or 'moderate'"
)

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

RANKING_EXPLANATION = "tier_then_cross_engine_presence"

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
    return f"mcp:{state.tenant}"


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


def _sensitive_check(state: McpState, engines: list[str]) -> dict[str, Any] | None:
    """Reject sensitive engines in targeted search unless granted."""
    sensitive = [name for name in engines if name in state.policy.sensitive_engines]
    if sensitive and not state.policy.targeted_sensitive_allowed:
        return _error(
            "tool_disabled",
            "explicit selection of sensitive engines requires an operator grant (MCP_TARGETED_SENSITIVE_ALLOWED=1)",
            field="engines",
            engines=sensitive,
        )
    return None


def _safesearch_value(safesearch: str) -> int:
    return {"off": 0, "moderate": 1, "strict": 2}[safesearch]


def _filter_warnings(state: McpState, language: str, time_range: str | None) -> list[str]:
    """Honest warnings for filter parameters no adapter enforces."""
    warnings: list[str] = []
    if language != "en":
        warnings.append(f"language '{language}' is not consumed by any adapter")
    if time_range:
        warnings.append(f"time_range '{time_range}' is not consumed by any adapter")
    return warnings


def _safesearch_warning_or_error(state: McpState, safesearch: str) -> list[str] | dict[str, Any]:
    """Strict SafeSearch fails closed; moderate returns a warning."""
    if safesearch == "strict":
        return _error("safesearch_unenforced", SAFESEARCH_UNENFORCED_NOTE, field="safesearch")
    if safesearch == "moderate":
        return ["moderate safesearch is requested but no adapter enforces it"]
    return []


def _result_to_dict(result: SearchResult) -> dict[str, Any]:
    """Normalize one result for MCP output (design §3.1)."""
    return {
        "title": result.title,
        "url": result.url,
        "snippet": (result.content or "")[:300],
        "source_engines": sorted(result.engines) if result.engines else [result.engine],
        "source_count": len(result.engines) if result.engines else 1,
        "primary_engine": result.engine,
        "category": result.category,
        "published_at": result.published_date,
        "score": result.score,
        "position": result.position,
        "tier": result.tier,
        "citation": {"label": result.title, "url": result.url},
    }


def _envelope(
    state: McpState,
    response: Any,
    *,
    requested_intent: str,
    warnings: list[str],
    cursor: str | None,
    include_suggestions: bool,
) -> dict[str, Any]:
    """Build the standard search envelope from a SearchResponse."""
    if response.all_unresponsive:
        return _error(
            "all_engines_failed",
            "every selected engine failed to respond",
            query_id=response.query_id,
            scope={
                "requested_intent": requested_intent,
                "selected_engines": response.scope.selected_engines,
                "routing_reason": response.scope.routing_rule,
            },
            engine_outcomes=[
                {"engine": o.engine, "status": o.status, "result_count": o.result_count, "message": o.message}
                for o in response.engine_outcomes
            ],
            retry_guidance="check the engine outcomes, adjust scope, and retry",
        )
    return {
        "query": response.query,
        "results": [_result_to_dict(result) for result in response.results],
        "scope": {
            "requested_intent": requested_intent,
            "resolved_categories": response.scope.resolved_categories,
            "selected_engines": response.scope.selected_engines,
            "routing_reason": response.scope.routing_rule,
        },
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
            "response_time_ms": response.response_time_ms,
            "partial": response.partial,
            "ranking": RANKING_EXPLANATION,
            "cursor": cursor,
            "suggestions": response.suggestions if include_suggestions else [],
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
) -> dict[str, Any]:
    """Execute one search through the service and build the envelope.

    The full ranked set is captured as an immutable snapshot (for
    pagination); ``max_results`` is a presentation bound applied to the
    returned page only.
    """
    try:
        response = await state.service.search(request)
    except QueryValidationError as exc:
        return _error("invalid_input", str(exc), field="query")
    except RateLimitExceededError:
        return _error("rate_limited", "too many requests; please retry later")

    # Capture the full ranked set as an immutable snapshot for pagination,
    # then present the bounded page.
    cursor = await state.snapshots.create(response.query, response.query_id, response.results, response.scope)
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
    )


def _deadline_iso(deadline: float) -> str:
    return _dt.datetime.fromtimestamp(deadline, tz=_dt.timezone.utc).isoformat()


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
    max_results: int | None = None,
    include: list[str] | None = None,
    freshness: str = "no_preference",
) -> dict[str, Any]:
    """Search across SlopSearX engines with intent-based routing.

    - intent: one of auto, web, news, science, reference, code, social,
      historical, jobs, security, medical, finance, packages, media,
      legal, geography. auto uses query-topic routing with tier-1 fallback.
    - categories: explicit category OR-filter (overridden by engines).
    - engines: explicit engine list (overrides everything; must be known).
    - safesearch: off | moderate | strict. strict fails closed because no
      adapter enforces it.
    - freshness: prefer_cache | prefer_fresh | no_preference.
    - include: subset of results, suggestions, engine_status, diagnostics.
    Returns results, scope, engine outcomes, and a pagination cursor.
    """
    state = get_state()

    if intent != "auto" and intent not in VALID_INTENTS:
        return _error(
            "invalid_input",
            f"unknown intent '{intent}'",
            field="intent",
            valid_alternatives=list(VALID_INTENTS),
        )
    if safesearch not in VALID_SAFESEARCH:
        return _error("invalid_input", "safesearch must be off, moderate, or strict", field="safesearch")
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

    # Strict SafeSearch fails closed: no adapter enforces it.
    safesearch_check = _safesearch_warning_or_error(state, safesearch)
    if isinstance(safesearch_check, dict):
        return safesearch_check

    # Resolve intent → scope (explicit inputs win over the profile).
    resolved_categories, resolved_engines, requested_intent, warnings = _resolve_scope(
        state, intent, categories, engines
    )
    if isinstance(resolved_categories, dict):  # error envelope
        return resolved_categories

    include_set = set(include) if include is not None else {"results", "engine_status"}
    max_results = _bounded_max_results(state, max_results)

    request = SearchRequest(
        query=query,
        categories=resolved_categories,
        engines=resolved_engines,
        language=language,
        time_range=time_range,
        safesearch=_safesearch_value(safesearch),
        include=include_set,
        freshness=freshness,
        client_identifier=_client_identifier(state),
    )
    warnings = warnings + _filter_warnings(state, language, time_range)
    if isinstance(safesearch_check, list):
        warnings = warnings + safesearch_check

    return await _run_search(
        state,
        request,
        requested_intent,
        warnings,
        include_suggestions="suggestions" in include_set,
        max_results=max_results,
    )


def _resolve_scope(
    state: McpState,
    intent: str,
    categories: list[str] | None,
    engines: list[str] | None,
) -> tuple[list[str] | dict[str, Any] | None, list[str] | None, str, list[str]]:
    """Resolve intent/scope precedence.

    Returns (categories, engines, requested_intent, warnings); a dict as
    the first element signals an error envelope.
    """
    if engines:
        error = _validate_engines(state, engines)
        if error:
            return error, None, intent, []
        return None, engines, intent, []

    if categories:
        return categories, None, intent, []

    if intent == "auto":
        return None, None, "auto", []

    profile = INTENT_PROFILES.get(intent)
    if profile is None:
        return _error("invalid_input", f"unknown intent '{intent}'", field="intent"), None, intent, []
    if profile.sensitive and not state.policy.tool_enabled("security"):
        return (
            _error(
                "tool_disabled",
                f"intent '{intent}' requires the security grant (MCP_GRANT_SECURITY=1)",
                field="intent",
            ),
            None,
            intent,
            [],
        )
    if profile.engines:
        engines_list = [name for name in profile.engines if name in state.catalog.known_names()]
        return None, engines_list, intent, [f"intent profile '{intent}' selected explicit engines"]
    return profile.categories, None, intent, [f"intent profile '{intent}' selected categories"]


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
    error = _validate_engines(state, engines)
    if error:
        return error
    error = _sensitive_check(state, engines)
    if error:
        return error
    if safesearch not in VALID_SAFESEARCH:
        return _error("invalid_input", "safesearch must be off, moderate, or strict", field="safesearch")

    safesearch_check = _safesearch_warning_or_error(state, safesearch)
    if isinstance(safesearch_check, dict):
        return safesearch_check

    request = SearchRequest(
        query=query,
        engines=engines,
        language=language,
        time_range=time_range,
        safesearch=_safesearch_value(safesearch),
        include={"results", "engine_status"},
        client_identifier=_client_identifier(state),
    )
    warnings = _filter_warnings(state, language, time_range)
    if isinstance(safesearch_check, list):
        warnings = warnings + safesearch_check
    return await _run_search(
        state,
        request,
        "explicit engine",
        warnings,
        include_suggestions=False,
        max_results=_bounded_max_results(state, max_results),
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
    error = _validate_engines(state, sources)
    if error:
        return error
    error = _sensitive_check(state, sources)
    if error:
        return error

    title = " ".join(keywords or []).strip()
    query = f"{title} at {company.strip()}" if title else f"jobs at {company.strip()}"
    warnings = [JOBS_LIMITATION_NOTE]
    if location:
        warnings.append(f"location '{location}' is not consumed by current adapters")
    if employment_type:
        warnings.append(f"employment_type '{employment_type}' is not consumed by current adapters")

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
        error = _validate_engines(state, engines)
        if error:
            return error
        selected = list(engines)
    else:
        selected = [name for name in selected if name in state.catalog.known_names()]

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
        error = _validate_engines(state, engines)
        if error:
            return error
        selected = list(engines)
    else:
        selected = [name for name in selected if name in state.catalog.known_names()]

    warnings = [SCIENCE_LIMITATION_NOTE, f"resolved source_types: {', '.join(source_types)}"]
    if date_from or date_to:
        warnings.append("date_from/date_to are not consumed by current adapters; use time_range in slopsearx_search")

    request = SearchRequest(
        query=query,
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
) -> dict[str, Any]:
    """Dry-run routing preview: which engines would run and why.

    Executes no searches and spends no rate limits. Useful to correct
    scope before dispatching.
    """
    state = get_state()
    error = _validate_query(query, state)
    if error:
        return error

    resolved_categories, resolved_engines, requested_intent, warnings = _resolve_scope(
        state, intent, categories, engines
    )
    if isinstance(resolved_categories, dict):
        return resolved_categories

    resolver = ScopeResolver(
        active_engines=state.ctx.active_engines,
        router=state.ctx.router,
        tier1_engines=state.ctx.tier1_engines,
        sensitive_engines=state.policy.sensitive_engines,
    )
    decision = resolver.explain(SearchRequest(query=query, categories=resolved_categories, engines=resolved_engines))
    return {
        "selected_engines": decision.selected_engines,
        "excluded_engines": [{"engine": e.engine, "reason": e.reason} for e in decision.excluded_engines],
        "routing_rule": decision.routing_rule,
        "matched_topic": decision.matched_topic,
        "requested_intent": requested_intent,
        "warnings": warnings + decision.warnings,
    }


async def slopsearx_get_service_status() -> dict[str, Any]:
    """Operational status: liveness, Valkey, engine inventory.

    /health does not actively probe external APIs — engine health is
    observed passively through search outcomes.
    """
    state = get_state()
    ctx = state.ctx
    window = ctx.client_rate_window
    valkey_connected: bool | None = False
    fail_closed = False
    if isinstance(window, ValkeySlidingWindow):
        valkey_connected = window._connected
        fail_closed = window._fail_closed

    return {
        "status": "ok",
        "version": state.version,
        "valkey": {"connected": valkey_connected, "fail_closed": fail_closed},
        "cache_connected": bool(ctx.cache is not None and ctx.cache.is_connected),
        "snapshots_available": state.snapshots.available,
        "job_store_available": state.job_store.available,
        "active_engines": len(ctx.active_engines),
        "router_enabled": bool(ctx.router is not None and ctx.router.enabled),
        "engine_health": {
            "note": "/health does not actively probe external APIs; use search outcomes for passive engine health"
        },
    }


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
    snapshot = await state.snapshots.get(cursor)
    if snapshot is None:
        return _error("invalid_cursor", "unknown or expired cursor", field="cursor")

    start = (page - 1) * page_size
    end = start + page_size
    page_results = snapshot.results[start:end]
    return {
        "query": snapshot.query,
        "cursor": cursor,
        "page": page,
        "results": [_result_to_dict(result) for result in page_results],
        "meta": {
            "total": snapshot.total,
            "has_more": end < snapshot.total,
            "query_id": snapshot.query_id,
        },
    }


async def slopsearx_read_result(result_id: str) -> dict[str, Any]:
    """Expand one server-issued result ID from a snapshot.

    Includes provenance (query, engines, rank explanation). SlopSearX
    does not fetch or verify the full page body.
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

    snapshot = await state.snapshots.get(snapshot_id)
    if snapshot is None:
        return _error("invalid_cursor", "unknown or expired snapshot", field="result_id")
    if index >= len(snapshot.results):
        return _error("invalid_result_id", "result index out of range", field="result_id")

    result = snapshot.results[index]
    expanded = _result_to_dict(result)
    expanded["provenance"] = {
        "query": snapshot.query,
        "query_id": snapshot.query_id,
        "source_engines": sorted(result.engines) if result.engines else [result.engine],
        "rank_explanation": RANKING_EXPLANATION,
    }
    expanded["note"] = "SlopSearX did not fetch or verify the full page body"
    return expanded


# ---------------------------------------------------------------------------
# Research jobs
# ---------------------------------------------------------------------------


async def slopsearx_start_research(
    question: str,
    strategy: str = "triangulate",
    max_queries: int | None = None,
    max_engines_per_query: int | None = None,
    deadline: str | None = None,
    idempotency_key: str | None = None,
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
        existing = await state.job_store.find_by_idempotency(idempotency_key)
        if existing is not None:
            result = _job_summary(existing)
            result["note"] = "returned existing job for idempotency_key"
            return result

    max_queries = min(max_queries or state.policy.job_max_queries, state.policy.job_max_queries)
    max_engines = min(
        max_engines_per_query or state.policy.job_max_engines_per_query, state.policy.job_max_engines_per_query
    )

    deadline_ts = _resolve_deadline(state, deadline)
    if isinstance(deadline_ts, dict):
        return deadline_ts

    queries, warnings = plan_research_queries(
        question.strip(),
        strategy,
        max_queries,
        max_engines,
        state.catalog,
        state.policy,
    )
    if not queries:
        return _error("invalid_input", "; ".join(warnings) or "no queries could be planned", field="strategy")

    job = ResearchJob(
        job_id=generate_job_id(),
        question=question.strip(),
        strategy=strategy,
        queries=queries,
        warnings=warnings,
        deadline=deadline_ts,
        tenant=state.tenant,
        idempotency_key=idempotency_key,
    )
    await state.job_store.save(job)
    state.runner.enqueue(job.job_id)

    result = _job_summary(job)
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
    job = await state.job_store.load(job_id)
    if job is None:
        return _error("invalid_job_id", "unknown job id", field="job_id")
    result = _job_summary(job)
    result["deadline"] = _deadline_iso(job.deadline)
    result["created_at"] = _dt.datetime.fromtimestamp(job.created_at, tz=_dt.timezone.utc).isoformat()
    result["note"] = "completed queries are immutable; their cursors remain readable"
    return result


async def slopsearx_cancel_job(job_id: str) -> dict[str, Any]:
    """Best-effort cancellation of a research job.

    Stops undispatched queries; in-flight upstream calls complete and
    their evidence stays readable.
    """
    state = get_state()
    job = await state.job_store.load(job_id)
    if job is None:
        return _error("invalid_job_id", "unknown job id", field="job_id")

    if job.state in ("succeeded", "failed", "cancelled", "expired"):
        return {
            "job_id": job.job_id,
            "state": job.state,
            "note": "job already finished; completed evidence remains readable",
        }

    job.cancel_requested = True
    job.state = "cancelled"
    await state.job_store.save(job)
    return {
        "job_id": job.job_id,
        "state": "cancelled",
        "note": "best-effort cancellation requested; undispatched queries are stopped, "
        "in-flight engine calls were not interrupted",
    }


def _job_summary(job: ResearchJob) -> dict[str, Any]:
    """Compact job view for tool responses."""
    completed, total = job.progress
    return {
        "job_id": job.job_id,
        "state": job.state,
        "question": job.question,
        "strategy": job.strategy,
        "progress": {"completed": completed, "total": total},
        "queries": [
            {
                "index": query.index,
                "query": query.query,
                "intent": query.intent,
                "state": query.state,
                "result_count": query.result_count,
                "query_id": query.query_id,
                "cursor": query.cursor,
                "error": query.error,
            }
            for query in job.queries
        ],
        "warnings": job.warnings,
    }
