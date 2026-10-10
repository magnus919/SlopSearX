"""Pure, offline registration and accounting primitives for coverage studies.

This module verifies caller-supplied external pins. It does not read files,
environment variables, credentials, clocks, networks, or providers, and it
never creates admission or execution authority.
"""

from __future__ import annotations

import hashlib
import json
import math
import re
import uuid
from dataclasses import dataclass
from typing import Iterable, Mapping

from scripts import intent_ranking_coverage as coverage

REGISTRATION_SCHEMA = "coverage-study-stage-registration/1"
INPUT_SCHEMA = "coverage-study-input-manifest/1"
ACQUISITION_SCHEMA = "coverage-study-acquisition-plan/1"
CAPTURE_SCHEMA = "coverage-study-capture-plan/1"
DEVELOPMENT_RECEIPT_SCHEMA = "coverage-study-development-gate-receipt/1"

MAX_MANIFEST_BYTES = 64_000_000
MAX_MATERIAL_BYTES = 64_000_000
MAX_CALLS_PER_STAGE = 44  # frozen EXP100 ceiling; this schedule uses at most 39
MAX_INPUT_TOTAL = 3_000_000
MAX_OUTPUT_TOTAL = 256_000
RESERVE_INPUT = 128_000
RESERVE_OUTPUT = 32_000
MAX_CALL_INPUT = 128_000
MAX_CALL_OUTPUT = 32_000
MAX_ACQUISITION_CALLS = 53
MAX_SCRAPER_CALLS = 640
HEALTH_CALLS = 1

NAVIGATION_SKIP_REASONS = frozenset({"production-arity-lt-2", "production-sensitive-query", "production-too-long"})
_SHA256 = re.compile(r"[0-9a-f]{64}\Z")
_GIT_SHA = re.compile(r"[0-9a-f]{40}\Z")
_OPERATION_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,63}\Z")
_TASK_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,95}\Z")
PIN_NAMES = (
    "protocol",
    "qualified_source_closure",
    "coverage_source",
    "production_rerank_source",
    "dependency_lock",
    "task_input_manifest",
    "source_capture_manifest",
    "reference_manifest",
    "answer_assessment_plan",
    "acquisition_plan",
    "capture_plan",
)
ENGINE_LIMITS = {"wikipedia": 2, "arxiv": 2, "github": 1, "openalex": 1}
TERMINAL_STATES = frozenset(
    {
        "complete-success",
        "complete-invalid-response",
        "transport-or-http-failure",
        "partial-or-interrupted",
        "preflight-budget-stop",
        "preflight-integrity-stop",
        "not-invoked-after-terminal-stop",
    }
)
CALL_RESULT_STATES = frozenset(
    {"complete-success", "complete-invalid-response", "transport-or-http-failure", "partial-or-interrupted"}
)


class StudyError(ValueError):
    """Invalid external registration, schedule, usage, or terminal state."""


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _strict_json(raw: bytes, label: str) -> object:
    if type(raw) is not bytes or len(raw) > MAX_MANIFEST_BYTES:
        raise StudyError(f"{label}-size-or-type")

    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise StudyError(f"{label}-duplicate-key")
            result[key] = value
        return result

    def reject_constant(value):
        raise StudyError(f"{label}-nonfinite:{value}")

    def finite_float(value):
        result = float(value)
        if not math.isfinite(result):
            raise StudyError(f"{label}-nonfinite")
        return result

    try:
        return json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=pairs,
            parse_constant=reject_constant,
            parse_float=finite_float,
        )
    except StudyError:
        raise
    except (UnicodeDecodeError, ValueError, RecursionError) as exc:
        raise StudyError(f"{label}-json-invalid") from exc


def _canonical(value: object) -> bytes:
    try:
        return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode(
            "utf-8"
        )
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise StudyError("canonical-json-invalid") from exc


def _sha256(value: object, label: str) -> str:
    if type(value) is not str or not _SHA256.fullmatch(value):
        raise StudyError(f"{label}-sha256-invalid")
    return value


def _canonical_uuid(value: object, label: str) -> str:
    if type(value) is not str:
        raise StudyError(f"{label}-type")
    try:
        parsed = uuid.UUID(value)
    except ValueError as exc:
        raise StudyError(f"{label}-invalid") from exc
    if str(parsed) != value:
        raise StudyError(f"{label}-noncanonical")
    return value


def _task_ids(values: object, label: str, *, expected_count: int | None = None) -> tuple[str, ...]:
    if type(values) is not list or not values or (expected_count is not None and len(values) != expected_count):
        raise StudyError(f"{label}-shape")
    if any(type(item) is not str or not _TASK_ID.fullmatch(item) for item in values):
        raise StudyError(f"{label}-invalid")
    if len(values) != len(set(values)):
        raise StudyError(f"{label}-duplicate")
    return tuple(values)


@dataclass(frozen=True)
class PlannedCall:
    operation_id: str
    family: str


def selector_inventory(navigation_skip_reasons: Iterable[str | None]) -> tuple[PlannedCall, ...]:
    """Construct the fixed two-neutral, 8-base, four-repeat/rotate, 5-nav schedule."""
    skips = tuple(navigation_skip_reasons)
    if len(skips) != 5:
        raise StudyError("navigation-inventory-count")
    if any(
        reason is not None and (type(reason) is not str or reason not in NAVIGATION_SKIP_REASONS) for reason in skips
    ):
        raise StudyError("navigation-skip-not-production-guard")

    calls = [
        PlannedCall("neutral-w0-first40", "neutral-readiness"),
        PlannedCall("neutral-candidate-80", "neutral-readiness"),
    ]
    for task in range(1, 9):
        calls.extend(
            (
                PlannedCall(f"research-{task:02d}-base-w0", "research-base"),
                PlannedCall(f"research-{task:02d}-base-candidate", "research-base"),
            )
        )
    # The frozen stability cases are 1, 4, 7, and 8; do not replace with 1..4.
    for task in (1, 4, 7, 8):
        for variant in ("repeat", "rotate"):
            calls.extend(
                (
                    PlannedCall(f"research-{task:02d}-{variant}-w0", "research-variant"),
                    PlannedCall(f"research-{task:02d}-{variant}-candidate", "research-variant"),
                )
            )
    for task, skip_reason in enumerate(skips, start=1):
        if skip_reason is None:
            calls.append(PlannedCall(f"navigation-{task:02d}-w0", "navigation"))

    ids = [call.operation_id for call in calls]
    if len(ids) != len(set(ids)) or len(ids) > 39 or len(ids) > MAX_CALLS_PER_STAGE:
        raise StudyError("selector-inventory-invalid")
    return tuple(calls)


def _parse_registration(raw: bytes) -> dict:
    value = _strict_json(raw, "registration")
    required = {
        "schema",
        "stage_uuid",
        "stage_kind",
        "source_revision",
        "pins",
        "operation_ids",
        "navigation_skip_reasons",
        "task_ids",
    }
    if type(value) is not dict or set(value) != required:
        raise StudyError("registration-schema")
    if value["schema"] != REGISTRATION_SCHEMA:
        raise StudyError("registration-schema-version")
    return value


@dataclass(frozen=True)
class PreparedStage:
    """A verified input summary only; never an execution or admission token."""

    stage_uuid: str
    stage_kind: str
    source_revision: str
    registration_sha256: str
    pins: Mapping[str, str]
    navigation_skip_reasons: tuple[str | None, ...]
    operation_ids: tuple[str, ...]
    acquisition_calls: int
    scraper_calls: int
    health_calls: int
    status: str = "preflight-verified-not-admitted"
    fresh_execution_authorized: bool = False
    selector_input_map_required: bool = False
    selector_input_map_schema: str | None = None


def _verify_input_manifest(raw: bytes, *, stage_uuid: str, expected_tasks: tuple[str, ...]) -> None:
    doc = _strict_json(raw, "input-manifest")
    if type(doc) is not dict or set(doc) != {
        "schema",
        "stage_uuid",
        "research_tasks",
        "navigation_tasks",
    }:
        raise StudyError("input-manifest-schema")
    if doc["schema"] != INPUT_SCHEMA:
        raise StudyError("input-manifest-version")
    if _canonical_uuid(doc["stage_uuid"], "input-stage-uuid") != stage_uuid:
        raise StudyError("input-stage-mismatch")
    research_ids, navigation_ids = expected_tasks[:8], expected_tasks[8:]
    research_tasks = doc["research_tasks"]
    navigation_tasks = doc["navigation_tasks"]
    if type(research_tasks) is not list or len(research_tasks) != 8:
        raise StudyError("input-research-task-count")
    if type(navigation_tasks) is not list or len(navigation_tasks) != 5:
        raise StudyError("input-navigation-task-count")

    pool_sizes = []
    for expected_id, row in zip(research_ids, research_tasks, strict=True):
        required = {
            "task_id",
            "query",
            "purpose",
            "facets",
            "candidates",
            "incumbent_order",
            "compiled_request_sha256",
        }
        if type(row) is not dict or set(row) != required or row["task_id"] != expected_id:
            raise StudyError("input-research-task-shape")
        expected_request_sha = _sha256(row["compiled_request_sha256"], "compiled-request")
        try:
            compiled = coverage.compile_request(
                query=row["query"],
                purpose=row["purpose"],
                facets=row["facets"],
                candidates=row["candidates"],
                incumbent_order=row["incumbent_order"],
            )
        except coverage.CoverageError as exc:
            raise StudyError("input-research-task-invalid") from exc
        if _sha(compiled.body) != expected_request_sha:
            raise StudyError("input-compiled-request-mismatch")
        pool_sizes.append(len(compiled.candidates))
    if sum(size >= 41 for size in pool_sizes) < 4:
        raise StudyError("input-large-pool-minimum")

    for expected_id, row in zip(navigation_ids, navigation_tasks, strict=True):
        required = {"task_id", "query", "target_candidate_id", "pool_ids", "w0_order"}
        if type(row) is not dict or set(row) != required or row["task_id"] != expected_id:
            raise StudyError("input-navigation-task-shape")
        query = row["query"]
        if type(query) is not str or not query.strip():
            raise StudyError("input-navigation-query-invalid")
        try:
            query_bytes = query.encode("utf-8")
        except UnicodeEncodeError as exc:
            raise StudyError("input-navigation-query-invalid") from exc
        if len(query_bytes) > 4096:
            raise StudyError("input-navigation-query-invalid")
        pool = row["pool_ids"]
        order = row["w0_order"]
        target = row["target_candidate_id"]
        if type(pool) is not list or not pool or any(type(item) is not str for item in pool):
            raise StudyError("input-navigation-pool-invalid")
        if (
            len(pool) != len(set(pool))
            or type(order) is not list
            or len(order) != len(pool)
            or any(type(item) is not str for item in order)
        ):
            raise StudyError("input-navigation-order-invalid")
        if len(set(order)) != len(order) or set(order) != set(pool):
            raise StudyError("input-navigation-order-invalid")
        if type(target) is not str or target not in pool:
            raise StudyError("input-navigation-target-invalid")


def _verify_stage_bound_manifest(raw: bytes, *, label: str, stage_uuid: str) -> None:
    doc = _strict_json(raw, label)
    if type(doc) is not dict or type(doc.get("schema")) is not str:
        raise StudyError(f"{label}-schema")
    if _canonical_uuid(doc.get("stage_uuid"), f"{label}-stage-uuid") != stage_uuid:
        raise StudyError(f"{label}-stage-mismatch")


def _verify_qualified_source_closure(
    raw: bytes,
    *,
    expected_revision: str,
    expected_pins: Mapping[str, str],
) -> None:
    doc = _strict_json(raw, "qualified-source-closure")
    required = {"schema", "source_revision", "material_pins"}
    if type(doc) is not dict or set(doc) != required:
        raise StudyError("qualified-source-closure-schema")
    if doc["schema"] != "coverage-study-qualified-source-closure/1":
        raise StudyError("qualified-source-closure-version")
    if doc["source_revision"] != expected_revision:
        raise StudyError("qualified-source-revision-mismatch")
    source_keys = {"coverage_source", "production_rerank_source", "dependency_lock"}
    pins = doc["material_pins"]
    if type(pins) is not dict or set(pins) != source_keys:
        raise StudyError("qualified-source-material-pin-shape")
    if any(pins[key] != expected_pins[key] for key in source_keys):
        raise StudyError("qualified-source-material-pin-mismatch")


def validate_acquisition_plan(
    raw: bytes,
    *,
    expected_sha256: str,
    expected_stage_uuid: str,
    expected_task_ids: tuple[str, ...],
) -> int:
    """Verify exact normalized per-task adapter-call rows and the 53-call cap."""
    expected_sha256 = _sha256(expected_sha256, "acquisition-plan")
    if _sha(raw) != expected_sha256:
        raise StudyError("acquisition-plan-digest-mismatch")
    doc = _strict_json(raw, "acquisition-plan")
    if type(doc) is not dict or set(doc) != {"schema", "stage_uuid", "tasks"}:
        raise StudyError("acquisition-plan-schema")
    if doc["schema"] != ACQUISITION_SCHEMA:
        raise StudyError("acquisition-plan-version")
    if _canonical_uuid(doc["stage_uuid"], "acquisition-stage-uuid") != expected_stage_uuid:
        raise StudyError("acquisition-stage-mismatch")
    tasks = doc["tasks"]
    if type(tasks) is not list or len(tasks) != len(expected_task_ids):
        raise StudyError("acquisition-task-count")
    observed: list[str] = []
    total = 0
    for row in tasks:
        if type(row) is not dict or set(row) != {"task_id", "engine_calls"}:
            raise StudyError("acquisition-task-schema")
        task_id = row["task_id"]
        if type(task_id) is not str or not _TASK_ID.fullmatch(task_id):
            raise StudyError("acquisition-task-id")
        observed.append(task_id)
        calls = row["engine_calls"]
        if type(calls) is not dict or set(calls) != set(ENGINE_LIMITS):
            raise StudyError("acquisition-engine-names")
        for engine, maximum in ENGINE_LIMITS.items():
            count = calls[engine]
            if type(count) is not int or not 0 <= count <= maximum:
                raise StudyError("acquisition-per-task-limit")
            total += count
    if len(observed) != len(set(observed)) or tuple(observed) != expected_task_ids:
        raise StudyError("acquisition-task-inventory")
    if total > MAX_ACQUISITION_CALLS:
        raise StudyError("acquisition-stage-limit")
    return total


def validate_capture_plan(
    raw: bytes,
    *,
    expected_sha256: str,
    expected_stage_uuid: str,
    expected_task_ids: tuple[str, ...],
) -> int:
    """Verify per-task scraper call plan (<=640) and one distinct health request."""
    expected_sha256 = _sha256(expected_sha256, "capture-plan")
    if _sha(raw) != expected_sha256:
        raise StudyError("capture-plan-digest-mismatch")
    doc = _strict_json(raw, "capture-plan")
    if type(doc) is not dict or set(doc) != {"schema", "stage_uuid", "tasks", "health_calls"}:
        raise StudyError("capture-plan-schema")
    if doc["schema"] != CAPTURE_SCHEMA:
        raise StudyError("capture-plan-version")
    if _canonical_uuid(doc["stage_uuid"], "capture-stage-uuid") != expected_stage_uuid:
        raise StudyError("capture-stage-mismatch")
    if type(doc["health_calls"]) is not int or doc["health_calls"] != HEALTH_CALLS:
        raise StudyError("capture-health-call-count")
    tasks = doc["tasks"]
    if type(tasks) is not list or len(tasks) != len(expected_task_ids):
        raise StudyError("capture-task-count")
    observed: list[str] = []
    total = 0
    for row in tasks:
        if type(row) is not dict or set(row) != {"task_id", "scraper_calls"}:
            raise StudyError("capture-task-schema")
        task_id, count = row["task_id"], row["scraper_calls"]
        if type(task_id) is not str or not _TASK_ID.fullmatch(task_id):
            raise StudyError("capture-task-id")
        if type(count) is not int or count < 0:
            raise StudyError("capture-call-count")
        observed.append(task_id)
        total += count
    if len(observed) != len(set(observed)) or tuple(observed) != expected_task_ids:
        raise StudyError("capture-task-inventory")
    if total > MAX_SCRAPER_CALLS:
        raise StudyError("capture-stage-limit")
    return total


def _verify_development_receipt(
    raw: bytes,
    *,
    expected_sha256: str,
    current_stage_uuid: str,
    expected_development_stage_uuid: str,
    forbidden_stage_uuids: tuple[str, ...],
    protocol_sha256: str,
    source_revision: str,
    source_closure_sha256: str,
) -> str:
    expected_sha256 = _sha256(expected_sha256, "development-receipt")
    if _sha(raw) != expected_sha256:
        raise StudyError("development-receipt-digest-mismatch")
    doc = _strict_json(raw, "development-receipt")
    keys = {
        "schema",
        "stage_uuid",
        "stage_kind",
        "status",
        "protocol_sha256",
        "source_revision",
        "source_closure_sha256",
        "result_sha256",
    }
    if type(doc) is not dict or set(doc) != keys:
        raise StudyError("development-receipt-schema")
    if doc["schema"] != DEVELOPMENT_RECEIPT_SCHEMA:
        raise StudyError("development-receipt-version")
    dev_uuid = _canonical_uuid(doc["stage_uuid"], "development-stage-uuid")
    if (
        dev_uuid != expected_development_stage_uuid
        or dev_uuid == current_stage_uuid
        or dev_uuid in forbidden_stage_uuids
        or doc["stage_kind"] != "development"
        or doc["status"] != "passed"
    ):
        raise StudyError("development-stage-not-passed")
    if _sha256(doc["protocol_sha256"], "development-protocol") != protocol_sha256:
        raise StudyError("development-protocol-mismatch")
    if doc["source_revision"] != source_revision:
        raise StudyError("development-source-revision-mismatch")
    if _sha256(doc["source_closure_sha256"], "development-source-closure") != source_closure_sha256:
        raise StudyError("development-source-closure-mismatch")
    _sha256(doc["result_sha256"], "development-result")
    return dev_uuid


def preflight_stage(
    *,
    registration_bytes: bytes,
    expected_registration_sha256: str,
    expected_stage_uuid: str,
    forbidden_stage_uuids: Iterable[str],
    expected_source_revision: str,
    observed_source_revision: str,
    materials: Mapping[str, bytes],
    expected_pins: Mapping[str, str],
    acquisition_plan_bytes: bytes,
    capture_plan_bytes: bytes,
    task_ids: tuple[str, ...],
    navigation_skip_reasons: Iterable[str | None],
    development_receipt_bytes: bytes | None = None,
    expected_development_receipt_sha256: str | None = None,
    expected_development_stage_uuid: str | None = None,
) -> PreparedStage:
    """Verify sealed registration and supplied bytes without granting authority.

    Expected hashes and source revision must come from an independent external
    seal/qualification path. A revision string alone is only a label. This
    function performs no file, Git, environment, credential, clock, or network
    access and always returns ``fresh_execution_authorized=False``.
    """
    if type(registration_bytes) is not bytes or len(registration_bytes) > MAX_MANIFEST_BYTES:
        raise StudyError("registration-size-or-type")
    expected_registration_sha256 = _sha256(expected_registration_sha256, "registration")
    if _sha(registration_bytes) != expected_registration_sha256:
        raise StudyError("registration-digest-mismatch")
    stage_uuid = _canonical_uuid(expected_stage_uuid, "expected-stage-uuid")
    forbidden = tuple(_canonical_uuid(value, "forbidden-stage-uuid") for value in forbidden_stage_uuids)
    if not forbidden or stage_uuid in forbidden or len(forbidden) != len(set(forbidden)):
        raise StudyError("stage-uuid-not-fresh")
    if type(expected_source_revision) is not str or not _GIT_SHA.fullmatch(expected_source_revision):
        raise StudyError("expected-source-revision-invalid")
    if type(observed_source_revision) is not str or observed_source_revision != expected_source_revision:
        raise StudyError("observed-source-revision-mismatch")
    if (
        type(task_ids) is not tuple
        or len(task_ids) != 13
        or any(type(value) is not str or not _TASK_ID.fullmatch(value) for value in task_ids)
    ):
        raise StudyError("expected-task-inventory-invalid")
    if len(set(task_ids)) != 13:
        raise StudyError("expected-task-inventory-duplicate")
    expected_navigation_skips = tuple(navigation_skip_reasons)

    registration = _parse_registration(registration_bytes)
    if _canonical_uuid(registration["stage_uuid"], "registration-stage-uuid") != stage_uuid:
        raise StudyError("registration-stage-mismatch")
    stage_kind = registration["stage_kind"]
    if stage_kind not in {"development", "confirmation"}:
        raise StudyError("stage-kind-invalid")
    if registration["source_revision"] != expected_source_revision:
        raise StudyError("registration-source-revision-mismatch")
    if _task_ids(registration["task_ids"], "registration-task-ids", expected_count=13) != tuple(task_ids):
        raise StudyError("registration-task-inventory-mismatch")

    if type(materials) is not dict or set(materials) != set(PIN_NAMES):
        raise StudyError("material-inventory-shape")
    if type(expected_pins) is not dict or set(expected_pins) != set(PIN_NAMES):
        raise StudyError("external-pin-inventory-shape")
    if type(registration["pins"]) is not dict or set(registration["pins"]) != set(PIN_NAMES):
        raise StudyError("registration-pin-inventory-shape")

    verified_pins: dict[str, str] = {}
    for name in PIN_NAMES:
        raw = materials[name]
        if type(raw) is not bytes or len(raw) > MAX_MATERIAL_BYTES:
            raise StudyError(f"material-{name}-size-or-type")
        external = _sha256(expected_pins[name], f"external-{name}")
        registered = _sha256(registration["pins"][name], f"registration-{name}")
        actual = _sha(raw)
        if registered != external or actual != external:
            raise StudyError(f"material-pin-mismatch:{name}")
        verified_pins[name] = actual

    _verify_qualified_source_closure(
        materials["qualified_source_closure"],
        expected_revision=expected_source_revision,
        expected_pins=verified_pins,
    )

    if materials["task_input_manifest"] != _canonical(_strict_json(materials["task_input_manifest"], "input-manifest")):
        # Input manifest bytes are canonical by contract; this avoids accepting
        # two byte encodings for a single registered task inventory.
        raise StudyError("input-manifest-not-canonical")
    selector_input_map_required = False
    selector_input_map_schema = None
    try:
        protocol_document = _strict_json(materials["protocol"], "protocol")
    except StudyError:
        # Pre-existing v1 test and replay fixtures may pin opaque protocol
        # bytes. Only a successfully parsed, explicitly versioned extension
        # changes the selector dispatch contract.
        protocol_document = None
    if type(protocol_document) is dict and "selector_input_map_schema" in protocol_document:
        selector_input_map_schema = protocol_document["selector_input_map_schema"]
        if selector_input_map_schema not in {
            "coverage-selector-input-map/2-draft",
            "coverage-selector-input-map/2-registered",
        }:
            raise StudyError("selector-input-map-schema-unsupported")
        selector_input_map_required = True
        if selector_input_map_schema.endswith("/2-registered") and protocol_document.get(
            "selector_input_map_status"
        ) != "registered":
            raise StudyError("selector-input-map-registration-status-required")
    _verify_input_manifest(materials["task_input_manifest"], stage_uuid=stage_uuid, expected_tasks=tuple(task_ids))
    for material_name in ("source_capture_manifest", "reference_manifest", "answer_assessment_plan"):
        _verify_stage_bound_manifest(materials[material_name], label=material_name, stage_uuid=stage_uuid)

    nav_skips = registration["navigation_skip_reasons"]
    if type(nav_skips) is not list or tuple(nav_skips) != expected_navigation_skips:
        raise StudyError("navigation-guard-binding-mismatch")
    operations = selector_inventory(expected_navigation_skips)
    registered_operation_ids = registration["operation_ids"]
    if type(registered_operation_ids) is not list or tuple(registered_operation_ids) != tuple(
        call.operation_id for call in operations
    ):
        raise StudyError("operation-inventory-mismatch")
    if any(type(value) is not str or not _OPERATION_ID.fullmatch(value) for value in registered_operation_ids):
        raise StudyError("operation-id-invalid")

    acquisition_calls = validate_acquisition_plan(
        acquisition_plan_bytes,
        expected_sha256=verified_pins["acquisition_plan"],
        expected_stage_uuid=stage_uuid,
        expected_task_ids=tuple(task_ids),
    )
    capture_calls = validate_capture_plan(
        capture_plan_bytes,
        expected_sha256=verified_pins["capture_plan"],
        expected_stage_uuid=stage_uuid,
        expected_task_ids=tuple(task_ids),
    )

    if stage_kind == "development":
        if (
            development_receipt_bytes is not None
            or expected_development_receipt_sha256 is not None
            or expected_development_stage_uuid is not None
        ):
            raise StudyError("development-receipt-unexpected")
    else:
        if (
            type(development_receipt_bytes) is not bytes
            or type(expected_development_receipt_sha256) is not str
            or type(expected_development_stage_uuid) is not str
        ):
            raise StudyError("confirmation-needs-sealed-development-pass")
        _verify_development_receipt(
            development_receipt_bytes,
            expected_sha256=expected_development_receipt_sha256,
            current_stage_uuid=stage_uuid,
            expected_development_stage_uuid=_canonical_uuid(
                expected_development_stage_uuid, "expected-development-stage-uuid"
            ),
            forbidden_stage_uuids=forbidden,
            protocol_sha256=verified_pins["protocol"],
            source_revision=expected_source_revision,
            source_closure_sha256=verified_pins["qualified_source_closure"],
        )

    return PreparedStage(
        stage_uuid=stage_uuid,
        stage_kind=stage_kind,
        source_revision=expected_source_revision,
        registration_sha256=expected_registration_sha256,
        pins=verified_pins,
        navigation_skip_reasons=expected_navigation_skips,
        operation_ids=tuple(call.operation_id for call in operations),
        acquisition_calls=acquisition_calls,
        scraper_calls=capture_calls,
        health_calls=HEALTH_CALLS,
        selector_input_map_required=selector_input_map_required,
        selector_input_map_schema=selector_input_map_schema,
    )


@dataclass(frozen=True)
class InventoryRow:
    operation_id: str
    state: str


@dataclass(frozen=True)
class StageClosure:
    status: str
    all_calls_complete: bool
    rows: tuple[InventoryRow, ...]
    input_tokens: int
    output_tokens: int


class StudyRun:
    """One-shot ordered token ledger plus strict terminal inventory."""

    def __init__(self, prepared: PreparedStage):
        if type(prepared) is not PreparedStage or prepared.fresh_execution_authorized is not False:
            raise StudyError("prepared-stage-required")
        self.prepared = prepared
        self.operation_ids = tuple(call.operation_id for call in selector_inventory(prepared.navigation_skip_reasons))
        if prepared.operation_ids != self.operation_ids:
            raise StudyError("prepared-operation-inventory-mismatch")
        if not 1 <= len(self.operation_ids) <= 39:
            raise StudyError("operation-inventory-count")
        if any(type(value) is not str or not _OPERATION_ID.fullmatch(value) for value in self.operation_ids):
            raise StudyError("operation-id-invalid")
        if len(self.operation_ids) != len(set(self.operation_ids)):
            raise StudyError("operation-inventory-duplicate")
        self.input_tokens = 0
        self.output_tokens = 0
        self.rows: list[InventoryRow] = []
        self.in_flight: str | None = None
        self.terminal_reason: str | None = None
        self._next = 0

    def begin(self, operation_id: str) -> None:
        if self.terminal_reason is not None:
            raise StudyError("stage-terminal")
        if self.in_flight is not None:
            self._terminal_integrity("operation-already-in-flight")
        if self._next >= len(self.operation_ids) or self.operation_ids[self._next] != operation_id:
            self._terminal_integrity("operation-order-mismatch")
        if self.input_tokens + RESERVE_INPUT > MAX_INPUT_TOTAL:
            self._terminal_before_call("input-reserve-failed")
        if self.output_tokens + RESERVE_OUTPUT > MAX_OUTPUT_TOTAL:
            self._terminal_before_call("output-reserve-failed")
        self.in_flight = operation_id

    def complete(
        self,
        operation_id: str,
        *,
        input_tokens: object,
        output_tokens: object,
        response_state: str = "complete-success",
    ) -> None:
        self._require_active(operation_id)
        if type(input_tokens) is not int or type(output_tokens) is not int:
            self._terminal_current("unknown-or-invalid-usage", "partial-or-interrupted")
        if not (0 <= input_tokens <= MAX_CALL_INPUT and 0 <= output_tokens <= MAX_CALL_OUTPUT):
            self._terminal_current("per-call-usage-limit", "partial-or-interrupted")
        if response_state not in {"complete-success", "complete-invalid-response"}:
            self._terminal_current("response-state-invalid", "partial-or-interrupted")
        next_input = self.input_tokens + input_tokens
        next_output = self.output_tokens + output_tokens
        if next_input > MAX_INPUT_TOTAL or next_output > MAX_OUTPUT_TOTAL:
            self._terminal_current("stage-usage-limit", "partial-or-interrupted")
        self.input_tokens = next_input
        self.output_tokens = next_output
        self.rows.append(InventoryRow(operation_id, response_state))
        self._next += 1
        self.in_flight = None
        if response_state != "complete-success":
            self._stop_after_record(operation_id, "invalid-complete-response")

    def external_failure(self, operation_id: str, *, state: str) -> None:
        self._require_active(operation_id)
        if state not in {"transport-or-http-failure", "partial-or-interrupted"}:
            self._terminal_current("external-failure-state-invalid", "partial-or-interrupted")
        self.rows.append(InventoryRow(operation_id, state))
        self._next += 1
        self.in_flight = None
        self._stop_after_record(operation_id, "external-failure")

    def close(self) -> StageClosure:
        if self.in_flight is not None:
            self._terminal_current("close-with-call-in-flight", "partial-or-interrupted")
        if self.terminal_reason is None and self._next != len(self.operation_ids):
            self._terminal_integrity("schedule-incomplete")
        if len(self.rows) != len(self.operation_ids):
            raise StudyError("terminal-inventory-incomplete")
        if tuple(row.operation_id for row in self.rows) != self.operation_ids:
            raise StudyError("terminal-inventory-order")
        if any(row.state not in TERMINAL_STATES for row in self.rows):
            raise StudyError("terminal-state-invalid")
        successful = self.terminal_reason is None and all(row.state == "complete-success" for row in self.rows)
        return StageClosure(
            status="all-registered-calls-complete" if successful else "terminal-failure",
            all_calls_complete=successful,
            rows=tuple(self.rows),
            input_tokens=self.input_tokens,
            output_tokens=self.output_tokens,
        )

    def _require_active(self, operation_id: str) -> None:
        if self.terminal_reason is not None:
            raise StudyError("stage-terminal")
        if self.in_flight != operation_id or self._next >= len(self.operation_ids):
            self._terminal_integrity("operation-finish-mismatch")

    def _terminal_current(self, reason: str, state: str) -> None:
        operation_id = self.in_flight
        if operation_id is None:
            self._terminal_integrity(reason)
        self.rows.append(InventoryRow(operation_id, state))
        self._next += 1
        self.in_flight = None
        self._stop_after_record(operation_id, reason)

    def _terminal_before_call(self, reason: str) -> None:
        if self.in_flight is not None:
            self._terminal_current(reason, "partial-or-interrupted")
        if self._next < len(self.operation_ids):
            operation_id = self.operation_ids[self._next]
            self.rows.append(InventoryRow(operation_id, "preflight-budget-stop"))
            self._next += 1
        self.terminal_reason = reason
        self._append_uninvoked()
        raise StudyError(reason)

    def _terminal_integrity(self, reason: str) -> None:
        if self.in_flight is not None:
            operation_id = self.in_flight
            self.rows.append(InventoryRow(operation_id, "partial-or-interrupted"))
            self._next += 1
            self.in_flight = None
        elif self._next < len(self.operation_ids):
            operation_id = self.operation_ids[self._next]
            self.rows.append(InventoryRow(operation_id, "preflight-integrity-stop"))
            self._next += 1
        self.terminal_reason = reason
        self._append_uninvoked()
        raise StudyError(reason)

    def _stop_after_record(self, _operation_id: str, reason: str) -> None:
        self.terminal_reason = reason
        self._append_uninvoked()
        raise StudyError(reason)

    def _append_uninvoked(self) -> None:
        while self._next < len(self.operation_ids):
            self.rows.append(InventoryRow(self.operation_ids[self._next], "not-invoked-after-terminal-stop"))
            self._next += 1
