"""One-shot, admitted collection of complete natural pools for frozen replay.

This producer is inert at import time and never loads credentials from the
process environment. Live use requires a source-bound external permit and an
exclusive one-shot lease. Tests may inject only an explicit HTTPX MockTransport
and pacer; those receipts are marked as test-only and cannot claim live proof.
"""

from __future__ import annotations

import asyncio
import hashlib
import json
import math
import os
import re
import stat
import time
from collections.abc import Mapping
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from pathlib import Path

import httpx

from scripts import coverage_live_acquire as live
from scripts import coverage_study_acquire as offline
from scripts import coverage_study_core as core
from scripts.coverage_study_acquire import AcquisitionOperation
from slopsearx.service import AppContext, SearchRequest, SearchService

POOL_INPUT_MODE = "frozen-pool/1"
COLLECTION_ADMISSION_SCHEMA = "coverage-frozen-collection-admission/1"
REQUEST_LEDGER_SCHEMA = "coverage-frozen-request-ledger-index/1"
COLLECTION_SUMMARY_SCHEMA = "coverage-frozen-collection-summary/1"
COLLECTOR_RECEIPT_SCHEMA = "coverage-frozen-pool-collection-receipt/1"
AVAILABILITY_SCHEMA = "coverage-engine-availability/1"
MAX_STAGE_WALL_SECONDS = 28_800
FINALIZATION_RESERVE_SECONDS = 300
MAX_RESPONSE_BYTES = live.MAX_RESPONSE_BYTES
MAX_RAW_CAPTURE_BYTES = MAX_RESPONSE_BYTES + 1
_SHA = re.compile(r"[0-9a-f]{64}\Z")


class FrozenCollectionError(RuntimeError):
    """Admission, collection, or durable evidence failed closed."""


@dataclass(frozen=True)
class FrozenCollectionResult:
    stage_uuid: str
    collection_directory: Path
    stage_manifest_sha256: str
    pool_manifest_sha256: str
    pool_index_sha256: str
    availability_sha256: str
    evidence_sha256: str
    collector_receipt_sha256: str
    status: str
    quality_credit: bool = False


@dataclass(frozen=True)
class FrozenCollectionStageClock:
    """Coordinator-pinned clock; monotonic values are valid only on one host boot."""

    stage_started_monotonic: float
    stage_deadline_monotonic: float
    stage_started_utc: str
    stage_deadline_utc: str
    clock_origin_sha256: str


@dataclass(frozen=True)
class VerifiedFrozenCollectionPermit:
    status: str
    scope: str
    stage_uuid: str
    stage_kind: str
    source_revision: str
    source_closure_sha256: str
    protocol_sha256: str
    cohorts_sha256: str
    input_manifest_sha256: str
    acquisition_plan_sha256: str
    legacy_manifest_sha256: str
    pool_input_mode: str
    stage_clock_sha256: str
    candidate_identity_sha256: str
    candidate_endpoint_sha256: str
    receipt_sha256: str


class FrozenCollectionPermitVerifier:
    def verify_collection(
        self,
        manifest,
        manifest_sha256,
        pool_input_mode,
        stage_clock_sha256,
        candidate_identity_sha256,
        candidate_endpoint_sha256,
        receipt_bytes,
        expected_receipt_sha256,
    ) -> VerifiedFrozenCollectionPermit: ...


class _StageCooldown:
    """Stage-scoped service cooldowns created only by observed HTTP 429s."""

    def __init__(self) -> None:
        self.sources: dict[str, int] = {}

    async def upstream_cooldown_remaining(self, engine: str) -> float:
        return 1.0 if engine in self.sources else 0.0


class _RawTeeStream(httpx.AsyncByteStream):
    def __init__(self, source: httpx.AsyncByteStream, callback) -> None:
        self._source = source
        self._callback = callback
        self._captured = bytearray()
        self._finished = False

    async def __aiter__(self):
        try:
            async for chunk in self._source:
                if len(self._captured) < MAX_RAW_CAPTURE_BYTES:
                    self._captured.extend(chunk[: MAX_RAW_CAPTURE_BYTES - len(self._captured)])
                yield chunk
        finally:
            if not self._finished:
                self._finished = True
                self._callback(bytes(self._captured))

    async def aclose(self) -> None:
        try:
            await self._source.aclose()
        finally:
            if not self._finished:
                self._finished = True
                self._callback(bytes(self._captured))


class _RawCapturingInner(httpx.AsyncBaseTransport):
    """Tee actual response bytes beneath the existing bounded transport."""

    def __init__(self, inner: httpx.AsyncBaseTransport, sink: live.PrivateReceiptSink, before_dispatch=None) -> None:
        self.inner = inner
        self.sink = sink
        self.before_dispatch = before_dispatch
        self.raw_rows: dict[int, dict[str, object]] = {}

    async def handle_async_request(self, request: httpx.Request) -> httpx.Response:
        sequence = self.sink._sequence
        if self.before_dispatch is not None and not self.before_dispatch():
            raise httpx.ConnectTimeout("stage deadline elapsed before physical dispatch", request=request)
        response = await self.inner.handle_async_request(request)

        def save(raw: bytes) -> None:
            if sequence in self.raw_rows:
                raise live.LiveAcquisitionError("duplicate-raw-response-sequence")
            filename = f"http-{sequence:04d}-response.bin"
            if self.before_dispatch is not None and not self.before_dispatch():
                raise live.LiveAcquisitionError("stage-deadline-before-raw-response-persist")
            digest = _write_private_bytes_once(self.sink.path / filename, raw)
            if self.before_dispatch is not None and not self.before_dispatch():
                raise live.LiveAcquisitionError("stage-deadline-after-raw-response-persist")
            self.raw_rows[sequence] = {"file": filename, "sha256": digest, "bytes": len(raw)}

        response.stream = _RawTeeStream(response.stream, save)
        return response

    async def aclose(self) -> None:
        await self.inner.aclose()


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    return live._canonical(value)


def _write_private_bytes_once(path: Path, raw: bytes) -> str:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    try:
        fd = os.open(path, flags, 0o600)
        with os.fdopen(fd, "wb") as stream:
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
        parent_fd = os.open(path.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0))
        try:
            os.fsync(parent_fd)
        finally:
            os.close(parent_fd)
    except FileExistsError as exc:
        raise FrozenCollectionError("raw-response-slot-already-exists") from exc
    except OSError as exc:
        raise FrozenCollectionError("raw-response-write-failed") from exc
    return _sha(raw)


class _DeadlineSink:
    """Hardened receipt sink that checks the coordinator deadline around writes."""

    def __init__(self, inner: live.PrivateReceiptSink, check_deadline) -> None:
        self.inner = inner
        self.path = inner.path
        self._check_deadline = check_deadline

    @property
    def _sequence(self) -> int:
        return self.inner._sequence

    @_sequence.setter
    def _sequence(self, value: int) -> None:
        self.inner._sequence = value

    def write_once(self, filename: str, payload: Mapping[str, object]) -> str:
        self._check_deadline()
        digest = self.inner.write_once(filename, payload)
        self._check_deadline()
        return digest

    def record_start(self, payload: Mapping[str, object]) -> str:
        self._check_deadline()
        digest = self.inner.record_start(payload)
        self._check_deadline()
        return digest

    def record_final(self, sequence: int, payload: Mapping[str, object]) -> str:
        self._check_deadline()
        digest = self.inner.record_final(sequence, payload)
        self._check_deadline()
        return digest


def _read_bytes(path: Path) -> bytes:
    try:
        info = path.lstat()
        if not stat.S_ISREG(info.st_mode) or info.st_mode & 0o777 != 0o600:
            raise FrozenCollectionError("collection-artifact-mode-invalid")
        return path.read_bytes()
    except FrozenCollectionError:
        raise
    except OSError as exc:
        raise FrozenCollectionError("collection-artifact-unavailable") from exc


def _utc(value: object, label: str) -> datetime:
    if type(value) is not str or not value.endswith("Z"):
        raise FrozenCollectionError(f"{label}-invalid")
    try:
        parsed = datetime.fromisoformat(value[:-1] + "+00:00")
    except ValueError as exc:
        raise FrozenCollectionError(f"{label}-invalid") from exc
    if parsed.tzinfo is None:
        raise FrozenCollectionError(f"{label}-invalid")
    return parsed.astimezone(timezone.utc)


def _validate_stage_manifest(stage_manifest_bytes: bytes, plan_bytes: bytes):
    if type(stage_manifest_bytes) is not bytes or len(stage_manifest_bytes) > core.MAX_MANIFEST_BYTES:
        raise FrozenCollectionError("stage-manifest-invalid")
    try:
        stage_manifest = core._strict_json(stage_manifest_bytes, "frozen-stage-manifest")
        if type(stage_manifest) is not dict or _canonical(stage_manifest) != stage_manifest_bytes:
            raise ValueError("non-canonical stage manifest")
    except Exception as exc:
        raise FrozenCollectionError("stage-manifest-invalid") from exc
    try:
        validated = live._validate_manifest(stage_manifest, plan_bytes)
    except Exception as exc:
        raise FrozenCollectionError("acquisition-manifest-invalid") from exc
    return dict(stage_manifest), validated


def _local_clock_origin_sha256() -> str:
    try:
        raw = Path("/proc/sys/kernel/random/boot_id").read_text(encoding="ascii").strip().encode("ascii")
    except OSError as exc:
        raise FrozenCollectionError("clock-origin-unavailable") from exc
    if not raw:
        raise FrozenCollectionError("clock-origin-unavailable")
    return _sha(raw)


def _validate_clock(clock: FrozenCollectionStageClock, *, test_mode: bool, test_pacer) -> dict[str, object]:
    if type(clock) is not FrozenCollectionStageClock:
        raise FrozenCollectionError("stage-clock-type-invalid")
    start, deadline = clock.stage_started_monotonic, clock.stage_deadline_monotonic
    if (
        type(start) not in (int, float)
        or type(deadline) not in (int, float)
        or not math.isfinite(start)
        or not math.isfinite(deadline)
        or not start < deadline
        or abs((deadline - start) - MAX_STAGE_WALL_SECONDS) > 0.000001
    ):
        raise FrozenCollectionError("stage-clock-monotonic-invalid")
    started_utc = _utc(clock.stage_started_utc, "stage-started-utc")
    deadline_utc = _utc(clock.stage_deadline_utc, "stage-deadline-utc")
    utc_span = (deadline_utc - started_utc).total_seconds()
    if not 0 < utc_span <= MAX_STAGE_WALL_SECONDS or abs(utc_span - (deadline - start)) > 2.0:
        raise FrozenCollectionError("stage-clock-domains-disagree")
    if type(clock.clock_origin_sha256) is not str or not _SHA.fullmatch(clock.clock_origin_sha256):
        raise FrozenCollectionError("clock-origin-pin-invalid")
    origin_sha = _sha(b"test-only-clock-origin") if test_mode else _local_clock_origin_sha256()
    if origin_sha != clock.clock_origin_sha256:
        raise FrozenCollectionError("clock-origin-mismatch")
    now = test_pacer.monotonic() if test_mode and callable(getattr(test_pacer, "monotonic", None)) else time.monotonic()
    now_utc = datetime.now(timezone.utc)
    remaining_utc = (deadline_utc - now_utc).total_seconds()
    elapsed_utc = (now_utc - started_utc).total_seconds()
    if not start <= now < deadline or remaining_utc <= 0 or abs(elapsed_utc - (now - start)) > 2.0:
        raise FrozenCollectionError("stage-clock-expired")
    return {
        "schema": "coverage-frozen-collection-stage-clock/1",
        "stage_started_monotonic": start,
        "stage_deadline_monotonic": deadline,
        "stage_started_utc": started_utc.isoformat().replace("+00:00", "Z"),
        "stage_deadline_utc": deadline_utc.isoformat().replace("+00:00", "Z"),
        "clock_origin_sha256": clock.clock_origin_sha256,
    }


def _make_inventory_root(inventory: str | os.PathLike[str], stage_uuid: str) -> Path:
    base = Path(inventory)
    try:
        base_info = base.lstat()
        if (
            base.name != stage_uuid
            or not stat.S_ISDIR(base_info.st_mode)
            or base_info.st_mode & 0o777 != 0o700
            or base.is_symlink()
        ):
            raise FrozenCollectionError("stage-inventory-not-private-directory")
        collection_dir = base / "collection"
        collection_dir.mkdir(mode=0o700)
        os.chmod(collection_dir, 0o700)
        return collection_dir
    except FrozenCollectionError:
        raise
    except FileExistsError as exc:
        raise FrozenCollectionError("stage-collection-already-exists") from exc
    except OSError as exc:
        raise FrozenCollectionError("stage-collection-directory-failed") from exc


def _persist_operation(
    sink: live.PrivateReceiptSink,
    operation: AcquisitionOperation,
    manifest: Mapping[str, object],
    manifest_sha: str,
    cooldown_refs: list[dict[str, object]],
) -> dict[str, object]:
    document = live._operation_snapshot(operation, manifest, manifest_sha)
    document["operation"]["cooldown_source_references"] = cooldown_refs
    raw = _canonical(document)
    filename = live._snapshot_filename(operation.operation_id)
    digest = sink.write_once(filename, document)
    return {
        "operation_id": operation.operation_id,
        "kind": operation.kind,
        "acquisition_status": operation.status,
        "pool_status": "complete" if _pool_eligible(operation) else "inconclusive",
        "file": filename,
        "sha256": digest,
        "bytes": len(raw),
        "card_count": operation.pool_count if operation.pool_count is not None else 0,
    }


def _pool_eligible(operation: AcquisitionOperation) -> bool:
    if operation.canonical_response is None:
        return False
    if operation.canonical_response.scope.selected_engines != list(operation.engines) or {
        outcome.engine for outcome in operation.canonical_response.engine_outcomes
    } != set(operation.engines):
        return False
    if operation.kind == "research":
        count = len(operation.canonical_response.results)
        return (operation.band == "research_le_40" and 1 <= count <= 40) or (
            operation.band == "research_41_80" and 41 <= count <= 80
        )
    return bool(
        operation.canonical_response.results and operation.target_url and operation.target_found_at_rank1 is True
    )


def _operation_failure_reasons(operation: AcquisitionOperation, response) -> list[str]:
    reasons: list[str] = []
    if response.scope.selected_engines != list(operation.engines):
        reasons.append("scope_mismatch")
    if response.ranking_explanation != "tier_then_cross_engine_presence":
        reasons.append("unexpected_ranking_path")
    if response.deadline_exceeded:
        reasons.append("search_deadline_exceeded")
    failed = [outcome.engine for outcome in response.engine_outcomes if outcome.status != "ok"]
    if failed:
        reasons.append("engine_failures:" + ",".join(sorted(failed)))
    if operation.kind == "research":
        count = len(response.results)
        in_band = (operation.band == "research_le_40" and 1 <= count <= 40) or (
            operation.band == "research_41_80" and 41 <= count <= 80
        )
        operation.band_valid = in_band
        if not in_band:
            reasons.append("natural_pool_band_mismatch")
    else:
        target = offline._normalise_url(operation.target_url) if operation.target_url else ""
        operation.target_found_at_rank1 = bool(
            response.results and offline._normalise_url(response.results[0].url) == target
        )
        if not operation.target_found_at_rank1:
            reasons.append("exact_target_not_rank1")
    return reasons


def _write_operation_stub(sink: live.PrivateReceiptSink, manifest, stage_sha, item, reason) -> dict[str, object]:
    operation = AcquisitionOperation(
        operation_id=item[0],
        kind=item[1],
        query=item[2],
        engines=item[3],
        status="not_invoked_after_fatal_failure",
        pool_count=None,
        band=item[4],
        band_valid=None,
        target_url=item[5],
        target_found_at_rank1=None,
        scope=None,
        canonical_response=None,
        exchanges=(),
        failure_reasons=[reason],
    )
    return _persist_operation(sink, operation, manifest, stage_sha, [])


async def collect_frozen_pool_stage(
    *,
    stage_manifest_bytes: bytes,
    acquisition_plan_bytes: bytes,
    stage_clock: FrozenCollectionStageClock,
    candidate_identity_sha256: str,
    candidate_endpoint_sha256: str,
    permit_receipt_bytes: bytes,
    expected_permit_receipt_sha256: str,
    permit_verifier: FrozenCollectionPermitVerifier,
    one_shot_lease: live.OneShotLease,
    stage_inventory_directory: str | os.PathLike[str],
    test_transport: httpx.MockTransport | None = None,
    pacer: live.Pacer | None = None,
) -> FrozenCollectionResult:
    """Collect the exact preregistered 8+5 sequence once and seal its receipts."""
    manifest, validated = _validate_stage_manifest(stage_manifest_bytes, acquisition_plan_bytes)
    full_manifest = dict(manifest)
    stage_sha = _sha(stage_manifest_bytes)
    manifest, _legacy_raw, operation_ids, _reserved, operation_limits = validated
    if offline.execution_profile_from_manifest(manifest) is not offline.CONTINUE_ACQUISITION_PROFILE:
        raise FrozenCollectionError("frozen-pool-collection-requires-continue-profile")
    low = [row for row in manifest["research_cases"] if row["pool_plan"].get("band") == "research_le_40"]
    high = [row for row in manifest["research_cases"] if row["pool_plan"].get("band") == "research_41_80"]
    if (
        len(manifest["research_cases"]) != 8
        or len(low) != 4
        or len(high) != 4
        or len(manifest["navigation_targets"]) != 5
    ):
        raise FrozenCollectionError("frozen-pool-registered-cohort-shape-invalid")
    if test_transport is not None and type(test_transport) is not httpx.MockTransport:
        raise FrozenCollectionError("test-transport-must-be-httpx-mocktransport")
    if pacer is not None and test_transport is None:
        raise FrozenCollectionError("pacer-injection-requires-explicit-mock-transport")

    test_mode = test_transport is not None
    clock_doc = _validate_clock(stage_clock, test_mode=test_mode, test_pacer=pacer)
    clock_sha = _sha(_canonical(clock_doc))
    for label, value in (
        ("candidate-identity", candidate_identity_sha256),
        ("candidate-endpoint", candidate_endpoint_sha256),
    ):
        if type(value) is not str or not _SHA.fullmatch(value):
            raise FrozenCollectionError(f"{label}-pin-invalid")
    if type(permit_receipt_bytes) is not bytes or not permit_receipt_bytes:
        raise FrozenCollectionError("external-acquisition-permit-required")
    expected_receipt_sha = live._valid_sha(expected_permit_receipt_sha256, "permit-receipt")
    if _sha(permit_receipt_bytes) != expected_receipt_sha:
        raise FrozenCollectionError("permit-receipt-digest-mismatch")
    verifier = getattr(permit_verifier, "verify_collection", None)
    if not callable(verifier):
        raise FrozenCollectionError("external-frozen-collection-permit-verifier-required")
    try:
        permit = verifier(
            manifest,
            stage_sha,
            POOL_INPUT_MODE,
            clock_sha,
            candidate_identity_sha256,
            candidate_endpoint_sha256,
            permit_receipt_bytes,
            expected_receipt_sha,
        )
    except Exception as exc:
        raise FrozenCollectionError("external-permit-verification-failed") from exc
    expected_permit = VerifiedFrozenCollectionPermit(
        status="verified",
        scope="frozen-pool-collection",
        stage_uuid=manifest["stage_uuid"],
        stage_kind=manifest["stage"],
        source_revision=manifest["source_revision"],
        source_closure_sha256=manifest["source_closure_sha256"],
        protocol_sha256=manifest["protocol_sha256"],
        cohorts_sha256=manifest["cohorts_sha256"],
        input_manifest_sha256=manifest["input_manifest_sha256"],
        acquisition_plan_sha256=manifest["acquisition_plan_sha256"],
        legacy_manifest_sha256=stage_sha,
        pool_input_mode=POOL_INPUT_MODE,
        stage_clock_sha256=clock_sha,
        candidate_identity_sha256=candidate_identity_sha256,
        candidate_endpoint_sha256=candidate_endpoint_sha256,
        receipt_sha256=expected_receipt_sha,
    )
    if type(permit) is not VerifiedFrozenCollectionPermit or permit != expected_permit:
        raise FrozenCollectionError("external-permit-binding-mismatch")

    collection_dir = _make_inventory_root(stage_inventory_directory, str(manifest["stage_uuid"]))
    sink_base = live.PrivateReceiptSink.__new__(live.PrivateReceiptSink)
    # Use the repository's hardened sink implementation at the fixed runner path.
    sink_base.path = collection_dir
    sink_base._sequence = 0
    try:
        lease_sha = live._consume_lease(one_shot_lease, permit)
    except Exception as exc:
        raise FrozenCollectionError("one-shot-lease-consumption-failed") from exc

    pacing = pacer
    if pacing is None:

        class _RealPacer:
            async def sleep(self, seconds: float) -> None:
                await asyncio.sleep(seconds)

        pacing = _RealPacer()
    monotonic_now = pacing.monotonic if test_mode and callable(getattr(pacing, "monotonic", None)) else time.monotonic
    stage_start_mono = monotonic_now()
    collection_start_utc = _utc(stage_clock.stage_started_utc, "stage-started-utc") + timedelta(
        seconds=stage_start_mono - stage_clock.stage_started_monotonic
    )
    deadline_mono = stage_clock.stage_deadline_monotonic
    dispatch_cutoff_mono = deadline_mono - FINALIZATION_RESERVE_SECONDS

    def check_deadline() -> None:
        if monotonic_now() >= deadline_mono:
            raise FrozenCollectionError("stage-deadline-exceeded")

    sink = _DeadlineSink(sink_base, check_deadline)
    admission = {
        "schema": COLLECTION_ADMISSION_SCHEMA,
        "pool_input_mode": POOL_INPUT_MODE,
        "stage_clock_sha256": clock_sha,
        "clock_origin_sha256": stage_clock.clock_origin_sha256,
        "stage_uuid": permit.stage_uuid,
        "stage": manifest["stage"],
        "source_revision": permit.source_revision,
        "source_closure_sha256": permit.source_closure_sha256,
        "protocol_sha256": permit.protocol_sha256,
        "cohorts_sha256": permit.cohorts_sha256,
        "input_manifest_sha256": permit.input_manifest_sha256,
        "acquisition_plan_sha256": permit.acquisition_plan_sha256,
        "candidate_identity_sha256": candidate_identity_sha256,
        "candidate_endpoint_sha256": candidate_endpoint_sha256,
        "stage_manifest_sha256": stage_sha,
        "permit_sha256": permit.receipt_sha256,
        "lease_sha256": lease_sha,
    }
    sink.write_once("admission.json", admission)
    sink.write_once("acquisition-manifest.json", full_manifest)
    try:
        os.chmod(collection_dir / "acquisition-manifest.json", 0o600)
    except OSError as exc:
        raise FrozenCollectionError("acquisition-manifest-write-failed") from exc

    owned_transport = test_transport is None
    inner_base = test_transport if test_transport is not None else httpx.AsyncHTTPTransport(retries=0, trust_env=False)
    raw_inner = _RawCapturingInner(inner_base, sink, before_dispatch=lambda: monotonic_now() < deadline_mono)
    operation_rows = [*manifest["research_cases"], *manifest["navigation_targets"]]
    expected_ids = tuple(row["task_id"] if i < 8 else row["target_id"] for i, row in enumerate(operation_rows))
    source_bindings = {
        "stage_uuid": permit.stage_uuid,
        "source_revision": permit.source_revision,
        "source_closure_sha256": permit.source_closure_sha256,
        "protocol_sha256": permit.protocol_sha256,
        "cohorts_sha256": permit.cohorts_sha256,
        "input_manifest_sha256": permit.input_manifest_sha256,
        "acquisition_plan_sha256": permit.acquisition_plan_sha256,
    }
    monotonic = monotonic_now
    transport = live._BoundedTransport(
        raw_inner,
        sink,
        operation_limits,
        pacing,
        monotonic=monotonic,
        expected_operation_ids=expected_ids,
        source_bindings=source_bindings,
        execution_profile=offline.CONTINUE_ACQUISITION_PROFILE,
        owned_pacer=pacer is None,
        owned_transport=owned_transport,
    )
    adapters = live._build_adapters(transport, offline.CONTINUE_ACQUISITION_PROFILE)
    cooldown = _StageCooldown()
    service = SearchService(
        AppContext(
            active_engines=dict(adapters),
            ranking_strategy="presence",
            rerank_provider=None,
            jev_router=None,
            cache=None,
            suggestion_service=None,
            rate_limiter=cooldown,
        )
    )
    operation_results: list[AcquisitionOperation] = []
    index_rows: list[dict[str, object]] = []
    fatal_reason: str | None = None
    start_utc_text = collection_start_utc.isoformat().replace("+00:00", "Z")
    deadline_utc_text = _utc(stage_clock.stage_deadline_utc, "stage-deadline-utc").isoformat().replace("+00:00", "Z")

    def plans():
        for row in manifest["research_cases"]:
            plan = row["pool_plan"]
            yield row["task_id"], "research", row["search_query"], tuple(plan["engines"]), plan["band"], None
        for row in manifest["navigation_targets"]:
            plan = row["navigation_pool_plan"]
            yield row["target_id"], "navigation", row["search_query"], tuple(plan["engines"]), None, row["target_url"]

    try:
        for index, item in enumerate(plans()):
            operation_id, kind, query, engines, band, target_url = item
            if fatal_reason is not None:
                result = _write_operation_stub(sink, manifest, stage_sha, item, fatal_reason)
                index_rows.append(result)
                operation_results.append(
                    AcquisitionOperation(
                        operation_id,
                        kind,
                        query,
                        engines,
                        "not_invoked_after_fatal_failure",
                        None,
                        band,
                        None,
                        target_url,
                        None,
                        None,
                        None,
                        (),
                        [fatal_reason],
                    )
                )
                continue
            now = monotonic_now()
            if now >= dispatch_cutoff_mono:
                fatal_reason = "stage_finalization_reserve_reached"
                result = _write_operation_stub(sink, manifest, stage_sha, item, fatal_reason)
                index_rows.append(result)
                operation_results.append(
                    AcquisitionOperation(
                        operation_id,
                        kind,
                        query,
                        engines,
                        "not_invoked_after_fatal_failure",
                        None,
                        band,
                        None,
                        target_url,
                        None,
                        None,
                        None,
                        (),
                        [fatal_reason],
                    )
                )
                continue
            if index:
                delay = offline.QUERY_PACING_SECONDS + offline.OFFSET_QUANTIZATION_GUARD_SECONDS
                if monotonic_now() + delay >= dispatch_cutoff_mono:
                    fatal_reason = "stage_finalization_reserve_reached"
                    row = _write_operation_stub(sink, manifest, stage_sha, item, fatal_reason)
                    index_rows.append(row)
                    operation_results.append(
                        AcquisitionOperation(
                            operation_id,
                            kind,
                            query,
                            engines,
                            "not_invoked_after_fatal_failure",
                            None,
                            band,
                            None,
                            target_url,
                            None,
                            None,
                            None,
                            (),
                            [fatal_reason],
                        )
                    )
                    continue
                await pacing.sleep(delay)
            if monotonic_now() >= dispatch_cutoff_mono:
                fatal_reason = "stage_finalization_reserve_reached"
                result = _write_operation_stub(sink, manifest, stage_sha, item, fatal_reason)
                index_rows.append(result)
                operation_results.append(
                    AcquisitionOperation(
                        operation_id,
                        kind,
                        query,
                        engines,
                        "not_invoked_after_fatal_failure",
                        None,
                        band,
                        None,
                        target_url,
                        None,
                        None,
                        None,
                        (),
                        [fatal_reason],
                    )
                )
                continue

            transport.begin_operation(operation_id)
            token_op = offline._CURRENT_OPERATION.set(operation_id)
            token_engines = offline._CURRENT_ENGINES.set(engines)
            before = len(transport.exchanges)
            prior_cooldown_refs = [
                {"engine": name, "request_sequence": seq, "observed_status": 429}
                for name, seq in sorted(cooldown.sources.items())
                if name in engines
            ]
            response = None
            exception_type = None
            failure_reasons: list[str] = []
            try:
                request = SearchRequest(
                    query=query,
                    engines=list(engines),
                    categories=[offline.REFERENCE_CATEGORY],
                    max_results=None,
                    freshness="prefer_fresh",
                    interactive_timeout_ms=int(
                        offline.CONTINUE_ACQUISITION_PROFILE.logical_query_timeout_seconds * 1000
                    ),
                    generate_suggestions=False,
                )
                timeout_context = asyncio.timeout(offline.CONTINUE_ACQUISITION_PROFILE.logical_query_timeout_seconds)
                token_timeout = offline._CURRENT_LOGICAL_TIMEOUT.set(timeout_context)
                try:
                    async with timeout_context:
                        response = await service.search(request)
                finally:
                    offline._CURRENT_LOGICAL_TIMEOUT.reset(token_timeout)
                provisional = AcquisitionOperation(
                    operation_id,
                    kind,
                    query,
                    engines,
                    "inconclusive",
                    len(response.results),
                    band,
                    None,
                    target_url,
                    None,
                    response.scope,
                    response,
                    tuple(transport.exchanges[before:]),
                    [],
                    None,
                )
                failure_reasons = _operation_failure_reasons(provisional, response)
                failure_reasons.extend(
                    f"transport:{exchange.failure_code}"
                    for exchange in transport.exchanges[before:]
                    if exchange.failure_code
                )
                provisional.failure_reasons = failure_reasons
                provisional.status = "complete" if not failure_reasons else "inconclusive"
                result = provisional
                # Read the bounded sink's actual final HTTP receipts. A cooldown
                # is installed only for an observed dispatched 429, and the
                # subsequent operation sidecar names this exact request slot.
                for sequence in range(1, sink._sequence + 1):
                    final_path = collection_dir / f"http-{sequence:04d}-final.json"
                    if not final_path.exists():
                        continue
                    final_doc = json.loads(_read_bytes(final_path))
                    if (
                        final_doc.get("operation_id") == operation_id
                        and final_doc.get("engine") in engines
                        and final_doc.get("response_status") == 429
                        and final_doc.get("dispatched") is True
                    ):
                        cooldown.sources.setdefault(str(final_doc["engine"]), sequence)
            except Exception as exc:
                exception_type = type(exc).__name__
                failure_reasons = ["logical_query_timeout" if isinstance(exc, TimeoutError) else "search_exception"]
                result = AcquisitionOperation(
                    operation_id,
                    kind,
                    query,
                    engines,
                    "inconclusive",
                    None,
                    band,
                    False if kind == "research" else None,
                    target_url,
                    False if kind == "navigation" else None,
                    None,
                    None,
                    tuple(transport.exchanges[before:]),
                    failure_reasons,
                    exception_type,
                )
            finally:
                offline._CURRENT_ENGINES.reset(token_engines)
                offline._CURRENT_OPERATION.reset(token_op)
                transport.finish_operation(operation_id)

            result.exchanges = tuple(transport.exchanges[before:])
            operation_results.append(result)
            completed_in_time = monotonic_now() < deadline_mono
            index_rows.append(_persist_operation(sink, result, manifest, stage_sha, prior_cooldown_refs))
            if not completed_in_time:
                fatal_reason = "stage-deadline-exceeded"
            elif monotonic_now() >= dispatch_cutoff_mono:
                fatal_reason = "stage_finalization_reserve_reached"
            if any(exchange.failure_code in offline._HARD_TRANSPORT_FAILURES for exchange in result.exchanges):
                fatal_reason = "transport-integrity-or-budget-failure"
            elif any(
                json.loads(_read_bytes(collection_dir / f"http-{sequence:04d}-final.json")).get("response_status")
                == 401
                for sequence in range(1, sink._sequence + 1)
                if json.loads(_read_bytes(collection_dir / f"http-{sequence:04d}-start.json")).get("operation_id")
                == operation_id
            ):
                fatal_reason = "provider-authentication-failure"
            elif (response is None and exception_type != "TimeoutError") or any(
                outcome.status in {"blocked", "auth_required", "error"}
                for outcome in (response.engine_outcomes if response is not None else [])
            ):
                fatal_reason = "provider-security-or-integrity-failure"
            elif response is not None and any(outcome.status == "unavailable" for outcome in response.engine_outcomes):
                # Preserve unavailability as an observed result; later operations remain independent.
                pass
            elif response is not None and any(outcome.status == "rate_limited" for outcome in response.engine_outcomes):
                # Rate limiting is a real provider outcome. The stage-scoped service
                # cooldown prevents another adapter dispatch for that engine.
                pass

        # Ensure every physical request has a raw artifact, including failed-before-response slots.
        for sequence in range(1, sink._sequence + 1):
            if sequence not in raw_inner.raw_rows:
                filename = f"http-{sequence:04d}-response.bin"
                check_deadline()
                raw_digest = _write_private_bytes_once(collection_dir / filename, b"")
                check_deadline()
                raw_inner.raw_rows[sequence] = {"file": filename, "sha256": raw_digest, "bytes": 0}

        # Freeze corpus eligibility independently from per-operation acquisition status.
        all_eligible = (
            len(index_rows) == 13
            and all(row["pool_status"] == "complete" for row in index_rows)
            and monotonic_now() < deadline_mono
        )
        operation_statuses = [
            {"operation_id": op.operation_id, "status": op.status, "failure_reasons": list(op.failure_reasons)}
            for op in operation_results
        ]
        pool_manifest_sha = _sha(_canonical(full_manifest))
        availability_rows = []
        for op in operation_results:
            outcomes = op.canonical_response.engine_outcomes if op.canonical_response else []
            availability_rows.append(
                {
                    "operation_id": op.operation_id,
                    "engines": [
                        {"engine": outcome.engine, "status": outcome.status, "result_count": outcome.result_count}
                        for outcome in outcomes
                    ],
                }
            )
        availability_doc = {
            "schema": AVAILABILITY_SCHEMA,
            "stage_uuid": manifest["stage_uuid"],
            "operations": availability_rows,
        }
        availability_raw = _canonical(availability_doc)
        availability_sha = sink.write_once("engine-availability.json", availability_doc)

        pool_index_doc = {
            "schema": "coverage-frozen-pool-index/1",
            "stage": manifest["stage"],
            "status": "complete" if all_eligible else "inconclusive",
            "stage_uuid": manifest["stage_uuid"],
            "stage_kind": manifest["stage"],
            "source_revision": manifest["source_revision"],
            "source_closure_sha256": manifest["source_closure_sha256"],
            "protocol_sha256": manifest["protocol_sha256"],
            "cohorts_sha256": manifest["cohorts_sha256"],
            "candidate_identity_sha256": candidate_identity_sha256,
            "candidate_endpoint_sha256": candidate_endpoint_sha256,
            "acquisition_plan_sha256": manifest["acquisition_plan_sha256"],
            "pool_manifest_sha256": pool_manifest_sha,
            "availability_sha256": availability_sha,
            "operations": index_rows,
        }
        pool_index_sha = sink.write_once("frozen-pool-index.json", pool_index_doc)

        end_mono = monotonic_now()
        end_utc = collection_start_utc + timedelta(seconds=end_mono - stage_start_mono)
        summary_doc = {
            "schema": COLLECTION_SUMMARY_SCHEMA,
            "stage_uuid": manifest["stage_uuid"],
            "stage_manifest_sha256": stage_sha,
            "pool_input_mode": POOL_INPUT_MODE,
            "status": "complete" if all_eligible else "inconclusive",
            "physical_request_count": transport.physical_dispatches,
            "transfer_bytes": transport.transfer_bytes,
            "operation_statuses": operation_statuses,
            "pool_eligibility": [
                {"operation_id": row["operation_id"], "pool_status": row["pool_status"]} for row in index_rows
            ],
            "fatal_uninvoked_slots": [
                row["operation_id"] for row in operation_statuses if row["status"] == "not_invoked_after_fatal_failure"
            ],
            "collection_started_monotonic": stage_start_mono,
            "collection_completed_monotonic": end_mono,
            "stage_deadline_monotonic": deadline_mono,
            "collection_started_utc": start_utc_text,
            "collection_completed_utc": end_utc.isoformat().replace("+00:00", "Z"),
            "stage_deadline_utc": deadline_utc_text,
            "quality_credit": False,
        }
        summary_sha = sink.write_once("collection-summary.json", summary_doc)
        control_doc = transport.execution_control_attestation(stage_sha)
        control_sha = sink.write_once("transport-control.json", control_doc)

        ledger_entries = []
        for sequence in range(1, sink._sequence + 1):
            start_name = f"http-{sequence:04d}-start.json"
            final_name = f"http-{sequence:04d}-final.json"
            start_raw = _read_bytes(collection_dir / start_name)
            final_raw = _read_bytes(collection_dir / final_name)
            raw_row = raw_inner.raw_rows[sequence]
            final_doc = json.loads(final_raw)
            ledger_entries.append(
                {
                    "sequence": sequence,
                    "operation_id": json.loads(start_raw)["operation_id"],
                    "engine": json.loads(start_raw)["engine"],
                    "start_file": start_name,
                    "start_sha256": _sha(start_raw),
                    "start_bytes": len(start_raw),
                    "final_file": final_name,
                    "final_sha256": _sha(final_raw),
                    "final_bytes": len(final_raw),
                    "raw_response_file": raw_row["file"],
                    "raw_response_sha256": raw_row["sha256"],
                    "raw_response_bytes": raw_row["bytes"],
                    "raw_response_complete": final_doc.get("complete") is True,
                    "raw_response_overflow_witness": raw_row["bytes"] > MAX_RESPONSE_BYTES,
                }
            )
        ledger_sha = sink.write_once(
            "request-ledger-index.json",
            {"schema": REQUEST_LEDGER_SCHEMA, "stage_uuid": manifest["stage_uuid"], "entries": ledger_entries},
        )

        evidence_doc = {
            "schema": "coverage-frozen-pool-evidence/1",
            "stage_uuid": manifest["stage_uuid"],
            "stage_kind": manifest["stage"],
            "source_revision": manifest["source_revision"],
            "source_closure_sha256": manifest["source_closure_sha256"],
            "protocol_sha256": manifest["protocol_sha256"],
            "cohorts_sha256": manifest["cohorts_sha256"],
            "candidate_identity_sha256": candidate_identity_sha256,
            "candidate_endpoint_sha256": candidate_endpoint_sha256,
            "acquisition_plan_sha256": manifest["acquisition_plan_sha256"],
            "pool_manifest_sha256": pool_manifest_sha,
            "pool_snapshot_index_sha256": pool_index_sha,
            "availability_sha256": availability_sha,
            "stage_deadline_monotonic": deadline_mono,
            "collector_receipt_sha256": "0" * 64,
            "collection_started_monotonic": stage_start_mono,
            "collection_completed_monotonic": end_mono,
            "collection_started_utc": start_utc_text,
            "collection_completed_utc": end_utc.isoformat().replace("+00:00", "Z"),
            "quality_credit": False,
        }
        # Receipt binds the component files and the frozen corpus; evidence then pins receipt bytes.
        receipt_doc = {
            "schema": COLLECTOR_RECEIPT_SCHEMA,
            "scope": "registered-frozen-pool-collection",
            "status": "complete" if all_eligible else "inconclusive",
            "stage_uuid": manifest["stage_uuid"],
            "stage_kind": manifest["stage"],
            "source_revision": manifest["source_revision"],
            "source_closure_sha256": manifest["source_closure_sha256"],
            "protocol_sha256": manifest["protocol_sha256"],
            "cohorts_sha256": manifest["cohorts_sha256"],
            "candidate_identity_sha256": candidate_identity_sha256,
            "candidate_endpoint_sha256": candidate_endpoint_sha256,
            "acquisition_plan_sha256": manifest["acquisition_plan_sha256"],
            "pool_manifest_sha256": pool_manifest_sha,
            "pool_snapshot_index_sha256": pool_index_sha,
            "availability_sha256": availability_sha,
            "stage_deadline_monotonic": deadline_mono,
            "admission_sha256": _sha(_read_bytes(collection_dir / "admission.json")),
            "transport_control_sha256": control_sha,
            "request_ledger_index_sha256": ledger_sha,
            "collection_summary_sha256": summary_sha,
            "collection_started_monotonic": stage_start_mono,
            "collection_completed_monotonic": end_mono,
            "collection_started_utc": start_utc_text,
            "collection_completed_utc": end_utc.isoformat().replace("+00:00", "Z"),
        }
        receipt_raw = _canonical(receipt_doc)
        receipt_sha = sink.write_once("collector-receipt.json", receipt_doc)
        evidence_doc["collector_receipt_sha256"] = receipt_sha
        evidence_sha = sink.write_once("frozen-pool-evidence.json", evidence_doc)
        # Verify complete frozen corpus bytes before returning an eligible
        # result. Task projection is a separate coordinator phase: it receives
        # the source-pinned full cohort rows, while this producer deliberately
        # retains the narrower, immutable acquisition-manifest schema.
        if all_eligible:
            from scripts.coverage_frozen_pool_inputs import (
                FrozenPoolEvidence,
                verify_frozen_pool_evidence,
            )

            task_ids = tuple(row["task_id"] for row in manifest["research_cases"])
            nav_ids = tuple(row["target_id"] for row in manifest["navigation_targets"])
            evidence = FrozenPoolEvidence(
                collection_dir,
                _read_bytes(collection_dir / "frozen-pool-evidence.json"),
                evidence_sha,
                receipt_raw,
                receipt_sha,
                availability_raw,
                availability_sha,
            )
            verified = verify_frozen_pool_evidence(
                evidence,
                expected_stage_uuid=manifest["stage_uuid"],
                expected_stage_kind=manifest["stage"],
                expected_source_revision=manifest["source_revision"],
                expected_source_closure_sha256=manifest["source_closure_sha256"],
                expected_protocol_sha256=manifest["protocol_sha256"],
                expected_cohorts_sha256=manifest["cohorts_sha256"],
                expected_candidate_identity_sha256=candidate_identity_sha256,
                expected_candidate_endpoint_sha256=candidate_endpoint_sha256,
                expected_acquisition_plan_sha256=manifest["acquisition_plan_sha256"],
                expected_task_ids=task_ids,
                expected_navigation_ids=nav_ids,
                stage_started_monotonic=stage_clock.stage_started_monotonic,
                stage_deadline_monotonic=deadline_mono,
                now_monotonic=monotonic_now(),
            )
            if verified.pool_manifest_sha256 != pool_manifest_sha:
                raise FrozenCollectionError("frozen-pool-composition-binding-mismatch")
            # This post-seal verifier check closes the stage clock over all
            # receipts, evidence fsyncs, and corpus verification work.
            check_deadline()
        return FrozenCollectionResult(
            manifest["stage_uuid"],
            collection_dir,
            stage_sha,
            pool_manifest_sha,
            pool_index_sha,
            availability_sha,
            evidence_sha,
            receipt_sha,
            "complete" if all_eligible else "inconclusive",
            False,
        )
    finally:
        for adapter in adapters.values():
            await adapter.shutdown()
        if owned_transport:
            await transport.aclose()


__all__ = ["FrozenCollectionError", "FrozenCollectionResult", "POOL_INPUT_MODE", "collect_frozen_pool_stage"]
