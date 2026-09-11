"""MCP tools for bounded, two-stage search operations."""

from __future__ import annotations

import dataclasses
import time
import uuid
from typing import Any

from slopsearx import metrics as m
from slopsearx.artifacts import artifact_ref
from slopsearx.capabilities import INTENT_PROFILES
from slopsearx.mcp import tools as core
from slopsearx.mcp.result_serialization import _result_to_dict
from slopsearx.mcp.state import current_tenant, get_state
from slopsearx.service import ScopeResolver, SearchRequest
from slopsearx.staged import CONTRACT, RETENTION_SECONDS, VERSION, plan_digest

_SCOPE_KEYS = {"engines", "intent"}
_FILTER_KEYS = {"language", "time_range", "safesearch", "freshness"}
_INCLUDE = {"results", "suggestions", "engine_status", "diagnostics", "payload"}
_TIME_RANGES = {None, "day", "week", "month", "year"}


def _error(code: str, message: str, *, field: str | None = None, **extra: Any) -> dict[str, Any]:
    return core._error(code, message, field=field, **extra)


def _scope_decision(
    state: Any, query: str, requested: dict[str, Any]
) -> tuple[dict[str, Any] | None, dict[str, Any] | None]:
    if not isinstance(requested, dict) or set(requested) - _SCOPE_KEYS or len(requested) != 1:
        return None, _error("invalid_input", "scope must contain exactly one of engines or intent", field="scope")
    engines: list[str] | None = None
    categories: list[str] | None = None
    intent = "auto"
    if "engines" in requested:
        raw = requested["engines"]
        if not isinstance(raw, list) or not raw or len(raw) > 64 or not all(isinstance(v, str) and v for v in raw):
            return None, _error(
                "invalid_input", "engines must be a nonempty list of at most 64 names", field="scope.engines"
            )
        if len(set(raw)) != len(raw):
            return None, _error("invalid_input", "duplicate engine names are not allowed", field="scope.engines")
        engines = list(raw)
        validation = core._validate_engines(state, engines)
        if validation:
            return None, validation
    else:
        intent_value = requested.get("intent")
        intent = intent_value if isinstance(intent_value, str) else ""
        if intent not in INTENT_PROFILES or intent in {"media", "images", "videos"}:
            return None, _error("invalid_input", "intent must be a supported text-search intent", field="scope.intent")
        resolved, engines, media, _, _ = core._resolve_scope(state, intent, None, None)
        if isinstance(resolved, dict):
            return None, resolved
        categories = resolved
        if media is not None:
            return None, _error("invalid_input", "media intents are unsupported", field="scope.intent")
    resolver = ScopeResolver(
        active_engines=state.ctx.active_engines,
        router=state.ctx.router,
        tier1_engines=state.ctx.tier1_engines,
        sensitive_engines=state.ctx.sensitive_engines,
        catalog=state.ctx.catalog,
        budget=state.ctx.routing_budget,
    )
    decision = resolver.explain(SearchRequest(query=query, categories=categories, engines=engines))
    if not decision.selected_engines:
        return None, _error("invalid_input", "scope resolves to no active engines", field="scope")
    return {
        "requested": requested,
        "selected_engines": decision.selected_engines,
        "excluded_engines": [dataclasses.asdict(item) for item in decision.excluded_engines],
        "routing": {
            "rule": decision.routing_rule,
            "matched_topic": decision.matched_topic,
            "fallback": decision.routing_fallback,
            "budget_applied": decision.routing_budget_applied,
        },
    }, None


def _build_plan(
    query: str,
    objectives: dict[str, Any],
    initial_scope: dict[str, Any],
    fallback_scope: dict[str, Any] | None,
    allow_scope_expansion: bool,
    filters: dict[str, Any] | None,
) -> tuple[dict[str, Any] | None, dict[str, Any] | None]:
    state = get_state()
    invalid = core._validate_query(query, state)
    if invalid:
        return None, invalid
    if not isinstance(objectives, dict) or set(objectives) != {"deadline_ms", "max_engine_calls"}:
        return None, _error(
            "invalid_input", "objectives requires exactly deadline_ms and max_engine_calls", field="objectives"
        )
    deadline = objectives.get("deadline_ms")
    calls = objectives.get("max_engine_calls")
    if type(deadline) is not int or not 1 <= deadline <= 30000:
        return None, _error(
            "invalid_input", "deadline_ms must be an integer from 1 to 30000", field="objectives.deadline_ms"
        )
    if type(calls) is not int or not 1 <= calls <= 64:
        return None, _error(
            "invalid_input", "max_engine_calls must be an integer from 1 to 64", field="objectives.max_engine_calls"
        )
    effective_deadline = min(deadline, state.policy.staged_max_deadline_ms)
    effective_calls = min(calls, state.policy.staged_max_engine_calls)
    if bool(fallback_scope) != allow_scope_expansion:
        return None, _error(
            "invalid_input",
            "fallback_scope and allow_scope_expansion=true must be supplied together",
            field="fallback_scope",
        )
    normalized_filters = {"language": "en", "time_range": None, "safesearch": "off", "freshness": "no_preference"}
    if filters is not None:
        if not isinstance(filters, dict) or set(filters) - _FILTER_KEYS:
            return None, _error("invalid_input", "filters contains unsupported fields", field="filters")
        normalized_filters.update(filters)
    if normalized_filters["safesearch"] not in core.VALID_SAFESEARCH:
        return None, _error("invalid_input", "invalid safesearch value", field="filters.safesearch")
    if normalized_filters["freshness"] not in core.VALID_FRESHNESS:
        return None, _error("invalid_input", "invalid freshness value", field="filters.freshness")
    if normalized_filters["time_range"] not in _TIME_RANGES:
        return None, _error("invalid_input", "invalid time_range value", field="filters.time_range")
    initial, error = _scope_decision(state, query.strip(), initial_scope)
    if error:
        return None, error
    fallback = None
    if fallback_scope is not None:
        fallback, error = _scope_decision(state, query.strip(), fallback_scope)
        if error:
            return None, error
    scopes: list[dict[str, Any]] = [scope for scope in (initial, fallback) if scope is not None]
    selected = [name for scope in scopes for name in scope["selected_engines"]]
    if len(set(selected)) != len(selected):
        return None, _error(
            "objective_conflict", "initial and fallback engine scopes must be disjoint", field="fallback_scope"
        )
    if len(selected) > effective_calls:
        return None, _error(
            "objective_conflict", "resolved scopes exceed max_engine_calls", field="objectives.max_engine_calls"
        )
    rejection = core._enforce_policy(state, selected)
    if rejection:
        rejection["error"]["code"] = "policy_rejected"
        return None, rejection
    if normalized_filters["safesearch"] == "strict" and any(
        not core._strict_safesearch_satisfiable(state, scope["selected_engines"]) for scope in scopes
    ):
        return None, _error(
            "policy_rejected",
            "strict SafeSearch cannot be enforced by the full staged scope",
            field="filters.safesearch",
        )
    requested = {"deadline_ms": deadline, "max_engine_calls": calls}
    effective = {"deadline_ms": effective_deadline, "max_engine_calls": effective_calls}
    identity = {
        "version": VERSION,
        "query": query.strip(),
        "objectives": {"requested": requested, "effective": effective},
        "initial_scope": initial,
        "fallback_scope": fallback,
        "allow_scope_expansion": allow_scope_expansion,
        "filters": normalized_filters,
    }
    return {
        "identity": identity,
        "digest": plan_digest(identity),
        "query": query.strip(),
        "filters": normalized_filters,
        "initial_scope": initial,
        "fallback_scope": fallback,
        "allow_scope_expansion": allow_scope_expansion,
        "fallback_when": "clean_empty",
        "requested_objectives": requested,
        "effective_objectives": effective,
    }, None


def _policy_check(record: dict[str, Any]) -> dict[str, Any] | None:
    state = get_state()
    if not state.policy.tool_enabled("staged_search"):
        return _error("tool_disabled", "staged search is disabled", grant="MCP_GRANT_STAGED_SEARCH")
    effective = record["objectives"]["effective"]
    if (
        effective["deadline_ms"] > state.policy.staged_max_deadline_ms
        or effective["max_engine_calls"] > state.policy.staged_max_engine_calls
    ):
        return _error("policy_rejected", "current staged-search operator limits are narrower")
    for stage in record["stages"]:
        requested = stage["scope"]["requested"]
        intent = requested.get("intent") if isinstance(requested, dict) else None
        grant = core.INTENT_GRANTS.get(intent) if isinstance(intent, str) else None
        if grant is not None and not state.policy.tool_enabled(grant):
            return _error("policy_rejected", f"current policy disables the captured {intent} intent")
        if isinstance(intent, str):
            current, error = _scope_decision(state, record["plan"]["query"], requested)
            if error or current is None or current["selected_engines"] != stage["scope"]["selected_engines"]:
                return _error("policy_rejected", "current routing no longer authorizes the captured scope")
    engines = [name for stage in record["stages"] for name in stage["scope"]["selected_engines"]]
    rejection = core._enforce_policy(state, engines)
    if rejection:
        rejection["error"]["code"] = "policy_rejected"
    return rejection


def _validate_view(include: list[str] | None, max_results: int | None) -> tuple[set[str], int] | dict[str, Any]:
    state = get_state()
    if include is not None and (not isinstance(include, list) or any(item not in _INCLUDE for item in include)):
        return _error("invalid_input", "include contains unsupported fields", field="include")
    if max_results is not None and (type(max_results) is not int or max_results < 1):
        return _error("invalid_input", "max_results must be a positive integer", field="max_results")
    return set(include or ["results", "engine_status"]), min(
        max_results or state.policy.max_results, state.policy.max_results
    )


def _replay_equivalent(
    record: dict[str, Any],
    query: str,
    objectives: dict[str, Any],
    initial_scope: dict[str, Any],
    fallback_scope: dict[str, Any] | None,
    allow_scope_expansion: bool,
    filters: dict[str, Any] | None,
) -> bool:
    normalized_filters = {"language": "en", "time_range": None, "safesearch": "off", "freshness": "no_preference"}
    if isinstance(filters, dict):
        normalized_filters.update(filters)
    plan = record["plan"]
    return (
        query.strip() == plan["query"]
        and objectives == record["objectives"]["requested"]
        and initial_scope == plan["initial_scope"]["requested"]
        and fallback_scope == (plan["fallback_scope"]["requested"] if plan["fallback_scope"] else None)
        and allow_scope_expansion == plan["allow_scope_expansion"]
        and normalized_filters == plan["filters"]
    )


async def _render(record: dict[str, Any], include: list[str] | None, max_results: int | None) -> dict[str, Any]:
    view = _validate_view(include, max_results)
    if isinstance(view, dict):
        return view
    include_set, limit = view
    results: list[dict[str, Any]] = []
    meta = {
        "cursor": None,
        "artifact": None,
        "query_id": None,
        "total": 0,
        "returned": 0,
        "has_more": False,
        "selected_result_attempt_id": record.get("selected_result_attempt_id"),
        "result_status": "not_available",
    }
    selected = record.get("selected_result_attempt_id")
    attempt = next((a for s in record["stages"] for a in s["attempts"] if a["attempt_id"] == selected), None)
    if attempt and attempt.get("cursor"):
        read = await get_state().snapshots.for_tenant(current_tenant()).read(attempt["cursor"])
        if read.snapshot is not None:
            snapshot = read.snapshot
            if "results" in include_set:
                results = [
                    _result_to_dict(
                        item,
                        result_id=get_state().snapshots.result_id(attempt["cursor"], i),
                        include_payload="payload" in include_set,
                    )
                    for i, item in enumerate(snapshot.results[:limit])
                ]
            meta.update(
                cursor=attempt["cursor"],
                artifact=artifact_ref("snapshot", attempt["cursor"]),
                query_id=attempt["query_id"],
                total=snapshot.total,
                returned=len(results),
                has_more=snapshot.total > len(results),
                result_status="available",
            )
        elif read.expired:
            meta["result_status"] = "expired_handle"
    elapsed_ms = max(0, int((time.time() - record["accepted_at"]) * 1000))
    meta["elapsed_ms"] = elapsed_ms
    budget = dict(record["budget"])
    budget["remaining"] = max(0, budget["limit"] - budget["reserved"])
    return {
        "contract": CONTRACT,
        "version": VERSION,
        "operation_id": record["operation_id"],
        "artifact": artifact_ref("staged_search", record["operation_id"]),
        "state": record["state"],
        "stop_reason": record.get("stop_reason"),
        "accepted_at": record["accepted_at"],
        "execution_deadline_at": record["execution_deadline_at"],
        "expires_at": record["expires_at"],
        "plan_digest": record["plan_digest"],
        "plan": record["plan"],
        "objectives": record["objectives"],
        "budget": budget,
        "stages": record["stages"],
        "results": results,
        "meta": meta,
    }


async def slopsearx_preview_staged_search(
    query: str,
    objectives: dict[str, Any],
    initial_scope: dict[str, Any],
    fallback_scope: dict[str, Any] | None = None,
    allow_scope_expansion: bool = False,
    filters: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Resolve a staged-search plan without dispatch or persistence."""
    state = get_state()
    if not state.policy.tool_enabled("staged_search"):
        return _error("tool_disabled", "staged search is disabled", grant="MCP_GRANT_STAGED_SEARCH")
    plan, error = _build_plan(query, objectives, initial_scope, fallback_scope, allow_scope_expansion, filters)
    if error:
        return error
    assert plan is not None
    return {
        "contract": CONTRACT,
        "version": VERSION,
        "plan": plan,
        "plan_digest": plan["digest"],
        "objectives": {"requested": plan["requested_objectives"], "effective": plan["effective_objectives"]},
        "dispatch": False,
    }


async def slopsearx_search_staged(
    query: str,
    objectives: dict[str, Any],
    initial_scope: dict[str, Any],
    idempotency_key: str,
    fallback_scope: dict[str, Any] | None = None,
    allow_scope_expansion: bool = False,
    filters: dict[str, Any] | None = None,
    include: list[str] | None = None,
    max_results: int | None = None,
) -> dict[str, Any]:
    """Accept a durable staged search and return immediately."""
    state = get_state()
    if not state.policy.tool_enabled("staged_search"):
        return _error("tool_disabled", "staged search is disabled", grant="MCP_GRANT_STAGED_SEARCH")
    if not isinstance(idempotency_key, str) or not idempotency_key.strip() or len(idempotency_key.strip()) > 128:
        return _error("invalid_input", "idempotency_key must contain 1 to 128 characters", field="idempotency_key")
    if isinstance(_validate_view(include, max_results), dict):
        return _validate_view(include, max_results)  # type: ignore[return-value]
    assert state.staged_store is not None
    assert state.staged_runner is not None
    existing = await state.staged_store.find_by_idempotency(current_tenant(), idempotency_key.strip())
    if existing.unavailable:
        return _error("store_unavailable", "staged search requires connected Valkey")
    if existing.expired:
        return _error("expired_handle", "staged operation expired", expires_at=existing.expires_at)
    if existing.record is not None:
        if not _replay_equivalent(
            existing.record,
            query,
            objectives,
            initial_scope,
            fallback_scope,
            allow_scope_expansion,
            filters,
        ):
            return _error("idempotency_conflict", "idempotency key refers to a different plan")
        rejection = _policy_check(existing.record)
        if rejection:
            return rejection
        return await _render(existing.record, include, max_results)
    plan, error = _build_plan(query, objectives, initial_scope, fallback_scope, allow_scope_expansion, filters)
    if error:
        return error
    assert plan is not None
    now = time.time()
    scopes = [plan["initial_scope"]] + ([plan["fallback_scope"]] if plan["fallback_scope"] else [])
    stages = [
        {
            "index": i,
            "scope": scope,
            "state": "pending",
            "skipped_reason": None,
            "attempts": [],
            "enforcement": core._core_filter_enforcement(
                state,
                scope["selected_engines"],
                language=plan["filters"]["language"],
                time_range=plan["filters"]["time_range"],
                safesearch=plan["filters"]["safesearch"],
            ),
        }
        for i, scope in enumerate(scopes)
    ]
    record = {
        "operation_id": str(uuid.uuid4()),
        "tenant": current_tenant(),
        "version": VERSION,
        "plan_digest": plan["digest"],
        "accepted_at": now,
        "execution_deadline_at": now + plan["effective_objectives"]["deadline_ms"] / 1000,
        "expires_at": now + RETENTION_SECONDS,
        "state": "queued",
        "stop_reason": None,
        "plan": {
            k: plan[k]
            for k in ("query", "filters", "initial_scope", "fallback_scope", "allow_scope_expansion", "fallback_when")
        },
        "objectives": {
            "requested": plan["requested_objectives"],
            "effective": plan["effective_objectives"],
            "unmet": [],
        },
        "budget": {
            "unit": "adapter_search_invocations",
            "limit": plan["effective_objectives"]["max_engine_calls"],
            "reserved": 0,
            "observed": 0,
        },
        "stages": stages,
        "retry_keys": [],
        "selected_result_attempt_id": None,
        "next_stage": 0,
    }
    status, stored = await state.staged_store.admit(current_tenant(), idempotency_key.strip(), plan["digest"], record)
    if status == "unavailable":
        return _error("store_unavailable", "staged search requires connected Valkey")
    if status == "quota":
        m.record_workflow_rejection("staged_search", "capacity")
        return _error("operation_quota_exceeded", "tenant has 32 retained staged operations")
    if status == "conflict":
        m.record_workflow_rejection("staged_search", "idempotency")
        return _error("idempotency_conflict", "idempotency key refers to a different plan")
    if status == "created":
        m.record_workflow_accepted("staged_search", "durable_leased")
        m.transition_workflow("staged_search", None, "queued")
        await state.staged_runner.enqueue(current_tenant(), record["operation_id"])
    return await _render(stored, include, max_results)  # type: ignore[arg-type]


async def slopsearx_get_staged_search(
    operation_id: str, include: list[str] | None = None, max_results: int | None = None
) -> dict[str, Any]:
    """Read a staged operation without dispatch."""
    state = get_state()
    if not state.policy.tool_enabled("staged_search"):
        return _error("tool_disabled", "staged search is disabled", grant="MCP_GRANT_STAGED_SEARCH")
    assert state.staged_store is not None
    read = await state.staged_store.read(current_tenant(), operation_id)
    if read.unavailable:
        return _error("store_unavailable", "staged search store is unavailable")
    if read.expired:
        m.record_workflow_expiry("staged_search", "operation")
        return _error("expired_handle", "staged operation expired", expires_at=read.expires_at)
    if read.record is None:
        return _error("invalid_operation_id", "unknown staged operation")
    rejection = _policy_check(read.record)
    if rejection:
        return rejection
    return await _render(read.record, include, max_results)


async def slopsearx_retry_staged_search(operation_id: str, retry_key: str) -> dict[str, Any]:
    """Retry the latest failed or interrupted stage under its original limits."""
    state = get_state()
    if not state.policy.tool_enabled("staged_search"):
        return _error("tool_disabled", "staged search is disabled", grant="MCP_GRANT_STAGED_SEARCH")
    if not isinstance(retry_key, str) or not retry_key.strip() or len(retry_key.strip()) > 128:
        return _error("invalid_input", "retry_key must contain 1 to 128 characters", field="retry_key")
    assert state.staged_store is not None
    assert state.staged_runner is not None
    read = await state.staged_store.read(current_tenant(), operation_id)
    if read.record is not None:
        rejection = _policy_check(read.record)
        if rejection:
            return rejection
    status, record = await state.staged_store.request_retry(current_tenant(), operation_id, retry_key.strip())
    errors = {
        "unavailable": ("store_unavailable", "staged search store is unavailable"),
        "expired": ("expired_handle", "staged operation expired"),
        "missing": ("invalid_operation_id", "unknown staged operation"),
        "busy": ("operation_busy", "operation already queued or running"),
        "none": ("no_retryable_work", "operation has no retryable stage"),
        "budget": ("budget_exhausted", "remaining budget cannot fund the full stage"),
        "deadline": ("deadline_exceeded", "operation deadline has passed"),
    }
    if status in errors:
        code, message = errors[status]
        if status == "budget":
            m.record_workflow_rejection("staged_search", "budget")
        elif status == "expired":
            m.record_workflow_expiry("staged_search", "operation")
        return _error(code, message)
    if status == "queued":
        m.record_workflow_retry("staged_search")
        m.transition_workflow("staged_search", read.record.get("state") if read.record else None, "queued")
        await state.staged_runner.enqueue(current_tenant(), operation_id)
    return await _render(record, None, None)  # type: ignore[arg-type]
