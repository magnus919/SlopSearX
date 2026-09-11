"""Bounded, tenant-safe lineage reads over existing durable artifacts."""

from __future__ import annotations

import time
from collections import deque
from typing import Any

from pydantic import StrictInt

from slopsearx.artifacts import (
    LINEAGE_CONTRACT,
    LINEAGE_VERSION,
    artifact_ref,
    composite_artifact_id,
    lineage_edge,
    parse_artifact_ref,
    parse_composite_artifact_id,
)
from slopsearx.mcp import tools as core
from slopsearx.mcp.entity_projection import entity_groups
from slopsearx.mcp.state import current_tenant, get_state
from slopsearx.research_store import JOB_RETENTION_SECONDS

_DIRECTIONS = frozenset({"outgoing", "incoming", "both"})
_MAX_DEPTH = 5
_MAX_NODES = 100


def _key(ref: dict[str, Any]) -> tuple[str, str]:
    return str(ref["kind"]), str(ref["id"])


def _edge_key(edge: dict[str, Any]) -> tuple[tuple[str, str], str, tuple[str, str]]:
    return _key(edge["from"]), str(edge["relation"]), _key(edge["to"])


def _validated_edges(values: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Ignore corrupt persisted relationships instead of breaking the graph read."""
    valid: list[dict[str, Any]] = []
    for value in values:
        try:
            valid.append(lineage_edge(value["from"], value["relation"], value["to"]))
        except (KeyError, TypeError, ValueError):
            continue
    return sorted(valid, key=_edge_key)


def _node(
    ref: dict[str, Any],
    status: str,
    *,
    created_at: float | None = None,
    expires_at: float | None = None,
    details: dict[str, Any] | None = None,
) -> dict[str, Any]:
    return {
        "artifact": ref,
        "status": status,
        "created_at": created_at,
        "expires_at": expires_at,
        **({"details": details} if details else {}),
    }


def _policy_node(ref: dict[str, Any]) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    return _node(ref, "policy_denied", details={"reason": "current_policy_denied"}), []


def _error_node(
    ref: dict[str, Any], status: str, expires_at: float | None = None
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    return _node(ref, status, expires_at=expires_at), []


async def _snapshot(ref: dict[str, Any]) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    state = get_state()
    read = await state.snapshots.for_tenant(current_tenant()).read(ref["id"])
    if read.unavailable:
        return _error_node(ref, "unavailable")
    if read.expired:
        return _error_node(ref, "expired", read.expires_at)
    if read.snapshot is None:
        return _error_node(ref, "missing")
    snapshot = read.snapshot
    if core._enforce_policy(state, snapshot.scope.selected_engines):
        return _policy_node(ref)
    edges = _validated_edges(snapshot.lineage)
    for index in range(len(snapshot.results)):
        edges.append(
            lineage_edge(
                artifact_ref("result", state.snapshots.result_id(snapshot.snapshot_id, index)),
                "derived_from",
                ref,
            )
        )
    for group in entity_groups(snapshot):
        if group["entity_id"]:
            edges.append(
                lineage_edge(
                    artifact_ref(
                        "entity_group",
                        composite_artifact_id(snapshot.snapshot_id, str(group["entity_id"])),
                    ),
                    "derived_from",
                    ref,
                )
            )
    return (
        _node(
            ref,
            "live",
            created_at=snapshot.created_at,
            expires_at=snapshot.expires_at,
            details={
                "query_id": snapshot.query_id,
                "result_count": snapshot.total,
                "lineage_status": "available" if snapshot.lineage_available else "lineage_unavailable",
            },
        ),
        sorted(edges, key=_edge_key),
    )


async def _result(ref: dict[str, Any]) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    try:
        snapshot_id, raw_index = ref["id"].rsplit(":", 1)
        index = int(raw_index)
        if index < 0 or str(index) != raw_index:
            raise ValueError
    except (ValueError, AttributeError):
        return _error_node(ref, "missing")
    snapshot_ref = artifact_ref("snapshot", snapshot_id)
    parent, _ = await _snapshot(snapshot_ref)
    if parent["status"] != "live":
        return _error_node(ref, parent["status"], parent.get("expires_at"))
    state = get_state()
    read = await state.snapshots.for_tenant(current_tenant()).read(snapshot_id)
    snapshot = read.snapshot
    if snapshot is None or index >= len(snapshot.results):
        return _error_node(ref, "missing")
    edges = [lineage_edge(ref, "derived_from", snapshot_ref)]
    if state.policy.tool_enabled("retrieval_receipts") and state.receipt_store is not None:
        store = state.receipt_store.for_tenant(current_tenant())
        receipts, _total = await store.read(ref["id"], now=time.time(), limit=20)
        for receipt in receipts:
            receipt_id = receipt.get("receipt_id")
            if isinstance(receipt_id, str) and receipt_id:
                edges.append(
                    lineage_edge(
                        artifact_ref("retrieval_receipt", composite_artifact_id(ref["id"], receipt_id)),
                        "retrieval_of",
                        ref,
                    )
                )
    return _node(
        ref, "live", created_at=snapshot.created_at, expires_at=snapshot.expires_at, details={"index": index}
    ), edges


async def _research_job(ref: dict[str, Any]) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    state = get_state()
    if not state.policy.tool_enabled("research"):
        return _policy_node(ref)
    store = state.job_store.for_tenant(current_tenant())
    if not store.available:
        return _error_node(ref, "unavailable")
    job = await store.load(ref["id"])
    if job is None:
        return _error_node(ref, "missing")
    if core._research_workflow_policy_error(state, job):
        return _policy_node(ref)
    edges = _validated_edges(job.workflow.get("lineage") or [])
    for query in job.queries:
        for attempt in query.attempts:
            attempt_ref = artifact_ref("research_attempt", composite_artifact_id(job.job_id, attempt.attempt_id))
            edges.append(lineage_edge(attempt_ref, "derived_from", ref))
            if attempt.cursor:
                edges.append(lineage_edge(artifact_ref("snapshot", attempt.cursor), "derived_from", attempt_ref))
    expires_at = job.created_at + JOB_RETENTION_SECONDS
    return _node(ref, "live", created_at=job.created_at, expires_at=expires_at, details={"state": job.state}), edges


async def _research_attempt(ref: dict[str, Any]) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    try:
        job_id, attempt_id = parse_composite_artifact_id(ref["id"], 2)
    except ValueError:
        return _error_node(ref, "missing")
    job_ref = artifact_ref("research_job", job_id)
    parent, _ = await _research_job(job_ref)
    if parent["status"] != "live":
        return _error_node(ref, parent["status"], parent.get("expires_at"))
    job = await get_state().job_store.for_tenant(current_tenant()).load(job_id)
    assert job is not None
    attempt = next((item for query in job.queries for item in query.attempts if item.attempt_id == attempt_id), None)
    if attempt is None:
        return _error_node(ref, "missing")
    edges = [lineage_edge(ref, "derived_from", job_ref)]
    if attempt.cursor:
        edges.append(lineage_edge(artifact_ref("snapshot", attempt.cursor), "derived_from", ref))
    return _node(
        ref,
        "live",
        created_at=attempt.attempted_at,
        expires_at=parent.get("expires_at"),
        details={"state": attempt.state},
    ), edges


async def _staged(ref: dict[str, Any]) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    state = get_state()
    if not state.policy.tool_enabled("staged_search") or state.staged_store is None:
        return _policy_node(ref)
    read = await state.staged_store.read(current_tenant(), ref["id"])
    if read.unavailable:
        return _error_node(ref, "unavailable")
    if read.expired:
        return _error_node(ref, "expired", read.expires_at)
    if read.record is None:
        return _error_node(ref, "missing")
    from slopsearx.mcp.staged_tools import _policy_check

    if _policy_check(read.record):
        return _policy_node(ref)
    edges = _validated_edges(read.record.get("lineage") or [])
    for stage in read.record.get("stages", []):
        for attempt in stage.get("attempts", []):
            cursor = attempt.get("cursor")
            if isinstance(cursor, str) and cursor:
                edges.append(lineage_edge(artifact_ref("snapshot", cursor), "derived_from", ref))
                if attempt.get("attempt_id") == read.record.get("selected_result_attempt_id"):
                    edges.append(lineage_edge(ref, "selected", artifact_ref("snapshot", cursor)))
    return _node(
        ref,
        "live",
        created_at=read.record.get("accepted_at"),
        expires_at=read.record.get("expires_at"),
        details={"state": read.record.get("state")},
    ), edges


async def _saved_search(ref: dict[str, Any]) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    state = get_state()
    if not state.policy.tool_enabled("saved_searches") or state.saved_store is None:
        return _policy_node(ref)
    store = state.saved_store.for_tenant(current_tenant())
    if not store.available:
        return _error_node(ref, "unavailable")
    definition = await store.load(ref["id"])
    if definition is None:
        return _error_node(ref, "missing")
    if core._enforce_policy(state, definition.engines):
        return _policy_node(ref)
    reports = await store.reports(
        definition.search_id,
        now=time.time(),
        limit=min(state.policy.saved_max_reports, 100),
    )
    edges = [
        lineage_edge(
            artifact_ref(
                "saved_report",
                composite_artifact_id(definition.search_id, str(report["run_id"])),
            ),
            "derived_from",
            ref,
        )
        for report in reports
        if report.get("run_id")
    ]
    return _node(
        ref,
        "live",
        created_at=definition.created_at,
        expires_at=definition.expires_at,
        details={"paused": definition.paused},
    ), edges


async def _saved_report(ref: dict[str, Any]) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    try:
        search_id, run_id = parse_composite_artifact_id(ref["id"], 2)
    except ValueError:
        return _error_node(ref, "missing")
    parent_ref = artifact_ref("saved_search", search_id)
    parent, _ = await _saved_search(parent_ref)
    if parent["status"] != "live":
        return _error_node(ref, parent["status"], parent.get("expires_at"))
    state = get_state()
    store = state.saved_store.for_tenant(current_tenant())  # type: ignore[union-attr]
    reports = await store.reports(search_id, now=time.time(), limit=min(state.policy.saved_max_reports, 100))
    report = next((item for item in reports if item.get("run_id") == run_id), None)
    if report is None:
        return _error_node(ref, "missing")
    return _node(
        ref,
        "live",
        created_at=report.get("finished_at"),
        expires_at=report.get("expires_at"),
        details={"status": report.get("status")},
    ), [lineage_edge(ref, "derived_from", parent_ref)]


async def _receipt(ref: dict[str, Any]) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    state = get_state()
    if not state.policy.tool_enabled("retrieval_receipts") or state.receipt_store is None:
        return _policy_node(ref)
    try:
        result_id, receipt_id = parse_composite_artifact_id(ref["id"], 2)
    except ValueError:
        return _error_node(ref, "missing")
    store = state.receipt_store.for_tenant(current_tenant())
    if not store.available:
        return _error_node(ref, "unavailable")
    receipts, _total = await store.read(result_id, now=time.time(), limit=20)
    receipt = next((item for item in receipts if item.get("receipt_id") == receipt_id), None)
    if receipt is None:
        return _error_node(ref, "missing")
    engines = receipt.get("discovery", {}).get("source_engines", [])
    if not isinstance(engines, list) or core._enforce_policy(state, [str(item) for item in engines]):
        return _policy_node(ref)
    return _node(
        ref,
        "live",
        created_at=receipt.get("submitted_at"),
        expires_at=receipt.get("expires_at"),
        details={"status": receipt.get("observation", {}).get("status")},
    ), [lineage_edge(ref, "retrieval_of", artifact_ref("result", result_id))]


async def _dossier(ref: dict[str, Any]) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    state = get_state()
    required = ("dependency_dossier", "research", "security")
    if any(not state.policy.tool_enabled(grant) for grant in required):
        return _policy_node(ref)
    store = state.job_store.for_tenant(current_tenant())
    if not store.available:
        return _error_node(ref, "unavailable")
    job = await store.load(ref["id"])
    if job is None or job.workflow.get("kind") != "dependency_dossier":
        return _error_node(ref, "missing")
    if core._research_workflow_policy_error(state, job):
        return _policy_node(ref)
    edges = _validated_edges(job.workflow.get("lineage") or [])
    for query in job.queries:
        for attempt in query.attempts:
            if attempt.cursor:
                edges.append(lineage_edge(artifact_ref("snapshot", attempt.cursor), "derived_from", ref))
    return _node(
        ref,
        "live",
        created_at=job.created_at,
        expires_at=job.created_at + JOB_RETENTION_SECONDS,
        details={"state": job.state},
    ), edges


async def _entity_group(ref: dict[str, Any]) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    try:
        snapshot_id, entity_id = parse_composite_artifact_id(ref["id"], 2)
    except ValueError:
        return _error_node(ref, "missing")
    snapshot_ref = artifact_ref("snapshot", snapshot_id)
    parent, _ = await _snapshot(snapshot_ref)
    if parent["status"] != "live":
        return _error_node(ref, parent["status"], parent.get("expires_at"))
    read = await get_state().snapshots.for_tenant(current_tenant()).read(snapshot_id)
    assert read.snapshot is not None
    group = next((item for item in entity_groups(read.snapshot) if item.get("entity_id") == entity_id), None)
    if group is None:
        return _error_node(ref, "missing")
    edges = [lineage_edge(ref, "derived_from", snapshot_ref)]
    edges.extend(lineage_edge(ref, "contains", artifact_ref("result", result_id)) for result_id in group["result_ids"])
    return _node(
        ref,
        "live",
        created_at=read.snapshot.created_at,
        expires_at=read.snapshot.expires_at,
        details={"namespace": group["namespace"], "member_count": len(group["result_ids"])},
    ), edges


async def _resolve(ref: dict[str, Any]) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    handlers = {
        "snapshot": _snapshot,
        "result": _result,
        "entity_group": _entity_group,
        "research_job": _research_job,
        "research_attempt": _research_attempt,
        "staged_search": _staged,
        "saved_search": _saved_search,
        "saved_report": _saved_report,
        "retrieval_receipt": _receipt,
        "dependency_dossier": _dossier,
    }
    handler = handlers.get(ref["kind"])
    if handler is None:
        return _error_node(ref, "unavailable")
    return await handler(ref)


async def slopsearx_get_artifact_lineage(
    artifact: dict[str, Any],
    direction: str = "both",
    max_depth: StrictInt = 2,
    max_nodes: StrictInt = 100,
) -> dict[str, Any]:
    """Resolve a bounded lineage graph without dispatch or retention extension."""
    try:
        root = parse_artifact_ref(artifact)
    except ValueError as exc:
        return core._error("invalid_input", str(exc), field="artifact")
    if direction not in _DIRECTIONS:
        return core._error("invalid_input", "direction must be outgoing, incoming, or both", field="direction")
    if type(max_depth) is not int or not 0 <= max_depth <= _MAX_DEPTH:
        return core._error("invalid_input", "max_depth must be an integer from 0 to 5", field="max_depth")
    if type(max_nodes) is not int or not 1 <= max_nodes <= _MAX_NODES:
        return core._error("invalid_input", "max_nodes must be an integer from 1 to 100", field="max_nodes")

    queue: deque[tuple[dict[str, Any], int]] = deque([(root, 0)])
    queued = {_key(root)}
    nodes: list[dict[str, Any]] = []
    edges: list[dict[str, Any]] = []
    edge_keys: set[tuple[tuple[str, str], str, tuple[str, str]]] = set()
    truncated = False
    while queue:
        ref, depth = queue.popleft()
        node, adjacent = await _resolve(ref)
        nodes.append(node)
        if depth >= max_depth:
            if adjacent:
                truncated = True
            continue
        for edge in sorted(adjacent, key=_edge_key):
            candidates: list[dict[str, Any]] = []
            if direction in {"outgoing", "both"} and _key(edge["from"]) == _key(ref):
                candidates.append(edge["to"])
            if direction in {"incoming", "both"} and _key(edge["to"]) == _key(ref):
                candidates.append(edge["from"])
            include_edge = False
            for candidate in candidates:
                candidate_key = _key(candidate)
                if candidate_key in queued:
                    include_edge = True
                    continue
                if len(queued) >= max_nodes:
                    truncated = True
                    continue
                queued.add(candidate_key)
                queue.append((candidate, depth + 1))
                include_edge = True
            edge_id = _edge_key(edge)
            if include_edge and edge_id not in edge_keys:
                edge_keys.add(edge_id)
                edges.append(edge)
    return {
        "contract": LINEAGE_CONTRACT,
        "version": LINEAGE_VERSION,
        "root": root,
        "direction": direction,
        "max_depth": max_depth,
        "max_nodes": max_nodes,
        "nodes": nodes,
        "edges": edges,
        "truncated": truncated,
        "counts": {"nodes": len(nodes), "edges": len(edges)},
    }
