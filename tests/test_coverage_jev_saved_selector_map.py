from __future__ import annotations

import hashlib
import json
from dataclasses import replace
from pathlib import Path
from unittest import mock

import pytest

from scripts import coverage_jev_execution as execution
from scripts import coverage_operator_runner as runner
from scripts import coverage_pipeline_inputs as pipeline
from scripts import coverage_saved_corpus as saved
from scripts import coverage_selector_admission as admission
from scripts import coverage_selector_input_map as input_map
from scripts import coverage_study_core as core
from tests import test_coverage_saved_corpus as saved_fixtures
from tests import test_coverage_selector_input_map as map_fixtures
from tests.test_coverage_jev_execution import permit_for


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode() + b"\n"


def _saved_schedule(tmp_path: Path):
    kwargs = map_fixtures._materials(tmp_path)
    make_stage = saved_fixtures.fixtures.make_stage

    def make_consistent_stage(cases):
        stage = make_stage(cases)
        for index, operation in enumerate(stage.operations[8:], start=1):
            operation.query = f"synthetic nav {index}"
            operation.canonical_response.query = operation.query
            operation.canonical_response.results.append(
                saved_fixtures.fixtures.result(
                    index,
                    namespace="navigation-extra",
                    url=f"https://github.com/example/extra/{index}",
                )
            )
            operation.pool_count = len(operation.canonical_response.results)
        return stage

    with mock.patch.object(saved_fixtures.fixtures, "make_stage", make_consistent_stage):
        origin, cases, navigation, _collection, _capture = saved_fixtures._origin_package(tmp_path)
    protocol_bytes = _canonical(
        {
            "schema": "coverage-first-study-protocol/2",
            "selector_input_map_schema": "coverage-selector-input-map/2-registered",
            "selector_input_map_status": "registered",
            "pool_input_mode_by_stage": {
                "development": "saved-corpus/1",
                "confirmation": "frozen-pool/1",
            },
        }
    )
    prepared = replace(
        kwargs["prepared"],
        pins={**kwargs["prepared"].pins, "protocol": _sha(protocol_bytes)},
        selector_input_map_required=True,
        selector_input_map_schema="coverage-selector-input-map/2-registered",
    )
    binding = replace(
        saved_fixtures._current_binding(origin),
        stage_uuid=prepared.stage_uuid,
        stage_kind=prepared.stage_kind,
        source_revision=prepared.source_revision,
        source_closure_sha256=prepared.pins["qualified_source_closure"],
        protocol_sha256=_sha(protocol_bytes),
        cohorts_sha256=origin.registration.cohorts_sha256,
    )
    imported = saved.verify_and_import_saved_corpus(origin, binding, cases, navigation)
    pool = imported.origin_pool_verified
    # Derive a matching core task manifest and pipeline projection from the
    # saved origin cohort and its verified raw pool snapshots.
    research_tasks = []
    pipeline_tasks = []
    for case in cases:
        task = pipeline._cohort_task(case)
        artifact = pool.operations[case["task_id"]]
        results = artifact["operation"]["canonical_response"]["results"]
        candidates = [
            {
                "id": f"c{index}",
                "title": pipeline._rerank_text(result["title"], 256),
                "url": pipeline._rerank_url(result["url"]),
                "snippet": pipeline._rerank_text(result["content"], 1200),
            }
            for index, result in enumerate(results)
        ]
        incumbent_order = [candidate["id"] for candidate in candidates]
        compiled = core.coverage.compile_request(
            query=task["query"],
            purpose=task["purpose"],
            facets=task["facets"],
            candidates=candidates,
            incumbent_order=incumbent_order,
        )
        research_tasks.append(
            {
                "task_id": task["task_id"],
                "query": task["query"],
                "purpose": task["purpose"],
                "facets": task["facets"],
                "candidates": candidates,
                "incumbent_order": incumbent_order,
                "compiled_request_sha256": _sha(compiled.body),
            }
        )
        pipeline_tasks.append(
            {
                "task_id": task["task_id"],
                "query": task["query"],
                "purpose": task["purpose"],
                "facets": task["facets"],
                "ranking_candidates": candidates,
                "native_order": incumbent_order,
            }
        )
    navigation_tasks = []
    for target in navigation:
        artifact = pool.operations[target["target_id"]]
        native_order = artifact["native_order"]
        navigation_tasks.append(
            {
                "task_id": target["target_id"],
                "query": target["search_query"],
                "target_candidate_id": native_order[0],
                "pool_ids": native_order,
                "w0_order": native_order,
            }
        )
    task_input_manifest_bytes = core._canonical(
        {
            "schema": core.INPUT_SCHEMA,
            "stage_uuid": prepared.stage_uuid,
            "research_tasks": research_tasks,
            "navigation_tasks": navigation_tasks,
        }
    )
    prepared = replace(
        prepared,
        pins={**prepared.pins, "task_input_manifest": _sha(task_input_manifest_bytes)},
    )
    initial_registration_sha256 = "a" * 64
    band_companion_sha256 = "b" * 64
    preacquisition_input = {
        "schema": "coverage-preacquisition-input-manifest/1",
        "stage_uuid": prepared.stage_uuid,
        "protocol_sha256": prepared.pins["protocol"],
        "cohorts_sha256": binding.cohorts_sha256,
        "task_ids": [row["task_id"] for row in cases] + [row["target_id"] for row in navigation],
        "band_companion_sha256": band_companion_sha256,
        "initial_registration_sha256": initial_registration_sha256,
    }
    import_plan = {
        "schema": "coverage-saved-import-plan/1",
        "stage": prepared.stage_kind,
        "stage_uuid": prepared.stage_uuid,
        "source_revision": prepared.source_revision,
        "source_closure_sha256": prepared.pins["qualified_source_closure"],
        "protocol_sha256": prepared.pins["protocol"],
        "cohorts_sha256": binding.cohorts_sha256,
        "input_manifest_sha256": _sha(runner._canonical(preacquisition_input)),
        "acquisition_plan_sha256": prepared.pins["acquisition_plan"],
        "research_cases": cases,
        "navigation_targets": navigation,
        "packet_stage_uuid": binding.packet_stage_uuid,
        "origin_package_sha256": origin.expected_package_sha256,
    }
    kwargs.update(
        {
            "prepared": prepared,
            "task_input_manifest_bytes": task_input_manifest_bytes,
            "pipeline_tasks": pipeline_tasks,
            "navigation_targets": [
                {"target_id": row["target_id"], "search_query": row["search_query"]} for row in navigation
            ],
            "snapshots_directory": imported.origin_pool_verified.snapshots_directory,
            "acquisition_snapshot_index_sha256": imported.origin_pool_verified.snapshot_index_sha256,
            "acquisition_manifest_bytes": runner._canonical(import_plan),
            "acquisition_manifest_sha256": _sha(runner._canonical(import_plan)),
            "verified_pool_source": imported.origin_pool_verified,
            "saved_corpus_import": imported,
            "initial_registration_sha256": initial_registration_sha256,
            "band_companion_sha256": band_companion_sha256,
            "operation_materials": {},
        }
    )
    map_bytes = input_map.build_selector_input_map(**kwargs)
    map_sha = _sha(map_bytes)
    receipt = _canonical(
        {
            "schema": admission.SCHEMA,
            "status": "admitted",
            "stage_uuid": prepared.stage_uuid,
            "source_revision": prepared.source_revision,
            "protocol_sha256": _sha(protocol_bytes),
            "registration_sha256": prepared.registration_sha256,
            "source_closure_sha256": prepared.pins["qualified_source_closure"],
            "selector_input_map_sha256": map_sha,
        }
    )
    verified_admission = admission.verify_selector_map_admission(
        receipt_bytes=receipt,
        expected_receipt_sha256=_sha(receipt),
        protocol_bytes=protocol_bytes,
        prepared=prepared,
        selector_input_map_sha256=map_sha,
    )
    execution._bind_selector_input_map(prepared, map_bytes, map_sha, admission=verified_admission)
    permits = {}
    for operation_id in prepared.operation_ids:
        material = kwargs["operation_materials"][operation_id]
        request, compiled = execution.build_selector_request(
            operation_id=operation_id,
            operation_input_bytes=material["operation_input_bytes"],
            legacy_control=material["legacy_control"],
        )
        permits[operation_id] = permit_for(
            prepared,
            operation_id,
            material["operation_input_bytes"],
            request,
            parser_mode="coverage" if compiled is not None else "original-v1",
        )
    return prepared, kwargs["operation_materials"], map_bytes, map_sha, permits


@pytest.mark.asyncio
async def test_saved_map_producer_admission_and_full_executor_preflight_are_provider_free(tmp_path):
    prepared, materials, map_bytes, map_sha, permits = _saved_schedule(tmp_path)

    await execution.preflight_registered_selector_schedule(
        prepared=prepared,
        selector_input_map_bytes=map_bytes,
        expected_selector_input_map_sha256=map_sha,
        operation_materials=materials,
        permits=permits,
        api_key="offline-preflight-only-key",
    )
    assert len(materials) == len(permits) == len(prepared.operation_ids) == 39


@pytest.mark.asyncio
async def test_saved_map_mutation_after_admission_is_rejected_before_claim_or_transport(tmp_path):
    prepared, materials, map_bytes, map_sha, permits = _saved_schedule(tmp_path)
    document = json.loads(map_bytes)
    document["origin_source_revision"] = "not-a-revision"
    tampered = input_map._canonical(document)

    with pytest.raises(execution.JevExecutionError, match="selector-input-map-pin-invalid"):
        await execution.preflight_registered_selector_schedule(
            prepared=prepared,
            selector_input_map_bytes=tampered,
            expected_selector_input_map_sha256=map_sha,
            operation_materials=materials,
            permits=permits,
            api_key="offline-preflight-only-key",
        )


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("origin_stage_uuid", None),
        ("origin_stage_uuid", "bad"),
        ("origin_source_revision", "z" * 40),
        ("pool_input_mode", "frozen-pool/1"),
        ("origin_pool_manifest_sha256", "not-a-sha256"),
    ],
)
def test_saved_map_extension_is_validated_even_when_rehashed(tmp_path, field, value):
    prepared, materials, map_bytes, _map_sha, _permits = _saved_schedule(tmp_path)
    document = json.loads(map_bytes)
    document[field] = value
    tampered = input_map._canonical(document)
    operation_id = prepared.operation_ids[0]
    operation_material = materials[operation_id]
    request_body, _compiled = execution.build_selector_request(
        operation_id=operation_id,
        operation_input_bytes=operation_material["operation_input_bytes"],
        legacy_control=operation_material["legacy_control"],
    )
    with pytest.raises(execution.JevExecutionError, match="selector-input-map-binding"):
        execution._verify_registered_operation_input(
            selector_input_map_bytes=tampered,
            expected_selector_input_map_sha256=_sha(tampered),
            prepared=prepared,
            operation_id=operation_id,
            operation_input_sha256=_sha(operation_material["operation_input_bytes"]),
            request_body_sha256=_sha(request_body),
        )


@pytest.mark.parametrize("mutation", ["same-current-uuid", "missing-field", "unknown-field"])
def test_saved_map_extension_cannot_be_reduced_or_broadened(tmp_path, mutation):
    prepared, materials, map_bytes, _map_sha, _permits = _saved_schedule(tmp_path)
    document = json.loads(map_bytes)
    if mutation == "same-current-uuid":
        document["origin_stage_uuid"] = prepared.stage_uuid
    elif mutation == "missing-field":
        document.pop("saved_import_receipt_sha256")
    else:
        document["operator_extra"] = "unexpected"
    tampered = input_map._canonical(document)
    operation_id = prepared.operation_ids[0]
    material = materials[operation_id]
    request_body, _compiled = execution.build_selector_request(
        operation_id=operation_id,
        operation_input_bytes=material["operation_input_bytes"],
        legacy_control=material["legacy_control"],
    )
    with pytest.raises(execution.JevExecutionError, match="selector-input-map-binding"):
        execution._verify_registered_operation_input(
            selector_input_map_bytes=tampered,
            expected_selector_input_map_sha256=_sha(tampered),
            prepared=prepared,
            operation_id=operation_id,
            operation_input_sha256=_sha(material["operation_input_bytes"]),
            request_body_sha256=_sha(request_body),
        )


def test_saved_map_extension_is_not_accepted_for_confirmation(tmp_path):
    prepared, materials, map_bytes, _map_sha, _permits = _saved_schedule(tmp_path)
    confirmation = replace(prepared, stage_kind="confirmation")
    operation_id = confirmation.operation_ids[0]
    material = materials[operation_id]
    request_body, _compiled = execution.build_selector_request(
        operation_id=operation_id,
        operation_input_bytes=material["operation_input_bytes"],
        legacy_control=material["legacy_control"],
    )
    with pytest.raises(execution.JevExecutionError, match="selector-input-map-binding"):
        execution._verify_registered_operation_input(
            selector_input_map_bytes=map_bytes,
            expected_selector_input_map_sha256=_sha(map_bytes),
            prepared=confirmation,
            operation_id=operation_id,
            operation_input_sha256=_sha(material["operation_input_bytes"]),
            request_body_sha256=_sha(request_body),
        )
