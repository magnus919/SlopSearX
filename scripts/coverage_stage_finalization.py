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

from scripts import coverage_atomic_io, coverage_resource_evidence
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
    expected_band_companion_sha256: str | None = None,
    expected_source_closure_sha256: str | None = None,
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
    if expected_band_companion_sha256 is not None and not _SHA.fullmatch(expected_band_companion_sha256):
        raise StageFinalizationError("finalization-companion-pin-invalid")
    if expected_source_closure_sha256 is not None and not _SHA.fullmatch(expected_source_closure_sha256):
        raise StageFinalizationError("finalization-source-closure-pin-invalid")
    if _sha(protocol_bytes) != expected_protocol_sha256 or _sha(cohorts_bytes) != expected_cohorts_sha256:
        raise StageFinalizationError("finalization-source-pin-mismatch")
    try:
        protocol = _strict_loads(protocol_bytes)
    except StageFinalizationError as exc:
        raise StageFinalizationError("finalization-protocol-invalid") from exc
    if type(protocol) is not dict:
        raise StageFinalizationError("finalization-protocol-invalid")
    if protocol.get("schema") not in {
        "coverage-first-study-protocol/2",
        "coverage-first-study-protocol/3",
        "coverage-first-study-protocol/4",
    } or protocol.get("status") not in {
        "draft_not_registered_not_admitted",
        "registered",
    }:
        raise StageFinalizationError("finalization-protocol-not-v2")
    if protocol.get("schema") == "coverage-first-study-protocol/4":
        quality = protocol.get("quality")
        if (
            type(quality) is not dict
            or quality.get("useful_count_role") != "diagnostic_only"
            or "useful_count_loss_allowed" in quality
        ):
            raise StageFinalizationError("finalization-useful-count-policy-invalid")
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
    companion_sha = gate.get("band_companion_sha256")
    companion_status = gate.get("band_companion_status")
    companion_gates = gate.get("band_companion_gates")
    if expected_band_companion_sha256 is None:
        if (
            any(key in gate for key in ("band_companion_sha256", "band_companion_status", "band_companion_gates"))
            or companion_status is not None
            or companion_gates is not None
            or any(
                key in inventory
                for key in ("band_companion_sha256", "band_companion_status", "release_qualification_status")
            )
            or "band_companion_sha256" in gate_input
        ):
            raise StageFinalizationError("finalization-companion-registration-required")
    else:
        expected_companion_gates = {
            f"band_{band}_{assessor}_{kind}"
            for band in ("research_le_40", "research_41_80")
            for assessor in ("A", "B")
            for kind in ("ranking",)
        } | {
            f"band_{band}_{assessor}_task_use"
            for band in ("research_le_40", "research_41_80")
            for assessor in ("R1", "R2")
        }
        if (
            companion_sha != expected_band_companion_sha256
            or inventory.get("band_companion_sha256") != expected_band_companion_sha256
            or gate_input.get("band_companion_sha256") != expected_band_companion_sha256
            or companion_status not in {"pass", "fail", "inconclusive"}
            or inventory.get("band_companion_status") != companion_status
            or type(companion_gates) is not dict
            or set(companion_gates) != expected_companion_gates
            or any(type(value) is not bool and value is not None for value in companion_gates.values())
        ):
            raise StageFinalizationError("finalization-companion-binding-mismatch")
        expected_companion_status = (
            "inconclusive"
            if any(value is None for value in companion_gates.values())
            else "fail"
            if any(value is False for value in companion_gates.values())
            else "pass"
        )
        if companion_status != expected_companion_status:
            raise StageFinalizationError("finalization-companion-status-mismatch")
    qualification_status = expected_gate_status
    if expected_band_companion_sha256 is not None:
        if "release_qualification_status" not in inventory:
            raise StageFinalizationError("finalization-combined-qualification-missing")
        qualification_status = (
            "inconclusive"
            if expected_gate_status == "inconclusive" or companion_status == "inconclusive"
            else "fail"
            if expected_gate_status == "fail" or companion_status == "fail"
            else "pass"
        )
    if inventory.get("release_qualification_status", expected_gate_status) != qualification_status:
        raise StageFinalizationError("finalization-combined-qualification-mismatch")
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
    metrics_companion = metrics.get("band_companion")
    if expected_band_companion_sha256 is None:
        if "band_companion" in metrics:
            raise StageFinalizationError("finalization-companion-registration-required")
    else:
        if (
            type(metrics_companion) is not dict
            or metrics_companion.get("status") != companion_status
            or metrics_companion.get("gates") != companion_gates
            or type(metrics_companion.get("groups")) is not dict
            or set(metrics_companion["groups"]) != {"research_le_40", "research_41_80"}
        ):
            raise StageFinalizationError("finalization-companion-metrics-mismatch")
        for band, group in metrics_companion["groups"].items():
            values = [companion_gates[f"band_{band}_{assessor}_ranking"] for assessor in ("A", "B")] + [
                companion_gates[f"band_{band}_{assessor}_task_use"] for assessor in ("R1", "R2")
            ]
            group_status = (
                "inconclusive"
                if any(value is None for value in values)
                else "fail"
                if any(value is False for value in values)
                else "pass"
            )
            if type(group) is not dict or group.get("status") != group_status:
                raise StageFinalizationError("finalization-companion-group-status-mismatch")
    input_resources = gate_input.get("resource_evidence")
    if (
        gate_input.get("stage_uuid") != expected_stage_uuid
        or gate_input.get("scope") != "mixed-workload-only"
        or inventory.get("scope") != "mixed-workload-only"
        or inventory.get("gate_scope") != "mixed-workload-only"
        or gate_input.get("protocol_sha256") != expected_protocol_sha256
        or gate_input.get("cohorts_sha256") != expected_cohorts_sha256
        or (
            expected_source_closure_sha256 is not None
            and gate_input.get("source_closure_sha256") != expected_source_closure_sha256
        )
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
        or (
            expected_source_closure_sha256 is not None
            and resource["provenance"].get("selector_source_closure_sha256") != expected_source_closure_sha256
        )
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
    expected_reason = None if qualification_status == "pass" else f"gate-{qualification_status}"
    if inventory.get("terminal_reason") != expected_reason:
        raise StageFinalizationError("finalization-terminal-reason-mismatch")
    if qualification_status == "pass" and all(value is True for value in gates.values()):
        status = "gates-pass-awaiting-independent-decision"
        eligible = True
    elif qualification_status == "fail":
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
        **(
            {"source_closure_sha256": expected_source_closure_sha256}
            if expected_source_closure_sha256 is not None
            else {}
        ),
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
    if expected_band_companion_sha256 is not None:
        receipt.update(
            {
                "band_companion_sha256": expected_band_companion_sha256,
                "band_companion_status": companion_status,
                "band_companion_gates": companion_gates,
                "release_qualification_status": qualification_status,
            }
        )
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


def issue_development_pass_receipt(
    *,
    plan: object,
    inventory_path: Path,
    expected_inventory_sha256: str,
    closeout_receipt_path: Path,
    expected_closeout_receipt_sha256: str,
    output_path: Path,
    expected_band_companion_sha256: str | None = None,
) -> bytes:
    """Re-verify a closed development stage, then seal its independent pass.

    ``plan`` must be the statically preflighted plan returned by
    ``OperatorStageRunner.preflight_from_file``. The inventory and closeout
    pins must be carried forward from the completed coordinator result. This
    function never dispatches work or grants product authorization.
    """
    stage_uuid = getattr(plan, "stage_uuid", None)
    stage_kind = getattr(plan, "stage_kind", None)
    source_revision = getattr(plan, "source_revision", None)
    source_closure_sha256 = getattr(plan, "source_closure_sha256", None)
    protocol_bytes = getattr(plan, "protocol_bytes", None)
    cohorts_bytes = getattr(plan, "cohorts_bytes", None)
    if (
        stage_kind != "development"
        or type(stage_uuid) is not str
        or type(source_revision) is not str
        or type(source_closure_sha256) is not str
        or not _SHA.fullmatch(source_closure_sha256)
        or type(protocol_bytes) is not bytes
        or type(cohorts_bytes) is not bytes
    ):
        raise StageFinalizationError("development-pass-plan-invalid")
    # Registration status lives in the source-bound operator config and its
    # initial registration, not in the immutable protocol document's status
    # label. The CLI obtains this plan from the runner's static preflight.
    registered_companion_sha256 = getattr(plan, "band_companion_sha256", None)
    if registered_companion_sha256 is not None:
        if not _SHA.fullmatch(registered_companion_sha256):
            raise StageFinalizationError("development-pass-companion-pin-invalid")
        if expected_band_companion_sha256 is not None and expected_band_companion_sha256 != registered_companion_sha256:
            raise StageFinalizationError("development-pass-companion-pin-mismatch")
        expected_band_companion_sha256 = registered_companion_sha256
    elif expected_band_companion_sha256 is not None:
        raise StageFinalizationError("development-pass-companion-not-registered")
    if (
        type(expected_inventory_sha256) is not str
        or not _SHA.fullmatch(expected_inventory_sha256)
        or type(expected_closeout_receipt_sha256) is not str
        or not _SHA.fullmatch(expected_closeout_receipt_sha256)
    ):
        raise StageFinalizationError("development-pass-outer-pin-invalid")

    # The outer inventory pin makes its child digests authoritative. The
    # verifier still checks every fixed filename, digest and cross-artifact
    # binding before any receipt is written.
    stage_dir = inventory_path.parent
    inventory_raw = _read(inventory_path, 4_000_000)
    if _sha(inventory_raw) != expected_inventory_sha256:
        raise StageFinalizationError("finalization-inventory-pin-mismatch")
    inventory = _json(inventory_raw)
    if (
        inventory.get("stage_kind") != "development"
        or inventory.get("stage_uuid") != stage_uuid
        or inventory.get("source_revision") != source_revision
        or inventory.get("protocol_sha256") != _sha(protocol_bytes)
        or inventory.get("cohorts_sha256") != _sha(cohorts_bytes)
    ):
        raise StageFinalizationError("development-pass-inventory-identity-mismatch")
    names = (
        inventory.get("pending_time_receipt_file"),
        inventory.get("gate_input_manifest_file"),
        inventory.get("gate_calculation_receipt_file"),
    )
    if names != (
        "stage-pending-time.json",
        "gate-input-manifest.json",
        "gate-calculation-receipt.json",
    ):
        raise StageFinalizationError("development-pass-inventory-artifact-names-invalid")
    finalization = verify_stage_finalization(
        inventory_path=inventory_path,
        expected_inventory_sha256=expected_inventory_sha256,
        closeout_receipt_path=closeout_receipt_path,
        expected_closeout_receipt_sha256=expected_closeout_receipt_sha256,
        pending_time_receipt_path=stage_dir / names[0],
        expected_pending_time_receipt_sha256=inventory.get("pending_time_receipt_sha256"),
        gate_input_manifest_path=stage_dir / names[1],
        expected_gate_input_manifest_sha256=inventory.get("gate_input_manifest_sha256"),
        gate_calculation_receipt_path=stage_dir / names[2],
        expected_gate_calculation_receipt_sha256=inventory.get("gate_calculation_receipt_sha256"),
        expected_stage_uuid=stage_uuid,
        expected_source_revision=source_revision,
        protocol_bytes=protocol_bytes,
        cohorts_bytes=cohorts_bytes,
        expected_protocol_sha256=_sha(protocol_bytes),
        expected_cohorts_sha256=_sha(cohorts_bytes),
        expected_band_companion_sha256=expected_band_companion_sha256,
        expected_source_closure_sha256=source_closure_sha256,
    )
    if (
        not finalization.eligible_for_independent_decision
        or finalization.status != "gates-pass-awaiting-independent-decision"
        or finalization.stage_uuid != stage_uuid
    ):
        raise StageFinalizationError("development-pass-not-eligible")

    from scripts import coverage_study_core

    receipt = {
        "schema": coverage_study_core.DEVELOPMENT_RECEIPT_SCHEMA,
        "stage_uuid": stage_uuid,
        "stage_kind": "development",
        "status": "passed",
        "protocol_sha256": _sha(protocol_bytes),
        "source_revision": source_revision,
        "source_closure_sha256": source_closure_sha256,
        "result_sha256": finalization.finalization_receipt_sha256,
    }
    raw = json.dumps(receipt, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    try:
        coverage_resource_evidence._require_private_directory(output_path.parent)
        coverage_atomic_io.write_new(output_path, raw)
    except FileExistsError as exc:
        raise StageFinalizationError("development-pass-output-already-exists-no-resume") from exc
    except (OSError, coverage_resource_evidence.ResourceEvidenceError) as exc:
        raise StageFinalizationError("development-pass-output-write-failed") from exc
    return raw


def main(argv: list[str] | None = None) -> int:
    """One-shot independent issuer CLI; it does not start or dispatch a stage."""
    import argparse
    import sys

    if (
        getattr(sys, "_coverage_operator_source_bootstrap", None) != "coverage-source-bootstrap/1"
        or not sys.dont_write_bytecode
    ):
        print(json.dumps({"status": "refused-or-terminal", "error_class": "source-bootstrap-required"}, sort_keys=True))
        return 1

    parser = argparse.ArgumentParser(description="Independently seal a registered development-stage pass.")
    parser.add_argument("--config", required=True)
    parser.add_argument("--config-sha256", required=True)
    parser.add_argument("--inventory-sha256", required=True, help="out-of-band pin returned by the completed run")
    parser.add_argument("--closeout-sha256", required=True, help="out-of-band pin returned by the completed run")
    parser.add_argument("--output", required=True)
    parser.add_argument("--band-companion-sha256")
    args = parser.parse_args(argv)
    try:
        from scripts.coverage_operator_runner import OperatorStageRunner

        runner = OperatorStageRunner.preflight_from_file(args.config, args.config_sha256)
        if runner.plan.stage_kind != "development":
            raise StageFinalizationError("development-pass-plan-invalid")
        stage_dir = Path(runner.directories["stage_inventory"]) / runner.plan.stage_uuid
        issue_development_pass_receipt(
            plan=runner.plan,
            inventory_path=stage_dir / "inventory.json",
            expected_inventory_sha256=args.inventory_sha256,
            closeout_receipt_path=stage_dir / "stage-closeout.json",
            expected_closeout_receipt_sha256=args.closeout_sha256,
            output_path=Path(args.output),
            expected_band_companion_sha256=args.band_companion_sha256,
        )
    except Exception as exc:
        print(json.dumps({"status": "refused-or-terminal", "error_class": type(exc).__name__}, sort_keys=True))
        return 1
    print(json.dumps({"status": "development-pass-receipt-issued", "product_authorized": False}, sort_keys=True))
    return 0
