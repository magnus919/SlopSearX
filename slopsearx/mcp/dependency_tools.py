"""Opt-in dependency dossier MCP workflow."""

from __future__ import annotations

import dataclasses
from typing import Any

from slopsearx import metrics as m
from slopsearx.artifacts import artifact_ref
from slopsearx.dependency_dossier import (
    ADVISORY_ENGINE,
    ECOSYSTEM_ENGINES,
    REPOSITORY_ENGINE,
    github_root_candidate,
    resolve_package_results,
    workflow_identity,
)
from slopsearx.mcp import tools as core
from slopsearx.mcp.result_serialization import _result_to_dict
from slopsearx.mcp.state import current_tenant, get_state
from slopsearx.research import ResearchJob, ResearchQuery, generate_job_id

CONTRACT = "slopsearx.dependency_dossier"
VERSION = 1


def _error(code: str, message: str, *, field: str | None = None, **extra: Any) -> dict[str, Any]:
    return core._error(code, message, field=field, **extra)


def _grant_error() -> dict[str, Any] | None:
    policy = get_state().policy
    required = (
        ("dependency_dossier", "MCP_GRANT_DEPENDENCY_DOSSIER"),
        ("research", "MCP_GRANT_RESEARCH"),
        ("security", "MCP_GRANT_SECURITY"),
    )
    missing = [env for grant, env in required if not policy.tool_enabled(grant)]
    return (
        _error("tool_disabled", "dependency dossier requires " + ", ".join(missing), grant=missing) if missing else None
    )


def _workflow_policy_error(job: ResearchJob) -> dict[str, Any] | None:
    if denied := _grant_error():
        return denied
    rejection = core._enforce_policy(get_state(), [engine for query in job.queries for engine in query.engines])
    if rejection:
        rejection["error"]["code"] = "policy_rejected"
    return rejection


def _validate_inputs(
    ecosystem: str, package: str, version: str | None, repository: str | None, deadline: str | None
) -> tuple[dict[str, Any] | None, str | None, dict[str, Any] | None]:
    if ecosystem not in ECOSYSTEM_ENGINES:
        return (
            None,
            None,
            _error(
                "invalid_input",
                "unsupported ecosystem",
                field="ecosystem",
                valid_alternatives=sorted(ECOSYSTEM_ENGINES),
            ),
        )
    if not isinstance(package, str) or not package.strip():
        return None, None, _error("invalid_input", "package is required", field="package")
    if version is not None and (not isinstance(version, str) or not version.strip() or len(version.strip()) > 128):
        return None, None, _error("invalid_input", "version must contain 1 to 128 characters", field="version")
    try:
        identity, digest = workflow_identity(ecosystem, package, version, repository, deadline)
    except ValueError as exc:
        return None, None, _error("invalid_input", str(exc), field="repository" if repository else "package")
    return identity, digest, None


async def slopsearx_start_dependency_dossier(
    ecosystem: str,
    package: str,
    version: str | None = None,
    repository: str | None = None,
    deadline: str | None = None,
    idempotency_key: str | None = None,
) -> dict[str, Any]:
    """Start an attributed package/repository/advisory investigation."""
    state = get_state()
    if denied := _grant_error():
        return denied
    identity, digest, error = _validate_inputs(ecosystem, package, version, repository, deadline)
    if error:
        return error
    assert identity is not None and digest is not None
    if idempotency_key is not None and (
        not isinstance(idempotency_key, str) or not idempotency_key.strip() or len(idempotency_key.strip()) > 128
    ):
        return _error("invalid_input", "idempotency_key must contain 1 to 128 characters", field="idempotency_key")
    tenant = current_tenant()
    store = state.job_store.for_tenant(tenant)
    if not store.available:
        return _error("store_unavailable", "dependency dossiers require connected Valkey")
    normalized = identity["package"]
    queries = [ResearchQuery(index=0, query=normalized, intent="packages", engines=[ECOSYSTEM_ENGINES[ecosystem]])]
    section_queries: dict[str, int | None] = {"package_information": 0, "repository_records": None}
    if identity["repository"]:
        section_queries["repository_records"] = len(queries)
        queries.append(
            ResearchQuery(
                index=len(queries), query=f"repo:{identity['repository']}", intent="code", engines=[REPOSITORY_ENGINE]
            )
        )
    advisory_terms = f"{ecosystem} {normalized}"
    if identity["requested_version"]:
        advisory_terms += f" {identity['requested_version']}"
    section_queries["advisory_leads"] = len(queries)
    queries.append(
        ResearchQuery(index=len(queries), query=advisory_terms, intent="security", engines=[ADVISORY_ENGINE])
    )
    if len(queries) > state.policy.job_max_queries:
        m.record_workflow_rejection("dependency_dossier", "budget")
        return _error("budget_exceeded", "research query budget cannot fund required dossier sections")
    selected = [engine for query in queries for engine in query.engines]
    if rejection := core._enforce_policy(state, selected):
        m.record_workflow_rejection("dependency_dossier", "policy")
        rejection["error"]["code"] = "policy_rejected"
        return rejection
    deadline_ts = core._resolve_deadline(state, deadline)
    if isinstance(deadline_ts, dict):
        return deadline_ts
    job = ResearchJob(
        job_id=generate_job_id(),
        question=f"Dependency dossier for {ecosystem}:{normalized}",
        strategy="dependency_dossier",
        queries=queries,
        deadline=deadline_ts,
        tenant=tenant,
        idempotency_key=idempotency_key.strip() if idempotency_key else None,
        workflow={
            "kind": "dependency_dossier",
            "version": VERSION,
            "identity": identity,
            "identity_digest": digest,
            "section_queries": section_queries,
            "budget": {
                "max_adapter_calls": min(state.policy.job_max_queries, len(queries) * 2),
                "max_results": state.policy.job_max_results,
                "used_adapter_calls": 0,
                "captured_results": 0,
                "admitted_results": {},
            },
        },
    )
    admitted, created = await store.create_idempotent(job)
    if admitted is None:
        return _error("store_unavailable", "dependency dossier admission could not be persisted")
    if not created:
        if admitted.workflow.get("kind") != "dependency_dossier" or admitted.workflow.get("identity_digest") != digest:
            m.record_workflow_rejection("dependency_dossier", "idempotency")
            return _error("idempotency_conflict", "idempotency key refers to a different workflow request")
        if rejection := _workflow_policy_error(admitted):
            return rejection
        return _start_envelope(admitted, replay=True)
    m.record_workflow_accepted("dependency_dossier", "durable_leased")
    m.transition_workflow("dependency_dossier", None, "queued")
    state.runner.enqueue(admitted.job_id, tenant=tenant)
    return _start_envelope(admitted, replay=False)


def _start_envelope(job: ResearchJob, *, replay: bool) -> dict[str, Any]:
    return {
        "contract": CONTRACT,
        "version": VERSION,
        "job_id": job.job_id,
        "artifact": artifact_ref("dependency_dossier", job.job_id),
        "state": job.state,
        "requested_identity": job.workflow["identity"],
        "created_at": job.created_at,
        "deadline": job.deadline,
        "replay": replay,
    }


def _coverage(query: ResearchQuery) -> list[dict[str, Any]]:
    return [dataclasses.asdict(item) for item in query.engine_coverage]


def _error_tokens(query: ResearchQuery) -> list[str]:
    mapping = {"error": "upstream_error", "unavailable": "unavailable"}
    tokens = [mapping.get(item.failure_class or "", item.failure_class) for item in query.engine_coverage]
    return [token for token in tokens if token and token != "ok"]


async def _section(
    job: ResearchJob,
    index: int | None,
    limit: int,
    *,
    repository: str | None = None,
) -> tuple[dict[str, Any], list[Any]]:
    if index is None:
        return {
            "state": "unresolved",
            "error": "not_executed",
            "query": None,
            "query_id": None,
            "cursor": None,
            "executed_at": None,
            "source_published_at": [],
            "coverage": [],
            "results": [],
        }, []
    query = job.queries[index]
    base = {
        "query": query.query,
        "query_id": query.query_id,
        "cursor": query.cursor,
        "executed_at": query.attempts[-1].attempted_at if query.attempts else None,
        "coverage": _coverage(query),
        "errors": _error_tokens(query),
        "results": [],
        "source_published_at": [],
    }
    if query.state in {"pending", "running"}:
        return {**base, "state": "partial", "error": "not_executed"}, []
    if query.state == "failed":
        unavailable = any(item.bucket == "unavailable" for item in query.engine_coverage)
        errors = _error_tokens(query)
        return {
            **base,
            "state": "unavailable" if unavailable else "failed",
            "error": errors[0] if errors else "upstream_error",
        }, []
    if not query.cursor:
        return {**base, "state": "empty" if query.result_count == 0 else "failed", "error": None}, []
    lookup = await get_state().snapshots.for_tenant(current_tenant()).read(query.cursor)
    if lookup.expired:
        return {**base, "state": "expired", "error": "expired_handle"}, []
    if lookup.snapshot is None:
        return {**base, "state": "failed", "error": "not_executed"}, []
    snapshot = lookup.snapshot
    admitted_by_query = (job.workflow.get("budget") or {}).get("admitted_results") or {}
    admitted_count = int(admitted_by_query.get(str(index), len(snapshot.results)))
    indexed_results = list(enumerate(snapshot.results[:admitted_count]))
    if repository is not None:
        indexed_results = [
            (idx, result) for idx, result in indexed_results if github_root_candidate(result.url) == repository
        ]
    cards = [
        _result_to_dict(result, result_id=get_state().snapshots.result_id(query.cursor, idx), include_payload=True)
        for idx, result in indexed_results[:limit]
    ]
    filtered_results = [result for _, result in indexed_results]
    base["results"] = cards
    base["source_published_at"] = [result.published_date for result in filtered_results if result.published_date]
    return {**base, "state": "available" if filtered_results else "empty", "error": None}, filtered_results


async def slopsearx_get_dependency_dossier(job_id: str, max_results: int | None = None) -> dict[str, Any]:
    """Read a stored dependency dossier without dispatching new searches."""
    state = get_state()
    if denied := _grant_error():
        return denied
    if max_results is not None and (type(max_results) is not int or max_results < 1):
        return _error("invalid_input", "max_results must be a positive integer", field="max_results")
    store = state.job_store.for_tenant(current_tenant())
    if not store.available:
        return _error("store_unavailable", "dependency dossier store is unavailable")
    job = await store.load(job_id)
    if job is None or job.workflow.get("kind") != "dependency_dossier":
        return _error("invalid_job_id", "unknown dependency dossier")
    if rejection := _workflow_policy_error(job):
        return rejection
    limit = min(max_results or state.policy.max_results, state.policy.max_results)
    indexes = job.workflow["section_queries"]
    package_section, package_results = await _section(job, indexes["package_information"], limit)
    repository_section, _ = await _section(
        job,
        indexes["repository_records"],
        limit,
        repository=job.workflow["identity"].get("repository"),
    )
    advisory_section, _ = await _section(job, indexes["advisory_leads"], limit)
    identity = job.workflow["identity"]
    resolution = resolve_package_results(identity["ecosystem"], identity["package"], package_results)
    observed_versions = sorted({str(item["version"]) for item in resolution["matches"] if item.get("version")})
    requested_version = identity.get("requested_version")
    version_match = "unknown"
    if requested_version and observed_versions:
        version_match = "exact" if requested_version in observed_versions else "different"
    repository_candidates = resolution.pop("repository_candidates")
    explicit_repository = identity.get("repository")
    if explicit_repository:
        repository_identity = {
            "status": "requested",
            "repository": explicit_repository,
            "basis": "caller_supplied",
            "proven_package_ownership": False,
            "metadata_conflict": bool(repository_candidates and repository_candidates != [explicit_repository]),
            "metadata_candidates": repository_candidates,
        }
    else:
        candidate_status = (
            "unresolved"
            if not repository_candidates
            else "unverified_metadata"
            if len(repository_candidates) == 1
            else "ambiguous"
        )
        repository_identity = {
            "status": candidate_status,
            "repository": repository_candidates[0] if len(repository_candidates) == 1 else None,
            "basis": "attributed_registry_metadata" if repository_candidates else None,
            "proven_package_ownership": False,
            "metadata_candidates": repository_candidates,
        }
    advisory_section["applicability"] = {
        "status": "not_evaluated",
        "requested_version": requested_version,
        "reason": "search leads do not expose attributable affected-version ranges",
    }
    partial = job.state in {"partial", "failed", "cancelled", "expired"} or any(
        section["state"] in {"partial", "failed", "unavailable", "expired"}
        for section in (package_section, repository_section, advisory_section)
    )
    followups = []
    if repository_identity["status"] in {"unresolved", "ambiguous", "unverified_metadata"}:
        followups.append(
            f"Find the canonical repository published by {identity['ecosystem']} for {identity['package']}"
        )
    followups.append(f"Verify affected-version ranges for advisory leads concerning {identity['package']}")
    return {
        "contract": CONTRACT,
        "version": VERSION,
        "job_id": job.job_id,
        "artifact": artifact_ref("dependency_dossier", job.job_id),
        "state": job.state,
        "partial": partial,
        "budget": job.workflow.get("budget", {}),
        "requested_identity": identity,
        "resolved_package_identity": {
            **resolution,
            "requested_version": requested_version,
            "observed_versions": observed_versions,
            "version_match": version_match,
        },
        "repository_identity": repository_identity,
        "sections": {
            "package_information": package_section,
            "repository_records": repository_section,
            "advisory_leads": advisory_section,
        },
        "missing_source_coverage": [
            {"section": name, "state": section["state"], "errors": section.get("errors", [])}
            for name, section in (
                ("package_information", package_section),
                ("repository_records", repository_section),
                ("advisory_leads", advisory_section),
            )
            if section["state"] not in {"available", "empty"}
        ],
        "suggested_followup_searches": followups,
        "limitations": [
            "advisory results are search leads, not confirmed affected-version findings",
            "repository candidates do not prove package ownership",
            "zero advisory leads does not establish absence of vulnerabilities",
        ],
    }
