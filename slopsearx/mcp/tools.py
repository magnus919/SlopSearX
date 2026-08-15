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
    summarize_coverage,
)
from slopsearx.service import (
    QueryValidationError,
    RateLimitExceededError,
    ScopeResolver,
    SearchRequest,
)
from slopsearx.snapshot import SearchSnapshot

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

VALID_INTENTS = tuple(INTENT_PROFILES)
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

# The compact card snippet length (progressive disclosure). Full content is
# a result-record concern; cards carry only the first N chars.
SNIPPET_LENGTH = 300

# Explicit, honest note on every expanded record: SlopSearX surfaces search
# evidence but never fetches or verifies the linked page.
NON_VERIFICATION_NOTE = "SlopSearX did not fetch or verify the linked page"

CONTENT_UNAVAILABLE_NOTE = "full content unavailable (adapter returned snippet only)"

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


# ---------------------------------------------------------------------------
# Structured filter-enforcement report
# ---------------------------------------------------------------------------

ENFORCEMENT_STATUSES: tuple[str, ...] = ("enforced", "partially_enforced", "unsupported", "rejected")


def _enforcement_entry(
    requested: Any,
    status: str,
    reason: str,
    enforced_by: list[str] | None = None,
) -> dict[str, Any]:
    """Build one machine-readable filter-enforcement entry (schema pin).

    Shape: ``{requested, status, reason, enforced_by}`` where ``status``
    is exactly one of the closed ``ENFORCEMENT_STATUSES`` enum.
    """
    return {
        "requested": requested,
        "status": status,
        "reason": reason,
        "enforced_by": list(enforced_by) if enforced_by else [],
    }


def _engine_supports_filter(state: McpState, engine_name: str, filter_name: str) -> bool:
    """Whether an adapter declares support for a filter via ``supported_filters``.

    No adapter currently declares support for language/time_range/safesearch,
    so this defaults to ``False`` (honest "unsupported"). It reads the
    optional class attribute so the report stays consistent with the
    capability catalog if adapters later declare support.
    """
    adapter = state.ctx.active_engines.get(engine_name)
    supported = getattr(adapter, "supported_filters", None) if adapter is not None else None
    if isinstance(supported, dict):
        return bool(supported.get(filter_name))
    return False


def _filter_enforcement(
    state: McpState,
    selected_engines: list[str],
    filter_name: str,
    requested: Any,
) -> dict[str, Any]:
    """One enforcement entry consistent with the selected engines' supported_filters."""
    supporting = [name for name in selected_engines if _engine_supports_filter(state, name, filter_name)]
    if supporting and len(supporting) == len(selected_engines) and selected_engines:
        return _enforcement_entry(
            requested, "enforced", f"{filter_name} is enforced by all selected adapters", supporting
        )
    if supporting:
        return _enforcement_entry(
            requested,
            "partially_enforced",
            f"{filter_name} is enforced by a subset of selected adapters",
            supporting,
        )
    return _enforcement_entry(requested, "unsupported", f"no selected adapter consumes the {filter_name} parameter", [])


def _core_filter_enforcement(
    state: McpState,
    selected_engines: list[str],
    *,
    language: str,
    time_range: str | None,
    safesearch: str,
) -> dict[str, Any]:
    """Structured enforcement report for language/time_range/safesearch.

    Strict SafeSearch fails closed earlier (structured rejection), so it
    never reaches here; moderate SafeSearch is reported ``unsupported``.
    """
    report: dict[str, Any] = {}
    if language and language != "en":
        report["language"] = _filter_enforcement(state, selected_engines, "language", language)
    if time_range:
        report["time_range"] = _filter_enforcement(state, selected_engines, "time_range", time_range)
    if safesearch == "moderate":
        report["safesearch"] = _filter_enforcement(state, selected_engines, "safesearch", safesearch)
    return report


def _source_engines(result: SearchResult) -> list[str]:
    """Sorted, duplicate-free list of contributing engine names.

    Falls back to the primary engine when ``engines`` is empty so the
    cross-engine presence signal is always a non-empty name list.
    """
    return sorted(result.engines) if result.engines else [result.engine]


def _result_to_dict(result: SearchResult, *, result_id: str | None = None) -> dict[str, Any]:
    """Normalize one result into a compact triage card (design §3.1).

    Cards carry triage fields plus a stable server-issued ``result_id``.
    Full ``content``, ``thumbnail``, and ``img_src`` belong to the expanded
    record (progressive disclosure), never the card.
    """
    card = {
        "title": result.title,
        "url": result.url,
        "snippet": (result.content or "")[:SNIPPET_LENGTH],
        "source_engines": _source_engines(result),
        "source_count": len(_source_engines(result)),
        "primary_engine": result.engine,
        "category": result.category,
        "published_at": result.published_date,
        "score": result.score,
        "position": result.position,
        "tier": result.tier,
        "citation": {"label": result.title, "url": result.url},
    }
    if result_id is not None:
        card["result_id"] = result_id
    return card


def _result_record(result: SearchResult, snapshot: SearchSnapshot, result_id: str) -> dict[str, Any]:
    """Build the full result record for ``slopsearx_read_result``.

    Reveals strictly more than the card: complete normalized ``content``,
    media fields, every contributing engine, provenance, a
    ``content_available`` flag, and an explicit non-verification note.
    """
    content = result.content or ""
    source_engines = _source_engines(result)
    content_available = len(content) > SNIPPET_LENGTH
    record = {
        "result_id": result_id,
        "title": result.title,
        "url": result.url,
        "content": content,
        "content_available": content_available,
        "thumbnail": result.thumbnail,
        "img_src": result.img_src,
        "source_engines": source_engines,
        "source_count": len(source_engines),
        "primary_engine": result.engine,
        "category": result.category,
        "published_at": result.published_date,
        "tier": result.tier,
        "position": result.position,
        "score": result.score,
        "citation": {"label": result.title, "url": result.url},
        "provenance": {
            "query": snapshot.query,
            "query_id": snapshot.query_id,
            "rank_explanation": RANKING_EXPLANATION,
            "source_engines": source_engines,
        },
        "snapshot": {
            "cursor": snapshot.snapshot_id,
            "query": snapshot.query,
            "query_id": snapshot.query_id,
            "total": snapshot.total,
        },
        "note": NON_VERIFICATION_NOTE,
    }
    if not content_available:
        record["content_unavailable_note"] = CONTENT_UNAVAILABLE_NOTE
    return record


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
) -> dict[str, Any]:
    """Build the standard search envelope from a SearchResponse.

    Surfaces every piece of evidence the service produced — answers,
    corrections, infoboxes, suggestions, per-engine outcomes, empty engines,
    excluded engines, aggregate count, and pagination signal — so no
    available evidence is silently discarded (envelope recovery).
    """
    excluded_engines = [{"engine": e.engine, "reason": e.reason} for e in response.scope.excluded_engines]
    scope = {
        "requested_intent": requested_intent,
        "resolved_categories": response.scope.resolved_categories,
        "selected_engines": response.scope.selected_engines,
        "routing_reason": response.scope.routing_rule,
        "excluded_engines": excluded_engines,
    }
    if response.all_unresponsive:
        return _error(
            "all_engines_failed",
            "every selected engine failed to respond",
            query_id=response.query_id,
            scope=scope,
            engine_outcomes=[
                {"engine": o.engine, "status": o.status, "result_count": o.result_count, "message": o.message}
                for o in response.engine_outcomes
            ],
            retry_guidance="check the engine outcomes, adjust scope, and retry",
        )
    return {
        "query": response.query,
        "results": [
            _result_to_dict(
                result,
                result_id=(state.snapshots.result_id(cursor, index) if cursor else None),
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
            "ranking": RANKING_EXPLANATION,
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
    # then present the bounded page. ``total`` is the aggregate captured
    # count (meta.total), independent of the max_results page bound.
    total = len(response.results)
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
        total=total,
        enforcement=enforcement,
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

    # Shared policy gate: sensitive engines (and specialist intents) are
    # fail-closed on the generic explicit-engine/profile path too.
    if resolved_engines is not None:
        policy_error = _enforce_policy(state, resolved_engines, field="engines")
        if policy_error:
            return policy_error

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

    # Structured filter-enforcement report. When engines are resolved
    # explicitly we use them; for auto/category routing we conservatively
    # report against the active engines (no adapter supports these filters).
    enforcement_engines = resolved_engines if resolved_engines is not None else sorted(state.ctx.active_engines)
    enforcement = _core_filter_enforcement(
        state, enforcement_engines, language=language, time_range=time_range, safesearch=safesearch
    )

    return await _run_search(
        state,
        request,
        requested_intent,
        warnings,
        include_suggestions="suggestions" in include_set,
        max_results=max_results,
        enforcement=enforcement,
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
    error = _enforce_policy(state, engines, field="engines")
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
    enforcement = _core_filter_enforcement(
        state, engines, language=language, time_range=time_range, safesearch=safesearch
    )
    return await _run_search(
        state,
        request,
        "explicit engine",
        warnings,
        include_suggestions=False,
        max_results=_bounded_max_results(state, max_results),
        enforcement=enforcement,
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
        enforcement["location"] = _enforcement_entry(
            location, "unsupported", "location is not consumed by current adapters"
        )
    if employment_type:
        enforcement["employment_type"] = _enforcement_entry(
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

    warnings = [SCIENCE_LIMITATION_NOTE, f"resolved source_types: {', '.join(source_types)}"]
    if date_from or date_to:
        warnings.append("date_from/date_to are not consumed by current adapters; use time_range in slopsearx_search")

    # Structured filter-enforcement report for science date filters.
    enforcement: dict[str, Any] = {}
    if date_from:
        enforcement["date_from"] = _enforcement_entry(
            date_from,
            "unsupported",
            "date_from is not consumed by current adapters; use time_range in slopsearx_search",
        )
    if date_to:
        enforcement["date_to"] = _enforcement_entry(
            date_to,
            "unsupported",
            "date_to is not consumed by current adapters; use time_range in slopsearx_search",
        )

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
    lookup = await state.snapshots.read(cursor)
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
            "has_more": end < snapshot.total,
            "query_id": snapshot.query_id,
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

    lookup = await state.snapshots.read(snapshot_id)
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
            }
            for query in job.queries
        ],
        "coverage": job_coverage.as_dict(),
        "warnings": job.warnings,
    }
