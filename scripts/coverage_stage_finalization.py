"""Independent, forward-only verifier for v2 coverage-stage closeout.

This module never changes a study gate or admits a product. It joins durable
stage artifacts after the coordinator has sampled actual elapsed time.
"""

from __future__ import annotations

import hashlib
import json
import math
import os
import re
import stat
from dataclasses import dataclass
from pathlib import Path
from typing import Mapping

from scripts import coverage_resource_evidence
from scripts import intent_ranking_receipts as receipts

_SHA = re.compile(r"[0-9a-f]{64}\Z")


def _effective_uid() -> int:
    return os.geteuid()


class StageFinalizationError(RuntimeError):
    """A stage artifact set is invalid, incomplete, or not eligible."""


@dataclass(frozen=True)
class FinalizationResult:
    status: str
    stage_uuid: str
    actual_elapsed_seconds: float
    gates: Mapping[str, bool | None]
    finalization_receipt_sha256: str
    receipt_bytes: bytes
    eligible_for_independent_decision: bool


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _read(path: Path, limit: int) -> bytes:
    try:
        before = path.lstat()
        if (
            stat.S_ISLNK(before.st_mode)
            or not stat.S_ISREG(before.st_mode)
            or stat.S_IMODE(before.st_mode) != 0o600
            or before.st_uid != _effective_uid()
        ):
            raise StageFinalizationError("finalization-artifact-unsafe")
        raw = receipts._read_private(path, max_bytes=limit)
        after = path.lstat()
        if (
            after.st_dev != before.st_dev
            or after.st_ino != before.st_ino
            or after.st_uid != before.st_uid
            or after.st_mode != before.st_mode
        ):
            raise StageFinalizationError("finalization-artifact-changed")
    except receipts.ReceiptError as exc:
        raise StageFinalizationError("finalization-artifact-unsafe-or-oversize") from exc
    except OSError as exc:
        raise StageFinalizationError("finalization-artifact-unavailable") from exc
    return raw


def _json(raw: bytes) -> dict:
    value = _strict_loads(raw)
    if (
        type(value) is not dict
        or json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode() != raw
    ):
        raise StageFinalizationError("finalization-json-noncanonical")
    return value


def _reject_json_constant(value: str):
    raise ValueError(f"non-finite-json-number:{value}")


def _strict_loads(raw: bytes) -> object:
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError("duplicate-json-key")
            result[key] = value
        return result

    try:
        value = json.loads(
            raw,
            object_pairs_hook=pairs,
            parse_constant=_reject_json_constant,
        )
    except (json.JSONDecodeError, UnicodeDecodeError, ValueError) as exc:
        raise StageFinalizationError("finalization-json-invalid") from exc
    return value


def _finite_nonnegative(value: object) -> bool:
    return type(value) in (int, float) and math.isfinite(value) and value >= 0


def verify_stage_finalization(
    *,
    inventory_path: Path,
    expected_inventory_sha256: str,
    closeout_receipt_path: Path,
    expected_closeout_receipt_sha256: str,
    pending_time_receipt_path: Path,
    expected_pending_time_receipt_sha256: str,
    gate_input_manifest_path: Path,
    expected_gate_input_manifest_sha256: str,
    gate_calculation_receipt_path: Path,
    expected_gate_calculation_receipt_sha256: str,
    expected_stage_uuid: str,
    expected_source_revision: str,
    protocol_bytes: bytes,
    cohorts_bytes: bytes,
    expected_protocol_sha256: str,
    expected_cohorts_sha256: str,
) -> FinalizationResult:
    """Join all v2 pins and return eligibility only after actual end-time proof.

    The pending sample is a lower bound taken before gate calculation. Since
    elapsed time is monotonic, a final under-deadline receipt proves the
    pending stage-wall predicate and the complete stage also fit the same
    unchanged limit. A late closeout can never be rescued by an early pass.
    """
    if any(
        not _SHA.fullmatch(value)
        for value in (
            expected_inventory_sha256,
            expected_closeout_receipt_sha256,
            expected_pending_time_receipt_sha256,
            expected_gate_input_manifest_sha256,
            expected_gate_calculation_receipt_sha256,
            expected_protocol_sha256,
            expected_cohorts_sha256,
        )
    ):
        raise StageFinalizationError("finalization-pin-invalid")
    if _sha(protocol_bytes) != expected_protocol_sha256 or _sha(cohorts_bytes) != expected_cohorts_sha256:
        raise StageFinalizationError("finalization-source-pin-mismatch")
    try:
        protocol = _strict_loads(protocol_bytes)
    except StageFinalizationError as exc:
        raise StageFinalizationError("finalization-protocol-invalid") from exc
    if type(protocol) is not dict:
        raise StageFinalizationError("finalization-protocol-invalid")
    if protocol.get("schema") != "coverage-first-study-protocol/2" or protocol.get("status") not in {
        "draft_not_registered_not_admitted",
        "registered",
    }:
        raise StageFinalizationError("finalization-protocol-not-v2")
    deadline = protocol.get("phase_limits", {}).get("stage_wall_seconds")
    if (
        type(deadline) not in (int, float)
        or not math.isfinite(deadline)
        or deadline <= 0
        or type(protocol.get("quality")) is not dict
        or type(protocol.get("task_use")) is not dict
    ):
        raise StageFinalizationError("finalization-deadline-invalid")
    thresholds_sha = _sha(
        json.dumps(
            {"quality": protocol.get("quality"), "task_use": protocol.get("task_use")},
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode()
    )

    stage_dir = inventory_path.parent
    expected_names = (
        (inventory_path, "inventory.json"),
        (closeout_receipt_path, "stage-closeout.json"),
        (pending_time_receipt_path, "stage-pending-time.json"),
        (gate_input_manifest_path, "gate-input-manifest.json"),
        (gate_calculation_receipt_path, "gate-calculation-receipt.json"),
        (stage_dir / "resource-evidence.json", "resource-evidence.json"),
    )
    if any(path.parent != stage_dir or path.name != name for path, name in expected_names):
        raise StageFinalizationError("finalization-artifact-path-mismatch")
    try:
        coverage_resource_evidence._require_private_directory(stage_dir.parent)
        coverage_resource_evidence._require_private_directory(stage_dir)
    except coverage_resource_evidence.ResourceEvidenceError as exc:
        raise StageFinalizationError("finalization-private-directory-invalid") from exc

    inventory_raw = _read(inventory_path, 4_000_000)
    if _sha(inventory_raw) != expected_inventory_sha256:
        raise StageFinalizationError("finalization-inventory-pin-mismatch")
    inventory = _json(inventory_raw)
    if (
        pending_time_receipt_path.name != inventory.get("pending_time_receipt_file")
        or gate_input_manifest_path.name != inventory.get("gate_input_manifest_file")
        or gate_calculation_receipt_path.name != inventory.get("gate_calculation_receipt_file")
    ):
        raise StageFinalizationError("finalization-artifact-name-mismatch")
    pending_raw = _read(pending_time_receipt_path, 32_000)
    if _sha(pending_raw) != expected_pending_time_receipt_sha256:
        raise StageFinalizationError("finalization-pending-pin-mismatch")
    pending = _json(pending_raw)
    if (
        pending.get("schema") != "coverage-stage-pending-time/1"
        or pending.get("stage_uuid") != expected_stage_uuid
        or pending.get("source_revision") != expected_source_revision
        or pending.get("protocol_sha256") != expected_protocol_sha256
        or pending.get("cohorts_sha256") != expected_cohorts_sha256
        or pending.get("preacquisition_binding_sha256") != inventory.get("preacquisition_binding_sha256")
        or inventory.get("pending_time_receipt_sha256") != expected_pending_time_receipt_sha256
        or inventory.get("pending_time_receipt_file") != pending_time_receipt_path.name
        or inventory.get("pending_elapsed_meaning") != "lower-bound-only"
    ):
        raise StageFinalizationError("finalization-pending-binding-mismatch")
    pending_elapsed = pending.get("pending_elapsed_seconds")
    pending_observed = pending.get("observed_monotonic")
    pending_started = pending.get("stage_started_monotonic")
    stage_deadline = pending.get("stage_deadline_seconds")
    if (
        not _finite_nonnegative(pending_elapsed)
        or not _finite_nonnegative(pending_observed)
        or not _finite_nonnegative(pending_started)
        or not _finite_nonnegative(stage_deadline)
        or stage_deadline <= 0
        or stage_deadline > deadline
        or inventory.get("pending_elapsed_seconds") != pending_elapsed
        or inventory.get("stage_deadline_seconds") != stage_deadline
        or pending_elapsed != pending_observed - pending_started
        or pending_started != inventory.get("stage_started_monotonic")
        or pending_elapsed >= stage_deadline
    ):
        raise StageFinalizationError("finalization-pending-time-invalid")

    gate_raw = _read(gate_calculation_receipt_path, 128_000)
    if _sha(gate_raw) != expected_gate_calculation_receipt_sha256:
        raise StageFinalizationError("finalization-gate-receipt-pin-mismatch")
    gate = _json(gate_raw)
    gate_input_raw = _read(gate_input_manifest_path, 1_000_000)
    if _sha(gate_input_raw) != expected_gate_input_manifest_sha256:
        raise StageFinalizationError("finalization-gate-input-pin-mismatch")
    gate_input = _json(gate_input_raw)
    gates = gate.get("gates")
    from scripts.coverage_gate_calculation import GATE_NAMES

    if (
        gate.get("schema") != "coverage-first-gate-calculation-receipt/1"
        or gate.get("source_stage_uuid") != expected_stage_uuid
        or gate.get("input_manifest_sha256") != inventory.get("gate_input_manifest_sha256")
        or gate.get("input_manifest_sha256") != expected_gate_input_manifest_sha256
        or inventory.get("gate_input_manifest_file") != gate_input_manifest_path.name
        or gate.get("threshold_sha256") != thresholds_sha
        or inventory.get("thresholds_sha256") != thresholds_sha
        or inventory.get("gate_calculation_receipt_sha256") != expected_gate_calculation_receipt_sha256
        or inventory.get("gate_calculation_receipt_file") != gate_calculation_receipt_path.name
        or type(gates) is not dict
        or set(gates) != GATE_NAMES
        or gate.get("status") != inventory.get("gate_status")
    ):
        raise StageFinalizationError("finalization-gate-binding-mismatch")
    expected_gate_status = (
        "inconclusive"
        if any(value is None for value in gates.values())
        else "fail"
        if any(value is False for value in gates.values())
        else "pass"
        if all(value is True for value in gates.values())
        else "inconclusive"
    )
    if any(type(value) is not bool and value is not None for value in gates.values()):
        raise StageFinalizationError("finalization-gates-invalid")
    if gate.get("status") != expected_gate_status:
        raise StageFinalizationError("finalization-gate-status-mismatch")
    metrics = inventory.get("gate_metrics")
    if (
        type(metrics) is not dict
        or metrics.get("gates") != gates
        or metrics.get("status") != expected_gate_status
        or _sha(
            json.dumps(metrics, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()
        )
        != gate.get("metrics_sha256")
        or gate.get("quality_credit") is not False
        or gate.get("admission_created") is not False
    ):
        raise StageFinalizationError("finalization-gate-metrics-binding-mismatch")
    input_resources = gate_input.get("resource_evidence")
    if (
        gate_input.get("stage_uuid") != expected_stage_uuid
        or gate_input.get("scope") != "mixed-workload-only"
        or inventory.get("scope") != "mixed-workload-only"
        or inventory.get("gate_scope") != "mixed-workload-only"
        or gate_input.get("protocol_sha256") != expected_protocol_sha256
        or gate_input.get("cohorts_sha256") != expected_cohorts_sha256
        or gate_input.get("pending_time_receipt_sha256") != expected_pending_time_receipt_sha256
        or type(input_resources) is not dict
        or input_resources.get("stage_elapsed_seconds") != pending_elapsed
        or gate_input.get("resource_evidence_receipt_sha256") != inventory.get("resource_evidence_receipt_sha256")
    ):
        raise StageFinalizationError("finalization-gate-input-binding-mismatch")

    resource_path = stage_dir / "resource-evidence.json"
    resource_raw = _read(resource_path, 2_000_000)
    resource_sha = _sha(resource_raw)
    if resource_sha != inventory.get("resource_evidence_receipt_sha256"):
        raise StageFinalizationError("finalization-resource-receipt-pin-mismatch")
    resource = _json(resource_raw)
    collector_observations = resource.get("observations")
    expected_gate_resource_observations = (
        {**collector_observations, "stage_elapsed_seconds": pending_elapsed}
        if type(collector_observations) is dict
        else None
    )
    collector_receipt = {
        "schema": "coverage-resource-evidence/1",
        "stage_uuid": resource.get("stage_uuid"),
        "observations": resource.get("observations"),
        "provenance": resource.get("provenance"),
    }
    if (
        resource.get("schema") != "coverage-resource-evidence/2"
        or resource.get("stage_uuid") != expected_stage_uuid
        or resource.get("source_revision") != expected_source_revision
        or resource.get("protocol_sha256") != expected_protocol_sha256
        or resource.get("cohorts_sha256") != expected_cohorts_sha256
        or type(collector_observations) is not dict
        or input_resources != expected_gate_resource_observations
        or type(resource.get("provenance")) is not dict
        or inventory.get("resource_evidence_receipt_file") != resource_path.name
        or gate_input.get("resource_evidence_receipt_sha256") != resource_sha
        or not _SHA.fullmatch(resource.get("collector_receipt_sha256", ""))
        or _sha(
            json.dumps(
                collector_receipt, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False
            ).encode()
        )
        != resource.get("collector_receipt_sha256")
    ):
        raise StageFinalizationError("finalization-resource-evidence-binding-mismatch")

    closeout = coverage_resource_evidence.verify_stage_closeout(
        inventory_path=inventory_path,
        expected_inventory_sha256=expected_inventory_sha256,
        closeout_receipt_path=closeout_receipt_path,
        expected_closeout_receipt_sha256=expected_closeout_receipt_sha256,
        expected_stage_uuid=expected_stage_uuid,
        expected_source_revision=expected_source_revision,
        protocol_bytes=protocol_bytes,
        cohorts_bytes=cohorts_bytes,
        expected_protocol_sha256=expected_protocol_sha256,
        expected_cohorts_sha256=expected_cohorts_sha256,
    )
    closeout_document = _json(_read(closeout_receipt_path, 16_384))
    closeout_observed = closeout_document.get("observed_after_final_inventory_fsync_monotonic")
    if (
        closeout_document.get("stage_deadline_seconds") != stage_deadline
        or closeout_document.get("stage_started_monotonic") != pending_started
        or closeout_document.get("stage_started_monotonic") != inventory.get("stage_started_monotonic")
        or not _finite_nonnegative(closeout_observed)
        or closeout_observed < pending_observed
    ):
        raise StageFinalizationError("finalization-closeout-deadline-or-order-mismatch")
    if closeout.stage_elapsed_seconds < pending_elapsed:
        raise StageFinalizationError("finalization-time-order-invalid")
    if closeout.stage_elapsed_seconds >= stage_deadline:
        raise StageFinalizationError("finalization-late-stage")
    if (
        inventory.get("status") != "pending-independent-closeout"
        or inventory.get("gate_result_authoritative") is not False
    ):
        raise StageFinalizationError("finalization-inventory-not-pending")
    expected_reason = None if expected_gate_status == "pass" else f"gate-{expected_gate_status}"
    if inventory.get("terminal_reason") != expected_reason:
        raise StageFinalizationError("finalization-terminal-reason-mismatch")
    if gate.get("status") == "pass" and all(value is True for value in gates.values()):
        status = "gates-pass-awaiting-independent-decision"
        eligible = True
    elif gate.get("status") == "fail" and any(value is False for value in gates.values()):
        status = "gates-failed"
        eligible = False
    else:
        status = "gates-inconclusive"
        eligible = False
    receipt = {
        "schema": "coverage-stage-finalization/1",
        "stage_uuid": expected_stage_uuid,
        "source_revision": expected_source_revision,
        "protocol_sha256": expected_protocol_sha256,
        "cohorts_sha256": expected_cohorts_sha256,
        "inventory_sha256": expected_inventory_sha256,
        "pending_time_receipt_sha256": expected_pending_time_receipt_sha256,
        "gate_calculation_receipt_sha256": expected_gate_calculation_receipt_sha256,
        "closeout_receipt_sha256": expected_closeout_receipt_sha256,
        "pending_elapsed_seconds": pending_elapsed,
        "actual_elapsed_seconds": closeout.stage_elapsed_seconds,
        "actual_time_source": "verified-after-final-inventory-fsync-closeout-receipt",
        "gates": gates,
        "status": status,
        "eligible_for_independent_decision": eligible,
        "product_authorized": False,
        "admission_created": False,
    }
    receipt_bytes = json.dumps(receipt, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
    return FinalizationResult(
        status=status,
        stage_uuid=expected_stage_uuid,
        actual_elapsed_seconds=closeout.stage_elapsed_seconds,
        gates=dict(gates),
        finalization_receipt_sha256=_sha(receipt_bytes),
        receipt_bytes=receipt_bytes,
        eligible_for_independent_decision=eligible,
    )
