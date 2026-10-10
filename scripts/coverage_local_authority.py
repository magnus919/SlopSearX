"""Strict adapters for operator-pinned receipts and local one-shot claims.

Receipt digests are supplied separately from the receipt bytes. This module
checks exact schemas and execution bindings; it does not issue permits,
qualification, selector admission, or a cryptographic signature claim.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import stat
import uuid
from dataclasses import asdict
from pathlib import Path
from typing import Mapping

from scripts import coverage_answer_execution as answer
from scripts import coverage_live_acquire as acquisition
from scripts import coverage_source_capture as capture

SCHEMA = "coverage-operator-authority-receipt/1"
_SHA = re.compile(r"[0-9a-f]{64}\Z")


class LocalAuthorityError(ValueError):
    """An operator-pinned receipt or local lease failed closed validation."""


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")


def _strict_json(raw: bytes) -> object:
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise LocalAuthorityError("receipt-duplicate-key")
            result[key] = value
        return result

    try:
        return json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=pairs,
            parse_constant=lambda value: (_ for _ in ()).throw(ValueError(value)),
        )
    except LocalAuthorityError:
        raise
    except (UnicodeDecodeError, ValueError, json.JSONDecodeError, RecursionError) as exc:
        raise LocalAuthorityError("receipt-json-invalid") from exc


def _pinned_document(
    raw: bytes,
    expected_sha256: str,
    *,
    scope: str,
    bindings: Mapping[str, object],
    extra: Mapping[str, object] | None = None,
    allow_evidence: bool = False,
) -> dict[str, object]:
    if (
        type(raw) is not bytes
        or not raw
        or len(raw) > 16_384
        or type(expected_sha256) is not str
        or not _SHA.fullmatch(expected_sha256)
        or _sha(raw) != expected_sha256
    ):
        raise LocalAuthorityError("receipt-pin-or-binding-invalid")
    document = _strict_json(raw)
    expected = {
        "schema": SCHEMA,
        "decision": "allow",
        "scope": scope,
        "bindings": _strict_json(_canonical(dict(bindings))),
    }
    if extra is not None:
        expected["evidence"] = _strict_json(_canonical(dict(extra)))
    expected_keys = set(expected)
    if allow_evidence:
        expected_keys.add("evidence")
    if (
        type(document) is not dict
        or set(document) != expected_keys
        or any(document.get(key) != value for key, value in expected.items())
        or (allow_evidence and type(document.get("evidence")) is not dict)
        or _canonical(document) != raw
    ):
        raise LocalAuthorityError("receipt-claims-mismatch")
    return document


class PinnedReceiptAuthorities:
    """Construct existing typed permits only from exact externally pinned claims."""

    @staticmethod
    def acquisition(
        manifest: Mapping[str, object],
        manifest_sha256: str,
        receipt_bytes: bytes,
        expected_receipt_sha256: str,
    ) -> acquisition.VerifiedAcquisitionPermit:
        bindings = {
            "stage_uuid": manifest["stage_uuid"],
            "source_revision": manifest["source_revision"],
            "source_closure_sha256": manifest["source_closure_sha256"],
            "protocol_sha256": manifest["protocol_sha256"],
            "cohorts_sha256": manifest["cohorts_sha256"],
            "input_manifest_sha256": manifest["input_manifest_sha256"],
            "acquisition_plan_sha256": manifest["acquisition_plan_sha256"],
            "stage_manifest_sha256": manifest_sha256,
        }
        _pinned_document(receipt_bytes, expected_receipt_sha256, scope="source-acquisition", bindings=bindings)
        return acquisition.VerifiedAcquisitionPermit(
            status="verified",
            scope="source-acquisition",
            **bindings,
            receipt_sha256=expected_receipt_sha256,
        )

    @staticmethod
    def source_capture(
        manifest_bytes: bytes, receipt_bytes: bytes, expected_receipt_sha256: str
    ) -> capture.VerifiedSourceCapturePermit:
        manifest = _strict_json(manifest_bytes)
        if type(manifest) is not dict or type(manifest.get("sources")) is not list:
            raise LocalAuthorityError("capture-manifest-invalid")
        bindings = {
            "stage_uuid": manifest["stage_uuid"],
            "manifest_sha256": _sha(manifest_bytes),
            "protocol_sha256": manifest["protocol_sha256"],
            "source_revision": manifest["source_revision"],
            "cohorts_sha256": manifest["cohorts_sha256"],
            "candidate_identity_sha256": manifest["candidate_identity_sha256"],
            "candidate_endpoint_sha256": manifest["candidate_endpoint_sha256"],
            "source_count": len(manifest["sources"]),
            "max_owned_calls": capture.MAX_OWNED_CALLS,
            "max_health_calls": capture.MAX_HEALTH_CALLS,
            "max_scrape_calls": capture.MAX_SCRAPE_CALLS,
            "timeout_seconds": capture.REQUEST_TIMEOUT_SECONDS,
            "response_bytes": capture.MAX_RESPONSE_BYTES,
            "source_context_characters": capture.MAX_SOURCE_CHARS,
        }
        _pinned_document(receipt_bytes, expected_receipt_sha256, scope="source-capture", bindings=bindings)
        return capture.VerifiedSourceCapturePermit(
            status="verified-admitted",
            scope="source-capture",
            **bindings,
            receipt_sha256=expected_receipt_sha256,
        )

    @staticmethod
    def protected_capture_qualification(
        receipt_bytes: bytes,
        expected_receipt_sha256: str,
        bindings: capture.ProtectedCaptureQualificationBindings,
    ) -> capture.VerifiedProtectedCaptureQualification:
        expected_bindings = asdict(bindings)
        document = _pinned_document(
            receipt_bytes,
            expected_receipt_sha256,
            scope="protected-source-capture",
            bindings=expected_bindings,
            allow_evidence=True,
        )
        evidence = document.get("evidence")
        if type(evidence) is not dict:
            raise LocalAuthorityError("capture-qualification-evidence-missing")
        # Qualification evidence is an exact operator-pinned record. Its digest
        # is not independently treated as proof by this adapter.
        required = {
            "grok_image_digest",
            "grok_config_sha256",
            "protected_profile_sha256",
            "full_boundary_evidence_sha256",
        }
        if set(evidence) != required or any(
            type(value) is not str or not _SHA.fullmatch(value) for value in evidence.values()
        ):
            raise LocalAuthorityError("capture-qualification-evidence-invalid")
        return capture.VerifiedProtectedCaptureQualification(
            status="verified-qualified",
            scope="protected-source-capture",
            bindings=bindings,
            receipt_sha256=expected_receipt_sha256,
            **evidence,
        )

    @staticmethod
    def answer(
        receipt_bytes: bytes, expected_receipt_sha256: str, bindings: Mapping[str, object]
    ) -> answer.VerifiedAnswerPermit:
        document = _pinned_document(
            receipt_bytes,
            expected_receipt_sha256,
            scope="answer-execution",
            bindings=bindings,
            extra={
                "max_calls": answer.MAX_CALLS,
                "request_bytes_maximum": answer.MAX_REQUEST_BYTES,
                "response_bytes_maximum": answer.MAX_RESPONSE_BYTES,
            },
        )
        evidence = document["evidence"]
        answer_bindings = dict(bindings)
        answer_bindings["operation_ids"] = tuple(bindings["operation_ids"])
        return answer.VerifiedAnswerPermit(
            status="verified-admitted",
            **answer_bindings,
            **evidence,
            receipt_sha256=expected_receipt_sha256,
        )


class FileOneShotLease:
    """Durable O_EXCL local consumption of an already pinned permit."""

    def __init__(self, root: str | os.PathLike[str], *, scope: str) -> None:
        if type(scope) is not str or not re.fullmatch(r"[a-z][a-z0-9-]{0,47}", scope):
            raise LocalAuthorityError("lease-scope-invalid")
        self.root = Path(root)
        self.scope = scope
        try:
            self.root.mkdir(mode=0o700, parents=True, exist_ok=True)
            info = self.root.lstat()
        except OSError as exc:
            raise LocalAuthorityError("lease-root-unavailable") from exc
        if (
            stat.S_ISLNK(info.st_mode)
            or not stat.S_ISDIR(info.st_mode)
            or stat.S_IMODE(info.st_mode) != 0o700
            or info.st_uid != getattr(os, "geteuid", lambda: info.st_uid)()
        ):
            raise LocalAuthorityError("lease-root-not-private")
        self._root_identity = (info.st_dev, info.st_ino)

    def consume_once(
        self,
        permit: object,
        *,
        operation_id: str | None = None,
        request_sha256: str | None = None,
    ) -> dict[str, str]:
        stage_uuid = getattr(permit, "stage_uuid", None)
        permit_sha = getattr(permit, "receipt_sha256", None)
        if type(permit) is dict:
            stage_uuid = permit.get("stage_uuid", stage_uuid)
            permit_sha = permit.get("receipt_sha256", permit_sha)
        try:
            if type(stage_uuid) is not str or str(uuid.UUID(stage_uuid)) != stage_uuid:
                raise ValueError
        except (TypeError, ValueError) as exc:
            raise LocalAuthorityError("lease-stage-uuid-invalid") from exc
        if type(permit_sha) is not str or not _SHA.fullmatch(permit_sha):
            raise LocalAuthorityError("lease-permit-digest-invalid")
        if operation_id is not None and (
            type(operation_id) is not str or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,63}", operation_id)
        ):
            raise LocalAuthorityError("lease-operation-id-invalid")
        if request_sha256 is not None and (type(request_sha256) is not str or not _SHA.fullmatch(request_sha256)):
            raise LocalAuthorityError("lease-request-digest-invalid")
        claims = {
            "schema": "coverage-local-one-shot-lease/1",
            "scope": self.scope,
            "status": "consumed",
            "stage_uuid": stage_uuid,
            "operation_id": operation_id,
            "request_sha256": request_sha256,
            "permit_sha256": permit_sha,
        }
        raw = _canonical(claims)
        name_key = _sha(_canonical([self.scope, stage_uuid, operation_id, request_sha256, permit_sha]))
        filename = f"{name_key}.lease.json"
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
        try:
            directory_fd = os.open(
                self.root, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0)
            )
        except OSError as exc:
            raise LocalAuthorityError("lease-root-changed") from exc
        try:
            metadata = os.fstat(directory_fd)
            if (
                not stat.S_ISDIR(metadata.st_mode)
                or (metadata.st_dev, metadata.st_ino) != self._root_identity
                or stat.S_IMODE(metadata.st_mode) != 0o700
                or metadata.st_uid != getattr(os, "geteuid", lambda: metadata.st_uid)()
            ):
                raise LocalAuthorityError("lease-root-changed")
            try:
                descriptor = os.open(filename, flags, 0o600, dir_fd=directory_fd)
            except FileExistsError as exc:
                raise LocalAuthorityError("lease-already-consumed") from exc
            try:
                with os.fdopen(descriptor, "wb", closefd=False) as stream:
                    stream.write(raw)
                    stream.flush()
                    os.fsync(stream.fileno())
            finally:
                os.close(descriptor)
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
        return {"status": "consumed", "stage_uuid": stage_uuid, "receipt_sha256": _sha(raw)}
