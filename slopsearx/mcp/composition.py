"""Resolve tenant-scoped artifacts before admitting composed workflows."""

from __future__ import annotations

import time
from dataclasses import dataclass, field
from typing import Any

from slopsearx.artifacts import parse_artifact_ref, parse_composite_artifact_id
from slopsearx.mcp.entity_projection import entity_groups
from slopsearx.mcp.state import current_tenant, get_state

MAX_SOURCE_RESULTS = 25

TRANSITIONS = {
    "research": frozenset({"snapshot", "saved_report", "staged_search"}),
    "dependency_dossier": frozenset({"snapshot", "entity_group", "result"}),
    "staged_search": frozenset({"saved_report"}),
    "research_manifest": frozenset({"staged_search", "research_attempt", "research_job"}),
}


@dataclass(slots=True)
class ResolvedSource:
    """A bounded immutable description used by a destination workflow."""

    artifact: dict[str, Any]
    status: str = "live"
    result_ids: list[str] = field(default_factory=list)
    query: str | None = None
    details: dict[str, Any] = field(default_factory=dict)

    def stored(self) -> dict[str, Any]:
        """Return the JSON-safe source descriptor persisted by a destination."""
        return {
            "artifact": self.artifact,
            "status": self.status,
            "result_ids": list(self.result_ids),
            "query": self.query,
            "details": dict(self.details),
        }


def _error(code: str, message: str, **extra: Any) -> dict[str, Any]:
    return {"error": {"code": code, "message": message, **extra}}


def _policy_error(engines: list[str]) -> dict[str, Any] | None:
    from slopsearx.mcp import tools as core

    rejected = core._enforce_policy(get_state(), engines)
    if rejected:
        return _error("source_policy_denied", "current policy denies the source artifact")
    return None


def _source_error(read: Any, *, missing: str = "source artifact was not found") -> dict[str, Any] | None:
    if read.unavailable:
        return _error("source_unavailable", "source artifact store is unavailable")
    if read.expired:
        return _error("source_expired", "source artifact has expired", expires_at=read.expires_at)
    if read.snapshot is None:
        return _error("source_not_found", missing)
    return None


async def _snapshot(ref: dict[str, Any]) -> ResolvedSource | dict[str, Any]:
    state = get_state()
    read = await state.snapshots.for_tenant(current_tenant()).read(ref["id"])
    if error := _source_error(read):
        return error
    snapshot = read.snapshot
    assert snapshot is not None
    if error := _policy_error(snapshot.scope.selected_engines):
        return error
    total = len(snapshot.results)
    kept = min(total, MAX_SOURCE_RESULTS)
    return ResolvedSource(
        artifact=ref,
        status="truncated" if total > kept else "live",
        result_ids=[state.snapshots.result_id(snapshot.snapshot_id, index) for index in range(kept)],
        query=snapshot.query,
        details={"query_id": snapshot.query_id, "result_count": total, "included_result_count": kept},
    )


async def _result(ref: dict[str, Any]) -> ResolvedSource | dict[str, Any]:
    try:
        snapshot_id, raw_index = ref["id"].rsplit(":", 1)
        index = int(raw_index)
        if index < 0 or str(index) != raw_index:
            raise ValueError
    except (AttributeError, ValueError):
        return _error("source_not_found", "source result was not found")
    parent = await _snapshot({**ref, "kind": "snapshot", "id": snapshot_id})
    if isinstance(parent, dict):
        return parent
    state = get_state()
    read = await state.snapshots.for_tenant(current_tenant()).read(snapshot_id)
    assert read.snapshot is not None
    if index >= len(read.snapshot.results):
        return _error("source_not_found", "source result was not found")
    return ResolvedSource(
        artifact=ref,
        result_ids=[ref["id"]],
        query=read.snapshot.query,
        details={"query_id": read.snapshot.query_id, "result_index": index},
    )


async def _entity_group(ref: dict[str, Any]) -> ResolvedSource | dict[str, Any]:
    try:
        snapshot_id, entity_id = parse_composite_artifact_id(ref["id"], 2)
    except ValueError:
        return _error("source_not_found", "source entity group was not found")
    parent = await _snapshot({**ref, "kind": "snapshot", "id": snapshot_id})
    if isinstance(parent, dict):
        return parent
    read = await get_state().snapshots.for_tenant(current_tenant()).read(snapshot_id)
    assert read.snapshot is not None
    group = next((item for item in entity_groups(read.snapshot) if item.get("entity_id") == entity_id), None)
    if group is None:
        return _error("source_not_found", "source entity group was not found")
    result_ids = [str(item) for item in group["result_ids"]]
    kept = result_ids[:MAX_SOURCE_RESULTS]
    return ResolvedSource(
        artifact=ref,
        status="truncated" if len(result_ids) > len(kept) else "live",
        result_ids=kept,
        query=read.snapshot.query,
        details={"entity_id": entity_id, "member_count": len(result_ids), "included_result_count": len(kept)},
    )


async def _saved_report(ref: dict[str, Any]) -> ResolvedSource | dict[str, Any]:
    state = get_state()
    if not state.policy.tool_enabled("saved_searches"):
        return _error("source_policy_denied", "saved-search access is disabled by current policy")
    if state.saved_store is None:
        return _error("source_unavailable", "saved-search store is unavailable")
    try:
        search_id, run_id = parse_composite_artifact_id(ref["id"], 2)
    except ValueError:
        return _error("source_not_found", "source saved-search report was not found")
    store = state.saved_store.for_tenant(current_tenant())
    if not store.available:
        return _error("source_unavailable", "saved-search store is unavailable")
    definition = await store.load(search_id)
    if definition is None:
        return _error("source_not_found", "source saved-search report was not found")
    if error := _policy_error(definition.engines):
        return error
    reports = await store.reports(search_id, now=time.time(), limit=state.policy.saved_max_reports)
    report = next((item for item in reports if item.get("run_id") == run_id), None)
    if report is None:
        return _error("source_not_found", "source saved-search report was not found")
    reasons = {str(item) for item in report.get("incomparable_reasons", [])}
    if reasons & {"observation_truncated", "report_size_exceeded"}:
        return _error("source_truncated", "source saved-search report is truncated", reasons=sorted(reasons))
    if report.get("status") == "incomparable" or reasons:
        return _error("source_incomparable", "source saved-search report is incomparable", reasons=sorted(reasons))
    window = report.get("observation", {}).get("window", {})
    report_query = window.get("query")
    report_engines = window.get("engines")
    if (
        not isinstance(report_query, str)
        or not report_query.strip()
        or not isinstance(report_engines, list)
        or not report_engines
    ):
        return _error("source_incomparable", "source saved-search report lacks a complete observation window")
    if error := _policy_error([str(engine) for engine in report_engines]):
        return error
    return ResolvedSource(
        artifact=ref,
        query=report_query,
        details={
            "search_id": search_id,
            "run_id": run_id,
            "report_status": report.get("status"),
            "revision": report.get("revision"),
        },
    )


async def _staged(ref: dict[str, Any]) -> ResolvedSource | dict[str, Any]:
    state = get_state()
    if not state.policy.tool_enabled("staged_search"):
        return _error("source_policy_denied", "staged-search access is disabled by current policy")
    if state.staged_store is None:
        return _error("source_unavailable", "staged-search store is unavailable")
    read = await state.staged_store.read(current_tenant(), ref["id"])
    if read.unavailable:
        return _error("source_unavailable", "staged-search store is unavailable")
    if read.expired:
        return _error("source_expired", "source staged search has expired", expires_at=read.expires_at)
    if read.record is None:
        return _error("source_not_found", "source staged search was not found")
    from slopsearx.mcp.staged_tools import _policy_check

    if _policy_check(read.record):
        return _error("source_policy_denied", "current policy denies the source staged search")
    attempt_id = read.record.get("selected_result_attempt_id")
    attempt = next(
        (
            item
            for stage in read.record.get("stages", [])
            for item in stage.get("attempts", [])
            if item.get("attempt_id") == attempt_id
        ),
        None,
    )
    if not attempt or not attempt.get("cursor"):
        return _error("source_partial", "source staged search has no selected result snapshot")
    selected = await _snapshot({**ref, "kind": "snapshot", "id": str(attempt["cursor"])})
    if isinstance(selected, dict):
        return selected
    return ResolvedSource(
        artifact=ref,
        status=selected.status,
        result_ids=selected.result_ids,
        query=selected.query,
        details={
            "operation_id": ref["id"],
            "selected_attempt_id": attempt_id,
            "selected_snapshot": attempt["cursor"],
            **selected.details,
        },
    )


async def _research(ref: dict[str, Any], *, attempt_only: bool) -> ResolvedSource | dict[str, Any]:
    state = get_state()
    if not state.policy.tool_enabled("research"):
        return _error("source_policy_denied", "research access is disabled by current policy")
    if not state.job_store.available:
        return _error("source_unavailable", "research job store is unavailable")
    attempt_id = None
    if attempt_only:
        try:
            job_id, attempt_id = parse_composite_artifact_id(ref["id"], 2)
        except ValueError:
            return _error("source_not_found", "source research attempt was not found")
    else:
        job_id = ref["id"]
    job = await state.job_store.for_tenant(current_tenant()).load(job_id)
    if job is None:
        return _error("source_not_found", "source research artifact was not found")
    if job.workflow.get("kind") == "dependency_dossier":
        return _error("source_not_found", "source research artifact was not found")
    if error := _policy_error([engine for query in job.queries for engine in query.engines]):
        return error
    attempts = [item for query in job.queries for item in query.attempts]
    if attempt_only:
        attempts = [item for item in attempts if item.attempt_id == attempt_id]
        if not attempts:
            return _error("source_not_found", "source research attempt was not found")
    cursors = [item.cursor for item in attempts if item.cursor]
    if not cursors:
        return _error("source_partial", "source research artifact has no captured result snapshot")
    result_ids: list[str] = []
    total = 0
    for cursor in cursors:
        selected = await _snapshot({**ref, "kind": "snapshot", "id": str(cursor)})
        if isinstance(selected, dict):
            return selected
        total += int(selected.details["result_count"])
        result_ids.extend(selected.result_ids)
    unique = list(dict.fromkeys(result_ids))[:MAX_SOURCE_RESULTS]
    return ResolvedSource(
        artifact=ref,
        status="truncated" if total > len(unique) else ("live" if job.state == "succeeded" else "partial"),
        result_ids=unique,
        query=job.question,
        details={"job_id": job_id, "job_state": job.state, "result_count": total, "included_result_count": len(unique)},
    )


async def resolve_source(source: Any, destination: str) -> ResolvedSource | dict[str, Any]:
    """Validate and resolve a supported source without persistence or dispatch."""
    try:
        ref = parse_artifact_ref(source)
    except ValueError as exc:
        return _error("unsupported_source_contract", str(exc), field="source")
    allowed = TRANSITIONS.get(destination)
    if allowed is None or ref["kind"] not in allowed:
        return _error(
            "unsupported_transition",
            f"{ref['kind']} cannot be used as a source for {destination}",
            source_kind=ref["kind"],
            destination=destination,
        )
    handlers = {
        "snapshot": _snapshot,
        "result": _result,
        "entity_group": _entity_group,
        "saved_report": _saved_report,
        "staged_search": _staged,
    }
    if ref["kind"] == "research_attempt":
        resolved = await _research(ref, attempt_only=True)
    elif ref["kind"] == "research_job":
        resolved = await _research(ref, attempt_only=False)
    else:
        resolved = await handlers[ref["kind"]](ref)
    if isinstance(resolved, ResolvedSource) and destination == "research" and resolved.status == "truncated":
        return _error(
            "source_truncated",
            "source artifact exceeds the research composition evidence bound",
            included_result_count=len(resolved.result_ids),
        )
    return resolved
