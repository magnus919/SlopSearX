from __future__ import annotations

import hashlib
import json
import os
import uuid
from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace

import pytest

from scripts import coverage_late_registration as late
from scripts import coverage_operator_runner as operator_runner
from scripts import coverage_saved_corpus as saved
from scripts import coverage_selector_input_map
from scripts import coverage_stage_orchestration as orchestration
from scripts import coverage_study_acquire as acquisition
from scripts import coverage_study_core as core
from tests.test_coverage_saved_corpus import _current_binding, _origin_package


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _saved_plan_and_manifest(origin, cases, navigation):
    protocol = (
        Path(__file__).parents[1]
        / "docs/experiments/evidence/coverage-first-study/saved-development-method-materials/protocol.json"
    ).read_bytes()
    protocol_doc = core._strict_json(protocol, "protocol")
    profile = acquisition.execution_profile_from_protocol(
        protocol_doc["acquisition"],
        pool_input_mode=acquisition.pool_input_mode_for_stage(protocol_doc, "development"),
    )
    stage_uuid = str(uuid.UUID(int=90_001))
    packet_stage_uuid = str(uuid.UUID(int=90_002))
    cohorts = b"registered saved cohorts"
    candidate = b"candidate identity"
    acquisition_plan = b"registered saved acquisition plan"
    source_closure_sha256 = _sha(b"closure")
    preacquisition_input = {
        "schema": "coverage-preacquisition-input-manifest/1",
        "stage_uuid": stage_uuid,
        "protocol_sha256": _sha(protocol),
        "cohorts_sha256": _sha(cohorts),
        "task_ids": [*(row["task_id"] for row in cases), *(row["target_id"] for row in navigation)],
    }
    manifest = {
        "schema": "coverage-saved-import-plan/1",
        "stage": "development",
        "stage_uuid": stage_uuid,
        "source_revision": "b" * 40,
        "source_closure_sha256": source_closure_sha256,
        "protocol_sha256": _sha(protocol),
        "cohorts_sha256": _sha(cohorts),
        "input_manifest_sha256": _sha(core._canonical(preacquisition_input)),
        "acquisition_plan_sha256": _sha(acquisition_plan),
        "research_cases": list(cases),
        "navigation_targets": list(navigation),
        "packet_stage_uuid": packet_stage_uuid,
        "origin_package_sha256": origin.expected_package_sha256,
    }
    if profile is acquisition.CONTINUE_ACQUISITION_PROFILE:
        manifest["execution_profile"] = profile.identifier
    manifest_bytes = core._canonical(manifest)
    plan = orchestration.StagePlan(
        stage_uuid=stage_uuid,
        stage_kind="development",
        source_revision=manifest["source_revision"],
        protocol_bytes=protocol,
        cohorts_bytes=cohorts,
        acquisition_plan_bytes=acquisition_plan,
        acquisition_manifest_bytes=manifest_bytes,
        source_closure_sha256=source_closure_sha256,
        research_cases=tuple(cases),
        navigation_targets=tuple(navigation),
        candidate_identity_bytes=candidate,
        candidate_endpoint_sha256=origin.registration.candidate_endpoint_sha256,
        packet_stage_uuid=packet_stage_uuid,
        stage_started_monotonic=1.0,
        stage_deadline_monotonic=10_000.0,
        expected_acquisition_manifest_sha256=_sha(manifest_bytes),
        pool_input_mode="saved-corpus/1",
        saved_origin=origin,
    )
    return plan, manifest


@pytest.mark.asyncio
async def test_saved_stage_coordinator_imports_without_live_acquisition_or_capture(tmp_path, monkeypatch):
    origin, cases, _navigation, _collection, _capture = _origin_package(tmp_path)
    binding = _current_binding(origin)
    imported = saved.verify_and_import_saved_corpus(origin, binding, cases, list(origin.navigation_targets))
    stage_uuid = binding.stage_uuid
    packet_stage_uuid = binding.packet_stage_uuid
    manifest = b"saved-acquisition-manifest"
    plan = orchestration.StagePlan(
        stage_uuid=stage_uuid,
        stage_kind="development",
        source_revision=origin.registration.source_revision,
        protocol_bytes=b"stage-specific saved protocol",
        cohorts_bytes=b"registered cohort bytes",
        acquisition_plan_bytes=b"registered acquisition plan",
        acquisition_manifest_bytes=manifest,
        source_closure_sha256=binding.source_closure_sha256,
        research_cases=tuple(cases),
        navigation_targets=tuple(origin.navigation_targets),
        candidate_identity_bytes=b"fresh candidate identity",
        candidate_endpoint_sha256=binding.candidate_endpoint_sha256,
        packet_stage_uuid=packet_stage_uuid,
        stage_started_monotonic=1.0,
        stage_deadline_monotonic=10_000.0,
        expected_acquisition_manifest_sha256=_sha(manifest),
        pool_input_mode="saved-corpus/1",
        saved_origin=origin,
    )

    # Keep this test about real coordinator dispatch and receipt handling; the
    # source-bound static-plan validator is exercised in its own contract tests.
    monkeypatch.setattr(
        orchestration,
        "_validate_plan",
        lambda _plan: {
            "protocol_sha256": _sha(plan.protocol_bytes),
            "cohorts_sha256": _sha(plan.cohorts_bytes),
            "acquisition_plan_sha256": _sha(plan.acquisition_plan_bytes),
            "task_ids": tuple(row["task_id"] for row in cases)
            + tuple(row["target_id"] for row in origin.navigation_targets),
            "source_closure_sha256": plan.source_closure_sha256,
            "acquisition_manifest_sha256": _sha(plan.acquisition_manifest_bytes),
            "candidate_identity_sha256": _sha(plan.candidate_identity_bytes),
            "candidate_endpoint_sha256": plan.candidate_endpoint_sha256,
            "thresholds_sha256": "a" * 64,
        },
    )
    monkeypatch.setattr(orchestration, "_monotonic", lambda: 2.0)
    inventory_root = tmp_path / "inventory-root"
    inventory_root.mkdir(mode=0o700)
    os.chmod(inventory_root, 0o700)
    calls: list[str] = []

    async def import_saved(_plan):
        calls.append("saved-import")
        return orchestration.SavedCorpusStageEvidence(origin, imported, tmp_path)

    async def forbidden(name):
        calls.append(name)
        raise AssertionError(f"saved development stage invoked live {name}")

    async def stop_after_late_preflight(*_args):
        calls.append("late-preflight")
        raise RuntimeError("intentional offline stop before grading")

    executors = orchestration.StageExecutors(
        acquire=lambda _plan: forbidden("acquisition"),
        verify_acquisition=lambda *_args: forbidden("acquisition-verification"),
        capture=lambda *_args: forbidden("capture"),
        prepare_after_acquisition=lambda *_args: forbidden("live-late-preflight"),
        verify_late_preflight=lambda *_args: forbidden("late-preflight-verification"),
        grade_references=lambda *_args: forbidden("reference-grading"),
        selectors=lambda *_args: forbidden("selectors"),
        answerer=lambda *_args: forbidden("answerer"),
        grade_answers=lambda *_args: forbidden("answer-grading"),
        import_saved_corpus=import_saved,
        prepare_after_saved_corpus=stop_after_late_preflight,
    )

    await orchestration.coordinate_coverage_stage(plan=plan, executors=executors, inventory_root=inventory_root)

    assert calls == ["saved-import", "late-preflight"]
    inventory = __import__("json").loads((inventory_root / stage_uuid / "inventory.json").read_bytes())
    assert inventory["phases"][0]["phase"] == "saved_corpus_import"
    assert inventory["phases"][0]["status"] == "complete"
    assert inventory["phases"][1]["status"] == "complete"
    assert inventory["phases"][2]["status"] == "terminal-failure"
    assert inventory["phases"][3]["phase"] == "saved_capture_import"
    assert inventory["phases"][3]["status"] == "not-invoked"
    assert imported.current_search_calls == imported.current_capture_calls == 0


@pytest.mark.asyncio
async def test_saved_import_reference_reaches_selector_and_answer_permit_handoffs(tmp_path, monkeypatch):
    origin, cases, navigation, _collection, _capture = _origin_package(tmp_path)
    binding = _current_binding(origin)
    imported = saved.verify_and_import_saved_corpus(origin, binding, cases, navigation)
    assert binding.packet_stage_uuid != origin.registration.stage_uuid

    protocol = json.dumps(
        {
            "pool_input_mode": "frozen-pool/1",
            "pool_input_mode_by_stage": {
                "development": "saved-corpus/1",
                "confirmation": "frozen-pool/1",
            },
            "inputs": {"source_artifact_reuse": "development-only"},
        },
        sort_keys=True,
        separators=(",", ":"),
    ).encode()
    manifest = b"fresh-manifest"
    plan = orchestration.StagePlan(
        stage_uuid=binding.stage_uuid,
        stage_kind="development",
        source_revision=binding.source_revision,
        protocol_bytes=protocol,
        cohorts_bytes=b"cohorts",
        acquisition_plan_bytes=b"acquisition-plan",
        acquisition_manifest_bytes=manifest,
        source_closure_sha256=binding.source_closure_sha256,
        research_cases=tuple(cases),
        navigation_targets=tuple(navigation),
        candidate_identity_bytes=b"candidate",
        candidate_endpoint_sha256=binding.candidate_endpoint_sha256,
        packet_stage_uuid=binding.packet_stage_uuid,
        stage_started_monotonic=1.0,
        stage_deadline_monotonic=10_000.0,
        expected_acquisition_manifest_sha256=_sha(manifest),
        pool_input_mode="saved-corpus/1",
        saved_origin=origin,
    )
    reference = late.saved_corpus_import_reference(imported, plan=plan)
    runner = object.__new__(operator_runner.OperatorStageRunner)
    runner.plan = plan
    runner.active_environment_receipt_bytes = b'{"schema":"coverage-active-python-environment/1"}'
    runner.static_materials = {"qualified_source_closure": b"closure"}
    runner.initial_registration = b"initial-registration"
    runner._active_registration_bytes = b"active-registration"
    runner._active_input_manifest_bytes = b"input-manifest"
    runner._pipeline_inputs = dict(imported.pipeline_inputs)
    runner._frozen_pool_reference_bytes = None
    runner._saved_corpus_reference_bytes = reference
    runner._frozen_stage_clock_bytes = b"stage-clock"
    runner.config = {"answer_endpoint": "https://answers.example.test/v1", "allow_trusted_private_http": False}
    runner.private_paths = {"answer_api_key": "unused"}
    capture_root = tmp_path / "capture"
    capture_dir = capture_root / plan.stage_uuid
    selector_dir = tmp_path / "selector"
    capture_dir.mkdir(parents=True)
    selector_dir.mkdir()
    inventory_bytes = imported.capture_inventory_bytes
    terminal_bytes = b'{"status":"all-registered-calls-complete"}'
    (capture_dir / "inventory.json").write_bytes(inventory_bytes)
    (selector_dir / f"{plan.stage_uuid}.terminal-inventory.json").write_bytes(terminal_bytes)
    os.chmod(capture_dir / "inventory.json", 0o600)
    os.chmod(selector_dir / f"{plan.stage_uuid}.terminal-inventory.json", 0o600)
    runner.directories = {
        "capture_receipts": capture_root,
        "selector_results": selector_dir,
        "lease_root": tmp_path / "leases",
        "answer_archive": tmp_path / "answer-archive",
        "answer_results": tmp_path / "answer-results",
    }
    runner.started_utc = __import__("datetime").datetime.now(__import__("datetime").timezone.utc)
    runner.deadline_utc = runner.started_utc + __import__("datetime").timedelta(hours=8)

    seen: list[tuple[str, dict[str, object], dict[str, bytes]]] = []
    permit = b"selector-permit"
    operation_id = "navigation-nav-01"
    operation_map = operator_runner._canonical({"operations": [{"operation_id": operation_id}]})
    batch = operator_runner._canonical(
        {
            "schema": operator_runner._BATCH_SCHEMA,
            "stage_uuid": plan.stage_uuid,
            "selector_input_map_sha256": _sha(operation_map),
            "operations": [
                {
                    "operation_id": operation_id,
                    "permit_base64": __import__("base64").b64encode(permit).decode(),
                    "permit_sha256": _sha(permit),
                }
            ],
        }
    )

    class FakeHandoff:
        async def request_async(self, *, stage_uuid, scope, request_id, bindings, deadline_monotonic, artifacts):
            assert stage_uuid == plan.stage_uuid
            assert deadline_monotonic == plan.stage_deadline_monotonic
            seen.append((scope, bindings, artifacts or {}))
            if scope == "selector-operation-permits":
                return batch, _sha(batch)
            return b"answer-permit", _sha(b"answer-permit")

    runner.handoff = FakeHandoff()
    runner._request = operator_runner.OperatorStageRunner._request.__get__(runner)
    monkeypatch.setattr(operator_runner, "_secret", lambda _path: "unused-secret")
    prepared = SimpleNamespace(registration_sha256="c" * 64, operation_ids=(operation_id,))
    await runner._selector_permits(
        plan,
        prepared,
        operation_map,
        _sha(operation_map),
        {operation_id: {"operation_input_bytes": b"source-bound operation input"}},
    )
    selector_scope, selector_bindings, selector_artifacts = seen[-1]
    assert selector_scope == "selector-operation-permits"
    assert selector_bindings["source_closure_sha256"] == plan.source_closure_sha256
    assert selector_artifacts["saved-corpus-import-reference"] == reference
    assert selector_artifacts["capture-manifest"] == imported.pipeline_inputs["capture_manifest_bytes"]

    monkeypatch.setattr(
        operator_runner.answer,
        "_make_requests",
        lambda _tasks: (SimpleNamespace(operation_id="readiness-1"),),
    )
    monkeypatch.setattr(
        operator_runner.answer,
        "_request_manifest",
        lambda *_args, **_kwargs: (b"request-manifest", "d" * 64),
    )
    await runner._answer_authority(plan, ())
    answer_scope, answer_bindings, answer_artifacts = seen[-1]
    assert answer_scope == "answer-execution"
    assert answer_bindings["source_closure_sha256"] == plan.source_closure_sha256
    source_reference = json.loads(answer_artifacts["answer-source-reference"])
    assert source_reference["capture_inventory_sha256"] == _sha(inventory_bytes)
    assert source_reference["selector_terminal_inventory_sha256"] == _sha(terminal_bytes)
    assert answer_artifacts["saved-corpus-import-reference"] == reference


@pytest.mark.asyncio
async def test_saved_selector_map_preflight_failure_stops_before_reference_grading(tmp_path, monkeypatch):
    """Exercise the saved-only map preflight; this is not a grading or qualification test."""
    origin, cases, navigation, _collection, _capture = _origin_package(tmp_path)
    binding = _current_binding(origin)
    imported = saved.verify_and_import_saved_corpus(origin, binding, cases, navigation)
    manifest = b"saved-acquisition-manifest"
    plan = orchestration.StagePlan(
        stage_uuid=binding.stage_uuid,
        stage_kind="development",
        source_revision=binding.source_revision,
        protocol_bytes=b"source-pinned protocol bytes",
        cohorts_bytes=b"source-pinned cohort bytes",
        acquisition_plan_bytes=b"source-pinned plan bytes",
        acquisition_manifest_bytes=manifest,
        source_closure_sha256=binding.source_closure_sha256,
        research_cases=tuple(cases),
        navigation_targets=tuple(navigation),
        candidate_identity_bytes=b"candidate identity",
        candidate_endpoint_sha256=binding.candidate_endpoint_sha256,
        packet_stage_uuid=binding.packet_stage_uuid,
        stage_started_monotonic=1.0,
        stage_deadline_monotonic=10_000.0,
        expected_acquisition_manifest_sha256=_sha(manifest),
        pool_input_mode="saved-corpus/1",
        saved_origin=origin,
    )
    monkeypatch.setattr(
        orchestration,
        "_validate_plan",
        lambda _plan: {
            "protocol_sha256": _sha(plan.protocol_bytes),
            "cohorts_sha256": _sha(plan.cohorts_bytes),
            "acquisition_plan_sha256": _sha(plan.acquisition_plan_bytes),
            "task_ids": tuple(row["task_id"] for row in cases) + tuple(row["target_id"] for row in navigation),
            "source_closure_sha256": plan.source_closure_sha256,
            "acquisition_manifest_sha256": _sha(plan.acquisition_manifest_bytes),
            "candidate_identity_sha256": _sha(plan.candidate_identity_bytes),
            "candidate_endpoint_sha256": plan.candidate_endpoint_sha256,
            "thresholds_sha256": "a" * 64,
        },
    )
    monkeypatch.setattr(orchestration, "_monotonic", lambda: 2.0)
    monkeypatch.setattr(orchestration, "_validate_late_preparation", lambda *_args: None)
    monkeypatch.setattr(orchestration, "_selector_map_build_kwargs", lambda *_args: {})

    def reject_map_preflight(**_kwargs):
        raise ValueError("offline synthetic map preflight failure")

    monkeypatch.setattr(coverage_selector_input_map, "build_selector_input_map", reject_map_preflight)
    inventory_root = tmp_path / "inventory-root"
    inventory_root.mkdir(mode=0o700)
    os.chmod(inventory_root, 0o700)
    called: list[str] = []

    async def import_saved(_plan):
        called.append("saved-import")
        return orchestration.SavedCorpusStageEvidence(origin, imported, tmp_path)

    async def prepare_saved(_plan, _evidence, _pipeline_inputs):
        called.append("late-preflight")
        input_bytes = b"source-bound task input manifest"
        prepared = SimpleNamespace()
        return orchestration.LatePreparationEvidence(prepared, input_bytes, "b" * 64)

    async def never_grade(_packets):
        called.append("reference-grading")
        raise AssertionError("reference grading must not run after map preflight fails")

    async def never_dispatch(name):
        called.append(name)
        raise AssertionError(f"unexpected downstream invocation: {name}")

    executors = orchestration.StageExecutors(
        acquire=lambda _plan: never_dispatch("acquisition"),
        verify_acquisition=lambda *_args: never_dispatch("acquisition-verification"),
        capture=lambda *_args: never_dispatch("live-capture"),
        prepare_after_acquisition=lambda *_args: never_dispatch("live-preflight"),
        verify_late_preflight=lambda *_args: True,
        grade_references=never_grade,
        selectors=lambda *_args: never_dispatch("selectors"),
        answerer=lambda *_args: never_dispatch("answerer"),
        grade_answers=lambda *_args: never_dispatch("answer-grading"),
        import_saved_corpus=import_saved,
        prepare_after_saved_corpus=prepare_saved,
    )

    await orchestration.coordinate_coverage_stage(plan=plan, executors=executors, inventory_root=inventory_root)

    assert called == ["saved-import", "late-preflight"]
    inventory = json.loads((inventory_root / plan.stage_uuid / "inventory.json").read_bytes())
    assert inventory["phases"][0]["status"] == "complete"
    assert inventory["phases"][1]["status"] == "complete"
    assert inventory["phases"][2]["status"] == "complete"
    assert inventory["phases"][3]["phase"] == "saved_capture_import"
    assert inventory["phases"][3]["status"] == "complete"
    assert inventory["phases"][4]["status"] == "terminal-failure"
    assert inventory["phases"][5]["status"] == "not-invoked"
    assert imported.current_search_calls == imported.current_capture_calls == 0


def test_saved_late_materials_validate_saved_import_plan_without_live_profile_manifest(tmp_path):
    origin, cases, navigation, _collection, _capture = _origin_package(tmp_path)
    # The shared synthetic acquisition fixture labels navigation operations from
    # zero while its registered target rows start at one. Use the exact recorded
    # operation query/result pairs for this post-acquisition reconstruction test.
    planned_navigation = [
        {
            **row,
            "search_query": f"synthetic nav {index}",
            "target_url": f"https://github.com/example/{index}",
        }
        for index, row in enumerate(navigation)
    ]
    plan, manifest = _saved_plan_and_manifest(origin, cases, planned_navigation)
    import_binding = replace(
        _current_binding(origin),
        stage_uuid=plan.stage_uuid,
        packet_stage_uuid=plan.packet_stage_uuid,
        source_revision=plan.source_revision,
        source_closure_sha256=plan.source_closure_sha256,
        protocol_sha256=_sha(plan.protocol_bytes),
    )
    imported = saved.verify_and_import_saved_corpus(origin, import_binding, cases, navigation)
    static_materials = {
        name: f"static:{name}".encode()
        for name in set(core.PIN_NAMES) - {"task_input_manifest", "source_capture_manifest"}
    }
    static_materials.update(
        {
            "protocol": plan.protocol_bytes,
            "qualified_source_closure": b"closure",
            "acquisition_plan": plan.acquisition_plan_bytes,
        }
    )
    materials = late.build_postacquisition_materials(
        plan=plan,
        acquisition_evidence=None,
        pipeline_inputs=imported.pipeline_inputs,
        static_materials=static_materials,
        saved_corpus_import=imported,
    )
    assert materials["protocol"] == plan.protocol_bytes
    assert materials["source_capture_manifest"] == imported.capture_manifest_bytes
    task_manifest = core._strict_json(materials["task_input_manifest"], "task-input-manifest")
    assert task_manifest["stage_uuid"] == plan.stage_uuid

    tampered = dict(manifest)
    tampered["execution_profile"] = "coverage-acquisition-profile/1"
    tampered_bytes = core._canonical(tampered)
    with pytest.raises(late.LateRegistrationError, match="postacquisition-profile-binding-invalid"):
        late.build_postacquisition_materials(
            plan=replace(
                plan,
                acquisition_manifest_bytes=tampered_bytes,
                expected_acquisition_manifest_sha256=_sha(tampered_bytes),
            ),
            acquisition_evidence=None,
            pipeline_inputs=imported.pipeline_inputs,
            static_materials=static_materials,
            saved_corpus_import=imported,
        )
