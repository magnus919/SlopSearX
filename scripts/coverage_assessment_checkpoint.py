"""Durable, one-shot checkpoints for prepared assessments and validated grades.

This module stores exact assessment bytes in an owner-only stage directory. It
does not dispatch grading, resume a stage, or interpret grade meaning.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import stat
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence

from scripts import coverage_assessment_packets as packets
from scripts import coverage_grade_closure as grades

PREPARED_SCHEMA = "coverage-assessment-checkpoint/1"
CLOSED_SCHEMA = "coverage-assessment-closure-checkpoint/1"
_SHA = re.compile(r"[0-9a-f]{64}\Z")
_PACKET_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9._:-]{0,127}\Z")
_JSON_CAP = 64_000_000
_PREPARED_PACKET_TOTAL_CAP = (packets.MAX_GRADER_SUBMISSIONS - 32) * packets.MAX_PACKET_BYTES
_CLOSED_RESPONSE_TOTAL_CAP = (packets.MAX_GRADER_SUBMISSIONS - 32) * grades.MAX_RESPONSE_BYTES


class AssessmentCheckpointError(ValueError):
    """A checkpoint is incomplete, unsafe, or inconsistent with validated grades."""


@dataclass(frozen=True)
class PreparedAssessmentCheckpoint:
    """Capability returned only after the complete prepared checkpoint is durable."""

    stage_uuid: str
    directory: Path
    index_sha256: str


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    try:
        return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode(
            "utf-8"
        )
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise AssessmentCheckpointError("checkpoint-json-invalid") from exc


def _safe_uuid(value: object) -> str:
    if type(value) is not str:
        raise AssessmentCheckpointError("checkpoint-stage-uuid-invalid")
    try:
        if str(uuid.UUID(value)) != value:
            raise ValueError
    except (TypeError, ValueError) as exc:
        raise AssessmentCheckpointError("checkpoint-stage-uuid-invalid") from exc
    return value


def _private_directory(path: Path, *, label: str, exact_mode: bool = True) -> None:
    try:
        info = path.lstat()
    except OSError as exc:
        raise AssessmentCheckpointError(f"{label}-directory-unavailable") from exc
    mode = stat.S_IMODE(info.st_mode)
    if (
        stat.S_ISLNK(info.st_mode)
        or not stat.S_ISDIR(info.st_mode)
        or (mode != 0o700 if exact_mode else bool(mode & 0o077))
        or info.st_uid != getattr(os, "geteuid", lambda: info.st_uid)()
    ):
        raise AssessmentCheckpointError(f"{label}-directory-unsafe")


def _mkdir_new(path: Path, *, label: str) -> None:
    try:
        path.mkdir(mode=0o700)
    except FileExistsError as exc:
        raise AssessmentCheckpointError(f"{label}-already-exists-no-resume") from exc
    _private_directory(path, label=label)
    _fsync_directory(path.parent)


def _fsync_directory(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0))
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _write_new(path: Path, raw: bytes, *, maximum_bytes: int) -> str:
    if type(raw) is not bytes or not raw or len(raw) > maximum_bytes:
        raise AssessmentCheckpointError("checkpoint-artifact-size-invalid")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags, 0o600)
    except FileExistsError as exc:
        raise AssessmentCheckpointError("checkpoint-artifact-already-exists-no-resume") from exc
    try:
        view = memoryview(raw)
        while view:
            count = os.write(descriptor, view)
            if count <= 0:
                raise AssessmentCheckpointError("checkpoint-artifact-short-write")
            view = view[count:]
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    _fsync_directory(path.parent)
    return _sha(raw)


def _read_private(path: Path, *, maximum_bytes: int) -> bytes:
    try:
        descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    except OSError as exc:
        raise AssessmentCheckpointError("checkpoint-artifact-unavailable") from exc
    try:
        info = os.fstat(descriptor)
        if (
            not stat.S_ISREG(info.st_mode)
            or stat.S_IMODE(info.st_mode) != 0o600
            or info.st_uid != getattr(os, "geteuid", lambda: info.st_uid)()
            or info.st_size <= 0
            or info.st_size > maximum_bytes
        ):
            raise AssessmentCheckpointError("checkpoint-artifact-unsafe")
        chunks = []
        total = 0
        while True:
            chunk = os.read(descriptor, min(1_048_576, maximum_bytes + 1 - total))
            if not chunk:
                break
            chunks.append(chunk)
            total += len(chunk)
            if total > maximum_bytes:
                raise AssessmentCheckpointError("checkpoint-artifact-over-cap")
        return b"".join(chunks)
    finally:
        os.close(descriptor)


def _validate_prepared(prepared: object) -> packets.PreparedAssessmentPackets:
    if type(prepared) is not packets.PreparedAssessmentPackets:
        raise AssessmentCheckpointError("prepared-assessment-required")
    stage_uuid = _safe_uuid(prepared.packet_stage_uuid)
    _safe_uuid(prepared.source_stage_uuid)
    manifest = prepared.preassessment_manifest_bytes
    binding = prepared.private_binding_bytes
    if (
        type(manifest) is not bytes
        or not manifest
        or len(manifest) > _JSON_CAP
        or type(binding) is not bytes
        or not binding
        or len(binding) > _JSON_CAP
        or type(prepared.preassessment_packets) is not tuple
        or not prepared.preassessment_packets
        or len(prepared.preassessment_packets) > packets.MAX_GRADER_SUBMISSIONS - 32
        or type(prepared.private_binding_sha256) is not str
        or not _SHA.fullmatch(prepared.private_binding_sha256)
    ):
        raise AssessmentCheckpointError("prepared-assessment-bounds-invalid")
    manifest_sha = _sha(manifest)
    try:
        grades._validate_preparation_pins(
            prepared,
            expected_manifest_sha256=manifest_sha,
            expected_private_binding_sha256=prepared.private_binding_sha256,
        )
        grades._verify_manifest_packets(
            manifest,
            None,
            prepared.preassessment_packets,
            "preassessment",
            stage_uuid,
            manifest_sha,
        )
        required = grades._required_preassessment_packets(prepared)
        grades._expected_packets(required, answer_phase=False)
    except (grades.GradeClosureError, packets.PacketPreparationError) as exc:
        raise AssessmentCheckpointError("prepared-assessment-integrity-invalid") from exc
    packet_ids: set[str] = set()
    packet_total = 0
    for packet in prepared.preassessment_packets:
        if type(packet) is not dict:
            raise AssessmentCheckpointError("prepared-packet-row-invalid")
        packet_id = packet.get("packet_id")
        raw = packet.get("bytes")
        if (
            type(packet_id) is not str
            or not _PACKET_ID.fullmatch(packet_id)
            or packet_id in packet_ids
            or type(raw) is not bytes
            or not raw
            or len(raw) > packets.MAX_PACKET_BYTES
            or _sha(raw) != packet.get("sha256")
            or len(raw) != packet.get("byte_count")
        ):
            raise AssessmentCheckpointError("prepared-packet-binding-invalid")
        packet_ids.add(packet_id)
        packet_total += len(raw)
    if packet_total > _PREPARED_PACKET_TOTAL_CAP:
        raise AssessmentCheckpointError("prepared-packet-total-over-cap")
    return prepared


def _prepared_index(prepared: packets.PreparedAssessmentPackets) -> dict[str, object]:
    return {
        "schema": PREPARED_SCHEMA,
        "status": "prepared-before-grading",
        "stage_uuid": prepared.packet_stage_uuid,
        "source_stage_uuid": prepared.source_stage_uuid,
        "manifest_sha256": _sha(prepared.preassessment_manifest_bytes),
        "private_binding_sha256": _sha(prepared.private_binding_bytes),
        "packet_count": len(prepared.preassessment_packets),
        "packets": [
            {
                "packet_id": packet["packet_id"],
                "task_id": packet["task_id"],
                "assessor_id": packet["assessor_id"],
                "role": packet["role"],
                "sha256": _sha(packet["bytes"]),
                "byte_count": len(packet["bytes"]),
            }
            for packet in prepared.preassessment_packets
        ],
    }


def persist_prepared_assessment(
    root: str | os.PathLike[str], prepared: packets.PreparedAssessmentPackets
) -> PreparedAssessmentCheckpoint:
    """Persist exact prepared bytes before handing packets to a grader."""
    prepared = _validate_prepared(prepared)
    root_path = Path(root)
    _private_directory(root_path, label="checkpoint-root")
    stage_dir = root_path / _safe_uuid(prepared.packet_stage_uuid)
    _mkdir_new(stage_dir, label="assessment-checkpoint")
    prepared_dir = stage_dir / "prepared"
    _mkdir_new(prepared_dir, label="prepared-checkpoint")
    packets_dir = prepared_dir / "packets"
    _mkdir_new(packets_dir, label="prepared-packets")

    _write_new(prepared_dir / "private-binding.json", prepared.private_binding_bytes, maximum_bytes=_JSON_CAP)
    _write_new(
        prepared_dir / "preassessment-manifest.json", prepared.preassessment_manifest_bytes, maximum_bytes=_JSON_CAP
    )
    for packet in prepared.preassessment_packets:
        raw = packet["bytes"]
        packet_id = packet["packet_id"]
        _write_new(packets_dir / f"{packet_id}.json", raw, maximum_bytes=packets.MAX_PACKET_BYTES)
    index_bytes = _canonical(_prepared_index(prepared))
    index_sha = _write_new(prepared_dir / "prepared-index.json", index_bytes, maximum_bytes=_JSON_CAP)
    _fsync_directory(stage_dir)
    return PreparedAssessmentCheckpoint(prepared.packet_stage_uuid, stage_dir, index_sha)


def _verify_prepared_checkpoint(
    checkpoint: PreparedAssessmentCheckpoint, prepared: packets.PreparedAssessmentPackets
) -> tuple[Path, bytes]:
    prepared = _validate_prepared(prepared)
    if (
        type(checkpoint) is not PreparedAssessmentCheckpoint
        or checkpoint.stage_uuid != prepared.packet_stage_uuid
        or checkpoint.directory.name != checkpoint.stage_uuid
        or type(checkpoint.index_sha256) is not str
        or not _SHA.fullmatch(checkpoint.index_sha256)
    ):
        raise AssessmentCheckpointError("prepared-checkpoint-binding-invalid")
    stage_dir = checkpoint.directory
    _private_directory(stage_dir, label="assessment-checkpoint")
    prepared_dir = stage_dir / "prepared"
    packets_dir = prepared_dir / "packets"
    _private_directory(prepared_dir, label="prepared-checkpoint")
    _private_directory(packets_dir, label="prepared-packets")
    index_bytes = _read_private(prepared_dir / "prepared-index.json", maximum_bytes=_JSON_CAP)
    if _sha(index_bytes) != checkpoint.index_sha256 or index_bytes != _canonical(_prepared_index(prepared)):
        raise AssessmentCheckpointError("prepared-checkpoint-index-mismatch")
    for path, expected, cap in (
        (prepared_dir / "private-binding.json", prepared.private_binding_bytes, _JSON_CAP),
        (prepared_dir / "preassessment-manifest.json", prepared.preassessment_manifest_bytes, _JSON_CAP),
    ):
        if _read_private(path, maximum_bytes=cap) != expected:
            raise AssessmentCheckpointError("prepared-checkpoint-artifact-mismatch")
    for packet in prepared.preassessment_packets:
        packet_id = packet["packet_id"]
        if _read_private(packets_dir / f"{packet_id}.json", maximum_bytes=packets.MAX_PACKET_BYTES) != packet["bytes"]:
            raise AssessmentCheckpointError("prepared-checkpoint-packet-mismatch")
    return stage_dir, index_bytes


def persist_validated_preassessment_closure(
    checkpoint: PreparedAssessmentCheckpoint,
    prepared: packets.PreparedAssessmentPackets,
    submissions: Sequence[grades.GradeSubmission],
    closure: grades.GradeClosure,
) -> str:
    """Revalidate a completed closure and persist it once, after validation."""
    stage_dir, prepared_index_bytes = _verify_prepared_checkpoint(checkpoint, prepared)
    if type(submissions) not in {list, tuple}:
        raise AssessmentCheckpointError("grade-submission-inventory-invalid")
    if len(submissions) > packets.MAX_GRADER_SUBMISSIONS - 32:
        raise AssessmentCheckpointError("grade-submission-count-over-cap")
    response_total = 0
    submission_rows = []
    for row in submissions:
        if (
            type(row) is not grades.GradeSubmission
            or type(row.response_bytes) is not bytes
            or not row.response_bytes
            or len(row.response_bytes) > grades.MAX_RESPONSE_BYTES
        ):
            raise AssessmentCheckpointError("grade-submission-invalid")
        response_total += len(row.response_bytes)
        submission_rows.append(row)
    if response_total > _CLOSED_RESPONSE_TOTAL_CAP:
        raise AssessmentCheckpointError("grade-submission-total-over-cap")
    try:
        expected = grades.close_preassessment(
            prepared,
            list(submission_rows),
            expected_manifest_sha256=_sha(prepared.preassessment_manifest_bytes),
            expected_private_binding_sha256=prepared.private_binding_sha256,
        )
    except grades.GradeClosureError as exc:
        raise AssessmentCheckpointError("preassessment-closure-validation-failed") from exc
    if (
        type(closure) is not grades.GradeClosure
        or closure.phase != "preassessment"
        or closure.stage_uuid != prepared.packet_stage_uuid
        or closure.receipt_bytes != expected.receipt_bytes
        or closure.receipt_sha256 != expected.receipt_sha256
        or closure._outputs_bytes != expected._outputs_bytes
        or closure._source_outputs_bytes != expected._source_outputs_bytes
        or len(closure.receipt_bytes) > _JSON_CAP
    ):
        raise AssessmentCheckpointError("preassessment-closure-binding-invalid")

    closed_dir = stage_dir / "closed"
    _mkdir_new(closed_dir, label="closed-checkpoint")
    response_dir = closed_dir / "submissions"
    _mkdir_new(response_dir, label="closed-submissions")
    response_index = []
    for row in sorted(submission_rows, key=lambda item: item.packet_id):
        response_sha = _write_new(
            response_dir / f"{row.packet_id}.response.json",
            row.response_bytes,
            maximum_bytes=grades.MAX_RESPONSE_BYTES,
        )
        response_index.append(
            {
                "packet_id": row.packet_id,
                "task_id": row.task_id,
                "stage_uuid": row.stage_uuid,
                "role": row.role,
                "assessor_id": row.assessor_id,
                "input_sha256": row.input_sha256,
                "response_sha256": response_sha,
                "response_byte_count": len(row.response_bytes),
            }
        )
    response_index_bytes = _canonical(
        {
            "schema": "coverage-assessment-submission-checkpoint/1",
            "stage_uuid": prepared.packet_stage_uuid,
            "submissions": response_index,
        }
    )
    response_index_sha = _write_new(closed_dir / "submission-index.json", response_index_bytes, maximum_bytes=_JSON_CAP)
    receipt_sha = _write_new(closed_dir / "grade-closure-receipt.json", closure.receipt_bytes, maximum_bytes=_JSON_CAP)
    outputs_sha = _write_new(
        closed_dir / "closed-outputs.json", closure._outputs_bytes, maximum_bytes=grades.MAX_CLOSED_OUTPUT_BYTES
    )
    source_outputs_sha = None
    if closure._source_outputs_bytes is not None:
        source_outputs_sha = _write_new(
            closed_dir / "closed-source-outputs.json",
            closure._source_outputs_bytes,
            maximum_bytes=grades.MAX_CLOSED_OUTPUT_BYTES,
        )
    index = {
        "schema": CLOSED_SCHEMA,
        "status": "validated-preassessment-closure",
        "stage_uuid": prepared.packet_stage_uuid,
        "prepared_index_sha256": _sha(prepared_index_bytes),
        "submission_index_sha256": response_index_sha,
        "grade_closure_receipt_sha256": receipt_sha,
        "closed_outputs_sha256": outputs_sha,
        "closed_source_outputs_sha256": source_outputs_sha,
        "submission_count": len(response_index),
    }
    index_sha = _write_new(closed_dir / "closure-index.json", _canonical(index), maximum_bytes=_JSON_CAP)
    _fsync_directory(stage_dir)
    return index_sha


__all__ = [
    "AssessmentCheckpointError",
    "CLOSED_SCHEMA",
    "PREPARED_SCHEMA",
    "PreparedAssessmentCheckpoint",
    "persist_prepared_assessment",
    "persist_validated_preassessment_closure",
]
