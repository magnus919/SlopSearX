from __future__ import annotations

import hashlib
import json
import uuid
from dataclasses import replace

import pytest

from scripts import coverage_late_registration as late
from scripts import coverage_local_authority as authority
from scripts import coverage_operator_handoff as handoff
from scripts import coverage_saved_corpus as saved
from scripts import coverage_stage_orchestration as orchestration
from tests.test_coverage_saved_corpus import _origin_package


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _plan(origin: saved.SavedCorpusOrigin) -> orchestration.StagePlan:
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
    cohorts = b"registered-cohorts"
    candidate = b"registered-candidate"
    origin = replace(
        origin,
        registration=replace(
            origin.registration,
            cohorts_sha256=_sha(cohorts),
            candidate_identity_sha256=_sha(candidate),
            candidate_endpoint_sha256="4" * 64,
        ),
    )
    manifest = b"{}"
    return orchestration.StagePlan(
        stage_uuid=str(uuid.UUID(int=7001)),
        stage_kind="development",
        source_revision=origin.registration.source_revision,
        protocol_bytes=protocol,
        cohorts_bytes=cohorts,
        acquisition_plan_bytes=b"plan",
        acquisition_manifest_bytes=manifest,
        source_closure_sha256="5" * 64,
        research_cases=origin.research_cases,
        navigation_targets=origin.navigation_targets,
        candidate_identity_bytes=candidate,
        candidate_endpoint_sha256=origin.registration.candidate_endpoint_sha256,
        packet_stage_uuid=str(uuid.UUID(int=7002)),
        stage_started_monotonic=10.0,
        stage_deadline_monotonic=100.0,
        expected_acquisition_manifest_sha256=_sha(manifest),
        pool_input_mode="saved-corpus/1",
        saved_origin=origin,
    )


def test_saved_import_permit_binds_fresh_stage_and_exact_origin_package(tmp_path):
    origin, _cases, _nav, _collection, _capture = _origin_package(tmp_path)
    plan = _plan(origin)
    bindings = {
        "stage_uuid": plan.stage_uuid,
        "packet_stage_uuid": plan.packet_stage_uuid,
        "source_revision": plan.source_revision,
        "source_closure_sha256": plan.source_closure_sha256,
        "protocol_sha256": _sha(plan.protocol_bytes),
        "cohorts_sha256": _sha(plan.cohorts_bytes),
        "candidate_identity_sha256": _sha(plan.candidate_identity_bytes),
        "candidate_endpoint_sha256": plan.candidate_endpoint_sha256,
        "origin_package_sha256": origin.expected_package_sha256,
    }
    receipt = authority._canonical(
        {
            "schema": authority.SCHEMA,
            "decision": "allow",
            "scope": "saved-corpus-import",
            "bindings": bindings,
        }
    )
    permit = authority.PinnedReceiptAuthorities.saved_corpus_import(plan, receipt, _sha(receipt))
    assert permit.status == "verified-admitted"
    assert permit.stage_uuid == plan.stage_uuid
    assert permit.origin_package_sha256 == origin.expected_package_sha256

    changed = authority._canonical(
        {
            "schema": authority.SCHEMA,
            "decision": "allow",
            "scope": "saved-corpus-import",
            "bindings": {**bindings, "packet_stage_uuid": origin.registration.stage_uuid},
        }
    )
    with pytest.raises(authority.LocalAuthorityError, match="receipt-claims-mismatch"):
        authority.PinnedReceiptAuthorities.saved_corpus_import(plan, changed, _sha(changed))


def test_saved_import_authority_rejects_confirmation_and_mutated_origin(tmp_path):
    origin, _cases, _nav, collection, capture = _origin_package(tmp_path)
    plan = _plan(origin)
    with pytest.raises(authority.LocalAuthorityError, match="saved-corpus-import-plan-invalid"):
        authority.PinnedReceiptAuthorities.saved_corpus_import(
            replace(plan, stage_kind="confirmation"), b"{}", _sha(b"{}")
        )

    (collection / "engine-availability.json").write_bytes(b"tamper")
    with pytest.raises(authority.LocalAuthorityError, match="saved-corpus-import-origin-package-mismatch"):
        authority.PinnedReceiptAuthorities.saved_corpus_import(plan, b"{}", _sha(b"{}"))


def test_saved_import_authority_uses_registered_stage_specific_protocol_mode(tmp_path):
    origin, _cases, _nav, _collection, _capture = _origin_package(tmp_path)
    plan = _plan(origin)
    protocol = json.loads(plan.protocol_bytes)
    protocol["pool_input_mode_by_stage"]["development"] = "frozen-pool/1"
    changed_plan = replace(
        plan,
        protocol_bytes=json.dumps(protocol, sort_keys=True, separators=(",", ":")).encode(),
    )

    with pytest.raises(authority.LocalAuthorityError, match="saved-corpus-import-protocol-invalid"):
        authority.PinnedReceiptAuthorities.saved_corpus_import(changed_plan, b"{}", _sha(b"{}"))


@pytest.mark.asyncio
@pytest.mark.parametrize("clock", [None, b"", "not-bytes"])
async def test_saved_import_requires_a_clock_before_requesting_authority(tmp_path, clock):
    origin, *_rest = _origin_package(tmp_path)
    plan = _plan(origin)
    exchange = handoff.OperatorReceiptHandoff(tmp_path / "handoff")
    with pytest.raises(late.LateRegistrationError, match="saved-corpus-import-stage-clock-required"):
        await late.request_saved_corpus_import(
            exchange=exchange,
            plan=plan,
            expected_initial_registration_sha256="a" * 64,
            stage_clock_bytes=clock,
            phase_bindings={},
            phase_artifacts={},
        )
    assert not list((tmp_path / "handoff").rglob("*.request"))


def test_saved_import_reference_is_current_stage_bound_and_contains_no_paths(tmp_path):
    origin, cases, nav, _collection, _capture = _origin_package(tmp_path)
    binding = saved.SavedCorpusStageBinding(
        stage_uuid=str(uuid.UUID(int=901)),
        packet_stage_uuid=str(uuid.UUID(int=902)),
        stage_kind="development",
        source_revision=origin.registration.source_revision,
        source_closure_sha256="d" * 64,
        protocol_sha256="e" * 64,
        cohorts_sha256=origin.registration.cohorts_sha256,
        candidate_identity_sha256=origin.registration.candidate_identity_sha256,
        candidate_endpoint_sha256=origin.registration.candidate_endpoint_sha256,
        origin_package_sha256=origin.expected_package_sha256,
        import_authority_receipt_sha256="f" * 64,
    )
    imported = saved.verify_and_import_saved_corpus(origin, binding, cases, nav)
    plan = orchestration.StagePlan(
        stage_uuid=binding.stage_uuid,
        stage_kind="development",
        source_revision=binding.source_revision,
        protocol_bytes=b"{}",
        cohorts_bytes=b"{}",
        acquisition_plan_bytes=b"plan",
        acquisition_manifest_bytes=b"{}",
        source_closure_sha256=binding.source_closure_sha256,
        research_cases=tuple(cases),
        navigation_targets=tuple(nav),
        candidate_identity_bytes=b"candidate",
        candidate_endpoint_sha256=binding.candidate_endpoint_sha256,
        packet_stage_uuid=binding.packet_stage_uuid,
        stage_started_monotonic=1.0,
        stage_deadline_monotonic=10.0,
        expected_acquisition_manifest_sha256=_sha(b"{}"),
        pool_input_mode="saved-corpus/1",
        saved_origin=origin,
    )
    reference = json.loads(late.saved_corpus_import_reference(imported, plan=plan))
    assert reference["origin_package_sha256"] == origin.expected_package_sha256
    assert reference["current_capture_calls"] == reference["current_search_calls"] == 0
    assert not any("path" in key for key in reference)
