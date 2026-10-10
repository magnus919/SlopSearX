"""Fixed, bounded exchange for externally issued phase receipts.

The runner writes a canonical binding request. An operator-side process must
write the receipt bytes and the expected SHA-256 pin as separate files. This
module does not issue receipts, infer qualification, or trust a status field as
evidence; the caller still runs the relevant source-bound verifier.
"""

from __future__ import annotations

import asyncio
import hashlib
import json
import os
import re
import stat
import time
import uuid
from pathlib import Path
from typing import Mapping

SCHEMA = "coverage-operator-phase-handoff/1"
_SHA = re.compile(r"[0-9a-f]{64}\Z")
_SCOPE = re.compile(r"[a-z][a-z0-9-]{0,47}\Z")
_REQUEST_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,95}\Z")
MAX_REQUEST_BYTES = 128_000
MAX_RECEIPT_BYTES = 16_384
MAX_WAIT_SECONDS = 8 * 60 * 60


class OperatorHandoffError(RuntimeError):
    """A phase receipt exchange was missing, stale, or malformed."""


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    try:
        return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode(
            "utf-8"
        )
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise OperatorHandoffError("handoff-request-invalid") from exc


def _strict(raw: bytes) -> object:
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise OperatorHandoffError("handoff-duplicate-key")
            result[key] = value
        return result

    try:
        return json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=pairs,
            parse_constant=lambda value: (_ for _ in ()).throw(ValueError(value)),
        )
    except OperatorHandoffError:
        raise
    except (UnicodeDecodeError, ValueError, RecursionError) as exc:
        raise OperatorHandoffError("handoff-json-invalid") from exc


def _private_root(value: str | os.PathLike[str]) -> Path:
    root = Path(value)
    root.mkdir(mode=0o700, parents=True, exist_ok=True)
    try:
        metadata = root.lstat()
    except OSError as exc:
        raise OperatorHandoffError("handoff-root-unavailable") from exc
    if (
        stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or stat.S_IMODE(metadata.st_mode) != 0o700
        or metadata.st_uid != getattr(os, "geteuid", lambda: metadata.st_uid)()
    ):
        raise OperatorHandoffError("handoff-root-not-private")
    return root


def _check_no_secret_keys(value: object) -> None:
    if type(value) is dict:
        for key, child in value.items():
            if type(key) is not str or any(
                marker in key.casefold() for marker in ("token", "password", "secret", "api_key", "authorization")
            ):
                raise OperatorHandoffError("handoff-request-secret-field")
            _check_no_secret_keys(child)
    elif type(value) is list:
        for child in value:
            _check_no_secret_keys(child)


def _write_new(path: Path, raw: bytes) -> None:
    descriptor = os.open(
        path,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
        0o600,
    )
    try:
        with os.fdopen(descriptor, "wb", closefd=False) as stream:
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
    finally:
        os.close(descriptor)
    parent_fd = os.open(path.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0))
    try:
        os.fsync(parent_fd)
    finally:
        os.close(parent_fd)


def _read_private(path: Path, maximum: int) -> bytes:
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    try:
        metadata = os.fstat(descriptor)
        if not stat.S_ISREG(metadata.st_mode) or stat.S_IMODE(metadata.st_mode) & 0o077:
            raise OperatorHandoffError("handoff-response-not-private")
        if metadata.st_size > maximum:
            raise OperatorHandoffError("handoff-response-over-cap")
        with os.fdopen(descriptor, "rb", closefd=False) as stream:
            raw = stream.read(maximum + 1)
        if len(raw) > maximum:
            raise OperatorHandoffError("handoff-response-over-cap")
        return raw
    finally:
        os.close(descriptor)


class OperatorReceiptHandoff:
    """One-shot request/receipt exchange over a fixed private directory tree."""

    def __init__(self, root: str | os.PathLike[str], *, poll_interval: float = 0.25):
        if type(poll_interval) not in {int, float} or not 0.02 <= poll_interval <= 5:
            raise OperatorHandoffError("handoff-poll-interval-invalid")
        self.root = _private_root(root)
        self.poll_interval = float(poll_interval)

    def request(
        self,
        *,
        stage_uuid: str,
        scope: str,
        request_id: str,
        bindings: Mapping[str, object],
        deadline_monotonic: float,
    ) -> tuple[bytes, str]:
        try:
            if type(stage_uuid) is not str or str(uuid.UUID(stage_uuid)) != stage_uuid:
                raise ValueError
        except (TypeError, ValueError) as exc:
            raise OperatorHandoffError("handoff-stage-uuid-invalid") from exc
        if (
            type(scope) is not str
            or not _SCOPE.fullmatch(scope)
            or type(request_id) is not str
            or not _REQUEST_ID.fullmatch(request_id)
            or type(bindings) is not dict
            or type(deadline_monotonic) not in {int, float}
            or not 0 < deadline_monotonic - time.monotonic() <= MAX_WAIT_SECONDS
        ):
            raise OperatorHandoffError("handoff-request-fields-invalid")
        binding_bytes = _canonical(dict(bindings))
        if len(binding_bytes) > MAX_REQUEST_BYTES:
            raise OperatorHandoffError("handoff-request-over-cap")
        _check_no_secret_keys(dict(bindings))
        stage_root = self.root / stage_uuid
        stage_root.mkdir(mode=0o700, exist_ok=True)
        if stage_root.is_symlink() or stat.S_IMODE(stage_root.stat(follow_symlinks=False).st_mode) != 0o700:
            raise OperatorHandoffError("handoff-stage-root-not-private")
        scope_root = stage_root / scope
        scope_root.mkdir(mode=0o700, exist_ok=True)
        if scope_root.is_symlink() or stat.S_IMODE(scope_root.stat(follow_symlinks=False).st_mode) != 0o700:
            raise OperatorHandoffError("handoff-scope-root-not-private")
        request_root, response_root, pin_root = (scope_root / name for name in ("requests", "responses", "pins"))
        for directory in (request_root, response_root, pin_root):
            directory.mkdir(mode=0o700, exist_ok=True)
            metadata = directory.stat(follow_symlinks=False)
            if directory.is_symlink() or not stat.S_ISDIR(metadata.st_mode) or stat.S_IMODE(metadata.st_mode) != 0o700:
                raise OperatorHandoffError("handoff-subdirectory-not-private")
        request_sha = _sha(binding_bytes)
        request = {
            "schema": SCHEMA,
            "stage_uuid": stage_uuid,
            "scope": scope,
            "request_id": request_id,
            "request_sha256": request_sha,
            "bindings": _strict(binding_bytes),
        }
        try:
            _write_new(request_root / f"{request_id}.json", _canonical(request))
        except FileExistsError as exc:
            raise OperatorHandoffError("handoff-request-already-created-no-resume") from exc
        response_path = response_root / f"{request_id}.receipt"
        pin_path = pin_root / f"{request_id}.sha256"
        while time.monotonic() < deadline_monotonic:
            if response_path.exists() and pin_path.exists():
                receipt_bytes = _read_private(response_path, MAX_RECEIPT_BYTES)
                pin_bytes = _read_private(pin_path, 66)
                try:
                    pinned = pin_bytes.decode("ascii").strip()
                except UnicodeDecodeError as exc:
                    raise OperatorHandoffError("handoff-pin-invalid") from exc
                if not _SHA.fullmatch(pinned) or _sha(receipt_bytes) != pinned:
                    raise OperatorHandoffError("handoff-pin-mismatch")
                return receipt_bytes, pinned
            time.sleep(min(self.poll_interval, max(0.0, deadline_monotonic - time.monotonic())))
        raise OperatorHandoffError("handoff-receipt-timeout-no-resume")

    async def request_async(self, **kwargs) -> tuple[bytes, str]:
        """Wait without blocking the coordinator's event loop."""
        return await asyncio.to_thread(self.request, **kwargs)
