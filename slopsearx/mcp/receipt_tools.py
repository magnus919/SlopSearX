"""MCP receipt/manifest boundary. Supplied URLs and text are inert data."""

from __future__ import annotations

import datetime as dt
import hashlib
import json
import re
import time
from typing import Any

from pydantic import StrictInt

from slopsearx import metrics as m
from slopsearx.artifacts import (
    artifact_ref,
    composite_artifact_id,
    lineage_edge,
    manifest_artifact_id,
    parse_artifact_ref,
)
from slopsearx.mcp.result_serialization import NON_VERIFICATION_NOTE, _retrieval_handoff
from slopsearx.mcp.state import current_tenant, get_state
from slopsearx.retrieval_receipts import RECEIPT_CONTRACT, RECEIPT_VERSION

MAX_RECEIPT_BYTES = 32 * 1024
MAX_MANIFEST_BYTES = 1024 * 1024
MAX_MANIFEST_RESULTS = 25
_FAILURE_CODE = re.compile(r"^[A-Za-z0-9_.-]{1,64}$")
_SHA256 = re.compile(r"^[0-9a-fA-F]{64}$")


def _error(code: str, message: str, **details: Any) -> dict[str, Any]:
    return {"error": {"code": code, "message": message, **details}}


def _enabled() -> tuple[Any, Any] | dict[str, Any]:
    state = get_state()
    if not state.policy.tool_enabled("retrieval_receipts"):
        return _error("tool_disabled", "retrieval receipts require MCP_GRANT_RETRIEVAL_RECEIPTS=1")
    if state.receipt_store is None or not state.receipt_store.available:
        return _error("store_unavailable", "retrieval receipts require connected Valkey")
    return state, state.receipt_store.for_tenant(current_tenant())


def _bounded(value: str | None, field: str, maximum: int, *, required: bool = False) -> str | None:
    if value is None:
        if required:
            raise ValueError(f"{field} is required")
        return None
    if not isinstance(value, str) or not value or len(value) > maximum:
        raise ValueError(f"{field} must be 1-{maximum} characters")
    return value


def _timestamp(value: str | None, *, required: bool) -> str | None:
    value = _bounded(value, "captured_at", 64, required=required)
    if value is None:
        return None
    try:
        parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        raise ValueError("captured_at must be RFC3339") from exc
    if parsed.tzinfo is None:
        raise ValueError("captured_at must include a timezone")
    return value


def _observation(
    *,
    retriever: str,
    idempotency_key: str,
    status: str,
    captured_at: str | None,
    final_url: str | None,
    content_sha256: str | None,
    capture_ref: str | None,
    passage_refs: list[dict[str, str]] | None,
    failure_code: str | None,
    failure_message: str | None,
) -> tuple[dict[str, Any], str]:
    retriever = _bounded(retriever, "retriever", 128, required=True) or ""
    _bounded(idempotency_key, "idempotency_key", 128, required=True)
    if status not in {"succeeded", "failed"}:
        raise ValueError("status must be succeeded or failed")
    captured_at = _timestamp(captured_at, required=status == "succeeded")
    final_url = _bounded(final_url, "final_url", 2048)
    capture_ref = _bounded(capture_ref, "capture_ref", 2048, required=status == "succeeded")
    if content_sha256 is not None and not _SHA256.fullmatch(content_sha256):
        raise ValueError("content_sha256 must be 64 hexadecimal characters")
    if status == "succeeded" and (failure_code is not None or failure_message is not None):
        raise ValueError("successful receipts reject failure fields")
    if status == "failed" and (not isinstance(failure_code, str) or not _FAILURE_CODE.fullmatch(failure_code)):
        raise ValueError("failed receipts require a valid failure_code")
    failure_message = _bounded(failure_message, "failure_message", 1024)
    passages = passage_refs or []
    if not isinstance(passages, list) or len(passages) > 32:
        raise ValueError("passage_refs must contain at most 32 entries")
    normalized: list[dict[str, str]] = []
    for passage in passages:
        if not isinstance(passage, dict) or not set(passage) <= {"ref", "label"}:
            raise ValueError("each passage reference accepts only ref and label")
        ref = _bounded(passage.get("ref"), "passage_refs.ref", 2048, required=True) or ""
        label = _bounded(passage.get("label"), "passage_refs.label", 256)
        normalized.append({"ref": ref, **({"label": label} if label is not None else {})})
    observation = {
        "status": status,
        "captured_at": captured_at,
        "final_url": final_url,
        "content_hash": {"algorithm": "sha256", "value": content_sha256.lower()} if content_sha256 else None,
        "capture_ref": capture_ref,
        "passage_refs": normalized,
        "failure": {"code": failure_code, "message": failure_message} if status == "failed" else None,
    }
    body = {"retriever": retriever, "observation": observation}
    encoded = json.dumps(body, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
    if len(encoded) > MAX_RECEIPT_BYTES:
        raise ValueError("encoded receipt exceeds 32768 bytes")
    return body, hashlib.sha256(encoded).hexdigest()


async def _source(result_id: str) -> tuple[Any, Any, int] | dict[str, Any]:
    state = get_state()
    try:
        snapshot_id, raw_index = result_id.rsplit(":", 1)
        index = int(raw_index)
        if index < 0 or str(index) != raw_index:
            raise ValueError
    except (ValueError, AttributeError):
        return _error("invalid_result_id", "result_id must be a server-issued snapshot result handle")
    outcome = await state.snapshots.for_tenant(current_tenant()).read(snapshot_id)
    if outcome.unavailable:
        return _error("store_unavailable", "snapshot store is unavailable")
    if outcome.expired:
        return _error("expired_handle", "originating snapshot has expired", expires_at=outcome.expires_at)
    if outcome.snapshot is None or index >= len(outcome.snapshot.results):
        return _error("invalid_result_id", "unknown result_id")
    return outcome.snapshot, outcome.snapshot.results[index], index


def _discovery(snapshot: Any, result: Any, result_id: str) -> dict[str, Any]:
    handoff = _retrieval_handoff(result, snapshot, result_id)
    return {
        "result_id": result_id,
        "title": result.title,
        "original_url": result.url,
        "query_id": snapshot.query_id,
        "snapshot_cursor": snapshot.snapshot_id,
        "query": snapshot.query,
        "source_engines": handoff["provenance"]["source_engines"],
        "handoff": handoff,
        "verified": False,
        "verification_note": NON_VERIFICATION_NOTE,
    }


def _decorate_receipt(result_id: str, receipt: dict[str, Any]) -> dict[str, Any]:
    """Project additive artifact identity without changing retained v1 data."""
    value = dict(receipt)
    value["artifact"] = artifact_ref(
        "retrieval_receipt",
        composite_artifact_id(result_id, str(value["receipt_id"])),
    )
    value["lineage"] = [lineage_edge(value["artifact"], "retrieval_of", artifact_ref("result", result_id))]
    return value


async def slopsearx_submit_retrieval_receipt(
    result_id: str,
    retriever: str,
    idempotency_key: str,
    status: str,
    captured_at: str | None = None,
    final_url: str | None = None,
    content_sha256: str | None = None,
    capture_ref: str | None = None,
    passage_refs: list[dict[str, str]] | None = None,
    failure_code: str | None = None,
    failure_message: str | None = None,
) -> dict[str, Any]:
    """Store one attributed retrieval observation without fetching supplied data."""
    enabled = _enabled()
    if isinstance(enabled, dict):
        return enabled
    _state, store = enabled
    try:
        body, digest = _observation(
            retriever=retriever,
            idempotency_key=idempotency_key,
            status=status,
            captured_at=captured_at,
            final_url=final_url,
            content_sha256=content_sha256,
            capture_ref=capture_ref,
            passage_refs=passage_refs,
            failure_code=failure_code,
            failure_message=failure_message,
        )
    except ValueError as exc:
        return _error("invalid_input", str(exc))
    now = time.time()
    replay, prior = await store.replay(result_id, idempotency_key, digest, now=now)
    if replay == "conflict":
        m.record_workflow_rejection("retrieval_receipt", "idempotency")
        return _error("idempotency_conflict", "idempotency key was already used with different content")
    if replay == "replayed" and prior:
        return {"state": "replayed", "receipt": _decorate_receipt(result_id, prior)}
    source = await _source(result_id)
    if isinstance(source, dict):
        return source
    snapshot, result, _index = source
    receipt = {
        "contract": RECEIPT_CONTRACT,
        "version": RECEIPT_VERSION,
        "result_id": result_id,
        "attribution": {"retriever": body["retriever"], "authenticated_tenant": current_tenant()},
        "observation": body["observation"],
        "discovery": _discovery(snapshot, result, result_id),
        "observations_verified": False,
    }
    outcome, stored = await store.submit(result_id, idempotency_key, digest, receipt, now=now)
    if outcome == "conflict":
        m.record_workflow_rejection("retrieval_receipt", "idempotency")
        return _error("idempotency_conflict", "idempotency key was already used with different content")
    if outcome == "capacity":
        m.record_workflow_rejection("retrieval_receipt", "capacity")
        return _error("resource_limit", "receipt limit reached for this result")
    if outcome == "size":
        return _error("invalid_input", "encoded receipt exceeds 32768 bytes")
    if outcome == "unavailable" or stored is None:
        return _error("store_unavailable", "receipt could not be persisted")
    m.record_workflow_accepted("retrieval_receipt", "immediate")
    m.record_workflow_terminal("retrieval_receipt", body["observation"]["status"])
    return {"state": outcome, "receipt": _decorate_receipt(result_id, stored)}


async def _read(result_id: str, limit: int) -> dict[str, Any]:
    enabled = _enabled()
    if isinstance(enabled, dict):
        return enabled
    state, store = enabled
    receipts, total = await store.read(result_id, now=time.time(), limit=limit)
    source = await _source(result_id)
    if isinstance(source, dict):
        code = source["error"]["code"]
        source_state = {
            "expired_handle": "expired",
            "invalid_result_id": "unknown",
            "store_unavailable": "unavailable",
        }.get(code, "unknown")
        if not receipts:
            if code == "expired_handle":
                m.record_workflow_expiry("retrieval_receipt", "receipt")
            return source
        discovery = receipts[0].get("discovery") if receipts else None
    else:
        source_state = "live"
        snapshot, result, _index = source
        discovery = _discovery(snapshot, result, result_id)
    return {
        "result_id": result_id,
        "source_snapshot_status": source_state,
        "discovery": discovery,
        "receipts": [_decorate_receipt(result_id, receipt) for receipt in receipts],
        "total": total,
        "returned": len(receipts),
        "has_more": total > len(receipts),
        "observations_verified": False,
    }


async def slopsearx_read_retrieval_receipts(result_id: str, limit: StrictInt = 20) -> dict[str, Any]:
    """Read bounded attributed observations without altering their source snapshot."""
    if type(limit) is not int or not 1 <= limit <= 20:
        return _error("invalid_input", "limit must be an integer between 1 and 20")
    return await _read(result_id, limit)


async def slopsearx_export_research_manifest(
    result_ids: list[str] | None = None,
    artifacts: list[dict[str, Any]] | None = None,
    max_depth: StrictInt = 2,
    max_nodes: StrictInt = 100,
) -> dict[str, Any]:
    """Join explicit results or a bounded lineage cut to retained observations."""
    enabled = _enabled()
    if isinstance(enabled, dict):
        return enabled
    if result_ids is None:
        result_ids = []
    else:
        result_ids = list(result_ids)
    if artifacts is None:
        artifacts = []
    if not isinstance(result_ids, list) or len(result_ids) > MAX_MANIFEST_RESULTS:
        return _error("invalid_input", "result_ids must contain at most 25 entries")
    if any(not isinstance(result_id, str) or not result_id for result_id in result_ids):
        return _error("invalid_input", "every result_id must be a non-empty string")
    if not isinstance(artifacts, list) or len(artifacts) > MAX_MANIFEST_RESULTS:
        return _error("invalid_input", "artifacts must contain at most 25 entries")
    if not result_ids and not artifacts:
        return _error("invalid_input", "provide at least one result_id or artifact")
    if type(max_depth) is not int or not 0 <= max_depth <= 5:
        return _error("invalid_input", "max_depth must be an integer from 0 to 5")
    if type(max_nodes) is not int or not 1 <= max_nodes <= 100:
        return _error("invalid_input", "max_nodes must be an integer from 1 to 100")
    roots: list[dict[str, Any]] = []
    try:
        roots = [parse_artifact_ref(item) for item in artifacts]
    except ValueError as exc:
        return _error("invalid_input", str(exc))
    if len({(item["kind"], item["id"]) for item in roots}) != len(roots):
        return _error("invalid_input", "artifacts must not contain duplicates")
    if len(set(result_ids)) != len(result_ids):
        return _error("invalid_input", "result_ids must not contain duplicates")
    lineage_cuts: list[dict[str, Any]] = []
    if roots:
        from slopsearx.mcp.lineage_tools import slopsearx_get_artifact_lineage

        for root in roots:
            cut = await slopsearx_get_artifact_lineage(
                root,
                direction="both",
                max_depth=max_depth,
                max_nodes=max_nodes,
            )
            if "error" in cut:
                return cut
            lineage_cuts.append(cut)
            result_ids.extend(
                node["artifact"]["id"]
                for node in cut["nodes"]
                if node.get("status") == "live" and node.get("artifact", {}).get("kind") == "result"
            )
    result_ids = list(dict.fromkeys(result_ids))
    if len(result_ids) > MAX_MANIFEST_RESULTS:
        return _error("resource_limit", "lineage selection exceeds 25 results")
    items = []
    for result_id in result_ids:
        item = await _read(result_id, 20)
        items.append({"result_id": result_id, **item} if "error" in item else item)
    manifest: dict[str, Any] = {
        "contract": "slopsearx.research_manifest",
        "version": 1,
        "generated_at": time.time(),
        "items": items,
        "lineage_cuts": lineage_cuts,
        "observations_verified": False,
        "verification_note": NON_VERIFICATION_NOTE,
    }
    manifest["artifact"] = artifact_ref("research_manifest", manifest_artifact_id(manifest))
    manifest["lineage"] = [
        lineage_edge(manifest["artifact"], "contains", artifact_ref("result", result_id)) for result_id in result_ids
    ]
    if len(json.dumps(manifest, separators=(",", ":"), ensure_ascii=False).encode()) > MAX_MANIFEST_BYTES:
        return _error("resource_limit", "encoded manifest exceeds 1048576 bytes")
    return manifest
