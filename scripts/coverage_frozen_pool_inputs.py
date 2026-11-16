"""Fail-closed consumer boundary for a fresh, separately collected pool corpus.

Frozen-pool eligibility is recorded separately from each preserved search
operation status. This module does not collect searches, issue permits, rewrite
operation outcomes, or award quality credit.
"""

from __future__ import annotations

import hashlib
import json
import math
import re
import stat
import uuid
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Mapping

from scripts import coverage_pipeline_inputs, coverage_study_acquire
from scripts.coverage_study_acquire import ALLOWED_ENGINES

SCHEMA = "coverage-frozen-pool-evidence/1"
INDEX_SCHEMA = "coverage-frozen-pool-index/1"
COLLECTION_RECEIPT_SCHEMA = "coverage-frozen-pool-collection-receipt/1"
AVAILABILITY_SCHEMA = "coverage-engine-availability/1"
_SHA = re.compile(r"[0-9a-f]{64}\Z")
_STATUSES = {"ok", "rate_limited", "blocked", "error", "timeout", "unavailable"}
_OPERATION_STATUSES = {"complete", "inconclusive"}
_VERIFIED_SEAL = object()


class FrozenPoolError(ValueError):
    """A frozen-pool corpus or its provenance is invalid."""


@dataclass(frozen=True)
class FrozenPoolEvidence:
    snapshots_directory: Path
    evidence_bytes: bytes
    expected_evidence_sha256: str
    collector_receipt_bytes: bytes
    expected_collector_receipt_sha256: str
    availability_bytes: bytes
    expected_availability_sha256: str


@dataclass(frozen=True)
class FrozenPoolRegistrationContext:
    """Externally pinned registration inputs for the bounded consumer phase."""

    stage_uuid: str
    stage_kind: str
    source_revision: str
    source_closure_sha256: str
    protocol_sha256: str
    cohorts_sha256: str
    candidate_identity_sha256: str
    candidate_endpoint_sha256: str
    acquisition_plan_sha256: str
    task_ids: tuple[str, ...]
    navigation_ids: tuple[str, ...]
    evidence_sha256: str
    collector_receipt_sha256: str
    availability_sha256: str


@dataclass(frozen=True)
class FrozenPoolPhaseResult:
    verified: "VerifiedFrozenPoolEvidence"
    pipeline_inputs: Mapping[str, object]


@dataclass(frozen=True)
class VerifiedPoolSnapshotSource:
    """Common, source-bound view consumed by pool-derived downstream phases."""

    snapshots_directory: Path
    snapshot_index_sha256: str
    pool_manifest_sha256: str
    pool_index: Mapping[str, object]
    operations: Mapping[str, Mapping[str, object]]
    evidence_sha256: str
    availability_sha256: str
    collector_receipt_sha256: str
    execution_profile: str
    proof_sha256: str
    _seal: object


VerifiedFrozenPoolEvidence = VerifiedPoolSnapshotSource


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    try:
        return (
            json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n"
        ).encode()
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise FrozenPoolError("frozen-pool-json-invalid") from exc


def _strict(raw: bytes, label: str) -> dict[str, object]:
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise FrozenPoolError(f"{label}-duplicate-key")
            result[key] = value
        return result

    try:
        value = json.loads(
            raw.decode("utf-8"), object_pairs_hook=pairs, parse_constant=lambda _v: (_ for _ in ()).throw(ValueError())
        )
    except FrozenPoolError:
        raise
    except Exception as exc:
        raise FrozenPoolError(f"{label}-invalid-json") from exc
    if type(value) is not dict or _canonical(value) != raw:
        raise FrozenPoolError(f"{label}-not-canonical")
    return value


def _read_private_regular(root: Path, filename: str, label: str) -> bytes:
    path = root / filename
    try:
        info = path.lstat()
        if not stat.S_ISREG(info.st_mode) or info.st_mode & 0o777 != 0o600:
            raise FrozenPoolError(f"{label}-file-mode-invalid")
        return path.read_bytes()
    except FrozenPoolError:
        raise
    except OSError as exc:
        raise FrozenPoolError(f"{label}-file-unavailable") from exc


def _utc(value: object, label: str) -> datetime:
    if type(value) is not str or not value.endswith("Z"):
        raise FrozenPoolError(f"{label}-invalid")
    try:
        return datetime.fromisoformat(value[:-1] + "+00:00")
    except ValueError as exc:
        raise FrozenPoolError(f"{label}-invalid") from exc


def verify_frozen_pool_evidence(
    evidence: FrozenPoolEvidence,
    *,
    expected_stage_uuid: str,
    expected_stage_kind: str,
    expected_source_revision: str,
    expected_source_closure_sha256: str,
    expected_protocol_sha256: str,
    expected_cohorts_sha256: str,
    expected_candidate_identity_sha256: str,
    expected_candidate_endpoint_sha256: str,
    expected_acquisition_plan_sha256: str,
    expected_task_ids: tuple[str, ...],
    expected_navigation_ids: tuple[str, ...],
    stage_started_monotonic: float,
    stage_deadline_monotonic: float,
    now_monotonic: float,
) -> VerifiedFrozenPoolEvidence:
    """Verify exact source pins, freshness, 8+5 membership, and raw outcomes."""
    if type(evidence) is not FrozenPoolEvidence or not isinstance(evidence.snapshots_directory, Path):
        raise FrozenPoolError("frozen-pool-evidence-type")
    try:
        if str(uuid.UUID(expected_stage_uuid)) != expected_stage_uuid:
            raise ValueError
    except (TypeError, ValueError) as exc:
        raise FrozenPoolError("frozen-pool-stage-identity-invalid") from exc
    if expected_stage_kind not in {"development", "confirmation"}:
        raise FrozenPoolError("frozen-pool-stage-kind-invalid")
    if type(expected_source_revision) is not str or re.fullmatch(r"[0-9a-f]{40}", expected_source_revision) is None:
        raise FrozenPoolError("frozen-pool-source-revision-invalid")
    for pin in (
        expected_source_closure_sha256,
        expected_protocol_sha256,
        expected_cohorts_sha256,
        expected_candidate_identity_sha256,
        expected_candidate_endpoint_sha256,
        expected_acquisition_plan_sha256,
    ):
        if type(pin) is not str or not _SHA.fullmatch(pin):
            raise FrozenPoolError("frozen-pool-registration-pin-invalid")
    if (
        not _SHA.fullmatch(evidence.expected_evidence_sha256)
        or _sha(evidence.evidence_bytes) != evidence.expected_evidence_sha256
    ):
        raise FrozenPoolError("frozen-pool-evidence-pin-mismatch")
    if (
        not _SHA.fullmatch(evidence.expected_collector_receipt_sha256)
        or _sha(evidence.collector_receipt_bytes) != evidence.expected_collector_receipt_sha256
    ):
        raise FrozenPoolError("frozen-pool-collector-receipt-pin-mismatch")
    if (
        not _SHA.fullmatch(evidence.expected_availability_sha256)
        or _sha(evidence.availability_bytes) != evidence.expected_availability_sha256
    ):
        raise FrozenPoolError("frozen-pool-availability-pin-mismatch")
    if (
        len(expected_task_ids) != 8
        or len(set(expected_task_ids)) != 8
        or len(expected_navigation_ids) != 5
        or len(set(expected_navigation_ids)) != 5
        or set(expected_task_ids) & set(expected_navigation_ids)
    ):
        raise FrozenPoolError("frozen-pool-registered-membership-invalid")
    clocks = (stage_started_monotonic, stage_deadline_monotonic, now_monotonic)
    if (
        any(type(value) not in (int, float) or not math.isfinite(value) for value in clocks)
        or not stage_started_monotonic < stage_deadline_monotonic
        or not stage_started_monotonic <= now_monotonic < stage_deadline_monotonic
    ):
        raise FrozenPoolError("frozen-pool-stage-clock-invalid")

    doc = _strict(evidence.evidence_bytes, "frozen-pool-evidence")
    required_doc = {
        "schema",
        "stage_uuid",
        "stage_kind",
        "source_revision",
        "source_closure_sha256",
        "protocol_sha256",
        "cohorts_sha256",
        "candidate_identity_sha256",
        "candidate_endpoint_sha256",
        "acquisition_plan_sha256",
        "pool_manifest_sha256",
        "pool_snapshot_index_sha256",
        "availability_sha256",
        "collector_receipt_sha256",
        "collection_started_monotonic",
        "collection_completed_monotonic",
        "stage_deadline_monotonic",
        "collection_started_utc",
        "collection_completed_utc",
        "quality_credit",
    }
    expected_doc = {
        "schema": SCHEMA,
        "stage_uuid": expected_stage_uuid,
        "stage_kind": expected_stage_kind,
        "source_revision": expected_source_revision,
        "source_closure_sha256": expected_source_closure_sha256,
        "protocol_sha256": expected_protocol_sha256,
        "cohorts_sha256": expected_cohorts_sha256,
        "candidate_identity_sha256": expected_candidate_identity_sha256,
        "candidate_endpoint_sha256": expected_candidate_endpoint_sha256,
        "acquisition_plan_sha256": expected_acquisition_plan_sha256,
        "availability_sha256": evidence.expected_availability_sha256,
        "collector_receipt_sha256": evidence.expected_collector_receipt_sha256,
        "quality_credit": False,
    }
    if set(doc) != required_doc or any(doc.get(key) != value for key, value in expected_doc.items()):
        raise FrozenPoolError("frozen-pool-evidence-binding-mismatch")
    for key in ("pool_manifest_sha256", "pool_snapshot_index_sha256"):
        if type(doc.get(key)) is not str or not _SHA.fullmatch(doc[key]):
            raise FrozenPoolError("frozen-pool-evidence-hash-invalid")
    started, completed = doc["collection_started_monotonic"], doc["collection_completed_monotonic"]
    if (
        type(started) not in (int, float)
        or type(completed) not in (int, float)
        or not math.isfinite(started)
        or not math.isfinite(completed)
        or type(doc.get("stage_deadline_monotonic")) not in (int, float)
        or doc["stage_deadline_monotonic"] != stage_deadline_monotonic
        or not stage_started_monotonic <= started <= completed <= now_monotonic < stage_deadline_monotonic
    ):
        raise FrozenPoolError("frozen-pool-collection-outside-stage")
    started_utc = _utc(doc["collection_started_utc"], "collection-started-utc")
    completed_utc = _utc(doc["collection_completed_utc"], "collection-completed-utc")
    if completed_utc < started_utc or abs((completed_utc - started_utc).total_seconds() - (completed - started)) > 2.0:
        raise FrozenPoolError("frozen-pool-clock-domains-disagree")

    root = evidence.snapshots_directory
    if root.is_symlink() or not root.is_dir() or root.stat(follow_symlinks=False).st_mode & 0o777 != 0o700:
        raise FrozenPoolError("frozen-pool-directory-invalid")
    manifest_raw = _read_private_regular(root, "acquisition-manifest.json", "frozen-pool-manifest")
    index_raw = _read_private_regular(root, "frozen-pool-index.json", "frozen-pool-index")
    if _sha(manifest_raw) != doc["pool_manifest_sha256"] or _sha(index_raw) != doc["pool_snapshot_index_sha256"]:
        raise FrozenPoolError("frozen-pool-snapshot-pin-mismatch")
    manifest = _strict(manifest_raw, "frozen-pool-manifest")
    index = _strict(index_raw, "frozen-pool-index")
    receipt = _strict(evidence.collector_receipt_bytes, "frozen-pool-collector-receipt")
    receipt_bindings = {
        "schema": COLLECTION_RECEIPT_SCHEMA,
        "scope": "registered-frozen-pool-collection",
        "status": "complete",
        "stage_uuid": expected_stage_uuid,
        "stage_kind": expected_stage_kind,
        "source_revision": expected_source_revision,
        "source_closure_sha256": expected_source_closure_sha256,
        "protocol_sha256": expected_protocol_sha256,
        "cohorts_sha256": expected_cohorts_sha256,
        "candidate_identity_sha256": expected_candidate_identity_sha256,
        "candidate_endpoint_sha256": expected_candidate_endpoint_sha256,
        "acquisition_plan_sha256": expected_acquisition_plan_sha256,
        "pool_manifest_sha256": doc["pool_manifest_sha256"],
        "pool_snapshot_index_sha256": doc["pool_snapshot_index_sha256"],
        "availability_sha256": evidence.expected_availability_sha256,
        "collection_started_monotonic": started,
        "collection_completed_monotonic": completed,
        "stage_deadline_monotonic": stage_deadline_monotonic,
        "collection_started_utc": doc["collection_started_utc"],
        "collection_completed_utc": doc["collection_completed_utc"],
    }
    component_names = {
        "admission_sha256": "admission.json",
        "transport_control_sha256": "transport-control.json",
        "request_ledger_index_sha256": "request-ledger-index.json",
        "collection_summary_sha256": "collection-summary.json",
    }
    components: dict[str, dict[str, object]] = {}
    for key, filename in component_names.items():
        component_raw = _read_private_regular(root, filename, "frozen-pool-collector-component")
        components[filename] = _strict(component_raw, filename)
        digest = _sha(component_raw)
        if receipt.get(key) != digest:
            raise FrozenPoolError("frozen-pool-collector-component-pin-mismatch")
        receipt_bindings[key] = digest
    if set(receipt) != set(receipt_bindings) or any(
        receipt.get(key) != value for key, value in receipt_bindings.items()
    ):
        raise FrozenPoolError("frozen-pool-collector-receipt-binding-mismatch")
    admission = components["admission.json"]
    admission_bindings = {
        "schema": "coverage-frozen-collection-admission/1",
        "pool_input_mode": "frozen-pool/1",
        "stage_clock_sha256": admission.get("stage_clock_sha256"),
        "clock_origin_sha256": admission.get("clock_origin_sha256"),
        "stage_uuid": expected_stage_uuid,
        "stage": expected_stage_kind,
        "source_revision": expected_source_revision,
        "source_closure_sha256": expected_source_closure_sha256,
        "protocol_sha256": expected_protocol_sha256,
        "cohorts_sha256": expected_cohorts_sha256,
        "input_manifest_sha256": manifest.get("input_manifest_sha256"),
        "acquisition_plan_sha256": expected_acquisition_plan_sha256,
        "candidate_identity_sha256": expected_candidate_identity_sha256,
        "candidate_endpoint_sha256": expected_candidate_endpoint_sha256,
        "stage_manifest_sha256": doc["pool_manifest_sha256"],
        "permit_sha256": admission.get("permit_sha256"),
        "lease_sha256": admission.get("lease_sha256"),
    }
    if (
        set(admission) != set(admission_bindings)
        or any(admission.get(key) != value for key, value in admission_bindings.items())
        or any(
            type(admission.get(key)) is not str or not _SHA.fullmatch(admission[key])
            for key in ("stage_clock_sha256", "clock_origin_sha256", "permit_sha256", "lease_sha256")
        )
    ):
        raise FrozenPoolError("frozen-pool-admission-binding-mismatch")
    manifest_bindings = {
        "stage": expected_stage_kind,
        "stage_uuid": expected_stage_uuid,
        "source_revision": expected_source_revision,
        "source_closure_sha256": expected_source_closure_sha256,
        "protocol_sha256": expected_protocol_sha256,
        "cohorts_sha256": expected_cohorts_sha256,
        "acquisition_plan_sha256": expected_acquisition_plan_sha256,
    }
    if any(manifest.get(key) != value for key, value in manifest_bindings.items()):
        raise FrozenPoolError("frozen-pool-manifest-binding-mismatch")
    if (
        type(manifest.get("research_cases")) is not list
        or [row.get("task_id") if type(row) is dict else None for row in manifest["research_cases"]]
        != list(expected_task_ids)
        or type(manifest.get("navigation_targets")) is not list
        or [row.get("target_id") if type(row) is dict else None for row in manifest["navigation_targets"]]
        != list(expected_navigation_ids)
    ):
        raise FrozenPoolError("frozen-pool-manifest-membership-mismatch")
    index_bindings = {
        "schema": INDEX_SCHEMA,
        "stage": expected_stage_kind,
        "status": "complete",
        "stage_uuid": expected_stage_uuid,
        "stage_kind": expected_stage_kind,
        "source_revision": expected_source_revision,
        "source_closure_sha256": expected_source_closure_sha256,
        "protocol_sha256": expected_protocol_sha256,
        "cohorts_sha256": expected_cohorts_sha256,
        "candidate_identity_sha256": expected_candidate_identity_sha256,
        "candidate_endpoint_sha256": expected_candidate_endpoint_sha256,
        "acquisition_plan_sha256": expected_acquisition_plan_sha256,
        "pool_manifest_sha256": doc["pool_manifest_sha256"],
        "availability_sha256": evidence.expected_availability_sha256,
    }
    if any(index.get(key) != value for key, value in index_bindings.items()):
        raise FrozenPoolError("frozen-pool-index-binding-mismatch")
    rows = index.get("operations")
    expected_ids = [*expected_task_ids, *expected_navigation_ids]
    if (
        type(rows) is not list
        or [row.get("operation_id") if type(row) is dict else None for row in rows] != expected_ids
    ):
        raise FrozenPoolError("frozen-pool-operation-membership-mismatch")
    operations: dict[str, Mapping[str, object]] = {}
    for row in rows:
        if (
            type(row) is not dict
            or set(row)
            != {"operation_id", "kind", "acquisition_status", "pool_status", "file", "sha256", "bytes", "card_count"}
            or row.get("pool_status") != "complete"
            or row.get("acquisition_status") not in _OPERATION_STATUSES
            or type(row.get("file")) is not str
            or Path(row["file"]).name != row["file"]
            or type(row.get("bytes")) is not int
            or row["bytes"] < 0
            or type(row.get("card_count")) is not int
            or row["card_count"] < 0
            or type(row.get("sha256")) is not str
            or not _SHA.fullmatch(row["sha256"])
        ):
            raise FrozenPoolError("frozen-pool-operation-row-invalid")
        try:
            operation_raw = _read_private_regular(root, row["file"], "frozen-pool-operation")
        except FrozenPoolError as exc:
            raise FrozenPoolError("frozen-pool-operation-file-unavailable") from exc
        if _sha(operation_raw) != row["sha256"] or len(operation_raw) != row["bytes"]:
            raise FrozenPoolError("frozen-pool-operation-file-pin-mismatch")
        artifact = _strict(operation_raw, "frozen-pool-operation")
        profile = coverage_study_acquire.execution_profile_from_manifest(manifest)
        expected_schema = (
            "coverage-native-pool-snapshot/2"
            if profile is coverage_study_acquire.CONTINUE_ACQUISITION_PROFILE
            else "coverage-native-pool-snapshot/1"
        )
        if (
            artifact.get("schema") != expected_schema
            or artifact.get("stage_uuid") != expected_stage_uuid
            or artifact.get("source_revision") != expected_source_revision
            or artifact.get("source_closure_sha256") != expected_source_closure_sha256
            or artifact.get("protocol_sha256") != expected_protocol_sha256
            or artifact.get("cohorts_sha256") != expected_cohorts_sha256
            or artifact.get("acquisition_plan_sha256") != expected_acquisition_plan_sha256
            or artifact.get("stage_manifest_sha256") != doc["pool_manifest_sha256"]
        ):
            raise FrozenPoolError("frozen-pool-operation-source-mismatch")
        operation = artifact.get("operation")
        response = operation.get("canonical_response") if type(operation) is dict else None
        if (
            type(operation) is not dict
            or operation.get("operation_id") != row["operation_id"]
            or operation.get("kind") != row["kind"]
            or operation.get("status") != row["acquisition_status"]
            or operation.get("pool_count") != row["card_count"]
            or type(response) is not dict
            or type(response.get("results")) is not list
            or len(response["results"]) != row["card_count"]
        ):
            raise FrozenPoolError("frozen-pool-original-operation-mismatch")
        operations[row["operation_id"]] = artifact

    for task_id, task in zip(expected_task_ids, manifest["research_cases"], strict=True):
        row = operations[task_id]["operation"]
        response = row["canonical_response"]
        band = task.get("pool_plan", {}).get("band") if type(task.get("pool_plan")) is dict else None
        engines = task.get("pool_plan", {}).get("engines") if type(task.get("pool_plan")) is dict else None
        count = len(response["results"])
        in_band = (band == "research_le_40" and 1 <= count <= 40) or (band == "research_41_80" and 41 <= count <= 80)
        if (
            row.get("kind") != "research"
            or row.get("query") != task.get("search_query")
            or row.get("band") != band
            or not in_band
            or response.get("query") != task.get("search_query")
            or response.get("scope", {}).get("selected_engines") != engines
        ):
            raise FrozenPoolError("frozen-pool-natural-band-invalid")
    for target_id, target in zip(expected_navigation_ids, manifest["navigation_targets"], strict=True):
        artifact = operations[target_id]
        row = artifact["operation"]
        response = row["canonical_response"]
        if (
            artifact.get("task_plan") != target
            or row.get("kind") != "navigation"
            or row.get("query") != target.get("search_query")
            or response.get("query") != target.get("search_query")
            or not response.get("results")
        ):
            raise FrozenPoolError("frozen-pool-navigation-invalid")

    availability = _strict(evidence.availability_bytes, "engine-availability")
    if (
        set(availability) != {"schema", "stage_uuid", "operations"}
        or availability.get("schema") != AVAILABILITY_SCHEMA
        or availability.get("stage_uuid") != expected_stage_uuid
    ):
        raise FrozenPoolError("frozen-pool-availability-binding-mismatch")
    availability_rows = availability.get("operations")
    if (
        type(availability_rows) is not list
        or [r.get("operation_id") if type(r) is dict else None for r in availability_rows] != expected_ids
    ):
        raise FrozenPoolError("frozen-pool-availability-membership-mismatch")
    for operation_id, availability_row in zip(expected_ids, availability_rows, strict=True):
        response = operations[operation_id]["operation"]["canonical_response"]
        outcomes = response.get("engine_outcomes")
        if type(outcomes) is not list:
            raise FrozenPoolError("frozen-pool-engine-outcomes-missing")
        actual = []
        seen = set()
        for outcome in outcomes:
            if (
                type(outcome) is not dict
                or outcome.get("engine") not in ALLOWED_ENGINES
                or outcome.get("engine") in seen
                or outcome.get("status") not in _STATUSES
                or type(outcome.get("result_count")) is not int
                or outcome["result_count"] < 0
            ):
                raise FrozenPoolError("frozen-pool-engine-outcome-invalid")
            seen.add(outcome["engine"])
            actual.append(
                {"engine": outcome["engine"], "status": outcome["status"], "result_count": outcome["result_count"]}
            )
        if availability_row.get("engines") != actual:
            raise FrozenPoolError("frozen-pool-availability-does-not-match-snapshot")
        operation = operations[operation_id]["operation"]
        if seen != set(operation.get("engines", [])):
            raise FrozenPoolError("frozen-pool-engine-outcome-membership-mismatch")

    summary = components["collection-summary.json"]
    status_rows = summary.get("operation_statuses")
    pool_rows = index.get("operations")
    expected_summary_keys = {
        "schema",
        "stage_uuid",
        "stage_manifest_sha256",
        "pool_input_mode",
        "status",
        "physical_request_count",
        "transfer_bytes",
        "operation_statuses",
        "pool_eligibility",
        "fatal_uninvoked_slots",
        "collection_started_monotonic",
        "collection_completed_monotonic",
        "stage_deadline_monotonic",
        "collection_started_utc",
        "collection_completed_utc",
        "stage_deadline_utc",
        "quality_credit",
    }
    eligibility_rows = summary.get("pool_eligibility")
    if (
        set(summary) != expected_summary_keys
        or type(status_rows) is not list
        or len(status_rows) != len(expected_ids)
        or type(pool_rows) is not list
        or len(pool_rows) != len(expected_ids)
        or summary.get("schema") != "coverage-frozen-collection-summary/1"
        or summary.get("stage_uuid") != expected_stage_uuid
        or summary.get("stage_manifest_sha256") != doc["pool_manifest_sha256"]
        or summary.get("pool_input_mode") != "frozen-pool/1"
        or summary.get("status") != "complete"
        or summary.get("quality_credit") is not False
        or summary.get("stage_deadline_monotonic") != stage_deadline_monotonic
        or summary.get("collection_started_monotonic") != started
        or summary.get("collection_completed_monotonic") != completed
        or summary.get("collection_started_utc") != doc["collection_started_utc"]
        or summary.get("collection_completed_utc") != doc["collection_completed_utc"]
        or type(summary.get("stage_deadline_utc")) is not str
        or _utc(summary["stage_deadline_utc"], "collection-stage-deadline-utc")
        <= _utc(doc["collection_completed_utc"], "collection-completed-utc")
        or type(summary.get("physical_request_count")) is not int
        or not 0 <= summary["physical_request_count"] <= 53
        or type(summary.get("transfer_bytes")) is not int
        or summary["transfer_bytes"] < 0
        or type(summary.get("fatal_uninvoked_slots")) is not list
        or summary["fatal_uninvoked_slots"]
        or type(eligibility_rows) is not list
        or len(eligibility_rows) != len(expected_ids)
    ):
        raise FrozenPoolError("frozen-pool-collection-summary-inventory")
    for operation_id, status_row, pool_row, eligibility in zip(
        expected_ids, status_rows, pool_rows, eligibility_rows, strict=True
    ):
        operation = operations[operation_id]["operation"]
        if (
            type(status_row) is not dict
            or status_row.get("operation_id") != operation_id
            or status_row.get("status") != operation.get("status")
            or status_row.get("failure_reasons") != operation.get("failure_reasons", [])
            or pool_row.get("acquisition_status") != operation.get("status")
            or pool_row.get("pool_status") != "complete"
            or type(eligibility) is not dict
            or set(eligibility) != {"operation_id", "pool_status"}
            or eligibility.get("operation_id") != operation_id
            or eligibility.get("pool_status") != pool_row.get("pool_status")
        ):
            raise FrozenPoolError("frozen-pool-collection-summary-status-mismatch")

    return VerifiedFrozenPoolEvidence(
        snapshots_directory=root,
        snapshot_index_sha256=doc["pool_snapshot_index_sha256"],
        pool_manifest_sha256=doc["pool_manifest_sha256"],
        pool_index=index,
        operations=operations,
        evidence_sha256=evidence.expected_evidence_sha256,
        availability_sha256=evidence.expected_availability_sha256,
        collector_receipt_sha256=evidence.expected_collector_receipt_sha256,
        execution_profile=coverage_study_acquire.execution_profile_from_manifest(manifest).identifier,
        proof_sha256=_sha(_canonical({"index": index, "operations": operations})),
        _seal=_VERIFIED_SEAL,
    )


def is_verified_pool_snapshot_source(value: object) -> bool:
    return (
        type(value) is VerifiedPoolSnapshotSource
        and value._seal is _VERIFIED_SEAL
        and value.proof_sha256 == _sha(_canonical({"index": value.pool_index, "operations": value.operations}))
    )


is_verified_pool_evidence = is_verified_pool_snapshot_source


def build_pipeline_inputs_from_frozen(
    research_cases: list[dict],
    *,
    evidence: FrozenPoolEvidence,
    verified: VerifiedFrozenPoolEvidence,
    expected_stage_uuid: str,
    expected_source_revision: str,
    expected_protocol_sha256: str,
    expected_cohorts_sha256: str,
    expected_candidate_identity_sha256: str,
    expected_candidate_endpoint_sha256: str,
) -> dict[str, object]:
    if not is_verified_pool_evidence(verified) or (
        verified.snapshots_directory != evidence.snapshots_directory
        or verified.evidence_sha256 != evidence.expected_evidence_sha256
        or verified.availability_sha256 != evidence.expected_availability_sha256
        or verified.collector_receipt_sha256 != evidence.expected_collector_receipt_sha256
    ):
        raise FrozenPoolError("frozen-pool-verification-object-mismatch")
    return coverage_pipeline_inputs.build_pipeline_inputs(
        research_cases,
        snapshots_directory=verified.snapshots_directory,
        expected_snapshot_index_sha256=verified.snapshot_index_sha256,
        expected_stage_manifest_sha256=verified.pool_manifest_sha256,
        stage_uuid=expected_stage_uuid,
        source_revision=expected_source_revision,
        protocol_sha256=expected_protocol_sha256,
        cohorts_sha256=expected_cohorts_sha256,
        candidate_identity_sha256=expected_candidate_identity_sha256,
        candidate_endpoint_sha256=expected_candidate_endpoint_sha256,
        expected_execution_profile=verified.execution_profile,
        verified_frozen_corpus=verified,
    )


def run_frozen_pool_input_phase(
    research_cases: list[dict],
    *,
    evidence: FrozenPoolEvidence,
    registration: FrozenPoolRegistrationContext,
    stage_started_monotonic: float,
    stage_deadline_monotonic: float,
    now_monotonic: float,
) -> FrozenPoolPhaseResult:
    """Verify registered corpus bytes, then project them through the shared builder.

    This is a consumer phase only. It does not collect, seal, or authorize a
    corpus; the three content digests must come from the external registration.
    """
    if type(registration) is not FrozenPoolRegistrationContext:
        raise FrozenPoolError("frozen-pool-registration-context-type")
    if (
        evidence.expected_evidence_sha256 != registration.evidence_sha256
        or evidence.expected_collector_receipt_sha256 != registration.collector_receipt_sha256
        or evidence.expected_availability_sha256 != registration.availability_sha256
    ):
        raise FrozenPoolError("frozen-pool-registration-content-pin-mismatch")
    verified = verify_frozen_pool_evidence(
        evidence,
        expected_stage_uuid=registration.stage_uuid,
        expected_stage_kind=registration.stage_kind,
        expected_source_revision=registration.source_revision,
        expected_source_closure_sha256=registration.source_closure_sha256,
        expected_protocol_sha256=registration.protocol_sha256,
        expected_cohorts_sha256=registration.cohorts_sha256,
        expected_candidate_identity_sha256=registration.candidate_identity_sha256,
        expected_candidate_endpoint_sha256=registration.candidate_endpoint_sha256,
        expected_acquisition_plan_sha256=registration.acquisition_plan_sha256,
        expected_task_ids=registration.task_ids,
        expected_navigation_ids=registration.navigation_ids,
        stage_started_monotonic=stage_started_monotonic,
        stage_deadline_monotonic=stage_deadline_monotonic,
        now_monotonic=now_monotonic,
    )
    inputs = build_pipeline_inputs_from_frozen(
        research_cases,
        evidence=evidence,
        verified=verified,
        expected_stage_uuid=registration.stage_uuid,
        expected_source_revision=registration.source_revision,
        expected_protocol_sha256=registration.protocol_sha256,
        expected_cohorts_sha256=registration.cohorts_sha256,
        expected_candidate_identity_sha256=registration.candidate_identity_sha256,
        expected_candidate_endpoint_sha256=registration.candidate_endpoint_sha256,
    )
    return FrozenPoolPhaseResult(verified=verified, pipeline_inputs=inputs)
