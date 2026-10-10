"""Durable, private handoff for externally executed native grader calls.

This module never invokes a model. It freezes packet IDs/bytes before handoff,
then accepts only exact, source-bound tool-result records written by the
authorized host. A missing or malformed submission is terminal for the phase.
"""

from __future__ import annotations

import asyncio
import base64
import hashlib
import json
import os
import re
import stat
import time
import uuid
from dataclasses import dataclass, replace
from pathlib import Path

from scripts.coverage_assessment_packets import MAX_GRADER_SUBMISSIONS
from scripts.coverage_grade_closure import GradeSubmission

SCHEMA = "coverage-native-grader-handoff/1"
RESULT_SCHEMA = "coverage-native-tool-result/1"
MAX_RESPONSE_BYTES = 2_000_000
_SHA256 = re.compile(r"[0-9a-f]{64}\Z")
_PACKET_ID = re.compile(r"[0-9a-f]{64}\Z")
_PHASE = {"references": "reference", "answers": "answer"}


class NativeHandoffError(RuntimeError):
    """The native grader handoff is incomplete, stale, or malformed."""


@dataclass(frozen=True)
class HandoffRequest:
    packet_id: str
    task_id: str
    assessor_id: str
    role: str
    packet_sha256: str
    packet_bytes: bytes


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    try:
        return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode(
            "utf-8"
        )
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise NativeHandoffError("handoff-canonical-json-invalid") from exc


def _strict(raw: bytes) -> object:
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise NativeHandoffError("duplicate-result-key")
            result[key] = value
        return result

    try:
        return json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=pairs,
            parse_constant=lambda value: (_ for _ in ()).throw(ValueError(value)),
        )
    except (UnicodeDecodeError, ValueError, RecursionError) as exc:
        raise NativeHandoffError("native-result-json-invalid") from exc


def _private_root(root: str | os.PathLike[str]) -> Path:
    path = Path(root)
    path.mkdir(mode=0o700, parents=True, exist_ok=True)
    mode = stat.S_IMODE(path.stat(follow_symlinks=False).st_mode)
    if path.is_symlink() or not path.is_dir() or mode & 0o077:
        raise NativeHandoffError("handoff-root-not-private")
    return path


def _write_new(path: Path, raw: bytes) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags, 0o600)
    try:
        with os.fdopen(descriptor, "wb", closefd=False) as stream:
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
    finally:
        os.close(descriptor)
    parent = os.open(path.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0))
    try:
        os.fsync(parent)
    finally:
        os.close(parent)


def _read_private(path: Path, *, max_bytes: int) -> bytes:
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    try:
        metadata = os.fstat(descriptor)
        if not stat.S_ISREG(metadata.st_mode) or stat.S_IMODE(metadata.st_mode) & 0o077 or metadata.st_size > max_bytes:
            raise NativeHandoffError("handoff-private-file-invalid")
        with os.fdopen(descriptor, "rb", closefd=False) as stream:
            raw = stream.read(max_bytes + 1)
        if len(raw) > max_bytes:
            raise NativeHandoffError("handoff-private-file-cap")
        return raw
    finally:
        os.close(descriptor)


def _private_directory(path: Path) -> None:
    metadata = path.stat(follow_symlinks=False)
    if path.is_symlink() or not stat.S_ISDIR(metadata.st_mode) or stat.S_IMODE(metadata.st_mode) & 0o077:
        raise NativeHandoffError("handoff-directory-not-private")


def _packet_rows(prepared_packets: object) -> tuple[HandoffRequest, ...]:
    packets = getattr(prepared_packets, "preassessment_packets", None)
    if packets is None:
        packets = getattr(prepared_packets, "packets", None)
    if type(packets) is not tuple or not packets:
        raise NativeHandoffError("prepared-packet-inventory-required")
    rows = []
    seen = set()
    for packet in packets:
        if type(packet) is not dict:
            raise NativeHandoffError("prepared-packet-row-invalid")
        packet_id = packet.get("packet_id")
        raw = packet.get("bytes")
        if (
            type(packet_id) is not str
            or not _PACKET_ID.fullmatch(packet_id)
            or packet_id in seen
            or type(raw) is not bytes
            or not raw
            or _sha(raw) != packet.get("sha256")
            or len(raw) != packet.get("byte_count")
            or packet.get("role") not in {"card", "source", "answer"}
            or packet.get("assessor_id") not in {"A", "B", "R1", "R2"}
            or type(packet.get("task_id")) is not str
            or not packet.get("task_id")
        ):
            raise NativeHandoffError("prepared-packet-binding-invalid")
        payload = _strict(raw)
        if (
            type(payload) is not dict
            or payload.get("packet_id") != packet_id
            or payload.get("task_id") != packet.get("task_id")
            or payload.get("assessor_id") != packet.get("assessor_id")
        ):
            raise NativeHandoffError("prepared-packet-identity-mismatch")
        if packet["role"] == "answer":
            if payload.get("packet_kind") != "answer":
                raise NativeHandoffError("prepared-answer-packet-kind")
        else:
            model_input = payload.get("model_input")
            if (
                type(model_input) is not dict
                or type(model_input.get("model_call_required")) is not bool
                or payload.get("role") != packet["role"]
            ):
                raise NativeHandoffError("prepared-reference-packet-kind")
            if not model_input["model_call_required"]:
                continue
        seen.add(packet_id)
        rows.append(
            HandoffRequest(
                packet_id=packet_id,
                task_id=str(packet["task_id"]),
                assessor_id=str(packet["assessor_id"]),
                role=str(packet["role"]),
                packet_sha256=_sha(raw),
                packet_bytes=raw,
            )
        )
    return tuple(rows)


class NativeGraderHandoff:
    """One-shot durable packet queue, with bounded collection of host results."""

    def __init__(self, root: str | os.PathLike[str], *, model: str = "gpt-6.1-sol") -> None:
        if model != "gpt-6.1-sol":
            raise NativeHandoffError("native-grader-model-mismatch")
        self.root = _private_root(root)
        self.model = model

    def __repr__(self) -> str:
        return "<NativeGraderHandoff>"

    async def collect(
        self,
        *,
        stage_uuid: str,
        phase: str,
        prepared_packets: object,
        deadline_monotonic: float,
        poll_interval: float = 0.1,
    ) -> tuple[GradeSubmission, ...]:
        if phase not in _PHASE:
            raise NativeHandoffError("grader-phase-invalid")
        if (
            type(deadline_monotonic) not in {int, float}
            or type(poll_interval) not in {int, float}
            or poll_interval <= 0
            or time.monotonic() >= deadline_monotonic
        ):
            raise NativeHandoffError("grader-deadline-invalid")
        requests = _packet_rows(prepared_packets)
        maximum = 32 if phase == "answers" else MAX_GRADER_SUBMISSIONS - 32
        if (
            not requests
            or len(requests) > maximum
            or (phase == "answers" and (len(requests) != 32 or any(row.role != "answer" for row in requests)))
            or (phase == "references" and any(row.role == "answer" for row in requests))
        ):
            raise NativeHandoffError("grader-call-inventory-count-invalid")
        try:
            if str(uuid.UUID(stage_uuid)) != stage_uuid:
                raise ValueError
        except (TypeError, ValueError) as exc:
            raise NativeHandoffError("grader-stage-uuid-invalid") from exc
        stage_root = self.root / stage_uuid
        try:
            stage_root.mkdir(mode=0o700)
        except FileExistsError:
            _private_directory(stage_root)
        os.chmod(stage_root, 0o700)
        scope = stage_root / phase
        try:
            scope.mkdir(mode=0o700)
        except FileExistsError as exc:
            raise NativeHandoffError("grader-handoff-already-created-no-resume") from exc
        os.chmod(scope, 0o700)
        request_root = scope / "requests"
        result_root = scope / "results"
        request_root.mkdir(mode=0o700)
        result_root.mkdir(mode=0o700)
        manifest_rows = []
        for request in requests:
            manifest_rows.append(
                {
                    "packet_id": request.packet_id,
                    "task_id": request.task_id,
                    "assessor_id": request.assessor_id,
                    "role": request.role,
                    "packet_sha256": request.packet_sha256,
                    "byte_count": len(request.packet_bytes),
                }
            )
            envelope = {
                "schema": SCHEMA,
                "stage_uuid": stage_uuid,
                "phase": phase,
                "model": self.model,
                **manifest_rows[-1],
                "packet_bytes_base64": base64.b64encode(request.packet_bytes).decode("ascii"),
            }
            _write_new(request_root / f"{request.packet_id}.request.json", _canonical(envelope))
        manifest = {
            "schema": SCHEMA,
            "stage_uuid": stage_uuid,
            "phase": phase,
            "model": self.model,
            "status": "awaiting-native-tool-results",
            "packet_count": len(requests),
            "packets": manifest_rows,
        }
        manifest_bytes = _canonical(manifest)
        _write_new(scope / "handoff-manifest.json", manifest_bytes)
        expected_ids = {request.packet_id for request in requests}
        while True:
            if time.monotonic() >= deadline_monotonic:
                raise NativeHandoffError("grader-native-handoff-deadline-expired")
            present = {path.stem.removesuffix(".result") for path in result_root.glob("*.result.json")}
            unexpected = present - expected_ids
            if unexpected:
                raise NativeHandoffError("grader-native-result-inventory-invalid")
            if present == expected_ids:
                break
            await asyncio.sleep(min(float(poll_interval), max(0.0, deadline_monotonic - time.monotonic())))
        submissions = []
        tool_call_ids = set()
        for request in requests:
            path = result_root / f"{request.packet_id}.result.json"
            try:
                raw = _read_private(path, max_bytes=MAX_RESPONSE_BYTES * 2 + 16_384)
                value = _strict(raw)
            except NativeHandoffError:
                raise
            except Exception as exc:
                raise NativeHandoffError("grader-native-result-unavailable") from exc
            required = {
                "schema",
                "stage_uuid",
                "phase",
                "model",
                "packet_id",
                "task_id",
                "assessor_id",
                "role",
                "packet_sha256",
                "tool_call_id",
                "tool_result_base64",
                "tool_result_sha256",
                "record_sha256",
            }
            if type(value) is not dict or set(value) != required:
                raise NativeHandoffError("grader-native-result-schema")
            record = dict(value)
            record_sha = record.pop("record_sha256")
            result_text = record.pop("tool_result_base64")
            try:
                response_bytes = base64.b64decode(result_text, validate=True)
            except (ValueError, TypeError) as exc:
                raise NativeHandoffError("grader-native-result-encoding") from exc
            expected_bindings = {
                "schema": RESULT_SCHEMA,
                "stage_uuid": stage_uuid,
                "phase": phase,
                "model": self.model,
                "packet_id": request.packet_id,
                "task_id": request.task_id,
                "assessor_id": request.assessor_id,
                "role": request.role,
                "packet_sha256": request.packet_sha256,
            }
            expected_record = {
                **expected_bindings,
                "tool_call_id": value.get("tool_call_id"),
                "tool_result_sha256": _sha(response_bytes),
            }
            if (
                record != expected_record
                or type(value.get("tool_call_id")) is not str
                or not value["tool_call_id"]
                or value["tool_call_id"] in tool_call_ids
                or not response_bytes
                or len(response_bytes) > MAX_RESPONSE_BYTES
                or _sha(_canonical({**record, "tool_result_base64": result_text})) != record_sha
                or _canonical(value) != raw
            ):
                raise NativeHandoffError("grader-native-result-binding-invalid")
            tool_call_ids.add(value["tool_call_id"])
            submissions.append(
                GradeSubmission(
                    packet_id=request.packet_id,
                    task_id=request.task_id,
                    stage_uuid=stage_uuid,
                    role=request.role,
                    assessor_id=request.assessor_id,
                    input_sha256=request.packet_sha256,
                    response_bytes=response_bytes,
                )
            )
        _write_new(
            scope / "results-closed.json",
            _canonical(
                {
                    "schema": SCHEMA,
                    "stage_uuid": stage_uuid,
                    "phase": phase,
                    "status": "complete",
                    "handoff_manifest_sha256": _sha(manifest_bytes),
                    "result_record_sha256": {
                        row.packet_id: _sha((result_root / f"{row.packet_id}.result.json").read_bytes())
                        for row in submissions
                    },
                }
            ),
        )
        return tuple(submissions)


def publish_native_tool_result(
    *,
    scope: str | os.PathLike[str],
    packet_id: str,
    tool_call_id: str,
    model: str,
    response_bytes: bytes,
) -> str:
    """Persist a host-returned native tool result against its frozen request."""
    scope_path = Path(scope)
    request_path = scope_path / "requests" / f"{packet_id}.request.json"
    try:
        request_raw = _read_private(request_path, max_bytes=MAX_RESPONSE_BYTES + 32_768)
        request = _strict(request_raw)
    except Exception as exc:
        raise NativeHandoffError("grader-request-unavailable") from exc
    if (
        type(request) is not dict
        or request.get("schema") != SCHEMA
        or request.get("packet_id") != packet_id
        or type(packet_id) is not str
        or not _PACKET_ID.fullmatch(packet_id)
        or model != request.get("model")
        or type(tool_call_id) is not str
        or not tool_call_id
        or type(response_bytes) is not bytes
        or not response_bytes
        or len(response_bytes) > MAX_RESPONSE_BYTES
    ):
        raise NativeHandoffError("native-tool-result-input-invalid")
    try:
        if str(uuid.UUID(request.get("stage_uuid"))) != request.get("stage_uuid"):
            raise ValueError
    except (TypeError, ValueError) as exc:
        raise NativeHandoffError("grader-request-stage-binding-invalid") from exc
    try:
        packet_raw = base64.b64decode(request["packet_bytes_base64"], validate=True)
    except Exception as exc:
        raise NativeHandoffError("grader-request-packet-invalid") from exc
    if _sha(packet_raw) != request.get("packet_sha256") or len(packet_raw) != request.get("byte_count"):
        raise NativeHandoffError("grader-request-packet-binding-invalid")
    values = {
        "schema": RESULT_SCHEMA,
        "stage_uuid": request["stage_uuid"],
        "phase": request["phase"],
        "model": model,
        "packet_id": packet_id,
        "task_id": request["task_id"],
        "assessor_id": request["assessor_id"],
        "role": request["role"],
        "packet_sha256": request["packet_sha256"],
        "tool_call_id": tool_call_id,
        "tool_result_sha256": _sha(response_bytes),
        "tool_result_base64": base64.b64encode(response_bytes).decode("ascii"),
    }
    values["record_sha256"] = _sha(_canonical(values))
    raw = _canonical(values)
    _write_new(scope_path / "results" / f"{packet_id}.result.json", raw)
    return _sha(raw)


def with_native_grader_handoff(
    executors,
    handoff: NativeGraderHandoff,
    *,
    stage_uuid: str,
    deadline_monotonic: float,
    poll_interval: float = 0.1,
):
    """Wire native tool-result collection into the existing phase executor set."""
    from scripts.coverage_stage_orchestration import StageExecutors

    if type(executors) is not StageExecutors or type(handoff) is not NativeGraderHandoff:
        raise NativeHandoffError("stage-executors-and-handoff-required")

    async def grade_references(prepared_packets):
        return await handoff.collect(
            stage_uuid=stage_uuid,
            phase="references",
            prepared_packets=prepared_packets,
            deadline_monotonic=deadline_monotonic,
            poll_interval=poll_interval,
        )

    async def grade_answers(prepared_packets):
        return await handoff.collect(
            stage_uuid=stage_uuid,
            phase="answers",
            prepared_packets=prepared_packets,
            deadline_monotonic=deadline_monotonic,
            poll_interval=poll_interval,
        )

    return replace(executors, grade_references=grade_references, grade_answers=grade_answers)
