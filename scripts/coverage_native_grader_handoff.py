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
PHASE_DEADLINE_SCHEMA = "coverage-native-phase-deadline/1"


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


def _validate_failed_dispatch_summary(
    *,
    summary_bytes: bytes,
    summary: object,
    stage_uuid: str,
    phase: str,
    expected_ids: set[str],
    result_ids: set[str],
) -> None:
    expected_keys = {
        "schema",
        "stage_uuid",
        "phase",
        "model",
        "model_list_response_sha256",
        "model_catalog_advertised_exact_model",
        "cli_executable_sha256",
        "cli_version",
        "maximum_concurrent_calls",
        "packet_count",
        "claimed_packet_ids",
        "completed_packet_ids",
        "unstarted_packet_ids",
        "failure_reasons",
        "status",
    }
    if type(summary) is not dict or set(summary) != expected_keys or _canonical(summary) != summary_bytes:
        raise NativeHandoffError("grader-native-host-failed-summary-invalid")
    if (
        summary.get("schema") != "coverage-native-host-dispatch-summary/1"
        or summary.get("stage_uuid") != stage_uuid
        or summary.get("phase") != phase
        or summary.get("model") != "gpt-6.1-sol"
        or summary.get("status") != "failed-terminal"
        or summary.get("packet_count") != len(expected_ids)
        or not _SHA256.fullmatch(str(summary.get("model_list_response_sha256")))
        or type(summary.get("model_catalog_advertised_exact_model")) is not bool
        or not _SHA256.fullmatch(str(summary.get("cli_executable_sha256")))
        or type(summary.get("cli_version")) is not str
        or not summary["cli_version"]
        or type(summary.get("maximum_concurrent_calls")) is not int
        or not 0 <= summary["maximum_concurrent_calls"] <= 2
    ):
        raise NativeHandoffError("grader-native-host-failed-summary-binding-invalid")
    claimed = summary.get("claimed_packet_ids")
    completed = summary.get("completed_packet_ids")
    unstarted = summary.get("unstarted_packet_ids")
    failures = summary.get("failure_reasons")
    if (
        type(claimed) is not list
        or type(completed) is not list
        or type(unstarted) is not list
        or any(type(packet_id) is not str for rows in (claimed, completed, unstarted) for packet_id in rows)
        or claimed != sorted(set(claimed))
        or completed != sorted(set(completed))
        or unstarted != sorted(set(unstarted))
        or not set(claimed) <= expected_ids
        or not set(completed) <= set(claimed)
        or unstarted != sorted(expected_ids - set(claimed))
        or type(failures) is not dict
        or set(failures) != set(claimed) - set(completed)
        or any(type(reason) is not str or not reason for reason in failures.values())
        or set(completed) != result_ids
    ):
        raise NativeHandoffError("grader-native-host-failed-summary-inventory-invalid")


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

    def __init__(
        self,
        root: str | os.PathLike[str],
        *,
        model: str = "gpt-6.1-sol",
        require_native_host_transcripts: bool = True,
    ) -> None:
        if model != "gpt-6.1-sol":
            raise NativeHandoffError("native-grader-model-mismatch")
        if type(require_native_host_transcripts) is not bool:
            raise NativeHandoffError("native-transcript-requirement-invalid")
        self.root = _private_root(root)
        self.model = model
        self.require_native_host_transcripts = require_native_host_transcripts

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
        _write_new(
            scope / "phase-deadline.json",
            _canonical(
                {
                    "schema": PHASE_DEADLINE_SCHEMA,
                    "stage_uuid": stage_uuid,
                    "phase": phase,
                    "deadline_monotonic": float(deadline_monotonic),
                }
            ),
        )
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
                if self.require_native_host_transcripts:
                    raise NativeHandoffError("grader-native-host-inventory-incomplete-terminal")
                raise NativeHandoffError("grader-native-handoff-deadline-expired")
            present = {path.stem.removesuffix(".result") for path in result_root.glob("*.result.json")}
            unexpected = present - expected_ids
            if unexpected:
                raise NativeHandoffError("grader-native-result-inventory-invalid")
            host_complete = True
            if self.require_native_host_transcripts:
                audit_root = scope / "native-host"
                if audit_root.exists() or audit_root.is_symlink():
                    _private_directory(audit_root)
                    summary_path = audit_root / "dispatch-summary.json"
                    if summary_path.exists():
                        summary_bytes = _read_private(summary_path, max_bytes=1_000_000)
                        summary = _strict(summary_bytes)
                        if (
                            type(summary) is not dict
                            or summary.get("stage_uuid") != stage_uuid
                            or summary.get("phase") != phase
                        ):
                            raise NativeHandoffError("grader-native-host-summary-binding-invalid")
                        if summary.get("status") == "failed-terminal":
                            _validate_failed_dispatch_summary(
                                summary_bytes=summary_bytes,
                                summary=summary,
                                stage_uuid=stage_uuid,
                                phase=phase,
                                expected_ids=expected_ids,
                                result_ids=present,
                            )
                            raise NativeHandoffError("grader-native-host-dispatch-failed-terminal")
                        if summary.get("status") != "complete":
                            raise NativeHandoffError("grader-native-host-summary-status-invalid")
                        expected_summary = {
                            "schema": "coverage-native-host-dispatch-summary/1",
                            "stage_uuid": stage_uuid,
                            "phase": phase,
                            "model": self.model,
                            "packet_count": len(requests),
                            "claimed_packet_ids": sorted(expected_ids),
                            "completed_packet_ids": sorted(expected_ids),
                            "unstarted_packet_ids": [],
                            "failure_reasons": {},
                        }
                        for name, expected in expected_summary.items():
                            if summary.get(name) != expected:
                                raise NativeHandoffError("grader-native-host-summary-inventory-invalid")
                        if (
                            type(summary.get("maximum_concurrent_calls")) is not int
                            or not 1 <= summary["maximum_concurrent_calls"] <= 2
                            or type(summary.get("cli_executable_sha256")) is not str
                            or not _SHA256.fullmatch(summary["cli_executable_sha256"])
                            or type(summary.get("cli_version")) is not str
                            or not summary["cli_version"]
                        ):
                            raise NativeHandoffError("grader-native-host-summary-metadata-invalid")
                        transcript_root = audit_root / "transcripts"
                        _private_directory(transcript_root)
                        transcript_ids = {
                            path.name.removesuffix(".transcript.json")
                            for path in transcript_root.glob("*.transcript.json")
                        }
                        if transcript_ids != expected_ids:
                            raise NativeHandoffError("grader-native-host-transcript-inventory-invalid")
                        failure_root = audit_root / "failures"
                        _private_directory(failure_root)
                        if list(failure_root.iterdir()):
                            raise NativeHandoffError("grader-native-host-failure-inventory-invalid")
                        host_complete = True
                    else:
                        host_complete = False
                else:
                    host_complete = False
            if present == expected_ids and host_complete:
                break
            await asyncio.sleep(min(float(poll_interval), max(0.0, deadline_monotonic - time.monotonic())))
        submissions = []
        tool_call_ids = set()
        result_bindings: dict[str, tuple[str, bytes]] = {}
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
            result_bindings[request.packet_id] = (value["tool_call_id"], response_bytes)
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
        native_host_evidence: dict[str, object] = {}
        if self.require_native_host_transcripts:
            native_host_evidence = self._verify_native_host_evidence(
                scope=scope,
                stage_uuid=stage_uuid,
                phase=phase,
                requests=requests,
                result_bindings=result_bindings,
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
                    **(
                        {
                            "native_host_transcript_schema": "coverage-native-host-transcript/1",
                            **native_host_evidence,
                        }
                        if self.require_native_host_transcripts
                        else {}
                    ),
                }
            ),
        )
        return tuple(submissions)

    def _verify_native_host_evidence(
        self,
        *,
        scope: Path,
        stage_uuid: str,
        phase: str,
        requests: tuple[HandoffRequest, ...],
        result_bindings: dict[str, tuple[str, bytes]],
    ) -> dict[str, object]:
        """Require an exact app-server transcript for every accepted result."""
        from scripts import coverage_native_host_dispatch as native_host

        audit_root = scope / "native-host"
        claim_root = audit_root / "claims"
        transcript_root = audit_root / "transcripts"
        _private_directory(audit_root)
        _private_directory(claim_root)
        _private_directory(transcript_root)
        summary_bytes = _read_private(audit_root / "dispatch-summary.json", max_bytes=1_000_000)
        summary = _strict(summary_bytes)
        expected_summary_keys = {
            "schema",
            "stage_uuid",
            "phase",
            "model",
            "model_list_response_sha256",
            "model_catalog_advertised_exact_model",
            "cli_executable_sha256",
            "cli_version",
            "maximum_concurrent_calls",
            "packet_count",
            "claimed_packet_ids",
            "completed_packet_ids",
            "unstarted_packet_ids",
            "failure_reasons",
            "status",
        }
        if type(summary) is not dict or set(summary) != expected_summary_keys or _canonical(summary) != summary_bytes:
            raise NativeHandoffError("grader-native-host-summary-invalid")
        model_list_path = audit_root / "model-list.response.json"
        model_list_bytes = _read_private(model_list_path, max_bytes=1_000_000)
        model_list = _strict(model_list_bytes)
        catalog_row_sha, catalog_response_sha = native_host.inspect_model_catalog(model_list)
        if (
            summary.get("schema") != "coverage-native-host-dispatch-summary/1"
            or _canonical(model_list) != model_list_bytes
            or _sha(model_list_bytes) != summary["model_list_response_sha256"]
            or _sha(_canonical(model_list)) != catalog_response_sha
            or summary["model_catalog_advertised_exact_model"] is not (catalog_row_sha is not None)
        ):
            raise NativeHandoffError("grader-native-host-model-list-canonical-invalid")
        expected_ids = {request.packet_id for request in requests}
        claim_files = list(claim_root.iterdir())
        if {path.name for path in claim_files} != {f"{packet_id}.claim.json" for packet_id in expected_ids}:
            raise NativeHandoffError("grader-native-host-claim-inventory-invalid")
        cli_sha = summary["cli_executable_sha256"]
        cli_version = summary["cli_version"]
        transcript_hashes: dict[str, str] = {}
        claim_hashes: dict[str, str] = {}
        for request in requests:
            request_bytes = _read_private(
                scope / "requests" / f"{request.packet_id}.request.json",
                max_bytes=MAX_RESPONSE_BYTES + 32_768,
            )
            claim_bytes = _read_private(claim_root / f"{request.packet_id}.claim.json", max_bytes=16_384)
            claim = _strict(claim_bytes)
            if claim != {
                "schema": "coverage-native-host-claim/1",
                "stage_uuid": stage_uuid,
                "phase": phase,
                "packet_id": request.packet_id,
                "packet_sha256": request.packet_sha256,
                "request_sha256": _sha(request_bytes),
                "cli_executable_sha256": cli_sha,
                "model": self.model,
                "reserved_before_dispatch": True,
            }:
                raise NativeHandoffError("grader-native-host-claim-binding-invalid")
            if _canonical(claim) != claim_bytes:
                raise NativeHandoffError("grader-native-host-claim-canonical-invalid")
            transcript_path = transcript_root / f"{request.packet_id}.transcript.json"
            transcript_bytes = _read_private(transcript_path, max_bytes=12_000_000)
            transcript = _strict(transcript_bytes)
            if (
                type(transcript) is not dict
                or transcript.get("cli_executable_sha256") != cli_sha
                or transcript.get("cli_version") != cli_version
            ):
                raise NativeHandoffError("grader-native-host-transcript-host-binding-invalid")
            verified = native_host.verify_transcript(
                transcript_bytes,
                stage_uuid=stage_uuid,
                phase=phase,
                packet_id=request.packet_id,
                packet_sha256=request.packet_sha256,
                expected_catalog_row_sha256=catalog_row_sha,
                expected_catalog_response_sha256=catalog_response_sha,
                expected_model=self.model,
            )
            result_tool_call_id, result_bytes = result_bindings[request.packet_id]
            if verified.turn_id != result_tool_call_id or verified.response_bytes != result_bytes:
                raise NativeHandoffError("grader-native-host-transcript-result-mismatch")
            transcript_hashes[request.packet_id] = _sha(transcript_bytes)
            claim_hashes[request.packet_id] = _sha(claim_bytes)
        expected_transcript_files = {f"{packet_id}.transcript.json" for packet_id in expected_ids}
        expected_claim_files = {f"{packet_id}.claim.json" for packet_id in expected_ids}
        if {path.name for path in transcript_root.iterdir()} != expected_transcript_files:
            raise NativeHandoffError("grader-native-host-transcript-inventory-invalid")
        if {path.name for path in claim_root.iterdir()} != expected_claim_files:
            raise NativeHandoffError("grader-native-host-claim-inventory-invalid")
        failure_root = audit_root / "failures"
        _private_directory(failure_root)
        if list(failure_root.iterdir()):
            raise NativeHandoffError("grader-native-host-failure-inventory-invalid")
        return {
            "native_host_transcript_sha256": transcript_hashes,
            "native_host_claim_sha256": claim_hashes,
            "native_host_dispatch_summary_sha256": _sha(summary_bytes),
            "model_list_response_sha256": _sha(model_list_bytes),
        }


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

    if (
        type(executors) is not StageExecutors
        or type(handoff) is not NativeGraderHandoff
        or not handoff.require_native_host_transcripts
    ):
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
