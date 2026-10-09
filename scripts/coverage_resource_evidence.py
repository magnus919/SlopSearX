"""Derive resource observations from verified coverage-stage artifacts.

This module does not execute providers, grant admission, or infer quality. It
returns only values supported by pinned snapshots, response archives, packet
bytes, and grade-closure receipts. Protocol limits are reported separately as
configuration provenance, never substituted for measured values.
"""

from __future__ import annotations

import hashlib
import json
import math
import os
import re
import stat
import time
from collections import Counter
from contextvars import ContextVar
from dataclasses import dataclass
from pathlib import Path
from typing import Mapping

from scripts import coverage_answer_execution as answer_execution
from scripts import coverage_live_acquire
from scripts import coverage_source_capture as source_capture
from scripts import coverage_study_acquire as offline_acquisition
from scripts import intent_ranking_receipts as receipts

_SHA = re.compile(r"[0-9a-f]{64}\Z")
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
        closeout["schema"] != "coverage-stage-closeout/1"
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
    if closeout["measurement_basis"] != (
        "stage start through final inventory file and directory fsync; closeout receipt fsync excluded"
    ):
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
) -> dict[str, object]:
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
    return {
        "terminal_inventory_complete": True if all_observed else None,
        "selector_elapsed_ms": max(elapsed_values) if all_observed else None,
        # The stage-wide timer is measured by the coordinator after its final fsync.
        "stage_elapsed_seconds": None,
        "selector_usage": usage_rows if all_observed else None,
        "selector_concurrency": max(concurrency_values) if all_observed else None,
        # Dispatches are observed at the HTTP transport, one expected per slot.
        "selector_retries": retries_total if all_observed else None,
        "selector_provider_dispatches": dispatch_total if all_observed else None,
    }


def coverage_source_capture_normalize(url: str) -> str:
    from slopsearx.merger import _normalise_url

    return _normalise_url(url)


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
    deadline_monotonic: float | None = None,
) -> dict[str, object]:
    try:
        verified = coverage_live_acquire.verify_pool_snapshot_index(
            snapshots_directory,
            expected_index_sha256=expected_index_sha256,
            expected_stage_manifest_sha256=expected_manifest_sha256,
            expected_stage_uuid=stage_uuid,
            expected_source_revision=source_revision,
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
    if _sha(acquisition_manifest_bytes) != expected_manifest_sha256:
        raise ResourceEvidenceError("acquisition-manifest-pin-mismatch")
    manifest = _strict_json(acquisition_manifest_bytes, "acquisition-manifest")
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
    admission = inventory.get("admission.json")
    if type(admission) is not dict or any(
        admission.get(key) != value
        for key, value in {
            "stage_uuid": stage_uuid,
            "source_revision": source_revision,
            "source_closure_sha256": source_closure_sha256,
            "protocol_sha256": protocol_sha256,
            "cohorts_sha256": cohorts_sha256,
            "acquisition_plan_sha256": acquisition_plan_sha256,
            "stage_manifest_sha256": expected_manifest_sha256,
        }.items()
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
        elif final.get("dispatched") is not False:
            unknown_engine = True
        request_bytes, response_bytes = start.get("request_bytes"), final.get("response_bytes")
        if type(request_bytes) is not int or request_bytes < 0 or type(response_bytes) is not int or response_bytes < 0:
            raise ResourceEvidenceError("acquisition-exchange-byte-count-invalid")
        response_sha = final.get("response_sha256")
        if response_bytes > 0 and (type(response_sha) is not str or not _SHA.fullmatch(response_sha)):
            raise ResourceEvidenceError("acquisition-exchange-response-digest-invalid")
        request_total += request_bytes if final.get("dispatched") is True else 0
        response_total += response_bytes
        max_response = max(max_response, response_bytes)
    summary = inventory.get("stage-summary.json")
    if (
        type(summary) is not dict
        or summary.get("stage") != manifest.get("stage")
        or summary.get("status") != "complete"
        or summary.get("physical_request_count") != physical_calls
    ):
        raise ResourceEvidenceError("acquisition-stage-summary-binding")
    result: dict[str, object] = {
        "acquisition_physical_http_calls": physical_calls,
        "acquisition_attempted_http_slots": len(starts),
        "acquisition_request_bytes_total": request_total,
        "acquisition_response_bytes_total": response_total,
        "acquisition_max_response_body_bytes_observed": max_response,
        "acquisition_timeout_seconds": (
            dispatched_timeouts[0]
            if dispatched_timeouts and len(set(dispatched_timeouts)) == 1
            else None
        ),
        "acquisition_timeout_seconds_by_exchange": dispatched_timeouts,
        "acquisition_timeout_observation_state": (
            "observed-uniform"
            if dispatched_timeouts and len(set(dispatched_timeouts)) == 1
            else "observed-variable"
            if dispatched_timeouts
            else "no-dispatched-exchanges"
        ),
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
            deadline_monotonic=deadline,
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
        )
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
        )
        observations = {**acq, **capture, **answer, **grader, **selector}
        observations["w0_control_source_sha256"] = None
        observations["w0_source_revision"] = None
        observations["w0_model"] = None
        observations["w0_provider_configured"] = None
        observations["w0_ranking_strategy"] = None
        observations["w0_parser_parity_verified"] = None
        observations["candidate_source_sha256"] = None
        observations["stage_elapsed_seconds"] = None
        observations["capture_timeout_seconds"] = None
        observations["capture_retries"] = None
        observations["answerer_timeout_seconds"] = None
        observations["answerer_retries"] = None
        observations["grader_concurrency"] = None
        observations["atomic_fallback_verified"] = None
        observations["capture_internal_fanout"] = None
        observations["capture_response_bytes_limit"] = None
        observations["acquisition_retries"] = None
        observations["acquisition_pacing_seconds"] = None
        observations["arxiv_pacing_seconds"] = None
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
            "protocol_limits_and_configuration": _configuration_provenance(protocol),
            "acquisition_timeout_measurement_basis": (
                "per-dispatched-request timeout extension recorded by the pinned HTTPX transport receipts; "
                "not elapsed request duration"
            ),
            "quality_credit": False,
            "admission_created": False,
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
        if row.get("response_bytes") != len(body) or row.get("response_sha256") != _sha(body):
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


__all__ = [
    "ResourceEvidenceError",
    "ResourceEvidenceReport",
    "StageCloseoutObservation",
    "collect_acquisition_observations",
    "collect_resource_evidence",
    "verify_stage_closeout",
]
