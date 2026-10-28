"""Synthetic tests for deterministic pre-capture pipeline input construction."""

from __future__ import annotations

import hashlib
import json
import os
import tempfile
import uuid
from dataclasses import replace

import pytest

from scripts import coverage_frozen_pool_inputs as frozen
from scripts import coverage_live_acquire as live
from scripts import coverage_pipeline_inputs as pipeline
from scripts import coverage_source_capture as capture
from scripts.coverage_study_acquire import AcquisitionOperation, StageAcquisition
from slopsearx.adapter import SearchResult
from slopsearx.service import ScopeDecision, SearchResponse


def sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def case(task: int, *, query: str | None = None) -> dict:
    return {
        "task_id": f"D-R{task:02d}",
        "query_id": f"D-Q{task:02d}",
        "search_query": query or f"synthetic topic {task}",
        "task_question": f"How does synthetic system {task} behave?",
        "purpose": f"Discover evidence about system {task}.",
        "purpose_id": f"D-P{task:02d}",
        "intent": "evidence_seeking",
        "pool_plan": {
            "band": "research_le_40",
            "engines": ["github", "wikipedia"],
        },
        "critical_facets": [
            {
                "facet_id": f"D-Q{task:02d}-F1",
                "definition": "The declared operating constraints.",
                "critical_checks": ["Do not include evaluator check text."],
            },
            {
                "facet_id": f"D-Q{task:02d}-F2",
                "definition": "The declared failure behavior.",
                "critical_checks": ["Also excluded from candidate inputs."],
            },
        ],
    }


def result(index: int, *, namespace: str = "", url: str | None = None) -> SearchResult:
    return SearchResult(
        url=url or f"https://example.test/{namespace}/card/{index}",
        title=f"Synthetic card {index}",
        content=f"Visible result description {index}",
        engine="github",
        engines={"github", "wikipedia"},
        score=float(2 + index),
        position=index + 1,
        category="reference",
        published_date="2025-01-01",
        thumbnail=None,
        img_src=None,
        tier=1,
        payload={"kind": "test", "nested": [1, "two"]},
        work_group={"members": [f"member-{index}"]},
    )


def make_stage(cases: list[dict], *, pools: list[list[SearchResult]] | None = None) -> StageAcquisition:
    pools = pools or [[result(i, namespace=task["task_id"]) for i in range(3)] for task in cases]
    operations = []
    for task, pool in zip(cases, pools, strict=True):
        response = SearchResponse(
            query=task["search_query"],
            results=pool,
            scope=ScopeDecision(selected_engines=["github", "wikipedia"]),
            engine_outcomes=[],
            ranking_explanation="tier_then_cross_engine_presence",
        )
        operations.append(
            AcquisitionOperation(
                operation_id=task["task_id"],
                kind="research",
                query=task["search_query"],
                engines=("github", "wikipedia"),
                status="complete",
                pool_count=len(pool),
                band="research_le_40",
                band_valid=True,
                target_url=None,
                target_found_at_rank1=None,
                scope=response.scope,
                canonical_response=response,
                exchanges=(),
            )
        )
    for index in range(5):
        target_url = f"https://github.com/example/{index}"
        nav_scope = ScopeDecision(selected_engines=["github"])
        nav_response = SearchResponse(
            query=f"synthetic nav {index}",
            results=[result(index, namespace="navigation", url=target_url)],
            scope=nav_scope,
            engine_outcomes=[],
            ranking_explanation="tier_then_cross_engine_presence",
        )
        operations.append(
            AcquisitionOperation(
                operation_id=f"D-N{index + 1:02d}",
                kind="navigation",
                query=f"synthetic nav {index}",
                engines=("github",),
                status="complete",
                pool_count=1,
                band=None,
                band_valid=None,
                target_url=target_url,
                target_found_at_rank1=True,
                scope=nav_scope,
                canonical_response=nav_response,
                exchanges=(),
            )
        )
    return StageAcquisition("development", "complete", operations, [], 0, 0, 0)


def write_snapshot(
    stage: StageAcquisition, cases: list[dict], root, *, acquisition_plan_sha256: str = "4" * 64
) -> tuple[str, str]:
    root.mkdir(mode=0o700)
    pins = {
        "protocol_sha256": "1" * 64,
        "cohorts_sha256": "2" * 64,
        "input_manifest_sha256": "6" * 64,
        "acquisition_plan_sha256": acquisition_plan_sha256,
        "source_closure_sha256": "5" * 64,
    }
    manifest = {
        "stage": stage.stage,
        "stage_uuid": str(uuid.UUID(int=123)),
        "source_revision": "a" * 40,
        **pins,
        "research_cases": cases,
        "navigation_targets": [
            {
                "target_id": op.operation_id,
                "search_query": op.query,
                "target_url": op.target_url,
                "navigation_pool_plan": {"engines": list(op.engines)},
            }
            for op in stage.operations[8:]
        ],
    }
    manifest_sha = sha(pipeline._canonical(manifest))
    (root / "acquisition-manifest.json").write_bytes(pipeline._canonical(manifest))
    os.chmod(root / "acquisition-manifest.json", 0o600)
    rows = []
    for operation in stage.operations:
        artifact = live._operation_snapshot(operation, manifest, manifest_sha)
        raw = pipeline._canonical(artifact)
        filename = live._snapshot_filename(operation.operation_id)
        path = root / filename
        path.write_bytes(raw)
        os.chmod(path, 0o600)
        rows.append(
            {
                "operation_id": operation.operation_id,
                "kind": operation.kind,
                "status": operation.status,
                "file": filename,
                "sha256": sha(raw),
                "bytes": len(raw),
                "card_count": operation.pool_count,
            }
        )
    index = {
        "schema": live.POOL_INDEX_SCHEMA,
        "stage": stage.stage,
        "stage_uuid": manifest["stage_uuid"],
        "source_revision": manifest["source_revision"],
        **pins,
        "stage_manifest_sha256": manifest_sha,
        "status": (
            "complete" if stage.status == "complete" and all(r["status"] == "complete" for r in rows) else "incomplete"
        ),
        "operations": rows,
    }
    index_raw = pipeline._canonical(index)
    (root / "pool-snapshot-index.json").write_bytes(index_raw)
    os.chmod(root / "pool-snapshot-index.json", 0o600)
    return sha(index_raw), manifest_sha


def build(cases: list[dict], stage: StageAcquisition | None = None) -> dict:
    stage = stage or make_stage(cases)
    with tempfile.TemporaryDirectory() as dirname:
        root = __import__("pathlib").Path(dirname) / "snapshots"
        index_sha, manifest_sha = write_snapshot(stage, cases, root)
        return pipeline.build_pipeline_inputs(
            cases,
            snapshots_directory=root,
            expected_snapshot_index_sha256=index_sha,
            expected_stage_manifest_sha256=manifest_sha,
            stage_uuid=str(uuid.UUID(int=123)),
            source_revision="a" * 40,
            protocol_sha256="1" * 64,
            cohorts_sha256="2" * 64,
            candidate_identity_sha256="3" * 64,
            candidate_endpoint_sha256="4" * 64,
        )


def test_builds_source_capture_manifest_and_full_native_card_maps():
    cases = [case(i) for i in range(1, 9)]
    pools = [[result(i, namespace=task["task_id"]) for i in range(3)] for task in cases]
    pools[0][0].url = "https://example.test/shared?utm_source=one"
    pools[1][0].url = "https://example.test/shared?utm_source=two"
    output = build(cases, make_stage(cases, pools=pools))

    manifest = output["capture_manifest"]
    assert manifest["schema"] == "coverage-source-capture-manifest/1"
    assert len(manifest["sources"]) == 24
    assert output["card_count"] == 24
    assert output["source_count"] == 23
    assert output["capture_manifest_bytes"].endswith(b"\n")
    assert output["capture_manifest_sha256"] == sha(output["capture_manifest_bytes"])
    parsed, rows = capture._validate_manifest(output["capture_manifest_bytes"], "1" * 64)
    assert parsed == manifest
    assert rows == manifest["sources"]

    first, second = output["tasks"][:2]
    assert first["native_order"] == ["c0", "c1", "c2"]
    assert first["source_id_by_card_id"]["c0"] == second["source_id_by_card_id"]["c0"]
    assert [card["native_rank"] for card in first["cards"]] == [1, 2, 3]
    assert first["cards"][0]["full_metadata"]["payload"]["kind"] == "test"
    assert first["cards"][0]["full_metadata_sha256"] == sha(pipeline._canonical(first["cards"][0]["full_metadata"]))
    assert first["cards"][0]["ranking_projection"] == {
        "id": "c0",
        "title": "Synthetic card 0",
        "url": "https://example.test/shared",
        "snippet": "Visible result description 0",
    }
    assert first["purpose"] == "How does synthetic system 1 behave?\nDiscover evidence about system 1."
    assert first["facets"] == [
        {"id": "D-Q01-F1", "description": "The declared operating constraints."},
        {"id": "D-Q01-F2", "description": "The declared failure behavior."},
    ]
    assert "critical_checks" not in json.dumps(first["facets"])
    assert all(row["task_id"].startswith("D-R") for row in manifest["sources"])


def test_projection_and_payload_digests_are_deterministic_and_sensitive():
    cases = [case(i) for i in range(1, 9)]
    original = build(cases)
    replay = build(cases)
    assert original["capture_manifest_bytes"] == replay["capture_manifest_bytes"]
    assert original["card_bindings_sha256"] == replay["card_bindings_sha256"]
    changed_stage = make_stage(cases)
    changed_stage.operations[0].canonical_response.results[0].content = "different snippet"
    changed = build(cases, changed_stage)
    original_card = original["tasks"][0]["cards"][0]
    changed_card = changed["tasks"][0]["cards"][0]
    assert original_card["full_metadata_sha256"] != changed_card["full_metadata_sha256"]
    assert original_card["ranking_projection_sha256"] != changed_card["ranking_projection_sha256"]


def test_duplicate_canonical_url_within_pool_fails_closed():
    cases = [case(i) for i in range(1, 9)]
    pools = [[result(i) for i in range(3)] for _ in range(8)]
    pools[0][1].url = pools[0][0].url + "?utm_campaign=duplicate"
    with pytest.raises(pipeline.PipelineInputError, match="duplicate-canonical-url-within-pool"):
        build(cases, make_stage(cases, pools=pools))


@pytest.mark.parametrize(
    ("mutation", "reason"),
    [
        (
            lambda stage, cases: setattr(stage, "status", "inconclusive"),
            "pool-snapshot-invalid:pool-snapshot-index-incomplete",
        ),
        (
            lambda stage, cases: setattr(stage.operations[0], "status", "inconclusive"),
            "pool-snapshot-invalid:pool-snapshot-index-incomplete",
        ),
        (
            lambda stage, cases: setattr(stage.operations[0], "band_valid", False),
            "research-pool-not-complete-and-in-band",
        ),
        (lambda stage, cases: setattr(stage.operations[0], "query", "wrong"), "cohort-query-acquisition-mismatch"),
        (lambda stage, cases: cases[0].update(search_query="different"), "cohort-query-acquisition-mismatch"),
    ],
)
def test_bad_stage_or_cohort_bindings_are_rejected(mutation, reason):
    cases = [case(i) for i in range(1, 9)]
    stage = make_stage(cases)
    mutation(stage, cases)
    with pytest.raises(pipeline.PipelineInputError, match=reason):
        build(cases, stage)


def test_invalid_pins_duplicate_tasks_and_oversized_pools_fail_closed():
    cases = [case(i) for i in range(1, 9)]
    with pytest.raises(pipeline.PipelineInputError, match="protocol-sha256-invalid"):
        pipeline.build_pipeline_inputs(
            cases,
            snapshots_directory="/does-not-exist",
            expected_snapshot_index_sha256="0" * 64,
            expected_stage_manifest_sha256="0" * 64,
            stage_uuid=str(uuid.UUID(int=123)),
            source_revision="a" * 40,
            protocol_sha256="short",
            cohorts_sha256="2" * 64,
            candidate_identity_sha256="3" * 64,
            candidate_endpoint_sha256="4" * 64,
        )
    dup = [case(i) for i in range(1, 9)]
    dup[1]["task_id"] = dup[0]["task_id"]
    with pytest.raises(pipeline.PipelineInputError, match="cohort-task-duplicate"):
        build(dup)
    pools = [[result(i, namespace=cases[0]["task_id"]) for i in range(81)]] + [
        [result(i, namespace=task["task_id"]) for i in range(2)] for task in cases[1:]
    ]
    with pytest.raises(pipeline.PipelineInputError, match="natural-pool-size-invalid"):
        build(cases, make_stage(cases, pools=pools))


def test_complete_640_card_high_band_pools_are_retained_without_truncation():
    cases = [case(i) for i in range(1, 9)]
    pools = [[result(i, namespace=task["task_id"]) for i in range(80)] for task in cases]
    stage = make_stage(cases, pools=pools)
    for task, operation in zip(cases, stage.operations[:8], strict=True):
        task["pool_plan"] = {"band": "research_41_80", "engines": ["arxiv", "github", "openalex", "wikipedia"]}
        operation.band = "research_41_80"
        operation.engines = ("arxiv", "github", "openalex", "wikipedia")
        operation.canonical_response.scope.selected_engines = list(operation.engines)
    output = build(cases, stage)
    assert output["card_count"] == 640
    assert output["source_count"] == 640
    assert all(len(task["cards"]) == 80 for task in output["tasks"])
    assert all(task["native_order"][0] == "c0" and task["native_order"][-1] == "c79" for task in output["tasks"])


def test_builder_reloads_only_hash_verified_snapshot_files(tmp_path):
    cases = [case(i) for i in range(1, 9)]
    stage = make_stage(cases)
    root = tmp_path / "snapshots"
    index_sha, manifest_sha = write_snapshot(stage, cases, root)
    # Caller-held objects are no longer the input authority.
    stage.operations[0].canonical_response.results[0].title = "memory-only mutation"
    output = pipeline.build_pipeline_inputs(
        cases,
        snapshots_directory=root,
        expected_snapshot_index_sha256=index_sha,
        expected_stage_manifest_sha256=manifest_sha,
        stage_uuid=str(uuid.UUID(int=123)),
        source_revision="a" * 40,
        protocol_sha256="1" * 64,
        cohorts_sha256="2" * 64,
        candidate_identity_sha256="3" * 64,
        candidate_endpoint_sha256="4" * 64,
    )
    assert output["tasks"][0]["cards"][0]["full_metadata"]["title"] == "Synthetic card 0"

    artifact_path = root / live._snapshot_filename(stage.operations[0].operation_id)
    changed = json.loads(artifact_path.read_bytes())
    changed["operation"]["query"] = "tampered query"
    artifact_path.write_bytes(pipeline._canonical(changed))
    os.chmod(artifact_path, 0o600)
    with pytest.raises(
        pipeline.PipelineInputError,
        match="pool-snapshot-invalid:pool-snapshot-operation-digest-mismatch",
    ):
        pipeline.build_pipeline_inputs(
            cases,
            snapshots_directory=root,
            expected_snapshot_index_sha256=index_sha,
            expected_stage_manifest_sha256=manifest_sha,
            stage_uuid=str(uuid.UUID(int=123)),
            source_revision="a" * 40,
            protocol_sha256="1" * 64,
            cohorts_sha256="2" * 64,
            candidate_identity_sha256="3" * 64,
            candidate_endpoint_sha256="4" * 64,
        )


def test_builder_binds_saved_task_plan_to_current_cohort_row():
    cases = [case(i) for i in range(1, 9)]
    stage = make_stage(cases)
    with tempfile.TemporaryDirectory() as dirname:
        root = __import__("pathlib").Path(dirname) / "snapshots"
        index_sha, manifest_sha = write_snapshot(stage, cases, root)
        cases[0]["unregistered_plan_field"] = "drift"
        with pytest.raises(pipeline.PipelineInputError, match="pool-snapshot-task-plan-mismatch"):
            pipeline.build_pipeline_inputs(
                cases,
                snapshots_directory=root,
                expected_snapshot_index_sha256=index_sha,
                expected_stage_manifest_sha256=manifest_sha,
                stage_uuid=str(uuid.UUID(int=123)),
                source_revision="a" * 40,
                protocol_sha256="1" * 64,
                cohorts_sha256="2" * 64,
                candidate_identity_sha256="3" * 64,
                candidate_endpoint_sha256="4" * 64,
            )


def _fresh_frozen_evidence(root, stage, cases, *, acquisition_plan_sha256: str = "4" * 64):
    index_sha, manifest_sha = write_snapshot(stage, cases, root, acquisition_plan_sha256=acquisition_plan_sha256)
    original_index = json.loads((root / "pool-snapshot-index.json").read_bytes())
    availability_rows = []
    frozen_rows = []
    for row in original_index["operations"]:
        path = root / row["file"]
        artifact = json.loads(path.read_bytes())
        operation = artifact["operation"]
        if operation["kind"] == "research" and "arxiv" in operation["engines"]:
            # Keep the genuine per-operation acquisition outcome inconclusive;
            # frozen-pool eligibility is a separate field in the new index.
            operation["status"] = "inconclusive"
            operation["band_valid"] = False
            operation["failure_reasons"] = ["engine_failures:arxiv"]
        engines = list(operation["engines"])
        outcomes = [
            {
                "engine": engine,
                "status": "rate_limited" if engine == "arxiv" else "ok",
                "result_count": 0 if engine == "arxiv" else 1,
            }
            for engine in engines
        ]
        artifact["operation"]["canonical_response"]["engine_outcomes"] = outcomes
        raw = pipeline._canonical(artifact)
        path.write_bytes(raw)
        os.chmod(path, 0o600)
        row["sha256"] = sha(raw)
        row["bytes"] = len(raw)
        frozen_rows.append(
            {
                "operation_id": row["operation_id"],
                "kind": row["kind"],
                "acquisition_status": operation["status"],
                "pool_status": "complete",
                "file": row["file"],
                "sha256": row["sha256"],
                "bytes": row["bytes"],
                "card_count": row["card_count"],
            }
        )
        availability_rows.append({"operation_id": row["operation_id"], "engines": outcomes})
    availability = {
        "schema": frozen.AVAILABILITY_SCHEMA,
        "stage_uuid": str(uuid.UUID(int=123)),
        "operations": availability_rows,
    }
    availability_raw = pipeline._canonical(availability)
    availability_sha = sha(availability_raw)
    frozen_index = {
        "schema": frozen.INDEX_SCHEMA,
        "stage": stage.stage,
        "status": "complete",
        "stage_uuid": str(uuid.UUID(int=123)),
        "stage_kind": stage.stage,
        "source_revision": "a" * 40,
        "source_closure_sha256": "5" * 64,
        "protocol_sha256": "1" * 64,
        "cohorts_sha256": "2" * 64,
        "candidate_identity_sha256": "3" * 64,
        "candidate_endpoint_sha256": "4" * 64,
        "acquisition_plan_sha256": acquisition_plan_sha256,
        "pool_manifest_sha256": manifest_sha,
        "availability_sha256": availability_sha,
        "operations": frozen_rows,
    }
    index_raw = pipeline._canonical(frozen_index)
    (root / "frozen-pool-index.json").write_bytes(index_raw)
    os.chmod(root / "frozen-pool-index.json", 0o600)
    index_sha = sha(index_raw)
    evidence_doc = {
        "schema": frozen.SCHEMA,
        "stage_uuid": str(uuid.UUID(int=123)),
        "stage_kind": "development",
        "source_revision": "a" * 40,
        "source_closure_sha256": "5" * 64,
        "protocol_sha256": "1" * 64,
        "cohorts_sha256": "2" * 64,
        "candidate_identity_sha256": "3" * 64,
        "candidate_endpoint_sha256": "4" * 64,
        "acquisition_plan_sha256": acquisition_plan_sha256,
        "pool_manifest_sha256": manifest_sha,
        "pool_snapshot_index_sha256": index_sha,
        "availability_sha256": availability_sha,
        "collector_receipt_sha256": "0" * 64,
        "collection_started_monotonic": 200.0,
        "collection_completed_monotonic": 300.0,
        "stage_deadline_monotonic": 10_000.0,
        "collection_started_utc": "2026-10-13T12:00:00Z",
        "collection_completed_utc": "2026-10-13T12:01:40Z",
        "quality_credit": False,
    }
    operation_statuses = []
    for row in frozen_rows:
        artifact = json.loads((root / row["file"]).read_bytes())
        operation = artifact["operation"]
        operation_statuses.append(
            {
                "operation_id": row["operation_id"],
                "status": operation["status"],
                "failure_reasons": operation.get("failure_reasons", []),
            }
        )
    component_docs = {
        "admission.json": {
            "schema": "coverage-frozen-collection-admission/1",
            "pool_input_mode": "frozen-pool/1",
            "stage_clock_sha256": "9" * 64,
            "clock_origin_sha256": "a" * 64,
            "stage_uuid": str(uuid.UUID(int=123)),
            "stage": stage.stage,
            "source_revision": "a" * 40,
            "source_closure_sha256": "5" * 64,
            "protocol_sha256": "1" * 64,
            "cohorts_sha256": "2" * 64,
            "input_manifest_sha256": "6" * 64,
            "acquisition_plan_sha256": acquisition_plan_sha256,
            "candidate_identity_sha256": "3" * 64,
            "candidate_endpoint_sha256": "4" * 64,
            "stage_manifest_sha256": manifest_sha,
            "permit_sha256": "7" * 64,
            "lease_sha256": "8" * 64,
        },
        "transport-control.json": {"schema": "coverage-acquisition-execution-controls/1"},
        "request-ledger-index.json": {"schema": "coverage-http-request-ledger-index/1"},
        "collection-summary.json": {
            "schema": "coverage-frozen-collection-summary/1",
            "stage_uuid": str(uuid.UUID(int=123)),
            "stage_manifest_sha256": manifest_sha,
            "pool_input_mode": "frozen-pool/1",
            "status": "complete",
            "physical_request_count": 13,
            "transfer_bytes": 1000,
            "collection_started_monotonic": 200.0,
            "collection_completed_monotonic": 300.0,
            "stage_deadline_monotonic": 10_000.0,
            "collection_started_utc": "2026-10-13T12:00:00Z",
            "collection_completed_utc": "2026-10-13T12:01:40Z",
            "stage_deadline_utc": "2026-10-13T20:00:00Z",
            "quality_credit": False,
            "fatal_uninvoked_slots": [],
            "operation_statuses": operation_statuses,
            "pool_eligibility": [
                {"operation_id": row["operation_id"], "pool_status": "complete"} for row in frozen_rows
            ],
        },
    }
    component_hashes = {}
    for name, value in component_docs.items():
        component_raw = pipeline._canonical(value)
        (root / name).write_bytes(component_raw)
        os.chmod(root / name, 0o600)
        component_hashes[name] = sha(component_raw)
    receipt_doc = {
        "schema": frozen.COLLECTION_RECEIPT_SCHEMA,
        "scope": "registered-frozen-pool-collection",
        "status": "complete",
        "stage_uuid": str(uuid.UUID(int=123)),
        "stage_kind": stage.stage,
        "source_revision": "a" * 40,
        "source_closure_sha256": "5" * 64,
        "protocol_sha256": "1" * 64,
        "cohorts_sha256": "2" * 64,
        "candidate_identity_sha256": "3" * 64,
        "candidate_endpoint_sha256": "4" * 64,
        "acquisition_plan_sha256": acquisition_plan_sha256,
        "pool_manifest_sha256": manifest_sha,
        "pool_snapshot_index_sha256": index_sha,
        "availability_sha256": availability_sha,
        "collection_started_monotonic": 200.0,
        "collection_completed_monotonic": 300.0,
        "stage_deadline_monotonic": 10_000.0,
        "collection_started_utc": "2026-10-13T12:00:00Z",
        "collection_completed_utc": "2026-10-13T12:01:40Z",
        "admission_sha256": component_hashes["admission.json"],
        "transport_control_sha256": component_hashes["transport-control.json"],
        "request_ledger_index_sha256": component_hashes["request-ledger-index.json"],
        "collection_summary_sha256": component_hashes["collection-summary.json"],
    }
    receipt_raw = pipeline._canonical(receipt_doc)
    evidence_doc["collector_receipt_sha256"] = sha(receipt_raw)
    evidence_raw = pipeline._canonical(evidence_doc)
    availability_path = root / "availability.json"
    availability_path.write_bytes(availability_raw)
    os.chmod(availability_path, 0o600)
    return frozen.FrozenPoolEvidence(
        root, evidence_raw, sha(evidence_raw), receipt_raw, sha(receipt_raw), availability_raw, availability_sha
    )


def test_fresh_frozen_pool_path_builds_same_cards_and_preserves_partial_availability(tmp_path):
    cases = [case(i) for i in range(1, 9)]
    stage = make_stage(cases, pools=[[result(i, namespace=row["task_id"]) for i in range(3)] for row in cases])
    for i, (task, operation) in enumerate(zip(cases, stage.operations[:8], strict=True)):
        if i < 4:
            task["pool_plan"] = {"band": "research_le_40", "engines": ["github", "wikipedia"]}
            operation.band = "research_le_40"
            operation.engines = ("github", "wikipedia")
            size = 21
        else:
            task["pool_plan"] = {"band": "research_41_80", "engines": ["arxiv", "github", "openalex", "wikipedia"]}
            operation.band = "research_41_80"
            operation.engines = ("arxiv", "github", "openalex", "wikipedia")
            size = 41
        operation.band_valid = True
        operation.canonical_response.scope.selected_engines = list(operation.engines)
        operation.canonical_response.results = [result(j, namespace=task["task_id"]) for j in range(size)]
        operation.pool_count = size
    root = tmp_path / "frozen"
    evidence = _fresh_frozen_evidence(root, stage, cases)
    registration = frozen.FrozenPoolRegistrationContext(
        stage_uuid=str(uuid.UUID(int=123)),
        stage_kind="development",
        source_revision="a" * 40,
        source_closure_sha256="5" * 64,
        protocol_sha256="1" * 64,
        cohorts_sha256="2" * 64,
        candidate_identity_sha256="3" * 64,
        candidate_endpoint_sha256="4" * 64,
        acquisition_plan_sha256="4" * 64,
        task_ids=tuple(task["task_id"] for task in cases),
        navigation_ids=tuple(f"D-N{i:02d}" for i in range(1, 6)),
        evidence_sha256=evidence.expected_evidence_sha256,
        collector_receipt_sha256=evidence.expected_collector_receipt_sha256,
        availability_sha256=evidence.expected_availability_sha256,
    )
    phase_result = frozen.run_frozen_pool_input_phase(
        cases,
        evidence=evidence,
        registration=registration,
        stage_started_monotonic=100.0,
        stage_deadline_monotonic=10_000.0,
        now_monotonic=400.0,
    )
    verified = phase_result.verified
    output = phase_result.pipeline_inputs
    assert [len(task["cards"]) for task in output["tasks"]] == [21] * 4 + [41] * 4
    assert output["quality_credit"] is False
    assert verified.operations[cases[4]["task_id"]]["operation"]["status"] == "inconclusive"
    assert verified.pool_index["operations"][4]["acquisition_status"] == "inconclusive"
    forged = replace(verified, pool_index={})
    with pytest.raises(frozen.FrozenPoolError, match="frozen-pool-verification-object-mismatch"):
        frozen.build_pipeline_inputs_from_frozen(
            cases,
            evidence=evidence,
            verified=forged,
            expected_stage_uuid=registration.stage_uuid,
            expected_source_revision=registration.source_revision,
            expected_protocol_sha256=registration.protocol_sha256,
            expected_cohorts_sha256=registration.cohorts_sha256,
            expected_candidate_identity_sha256=registration.candidate_identity_sha256,
            expected_candidate_endpoint_sha256=registration.candidate_endpoint_sha256,
        )
    sidecar = json.loads(evidence.availability_bytes)
    assert sidecar["operations"][4]["engines"][0] == {"engine": "arxiv", "status": "rate_limited", "result_count": 0}


def _reseal_frozen_evidence(root, evidence, *, availability_changed=False):
    doc = json.loads(evidence.evidence_bytes)
    receipt = json.loads(evidence.collector_receipt_bytes)
    availability = evidence.availability_bytes
    if availability_changed:
        sidecar = json.loads(availability)
        availability = pipeline._canonical(sidecar)
        evidence = replace(evidence, availability_bytes=availability, expected_availability_sha256=sha(availability))
    index = json.loads((root / "frozen-pool-index.json").read_bytes())
    index["availability_sha256"] = evidence.expected_availability_sha256
    index_raw = pipeline._canonical(index)
    (root / "frozen-pool-index.json").write_bytes(index_raw)
    os.chmod(root / "frozen-pool-index.json", 0o600)
    index_sha = sha(index_raw)
    summary_path = root / "collection-summary.json"
    summary = json.loads(summary_path.read_bytes())
    summary["pool_snapshot_index_sha256"] = index_sha
    summary["availability_sha256"] = evidence.expected_availability_sha256
    summary_raw = pipeline._canonical(summary)
    summary_path.write_bytes(summary_raw)
    os.chmod(summary_path, 0o600)
    receipt["pool_snapshot_index_sha256"] = index_sha
    receipt["availability_sha256"] = evidence.expected_availability_sha256
    receipt["collection_summary_sha256"] = sha(summary_raw)
    receipt_raw = pipeline._canonical(receipt)
    doc["pool_snapshot_index_sha256"] = index_sha
    doc["availability_sha256"] = evidence.expected_availability_sha256
    doc["collector_receipt_sha256"] = sha(receipt_raw)
    evidence_raw = pipeline._canonical(doc)
    return replace(
        evidence,
        evidence_bytes=evidence_raw,
        expected_evidence_sha256=sha(evidence_raw),
        collector_receipt_bytes=receipt_raw,
        expected_collector_receipt_sha256=sha(receipt_raw),
    )


@pytest.mark.parametrize(
    ("change", "reason"),
    [
        ("partial", "frozen-pool-operation-membership-mismatch"),
        ("retired_identity", "frozen-pool-evidence-binding-mismatch"),
        ("source_mismatch", "frozen-pool-evidence-binding-mismatch"),
        ("wrong_band", "frozen-pool-natural-band-invalid"),
        ("availability_success_lie", "frozen-pool-availability-does-not-match-snapshot"),
        ("wrong_clock", "frozen-pool-collection-outside-stage"),
    ],
)
def test_frozen_pool_path_rejects_missing_replaced_or_misbound_evidence(tmp_path, change, reason):
    cases = [case(i) for i in range(1, 9)]
    stage = make_stage(cases)
    root = tmp_path / change
    evidence = _fresh_frozen_evidence(root, stage, cases)
    if change == "partial":
        index = json.loads((root / "frozen-pool-index.json").read_bytes())
        index["operations"].pop()
        raw = pipeline._canonical(index)
        (root / "frozen-pool-index.json").write_bytes(raw)
        os.chmod(root / "frozen-pool-index.json", 0o600)
        evidence = _reseal_frozen_evidence(root, evidence)
    elif change == "retired_identity":
        doc = json.loads(evidence.evidence_bytes)
        doc["stage_uuid"] = str(uuid.UUID(int=999))
        raw = pipeline._canonical(doc)
        evidence = replace(evidence, evidence_bytes=raw, expected_evidence_sha256=sha(raw))
    elif change == "source_mismatch":
        doc = json.loads(evidence.evidence_bytes)
        doc["source_revision"] = "b" * 40
        raw = pipeline._canonical(doc)
        evidence = replace(evidence, evidence_bytes=raw, expected_evidence_sha256=sha(raw))
    elif change == "wrong_band":
        index = json.loads((root / "frozen-pool-index.json").read_bytes())
        first = index["operations"][0]
        artifact_path = root / first["file"]
        artifact = json.loads(artifact_path.read_bytes())
        artifact["operation"]["band"] = "research_41_80"
        artifact_raw = pipeline._canonical(artifact)
        artifact_path.write_bytes(artifact_raw)
        os.chmod(artifact_path, 0o600)
        first["sha256"] = sha(artifact_raw)
        first["bytes"] = len(artifact_raw)
        index_raw = pipeline._canonical(index)
        (root / "frozen-pool-index.json").write_bytes(index_raw)
        os.chmod(root / "frozen-pool-index.json", 0o600)
        evidence = _reseal_frozen_evidence(root, evidence)
    elif change == "availability_success_lie":
        sidecar = json.loads(evidence.availability_bytes)
        sidecar["operations"][4]["engines"][0]["status"] = "blocked"
        availability_raw = pipeline._canonical(sidecar)
        evidence = replace(
            evidence, availability_bytes=availability_raw, expected_availability_sha256=sha(availability_raw)
        )
        evidence = _reseal_frozen_evidence(root, evidence, availability_changed=True)
    with pytest.raises(frozen.FrozenPoolError, match=reason):
        frozen.verify_frozen_pool_evidence(
            evidence,
            expected_stage_uuid=str(uuid.UUID(int=123)),
            expected_stage_kind="development",
            expected_source_revision="a" * 40,
            expected_source_closure_sha256="5" * 64,
            expected_protocol_sha256="1" * 64,
            expected_cohorts_sha256="2" * 64,
            expected_candidate_identity_sha256="3" * 64,
            expected_candidate_endpoint_sha256="4" * 64,
            expected_acquisition_plan_sha256="4" * 64,
            expected_task_ids=tuple(task["task_id"] for task in cases),
            expected_navigation_ids=tuple(f"D-N{i:02d}" for i in range(1, 6)),
            stage_started_monotonic=(301.0 if change == "wrong_clock" else 100.0),
            stage_deadline_monotonic=10_000.0,
            now_monotonic=400.0,
        )
