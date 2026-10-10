"""Verify an externally sealed post-acquisition registration bundle.

Only pool-derived task-input and source-capture material pins may change from
the pre-acquisition registration. All frozen study identity, task assignment,
source closure, budgets (inside the pinned protocol), and plans stay fixed.
The helper verifies an external late registration; it never creates one.
"""

from __future__ import annotations

import hashlib
import re
from dataclasses import dataclass
from typing import Mapping

from scripts import coverage_live_acquire
from scripts import coverage_pipeline_inputs as pipeline
from scripts import coverage_study_core as core
from scripts.intent_ranking_coverage import compile_request

_SHA = re.compile(r"[0-9a-f]{64}\Z")
_LATE_MATERIALS = frozenset({"task_input_manifest", "source_capture_manifest"})


class LateRegistrationError(ValueError):
    """The post-acquisition registration changed frozen study identity."""


@dataclass(frozen=True)
class VerifiedLateRegistration:
    prepared: core.PreparedStage
    registration_sha256: str
    input_manifest_bytes: bytes
    source_capture_manifest_bytes: bytes


def build_postacquisition_materials(
    *,
    plan: object,
    acquisition_evidence: object,
    pipeline_inputs: Mapping[str, object],
    static_materials: Mapping[str, bytes],
) -> dict[str, bytes]:
    """Rebuild the exact late core materials from verified pool snapshots.

    This returns data for an external late registration request. It never
    chooses or changes cohort tasks, candidate pools, source closure, or gates.
    """
    from scripts import coverage_stage_orchestration as orchestration

    if (
        type(plan) is not orchestration.StagePlan
        or type(acquisition_evidence) is not orchestration.AcquisitionEvidence
        or type(pipeline_inputs) is not dict
        or type(static_materials) is not dict
        or set(static_materials) != set(core.PIN_NAMES) - _LATE_MATERIALS
        or pipeline_inputs.get("stage_uuid") != plan.stage_uuid
        or type(pipeline_inputs.get("capture_manifest_bytes")) is not bytes
    ):
        raise LateRegistrationError("postacquisition-material-input-invalid")
    index = core._sha256(acquisition_evidence.snapshot_index_sha256, "acquisition-index")
    try:
        verified = coverage_live_acquire.verify_pool_snapshot_index(
            acquisition_evidence.snapshots_directory,
            expected_index_sha256=index,
            expected_stage_manifest_sha256=plan.expected_acquisition_manifest_sha256,
            expected_stage_uuid=plan.stage_uuid,
            expected_source_revision=plan.source_revision,
        )
    except Exception as exc:
        raise LateRegistrationError("postacquisition-snapshot-invalid") from exc
    tasks = pipeline_inputs.get("tasks")
    if type(tasks) is not list or len(tasks) != 8:
        raise LateRegistrationError("postacquisition-task-inventory")
    research_tasks = []
    for task in tasks:
        if type(task) is not dict:
            raise LateRegistrationError("postacquisition-task-row")
        candidates = task.get("ranking_candidates")
        native_order = task.get("native_order")
        try:
            compiled = compile_request(
                query=task["query"],
                purpose=task["purpose"],
                facets=task["facets"],
                candidates=candidates,
                incumbent_order=native_order,
            )
        except Exception as exc:
            raise LateRegistrationError("postacquisition-task-input-invalid") from exc
        research_tasks.append(
            {
                "task_id": task["task_id"],
                "query": task["query"],
                "purpose": task["purpose"],
                "facets": task["facets"],
                "candidates": candidates,
                "incumbent_order": native_order,
                "compiled_request_sha256": _sha(compiled.body),
            }
        )
    target_rows = list(plan.navigation_targets)
    if len(target_rows) != 5:
        raise LateRegistrationError("postacquisition-navigation-count")
    operations = verified["operations"]
    navigation_tasks = []
    for target in target_rows:
        target_id = target["target_id"]
        artifact = operations.get(target_id)
        operation = artifact.get("operation") if type(artifact) is dict else None
        response = operation.get("canonical_response") if type(operation) is dict else None
        rows = response.get("results") if type(response) is dict else None
        if type(rows) is not list or not rows:
            raise LateRegistrationError("postacquisition-navigation-pool-invalid")
        pool_ids = [f"n{index}" for index in range(len(rows))]
        target_url = target.get("target_url")
        try:
            target_canonical = pipeline._canonical_public_url(target_url)[1]
            matching_ids = [
                pool_ids[index]
                for index, row in enumerate(rows)
                if type(row) is dict and pipeline._canonical_public_url(row.get("url"))[1] == target_canonical
            ]
        except Exception as exc:
            raise LateRegistrationError("postacquisition-navigation-target-invalid") from exc
        if len(matching_ids) != 1 or response.get("query") != target.get("search_query"):
            raise LateRegistrationError("postacquisition-navigation-target-not-acquired")
        navigation_tasks.append(
            {
                "task_id": target_id,
                "query": target["search_query"],
                "target_candidate_id": matching_ids[0],
                "pool_ids": pool_ids,
                "w0_order": pool_ids,
            }
        )
    input_bytes = core._canonical(
        {
            "schema": core.INPUT_SCHEMA,
            "stage_uuid": plan.stage_uuid,
            "research_tasks": research_tasks,
            "navigation_tasks": navigation_tasks,
        }
    )
    if (
        static_materials["protocol"] != plan.protocol_bytes
        or static_materials["acquisition_plan"] != plan.acquisition_plan_bytes
        or _sha(static_materials["qualified_source_closure"]) != plan.source_closure_sha256
    ):
        raise LateRegistrationError("postacquisition-static-binding-mismatch")
    materials = dict(static_materials)
    materials["task_input_manifest"] = input_bytes
    capture_bytes = pipeline_inputs.get("capture_manifest_bytes")
    if type(capture_bytes) is not bytes:
        raise LateRegistrationError("postacquisition-capture-manifest-invalid")
    materials["source_capture_manifest"] = capture_bytes
    return materials


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _registration(raw: bytes, expected_sha256: str) -> dict[str, object]:
    if type(raw) is not bytes or not raw or type(expected_sha256) is not str or not _SHA.fullmatch(expected_sha256):
        raise LateRegistrationError("late-registration-input-invalid")
    if _sha(raw) != expected_sha256:
        raise LateRegistrationError("late-registration-pin-mismatch")
    value = core._strict_json(raw, "late-registration")
    if type(value) is not dict or core._canonical(value) != raw:
        raise LateRegistrationError("late-registration-noncanonical")
    return value


def verify_late_registration(
    *,
    initial_registration_bytes: bytes,
    expected_initial_registration_sha256: str,
    late_registration_bytes: bytes,
    expected_late_registration_sha256: str,
    materials: Mapping[str, bytes],
    expected_source_revision: str,
    expected_stage_uuid: str,
    forbidden_stage_uuids: tuple[str, ...],
    task_ids: tuple[str, ...],
    navigation_skip_reasons: tuple[str | None, ...],
) -> VerifiedLateRegistration:
    """Validate lineage and run the original pure registration preflight."""
    initial = _registration(initial_registration_bytes, expected_initial_registration_sha256)
    late = _registration(late_registration_bytes, expected_late_registration_sha256)
    if set(initial) != {
        "schema",
        "stage_uuid",
        "stage_kind",
        "source_revision",
        "pins",
        "operation_ids",
        "navigation_skip_reasons",
        "task_ids",
    }:
        raise LateRegistrationError("initial-registration-shape")
    if set(late) != set(initial):
        raise LateRegistrationError("late-registration-shape")
    if type(initial.get("pins")) is not dict or type(late.get("pins")) is not dict:
        raise LateRegistrationError("late-registration-pins-shape")
    if set(initial["pins"]) != set(core.PIN_NAMES) or set(late["pins"]) != set(core.PIN_NAMES):
        raise LateRegistrationError("late-registration-pins-inventory")
    immutable_fields = {key for key in initial if key != "pins"}
    if any(initial[key] != late[key] for key in immutable_fields):
        raise LateRegistrationError("late-registration-lineage-drift")
    for name in core.PIN_NAMES:
        if name not in _LATE_MATERIALS and initial["pins"][name] != late["pins"][name]:
            raise LateRegistrationError(f"late-registration-frozen-pin-drift:{name}")
    if type(materials) is not dict or set(materials) != set(core.PIN_NAMES):
        raise LateRegistrationError("late-registration-material-inventory")
    pins = late["pins"]
    for name in core.PIN_NAMES:
        raw = materials[name]
        if type(raw) is not bytes or _sha(raw) != pins[name]:
            raise LateRegistrationError(f"late-registration-material-pin-mismatch:{name}")
    try:
        prepared = core.preflight_stage(
            registration_bytes=late_registration_bytes,
            expected_registration_sha256=expected_late_registration_sha256,
            expected_stage_uuid=expected_stage_uuid,
            forbidden_stage_uuids=forbidden_stage_uuids,
            expected_source_revision=expected_source_revision,
            observed_source_revision=expected_source_revision,
            materials=materials,
            expected_pins=pins,
            acquisition_plan_bytes=materials["acquisition_plan"],
            capture_plan_bytes=materials["capture_plan"],
            task_ids=task_ids,
            navigation_skip_reasons=navigation_skip_reasons,
        )
    except Exception as exc:
        raise LateRegistrationError("late-registration-core-preflight-failed") from exc
    return VerifiedLateRegistration(
        prepared=prepared,
        registration_sha256=expected_late_registration_sha256,
        input_manifest_bytes=materials["task_input_manifest"],
        source_capture_manifest_bytes=materials["source_capture_manifest"],
    )


async def request_late_registration(
    *,
    exchange: object,
    plan: object,
    acquisition_evidence: object,
    pipeline_inputs: Mapping[str, object],
    static_materials: Mapping[str, bytes],
    initial_registration_bytes: bytes,
    expected_initial_registration_sha256: str,
    forbidden_stage_uuids: tuple[str, ...],
    task_ids: tuple[str, ...],
    navigation_skip_reasons: tuple[str | None, ...],
) -> object:
    """Wait for an external late registration, then run core preflight.

    The exchange expects a separate pin file in its private operator root.
    This function never writes a registration and never turns preflight into
    execution authorization.
    """
    from scripts import coverage_operator_handoff
    from scripts import coverage_stage_orchestration as orchestration

    if type(exchange) is not coverage_operator_handoff.OperatorReceiptHandoff:
        raise LateRegistrationError("late-registration-operator-exchange-required")
    if type(plan) is not orchestration.StagePlan or type(acquisition_evidence) is not orchestration.AcquisitionEvidence:
        raise LateRegistrationError("late-registration-stage-context-invalid")
    materials = build_postacquisition_materials(
        plan=plan,
        acquisition_evidence=acquisition_evidence,
        pipeline_inputs=pipeline_inputs,
        static_materials=static_materials,
    )
    immutable = _registration(initial_registration_bytes, expected_initial_registration_sha256)
    request_bindings = {
        "initial_registration_sha256": expected_initial_registration_sha256,
        "stage_uuid": plan.stage_uuid,
        "source_revision": plan.source_revision,
        "source_closure_sha256": plan.source_closure_sha256,
        "protocol_sha256": _sha(plan.protocol_bytes),
        "cohorts_sha256": _sha(plan.cohorts_bytes),
        "acquisition_plan_sha256": _sha(plan.acquisition_plan_bytes),
        "acquisition_manifest_sha256": acquisition_evidence.acquisition_manifest_sha256,
        "snapshot_index_sha256": acquisition_evidence.snapshot_index_sha256,
        "task_ids": list(task_ids),
        "operation_ids": immutable.get("operation_ids"),
        "navigation_skip_reasons": list(navigation_skip_reasons),
        "late_material_sha256": {name: _sha(materials[name]) for name in sorted(_LATE_MATERIALS)},
    }
    try:
        registration_bytes, registration_sha256 = await exchange.request_async(
            stage_uuid=plan.stage_uuid,
            scope="late-registration",
            request_id="registration-1",
            bindings=request_bindings,
            deadline_monotonic=plan.stage_deadline_monotonic,
        )
    except Exception as exc:
        raise LateRegistrationError("late-registration-handoff-failed") from exc
    verified = verify_late_registration(
        initial_registration_bytes=initial_registration_bytes,
        expected_initial_registration_sha256=expected_initial_registration_sha256,
        late_registration_bytes=registration_bytes,
        expected_late_registration_sha256=registration_sha256,
        materials=materials,
        expected_source_revision=plan.source_revision,
        expected_stage_uuid=plan.stage_uuid,
        forbidden_stage_uuids=forbidden_stage_uuids,
        task_ids=task_ids,
        navigation_skip_reasons=navigation_skip_reasons,
    )
    return orchestration.LatePreparationEvidence(
        prepared=verified.prepared,
        input_manifest_bytes=verified.input_manifest_bytes,
        preflight_receipt_sha256=verified.registration_sha256,
    )
