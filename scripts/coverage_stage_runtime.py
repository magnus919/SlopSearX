"""Concrete stage-executor wiring for the guarded coverage-study pipeline.

This module owns no admission, permit, endpoint, credential, or model authority.
The caller supplies those explicit values through narrowly scoped callbacks;
the existing acquisition, capture, selector, answer, closure, and finalizer
implementations remain the execution and validation boundaries.
"""

from __future__ import annotations

import hashlib
import inspect
import json
import os
import stat
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Awaitable, Callable, Mapping

from scripts import coverage_answer_execution as answer_execution
from scripts import coverage_live_acquire, coverage_native_grader_handoff, coverage_source_capture
from scripts import coverage_stage_orchestration as orchestration


class RuntimeWiringError(RuntimeError):
    """Explicit stage inputs or a typed execution result failed validation."""


async def _await(value: Any) -> Any:
    return await value if inspect.isawaitable(value) else value


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")


def _authority(value: object, required: set[str], optional: set[str], label: str) -> dict[str, Any]:
    if type(value) is not dict or not required <= set(value) or not set(value) <= required | optional:
        raise RuntimeWiringError(f"{label}-authority-shape-invalid")
    return value


def _read_private(path: Path, *, maximum_bytes: int) -> bytes:
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    try:
        metadata = os.fstat(descriptor)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or stat.S_IMODE(metadata.st_mode) & 0o077
            or metadata.st_size > maximum_bytes
        ):
            raise RuntimeWiringError("private-stage-artifact-invalid")
        with os.fdopen(descriptor, "rb", closefd=False) as stream:
            raw = stream.read(maximum_bytes + 1)
        if len(raw) > maximum_bytes:
            raise RuntimeWiringError("private-stage-artifact-cap")
        return raw
    finally:
        os.close(descriptor)


@dataclass(frozen=True)
class RuntimeBindings:
    """Explicit non-secret authority providers and stage-specific executors.

    Callbacks must read already qualified, externally issued artifacts. This
    factory does not mint permits, leases, selector admissions, or health proof.
    The production factory never accepts injected transports or resolvers.
    """

    acquisition_authority: Callable[[orchestration.StagePlan], Mapping[str, object] | Awaitable[Mapping[str, object]]]
    verify_acquisition: Callable[[orchestration.StagePlan, orchestration.AcquisitionEvidence], Awaitable[bool]]
    capture_authority: Callable[
        [orchestration.StagePlan, Mapping[str, object]], Mapping[str, object] | Awaitable[Mapping[str, object]]
    ]
    prepare_after_acquisition: Callable[
        [orchestration.StagePlan, orchestration.AcquisitionEvidence, Mapping[str, object]],
        Awaitable[orchestration.LatePreparationEvidence],
    ]
    verify_late_preflight: Callable[[orchestration.StagePlan, orchestration.LatePreparationEvidence], Awaitable[bool]]
    selector_permit_resolver: Callable[..., Awaitable[Mapping[str, tuple[bytes, str]]]]
    selector_admission_verifier: Callable[..., Awaitable[Any]]
    selector_evidence_builder: Callable[..., orchestration.SelectorEvidence]
    answer_authority: Callable[
        [orchestration.StagePlan, tuple[answer_execution.AnswerTask, ...]],
        Mapping[str, object] | Awaitable[Mapping[str, object]],
    ]
    selector_api_key: str
    selector_lease_root: str | os.PathLike[str]
    selector_archive_root: str | os.PathLike[str]
    selector_result_root: str | os.PathLike[str]
    native_handoff: coverage_native_grader_handoff.NativeGraderHandoff


def build_stage_executors(plan: orchestration.StagePlan, bindings: RuntimeBindings) -> orchestration.StageExecutors:
    """Wire existing guarded modules into the coordinator's ordered phase graph."""
    if type(plan) is not orchestration.StagePlan or type(bindings) is not RuntimeBindings:
        raise RuntimeWiringError("stage-plan-and-runtime-bindings-required")
    if (
        not callable(bindings.acquisition_authority)
        or not callable(bindings.verify_acquisition)
        or not callable(bindings.capture_authority)
        or not callable(bindings.prepare_after_acquisition)
        or not callable(bindings.verify_late_preflight)
        or not callable(bindings.selector_permit_resolver)
        or not callable(bindings.selector_admission_verifier)
        or not callable(bindings.selector_evidence_builder)
        or not callable(bindings.answer_authority)
        or type(bindings.selector_api_key) is not str
        or not bindings.selector_api_key
        or type(bindings.native_handoff) is not coverage_native_grader_handoff.NativeGraderHandoff
        or not bindings.native_handoff.require_native_host_transcripts
    ):
        raise RuntimeWiringError("runtime-binding-member-invalid")

    async def acquire(plan: orchestration.StagePlan) -> orchestration.AcquisitionEvidence:
        authority = _authority(
            await _await(bindings.acquisition_authority(plan)),
            {
                "permit_receipt_bytes",
                "expected_permit_receipt_sha256",
                "permit_verifier",
                "one_shot_lease",
                "receipt_directory",
            },
            set(),
            "acquisition",
        )
        manifest = json.loads(plan.acquisition_manifest_bytes)
        result = await coverage_live_acquire.acquire_live_coverage_stage(
            stage_manifest=manifest,
            acquisition_plan_bytes=plan.acquisition_plan_bytes,
            permit_receipt_bytes=authority["permit_receipt_bytes"],
            expected_permit_receipt_sha256=authority["expected_permit_receipt_sha256"],
            permit_verifier=authority["permit_verifier"],
            one_shot_lease=authority["one_shot_lease"],
            receipt_directory=authority["receipt_directory"],
        )
        evidence = orchestration.AcquisitionEvidence(
            result=result.stage,
            snapshots_directory=result.receipt_directory,
            snapshot_index_sha256=result.pool_snapshot_index_sha256,
            acquisition_manifest_sha256=result.stage_manifest_sha256,
            receipt_sha256=result.permit_receipt_sha256,
            source_bound_receipt_status="externally-verified",
        )
        orchestration._validate_acquisition(plan, evidence)
        return evidence

    async def capture(
        plan: orchestration.StagePlan, pipeline_inputs: Mapping[str, object]
    ) -> orchestration.CaptureEvidence:
        authority = _authority(
            await _await(bindings.capture_authority(plan, pipeline_inputs)),
            {
                "candidate_base_url",
                "operator_token",
                "permit_receipt_bytes",
                "expected_permit_receipt_sha256",
                "permit_verifier",
                "one_shot_lease",
                "receipt_root",
            },
            {
                "qualification_receipt_bytes",
                "expected_qualification_receipt_sha256",
                "qualification_verifier",
                "ca_bundle_pem_bytes",
                "expected_ca_bundle_sha256",
            },
            "capture",
        )
        result = await coverage_source_capture.capture_sources_once(
            protocol_bytes=plan.protocol_bytes,
            source_manifest_bytes=pipeline_inputs["capture_manifest_bytes"],
            candidate_identity_bytes=plan.candidate_identity_bytes,
            candidate_base_url=authority["candidate_base_url"],
            operator_token=authority["operator_token"],
            permit_receipt_bytes=authority["permit_receipt_bytes"],
            expected_permit_receipt_sha256=authority["expected_permit_receipt_sha256"],
            permit_verifier=authority["permit_verifier"],
            one_shot_lease=authority["one_shot_lease"],
            receipt_root=Path(authority["receipt_root"]),
            transport=None,
            stage_started_monotonic=plan.stage_started_monotonic,
            stage_deadline_monotonic=plan.stage_deadline_monotonic,
            qualification_receipt_bytes=authority.get("qualification_receipt_bytes"),
            expected_qualification_receipt_sha256=authority.get("expected_qualification_receipt_sha256"),
            qualification_verifier=authority.get("qualification_verifier"),
            ca_bundle_pem_bytes=authority.get("ca_bundle_pem_bytes"),
            expected_ca_bundle_sha256=authority.get("expected_ca_bundle_sha256"),
        )
        inventory_path = result.receipt_directory / "inventory.json"
        inventory_bytes = _read_private(inventory_path, maximum_bytes=4_000_000)
        contexts: dict[str, bytes] = {}
        for row in result.private_inventory:
            artifact_name = row.get("context_artifact")
            if artifact_name is not None:
                if type(artifact_name) is not str or Path(artifact_name).name != artifact_name:
                    raise RuntimeWiringError("capture-context-artifact-name-invalid")
                contexts[artifact_name] = _read_private(
                    result.receipt_directory / artifact_name, maximum_bytes=1_000_000
                )
        return orchestration.CaptureEvidence(
            result=result,
            inventory_bytes=inventory_bytes,
            context_artifacts=contexts,
            receipt_sha256=_sha(inventory_bytes),
        )

    async def answerer(
        plan: orchestration.StagePlan, tasks: tuple[answer_execution.AnswerTask, ...]
    ) -> orchestration.AnswerEvidence:
        authority = _authority(
            await _await(bindings.answer_authority(plan, tuple(tasks))),
            {
                "endpoint",
                "api_key",
                "permit_bytes",
                "expected_permit_sha256",
                "permit_verifier",
                "one_shot_lease",
                "lease_root",
                "archive_root",
                "result_root",
                "stage_started_utc",
                "stage_deadline_utc",
                "answer_manifest_sha256",
            },
            {"allow_trusted_private_http"},
            "answer",
        )
        endpoint_mode = answer_execution._chat_url(
            authority["endpoint"],
            allow_trusted_private_http=authority.get("allow_trusted_private_http", False),
        )[1]
        requests = answer_execution._make_requests(tasks)
        _manifest, manifest_sha = answer_execution._request_manifest(requests, endpoint_security_mode=endpoint_mode)
        if manifest_sha != authority["answer_manifest_sha256"]:
            raise RuntimeWiringError("answer-operation-manifest-authority-mismatch")
        result = await answer_execution.execute_answer_stage(
            stage_uuid=plan.stage_uuid,
            source_revision=plan.source_revision,
            protocol_sha256=_sha(plan.protocol_bytes),
            cohorts_sha256=_sha(plan.cohorts_bytes),
            answer_manifest_sha256=manifest_sha,
            stage_started_utc=authority["stage_started_utc"],
            stage_deadline_utc=authority["stage_deadline_utc"],
            tasks=tasks,
            endpoint=authority["endpoint"],
            allow_trusted_private_http=authority.get("allow_trusted_private_http", False),
            api_key=authority["api_key"],
            permit_bytes=authority["permit_bytes"],
            expected_permit_sha256=authority["expected_permit_sha256"],
            permit_verifier=authority["permit_verifier"],
            one_shot_lease=authority["one_shot_lease"],
            lease_root=authority["lease_root"],
            archive_root=authority["archive_root"],
            result_root=authority["result_root"],
        )
        archive_root = Path(authority["archive_root"])
        result_root = Path(authority["result_root"])
        terminal_path = result_root / f"{plan.stage_uuid}.answer-terminal-inventory.json"
        terminal_bytes = _read_private(terminal_path, maximum_bytes=4_000_000)
        restored: dict[str, dict[str, object]] = {task.task_id: {} for task in tasks}
        input_bodies: dict[str, dict[str, bytes]] = {task.task_id: {} for task in tasks}
        for task in tasks:
            for answer in task.answers:
                operation_id = f"answer-{task.task_id}-{answer.arm}"
                path = result_root / f"{plan.stage_uuid}.{operation_id}.answer.json"
                restored[task.task_id][answer.arm] = answer_execution._strict_json(
                    _read_private(path, maximum_bytes=2_000_000)
                )
                input_bodies[task.task_id][answer.arm] = answer.body
        return orchestration.AnswerEvidence(
            result=result,
            answer_inputs=input_bodies,
            restored_answers=restored,
            terminal_inventory_bytes=terminal_bytes,
            archive_root=archive_root,
            result_root=result_root,
        )

    async def grade_references(prepared_packets):
        return await bindings.native_handoff.collect(
            stage_uuid=plan.packet_stage_uuid,
            phase="references",
            prepared_packets=prepared_packets,
            deadline_monotonic=plan.stage_deadline_monotonic,
        )

    async def grade_answers(prepared_packets):
        return await bindings.native_handoff.collect(
            stage_uuid=plan.packet_stage_uuid,
            phase="answers",
            prepared_packets=prepared_packets,
            deadline_monotonic=plan.stage_deadline_monotonic,
        )

    selector = orchestration.registered_selector_executor_factory(
        permit_resolver=bindings.selector_permit_resolver,
        api_key=bindings.selector_api_key,
        lease_root=bindings.selector_lease_root,
        archive_root=bindings.selector_archive_root,
        result_root=bindings.selector_result_root,
        evidence_builder=bindings.selector_evidence_builder,
    )
    return orchestration.StageExecutors(
        acquire=acquire,
        verify_acquisition=bindings.verify_acquisition,
        capture=capture,
        prepare_after_acquisition=bindings.prepare_after_acquisition,
        verify_late_preflight=bindings.verify_late_preflight,
        grade_references=grade_references,
        selectors=selector,
        answerer=answerer,
        grade_answers=grade_answers,
        verify_selector_admission=bindings.selector_admission_verifier,
    )
