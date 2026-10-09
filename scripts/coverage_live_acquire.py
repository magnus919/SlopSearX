"""Explicitly admitted, bounded source acquisition for coverage studies.

This module has no import-time side effects and no credential/environment
lookup. Callers must supply an external permit verifier and one-shot lease.
The only live transport created here is an owned HTTPX transport with retries
and environment proxies disabled, and only after those guards succeed.
"""

from __future__ import annotations

import asyncio
import hashlib
import json
import math
import os
import re
import time
from collections import Counter
from collections.abc import Callable, Mapping
from dataclasses import dataclass, fields, is_dataclass
from pathlib import Path
from typing import Protocol

import httpx

from engines.arxiv import ArxivAdapter
from engines.github import GitHubAdapter
from engines.openalex import OpenAlexAdapter
from engines.wikipedia import WikipediaAdapter
from scripts import coverage_study_acquire as offline
from scripts import coverage_study_core as core
from scripts.coverage_study_acquire import StageAcquisition
from slopsearx.adapter import EngineAdapter

MAX_RESPONSE_BYTES = 2_000_000
MAX_PHYSICAL_REQUESTS = 53
# Each physical request material is bounded by the registration material cap;
# aggregate transfer therefore permits 53 × (2 MB response + 64 MB request).
MAX_REQUEST_MATERIAL_BYTES = core.MAX_MATERIAL_BYTES
MAX_STAGE_TRANSFER_BYTES = MAX_PHYSICAL_REQUESTS * (MAX_RESPONSE_BYTES + MAX_REQUEST_MATERIAL_BYTES)
MAX_POOL_SNAPSHOT_BYTES = core.MAX_MATERIAL_BYTES
POOL_INDEX_SCHEMA = "coverage-native-pool-snapshot-index/1"
REQUEST_TIMEOUT_SECONDS = 10.0
QUERY_PACING_SECONDS = 7.0
ARXIV_PHYSICAL_PACING_SECONDS = 3.0
_SHA256 = re.compile(r"[0-9a-f]{64}\Z")
_SOURCE_REVISION = re.compile(r"[0-9a-f]{40}\Z")
_MANIFEST_KEYS = {
    "schema",
    "stage",
    "stage_uuid",
    "source_revision",
    "source_closure_sha256",
    "protocol_sha256",
    "cohorts_sha256",
    "input_manifest_sha256",
    "acquisition_plan_sha256",
    "research_cases",
    "navigation_targets",
}


class LiveAcquisitionError(RuntimeError):
    """Admission, receipt, transport, or durable sink refused execution."""


@dataclass(frozen=True)
class VerifiedAcquisitionPermit:
    """An external verifier's exact, stage-bound acquisition permit."""

    status: str
    scope: str
    stage_uuid: str
    source_revision: str
    source_closure_sha256: str
    protocol_sha256: str
    cohorts_sha256: str
    input_manifest_sha256: str
    acquisition_plan_sha256: str
    stage_manifest_sha256: str
    receipt_sha256: str


class PermitVerifier(Protocol):
    def verify(
        self,
        manifest: Mapping[str, object],
        manifest_sha256: str,
        receipt_bytes: bytes,
        expected_receipt_sha256: str,
    ) -> VerifiedAcquisitionPermit: ...


class OneShotLease(Protocol):
    def consume_once(self, permit: VerifiedAcquisitionPermit) -> Mapping[str, object]: ...


class Pacer(Protocol):
    async def sleep(self, seconds: float) -> None: ...


@dataclass(frozen=True)
class LiveExchange:
    operation_id: str | None
    engine: str | None
    method: str
    url: str
    request_sha256: str
    request_bytes: int
    response_status: int | None
    response_sha256: str | None
    response_bytes: int
    fixture_id: str | None
    timeout_s: float | None
    accepted: bool
    failure_code: str | None = None
    redirect_allowed: bool | None = None
    dispatched: bool = False


@dataclass(frozen=True)
class LiveAcquisitionResult:
    stage: StageAcquisition
    stage_manifest_sha256: str
    permit_receipt_sha256: str
    lease_receipt_sha256: str
    receipt_directory: Path
    pool_snapshot_index_sha256: str


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _snapshot_filename(operation_id: str) -> str:
    return f"pool-{_sha(operation_id.encode('utf-8'))}.json"


def _canonical(value: object) -> bytes:
    try:
        text = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
        return (text + "\n").encode("utf-8")
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise LiveAcquisitionError("acquisition-manifest-json-invalid") from exc


def _plain(value: object) -> object:
    """Serialize complete dataclass metadata without dropping response fields."""
    if is_dataclass(value) and not isinstance(value, type):
        return {field.name: _plain(getattr(value, field.name)) for field in fields(value)}
    if isinstance(value, Mapping):
        if any(type(key) is not str for key in value):
            raise LiveAcquisitionError("pool-snapshot-metadata-key-invalid")
        return {key: _plain(item) for key, item in value.items()}
    if isinstance(value, (set, frozenset)):
        items = [_plain(item) for item in value]
        if any(type(item) is not str for item in items):
            raise LiveAcquisitionError("pool-snapshot-metadata-set-invalid")
        return sorted(items)
    if isinstance(value, (tuple, list)):
        return [_plain(item) for item in value]
    if type(value) in (str, int, float, bool, type(None)):
        if type(value) is float and not math.isfinite(value):
            raise LiveAcquisitionError("pool-snapshot-metadata-nonfinite")
        return value
    raise LiveAcquisitionError("pool-snapshot-metadata-type-invalid")


def _valid_sha(value: object, label: str) -> str:
    if type(value) is not str or not _SHA256.fullmatch(value):
        raise LiveAcquisitionError(f"{label}-sha256-invalid")
    return value


def _validate_manifest(
    manifest: Mapping[str, object], acquisition_plan_bytes: bytes
) -> tuple[dict, bytes, tuple[str, ...], int, dict[str, dict[str, int]]]:
    if type(manifest) is not dict or set(manifest) != _MANIFEST_KEYS:
        raise LiveAcquisitionError("acquisition-manifest-schema")
    if manifest["schema"] != "coverage-live-acquisition-manifest/1":
        raise LiveAcquisitionError("acquisition-manifest-version")
    if type(manifest["stage"]) is not str or not manifest["stage"]:
        raise LiveAcquisitionError("acquisition-stage-invalid")
    stage_uuid = manifest["stage_uuid"]
    if type(stage_uuid) is not str:
        raise LiveAcquisitionError("acquisition-stage-uuid-invalid")
    try:
        import uuid

        if str(uuid.UUID(stage_uuid)) != stage_uuid:
            raise ValueError
    except ValueError as exc:
        raise LiveAcquisitionError("acquisition-stage-uuid-invalid") from exc
    if type(manifest["source_revision"]) is not str or not _SOURCE_REVISION.fullmatch(manifest["source_revision"]):
        raise LiveAcquisitionError("acquisition-source-revision-invalid")
    for field in (
        "source_closure_sha256",
        "protocol_sha256",
        "cohorts_sha256",
        "input_manifest_sha256",
        "acquisition_plan_sha256",
    ):
        _valid_sha(manifest[field], field)

    research = manifest["research_cases"]
    navigation = manifest["navigation_targets"]
    if type(research) is not list or len(research) != 8 or type(navigation) is not list or len(navigation) != 5:
        raise LiveAcquisitionError("acquisition-operation-count")
    ids: list[str] = []
    operation_engines: dict[str, tuple[str, ...]] = {}
    reserved = 0
    for row in research:
        if type(row) is not dict:
            raise LiveAcquisitionError("acquisition-research-row")
        operation_id = offline._required_string(row, "task_id")
        query = offline._required_string(row, "search_query")
        if len(query.encode("utf-8")) > 4096:
            raise LiveAcquisitionError("acquisition-query-too-long")
        plan = row.get("pool_plan")
        if not isinstance(plan, Mapping):
            raise LiveAcquisitionError("acquisition-pool-plan-required")
        band = plan.get("band")
        if band not in {"research_le_40", "research_41_80"}:
            raise LiveAcquisitionError("acquisition-pool-band-invalid")
        engines = offline._validate_plan(plan, navigation=False)
        operation_engines[operation_id] = engines
        reserved += sum(offline.MAX_PHYSICAL_REQUESTS_PER_ENGINE[name] for name in engines)
        ids.append(operation_id)
    for row in navigation:
        if type(row) is not dict:
            raise LiveAcquisitionError("acquisition-navigation-row")
        operation_id = offline._required_string(row, "target_id")
        query = offline._required_string(row, "search_query")
        target = offline._required_string(row, "target_url")
        if len(query.encode("utf-8")) > 4096 or len(target.encode("utf-8")) > 2048:
            raise LiveAcquisitionError("acquisition-navigation-value-too-long")
        plan = row.get("navigation_pool_plan")
        if not isinstance(plan, Mapping):
            raise LiveAcquisitionError("acquisition-navigation-plan-required")
        engines = offline._validate_plan(plan, navigation=True)
        operation_engines[operation_id] = engines
        reserved += sum(offline.MAX_PHYSICAL_REQUESTS_PER_ENGINE[name] for name in engines)
        ids.append(operation_id)
    if len(set(ids)) != 13:
        raise LiveAcquisitionError("acquisition-operation-identities-invalid")
    if reserved > MAX_PHYSICAL_REQUESTS:
        raise LiveAcquisitionError("acquisition-reserved-request-budget")
    try:
        planned_total = core.validate_acquisition_plan(
            acquisition_plan_bytes,
            expected_sha256=manifest["acquisition_plan_sha256"],
            expected_stage_uuid=manifest["stage_uuid"],
            expected_task_ids=tuple(ids),
        )
    except Exception as exc:
        raise LiveAcquisitionError("acquisition-plan-invalid") from exc
    plan_doc = core._strict_json(acquisition_plan_bytes, "acquisition-plan")
    per_operation_limits: dict[str, dict[str, int]] = {}
    for row in plan_doc["tasks"]:
        operation_id = row["task_id"]
        engines = set(operation_engines[operation_id])
        counts = row["engine_calls"]
        if any((count > 0) != (engine in engines) for engine, count in counts.items()):
            raise LiveAcquisitionError("acquisition-plan-scope-mismatch")
        per_operation_limits[operation_id] = dict(counts)
    if planned_total > MAX_PHYSICAL_REQUESTS:
        raise LiveAcquisitionError("acquisition-plan-stage-limit")
    raw = _canonical(manifest)
    return dict(manifest), raw, tuple(ids), reserved, per_operation_limits


def _verify_permit(
    manifest: Mapping[str, object],
    manifest_sha256: str,
    receipt_bytes: bytes,
    expected_receipt_sha256: str,
    verifier: PermitVerifier | None,
) -> VerifiedAcquisitionPermit:
    if type(receipt_bytes) is not bytes or not receipt_bytes:
        raise LiveAcquisitionError("external-acquisition-permit-required")
    expected = _valid_sha(expected_receipt_sha256, "permit-receipt")
    if _sha(receipt_bytes) != expected:
        raise LiveAcquisitionError("permit-receipt-digest-mismatch")
    if verifier is None or not callable(getattr(verifier, "verify", None)):
        raise LiveAcquisitionError("external-permit-verifier-required")
    permit = verifier.verify(manifest, manifest_sha256, receipt_bytes, expected)
    if type(permit) is not VerifiedAcquisitionPermit:
        raise LiveAcquisitionError("external-permit-result-invalid")
    expected_fields = {
        "status": "verified",
        "scope": "source-acquisition",
        "stage_uuid": manifest["stage_uuid"],
        "source_revision": manifest["source_revision"],
        "source_closure_sha256": manifest["source_closure_sha256"],
        "protocol_sha256": manifest["protocol_sha256"],
        "cohorts_sha256": manifest["cohorts_sha256"],
        "input_manifest_sha256": manifest["input_manifest_sha256"],
        "acquisition_plan_sha256": manifest["acquisition_plan_sha256"],
        "stage_manifest_sha256": manifest_sha256,
        "receipt_sha256": expected,
    }
    if permit.__dict__ != expected_fields:
        raise LiveAcquisitionError("external-permit-binding-mismatch")
    return permit


def _consume_lease(lease: OneShotLease | None, permit: VerifiedAcquisitionPermit) -> str:
    if lease is None or not callable(getattr(lease, "consume_once", None)):
        raise LiveAcquisitionError("external-one-shot-lease-required")
    result = lease.consume_once(permit)
    if type(result) is not dict or set(result) != {"status", "stage_uuid", "receipt_sha256"}:
        raise LiveAcquisitionError("external-lease-receipt-shape")
    if result["status"] != "consumed" or result["stage_uuid"] != permit.stage_uuid:
        raise LiveAcquisitionError("external-lease-not-consumed")
    return _valid_sha(result["receipt_sha256"], "lease-receipt")


class PrivateReceiptSink:
    """Exclusive 0700/0600 sink for sanitized transport receipts."""

    def __init__(self, directory: str | os.PathLike[str]) -> None:
        self.path = Path(directory)
        if (
            self.path.exists()
            or self.path.is_symlink()
            or not self.path.parent.is_dir()
            or self.path.parent.is_symlink()
        ):
            raise LiveAcquisitionError("receipt-directory-not-fresh-safe")
        try:
            self.path.mkdir(mode=0o700)
            os.chmod(self.path, 0o700)
            parent_fd = os.open(self.path.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
            try:
                os.fsync(parent_fd)
            finally:
                os.close(parent_fd)
        except OSError as exc:
            raise LiveAcquisitionError("receipt-directory-create-failed") from exc
        self._sequence = 0

    def write_once(self, name: str, value: Mapping[str, object]) -> str:
        if not re.fullmatch(r"[a-z0-9][a-z0-9._-]{0,100}", name):
            raise LiveAcquisitionError("receipt-filename-invalid")
        raw = _canonical(dict(value))
        if len(raw) > MAX_POOL_SNAPSHOT_BYTES:
            raise LiveAcquisitionError("durable-artifact-overbound")
        path = self.path / name
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
        try:
            fd = os.open(path, flags, 0o600)
            with os.fdopen(fd, "wb") as stream:
                stream.write(raw)
                stream.flush()
                os.fsync(stream.fileno())
            dir_fd = os.open(self.path, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0))
            try:
                os.fsync(dir_fd)
            finally:
                os.close(dir_fd)
        except FileExistsError as exc:
            raise LiveAcquisitionError("receipt-slot-already-exists") from exc
        except OSError as exc:
            raise LiveAcquisitionError("receipt-write-failed") from exc
        return _sha(raw)

    def record_start(self, payload: Mapping[str, object]) -> str:
        self._sequence += 1
        return self.write_once(f"http-{self._sequence:04d}-start.json", payload)

    def record_final(self, sequence: int, payload: Mapping[str, object]) -> str:
        return self.write_once(f"http-{sequence:04d}-final.json", payload)


def _operation_snapshot(operation: object, manifest: Mapping[str, object], manifest_sha256: str) -> dict[str, object]:
    plans = [*manifest["research_cases"], *manifest["navigation_targets"]]
    operation_id = operation.operation_id
    plan = next(row for row in plans if row.get("task_id", row.get("target_id")) == operation_id)
    response = operation.canonical_response
    native_order = [f"c{index}" for index, _item in enumerate(response.results)] if response is not None else []
    operation_record = {
        "operation_id": operation.operation_id,
        "kind": operation.kind,
        "query": operation.query,
        "engines": list(operation.engines),
        "status": operation.status,
        "pool_count": operation.pool_count,
        "band": operation.band,
        "band_valid": operation.band_valid,
        "target_url": operation.target_url,
        "target_found_at_rank1": operation.target_found_at_rank1,
        "scope": _plain(operation.scope),
        "canonical_response": _plain(response),
        "failure_reasons": list(operation.failure_reasons),
        "exception_type": operation.exception_type,
    }
    return {
        "schema": "coverage-native-pool-snapshot/1",
        "stage": manifest["stage"],
        "stage_uuid": manifest["stage_uuid"],
        "source_revision": manifest["source_revision"],
        "source_closure_sha256": manifest["source_closure_sha256"],
        "protocol_sha256": manifest["protocol_sha256"],
        "cohorts_sha256": manifest["cohorts_sha256"],
        "input_manifest_sha256": manifest["input_manifest_sha256"],
        "acquisition_plan_sha256": manifest["acquisition_plan_sha256"],
        "stage_manifest_sha256": manifest_sha256,
        "task_plan": _plain(plan),
        "native_order": native_order,
        "operation": operation_record,
    }


def _read_canonical_artifact(
    path: Path,
    *,
    max_bytes: int,
    check_deadline: Callable[[], None] | None = None,
) -> tuple[dict, bytes]:
    if check_deadline is not None:
        check_deadline()
    if path.is_symlink() or not path.is_file():
        raise LiveAcquisitionError("pool-snapshot-artifact-path-invalid")
    mode = path.stat(follow_symlinks=False).st_mode & 0o777
    if mode != 0o600:
        raise LiveAcquisitionError("pool-snapshot-artifact-mode-invalid")
    if check_deadline is not None:
        check_deadline()
    raw = path.read_bytes()
    if check_deadline is not None:
        check_deadline()
    if not raw or len(raw) > max_bytes:
        raise LiveAcquisitionError("pool-snapshot-artifact-size-invalid")

    def no_duplicates(pairs: list[tuple[str, object]]) -> dict:
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError("duplicate-key")
            result[key] = value
        return result

    try:
        if check_deadline is not None:
            check_deadline()
        value = json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=no_duplicates,
            parse_constant=lambda _value: (_ for _ in ()).throw(ValueError("constant")),
        )
    except (UnicodeDecodeError, json.JSONDecodeError, ValueError) as exc:
        raise LiveAcquisitionError("pool-snapshot-artifact-json-invalid") from exc
    if check_deadline is not None:
        check_deadline()
    if type(value) is not dict or _canonical(value) != raw:
        raise LiveAcquisitionError("pool-snapshot-artifact-not-canonical")
    if check_deadline is not None:
        check_deadline()
    return value, raw


def verify_pool_snapshot_index(
    receipt_directory: str | os.PathLike[str],
    *,
    expected_index_sha256: str,
    expected_stage_manifest_sha256: str,
    expected_stage_uuid: str,
    expected_source_revision: str,
    deadline_monotonic: float | None = None,
) -> dict[str, object]:
    """Verify a complete durable pool index against externally supplied pins."""
    if deadline_monotonic is not None and (
        type(deadline_monotonic) not in {int, float} or not math.isfinite(deadline_monotonic) or deadline_monotonic <= 0
    ):
        raise LiveAcquisitionError("pool-snapshot-deadline-invalid")

    def check_deadline() -> None:
        if deadline_monotonic is not None and time.monotonic() >= deadline_monotonic:
            raise LiveAcquisitionError("pool-snapshot-deadline-exceeded")

    root = Path(receipt_directory)
    check_deadline()
    if root.is_symlink() or not root.is_dir() or (root.stat(follow_symlinks=False).st_mode & 0o777) != 0o700:
        raise LiveAcquisitionError("pool-snapshot-directory-invalid")
    index, raw_index = _read_canonical_artifact(
        root / "pool-snapshot-index.json", max_bytes=MAX_POOL_SNAPSHOT_BYTES, check_deadline=check_deadline
    )
    check_deadline()
    index_digest = _sha(raw_index)
    check_deadline()
    if index_digest != _valid_sha(expected_index_sha256, "pool-snapshot-index"):
        raise LiveAcquisitionError("pool-snapshot-index-digest-mismatch")
    if index.get("schema") != POOL_INDEX_SCHEMA or index.get("status") != "complete":
        raise LiveAcquisitionError("pool-snapshot-index-incomplete")
    if set(index) != {
        "schema",
        "stage",
        "stage_uuid",
        "source_revision",
        "source_closure_sha256",
        "protocol_sha256",
        "cohorts_sha256",
        "input_manifest_sha256",
        "acquisition_plan_sha256",
        "stage_manifest_sha256",
        "status",
        "operations",
    }:
        raise LiveAcquisitionError("pool-snapshot-index-schema")
    expected = {
        "stage_manifest_sha256": _valid_sha(expected_stage_manifest_sha256, "stage-manifest"),
        "stage_uuid": expected_stage_uuid,
        "source_revision": expected_source_revision,
    }
    if any(index.get(key) != value for key, value in expected.items()):
        raise LiveAcquisitionError("pool-snapshot-index-binding-mismatch")
    rows = index.get("operations")
    if type(rows) is not list or len(rows) != 13:
        raise LiveAcquisitionError("pool-snapshot-index-inventory-invalid")
    ids: set[str] = set()
    loaded: dict[str, object] = {}
    for row in rows:
        check_deadline()
        if (
            type(row) is not dict
            or set(row) != {"operation_id", "kind", "status", "file", "sha256", "bytes", "card_count"}
            or row.get("status") != "complete"
        ):
            raise LiveAcquisitionError("pool-snapshot-operation-incomplete")
        operation_id = row.get("operation_id")
        if type(operation_id) is not str or operation_id in ids:
            raise LiveAcquisitionError("pool-snapshot-operation-identity-invalid")
        ids.add(operation_id)
        filename = row.get("file")
        if filename != _snapshot_filename(operation_id):
            raise LiveAcquisitionError("pool-snapshot-operation-filename-invalid")
        if row.get("kind") not in {"research", "navigation"} or type(row.get("bytes")) is not int:
            raise LiveAcquisitionError("pool-snapshot-operation-row-invalid")
        if type(row.get("sha256")) is not str or not _SHA256.fullmatch(row["sha256"]):
            raise LiveAcquisitionError("pool-snapshot-operation-row-invalid")
        artifact, raw = _read_canonical_artifact(
            root / filename, max_bytes=MAX_POOL_SNAPSHOT_BYTES, check_deadline=check_deadline
        )
        check_deadline()
        artifact_digest = _sha(raw)
        check_deadline()
        if artifact_digest != row.get("sha256") or len(raw) != row.get("bytes"):
            raise LiveAcquisitionError("pool-snapshot-operation-digest-mismatch")
        if (
            set(artifact)
            != {
                "schema",
                "stage",
                "stage_uuid",
                "source_revision",
                "source_closure_sha256",
                "protocol_sha256",
                "cohorts_sha256",
                "input_manifest_sha256",
                "acquisition_plan_sha256",
                "stage_manifest_sha256",
                "task_plan",
                "native_order",
                "operation",
            }
            or artifact.get("schema") != "coverage-native-pool-snapshot/1"
        ):
            raise LiveAcquisitionError("pool-snapshot-artifact-schema")
        for field in (
            "stage_uuid",
            "source_revision",
            "source_closure_sha256",
            "protocol_sha256",
            "cohorts_sha256",
            "input_manifest_sha256",
            "acquisition_plan_sha256",
            "stage_manifest_sha256",
        ):
            if artifact.get(field) != index.get(field):
                raise LiveAcquisitionError("pool-snapshot-operation-binding-mismatch")
        if artifact.get("stage_manifest_sha256") != expected_stage_manifest_sha256:
            raise LiveAcquisitionError("pool-snapshot-operation-binding-mismatch")
        operation = artifact.get("operation")
        if type(operation) is not dict or operation.get("operation_id") != operation_id:
            raise LiveAcquisitionError("pool-snapshot-operation-shape-invalid")
        if operation.get("status") != "complete":
            raise LiveAcquisitionError("pool-snapshot-operation-incomplete")
        if operation.get("kind") != row.get("kind") or operation.get("pool_count") != row.get("card_count"):
            raise LiveAcquisitionError("pool-snapshot-operation-row-mismatch")
        response = operation.get("canonical_response")
        native_order = artifact.get("native_order")
        if (
            type(response) is not dict
            or type(response.get("results")) is not list
            or type(native_order) is not list
            or native_order != [f"c{index}" for index in range(len(response["results"]))]
            or len(response["results"]) != row.get("card_count")
        ):
            raise LiveAcquisitionError("pool-snapshot-native-order-invalid")
        loaded[operation_id] = artifact
    check_deadline()
    if len(ids) != 13:
        raise LiveAcquisitionError("pool-snapshot-index-inventory-invalid")
    return {"index": index, "operations": loaded}


class _BoundedTransport(httpx.AsyncBaseTransport):
    _coverage_live_transport = True

    def __init__(
        self,
        inner: httpx.AsyncBaseTransport,
        sink: PrivateReceiptSink,
        per_operation_limits: Mapping[str, Mapping[str, int]],
        pacer: Pacer,
        monotonic: Callable[[], float] = time.monotonic,
    ) -> None:
        self.inner = inner
        self.sink = sink
        self.exchanges: list[LiveExchange] = []
        self._engine_uses: Counter[tuple[str | None, str]] = Counter()
        self._physical_dispatches = 0
        self._transfer_bytes = 0
        self._operation_limits = {key: dict(value) for key, value in per_operation_limits.items()}
        self._pacer = pacer
        self._monotonic = monotonic
        self._last_arxiv_dispatch: float | None = None
        self._arxiv_dispatch_times: list[float] = []

    @property
    def transfer_bytes(self) -> int:
        return self._transfer_bytes

    @property
    def physical_dispatches(self) -> int:
        return self._physical_dispatches

    async def handle_async_request(self, request: httpx.Request) -> httpx.Response:
        operation_id = offline._CURRENT_OPERATION.get()
        expected_engines = offline._CURRENT_ENGINES.get()
        host = (request.url.host or "").lower()
        engine = next((name for name, allowed in offline.ENGINE_HOSTS.items() if host == allowed), None)
        material = offline._request_material(request)
        request_sha = _sha(material)
        timeout_s = offline._timeout_value(request)
        seq = self.sink._sequence + 1
        base = {
            "schema": "coverage-acquisition-http-receipt/1",
            "sequence": seq,
            "operation_id": operation_id,
            "engine": engine,
            "method": request.method.upper(),
            "host": host,
            "path": request.url.path,
            "request_sha256": request_sha,
            "request_bytes": len(material),
            "timeout_seconds": timeout_s,
        }
        self.sink.record_start({**base, "status": "attempted"})

        failure: str | None = None
        if operation_id is None or engine is None or engine not in expected_engines:
            failure = "unexpected_engine_or_operation"
        elif len(material) > MAX_REQUEST_MATERIAL_BYTES:
            failure = "request_material_cap"
        elif request.url.scheme != "https" or request.url.path != offline.ENGINE_PATHS[engine]:
            failure = "unexpected_endpoint"
        elif request.method.upper() != "GET":
            failure = "unexpected_method"
        elif timeout_s is None or timeout_s > REQUEST_TIMEOUT_SECONDS:
            failure = "request_timeout_exceeded"
        elif any(name.lower() in {b"authorization", b"x-api-key", b"api-key"} for name, _ in request.headers.raw):
            failure = "credential_header_forbidden"
        elif self._engine_uses[(operation_id, engine)] >= self._operation_limits[operation_id].get(engine, 0):
            failure = "per_engine_request_cap"
        elif self._physical_dispatches >= MAX_PHYSICAL_REQUESTS:
            failure = "stage_request_cap"

        if failure is not None:
            self.sink.record_final(
                seq,
                {
                    **base,
                    "status": "rejected-before-dispatch",
                    "failure_code": failure,
                    "dispatched": False,
                    "response_status": None,
                    "response_sha256": None,
                    "response_bytes": 0,
                },
            )
            self.exchanges.append(
                LiveExchange(
                    operation_id,
                    engine,
                    request.method.upper(),
                    f"https://{host}{request.url.path}",
                    request_sha,
                    len(material),
                    None,
                    None,
                    0,
                    None,
                    timeout_s,
                    False,
                    failure,
                    None,
                    False,
                )
            )
            raise httpx.ConnectError("coverage acquisition transport rejected request", request=request)

        assert operation_id is not None and engine is not None
        response_status: int | None = None
        response_body = bytearray()
        response_complete = False
        response_digest: str | None = None
        failure = None
        redirect_allowed: bool | None = None
        transfer_increment = len(material)
        response_observed = 0
        response: httpx.Response | None = None
        dispatched = False
        try:
            async with asyncio.timeout(REQUEST_TIMEOUT_SECONDS):
                if engine == "arxiv" and self._last_arxiv_dispatch is not None:
                    elapsed = self._monotonic() - self._last_arxiv_dispatch
                    remaining = ARXIV_PHYSICAL_PACING_SECONDS - elapsed
                    if remaining > 0:
                        await self._pacer.sleep(remaining)
                dispatch_at = self._monotonic()
                if engine == "arxiv":
                    self._last_arxiv_dispatch = dispatch_at
                    self._arxiv_dispatch_times.append(dispatch_at)
                self._engine_uses[(operation_id, engine)] += 1
                self._physical_dispatches += 1
                dispatched = True
                response = await self.inner.handle_async_request(request)
                response_status = response.status_code
                if 300 <= response.status_code < 400:
                    if engine == "arxiv":
                        redirect_allowed = offline._safe_redirect(request, response.headers.get("location"))
                    else:
                        redirect_allowed = False
                    if not redirect_allowed:
                        failure = "redirect_not_allowed"
                async for chunk in response.stream:
                    response_observed += len(chunk)
                    transfer_increment += len(chunk)
                    remaining_response = MAX_RESPONSE_BYTES - len(response_body)
                    if remaining_response > 0:
                        response_body.extend(chunk[:remaining_response])
                    if response_observed > MAX_RESPONSE_BYTES:
                        failure = "response_body_cap"
                        raise httpx.ReadError("acquisition response exceeded cap", request=request)
                    if self._transfer_bytes + transfer_increment > MAX_STAGE_TRANSFER_BYTES:
                        failure = "stage_transfer_byte_cap"
                        raise httpx.ReadError("acquisition stage transfer cap exceeded", request=request)
                response_complete = True
                await response.aclose()
            self._transfer_bytes += transfer_increment
            response_digest = _sha(bytes(response_body))
            accepted = failure is None
            self.sink.record_final(
                seq,
                {
                    **base,
                    "status": "complete" if accepted else "rejected-response",
                    "failure_code": failure,
                    "dispatched": True,
                    "response_status": response_status,
                    "response_sha256": response_digest,
                    "response_bytes": len(response_body),
                    "complete": response_complete,
                },
            )
            self.exchanges.append(
                LiveExchange(
                    operation_id,
                    engine,
                    request.method.upper(),
                    f"https://{host}{request.url.path}",
                    request_sha,
                    len(material),
                    response_status,
                    response_digest,
                    len(response_body),
                    None,
                    timeout_s,
                    accepted,
                    failure,
                    redirect_allowed,
                    True,
                )
            )
            return httpx.Response(
                response_status,
                headers=response.headers,
                stream=httpx.ByteStream(bytes(response_body)),
                request=request,
                extensions=response.extensions,
            )
        except BaseException as exc:
            if isinstance(exc, (KeyboardInterrupt, SystemExit)):
                raise
            if response is not None:
                try:
                    await response.aclose()
                except Exception:
                    pass
            if response_complete is False and response_body:
                response_digest = _sha(bytes(response_body)) if response_body else None
            if failure is None:
                failure = (
                    "request_deadline_before_dispatch"
                    if not dispatched and isinstance(exc, TimeoutError)
                    else "request_timeout"
                    if dispatched and isinstance(exc, TimeoutError)
                    else "transport_or_partial_response"
                    if dispatched
                    else "pacing_or_dispatch_failure"
                )
            try:
                if dispatched:
                    self._transfer_bytes += transfer_increment
                self.sink.record_final(
                    seq,
                    {
                        **base,
                        "status": "partial-or-transport-failure" if dispatched else "failed-before-dispatch",
                        "failure_code": failure,
                        "dispatched": dispatched,
                        "response_status": response_status,
                        "response_sha256": response_digest,
                        "response_bytes": response_observed,
                        "complete": False,
                    },
                )
            finally:
                self.exchanges.append(
                    LiveExchange(
                        operation_id,
                        engine,
                        request.method.upper(),
                        f"https://{host}{request.url.path}",
                        request_sha,
                        len(material),
                        response_status,
                        response_digest,
                        response_observed,
                        None,
                        timeout_s,
                        False,
                        failure,
                        redirect_allowed,
                        dispatched,
                    )
                )
            if isinstance(exc, TimeoutError) and not isinstance(exc, httpx.TimeoutException):
                raise httpx.ReadTimeout("acquisition request deadline exceeded", request=request) from None
            raise

    async def aclose(self) -> None:
        await self.inner.aclose()


def _build_adapters(transport: httpx.AsyncBaseTransport) -> dict[str, EngineAdapter]:
    classes = {
        "arxiv": ArxivAdapter,
        "github": GitHubAdapter,
        "openalex": OpenAlexAdapter,
        "wikipedia": WikipediaAdapter,
    }
    bases = {
        "arxiv": "https://export.arxiv.org/api/query",
        "github": "https://api.github.com",
        "openalex": "https://api.openalex.org",
        "wikipedia": "https://en.wikipedia.org/w/api.php",
    }
    adapters: dict[str, EngineAdapter] = {}
    for engine, cls in classes.items():
        adapter = cls(
            config={
                "base_url": bases[engine],
                "api_key": "",
                "max_results": 20,
                "timeout_ms": int(REQUEST_TIMEOUT_SECONDS * 1000),
            },
            rate_limiter=None,
        )
        adapter.set_http_transport(transport)
        adapters[engine] = adapter
    return adapters


def _summary(stage: StageAcquisition) -> dict:
    return {
        "schema": "coverage-live-acquisition-summary/1",
        "stage": stage.stage,
        "status": stage.status,
        "physical_request_count": stage.physical_request_count,
        "transfer_bytes": stage.transfer_bytes,
        "reserved_worst_case_requests": stage.reserved_worst_case_requests,
        "failure_reasons": list(stage.failure_reasons),
        "operations": [
            {
                "operation_id": item.operation_id,
                "kind": item.kind,
                "status": item.status,
                "pool_count": item.pool_count,
                "band": item.band,
                "band_valid": item.band_valid,
                "target_found_at_rank1": item.target_found_at_rank1,
                "failure_reasons": list(item.failure_reasons),
            }
            for item in stage.operations
        ],
        "quality_credit": False,
        "replacement_or_rescue": False,
    }


async def acquire_live_coverage_stage(
    *,
    stage_manifest: Mapping[str, object],
    acquisition_plan_bytes: bytes,
    permit_receipt_bytes: bytes,
    expected_permit_receipt_sha256: str,
    permit_verifier: PermitVerifier,
    one_shot_lease: OneShotLease,
    receipt_directory: str | os.PathLike[str],
    test_transport: httpx.MockTransport | None = None,
    pacer: Pacer | None = None,
) -> LiveAcquisitionResult:
    """Acquire one fully planned source stage after external admission.

    Production callers get an owned zero-retry, trust-env-disabled transport.
    The only transport injection is the explicit MockTransport test seam.
    This function never reads credentials or an environment variable.
    """
    manifest, manifest_raw, _ids, _reserved, operation_limits = _validate_manifest(
        stage_manifest, acquisition_plan_bytes
    )
    manifest_sha = _sha(manifest_raw)
    permit = _verify_permit(
        manifest, manifest_sha, permit_receipt_bytes, expected_permit_receipt_sha256, permit_verifier
    )
    if test_transport is not None and type(test_transport) is not httpx.MockTransport:
        raise LiveAcquisitionError("test-transport-must-be-httpx-mocktransport")
    sink = PrivateReceiptSink(receipt_directory)
    lease_receipt_sha = _consume_lease(one_shot_lease, permit)
    sink.write_once(
        "admission.json",
        {
            "schema": "coverage-live-acquisition-admission/1",
            "stage_uuid": permit.stage_uuid,
            "source_revision": permit.source_revision,
            "source_closure_sha256": permit.source_closure_sha256,
            "protocol_sha256": permit.protocol_sha256,
            "cohorts_sha256": permit.cohorts_sha256,
            "input_manifest_sha256": permit.input_manifest_sha256,
            "acquisition_plan_sha256": permit.acquisition_plan_sha256,
            "stage_manifest_sha256": manifest_sha,
            "permit_receipt_sha256": permit.receipt_sha256,
            "lease_receipt_sha256": lease_receipt_sha,
        },
    )

    pacing = pacer
    if pacing is None:

        class _RealPacer:
            async def sleep(self, seconds: float) -> None:
                await asyncio.sleep(seconds)

        pacing = _RealPacer()

    owned_transport = test_transport is None
    inner = test_transport if test_transport is not None else httpx.AsyncHTTPTransport(retries=0, trust_env=False)
    transport = _BoundedTransport(inner, sink, operation_limits, pacing)
    adapters = _build_adapters(transport)
    snapshot_rows: dict[str, dict[str, object]] = {}

    def persist_operation(operation: object) -> None:
        document = _operation_snapshot(operation, manifest, manifest_sha)
        raw = _canonical(document)
        if len(raw) > MAX_POOL_SNAPSHOT_BYTES:
            raise LiveAcquisitionError("pool-snapshot-overbound")
        filename = _snapshot_filename(operation.operation_id)
        digest = sink.write_once(filename, document)
        snapshot_rows[operation.operation_id] = {
            "operation_id": operation.operation_id,
            "kind": operation.kind,
            "status": operation.status,
            "file": filename,
            "sha256": digest,
            "bytes": len(raw),
            "card_count": operation.pool_count,
        }

    stage: StageAcquisition | None = None
    try:
        stage = await offline._acquire_coverage_stage(
            manifest,
            adapters,
            transport,
            pacing,
            _live_token=offline._LIVE_ACQUISITION_TOKEN,
            on_operation_complete=persist_operation,
        )
        ordered_ids = [
            *(_row["task_id"] for _row in manifest["research_cases"]),
            *(_row["target_id"] for _row in manifest["navigation_targets"]),
        ]
        index_rows = [
            snapshot_rows.get(
                operation_id,
                {
                    "operation_id": operation_id,
                    "kind": "research" if index < 8 else "navigation",
                    "status": "pool_snapshot_write_failure",
                    "file": None,
                    "sha256": None,
                    "bytes": None,
                    "card_count": None,
                },
            )
            for index, operation_id in enumerate(ordered_ids)
        ]
        index_doc = {
            "schema": POOL_INDEX_SCHEMA,
            "stage": manifest["stage"],
            "stage_uuid": manifest["stage_uuid"],
            "source_revision": manifest["source_revision"],
            "source_closure_sha256": manifest["source_closure_sha256"],
            "protocol_sha256": manifest["protocol_sha256"],
            "cohorts_sha256": manifest["cohorts_sha256"],
            "input_manifest_sha256": manifest["input_manifest_sha256"],
            "acquisition_plan_sha256": manifest["acquisition_plan_sha256"],
            "stage_manifest_sha256": manifest_sha,
            "status": "complete"
            if stage.status == "complete" and all(row["status"] == "complete" for row in index_rows)
            else "incomplete",
            "operations": index_rows,
        }
        index_sha = sink.write_once("pool-snapshot-index.json", index_doc)
        sink.write_once("stage-summary.json", _summary(stage))
        return LiveAcquisitionResult(
            stage, manifest_sha, permit.receipt_sha256, lease_receipt_sha, sink.path, index_sha
        )
    except BaseException as exc:
        if isinstance(exc, (KeyboardInterrupt, SystemExit)):
            raise
        # Retain terminal failure classification without leaking exception text.
        try:
            sink.write_once(
                "stage-summary.json",
                {
                    "schema": "coverage-live-acquisition-summary/1",
                    "stage": manifest["stage"],
                    "stage_uuid": manifest["stage_uuid"],
                    "status": "terminal-incomplete",
                    "failure_class": type(exc).__name__,
                    "physical_request_count": transport.physical_dispatches,
                    "transfer_bytes": transport.transfer_bytes,
                    "quality_credit": False,
                },
            )
        except LiveAcquisitionError:
            pass
        raise
    finally:
        for adapter in adapters.values():
            await adapter.shutdown()
        if owned_transport:
            await transport.aclose()


__all__ = [
    "LiveAcquisitionError",
    "LiveAcquisitionResult",
    "OneShotLease",
    "PermitVerifier",
    "PrivateReceiptSink",
    "VerifiedAcquisitionPermit",
    "acquire_live_coverage_stage",
    "verify_pool_snapshot_index",
]
