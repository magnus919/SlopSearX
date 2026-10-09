"""Ordered, receipt-bound coordinator for one coverage-study stage.

The coordinator composes the existing offline preparation modules with
caller-injected acquisition, capture, selector, answer and assessor executors.
It has no transport, credential, admission, retry, or product-deployment path of
its own. A typed executor result is not an authorization or scientific claim;
the external admission and source qualification prerequisites remain required.
"""

from __future__ import annotations

import hashlib
import inspect
import json
import os
import re
import stat
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Awaitable, Callable, Mapping, Sequence

from scripts import coverage_answer_execution as answer_execution
from scripts import coverage_assessment_packets as packets
from scripts import coverage_consumer_inputs as consumer
from scripts import coverage_live_acquire
from scripts import coverage_pipeline_inputs as pipeline
from scripts import coverage_source_capture as source_capture
from scripts import coverage_study_acquire as acquisition
from scripts import coverage_study_core as core
from scripts import intent_ranking_receipts as receipts

SCHEMA = "coverage-stage-orchestration-inventory/1"
MAX_STAGE_WALL_SECONDS = 28_800.0
_SHA256 = re.compile(r"[0-9a-f]{64}\Z")

PHASES = (
    "acquisition",
    "pipeline_inputs",
    "late_preflight",
    "capture",
    "reference_packet_preparation",
    "reference_grading",
    "reference_grade_closure",
    "selector",
    "paired_answer_inputs",
    "answerer",
    "answerer_dispatch_verification",
    "answer_packet_preparation",
    "answer_grading",
    "answer_grade_closure",
    "gate_calculation",
)


class OrchestrationError(RuntimeError):
    """A stage was refused or stopped; no exception text includes source data."""


@dataclass(frozen=True)
class StagePlan:
    stage_uuid: str
    stage_kind: str
    source_revision: str
    protocol_bytes: bytes
    cohorts_bytes: bytes
    acquisition_plan_bytes: bytes
    acquisition_manifest_bytes: bytes
    source_closure_sha256: str
    research_cases: tuple[dict[str, object], ...]
    navigation_targets: tuple[dict[str, object], ...]
    candidate_identity_bytes: bytes
    candidate_endpoint_sha256: str
    packet_stage_uuid: str
    stage_started_monotonic: float
    stage_deadline_monotonic: float
    expected_acquisition_manifest_sha256: str


@dataclass(frozen=True)
class AcquisitionEvidence:
    result: acquisition.StageAcquisition
    snapshots_directory: Path
    snapshot_index_sha256: str
    acquisition_manifest_sha256: str
    receipt_sha256: str
    source_bound_receipt_status: str


@dataclass(frozen=True)
class LatePreparationEvidence:
    """Post-pool core preflight bound to the complete pool-derived input manifest."""

    prepared: core.PreparedStage
    input_manifest_bytes: bytes
    preflight_receipt_sha256: str


@dataclass(frozen=True)
class CaptureEvidence:
    result: source_capture.SourceCaptureResult
    inventory_bytes: bytes
    context_artifacts: Mapping[str, bytes]
    receipt_sha256: str


@dataclass(frozen=True)
class SelectorEvidence:
    """Closed original selector schedule plus complete W0/candidate orders."""

    stage_uuid: str
    terminal_inventory_sha256: str
    operation_rows: tuple[Mapping[str, object], ...]
    research_orders: Mapping[str, Mapping[str, Sequence[str]]]
    navigation_rank_one: Mapping[str, bool]
    reference_grade_receipt_sha256: str
    usage_status: str
    # Optional only when the selector emits the complete protocol variants.
    stability_orders: Mapping[str, Mapping[str, Mapping[str, Sequence[str]]]] | None = None
    # Exact candidate top-1 URLs, keyed by navigation task ID. Boolean rank
    # claims alone cannot prove that the selected URL is the exact target.
    navigation_top1_urls: Mapping[str, str] | None = None


@dataclass(frozen=True)
class AnswerEvidence:
    result: answer_execution.AnswerStageResult
    answer_inputs: Mapping[str, Mapping[str, bytes]]
    restored_answers: Mapping[str, Mapping[str, object]]
    terminal_inventory_bytes: bytes
    archive_root: Path
    result_root: Path


@dataclass(frozen=True)
class GateEvaluation:
    """External deterministic calculation, bound to all closed upstream inputs."""

    status: str
    scope: str
    input_manifest_sha256: str
    threshold_sha256: str
    gates: Mapping[str, bool | None]
    band_diagnostics: Mapping[str, object]
    metrics_sha256: str
    calculation_receipt_sha256: str


@dataclass(frozen=True)
class StageExecutors:
    acquire: Callable[[StagePlan], Awaitable[AcquisitionEvidence]]
    verify_acquisition: Callable[[StagePlan, AcquisitionEvidence], Awaitable[bool]]
    capture: Callable[[StagePlan, Mapping[str, object]], Awaitable[CaptureEvidence]]
    prepare_after_acquisition: Callable[
        [StagePlan, AcquisitionEvidence, Mapping[str, object]], Awaitable[LatePreparationEvidence]
    ]
    verify_late_preflight: Callable[[StagePlan, LatePreparationEvidence], Awaitable[bool]]
    grade_references: Callable[[packets.PreparedAssessmentPackets], Awaitable[Sequence[Any]]]
    selectors: Callable[[StagePlan, core.PreparedStage, Mapping[str, object], Any], Awaitable[SelectorEvidence]]
    answerer: Callable[[StagePlan, Sequence[answer_execution.AnswerTask]], Awaitable[AnswerEvidence]]
    grade_answers: Callable[[packets.PreparedAnswerPackets], Awaitable[Sequence[Any]]]


@dataclass(frozen=True)
class StageResult:
    status: str
    stage_uuid: str
    inventory_path: Path
    inventory_sha256: str
    terminal_reason: str | None
    product_authorized: bool = False
    admission_created: bool = False
    scientific_calls_made_by_coordinator: bool = False


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _monotonic() -> float:
    """Module-local clock seam for deterministic stage-deadline tests."""
    return time.monotonic()


def _canonical(value: object) -> bytes:
    try:
        return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode(
            "utf-8"
        )
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise OrchestrationError("orchestration-json-invalid") from exc


def _strict_json(raw: bytes, label: str) -> object:
    if type(raw) is not bytes:
        raise OrchestrationError(f"{label}-bytes-invalid")

    def pairs(items: list[tuple[str, object]]) -> dict[str, object]:
        result = {}
        for key, value in items:
            if key in result:
                raise OrchestrationError(f"{label}-duplicate-key")
            result[key] = value
        return result

    try:
        return json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=pairs,
            parse_constant=lambda _value: (_ for _ in ()).throw(OrchestrationError(f"{label}-nonfinite")),
        )
    except OrchestrationError:
        raise
    except (UnicodeDecodeError, json.JSONDecodeError, RecursionError) as exc:
        raise OrchestrationError(f"{label}-json-invalid") from exc


def _validate_plan(plan: StagePlan) -> dict[str, object]:
    if type(plan) is not StagePlan:
        raise OrchestrationError("preacquisition-plan-required")
    if plan.stage_kind not in {"development", "confirmation"}:
        raise OrchestrationError("stage-kind-invalid")
    if str(uuid.UUID(plan.stage_uuid)) != plan.stage_uuid:
        raise OrchestrationError("stage-uuid-invalid")
    if type(plan.source_revision) is not str or not re.fullmatch(r"[0-9a-f]{40}", plan.source_revision):
        raise OrchestrationError("source-revision-invalid")
    for name, value in (
        ("protocol", plan.protocol_bytes),
        ("cohorts", plan.cohorts_bytes),
        ("acquisition-plan", plan.acquisition_plan_bytes),
        ("acquisition-manifest", plan.acquisition_manifest_bytes),
        ("candidate-identity", plan.candidate_identity_bytes),
    ):
        if type(value) is not bytes or not value:
            raise OrchestrationError(f"{name}-bytes-invalid")
    protocol_sha = _sha(plan.protocol_bytes)
    cohorts_sha = _sha(plan.cohorts_bytes)
    if type(plan.candidate_endpoint_sha256) is not str or not _SHA256.fullmatch(plan.candidate_endpoint_sha256):
        raise OrchestrationError("candidate-endpoint-pin-invalid")
    if type(plan.expected_acquisition_manifest_sha256) is not str or not _SHA256.fullmatch(
        plan.expected_acquisition_manifest_sha256
    ):
        raise OrchestrationError("acquisition-manifest-pin-invalid")
    if _sha(plan.acquisition_manifest_bytes) != plan.expected_acquisition_manifest_sha256:
        raise OrchestrationError("acquisition-manifest-pin-mismatch")
    if type(plan.source_closure_sha256) is not str or not _SHA256.fullmatch(plan.source_closure_sha256):
        raise OrchestrationError("source-closure-pin-invalid")
    if type(plan.packet_stage_uuid) is not str or str(uuid.UUID(plan.packet_stage_uuid)) != plan.packet_stage_uuid:
        raise OrchestrationError("packet-stage-uuid-invalid")
    if plan.packet_stage_uuid == plan.stage_uuid:
        raise OrchestrationError("packet-stage-must-be-fresh")
    if (
        type(plan.stage_started_monotonic) not in {int, float}
        or type(plan.stage_deadline_monotonic) not in {int, float}
        or plan.stage_deadline_monotonic <= plan.stage_started_monotonic
        or plan.stage_deadline_monotonic - plan.stage_started_monotonic > MAX_STAGE_WALL_SECONDS
    ):
        raise OrchestrationError("stage-deadline-invalid")
    if not plan.stage_started_monotonic <= _monotonic() < plan.stage_deadline_monotonic:
        raise OrchestrationError("stage-deadline-inactive")

    protocol = _strict_json(plan.protocol_bytes, "protocol")
    cohorts = _strict_json(plan.cohorts_bytes, "cohorts")
    if type(protocol) is not dict or type(cohorts) is not dict:
        raise OrchestrationError("study-material-shape")
    # Fresh pool membership does not exist yet: only cohort and acquisition
    # material may be pinned before the first acquisition operation.
    if protocol.get("cohorts_sha256") != cohorts_sha:
        raise OrchestrationError("protocol-cohorts-binding-mismatch")
    if type(cohorts.get("stages")) is not list:
        raise OrchestrationError("cohort-stages-invalid")
    stage_rows = [row for row in cohorts["stages"] if type(row) is dict and row.get("stage") == plan.stage_kind]
    if len(stage_rows) != 1:
        raise OrchestrationError("cohort-stage-not-unique")
    stage_row = stage_rows[0]
    if tuple(plan.research_cases) != tuple(stage_row.get("research_cases", [])):
        raise OrchestrationError("research-cohort-content-mismatch")
    if tuple(plan.navigation_targets) != tuple(stage_row.get("navigation_targets", [])):
        raise OrchestrationError("navigation-cohort-content-mismatch")
    if len(plan.research_cases) != 8 or len(plan.navigation_targets) != 5:
        raise OrchestrationError("cohort-operation-count")
    research_ids = [row.get("task_id") for row in plan.research_cases]
    navigation_ids = [row.get("target_id") for row in plan.navigation_targets]
    if len(plan.research_cases) != 8:
        raise OrchestrationError("research-task-count")
    by_band: dict[str, list[dict[str, object]]] = {"research_le_40": [], "research_41_80": []}
    for row in plan.research_cases:
        if type(row) is not dict or type(row.get("pool_plan")) is not dict:
            raise OrchestrationError("research-case-shape")
        band = row["pool_plan"].get("band")
        if band not in by_band:
            raise OrchestrationError("research-band-invalid")
        by_band[band].append(row)
    if any(len(rows) != 4 for rows in by_band.values()):
        raise OrchestrationError("mixed-workload-band-balance")
    intents = ("evidence_seeking", "source_discovery")
    intent_counts = {
        band: {intent: sum(row.get("intent") == intent for row in rows) for intent in intents}
        for band, rows in by_band.items()
    }
    if any(count != 2 for counts in intent_counts.values() for count in counts.values()):
        raise OrchestrationError("mixed-workload-intent-balance")
    acquisition_manifest = _strict_json(plan.acquisition_manifest_bytes, "acquisition-manifest")
    if type(acquisition_manifest) is not dict:
        raise OrchestrationError("acquisition-manifest-shape")
    try:
        from scripts import coverage_live_acquire

        coverage_live_acquire._validate_manifest(acquisition_manifest, plan.acquisition_plan_bytes)
    except Exception as exc:
        raise OrchestrationError("acquisition-manifest-invalid") from exc
    expected_acquisition_identity = {
        "stage_uuid": plan.stage_uuid,
        "stage": plan.stage_kind,
        "source_revision": plan.source_revision,
        "source_closure_sha256": plan.source_closure_sha256,
        "protocol_sha256": protocol_sha,
        "cohorts_sha256": cohorts_sha,
        "acquisition_plan_sha256": _sha(plan.acquisition_plan_bytes),
        "research_cases": list(plan.research_cases),
        "navigation_targets": list(plan.navigation_targets),
    }
    if any(acquisition_manifest.get(key) != value for key, value in expected_acquisition_identity.items()):
        raise OrchestrationError("acquisition-manifest-plan-binding-mismatch")
    preacquisition_input = {
        "schema": "coverage-preacquisition-input-manifest/1",
        "stage_uuid": plan.stage_uuid,
        "protocol_sha256": protocol_sha,
        "cohorts_sha256": cohorts_sha,
        "task_ids": research_ids + navigation_ids,
    }
    if acquisition_manifest.get("input_manifest_sha256") != _sha(_canonical(preacquisition_input)):
        raise OrchestrationError("preacquisition-input-manifest-binding-mismatch")
    return {
        "protocol_sha256": protocol_sha,
        "cohorts_sha256": cohorts_sha,
        "task_ids": research_ids + navigation_ids,
        "source_closure_sha256": plan.source_closure_sha256,
        "acquisition_plan_sha256": _sha(plan.acquisition_plan_bytes),
        "acquisition_manifest_sha256": plan.expected_acquisition_manifest_sha256,
        "candidate_identity_sha256": _sha(plan.candidate_identity_bytes),
        "candidate_endpoint_sha256": plan.candidate_endpoint_sha256,
        "thresholds_sha256": _sha(_canonical({"quality": protocol["quality"], "task_use": protocol["task_use"]})),
        "band_task_counts": {name: len(rows) for name, rows in by_band.items()},
        "intent_counts_by_band": intent_counts,
        "scope": "mixed-workload-only",
    }


def _private_root(path: str | os.PathLike[str]) -> Path:
    root = Path(path)
    try:
        info = root.lstat()
    except OSError as exc:
        raise OrchestrationError("inventory-root-unavailable") from exc
    if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode) or stat.S_IMODE(info.st_mode) != 0o700:
        raise OrchestrationError("inventory-root-unsafe")
    if info.st_uid != getattr(os, "geteuid", lambda: info.st_uid)():
        raise OrchestrationError("inventory-root-owner")
    return root


def _write_new(path: Path, raw: bytes) -> str:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    try:
        fd = os.open(path, flags, 0o600)
    except FileExistsError as exc:
        raise OrchestrationError("stage-inventory-already-exists") from exc
    try:
        view = memoryview(raw)
        while view:
            size = os.write(fd, view)
            if size <= 0:
                raise OrchestrationError("stage-inventory-short-write")
            view = view[size:]
        os.fsync(fd)
    finally:
        os.close(fd)
    parent = os.open(path.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0))
    try:
        os.fsync(parent)
    finally:
        os.close(parent)
    return _sha(raw)


def _write_inventory(path: Path, value: Mapping[str, object]) -> str:
    raw = _canonical(value)
    temp = path.with_name(f"inventory.{uuid.uuid4().hex}.tmp")
    _write_new(temp, raw)
    os.replace(temp, path)
    parent = os.open(path.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0))
    try:
        os.fsync(parent)
    finally:
        os.close(parent)
    return _sha(raw)


async def _await(value: Any) -> Any:
    return await value if inspect.isawaitable(value) else value


def _validate_answer_evidence(
    evidence: AnswerEvidence,
    tasks: Sequence[answer_execution.AnswerTask],
    *,
    expected_inputs: Mapping[str, Mapping[str, bytes]],
    stage_uuid: str,
    source_revision: str,
) -> str:
    if type(evidence) is not AnswerEvidence or type(evidence.result) is not answer_execution.AnswerStageResult:
        raise OrchestrationError("answer-evidence-type")
    result = evidence.result
    if (
        result.status != "complete-structurally-valid-not-semantically-graded"
        or result.stage_uuid != stage_uuid
        or result.owned_http_calls != answer_execution.MAX_CALLS
        or result.usage_status not in {"reported", "invalid", "unavailable"}
        or result.semantic_grade is not False
        or evidence.answer_inputs != expected_inputs
    ):
        raise OrchestrationError("answerer-stage-or-input-not-complete")
    terminal_bytes = evidence.terminal_inventory_bytes
    if type(terminal_bytes) is not bytes or _sha(terminal_bytes) != result.terminal_receipt_sha256:
        raise OrchestrationError("answerer-terminal-inventory-pin")
    terminal = _strict_json(terminal_bytes, "answerer-terminal-inventory")
    if (
        type(terminal) is not dict
        or terminal.get("schema") != answer_execution.RESULT_SCHEMA
        or terminal.get("status") != result.status
        or terminal.get("stage_uuid") != stage_uuid
        or terminal.get("source_revision") != source_revision
        or terminal.get("owned_http_calls") != answer_execution.MAX_CALLS
        or terminal.get("usage_status") != result.usage_status
        or terminal.get("semantic_grade") is not False
        or terminal.get("operations") != list(result.operations)
    ):
        raise OrchestrationError("answerer-terminal-inventory-binding")
    requests = answer_execution._make_requests(tasks)
    security_mode = terminal.get("endpoint_security_mode")
    if security_mode not in {"https-required", "trusted-private-http"}:
        raise OrchestrationError("answerer-endpoint-security-mode-invalid")
    _manifest_bytes, expected_manifest_sha = answer_execution._request_manifest(
        requests, endpoint_security_mode=security_mode
    )
    if terminal.get("answer_manifest_sha256") != expected_manifest_sha:
        raise OrchestrationError("answerer-operation-manifest-mismatch")
    rows = terminal.get("operations")
    if type(rows) is not list or len(rows) != answer_execution.MAX_CALLS or len(requests) != answer_execution.MAX_CALLS:
        raise OrchestrationError("answerer-operation-count")
    if set(evidence.restored_answers) != {task.task_id for task in tasks}:
        raise OrchestrationError("answerer-output-task-coverage")
    answer_bytes_by_operation: dict[str, bytes] = {}
    for task in tasks:
        if set(evidence.restored_answers[task.task_id]) != {"w0", "candidate"}:
            raise OrchestrationError("answerer-output-arm-coverage")
    for request, row in zip(requests, rows, strict=True):
        success_status = "readiness-complete" if request.kind == "readiness" else "answer-structurally-valid"
        if (
            type(row) is not dict
            or row.get("operation_id") != request.operation_id
            or row.get("task_id") != request.task_id
            or row.get("arm") != request.arm
            or row.get("status") != success_status
            or row.get("usage_status") not in {"reported", "invalid", "unavailable"}
        ):
            raise OrchestrationError("answerer-operation-order-or-status")
        provider_usage = row.get("provider_usage")
        if row["usage_status"] == "reported":
            if (
                type(provider_usage) is not dict
                or not provider_usage
                or any(type(count) is not int or count < 0 for count in provider_usage.values())
                or set(provider_usage) - {"prompt_tokens", "completion_tokens", "total_tokens"}
            ):
                raise OrchestrationError("answerer-provider-usage-invalid")
        elif provider_usage is not None:
            raise OrchestrationError("answerer-provider-usage-status-mismatch")
        expected_bindings = {
            "stage_uuid": stage_uuid,
            "operation_id": request.operation_id,
            "request_body_sha256": _sha(request.body),
            "source_revision": source_revision,
        }
        try:
            archive_receipt, response_bytes = receipts.replay_response(
                evidence.archive_root,
                expected_bindings=expected_bindings,
                expected_receipt_sha256=row["archive_receipt_sha256"],
            )
        except Exception as exc:
            raise OrchestrationError("answerer-response-archive-binding") from exc
        if (
            archive_receipt.get("complete") is not True
            or archive_receipt.get("status") != "complete"
            or type(archive_receipt.get("http_status")) is not int
            or not 200 <= archive_receipt["http_status"] < 300
            or _sha(response_bytes) != row.get("response_sha256")
            or len(response_bytes) != row.get("response_bytes")
        ):
            raise OrchestrationError("answerer-response-archive-incomplete")
        envelope = answer_execution._strict_json(response_bytes)
        choices = envelope.get("choices") if type(envelope) is dict else None
        if type(choices) is not list or len(choices) != 1 or type(choices[0]) is not dict:
            raise OrchestrationError("answerer-archived-choice-invalid")
        message = choices[0].get("message")
        if type(message) is not dict or type(message.get("content")) is not str:
            raise OrchestrationError("answerer-archived-content-invalid")
        if request.kind == "readiness":
            if answer_execution._strict_json(message["content"].encode("utf-8")) != {"ready": True}:
                raise OrchestrationError("answerer-readiness-content-invalid")
            continue
        assert request.task is not None and request.answer_input is not None and request.arm is not None
        parsed = answer_execution._strict_json(message["content"].encode("utf-8"))
        check_ids = [check["id"] for check in request.task.critical_checks]
        try:
            restored = consumer.restore_answer_citations(
                parsed,
                request.answer_input,
                critical_check_ids=check_ids,
                catalog_bytes=request.task.catalog_bytes,
                expected_catalog_sha256=request.task.catalog_sha256,
            )
        except Exception as exc:
            raise OrchestrationError("answerer-archived-answer-invalid") from exc
        restored_bytes = _canonical(restored)
        if row.get("answer_sha256") != _sha(restored_bytes):
            raise OrchestrationError("answerer-restored-answer-hash-mismatch")
        answer_path = evidence.result_root / f"{stage_uuid}.{request.operation_id}.answer.json"
        try:
            on_disk = receipts._read_private(answer_path, max_bytes=answer_execution.MAX_RESPONSE_BYTES)
        except Exception as exc:
            raise OrchestrationError("answerer-restored-answer-file-invalid") from exc
        if on_disk != restored_bytes:
            raise OrchestrationError("answerer-restored-answer-file-mismatch")
        answer_bytes_by_operation[request.operation_id] = on_disk
        exposed = evidence.restored_answers[request.task.task_id][request.arm]
        if _canonical(exposed) != restored_bytes:
            raise OrchestrationError("answerer-exposed-output-mismatch")
    return _phase_digest(result.terminal_receipt_sha256, *(_sha(value) for value in answer_bytes_by_operation.values()))


def _record_phase(
    inventory: dict[str, object],
    index: int,
    *,
    status: str,
    output_sha256: str | None = None,
    error_class: str | None = None,
) -> None:
    phases = inventory["phases"]
    assert isinstance(phases, list)
    row = phases[index]
    assert isinstance(row, dict)
    row["status"] = status
    if output_sha256 is not None:
        row["output_sha256"] = output_sha256
    if error_class is not None:
        row["error_class"] = error_class


def _phase_digest(*values: object) -> str:
    """Stable output binding from exact receipts and validated manifests."""
    normalized = []
    for value in values:
        if type(value) is bytes:
            normalized.append({"sha256": _sha(value), "bytes": len(value)})
        elif type(value) is str and _SHA256.fullmatch(value):
            normalized.append(value)
        else:
            raise OrchestrationError("phase-output-digest-input-invalid")
    return _sha(_canonical(normalized))


def _pipeline_receipt(value: Mapping[str, object], expected_stage_uuid: str) -> str:
    if value.get("stage_uuid") != expected_stage_uuid:
        raise OrchestrationError("pipeline-stage-mismatch")
    return _phase_digest(value["capture_manifest_sha256"], value["card_bindings_sha256"])


def _validate_acquisition(plan: StagePlan, evidence: AcquisitionEvidence) -> None:
    if type(evidence) is not AcquisitionEvidence or type(evidence.result) is not acquisition.StageAcquisition:
        raise OrchestrationError("acquisition-evidence-type")
    if evidence.result.stage != plan.stage_kind or evidence.result.status != "complete":
        raise OrchestrationError("acquisition-not-complete")
    if len(evidence.result.operations) != 13 or any(row.status != "complete" for row in evidence.result.operations):
        raise OrchestrationError("acquisition-operation-incomplete")
    if evidence.result.physical_request_count > 53:
        raise OrchestrationError("acquisition-physical-call-cap")
    if evidence.acquisition_manifest_sha256 != plan.expected_acquisition_manifest_sha256:
        raise OrchestrationError("acquisition-manifest-binding-mismatch")
    if type(evidence.snapshot_index_sha256) is not str or not _SHA256.fullmatch(evidence.snapshot_index_sha256):
        raise OrchestrationError("acquisition-index-output-pin-invalid")
    try:
        from scripts import coverage_live_acquire

        verified = coverage_live_acquire.verify_pool_snapshot_index(
            evidence.snapshots_directory,
            expected_index_sha256=evidence.snapshot_index_sha256,
            expected_stage_manifest_sha256=plan.expected_acquisition_manifest_sha256,
            expected_stage_uuid=plan.stage_uuid,
            expected_source_revision=plan.source_revision,
        )
    except Exception as exc:
        raise OrchestrationError("acquisition-snapshot-not-durable-or-bound") from exc
    index = verified["index"]
    expected_index_fields = {
        "source_closure_sha256": plan.source_closure_sha256,
        "protocol_sha256": _sha(plan.protocol_bytes),
        "cohorts_sha256": _sha(plan.cohorts_bytes),
        "acquisition_plan_sha256": _sha(plan.acquisition_plan_bytes),
    }
    if any(index.get(key) != value for key, value in expected_index_fields.items()):
        raise OrchestrationError("acquisition-snapshot-pin-mismatch")
    observed_operation_ids = [row.get("operation_id") for row in index["operations"]]
    expected_operation_ids = [row["task_id"] for row in plan.research_cases] + [
        row["target_id"] for row in plan.navigation_targets
    ]
    if observed_operation_ids != expected_operation_ids:
        raise OrchestrationError("acquisition-snapshot-operation-order")
    if type(evidence.receipt_sha256) is not str or not _SHA256.fullmatch(evidence.receipt_sha256):
        raise OrchestrationError("acquisition-receipt-invalid")


def _validate_late_preparation(
    plan: StagePlan, evidence: LatePreparationEvidence, pipeline_inputs: Mapping[str, object]
) -> None:
    if type(evidence) is not LatePreparationEvidence or type(evidence.prepared) is not core.PreparedStage:
        raise OrchestrationError("late-preflight-evidence-type")
    prepared = evidence.prepared
    if prepared.fresh_execution_authorized is not False or prepared.status != "preflight-verified-not-admitted":
        raise OrchestrationError("late-preflight-not-summary-only")
    if (
        prepared.stage_uuid != plan.stage_uuid
        or prepared.stage_kind != plan.stage_kind
        or prepared.source_revision != plan.source_revision
        or prepared.pins.get("protocol") != _sha(plan.protocol_bytes)
        or prepared.pins.get("qualified_source_closure") != plan.source_closure_sha256
        or type(evidence.input_manifest_bytes) is not bytes
        or _sha(evidence.input_manifest_bytes) != prepared.pins.get("task_input_manifest")
    ):
        raise OrchestrationError("late-preflight-pins-mismatch")
    input_doc = _strict_json(evidence.input_manifest_bytes, "late-task-input-manifest")
    if type(input_doc) is not dict or input_doc.get("stage_uuid") != plan.stage_uuid:
        raise OrchestrationError("late-task-input-manifest-stage")
    input_tasks = input_doc.get("research_tasks")
    pipeline_tasks = pipeline_inputs.get("tasks")
    if (
        type(input_tasks) is not list
        or type(pipeline_tasks) is not list
        or len(input_tasks) != 8
        or len(pipeline_tasks) != 8
    ):
        raise OrchestrationError("late-task-input-manifest-count")
    for input_task, pipeline_task in zip(input_tasks, pipeline_tasks, strict=True):
        if type(input_task) is not dict or type(pipeline_task) is not dict:
            raise OrchestrationError("late-task-input-manifest-row")
        if (
            input_task.get("task_id") != pipeline_task.get("task_id")
            or input_task.get("query") != pipeline_task.get("query")
            or input_task.get("purpose") != pipeline_task.get("purpose")
            or input_task.get("facets") != pipeline_task.get("facets")
            or input_task.get("candidates") != pipeline_task.get("ranking_candidates")
            or input_task.get("incumbent_order") != pipeline_task.get("native_order")
        ):
            raise OrchestrationError("late-preflight-pool-projection-mismatch")
    input_navigation = input_doc.get("navigation_tasks")
    if type(input_navigation) is not list or [row.get("task_id") for row in input_navigation if type(row) is dict] != [
        row["target_id"] for row in plan.navigation_targets
    ]:
        raise OrchestrationError("late-navigation-input-manifest-mismatch")
    if type(evidence.preflight_receipt_sha256) is not str or not _SHA256.fullmatch(evidence.preflight_receipt_sha256):
        raise OrchestrationError("late-preflight-receipt-invalid")
    expected_ops = tuple(row.operation_id for row in core.selector_inventory(prepared.navigation_skip_reasons))
    if prepared.operation_ids != expected_ops:
        raise OrchestrationError("late-preflight-selector-inventory-mismatch")


def _validate_capture(
    plan: StagePlan, pipeline_inputs: Mapping[str, object], evidence: CaptureEvidence
) -> dict[str, object]:
    if type(evidence) is not CaptureEvidence or type(evidence.result) is not source_capture.SourceCaptureResult:
        raise OrchestrationError("capture-evidence-type")
    result = evidence.result
    if result.stage_uuid != plan.stage_uuid:
        raise OrchestrationError("capture-stage-mismatch")
    if result.status not in {"complete", "complete-with-source-failures"}:
        raise OrchestrationError("capture-not-complete")
    if result.quality_credit or result.source_count != len(pipeline_inputs["capture_manifest"]["sources"]):
        raise OrchestrationError("capture-inventory-count-or-quality-flag")
    if type(evidence.inventory_bytes) is not bytes or not evidence.inventory_bytes:
        raise OrchestrationError("capture-inventory-bytes-invalid")
    inventory = _strict_json(evidence.inventory_bytes, "capture-inventory")
    if (
        type(inventory) is not dict
        or inventory.get("schema") != "coverage-source-capture-inventory/1"
        or inventory.get("stage_uuid") != result.stage_uuid
        or inventory.get("status") != result.status
    ):
        raise OrchestrationError("capture-inventory-binding")
    rows = inventory.get("sources")
    if type(rows) is not list or len(rows) != result.source_count or rows != list(result.private_inventory):
        raise OrchestrationError("capture-private-inventory-mismatch")
    if type(evidence.receipt_sha256) is not str or not _SHA256.fullmatch(evidence.receipt_sha256):
        raise OrchestrationError("capture-receipt-invalid")
    return inventory


def _check_selector(
    selector: SelectorEvidence,
    plan: StagePlan,
    prepared: core.PreparedStage,
    pipeline_inputs: Mapping[str, object],
    reference_sha: str,
) -> None:
    if type(selector) is not SelectorEvidence or selector.stage_uuid != plan.stage_uuid:
        raise OrchestrationError("selector-stage-binding")
    if selector.reference_grade_receipt_sha256 != reference_sha:
        raise OrchestrationError("selector-reference-grade-binding")
    if selector.usage_status != "known":
        raise OrchestrationError("selector-usage-unknown")
    if type(selector.terminal_inventory_sha256) is not str or not _SHA256.fullmatch(selector.terminal_inventory_sha256):
        raise OrchestrationError("selector-terminal-inventory-pin")
    expected_ids = prepared.operation_ids
    observed_ids = tuple(row.get("operation_id") for row in selector.operation_rows)
    if observed_ids != expected_ids:
        raise OrchestrationError("selector-operation-inventory-order")
    if any(row.get("state") != "complete-success" for row in selector.operation_rows):
        raise OrchestrationError("selector-incomplete-terminal-inventory")
    pipeline_tasks = pipeline_inputs.get("tasks")
    if type(pipeline_tasks) is not list or set(selector.research_orders) != {row["task_id"] for row in pipeline_tasks}:
        raise OrchestrationError("selector-research-task-coverage")
    for task in pipeline_tasks:
        task_id = task["task_id"]
        expected_cards = task["native_order"]
        orders = selector.research_orders[task_id]
        if set(orders) != {"w0", "candidate"}:
            raise OrchestrationError("selector-arm-coverage")
        for order in orders.values():
            if (
                type(order) not in {list, tuple}
                or len(order) != len(expected_cards)
                or set(order) != set(expected_cards)
            ):
                raise OrchestrationError("selector-order-not-full-pool-permutation")
    expected_nav = {row["target_id"] for row in plan.navigation_targets}
    if set(selector.navigation_rank_one) != expected_nav or any(
        type(value) is not bool for value in selector.navigation_rank_one.values()
    ):
        raise OrchestrationError("selector-navigation-target-coverage")
    if selector.navigation_top1_urls is not None:
        if type(selector.navigation_top1_urls) is not dict or set(selector.navigation_top1_urls) != expected_nav:
            raise OrchestrationError("selector-navigation-top1-coverage")
        try:
            for value in selector.navigation_top1_urls.values():
                pipeline._canonical_public_url(value)
        except Exception as exc:
            raise OrchestrationError("selector-navigation-top1-url-invalid") from exc


def _sealed_navigation_observations(
    plan: StagePlan, evidence: AcquisitionEvidence, selector: SelectorEvidence
) -> dict[str, dict[str, object]]:
    """Re-read navigation URLs only from the pinned complete snapshot artifacts."""
    try:
        verified = coverage_live_acquire.verify_pool_snapshot_index(
            evidence.snapshots_directory,
            expected_index_sha256=evidence.snapshot_index_sha256,
            expected_stage_manifest_sha256=evidence.acquisition_manifest_sha256,
            expected_stage_uuid=plan.stage_uuid,
            expected_source_revision=plan.source_revision,
        )
        operations = verified["operations"]
        result: dict[str, dict[str, object]] = {}
        for target in plan.navigation_targets:
            operation_id = target["target_id"]
            artifact = operations.get(operation_id)
            if type(artifact) is not dict or artifact.get("task_plan") != target:
                raise OrchestrationError("navigation-snapshot-cohort-binding")
            operation = artifact.get("operation")
            response = operation.get("canonical_response") if type(operation) is dict else None
            rows = response.get("results") if type(response) is dict else None
            if type(rows) is not list:
                raise OrchestrationError("navigation-snapshot-response-invalid")
            acquired_urls = []
            for row in rows:
                if type(row) is not dict or type(row.get("url")) is not str:
                    raise OrchestrationError("navigation-snapshot-url-invalid")
                acquired_urls.append(pipeline._canonical_public_url(row["url"])[1])
            top1 = None
            if selector.navigation_top1_urls is not None:
                top1 = pipeline._canonical_public_url(selector.navigation_top1_urls[operation_id])[1]
            result[operation_id] = {"acquired_urls": acquired_urls}
            if top1 is not None:
                result[operation_id]["candidate_top1_url"] = top1
        return result
    except OrchestrationError:
        raise
    except Exception as exc:
        raise OrchestrationError("navigation-snapshot-reverification-failed") from exc


def _prepare_answer_tasks(
    plan: StagePlan,
    pipeline_inputs: Mapping[str, object],
    prepared_references: packets.PreparedAssessmentPackets,
    selector: SelectorEvidence,
    capture_inventory: Mapping[str, object],
) -> tuple[tuple[answer_execution.AnswerTask, ...], dict[str, dict[str, bytes]]]:
    tasks = pipeline_inputs["tasks"]
    captures_by_task: dict[str, dict[str, dict[str, object]]] = {}
    input_bodies: dict[str, dict[str, bytes]] = {}
    answer_tasks = []
    for task_index, task in enumerate(tasks, start=1):
        task_id = task["task_id"]
        contexts = dict(prepared_references.task_contexts[task_id])
        captures_by_task[task_id] = contexts
        contexts_sha = _sha(consumer._canonical(contexts))
        orders = selector.research_orders[task_id]
        pairs = consumer.prepare_pair(
            task_index=task_index,
            task_id=task_id,
            task_question=str(task["query"]),
            purpose=str(task["purpose"]),
            critical_checks=list(prepared_references.task_checks[task_id]),
            card_ids=list(task["native_order"]),
            source_id_by_card_id=task["source_id_by_card_id"],
            captures=contexts,
            expected_capture_inventory_sha256=contexts_sha,
            orders={arm: list(orders[arm]) for arm in ("w0", "candidate")},
        )
        input_bodies[task_id] = {answer.arm: answer.body for answer in pairs.answers}
        metadata = prepared_references.task_metadata[task_id]
        answer_tasks.append(
            answer_execution.AnswerTask(
                task_id=task_id,
                kind="research",
                query=str(metadata["query"]),
                purpose=str(metadata["purpose"]),
                critical_checks=prepared_references.task_checks[task_id],
                answers=pairs.answers,
                catalog_bytes=pairs.private_catalog_bytes,
                catalog_sha256=pairs.catalog_sha256,
            )
        )
    if set(input_bodies) != {row["task_id"] for row in tasks}:
        raise OrchestrationError("paired-answer-task-coverage")
    # Ensure every task capture still maps to the sealed source rows; packet
    # preparation already checked provenance, this is a consistency join.
    source_rows = capture_inventory["sources"]
    for task_id, context_map in captures_by_task.items():
        expected_source_ids = {row["source_id"] for row in source_rows if row.get("task_id") == task_id}
        if set(context_map) != expected_source_ids:
            raise OrchestrationError("paired-answer-capture-source-coverage")
    return tuple(answer_tasks), input_bodies


async def coordinate_coverage_stage(
    *, plan: StagePlan, executors: StageExecutors, inventory_root: str | os.PathLike[str]
) -> StageResult:
    """Run the ordered phase graph once; every post-failure slot stays uninvoked.

    This function is an orchestration seam, not a CLI and not an admission
    mechanism. All network and model interactions are injected executors. The
    only success it reports is an externally calculated stage-gate result;
    it never authorizes product rollout or automatically starts confirmation.
    """
    root = _private_root(inventory_root)
    identity = _validate_plan(plan)
    stage_dir = root / plan.stage_uuid
    try:
        stage_dir.mkdir(mode=0o700)
        os.chmod(stage_dir, 0o700)
        parent = os.open(root, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0))
        try:
            os.fsync(parent)
        finally:
            os.close(parent)
    except FileExistsError as exc:
        raise OrchestrationError("stage-already-claimed-no-resume") from exc
    except OSError as exc:
        raise OrchestrationError("stage-inventory-directory-create-failed") from exc

    phase_rows = [{"phase": name, "status": "not-invoked"} for name in PHASES]
    inventory: dict[str, object] = {
        "schema": SCHEMA,
        "stage_uuid": plan.stage_uuid,
        "stage_kind": plan.stage_kind,
        "source_revision": plan.source_revision,
        "preacquisition_binding_sha256": _sha(
            _canonical(
                {
                    "stage_uuid": plan.stage_uuid,
                    "source_revision": plan.source_revision,
                    "protocol_sha256": identity["protocol_sha256"],
                    "cohorts_sha256": identity["cohorts_sha256"],
                    "acquisition_plan_sha256": identity["acquisition_plan_sha256"],
                    "task_ids": identity["task_ids"],
                    "source_closure_sha256": identity["source_closure_sha256"],
                }
            )
        ),
        "protocol_sha256": identity["protocol_sha256"],
        "cohorts_sha256": identity["cohorts_sha256"],
        "acquisition_manifest_sha256": identity["acquisition_manifest_sha256"],
        "candidate_identity_sha256": identity["candidate_identity_sha256"],
        "candidate_endpoint_sha256": identity["candidate_endpoint_sha256"],
        "scope": "mixed-workload-only",
        "per_band_qualification": "diagnostic-only",
        "product_authorized": False,
        "admission_created": False,
        "scientific_calls_made_by_coordinator": False,
        "status": "in-progress",
        "terminal_reason": None,
        "phases": phase_rows,
    }
    inventory_path = stage_dir / "inventory.json"
    _write_new(inventory_path, _canonical(inventory))
    artifacts: dict[str, object] = {}

    async def phase(index: int, name: str, invoke: Callable[[], Any], summarize: Callable[[Any], str]) -> Any:
        _record_phase(inventory, index, status="in-flight")
        _write_inventory(inventory_path, inventory)
        try:
            if _monotonic() >= plan.stage_deadline_monotonic:
                raise OrchestrationError("stage-deadline-exceeded-before-phase")
            value = await _await(invoke())
            if _monotonic() >= plan.stage_deadline_monotonic:
                raise OrchestrationError("stage-deadline-exceeded-during-phase")
            output_sha = summarize(value)
            if type(output_sha) is not str or not _SHA256.fullmatch(output_sha):
                raise OrchestrationError(f"{name}-output-receipt-invalid")
            _record_phase(inventory, index, status="complete", output_sha256=output_sha)
            artifacts[name] = value
            _write_inventory(inventory_path, inventory)
            return value
        except Exception as exc:
            _record_phase(inventory, index, status="terminal-failure", error_class=type(exc).__name__)
            inventory["status"] = "terminal-incomplete"
            inventory["terminal_reason"] = f"{name}-failed"
            _write_inventory(inventory_path, inventory)
            raise

    try:

        async def acquire_and_verify() -> AcquisitionEvidence:
            evidence = await executors.acquire(plan)
            _validate_acquisition(plan, evidence)
            if await _await(executors.verify_acquisition(plan, evidence)) is not True:
                raise OrchestrationError("acquisition-source-bound-receipt-unverified")
            return evidence

        acq = await phase(
            0,
            "acquisition",
            acquire_and_verify,
            lambda value: _phase_digest(value.receipt_sha256, value.snapshot_index_sha256),
        )
        pipeline_inputs = await phase(
            1,
            "pipeline_inputs",
            lambda: pipeline.build_pipeline_inputs(
                list(plan.research_cases),
                snapshots_directory=acq.snapshots_directory,
                expected_snapshot_index_sha256=acq.snapshot_index_sha256,
                expected_stage_manifest_sha256=acq.acquisition_manifest_sha256,
                stage_uuid=plan.stage_uuid,
                source_revision=plan.source_revision,
                protocol_sha256=identity["protocol_sha256"],
                cohorts_sha256=identity["cohorts_sha256"],
                candidate_identity_sha256=identity["candidate_identity_sha256"],
                candidate_endpoint_sha256=identity["candidate_endpoint_sha256"],
            ),
            lambda value: _pipeline_receipt(value, plan.stage_uuid),
        )

        async def late_prepare_and_verify() -> LatePreparationEvidence:
            evidence = await executors.prepare_after_acquisition(plan, acq, pipeline_inputs)
            _validate_late_preparation(plan, evidence, pipeline_inputs)
            if await _await(executors.verify_late_preflight(plan, evidence)) is not True:
                raise OrchestrationError("late-preflight-external-binding-unverified")
            return evidence

        late_preparation = await phase(
            2,
            "late_preflight",
            late_prepare_and_verify,
            lambda value: (
                _validate_late_preparation(plan, value, pipeline_inputs)
                or _phase_digest(value.preflight_receipt_sha256, _sha(value.input_manifest_bytes))
            ),
        )
        prepared = late_preparation.prepared
        captured = await phase(
            3,
            "capture",
            lambda: executors.capture(plan, pipeline_inputs),
            lambda value: (
                _validate_capture(plan, pipeline_inputs, value)
                and _phase_digest(value.receipt_sha256, _sha(value.inventory_bytes))
            ),
        )
        capture_inventory = _strict_json(captured.inventory_bytes, "capture-inventory")
        prepared_references = await phase(
            4,
            "reference_packet_preparation",
            lambda: packets.prepare_assessment_packets(
                packet_stage_uuid=plan.packet_stage_uuid,
                pipeline_inputs=pipeline_inputs,
                research_cases=list(plan.research_cases),
                capture_inventory_bytes=captured.inventory_bytes,
                expected_capture_inventory_sha256=_sha(captured.inventory_bytes),
                context_artifacts=captured.context_artifacts,
            ),
            lambda value: _phase_digest(
                _sha(value.preassessment_manifest_bytes),
                value.private_binding_sha256,
            ),
        )
        preassessment_manifest_pin = _sha(prepared_references.preassessment_manifest_bytes)
        private_binding_pin = prepared_references.private_binding_sha256
        reference_submissions = await phase(
            5,
            "reference_grading",
            lambda: executors.grade_references(prepared_references),
            lambda value: _phase_digest(*(_sha(row.response_bytes) for row in value)),
        )
        from scripts import coverage_grade_closure as grade_closure

        closed_references = await phase(
            6,
            "reference_grade_closure",
            lambda: grade_closure.close_preassessment(
                prepared_references,
                reference_submissions,
                expected_manifest_sha256=preassessment_manifest_pin,
                expected_private_binding_sha256=private_binding_pin,
            ),
            lambda value: _phase_digest(value.receipt_sha256),
        )
        selector = await phase(
            7,
            "selector",
            lambda: executors.selectors(plan, prepared, pipeline_inputs, closed_references),
            lambda value: (
                _check_selector(value, plan, prepared, pipeline_inputs, closed_references.receipt_sha256)
                or _phase_digest(value.terminal_inventory_sha256)
            ),
        )
        answer_tasks, answer_inputs = await phase(
            8,
            "paired_answer_inputs",
            lambda: _prepare_answer_tasks(plan, pipeline_inputs, prepared_references, selector, capture_inventory),
            lambda value: _phase_digest(*(answer.body_sha256 for task in value[0] for answer in task.answers)),
        )
        answer_evidence = await phase(
            9,
            "answerer",
            lambda: executors.answerer(plan, answer_tasks),
            lambda value: _phase_digest(value.result.terminal_receipt_sha256),
        )
        await phase(
            10,
            "answerer_dispatch_verification",
            lambda: _validate_answer_evidence(
                answer_evidence,
                answer_tasks,
                expected_inputs=answer_inputs,
                stage_uuid=plan.stage_uuid,
                source_revision=plan.source_revision,
            ),
            lambda value: value,
        )
        prepared_answer_packets = await phase(
            11,
            "answer_packet_preparation",
            lambda: packets.prepare_answer_assessment_packets(
                prepared=prepared_references,
                answer_inputs=answer_evidence.answer_inputs,
                answer_outputs=answer_evidence.restored_answers,
                source_outputs=closed_references.source_outputs_for_answer(),
            ),
            lambda value: _phase_digest(_sha(value.manifest_bytes), value.manifest_sha256),
        )
        answer_manifest_pin = prepared_answer_packets.manifest_sha256
        answer_submissions = await phase(
            12,
            "answer_grading",
            lambda: executors.grade_answers(prepared_answer_packets),
            lambda value: _phase_digest(*(_sha(row.response_bytes) for row in value)),
        )
        closed_answers = await phase(
            13,
            "answer_grade_closure",
            lambda: grade_closure.close_answer_assessments(
                prepared_references,
                prepared_answer_packets,
                closed_references,
                answer_submissions,
                expected_preassessment_manifest_sha256=preassessment_manifest_pin,
                expected_private_binding_sha256=private_binding_pin,
                expected_answer_manifest_sha256=answer_manifest_pin,
            ),
            lambda value: _phase_digest(value.receipt_sha256),
        )
        task_inputs = [
            {
                "task_id": row["task_id"],
                "card_ids": list(row["native_order"]),
                "facet_ids": [facet["id"] for facet in row["facets"]],
                "band": row["band"],
                "intent": row["intent"],
            }
            for row in pipeline_inputs["tasks"]
        ]
        navigation_inputs = [
            {"task_id": row["target_id"], "target_url": row["target_url"]} for row in plan.navigation_targets
        ]
        navigation_observations = _sealed_navigation_observations(plan, acq, selector)
        nav_rank_one_by_task = {
            row["task_id"]: selector.navigation_rank_one.get(row["task_id"])
            for row in navigation_inputs
            if row["task_id"] in selector.navigation_rank_one
        }
        # Only measurements with an explicit source are supplied. In
        # particular, a hash string alone is not a terminal-inventory receipt,
        # and unavailable provider usage is not a zero-token observation.
        resource_evidence: dict[str, object] = {}
        # A pre-calculation elapsed sample would omit calculation, receipt and
        # final inventory fsync. It is not supplied as completed-stage timing.
        if selector.usage_status == "known":
            usage_rows = []
            for row in selector.operation_rows:
                if any(type(row.get(key)) is not int or row[key] < 0 for key in ("input_tokens", "output_tokens")):
                    usage_rows = []
                    break
                usage_rows.append(
                    {
                        "operation_id": row["operation_id"],
                        "status": "reported",
                        "input_tokens": row["input_tokens"],
                        "output_tokens": row["output_tokens"],
                    }
                )
            if usage_rows:
                resource_evidence["selector_usage"] = usage_rows
        gate_inputs = {
            "stage_uuid": plan.stage_uuid,
            "protocol_sha256": identity["protocol_sha256"],
            "cohorts_sha256": identity["cohorts_sha256"],
            "snapshot_index_sha256": acq.snapshot_index_sha256,
            "acquisition_manifest_sha256": acq.acquisition_manifest_sha256,
            "source_closure_sha256": plan.source_closure_sha256,
            "scope": "mixed-workload-only",
            "per_band_qualification": "diagnostic-only",
            "pipeline_inputs_sha256": pipeline_inputs["capture_manifest_sha256"],
            "task_input_manifest_sha256": prepared.pins["task_input_manifest"],
            "capture_receipt_sha256": captured.receipt_sha256,
            "reference_grade_receipt_sha256": closed_references.receipt_sha256,
            "selector_inventory_sha256": selector.terminal_inventory_sha256,
            "answer_receipt_sha256": answer_evidence.result.terminal_receipt_sha256,
            "answer_grade_receipt_sha256": closed_answers.receipt_sha256,
            "selector_evidence": {
                "terminal_inventory_sha256": selector.terminal_inventory_sha256,
                "operation_rows": list(selector.operation_rows),
                "research_orders": selector.research_orders,
                "stability_orders": selector.stability_orders,
                "navigation_rank_one": selector.navigation_rank_one,
                "navigation_top1_urls": selector.navigation_top1_urls,
                "usage_status": selector.usage_status,
            },
            "navigation_observations": navigation_observations,
            "resource_evidence": resource_evidence,
        }
        gate_input_sha = _sha(_canonical(gate_inputs))

        async def calculate_frozen_gates() -> GateEvaluation:
            from scripts.coverage_gate_calculation import calculate_coverage_gate_report

            report = calculate_coverage_gate_report(
                protocol=_strict_json(plan.protocol_bytes, "protocol"),
                task_inputs=task_inputs,
                navigation_inputs=navigation_inputs,
                prepared_assessments=prepared_references,
                reference_closure=closed_references,
                answer_closure=closed_answers,
                selector_evidence=SelectorEvidence(
                    **{
                        **selector.__dict__,
                        "navigation_rank_one": nav_rank_one_by_task,
                    }
                ),
                stability_orders=selector.stability_orders,
                navigation_observations=navigation_observations,
                resource_evidence=resource_evidence,
                input_manifest_sha256=gate_input_sha,
            )
            receipt_path = stage_dir / "gate-calculation-receipt.json"
            _write_new(receipt_path, report.receipt_bytes)
            artifacts["gate_calculation_receipt_bytes"] = report.receipt_bytes
            artifacts["gate_calculation_metrics"] = report.metrics
            artifacts["gate_calculation_receipt_path"] = receipt_path.name
            return report.evaluation

        gates = await phase(
            14,
            "gate_calculation",
            calculate_frozen_gates,
            lambda value: (
                _check_gates(value, gate_input_sha, plan, identity)
                or _phase_digest(value.metrics_sha256, value.calculation_receipt_sha256)
            ),
        )
        inventory["status"] = {
            "pass": "gates-pass-awaiting-independent-decision",
            "fail": "gates-failed",
            "inconclusive": "gates-inconclusive",
        }[gates.status]
        inventory["terminal_reason"] = None if gates.status == "pass" else f"gate-{gates.status}"
        inventory["gate_status"] = gates.status
        inventory["gate_result_authoritative"] = True
        inventory["gate_scope"] = gates.scope
        inventory["band_diagnostics_sha256"] = _sha(_canonical(gates.band_diagnostics))
        inventory["gate_calculation_receipt_sha256"] = gates.calculation_receipt_sha256
        inventory["gate_calculation_receipt_file"] = artifacts["gate_calculation_receipt_path"]
        metrics = artifacts.get("gate_calculation_metrics")
        if type(metrics) is dict:
            inventory["gate_metrics"] = metrics
    except Exception as exc:
        # The phase wrapper already persisted the terminal phase and left every
        # later phase explicitly not-invoked. Cross-output checks outside a
        # phase wrapper receive a terminal controller row without retry.
        if inventory["status"] == "in-progress":
            inventory["status"] = "terminal-incomplete"
            inventory["terminal_reason"] = "orchestration-binding-failed"
            failed = next((row for row in phase_rows if row["status"] == "complete"), None)
            if failed is not None:
                failed["postcondition_status"] = "terminal-failure"
                failed["postcondition_error_class"] = type(exc).__name__
        _write_inventory(inventory_path, inventory)

    final_sha = _write_inventory(inventory_path, inventory)
    completed_at = _monotonic()
    if completed_at >= plan.stage_deadline_monotonic and inventory.get("status") != "terminal-incomplete":
        original_inventory_sha256 = final_sha
        inventory["status"] = "terminal-incomplete"
        inventory["terminal_reason"] = "stage-deadline-exceeded-after-final-fsync"
        inventory["gate_result_authoritative"] = False
        inventory["deadline_overrun_correction"] = {
            "original_inventory_sha256": original_inventory_sha256,
            "observed_after_final_fsync": completed_at,
            "deadline_monotonic": plan.stage_deadline_monotonic,
        }
        final_sha = _write_inventory(inventory_path, inventory)
    terminal_reason = inventory.get("terminal_reason")
    return StageResult(
        status=str(inventory["status"]),
        stage_uuid=plan.stage_uuid,
        inventory_path=inventory_path,
        inventory_sha256=final_sha,
        terminal_reason=str(terminal_reason) if terminal_reason is not None else None,
    )


def _check_gates(value: GateEvaluation, input_sha: str, plan: StagePlan, identity: Mapping[str, object]) -> None:
    if type(value) is not GateEvaluation:
        raise OrchestrationError("gate-evaluation-type")
    if value.scope != "mixed-workload-only" or value.input_manifest_sha256 != input_sha:
        raise OrchestrationError("gate-evaluation-input-binding")
    if value.threshold_sha256 != identity["thresholds_sha256"]:
        raise OrchestrationError("gate-threshold-binding")
    if value.status not in {"pass", "fail", "inconclusive"}:
        raise OrchestrationError("gate-status-invalid")
    if set(value.band_diagnostics) != {"research_le_40", "research_41_80"}:
        raise OrchestrationError("gate-band-diagnostics-missing")
    if type(value.metrics_sha256) is not str or not _SHA256.fullmatch(value.metrics_sha256):
        raise OrchestrationError("gate-metrics-pin-invalid")
    if type(value.calculation_receipt_sha256) is not str or not _SHA256.fullmatch(value.calculation_receipt_sha256):
        raise OrchestrationError("gate-calculation-receipt-invalid")
    if not value.gates or any(type(result) not in {bool, type(None)} for result in value.gates.values()):
        raise OrchestrationError("gate-values-invalid")
    if value.status == "pass" and not all(result is True for result in value.gates.values()):
        raise OrchestrationError("gate-pass-with-nonpassing-value")
    if value.status == "fail" and not any(result is False for result in value.gates.values()):
        raise OrchestrationError("gate-fail-without-failed-value")
    if value.status == "inconclusive" and all(result is True for result in value.gates.values()):
        raise OrchestrationError("gate-inconclusive-with-all-passing-values")


__all__ = [
    "AcquisitionEvidence",
    "AnswerEvidence",
    "CaptureEvidence",
    "GateEvaluation",
    "OrchestrationError",
    "PHASES",
    "SelectorEvidence",
    "StageExecutors",
    "StagePlan",
    "StageResult",
    "coordinate_coverage_stage",
]
