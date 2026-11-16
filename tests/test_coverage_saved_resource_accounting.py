from __future__ import annotations

import hashlib
import json
from dataclasses import replace
from pathlib import Path
from urllib.parse import parse_qs

import httpx
import pytest

from scripts import (
    coverage_execution_controls,
    coverage_resource_evidence,
    coverage_saved_corpus,
    coverage_source_capture,
    coverage_study_acquire,
)
from scripts import (
    coverage_frozen_pool_inputs as frozen_pool,
)
from tests import test_coverage_frozen_pool_collect as pool_fixtures
from tests import test_coverage_source_capture as capture_fixtures
from tests.test_coverage_source_capture import (
    _candidate_identity,
    _healthy_response,
    _Lease,
    _response,
    _source,
    _Verifier,
)


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def test_source_capture_accepts_protocol_v3_without_changing_capture_bounds():
    protocol_path = (
        Path(__file__).parents[1]
        / "docs/experiments/evidence/coverage-first-study/saved-development-method-materials/protocol.json"
    )
    protocol_bytes = protocol_path.read_bytes()
    protocol, digest = coverage_source_capture._validate_protocol(protocol_bytes)

    assert digest == _sha(protocol_bytes)
    assert protocol["schema"] == "coverage-first-study-protocol/3"
    assert protocol["capture"]["max_owned_http_calls"] == coverage_source_capture.MAX_OWNED_CALLS
    assert protocol["capture"]["max_owned_scrape_calls"] == coverage_source_capture.MAX_SCRAPE_CALLS
    assert protocol["capture"]["response_bytes"] == coverage_source_capture.MAX_RESPONSE_BYTES
    assert protocol["capture"]["retries"] == 0
    assert protocol["phase_limits"]["stage_wall_seconds"] == coverage_source_capture.MAX_STAGE_WALL_SECONDS


@pytest.mark.asyncio
async def test_saved_origin_resource_observer_replays_pinned_collection_and_capture(tmp_path):
    evidence_root = Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study"
    origin_protocol_bytes = (evidence_root / "frozen-input-method-materials/protocol.json").read_bytes()
    cohorts_bytes = (evidence_root / "frozen-input-method-materials/cohorts.json").read_bytes()
    cohorts = json.loads(cohorts_bytes)
    development = next(row for row in cohorts["stages"] if row["stage"] == "development")
    cases = development["research_cases"]
    navigation = development["navigation_targets"]
    identity = capture_fixtures._candidate_identity()
    identity_sha = _sha(identity)
    candidate_endpoint = "https://capture.example"
    endpoint_sha = _sha(candidate_endpoint.encode())

    manifest, plan_bytes = pool_fixtures._manifest()
    manifest.update(
        {
            "source_revision": "d1d4cd35cb1e5653404d605345377e4447938704",
            "source_closure_sha256": "5" * 64,
            "protocol_sha256": _sha(origin_protocol_bytes),
            "cohorts_sha256": _sha(cohorts_bytes),
            "research_cases": cases,
            "navigation_targets": navigation,
        }
    )
    plan_doc = json.loads(plan_bytes)
    engine_caps = pool_fixtures.offline.MAX_PHYSICAL_REQUESTS_PER_ENGINE
    plan_doc["tasks"] = [
        {
            "task_id": row["task_id"],
            "engine_calls": {
                engine: engine_caps[engine] if engine in row["pool_plan"]["engines"] else 0 for engine in engine_caps
            },
        }
        for row in cases
    ] + [
        {
            "task_id": row["target_id"],
            "engine_calls": {
                engine: engine_caps[engine] if engine in row["navigation_pool_plan"]["engines"] else 0
                for engine in engine_caps
            },
        }
        for row in navigation
    ]
    plan_bytes = pool_fixtures._canonical(plan_doc)
    manifest["acquisition_plan_sha256"] = _sha(plan_bytes)
    stage_bytes = pool_fixtures._canonical(manifest)

    pacer = pool_fixtures._Pacer()
    calls = []
    pool_handler = pool_fixtures._handler(calls)
    navigation_by_query = {row["search_query"]: row["target_url"] for row in navigation}

    def handle_pool_request(request: httpx.Request):
        query = parse_qs(request.url.query.decode())
        search = query.get("q", [""])[0]
        if request.url.host == "api.github.com" and search in navigation_by_query:
            calls.append(request)
            target = navigation_by_query[search]
            return httpx.Response(
                200,
                json={
                    "total_count": 1,
                    "items": [
                        {
                            "full_name": target.removeprefix("https://github.com/"),
                            "html_url": target,
                            "description": "official repository",
                            "stargazers_count": 1,
                            "language": "Python",
                        }
                    ],
                },
                request=request,
            )
        return pool_handler(request)

    transport = httpx.MockTransport(handle_pool_request)
    inventory_root = tmp_path / "pool-inventory"
    inventory_root.mkdir(mode=0o700)
    stage_root = inventory_root / manifest["stage_uuid"]
    stage_root.mkdir(mode=0o700)
    collected = await pool_fixtures.collector.collect_frozen_pool_stage(
        stage_manifest_bytes=stage_bytes,
        acquisition_plan_bytes=plan_bytes,
        stage_clock=pool_fixtures._clock(pacer),
        candidate_identity_sha256=identity_sha,
        candidate_endpoint_sha256=endpoint_sha,
        permit_receipt_bytes=b"mock only",
        expected_permit_receipt_sha256=_sha(b"mock only"),
        permit_verifier=pool_fixtures._PermitVerifier(),
        one_shot_lease=pool_fixtures._Lease(),
        stage_inventory_directory=stage_root,
        test_transport=transport,
        pacer=pacer,
    )
    collection_root = collected.collection_directory
    operator_handoff_root = tmp_path / "operator-handoff"
    operator_handoff_root.mkdir(mode=0o700)
    acquisition_plan_sidecar = operator_handoff_root / "acquisition-plan.bin"
    acquisition_plan_sidecar.write_bytes(plan_bytes)
    acquisition_plan_sidecar.chmod(0o600)
    historical_source_dir = tmp_path / "historical-source"
    historical_source_dir.mkdir(mode=0o700)
    historical_source_bytes = Path(coverage_study_acquire.__file__).read_bytes()
    historical_source_sidecar = historical_source_dir / "coverage_study_acquire.py"
    historical_source_sidecar.write_bytes(historical_source_bytes)
    historical_source_sidecar.chmod(0o600)
    receipt_inventory_sha = coverage_resource_evidence.receipt_inventory_sha256(collection_root)
    registration = frozen_pool.FrozenPoolRegistrationContext(
        stage_uuid=manifest["stage_uuid"],
        stage_kind="development",
        source_revision=manifest["source_revision"],
        source_closure_sha256=manifest["source_closure_sha256"],
        protocol_sha256=manifest["protocol_sha256"],
        cohorts_sha256=manifest["cohorts_sha256"],
        candidate_identity_sha256=identity_sha,
        candidate_endpoint_sha256=endpoint_sha,
        acquisition_plan_sha256=manifest["acquisition_plan_sha256"],
        task_ids=tuple(row["task_id"] for row in cases),
        navigation_ids=tuple(row["target_id"] for row in navigation),
        evidence_sha256=collected.evidence_sha256,
        collector_receipt_sha256=collected.collector_receipt_sha256,
        availability_sha256=collected.availability_sha256,
    )
    pool_evidence = frozen_pool.FrozenPoolEvidence(
        snapshots_directory=collection_root,
        evidence_bytes=(collection_root / "frozen-pool-evidence.json").read_bytes(),
        expected_evidence_sha256=collected.evidence_sha256,
        collector_receipt_bytes=(collection_root / "collector-receipt.json").read_bytes(),
        expected_collector_receipt_sha256=collected.collector_receipt_sha256,
        availability_bytes=(collection_root / "engine-availability.json").read_bytes(),
        expected_availability_sha256=collected.availability_sha256,
    )
    phase = frozen_pool.run_frozen_pool_input_phase(
        cases,
        evidence=pool_evidence,
        registration=registration,
        stage_started_monotonic=0.0,
        stage_deadline_monotonic=28_800.0,
        now_monotonic=pacer.elapsed,
    )

    capture_manifest = {
        **phase.pipeline_inputs["capture_manifest"],
        "stage": "development",
        "stage_uuid": registration.stage_uuid,
        "source_revision": registration.source_revision,
        "protocol_sha256": registration.protocol_sha256,
        "cohorts_sha256": registration.cohorts_sha256,
        "candidate_identity_sha256": identity_sha,
        "candidate_endpoint_sha256": endpoint_sha,
    }
    capture_manifest_bytes = coverage_source_capture._canonical_json(capture_manifest)
    capture_receipt_root = tmp_path / "capture-receipts"
    capture_receipt_root.mkdir(mode=0o700)
    capture_transport = httpx.MockTransport(
        lambda request: (
            capture_fixtures._healthy_response()
            if request.method == "GET"
            else httpx.Response(200, content=capture_fixtures._response("Saved source evidence."), request=request)
        )
    )
    captured = await coverage_source_capture.capture_sources_once(
        protocol_bytes=origin_protocol_bytes,
        source_manifest_bytes=capture_manifest_bytes,
        candidate_identity_bytes=identity,
        candidate_base_url=candidate_endpoint,
        operator_token=None,
        permit_receipt_bytes=b"external-permit-receipt",
        expected_permit_receipt_sha256=_sha(b"external-permit-receipt"),
        permit_verifier=capture_fixtures._Verifier(),
        one_shot_lease=capture_fixtures._Lease(),
        receipt_root=capture_receipt_root,
        transport=capture_transport,
        stage_started_monotonic=0.0,
        stage_deadline_monotonic=28_800.0,
        clock=lambda: 100.0,
    )

    package_sha = coverage_saved_corpus.package_sha256(collection_root, captured.receipt_directory)
    origin = coverage_saved_corpus.SavedCorpusOrigin(
        registration=registration,
        research_cases=tuple(cases),
        navigation_targets=tuple(navigation),
        collection_directory=collection_root,
        expected_collection_receipt_inventory_sha256=receipt_inventory_sha,
        capture_stage_directory=captured.receipt_directory,
        expected_capture_inventory_sha256=_sha((captured.receipt_directory / "inventory.json").read_bytes()),
        expected_capture_manifest_sha256=_sha(capture_manifest_bytes),
        expected_package_sha256=package_sha,
        acquisition_plan_sidecar=acquisition_plan_sidecar,
        operation_runner_source_sidecar=historical_source_sidecar,
        operation_runner_source_sha256=_sha(historical_source_bytes),
    )
    current_protocol = json.loads((evidence_root / "saved-development-method-materials/protocol.json").read_bytes())
    origin_acquisition = json.loads(origin_protocol_bytes)["acquisition"]
    origin_profile = coverage_resource_evidence.offline_acquisition.execution_profile_from_protocol(
        origin_acquisition, pool_input_mode="frozen-pool/1"
    )
    current_stage_uuid = "00000000-0000-0000-0000-000000000991"
    packet_stage_uuid = "00000000-0000-0000-0000-000000000992"
    current_source_revision = "e" * 40
    current_source_closure = "f" * 64
    current_protocol["saved_development_origin"].update(
        {
            "stage_uuid": registration.stage_uuid,
            "source_revision": registration.source_revision,
            "package_sha256": package_sha,
            "capture_producer_source_sha256": coverage_execution_controls.module_source_sha256(
                coverage_source_capture.__file__
            ),
        }
    )
    current_protocol_bytes = coverage_source_capture._canonical_json(current_protocol)
    historical_scrape_calls = sum(row.get("attempted") is True for row in captured.private_inventory)
    derived_capture_manifest = {
        **capture_manifest,
        "stage_uuid": current_stage_uuid,
        "source_revision": current_source_revision,
        "protocol_sha256": _sha(current_protocol_bytes),
    }
    derived_capture_manifest_bytes = coverage_source_capture._canonical_json(derived_capture_manifest)
    import_doc = {
        "schema": coverage_saved_corpus.SCHEMA,
        "status": "verified-for-fresh-grading",
        "stage_uuid": current_stage_uuid,
        "stage_kind": "development",
        "source_revision": current_source_revision,
        "source_closure_sha256": current_source_closure,
        "protocol_sha256": _sha(current_protocol_bytes),
        "cohorts_sha256": _sha(cohorts_bytes),
        "candidate_identity_sha256": identity_sha,
        "candidate_endpoint_sha256": endpoint_sha,
        "packet_stage_uuid": packet_stage_uuid,
        "origin_stage_uuid": registration.stage_uuid,
        "origin_source_revision": registration.source_revision,
        "origin_package_sha256": package_sha,
        "origin_pool_manifest_sha256": phase.verified.pool_manifest_sha256,
        "origin_pool_snapshot_index_sha256": phase.verified.snapshot_index_sha256,
        "origin_collection_receipt_sha256": registration.collector_receipt_sha256,
        "origin_capture_manifest_sha256": _sha(capture_manifest_bytes),
        "origin_capture_inventory_sha256": origin.expected_capture_inventory_sha256,
        "derived_capture_manifest_sha256": _sha(derived_capture_manifest_bytes),
        "import_authority_receipt_sha256": "a" * 64,
        "research_task_ids": list(registration.task_ids),
        "navigation_target_ids": list(registration.navigation_ids),
        "source_rows": len(capture_manifest["sources"]),
        "captured_contexts": captured.captured_count,
        "source_failures": captured.failed_count,
        "historical_search_calls": len(calls),
        "historical_capture_calls": captured.owned_http_calls,
        "historical_scrape_calls": historical_scrape_calls,
        "historical_health_calls": 1,
        "current_search_calls": 0,
        "current_capture_calls": 0,
        "quality_credit": False,
    }
    import_bytes = coverage_saved_corpus._canonical(import_doc)
    imported = coverage_saved_corpus.SavedCorpusImportResult(
        pipeline_inputs={},
        capture_inventory_bytes=b"{}",
        context_artifacts={},
        origin_pool_verified=phase.verified,
        origin_capture_result=captured,
        origin_package_sha256=package_sha,
        import_receipt_bytes=import_bytes,
        import_receipt_sha256=_sha(import_bytes),
        capture_manifest_bytes=derived_capture_manifest_bytes,
        capture_manifest_sha256=_sha(derived_capture_manifest_bytes),
        historical_search_calls=len(calls),
        historical_capture_calls=captured.owned_http_calls,
        historical_scrape_calls=historical_scrape_calls,
        historical_health_calls=1,
        historical_captured_sources=captured.captured_count,
        historical_failed_sources=captured.failed_count,
        acquisition_plan_bytes=plan_bytes,
        historical_operation_runner_module=coverage_saved_corpus.verify_historical_operation_runner_module(origin),
    )
    resource_kwargs = {
        "stage_uuid": current_stage_uuid,
        "stage_kind": "development",
        "source_revision": current_source_revision,
        "source_closure_sha256": current_source_closure,
        "protocol_bytes": current_protocol_bytes,
        "cohorts_bytes": cohorts_bytes,
        "candidate_identity_sha256": identity_sha,
        "candidate_endpoint_sha256": endpoint_sha,
        "packet_stage_uuid": packet_stage_uuid,
        "saved_origin": origin,
        "saved_corpus_import": imported,
        "candidate_identity_bytes": identity,
        "stage_started_monotonic": 100.0,
    }
    with pytest.raises(
        coverage_resource_evidence.ResourceEvidenceError,
        match="historical-acquisition-source-proof-invalid",
    ):
        coverage_resource_evidence.collect_acquisition_observations(
            snapshots_directory=collection_root,
            receipt_directory=collection_root,
            expected_receipt_inventory_sha256=receipt_inventory_sha,
            acquisition_manifest_bytes=stage_bytes,
            acquisition_plan_bytes=plan_bytes,
            expected_index_sha256=collected.pool_index_sha256,
            expected_manifest_sha256=collected.pool_manifest_sha256,
            stage_uuid=manifest["stage_uuid"],
            source_revision=manifest["source_revision"],
            protocol_sha256=manifest["protocol_sha256"],
            cohorts_sha256=manifest["cohorts_sha256"],
            source_closure_sha256=manifest["source_closure_sha256"],
            acquisition_plan_sha256=manifest["acquisition_plan_sha256"],
            configured_timeout_seconds=origin_profile.request_timeout_seconds,
            expected_execution_profile=origin_profile.identifier,
            configured_query_pacing_seconds=origin_acquisition["pacing_seconds"],
            configured_arxiv_pacing_seconds=origin_acquisition["arxiv_physical_pacing_seconds"],
            verified_historical_operation_runner_module=imported.historical_operation_runner_module,
            verified_saved_origin=origin,
        )
    acquisition, capture, provenance = coverage_resource_evidence._saved_origin_resource_observations(
        resource_kwargs, current_protocol
    )
    assert acquisition["acquisition_physical_http_calls"] == len(calls)
    assert capture["capture_owned_http_calls"] == captured.owned_http_calls
    assert capture["capture_health_calls"] == 1
    assert provenance["historical_input_costs"] is True
    assert provenance["origin_stage_uuid"] == registration.stage_uuid

    wrong_source_identity = coverage_saved_corpus.VerifiedHistoricalOperationRunnerModule(
        origin_stage_uuid=registration.stage_uuid,
        origin_source_revision=registration.source_revision,
        origin_package_sha256="0" * 64,
        module_sha256=imported.historical_operation_runner_module.module_sha256,
        module_bytes=imported.historical_operation_runner_module.module_bytes,
    )
    with pytest.raises(
        coverage_resource_evidence.ResourceEvidenceError,
        match="saved-origin-operation-runner-source-proof-mismatch",
    ):
        coverage_resource_evidence._saved_origin_resource_observations(
            {
                **resource_kwargs,
                "saved_corpus_import": replace(imported, historical_operation_runner_module=wrong_source_identity),
            },
            current_protocol,
        )

    summary_path = collection_root / "collection-summary.json"
    summary_path.write_bytes(summary_path.read_bytes() + b" ")
    with pytest.raises(coverage_resource_evidence.ResourceEvidenceError, match="saved-origin-package-pin-mismatch"):
        coverage_resource_evidence._saved_origin_resource_observations(resource_kwargs, current_protocol)


@pytest.mark.asyncio
async def test_saved_capture_accounting_accepts_only_explicit_historical_producer_pin(tmp_path):
    material_root = Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study"
    protocol_bytes = (material_root / "saved-development-method-materials/protocol.json").read_bytes()
    protocol = json.loads(protocol_bytes)
    producer_sha = coverage_execution_controls.module_source_sha256(coverage_source_capture.__file__)
    protocol["saved_development_origin"]["capture_producer_source_sha256"] = producer_sha
    protocol_bytes = coverage_source_capture._canonical_json(protocol)
    protocol_sha = _sha(protocol_bytes)
    cohorts_bytes = (material_root / "saved-development-method-materials/cohorts.json").read_bytes()
    cohorts_sha = _sha(cohorts_bytes)
    identity = _candidate_identity()
    identity_sha = _sha(identity)
    endpoint = "https://capture.example"
    endpoint_sha = _sha(endpoint.encode())
    sources = [_source("D26-R01", "source-1", 1, "https://docs.example/page")]
    stage_uuid = "f611a79a-9eef-46f4-b211-3d4d394e9e21"
    source_revision = "d1d4cd35cb1e5653404d605345377e4447938704"
    source_manifest = {
        "schema": "coverage-source-capture-manifest/1",
        "stage": "development",
        "stage_uuid": stage_uuid,
        "source_revision": source_revision,
        "protocol_sha256": protocol_sha,
        "cohorts_sha256": cohorts_sha,
        "candidate_identity_sha256": identity_sha,
        "candidate_endpoint_sha256": endpoint_sha,
        "sources": sources,
    }
    manifest_bytes = coverage_source_capture._canonical_json(source_manifest)
    receipt_root = tmp_path / "capture-receipts"
    receipt_root.mkdir(mode=0o700)

    def handler(request: httpx.Request):
        if request.method == "GET":
            return _healthy_response()
        return httpx.Response(200, content=_response("A saved source passage."), request=request)

    result = await coverage_source_capture.capture_sources_once(
        protocol_bytes=protocol_bytes,
        source_manifest_bytes=manifest_bytes,
        candidate_identity_bytes=identity,
        candidate_base_url=endpoint,
        operator_token="test-only-token",
        permit_receipt_bytes=b"external-permit-receipt",
        expected_permit_receipt_sha256=_sha(b"external-permit-receipt"),
        permit_verifier=_Verifier(),
        one_shot_lease=_Lease(),
        receipt_root=receipt_root,
        transport=httpx.MockTransport(handler),
        stage_started_monotonic=100.0,
        stage_deadline_monotonic=200.0,
        clock=lambda: 110.0,
    )
    inventory_bytes = (result.receipt_directory / "inventory.json").read_bytes()
    observed = coverage_resource_evidence._capture_observations(
        capture_result=result,
        expected_inventory_sha256=_sha(inventory_bytes),
        expected_manifest_sha256=_sha(manifest_bytes),
        protocol_sha256=protocol_sha,
        cohorts_sha256=cohorts_sha,
        source_revision=source_revision,
        candidate_identity_bytes=identity,
        candidate_identity_sha256=identity_sha,
        candidate_endpoint_sha256=endpoint_sha,
        timeout_limit_seconds=protocol["capture"]["timeout_seconds"],
        response_bytes_limit=protocol["capture"]["response_bytes"],
        producer_source_sha256=protocol["saved_development_origin"]["capture_producer_source_sha256"],
    )
    assert observed["capture_owned_http_calls"] == 2
    assert observed["capture_health_calls"] == 1
    assert observed["capture_scrape_calls"] == 1
    assert observed["capture_application_retries_observed"] == 0

    with pytest.raises(coverage_resource_evidence.ResourceEvidenceError, match="capture-control-attestation-binding"):
        coverage_resource_evidence._capture_observations(
            capture_result=result,
            expected_inventory_sha256=_sha(inventory_bytes),
            expected_manifest_sha256=_sha(manifest_bytes),
            protocol_sha256=protocol_sha,
            cohorts_sha256=cohorts_sha,
            source_revision=source_revision,
            candidate_identity_bytes=identity,
            candidate_identity_sha256=identity_sha,
            candidate_endpoint_sha256=endpoint_sha,
            timeout_limit_seconds=protocol["capture"]["timeout_seconds"],
            response_bytes_limit=protocol["capture"]["response_bytes"],
            producer_source_sha256="0" * 64,
        )
