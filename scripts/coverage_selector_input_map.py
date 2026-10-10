"""Forward-only predispatch map for selector operation inputs.

This draft helper creates no permit and does not authorize provider calls. It
binds every scheduled operation to deterministic request inputs before the
selector phase; old registrations have no such map and remain unknown.
"""

from __future__ import annotations

import hashlib
import json
import re
from dataclasses import asdict
from pathlib import Path
from types import SimpleNamespace
from typing import Mapping, Sequence

from scripts import coverage_jev_execution as execution
from scripts import coverage_legacy_control as legacy
from scripts import coverage_live_acquire
from scripts import coverage_study_core as core
from slopsearx.adapter import SearchResult
from slopsearx.service import SearchService, _rerank_text, _rerank_url

DRAFT_SCHEMA = "coverage-selector-input-map/2-draft"
REGISTERED_SCHEMA = "coverage-selector-input-map/2-registered"
SCHEMA = DRAFT_SCHEMA
NEUTRAL_FIXTURE_SHA256 = "59e7622cdaf7381d4c0797d57e4bc00af6d5fea23153525e23e35351fa87f09f"
SHA256 = re.compile(r"[0-9a-f]{64}\Z")


class SelectorInputMapError(ValueError):
    """The frozen, predispatch operation input map is invalid."""


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _strict(raw: bytes) -> object:
    def pairs(items):
        value = {}
        for key, item in items:
            if key in value:
                raise SelectorInputMapError("duplicate-key")
            value[key] = item
        return value

    def reject_constant(value: str) -> None:
        raise ValueError(value)

    try:
        return json.loads(raw.decode("utf-8"), object_pairs_hook=pairs, parse_constant=reject_constant)
    except (UnicodeDecodeError, ValueError, RecursionError) as exc:
        raise SelectorInputMapError("json-invalid") from exc


def _canonical(value: object) -> bytes:
    return execution._canonical(value)


def _candidate_input(row: Mapping[str, object], *, candidates: list[dict[str, object]] | None = None) -> bytes:
    value = {
        "schema": "coverage-jev-operation-input/1",
        "query": row["query"],
        "purpose": row["purpose"],
        "facets": row["facets"],
        "candidates": list(row["candidates"]) if candidates is None else candidates,
        "incumbent_order": row["incumbent_order"],
    }
    return _canonical(value)


def _legacy_pool(rows: Sequence[Mapping[str, object]]) -> tuple[SearchResult, ...]:
    results = []
    for index, row in enumerate(rows):
        results.append(
            SearchResult(
                url=str(row["url"]),
                title=str(row["title"]),
                content=str(row["snippet"]),
                engine="registered-pool",
                position=index + 1,
            )
        )
    return tuple(results)


def _w0_input(
    *,
    operation_id: str,
    query: str,
    rows: Sequence[Mapping[str, object]],
    frozen_v1_source_bytes: bytes,
    current_service_source_bytes: bytes,
) -> tuple[bytes, bytes, dict[str, object]]:
    if not 2 <= len(rows) <= 40:
        raise SelectorInputMapError("w0-card-count-invalid")
    pool = _legacy_pool(rows)
    v1 = legacy._load_frozen_v1(frozen_v1_source_bytes)
    candidates = [
        asdict(
            v1.RerankCandidate(
                id=f"c{index}",
                title=_rerank_text(result.title, 256),
                url=_rerank_url(result.url),
                snippet=_rerank_text(result.content, 1200),
            )
        )
        for index, result in enumerate(pool)
    ]
    input_bytes = _canonical({"schema": "coverage-jev-operation-input/1", "query": query, "candidates": candidates})
    service = object.__new__(SearchService)
    service._ctx = SimpleNamespace(rerank_provider=None, ranking_strategy="presence")
    legacy_control = {
        "service": service,
        "query": query,
        "canonical_pool": pool,
        "frozen_v1_source_bytes": frozen_v1_source_bytes,
        "current_service_source_bytes": current_service_source_bytes,
    }
    request_body, compiled = execution.build_selector_request(
        operation_id=operation_id,
        operation_input_bytes=input_bytes,
        legacy_control=legacy_control,
    )
    if compiled is not None:
        raise SelectorInputMapError("w0-request-builder-returned-candidate")
    return input_bytes, request_body, legacy_control


def _candidate_request(operation_id: str, input_bytes: bytes) -> bytes:
    request_body, compiled = execution.build_selector_request(
        operation_id=operation_id, operation_input_bytes=input_bytes
    )
    if compiled is None:
        raise SelectorInputMapError("candidate-request-builder-returned-w0")
    return request_body


def build_selector_input_map(
    *,
    prepared: core.PreparedStage,
    task_input_manifest_bytes: bytes,
    pipeline_tasks: Sequence[Mapping[str, object]],
    navigation_targets: Sequence[Mapping[str, object]],
    snapshots_directory: str | Path,
    acquisition_snapshot_index_sha256: str,
    acquisition_manifest_bytes: bytes,
    acquisition_manifest_sha256: str,
    neutral_fixture_bytes: bytes,
    frozen_v1_source_bytes: bytes,
    current_service_source_bytes: bytes,
    expected_builder_source_sha256: str,
    operation_materials: dict[str, dict[str, object]] | None = None,
) -> bytes:
    """Rebuild every registered request input from pinned stage material.

    Call only after acquisition and late input preflight have closed. The
    returned canonical bytes must be durably recorded and externally admitted
    before the first selector request. ``prepared`` itself grants no authority.
    """
    if type(prepared) is not core.PreparedStage or prepared.fresh_execution_authorized is not False:
        raise SelectorInputMapError("prepared-summary-required")
    if (
        _sha(task_input_manifest_bytes) != prepared.pins.get("task_input_manifest")
        or _sha(frozen_v1_source_bytes) != prepared.pins.get("production_rerank_source")
        or _sha(acquisition_manifest_bytes) != acquisition_manifest_sha256
        or type(acquisition_snapshot_index_sha256) is not str
        or not SHA256.fullmatch(acquisition_snapshot_index_sha256)
        or type(acquisition_manifest_sha256) is not str
        or not SHA256.fullmatch(acquisition_manifest_sha256)
    ):
        raise SelectorInputMapError("input-map-source-pin-mismatch")
    acquisition_manifest = _strict(acquisition_manifest_bytes)
    if (
        type(acquisition_manifest) is not dict
        or acquisition_manifest.get("stage_uuid") != prepared.stage_uuid
        or acquisition_manifest.get("source_revision") != prepared.source_revision
        or acquisition_manifest.get("source_closure_sha256") != prepared.pins.get("qualified_source_closure")
        or acquisition_manifest.get("protocol_sha256") != prepared.pins.get("protocol")
    ):
        raise SelectorInputMapError("acquisition-manifest-binding")
    try:
        verified_snapshot = coverage_live_acquire.verify_pool_snapshot_index(
            snapshots_directory,
            expected_index_sha256=acquisition_snapshot_index_sha256,
            expected_stage_manifest_sha256=acquisition_manifest_sha256,
            expected_stage_uuid=prepared.stage_uuid,
            expected_source_revision=prepared.source_revision,
        )
    except Exception as exc:
        raise SelectorInputMapError("acquisition-snapshot-verification-failed") from exc
    index = verified_snapshot["index"]
    if (
        index.get("source_closure_sha256") != prepared.pins.get("qualified_source_closure")
        or index.get("protocol_sha256") != prepared.pins.get("protocol")
        or index.get("cohorts_sha256") != acquisition_manifest.get("cohorts_sha256")
        or index.get("acquisition_plan_sha256") != acquisition_manifest.get("acquisition_plan_sha256")
    ):
        raise SelectorInputMapError("acquisition-snapshot-source-binding")
    builder_sha = _sha(Path(__file__).read_bytes())
    if expected_builder_source_sha256 != builder_sha:
        raise SelectorInputMapError("builder-source-pin-mismatch")
    if _sha(neutral_fixture_bytes) != NEUTRAL_FIXTURE_SHA256:
        raise SelectorInputMapError("neutral-fixture-pin-mismatch")
    fixture = _strict(neutral_fixture_bytes)
    if (
        type(fixture) is not dict
        or fixture.get("schema") != "coverage-selector-readiness-input-fixtures/2-draft"
        or fixture.get("quality_credit") is not False
        or type(fixture.get("inputs")) is not dict
        or set(fixture["inputs"]) != {"neutral-w0-first40", "neutral-candidate-80"}
    ):
        raise SelectorInputMapError("neutral-fixture-shape-invalid")
    manifest = _strict(task_input_manifest_bytes)
    if (
        type(manifest) is not dict
        or manifest.get("schema") != core.INPUT_SCHEMA
        or manifest.get("stage_uuid") != prepared.stage_uuid
        or type(manifest.get("research_tasks")) is not list
        or type(manifest.get("navigation_tasks")) is not list
        or len(manifest["research_tasks"]) != 8
        or len(manifest["navigation_tasks"]) != 5
    ):
        raise SelectorInputMapError("task-input-manifest-invalid")
    if len(pipeline_tasks) != 8 or len(navigation_targets) != 5:
        raise SelectorInputMapError("pipeline-task-count-invalid")
    for row, task in zip(manifest["research_tasks"], pipeline_tasks, strict=True):
        if type(row) is not dict or type(task) is not dict:
            raise SelectorInputMapError("research-task-shape-invalid")
        task_id = row.get("task_id")
        if (
            type(task_id) is not str
            or row.get("query") != task.get("query")
            or row.get("purpose") != task.get("purpose")
            or row.get("facets") != task.get("facets")
            or row.get("candidates") != task.get("ranking_candidates")
            or row.get("incumbent_order") != task.get("native_order")
        ):
            raise SelectorInputMapError("research-task-input-binding")
        snapshot = verified_snapshot["operations"].get(task_id)
        operation = snapshot.get("operation") if type(snapshot) is dict else None
        response = operation.get("canonical_response") if type(operation) is dict else None
        results = response.get("results") if type(response) is dict else None
        if (
            type(results) is not list
            or type(snapshot.get("task_plan")) is not dict
            or snapshot["task_plan"].get("task_id") != task_id
            or response.get("query") != row.get("query")
            or len(results) != len(row["candidates"])
        ):
            raise SelectorInputMapError("research-snapshot-binding")
        snapshot_projections = [
            {
                "id": row["candidates"][index]["id"],
                "title": _rerank_text(result.get("title", ""), 256),
                "url": _rerank_url(result.get("url", "")),
                "snippet": _rerank_text(result.get("content", ""), 1200),
            }
            for index, result in enumerate(results)
            if type(result) is dict
        ]
        if snapshot_projections != row["candidates"]:
            raise SelectorInputMapError("research-snapshot-projection-mismatch")

    rows_by_id: dict[str, dict[str, object]] = {}

    def save_material(operation_id: str, input_bytes: bytes, legacy_control: dict[str, object] | None = None) -> None:
        if operation_materials is not None:
            operation_materials[operation_id] = {
                "operation_input_bytes": input_bytes,
                "legacy_control": legacy_control,
            }

    fixture_inputs = fixture["inputs"]
    for operation_id, input_value in fixture_inputs.items():
        if type(operation_id) is not str or type(input_value) is not dict:
            raise SelectorInputMapError("neutral-input-shape-invalid")
        input_bytes = _canonical(input_value)
        if operation_id == "neutral-w0-first40":
            inp, request_body, legacy_control = _w0_input(
                operation_id=operation_id,
                query=input_value["query"],
                rows=input_value["candidates"],
                frozen_v1_source_bytes=frozen_v1_source_bytes,
                current_service_source_bytes=current_service_source_bytes,
            )
        else:
            inp = input_bytes
            request_body = _candidate_request(operation_id, inp)
            legacy_control = None
        save_material(operation_id, inp, legacy_control)
        parser_mode = "original-v1" if "w0" in operation_id else "coverage"
        rows_by_id[operation_id] = _entry(operation_id, inp, request_body, parser_mode)

    for task_index, task in enumerate(manifest["research_tasks"], start=1):
        task_id = task["task_id"]
        candidates = task["candidates"]
        if type(candidates) is not list or len(candidates) < 2:
            raise SelectorInputMapError("research-candidate-count-invalid")
        base_candidate = _candidate_input(task)
        rotated = [*candidates[1:], candidates[0]]
        base_id = f"research-{task_index:02d}-base-candidate"
        save_material(base_id, base_candidate)
        rows_by_id[base_id] = _entry(base_id, base_candidate, _candidate_request(base_id, base_candidate), "coverage")

        projections = task["candidates"][:40]
        operation_id = f"research-{task_index:02d}-base-w0"
        inp, request_body, legacy_control = _w0_input(
            operation_id=operation_id,
            query=task["query"],
            rows=projections,
            frozen_v1_source_bytes=frozen_v1_source_bytes,
            current_service_source_bytes=current_service_source_bytes,
        )
        save_material(operation_id, inp, legacy_control)
        rows_by_id[operation_id] = _entry(operation_id, inp, request_body, "original-v1")
        if task_index in {1, 4, 7, 8}:
            for operation_id, input_bytes in (
                (f"research-{task_index:02d}-repeat-candidate", base_candidate),
                (f"research-{task_index:02d}-rotate-candidate", _candidate_input(task, candidates=rotated)),
            ):
                save_material(operation_id, input_bytes)
                rows_by_id[operation_id] = _entry(
                    operation_id, input_bytes, _candidate_request(operation_id, input_bytes), "coverage"
                )
            operation_id = f"research-{task_index:02d}-repeat-w0"
            inp, request_body, legacy_control = _w0_input(
                operation_id=operation_id,
                query=task["query"],
                rows=projections,
                frozen_v1_source_bytes=frozen_v1_source_bytes,
                current_service_source_bytes=current_service_source_bytes,
            )
            save_material(operation_id, inp, legacy_control)
            rows_by_id[operation_id] = _entry(operation_id, inp, request_body, "original-v1")
            rotated_w0 = [*projections[1:], projections[0]]
            operation_id = f"research-{task_index:02d}-rotate-w0"
            inp, request_body, legacy_control = _w0_input(
                operation_id=operation_id,
                query=task["query"],
                rows=rotated_w0,
                frozen_v1_source_bytes=frozen_v1_source_bytes,
                current_service_source_bytes=current_service_source_bytes,
            )
            save_material(operation_id, inp, legacy_control)
            rows_by_id[operation_id] = _entry(operation_id, inp, request_body, "original-v1")
            operation_id = f"research-{task_index:02d}-rotate-candidate"
            input_bytes = _candidate_input(task, candidates=rotated)
            save_material(operation_id, input_bytes)
            rows_by_id[operation_id] = _entry(
                operation_id, input_bytes, _candidate_request(operation_id, input_bytes), "coverage"
            )

    targets_by_id = {row["target_id"]: row for row in navigation_targets}
    if len(targets_by_id) != len(navigation_targets):
        raise SelectorInputMapError("navigation-target-duplicate")
    for task_index, nav in enumerate(manifest["navigation_tasks"], start=1):
        operation_id = f"navigation-{task_index:02d}-w0"
        if operation_id not in prepared.operation_ids:
            continue
        target_id = nav.get("task_id")
        matching = [row for row in navigation_targets if row.get("target_id") == target_id]
        if len(matching) != 1:
            raise SelectorInputMapError("navigation-target-binding")
        target = matching[0]
        snapshot = verified_snapshot["operations"].get(target_id)
        operation = snapshot.get("operation") if type(snapshot) is dict else None
        response = operation.get("canonical_response") if type(operation) is dict else None
        results = response.get("results") if type(response) is dict else None
        pool_ids = nav.get("pool_ids")
        w0_order = nav.get("w0_order")
        if type(results) is not list or type(w0_order) is not list:
            raise SelectorInputMapError("navigation-snapshot-missing")
        result_rows = list(results)
        if (
            type(pool_ids) is not list
            or len(result_rows) != len(pool_ids)
            or set(w0_order) != set(pool_ids)
            or response.get("query") != nav.get("query")
        ):
            raise SelectorInputMapError("navigation-pool-binding")
        if nav.get("query") != target.get("search_query"):
            raise SelectorInputMapError("navigation-query-binding")
        by_id = {}
        for index, result in enumerate(result_rows):
            if type(result) is not dict:
                raise SelectorInputMapError("navigation-result-shape")
            card_id = pool_ids[index]
            by_id[card_id] = {
                "id": card_id,
                "title": _rerank_text(result.get("title", ""), 256),
                "url": _rerank_url(result.get("url", "")),
                "snippet": _rerank_text(result.get("content", ""), 1200),
            }
        ordered = [by_id[card_id] for card_id in w0_order]
        inp, request_body, legacy_control = _w0_input(
            operation_id=operation_id,
            query=nav["query"],
            rows=ordered,
            frozen_v1_source_bytes=frozen_v1_source_bytes,
            current_service_source_bytes=current_service_source_bytes,
        )
        save_material(operation_id, inp, legacy_control)
        rows_by_id[operation_id] = _entry(operation_id, inp, request_body, "original-v1")

    if set(rows_by_id) != set(prepared.operation_ids):
        raise SelectorInputMapError("operation-input-inventory-mismatch")
    if operation_materials is not None and set(operation_materials) != set(prepared.operation_ids):
        raise SelectorInputMapError("operation-material-inventory-mismatch")
    ordered_entries = [rows_by_id[operation_id] for operation_id in prepared.operation_ids]
    document = {
        "schema": prepared.selector_input_map_schema or SCHEMA,
        # The registration mode is externally pinned, but the map itself is
        # only a deterministic predispatch artifact and never grants authority.
        "status": (
            "complete-awaiting-external-admission"
            if prepared.selector_input_map_schema == REGISTERED_SCHEMA
            else "draft-unadmitted"
        ),
        "stage_uuid": prepared.stage_uuid,
        "source_revision": prepared.source_revision,
        "original_registration_sha256": prepared.registration_sha256,
        "coverage_source_sha256": prepared.pins["coverage_source"],
        "production_rerank_source_sha256": prepared.pins["production_rerank_source"],
        "execution_source_sha256": _sha(Path(execution.__file__).read_bytes()),
        "legacy_control_source_sha256": _sha(Path(legacy.__file__).read_bytes()),
        "current_service_source_sha256": _sha(current_service_source_bytes),
        "acquisition_snapshot_index_sha256": acquisition_snapshot_index_sha256,
        "acquisition_manifest_sha256": acquisition_manifest_sha256,
        "task_input_manifest_sha256": _sha(task_input_manifest_bytes),
        "neutral_fixture_sha256": _sha(neutral_fixture_bytes),
        "builder_source_sha256": builder_sha,
        "operations": ordered_entries,
    }
    return _canonical(document)


def _entry(operation_id: str, input_bytes: bytes, request_body: bytes, parser_mode: str) -> dict[str, object]:
    return {
        "operation_id": operation_id,
        "parser_mode": parser_mode,
        "operation_input_sha256": _sha(input_bytes),
        "request_body_sha256": _sha(request_body),
    }


def verify_selector_input_map(
    *,
    input_map_bytes: bytes,
    expected_sha256: str,
    **build_kwargs,
) -> dict[str, object]:
    """Recompute the full map from its pinned inputs before accepting it."""
    if type(input_map_bytes) is not bytes or _sha(input_map_bytes) != expected_sha256:
        raise SelectorInputMapError("input-map-digest-mismatch")
    expected = build_selector_input_map(**build_kwargs)
    if expected != input_map_bytes:
        raise SelectorInputMapError("input-map-recomputation-mismatch")
    value = _strict(input_map_bytes)
    if type(value) is not dict:
        raise SelectorInputMapError("input-map-shape-invalid")
    return value
