from __future__ import annotations

import json
import os
import threading
import time
import uuid
from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import pytest

from scripts import coverage_atomic_io
from scripts import coverage_late_registration as late
from scripts import coverage_local_authority as authority
from scripts import coverage_operator_handoff as handoff
from scripts import coverage_operator_runner as operator_runner
from scripts import coverage_saved_corpus as saved
from scripts import coverage_stage_orchestration as orchestration
from scripts import coverage_study_core as core
from tests.test_coverage_operator_handoff import canonical, digest
from tests.test_coverage_saved_corpus import _current_binding, _origin_package

_MISSING = object()


def _write_private(path: Path, raw: bytes) -> None:
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    os.chmod(path.parent, 0o700)
    coverage_atomic_io.write_new(path, raw)


def _saved_case(tmp_path: Path):
    origin_root = tmp_path / "origin"
    origin_root.mkdir(mode=0o700)
    origin, cases, navigation, _collection, _capture = _origin_package(origin_root)
    stage_uuid = str(uuid.UUID(int=94001))
    packet_uuid = str(uuid.UUID(int=94002))
    cohorts = canonical({"registered": "saved fixture cohorts"})
    candidate = b"saved fixture candidate identity"
    origin_registration = replace(
        origin.registration,
        cohorts_sha256=digest(cohorts),
        candidate_identity_sha256=digest(candidate),
    )
    origin = replace(origin, registration=origin_registration)
    closure_bytes = canonical({"source_hash": "a" * 64})
    closure = digest(closure_bytes)
    protocol = canonical(
        {
            "cohorts_sha256": digest(cohorts),
            "selector_input_map_schema": "coverage-selector-input-map/2-registered",
            "selector_input_map_status": "registered",
            "pool_input_mode_by_stage": {
                "development": "saved-corpus/1",
                "confirmation": "frozen-pool/1",
            },
            "inputs": {"source_artifact_reuse": "development-only"},
        }
    )
    acquisition_plan = canonical({"fixture": "registered"})
    manifest = canonical({"fixture": "saved-import-plan"})
    plan = orchestration.StagePlan(
        stage_uuid=stage_uuid,
        stage_kind="development",
        source_revision="b" * 40,
        protocol_bytes=protocol,
        cohorts_bytes=cohorts,
        acquisition_plan_bytes=acquisition_plan,
        acquisition_manifest_bytes=manifest,
        source_closure_sha256=closure,
        research_cases=tuple(cases),
        navigation_targets=tuple(navigation),
        candidate_identity_bytes=candidate,
        candidate_endpoint_sha256=origin_registration.candidate_endpoint_sha256,
        packet_stage_uuid=packet_uuid,
        stage_started_monotonic=100.0,
        stage_deadline_monotonic=28_900.0,
        expected_acquisition_manifest_sha256=digest(manifest),
        pool_input_mode="saved-corpus/1",
        saved_origin=origin,
    )
    receipt_raw = saved._canonical(
        {
            "schema": saved.SCHEMA,
            "origin_stage_uuid": origin_registration.stage_uuid,
            "origin_package_sha256": origin.expected_package_sha256,
            "stage_uuid": stage_uuid,
            "packet_stage_uuid": packet_uuid,
            "current_search_calls": 0,
            "current_capture_calls": 0,
            "historical_search_calls": 13,
            "historical_capture_calls": 363,
            "historical_health_calls": 1,
            "historical_scrape_calls": 362,
            "source_rows": 362,
            "captured_contexts": 180,
            "source_failures": 182,
            "import_authority_receipt_sha256": "f" * 64,
            "derived_capture_manifest_sha256": digest(b"derived capture manifest"),
            "origin_capture_inventory_sha256": "e" * 64,
        }
    )
    receipt_sha = digest(receipt_raw)
    capture_inventory = saved._canonical(
        {
            "schema": saved.CAPTURE_IMPORT_SCHEMA,
            "stage_uuid": stage_uuid,
            "origin_stage_uuid": origin_registration.stage_uuid,
            "origin_inventory_sha256": "e" * 64,
            "import_receipt_sha256": receipt_sha,
        }
    )
    capture_manifest = b"derived capture manifest"
    imported = saved.SavedCorpusImportResult(
        pipeline_inputs={"stage_uuid": stage_uuid},
        capture_inventory_bytes=capture_inventory,
        context_artifacts={},
        origin_pool_verified=SimpleNamespace(),
        origin_capture_result=SimpleNamespace(),
        origin_package_sha256=origin.expected_package_sha256,
        import_receipt_bytes=receipt_raw,
        import_receipt_sha256=receipt_sha,
        capture_manifest_bytes=capture_manifest,
        capture_manifest_sha256=digest(capture_manifest),
        historical_search_calls=13,
        historical_capture_calls=363,
        historical_scrape_calls=362,
        historical_health_calls=1,
        historical_captured_sources=180,
        historical_failed_sources=182,
    )
    reference = late.saved_corpus_import_reference(imported, plan=plan)

    pins = {name: digest(name.encode()) for name in core.PIN_NAMES}
    pins.update({"protocol": digest(protocol), "qualified_source_closure": closure})
    registration = canonical(
        {
            "schema": core.REGISTRATION_SCHEMA,
            "stage_uuid": stage_uuid,
            "stage_kind": "development",
            "source_revision": plan.source_revision,
            "pins": pins,
            "operation_ids": [],
            "navigation_skip_reasons": [None] * len(navigation),
            "task_ids": [row["task_id"] for row in cases],
        }
    )
    prepared = core.PreparedStage(
        stage_uuid=stage_uuid,
        stage_kind="development",
        source_revision=plan.source_revision,
        registration_sha256=digest(registration),
        pins=pins,
        navigation_skip_reasons=(None,) * len(navigation),
        operation_ids=(),
        acquisition_calls=0,
        scraper_calls=0,
        health_calls=0,
        selector_input_map_schema="coverage-selector-input-map/2-registered",
    )
    selector_map = canonical({"schema": prepared.selector_input_map_schema, "operations": []})
    artifacts = {
        "protocol": protocol,
        "cohorts": cohorts,
        "source-closure": closure_bytes,
        "initial-registration": registration,
        "active-registration": registration,
        "selector-map": selector_map,
        "saved-corpus-import-reference": reference,
        "saved-corpus-import-receipt-sha256": receipt_sha.encode("ascii"),
        "capture-manifest": capture_manifest,
    }
    request_bindings = {
        "stage_uuid": stage_uuid,
        "source_revision": plan.source_revision,
        "protocol_sha256": digest(protocol),
        "registration_sha256": digest(registration),
        "source_closure_sha256": closure,
        "selector_input_map_sha256": digest(selector_map),
        "origin_package_sha256": imported.origin_package_sha256,
        "saved_corpus_import_receipt_sha256": imported.import_receipt_sha256,
    }
    permit_bindings = {
        "stage_uuid": plan.stage_uuid,
        "packet_stage_uuid": plan.packet_stage_uuid,
        "source_revision": plan.source_revision,
        "source_closure_sha256": plan.source_closure_sha256,
        "protocol_sha256": digest(plan.protocol_bytes),
        "cohorts_sha256": digest(plan.cohorts_bytes),
        "candidate_identity_sha256": digest(plan.candidate_identity_bytes),
        "candidate_endpoint_sha256": plan.candidate_endpoint_sha256,
        "origin_package_sha256": origin.expected_package_sha256,
    }
    permit = authority._canonical(
        {
            "schema": authority.SCHEMA,
            "scope": "saved-corpus-import",
            "decision": "allow",
            "bindings": permit_bindings,
        }
    )
    permit_base = tmp_path / "handoff" / stage_uuid / "saved-corpus-import"
    _write_private(permit_base / "responses" / "permit.receipt", permit)
    _write_private(permit_base / "pins" / "permit.sha256", (digest(permit) + "\n").encode())
    return plan, prepared, selector_map, artifacts, request_bindings, imported, permit_base


def _issue(tmp_path: Path, case, *, changed_bindings=None, changed_artifacts=None, saved_mode=True):
    plan, prepared, selector_map, artifacts, bindings, imported, permit_base = case
    stage = plan.stage_uuid
    closure = plan.source_closure_sha256
    if not saved_mode:
        plan = replace(plan, pool_input_mode="frozen-pool/1", saved_origin=None)
    runner = SimpleNamespace(
        plan=plan,
        active_environment_receipt_bytes=b'{"schema":"coverage-active-python-environment/1"}',
        static_materials={"qualified_source_closure": artifacts["source-closure"]},
        initial_registration=artifacts["initial-registration"],
        _active_registration_bytes=artifacts["active-registration"],
        _active_input_manifest_bytes=None,
        _frozen_pool_reference_bytes=None,
        _saved_corpus_reference_bytes=artifacts.get("saved-corpus-import-reference") if saved_mode else None,
        _frozen_stage_clock_bytes=None,
        _pipeline_inputs={"capture_manifest_bytes": artifacts.get("capture-manifest")},
    )
    context_bindings, artifacts = operator_runner.OperatorStageRunner._phase_request_context(runner, dict(artifacts))
    bindings = {**bindings, **context_bindings}
    if changed_artifacts:
        artifacts = {**artifacts, **changed_artifacts}
    if not saved_mode:
        bindings = {
            key: value
            for key, value in bindings.items()
            if key not in {"origin_package_sha256", "saved_corpus_import_receipt_sha256"}
        }
    if changed_bindings:
        bindings = dict(bindings)
        for key, value in changed_bindings.items():
            if value is _MISSING:
                bindings.pop(key, None)
            else:
                bindings[key] = value
    root = tmp_path / f"handoff-{uuid.uuid4()}"
    root.mkdir(mode=0o700)
    os.chmod(root, 0o700)
    exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
    stage_root = root / stage
    stage_root.mkdir(mode=0o700)
    os.chmod(stage_root, 0o700)
    request_permit = root / stage / "saved-corpus-import"
    request_permit.mkdir(mode=0o700)
    os.chmod(request_permit, 0o700)
    for child in ("responses", "pins"):
        (request_permit / child).mkdir(mode=0o700)
        os.chmod(request_permit / child, 0o700)
    _write_private(
        request_permit / "responses" / "permit.receipt",
        (permit_base / "responses" / "permit.receipt").read_bytes(),
    )
    _write_private(
        request_permit / "pins" / "permit.sha256",
        (permit_base / "pins" / "permit.sha256").read_bytes(),
    )
    results = []

    def request():
        try:
            results.append(
                exchange.request(
                    stage_uuid=stage,
                    scope="selector-map-admission",
                    request_id="map",
                    bindings=bindings,
                    deadline_monotonic=time.monotonic() + 3,
                    artifacts=artifacts,
                )
            )
        except Exception as exc:  # captured for exact assertions below
            results.append(exc)

    worker = threading.Thread(target=request)
    worker.start()
    request_path = root / stage / "selector-map-admission" / "requests" / "map.json"
    for _ in range(300):
        if request_path.exists():
            break
        time.sleep(0.01)
    if not request_path.exists():
        worker.join(timeout=1)
        raise AssertionError(f"handoff request did not arrive: {results!r}")
    request_bytes = request_path.read_bytes()
    try:
        with mock.patch.object(saved, "verify_and_import_saved_corpus", return_value=imported):
            receipt = authority.PhaseArtifactReceiptIssuer.selector_map_admission(
                root=root,
                stage_uuid=stage,
                request_id="map",
                request_bytes=request_bytes,
                expected_source_closure_sha256=closure,
                expected_initial_registration_sha256=digest(artifacts["initial-registration"]),
                expected_active_registration_sha256=prepared.registration_sha256,
                prepared=prepared,
                expected_selector_map_bytes=selector_map,
                plan=plan,
            )
        response = request_path.parent.parent / "responses" / "map.receipt"
        _write_private(response, receipt)
        pin = request_path.parent.parent / "pins" / "map.sha256"
        _write_private(pin, (digest(receipt) + "\n").encode())
        return receipt, results, request_path, root
    except Exception:
        response = request_path.parent.parent / "responses" / "map.receipt"
        _write_private(response, b"refused")
        pin = request_path.parent.parent / "pins" / "map.sha256"
        _write_private(pin, (digest(b"refused") + "\n").encode())
        raise
    finally:
        if worker.is_alive():
            worker.join(timeout=3)


def test_saved_development_selector_map_admission_binds_import_provenance_and_keeps_receipt_schema(tmp_path):
    """Exercise file handoff and permit bindings with the deterministic saved-import fixture.

    Full archive replay is independently covered by the saved-corpus verifier tests.
    """
    case = _saved_case(tmp_path)
    receipt, results, _request_path, _root = _issue(tmp_path, case)
    assert len(results) == 1 and isinstance(results[0], tuple)
    doc = json.loads(receipt)
    assert set(doc) == {
        "schema",
        "status",
        "stage_uuid",
        "source_revision",
        "protocol_sha256",
        "registration_sha256",
        "source_closure_sha256",
        "selector_input_map_sha256",
    }
    assert doc["status"] == "admitted"


def test_saved_origin_import_is_replayed_by_the_unmocked_read_only_verifier(tmp_path):
    origin_root = tmp_path / "origin-replay"
    origin_root.mkdir(mode=0o700)
    origin, cases, navigation, collection, capture_root = _origin_package(origin_root)
    package_before = saved.package_sha256(collection, capture_root)

    imported = saved.verify_and_import_saved_corpus(
        origin,
        _current_binding(origin),
        cases,
        navigation,
    )

    assert imported.origin_package_sha256 == package_before == origin.expected_package_sha256
    assert imported.current_search_calls == 0
    assert imported.current_capture_calls == 0


@pytest.mark.parametrize(
    "changes",
    [
        {"origin_package_sha256": _MISSING},
        {"origin_package_sha256": "0" * 64},
        {"saved_corpus_import_receipt_sha256": _MISSING},
        {"saved_corpus_import_receipt_sha256": "0" * 64},
    ],
)
def test_saved_development_selector_map_rejects_missing_or_tampered_import_bindings(tmp_path, changes):
    case = _saved_case(tmp_path)
    with pytest.raises(authority.LocalAuthorityError, match="selector-map-request-binding-mismatch"):
        _issue(tmp_path, case, changed_bindings=changes)


@pytest.mark.parametrize(
    "artifact_name",
    [
        "saved-corpus-import-reference",
        "saved-corpus-import-receipt-sha256",
        "capture-manifest",
    ],
)
def test_saved_development_selector_map_rejects_tampered_import_artifacts(tmp_path, artifact_name):
    case = _saved_case(tmp_path)
    artifact = case[3][artifact_name]
    tampered = artifact[:-1] + (b"0" if artifact[-1:] != b"0" else b"1")
    with pytest.raises(authority.LocalAuthorityError, match="saved-corpus-selector-map-import-binding-mismatch"):
        _issue(tmp_path, case, changed_artifacts={artifact_name: tampered})


def test_fresh_selector_map_rejects_saved_import_fields(tmp_path):
    case = _saved_case(tmp_path)
    plan, prepared, selector_map, artifacts, bindings, _imported, _permit = case
    registration = artifacts["initial-registration"]
    plan = replace(plan, pool_input_mode="frozen-pool/1", saved_origin=None)
    fresh_prepared = replace(prepared, pins={**prepared.pins})
    runner = SimpleNamespace(
        plan=plan,
        active_environment_receipt_bytes=b'{"schema":"coverage-active-python-environment/1"}',
        static_materials={"qualified_source_closure": artifacts["source-closure"]},
        initial_registration=registration,
        _active_registration_bytes=registration,
        _active_input_manifest_bytes=None,
        _frozen_pool_reference_bytes=None,
        _saved_corpus_reference_bytes=None,
        _frozen_stage_clock_bytes=None,
        _pipeline_inputs={"capture_manifest_bytes": artifacts["capture-manifest"]},
    )
    context_bindings, context_artifacts = operator_runner.OperatorStageRunner._phase_request_context(
        runner, {key: value for key, value in artifacts.items() if not key.startswith("saved-corpus-import")}
    )
    # The same exact six-key public contract applies to the fresh mode.
    root = tmp_path / "fresh-handoff"
    exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
    result = []

    def request():
        try:
            result.append(
                exchange.request(
                    stage_uuid=plan.stage_uuid,
                    scope="selector-map-admission",
                    request_id="fresh",
                    bindings={
                        **{
                            k: v
                            for k, v in bindings.items()
                            if k not in {"origin_package_sha256", "saved_corpus_import_receipt_sha256"}
                        },
                        **context_bindings,
                        "origin_package_sha256": "1" * 64,
                        "saved_corpus_import_receipt_sha256": "2" * 64,
                    },
                    deadline_monotonic=time.monotonic() + 3,
                    artifacts=context_artifacts,
                )
            )
        except Exception as exc:
            result.append(exc)

    worker = threading.Thread(target=request)
    worker.start()
    request_path = root / plan.stage_uuid / "selector-map-admission" / "requests" / "fresh.json"
    for _ in range(300):
        if request_path.exists():
            break
        time.sleep(0.01)
    with pytest.raises(authority.LocalAuthorityError, match="selector-map-request-binding-mismatch"):
        authority.PhaseArtifactReceiptIssuer.selector_map_admission(
            root=root,
            stage_uuid=plan.stage_uuid,
            request_id="fresh",
            request_bytes=request_path.read_bytes(),
            expected_source_closure_sha256=plan.source_closure_sha256,
            expected_initial_registration_sha256=digest(registration),
            expected_active_registration_sha256=fresh_prepared.registration_sha256,
            prepared=fresh_prepared,
            expected_selector_map_bytes=selector_map,
            plan=plan,
        )
    response = request_path.parent.parent / "responses" / "fresh.receipt"
    _write_private(response, b"refused")
    pin = request_path.parent.parent / "pins" / "fresh.sha256"
    _write_private(pin, (digest(b"refused") + "\n").encode())
    worker.join(timeout=3)
