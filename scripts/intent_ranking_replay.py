"""Offline, externally pinned replay harness for the coverage prototype."""

from __future__ import annotations

import hashlib
import json
import math
import re
import uuid

from scripts import intent_ranking_coverage as coverage
from scripts import intent_ranking_receipts as receipts

MANIFEST_SCHEMA = "intent-ranking-offline-replay-manifest/1"
MAX_MANIFEST_BYTES = 64_000_000
MAX_OPERATIONS = 160
_SHA256 = re.compile(r"[0-9a-f]{64}\Z")
_REVISION = re.compile(r"[0-9a-f]{40}\Z")
_OPERATION_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,63}\Z")
_MANIFEST_KEYS = {"schema", "stage_uuid", "source_revision", "operations"}
_OPERATION_KEYS = {
    "operation_id",
    "request_body_sha256",
    "receipt_sha256",
    "query",
    "purpose",
    "facets",
    "candidates",
    "incumbent_order",
}


class ReplayError(ValueError):
    """Pinned manifest, operation inventory, or replay integrity failure."""


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    try:
        return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode(
            "utf-8"
        )
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise ReplayError("result-json-invalid") from exc


def _parse_manifest(raw: bytes) -> dict:
    if type(raw) is not bytes or len(raw) > MAX_MANIFEST_BYTES:
        raise ReplayError("manifest-size-or-type")

    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ReplayError("manifest-duplicate-key")
            result[key] = value
        return result

    def reject_constant(value):
        raise ReplayError(f"manifest-nonfinite:{value}")

    def finite_float(value):
        parsed = float(value)
        if not math.isfinite(parsed):
            raise ReplayError("manifest-nonfinite")
        return parsed

    try:
        data = json.loads(
            raw.decode("utf-8"), object_pairs_hook=unique, parse_constant=reject_constant, parse_float=finite_float
        )
    except ReplayError:
        raise
    except (UnicodeDecodeError, ValueError, RecursionError) as exc:
        raise ReplayError("manifest-json-invalid") from exc
    if type(data) is not dict or set(data) != _MANIFEST_KEYS:
        raise ReplayError("manifest-schema")
    return data


def _validate_uuid(value: object, label: str) -> str:
    if type(value) is not str:
        raise ReplayError(f"{label}-type")
    try:
        parsed = uuid.UUID(value)
    except ValueError as exc:
        raise ReplayError(f"{label}-invalid") from exc
    if str(parsed) != value:
        raise ReplayError(f"{label}-noncanonical")
    return value


def _validate_expected_ids(values: object) -> tuple[str, ...]:
    if type(values) not in (list, tuple) or not values or len(values) > MAX_OPERATIONS:
        raise ReplayError("expected-operation-inventory-shape")
    ids = []
    for value in values:
        if type(value) is not str or not _OPERATION_ID.fullmatch(value):
            raise ReplayError("expected-operation-id-invalid")
        ids.append(value)
    if len(ids) != len(set(ids)):
        raise ReplayError("expected-operation-duplicate")
    return tuple(ids)


def _validate_operation_row(row: object) -> dict:
    if type(row) is not dict or set(row) != _OPERATION_KEYS:
        raise ReplayError("operation-schema")
    operation_id = row["operation_id"]
    if type(operation_id) is not str or not _OPERATION_ID.fullmatch(operation_id):
        raise ReplayError("operation-id-invalid")
    for field in ("request_body_sha256", "receipt_sha256"):
        value = row[field]
        if type(value) is not str or not _SHA256.fullmatch(value):
            raise ReplayError(f"{field}-invalid")
    return row


def replay_manifest(
    *,
    archive_root,
    manifest_bytes: bytes,
    expected_manifest_sha256: str,
    expected_stage_uuid: str,
    expected_source_revision: str,
    expected_operation_ids: list[str] | tuple[str, ...],
    expected_result_sha256: str | None = None,
) -> dict:
    """Verify a sealed manifest and complete archive before parsing any response.

    The caller supplies all authority-bearing pins. This function is offline and
    returns diagnostics only; a fallback ranking remains a diagnostic outcome.
    """
    if type(manifest_bytes) is not bytes or len(manifest_bytes) > MAX_MANIFEST_BYTES:
        raise ReplayError("manifest-size-or-type")
    if type(expected_manifest_sha256) is not str or not _SHA256.fullmatch(expected_manifest_sha256):
        raise ReplayError("expected-manifest-digest-invalid")
    if _sha(manifest_bytes) != expected_manifest_sha256:
        raise ReplayError("manifest-digest-mismatch")
    stage_uuid = _validate_uuid(expected_stage_uuid, "expected-stage-uuid")
    if type(expected_source_revision) is not str or not _REVISION.fullmatch(expected_source_revision):
        raise ReplayError("expected-source-revision-invalid")
    expected_ids = _validate_expected_ids(expected_operation_ids)

    manifest = _parse_manifest(manifest_bytes)
    if manifest["schema"] != MANIFEST_SCHEMA:
        raise ReplayError("manifest-schema-unsupported")
    if _validate_uuid(manifest["stage_uuid"], "manifest-stage-uuid") != stage_uuid:
        raise ReplayError("stage-binding-mismatch")
    source_revision = manifest["source_revision"]
    if (
        type(source_revision) is not str
        or not _REVISION.fullmatch(source_revision)
        or source_revision != expected_source_revision
    ):
        raise ReplayError("source-revision-binding-mismatch")
    operations = manifest["operations"]
    if type(operations) is not list or len(operations) != len(expected_ids):
        raise ReplayError("operation-inventory-count")
    rows = [_validate_operation_row(row) for row in operations]
    observed_ids = [row["operation_id"] for row in rows]
    if len(observed_ids) != len(set(observed_ids)):
        raise ReplayError("operation-inventory-duplicate")
    if set(observed_ids) != set(expected_ids):
        raise ReplayError("operation-inventory-mismatch")
    if tuple(observed_ids) != expected_ids:
        raise ReplayError("operation-inventory-order")

    # Compile and bind every request before reading any archived response bytes.
    compiled_by_id = {}
    inventory = []
    for row in rows:
        try:
            compiled = coverage.compile_request(
                query=row["query"],
                purpose=row["purpose"],
                facets=row["facets"],
                candidates=row["candidates"],
                incumbent_order=row["incumbent_order"],
            )
        except coverage.CoverageError as exc:
            raise ReplayError("operation-input-invalid") from exc
        if _sha(compiled.body) != row["request_body_sha256"]:
            raise ReplayError("request-body-binding-mismatch")
        bindings = {
            "stage_uuid": stage_uuid,
            "operation_id": row["operation_id"],
            "request_body_sha256": row["request_body_sha256"],
            "source_revision": source_revision,
        }
        compiled_by_id[row["operation_id"]] = compiled
        inventory.append({"bindings": bindings, "receipt_sha256": row["receipt_sha256"]})

    # The exact archive inventory is checked once, before any parser/selector runs.
    try:
        replayed = receipts.verify_inventory(archive_root, inventory)
    except receipts.ReceiptError as exc:
        raise ReplayError("response-archive-invalid") from exc

    results = []
    for row, (_receipt, raw_body) in zip(rows, replayed, strict=True):
        compiled = compiled_by_id[row["operation_id"]]
        try:
            parsed = coverage.parse_response(raw_body, compiled)
            ranking = coverage.select(compiled, parsed)
        except coverage.CoverageError:
            ranking = coverage.RankingResult(compiled.incumbent_order, "fallback", "invalid_response", {})
        results.append(
            {
                "operation_id": row["operation_id"],
                "status": ranking.status,
                "reason": ranking.reason,
                "ordered_ids": list(ranking.ordered_ids),
                "audit": ranking.audit,
                "receipt_sha256": row["receipt_sha256"],
            }
        )

    result_doc = {
        "schema": "intent-ranking-offline-replay-result/1",
        "status": "diagnostic_only",
        "stage_uuid": stage_uuid,
        "source_revision": source_revision,
        "manifest_sha256": expected_manifest_sha256,
        "results": results,
    }
    result_sha256 = _sha(_canonical(result_doc))
    if expected_result_sha256 is not None:
        if (
            type(expected_result_sha256) is not str
            or not _SHA256.fullmatch(expected_result_sha256)
            or result_sha256 != expected_result_sha256
        ):
            raise ReplayError("result-digest-mismatch")
    return {**result_doc, "result_sha256": result_sha256}
