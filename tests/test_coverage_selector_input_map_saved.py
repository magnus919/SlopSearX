from __future__ import annotations

import hashlib
import json
from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import pytest

from scripts import coverage_operator_runner as runner
from scripts import coverage_saved_corpus as saved
from scripts import coverage_selector_input_map as input_map
from tests import test_coverage_saved_corpus as saved_fixtures
from tests import test_coverage_selector_input_map as selector_fixtures


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _saved_kwargs(tmp_path: Path):
    kwargs = selector_fixtures._materials(tmp_path)
    origin, cases, navigation, _collection, _capture = saved_fixtures._origin_package(tmp_path)
    prepared = replace(
        kwargs["prepared"],
        registration_sha256="9" * 64,
        selector_input_map_required=True,
        selector_input_map_schema=input_map.REGISTERED_SCHEMA,
    )
    kwargs["prepared"] = prepared
    initial_registration_sha256 = "a" * 64
    band_companion_sha256 = "b" * 64
    current_cohorts_sha256 = origin.registration.cohorts_sha256
    binding = replace(
        saved_fixtures._current_binding(origin),
        stage_uuid=prepared.stage_uuid,
        source_revision=prepared.source_revision,
        source_closure_sha256=prepared.pins["qualified_source_closure"],
        protocol_sha256=prepared.pins["protocol"],
        cohorts_sha256=current_cohorts_sha256,
    )
    imported = saved.verify_and_import_saved_corpus(origin, binding, cases, navigation)
    preacquisition_input = {
        "schema": "coverage-preacquisition-input-manifest/1",
        "stage_uuid": prepared.stage_uuid,
        "protocol_sha256": prepared.pins["protocol"],
        "cohorts_sha256": current_cohorts_sha256,
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
        "cohorts_sha256": current_cohorts_sha256,
        "input_manifest_sha256": _sha(runner._canonical(preacquisition_input)),
        "acquisition_plan_sha256": prepared.pins["acquisition_plan"],
        "research_cases": cases,
        "navigation_targets": navigation,
        "packet_stage_uuid": binding.packet_stage_uuid,
        "origin_package_sha256": origin.expected_package_sha256,
    }
    kwargs.update(
        {
            "snapshots_directory": imported.origin_pool_verified.snapshots_directory,
            "acquisition_snapshot_index_sha256": imported.origin_pool_verified.snapshot_index_sha256,
            "acquisition_manifest_bytes": runner._canonical(import_plan),
            "acquisition_manifest_sha256": _sha(runner._canonical(import_plan)),
            "verified_pool_source": imported.origin_pool_verified,
            "saved_corpus_import": imported,
            "initial_registration_sha256": initial_registration_sha256,
            "band_companion_sha256": band_companion_sha256,
        }
    )
    return kwargs, imported


def test_saved_selector_map_uses_origin_pool_and_binds_current_import(tmp_path):
    kwargs, imported = _saved_kwargs(tmp_path)
    plan = input_map._strict(kwargs["acquisition_manifest_bytes"])
    snapshot, verified_plan = input_map._saved_import_snapshot(
        prepared=kwargs["prepared"],
        acquisition_manifest=plan,
        acquisition_manifest_sha256=kwargs["acquisition_manifest_sha256"],
        acquisition_snapshot_index_sha256=kwargs["acquisition_snapshot_index_sha256"],
        snapshots_directory=kwargs["snapshots_directory"],
        verified_pool_source=kwargs["verified_pool_source"],
        saved_corpus_import=imported,
        initial_registration_sha256=kwargs["initial_registration_sha256"],
        band_companion_sha256=kwargs["band_companion_sha256"],
    )

    assert verified_plan["schema"] == "coverage-saved-import-plan/1"
    assert snapshot["index"]["stage_uuid"] == imported.origin_pool_verified.pool_index["stage_uuid"]
    assert snapshot["index"] is imported.origin_pool_verified.pool_index
    assert snapshot["operations"] is imported.origin_pool_verified.operations
    assert len(snapshot["operations"]) == 13
    assert kwargs["acquisition_manifest_bytes"] == runner._canonical(verified_plan)
    assert not kwargs["acquisition_manifest_bytes"].endswith(b"\n")


@pytest.mark.parametrize("tamper", ["receipt", "derived-manifest", "plan-stage", "index-pin"])
def test_saved_selector_map_rejects_rehashed_binding_drift(tmp_path, tamper):
    kwargs, imported = _saved_kwargs(tmp_path)
    plan = input_map._strict(kwargs["acquisition_manifest_bytes"])
    if tamper == "receipt":
        receipt = json.loads(imported.import_receipt_bytes)
        receipt["origin_package_sha256"] = "0" * 64
        forged = saved._canonical(receipt)
        kwargs["saved_corpus_import"] = replace(
            imported, import_receipt_bytes=forged, import_receipt_sha256=_sha(forged)
        )
    elif tamper == "derived-manifest":
        manifest = json.loads(imported.capture_manifest_bytes)
        manifest["source_revision"] = "0" * 40
        forged = saved._canonical(manifest)
        kwargs["saved_corpus_import"] = replace(
            imported, capture_manifest_bytes=forged, capture_manifest_sha256=_sha(forged)
        )
    elif tamper == "plan-stage":
        plan = json.loads(kwargs["acquisition_manifest_bytes"])
        plan["stage_uuid"] = "00000000-0000-0000-0000-000000000001"
        kwargs["acquisition_manifest_bytes"] = runner._canonical(plan)
        kwargs["acquisition_manifest_sha256"] = _sha(kwargs["acquisition_manifest_bytes"])
    else:
        kwargs["acquisition_snapshot_index_sha256"] = "0" * 64
    with pytest.raises(input_map.SelectorInputMapError):
        input_map._saved_import_snapshot(
            prepared=kwargs["prepared"],
            acquisition_manifest=input_map._strict(kwargs["acquisition_manifest_bytes"]),
            acquisition_manifest_sha256=kwargs["acquisition_manifest_sha256"],
            acquisition_snapshot_index_sha256=kwargs["acquisition_snapshot_index_sha256"],
            snapshots_directory=kwargs["snapshots_directory"],
            verified_pool_source=kwargs["verified_pool_source"],
            saved_corpus_import=kwargs["saved_corpus_import"],
            initial_registration_sha256=kwargs.get("initial_registration_sha256"),
            band_companion_sha256=kwargs.get("band_companion_sha256"),
        )


@pytest.mark.parametrize("newline_variant", ["preacquisition-input", "saved-import-plan"])
def test_saved_selector_map_rejects_rehashed_newline_canonical_variants(tmp_path, newline_variant):
    kwargs, imported = _saved_kwargs(tmp_path)
    plan = input_map._strict(kwargs["acquisition_manifest_bytes"])
    expected_error = "saved-import-plan-input-manifest-invalid"
    if newline_variant == "preacquisition-input":
        preacquisition_input = {
            "schema": "coverage-preacquisition-input-manifest/1",
            "stage_uuid": plan["stage_uuid"],
            "protocol_sha256": plan["protocol_sha256"],
            "cohorts_sha256": plan["cohorts_sha256"],
            "task_ids": [row["task_id"] for row in plan["research_cases"]]
            + [row["target_id"] for row in plan["navigation_targets"]],
            "band_companion_sha256": kwargs["band_companion_sha256"],
            "initial_registration_sha256": kwargs["initial_registration_sha256"],
        }
        plan["input_manifest_sha256"] = _sha(input_map._canonical(preacquisition_input))
        kwargs["acquisition_manifest_bytes"] = runner._canonical(plan)
        kwargs["acquisition_manifest_sha256"] = _sha(kwargs["acquisition_manifest_bytes"])
    else:
        expected_error = "saved-import-plan-digest-mismatch"
        kwargs["acquisition_manifest_bytes"] += b"\n"
        kwargs["acquisition_manifest_sha256"] = _sha(kwargs["acquisition_manifest_bytes"])

    with pytest.raises(input_map.SelectorInputMapError, match=expected_error):
        input_map._saved_import_snapshot(
            prepared=kwargs["prepared"],
            acquisition_manifest=input_map._strict(kwargs["acquisition_manifest_bytes"]),
            acquisition_manifest_sha256=kwargs["acquisition_manifest_sha256"],
            acquisition_snapshot_index_sha256=kwargs["acquisition_snapshot_index_sha256"],
            snapshots_directory=kwargs["snapshots_directory"],
            verified_pool_source=kwargs["verified_pool_source"],
            saved_corpus_import=imported,
            initial_registration_sha256=kwargs["initial_registration_sha256"],
            band_companion_sha256=kwargs["band_companion_sha256"],
        )


def test_saved_selector_map_emits_origin_metadata_after_full_operation_assembly(tmp_path):
    """Exercise shared 39-operation assembly with only the saved resolver isolated."""
    kwargs = selector_fixtures._materials(tmp_path)
    task_manifest = input_map._strict(kwargs["task_input_manifest_bytes"])
    research_cases = [
        {"task_id": row["task_id"], "search_query": row["query"]} for row in task_manifest["research_tasks"]
    ]
    navigation_targets = [
        {"target_id": row["task_id"], "search_query": row["query"]} for row in task_manifest["navigation_tasks"]
    ]
    import_plan = {
        "schema": "coverage-saved-import-plan/1",
        "stage": kwargs["prepared"].stage_kind,
        "stage_uuid": kwargs["prepared"].stage_uuid,
        "source_revision": kwargs["prepared"].source_revision,
        "source_closure_sha256": kwargs["prepared"].pins["qualified_source_closure"],
        "protocol_sha256": kwargs["prepared"].pins["protocol"],
        "cohorts_sha256": "3" * 64,
        "input_manifest_sha256": "4" * 64,
        "acquisition_plan_sha256": "5" * 64,
        "research_cases": research_cases,
        "navigation_targets": navigation_targets,
        "packet_stage_uuid": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
        "origin_package_sha256": "6" * 64,
    }
    kwargs["acquisition_manifest_bytes"] = runner._canonical(import_plan)
    kwargs["acquisition_manifest_sha256"] = _sha(kwargs["acquisition_manifest_bytes"])
    kwargs["saved_corpus_import"] = SimpleNamespace(
        origin_pool_verified=SimpleNamespace(
            snapshot_index_sha256="7" * 64,
            pool_manifest_sha256="8" * 64,
            pool_index={
                "stage_uuid": "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
                "source_revision": "9" * 40,
                "source_closure_sha256": "a" * 64,
                "protocol_sha256": "b" * 64,
                "cohorts_sha256": "c" * 64,
                "acquisition_plan_sha256": "d" * 64,
            },
        ),
        import_receipt_sha256="e" * 64,
        capture_manifest_sha256="f" * 64,
    )
    kwargs["initial_registration_sha256"] = "1" * 64
    kwargs["band_companion_sha256"] = "2" * 64
    pool_index_path = Path(kwargs["snapshots_directory"]) / "pool-snapshot-index.json"
    fixture_pool_index = input_map._strict(pool_index_path.read_bytes())
    fixture_operations = {
        row["operation_id"]: input_map._strict((pool_index_path.parent / row["file"]).read_bytes())
        for row in fixture_pool_index["operations"]
    }
    resolved = (
        {"index": kwargs["saved_corpus_import"].origin_pool_verified.pool_index, "operations": fixture_operations},
        import_plan,
    )
    with mock.patch.object(input_map, "_saved_import_snapshot", return_value=resolved):
        raw = input_map.build_selector_input_map(**kwargs)

    document = input_map._strict(raw)
    assert len(document["operations"]) == len(kwargs["prepared"].operation_ids) == 39
    assert document["pool_input_mode"] == "saved-corpus/1"
    assert document["origin_stage_uuid"] == "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"
    assert document["origin_source_revision"] == "9" * 40
    assert document["origin_source_closure_sha256"] == "a" * 64
    assert document["origin_protocol_sha256"] == "b" * 64
    assert document["origin_cohorts_sha256"] == "c" * 64
    assert document["origin_acquisition_plan_sha256"] == "d" * 64


@pytest.mark.parametrize("missing", ["initial_registration_sha256", "band_companion_sha256"])
def test_registered_saved_map_requires_external_initial_and_companion_pins(tmp_path, missing):
    kwargs, imported = _saved_kwargs(tmp_path)
    kwargs.pop(missing)
    with pytest.raises(input_map.SelectorInputMapError, match="saved-import-registration-pins-required"):
        input_map._saved_import_snapshot(
            prepared=kwargs["prepared"],
            acquisition_manifest=input_map._strict(kwargs["acquisition_manifest_bytes"]),
            acquisition_manifest_sha256=kwargs["acquisition_manifest_sha256"],
            acquisition_snapshot_index_sha256=kwargs["acquisition_snapshot_index_sha256"],
            snapshots_directory=kwargs["snapshots_directory"],
            verified_pool_source=kwargs["verified_pool_source"],
            saved_corpus_import=imported,
            initial_registration_sha256=kwargs.get("initial_registration_sha256"),
            band_companion_sha256=kwargs.get("band_companion_sha256"),
        )


@pytest.mark.parametrize("wrong", ["initial_registration_sha256", "band_companion_sha256"])
def test_registered_saved_map_rejects_wrong_external_registration_pins(tmp_path, wrong):
    kwargs, imported = _saved_kwargs(tmp_path)
    kwargs[wrong] = "c" * 64
    with pytest.raises(input_map.SelectorInputMapError, match="saved-import-plan-input-manifest-invalid"):
        input_map._saved_import_snapshot(
            prepared=kwargs["prepared"],
            acquisition_manifest=input_map._strict(kwargs["acquisition_manifest_bytes"]),
            acquisition_manifest_sha256=kwargs["acquisition_manifest_sha256"],
            acquisition_snapshot_index_sha256=kwargs["acquisition_snapshot_index_sha256"],
            snapshots_directory=kwargs["snapshots_directory"],
            verified_pool_source=kwargs["verified_pool_source"],
            saved_corpus_import=imported,
            initial_registration_sha256=kwargs["initial_registration_sha256"],
            band_companion_sha256=kwargs["band_companion_sha256"],
        )
