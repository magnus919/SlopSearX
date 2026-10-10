import hashlib
import json
from pathlib import Path

import pytest

from scripts import coverage_selector_input_map as input_map
from scripts import coverage_study_core as core
from tests.test_coverage_study_core import make_fixture, reseal_fixture


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _materials(tmp_path, *, navigation_skips=(None,) * 5):
    args, materials, *_ = make_fixture(navigation_skips=navigation_skips)
    repo = Path(__file__).parents[1]
    materials["coverage_source"] = (repo / "scripts/intent_ranking_coverage.py").read_bytes()
    materials["production_rerank_source"] = (
        repo / "docs/experiments/evidence/coverage-first-study/w0-rerank-v1.py.txt"
    ).read_bytes()
    pins = {name: _sha(materials[name]) for name in ("coverage_source", "production_rerank_source", "dependency_lock")}
    materials["qualified_source_closure"] = core._canonical(
        {
            "schema": "coverage-study-qualified-source-closure/1",
            "source_revision": "a" * 40,
            "material_pins": pins,
        }
    )
    args = reseal_fixture(args, materials, nav_skips=navigation_skips)
    prepared = core.preflight_stage(**args)
    manifest = json.loads(materials["task_input_manifest"])
    tasks = [
        {
            "task_id": row["task_id"],
            "query": row["query"],
            "purpose": row["purpose"],
            "facets": row["facets"],
            "ranking_candidates": row["candidates"],
            "native_order": row["incumbent_order"],
        }
        for row in manifest["research_tasks"]
    ]
    targets = [{"target_id": row["task_id"], "search_query": row["query"]} for row in manifest["navigation_tasks"]]
    snapshot_root = tmp_path / "snapshots"
    snapshot_root.mkdir(mode=0o700)
    snapshot_root.chmod(0o700)
    acquisition_manifest = {
        "schema": "coverage-live-acquisition-manifest/1",
        "stage": "development",
        "stage_uuid": prepared.stage_uuid,
        "source_revision": prepared.source_revision,
        "source_closure_sha256": prepared.pins["qualified_source_closure"],
        "protocol_sha256": prepared.pins["protocol"],
        "cohorts_sha256": "3" * 64,
        "input_manifest_sha256": "4" * 64,
        "acquisition_plan_sha256": "5" * 64,
    }
    acquisition_manifest_bytes = input_map._canonical(acquisition_manifest)
    stage_manifest_sha = _sha(acquisition_manifest_bytes)
    index_rows = []
    all_specs = [(row["task_id"], "research", row["query"], row["candidates"]) for row in manifest["research_tasks"]]
    for row in manifest["navigation_tasks"]:
        results = [
            {
                "title": f"Navigation {i}",
                "url": f"https://example.test/nav/{row['task_id']}/{i}",
                "content": f"Nav snippet {i}",
            }
            for i, _ in enumerate(row["pool_ids"])
        ]
        all_specs.append((row["task_id"], "navigation", row["query"], results))
    for operation_id, kind, query, candidates in all_specs:
        if kind == "research":
            results = [{"title": row["title"], "url": row["url"], "content": row["snippet"]} for row in candidates]
        else:
            results = candidates
        operation = {
            "operation_id": operation_id,
            "kind": kind,
            "status": "complete",
            "pool_count": len(results),
            "canonical_response": {"query": query, "results": results},
        }
        artifact = {
            "schema": "coverage-native-pool-snapshot/1",
            "stage": "development",
            "stage_uuid": prepared.stage_uuid,
            "source_revision": prepared.source_revision,
            "source_closure_sha256": prepared.pins["qualified_source_closure"],
            "protocol_sha256": prepared.pins["protocol"],
            "cohorts_sha256": acquisition_manifest["cohorts_sha256"],
            "input_manifest_sha256": acquisition_manifest["input_manifest_sha256"],
            "acquisition_plan_sha256": acquisition_manifest["acquisition_plan_sha256"],
            "stage_manifest_sha256": stage_manifest_sha,
            "task_plan": {"task_id": operation_id},
            "native_order": [f"c{i}" for i in range(len(results))],
            "operation": operation,
        }
        raw = input_map._canonical(artifact)
        name = input_map.coverage_live_acquire._snapshot_filename(operation_id)
        (snapshot_root / name).write_bytes(raw)
        (snapshot_root / name).chmod(0o600)
        index_rows.append(
            {
                "operation_id": operation_id,
                "kind": kind,
                "status": "complete",
                "file": name,
                "sha256": _sha(raw),
                "bytes": len(raw),
                "card_count": len(results),
            }
        )
    index = {
        "schema": input_map.coverage_live_acquire.POOL_INDEX_SCHEMA,
        "stage": "development",
        "stage_uuid": prepared.stage_uuid,
        "source_revision": prepared.source_revision,
        "source_closure_sha256": prepared.pins["qualified_source_closure"],
        "protocol_sha256": prepared.pins["protocol"],
        "cohorts_sha256": acquisition_manifest["cohorts_sha256"],
        "input_manifest_sha256": acquisition_manifest["input_manifest_sha256"],
        "acquisition_plan_sha256": acquisition_manifest["acquisition_plan_sha256"],
        "stage_manifest_sha256": stage_manifest_sha,
        "status": "complete",
        "operations": index_rows,
    }
    index_raw = input_map._canonical(index)
    (snapshot_root / "pool-snapshot-index.json").write_bytes(index_raw)
    (snapshot_root / "pool-snapshot-index.json").chmod(0o600)
    fixture_path = repo / "docs/experiments/evidence/coverage-first-study/draft-v2/selector-readiness-inputs.json"
    kwargs = {
        "prepared": prepared,
        "task_input_manifest_bytes": materials["task_input_manifest"],
        "pipeline_tasks": tasks,
        "navigation_targets": targets,
        "snapshots_directory": snapshot_root,
        "acquisition_manifest_bytes": acquisition_manifest_bytes,
        "neutral_fixture_bytes": fixture_path.read_bytes(),
        "acquisition_snapshot_index_sha256": _sha(index_raw),
        "acquisition_manifest_sha256": stage_manifest_sha,
        "frozen_v1_source_bytes": materials["production_rerank_source"],
        "current_service_source_bytes": (repo / "slopsearx/service.py").read_bytes(),
        "expected_builder_source_sha256": _sha(Path(input_map.__file__).read_bytes()),
    }
    return kwargs


@pytest.mark.parametrize("reason", sorted(core.NAVIGATION_SKIP_REASONS))
@pytest.mark.parametrize("skip_all", [False, True])
def test_map_and_materials_omit_registered_navigation_skips(tmp_path, reason, skip_all):
    skips = (reason,) * 5 if skip_all else (reason, None, None, None, None)
    kwargs = _materials(tmp_path, navigation_skips=skips)
    operation_materials = {}
    raw = input_map.build_selector_input_map(**kwargs, operation_materials=operation_materials)
    document = json.loads(raw)
    expected = set(kwargs["prepared"].operation_ids)
    assert {row["operation_id"] for row in document["operations"]} == expected
    assert set(operation_materials) == expected
    assert "navigation-01-w0" not in expected
    assert len(document["operations"]) == (34 if skip_all else 38)


def test_builds_complete_draft_map_from_pinned_inputs_without_authority(tmp_path):
    kwargs = _materials(tmp_path)
    operation_materials = {}
    kwargs["operation_materials"] = operation_materials
    raw = input_map.build_selector_input_map(**kwargs)
    doc = json.loads(raw)
    assert doc["schema"] == "coverage-selector-input-map/2-draft"
    assert doc["status"] == "draft-unadmitted"
    assert doc["stage_uuid"] == kwargs["prepared"].stage_uuid
    assert doc["original_registration_sha256"] == kwargs["prepared"].registration_sha256
    assert [row["operation_id"] for row in doc["operations"]] == list(kwargs["prepared"].operation_ids)
    assert len(doc["operations"]) == 39
    assert all(row["parser_mode"] in {"coverage", "original-v1"} for row in doc["operations"])
    assert doc["operations"][0]["operation_id"] == "neutral-w0-first40"
    by_id = {row["operation_id"]: row for row in doc["operations"]}
    assert set(operation_materials) == set(kwargs["prepared"].operation_ids)
    for operation_id, material in operation_materials.items():
        assert _sha(material["operation_input_bytes"]) == by_id[operation_id]["operation_input_sha256"]
        assert (material["legacy_control"] is not None) == (by_id[operation_id]["parser_mode"] == "original-v1")
    assert (
        input_map.verify_selector_input_map(
            input_map_bytes=raw,
            expected_sha256=_sha(raw),
            **kwargs,
        )
        == doc
    )


@pytest.mark.parametrize(
    ("field", "error"),
    [
        ("neutral_fixture_bytes", "neutral-fixture-pin-mismatch"),
        ("task_input_manifest_bytes", "input-map-source-pin-mismatch"),
    ],
)
def test_rejects_tampered_pinned_material(tmp_path, field, error):
    kwargs = _materials(tmp_path)
    kwargs[field] += b"x"
    with pytest.raises(input_map.SelectorInputMapError, match=error):
        input_map.build_selector_input_map(**kwargs)


def test_rejects_pipeline_projection_drift_before_map_creation(tmp_path):
    kwargs = _materials(tmp_path)
    tasks = list(kwargs["pipeline_tasks"])
    altered = dict(tasks[0])
    altered["ranking_candidates"] = [*altered["ranking_candidates"]]
    altered["ranking_candidates"][0] = {**altered["ranking_candidates"][0], "title": "caller replacement"}
    tasks[0] = altered
    kwargs["pipeline_tasks"] = tasks
    with pytest.raises(input_map.SelectorInputMapError, match="research-task-input-binding"):
        input_map.build_selector_input_map(**kwargs)


def test_rejects_navigation_snapshot_drift_before_map_creation(tmp_path):
    kwargs = _materials(tmp_path)
    path = Path(kwargs["snapshots_directory"])
    index = input_map._strict((path / "pool-snapshot-index.json").read_bytes())
    operation = next(row for row in index["operations"] if row["operation_id"] == "navigation-1")
    artifact_path = path / operation["file"]
    artifact = input_map._strict(artifact_path.read_bytes())
    artifact["operation"]["canonical_response"]["results"][0]["url"] = "https://attacker.invalid/"
    forged = input_map._canonical(artifact)
    artifact_path.write_bytes(forged)
    artifact_path.chmod(0o600)
    operation["sha256"] = _sha(forged)
    operation["bytes"] = len(forged)
    index_raw = input_map._canonical(index)
    (path / "pool-snapshot-index.json").write_bytes(index_raw)
    (path / "pool-snapshot-index.json").chmod(0o600)
    with pytest.raises(input_map.SelectorInputMapError, match="acquisition-snapshot-verification-failed"):
        input_map.build_selector_input_map(**kwargs)


def test_verifier_rejects_map_tamper_even_when_caller_rehashes_it(tmp_path):
    kwargs = _materials(tmp_path)
    raw = input_map.build_selector_input_map(**kwargs)
    doc = json.loads(raw)
    doc["operations"][0]["operation_input_sha256"] = "0" * 64
    forged = input_map._canonical(doc)
    with pytest.raises(input_map.SelectorInputMapError, match="input-map-recomputation-mismatch"):
        input_map.verify_selector_input_map(
            input_map_bytes=forged,
            expected_sha256=_sha(forged),
            **kwargs,
        )
