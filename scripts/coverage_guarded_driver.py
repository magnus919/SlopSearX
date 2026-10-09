"""Guarded no-call driver shell for the prospective coverage study.

This module composes the pure registration preflight with an injected
source-bound admission verifier and durable one-shot selector lease. It does
not implement live acquisition, health, Jev, answer, or assessor transport.
The CLI therefore reports the missing live admission/transport seams and
never dispatches a request.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Mapping, Protocol

from scripts import coverage_study_core as core

LEASE_SCHEMA = "coverage-no-call-selector-lease/1"
TERMINAL_SCHEMA = "coverage-no-call-terminal-inventory/1"
_SHA256 = re.compile(r"[0-9a-f]{64}\Z")
_DISPATCH_GAP = "qualified-end-to-end-study-orchestration-unavailable"
UNMET_REQUIREMENTS = (
    "source-reference-closure-verifier",
    "source-qualified-live-acquisition-and-capture-admissions",
    "frozen-first40-and-41to80-scope-decisions",
    "registered-candidate-native-prejev-fallback-binding",
    "qualified-answer-and-scientific-assessment-dispatcher",
)


class DriverError(RuntimeError):
    """The no-call guard refused an unqualified or reused stage."""


@dataclass(frozen=True)
class VerifiedAdmission:
    """Result type required from an independently qualified admission adapter.

    The driver checks the binding but does not verify an admission itself.
    Construction by arbitrary callers is not a security boundary; only a
    separately source-qualified verifier may be injected in a future runner.
    """

    status: str
    scope: str
    stage_uuid: str
    registration_sha256: str
    receipt_sha256: str
    pins: Mapping[str, str]


class AdmissionVerifier(Protocol):
    def verify(
        self,
        prepared: core.PreparedStage,
        receipt_bytes: bytes,
        expected_receipt_sha256: str,
        *,
        scope: str,
    ) -> VerifiedAdmission: ...


@dataclass(frozen=True)
class NoCallResult:
    status: str
    stage_uuid: str
    admission_scope: str
    lease_sha256: str
    terminal_inventory_sha256: str
    operation_count: int
    unmet_requirements: tuple[str, ...] = UNMET_REQUIREMENTS
    provider_dispatch: bool = False
    execution_authorized: bool = False


def _canonical_json(value: object) -> bytes:
    try:
        return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode(
            "utf-8"
        )
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise DriverError("driver-record-json-invalid") from exc


def _digest(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _validate_external_admission(
    prepared: core.PreparedStage,
    receipt_bytes: bytes,
    expected_receipt_sha256: str,
    verifier: AdmissionVerifier | None,
    *,
    scope: str,
) -> VerifiedAdmission:
    if scope != "selector":
        raise DriverError("only-selector-admission-is-composed-by-this-skeleton")
    if type(receipt_bytes) is not bytes or not receipt_bytes:
        raise DriverError("admission-receipt-required")
    if type(expected_receipt_sha256) is not str or not _SHA256.fullmatch(expected_receipt_sha256):
        raise DriverError("admission-receipt-pin-invalid")
    if _digest(receipt_bytes) != expected_receipt_sha256:
        raise DriverError("admission-receipt-digest-mismatch")
    if verifier is None or not callable(getattr(verifier, "verify", None)):
        raise DriverError("qualified-admission-verifier-unavailable")

    result = verifier.verify(prepared, receipt_bytes, expected_receipt_sha256, scope=scope)
    if type(result) is not VerifiedAdmission:
        raise DriverError("admission-verifier-result-type")
    if (
        result.status != "verified-admitted"
        or result.scope != scope
        or result.stage_uuid != prepared.stage_uuid
        or result.registration_sha256 != prepared.registration_sha256
        or result.receipt_sha256 != expected_receipt_sha256
        or type(result.pins) is not dict
        or dict(result.pins) != dict(prepared.pins)
    ):
        raise DriverError("source-bound-admission-mismatch")
    return result


def _open_private_root(directory: str | os.PathLike[str]) -> int:
    path = Path(directory)
    try:
        info = path.lstat()
    except OSError as exc:
        raise DriverError("private-state-directory-unavailable") from exc
    if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode):
        raise DriverError("private-state-path-unsafe")
    if stat.S_IMODE(info.st_mode) != 0o700:
        raise DriverError("private-state-directory-mode")
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        return os.open(path, flags)
    except OSError as exc:
        raise DriverError("private-state-directory-open-failed") from exc


def _write_new_private_file(dir_fd: int, name: str, body: bytes) -> None:
    if "/" in name or name in {".", ".."}:
        raise DriverError("private-filename-invalid")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    try:
        fd = os.open(name, flags, 0o600, dir_fd=dir_fd)
    except FileExistsError as exc:
        raise DriverError("one-shot-stage-artifact-already-exists") from exc
    except OSError as exc:
        raise DriverError("one-shot-stage-artifact-create-failed") from exc
    try:
        view = memoryview(body)
        while view:
            written = os.write(fd, view)
            if written <= 0:
                raise DriverError("one-shot-stage-artifact-short-write")
            view = view[written:]
        os.fsync(fd)
    except Exception:
        # Keep partial exclusive artifacts. Their presence blocks any retry.
        raise
    finally:
        os.close(fd)
    try:
        os.fsync(dir_fd)
    except OSError as exc:
        raise DriverError("private-state-directory-fsync-failed") from exc


def prepare_no_call_selector_stage(
    *,
    preflight_kwargs: Mapping[str, object],
    admission_receipt_bytes: bytes,
    expected_admission_receipt_sha256: str,
    admission_verifier: AdmissionVerifier | None,
    private_state_directory: str | os.PathLike[str],
) -> NoCallResult:
    """Preflight, verify external admission, then consume a durable dry lease.

    This deliberately terminates with every selector slot uninvoked. It is a
    local contract test and cannot be used as a substitute for the missing
    qualified acquisition/reference/selector runtime.
    """
    prepared = core.preflight_stage(**dict(preflight_kwargs))
    if prepared.status != "preflight-verified-not-admitted" or prepared.fresh_execution_authorized:
        raise DriverError("core-preflight-status-unexpected")
    admission = _validate_external_admission(
        prepared,
        admission_receipt_bytes,
        expected_admission_receipt_sha256,
        admission_verifier,
        scope="selector",
    )

    try:
        dir_fd = _open_private_root(private_state_directory)
    except DriverError:
        raise
    try:
        lease_doc = {
            "schema": LEASE_SCHEMA,
            "status": "dry-run-consumed-not-executed",
            "stage_uuid": prepared.stage_uuid,
            "scope": admission.scope,
            "registration_sha256": prepared.registration_sha256,
            "admission_receipt_sha256": admission.receipt_sha256,
            "source_revision": prepared.source_revision,
            "pins": dict(prepared.pins),
            "operation_ids": list(prepared.operation_ids),
            "dispatch_implemented": False,
            "unmet_requirements": list(UNMET_REQUIREMENTS),
        }
        lease_bytes = _canonical_json(lease_doc)
        lease_name = f"{prepared.stage_uuid}.selector.lease.json"
        _write_new_private_file(dir_fd, lease_name, lease_bytes)

        # A dry-run is terminal by construction. The first slot records the
        # missing dispatch implementation; all remaining slots stay uninvoked.
        inventory = [
            {
                "operation_id": operation_id,
                "status": "preflight-integrity-stop" if index == 0 else "not-invoked-after-terminal-stop",
            }
            for index, operation_id in enumerate(prepared.operation_ids)
        ]
        terminal_doc = {
            "schema": TERMINAL_SCHEMA,
            "status": "no-call-skeleton-terminal",
            "terminal_reason": _DISPATCH_GAP,
            "unmet_requirements": list(UNMET_REQUIREMENTS),
            "stage_uuid": prepared.stage_uuid,
            "scope": admission.scope,
            "lease_sha256": _digest(lease_bytes),
            "operation_count": len(inventory),
            "provider_dispatch": False,
            "usage_state": "not-observed",
            "input_tokens": None,
            "output_tokens": None,
            "operations": inventory,
        }
        terminal_bytes = _canonical_json(terminal_doc)
        terminal_name = f"{prepared.stage_uuid}.selector.terminal.json"
        _write_new_private_file(dir_fd, terminal_name, terminal_bytes)
    finally:
        os.close(dir_fd)

    return NoCallResult(
        status="no-call-skeleton-terminal",
        stage_uuid=prepared.stage_uuid,
        admission_scope=admission.scope,
        lease_sha256=_digest(lease_bytes),
        terminal_inventory_sha256=_digest(terminal_bytes),
        operation_count=len(prepared.operation_ids),
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Offline status for the guarded coverage study driver.")
    parser.add_argument(
        "--status",
        action="store_true",
        help="print no-call readiness and missing integration seams; never dispatch",
    )
    args = parser.parse_args(argv)
    if not args.status:
        parser.print_help(sys.stderr)
        return 2
    print(
        json.dumps(
            {
                "status": "skeleton-no-call",
                "provider_dispatch": False,
                "credential_access": False,
                "admission_verifier": "injected-interface-only",
                "source_reference_closure": "not-implemented",
                "live_acquisition_transport": "implemented-not-qualified",
                "source_capture_transport": "implemented-not-qualified",
                "selector_transport": "implemented-not-qualified",
                "paired_consumer_inputs": "implemented-not-qualified",
                "first40_and_41to80_scope_decisions": "not-frozen",
                "candidate_native_prejev_fallback_binding": "implemented-not-registered",
                "configured_v1_control_order": "separate-and-not-executed",
                "answer_assessor_orchestration": "not-integrated",
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":  # pragma: no cover - exercised via main() in unit tests
    raise SystemExit(main())
