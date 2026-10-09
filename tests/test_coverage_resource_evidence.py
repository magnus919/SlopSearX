from __future__ import annotations

import asyncio
import hashlib
import json
from pathlib import Path

import httpx
import pytest

from scripts import coverage_answer_execution as answer_execution
from scripts import coverage_resource_evidence as resource_evidence
from tests.test_coverage_answer_execution import _answer_content, _response, _stage_inputs
from tests.test_coverage_grade_closure import _answer_outputs, _closed_prepared, _pre_submissions
from tests.test_coverage_live_acquire import FakePermitVerifier, OneShot, make_manifest
from tests.test_coverage_live_acquire import _run as _run_acquisition
from tests.test_coverage_source_capture import (
    _candidate_identity,
    _capture,
    _healthy_response,
    _source,
)
from tests.test_coverage_source_capture import (
    _response as capture_response,
)


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def test_expired_collection_stops_before_artifact_access_and_restores_context(monkeypatch):
    from types import SimpleNamespace

    monkeypatch.setattr(resource_evidence, "time", SimpleNamespace(monotonic=lambda: 10.0))
    with pytest.raises(resource_evidence.ResourceEvidenceError, match="resource-collection-deadline-exceeded"):
        resource_evidence.collect_resource_evidence(deadline_monotonic=9.0)
    assert resource_evidence._DEADLINE.get() is None


def test_collection_checks_deadline_after_each_bounded_read(monkeypatch):
    from types import SimpleNamespace

    now = [1.0]
    reads = []
    monkeypatch.setattr(resource_evidence, "time", SimpleNamespace(monotonic=lambda: now[0]))

    def slow_read(path, *, max_bytes):
        reads.append((path, max_bytes))
        now[0] = 3.0
        return b"{}"

    monkeypatch.setattr(resource_evidence.receipts, "_read_private", slow_read)
    token = resource_evidence._DEADLINE.set(2.0)
    try:
        with pytest.raises(resource_evidence.ResourceEvidenceError, match="resource-collection-deadline-exceeded"):
            resource_evidence._read_private(Path("synthetic.json"), max_bytes=10)
        with pytest.raises(resource_evidence.ResourceEvidenceError, match="resource-collection-deadline-exceeded"):
            resource_evidence._read_private(Path("must-not-read.json"), max_bytes=10)
        assert reads == [(Path("synthetic.json"), 10)]
    finally:
        resource_evidence._DEADLINE.reset(token)


@pytest.mark.asyncio
async def test_capture_observations_replay_actual_mock_response_receipts(tmp_path: Path):
    protocol_path = Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study/protocol.json"
    protocol_bytes = protocol_path.read_bytes()
    protocol = json.loads(protocol_bytes)
    sources = [
        _source("D-R01", "source-local", 1, "file:///private/not-captured"),
        _source("D-R01", "source-a", 2, "https://docs.example/a"),
    ]

    def handler(request: httpx.Request):
        if request.method == "GET":
            return _healthy_response()
        return httpx.Response(200, content=capture_response("A short implementation passage."))

    result = await _capture(tmp_path=tmp_path, transport=httpx.MockTransport(handler), sources=sources)
    root = result.receipt_directory
    inventory_bytes = (root / "inventory.json").read_bytes()
    manifest_bytes = (root / "source-manifest.json").read_bytes()
    identity = _candidate_identity()
    manifest = json.loads(manifest_bytes)
    observed = resource_evidence._capture_observations(
        capture_result=result,
        expected_inventory_sha256=_sha(inventory_bytes),
        expected_manifest_sha256=_sha(manifest_bytes),
        protocol_sha256=_sha(protocol_bytes),
        cohorts_sha256=protocol["cohorts_sha256"],
        source_revision=manifest["source_revision"],
        candidate_identity_bytes=identity,
        candidate_identity_sha256=_sha(identity),
        candidate_endpoint_sha256=manifest["candidate_endpoint_sha256"],
    )
    assert observed["capture_owned_http_calls"] == 2
    assert observed["capture_health_calls"] == 1
    assert observed["capture_scrape_calls"] == 1
    assert observed["capture_attempted_unique_public_urls"] == 1
    assert observed["capture_max_response_body_bytes_observed"] == max(
        len(_healthy_response().content), len(capture_response("A short implementation passage."))
    )
    assert observed["capture_max_context_characters_observed"] == len("A short implementation passage.")
    assert observed["capture_internal_fanout"] is None

    altered = json.loads(inventory_bytes)
    altered["owned_http_calls"] = 0
    (root / "inventory.json").write_text(json.dumps(altered))
    with pytest.raises(resource_evidence.ResourceEvidenceError, match="capture-artifact-pin-mismatch"):
        resource_evidence._capture_observations(
            capture_result=result,
            expected_inventory_sha256=_sha(inventory_bytes),
            expected_manifest_sha256=_sha(manifest_bytes),
            protocol_sha256=_sha(protocol_bytes),
            cohorts_sha256=protocol["cohorts_sha256"],
            source_revision=manifest["source_revision"],
            candidate_identity_bytes=identity,
            candidate_identity_sha256=_sha(identity),
            candidate_endpoint_sha256=manifest["candidate_endpoint_sha256"],
        )


def test_acquisition_observations_bind_real_mock_exchange_receipts_and_stage(tmp_path: Path):
    manifest, acquisition_plan = make_manifest()
    evidence, _verifier, _lease, _pacer, _calls = asyncio.run(_run_acquisition(tmp_path))
    receipt_sha = resource_evidence.receipt_inventory_sha256(evidence.receipt_directory)
    observed = resource_evidence.collect_acquisition_observations(
        snapshots_directory=evidence.receipt_directory,
        receipt_directory=evidence.receipt_directory,
        expected_receipt_inventory_sha256=receipt_sha,
        acquisition_manifest_bytes=resource_evidence.coverage_live_acquire._canonical(manifest),
        acquisition_plan_bytes=acquisition_plan,
        expected_index_sha256=evidence.pool_snapshot_index_sha256,
        expected_manifest_sha256=evidence.stage_manifest_sha256,
        stage_uuid=manifest["stage_uuid"],
        source_revision=manifest["source_revision"],
        protocol_sha256=manifest["protocol_sha256"],
        cohorts_sha256=manifest["cohorts_sha256"],
        source_closure_sha256=manifest["source_closure_sha256"],
        acquisition_plan_sha256=_sha(acquisition_plan),
    )
    assert observed["acquisition_physical_http_calls"] == evidence.stage.physical_request_count
    assert sum(observed["acquisition_engine_calls"].values()) == evidence.stage.physical_request_count
    assert observed["acquisition_engine_calls"]["brave"] == 0
    assert observed["acquisition_response_bytes_total"] > 0
    final_receipt = evidence.receipt_directory / "http-0001-final.json"
    final_receipt.write_bytes(final_receipt.read_bytes() + b" ")
    with pytest.raises(resource_evidence.ResourceEvidenceError, match="acquisition-receipt-inventory-invalid"):
        resource_evidence.collect_acquisition_observations(
            snapshots_directory=evidence.receipt_directory,
            receipt_directory=evidence.receipt_directory,
            expected_receipt_inventory_sha256=receipt_sha,
            acquisition_manifest_bytes=resource_evidence.coverage_live_acquire._canonical(manifest),
            acquisition_plan_bytes=acquisition_plan,
            expected_index_sha256=evidence.pool_snapshot_index_sha256,
            expected_manifest_sha256=evidence.stage_manifest_sha256,
            stage_uuid=manifest["stage_uuid"],
            source_revision=manifest["source_revision"],
            protocol_sha256=manifest["protocol_sha256"],
            cohorts_sha256=manifest["cohorts_sha256"],
            source_closure_sha256=manifest["source_closure_sha256"],
            acquisition_plan_sha256=_sha(acquisition_plan),
        )


def test_answer_observations_replay_mock_archives_and_reject_wrong_stage(tmp_path: Path):
    args = _stage_inputs(tmp_path)
    dispatches = []
    by_id = {task.task_id: task for task in args["tasks"]}

    async def handler(request):
        payload = json.loads(request.content)
        user = payload["messages"][1]["content"]
        dispatches.append(request)
        if user.startswith("This is a synthetic endpoint readiness check"):
            return _response('{"ready":true}')
        task_id = json.loads(user)["task_id"]
        return _response(_answer_content(by_id[task_id]))

    args["transport"] = httpx.MockTransport(handler)
    result = asyncio.run(answer_execution.execute_answer_stage(**args))
    terminal_path = args["result_root"] / f"{args['stage_uuid']}.answer-terminal-inventory.json"
    terminal_sha = _sha(terminal_path.read_bytes())
    observed = resource_evidence._answer_observations(
        stage_uuid=args["stage_uuid"],
        source_revision=args["source_revision"],
        protocol_sha256=args["protocol_sha256"],
        cohorts_sha256=args["cohorts_sha256"],
        answer_result=result,
        answer_tasks=args["tasks"],
        answer_archive_root=args["archive_root"],
        answer_result_root=args["result_root"],
        expected_terminal_sha256=terminal_sha,
        expected_manifest_sha256=args["answer_manifest_sha256"],
    )
    assert observed["answerer_calls"] == 18
    assert observed["answerer_max_request_body_bytes_observed"] > 0
    assert observed["answerer_max_response_body_bytes_observed"] > 0
    assert observed["answerer_max_words_observed"] > 0
    with pytest.raises(resource_evidence.ResourceEvidenceError, match="answer-stage-binding-mismatch"):
        resource_evidence._answer_observations(
            stage_uuid="00000000-0000-0000-0000-000000000000",
            source_revision=args["source_revision"],
            protocol_sha256=args["protocol_sha256"],
            cohorts_sha256=args["cohorts_sha256"],
            answer_result=result,
            answer_tasks=args["tasks"],
            answer_archive_root=args["archive_root"],
            answer_result_root=args["result_root"],
            expected_terminal_sha256=terminal_sha,
            expected_manifest_sha256=args["answer_manifest_sha256"],
        )


def test_grade_observations_reclose_actual_mock_submissions():
    prepared, preclosure, prepared_answers = _closed_prepared()
    pre_submissions = _pre_submissions(prepared)
    answer_submissions = _answer_outputs(prepared_answers, prepared)
    from scripts import coverage_grade_closure

    answer_closure = coverage_grade_closure.close_answer_assessments(
        prepared,
        prepared_answers,
        preclosure,
        answer_submissions,
        expected_preassessment_manifest_sha256=_sha(prepared.preassessment_manifest_bytes),
        expected_private_binding_sha256=prepared.private_binding_sha256,
        expected_answer_manifest_sha256=prepared_answers.manifest_sha256,
    )
    values = resource_evidence._grade_observations(
        {
            "prepared_references": prepared,
            "reference_submissions": pre_submissions,
            "reference_closure": preclosure,
            "expected_preassessment_manifest_sha256": _sha(prepared.preassessment_manifest_bytes),
            "expected_private_binding_sha256": prepared.private_binding_sha256,
            "prepared_answers": prepared_answers,
            "answer_submissions": answer_submissions,
            "answer_closure": answer_closure,
            "expected_answer_assessment_manifest_sha256": prepared_answers.manifest_sha256,
            "expected_reference_closure_sha256": preclosure.receipt_sha256,
            "expected_answer_closure_sha256": answer_closure.receipt_sha256,
        }
    )
    assert values["grader_submissions"] == len(preclosure.submission_receipts) + len(answer_closure.submission_receipts)
    assert values["max_grader_request_body_bytes_observed"] > 0
    assert values["max_grader_response_body_bytes_observed"] > 0


def test_grade_resource_totals_exclude_prepared_no_call_packets():
    from dataclasses import replace

    from scripts import coverage_assessment_packets as packet_prep
    from scripts import coverage_grade_closure as closure
    from tests.test_coverage_assessment_packets import _answer_stage, _fixture

    prepared, captures = _fixture()
    base = next(row for row in prepared.preassessment_packets if row["role"] == "source")
    envelope = json.loads(base["bytes"])
    envelope["packet_id"] = "synthetic-unavailable-no-call"
    envelope["model_input"]["chunk_id"] = "synthetic-unavailable-chunk"
    envelope["model_input"]["model_call_required"] = False
    envelope["model_input"]["source_inventory"] = [{"source_id": "s-missing", "state": "not_acquired"}]
    envelope["model_input"]["sources"] = []
    envelope["output_schema"] = closure._derive_preassessment_schema(envelope, "source")
    raw = json.dumps(envelope, sort_keys=True).encode()
    no_call = {
        "packet_id": envelope["packet_id"],
        "task_id": base["task_id"],
        "assessor_id": base["assessor_id"],
        "role": "source",
        "bytes": raw,
        "sha256": _sha(raw),
        "byte_count": len(raw),
    }
    manifest = json.loads(prepared.preassessment_manifest_bytes)
    manifest["packets"].append(
        {key: no_call[key] for key in ("packet_id", "task_id", "assessor_id", "role", "sha256", "byte_count")}
    )
    manifest["grader_packet_count"] += 1
    manifest_bytes = json.dumps(manifest, sort_keys=True).encode()
    binding = json.loads(prepared.private_binding_bytes)
    binding["manifest_sha256"] = _sha(manifest_bytes)
    binding_bytes = json.dumps(binding, sort_keys=True).encode()
    prepared = replace(
        prepared,
        preassessment_packets=(*prepared.preassessment_packets, no_call),
        preassessment_manifest_bytes=manifest_bytes,
        private_binding_bytes=binding_bytes,
        private_binding_sha256=_sha(binding_bytes),
    )
    pre_submissions = _pre_submissions(prepared)
    pins = {
        "expected_preassessment_manifest_sha256": _sha(manifest_bytes),
        "expected_private_binding_sha256": _sha(binding_bytes),
    }
    pre = closure.close_preassessment(
        prepared,
        pre_submissions,
        expected_manifest_sha256=_sha(manifest_bytes),
        expected_private_binding_sha256=_sha(binding_bytes),
    )
    inputs, outputs, _ = _answer_stage(prepared, captures)
    answers = packet_prep.prepare_answer_assessment_packets(
        prepared=prepared,
        answer_inputs=inputs,
        answer_outputs=outputs,
        source_outputs=pre.source_outputs_for_answer(),
    )
    answer_submissions = _answer_outputs(answers, prepared)
    closed_answers = closure.close_answer_assessments(
        prepared,
        answers,
        pre,
        answer_submissions,
        **pins,
        expected_answer_manifest_sha256=answers.manifest_sha256,
    )
    observed = resource_evidence._grade_observations(
        {
            **pins,
            "prepared_references": prepared,
            "reference_submissions": pre_submissions,
            "reference_closure": pre,
            "expected_reference_closure_sha256": pre.receipt_sha256,
            "prepared_answers": answers,
            "answer_submissions": answer_submissions,
            "answer_closure": closed_answers,
            "expected_answer_closure_sha256": closed_answers.receipt_sha256,
            "expected_answer_assessment_manifest_sha256": answers.manifest_sha256,
        }
    )
    submitted_ids = {row.packet_id for row in (*pre.submission_receipts, *closed_answers.submission_receipts)}
    sizes = [
        len(row["bytes"])
        for row in (*prepared.preassessment_packets, *answers.packets)
        if row["packet_id"] in submitted_ids
    ]
    assert no_call["packet_id"] not in submitted_ids
    assert observed["grader_request_bytes_total"] == sum(sizes)
    assert observed["max_grader_request_body_bytes_observed"] == max(sizes)
    assert observed["grader_submissions"] == len(sizes)


@pytest.mark.asyncio
async def test_full_collector_binds_mock_stages_and_preserves_unknowns(tmp_path: Path):
    protocol_path = Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study/protocol.json"
    protocol_bytes = protocol_path.read_bytes()
    cohort_path = protocol_path.with_name("cohorts.json")
    cohorts_bytes = cohort_path.read_bytes()
    source_revision = "8f3577d022e2d98fcd405d915b5c3b9b16e899bf"
    stage_uuid = "f611a79a-9eef-46f4-b211-3d4d394e9e21"
    identity = _candidate_identity()
    source_rows = [_source("D-R01", "source-a", 1, "https://docs.example/a")]

    def capture_handler(request: httpx.Request):
        if request.method == "GET":
            return _healthy_response()
        return httpx.Response(200, content=capture_response("A captured implementation passage."))

    (tmp_path / "capture").mkdir()
    capture = await _capture(
        tmp_path=tmp_path / "capture",
        transport=httpx.MockTransport(capture_handler),
        sources=source_rows,
    )
    capture_manifest_bytes = (capture.receipt_directory / "source-manifest.json").read_bytes()
    capture_manifest = json.loads(capture_manifest_bytes)

    acquisition_manifest, acquisition_plan = make_manifest(source_revision=source_revision)
    acquisition_manifest.update(
        stage_uuid=stage_uuid,
        protocol_sha256=_sha(protocol_bytes),
        cohorts_sha256=_sha(cohorts_bytes),
        source_closure_sha256="1" * 64,
    )
    plan_doc = json.loads(acquisition_plan)
    plan_doc["stage_uuid"] = stage_uuid
    acquisition_plan = resource_evidence.coverage_live_acquire._canonical(plan_doc)
    acquisition_manifest["acquisition_plan_sha256"] = _sha(acquisition_plan)
    receipt_bytes = b"synthetic-admission"
    acquisition_root = tmp_path / "acquisition"
    acquisition = await resource_evidence.coverage_live_acquire.acquire_live_coverage_stage(
        stage_manifest=acquisition_manifest,
        acquisition_plan_bytes=acquisition_plan,
        permit_receipt_bytes=receipt_bytes,
        expected_permit_receipt_sha256=_sha(receipt_bytes),
        permit_verifier=FakePermitVerifier(),
        one_shot_lease=OneShot(),
        receipt_directory=acquisition_root,
        test_transport=httpx.MockTransport(
            __import__("tests.test_coverage_live_acquire", fromlist=["_mock_handler"])._mock_handler()
        ),
        pacer=__import__("tests.test_coverage_live_acquire", fromlist=["FakePacer"]).FakePacer(),
    )

    (tmp_path / "answer").mkdir()
    answer_args = _stage_inputs(tmp_path / "answer")
    answer_args.update(
        stage_uuid=stage_uuid,
        source_revision=source_revision,
        protocol_sha256=_sha(protocol_bytes),
        cohorts_sha256=_sha(cohorts_bytes),
    )
    by_id = {task.task_id: task for task in answer_args["tasks"]}

    async def answer_handler(request):
        user = json.loads(request.content)["messages"][1]["content"]
        if user.startswith("This is a synthetic endpoint readiness check"):
            return _response('{"ready":true}')
        task_id = json.loads(user)["task_id"]
        return _response(_answer_content(by_id[task_id]))

    answer_args["transport"] = httpx.MockTransport(answer_handler)
    answer_result = await answer_execution.execute_answer_stage(**answer_args)
    answer_terminal = answer_args["result_root"] / f"{stage_uuid}.answer-terminal-inventory.json"

    prepared, reference_closure, prepared_answers = _closed_prepared()
    reference_submissions = _pre_submissions(prepared)
    answer_submissions = _answer_outputs(prepared_answers, prepared)
    from scripts import coverage_grade_closure

    answer_closure = coverage_grade_closure.close_answer_assessments(
        prepared,
        prepared_answers,
        reference_closure,
        answer_submissions,
        expected_preassessment_manifest_sha256=_sha(prepared.preassessment_manifest_bytes),
        expected_private_binding_sha256=prepared.private_binding_sha256,
        expected_answer_manifest_sha256=prepared_answers.manifest_sha256,
    )
    acquisition_bytes = resource_evidence.coverage_live_acquire._canonical(acquisition_manifest)
    capture_inventory_bytes = (capture.receipt_directory / "inventory.json").read_bytes()
    capture_endpoint_sha = capture_manifest["candidate_endpoint_sha256"]
    report = resource_evidence.collect_resource_evidence(
        stage_uuid=stage_uuid,
        source_revision=source_revision,
        protocol_bytes=protocol_bytes,
        cohorts_bytes=cohorts_bytes,
        source_closure_sha256="1" * 64,
        acquisition_plan_bytes=acquisition_plan,
        snapshots_directory=acquisition.receipt_directory,
        receipt_directory=acquisition.receipt_directory,
        expected_receipt_inventory_sha256=resource_evidence.receipt_inventory_sha256(acquisition.receipt_directory),
        acquisition_manifest_bytes=acquisition_bytes,
        expected_snapshot_index_sha256=acquisition.pool_snapshot_index_sha256,
        acquisition_manifest_sha256=acquisition.stage_manifest_sha256,
        capture_result=capture,
        expected_capture_manifest_sha256=_sha(capture_manifest_bytes),
        expected_capture_inventory_sha256=_sha(capture_inventory_bytes),
        candidate_identity_bytes=identity,
        candidate_identity_sha256=_sha(identity),
        candidate_endpoint_sha256=capture_endpoint_sha,
        answer_result=answer_result,
        answer_tasks=answer_args["tasks"],
        answer_archive_root=answer_args["archive_root"],
        answer_result_root=answer_args["result_root"],
        expected_answer_terminal_sha256=_sha(answer_terminal.read_bytes()),
        expected_answer_operation_manifest_sha256=answer_args["answer_manifest_sha256"],
        prepared_references=prepared,
        reference_submissions=reference_submissions,
        reference_closure=reference_closure,
        expected_preassessment_manifest_sha256=_sha(prepared.preassessment_manifest_bytes),
        expected_private_binding_sha256=prepared.private_binding_sha256,
        prepared_answers=prepared_answers,
        answer_submissions=answer_submissions,
        answer_closure=answer_closure,
        expected_answer_assessment_manifest_sha256=prepared_answers.manifest_sha256,
        expected_reference_closure_sha256=reference_closure.receipt_sha256,
        expected_answer_closure_sha256=answer_closure.receipt_sha256,
    )
    assert report.stage_uuid == stage_uuid
    assert report.observations["acquisition_engine_calls"]["brave"] == 0
    assert report.observations["answerer_calls"] == 18
    assert report.observations["grader_submissions"] > 0
    assert report.observations["capture_internal_fanout"] is None
    assert report.observations["selector_elapsed_ms"] is None
    assert report.observations["stage_elapsed_seconds"] is None
    assert report.configuration_provenance["quality_credit"] is False
