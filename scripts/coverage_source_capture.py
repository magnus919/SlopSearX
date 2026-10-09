"""One-shot, bounded private source-capture seam for the coverage-first study.

No endpoint, credentials, or transport are discovered at import time. Callers
must provide a verified source-bound permit, one-shot lease, explicit candidate
endpoint/authentication, and an HTTPX transport. The permit object is an
integration contract, not a security boundary; this module is not yet source
qualified. Scraper-internal network fanout remains unknown and unbounded here.
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
from collections.abc import Mapping
from contextvars import ContextVar
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol
from urllib.parse import urlsplit

import httpx

from scripts import intent_ranking_receipts as receipts

MAX_OWNED_CALLS = 641
MAX_HEALTH_CALLS = 1
MAX_SCRAPE_CALLS = 640
MAX_SOURCE_CHARS = 8_000
MAX_SOURCE_CHUNK = 20
MAX_RESPONSE_BYTES = 2_000_000
REQUEST_TIMEOUT_SECONDS = 30.0
MAX_STAGE_WALL_SECONDS = 28_800.0
MAX_SOURCE_URL_BYTES = 4_096
_SHA256 = re.compile(r"[0-9a-f]{64}\Z")
_GIT_SHA = re.compile(r"[0-9a-f]{40}\Z")
_TASK_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,63}\Z")
_SOURCE_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9._:-]{0,127}\Z")
_ACTIVE_OPERATION: ContextVar[dict[str, object] | None] = ContextVar("coverage_capture_operation", default=None)


class SourceCaptureError(RuntimeError):
    """A bounded capture stopped; exception messages intentionally omit inputs."""


@dataclass(frozen=True)
class VerifiedSourceCapturePermit:
    """A verifier's exact source-capture permit; construction itself is not authority."""

    status: str
    scope: str
    stage_uuid: str
    manifest_sha256: str
    protocol_sha256: str
    source_revision: str
    cohorts_sha256: str
    candidate_identity_sha256: str
    candidate_endpoint_sha256: str
    source_count: int
    max_owned_calls: int
    max_health_calls: int
    max_scrape_calls: int
    timeout_seconds: float
    response_bytes: int
    source_context_characters: int
    receipt_sha256: str


class PermitVerifier(Protocol):
    def verify(
        self, manifest_bytes: bytes, receipt_bytes: bytes, expected_receipt_sha256: str
    ) -> VerifiedSourceCapturePermit: ...


class OneShotLease(Protocol):
    def consume_once(self, permit: VerifiedSourceCapturePermit) -> Mapping[str, object]: ...


@dataclass(frozen=True)
class SourceCaptureResult:
    """Private outcome rows plus a sanitized stage summary and receipt location."""

    status: str
    stage_uuid: str
    source_count: int
    captured_count: int
    failed_count: int
    unattempted_count: int
    owned_http_calls: int
    receipt_directory: Path
    private_inventory: tuple[Mapping[str, object], ...]
    internal_scraper_fanout: str = "unknown unless independently exposed; not counted as zero"
    quality_credit: bool = False


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical_json(value: object) -> bytes:
    encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n"
    return encoded.encode("utf-8")


def _strict_json(raw: bytes) -> object:
    def pairs(items: list[tuple[str, object]]) -> dict[str, object]:
        result: dict[str, object] = {}
        for key, value in items:
            if key in result:
                raise SourceCaptureError("duplicate-json-key")
            result[key] = value
        return result

    try:
        return json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=pairs,
            parse_constant=lambda _value: (_ for _ in ()).throw(SourceCaptureError("nonfinite-json")),
        )
    except SourceCaptureError:
        raise
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise SourceCaptureError("invalid-json") from exc


def _validate_protocol(protocol_bytes: bytes) -> tuple[dict[str, object], str]:
    obj = _strict_json(protocol_bytes)
    if type(obj) is not dict or obj.get("schema") != "coverage-first-study-protocol/1":
        raise SourceCaptureError("protocol-schema")
    capture = obj.get("capture")
    if type(capture) is not dict or (
        capture.get("context_unicode_characters") != MAX_SOURCE_CHARS
        or capture.get("max_health_calls") != MAX_HEALTH_CALLS
        or capture.get("max_owned_http_calls") != MAX_OWNED_CALLS
        or capture.get("max_owned_scrape_calls") != MAX_SCRAPE_CALLS
        or capture.get("source_chunk_maximum") != MAX_SOURCE_CHUNK
        or capture.get("response_bytes") != MAX_RESPONSE_BYTES
        or capture.get("retries") != 0
        or capture.get("timeout_seconds") != REQUEST_TIMEOUT_SECONDS
        or capture.get("one_attempt_per_distinct_public_url") is not True
    ):
        raise SourceCaptureError("protocol-capture-bounds-mismatch")
    phase = obj.get("phase_limits")
    if type(phase) is not dict or phase.get("stage_wall_seconds") != MAX_STAGE_WALL_SECONDS:
        raise SourceCaptureError("protocol-stage-deadline-mismatch")
    if type(obj.get("cohorts_sha256")) is not str or not _SHA256.fullmatch(obj["cohorts_sha256"]):
        raise SourceCaptureError("protocol-cohorts-digest-invalid")
    return obj, _sha(protocol_bytes)


def _validate_manifest(
    manifest_bytes: bytes, protocol_sha256: str
) -> tuple[dict[str, object], list[dict[str, object]]]:
    obj = _strict_json(manifest_bytes)
    required = {
        "schema",
        "stage",
        "stage_uuid",
        "source_revision",
        "protocol_sha256",
        "cohorts_sha256",
        "candidate_identity_sha256",
        "candidate_endpoint_sha256",
        "sources",
    }
    if type(obj) is not dict or set(obj) != required or obj.get("schema") != "coverage-source-capture-manifest/1":
        raise SourceCaptureError("capture-manifest-shape")
    if _canonical_json(obj) != manifest_bytes:
        raise SourceCaptureError("capture-manifest-not-canonical")
    if obj.get("stage") not in {"development", "confirmation"}:
        raise SourceCaptureError("capture-stage-invalid")
    stage_uuid = obj.get("stage_uuid")
    if type(stage_uuid) is not str:
        raise SourceCaptureError("capture-stage-uuid-invalid")
    try:
        if str(uuid.UUID(stage_uuid)) != stage_uuid:
            raise ValueError
    except ValueError as exc:
        raise SourceCaptureError("capture-stage-uuid-invalid") from exc
    if type(obj.get("source_revision")) is not str or not _GIT_SHA.fullmatch(obj["source_revision"]):
        raise SourceCaptureError("capture-source-revision-invalid")
    for field in (
        "protocol_sha256",
        "cohorts_sha256",
        "candidate_identity_sha256",
        "candidate_endpoint_sha256",
    ):
        if type(obj.get(field)) is not str or not _SHA256.fullmatch(obj[field]):
            raise SourceCaptureError(f"capture-{field}-invalid")
    if obj["protocol_sha256"] != protocol_sha256:
        raise SourceCaptureError("capture-protocol-binding-mismatch")
    rows = obj.get("sources")
    if type(rows) is not list or len(rows) > MAX_SCRAPE_CALLS:
        raise SourceCaptureError("capture-source-count-limit")
    ids: set[tuple[str, str]] = set()
    source_identity_urls: dict[str, tuple[bool, str | None]] = {}
    for row in rows:
        if type(row) is not dict or set(row) != {"task_id", "source_id", "result_index", "url", "title", "engine"}:
            raise SourceCaptureError("capture-source-row-shape")
        task_id, source_id, index = row["task_id"], row["source_id"], row["result_index"]
        if type(task_id) is not str or not _TASK_ID.fullmatch(task_id):
            raise SourceCaptureError("capture-task-id-invalid")
        identity = (task_id, source_id) if type(task_id) is str and type(source_id) is str else ("", "")
        if type(source_id) is not str or not _SOURCE_ID.fullmatch(source_id) or identity in ids:
            raise SourceCaptureError("capture-source-id-invalid-or-duplicate")
        if type(index) is not int or index < 1:
            raise SourceCaptureError("capture-result-index-invalid")
        ids.add(identity)
        if row["url"] is not None:
            try:
                url_size = len(row["url"].encode("utf-8")) if type(row["url"]) is str else MAX_SOURCE_URL_BYTES + 1
            except UnicodeEncodeError as exc:
                raise SourceCaptureError("capture-source-url-invalid") from exc
            if type(row["url"]) is not str or url_size > MAX_SOURCE_URL_BYTES:
                raise SourceCaptureError("capture-source-url-invalid")
        if type(row["title"]) is not str or len(row["title"]) > 4096:
            raise SourceCaptureError("capture-source-title-invalid")
        if type(row["engine"]) is not str or not row["engine"] or len(row["engine"]) > 64:
            raise SourceCaptureError("capture-source-engine-invalid")
        eligible, normalized_url = _eligible_public_url(row["url"])
        identity_url = normalized_url if eligible else (row["url"] if type(row["url"]) is str else None)
        source_identity = (eligible, identity_url)
        prior_url = source_identity_urls.get(source_id)
        if prior_url is not None and prior_url != source_identity:
            raise SourceCaptureError("capture-source-id-url-conflict")
        source_identity_urls[source_id] = source_identity
    return obj, rows


def _validate_candidate_identity(raw: bytes, expected_sha256: str) -> dict[str, object]:
    if type(raw) is not bytes:
        raise SourceCaptureError("candidate-identity-bytes-required")
    identity = _strict_json(raw)
    if (
        type(identity) is not dict
        or set(identity) != {"runtime"}
        or type(identity["runtime"]) is not dict
        or set(identity["runtime"]) != {"revision", "model"}
    ):
        raise SourceCaptureError("candidate-identity-shape")
    runtime = identity["runtime"]
    if type(runtime["revision"]) is not str or not _GIT_SHA.fullmatch(runtime["revision"]):
        raise SourceCaptureError("candidate-runtime-revision-invalid")
    if runtime["model"] != "free" or type(runtime["model"]) is not str:
        raise SourceCaptureError("candidate-runtime-model-invalid")
    if _canonical_json(identity) != raw:
        raise SourceCaptureError("candidate-identity-not-canonical")
    if _sha(raw) != expected_sha256:
        raise SourceCaptureError("candidate-identity-binding-mismatch")
    return identity


def _verify_permit(
    manifest: dict[str, object],
    manifest_bytes: bytes,
    permit_receipt_bytes: bytes,
    expected_permit_receipt_sha256: str,
    verifier: PermitVerifier | None,
) -> VerifiedSourceCapturePermit:
    if type(permit_receipt_bytes) is not bytes or not permit_receipt_bytes:
        raise SourceCaptureError("external-capture-permit-required")
    if type(expected_permit_receipt_sha256) is not str or not _SHA256.fullmatch(expected_permit_receipt_sha256):
        raise SourceCaptureError("capture-permit-digest-invalid")
    if _sha(permit_receipt_bytes) != expected_permit_receipt_sha256:
        raise SourceCaptureError("capture-permit-digest-mismatch")
    if verifier is None or not callable(getattr(verifier, "verify", None)):
        raise SourceCaptureError("external-capture-permit-verifier-required")
    permit = verifier.verify(manifest_bytes, permit_receipt_bytes, expected_permit_receipt_sha256)
    expected = {
        "status": "verified-admitted",
        "scope": "source-capture",
        "stage_uuid": manifest["stage_uuid"],
        "manifest_sha256": _sha(manifest_bytes),
        "protocol_sha256": manifest["protocol_sha256"],
        "source_revision": manifest["source_revision"],
        "cohorts_sha256": manifest["cohorts_sha256"],
        "candidate_identity_sha256": manifest["candidate_identity_sha256"],
        "candidate_endpoint_sha256": manifest["candidate_endpoint_sha256"],
        "source_count": len(manifest["sources"]),
        "max_owned_calls": MAX_OWNED_CALLS,
        "max_health_calls": MAX_HEALTH_CALLS,
        "max_scrape_calls": MAX_SCRAPE_CALLS,
        "timeout_seconds": REQUEST_TIMEOUT_SECONDS,
        "response_bytes": MAX_RESPONSE_BYTES,
        "source_context_characters": MAX_SOURCE_CHARS,
        "receipt_sha256": expected_permit_receipt_sha256,
    }
    if type(permit) is not VerifiedSourceCapturePermit or _canonical_json(permit.__dict__) != _canonical_json(expected):
        raise SourceCaptureError("external-capture-permit-binding-mismatch")
    return permit


def _consume_lease(lease: OneShotLease | None, permit: VerifiedSourceCapturePermit) -> str:
    if lease is None or not callable(getattr(lease, "consume_once", None)):
        raise SourceCaptureError("capture-one-shot-lease-required")
    result = lease.consume_once(permit)
    if type(result) is not dict or set(result) != {"status", "stage_uuid", "receipt_sha256"}:
        raise SourceCaptureError("capture-lease-receipt-invalid")
    if result["status"] != "consumed" or result["stage_uuid"] != permit.stage_uuid:
        raise SourceCaptureError("capture-lease-not-consumed")
    if type(result["receipt_sha256"]) is not str or not _SHA256.fullmatch(result["receipt_sha256"]):
        raise SourceCaptureError("capture-lease-receipt-invalid")
    return result["receipt_sha256"]


def _candidate_endpoints(base_url: str) -> tuple[str, str, str]:
    if type(base_url) is not str or len(base_url) > 2048:
        raise SourceCaptureError("candidate-endpoint-invalid")
    try:
        parsed = urlsplit(base_url)
        if (
            parsed.scheme != "https"
            or not parsed.hostname
            or parsed.username is not None
            or parsed.password is not None
            or parsed.query
            or parsed.fragment
        ):
            raise ValueError
        _ = parsed.port
    except ValueError as exc:
        raise SourceCaptureError("candidate-endpoint-invalid") from exc
    prefix = parsed.path.rstrip("/")
    origin = f"https://{parsed.netloc}{prefix}"
    return origin + "/health", origin + "/v2/scrape", _sha(origin.encode("utf-8"))


def _private_root(path: Path) -> Path:
    try:
        info = path.lstat()
    except OSError as exc:
        raise SourceCaptureError("capture-receipt-root-unavailable") from exc
    if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode) or stat.S_IMODE(info.st_mode) != 0o700:
        raise SourceCaptureError("capture-receipt-root-not-private")
    return path


def _fsync_directory(path: Path) -> None:
    fd = os.open(path, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def _validate_transport(transport: httpx.AsyncBaseTransport) -> None:
    if type(transport) is httpx.MockTransport:
        return
    # The fetch is delegated to the candidate scraper, whose every redirect
    # and DNS resolution/rebinding boundary is not controlled by this caller.
    # Do not treat a plain HTTPX transport's retry/proxy settings as SSRF
    # protection. Live capture stays disabled until that fetch boundary is
    # independently qualified; synthetic tests may inject only MockTransport.
    raise SourceCaptureError("live-fetch-boundary-not-qualified-mock-transport-required")


def _write_once(path: Path, raw: bytes) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        fd = os.open(path, flags, 0o600)
        with os.fdopen(fd, "wb") as stream:
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
        dir_fd = os.open(path.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
        try:
            os.fsync(dir_fd)
        finally:
            os.close(dir_fd)
    except FileExistsError as exc:
        raise SourceCaptureError("capture-private-slot-already-exists") from exc
    except OSError as exc:
        raise SourceCaptureError("capture-private-write-failed") from exc


def _write_inventory(path: Path, inventory: list[dict[str, object]], state: dict[str, object]) -> None:
    raw = _canonical_json({"schema": "coverage-source-capture-inventory/1", **state, "sources": inventory})
    temporary = path.with_name("inventory.json.tmp")
    try:
        fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_TRUNC | getattr(os, "O_NOFOLLOW", 0), 0o600)
        with os.fdopen(fd, "wb") as stream:
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
        fd = os.open(path.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
        try:
            os.fsync(fd)
        finally:
            os.close(fd)
    except OSError as exc:
        raise SourceCaptureError("capture-inventory-write-failed") from exc


def _eligible_public_url(url: object) -> tuple[bool, str]:
    if type(url) is not str or len(url.encode("utf-8")) > MAX_SOURCE_URL_BYTES:
        return False, "unsupported_url_not_attempted"
    from slopsearx.merger import _normalise_url
    from slopsearx.retrieval_url import RETRIEVAL_URL_STATUS_OK, _retrieval_url

    status, _reason, scheme, safe_url = _retrieval_url(url)
    if status != RETRIEVAL_URL_STATUS_OK or scheme not in {"http", "https"} or safe_url != url:
        return False, "unsupported_url_not_attempted"
    # Exact URL strings remain in the private manifest; this key only prevents
    # repeat requests for merger-equivalent public URLs.
    return True, _normalise_url(url)


class _CaptureTransport(httpx.AsyncBaseTransport):
    def __init__(
        self,
        inner: httpx.AsyncBaseTransport,
        stage_dir: Path,
        stage_uuid: str,
        source_revision: str,
        forbidden_response_bytes: bytes | None = None,
    ):
        self.inner = inner
        self.stage_dir = stage_dir
        self.stage_uuid = stage_uuid
        self.source_revision = source_revision
        self._forbidden_response_bytes = forbidden_response_bytes
        self.owned_calls = 0
        self.health_calls = 0
        self.scrape_calls = 0

    async def handle_async_request(self, request: httpx.Request) -> httpx.Response:
        operation = _ACTIVE_OPERATION.get()
        if operation is None or operation.get("dispatched") is True:
            raise SourceCaptureError("capture-operation-context-invalid")
        operation["dispatched"] = True
        self.owned_calls += 1
        if operation["kind"] == "health":
            self.health_calls += 1
        else:
            self.scrape_calls += 1
        if (
            self.owned_calls > MAX_OWNED_CALLS
            or self.health_calls > MAX_HEALTH_CALLS
            or self.scrape_calls > MAX_SCRAPE_CALLS
        ):
            operation["failure"] = "owned-request-cap"
            raise SourceCaptureError("capture-owned-request-cap")

        request_body = request.content
        if len(request_body) > 16_384 or request.method != operation["method"] or request.url.path != operation["path"]:
            operation["failure"] = "request-shape-mismatch"
            raise SourceCaptureError("capture-request-shape-mismatch")
        request_sha = _sha(request_body)
        binding = {
            "stage_uuid": self.stage_uuid,
            "operation_id": operation["operation_id"],
            "request_body_sha256": request_sha,
            "source_revision": self.source_revision,
        }
        response_status: int | None = None
        body = bytearray()
        decoded_body = bytearray()
        observed_body_bytes = 0
        complete = False
        status = "transport-failure"
        timeout_seconds = operation.get("timeout_seconds", REQUEST_TIMEOUT_SECONDS)
        if type(timeout_seconds) not in {int, float} or not 0 < timeout_seconds <= REQUEST_TIMEOUT_SECONDS:
            operation["failure"] = "request-timeout-invalid"
            raise SourceCaptureError("capture-request-timeout-invalid")
        try:
            async with asyncio.timeout(float(timeout_seconds)):
                response = await self.inner.handle_async_request(request)
                try:
                    response_status = response.status_code
                    # Inspect decoded bytes before the response can be archived or
                    # consumed by the caller. Keep `body` as the exact encoded
                    # transport body: it is the archived/replayed representation,
                    # and the replacement Response below lets HTTPX decode it once
                    # for the normal caller path.
                    # Mock/custom transports may return a buffered Response;
                    # HTTPX has already decoded its `_content` in that case.
                    # A network transport returns an unconsumed stream and needs
                    # a fresh decoder here.
                    already_decoded = response.content if response.is_stream_consumed else None
                    decoder = None if already_decoded is not None else response._get_content_decoder()

                    def inspect_decoded(decoded: bytes, *, enforce_cap: bool = True) -> None:
                        if not decoded:
                            return
                        forbidden = self._forbidden_response_bytes
                        if forbidden:
                            candidate = bytes(decoded_body) + decoded
                            reflected_prefix = any(
                                candidate.endswith(forbidden[:size])
                                for size in range(1, min(len(forbidden) - 1, len(candidate)) + 1)
                            )
                            if forbidden in candidate or reflected_prefix:
                                body.clear()
                                operation["failure"] = "credential-reflection-suppressed"
                                raise SourceCaptureError("credential-reflection-suppressed")
                        decoded_too_large = len(decoded_body) + len(decoded) > MAX_RESPONSE_BYTES
                        if enforce_cap and decoded_too_large:
                            body.clear()
                            operation["failure"] = "capture-decoded-response-byte-cap"
                            raise SourceCaptureError("capture-decoded-response-byte-cap")
                        if not decoded_too_large:
                            decoded_body.extend(decoded)

                    async for chunk in response.stream:
                        observed_body_bytes += len(chunk)
                        operation["observed_response_body_bytes"] = observed_body_bytes
                        if decoder is not None:
                            inspect_decoded(decoder.decode(chunk))
                        room = MAX_RESPONSE_BYTES - len(body)
                        if room > 0:
                            body.extend(chunk[:room])
                        if len(chunk) > room:
                            if already_decoded is not None:
                                inspect_decoded(already_decoded, enforce_cap=False)
                            status = "response-byte-cap"
                            raise SourceCaptureError("capture-response-byte-cap")
                    if already_decoded is not None:
                        inspect_decoded(already_decoded)
                    if decoder is not None:
                        inspect_decoded(decoder.flush())
                    complete = True
                    status = (
                        "complete" if response_status is not None and 200 <= response_status < 300 else "http-failure"
                    )
                    headers = response.headers
                    extensions = response.extensions
                finally:
                    await response.aclose()
            operation["http_status"] = response_status
            operation["response_complete"] = complete
            operation["observed_response_body_bytes"] = observed_body_bytes
            operation["response_body"] = bytes(body)
            operation["status"] = status
            archived = receipts.archive_response(
                self.stage_dir.parent,
                bindings=binding,
                complete=complete,
                status=status,
                http_status=response_status,
                response_body=bytes(body),
            )
            operation["receipt"] = archived
            return httpx.Response(
                response_status,
                headers=headers,
                stream=httpx.ByteStream(bytes(body)),
                request=request,
                extensions=extensions,
            )
        except BaseException as exc:
            if isinstance(exc, (KeyboardInterrupt, SystemExit, asyncio.CancelledError)):
                raise
            operation["http_status"] = response_status
            operation["response_complete"] = False
            operation["observed_response_body_bytes"] = observed_body_bytes
            operation["response_body"] = bytes(body)
            operation["status"] = status
            operation["failure"] = type(exc).__name__ if not isinstance(exc, SourceCaptureError) else str(exc)
            try:
                archived = receipts.archive_response(
                    self.stage_dir.parent,
                    bindings=binding,
                    complete=False,
                    status=status,
                    http_status=response_status,
                    response_body=bytes(body),
                )
                operation["receipt"] = archived
            except Exception:
                operation["archive_failed"] = True
            raise

    async def aclose(self) -> None:
        await self.inner.aclose()


def _context_artifact(stage_dir: Path, sequence: int, source: Mapping[str, object], markdown: str) -> dict[str, object]:
    context = markdown[:MAX_SOURCE_CHARS]
    context_bytes = context.encode("utf-8")
    artifact = {
        "schema": "coverage-source-context/1",
        "task_id": source["task_id"],
        "source_id": source["source_id"],
        "result_index": source["result_index"],
        "source_url": source["url"],
        "title": source["title"],
        "engine": source["engine"],
        "context": context,
        "context_characters": len(context),
        "context_sha256": _sha(context_bytes),
        "truncated": len(markdown) > MAX_SOURCE_CHARS,
        "relative_path": f"context-{sequence:04d}.json",
    }
    path = stage_dir / f"context-{sequence:04d}.json"
    _write_once(path, _canonical_json(artifact))
    return {key: value for key, value in artifact.items() if key != "context"}


async def capture_sources_once(
    *,
    protocol_bytes: bytes,
    source_manifest_bytes: bytes,
    candidate_identity_bytes: bytes,
    candidate_base_url: str,
    operator_token: str | None,
    permit_receipt_bytes: bytes,
    expected_permit_receipt_sha256: str,
    permit_verifier: PermitVerifier,
    one_shot_lease: OneShotLease,
    receipt_root: Path,
    transport: httpx.AsyncBaseTransport,
    stage_started_monotonic: float,
    stage_deadline_monotonic: float,
    clock=time.monotonic,
) -> SourceCaptureResult:
    """Capture one exact source inventory through an injected HTTPX transport.

    This accepts only explicit endpoint/token/transport inputs; it does not
    consult environment variables. One health request precedes at most one
    scrape request per distinct eligible public URL. On a transport,
    response-cap, deadline, or receipt-integrity terminal failure, remaining
    inventory rows remain uninvoked and are retained as such.
    """
    protocol, protocol_sha = _validate_protocol(protocol_bytes)
    manifest, sources = _validate_manifest(source_manifest_bytes, protocol_sha)
    if manifest["cohorts_sha256"] != protocol["cohorts_sha256"]:
        raise SourceCaptureError("capture-cohorts-binding-mismatch")
    candidate_identity = _validate_candidate_identity(
        candidate_identity_bytes, str(manifest["candidate_identity_sha256"])
    )
    health_url, scrape_url, candidate_endpoint_sha256 = _candidate_endpoints(candidate_base_url)
    if manifest["candidate_endpoint_sha256"] != candidate_endpoint_sha256:
        raise SourceCaptureError("capture-candidate-endpoint-binding-mismatch")
    if operator_token is not None and (
        type(operator_token) is not str
        or not operator_token
        or len(operator_token) > 8192
        or any(ord(char) < 0x21 or ord(char) == 0x7F for char in operator_token)
    ):
        raise SourceCaptureError("operator-token-invalid")
    _private_root(receipt_root)
    if not isinstance(transport, httpx.AsyncBaseTransport):
        raise SourceCaptureError("injected-httpx-transport-required")
    _validate_transport(transport)
    if (
        type(stage_started_monotonic) not in {int, float}
        or type(stage_deadline_monotonic) not in {int, float}
        or stage_deadline_monotonic <= stage_started_monotonic
        or stage_deadline_monotonic - stage_started_monotonic > MAX_STAGE_WALL_SECONDS
    ):
        raise SourceCaptureError("capture-stage-deadline-invalid")
    now = clock()
    if type(now) not in {int, float} or not stage_started_monotonic <= now < stage_deadline_monotonic:
        raise SourceCaptureError("capture-stage-deadline-inactive")

    permit = _verify_permit(
        manifest,
        source_manifest_bytes,
        permit_receipt_bytes,
        expected_permit_receipt_sha256,
        permit_verifier,
    )
    lease_sha = _consume_lease(one_shot_lease, permit)
    stage_uuid = str(manifest["stage_uuid"])
    stage_dir = receipt_root / stage_uuid
    try:
        stage_dir.mkdir(mode=0o700)
        os.chmod(stage_dir, 0o700)
        _fsync_directory(receipt_root)
    except FileExistsError as exc:
        raise SourceCaptureError("capture-stage-already-exists") from exc
    except OSError as exc:
        raise SourceCaptureError("capture-stage-directory-create-failed") from exc

    _write_once(stage_dir / "source-manifest.json", source_manifest_bytes)
    inventory: list[dict[str, object]] = []
    normalized_urls: set[str] = set()
    for index, source in enumerate(sources, start=1):
        eligible, key = _eligible_public_url(source["url"])
        status = "pending"
        if not eligible:
            status = "unsupported_url_not_attempted"
        elif key in normalized_urls:
            eligible = False
            status = "duplicate_url_not_attempted"
        else:
            normalized_urls.add(key)
        inventory.append(
            {
                "task_id": source["task_id"],
                "source_id": source["source_id"],
                "result_index": source["result_index"],
                "url": source["url"],
                "title": source["title"],
                "engine": source["engine"],
                "status": status,
                "attempted": False,
                "receipt_sha256": None,
                "response_sha256": None,
                "response_body_bytes": None,
                "observed_response_body_bytes": None,
                "context_sha256": None,
                "failure_code": None,
            }
        )
    state: dict[str, object] = {
        "stage_uuid": stage_uuid,
        "source_revision": manifest["source_revision"],
        "source_manifest_sha256": _sha(source_manifest_bytes),
        "protocol_sha256": protocol_sha,
        "cohorts_sha256": manifest["cohorts_sha256"],
        "candidate_identity_sha256": manifest["candidate_identity_sha256"],
        "lease_receipt_sha256": lease_sha,
        "status": "pending",
        "owned_http_calls": 0,
        "health_calls": 0,
        "scrape_calls": 0,
        "health_response_body_bytes": None,
        "health_observed_response_body_bytes": None,
        "internal_scraper_fanout": "unknown unless independently exposed; not counted as zero",
        "quality_credit": False,
    }
    inventory_path = stage_dir / "inventory.json"
    _write_inventory(inventory_path, inventory, state)
    auth_headers = {"Authorization": f"Bearer {operator_token}"} if operator_token is not None else {}
    wrapped = _CaptureTransport(
        transport,
        stage_dir,
        stage_uuid,
        str(manifest["source_revision"]),
        operator_token.encode("utf-8") if operator_token else None,
    )
    headers = {"Accept": "application/json", **auth_headers}
    health_operation: dict[str, object] = {
        "operation_id": "health",
        "kind": "health",
        "method": "GET",
        "path": urlsplit(health_url).path,
        "dispatched": False,
    }
    terminal = False
    try:
        async with httpx.AsyncClient(
            transport=wrapped,
            headers=headers,
            timeout=httpx.Timeout(REQUEST_TIMEOUT_SECONDS),
            follow_redirects=False,
            trust_env=False,
        ) as client:
            remaining = stage_deadline_monotonic - clock()
            if remaining <= 0:
                state["health_status"] = "not_attempted_stage_deadline"
                terminal = True
            else:
                health_timeout = min(REQUEST_TIMEOUT_SECONDS, remaining)
                health_operation["timeout_seconds"] = health_timeout
                _ACTIVE_OPERATION.set(health_operation)
                try:
                    response = await client.get(health_url, timeout=httpx.Timeout(health_timeout))
                    health_body = await response.aread()
                    health_payload = _strict_json(health_body) if 200 <= response.status_code < 300 else None
                    runtime = health_payload.get("runtime") if type(health_payload) is dict else None
                    if not 200 <= response.status_code < 300:
                        state["health_status"] = "health_http_failure"
                    elif type(health_payload) is not dict or health_payload.get("status") != "ok":
                        state["health_status"] = "health_payload_failure"
                    elif runtime != candidate_identity["runtime"]:
                        state["health_status"] = "health_runtime_mismatch"
                    else:
                        state["health_status"] = "healthy_runtime_match"
                    state["health_receipt_sha256"] = health_operation.get("receipt", {}).get("receipt_sha256")
                    if state["health_status"] != "healthy_runtime_match":
                        terminal = True
                    elif clock() >= stage_deadline_monotonic:
                        state["health_status"] = "completed_after_stage_deadline"
                        terminal = True
                except BaseException as exc:
                    if isinstance(exc, (KeyboardInterrupt, SystemExit, asyncio.CancelledError)):
                        raise
                    state["health_status"] = "terminal_transport_failure"
                    state["health_failure_class"] = type(exc).__name__
                    terminal = True
            health_receipt = health_operation.get("receipt", {})
            if type(health_receipt) is dict:
                state["health_response_body_bytes"] = health_receipt.get("response_body_bytes")
            state["health_observed_response_body_bytes"] = health_operation.get("observed_response_body_bytes")
            if not terminal:
                for sequence, (source, row) in enumerate(zip(sources, inventory), start=1):
                    if row["status"] != "pending":
                        continue
                    remaining = stage_deadline_monotonic - clock()
                    if remaining <= 0:
                        row["status"] = "not_attempted_stage_deadline"
                        terminal = True
                        for later in inventory[sequence:]:
                            if later["status"] == "pending":
                                later["status"] = "not_attempted_stage_deadline"
                        break
                    operation_id = f"source-{sequence:04d}"
                    request_body = _canonical_json(
                        {"url": source["url"], "formats": ["markdown"], "onlyMainContent": True}
                    )
                    operation: dict[str, object] = {
                        "operation_id": operation_id,
                        "kind": "scrape",
                        "method": "POST",
                        "path": urlsplit(scrape_url).path,
                        "dispatched": False,
                        "timeout_seconds": min(REQUEST_TIMEOUT_SECONDS, remaining),
                    }
                    row["status"] = "in_flight"
                    _write_inventory(inventory_path, inventory, state)
                    _ACTIVE_OPERATION.set(operation)
                    try:
                        response = await client.post(
                            scrape_url,
                            content=request_body,
                            headers={"Content-Type": "application/json"},
                            timeout=httpx.Timeout(float(operation["timeout_seconds"])),
                        )
                        raw_body = await response.aread()
                        row["attempted"] = bool(operation.get("dispatched"))
                        receipt_info = operation.get("receipt", {})
                        row["receipt_sha256"] = receipt_info.get("receipt_sha256")
                        row["response_sha256"] = receipt_info.get("response_body_sha256")
                        row["response_body_bytes"] = receipt_info.get("response_body_bytes")
                        row["observed_response_body_bytes"] = operation.get("observed_response_body_bytes")
                        payload = _strict_json(raw_body) if 200 <= response.status_code < 300 else None
                        data = payload.get("data", payload) if type(payload) is dict else None
                        markdown = data.get("markdown") if type(data) is dict else None
                        if (
                            response.status_code < 200
                            or response.status_code >= 300
                            or type(payload) is not dict
                            or payload.get("success") is not True
                            or type(markdown) is not str
                            or not markdown
                        ):
                            row["status"] = "source_response_failure"
                            row["failure_code"] = "http-or-payload-rejected"
                            state["status"] = "partial"
                        else:
                            context_meta = _context_artifact(stage_dir, sequence, source, markdown)
                            row["status"] = "captured"
                            row["context_sha256"] = context_meta["context_sha256"]
                            row["context_characters"] = context_meta["context_characters"]
                            row["context_truncated"] = context_meta["truncated"]
                            row["context_artifact"] = context_meta["relative_path"]
                            state["status"] = "partial"
                    except BaseException as exc:
                        if isinstance(exc, (KeyboardInterrupt, SystemExit, asyncio.CancelledError)):
                            raise
                        row["attempted"] = bool(operation.get("dispatched"))
                        receipt_info = operation.get("receipt", {})
                        row["receipt_sha256"] = receipt_info.get("receipt_sha256")
                        row["response_sha256"] = receipt_info.get("response_body_sha256")
                        row["response_body_bytes"] = receipt_info.get("response_body_bytes")
                        row["observed_response_body_bytes"] = operation.get("observed_response_body_bytes")
                        row["status"] = "terminal_capture_failure"
                        row["failure_code"] = str(operation.get("failure") or type(exc).__name__)
                        terminal = True
                    if not terminal and clock() >= stage_deadline_monotonic:
                        row["completed_after_stage_deadline"] = True
                        terminal = True
                    state["owned_http_calls"] = wrapped.owned_calls
                    state["health_calls"] = wrapped.health_calls
                    state["scrape_calls"] = wrapped.scrape_calls
                    _write_inventory(inventory_path, inventory, state)
                    if terminal:
                        for remaining in inventory[sequence:]:
                            if remaining["status"] == "pending":
                                remaining["status"] = "not_attempted_terminal_failure"
                        break
        if terminal:
            unattempted_status = (
                "not_attempted_stage_deadline"
                if state.get("health_status") in {"not_attempted_stage_deadline", "completed_after_stage_deadline"}
                else "not_attempted_terminal_failure"
            )
            for row in inventory:
                if row["status"] == "pending":
                    row["status"] = unattempted_status
    finally:
        _ACTIVE_OPERATION.set(None)
        await wrapped.aclose()

    state["owned_http_calls"] = wrapped.owned_calls
    state["health_calls"] = wrapped.health_calls
    state["scrape_calls"] = wrapped.scrape_calls
    failed = sum(row["status"] in {"source_response_failure", "terminal_capture_failure"} for row in inventory)
    captured = sum(row["status"] == "captured" for row in inventory)
    unattempted = sum(not row["attempted"] for row in inventory)
    if terminal:
        state["status"] = "terminal-incomplete"
    else:
        state["status"] = "complete" if failed == 0 else "complete-with-source-failures"
    _write_inventory(inventory_path, inventory, state)
    return SourceCaptureResult(
        status=str(state["status"]),
        stage_uuid=stage_uuid,
        source_count=len(sources),
        captured_count=captured,
        failed_count=failed,
        unattempted_count=unattempted,
        owned_http_calls=wrapped.owned_calls,
        receipt_directory=stage_dir,
        private_inventory=tuple(inventory),
    )


__all__ = [
    "OneShotLease",
    "PermitVerifier",
    "SourceCaptureError",
    "SourceCaptureResult",
    "VerifiedSourceCapturePermit",
    "capture_sources_once",
]
