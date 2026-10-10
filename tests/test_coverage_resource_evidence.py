from __future__ import annotations

import asyncio
import gzip
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
from tests.test_coverage_live_acquire import _mock_handler as acquisition_mock_handler
from tests.test_coverage_live_acquire import _run as _run_acquisition
from tests.test_coverage_source_capture import (
    _candidate_identity,
    _capture,
    _healthy_response,
    _QualificationVerifier,
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
async def test_capture_observations_replay_actual_mock_response_receipts(tmp_path: Path, monkeypatch):
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
        timeout_limit_seconds=protocol["capture"]["timeout_seconds"],
        response_bytes_limit=protocol["capture"]["response_bytes"],
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
    assert observed["capture_timeout_seconds"] == 30.0
    assert observed["capture_application_retries_observed"] == 0
    assert observed["capture_retries"] is None
    assert observed["capture_response_bytes_limit_applied"] == 2_000_000
    assert observed["capture_concurrency_observed"] == 1

    # The consumer reads and hashes producer source from disk. A changed
    # producer cannot validate an attestation written by the earlier source.
    producer_source = Path(resource_evidence.source_capture.__file__)
    changed_source = tmp_path / "changed-source-capture.py"
    changed_source.write_bytes(producer_source.read_bytes() + b"\n# source changed after receipt\n")
    monkeypatch.setattr(resource_evidence.source_capture, "__file__", str(changed_source))
    with pytest.raises(resource_evidence.ResourceEvidenceError, match="capture-control-attestation-binding"):
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
            timeout_limit_seconds=protocol["capture"]["timeout_seconds"],
            response_bytes_limit=protocol["capture"]["response_bytes"],
        )
    monkeypatch.setattr(resource_evidence.source_capture, "__file__", str(producer_source))

    # A newly resealed inventory cannot substitute a different producer module.
    forged_control = json.loads(inventory_bytes)
    forged_control["execution_control_attestation"]["producer_module_sha256"] = "0" * 64
    forged_control_bytes = resource_evidence.source_capture._canonical_json(forged_control)
    (root / "inventory.json").write_bytes(forged_control_bytes)
    with pytest.raises(resource_evidence.ResourceEvidenceError, match="capture-control-attestation-binding"):
        resource_evidence._capture_observations(
            capture_result=result,
            expected_inventory_sha256=_sha(forged_control_bytes),
            expected_manifest_sha256=_sha(manifest_bytes),
            protocol_sha256=_sha(protocol_bytes),
            cohorts_sha256=protocol["cohorts_sha256"],
            source_revision=manifest["source_revision"],
            candidate_identity_bytes=identity,
            candidate_identity_sha256=_sha(identity),
            candidate_endpoint_sha256=manifest["candidate_endpoint_sha256"],
            timeout_limit_seconds=protocol["capture"]["timeout_seconds"],
            response_bytes_limit=protocol["capture"]["response_bytes"],
        )
    # Historical inventories without this new producer receipt remain unknown.
    legacy_control = json.loads(inventory_bytes)
    legacy_control.pop("execution_control_attestation")
    legacy_control_bytes = resource_evidence.source_capture._canonical_json(legacy_control)
    (root / "inventory.json").write_bytes(legacy_control_bytes)
    legacy_observed = resource_evidence._capture_observations(
        capture_result=result,
        expected_inventory_sha256=_sha(legacy_control_bytes),
        expected_manifest_sha256=_sha(manifest_bytes),
        protocol_sha256=_sha(protocol_bytes),
        cohorts_sha256=protocol["cohorts_sha256"],
        source_revision=manifest["source_revision"],
        candidate_identity_bytes=identity,
        candidate_identity_sha256=_sha(identity),
        candidate_endpoint_sha256=manifest["candidate_endpoint_sha256"],
        timeout_limit_seconds=protocol["capture"]["timeout_seconds"],
        response_bytes_limit=protocol["capture"]["response_bytes"],
    )
    assert legacy_observed["capture_timeout_seconds"] is None
    assert legacy_observed["capture_retries"] is None

    pre_owned_inventory = json.loads(inventory_bytes)
    pre_owned_inventory["execution_control_attestation"].pop("transport_configuration")
    pre_owned_bytes = resource_evidence.source_capture._canonical_json(pre_owned_inventory)
    (root / "inventory.json").write_bytes(pre_owned_bytes)
    pre_owned_observed = resource_evidence._capture_observations(
        capture_result=result,
        expected_inventory_sha256=_sha(pre_owned_bytes),
        expected_manifest_sha256=_sha(manifest_bytes),
        protocol_sha256=_sha(protocol_bytes),
        cohorts_sha256=protocol["cohorts_sha256"],
        source_revision=manifest["source_revision"],
        candidate_identity_bytes=identity,
        candidate_identity_sha256=_sha(identity),
        candidate_endpoint_sha256=manifest["candidate_endpoint_sha256"],
        timeout_limit_seconds=protocol["capture"]["timeout_seconds"],
        response_bytes_limit=protocol["capture"]["response_bytes"],
    )
    assert pre_owned_observed["capture_retries"] is None


@pytest.mark.asyncio
async def test_owned_capture_controls_require_archived_qualification_binding(tmp_path: Path, monkeypatch):
    from scripts import coverage_source_capture as capture_module

    protocol_path = Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study/protocol.json"
    protocol_bytes = protocol_path.read_bytes()
    protocol = json.loads(protocol_bytes)
    identity = _candidate_identity()
    receipt = b"independently-verified-protected-profile"

    def handler(request: httpx.Request):
        if request.method == "GET":
            return _healthy_response()
        return httpx.Response(200, content=capture_response("Bounded local fixture."))

    monkeypatch.setattr(capture_module, "_owned_httpx_transport", lambda: httpx.MockTransport(handler))
    result = await _capture(
        tmp_path=tmp_path,
        transport=None,
        sources=[_source("D-R01", "source-a", 1, "https://docs.example/a")],
        candidate_base_url="https://capture.example",
        qualification_receipt_bytes=receipt,
        expected_qualification_receipt_sha256=_sha(receipt),
        qualification_verifier=_QualificationVerifier(),
    )
    root = result.receipt_directory
    inventory_bytes = (root / "inventory.json").read_bytes()
    manifest_bytes = (root / "source-manifest.json").read_bytes()
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
        timeout_limit_seconds=protocol["capture"]["timeout_seconds"],
        response_bytes_limit=protocol["capture"]["response_bytes"],
    )
    assert observed["capture_retries"] == 0
    assert observed["capture_application_retries_observed"] == 0
    assert observed["capture_response_bytes_limit_applied"] == 2_000_000
    assert result.candidate_endpoint_scheme == "https"

    qualification_path = root / "protected-capture-qualification.json"
    qualification_path.write_bytes(b"replacement receipt")
    with pytest.raises(resource_evidence.ResourceEvidenceError, match="capture-qualification-receipt-mismatch"):
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
            timeout_limit_seconds=protocol["capture"]["timeout_seconds"],
            response_bytes_limit=protocol["capture"]["response_bytes"],
        )

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
            timeout_limit_seconds=protocol["capture"]["timeout_seconds"],
            response_bytes_limit=protocol["capture"]["response_bytes"],
        )


@pytest.mark.asyncio
async def test_capture_observations_replay_pinned_gzip_health_and_source(tmp_path: Path):
    protocol_path = Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study/protocol.json"
    protocol_bytes = protocol_path.read_bytes()
    protocol = json.loads(protocol_bytes)
    protocol = json.loads(protocol_bytes)
    sources = [_source("D-R01", "source-a", 1, "https://docs.example/a")]
    health_body = gzip.compress(_healthy_response().content)
    source_body = gzip.compress(capture_response("Compressed implementation passage."))

    def handler(request: httpx.Request):
        if request.method == "GET":
            return httpx.Response(
                200,
                content=health_body,
                headers={"Content-Encoding": "gzip", "Content-Type": "application/json"},
            )
        return httpx.Response(
            200,
            content=source_body,
            headers={"Content-Encoding": "gzip", "Content-Type": "application/json"},
        )

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
        timeout_limit_seconds=protocol["capture"]["timeout_seconds"],
        response_bytes_limit=protocol["capture"]["response_bytes"],
    )

    inventory = json.loads(inventory_bytes)
    assert inventory["health_response_content_encoding"] == "gzip"
    assert inventory["sources"][0]["response_content_encoding"] == "gzip"
    assert inventory["health_response_body_bytes"] == len(health_body)
    assert inventory["sources"][0]["response_body_bytes"] == len(source_body)
    assert (root / "health" / "response.bin").read_bytes() == health_body
    assert (root / "source-0001" / "response.bin").read_bytes() == source_body
    assert observed["capture_health_runtime_match_observed"] is True
    assert observed["capture_max_context_characters_observed"] == len("Compressed implementation passage.")
    assert (root / str(inventory["sources"][0]["context_artifact"])).is_file()

    inventory["sources"][0]["response_content_encoding"] = "identity"
    (root / "inventory.json").write_text(json.dumps(inventory))
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
            timeout_limit_seconds=protocol["capture"]["timeout_seconds"],
            response_bytes_limit=protocol["capture"]["response_bytes"],
        )


def test_mock_acquisition_receipts_do_not_qualify_execution_controls(tmp_path: Path):
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
        configured_timeout_seconds=10,
        configured_query_pacing_seconds=7,
        configured_arxiv_pacing_seconds=3,
    )
    assert observed["acquisition_physical_http_calls"] == evidence.stage.physical_request_count
    assert sum(observed["acquisition_engine_calls"].values()) == evidence.stage.physical_request_count
    assert observed["acquisition_engine_calls"]["brave"] == 0
    assert observed["acquisition_response_bytes_total"] > 0
    assert observed["acquisition_timeout_seconds"] == 10.0
    assert set(observed["acquisition_timeout_seconds_by_exchange"]) == {10.0}
    assert observed["acquisition_timeout_observation_state"] == "observed-uniform"
    assert observed["acquisition_retries"] is None
    assert observed["acquisition_pacing_seconds"] is None
    assert observed["acquisition_query_min_idle_gap_microseconds_observed"] is None
    assert observed["arxiv_pacing_seconds"] is None
    assert observed["acquisition_arxiv_min_gap_microseconds_observed"] is None

    # The query invocation ledger and final HTTP receipt independently bind
    # timestamps. Re-sealing a modified control claim cannot alter the actual
    # dispatch timestamps in the transport receipts.
    summary_path = evidence.receipt_directory / "stage-summary.json"
    summary = json.loads(summary_path.read_bytes())
    summary["execution_control_attestation"]["physical_dispatch_offsets_us"][0]["dispatch_offset_us"] += 1
    summary_path.write_bytes(resource_evidence.coverage_live_acquire._canonical(summary))
    resealed_inventory = resource_evidence.receipt_inventory_sha256(evidence.receipt_directory)
    with pytest.raises(
        resource_evidence.ResourceEvidenceError,
        match="acquisition-dispatch-offset-receipt-mismatch",
    ):
        resource_evidence.collect_acquisition_observations(
            snapshots_directory=evidence.receipt_directory,
            receipt_directory=evidence.receipt_directory,
            expected_receipt_inventory_sha256=resealed_inventory,
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
            configured_timeout_seconds=10,
            configured_query_pacing_seconds=7,
            configured_arxiv_pacing_seconds=3,
        )
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
            configured_timeout_seconds=10,
            configured_query_pacing_seconds=7,
            configured_arxiv_pacing_seconds=3,
        )


def test_acquisition_timeout_observation_rejects_receipt_value_above_bound(tmp_path: Path):
    manifest, acquisition_plan = make_manifest()
    evidence, _verifier, _lease, _pacer, _calls = asyncio.run(_run_acquisition(tmp_path))
    start_path = evidence.receipt_directory / "http-0001-start.json"
    final_path = evidence.receipt_directory / "http-0001-final.json"
    for path in (start_path, final_path):
        row = json.loads(path.read_bytes())
        row["timeout_seconds"] = 11
        path.write_bytes(resource_evidence.coverage_live_acquire._canonical(row))
    receipt_sha = resource_evidence.receipt_inventory_sha256(evidence.receipt_directory)
    with pytest.raises(resource_evidence.ResourceEvidenceError, match="acquisition-exchange-timeout-invalid"):
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
            configured_timeout_seconds=10,
            configured_query_pacing_seconds=7,
            configured_arxiv_pacing_seconds=3,
        )


def test_mock_fake_clock_cannot_qualify_or_report_pacing(tmp_path: Path):
    class NoWaitPacer:
        def __init__(self):
            self.elapsed = 0.0

        async def sleep(self, _seconds):
            return None

        def monotonic(self):
            return self.elapsed

    manifest, acquisition_plan = make_manifest()
    evidence, _verifier, _lease, _pacer, _calls = asyncio.run(
        _run_acquisition(tmp_path, pacer=NoWaitPacer())
    )
    observed = resource_evidence.collect_acquisition_observations(
        snapshots_directory=evidence.receipt_directory,
        receipt_directory=evidence.receipt_directory,
        expected_receipt_inventory_sha256=resource_evidence.receipt_inventory_sha256(evidence.receipt_directory),
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
        configured_timeout_seconds=10,
        configured_query_pacing_seconds=7,
        configured_arxiv_pacing_seconds=3,
    )
    assert observed["acquisition_pacing_seconds"] is None
    assert observed["acquisition_query_min_idle_gap_microseconds_observed"] is None


def test_acquisition_collector_derives_arxiv_physical_gap_from_dispatch_receipts(tmp_path: Path):
    manifest, acquisition_plan = make_manifest(include_arxiv=True)
    evidence, _verifier, _lease, _pacer, _calls = asyncio.run(
        _run_acquisition(
            tmp_path,
            handler=acquisition_mock_handler(arxiv_redirect=True),
            include_arxiv=True,
        )
    )
    observed = resource_evidence.collect_acquisition_observations(
        snapshots_directory=evidence.receipt_directory,
        receipt_directory=evidence.receipt_directory,
        expected_receipt_inventory_sha256=resource_evidence.receipt_inventory_sha256(evidence.receipt_directory),
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
        configured_timeout_seconds=10,
        configured_query_pacing_seconds=7,
        configured_arxiv_pacing_seconds=3,
    )
    assert observed["acquisition_engine_calls"]["arxiv"] == 2
    assert observed["arxiv_pacing_seconds"] is None
    assert observed["acquisition_arxiv_min_gap_microseconds_observed"] is None


@pytest.mark.parametrize("threshold_seconds", [3, 7])
def test_rounded_gap_lower_bound_rejects_submicrosecond_shortfall(threshold_seconds: int):
    threshold_us = threshold_seconds * 1_000_000
    # A real interval that is 0.5 microseconds short can round to the exact
    # threshold when its two endpoints are persisted independently in us.
    rounded_earlier_us = round(0.0)
    rounded_later_us = round(threshold_us - 0.5)
    assert rounded_later_us == threshold_us
    lower_bound = resource_evidence._conservative_rounded_gap_lower_bound_us(
        rounded_earlier_us, rounded_later_us
    )
    assert lower_bound == threshold_us - 1
    assert lower_bound < threshold_us


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
    assert observed["answerer_timeout_seconds"] == 30.0
    assert observed["answerer_application_retries_observed"] == 0
    assert observed["answerer_retries"] is None  # Injected transport retry behavior is not attested.
    assert observed["answerer_concurrency_observed"] == 1

    from dataclasses import replace

    original_terminal = terminal_path.read_bytes()
    forged_terminal = json.loads(original_terminal)
    forged_terminal["execution_control_attestation"]["producer_module_sha256"] = "0" * 64
    forged_terminal_bytes = answer_execution._canonical(forged_terminal)
    terminal_path.write_bytes(forged_terminal_bytes)
    with pytest.raises(resource_evidence.ResourceEvidenceError, match="answer-control-attestation-binding"):
        resource_evidence._answer_observations(
            stage_uuid=args["stage_uuid"],
            source_revision=args["source_revision"],
            protocol_sha256=args["protocol_sha256"],
            cohorts_sha256=args["cohorts_sha256"],
            answer_result=replace(result, terminal_receipt_sha256=_sha(forged_terminal_bytes)),
            answer_tasks=args["tasks"],
            answer_archive_root=args["archive_root"],
            answer_result_root=args["result_root"],
            expected_terminal_sha256=_sha(forged_terminal_bytes),
            expected_manifest_sha256=args["answer_manifest_sha256"],
        )

    legacy_terminal = json.loads(original_terminal)
    legacy_terminal.pop("execution_control_attestation")
    legacy_terminal_bytes = answer_execution._canonical(legacy_terminal)
    terminal_path.write_bytes(legacy_terminal_bytes)
    legacy_observed = resource_evidence._answer_observations(
        stage_uuid=args["stage_uuid"],
        source_revision=args["source_revision"],
        protocol_sha256=args["protocol_sha256"],
        cohorts_sha256=args["cohorts_sha256"],
        answer_result=replace(result, terminal_receipt_sha256=_sha(legacy_terminal_bytes)),
        answer_tasks=args["tasks"],
        answer_archive_root=args["archive_root"],
        answer_result_root=args["result_root"],
        expected_terminal_sha256=_sha(legacy_terminal_bytes),
        expected_manifest_sha256=args["answer_manifest_sha256"],
    )
    assert legacy_observed["answerer_timeout_seconds"] is None
    assert legacy_observed["answerer_retries"] is None
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
async def test_full_collector_binds_mock_stages_and_preserves_unknowns(tmp_path: Path, monkeypatch):
    from scripts import coverage_jev_execution as selector_execution
    from scripts import coverage_study_core as study_core
    from tests import test_coverage_study_core as study_fixtures
    from tests.test_coverage_jev_execution import PrivateRoots, compile_small, invoke_w0_kwargs, valid_response
    from tests.test_coverage_jev_resource_observations import _call_kwargs as candidate_call_kwargs
    from tests.test_coverage_legacy_control import _response as legacy_response
    from tests.test_coverage_study_core import make_fixture, reseal_fixture

    protocol_path = Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study/protocol.json"
    protocol_bytes = protocol_path.read_bytes()
    protocol = json.loads(protocol_bytes)
    cohort_path = protocol_path.with_name("cohorts.json")
    cohorts_bytes = cohort_path.read_bytes()
    source_revision = "8f3577d022e2d98fcd405d915b5c3b9b16e899bf"
    stage_uuid = "f611a79a-9eef-46f4-b211-3d4d394e9e21"
    monkeypatch.setattr(study_fixtures, "SOURCE_REVISION", source_revision)
    selector_args, selector_materials, *_ = make_fixture(stage_uuid=stage_uuid)
    selector_materials["protocol"] = protocol_bytes
    selector_materials["coverage_source"] = (
        Path(__file__).parents[1] / "scripts/intent_ranking_coverage.py"
    ).read_bytes()
    selector_materials["production_rerank_source"] = (
        Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study/w0-rerank-v1.py.txt"
    ).read_bytes()
    source_pins = {
        name: _sha(selector_materials[name])
        for name in ("coverage_source", "production_rerank_source", "dependency_lock")
    }
    selector_materials["qualified_source_closure"] = study_core._canonical(
        {
            "schema": "coverage-study-qualified-source-closure/1",
            "source_revision": source_revision,
            "material_pins": source_pins,
        }
    )
    selector_args = reseal_fixture(selector_args, selector_materials, stage_uuid=stage_uuid)
    prepared_selector = study_core.preflight_stage(**selector_args)
    source_closure_sha256 = prepared_selector.pins["qualified_source_closure"]
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
        source_closure_sha256=source_closure_sha256,
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

    # Execute the registered selector schedule against response bytes carried
    # by an in-process mock transport; the full collector must derive usage and
    # elapsed values from these archived receipts, ignoring lookalike caller
    # fields passed below.
    selector_ledger = study_core.StudyRun(prepared_selector)
    selector_roots = PrivateRoots()
    candidate_compiled = compile_small()
    candidate_body = valid_response(candidate_compiled, input_tokens=17, output_tokens=6)
    w0_body = legacy_response({"c0": 7, "c1": 8, "c2": 6, "c3": 9})

    async def selector_handler(request):
        body = candidate_body if request.content == candidate_compiled.body else w0_body
        return httpx.Response(200, content=body, request=request)

    for operation_id in prepared_selector.operation_ids:
        transport = httpx.MockTransport(selector_handler)
        if "-w0" in operation_id:
            call = invoke_w0_kwargs(prepared_selector, selector_ledger, selector_roots, operation_id, transport)
        else:
            call = candidate_call_kwargs(
                prepared_selector,
                selector_ledger,
                selector_roots,
                operation_id,
                candidate_compiled,
                transport,
            )
        await selector_execution.execute_selector_call(**call)
    selector_terminal = selector_execution.close_stage(
        prepared=prepared_selector, ledger=selector_ledger, result_root=selector_roots.result
    )
    selector_terminal_bytes = (
        selector_roots.result / f"{prepared_selector.stage_uuid}.terminal-inventory.json"
    ).read_bytes()
    selector_operation_rows = tuple(
        {
            "operation_id": operation_id,
            "state": "complete-success",
            "caller_usage": {"input_tokens": 999_999, "output_tokens": 999_999},
        }
        for operation_id in prepared_selector.operation_ids
    )
    acquisition_bytes = resource_evidence.coverage_live_acquire._canonical(acquisition_manifest)
    capture_inventory_bytes = (capture.receipt_directory / "inventory.json").read_bytes()
    capture_endpoint_sha = capture_manifest["candidate_endpoint_sha256"]
    collector_kwargs = dict(
        stage_uuid=stage_uuid,
        source_revision=source_revision,
        protocol_bytes=protocol_bytes,
        cohorts_bytes=cohorts_bytes,
        source_closure_sha256=source_closure_sha256,
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
        expected_selector_terminal_sha256=selector_terminal["sha256"],
        selector_terminal_inventory_bytes=selector_terminal_bytes,
        selector_result_root=selector_roots.result,
        selector_archive_root=selector_roots.archive,
        selector_expected_operation_ids=prepared_selector.operation_ids,
        selector_operation_rows=selector_operation_rows,
        selector_registration_sha256=prepared_selector.registration_sha256,
        selector_usage=[{"input_tokens": 999_999, "output_tokens": 999_999}],
        selector_elapsed_ms=999_999,
        selector_concurrency=999,
        selector_retries=999,
        w0_control_source_sha256="caller-fake-source",
        w0_provider_configured=True,
        w0_parser_parity_verified=True,
        candidate_source_sha256="caller-fake-candidate",
        atomic_fallback_verified=True,
    )
    report = resource_evidence.collect_resource_evidence(**collector_kwargs)
    assert report.stage_uuid == stage_uuid
    assert report.observations["acquisition_engine_calls"]["brave"] == 0
    assert report.observations["answerer_calls"] == 18
    assert report.observations["grader_submissions"] > 0
    assert report.observations["capture_internal_fanout"] is None
    assert report.observations["capture_timeout_seconds"] == 30.0
    assert report.observations["capture_response_bytes_limit"] == 2_000_000
    assert report.observations["capture_response_bytes_limit_applied"] == 2_000_000
    assert report.observations["capture_retries"] is None
    assert report.observations["answerer_timeout_seconds"] == 30.0
    assert report.observations["answerer_retries"] is None
    assert report.observations["answerer_concurrency_observed"] == 1
    selector_terminal_doc = json.loads(selector_terminal_bytes)
    selector_expected_elapsed = max(
        row["elapsed_ms_through_final_receipt_fsync"]
        for row in selector_terminal_doc["resource_observations"]
        if row.get("status") == "observed"
    )
    assert report.observations["terminal_inventory_complete"] is True
    assert report.observations["selector_elapsed_ms"] == selector_expected_elapsed
    assert report.observations["selector_usage"] == [
        {
            "operation_id": operation_id,
            "status": "reported",
            "input_tokens": 123 if "-w0" in operation_id else 17,
            "output_tokens": 12 if "-w0" in operation_id else 6,
        }
        for operation_id in prepared_selector.operation_ids
    ]
    assert report.observations["selector_concurrency"] == 1
    assert report.observations["selector_retries"] == 0
    assert report.observations["selector_provider_dispatches"] == len(prepared_selector.operation_ids)
    assert report.observations["stage_elapsed_seconds"] is None
    # Complete archived calls are measurable, but source/registry identity
    # remains unknown until a separately sealed v2 pre-dispatch input map is
    # supplied and recomputed by the coordinator.
    assert report.observations["w0_control_source_sha256"] is None
    assert report.observations["w0_source_revision"] is None
    assert report.observations["w0_model"] is None
    assert report.observations["w0_provider_configured"] is None
    assert report.observations["w0_ranking_strategy"] is None
    assert report.observations["candidate_source_sha256"] is None
    assert report.observations["w0_parser_parity_verified"] is None
    assert report.observations["atomic_fallback_verified"] is None
    assert report.observations["selector_transaction_proof"] is None
    assert report.configuration_provenance["selector_transaction_proof_sha256"] is None
    assert report.configuration_provenance["quality_credit"] is False

    # Exercise the collector's map join with a synthetic, externally pinned
    # map assembled from the just-created immutable operation receipts. The
    # independent builder/recomputation path is covered separately.
    map_rows = []
    for operation_id in prepared_selector.operation_ids:
        receipt = json.loads((selector_roots.result / f"{stage_uuid}.{operation_id}.result.json").read_bytes())
        provenance = receipt["execution_provenance"]
        map_rows.append(
            {
                "operation_id": operation_id,
                "parser_mode": provenance["parser_mode"],
                "operation_input_sha256": provenance["operation_input_sha256"],
                "request_body_sha256": provenance["request_body_sha256"],
            }
        )
    selector_map = selector_execution._canonical(
        {
            "schema": "coverage-selector-input-map/2-draft",
            "status": "draft-unadmitted",
            "stage_uuid": stage_uuid,
            "source_revision": source_revision,
            "original_registration_sha256": prepared_selector.registration_sha256,
            "coverage_source_sha256": protocol["candidate_source_sha256"],
            "production_rerank_source_sha256": protocol["primary_control"]["rerank_source_sha256"],
            "execution_source_sha256": "a" * 64,
            "legacy_control_source_sha256": "b" * 64,
            "current_service_source_sha256": "c" * 64,
            "acquisition_snapshot_index_sha256": "a" * 64,
            "acquisition_manifest_sha256": "b" * 64,
            "task_input_manifest_sha256": "c" * 64,
            "neutral_fixture_sha256": "d" * 64,
            "builder_source_sha256": "e" * 64,
            "operations": map_rows,
        }
    )
    selector_map_sha = _sha(selector_map)
    terminal_doc = json.loads(selector_terminal_bytes)
    for observation in terminal_doc["resource_observations"]:
        operation_id = observation["operation_id"]
        result_path = selector_roots.result / f"{stage_uuid}.{operation_id}.result.json"
        result_doc = json.loads(result_path.read_bytes())
        result_doc["execution_provenance"]["selector_input_map_sha256"] = selector_map_sha
        result_bytes = selector_execution._canonical(result_doc)
        result_path.write_bytes(result_bytes)
        observation["result_receipt_sha256"] = _sha(result_bytes)
    selector_terminal_bytes = selector_execution._canonical(terminal_doc)
    (selector_roots.result / f"{stage_uuid}.terminal-inventory.json").write_bytes(selector_terminal_bytes)
    mapped = resource_evidence._selector_observations(
        stage_uuid=stage_uuid,
        source_revision=source_revision,
        protocol_sha256=prepared_selector.pins["protocol"],
        source_closure_sha256=source_closure_sha256,
        expected_terminal_sha256=_sha(selector_terminal_bytes),
        terminal_inventory_bytes=selector_terminal_bytes,
        result_root=selector_roots.result,
        archive_root=selector_roots.archive,
        expected_operation_ids=prepared_selector.operation_ids,
        operation_rows=selector_operation_rows,
        expected_registration_sha256=prepared_selector.registration_sha256,
        expected_primary_control=protocol["primary_control"],
        expected_candidate_source_sha256=protocol["candidate_source_sha256"],
        selector_input_map_bytes=selector_map,
        expected_selector_input_map_sha256=selector_map_sha,
    )
    mapped_proof = mapped["selector_transaction_proof"]
    assert mapped_proof["input_map_sha256"] == selector_map_sha
    assert len(mapped_proof["transactions"]) == len(prepared_selector.operation_ids)
    assert mapped["w0_control_source_sha256"] == protocol["primary_control"]["rerank_source_sha256"]
    # Keep the later full-collector pass bound to the same newly sealed
    # terminal bytes used by the direct map-join assertion above.
    collector_kwargs["selector_terminal_inventory_bytes"] = selector_terminal_bytes
    collector_kwargs["expected_selector_terminal_sha256"] = _sha(selector_terminal_bytes)
    collector_kwargs["selector_input_map_bytes"] = selector_map
    collector_kwargs["expected_selector_input_map_sha256"] = selector_map_sha
    # A full collection over historical inventory bytes without the new
    # attestation must keep the measured control unknown.
    capture_inventory_doc = json.loads(capture_inventory_bytes)
    capture_inventory_doc.pop("execution_control_attestation")
    legacy_capture_inventory_bytes = resource_evidence.source_capture._canonical_json(capture_inventory_doc)
    (capture.receipt_directory / "inventory.json").write_bytes(legacy_capture_inventory_bytes)
    from dataclasses import replace

    collector_kwargs["capture_result"] = replace(
        capture,
        private_inventory=tuple(capture_inventory_doc["sources"]),
    )
    collector_kwargs["expected_capture_inventory_sha256"] = _sha(legacy_capture_inventory_bytes)
    legacy_report = resource_evidence.collect_resource_evidence(**collector_kwargs)
    assert legacy_report.observations["capture_response_bytes_limit"] is None
    assert legacy_report.observations["capture_response_bytes_limit_applied"] is None
    selector_roots.close()


@pytest.mark.asyncio
async def test_selector_resources_replay_terminal_result_and_archive_chain(monkeypatch):
    from scripts import coverage_jev_execution as execution
    from scripts import coverage_resource_evidence as resource_evidence
    from scripts import coverage_study_core as core
    from tests.test_coverage_jev_execution import (
        PrivateRoots,
        compile_small,
        invoke_w0_kwargs,
        prepared_stage,
        valid_response,
    )
    from tests.test_coverage_jev_resource_observations import _call_kwargs
    from tests.test_coverage_legacy_control import _response as legacy_response

    prepared = prepared_stage()
    ledger = core.StudyRun(prepared)
    roots = PrivateRoots()
    compiled = compile_small()
    response = valid_response(compiled, input_tokens=17, output_tokens=6)
    v1_response = legacy_response({"c0": 7, "c1": 8, "c2": 6, "c3": 9})

    async def handler(request):
        if request.content == compiled.body:
            return httpx.Response(200, content=response, request=request)
        return httpx.Response(200, content=v1_response, request=request)

    def call_kwargs(operation_id):
        if "-w0" in operation_id:
            return invoke_w0_kwargs(
                prepared,
                ledger,
                roots,
                operation_id,
                httpx.MockTransport(handler),
            )
        return _call_kwargs(
            prepared,
            ledger,
            roots,
            operation_id,
            compiled,
            httpx.MockTransport(handler),
        )

    for operation_id in prepared.operation_ids:
        result = await execution.execute_selector_call(**call_kwargs(operation_id))
        assert result.dispatch_count == 1
        if "-w0" in operation_id:
            assert result.status == "complete-original-v1"
            assert result.input_tokens_observed == 123
            assert result.output_tokens_observed == 12
        else:
            assert result.status in {"selected", "fallback"}
            assert result.input_tokens_observed == 17
            assert result.output_tokens_observed == 6

    terminal = execution.close_stage(prepared=prepared, ledger=ledger, result_root=roots.result)
    terminal_bytes = (roots.result / f"{prepared.stage_uuid}.terminal-inventory.json").read_bytes()
    operation_rows = tuple(
        {"operation_id": operation_id, "state": "complete-success"} for operation_id in prepared.operation_ids
    )
    kwargs = {
        "stage_uuid": prepared.stage_uuid,
        "source_revision": prepared.source_revision,
        "protocol_sha256": prepared.pins["protocol"],
        "source_closure_sha256": prepared.pins["qualified_source_closure"],
        "expected_terminal_sha256": terminal["sha256"],
        "terminal_inventory_bytes": terminal_bytes,
        "result_root": roots.result,
        "archive_root": roots.archive,
        "expected_operation_ids": prepared.operation_ids,
        "operation_rows": operation_rows,
        "expected_registration_sha256": prepared.registration_sha256,
        "expected_primary_control": {
            "rerank_source_sha256": prepared.pins["production_rerank_source"],
            "model": "jev-1.13.0",
            "ranking_strategy": "presence",
        },
        "expected_candidate_source_sha256": prepared.pins["coverage_source"],
    }
    observed = resource_evidence._selector_observations(**kwargs)
    terminal_doc = json.loads(terminal_bytes)
    elapsed = [row["elapsed_ms_through_final_receipt_fsync"] for row in terminal_doc["resource_observations"]]
    assert observed["terminal_inventory_complete"] is True
    assert observed["selector_elapsed_ms"] == max(elapsed)
    assert observed["selector_usage"] == [
        {
            "operation_id": operation_id,
            "status": "reported",
            "input_tokens": 123 if "-w0" in operation_id else 17,
            "output_tokens": 12 if "-w0" in operation_id else 6,
        }
        for operation_id in prepared.operation_ids
    ]
    assert observed["selector_concurrency"] == 1
    assert observed["selector_retries"] == 0
    assert observed["selector_provider_dispatches"] == len(prepared.operation_ids)
    assert observed["stage_elapsed_seconds"] is None

    # A changed but self-canonical terminal body cannot replace the external pin.
    altered_doc = json.loads(terminal_bytes)
    altered_doc["resource_observations"][0]["elapsed_ms_through_final_receipt_fsync"] += 1
    altered_terminal = execution._canonical(altered_doc)
    with pytest.raises(resource_evidence.ResourceEvidenceError, match="terminal-inventory-pin-mismatch"):
        resource_evidence._selector_observations(**{**kwargs, "terminal_inventory_bytes": altered_terminal})
    terminal_path = roots.result / f"{prepared.stage_uuid}.terminal-inventory.json"
    saved_terminal = terminal_path.read_bytes()
    terminal_path.write_bytes(altered_terminal)
    with pytest.raises(resource_evidence.ResourceEvidenceError, match="terminal-inventory-file-mismatch"):
        resource_evidence._selector_observations(**kwargs)
    terminal_path.write_bytes(saved_terminal)

    # Result receipts and archived bodies are independently checked after the terminal pin.
    first_id = prepared.operation_ids[0]
    result_path = roots.result / f"{prepared.stage_uuid}.{first_id}.result.json"
    saved_result = result_path.read_bytes()
    result_path.write_bytes(saved_result + b" ")
    with pytest.raises(resource_evidence.ResourceEvidenceError, match="result-receipt-pin-mismatch"):
        resource_evidence._selector_observations(**kwargs)
    result_path.write_bytes(saved_result)

    archive_body = roots.archive / prepared.stage_uuid / first_id / "response.bin"
    saved_body = archive_body.read_bytes()
    archive_body.write_bytes(saved_body + b"x")
    with pytest.raises(resource_evidence.ResourceEvidenceError, match="receipt-body-mismatch"):
        resource_evidence._selector_observations(**kwargs)
    archive_body.write_bytes(saved_body)

    # Even a replacement terminal/result chain that is externally re-pinned
    # cannot claim usage absent from the unchanged archived provider body.
    saved_terminal = terminal_path.read_bytes()
    result_path = roots.result / f"{prepared.stage_uuid}.{first_id}.result.json"
    saved_result = result_path.read_bytes()
    forged_result = json.loads(saved_result)
    forged_result["input_tokens_observed"] += 1
    forged_result_bytes = execution._canonical(forged_result)
    result_path.write_bytes(forged_result_bytes)
    forged_terminal_doc = json.loads(saved_terminal)
    forged_row = forged_terminal_doc["resource_observations"][0]
    forged_row["input_tokens_observed"] += 1
    forged_row["result_receipt_sha256"] = _sha(forged_result_bytes)
    forged_terminal_bytes = execution._canonical(forged_terminal_doc)
    terminal_path.write_bytes(forged_terminal_bytes)
    with pytest.raises(resource_evidence.ResourceEvidenceError, match="selector-usage-archive-mismatch"):
        resource_evidence._selector_observations(
            **{
                **kwargs,
                "expected_terminal_sha256": _sha(forged_terminal_bytes),
                "terminal_inventory_bytes": forged_terminal_bytes,
            }
        )
    result_path.write_bytes(saved_result)
    terminal_path.write_bytes(saved_terminal)

    # A self-consistent digest chain still cannot claim candidate parsing for
    # the production W0 operation, or a candidate status for original-v1.
    forged_result = json.loads(saved_result)
    forged_result["status"] = "fallback"
    forged_result["parser_mode"] = "coverage"
    forged_result["ranking_status"] = "fallback"
    forged_result_bytes = execution._canonical(forged_result)
    result_path.write_bytes(forged_result_bytes)
    forged_terminal_doc = json.loads(saved_terminal)
    forged_terminal_doc["resource_observations"][0]["result_receipt_sha256"] = _sha(forged_result_bytes)
    forged_terminal_bytes = execution._canonical(forged_terminal_doc)
    terminal_path.write_bytes(forged_terminal_bytes)
    with pytest.raises(resource_evidence.ResourceEvidenceError, match="selector-result-observation-binding"):
        resource_evidence._selector_observations(
            **{
                **kwargs,
                "expected_terminal_sha256": _sha(forged_terminal_bytes),
                "terminal_inventory_bytes": forged_terminal_bytes,
            }
        )
    result_path.write_bytes(saved_result)
    terminal_path.write_bytes(saved_terminal)
    forged_result = json.loads(saved_result)
    forged_result["execution_provenance"]["source_sha256"] = "0" * 64
    forged_result_bytes = execution._canonical(forged_result)
    result_path.write_bytes(forged_result_bytes)
    forged_terminal_doc = json.loads(saved_terminal)
    forged_terminal_doc["resource_observations"][0]["result_receipt_sha256"] = _sha(forged_result_bytes)
    forged_terminal_bytes = execution._canonical(forged_terminal_doc)
    terminal_path.write_bytes(forged_terminal_bytes)
    with pytest.raises(resource_evidence.ResourceEvidenceError, match="selector-execution-provenance-binding"):
        resource_evidence._selector_observations(
            **{
                **kwargs,
                "expected_terminal_sha256": _sha(forged_terminal_bytes),
                "terminal_inventory_bytes": forged_terminal_bytes,
            }
        )
    result_path.write_bytes(saved_result)
    terminal_path.write_bytes(saved_terminal)

    # A forged W0 model remains invalid even if the result and terminal files
    # are re-canonicalized and their local digest chain is recomputed.
    forged_result = json.loads(saved_result)
    forged_result["execution_provenance"]["requested_model"] = "caller-selected-model"
    forged_result_bytes = execution._canonical(forged_result)
    result_path.write_bytes(forged_result_bytes)
    forged_terminal_doc = json.loads(saved_terminal)
    forged_terminal_doc["resource_observations"][0]["result_receipt_sha256"] = _sha(forged_result_bytes)
    forged_terminal_bytes = execution._canonical(forged_terminal_doc)
    terminal_path.write_bytes(forged_terminal_bytes)
    with pytest.raises(resource_evidence.ResourceEvidenceError, match="selector-execution-provenance-binding"):
        resource_evidence._selector_observations(
            **{
                **kwargs,
                "expected_terminal_sha256": _sha(forged_terminal_bytes),
                "terminal_inventory_bytes": forged_terminal_bytes,
            }
        )
    result_path.write_bytes(saved_result)
    terminal_path.write_bytes(saved_terminal)
    roots.close()


def test_missing_selector_terminal_proof_remains_unknown():
    from scripts import coverage_resource_evidence as resource_evidence

    observed = resource_evidence._selector_observations(
        stage_uuid="f611a79a-9eef-46f4-b211-3d4d394e9e21",
        source_revision="a" * 40,
        protocol_sha256="b" * 64,
        source_closure_sha256="c" * 64,
        expected_terminal_sha256="d" * 64,
        terminal_inventory_bytes=None,
        result_root=None,
        archive_root=None,
        expected_operation_ids=("research-01-base-candidate",),
        operation_rows=({"operation_id": "research-01-base-candidate", "state": "complete-success"},),
    )
    assert observed == {
        "terminal_inventory_complete": None,
        "selector_elapsed_ms": None,
        "stage_elapsed_seconds": None,
        "selector_usage": None,
        "selector_concurrency": None,
        "selector_retries": None,
        "selector_provider_dispatches": None,
        "selector_transaction_proof": None,
        "w0_control_source_sha256": None,
        "w0_source_revision": None,
        "w0_model": None,
        "w0_provider_configured": None,
        "w0_ranking_strategy": None,
        "w0_parser_parity_verified": None,
        "candidate_source_sha256": None,
        "atomic_fallback_verified": None,
    }
