"""Bounded, offline archive and replay for raw intent-ranking responses.

This stores exact response bytes and integrity metadata only. It does not parse
responses, authorize execution, or make scientific/quality claims.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import stat
import uuid
from pathlib import Path

SCHEMA = "intent-ranking-response-archive/1"
MAX_BODY_BYTES = 2_000_000
_OPERATION = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,63}\Z")
_SHA256 = re.compile(r"[0-9a-f]{64}\Z")
_SOURCE_REVISION = re.compile(r"[0-9a-f]{40}\Z")
_FILES = {"response.bin", "receipt.json"}
_RECEIPT_KEYS = {
    "schema",
    "stage_uuid",
    "operation_id",
    "request_body_sha256",
    "source_revision",
    "complete",
    "status",
    "http_status",
    "response_body_sha256",
    "response_body_bytes",
}


class ReceiptError(ValueError):
    """An archive is unsafe, incomplete, unbound, or inconsistent."""


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical_json(value: object) -> bytes:
    return (
        json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":"), allow_nan=False) + "\n"
    ).encode("utf-8")


def _strict_json(raw: bytes) -> object:
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ReceiptError("duplicate-receipt-key")
            result[key] = value
        return result

    try:
        return json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=pairs,
            parse_constant=lambda _value: (_ for _ in ()).throw(ReceiptError("nonfinite-receipt-value")),
        )
    except ReceiptError:
        raise
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ReceiptError("invalid-receipt-json") from exc


def _validate_bindings(bindings: object) -> dict:
    if type(bindings) is not dict or set(bindings) != {
        "stage_uuid",
        "operation_id",
        "request_body_sha256",
        "source_revision",
    }:
        raise ReceiptError("binding-shape")
    stage_uuid = bindings["stage_uuid"]
    if type(stage_uuid) is not str:
        raise ReceiptError("stage-uuid-type")
    try:
        parsed = uuid.UUID(stage_uuid)
    except (ValueError, AttributeError) as exc:
        raise ReceiptError("stage-uuid-invalid") from exc
    if str(parsed) != stage_uuid:
        raise ReceiptError("stage-uuid-not-canonical")
    op_id = bindings["operation_id"]
    if type(op_id) is not str or not _OPERATION.fullmatch(op_id) or op_id in {".", ".."}:
        raise ReceiptError("operation-id-invalid")
    if type(bindings["request_body_sha256"]) is not str or not _SHA256.fullmatch(bindings["request_body_sha256"]):
        raise ReceiptError("request-digest-invalid")
    if type(bindings["source_revision"]) is not str or not _SOURCE_REVISION.fullmatch(bindings["source_revision"]):
        raise ReceiptError("source-revision-invalid")
    return dict(bindings)


def _root_dir(root: Path) -> Path:
    root = Path(root)
    try:
        info = root.lstat()
    except OSError as exc:
        raise ReceiptError("archive-root-unavailable") from exc
    if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode):
        raise ReceiptError("archive-root-not-directory")
    return root


def _fsync_directory(path: Path) -> None:
    flags = os.O_RDONLY
    if hasattr(os, "O_DIRECTORY"):
        flags |= os.O_DIRECTORY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        fd = os.open(path, flags)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)
    except OSError as exc:
        raise ReceiptError("archive-directory-fsync-failed") from exc


def _private_dir(parent: Path, name: str, *, create: bool) -> Path:
    path = parent / name
    try:
        info = path.lstat()
    except FileNotFoundError:
        if not create:
            raise ReceiptError("archive-directory-missing")
        try:
            path.mkdir(mode=0o700)
            _fsync_directory(parent)
        except FileExistsError:
            pass
        except OSError as exc:
            raise ReceiptError("archive-directory-create-failed") from exc
        info = path.lstat()
    except OSError as exc:
        raise ReceiptError("archive-directory-unavailable") from exc
    if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode) or stat.S_IMODE(info.st_mode) != 0o700:
        raise ReceiptError("archive-directory-unsafe")
    return path


def _claim_slot(parent: Path, name: str) -> Path:
    """Claim once, including empty slots left by an interrupted write."""
    path = parent / name
    try:
        path.mkdir(mode=0o700)
    except FileExistsError as exc:
        raise ReceiptError("archive-slot-already-exists") from exc
    except OSError as exc:
        raise ReceiptError("archive-slot-create-failed") from exc
    _fsync_directory(parent)
    return _private_dir(parent, name, create=False)


def _write_exclusive(path: Path, raw: bytes) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        fd = os.open(path, flags, 0o600)
    except FileExistsError as exc:
        raise ReceiptError("archive-slot-already-exists") from exc
    except OSError as exc:
        raise ReceiptError("archive-write-open-failed") from exc
    try:
        os.fchmod(fd, 0o600)
        view = memoryview(raw)
        while view:
            written = os.write(fd, view)
            if written <= 0:
                raise ReceiptError("archive-short-write")
            view = view[written:]
        os.fsync(fd)
    except Exception:
        # Do not unlink or repair a partial file: replay must reject it.
        raise
    finally:
        os.close(fd)
        _fsync_directory(path.parent)


def _read_private(path: Path, *, max_bytes: int) -> bytes:
    try:
        info = path.lstat()
    except OSError as exc:
        raise ReceiptError("archive-file-missing") from exc
    if (
        stat.S_ISLNK(info.st_mode)
        or not stat.S_ISREG(info.st_mode)
        or stat.S_IMODE(info.st_mode) != 0o600
        or info.st_size > max_bytes
    ):
        raise ReceiptError("archive-file-unsafe")
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        fd = os.open(path, flags)
    except OSError as exc:
        raise ReceiptError("archive-file-open-failed") from exc
    try:
        opened = os.fstat(fd)
        if (
            not stat.S_ISREG(opened.st_mode)
            or opened.st_ino != info.st_ino
            or opened.st_dev != info.st_dev
            or stat.S_IMODE(opened.st_mode) != 0o600
        ):
            raise ReceiptError("archive-file-changed")
        chunks = []
        remaining = max_bytes + 1
        while remaining:
            chunk = os.read(fd, min(65536, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        raw = b"".join(chunks)
        if len(raw) > max_bytes:
            raise ReceiptError("archive-file-too-large")
        return raw
    finally:
        os.close(fd)


def archive_response(
    root: Path, *, bindings: dict, complete: bool, status: str, http_status: int | None, response_body: bytes
) -> dict:
    """Write a single immutable response slot and return sanitized metadata.

    ``root`` must be an existing directory owned by the caller. Newly created
    stage/operation directories are 0700; body and receipt files are 0600.
    """
    bound = _validate_bindings(bindings)
    if type(response_body) is not bytes:
        raise ReceiptError("response-body-type")
    if len(response_body) > MAX_BODY_BYTES:
        raise ReceiptError("response-body-too-large")
    if type(complete) is not bool:
        raise ReceiptError("completion-type")
    if type(status) is not str or not status or len(status) > 64 or any(ord(ch) < 0x20 for ch in status):
        raise ReceiptError("status-invalid")
    if http_status is not None and (type(http_status) is not int or not 100 <= http_status <= 599):
        raise ReceiptError("http-status-invalid")

    archive_root = _root_dir(root)
    stage_dir = _private_dir(archive_root, bound["stage_uuid"], create=True)
    slot_dir = _claim_slot(stage_dir, bound["operation_id"])
    body_sha = _sha(response_body)
    receipt = {
        "schema": SCHEMA,
        **bound,
        "complete": complete,
        "status": status,
        "http_status": http_status,
        "response_body_sha256": body_sha,
        "response_body_bytes": len(response_body),
    }
    receipt_raw = _canonical_json(receipt)
    _write_exclusive(slot_dir / "response.bin", response_body)
    _write_exclusive(slot_dir / "receipt.json", receipt_raw)
    return {
        "stage_uuid": bound["stage_uuid"],
        "operation_id": bound["operation_id"],
        "receipt_sha256": _sha(receipt_raw),
        "response_body_sha256": body_sha,
        "response_body_bytes": len(response_body),
    }


def replay_response(root: Path, *, expected_bindings: dict, expected_receipt_sha256: str) -> tuple[dict, bytes]:
    """Return exact body bytes only after externally pinned receipt validation.

    This does not decode or interpret the body. The unchanged original parser
    remains responsible for accepting/rejecting its bytes.
    """
    bound = _validate_bindings(expected_bindings)
    if type(expected_receipt_sha256) is not str or not _SHA256.fullmatch(expected_receipt_sha256):
        raise ReceiptError("expected-receipt-digest-invalid")
    archive_root = _root_dir(root)
    stage_dir = _private_dir(archive_root, bound["stage_uuid"], create=False)
    slot_dir = _private_dir(stage_dir, bound["operation_id"], create=False)
    try:
        if set(os.listdir(slot_dir)) != _FILES:
            raise ReceiptError("archive-slot-inventory")
    except OSError as exc:
        raise ReceiptError("archive-slot-unreadable") from exc
    receipt_raw = _read_private(slot_dir / "receipt.json", max_bytes=16_384)
    if _sha(receipt_raw) != expected_receipt_sha256:
        raise ReceiptError("receipt-digest-mismatch")
    receipt = _strict_json(receipt_raw)
    if type(receipt) is not dict or set(receipt) != _RECEIPT_KEYS:
        raise ReceiptError("receipt-shape")
    if _canonical_json(receipt) != receipt_raw:
        raise ReceiptError("receipt-not-canonical")
    if receipt != {
        "schema": SCHEMA,
        **bound,
        "complete": receipt.get("complete"),
        "status": receipt.get("status"),
        "http_status": receipt.get("http_status"),
        "response_body_sha256": receipt.get("response_body_sha256"),
        "response_body_bytes": receipt.get("response_body_bytes"),
    }:
        raise ReceiptError("receipt-binding-mismatch")
    if type(receipt["complete"]) is not bool or receipt["complete"] is not True:
        raise ReceiptError("response-incomplete")
    if type(receipt["status"]) is not str or receipt["status"] != "complete":
        raise ReceiptError("response-status-incomplete")
    if type(receipt["http_status"]) is not int or not 200 <= receipt["http_status"] < 300:
        raise ReceiptError("response-http-failure")
    if (
        type(receipt["response_body_sha256"]) is not str
        or not _SHA256.fullmatch(receipt["response_body_sha256"])
        or type(receipt["response_body_bytes"]) is not int
        or not 0 <= receipt["response_body_bytes"] <= MAX_BODY_BYTES
    ):
        raise ReceiptError("response-metadata-invalid")
    body = _read_private(slot_dir / "response.bin", max_bytes=MAX_BODY_BYTES)
    if len(body) != receipt["response_body_bytes"] or _sha(body) != receipt["response_body_sha256"]:
        raise ReceiptError("response-body-digest-mismatch")
    return receipt, body


def verify_inventory(root: Path, expected_slots: list[dict]) -> list[tuple[dict, bytes]]:
    """Replay an exact sealed inventory; missing or additional slots fail."""
    if type(expected_slots) is not list:
        raise ReceiptError("expected-inventory-type")
    expected = {}
    for item in expected_slots:
        if type(item) is not dict or set(item) != {"bindings", "receipt_sha256"}:
            raise ReceiptError("expected-inventory-row")
        bound = _validate_bindings(item["bindings"])
        key = (bound["stage_uuid"], bound["operation_id"])
        if key in expected:
            raise ReceiptError("expected-inventory-duplicate")
        expected[key] = item["receipt_sha256"]
    archive_root = _root_dir(root)
    actual = set()
    try:
        for stage_name in os.listdir(archive_root):
            stage_dir = _private_dir(archive_root, stage_name, create=False)
            try:
                if str(uuid.UUID(stage_name)) != stage_name:
                    raise ValueError("noncanonical")
            except ValueError as exc:
                raise ReceiptError("inventory-stage-name") from exc
            for operation_id in os.listdir(stage_dir):
                if not _OPERATION.fullmatch(operation_id) or operation_id in {".", ".."}:
                    raise ReceiptError("inventory-operation-name")
                actual.add((stage_name, operation_id))
    except OSError as exc:
        raise ReceiptError("archive-inventory-unreadable") from exc
    if actual != set(expected):
        raise ReceiptError("archive-inventory-mismatch")
    return [
        replay_response(
            archive_root, expected_bindings=item["bindings"], expected_receipt_sha256=item["receipt_sha256"]
        )
        for item in expected_slots
    ]
