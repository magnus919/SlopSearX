"""Read-only plan previews over the existing research execution contract."""

from __future__ import annotations

import hashlib
import json
import time
from typing import Any

from pydantic import StrictInt

from slopsearx.mcp import tools as t
from slopsearx.mcp.state import current_tenant, get_state
from slopsearx.research_budget import budget_summary, initialize_budget
from slopsearx.research_models import ResearchMutationError, ResearchQuery
from slopsearx.research_planning import query_identity, validate_variant


def _entry(query: ResearchQuery) -> dict[str, Any]:
    """Only caller-writable execution fields; server evidence scope stays private to validation."""
    result: dict[str, Any] = {
        "query": query.query,
        "intent": query.intent,
        "engines": list(query.engines),
        "planning_method": query.planning_method,
    }
    for name in ("subquestion_id", "rationale", "parent_attempt_id"):
        value = getattr(query, name)
        if value is not None:
            result[name] = value
    if query.evidence_result_ids:
        result["evidence_result_ids"] = list(query.evidence_result_ids)
    return result


def _limits(
    queries: int | None, engines: int | None, attempts: int | None, engine_attempts: int | None, results: int | None
) -> dict[str, int]:
    policy = get_state().policy
    limits = {
        "queries": t._research_limit(queries, policy.job_max_queries, "max_queries"),
        "engines_per_query": t._research_limit(engines, policy.job_max_engines_per_query, "max_engines_per_query"),
        "attempts": t._research_limit(attempts, policy.job_max_queries, "max_attempts"),
        "engine_attempts": t._research_limit(
            engine_attempts, policy.job_max_queries * policy.job_max_engines_per_query, "max_engine_attempts"
        ),
        "results": t._research_limit(results, policy.job_max_results, "max_results"),
    }
    limits["engine_attempts"] = min(limits["engine_attempts"], limits["attempts"] * limits["engines_per_query"])
    return limits


def _initial_preview(
    question: str,
    entries: list[dict[str, Any]],
    subquestions: list[dict[str, str]],
    limits: dict[str, int],
) -> dict[str, Any]:
    state = get_state()
    if not isinstance(question, str) or t._validate_query(question, state):
        raise ResearchMutationError("invalid_input", "question must be a nonempty bounded string")
    if len(entries) > min(limits["queries"], limits["attempts"]):
        raise ResearchMutationError("job_budget_exceeded", "plan exceeds query or attempt budget; no entries dropped")
    queries = [t._prepare_research_query(state, entry, limits["engines_per_query"]) for entry in entries]
    if sum(len(query.engines) for query in queries) > limits["engine_attempts"]:
        raise ResearchMutationError("job_budget_exceeded", "plan exceeds engine-attempt budget; no entries dropped")
    identities = [query_identity(query.query, query.engines) for query in queries]
    if len(set(identities)) != len(identities):
        raise ResearchMutationError("duplicate_query", "plan repeats the same query and source scope")
    for query in queries:
        if query.planning_method == "terminology_expansion":
            validate_variant(question, query.query)
    arguments = {
        "question": question.strip(),
        "initial_plan": [_entry(query) for query in queries],
        "subquestions": subquestions,
        "max_queries": limits["queries"],
        "max_attempts": limits["attempts"],
        "max_engines_per_query": limits["engines_per_query"],
        "max_engine_attempts": limits["engine_attempts"],
        "max_results": limits["results"],
    }
    return {
        "contract": "slopsearx.research.plan",
        "version": 1,
        "execution": {"tool": "slopsearx_start_research", "arguments": arguments},
        "plan_digest": hashlib.sha256(json.dumps(arguments, sort_keys=True).encode()).hexdigest(),
        "budget": {
            "limits": limits,
            "planned_queries": len(queries),
            "planned_engine_attempts": sum(len(q.engines) for q in queries),
        },
        "dispatched": False,
        "note": (
            "Caller-proposed reading-lead searches; execution revalidates policy and budgets. "
            "No evidence sufficiency is certified."
        ),
    }


async def slopsearx_plan_research(
    question: str,
    subquestions: list[dict[str, Any]],
    intent: str = "web",
    engines: list[str] | None = None,
    max_queries: StrictInt | None = None,
    max_engines_per_query: StrictInt | None = None,
    max_attempts: StrictInt | None = None,
    max_engine_attempts: StrictInt | None = None,
    max_results: StrictInt | None = None,
) -> dict[str, Any]:
    """Preview an original-plus-facets research plan without searching or storing it.

    Supply distinct evidence needs as id/question pairs, optionally query/intent/engines.
    Returns start_research arguments; the caller owns decomposition and sufficiency.
    """
    state = get_state()
    if not state.policy.tool_enabled("research"):
        return t._error("tool_disabled", "query planning requires MCP_GRANT_RESEARCH=1")
    try:
        limits = _limits(max_queries, max_engines_per_query, max_attempts, max_engine_attempts, max_results)
        if not isinstance(subquestions, list) or not subquestions or len(subquestions) >= limits["queries"]:
            raise ResearchMutationError("invalid_input", "subquestions must leave a query slot for the original")
        declared: list[dict[str, str]] = []
        identities: set[str] = set()
        entries: list[dict[str, Any]] = [
            {"query": question, "intent": intent, "engines": engines, "planning_method": "original"}
        ]
        for item in subquestions:
            if not isinstance(item, dict) or set(item) - {"id", "question", "query", "intent", "engines"}:
                raise ResearchMutationError("invalid_input", "invalid subquestion fields")
            identity = t._research_metadata(item.get("id"), "subquestion id", 128)
            text = t._research_metadata(item.get("question"), "subquestion question", state.policy.max_query_length)
            if identity is None or text is None or identity in identities:
                raise ResearchMutationError("invalid_input", "subquestion ids must be nonempty and unique")
            identities.add(identity)
            declared.append({"id": identity, "question": text})
            entries.append(
                {
                    "query": item.get("query", text),
                    "intent": item.get("intent", intent),
                    "engines": item.get("engines", engines),
                    "subquestion_id": identity,
                    "rationale": "Caller-decomposed evidence need",
                    "planning_method": "decomposition",
                }
            )
        return _initial_preview(question, entries, declared, limits)
    except (ResearchMutationError, ValueError) as exc:
        return t._error(getattr(exc, "code", "invalid_input"), str(exc))


async def slopsearx_plan_query_variants(
    question: str,
    variants: list[str],
    intent: str = "web",
    engines: list[str] | None = None,
    max_queries: StrictInt | None = None,
    max_engines_per_query: StrictInt | None = None,
    max_attempts: StrictInt | None = None,
    max_engine_attempts: StrictInt | None = None,
    max_results: StrictInt | None = None,
) -> dict[str, Any]:
    """Preview bounded caller-authored terminology variants, retaining the original.

    Variants must preserve exact recognizable identifiers. No model runs, factual
    claims, automatic default expansion or search-quality guarantees are added.
    """
    state = get_state()
    if not state.policy.tool_enabled("research"):
        return t._error("tool_disabled", "query planning requires MCP_GRANT_RESEARCH=1")
    try:
        limits = _limits(max_queries, max_engines_per_query, max_attempts, max_engine_attempts, max_results)
        if not isinstance(variants, list) or not variants or len(variants) >= limits["queries"]:
            raise ResearchMutationError("invalid_input", "variants must leave a query slot for the original")
        entries = [{"query": question, "intent": intent, "engines": engines, "planning_method": "original"}]
        entries.extend(
            {
                "query": variant,
                "intent": intent,
                "engines": engines,
                "planning_method": "terminology_expansion",
                "rationale": "Caller terminology proposal; generated wording is not evidence",
            }
            for variant in variants
        )
        return _initial_preview(question, entries, [], limits)
    except (ResearchMutationError, ValueError) as exc:
        return t._error(getattr(exc, "code", "invalid_input"), str(exc))


async def slopsearx_plan_research_followup(
    job_id: str,
    query: str,
    parent_attempt_id: str,
    evidence_result_ids: list[str],
    rationale: str,
    subquestion_id: str | None = None,
    intent: str = "web",
    engines: list[str] | None = None,
) -> dict[str, Any]:
    """Preview an evidence-linked follow-up using admitted results from this job.

    Reads permitted unexpired snapshots; never appends, executes, reserves a budget
    or marks a subquestion resolved. Returned arguments are revalidated by extend.
    """
    state = get_state()
    if not state.policy.tool_enabled("research"):
        return t._error("tool_disabled", "query planning requires MCP_GRANT_RESEARCH=1")
    store = state.job_store.for_tenant(current_tenant())
    if not store.available:
        return t._error("store_unavailable", "research job store is unavailable")
    job = await store.load(job_id)
    if job is None:
        return t._error("invalid_job_id", "unknown job id")
    # Gate before any retained scope, parent or result disclosure.
    if rejection := t._enforce_policy(state, list({e for q in job.queries for e in q.engines})):
        return rejection
    if job.workflow:
        if rejection := t._research_workflow_policy_error(state, job):
            return rejection
        return t._error("invalid_input", "workflow research jobs cannot be extended")
    if job.state in {"queued", "running", "cancelled", "expired"} or job.caller_completed:
        return t._error("invalid_job_state", "job must be idle and open to continuations")
    if time.time() >= job.deadline:
        return t._error("deadline_exceeded", "job deadline has passed")
    try:
        initialize_budget(job, state.policy)  # loaded copy only; never saved by a preview
        proposed = t._prepare_research_query(
            state,
            {
                "query": query,
                "intent": intent,
                "engines": engines,
                "parent_attempt_id": parent_attempt_id,
                "evidence_result_ids": evidence_result_ids,
                "rationale": rationale,
                "subquestion_id": subquestion_id,
                "planning_method": "evidence_followup",
            },
            job.budget_limits["engines_per_query"],
        )
        t._validate_research_associations(job, proposed, state)
        if any(
            query_identity(proposed.query, proposed.engines) == query_identity(q.query, q.engines) for q in job.queries
        ):
            raise ResearchMutationError("duplicate_query", "query already searched or planned in this scope")
        remaining = budget_summary(job)["remaining"]
        if any(
            remaining[key] < amount
            for key, amount in (
                ("queries", 1),
                ("attempts", 1),
                ("engine_attempts", len(proposed.engines)),
                ("results", 1),
            )
        ):
            raise ResearchMutationError("job_budget_exceeded", "job has insufficient remaining budget")
        evidence = []
        for result_id in proposed.evidence_result_ids:
            record = await t.slopsearx_read_result(result_id)
            if "error" in record:
                return record
            sources = record.get("source_engines", [])
            if not isinstance(sources, list) or not sources or any(not isinstance(name, str) for name in sources):
                return t._error("invalid_result_id", "captured evidence has invalid source provenance")
            if rejection := t._enforce_policy(state, sources):
                return rejection
            if not set(sources).issubset(proposed.evidence_engines):
                return t._error("invalid_result_id", "captured evidence does not match its parent source scope")
            evidence.append({"result_id": result_id, "title": record.get("title"), "citation": record.get("citation")})
        arguments = {"job_id": job_id, **_entry(proposed)}
        return {
            "contract": "slopsearx.research.followup_plan",
            "version": 1,
            "execution": {"tool": "slopsearx_extend_research", "arguments": arguments},
            "evidence": evidence,
            "budget": budget_summary(job),
            "dispatched": False,
            "note": "Evidence links attribute the caller's proposal; snippets are untrusted leads, not verified facts.",
        }
    except (ResearchMutationError, ValueError) as exc:
        return t._error(getattr(exc, "code", "invalid_input"), str(exc))
