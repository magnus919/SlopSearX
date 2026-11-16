"""Read-only verification and current-stage projection of saved study inputs.

This module imports a previously sealed development corpus and its capture
archive without rerunning search or capture. Original stage identities and
receipts remain authoritative for the source artifacts; a separate import
record binds their verified bytes to a fresh grading stage.
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
from typing import Mapping, Sequence

from scripts import coverage_frozen_pool_inputs as frozen
from scripts import coverage_resource_evidence as resource_evidence
from scripts import coverage_source_capture as source_capture
from scripts import intent_ranking_receipts as response_receipts

SCHEMA = "coverage-saved-corpus-import/1"
CAPTURE_IMPORT_SCHEMA = "coverage-saved-source-capture-inventory/1"
_SHA = re.compile(r"[0-9a-f]{64}\Z")
_REVISION = re.compile(r"[0-9a-f]{40}\Z")
_UUID = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\Z")
_MAX_CAPTURE_FILE = source_capture.MAX_RESPONSE_BYTES
_MAX_CONTEXT_FILE = 64_000


class SavedCorpusError(ValueError):
    """A saved source package is incomplete, unsafe, or does not match its pins."""


@dataclass(frozen=True)
class SavedCorpusOrigin:
    """Immutable, source-bound origin package description supplied by the runner."""

    registration: frozen.FrozenPoolRegistrationContext
    research_cases: tuple[dict[str, object], ...]
    navigation_targets: tuple[dict[str, object], ...]
    collection_directory: Path
    expected_collection_receipt_inventory_sha256: str
    capture_stage_directory: Path
    expected_capture_inventory_sha256: str
    expected_capture_manifest_sha256: str
    expected_package_sha256: str
    acquisition_plan_sidecar: Path | None = None
    operation_runner_source_sidecar: Path | None = None
    operation_runner_source_sha256: str | None = None


@dataclass(frozen=True)
class VerifiedHistoricalOperationRunnerModule:
    """Pinned historical source identity used only for saved-origin accounting."""

    origin_stage_uuid: str
    origin_source_revision: str
    origin_package_sha256: str
    module_sha256: str
    module_bytes: bytes


@dataclass(frozen=True)
class SavedCorpusStageBinding:
    """Fresh grading-stage binding, distinct from all origin receipt bindings."""

    stage_uuid: str
    stage_kind: str
    source_revision: str
    source_closure_sha256: str
    protocol_sha256: str
    cohorts_sha256: str
    candidate_identity_sha256: str
    candidate_endpoint_sha256: str
    packet_stage_uuid: str
    origin_package_sha256: str
    import_authority_receipt_sha256: str


@dataclass(frozen=True)
class VerifiedSavedCorpusImportPermit:
    """Typed result of checking a fresh, source-bound saved-corpus import permit."""

    status: str
    scope: str
    stage_uuid: str
    packet_stage_uuid: str
    source_revision: str
    source_closure_sha256: str
    protocol_sha256: str
    cohorts_sha256: str
    candidate_identity_sha256: str
    candidate_endpoint_sha256: str
    origin_package_sha256: str
    receipt_sha256: str


@dataclass(frozen=True)
class SavedCorpusImportResult:
    """Verified origin plus explicitly derived current-stage assessment inputs."""

    pipeline_inputs: Mapping[str, object]
    capture_inventory_bytes: bytes
    context_artifacts: Mapping[str, bytes]
    origin_pool_verified: frozen.VerifiedFrozenPoolEvidence
    origin_capture_result: source_capture.SourceCaptureResult
    origin_package_sha256: str
    import_receipt_bytes: bytes
    import_receipt_sha256: str
    capture_manifest_bytes: bytes
    capture_manifest_sha256: str
    historical_search_calls: int
    historical_capture_calls: int
    historical_scrape_calls: int
    historical_health_calls: int
    historical_captured_sources: int
    historical_failed_sources: int
    current_search_calls: int = 0
    current_capture_calls: int = 0
    acquisition_plan_bytes: bytes | None = None
    historical_operation_runner_module: VerifiedHistoricalOperationRunnerModule | None = None


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    try:
        return (
            json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n"
        ).encode()
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise SavedCorpusError("saved-corpus-json-invalid") from exc


def _strict(raw: bytes, label: str) -> dict[str, object]:
    def pairs(items: list[tuple[str, object]]) -> dict[str, object]:
        result: dict[str, object] = {}
        for key, value in items:
            if key in result:
                raise SavedCorpusError(f"{label}-duplicate-key")
            result[key] = value
        return result

    try:
        value = json.loads(
            raw.decode("utf-8"), object_pairs_hook=pairs, parse_constant=lambda _v: (_ for _ in ()).throw(ValueError())
        )
    except SavedCorpusError:
        raise
    except Exception as exc:
        raise SavedCorpusError(f"{label}-invalid-json") from exc
    canonical = _canonical(value)
    if type(value) is not dict or raw not in {canonical, canonical[:-1]}:
        raise SavedCorpusError(f"{label}-not-canonical")
    return value


def _private_file(path: Path, *, max_bytes: int) -> bytes:
    try:
        before = path.lstat()
        if (
            stat.S_ISLNK(before.st_mode)
            or not stat.S_ISREG(before.st_mode)
            or stat.S_IMODE(before.st_mode) != 0o600
            or before.st_size > max_bytes
        ):
            raise SavedCorpusError("saved-corpus-file-unsafe")
        flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
        fd = os.open(path, flags)
        try:
            after = os.fstat(fd)
            if (
                not stat.S_ISREG(after.st_mode)
                or after.st_ino != before.st_ino
                or after.st_dev != before.st_dev
                or stat.S_IMODE(after.st_mode) != 0o600
            ):
                raise SavedCorpusError("saved-corpus-file-raced")
            chunks: list[bytes] = []
            remaining = max_bytes + 1
            while remaining:
                chunk = os.read(fd, min(64_000, remaining))
                if not chunk:
                    break
                chunks.append(chunk)
                remaining -= len(chunk)
            raw = b"".join(chunks)
            if len(raw) > max_bytes:
                raise SavedCorpusError("saved-corpus-file-too-large")
            return raw
        finally:
            os.close(fd)
    except SavedCorpusError:
        raise
    except OSError as exc:
        raise SavedCorpusError("saved-corpus-file-unavailable") from exc


def _private_dir(path: Path) -> None:
    try:
        info = path.lstat()
    except OSError as exc:
        raise SavedCorpusError("saved-corpus-directory-unavailable") from exc
    if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode) or stat.S_IMODE(info.st_mode) != 0o700:
        raise SavedCorpusError("saved-corpus-directory-unsafe")


def _tree_rows(root: Path, label: str) -> list[dict[str, object]]:
    _private_dir(root)
    rows = []
    for path in sorted(root.rglob("*"), key=lambda item: item.relative_to(root).as_posix()):
        relative = path.relative_to(root).as_posix()
        if path.is_symlink():
            raise SavedCorpusError(f"{label}-symlink-rejected")
        info = path.lstat()
        if stat.S_ISDIR(info.st_mode):
            if stat.S_IMODE(info.st_mode) != 0o700:
                raise SavedCorpusError(f"{label}-directory-mode-invalid")
            continue
        raw = _private_file(path, max_bytes=max(_MAX_CAPTURE_FILE, _MAX_CONTEXT_FILE, 64_000_000))
        rows.append({"name": relative, "sha256": _sha(raw), "bytes": len(raw)})
    return rows


def package_sha256(collection_directory: Path, capture_stage_directory: Path) -> str:
    """Hash every immutable package byte by package-relative name."""
    rows = [{"tree": "collection", **row} for row in _tree_rows(Path(collection_directory), "collection")] + [
        {"tree": "capture", **row} for row in _tree_rows(Path(capture_stage_directory), "capture")
    ]
    return _sha(_canonical(rows))


def origin_acquisition_plan_bytes(origin: SavedCorpusOrigin) -> bytes:
    """Load the origin plan from its explicit sidecar or fixed legacy package path.

    A sidecar remains outside the immutable package and is trusted only through
    the acquisition-plan digest already present in the origin registration.
    No discovery or alternate-path search is performed.
    """
    if type(origin) is not SavedCorpusOrigin:
        raise SavedCorpusError("origin-acquisition-plan-origin-invalid")
    registration = origin.registration
    package_roots = (Path(origin.collection_directory), Path(origin.capture_stage_directory))
    embedded_path = package_roots[0] / "acquisition-plan.json"
    sidecar = origin.acquisition_plan_sidecar
    if sidecar is None:
        raw = _private_file(embedded_path, max_bytes=2_000_000)
    else:
        if not isinstance(sidecar, Path) or not sidecar.is_absolute():
            raise SavedCorpusError("origin-acquisition-plan-sidecar-path-invalid")
        try:
            if stat.S_ISLNK(sidecar.lstat().st_mode):
                raise SavedCorpusError("saved-corpus-file-unsafe")
            normalized_sidecar = sidecar.resolve(strict=True)
        except SavedCorpusError:
            raise
        except OSError as exc:
            raise SavedCorpusError("saved-corpus-file-unavailable") from exc
        for root in package_roots:
            try:
                normalized_root = root.resolve(strict=True)
            except OSError as exc:
                raise SavedCorpusError("saved-corpus-file-unavailable") from exc
            try:
                normalized_sidecar.relative_to(normalized_root)
            except ValueError:
                continue
            raise SavedCorpusError("origin-acquisition-plan-sidecar-inside-package")
        raw = _private_file(normalized_sidecar, max_bytes=2_000_000)
        try:
            embedded_raw = _private_file(embedded_path, max_bytes=2_000_000)
        except SavedCorpusError as exc:
            if exc.args != ("saved-corpus-file-unavailable",):
                raise
        else:
            if embedded_raw != raw:
                raise SavedCorpusError("origin-acquisition-plan-sidecar-conflicts-with-package")
    if _sha(raw) != registration.acquisition_plan_sha256:
        raise SavedCorpusError("origin-acquisition-plan-pin-mismatch")
    return raw


def verify_historical_operation_runner_module(origin: SavedCorpusOrigin) -> VerifiedHistoricalOperationRunnerModule:
    """Verify the explicitly supplied historical runner module sidecar.

    The expected digest is part of the current saved-origin registration/config.
    Preparation must verify that digest against the original source revision;
    runtime deliberately performs no Git or network lookup.
    """
    if type(origin) is not SavedCorpusOrigin:
        raise SavedCorpusError("origin-operation-runner-source-origin-invalid")
    registration = origin.registration
    sidecar = origin.operation_runner_source_sidecar
    expected_sha = origin.operation_runner_source_sha256
    if not isinstance(sidecar, Path) or not sidecar.is_absolute():
        raise SavedCorpusError("origin-operation-runner-source-sidecar-required")
    if type(expected_sha) is not str or not _SHA.fullmatch(expected_sha):
        raise SavedCorpusError("origin-operation-runner-source-pin-invalid")
    try:
        if stat.S_ISLNK(sidecar.lstat().st_mode):
            raise SavedCorpusError("saved-corpus-file-unsafe")
        normalized_sidecar = sidecar.resolve(strict=True)
        collection_root = Path(origin.collection_directory).resolve(strict=True)
        capture_root = Path(origin.capture_stage_directory).resolve(strict=True)
    except SavedCorpusError:
        raise
    except OSError as exc:
        raise SavedCorpusError("origin-operation-runner-source-unavailable") from exc
    for root in (collection_root, capture_root):
        try:
            normalized_sidecar.relative_to(root)
        except ValueError:
            continue
        raise SavedCorpusError("origin-operation-runner-source-sidecar-inside-package")
    module_bytes = _private_file(normalized_sidecar, max_bytes=2_000_000)
    if _sha(module_bytes) != expected_sha:
        raise SavedCorpusError("origin-operation-runner-source-pin-mismatch")
    if (
        type(registration) is not frozen.FrozenPoolRegistrationContext
        or type(registration.stage_uuid) is not str
        or _UUID.fullmatch(registration.stage_uuid) is None
        or type(registration.source_revision) is not str
        or re.fullmatch(r"[0-9a-f]{40}", registration.source_revision) is None
        or type(registration.source_closure_sha256) is not str
        or not _SHA.fullmatch(registration.source_closure_sha256)
        or type(origin.expected_package_sha256) is not str
        or not _SHA.fullmatch(origin.expected_package_sha256)
    ):
        raise SavedCorpusError("origin-operation-runner-source-registration-invalid")
    return VerifiedHistoricalOperationRunnerModule(
        origin_stage_uuid=registration.stage_uuid,
        origin_source_revision=registration.source_revision,
        origin_package_sha256=origin.expected_package_sha256,
        module_sha256=expected_sha,
        module_bytes=module_bytes,
    )


def _verify_archive_response(
    *,
    capture_root: Path,
    origin_stage_uuid: str,
    source_revision: str,
    operation_id: str,
    expected_request_body_sha256: str,
    expected_receipt_sha256: str,
    expected_response_sha256: str | None,
    expected_response_bytes: int | None,
    must_succeed: bool,
) -> tuple[dict[str, object], bytes]:
    operation_dir = capture_root / operation_id
    _private_dir(operation_dir)
    receipt_raw = _private_file(operation_dir / "receipt.json", max_bytes=16_384)
    receipt = _strict(receipt_raw, "capture-response-receipt")
    if set(receipt) != {
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
    } or set(os.listdir(operation_dir)) != {"receipt.json", "response.bin"}:
        raise SavedCorpusError("capture-response-receipt-shape")
    if _sha(receipt_raw) != expected_receipt_sha256:
        raise SavedCorpusError("capture-response-receipt-pin-mismatch")
    body = _private_file(operation_dir / "response.bin", max_bytes=_MAX_CAPTURE_FILE)
    if (
        receipt.get("schema") != response_receipts.SCHEMA
        or receipt.get("stage_uuid") != origin_stage_uuid
        or receipt.get("source_revision") != source_revision
        or receipt.get("operation_id") != operation_id
        or receipt.get("request_body_sha256") != expected_request_body_sha256
        or receipt.get("response_body_sha256") != _sha(body)
        or receipt.get("response_body_bytes") != len(body)
        or (expected_response_sha256 is not None and expected_response_sha256 != _sha(body))
        or (expected_response_bytes is not None and expected_response_bytes != len(body))
    ):
        raise SavedCorpusError("capture-response-body-binding-mismatch")
    status = receipt.get("status")
    http_status = receipt.get("http_status")
    complete = receipt.get("complete")
    if type(complete) is not bool or type(status) is not str:
        raise SavedCorpusError("capture-response-status-invalid")
    if must_succeed and not (
        complete and status == "complete" and type(http_status) is int and 200 <= http_status < 300
    ):
        raise SavedCorpusError("captured-source-response-not-successful")
    return receipt, body


def _validate_origin_binding(binding: SavedCorpusStageBinding, origin: SavedCorpusOrigin) -> None:
    registration = origin.registration
    for name, value in (
        ("stage_uuid", binding.stage_uuid),
        ("packet_stage_uuid", binding.packet_stage_uuid),
    ):
        if type(value) is not str or _UUID.fullmatch(value) is None or str(uuid.UUID(value)) != value:
            raise SavedCorpusError(f"current-{name}-invalid")
    if binding.stage_kind != "development":
        raise SavedCorpusError("saved-corpus-development-only")
    if (
        binding.stage_uuid in {registration.stage_uuid, binding.packet_stage_uuid}
        or binding.packet_stage_uuid == registration.stage_uuid
    ):
        raise SavedCorpusError("current-stage-identities-not-fresh")
    if type(binding.source_revision) is not str or _REVISION.fullmatch(binding.source_revision) is None:
        raise SavedCorpusError("current-source-revision-invalid")
    for pin in (
        binding.source_closure_sha256,
        binding.protocol_sha256,
        binding.cohorts_sha256,
        binding.candidate_identity_sha256,
        binding.candidate_endpoint_sha256,
        binding.origin_package_sha256,
        binding.import_authority_receipt_sha256,
    ):
        if type(pin) is not str or _SHA.fullmatch(pin) is None:
            raise SavedCorpusError("current-stage-pin-invalid")
    if binding.origin_package_sha256 != origin.expected_package_sha256:
        raise SavedCorpusError("origin-package-registration-pin-mismatch")
    if binding.cohorts_sha256 != registration.cohorts_sha256:
        raise SavedCorpusError("cohort-bytes-must-remain-identical")
    if binding.candidate_identity_sha256 != registration.candidate_identity_sha256:
        raise SavedCorpusError("candidate-identity-must-remain-fixed")
    if binding.candidate_endpoint_sha256 != registration.candidate_endpoint_sha256:
        raise SavedCorpusError("candidate-endpoint-must-remain-fixed")
    if len(origin.research_cases) != 8 or len(origin.navigation_targets) != 5:
        raise SavedCorpusError("origin-operation-count-invalid")


def verify_and_import_saved_corpus(
    origin: SavedCorpusOrigin,
    binding: SavedCorpusStageBinding,
    research_cases: Sequence[dict[str, object]],
    navigation_targets: Sequence[dict[str, object]],
) -> SavedCorpusImportResult:
    """Verify original artifacts then derive assessment inputs for fresh grading.

    This function performs no network requests and writes no files. A caller
    must verify its external import-authority receipt before invoking it.
    """
    if type(origin) is not SavedCorpusOrigin or type(binding) is not SavedCorpusStageBinding:
        raise SavedCorpusError("saved-corpus-input-type")
    _validate_origin_binding(binding, origin)
    if tuple(research_cases) != origin.research_cases or tuple(navigation_targets) != origin.navigation_targets:
        raise SavedCorpusError("current-cohort-must-match-origin-exactly")
    if tuple(row.get("task_id") for row in research_cases) != origin.registration.task_ids:
        raise SavedCorpusError("research-task-identities-mismatch")
    if tuple(row.get("target_id") for row in navigation_targets) != origin.registration.navigation_ids:
        raise SavedCorpusError("navigation-identities-mismatch")
    observed_package_sha = package_sha256(origin.collection_directory, origin.capture_stage_directory)
    if observed_package_sha != origin.expected_package_sha256:
        raise SavedCorpusError("origin-package-integrity-mismatch")
    try:
        acquisition_plan_bytes = origin_acquisition_plan_bytes(origin)
    except SavedCorpusError as exc:
        raise SavedCorpusError("origin-acquisition-plan-unavailable") from exc
    historical_operation_runner_module = verify_historical_operation_runner_module(origin)
    try:
        observed_collection_inventory = resource_evidence.receipt_inventory_sha256(Path(origin.collection_directory))
    except Exception as exc:
        raise SavedCorpusError("origin-collection-receipt-inventory-invalid") from exc
    if observed_collection_inventory != origin.expected_collection_receipt_inventory_sha256:
        raise SavedCorpusError("origin-collection-receipt-inventory-mismatch")

    collection_root = Path(origin.collection_directory)
    try:
        evidence_bytes = _private_file(collection_root / "frozen-pool-evidence.json", max_bytes=64_000_000)
        receipt_bytes = _private_file(collection_root / "collector-receipt.json", max_bytes=64_000_000)
        availability_bytes = _private_file(collection_root / "engine-availability.json", max_bytes=64_000_000)
        evidence = frozen.FrozenPoolEvidence(
            snapshots_directory=collection_root,
            evidence_bytes=evidence_bytes,
            expected_evidence_sha256=_sha(evidence_bytes),
            collector_receipt_bytes=receipt_bytes,
            expected_collector_receipt_sha256=_sha(receipt_bytes),
            availability_bytes=availability_bytes,
            expected_availability_sha256=_sha(availability_bytes),
        )
        # The archive's original completion instant is used as its historical
        # verification point; it is not compared to the new stage's clock.
        evidence_doc = _strict(evidence_bytes, "origin-pool-evidence")
        started = evidence_doc["collection_started_monotonic"]
        completed = evidence_doc["collection_completed_monotonic"]
        deadline = evidence_doc["stage_deadline_monotonic"]
        phase = frozen.run_frozen_pool_input_phase(
            list(origin.research_cases),
            evidence=evidence,
            registration=origin.registration,
            stage_started_monotonic=started,
            stage_deadline_monotonic=deadline,
            now_monotonic=completed,
        )
    except SavedCorpusError:
        raise
    except Exception as exc:
        raise SavedCorpusError("origin-pool-verification-failed") from exc

    capture_root = Path(origin.capture_stage_directory)
    inventory_raw = _private_file(capture_root / "inventory.json", max_bytes=4_000_000)
    source_manifest_raw = _private_file(capture_root / "source-manifest.json", max_bytes=4_000_000)
    if _sha(inventory_raw) != origin.expected_capture_inventory_sha256:
        raise SavedCorpusError("origin-capture-inventory-pin-mismatch")
    if _sha(source_manifest_raw) != origin.expected_capture_manifest_sha256:
        raise SavedCorpusError("origin-capture-manifest-pin-mismatch")
    capture_inventory = _strict(inventory_raw, "origin-capture-inventory")
    source_manifest = _strict(source_manifest_raw, "origin-capture-manifest")
    registration = origin.registration
    try:
        source_manifest, validated_source_rows = source_capture._validate_manifest(
            source_manifest_raw, registration.protocol_sha256
        )
    except Exception as exc:
        raise SavedCorpusError("origin-capture-manifest-invalid") from exc
    if (
        capture_inventory.get("schema") != "coverage-source-capture-inventory/1"
        or capture_inventory.get("stage_uuid") != registration.stage_uuid
        or capture_inventory.get("source_revision") != registration.source_revision
        or capture_inventory.get("protocol_sha256") != registration.protocol_sha256
        or capture_inventory.get("cohorts_sha256") != registration.cohorts_sha256
        or capture_inventory.get("status") not in {"complete", "complete-with-source-failures"}
        or capture_inventory.get("quality_credit") is not False
        or source_manifest.get("stage_uuid") != registration.stage_uuid
        or source_manifest.get("source_revision") != registration.source_revision
        or source_manifest.get("protocol_sha256") != registration.protocol_sha256
        or source_manifest.get("cohorts_sha256") != registration.cohorts_sha256
        or source_manifest.get("candidate_identity_sha256") != registration.candidate_identity_sha256
        or source_manifest.get("candidate_endpoint_sha256") != registration.candidate_endpoint_sha256
    ):
        raise SavedCorpusError("origin-capture-binding-mismatch")
    capture_qualification = capture_inventory.get("protected_capture_qualification")
    if type(capture_qualification) is not dict:
        raise SavedCorpusError("origin-capture-qualification-missing")
    qualification_raw = _private_file(capture_root / "protected-capture-qualification.json", max_bytes=64_000)
    qualification_sha = _sha(qualification_raw)
    qualification_doc = _strict(qualification_raw, "origin-capture-qualification")
    qualification_bindings = qualification_doc.get("bindings")
    if (
        capture_qualification.get("receipt_file") != "protected-capture-qualification.json"
        or capture_qualification.get("receipt_sha256") != qualification_sha
        or qualification_doc.get("schema") != "coverage-operator-authority-receipt/1"
        or qualification_doc.get("decision") != "allow"
        or qualification_doc.get("scope") != "protected-source-capture"
        or type(qualification_bindings) is not dict
        or capture_qualification.get("bindings") != qualification_bindings
        or qualification_bindings.get("stage_uuid") != registration.stage_uuid
        or qualification_bindings.get("source_revision") != registration.source_revision
        or qualification_bindings.get("protocol_sha256") != registration.protocol_sha256
        or qualification_bindings.get("candidate_identity_sha256") != registration.candidate_identity_sha256
        or qualification_bindings.get("candidate_endpoint_sha256") != registration.candidate_endpoint_sha256
        or qualification_bindings.get("candidate_endpoint_scheme") not in {"http", "https"}
        or (
            qualification_bindings.get("ca_bundle_sha256") is not None
            and (
                type(qualification_bindings.get("ca_bundle_sha256")) is not str
                or _SHA.fullmatch(qualification_bindings["ca_bundle_sha256"]) is None
            )
        )
    ):
        raise SavedCorpusError("origin-capture-qualification-binding-mismatch")
    if source_manifest.get("sources") != list(phase.pipeline_inputs["capture_manifest"]["sources"]):
        raise SavedCorpusError("origin-capture-search-provenance-mismatch")
    capture_rows = capture_inventory.get("sources")
    source_rows = validated_source_rows
    if type(capture_rows) is not list or type(source_rows) is not list or len(capture_rows) != len(source_rows):
        raise SavedCorpusError("origin-capture-row-count-mismatch")
    if capture_inventory.get("source_manifest_sha256") != _sha(source_manifest_raw):
        raise SavedCorpusError("origin-capture-manifest-binding-mismatch")

    context_artifacts: dict[str, bytes] = {}
    expected_operation_dirs = {"health"}
    captured_count = failed_count = scrape_calls = 0
    for sequence, (row, source) in enumerate(zip(capture_rows, source_rows, strict=True), start=1):
        if type(row) is not dict or type(source) is not dict:
            raise SavedCorpusError("origin-capture-row-shape")
        for key in ("task_id", "source_id", "result_index", "url", "title", "engine"):
            source_key = "url" if key == "url" else key
            if row.get(key) != source.get(source_key):
                raise SavedCorpusError("origin-capture-row-provenance-mismatch")
        if row.get("attempted") is True:
            operation_id = f"source-{sequence:04d}"
            expected_operation_dirs.add(operation_id)
            scrape_calls += 1
            _verify_archive_response(
                capture_root=capture_root,
                origin_stage_uuid=registration.stage_uuid,
                source_revision=registration.source_revision,
                operation_id=operation_id,
                expected_request_body_sha256=_sha(
                    source_capture._canonical_json(
                        {"url": source["url"], "formats": ["markdown"], "onlyMainContent": True}
                    )
                ),
                expected_receipt_sha256=row.get("receipt_sha256"),
                expected_response_sha256=row.get("response_sha256"),
                expected_response_bytes=row.get("response_body_bytes"),
                must_succeed=row.get("status") == "captured",
            )
        if row.get("status") == "captured":
            if row.get("attempted") is not True:
                raise SavedCorpusError("captured-source-not-attempted")
            artifact_name = row.get("context_artifact")
            if type(artifact_name) is not str or re.fullmatch(r"context-[0-9]{4}\.json", artifact_name) is None:
                raise SavedCorpusError("captured-context-name-invalid")
            raw = _private_file(capture_root / artifact_name, max_bytes=_MAX_CONTEXT_FILE)
            artifact = _strict(raw, "origin-capture-context")
            if (
                artifact.get("schema") != "coverage-source-context/1"
                or artifact.get("task_id") != row.get("task_id")
                or artifact.get("source_id") != row.get("source_id")
                or artifact.get("result_index") != row.get("result_index")
                or artifact.get("source_url") != row.get("url")
                or artifact.get("title") != row.get("title")
                or artifact.get("engine") != row.get("engine")
                or artifact.get("context_sha256") != row.get("context_sha256")
                or _sha(str(artifact.get("context", "")).encode("utf-8")) != row.get("context_sha256")
                or artifact.get("context_characters") != row.get("context_characters")
            ):
                raise SavedCorpusError("origin-capture-context-binding-mismatch")
            if artifact_name in context_artifacts:
                raise SavedCorpusError("origin-capture-context-duplicate")
            context_artifacts[artifact_name] = raw
            captured_count += 1
        elif row.get("status") == "source_response_failure":
            failed_count += 1
            if row.get("attempted") is not True:
                raise SavedCorpusError("failed-source-not-attempted")
            if row.get("context_artifact") is not None or row.get("context_sha256") is not None:
                raise SavedCorpusError("failed-source-has-captured-context")
        elif row.get("status") in {"duplicate_url_not_attempted", "unsupported_url_not_attempted"}:
            if row.get("attempted") is not False:
                raise SavedCorpusError("unattempted-source-dispatch-contradiction")
        else:
            raise SavedCorpusError("origin-capture-status-invalid")

    if set(context_artifacts) != {path.name for path in capture_root.glob("context-[0-9][0-9][0-9][0-9].json")}:
        raise SavedCorpusError("origin-capture-context-inventory-mismatch")
    expected_operation_dirs.add("health")
    actual_operation_dirs = {path.name for path in capture_root.iterdir() if path.is_dir()}
    if actual_operation_dirs != expected_operation_dirs:
        raise SavedCorpusError("origin-capture-response-inventory-mismatch")
    expected_top_level = {
        "inventory.json",
        "source-manifest.json",
        "protected-capture-qualification.json",
        *expected_operation_dirs,
        *context_artifacts,
    }
    if {path.name for path in capture_root.iterdir()} != expected_top_level:
        raise SavedCorpusError("origin-capture-top-level-inventory-mismatch")
    health_receipt = capture_inventory.get("health_receipt_sha256")
    if type(health_receipt) is not str or _SHA.fullmatch(health_receipt) is None:
        raise SavedCorpusError("origin-health-receipt-pin-invalid")
    _verify_archive_response(
        capture_root=capture_root,
        origin_stage_uuid=registration.stage_uuid,
        source_revision=registration.source_revision,
        operation_id="health",
        expected_request_body_sha256=_sha(b""),
        expected_receipt_sha256=health_receipt,
        expected_response_sha256=_sha(
            _private_file(capture_root / "health" / "response.bin", max_bytes=_MAX_CAPTURE_FILE)
        ),
        expected_response_bytes=capture_inventory.get("health_response_body_bytes"),
        must_succeed=True,
    )
    if (
        capture_inventory.get("scrape_calls") != scrape_calls
        or capture_inventory.get("health_calls") != 1
        or capture_inventory.get("owned_http_calls") != scrape_calls + 1
        or capture_inventory.get("status") != ("complete" if failed_count == 0 else "complete-with-source-failures")
    ):
        raise SavedCorpusError("origin-capture-call-count-mismatch")

    origin_inputs = dict(phase.pipeline_inputs)
    derived_manifest = dict(origin_inputs["capture_manifest"])
    derived_manifest.update(
        {
            "stage_uuid": binding.stage_uuid,
            "source_revision": binding.source_revision,
            "protocol_sha256": binding.protocol_sha256,
            "cohorts_sha256": binding.cohorts_sha256,
            "candidate_identity_sha256": binding.candidate_identity_sha256,
            "candidate_endpoint_sha256": binding.candidate_endpoint_sha256,
        }
    )
    derived_manifest_bytes = _canonical(derived_manifest)
    origin_inputs.update(
        {
            "stage_uuid": binding.stage_uuid,
            "capture_manifest": derived_manifest,
            "capture_manifest_bytes": derived_manifest_bytes,
            "capture_manifest_sha256": _sha(derived_manifest_bytes),
            "candidate_identity_sha256": binding.candidate_identity_sha256,
            "candidate_endpoint_sha256": binding.candidate_endpoint_sha256,
            "quality_credit": False,
            "admission_created": False,
            "saved_origin": {
                "schema": "coverage-saved-corpus-origin-reference/1",
                "stage_uuid": registration.stage_uuid,
                "source_revision": registration.source_revision,
                "pool_manifest_sha256": phase.verified.pool_manifest_sha256,
                "pool_snapshot_index_sha256": phase.verified.snapshot_index_sha256,
                "source_manifest_sha256": _sha(source_manifest_raw),
                "capture_inventory_sha256": _sha(inventory_raw),
            },
        }
    )
    import_doc = {
        "schema": SCHEMA,
        "status": "verified-for-fresh-grading",
        "stage_uuid": binding.stage_uuid,
        "stage_kind": binding.stage_kind,
        "source_revision": binding.source_revision,
        "source_closure_sha256": binding.source_closure_sha256,
        "protocol_sha256": binding.protocol_sha256,
        "cohorts_sha256": binding.cohorts_sha256,
        "candidate_identity_sha256": binding.candidate_identity_sha256,
        "candidate_endpoint_sha256": binding.candidate_endpoint_sha256,
        "packet_stage_uuid": binding.packet_stage_uuid,
        "origin_stage_uuid": registration.stage_uuid,
        "origin_source_revision": registration.source_revision,
        "origin_package_sha256": observed_package_sha,
        "origin_pool_manifest_sha256": phase.verified.pool_manifest_sha256,
        "origin_pool_snapshot_index_sha256": phase.verified.snapshot_index_sha256,
        "origin_collection_receipt_sha256": phase.verified.collector_receipt_sha256,
        "origin_capture_manifest_sha256": _sha(source_manifest_raw),
        "origin_capture_inventory_sha256": _sha(inventory_raw),
        "derived_capture_manifest_sha256": _sha(derived_manifest_bytes),
        "import_authority_receipt_sha256": binding.import_authority_receipt_sha256,
        "research_task_ids": list(registration.task_ids),
        "navigation_target_ids": list(registration.navigation_ids),
        "source_rows": len(capture_rows),
        "captured_contexts": captured_count,
        "source_failures": failed_count,
        "historical_search_calls": _physical_search_calls(collection_root),
        "historical_capture_calls": capture_inventory["owned_http_calls"],
        "historical_scrape_calls": scrape_calls,
        "historical_health_calls": 1,
        "current_search_calls": 0,
        "current_capture_calls": 0,
        "quality_credit": False,
    }
    import_bytes = _canonical(import_doc)
    current_inventory = {
        "schema": CAPTURE_IMPORT_SCHEMA,
        "stage_uuid": binding.stage_uuid,
        "source_revision": binding.source_revision,
        "source_manifest_sha256": _sha(derived_manifest_bytes),
        "protocol_sha256": binding.protocol_sha256,
        "cohorts_sha256": binding.cohorts_sha256,
        "status": capture_inventory["status"],
        "owned_http_calls": 0,
        "health_calls": 0,
        "scrape_calls": 0,
        "quality_credit": False,
        "capture_origin": "saved-corpus-import/1",
        "origin_stage_uuid": registration.stage_uuid,
        "origin_inventory_sha256": _sha(inventory_raw),
        "import_receipt_sha256": _sha(import_bytes),
        "source_count": len(capture_rows),
        "captured_count": captured_count,
        "failed_count": failed_count,
        "unattempted_count": sum(row.get("attempted") is False for row in capture_rows),
        "sources": capture_rows,
    }
    current_inventory_bytes = _canonical(current_inventory)
    origin_capture_result = source_capture.SourceCaptureResult(
        status=capture_inventory["status"],
        stage_uuid=registration.stage_uuid,
        source_count=len(capture_rows),
        captured_count=captured_count,
        failed_count=failed_count,
        unattempted_count=current_inventory["unattempted_count"],
        owned_http_calls=capture_inventory["owned_http_calls"],
        receipt_directory=capture_root,
        private_inventory=tuple(capture_rows),
        candidate_endpoint_scheme=qualification_bindings["candidate_endpoint_scheme"],
        ca_bundle_sha256=qualification_bindings["ca_bundle_sha256"],
        quality_credit=False,
    )
    return SavedCorpusImportResult(
        pipeline_inputs=origin_inputs,
        capture_inventory_bytes=current_inventory_bytes,
        context_artifacts=context_artifacts,
        origin_pool_verified=phase.verified,
        origin_capture_result=origin_capture_result,
        origin_package_sha256=observed_package_sha,
        import_receipt_bytes=import_bytes,
        import_receipt_sha256=_sha(import_bytes),
        capture_manifest_bytes=derived_manifest_bytes,
        capture_manifest_sha256=_sha(derived_manifest_bytes),
        historical_search_calls=_physical_search_calls(collection_root),
        historical_capture_calls=capture_inventory["owned_http_calls"],
        historical_scrape_calls=scrape_calls,
        historical_health_calls=1,
        historical_captured_sources=captured_count,
        historical_failed_sources=failed_count,
        acquisition_plan_bytes=acquisition_plan_bytes,
        historical_operation_runner_module=historical_operation_runner_module,
    )


def _physical_search_calls(collection_root: Path) -> int:
    summary = _strict(
        _private_file(collection_root / "collection-summary.json", max_bytes=64_000_000), "origin-collection-summary"
    )
    if summary.get("status") != "complete" or summary.get("fatal_uninvoked_slots") != []:
        raise SavedCorpusError("origin-collection-not-complete")
    count = summary.get("physical_request_count")
    if type(count) is not int or count < 0:
        raise SavedCorpusError("origin-search-call-count-invalid")
    return count


__all__ = [
    "SavedCorpusError",
    "SavedCorpusImportResult",
    "SavedCorpusOrigin",
    "SavedCorpusStageBinding",
    "VerifiedSavedCorpusImportPermit",
    "package_sha256",
    "verify_and_import_saved_corpus",
]
