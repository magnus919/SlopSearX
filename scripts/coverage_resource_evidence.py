"""Derive resource observations from verified coverage-stage artifacts.

This module does not execute providers, grant admission, or infer quality. It
returns only values supported by pinned snapshots, response archives, packet
bytes, and grade-closure receipts. Protocol limits are reported separately as
configuration provenance, never substituted for measured values. Capture and
answer execution settings require source-hashed producer attestations;
historical inventories without them remain unknown. Injected transport retry
behavior is not inferred from the executor's own call count.
"""

from __future__ import annotations

import hashlib
import json
import math
import os
import re
import stat
import time
import uuid
from collections import Counter
from contextvars import ContextVar
from dataclasses import dataclass
from pathlib import Path
from typing import Mapping

from scripts import coverage_answer_execution as answer_execution
from scripts import coverage_execution_controls as execution_controls
from scripts import coverage_live_acquire
from scripts import coverage_source_capture as source_capture
from scripts import coverage_study_acquire as offline_acquisition
from scripts import intent_ranking_receipts as receipts

_SHA = re.compile(r"[0-9a-f]{64}\Z")
_REVISION = re.compile(r"[0-9a-f]{40}\Z")
_SAVED_SELECTOR_MAP_FIELDS = frozenset(
    {
        "pool_input_mode",
        "origin_pool_snapshot_index_sha256",
        "origin_pool_manifest_sha256",
        "origin_stage_uuid",
        "origin_source_revision",
        "origin_source_closure_sha256",
        "origin_protocol_sha256",
        "origin_cohorts_sha256",
        "origin_acquisition_plan_sha256",
        "saved_import_receipt_sha256",
        "derived_capture_manifest_sha256",
    }
)
_ENGINE_NAMES = ("wikipedia", "arxiv", "github", "openalex", "brave")
_DEADLINE: ContextVar[float | None] = ContextVar("coverage_resource_deadline", default=None)


class ResourceEvidenceError(ValueError):
    """An observation could not be bound to the supplied stage artifacts."""


def _check_deadline() -> None:
    deadline = _DEADLINE.get()
    if deadline is not None and time.monotonic() >= deadline:
        raise ResourceEvidenceError("resource-collection-deadline-exceeded")


def _read_private(path: Path, *, max_bytes: int) -> bytes:
    # Stop between bounded reads; an OS filesystem call already in progress
    # cannot be interrupted here. The coordinator also checks final fsync.
    _check_deadline()
    raw = receipts._read_private(path, max_bytes=max_bytes)
    _check_deadline()
    return raw


@dataclass(frozen=True)
class ResourceEvidenceReport:
    stage_uuid: str
    observations: Mapping[str, object]
    configuration_provenance: Mapping[str, object]
    source_receipt_sha256: str


@dataclass(frozen=True)
class StageCloseoutObservation:
    """Elapsed-time proof joined to its independently pinned final inventory."""

    stage_uuid: str
    stage_elapsed_seconds: float
    terminal_status: str
    terminal_reason: str | None
    inventory_sha256: str
    closeout_receipt_sha256: str


def _sha(raw: bytes) -> str:
    _check_deadline()
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")


def _strict_json(raw: bytes, label: str) -> object:
    _check_deadline()

    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ResourceEvidenceError(f"{label}-duplicate-key")
            result[key] = value
        return result

    try:
        return json.loads(
            raw.decode("utf-8"), object_pairs_hook=pairs, parse_constant=lambda _x: (_ for _ in ()).throw(ValueError())
        )
    except Exception as exc:
        raise ResourceEvidenceError(f"{label}-invalid") from exc


def _decode_capture_body(raw: bytes, content_encoding: object, label: str) -> bytes:
    """Replay the source-capture decoder against its pinned encoded archive."""
    encoding = "identity" if content_encoding is None else content_encoding
    if type(encoding) is not str:
        raise ResourceEvidenceError(f"{label}-content-encoding-invalid")
    try:
        decoder = source_capture._BoundedContentDecoder(encoding)
        decoded = decoder.decode(raw, source_capture.MAX_RESPONSE_BYTES)
        if len(decoded) > source_capture.MAX_RESPONSE_BYTES:
            raise ResourceEvidenceError(f"{label}-decoded-response-too-large")
        tail = decoder.finish(source_capture.MAX_RESPONSE_BYTES - len(decoded))
        if len(decoded) + len(tail) > source_capture.MAX_RESPONSE_BYTES:
            raise ResourceEvidenceError(f"{label}-decoded-response-too-large")
        return decoded + tail
    except ResourceEvidenceError:
        raise
    except Exception as exc:
        raise ResourceEvidenceError(f"{label}-content-encoding-invalid") from exc


def _require_private_directory(path: Path) -> None:
    try:
        info = path.lstat()
    except OSError as exc:
        raise ResourceEvidenceError("private-directory-unavailable") from exc
    if (
        stat.S_ISLNK(info.st_mode)
        or not stat.S_ISDIR(info.st_mode)
        or stat.S_IMODE(info.st_mode) != 0o700
        or info.st_uid != os.geteuid()
    ):
        raise ResourceEvidenceError("private-directory-invalid")


def verify_stage_closeout(
    *,
    inventory_path: str | os.PathLike[str],
    expected_inventory_sha256: str,
    closeout_receipt_path: str | os.PathLike[str],
    expected_closeout_receipt_sha256: str,
    expected_stage_uuid: str,
    expected_source_revision: str,
    protocol_bytes: bytes,
    cohorts_bytes: bytes,
    expected_protocol_sha256: str,
    expected_cohorts_sha256: str,
) -> StageCloseoutObservation:
    """Verify the post-inventory-fsync timing receipt for later-stage use.

    The closeout file is intentionally written after the measured endpoint;
    its own fsync is not included in ``stage_elapsed_seconds``. Both files must
    live in the same private stage directory, and their expected digests must
    come from the immutable StageResult/independent stage handoff.
    """
    inv_path = Path(inventory_path)
    close_path = Path(closeout_receipt_path)
    if close_path.parent != inv_path.parent or close_path.name != "stage-closeout.json":
        raise ResourceEvidenceError("stage-closeout-path-binding")
    _require_private_directory(inv_path.parent)
    if not _SHA.fullmatch(expected_inventory_sha256) or not _SHA.fullmatch(expected_closeout_receipt_sha256):
        raise ResourceEvidenceError("stage-closeout-expected-pin-invalid")
    if (
        type(protocol_bytes) is not bytes
        or type(cohorts_bytes) is not bytes
        or _sha(protocol_bytes) != expected_protocol_sha256
        or _sha(cohorts_bytes) != expected_cohorts_sha256
    ):
        raise ResourceEvidenceError("stage-closeout-protocol-cohort-pin-mismatch")
    protocol = _strict_json(protocol_bytes, "stage-closeout-protocol")
    if type(protocol) is not dict or protocol.get("cohorts_sha256") != expected_cohorts_sha256:
        raise ResourceEvidenceError("stage-closeout-protocol-cohort-binding")
    try:
        inventory_raw = _read_private(inv_path, max_bytes=4_000_000)
        closeout_raw = _read_private(close_path, max_bytes=16_384)
    except Exception as exc:
        raise ResourceEvidenceError("stage-closeout-artifact-unavailable") from exc
    if _sha(inventory_raw) != expected_inventory_sha256:
        raise ResourceEvidenceError("stage-closeout-inventory-pin-mismatch")
    if _sha(closeout_raw) != expected_closeout_receipt_sha256:
        raise ResourceEvidenceError("stage-closeout-receipt-pin-mismatch")
    inventory = _strict_json(inventory_raw, "stage-inventory")
    closeout = _strict_json(closeout_raw, "stage-closeout")
    if _canonical(inventory) != inventory_raw or _canonical(closeout) != closeout_raw:
        raise ResourceEvidenceError("stage-closeout-noncanonical")
    if type(inventory) is not dict or type(closeout) is not dict:
        raise ResourceEvidenceError("stage-closeout-shape-invalid")
    expected_keys = {
        "schema",
        "stage_uuid",
        "stage_kind",
        "source_revision",
        "protocol_sha256",
        "cohorts_sha256",
        "preacquisition_binding_sha256",
        "final_inventory_sha256",
        "final_inventory_status",
        "final_inventory_terminal_reason",
        "stage_started_monotonic",
        "observed_after_final_inventory_fsync_monotonic",
        "stage_elapsed_seconds",
        "stage_deadline_seconds",
        "measurement_basis",
    }
    if set(closeout) != expected_keys:
        raise ResourceEvidenceError("stage-closeout-fields-invalid")
    if (
        closeout["schema"] not in {"coverage-stage-closeout/1", "coverage-stage-closeout/2"}
        or closeout["stage_uuid"] != expected_stage_uuid
        or closeout["source_revision"] != expected_source_revision
        or closeout["protocol_sha256"] != expected_protocol_sha256
        or closeout["cohorts_sha256"] != expected_cohorts_sha256
        or closeout["final_inventory_sha256"] != expected_inventory_sha256
        or closeout["stage_kind"] != inventory.get("stage_kind")
        or closeout["preacquisition_binding_sha256"] != inventory.get("preacquisition_binding_sha256")
        or closeout["final_inventory_status"] != inventory.get("status")
        or closeout["final_inventory_terminal_reason"] != inventory.get("terminal_reason")
        or inventory.get("stage_uuid") != expected_stage_uuid
        or inventory.get("source_revision") != expected_source_revision
        or inventory.get("protocol_sha256") != expected_protocol_sha256
        or inventory.get("cohorts_sha256") != expected_cohorts_sha256
    ):
        raise ResourceEvidenceError("stage-closeout-binding-mismatch")
    started = closeout["stage_started_monotonic"]
    stopped = closeout["observed_after_final_inventory_fsync_monotonic"]
    elapsed = closeout["stage_elapsed_seconds"]
    deadline = closeout["stage_deadline_seconds"]
    numbers = (started, stopped, elapsed, deadline)
    if any(type(value) not in {int, float} or not math.isfinite(value) for value in numbers):
        raise ResourceEvidenceError("stage-closeout-time-invalid")
    if started < 0 or stopped < started or elapsed < 0 or deadline <= 0 or deadline > 28_800:
        raise ResourceEvidenceError("stage-closeout-time-range-invalid")
    if elapsed != stopped - started:
        raise ResourceEvidenceError("stage-closeout-elapsed-mismatch")
    expected_measurement = (
        "stage start through final inventory file and directory fsync; closeout receipt fsync excluded"
        if closeout["schema"] == "coverage-stage-closeout/1"
        else "stage start through final decision inventory file and directory fsync; closeout receipt fsync excluded"
    )
    if closeout["measurement_basis"] != expected_measurement:
        raise ResourceEvidenceError("stage-closeout-measurement-basis-invalid")
    if elapsed >= deadline and inventory.get("status") != "terminal-incomplete":
        raise ResourceEvidenceError("stage-closeout-late-stage-not-invalidated")
    if elapsed >= deadline and inventory.get("gate_result_authoritative") is not False:
        raise ResourceEvidenceError("stage-closeout-late-gates-still-authoritative")
    terminal_reason = inventory.get("terminal_reason")
    if terminal_reason is not None and type(terminal_reason) is not str:
        raise ResourceEvidenceError("stage-closeout-terminal-reason-invalid")
    return StageCloseoutObservation(
        stage_uuid=expected_stage_uuid,
        stage_elapsed_seconds=float(elapsed),
        terminal_status=inventory["status"],
        terminal_reason=terminal_reason,
        inventory_sha256=expected_inventory_sha256,
        closeout_receipt_sha256=expected_closeout_receipt_sha256,
    )


def _receipt_any(root: Path, *, bindings: dict, expected_sha256: str) -> tuple[dict, bytes]:
    """Verify complete or partial receipt bytes with the archive's safe readers."""
    try:
        _require_private_directory(root)
        bound = receipts._validate_bindings(bindings)
        if not _SHA.fullmatch(expected_sha256):
            raise ResourceEvidenceError("receipt-sha-invalid")
        archive_root = receipts._root_dir(root)
        stage_dir = receipts._private_dir(archive_root, bound["stage_uuid"], create=False)
        slot = receipts._private_dir(stage_dir, bound["operation_id"], create=False)
        if {path.name for path in slot.iterdir()} != receipts._FILES:
            raise ResourceEvidenceError("receipt-slot-inventory")
        raw_receipt = _read_private(slot / "receipt.json", max_bytes=16_384)
        if _sha(raw_receipt) != expected_sha256:
            raise ResourceEvidenceError("receipt-digest-mismatch")
        receipt = receipts._strict_json(raw_receipt)
        if type(receipt) is not dict or receipts._canonical_json(receipt) != raw_receipt:
            raise ResourceEvidenceError("receipt-shape-or-canonicality")
        expected = {
            "schema": receipts.SCHEMA,
            **bound,
            "complete": receipt.get("complete"),
            "status": receipt.get("status"),
            "http_status": receipt.get("http_status"),
            "response_body_sha256": receipt.get("response_body_sha256"),
            "response_body_bytes": receipt.get("response_body_bytes"),
        }
        if receipt != expected or type(receipt["complete"]) is not bool:
            raise ResourceEvidenceError("receipt-binding-mismatch")
        body = _read_private(slot / "response.bin", max_bytes=receipts.MAX_BODY_BYTES)
        if len(body) != receipt["response_body_bytes"] or _sha(body) != receipt["response_body_sha256"]:
            raise ResourceEvidenceError("receipt-body-mismatch")
        return receipt, body
    except ResourceEvidenceError:
        raise
    except Exception as exc:
        raise ResourceEvidenceError("receipt-verification-failed") from exc


def _capture_observations(
    *,
    capture_result,
    expected_inventory_sha256: str,
    expected_manifest_sha256: str,
    protocol_sha256: str,
    cohorts_sha256: str,
    source_revision: str,
    candidate_identity_bytes: bytes,
    candidate_identity_sha256: str,
    candidate_endpoint_sha256: str,
    timeout_limit_seconds: int | float,
    response_bytes_limit: int,
    producer_source_sha256: str | None = None,
) -> dict[str, object]:
    if producer_source_sha256 is not None and (
        type(producer_source_sha256) is not str or not _SHA.fullmatch(producer_source_sha256)
    ):
        raise ResourceEvidenceError("capture-producer-source-pin-invalid")
    root = Path(capture_result.receipt_directory)
    _require_private_directory(root)
    try:
        inventory_bytes = _read_private(root / "inventory.json", max_bytes=4_000_000)
        manifest_bytes = _read_private(root / "source-manifest.json", max_bytes=2_000_000)
    except Exception as exc:
        raise ResourceEvidenceError("capture-artifact-unreadable") from exc
    if _sha(inventory_bytes) != expected_inventory_sha256 or _sha(manifest_bytes) != expected_manifest_sha256:
        raise ResourceEvidenceError("capture-artifact-pin-mismatch")
    inventory = _strict_json(inventory_bytes, "capture-inventory")
    manifest = _strict_json(manifest_bytes, "capture-manifest")
    if type(inventory) is not dict or type(manifest) is not dict:
        raise ResourceEvidenceError("capture-artifact-shape")
    if (
        source_capture._canonical_json(inventory) != inventory_bytes
        or source_capture._canonical_json(manifest) != manifest_bytes
    ):
        raise ResourceEvidenceError("capture-artifact-not-canonical")
    try:
        checked_manifest, checked_sources = source_capture._validate_manifest(manifest_bytes, protocol_sha256)
        checked_identity = source_capture._validate_candidate_identity(
            candidate_identity_bytes, candidate_identity_sha256
        )
    except Exception as exc:
        raise ResourceEvidenceError("capture-manifest-or-identity-invalid") from exc
    if checked_manifest != manifest or checked_sources != manifest.get("sources"):
        raise ResourceEvidenceError("capture-manifest-parse-mismatch")
    expected_identity = {"runtime": checked_identity.get("runtime")}
    if (
        _sha(candidate_identity_bytes) != candidate_identity_sha256
        or manifest.get("stage_uuid") != capture_result.stage_uuid
        or manifest.get("protocol_sha256") != protocol_sha256
        or manifest.get("cohorts_sha256") != cohorts_sha256
        or manifest.get("source_revision") != source_revision
        or manifest.get("candidate_identity_sha256") != candidate_identity_sha256
        or manifest.get("candidate_endpoint_sha256") != candidate_endpoint_sha256
        or inventory.get("stage_uuid") != capture_result.stage_uuid
        or inventory.get("source_revision") != source_revision
        or inventory.get("protocol_sha256") != protocol_sha256
        or inventory.get("cohorts_sha256") != cohorts_sha256
        or inventory.get("candidate_identity_sha256") != candidate_identity_sha256
    ):
        raise ResourceEvidenceError("capture-stage-binding-mismatch")
    source_rows = manifest.get("sources")
    rows = inventory.get("sources")
    if type(source_rows) is not list or type(rows) is not list or len(rows) != len(source_rows):
        raise ResourceEvidenceError("capture-source-inventory-incomplete")
    if rows != list(capture_result.private_inventory) or len(rows) != capture_result.source_count:
        raise ResourceEvidenceError("capture-source-result-mismatch")
    contexts = []
    response_sizes = []
    archived_sizes = []
    observed_bytes_unknown = False
    attempted_urls = set()
    scrape_attempts = 0
    for source_index, (source, row) in enumerate(zip(source_rows, rows, strict=True), start=1):
        if type(row) is not dict or any(
            row.get(k) != source.get(k) for k in ("task_id", "source_id", "result_index", "url")
        ):
            raise ResourceEvidenceError("capture-source-binding-mismatch")
        attempted = row.get("attempted")
        if type(attempted) is not bool:
            raise ResourceEvidenceError("capture-attempt-state-invalid")
        if attempted:
            scrape_attempts += 1
            attempted_urls.add(coverage_source_capture_normalize(source["url"]))
            receipt_sha = row.get("receipt_sha256")
            request_body = source_capture._canonical_json(
                {"url": source["url"], "formats": ["markdown"], "onlyMainContent": True}
            )
            receipt, response_body = _receipt_any(
                root.parent,
                bindings={
                    "stage_uuid": capture_result.stage_uuid,
                    "operation_id": f"source-{source_index:04d}",
                    "request_body_sha256": _sha(request_body),
                    "source_revision": source_revision,
                },
                expected_sha256=receipt_sha,
            )
            if (
                row.get("response_sha256") != receipt["response_body_sha256"]
                or row.get("response_body_bytes") != receipt["response_body_bytes"]
            ):
                raise ResourceEvidenceError("capture-source-response-binding")
            observed_bytes = row.get("observed_response_body_bytes")
            if observed_bytes is None:
                observed_bytes_unknown = True
            elif type(observed_bytes) is not int or observed_bytes < receipt["response_body_bytes"]:
                raise ResourceEvidenceError("capture-observed-byte-count-invalid")
            elif receipt["complete"] is True and observed_bytes != receipt["response_body_bytes"]:
                raise ResourceEvidenceError("capture-complete-byte-count-mismatch")
            else:
                response_sizes.append(observed_bytes)
            archived_sizes.append(receipt["response_body_bytes"])
            encoding = row.get("response_content_encoding", "identity")
            if encoding is None:
                encoding = "identity"
            decoded_payload = None
            if row.get("status") == "captured":
                if receipt["complete"] is not True:
                    raise ResourceEvidenceError("capture-context-from-incomplete-response")
                decoded_response = _decode_capture_body(response_body, encoding, "capture-source")
                decoded_payload = _strict_json(decoded_response, "capture-source-response")
                data = decoded_payload.get("data", decoded_payload) if type(decoded_payload) is dict else None
                if (
                    type(decoded_payload) is not dict
                    or decoded_payload.get("success") is not True
                    or type(data) is not dict
                    or type(data.get("markdown")) is not str
                ):
                    raise ResourceEvidenceError("capture-source-payload-invalid")
        elif row.get("receipt_sha256") is not None or row.get("response_body_bytes") is not None:
            raise ResourceEvidenceError("capture-unattempted-source-has-response")
        if row.get("status") == "captured":
            context_name = row.get("context_artifact")
            if type(context_name) is not str or Path(context_name).name != context_name:
                raise ResourceEvidenceError("capture-context-path-invalid")
            context_doc = _strict_json(_read_private(root / context_name, max_bytes=128_000), "context")
            context = context_doc.get("context") if type(context_doc) is dict else None
            if (
                type(context) is not str
                or context_doc.get("stage") is not None
                or context_doc.get("stage_uuid") is not None
                or context_doc.get("source_id") != source.get("source_id")
                or context_doc.get("task_id") != source.get("task_id")
                or context_doc.get("result_index") != source.get("result_index")
                or context_doc.get("source_url") != source.get("url")
                or source_capture._canonical_json(context_doc) != _read_private(root / context_name, max_bytes=128_000)
                or _sha(context.encode("utf-8")) != row.get("context_sha256")
                or len(context) != row.get("context_characters")
            ):
                raise ResourceEvidenceError("capture-context-binding-mismatch")
            if attempted and decoded_payload is not None:
                decoded_markdown = data["markdown"]
                if context != decoded_markdown[: source_capture.MAX_SOURCE_CHARS]:
                    raise ResourceEvidenceError("capture-context-response-mismatch")
            contexts.append(len(context))
    state = {
        key: inventory.get(key)
        for key in (
            "owned_http_calls",
            "health_calls",
            "scrape_calls",
            "health_status",
            "health_receipt_sha256",
            "health_response_body_bytes",
            "health_observed_response_body_bytes",
            "health_response_content_encoding",
            "internal_scraper_fanout",
            "status",
        )
    }
    for key in ("owned_http_calls", "health_calls", "scrape_calls"):
        if type(state[key]) is not int or state[key] < 0:
            raise ResourceEvidenceError("capture-call-count-invalid")
    if (
        state["scrape_calls"] != scrape_attempts
        or state["owned_http_calls"] != state["health_calls"] + state["scrape_calls"]
    ):
        raise ResourceEvidenceError("capture-call-count-mismatch")
    health_bytes = state["health_response_body_bytes"]
    health_sha = state["health_receipt_sha256"]
    if state["health_calls"] == 1:
        health_receipt, health_body = _receipt_any(
            root.parent,
            bindings={
                "stage_uuid": capture_result.stage_uuid,
                "operation_id": "health",
                "request_body_sha256": _sha(b""),
                "source_revision": source_revision,
            },
            expected_sha256=health_sha,
        )
        observed_health_bytes = state["health_observed_response_body_bytes"]
        if health_bytes != health_receipt["response_body_bytes"]:
            raise ResourceEvidenceError("capture-health-response-binding")
        if observed_health_bytes is None:
            observed_bytes_unknown = True
        elif (
            type(observed_health_bytes) is not int
            or observed_health_bytes < health_bytes
            or (health_receipt["complete"] is True and observed_health_bytes != health_bytes)
        ):
            raise ResourceEvidenceError("capture-health-observed-byte-count-invalid")
        else:
            response_sizes.append(observed_health_bytes)
        archived_sizes.append(health_receipt["response_body_bytes"])
        try:
            decoded_health_body = _decode_capture_body(
                health_body, state["health_response_content_encoding"], "capture-health"
            )
            health = _strict_json(decoded_health_body, "health")
        except ResourceEvidenceError:
            health = None
        state["health_runtime_match"] = bool(
            type(health) is dict
            and health.get("status") == "ok"
            and health.get("runtime") == expected_identity["runtime"]
        )
    else:
        state["health_runtime_match"] = None

    control_attestation = inventory.get("execution_control_attestation")
    protected_qualification = inventory.get("protected_capture_qualification")
    owned_capture_transport = protected_qualification is not None
    if owned_capture_transport and control_attestation is None:
        raise ResourceEvidenceError("capture-qualification-without-control-attestation")
    capture_timeout_seconds = None
    capture_application_retries = None
    capture_response_limit_applied = None
    capture_concurrency_observed = None
    if control_attestation is not None:
        expected_control_bindings = {
            "schema": "coverage-capture-execution-controls/1",
            "stage_uuid": capture_result.stage_uuid,
            "source_revision": source_revision,
            "protocol_sha256": protocol_sha256,
            "cohorts_sha256": cohorts_sha256,
            "operation_plan_sha256": expected_manifest_sha256,
            "producer_module_sha256": (
                producer_source_sha256 or execution_controls.module_source_sha256(source_capture.__file__)
            ),
            "execution_controls_module_sha256": execution_controls.module_source_sha256(execution_controls.__file__),
            "timeout_limit_seconds_configured": timeout_limit_seconds,
            "response_bytes_limit_applied": response_bytes_limit,
            "application_retry_policy": "one-dispatch-per-operation",
            "transport_retry_policy": (
                "httpx-owned-retries-zero" if owned_capture_transport else "unknown-injected-transport"
            ),
        }
        transport_configuration = (
            {
                "trust_env": False,
                "retries": 0,
                "max_connections": 1,
                "max_keepalive_connections": 0,
                "tls_verification": "pinned-public-ca"
                if capture_result.ca_bundle_sha256 is not None
                else "system-default",
                "ca_bundle_sha256": capture_result.ca_bundle_sha256,
            }
            if owned_capture_transport
            else None
        )
        legacy_control_keys = {
            *expected_control_bindings,
            "max_concurrent_requests_observed",
            "operations",
        }
        control_keys = {
            *expected_control_bindings,
            "transport_configuration",
            "max_concurrent_requests_observed",
            "operations",
        }
        if (
            type(control_attestation) is not dict
            or frozenset(control_attestation) not in {frozenset(control_keys), frozenset(legacy_control_keys)}
            or (owned_capture_transport and set(control_attestation) != control_keys)
            or any(control_attestation.get(key) != value for key, value in expected_control_bindings.items())
            or (
                set(control_attestation) == control_keys
                and control_attestation.get("transport_configuration") != transport_configuration
            )
        ):
            raise ResourceEvidenceError("capture-control-attestation-binding")
        if owned_capture_transport:
            expected_qualification_bindings = {
                "stage_uuid": capture_result.stage_uuid,
                "source_revision": source_revision,
                "protocol_sha256": protocol_sha256,
                "candidate_identity_sha256": candidate_identity_sha256,
                "candidate_endpoint_sha256": candidate_endpoint_sha256,
                "candidate_endpoint_scheme": capture_result.candidate_endpoint_scheme,
                "candidate_runtime_revision": expected_identity["runtime"]["revision"],
                "capture_module_sha256": (
                    producer_source_sha256 or execution_controls.module_source_sha256(source_capture.__file__)
                ),
                "ca_bundle_sha256": capture_result.ca_bundle_sha256,
            }
            qualification_bindings = (
                protected_qualification.get("bindings") if type(protected_qualification) is dict else None
            )
            # Qualification pins the application endpoint, but does not itself
            # protect bearer credentials on the network hop. All owned capture
            # modes therefore require an HTTPS endpoint across protocol versions.
            allowed_endpoint_schemes = {"https"}
            if (
                type(qualification_bindings) is not dict
                or set(qualification_bindings) != {*expected_qualification_bindings, "candidate_endpoint_scheme"}
                or type(qualification_bindings.get("candidate_endpoint_scheme")) is not str
                or qualification_bindings.get("candidate_endpoint_scheme") not in allowed_endpoint_schemes
                or type(capture_result.candidate_endpoint_scheme) is not str
                or capture_result.candidate_endpoint_scheme not in allowed_endpoint_schemes
                or any(
                    qualification_bindings.get(key) != value for key, value in expected_qualification_bindings.items()
                )
            ):
                raise ResourceEvidenceError("capture-qualification-binding")
            if (
                type(protected_qualification) is not dict
                or set(protected_qualification)
                != {
                    "status",
                    "scope",
                    "receipt_file",
                    "receipt_sha256",
                    "grok_image_digest",
                    "grok_config_sha256",
                    "protected_profile_sha256",
                    "full_boundary_evidence_sha256",
                    "bindings",
                }
                or protected_qualification.get("status") != "verified-qualified"
                or protected_qualification.get("scope") != "protected-source-capture"
                or protected_qualification.get("receipt_file") != "protected-capture-qualification.json"
                or not re.fullmatch(r"sha256:[0-9a-f]{64}", str(protected_qualification.get("grok_image_digest", "")))
                or not _SHA.fullmatch(str(protected_qualification.get("grok_config_sha256", "")))
                or not _SHA.fullmatch(str(protected_qualification.get("protected_profile_sha256", "")))
                or not _SHA.fullmatch(str(protected_qualification.get("full_boundary_evidence_sha256", "")))
                or not _SHA.fullmatch(str(protected_qualification.get("receipt_sha256", "")))
            ):
                raise ResourceEvidenceError("capture-qualification-binding")
            qualification_bytes = _read_private(root / protected_qualification["receipt_file"], max_bytes=65_536)
            if _sha(qualification_bytes) != protected_qualification["receipt_sha256"]:
                raise ResourceEvidenceError("capture-qualification-receipt-mismatch")
        elif inventory.get("protected_capture_qualification") is not None:
            raise ResourceEvidenceError("capture-qualification-shape")
        operation_controls = control_attestation.get("operations")
        expected_operation_ids = ["health", *(f"source-{index:04d}" for index in range(1, len(rows) + 1))]
        if (
            type(operation_controls) is not list
            or [row.get("operation_id") if type(row) is dict else None for row in operation_controls]
            != expected_operation_ids
        ):
            raise ResourceEvidenceError("capture-control-operation-inventory")
        timeout_values = []
        dispatched_operations = 0
        expected_capture_dispatches = state["health_calls"] + state["scrape_calls"]
        for control_index, control_row in enumerate(operation_controls):
            if type(control_row) is not dict or set(control_row) != {
                "operation_id",
                "timeout_seconds_applied",
                "transport_dispatch_count",
                "response_bytes_limit_applied",
            }:
                raise ResourceEvidenceError("capture-control-operation-shape")
            dispatch_count = control_row["transport_dispatch_count"]
            timeout_value = control_row["timeout_seconds_applied"]
            response_limit = control_row["response_bytes_limit_applied"]
            expected_attempted = (
                state["health_calls"] == 1 if control_index == 0 else rows[control_index - 1].get("attempted") is True
            )
            if dispatch_count is None:
                if expected_attempted or timeout_value is not None or response_limit is not None:
                    raise ResourceEvidenceError("capture-control-uninvoked-operation")
                continue
            if type(dispatch_count) is not int or dispatch_count not in {0, 1}:
                raise ResourceEvidenceError("capture-control-dispatch-count")
            if dispatch_count == 0:
                if expected_attempted or timeout_value is not None or response_limit is not None:
                    raise ResourceEvidenceError("capture-control-undispatched-operation")
                continue
            if not expected_attempted:
                raise ResourceEvidenceError("capture-control-attempt-binding")
            if (
                type(timeout_value) not in {int, float}
                or isinstance(timeout_value, bool)
                or not math.isfinite(float(timeout_value))
                or not 0 < float(timeout_value) <= float(timeout_limit_seconds)
                or response_limit != response_bytes_limit
            ):
                raise ResourceEvidenceError("capture-control-applied-bound-invalid")
            timeout_values.append(float(timeout_value))
            dispatched_operations += 1
        max_concurrency = control_attestation.get("max_concurrent_requests_observed")
        if (
            type(max_concurrency) is not int
            or max_concurrency < 0
            or (dispatched_operations and max_concurrency != 1)
            or (not dispatched_operations and max_concurrency not in {0, 1})
            or dispatched_operations != expected_capture_dispatches
        ):
            raise ResourceEvidenceError("capture-control-observation-invalid")
        if timeout_values and len(set(timeout_values)) == 1:
            capture_timeout_seconds = timeout_values[0]
        if dispatched_operations:
            capture_application_retries = 0
            capture_response_limit_applied = response_bytes_limit
            capture_concurrency_observed = max_concurrency
    observed = {
        "capture_owned_http_calls": state["owned_http_calls"],
        "capture_health_calls": state["health_calls"],
        "capture_scrape_calls": state["scrape_calls"],
        "capture_attempts": scrape_attempts,
        "capture_attempted_unique_public_urls": len(attempted_urls),
        "capture_max_context_characters_observed": max(contexts) if contexts else None,
        "capture_max_context_characters": max(contexts) if contexts else None,
        "capture_total_context_characters": sum(contexts),
        "capture_max_response_body_bytes_observed": (
            max(response_sizes) if response_sizes and not observed_bytes_unknown else None
        ),
        "capture_max_response_bytes": max(response_sizes) if response_sizes and not observed_bytes_unknown else None,
        "capture_total_response_body_bytes": sum(response_sizes) if not observed_bytes_unknown else None,
        "capture_total_archived_response_body_bytes": sum(archived_sizes),
        "capture_unique_public_urls": len(attempted_urls),
        "capture_internal_fanout": None,
        "capture_health_runtime_match_observed": state["health_runtime_match"],
        "capture_status": state["status"],
        "capture_timeout_seconds": capture_timeout_seconds,
        # Only the source-owned transport has an independently source-bound
        # retries=0 constructor. Mock/injected transport retry behavior remains
        # unknown and is not collapsed to zero.
        "capture_retries": 0 if owned_capture_transport else None,
        "capture_application_retries_observed": capture_application_retries,
        "capture_response_bytes_limit_applied": capture_response_limit_applied,
        "capture_concurrency_observed": capture_concurrency_observed,
        "capture_tls_verification": (
            "pinned-public-ca"
            if owned_capture_transport and capture_result.ca_bundle_sha256 is not None
            else "system-default"
            if owned_capture_transport
            else None
        ),
        "capture_ca_bundle_sha256": capture_result.ca_bundle_sha256 if owned_capture_transport else None,
    }
    return observed


def _selector_observations(
    *,
    stage_uuid: str,
    source_revision: str,
    protocol_sha256: str,
    source_closure_sha256: str,
    expected_terminal_sha256: str | None,
    terminal_inventory_bytes: bytes | None,
    result_root: Path | None,
    archive_root: Path | None,
    expected_operation_ids: tuple[str, ...],
    operation_rows: tuple[Mapping[str, object], ...],
    expected_registration_sha256: str | None = None,
    expected_primary_control: Mapping[str, object] | None = None,
    expected_candidate_source_sha256: str | None = None,
    expected_selector_input_map_schema: str | None = None,
    selector_input_map_bytes: bytes | None = None,
    expected_selector_input_map_sha256: str | None = None,
    expected_saved_import_map_bindings: Mapping[str, object] | None = None,
) -> dict[str, object]:
    """Consume selector measurements only from a pinned terminal/result/archive chain.

    Missing proof is represented as unknown observations. Present but altered or
    misbound proof is rejected, so caller-supplied usage rows cannot fill gaps.
    """
    unknown = {
        "terminal_inventory_complete": None,
        "selector_elapsed_ms": None,
        "stage_elapsed_seconds": None,
        "selector_usage": None,
        "selector_concurrency": None,
        "selector_retries": None,
        "selector_provider_dispatches": None,
        "selector_transaction_proof": None,
        "w0_control_source_sha256": None,
        "w0_source_revision": None,
        "w0_model": None,
        "w0_provider_configured": None,
        "w0_ranking_strategy": None,
        "w0_parser_parity_verified": None,
        "candidate_source_sha256": None,
        "atomic_fallback_verified": None,
    }
    if terminal_inventory_bytes is None:
        return unknown
    if (
        type(expected_terminal_sha256) is not str
        or not _SHA.fullmatch(expected_terminal_sha256)
        or type(terminal_inventory_bytes) is not bytes
        or _sha(terminal_inventory_bytes) != expected_terminal_sha256
    ):
        raise ResourceEvidenceError("selector-terminal-inventory-pin-mismatch")
    if result_root is None or archive_root is None:
        return unknown
    _require_private_directory(Path(result_root))
    _require_private_directory(Path(archive_root))
    try:
        sealed_terminal_bytes = _read_private(
            Path(result_root) / f"{stage_uuid}.terminal-inventory.json", max_bytes=4_000_000
        )
    except Exception as exc:
        raise ResourceEvidenceError("selector-terminal-inventory-unavailable") from exc
    if sealed_terminal_bytes != terminal_inventory_bytes or _sha(sealed_terminal_bytes) != expected_terminal_sha256:
        raise ResourceEvidenceError("selector-terminal-inventory-file-mismatch")
    terminal = _strict_json(terminal_inventory_bytes, "selector-terminal-inventory")
    # The selector producer writes canonical JSON with one trailing newline.
    canonical_terminal = (
        json.dumps(terminal, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n"
    ).encode("utf-8")
    if terminal != json.loads(canonical_terminal) or canonical_terminal != terminal_inventory_bytes:
        raise ResourceEvidenceError("selector-terminal-inventory-not-canonical")
    if type(terminal) is not dict:
        raise ResourceEvidenceError("selector-terminal-inventory-shape")
    registered_input_rows: dict[str, Mapping[str, object]] | None = None
    if selector_input_map_bytes is not None or expected_selector_input_map_sha256 is not None:
        if expected_selector_input_map_schema not in {
            None,
            "coverage-selector-input-map/2-draft",
            "coverage-selector-input-map/2-registered",
        }:
            raise ResourceEvidenceError("selector-input-map-schema-unsupported")
        if (
            type(selector_input_map_bytes) is not bytes
            or type(expected_selector_input_map_sha256) is not str
            or not _SHA.fullmatch(expected_selector_input_map_sha256)
            or _sha(selector_input_map_bytes) != expected_selector_input_map_sha256
        ):
            raise ResourceEvidenceError("selector-input-map-pin-invalid")
        input_map = _strict_json(selector_input_map_bytes, "selector-input-map")
        base_map_fields = {
            "schema",
            "status",
            "stage_uuid",
            "source_revision",
            "original_registration_sha256",
            "coverage_source_sha256",
            "production_rerank_source_sha256",
            "execution_source_sha256",
            "legacy_control_source_sha256",
            "current_service_source_sha256",
            "acquisition_snapshot_index_sha256",
            "acquisition_manifest_sha256",
            "task_input_manifest_sha256",
            "neutral_fixture_sha256",
            "builder_source_sha256",
            "operations",
        }
        expected_map_fields = set(base_map_fields)
        if expected_saved_import_map_bindings is not None:
            if (
                type(expected_saved_import_map_bindings) is not dict
                or set(expected_saved_import_map_bindings) != _SAVED_SELECTOR_MAP_FIELDS
                or expected_selector_input_map_schema != "coverage-selector-input-map/2-registered"
                or expected_saved_import_map_bindings.get("pool_input_mode") != "saved-corpus/1"
                or type(expected_saved_import_map_bindings.get("origin_stage_uuid")) is not str
                or type(expected_saved_import_map_bindings.get("origin_source_revision")) is not str
                or not _REVISION.fullmatch(expected_saved_import_map_bindings["origin_source_revision"])
            ):
                raise ResourceEvidenceError("saved-selector-input-map-expectation-invalid")
            try:
                canonical_origin_uuid = str(uuid.UUID(expected_saved_import_map_bindings["origin_stage_uuid"]))
            except (ValueError, TypeError, AttributeError) as exc:
                raise ResourceEvidenceError("saved-selector-input-map-expectation-invalid") from exc
            if (
                canonical_origin_uuid != expected_saved_import_map_bindings["origin_stage_uuid"]
                or canonical_origin_uuid == stage_uuid
            ):
                raise ResourceEvidenceError("saved-selector-input-map-expectation-invalid")
            for name in _SAVED_SELECTOR_MAP_FIELDS - {
                "pool_input_mode",
                "origin_stage_uuid",
                "origin_source_revision",
            }:
                value = expected_saved_import_map_bindings.get(name)
                if type(value) is not str or not _SHA.fullmatch(value):
                    raise ResourceEvidenceError("saved-selector-input-map-expectation-invalid")
            expected_map_fields.update(_SAVED_SELECTOR_MAP_FIELDS)
        if (
            type(input_map) is not dict
            or set(input_map) != expected_map_fields
            or (
                expected_saved_import_map_bindings is not None
                and any(input_map.get(name) != value for name, value in expected_saved_import_map_bindings.items())
            )
            or input_map.get("schema") != (expected_selector_input_map_schema or "coverage-selector-input-map/2-draft")
            or input_map.get("status")
            != (
                "complete-awaiting-external-admission"
                if expected_selector_input_map_schema == "coverage-selector-input-map/2-registered"
                else "draft-unadmitted"
            )
            or input_map.get("stage_uuid") != stage_uuid
            or input_map.get("source_revision") != source_revision
            or input_map.get("original_registration_sha256") != expected_registration_sha256
            or input_map.get("coverage_source_sha256") != expected_candidate_source_sha256
            or expected_primary_control is None
            or input_map.get("production_rerank_source_sha256") != expected_primary_control.get("rerank_source_sha256")
            or (
                json.dumps(input_map, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n"
            ).encode("utf-8")
            != selector_input_map_bytes
            or type(input_map.get("operations")) is not list
            or [row.get("operation_id") if type(row) is dict else None for row in input_map["operations"]]
            != list(expected_operation_ids)
        ):
            raise ResourceEvidenceError("selector-input-map-binding")
        for name in (
            "coverage_source_sha256",
            "production_rerank_source_sha256",
            "execution_source_sha256",
            "legacy_control_source_sha256",
            "current_service_source_sha256",
            "acquisition_snapshot_index_sha256",
            "acquisition_manifest_sha256",
            "task_input_manifest_sha256",
            "neutral_fixture_sha256",
            "builder_source_sha256",
        ):
            if type(input_map.get(name)) is not str or not _SHA.fullmatch(input_map[name]):
                raise ResourceEvidenceError("selector-input-map-material-pin-invalid")
        registered_input_rows = {row["operation_id"]: row for row in input_map["operations"]}
        for operation_id, row in registered_input_rows.items():
            is_w0 = operation_id.endswith("-w0") or operation_id == "neutral-w0-first40"
            if (
                type(row) is not dict
                or set(row) != {"operation_id", "parser_mode", "operation_input_sha256", "request_body_sha256"}
                or row.get("parser_mode") != ("original-v1" if is_w0 else "coverage")
                or type(row.get("operation_input_sha256")) is not str
                or not _SHA.fullmatch(row["operation_input_sha256"])
                or type(row.get("request_body_sha256")) is not str
                or not _SHA.fullmatch(row["request_body_sha256"])
            ):
                raise ResourceEvidenceError("selector-input-map-operation-invalid")
    expected_bindings = {
        "schema": "coverage-jev-terminal-inventory/1",
        "stage_uuid": stage_uuid,
        "source_revision": source_revision,
        "protocol_sha256": protocol_sha256,
        "source_closure_sha256": source_closure_sha256,
    }
    if any(terminal.get(key) != value for key, value in expected_bindings.items()):
        raise ResourceEvidenceError("selector-terminal-stage-binding")
    if type(expected_operation_ids) is not tuple or not expected_operation_ids:
        raise ResourceEvidenceError("selector-operation-inventory-required")
    operations = terminal.get("operations")
    expected_operations = [
        {"operation_id": operation_id, "state": "complete-success"} for operation_id in expected_operation_ids
    ]
    supplied_operations = [
        {"operation_id": row.get("operation_id"), "state": row.get("state")}
        for row in operation_rows
        if type(row) is dict
    ]
    if (
        operations != expected_operations
        or supplied_operations != expected_operations
        or len(supplied_operations) != len(operation_rows)
        or terminal.get("all_calls_complete") is not True
        or terminal.get("status") != "all-registered-calls-complete"
    ):
        raise ResourceEvidenceError("selector-terminal-operation-inventory-mismatch")
    rows = terminal.get("resource_observations")
    if type(rows) is not list or [row.get("operation_id") if type(row) is dict else None for row in rows] != list(
        expected_operation_ids
    ):
        raise ResourceEvidenceError("selector-terminal-observation-inventory-mismatch")

    usage_rows: list[dict[str, object]] = []
    transaction_proofs: list[dict[str, object]] = []
    provenance_states: list[bool] = []
    w0_models: set[str] = set()
    w0_strategies: set[str] = set()
    w0_transports: list[str] = []
    elapsed_values: list[int] = []
    concurrency_values: list[int] = []
    dispatch_total = 0
    retries_total = 0
    for index, (operation_id, observation) in enumerate(zip(expected_operation_ids, rows, strict=True), start=1):
        if type(observation) is not dict:
            raise ResourceEvidenceError("selector-terminal-observation-shape")
        if observation == {"operation_id": operation_id, "status": "not-observed"}:
            usage_rows.append({"operation_id": operation_id, "status": "unknown"})
            continue
        required_keys = {
            "operation_id",
            "status",
            "result_receipt_sha256",
            "archive_receipt_sha256",
            "elapsed_ms_through_final_receipt_fsync",
            "provider_dispatch_count",
            "request_bytes",
            "response_bytes",
            "usage_state",
            "input_tokens_observed",
            "output_tokens_observed",
            "serialized_operation_sequence",
            "max_concurrent_operations_observed",
            "serialization_scope",
        }
        if (
            set(observation) != required_keys
            or observation.get("operation_id") != operation_id
            or observation.get("status") != "observed"
        ):
            raise ResourceEvidenceError("selector-terminal-observation-fields")
        result_sha = observation.get("result_receipt_sha256")
        archive_sha = observation.get("archive_receipt_sha256")
        if (
            type(result_sha) is not str
            or not _SHA.fullmatch(result_sha)
            or type(archive_sha) is not str
            or not _SHA.fullmatch(archive_sha)
        ):
            raise ResourceEvidenceError("selector-result-or-archive-pin-invalid")
        result_path = Path(result_root) / f"{stage_uuid}.{operation_id}.result.json"
        try:
            result_bytes = _read_private(result_path, max_bytes=65_536)
        except Exception as exc:
            raise ResourceEvidenceError("selector-result-receipt-unavailable") from exc
        if _sha(result_bytes) != result_sha:
            raise ResourceEvidenceError("selector-result-receipt-pin-mismatch")
        result = _strict_json(result_bytes, "selector-result-receipt")
        canonical_result = (
            json.dumps(result, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n"
        ).encode("utf-8")
        if type(result) is not dict or canonical_result != result_bytes:
            raise ResourceEvidenceError("selector-result-receipt-not-canonical")
        result_bindings = {
            "schema": "coverage-jev-operation-result/1",
            "stage_uuid": stage_uuid,
            "operation_id": operation_id,
            "source_revision": source_revision,
            "protocol_sha256": protocol_sha256,
            "source_closure_sha256": source_closure_sha256,
        }
        if any(result.get(key) != value for key, value in result_bindings.items()):
            raise ResourceEvidenceError("selector-result-stage-binding")
        if (
            result.get("archive_receipt_sha256") != archive_sha
            or result.get("parser_mode") not in {"coverage", "original-v1"}
            or result.get("parser_mode") != ("coverage" if "candidate" in operation_id else "original-v1")
            or (
                result.get("parser_mode") == "coverage"
                and (
                    result.get("status") not in {"selected", "fallback"}
                    or result.get("ranking_status") != result.get("status")
                )
            )
            or (result.get("parser_mode") == "original-v1" and result.get("status") != "complete-original-v1")
            or result.get("timed_out") is not False
            or result.get("terminal_reason") is not None
            or result.get("provider_dispatch_count") != observation.get("provider_dispatch_count")
            or result.get("request_bytes") != observation.get("request_bytes")
            or result.get("response_bytes") != observation.get("response_bytes")
            or result.get("input_tokens_observed") != observation.get("input_tokens_observed")
            or result.get("output_tokens_observed") != observation.get("output_tokens_observed")
            or result.get("serialized_operation_sequence") != observation.get("serialized_operation_sequence")
            or result.get("max_concurrent_operations_observed") != observation.get("max_concurrent_operations_observed")
        ):
            raise ResourceEvidenceError("selector-result-observation-binding")
        provenance = result.get("execution_provenance")
        if registered_input_rows is not None and type(provenance) is not dict:
            raise ResourceEvidenceError("selector-execution-provenance-required")
        provenance_states.append(provenance is not None)
        elapsed = observation.get("elapsed_ms_through_final_receipt_fsync")
        dispatches = observation.get("provider_dispatch_count")
        request_bytes = observation.get("request_bytes")
        response_bytes = observation.get("response_bytes")
        sequence = observation.get("serialized_operation_sequence")
        concurrency = observation.get("max_concurrent_operations_observed")
        input_tokens = observation.get("input_tokens_observed")
        output_tokens = observation.get("output_tokens_observed")
        usage_state = observation.get("usage_state")
        if (
            type(elapsed) is not int
            or elapsed < 0
            or type(dispatches) is not int
            or dispatches != 1
            or type(request_bytes) is not int
            or request_bytes <= 0
            or type(response_bytes) is not int
            or response_bytes < 0
            or type(sequence) is not int
            or sequence != index
            or type(concurrency) is not int
            or concurrency != 1
            or observation.get("serialization_scope") != "same-study-ledger-lock"
            or (input_tokens is None) != (output_tokens is None)
            or usage_state != ("known" if input_tokens is not None else "unknown-or-not-parsed")
            or (input_tokens is not None and (type(input_tokens) is not int or input_tokens < 0))
            or (output_tokens is not None and (type(output_tokens) is not int or output_tokens < 0))
            or result.get("usage_state") != usage_state
        ):
            raise ResourceEvidenceError("selector-resource-observation-invalid")
        receipt, response = _receipt_any(
            Path(archive_root),
            bindings={
                "stage_uuid": stage_uuid,
                "operation_id": operation_id,
                "request_body_sha256": result.get("request_body_sha256"),
                "source_revision": source_revision,
            },
            expected_sha256=archive_sha,
        )
        if (
            receipt.get("complete") is not True
            or receipt.get("status") != "complete"
            or type(receipt.get("http_status")) is not int
            or not 200 <= receipt["http_status"] < 300
            or len(response) != response_bytes
            or _sha(response) != result.get("response_sha256")
        ):
            raise ResourceEvidenceError("selector-archive-result-binding")
        if provenance is not None:
            required_provenance_keys = {
                "schema",
                "registration_sha256",
                "stage_uuid",
                "source_revision",
                "operation_id",
                "operation_input_sha256",
                "request_body_sha256",
                "parser_mode",
                "source_sha256",
                "requested_model",
                "transport_kind",
                "ranking_strategy",
            }
            if registered_input_rows is not None:
                required_provenance_keys.add("selector_input_map_sha256")
            is_w0 = operation_id.endswith("-w0") or operation_id == "neutral-w0-first40"
            expected_parser = "original-v1" if is_w0 else "coverage"
            expected_source = (
                expected_primary_control.get("rerank_source_sha256")
                if is_w0 and expected_primary_control is not None
                else expected_candidate_source_sha256
            )
            if (
                expected_registration_sha256 is None
                or expected_primary_control is None
                or expected_candidate_source_sha256 is None
                or type(provenance) is not dict
                or set(provenance) != required_provenance_keys
                or provenance.get("schema") != "coverage-selector-execution-provenance/1"
                or provenance.get("registration_sha256") != expected_registration_sha256
                or provenance.get("stage_uuid") != stage_uuid
                or provenance.get("source_revision") != source_revision
                or provenance.get("operation_id") != operation_id
                or provenance.get("operation_input_sha256") != result.get("operation_input_sha256")
                or type(provenance.get("operation_input_sha256")) is not str
                or not _SHA.fullmatch(provenance["operation_input_sha256"])
                or provenance.get("request_body_sha256") != result.get("request_body_sha256")
                or provenance.get("request_body_sha256") != receipt.get("request_body_sha256")
                or (
                    registered_input_rows is not None
                    and provenance.get("selector_input_map_sha256") != expected_selector_input_map_sha256
                )
                or (
                    registered_input_rows is not None
                    and (
                        provenance.get("operation_input_sha256")
                        != registered_input_rows[operation_id].get("operation_input_sha256")
                        or provenance.get("request_body_sha256")
                        != registered_input_rows[operation_id].get("request_body_sha256")
                    )
                )
                or provenance.get("parser_mode") != expected_parser
                or provenance.get("parser_mode") != result.get("parser_mode")
                or provenance.get("source_sha256") != expected_source
                or type(provenance.get("requested_model")) is not str
                or provenance.get("transport_kind") not in {"default-httpx", "injected"}
                or (
                    is_w0
                    and (
                        provenance.get("ranking_strategy") != expected_primary_control.get("ranking_strategy")
                        or provenance.get("requested_model") != expected_primary_control.get("model")
                    )
                )
                or (not is_w0 and provenance.get("ranking_strategy") is not None)
            ):
                raise ResourceEvidenceError("selector-execution-provenance-binding")
            transaction_proofs.append(
                {
                    "operation_id": operation_id,
                    "parser_mode": expected_parser,
                    "source_sha256": expected_source,
                    "operation_input_sha256": provenance["operation_input_sha256"],
                    "request_body_sha256": provenance["request_body_sha256"],
                    "requested_model": provenance["requested_model"],
                    "archive_receipt_sha256": archive_sha,
                    "result_receipt_sha256": result_sha,
                    "transport_kind": provenance["transport_kind"],
                    "ranking_strategy": provenance["ranking_strategy"],
                }
            )
            if is_w0:
                w0_models.add(provenance["requested_model"])
                w0_strategies.add(provenance["ranking_strategy"])
                w0_transports.append(provenance["transport_kind"])
        # Token counts are measurements from the archived provider body. The
        # terminal and result receipts duplicate these fields for auditability,
        # but neither duplicate is an independent source of truth.
        if result.get("parser_mode") == "coverage":
            from scripts import coverage_jev_execution as jev_execution

            body_usage = jev_execution._observed_usage(response)
        else:
            from scripts import coverage_legacy_control

            body_usage_state, body_input_tokens, body_output_tokens = coverage_legacy_control._usage_observation(
                response
            )
            body_usage = (body_input_tokens, body_output_tokens) if body_usage_state == "known" else None
        claimed_usage = (input_tokens, output_tokens) if usage_state == "known" else None
        if body_usage != claimed_usage:
            raise ResourceEvidenceError("selector-usage-archive-mismatch")
        dispatch_total += dispatches
        retries_total += dispatches - 1
        elapsed_values.append(elapsed)
        concurrency_values.append(concurrency)
        if usage_state == "known":
            usage_rows.append(
                {
                    "operation_id": operation_id,
                    "status": "reported",
                    "input_tokens": input_tokens,
                    "output_tokens": output_tokens,
                }
            )
        else:
            usage_rows.append({"operation_id": operation_id, "status": "unknown"})

    all_observed = len(elapsed_values) == len(expected_operation_ids)
    if any(provenance_states) and not all(provenance_states):
        raise ResourceEvidenceError("selector-execution-provenance-incomplete")
    output: dict[str, object] = {
        "terminal_inventory_complete": True if all_observed else None,
        "selector_elapsed_ms": max(elapsed_values) if all_observed else None,
        # The stage-wide timer is measured by the coordinator after its final fsync.
        "stage_elapsed_seconds": None,
        "selector_usage": usage_rows if all_observed else None,
        "selector_concurrency": max(concurrency_values) if all_observed else None,
        # Dispatches are observed at the HTTP transport, one expected per slot.
        "selector_retries": retries_total if all_observed else None,
        "selector_provider_dispatches": dispatch_total if all_observed else None,
        "selector_transaction_proof": None,
        "w0_control_source_sha256": None,
        "w0_source_revision": None,
        "w0_model": None,
        "w0_provider_configured": None,
        "w0_ranking_strategy": None,
        "w0_parser_parity_verified": None,
        "candidate_source_sha256": None,
        "atomic_fallback_verified": None,
    }
    if (
        all_observed
        and len(transaction_proofs) == len(expected_operation_ids)
        and registered_input_rows is not None
        and type(expected_registration_sha256) is str
        and _SHA.fullmatch(expected_registration_sha256)
        and expected_primary_control is not None
        and expected_candidate_source_sha256 is not None
        and len(w0_models) == 1
        and len(w0_strategies) == 1
        and next(iter(w0_models)) == expected_primary_control.get("model")
        and next(iter(w0_strategies)) == expected_primary_control.get("ranking_strategy")
    ):
        output.update(
            {
                "selector_transaction_proof": {
                    "schema": "coverage-selector-source-archive-registry-proof/1",
                    "registration_sha256": expected_registration_sha256,
                    "input_map_sha256": expected_selector_input_map_sha256,
                    "stage_uuid": stage_uuid,
                    "source_revision": source_revision,
                    "transactions": transaction_proofs,
                    "parser_parity_verified": None,
                    "atomic_fallback_verified": None,
                },
                "w0_control_source_sha256": expected_primary_control["rerank_source_sha256"],
                "w0_source_revision": source_revision,
                "w0_model": next(iter(w0_models)),
                "w0_provider_configured": (True if all(kind == "default-httpx" for kind in w0_transports) else None),
                "w0_ranking_strategy": next(iter(w0_strategies)),
                "candidate_source_sha256": expected_candidate_source_sha256,
            }
        )
    return output


def coverage_source_capture_normalize(url: str) -> str:
    from slopsearx.merger import _normalise_url

    return _normalise_url(url)


def _conservative_rounded_gap_lower_bound_us(earlier_us: int, later_us: int) -> int:
    """Lower-bound a gap computed from independently rounded microsecond offsets.

    Two nearest-microsecond samples can overstate their interval by almost one
    microsecond. Remove that full quantization interval before comparison with
    a frozen pacing threshold. An exact-looking boundary is therefore rejected
    when the underlying sub-microsecond interval cannot be distinguished.
    """
    return later_us - earlier_us - 1


def collect_acquisition_observations(
    *,
    snapshots_directory: Path,
    receipt_directory: Path,
    expected_receipt_inventory_sha256: str,
    acquisition_manifest_bytes: bytes,
    acquisition_plan_bytes: bytes,
    expected_index_sha256: str,
    expected_manifest_sha256: str,
    stage_uuid: str,
    source_revision: str,
    protocol_sha256: str,
    cohorts_sha256: str,
    source_closure_sha256: str,
    acquisition_plan_sha256: str,
    configured_timeout_seconds: int | float,
    expected_execution_profile: str = offline_acquisition.LEGACY_ACQUISITION_PROFILE.identifier,
    configured_query_pacing_seconds: int | float | None = None,
    configured_arxiv_pacing_seconds: int | float | None = None,
    deadline_monotonic: float | None = None,
    verified_pool_source: object | None = None,
    verified_historical_operation_runner_module: object | None = None,
    verified_saved_origin: object | None = None,
) -> dict[str, object]:
    if _sha(acquisition_manifest_bytes) != expected_manifest_sha256:
        raise ResourceEvidenceError("acquisition-manifest-pin-mismatch")
    manifest = _strict_json(acquisition_manifest_bytes, "acquisition-manifest")
    try:
        profile = offline_acquisition.execution_profile_from_manifest(manifest)
    except Exception as exc:
        raise ResourceEvidenceError("acquisition-profile-invalid") from exc
    if float(configured_timeout_seconds) != profile.request_timeout_seconds:
        raise ResourceEvidenceError("acquisition-profile-timeout-mismatch")
    if profile.identifier != expected_execution_profile:
        raise ResourceEvidenceError("acquisition-profile-protocol-mismatch")
    frozen_mode = verified_pool_source is not None
    operation_runner_module_sha256 = execution_controls.module_source_sha256(offline_acquisition.__file__)
    if verified_historical_operation_runner_module is not None or verified_saved_origin is not None:
        from scripts import coverage_saved_corpus
        from scripts.coverage_frozen_pool_inputs import is_verified_pool_snapshot_source

        historical = verified_historical_operation_runner_module
        if (
            not frozen_mode
            or not is_verified_pool_snapshot_source(verified_pool_source)
            or type(verified_saved_origin) is not coverage_saved_corpus.SavedCorpusOrigin
            or type(historical) is not coverage_saved_corpus.VerifiedHistoricalOperationRunnerModule
            or verified_pool_source.pool_index.get("stage_uuid") != verified_saved_origin.registration.stage_uuid
            or verified_pool_source.pool_index.get("source_revision")
            != verified_saved_origin.registration.source_revision
            or verified_pool_source.pool_index.get("source_closure_sha256")
            != verified_saved_origin.registration.source_closure_sha256
            or verified_pool_source.snapshot_index_sha256 is None
            or verified_saved_origin.registration.stage_uuid != stage_uuid
            or verified_saved_origin.registration.source_revision != source_revision
            or coverage_saved_corpus.package_sha256(
                Path(verified_saved_origin.collection_directory), Path(verified_saved_origin.capture_stage_directory)
            )
            != verified_saved_origin.expected_package_sha256
            or historical != coverage_saved_corpus.verify_historical_operation_runner_module(verified_saved_origin)
            or historical.origin_package_sha256 != verified_saved_origin.expected_package_sha256
            or historical.origin_stage_uuid != stage_uuid
            or historical.origin_source_revision != source_revision
            or type(historical.module_bytes) is not bytes
            or _sha(historical.module_bytes) != historical.module_sha256
        ):
            raise ResourceEvidenceError("historical-acquisition-source-proof-invalid")
        operation_runner_module_sha256 = historical.module_sha256
    if frozen_mode:
        from scripts.coverage_frozen_pool_inputs import is_verified_pool_snapshot_source

        if not is_verified_pool_snapshot_source(verified_pool_source) or (
            verified_pool_source.snapshots_directory != Path(snapshots_directory)
            or verified_pool_source.snapshot_index_sha256 != expected_index_sha256
            or verified_pool_source.pool_manifest_sha256 != expected_manifest_sha256
        ):
            raise ResourceEvidenceError("frozen-pool-snapshot-source-invalid")
        verified = {"index": verified_pool_source.pool_index, "operations": verified_pool_source.operations}
    else:
        try:
            verified = coverage_live_acquire.verify_pool_snapshot_index(
                snapshots_directory,
                expected_index_sha256=expected_index_sha256,
                expected_stage_manifest_sha256=expected_manifest_sha256,
                expected_stage_uuid=stage_uuid,
                expected_source_revision=source_revision,
                expected_execution_profile=profile.identifier,
                deadline_monotonic=deadline_monotonic,
            )
        except Exception as exc:
            raise ResourceEvidenceError("acquisition-snapshot-verification-failed") from exc
    index = verified["index"]
    expected = {
        "protocol_sha256": protocol_sha256,
        "cohorts_sha256": cohorts_sha256,
        "source_closure_sha256": source_closure_sha256,
        "acquisition_plan_sha256": acquisition_plan_sha256,
    }
    if any(index.get(key) != value for key, value in expected.items()):
        raise ResourceEvidenceError("acquisition-stage-binding-mismatch")
    try:
        checked_manifest, _raw, _ids, _reserved, _limits = coverage_live_acquire._validate_manifest(
            manifest, acquisition_plan_bytes
        )
    except Exception as exc:
        raise ResourceEvidenceError("acquisition-manifest-invalid") from exc
    if checked_manifest != manifest:
        raise ResourceEvidenceError("acquisition-manifest-parse-mismatch")
    receipt_root = Path(receipt_directory)
    try:
        inventory = _read_pinned_receipt_directory(receipt_root, expected_receipt_inventory_sha256)
    except Exception as exc:
        raise ResourceEvidenceError("acquisition-receipt-inventory-invalid") from exc
    if frozen_mode:
        component_names = {
            "pool_snapshot_index_sha256": "frozen-pool-index.json",
            "availability_sha256": "engine-availability.json",
            "admission_sha256": "admission.json",
            "transport_control_sha256": "transport-control.json",
            "request_ledger_index_sha256": "request-ledger-index.json",
            "collection_summary_sha256": "collection-summary.json",
        }
        receipt_raw = _read_private(
            receipt_root / "collector-receipt.json",
            max_bytes=coverage_live_acquire.MAX_POOL_SNAPSHOT_BYTES,
        )
        if _sha(receipt_raw) != verified_pool_source.collector_receipt_sha256:
            raise ResourceEvidenceError("frozen-pool-collector-receipt-mutated")
        evidence_raw = _read_private(
            receipt_root / "frozen-pool-evidence.json",
            max_bytes=coverage_live_acquire.MAX_POOL_SNAPSHOT_BYTES,
        )
        if _sha(evidence_raw) != verified_pool_source.evidence_sha256:
            raise ResourceEvidenceError("frozen-pool-evidence-mutated")
        collector_receipt = inventory.get("collector-receipt.json")
        if type(collector_receipt) is not dict:
            raise ResourceEvidenceError("frozen-pool-collector-receipt-invalid")
        for digest_key, filename in component_names.items():
            raw = _read_private(
                receipt_root / filename,
                max_bytes=coverage_live_acquire.MAX_POOL_SNAPSHOT_BYTES,
            )
            if (
                _sha(raw) != collector_receipt.get(digest_key)
                or (digest_key == "pool_snapshot_index_sha256" and _sha(raw) != expected_index_sha256)
                or (digest_key == "availability_sha256" and _sha(raw) != verified_pool_source.availability_sha256)
            ):
                raise ResourceEvidenceError("frozen-pool-component-mutated")
    admission = inventory.get("admission.json")
    admission_bindings = {
        "stage_uuid": stage_uuid,
        "source_revision": source_revision,
        "source_closure_sha256": source_closure_sha256,
        "protocol_sha256": protocol_sha256,
        "cohorts_sha256": cohorts_sha256,
        "input_manifest_sha256": manifest.get("input_manifest_sha256"),
        "acquisition_plan_sha256": acquisition_plan_sha256,
        "stage_manifest_sha256": expected_manifest_sha256,
    }
    if frozen_mode:
        admission_bindings.update(
            {
                "schema": "coverage-frozen-collection-admission/1",
                "pool_input_mode": "frozen-pool/1",
                "candidate_identity_sha256": index.get("candidate_identity_sha256"),
                "candidate_endpoint_sha256": index.get("candidate_endpoint_sha256"),
            }
        )
    if type(admission) is not dict or any(admission.get(key) != value for key, value in admission_bindings.items()):
        raise ResourceEvidenceError("acquisition-admission-binding-mismatch")
    if frozen_mode and (
        admission.get("stage") != manifest.get("stage")
        or type(admission.get("permit_sha256")) is not str
        or not _SHA.fullmatch(admission["permit_sha256"])
        or type(admission.get("lease_sha256")) is not str
        or not _SHA.fullmatch(admission["lease_sha256"])
        or type(admission.get("stage_clock_sha256")) is not str
        or not _SHA.fullmatch(admission["stage_clock_sha256"])
        or type(admission.get("clock_origin_sha256")) is not str
        or not _SHA.fullmatch(admission["clock_origin_sha256"])
    ):
        raise ResourceEvidenceError("acquisition-admission-binding-mismatch")
    starts = {}
    finals = {}
    for name, document in inventory.items():
        if name.startswith("http-") and name.endswith("-start.json"):
            starts[name.removesuffix("-start.json")] = document
        elif name.startswith("http-") and name.endswith("-final.json"):
            finals[name.removesuffix("-final.json")] = document
    if not starts or set(starts) != set(finals):
        raise ResourceEvidenceError("acquisition-exchange-receipts-incomplete")
    expected_sequences = {f"http-{number:04d}" for number in range(1, len(starts) + 1)}
    if set(starts) != expected_sequences:
        raise ResourceEvidenceError("acquisition-exchange-sequence-gap")
    if frozen_mode:
        ledger = inventory.get("request-ledger-index.json")
        entries = ledger.get("entries") if type(ledger) is dict else None
        if (
            type(ledger) is not dict
            or set(ledger) != {"schema", "stage_uuid", "entries"}
            or ledger.get("schema") != "coverage-frozen-request-ledger-index/1"
            or ledger.get("stage_uuid") != stage_uuid
            or type(entries) is not list
            or len(entries) != len(starts)
        ):
            raise ResourceEvidenceError("acquisition-request-ledger-index-invalid")
        expected_files: set[str] = set()
        overflow_witness_seen = False
        for sequence, row in enumerate(entries, start=1):
            required = {
                "sequence",
                "operation_id",
                "engine",
                "start_file",
                "start_sha256",
                "start_bytes",
                "final_file",
                "final_sha256",
                "final_bytes",
                "raw_response_file",
                "raw_response_sha256",
                "raw_response_bytes",
                "raw_response_complete",
                "raw_response_overflow_witness",
            }
            if type(row) is not dict or set(row) != required or row.get("sequence") != sequence:
                raise ResourceEvidenceError("acquisition-request-ledger-entry-invalid")
            for prefix, suffix in (("start", "-start.json"), ("final", "-final.json")):
                name = row.get(f"{prefix}_file")
                try:
                    pinned_raw = (
                        _read_private(
                            receipt_root / name,
                            max_bytes=coverage_live_acquire.MAX_POOL_SNAPSHOT_BYTES,
                        )
                        if type(name) is str and Path(name).name == name
                        else b""
                    )
                except Exception as exc:
                    raise ResourceEvidenceError("acquisition-request-ledger-receipt-mismatch") from exc
                if (
                    type(name) is not str
                    or Path(name).name != name
                    or name != f"http-{sequence:04d}{suffix}"
                    or name not in inventory
                    or _sha(pinned_raw) != row.get(f"{prefix}_sha256")
                    or row.get(f"{prefix}_bytes") != len(pinned_raw)
                ):
                    raise ResourceEvidenceError("acquisition-request-ledger-receipt-mismatch")
                expected_files.add(name)
            raw_name = row.get("raw_response_file")
            if (
                type(raw_name) is not str
                or Path(raw_name).name != raw_name
                or raw_name != f"http-{sequence:04d}-response.bin"
                or type(row.get("raw_response_sha256")) is not str
                or not _SHA.fullmatch(row["raw_response_sha256"])
                or type(row.get("raw_response_bytes")) is not int
                or row["raw_response_bytes"] < 0
                or type(row.get("raw_response_complete")) is not bool
                or type(row.get("raw_response_overflow_witness")) is not bool
            ):
                raise ResourceEvidenceError("acquisition-request-ledger-raw-response-invalid")
            try:
                raw_bytes = _read_private(
                    receipt_root / raw_name,
                    max_bytes=coverage_live_acquire.MAX_RESPONSE_BYTES + 1,
                )
            except Exception as exc:
                raise ResourceEvidenceError("acquisition-request-ledger-raw-response-unavailable") from exc
            if _sha(raw_bytes) != row["raw_response_sha256"] or len(raw_bytes) != row["raw_response_bytes"]:
                raise ResourceEvidenceError("acquisition-request-ledger-raw-response-mismatch")
            expected_files.add(raw_name)
            start_doc = inventory[f"http-{sequence:04d}-start.json"]
            final_doc = inventory[f"http-{sequence:04d}-final.json"]
            if row.get("operation_id") != start_doc.get("operation_id") or row.get("engine") != start_doc.get("engine"):
                raise ResourceEvidenceError("acquisition-request-ledger-operation-binding")
            overflow = row["raw_response_overflow_witness"]
            if overflow:
                overflow_witness_seen = True
                if (
                    row["raw_response_complete"] is not False
                    or len(raw_bytes) != coverage_live_acquire.MAX_RESPONSE_BYTES + 1
                    or final_doc.get("failure_code") != "response_body_cap"
                    or final_doc.get("complete") is not False
                    or type(final_doc.get("response_bytes")) is not int
                    or final_doc["response_bytes"] < coverage_live_acquire.MAX_RESPONSE_BYTES + 1
                    or final_doc["response_bytes"] > coverage_live_acquire.MAX_STAGE_TRANSFER_BYTES
                    or final_doc.get("response_sha256") != _sha(raw_bytes[: coverage_live_acquire.MAX_RESPONSE_BYTES])
                ):
                    raise ResourceEvidenceError("acquisition-request-ledger-overflow-witness-invalid")
            elif (
                row["raw_response_complete"] is not True
                or len(raw_bytes) > coverage_live_acquire.MAX_RESPONSE_BYTES
                or final_doc.get("complete") is not True
                or final_doc.get("response_bytes") != row["raw_response_bytes"]
                or final_doc.get("response_sha256") != row["raw_response_sha256"]
            ):
                raise ResourceEvidenceError("acquisition-request-ledger-response-length")
        actual_files = {
            path.name
            for path in receipt_root.iterdir()
            if path.name.startswith("http-")
            and (
                path.name.endswith("-start.json")
                or path.name.endswith("-final.json")
                or path.name.endswith("-response.bin")
            )
        }
        if actual_files != expected_files:
            raise ResourceEvidenceError("acquisition-request-ledger-inventory-mismatch")
        if overflow_witness_seen:
            # The bounded +1 byte witness is authentic diagnostic evidence for
            # the hard response cap, but such a response cannot enter a valid
            # frozen corpus or earn downstream resource credit.
            raise ResourceEvidenceError("acquisition-request-ledger-overflow-ineligible")
    operation_engine_allowlist = {}
    for row in [*manifest.get("research_cases", []), *manifest.get("navigation_targets", [])]:
        op_id = row.get("task_id", row.get("target_id"))
        plan = row.get("pool_plan", row.get("navigation_pool_plan"))
        operation_engine_allowlist[op_id] = set(plan.get("engines", [])) if type(plan) is dict else set()
    if manifest.get("stage_uuid") != stage_uuid:
        raise ResourceEvidenceError("acquisition-manifest-binding-mismatch")
    counts = Counter()
    physical_calls = 0
    unknown_engine = False
    request_total = response_total = max_response = 0
    dispatched_timeouts: list[float] = []
    dispatched_receipt_offsets: list[dict[str, object]] = []
    for sequence in sorted(starts):
        start, final = starts[sequence], finals[sequence]
        if type(start) is not dict or type(final) is not dict:
            raise ResourceEvidenceError("acquisition-exchange-receipt-shape")
        common = (
            "operation_id",
            "engine",
            "method",
            "host",
            "path",
            "request_sha256",
            "request_bytes",
            "timeout_seconds",
        )
        if any(start.get(key) != final.get(key) for key in common) or start.get("status") != "attempted":
            raise ResourceEvidenceError("acquisition-exchange-request-binding")
        if start.get("sequence") != int(sequence.removeprefix("http-")):
            raise ResourceEvidenceError("acquisition-exchange-sequence-binding")
        timeout = start.get("timeout_seconds")
        configured_timeout = configured_timeout_seconds
        if (
            isinstance(timeout, bool)
            or not isinstance(timeout, (int, float))
            or isinstance(configured_timeout, bool)
            or not isinstance(configured_timeout, (int, float))
        ):
            raise ResourceEvidenceError("acquisition-exchange-timeout-invalid")
        timeout_value = float(timeout)
        configured_timeout_value = float(configured_timeout)
        if (
            not math.isfinite(timeout_value)
            or timeout_value <= 0
            or not math.isfinite(configured_timeout_value)
            or configured_timeout_value <= 0
            or timeout_value > configured_timeout_value
        ):
            raise ResourceEvidenceError("acquisition-exchange-timeout-invalid")
        operation_id, engine = start.get("operation_id"), start.get("engine")
        if type(operation_id) is not str or operation_id not in operation_engine_allowlist:
            raise ResourceEvidenceError("acquisition-exchange-operation-unknown")
        if type(engine) is not str or engine not in operation_engine_allowlist[operation_id]:
            unknown_engine = True
        elif (
            engine not in offline_acquisition.ENGINE_HOSTS
            or start.get("host") != offline_acquisition.ENGINE_HOSTS[engine]
            or start.get("path") != offline_acquisition.ENGINE_PATHS[engine]
            or start.get("method") != "GET"
        ):
            unknown_engine = True
        if final.get("dispatched") is True:
            physical_calls += 1
            # These receipts are written at the HTTPX transport boundary. The
            # value is the timeout configuration on that concrete request,
            # not its elapsed duration or the protocol ceiling itself.
            dispatched_timeouts.append(timeout_value)
            if type(engine) is str:
                counts[engine] += 1
            dispatched_receipt_offsets.append(
                {
                    "sequence": start.get("sequence"),
                    "operation_id": operation_id,
                    "engine": engine,
                    "dispatch_offset_us": final.get("dispatch_offset_us"),
                }
            )
        elif final.get("dispatched") is not False:
            unknown_engine = True
        elif final.get("dispatch_offset_us") is not None:
            raise ResourceEvidenceError("acquisition-undispatched-offset-invalid")
        request_bytes, response_bytes = start.get("request_bytes"), final.get("response_bytes")
        if type(request_bytes) is not int or request_bytes < 0 or type(response_bytes) is not int or response_bytes < 0:
            raise ResourceEvidenceError("acquisition-exchange-byte-count-invalid")
        response_sha = final.get("response_sha256")
        if response_bytes > 0 and (type(response_sha) is not str or not _SHA.fullmatch(response_sha)):
            raise ResourceEvidenceError("acquisition-exchange-response-digest-invalid")
        request_total += request_bytes if final.get("dispatched") is True else 0
        response_total += response_bytes
        max_response = max(max_response, response_bytes)
    summary = inventory.get("collection-summary.json") if frozen_mode else inventory.get("stage-summary.json")
    if summary is None:
        summary = inventory.get("stage-summary.json")
    allowed_statuses = {"complete"}
    if profile is offline_acquisition.CONTINUE_ACQUISITION_PROFILE:
        allowed_statuses.add("inconclusive")
    summary_valid = (
        type(summary) is not dict
        or summary.get("status") not in allowed_statuses
        or summary.get("physical_request_count") != physical_calls
    )
    if profile is offline_acquisition.CONTINUE_ACQUISITION_PROFILE:
        if frozen_mode:
            expected_ids = [row["task_id"] for row in manifest["research_cases"]] + [
                row["target_id"] for row in manifest["navigation_targets"]
            ]
            eligibility = summary.get("pool_eligibility")
            eligibility_valid = (
                type(eligibility) is list
                and len(eligibility) == len(expected_ids)
                and all(
                    type(row) is dict
                    and set(row) == {"operation_id", "pool_status"}
                    and row.get("operation_id") == operation_id
                    and row.get("pool_status") == "complete"
                    for row, operation_id in zip(eligibility, expected_ids, strict=True)
                )
            )
            operation_statuses = summary.get("operation_statuses")
            operation_statuses_valid = (
                type(operation_statuses) is list
                and len(operation_statuses) == len(expected_ids)
                and all(
                    type(row) is dict
                    and set(row) == {"operation_id", "status", "failure_reasons"}
                    and row.get("operation_id") == operation_id
                    and row.get("status") == index_row.get("acquisition_status")
                    for row, operation_id, index_row in zip(
                        operation_statuses, expected_ids, index.get("operations", []), strict=True
                    )
                )
            )
            summary_valid = (
                summary_valid
                or any(
                    summary.get(key) != value
                    for key, value in {
                        "schema": "coverage-frozen-collection-summary/1",
                        "stage_uuid": stage_uuid,
                        "stage_manifest_sha256": expected_manifest_sha256,
                        "pool_input_mode": "frozen-pool/1",
                        "status": "complete",
                        "stage_deadline_monotonic": deadline_monotonic,
                        "quality_credit": False,
                        "fatal_uninvoked_slots": [],
                    }.items()
                )
                or not eligibility_valid
                or not operation_statuses_valid
                or (type(summary.get("transfer_bytes")) is not int or summary["transfer_bytes"] < 0)
            )
        else:
            summary_valid = summary_valid or (
                summary.get("stage") != manifest.get("stage")
                or summary.get("schema") != "coverage-live-acquisition-summary/2"
                or summary.get("execution_profile") != profile.identifier
                or index.get("status") != ("complete" if summary.get("status") == "complete" else "sealed-inconclusive")
            )
    elif not frozen_mode:
        summary_valid = summary_valid or summary.get("stage") != manifest.get("stage")
    if summary_valid:
        raise ResourceEvidenceError("acquisition-stage-summary-binding")
    controls = inventory.get("transport-control.json") if frozen_mode else summary.get("execution_control_attestation")
    acquisition_retries = acquisition_pacing = arxiv_pacing = None
    query_min_gap_us = arxiv_min_gap_us = None
    if controls is not None:
        expected_control_bindings = {
            "schema": "coverage-acquisition-execution-controls/1",
            "stage_uuid": stage_uuid,
            "source_revision": source_revision,
            "source_closure_sha256": source_closure_sha256,
            "protocol_sha256": protocol_sha256,
            "cohorts_sha256": cohorts_sha256,
            "input_manifest_sha256": manifest.get("input_manifest_sha256"),
            "acquisition_plan_sha256": acquisition_plan_sha256,
            "stage_manifest_sha256": expected_manifest_sha256,
            "producer_module_sha256": execution_controls.module_source_sha256(coverage_live_acquire.__file__),
            "operation_runner_module_sha256": operation_runner_module_sha256,
            "control_identity": "coverage-acquisition-one-shot/1",
            "query_pacing_seconds_enforced": coverage_live_acquire.QUERY_PACING_SECONDS,
            "arxiv_physical_pacing_seconds_enforced": coverage_live_acquire.ARXIV_PHYSICAL_PACING_SECONDS,
            "offset_quantization_guard_seconds": coverage_live_acquire.OFFSET_QUANTIZATION_GUARD_SECONDS,
            "application_retry_policy": "one-search-invocation-per-planned-operation",
        }
        if profile is offline_acquisition.CONTINUE_ACQUISITION_PROFILE:
            expected_control_bindings.update(
                {
                    "execution_profile": profile.identifier,
                    "request_timeout_seconds_enforced": profile.request_timeout_seconds,
                    "logical_query_timeout_seconds_enforced": profile.logical_query_timeout_seconds,
                    "ordinary_failure_policy": "continue-planned-operations",
                }
            )
        control_keys = {
            *expected_control_bindings,
            "pacer_identity",
            "transport_identity",
            "transport_retry_policy",
            "transport_retries_configured",
            "operation_invocations",
            "physical_dispatch_offsets_us",
        }
        if (
            type(controls) is not dict
            or set(controls) != control_keys
            or any(controls.get(key) != value for key, value in expected_control_bindings.items())
        ):
            raise ResourceEvidenceError("acquisition-control-attestation-binding")
        if (
            configured_query_pacing_seconds != coverage_live_acquire.QUERY_PACING_SECONDS
            or configured_arxiv_pacing_seconds != coverage_live_acquire.ARXIV_PHYSICAL_PACING_SECONDS
        ):
            raise ResourceEvidenceError("acquisition-control-protocol-pacing-mismatch")
        pacer_identity = controls.get("pacer_identity")
        transport_identity = controls.get("transport_identity")
        retry_policy = controls.get("transport_retry_policy")
        retry_config = controls.get("transport_retries_configured")
        if pacer_identity not in {"owned-asyncio-sleep", "injected-mock-only"}:
            raise ResourceEvidenceError("acquisition-control-pacer-invalid")
        if transport_identity == "owned-httpx":
            if (
                pacer_identity != "owned-asyncio-sleep"
                or retry_policy != "configured-zero-owned-httpx-transport"
                or retry_config != 0
            ):
                raise ResourceEvidenceError("acquisition-control-transport-binding")
            production_transport = True
        elif transport_identity == "injected-mocktransport":
            if retry_policy != "mock-transport-single-dispatch" or retry_config != 0:
                raise ResourceEvidenceError("acquisition-control-transport-binding")
            # Mock receipts are useful for parser/receipt tests, but their
            # injected clock and transport do not establish real execution
            # pacing or retry behavior. Keep the qualification fields unknown.
            production_transport = False
        else:
            raise ResourceEvidenceError("acquisition-control-retry-policy-invalid")

        expected_ids = [row.get("task_id") for row in manifest["research_cases"]] + [
            row.get("target_id") for row in manifest["navigation_targets"]
        ]
        invocations = controls.get("operation_invocations")
        if type(invocations) is not list or len(invocations) != len(expected_ids):
            raise ResourceEvidenceError("acquisition-operation-invocation-inventory")
        invoked_rows: list[dict[str, object]] = []
        stopped = False
        for index, (operation_id, row) in enumerate(zip(expected_ids, invocations, strict=True), start=1):
            if type(row) is not dict or set(row) != {"operation_id", "sequence", "start_offset_us", "end_offset_us"}:
                raise ResourceEvidenceError("acquisition-operation-invocation-shape")
            if row.get("operation_id") != operation_id or row.get("sequence") != index:
                raise ResourceEvidenceError("acquisition-operation-invocation-order")
            start_offset = row.get("start_offset_us")
            end_offset = row.get("end_offset_us")
            if start_offset is None and end_offset is None:
                stopped = True
                continue
            if (
                stopped
                or type(start_offset) is not int
                or type(end_offset) is not int
                or start_offset < 0
                or end_offset < start_offset
            ):
                raise ResourceEvidenceError("acquisition-operation-invocation-time-invalid")
            invoked_rows.append(row)
        if summary.get("status") == "complete" and len(invoked_rows) != len(expected_ids):
            raise ResourceEvidenceError("acquisition-operation-invocation-incomplete")
        if (
            profile is offline_acquisition.CONTINUE_ACQUISITION_PROFILE
            and summary.get("status") == "inconclusive"
            and len(invoked_rows) != len(expected_ids)
        ):
            raise ResourceEvidenceError("acquisition-v2-planned-operation-not-invoked")
        if production_transport and len(invoked_rows) >= 2:
            raw_idle_gaps = [
                int(next_row["start_offset_us"]) - int(previous_row["end_offset_us"])
                for previous_row, next_row in zip(invoked_rows, invoked_rows[1:])
            ]
            if any(gap < 0 for gap in raw_idle_gaps):
                raise ResourceEvidenceError("acquisition-query-time-order-invalid")
            idle_gaps = [
                _conservative_rounded_gap_lower_bound_us(
                    int(previous_row["end_offset_us"]), int(next_row["start_offset_us"])
                )
                for previous_row, next_row in zip(invoked_rows, invoked_rows[1:])
            ]
            query_min_gap_us = min(idle_gaps)
            query_bound_us = int(float(configured_query_pacing_seconds) * 1_000_000)
            acquisition_pacing = (
                float(configured_query_pacing_seconds)
                if query_min_gap_us >= query_bound_us
                else query_min_gap_us / 1_000_000
            )

        dispatch_offsets = controls.get("physical_dispatch_offsets_us")
        if type(dispatch_offsets) is not list or dispatch_offsets != dispatched_receipt_offsets:
            raise ResourceEvidenceError("acquisition-dispatch-offset-receipt-mismatch")
        arxiv_offsets = []
        for row in dispatch_offsets:
            if type(row) is not dict or set(row) != {"sequence", "operation_id", "engine", "dispatch_offset_us"}:
                raise ResourceEvidenceError("acquisition-dispatch-offset-shape")
            offset = row.get("dispatch_offset_us")
            if type(offset) is not int or offset < 0:
                raise ResourceEvidenceError("acquisition-dispatch-offset-invalid")
            if row.get("engine") == "arxiv":
                arxiv_offsets.append(offset)
        if production_transport and len(arxiv_offsets) >= 2:
            raw_arxiv_gaps = [right - left for left, right in zip(arxiv_offsets, arxiv_offsets[1:])]
            if any(gap < 0 for gap in raw_arxiv_gaps):
                raise ResourceEvidenceError("acquisition-arxiv-time-order-invalid")
            arxiv_min_gap_us = min(
                _conservative_rounded_gap_lower_bound_us(left, right)
                for left, right in zip(arxiv_offsets, arxiv_offsets[1:])
            )
            arxiv_bound_us = int(float(configured_arxiv_pacing_seconds) * 1_000_000)
            arxiv_pacing = (
                float(configured_arxiv_pacing_seconds)
                if arxiv_min_gap_us >= arxiv_bound_us
                else arxiv_min_gap_us / 1_000_000
            )
        if production_transport and len(invoked_rows) == len(expected_ids) and retry_config == 0:
            acquisition_retries = 0
    result: dict[str, object] = {
        "acquisition_physical_http_calls": physical_calls,
        "acquisition_attempted_http_slots": len(starts),
        "acquisition_request_bytes_total": request_total,
        "acquisition_response_bytes_total": response_total,
        "acquisition_max_response_body_bytes_observed": max_response,
        "acquisition_timeout_seconds": (
            dispatched_timeouts[0] if dispatched_timeouts and len(set(dispatched_timeouts)) == 1 else None
        ),
        "acquisition_timeout_seconds_by_exchange": dispatched_timeouts,
        "acquisition_timeout_observation_state": (
            "observed-uniform"
            if dispatched_timeouts and len(set(dispatched_timeouts)) == 1
            else "observed-variable"
            if dispatched_timeouts
            else "no-dispatched-exchanges"
        ),
        "acquisition_retries": acquisition_retries,
        "acquisition_pacing_seconds": acquisition_pacing,
        "arxiv_pacing_seconds": arxiv_pacing,
        "acquisition_query_min_idle_gap_microseconds_observed": query_min_gap_us,
        "acquisition_arxiv_min_gap_microseconds_observed": arxiv_min_gap_us,
    }
    if not unknown_engine and set(counts) <= set(_ENGINE_NAMES):
        result["acquisition_engine_calls"] = {engine: counts[engine] for engine in _ENGINE_NAMES}
    else:
        result["acquisition_engine_calls"] = None
    return result


def _read_pinned_receipt_directory(root: Path, expected_sha256: str) -> dict[str, object]:
    if not _SHA.fullmatch(expected_sha256):
        raise ResourceEvidenceError("receipt-inventory-pin-invalid")
    _require_private_directory(root)
    documents = {}
    digests = []
    for path in sorted(root.iterdir(), key=lambda item: item.name):
        if path.is_symlink() or not path.is_file():
            raise ResourceEvidenceError("receipt-inventory-entry-invalid")
        raw = _read_private(path, max_bytes=coverage_live_acquire.MAX_POOL_SNAPSHOT_BYTES)
        digests.append({"name": path.name, "sha256": _sha(raw), "bytes": len(raw)})
        if path.suffix == ".json":
            document = _strict_json(raw, path.name)
            if coverage_live_acquire._canonical(document) != raw:
                raise ResourceEvidenceError("receipt-inventory-json-not-canonical")
            documents[path.name] = document
    if _sha(_canonical(digests)) != expected_sha256:
        raise ResourceEvidenceError("receipt-inventory-pin-mismatch")
    return documents


def receipt_inventory_sha256(root: Path) -> str:
    """Compute a digest for an external seal to pin before collection."""
    _require_private_directory(root)
    rows = []
    for path in sorted(root.iterdir(), key=lambda item: item.name):
        if path.is_symlink() or not path.is_file():
            raise ResourceEvidenceError("receipt-inventory-entry-invalid")
        raw = _read_private(path, max_bytes=coverage_live_acquire.MAX_POOL_SNAPSHOT_BYTES)
        rows.append({"name": path.name, "sha256": _sha(raw), "bytes": len(raw)})
    return _sha(_canonical(rows))


def collect_resource_evidence(**kwargs) -> ResourceEvidenceReport:
    """Collect independently pinned stage observations; see keyword contract below.

    Required inputs are explicit immutable objects/bytes and caller-supplied
    expected hashes: stage_uuid, source_revision, protocol_bytes, cohorts_bytes,
    source_closure_sha256, acquisition_plan_bytes, snapshots_directory,
    receipt_directory, expected_receipt_inventory_sha256, acquisition_manifest_bytes,
    expected_snapshot_index_sha256, acquisition_manifest_sha256, capture_result,
    expected_capture_manifest_sha256, expected_capture_inventory_sha256,
    candidate_identity_bytes, candidate_identity_sha256, candidate_endpoint_sha256,
    answer_result, answer_tasks, answer_archive_root, answer_result_root,
    expected_answer_terminal_sha256, expected_answer_operation_manifest_sha256,
    prepared_references, reference_submissions, reference_closure,
    prepared_answers, answer_submissions, answer_closure, and the three grade
    closure pin hashes from their packet-preparation boundaries.
    """
    deadline = kwargs.get("deadline_monotonic")
    if deadline is not None and (type(deadline) not in {int, float} or not math.isfinite(deadline) or deadline <= 0):
        raise ResourceEvidenceError("resource-collection-deadline-invalid")
    token = _DEADLINE.set(deadline)
    try:
        _check_deadline()
        stage_uuid = kwargs["stage_uuid"]
        source_revision = kwargs["source_revision"]
        protocol_bytes = kwargs["protocol_bytes"]
        cohorts_bytes = kwargs["cohorts_bytes"]
        protocol = _strict_json(protocol_bytes, "protocol")
        if _sha(cohorts_bytes) != protocol.get("cohorts_sha256"):
            raise ResourceEvidenceError("protocol-cohort-binding")
        protocol_sha256 = _sha(protocol_bytes)
        cohorts_sha256 = _sha(cohorts_bytes)
        input_mode = offline_acquisition.pool_input_mode_for_stage(protocol, kwargs.get("stage_kind"))
        try:
            acquisition_profile = offline_acquisition.execution_profile_from_protocol(
                protocol.get("acquisition"),
                pool_input_mode=input_mode,
            )
        except Exception as exc:
            raise ResourceEvidenceError("protocol-acquisition-profile-invalid") from exc
        if input_mode == "saved-corpus/1":
            acq, capture, saved_provenance = _saved_origin_resource_observations(kwargs, protocol)
        else:
            acquisition_plan_bytes = kwargs["acquisition_plan_bytes"]
            acq = collect_acquisition_observations(
                snapshots_directory=kwargs["snapshots_directory"],
                receipt_directory=kwargs["receipt_directory"],
                expected_receipt_inventory_sha256=kwargs["expected_receipt_inventory_sha256"],
                acquisition_manifest_bytes=kwargs["acquisition_manifest_bytes"],
                acquisition_plan_bytes=acquisition_plan_bytes,
                expected_index_sha256=kwargs["expected_snapshot_index_sha256"],
                expected_manifest_sha256=kwargs["acquisition_manifest_sha256"],
                stage_uuid=stage_uuid,
                source_revision=source_revision,
                protocol_sha256=protocol_sha256,
                cohorts_sha256=cohorts_sha256,
                source_closure_sha256=kwargs["source_closure_sha256"],
                acquisition_plan_sha256=_sha(acquisition_plan_bytes),
                configured_timeout_seconds=protocol["acquisition"]["timeout_seconds"],
                expected_execution_profile=acquisition_profile.identifier,
                configured_query_pacing_seconds=protocol["acquisition"]["pacing_seconds"],
                configured_arxiv_pacing_seconds=protocol["acquisition"]["arxiv_physical_pacing_seconds"],
                deadline_monotonic=deadline,
                verified_pool_source=kwargs.get("verified_pool_source"),
            )
            capture = _capture_observations(
                capture_result=kwargs["capture_result"],
                expected_inventory_sha256=kwargs["expected_capture_inventory_sha256"],
                expected_manifest_sha256=kwargs["expected_capture_manifest_sha256"],
                protocol_sha256=protocol_sha256,
                cohorts_sha256=cohorts_sha256,
                source_revision=source_revision,
                candidate_identity_bytes=kwargs["candidate_identity_bytes"],
                candidate_identity_sha256=kwargs["candidate_identity_sha256"],
                candidate_endpoint_sha256=kwargs["candidate_endpoint_sha256"],
                timeout_limit_seconds=protocol["capture"]["timeout_seconds"],
                response_bytes_limit=protocol["capture"]["response_bytes"],
            )
            saved_provenance = None
        answer = _answer_observations(
            stage_uuid=stage_uuid,
            source_revision=source_revision,
            protocol_sha256=protocol_sha256,
            cohorts_sha256=cohorts_sha256,
            answer_result=kwargs["answer_result"],
            answer_tasks=kwargs["answer_tasks"],
            answer_archive_root=Path(kwargs["answer_archive_root"]),
            answer_result_root=Path(kwargs["answer_result_root"]),
            expected_terminal_sha256=kwargs["expected_answer_terminal_sha256"],
            expected_manifest_sha256=kwargs["expected_answer_operation_manifest_sha256"],
        )
        grader = _grade_observations(kwargs)
        selector = _selector_observations(
            stage_uuid=stage_uuid,
            source_revision=source_revision,
            protocol_sha256=protocol_sha256,
            source_closure_sha256=kwargs["source_closure_sha256"],
            expected_terminal_sha256=kwargs.get("expected_selector_terminal_sha256"),
            terminal_inventory_bytes=kwargs.get("selector_terminal_inventory_bytes"),
            result_root=(
                Path(kwargs["selector_result_root"]) if kwargs.get("selector_result_root") is not None else None
            ),
            archive_root=(
                Path(kwargs["selector_archive_root"]) if kwargs.get("selector_archive_root") is not None else None
            ),
            expected_operation_ids=tuple(kwargs.get("selector_expected_operation_ids", ())),
            operation_rows=tuple(kwargs.get("selector_operation_rows", ())),
            expected_registration_sha256=kwargs.get("selector_registration_sha256"),
            expected_primary_control=protocol.get("primary_control") if type(protocol) is dict else None,
            expected_candidate_source_sha256=(
                protocol.get("candidate_source_sha256") if type(protocol) is dict else None
            ),
            expected_selector_input_map_schema=(
                protocol.get("selector_input_map_schema") if type(protocol) is dict else None
            ),
            selector_input_map_bytes=kwargs.get("selector_input_map_bytes"),
            expected_selector_input_map_sha256=kwargs.get("expected_selector_input_map_sha256"),
            expected_saved_import_map_bindings=(
                {
                    "pool_input_mode": "saved-corpus/1",
                    "origin_pool_snapshot_index_sha256": saved_provenance["origin_pool_snapshot_index_sha256"],
                    "origin_pool_manifest_sha256": saved_provenance["origin_pool_manifest_sha256"],
                    "origin_stage_uuid": saved_provenance["origin_stage_uuid"],
                    "origin_source_revision": saved_provenance["origin_source_revision"],
                    "origin_source_closure_sha256": saved_provenance["origin_source_closure_sha256"],
                    "origin_protocol_sha256": saved_provenance["origin_protocol_sha256"],
                    "origin_cohorts_sha256": saved_provenance["origin_cohorts_sha256"],
                    "origin_acquisition_plan_sha256": saved_provenance["origin_acquisition_plan_sha256"],
                    "saved_import_receipt_sha256": saved_provenance["saved_import_receipt_sha256"],
                    "derived_capture_manifest_sha256": saved_provenance["derived_capture_manifest_sha256"],
                }
                if saved_provenance is not None
                else None
            ),
        )
        observations = {**acq, **capture, **answer, **grader, **selector}
        if input_mode == "saved-corpus/1":
            stage_started = kwargs.get("stage_started_monotonic")
            if (
                type(stage_started) not in {int, float}
                or isinstance(stage_started, bool)
                or not math.isfinite(float(stage_started))
                or stage_started < 0
            ):
                raise ResourceEvidenceError("saved-stage-start-time-invalid")
            observations.update(
                {
                    "historical_input_search_calls": acq["acquisition_physical_http_calls"],
                    "historical_input_capture_calls": capture["capture_owned_http_calls"],
                    "historical_input_scrape_calls": capture["capture_scrape_calls"],
                    "historical_input_health_calls": capture["capture_health_calls"],
                    "current_stage_search_calls": 0,
                    "current_stage_capture_calls": 0,
                    "current_stage_elapsed_seconds": (max(0.0, time.monotonic() - float(stage_started))),
                }
            )
        observations["stage_elapsed_seconds"] = None
        observations["grader_concurrency"] = None
        observations["capture_internal_fanout"] = None
        observations["capture_response_bytes_limit"] = capture.get("capture_response_bytes_limit_applied")
        observations["answerer_max_words"] = answer.get("answerer_max_words")
        provenance = {
            "protocol_sha256": protocol_sha256,
            "cohorts_sha256": cohorts_sha256,
            "source_revision": source_revision,
            "acquisition_snapshot_index_sha256": kwargs["expected_snapshot_index_sha256"],
            "acquisition_receipt_inventory_sha256": kwargs["expected_receipt_inventory_sha256"],
            "capture_manifest_sha256": kwargs["expected_capture_manifest_sha256"],
            "capture_inventory_sha256": kwargs["expected_capture_inventory_sha256"],
            "answer_terminal_sha256": kwargs["expected_answer_terminal_sha256"],
            "answer_manifest_sha256": kwargs["expected_answer_operation_manifest_sha256"],
            "selector_terminal_inventory_sha256": kwargs.get("expected_selector_terminal_sha256"),
            "selector_stage_uuid": stage_uuid,
            "selector_source_revision": source_revision,
            "selector_protocol_sha256": protocol_sha256,
            "selector_source_closure_sha256": kwargs["source_closure_sha256"],
            "selector_registration_sha256": kwargs.get("selector_registration_sha256"),
            "protocol_limits_and_configuration": _configuration_provenance(protocol),
            "acquisition_timeout_measurement_basis": (
                "per-dispatched-request timeout extension recorded by the pinned HTTPX transport receipts; "
                "not elapsed request duration"
            ),
            "quality_credit": False,
            "admission_created": False,
            "resource_accounting_mode": input_mode,
            "saved_origin": saved_provenance,
            "resource_accounting_note": (
                "saved development search and capture costs are historical input-creation measurements; "
                "current-stage search and capture dispatch counts are separately zero"
                if input_mode == "saved-corpus/1"
                else "resource costs are derived from the registered input method"
            ),
            "selector_transaction_proof_sha256": (
                _sha(_canonical(selector["selector_transaction_proof"]))
                if selector.get("selector_transaction_proof") is not None
                else None
            ),
        }
        receipt = _canonical(
            {
                "schema": "coverage-resource-evidence/1",
                "stage_uuid": stage_uuid,
                "observations": observations,
                "provenance": provenance,
            }
        )
        return ResourceEvidenceReport(stage_uuid, observations, provenance, _sha(receipt))
    except ResourceEvidenceError:
        raise
    except (KeyError, TypeError, ValueError, OSError) as exc:
        raise ResourceEvidenceError("resource-evidence-input-invalid") from exc
    finally:
        _DEADLINE.reset(token)


def _answer_observations(**kwargs) -> dict[str, object]:
    stage_uuid = kwargs["stage_uuid"]
    result = kwargs["answer_result"]
    if result.stage_uuid != stage_uuid or result.semantic_grade is not False:
        raise ResourceEvidenceError("answer-stage-binding-mismatch")
    terminal_path = kwargs["answer_result_root"] / f"{stage_uuid}.answer-terminal-inventory.json"
    try:
        _require_private_directory(kwargs["answer_result_root"])
        _require_private_directory(kwargs["answer_archive_root"])
        terminal_bytes = _read_private(terminal_path, max_bytes=4_000_000)
    except OSError as exc:
        raise ResourceEvidenceError("answer-terminal-unreadable") from exc
    if (
        _sha(terminal_bytes) != kwargs["expected_terminal_sha256"]
        or result.terminal_receipt_sha256 != kwargs["expected_terminal_sha256"]
    ):
        raise ResourceEvidenceError("answer-terminal-pin-mismatch")
    terminal = _strict_json(terminal_bytes, "answer-terminal")
    expected_terminal = {
        "stage_uuid": stage_uuid,
        "source_revision": kwargs["source_revision"],
        "protocol_sha256": kwargs["protocol_sha256"],
        "cohorts_sha256": kwargs["cohorts_sha256"],
        "answer_manifest_sha256": kwargs["expected_manifest_sha256"],
    }
    if (
        type(terminal) is not dict
        or answer_execution._canonical(terminal) != terminal_bytes
        or any(terminal.get(k) != v for k, v in expected_terminal.items())
        or terminal.get("status") != "complete-structurally-valid-not-semantically-graded"
        or result.status != terminal.get("status")
    ):
        raise ResourceEvidenceError("answer-terminal-stage-binding")
    requests = answer_execution._make_requests(kwargs["answer_tasks"])
    _request_manifest, actual_manifest_sha = answer_execution._request_manifest(
        requests, endpoint_security_mode=terminal.get("endpoint_security_mode")
    )
    if actual_manifest_sha != kwargs["expected_manifest_sha256"]:
        raise ResourceEvidenceError("answer-request-manifest-mismatch")
    if len(requests) != 18 or terminal.get("owned_http_calls") != 18 or result.owned_http_calls != 18:
        raise ResourceEvidenceError("answer-operation-count-invalid")
    rows = terminal.get("operations")
    if type(rows) is not list or len(rows) != 18 or list(result.operations) != rows:
        raise ResourceEvidenceError("answer-operation-inventory-mismatch")
    control_attestation = terminal.get("execution_control_attestation")
    control_operations = None
    answerer_timeout = None
    answerer_retries = None
    answerer_application_retries = None
    answerer_concurrency = None
    if control_attestation is not None:
        expected_control_bindings = {
            "schema": "coverage-answer-execution-controls/1",
            "stage_uuid": stage_uuid,
            "source_revision": kwargs["source_revision"],
            "protocol_sha256": kwargs["protocol_sha256"],
            "cohorts_sha256": kwargs["cohorts_sha256"],
            "operation_plan_sha256": kwargs["expected_manifest_sha256"],
            "producer_module_sha256": execution_controls.module_source_sha256(answer_execution.__file__),
            "execution_controls_module_sha256": execution_controls.module_source_sha256(execution_controls.__file__),
            "timeout_seconds_limit_configured": answer_execution.REQUEST_TIMEOUT_SECONDS,
            "response_bytes_limit_applied": answer_execution.MAX_RESPONSE_BYTES,
            "request_bytes_limit_applied": answer_execution.MAX_REQUEST_BYTES,
            "output_tokens_limit_applied": answer_execution.MAX_OUTPUT_TOKENS,
        }
        retry_policy = control_attestation.get("transport_retry_policy") if type(control_attestation) is dict else None
        expected_retry_fields = (
            {"transport_retry_policy": "configured-zero-owned-httpx-transport", "transport_retries_configured": 0}
            if retry_policy == "configured-zero-owned-httpx-transport"
            else {"transport_retry_policy": "unknown-injected-transport", "transport_retries_configured": None}
        )
        control_keys = {
            *expected_control_bindings,
            *expected_retry_fields,
            "application_retries_observed",
            "max_concurrent_requests_observed",
            "operations",
        }
        if (
            type(control_attestation) is not dict
            or set(control_attestation) != control_keys
            or any(control_attestation.get(key) != value for key, value in expected_control_bindings.items())
            or any(control_attestation.get(key) != value for key, value in expected_retry_fields.items())
        ):
            raise ResourceEvidenceError("answer-control-attestation-binding")
        control_operations = control_attestation.get("operations")
        if type(control_operations) is not list or len(control_operations) != len(requests):
            raise ResourceEvidenceError("answer-control-operation-inventory")
        for index, (request, row, control_row) in enumerate(
            zip(requests, rows, control_operations, strict=True), start=1
        ):
            if (
                type(control_row) is not dict
                or set(control_row)
                != {
                    "operation_id",
                    "sequence",
                    "request_sha256",
                    "timeout_seconds_applied",
                    "transport_dispatch_count",
                }
                or control_row.get("operation_id") != request.operation_id
                or control_row.get("sequence") != index
                or control_row.get("request_sha256") != _sha(request.body)
                or control_row.get("transport_dispatch_count") != 1
                or control_row.get("timeout_seconds_applied") != answer_execution.REQUEST_TIMEOUT_SECONDS
                or len(request.body) > answer_execution.MAX_REQUEST_BYTES
            ):
                raise ResourceEvidenceError("answer-control-operation-binding")
        max_concurrency = control_attestation.get("max_concurrent_requests_observed")
        if type(max_concurrency) is not int or max_concurrency != 1:
            raise ResourceEvidenceError("answer-control-concurrency-invalid")
        if control_attestation.get("application_retries_observed") != 0:
            raise ResourceEvidenceError("answer-control-application-retries-invalid")
        answerer_timeout = float(answer_execution.REQUEST_TIMEOUT_SECONDS)
        answerer_application_retries = 0
        answerer_concurrency = max_concurrency
        if retry_policy == "configured-zero-owned-httpx-transport":
            answerer_retries = 0
    max_request = max_response = max_words = 0
    total_response = 0
    for request, row in zip(requests, rows, strict=True):
        if type(row) is not dict or row.get("operation_id") != request.operation_id:
            raise ResourceEvidenceError("answer-operation-order")
        receipt, body = receipts.replay_response(
            kwargs["answer_archive_root"],
            expected_bindings={
                "stage_uuid": stage_uuid,
                "operation_id": request.operation_id,
                "request_body_sha256": _sha(request.body),
                "source_revision": kwargs["source_revision"],
            },
            expected_receipt_sha256=row["archive_receipt_sha256"],
        )
        if (
            row.get("response_bytes") != len(body)
            or row.get("response_sha256") != _sha(body)
            or len(body) > answer_execution.MAX_RESPONSE_BYTES
        ):
            raise ResourceEvidenceError("answer-response-binding")
        max_request = max(max_request, len(request.body))
        max_response = max(max_response, len(body))
        total_response += len(body)
        envelope = answer_execution._strict_json(body)
        message = envelope["choices"][0]["message"]["content"]
        if request.kind == "answer":
            answer_obj = answer_execution._strict_json(message.encode("utf-8"))
            validation = answer_execution.consumer.validate_answer(
                answer_obj,
                request.answer_input,
                critical_check_ids=[check["id"] for check in request.task.critical_checks],
            )
            max_words = max(max_words, validation["words"])
            if row.get("status") != "answer-structurally-valid":
                raise ResourceEvidenceError("answer-operation-status-mismatch")
        elif request.kind == "readiness":
            if answer_execution._strict_json(message.encode("utf-8")) != {"ready": True}:
                raise ResourceEvidenceError("answer-readiness-response-invalid")
            if row.get("status") != "readiness-complete":
                raise ResourceEvidenceError("answer-readiness-status-mismatch")
    return {
        "answerer_calls": len(requests),
        "answerer_request_bytes_total": sum(len(req.body) for req in requests),
        "answerer_max_request_body_bytes_observed": max_request,
        "answerer_max_request_bytes": max_request,
        "answerer_total_response_body_bytes": total_response,
        "answerer_max_response_body_bytes_observed": max_response,
        "answerer_max_response_bytes": max_response,
        "answerer_max_words_observed": max_words,
        "answerer_max_words": max_words,
        "answerer_output_tokens_requested": _uniform_request_integer(requests, "max_tokens"),
        "max_sources_per_task": _max_delivered_sources(requests),
        "answerer_usage_status": terminal.get("usage_status"),
        "answerer_timeout_seconds": answerer_timeout,
        "answerer_retries": answerer_retries,
        "answerer_application_retries_observed": answerer_application_retries,
        "answerer_concurrency_observed": answerer_concurrency,
    }


def _uniform_request_integer(requests, key: str) -> int | None:
    values = []
    for request in requests:
        body = _strict_json(request.body, "answer-request")
        value = body.get(key) if type(body) is dict else None
        if type(value) is not int or value < 0:
            return None
        values.append(value)
    return values[0] if values and len(set(values)) == 1 else None


def _max_delivered_sources(requests) -> int | None:
    counts = []
    for request in requests:
        if request.kind != "answer":
            continue
        body = _strict_json(request.body, "answer-request")
        messages = body.get("messages") if type(body) is dict else None
        if type(messages) is not list or len(messages) != 2 or type(messages[1]) is not dict:
            return None
        user_content = messages[1].get("content")
        if type(user_content) is not str:
            return None
        payload = _strict_json(user_content.encode("utf-8"), "answer-user-payload")
        source_rows = payload.get("sources") if type(payload) is dict else None
        if type(source_rows) is not list:
            return None
        counts.append(len(source_rows))
    return max(counts) if counts else None


def _grade_observations(kwargs: Mapping[str, object]) -> dict[str, object]:
    from scripts import coverage_grade_closure as closure

    prepared_refs = kwargs["prepared_references"]
    prepared_answers = kwargs["prepared_answers"]
    reference = closure.close_preassessment(
        prepared_refs,
        kwargs["reference_submissions"],
        expected_manifest_sha256=kwargs["expected_preassessment_manifest_sha256"],
        expected_private_binding_sha256=kwargs["expected_private_binding_sha256"],
    )
    answers = closure.close_answer_assessments(
        prepared_refs,
        prepared_answers,
        reference,
        kwargs["answer_submissions"],
        expected_preassessment_manifest_sha256=kwargs["expected_preassessment_manifest_sha256"],
        expected_private_binding_sha256=kwargs["expected_private_binding_sha256"],
        expected_answer_manifest_sha256=kwargs["expected_answer_assessment_manifest_sha256"],
    )
    if (
        reference.receipt_sha256 != kwargs["reference_closure"].receipt_sha256
        or answers.receipt_sha256 != kwargs["answer_closure"].receipt_sha256
        or reference.receipt_sha256 != kwargs["expected_reference_closure_sha256"]
        or answers.receipt_sha256 != kwargs["expected_answer_closure_sha256"]
    ):
        raise ResourceEvidenceError("grade-closure-replay-mismatch")
    request_sizes = []
    for packet_rows, submitted in (
        (prepared_refs.preassessment_packets, reference.submission_receipts),
        (prepared_answers.packets, answers.submission_receipts),
    ):
        packets_by_id = {packet["packet_id"]: packet for packet in packet_rows}
        if len(packets_by_id) != len(packet_rows):
            raise ResourceEvidenceError("grader-request-packet-binding")
        # No-call source packets remain part of the prepared manifest but
        # have no submission receipt. Count only requests actually submitted.
        for submission in submitted:
            packet = packets_by_id.get(submission.packet_id)
            if packet is None:
                raise ResourceEvidenceError("grader-request-packet-binding")
            raw = packet.get("bytes")
            if (
                type(raw) is not bytes
                or packet.get("byte_count") != len(raw)
                or packet.get("sha256") != _sha(raw)
                or submission.input_sha256 != _sha(raw)
            ):
                raise ResourceEvidenceError("grader-request-packet-binding")
            request_sizes.append(len(raw))
    response_sizes = [row.response_byte_count for row in (*reference.submission_receipts, *answers.submission_receipts)]
    return {
        "grader_submissions": len(response_sizes),
        "grader_request_bytes_total": sum(request_sizes),
        "max_grader_request_body_bytes_observed": max(request_sizes) if request_sizes else None,
        "grader_response_bytes_total": sum(response_sizes),
        "max_grader_response_body_bytes_observed": max(response_sizes) if response_sizes else None,
    }


def _configuration_provenance(protocol: Mapping[str, object]) -> dict[str, object]:
    return {
        "acquisition": protocol.get("acquisition"),
        "capture": protocol.get("capture"),
        "consumer": protocol.get("consumer"),
        "provider_schedule": protocol.get("provider_schedule"),
        "phase_limits": protocol.get("phase_limits"),
        "quality_credit": False,
        "note": "configured ceilings are not measured observations",
    }


def _saved_method_material_path(name: str) -> Path:
    if name not in {"protocol.json", "cohorts.json"}:
        raise ResourceEvidenceError("saved-origin-method-material-name-invalid")
    return (
        Path(__file__).resolve().parent.parent
        / "docs/experiments/evidence/coverage-first-study/frozen-input-method-materials"
        / name
    )


def _read_saved_method_material(name: str, *, max_bytes: int) -> bytes:
    path = _saved_method_material_path(name)
    try:
        fd = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    except OSError as exc:
        raise ResourceEvidenceError("saved-origin-method-material-unavailable") from exc
    try:
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or info.st_size > max_bytes:
            raise ResourceEvidenceError("saved-origin-method-material-invalid")
        with os.fdopen(fd, "rb", closefd=False) as handle:
            raw = handle.read(max_bytes + 1)
        if len(raw) > max_bytes:
            raise ResourceEvidenceError("saved-origin-method-material-too-large")
        return raw
    finally:
        os.close(fd)


def _saved_origin_resource_observations(
    kwargs: Mapping[str, object], current_protocol: Mapping[str, object]
) -> tuple[dict[str, object], dict[str, object], dict[str, object]]:
    """Recompute original input-creation costs from the pinned saved package.

    The saved branch accounts the origin's search and capture artifacts under
    their original registration. It does not relabel them as calls made by the
    current grading stage.
    """
    from scripts import coverage_frozen_pool_inputs as frozen_pool
    from scripts import coverage_saved_corpus

    origin = kwargs.get("saved_origin")
    imported = kwargs.get("saved_corpus_import")
    if (
        type(origin) is not coverage_saved_corpus.SavedCorpusOrigin
        or type(imported) is not coverage_saved_corpus.SavedCorpusImportResult
    ):
        raise ResourceEvidenceError("saved-origin-resource-input-invalid")
    registration = origin.registration
    if (
        kwargs.get("stage_kind") != "development"
        or registration.stage_kind != "development"
        or imported.origin_package_sha256 != origin.expected_package_sha256
        or imported.origin_pool_verified is None
        or imported.origin_capture_result.stage_uuid != registration.stage_uuid
    ):
        raise ResourceEvidenceError("saved-origin-resource-binding-mismatch")
    import_receipt_bytes = imported.import_receipt_bytes
    try:
        import_receipt = _strict_json(import_receipt_bytes, "saved-import-receipt")
    except Exception as exc:
        raise ResourceEvidenceError("saved-origin-import-receipt-invalid") from exc
    expected_import_keys = {
        "schema",
        "status",
        "stage_uuid",
        "stage_kind",
        "source_revision",
        "source_closure_sha256",
        "protocol_sha256",
        "cohorts_sha256",
        "candidate_identity_sha256",
        "candidate_endpoint_sha256",
        "packet_stage_uuid",
        "origin_stage_uuid",
        "origin_source_revision",
        "origin_package_sha256",
        "origin_pool_manifest_sha256",
        "origin_pool_snapshot_index_sha256",
        "origin_collection_receipt_sha256",
        "origin_capture_manifest_sha256",
        "origin_capture_inventory_sha256",
        "derived_capture_manifest_sha256",
        "import_authority_receipt_sha256",
        "research_task_ids",
        "navigation_target_ids",
        "source_rows",
        "captured_contexts",
        "source_failures",
        "historical_search_calls",
        "historical_capture_calls",
        "historical_scrape_calls",
        "historical_health_calls",
        "current_search_calls",
        "current_capture_calls",
        "quality_credit",
    }
    expected_current_bindings = {
        "schema": "coverage-saved-corpus-import/1",
        "status": "verified-for-fresh-grading",
        "stage_uuid": kwargs.get("stage_uuid"),
        "stage_kind": kwargs.get("stage_kind"),
        "source_revision": kwargs.get("source_revision"),
        "source_closure_sha256": kwargs.get("source_closure_sha256"),
        "protocol_sha256": _sha(kwargs.get("protocol_bytes", b"")),
        "cohorts_sha256": _sha(kwargs.get("cohorts_bytes", b"")),
        "candidate_identity_sha256": kwargs.get("candidate_identity_sha256"),
        "candidate_endpoint_sha256": kwargs.get("candidate_endpoint_sha256"),
        "origin_stage_uuid": registration.stage_uuid,
        "origin_source_revision": registration.source_revision,
        "origin_package_sha256": origin.expected_package_sha256,
        "research_task_ids": list(registration.task_ids),
        "navigation_target_ids": list(registration.navigation_ids),
        "current_search_calls": 0,
        "current_capture_calls": 0,
        "quality_credit": False,
    }
    if (
        type(import_receipt) is not dict
        or set(import_receipt) != expected_import_keys
        or _sha(import_receipt_bytes) != imported.import_receipt_sha256
        or coverage_saved_corpus._canonical(import_receipt) != import_receipt_bytes
        or any(import_receipt.get(key) != value for key, value in expected_current_bindings.items())
        or type(import_receipt.get("packet_stage_uuid")) is not str
        or not re.fullmatch(
            r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
            import_receipt["packet_stage_uuid"],
        )
        or (
            kwargs.get("packet_stage_uuid") is not None
            and import_receipt.get("packet_stage_uuid") != kwargs.get("packet_stage_uuid")
        )
        or type(import_receipt.get("import_authority_receipt_sha256")) is not str
        or not _SHA.fullmatch(import_receipt["import_authority_receipt_sha256"])
        or _sha(imported.capture_manifest_bytes) != imported.capture_manifest_sha256
    ):
        raise ResourceEvidenceError("saved-origin-import-receipt-binding-mismatch")

    # Resolve only the committed, reviewed method materials. The registration
    # hashes remain the authority; no caller-supplied filesystem path is used.
    origin_protocol_bytes = _read_saved_method_material("protocol.json", max_bytes=2_000_000)
    origin_cohorts_bytes = _read_saved_method_material("cohorts.json", max_bytes=2_000_000)
    if (
        _sha(origin_protocol_bytes) != registration.protocol_sha256
        or _sha(origin_cohorts_bytes) != registration.cohorts_sha256
    ):
        raise ResourceEvidenceError("saved-origin-locked-material-pin-mismatch")
    origin_protocol = _strict_json(origin_protocol_bytes, "saved-origin-protocol")
    origin_cohorts = _strict_json(origin_cohorts_bytes, "saved-origin-cohorts")
    if type(origin_protocol) is not dict or type(origin_cohorts) is not dict:
        raise ResourceEvidenceError("saved-origin-locked-material-invalid")

    current_origin = current_protocol.get("saved_development_origin")
    historical_producer_sha = (
        current_origin.get("capture_producer_source_sha256") if type(current_origin) is dict else None
    )
    if (
        type(current_origin) is not dict
        or current_origin.get("stage_uuid") != registration.stage_uuid
        or current_origin.get("source_revision") != registration.source_revision
        or current_origin.get("package_sha256") != origin.expected_package_sha256
        or type(historical_producer_sha) is not str
        or not _SHA.fullmatch(historical_producer_sha)
    ):
        raise ResourceEvidenceError("saved-origin-capture-producer-pin-missing")

    try:
        observed_package_sha = coverage_saved_corpus.package_sha256(
            Path(origin.collection_directory), Path(origin.capture_stage_directory)
        )
        collection_inventory_sha = receipt_inventory_sha256(Path(origin.collection_directory))
    except Exception as exc:
        raise ResourceEvidenceError("saved-origin-package-reverification-failed") from exc
    if (
        observed_package_sha != origin.expected_package_sha256
        or observed_package_sha != imported.origin_package_sha256
        or collection_inventory_sha != origin.expected_collection_receipt_inventory_sha256
    ):
        raise ResourceEvidenceError("saved-origin-package-pin-mismatch")
    historical_operation_runner_module = coverage_saved_corpus.verify_historical_operation_runner_module(origin)
    if imported.historical_operation_runner_module != historical_operation_runner_module:
        raise ResourceEvidenceError("saved-origin-operation-runner-source-proof-mismatch")

    collection_root = Path(origin.collection_directory)
    try:
        evidence_bytes = _read_private(
            collection_root / "frozen-pool-evidence.json", max_bytes=coverage_live_acquire.MAX_POOL_SNAPSHOT_BYTES
        )
        collector_receipt_bytes = _read_private(
            collection_root / "collector-receipt.json", max_bytes=coverage_live_acquire.MAX_POOL_SNAPSHOT_BYTES
        )
        availability_bytes = _read_private(
            collection_root / "engine-availability.json", max_bytes=coverage_live_acquire.MAX_POOL_SNAPSHOT_BYTES
        )
        evidence_doc = frozen_pool._strict(evidence_bytes, "saved-origin-pool-evidence")
        origin_deadline = evidence_doc.get("stage_deadline_monotonic")
        origin_completed = evidence_doc.get("collection_completed_monotonic")
        stage_wall = origin_protocol.get("phase_limits", {}).get("stage_wall_seconds")
        if (
            type(origin_deadline) not in (int, float)
            or type(origin_completed) not in (int, float)
            or type(stage_wall) not in (int, float)
            or origin_deadline < stage_wall
        ):
            raise ValueError("saved-origin-clock-invalid")
        origin_started = origin_deadline - stage_wall
        pool_evidence = frozen_pool.FrozenPoolEvidence(
            snapshots_directory=collection_root,
            evidence_bytes=evidence_bytes,
            expected_evidence_sha256=_sha(evidence_bytes),
            collector_receipt_bytes=collector_receipt_bytes,
            expected_collector_receipt_sha256=_sha(collector_receipt_bytes),
            availability_bytes=availability_bytes,
            expected_availability_sha256=_sha(availability_bytes),
        )
        verified_origin_pool = frozen_pool.verify_frozen_pool_evidence(
            pool_evidence,
            expected_stage_uuid=registration.stage_uuid,
            expected_stage_kind=registration.stage_kind,
            expected_source_revision=registration.source_revision,
            expected_source_closure_sha256=registration.source_closure_sha256,
            expected_protocol_sha256=registration.protocol_sha256,
            expected_cohorts_sha256=registration.cohorts_sha256,
            expected_candidate_identity_sha256=registration.candidate_identity_sha256,
            expected_candidate_endpoint_sha256=registration.candidate_endpoint_sha256,
            expected_acquisition_plan_sha256=registration.acquisition_plan_sha256,
            expected_task_ids=registration.task_ids,
            expected_navigation_ids=registration.navigation_ids,
            stage_started_monotonic=origin_started,
            stage_deadline_monotonic=origin_deadline,
            now_monotonic=origin_completed,
        )
    except Exception as exc:
        raise ResourceEvidenceError("saved-origin-pool-reverification-failed") from exc
    if (
        verified_origin_pool.snapshot_index_sha256 != imported.origin_pool_verified.snapshot_index_sha256
        or verified_origin_pool.pool_manifest_sha256 != imported.origin_pool_verified.pool_manifest_sha256
        or verified_origin_pool.proof_sha256 != imported.origin_pool_verified.proof_sha256
        or import_receipt.get("origin_pool_snapshot_index_sha256") != verified_origin_pool.snapshot_index_sha256
        or import_receipt.get("origin_pool_manifest_sha256") != verified_origin_pool.pool_manifest_sha256
        or import_receipt.get("origin_collection_receipt_sha256") != registration.collector_receipt_sha256
    ):
        raise ResourceEvidenceError("saved-origin-pool-proof-mismatch")

    acquisition_plan_bytes = imported.acquisition_plan_bytes
    if acquisition_plan_bytes is None:
        try:
            acquisition_plan_bytes = coverage_saved_corpus.origin_acquisition_plan_bytes(origin)
        except Exception as exc:
            raise ResourceEvidenceError("saved-origin-acquisition-plan-unavailable") from exc
    acquisition_manifest_bytes = _read_private(
        collection_root / "acquisition-manifest.json", max_bytes=coverage_live_acquire.MAX_POOL_SNAPSHOT_BYTES
    )
    if (
        type(acquisition_plan_bytes) is not bytes
        or _sha(acquisition_plan_bytes) != registration.acquisition_plan_sha256
    ):
        raise ResourceEvidenceError("saved-origin-acquisition-plan-pin-mismatch")
    origin_acquisition = origin_protocol.get("acquisition")
    origin_profile = offline_acquisition.execution_profile_from_protocol(
        origin_acquisition, pool_input_mode="frozen-pool/1"
    )
    origin_acq = collect_acquisition_observations(
        snapshots_directory=collection_root,
        receipt_directory=collection_root,
        expected_receipt_inventory_sha256=origin.expected_collection_receipt_inventory_sha256,
        acquisition_manifest_bytes=acquisition_manifest_bytes,
        acquisition_plan_bytes=acquisition_plan_bytes,
        expected_index_sha256=verified_origin_pool.snapshot_index_sha256,
        expected_manifest_sha256=verified_origin_pool.pool_manifest_sha256,
        stage_uuid=registration.stage_uuid,
        source_revision=registration.source_revision,
        protocol_sha256=registration.protocol_sha256,
        cohorts_sha256=registration.cohorts_sha256,
        source_closure_sha256=registration.source_closure_sha256,
        acquisition_plan_sha256=registration.acquisition_plan_sha256,
        configured_timeout_seconds=origin_acquisition["timeout_seconds"],
        expected_execution_profile=origin_profile.identifier,
        configured_query_pacing_seconds=origin_acquisition["pacing_seconds"],
        configured_arxiv_pacing_seconds=origin_acquisition["arxiv_physical_pacing_seconds"],
        deadline_monotonic=origin_deadline,
        verified_pool_source=verified_origin_pool,
        verified_historical_operation_runner_module=historical_operation_runner_module,
        verified_saved_origin=origin,
    )
    original_capture = imported.origin_capture_result
    try:
        origin_capture_inventory = _read_private(
            Path(origin.capture_stage_directory) / "inventory.json", max_bytes=4_000_000
        )
        origin_capture_manifest = _read_private(
            Path(origin.capture_stage_directory) / "source-manifest.json", max_bytes=2_000_000
        )
    except Exception as exc:
        raise ResourceEvidenceError("saved-origin-capture-artifact-unreadable") from exc
    if (
        _sha(origin_capture_inventory) != origin.expected_capture_inventory_sha256
        or _sha(origin_capture_manifest) != origin.expected_capture_manifest_sha256
        or import_receipt.get("origin_capture_inventory_sha256") != origin.expected_capture_inventory_sha256
        or import_receipt.get("origin_capture_manifest_sha256") != origin.expected_capture_manifest_sha256
        or import_receipt.get("derived_capture_manifest_sha256") != imported.capture_manifest_sha256
    ):
        raise ResourceEvidenceError("saved-origin-capture-artifact-pin-mismatch")
    try:
        checked_origin_manifest, _checked_origin_sources = source_capture._validate_manifest(
            origin_capture_manifest, registration.protocol_sha256
        )
    except Exception as exc:
        raise ResourceEvidenceError("saved-origin-capture-manifest-invalid") from exc
    expected_derived_manifest = dict(checked_origin_manifest)
    expected_derived_manifest.update(
        {
            "stage_uuid": kwargs["stage_uuid"],
            "source_revision": kwargs["source_revision"],
            "protocol_sha256": _sha(kwargs["protocol_bytes"]),
            "cohorts_sha256": _sha(kwargs["cohorts_bytes"]),
            "candidate_identity_sha256": kwargs["candidate_identity_sha256"],
            "candidate_endpoint_sha256": kwargs["candidate_endpoint_sha256"],
        }
    )
    expected_derived_manifest_bytes = source_capture._canonical_json(expected_derived_manifest)
    if (
        imported.capture_manifest_bytes != expected_derived_manifest_bytes
        or imported.capture_manifest_sha256 != _sha(expected_derived_manifest_bytes)
        or import_receipt.get("derived_capture_manifest_sha256") != _sha(expected_derived_manifest_bytes)
    ):
        raise ResourceEvidenceError("saved-origin-derived-capture-manifest-mismatch")
    origin_capture = _capture_observations(
        capture_result=original_capture,
        expected_inventory_sha256=origin.expected_capture_inventory_sha256,
        expected_manifest_sha256=origin.expected_capture_manifest_sha256,
        protocol_sha256=registration.protocol_sha256,
        cohorts_sha256=registration.cohorts_sha256,
        source_revision=registration.source_revision,
        candidate_identity_bytes=kwargs["candidate_identity_bytes"],
        candidate_identity_sha256=registration.candidate_identity_sha256,
        candidate_endpoint_sha256=registration.candidate_endpoint_sha256,
        timeout_limit_seconds=origin_protocol["capture"]["timeout_seconds"],
        response_bytes_limit=origin_protocol["capture"]["response_bytes"],
        producer_source_sha256=historical_producer_sha,
    )
    observed_import_costs = {
        "historical_search_calls": origin_acq["acquisition_physical_http_calls"],
        "historical_capture_calls": origin_capture["capture_owned_http_calls"],
        "historical_scrape_calls": origin_capture["capture_scrape_calls"],
        "historical_health_calls": origin_capture["capture_health_calls"],
        "historical_captured_sources": original_capture.captured_count,
        "historical_failed_sources": original_capture.failed_count,
    }
    for key, observed in observed_import_costs.items():
        imported_value = getattr(imported, key, None)
        if type(observed) is not int or type(imported_value) is not int or imported_value != observed:
            raise ResourceEvidenceError(f"saved-origin-import-counter-mismatch:{key}")
        receipt_key = {
            "historical_captured_sources": "captured_contexts",
            "historical_failed_sources": "source_failures",
        }.get(key, key)
        if import_receipt.get(receipt_key) != observed:
            raise ResourceEvidenceError(f"saved-origin-import-counter-mismatch:{key}")
    if (
        import_receipt.get("source_rows") != original_capture.source_count
        or import_receipt.get("captured_contexts") != original_capture.captured_count
        or import_receipt.get("source_failures") != original_capture.failed_count
    ):
        raise ResourceEvidenceError("saved-origin-import-counter-mismatch")
    provenance = {
        "origin_stage_uuid": registration.stage_uuid,
        "origin_source_revision": registration.source_revision,
        "origin_protocol_sha256": registration.protocol_sha256,
        "origin_cohorts_sha256": registration.cohorts_sha256,
        "origin_source_closure_sha256": registration.source_closure_sha256,
        "origin_package_sha256": observed_package_sha,
        "origin_pool_snapshot_index_sha256": verified_origin_pool.snapshot_index_sha256,
        "origin_pool_manifest_sha256": verified_origin_pool.pool_manifest_sha256,
        "origin_acquisition_plan_sha256": registration.acquisition_plan_sha256,
        "origin_capture_inventory_sha256": origin.expected_capture_inventory_sha256,
        "origin_capture_manifest_sha256": origin.expected_capture_manifest_sha256,
        "saved_import_receipt_sha256": imported.import_receipt_sha256,
        "derived_capture_manifest_sha256": imported.capture_manifest_sha256,
        "historical_input_costs": True,
    }
    return origin_acq, origin_capture, provenance


__all__ = [
    "ResourceEvidenceError",
    "ResourceEvidenceReport",
    "StageCloseoutObservation",
    "collect_acquisition_observations",
    "collect_resource_evidence",
    "verify_stage_closeout",
]
