from __future__ import annotations

import hashlib
import json
import os
import uuid
from dataclasses import replace
from pathlib import Path

import pytest

from scripts import coverage_frozen_pool_inputs as frozen
from scripts import coverage_pipeline_inputs as pipeline
from scripts import coverage_saved_corpus as saved
from scripts import coverage_source_capture as source_capture
from scripts import intent_ranking_receipts as response_receipts
from tests import test_coverage_pipeline_inputs as fixtures


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _write_private(path: Path, raw: bytes) -> None:
    path.write_bytes(raw)
    os.chmod(path, 0o600)


def _mkdir_private(path: Path) -> None:
    path.mkdir(mode=0o700)
    os.chmod(path, 0o700)


def _registration(
    cases: list[dict[str, object]], *, acquisition_plan_sha256: str = "4" * 64
) -> frozen.FrozenPoolRegistrationContext:
    return frozen.FrozenPoolRegistrationContext(
        stage_uuid=str(uuid.UUID(int=123)),
        stage_kind="development",
        source_revision="a" * 40,
        source_closure_sha256="5" * 64,
        protocol_sha256="1" * 64,
        cohorts_sha256="2" * 64,
        candidate_identity_sha256="3" * 64,
        candidate_endpoint_sha256="4" * 64,
        acquisition_plan_sha256=acquisition_plan_sha256,
        task_ids=tuple(row["task_id"] for row in cases),
        navigation_ids=tuple(f"D-N{i:02d}" for i in range(1, 6)),
        evidence_sha256="",
        collector_receipt_sha256="",
        availability_sha256="",
    )


def _origin_package(tmp_path: Path):
    cases = [fixtures.case(i) for i in range(1, 9)]
    stage = fixtures.make_stage(cases)
    for index, (task, operation) in enumerate(zip(cases, stage.operations[:8], strict=True)):
        if index < 4:
            task["pool_plan"] = {"band": "research_le_40", "engines": ["github", "wikipedia"]}
            operation.engines = ("github", "wikipedia")
            size = 21
        else:
            task["pool_plan"] = {
                "band": "research_41_80",
                "engines": ["arxiv", "github", "openalex", "wikipedia"],
            }
            operation.engines = ("arxiv", "github", "openalex", "wikipedia")
            size = 41
        operation.band = task["pool_plan"]["band"]
        operation.band_valid = True
        operation.canonical_response.scope.selected_engines = list(operation.engines)
        operation.canonical_response.results = [
            fixtures.result(card, namespace=task["task_id"]) for card in range(size)
        ]
        operation.pool_count = size

    acquisition_plan = b'{"schema":"coverage-acquisition-plan/1","operations":[]}'
    acquisition_plan_sha256 = _sha(acquisition_plan)
    collection = tmp_path / "collection"
    historical_source_dir = tmp_path / "historical-source"
    _mkdir_private(historical_source_dir)
    historical_source_bytes = b"# historical operation runner\n"
    historical_source_path = historical_source_dir / "coverage_study_acquire.py"
    _write_private(historical_source_path, historical_source_bytes)
    evidence = fixtures._fresh_frozen_evidence(
        collection, stage, cases, acquisition_plan_sha256=acquisition_plan_sha256
    )
    _write_private(collection / "acquisition-plan.json", acquisition_plan)
    _write_private(collection / "frozen-pool-evidence.json", evidence.evidence_bytes)
    _write_private(collection / "collector-receipt.json", evidence.collector_receipt_bytes)
    _write_private(collection / "engine-availability.json", evidence.availability_bytes)

    original_registration = _registration(cases, acquisition_plan_sha256=acquisition_plan_sha256)
    original_registration = replace(
        original_registration,
        evidence_sha256=evidence.expected_evidence_sha256,
        collector_receipt_sha256=evidence.expected_collector_receipt_sha256,
        availability_sha256=evidence.expected_availability_sha256,
    )
    phase = frozen.run_frozen_pool_input_phase(
        cases,
        evidence=evidence,
        registration=original_registration,
        stage_started_monotonic=100.0,
        stage_deadline_monotonic=10_000.0,
        now_monotonic=400.0,
    )
    old_manifest = phase.pipeline_inputs["capture_manifest"]

    receipt_parent = tmp_path / "capture-archive"
    _mkdir_private(receipt_parent)
    capture_root = receipt_parent / original_registration.stage_uuid
    _mkdir_private(capture_root)
    sources = old_manifest["sources"]
    source_manifest = {
        "schema": "coverage-source-capture-manifest/1",
        "stage": "development",
        "stage_uuid": original_registration.stage_uuid,
        "source_revision": original_registration.source_revision,
        "protocol_sha256": original_registration.protocol_sha256,
        "cohorts_sha256": original_registration.cohorts_sha256,
        "candidate_identity_sha256": original_registration.candidate_identity_sha256,
        "candidate_endpoint_sha256": original_registration.candidate_endpoint_sha256,
        "sources": sources,
    }
    source_manifest_raw = source_capture._canonical_json(source_manifest)
    _write_private(capture_root / "source-manifest.json", source_manifest_raw)
    qualification_bindings = {
        "stage_uuid": original_registration.stage_uuid,
        "source_revision": original_registration.source_revision,
        "protocol_sha256": original_registration.protocol_sha256,
        "candidate_identity_sha256": original_registration.candidate_identity_sha256,
        "candidate_endpoint_sha256": original_registration.candidate_endpoint_sha256,
        "candidate_endpoint_scheme": "https",
        "ca_bundle_sha256": "6" * 64,
        "candidate_runtime_revision": "a" * 40,
        "capture_module_sha256": "7" * 64,
    }
    qualification_raw = pipeline._canonical(
        {
            "schema": "coverage-operator-authority-receipt/1",
            "scope": "protected-source-capture",
            "decision": "allow",
            "bindings": qualification_bindings,
            "evidence": {
                "full_boundary_evidence_sha256": "8" * 64,
                "grok_config_sha256": "9" * 64,
                "grok_image_digest": "sha256:" + "a" * 64,
                "protected_profile_sha256": "b" * 64,
            },
        }
    )
    _write_private(capture_root / "protected-capture-qualification.json", qualification_raw)

    inventory_rows = []
    captured = failed = 0
    for sequence, source in enumerate(sources, start=1):
        operation_id = f"source-{sequence:04d}"
        successful = sequence == 1
        body = b"synthetic captured context" if successful else b"provider response unavailable"
        archive_meta = response_receipts.archive_response(
            receipt_parent,
            bindings={
                "stage_uuid": original_registration.stage_uuid,
                "operation_id": operation_id,
                "request_body_sha256": _sha(
                    source_capture._canonical_json(
                        {"url": source["url"], "formats": ["markdown"], "onlyMainContent": True}
                    )
                ),
                "source_revision": original_registration.source_revision,
            },
            complete=successful,
            status="complete" if successful else "http-error",
            http_status=200 if successful else 503,
            response_body=body,
        )
        row = {
            **source,
            "status": "captured" if successful else "source_response_failure",
            "attempted": True,
            "receipt_sha256": archive_meta["receipt_sha256"],
            "response_sha256": archive_meta["response_body_sha256"],
            "response_body_bytes": archive_meta["response_body_bytes"],
            "observed_response_body_bytes": len(body),
            "response_content_encoding": "identity",
            "timeout_seconds_applied": 30.0,
            "transport_dispatch_count": 1,
            "response_bytes_limit_applied": source_capture.MAX_RESPONSE_BYTES,
            "context_sha256": None,
            "failure_code": None if successful else "http-or-payload-rejected",
        }
        if successful:
            context = {
                "schema": "coverage-source-context/1",
                "task_id": source["task_id"],
                "source_id": source["source_id"],
                "result_index": source["result_index"],
                "source_url": source["url"],
                "title": source["title"],
                "engine": source["engine"],
                "context": "synthetic captured context",
                "context_characters": len("synthetic captured context"),
                "context_sha256": _sha(b"synthetic captured context"),
                "truncated": False,
                "relative_path": f"context-{sequence:04d}.json",
            }
            _write_private(capture_root / context["relative_path"], pipeline._canonical(context))
            row.update(
                {
                    "context_sha256": context["context_sha256"],
                    "context_characters": context["context_characters"],
                    "context_truncated": False,
                    "context_artifact": context["relative_path"],
                }
            )
            captured += 1
        else:
            failed += 1
        inventory_rows.append(row)

    health_body = b'{"ok":true}'
    health_meta = response_receipts.archive_response(
        receipt_parent,
        bindings={
            "stage_uuid": original_registration.stage_uuid,
            "operation_id": "health",
            "request_body_sha256": _sha(b""),
            "source_revision": original_registration.source_revision,
        },
        complete=True,
        status="complete",
        http_status=200,
        response_body=health_body,
    )
    capture_inventory = {
        "schema": "coverage-source-capture-inventory/1",
        "stage_uuid": original_registration.stage_uuid,
        "source_revision": original_registration.source_revision,
        "source_manifest_sha256": _sha(source_manifest_raw),
        "protocol_sha256": original_registration.protocol_sha256,
        "cohorts_sha256": original_registration.cohorts_sha256,
        "status": "complete-with-source-failures",
        "owned_http_calls": len(sources) + 1,
        "health_calls": 1,
        "scrape_calls": len(sources),
        "health_receipt_sha256": health_meta["receipt_sha256"],
        "health_response_body_bytes": len(health_body),
        "protected_capture_qualification": {
            "status": "qualified",
            "scope": "protected-source-capture",
            "receipt_file": "protected-capture-qualification.json",
            "receipt_sha256": _sha(qualification_raw),
            "grok_image_digest": "sha256:" + "a" * 64,
            "grok_config_sha256": "9" * 64,
            "protected_profile_sha256": "b" * 64,
            "full_boundary_evidence_sha256": "8" * 64,
            "bindings": qualification_bindings,
        },
        "quality_credit": False,
        "sources": inventory_rows,
    }
    inventory_raw = source_capture._canonical_json(capture_inventory)
    _write_private(capture_root / "inventory.json", inventory_raw)
    receipt_inventory_sha = __import__("scripts.coverage_resource_evidence", fromlist=["x"]).receipt_inventory_sha256(
        collection
    )
    package_sha = saved.package_sha256(collection, capture_root)
    origin = saved.SavedCorpusOrigin(
        registration=original_registration,
        research_cases=tuple(cases),
        navigation_targets=tuple(
            {
                "target_id": f"D-N{i:02d}",
                "search_query": f"synthetic nav {i}",
                "target_url": f"https://github.com/example/{i}",
                "navigation_pool_plan": {"engines": ["github"]},
            }
            for i in range(1, 6)
        ),
        collection_directory=collection,
        expected_collection_receipt_inventory_sha256=receipt_inventory_sha,
        capture_stage_directory=capture_root,
        expected_capture_inventory_sha256=_sha(inventory_raw),
        expected_capture_manifest_sha256=_sha(source_manifest_raw),
        expected_package_sha256=package_sha,
        operation_runner_source_sidecar=historical_source_path,
        operation_runner_source_sha256=_sha(historical_source_bytes),
    )
    return origin, cases, list(origin.navigation_targets), collection, capture_root


def _current_binding(
    origin: saved.SavedCorpusOrigin, *, source_revision: str | None = None
) -> saved.SavedCorpusStageBinding:
    return saved.SavedCorpusStageBinding(
        stage_uuid=str(uuid.UUID(int=901)),
        stage_kind="development",
        source_revision=source_revision or origin.registration.source_revision,
        source_closure_sha256="d" * 64,
        protocol_sha256="e" * 64,
        cohorts_sha256=origin.registration.cohorts_sha256,
        candidate_identity_sha256=origin.registration.candidate_identity_sha256,
        candidate_endpoint_sha256=origin.registration.candidate_endpoint_sha256,
        packet_stage_uuid=str(uuid.UUID(int=902)),
        origin_package_sha256=origin.expected_package_sha256,
        import_authority_receipt_sha256="f" * 64,
    )


def test_saved_corpus_import_preserves_origin_and_failed_capture_rows(tmp_path):
    origin, cases, nav, collection, capture_root = _origin_package(tmp_path)
    before_collection = saved.package_sha256(collection, capture_root)
    result = saved.verify_and_import_saved_corpus(origin, _current_binding(origin), cases, nav)

    imported_inventory = json.loads(result.capture_inventory_bytes)
    imported_manifest = json.loads(result.capture_manifest_bytes)
    receipt = json.loads(result.import_receipt_bytes)
    assert imported_inventory["schema"] == saved.CAPTURE_IMPORT_SCHEMA
    assert imported_inventory["stage_uuid"] != origin.registration.stage_uuid
    assert imported_inventory["owned_http_calls"] == 0
    assert imported_inventory["scrape_calls"] == 0
    assert imported_inventory["status"] == "complete-with-source-failures"
    assert imported_inventory["captured_count"] == 1
    assert imported_inventory["failed_count"] == len(imported_inventory["sources"]) - 1
    assert len(result.context_artifacts) == 1
    assert result.current_search_calls == result.current_capture_calls == 0
    assert result.historical_search_calls == 13
    assert result.historical_scrape_calls == len(imported_inventory["sources"])
    assert result.origin_capture_result.stage_uuid == origin.registration.stage_uuid
    assert result.origin_capture_result.failed_count == len(imported_inventory["sources"]) - 1
    assert imported_manifest["stage_uuid"] != origin.registration.stage_uuid
    assert imported_manifest["sources"] == json.loads((capture_root / "source-manifest.json").read_bytes())["sources"]
    assert receipt["origin_stage_uuid"] == origin.registration.stage_uuid
    assert receipt["current_search_calls"] == receipt["current_capture_calls"] == 0
    assert receipt["quality_credit"] is False
    assert saved.package_sha256(collection, capture_root) == before_collection


def test_saved_corpus_import_allows_fresh_source_revision_and_rejects_malformed_revision(tmp_path):
    origin, cases, nav, _collection, capture_root = _origin_package(tmp_path)
    fresh_revision = "b" * 40
    binding = _current_binding(origin, source_revision=fresh_revision)
    result = saved.verify_and_import_saved_corpus(origin, binding, cases, nav)

    old_capture_manifest = json.loads((capture_root / "source-manifest.json").read_bytes())
    new_capture_manifest = json.loads(result.capture_manifest_bytes)
    receipt = json.loads(result.import_receipt_bytes)
    assert origin.registration.source_revision != fresh_revision
    assert old_capture_manifest["source_revision"] == origin.registration.source_revision
    assert new_capture_manifest["source_revision"] == fresh_revision
    assert receipt["source_revision"] == fresh_revision
    assert receipt["origin_source_revision"] == origin.registration.source_revision

    malformed = saved.SavedCorpusStageBinding(**{**binding.__dict__, "source_revision": "not-a-revision"})
    with pytest.raises(saved.SavedCorpusError, match="current-source-revision-invalid"):
        saved.verify_and_import_saved_corpus(origin, malformed, cases, nav)


def test_saved_corpus_import_fails_closed_on_tampering_and_identity_reuse(tmp_path):
    origin, cases, nav, collection, capture_root = _origin_package(tmp_path)
    binding = _current_binding(origin)
    original = (collection / "collection-summary.json").read_bytes()
    _write_private(collection / "collection-summary.json", original + b" ")
    with pytest.raises(saved.SavedCorpusError, match="origin-package-integrity-mismatch"):
        saved.verify_and_import_saved_corpus(origin, binding, cases, nav)
    _write_private(collection / "collection-summary.json", original)

    bad_packet = saved.SavedCorpusStageBinding(
        **{**binding.__dict__, "packet_stage_uuid": origin.registration.stage_uuid}
    )
    with pytest.raises(saved.SavedCorpusError, match="current-stage-identities-not-fresh"):
        saved.verify_and_import_saved_corpus(origin, bad_packet, cases, nav)

    bad_cases = [dict(row) for row in cases]
    bad_cases[0]["task_id"] = "D-R99"
    with pytest.raises(saved.SavedCorpusError, match="current-cohort-must-match-origin-exactly"):
        saved.verify_and_import_saved_corpus(origin, binding, bad_cases, nav)

    captured = json.loads((capture_root / "context-0001.json").read_bytes())
    captured["context"] = "tampered"
    _write_private(capture_root / "context-0001.json", pipeline._canonical(captured))
    new_inventory_sha = _sha((capture_root / "inventory.json").read_bytes())
    tampered_origin = saved.SavedCorpusOrigin(
        **{**origin.__dict__, "expected_capture_inventory_sha256": new_inventory_sha}
    )
    tampered_origin = saved.SavedCorpusOrigin(
        **{
            **tampered_origin.__dict__,
            "expected_package_sha256": saved.package_sha256(collection, capture_root),
        }
    )
    tampered_binding = saved.SavedCorpusStageBinding(
        **{**binding.__dict__, "origin_package_sha256": tampered_origin.expected_package_sha256}
    )
    with pytest.raises(saved.SavedCorpusError, match="origin-capture-context-binding-mismatch"):
        saved.verify_and_import_saved_corpus(tampered_origin, tampered_binding, cases, nav)


def test_saved_corpus_rejects_missing_context_confirmation_and_packet_collision(tmp_path):
    origin, cases, nav, _collection, capture_root = _origin_package(tmp_path)
    binding = _current_binding(origin)

    confirmation = saved.SavedCorpusStageBinding(**{**binding.__dict__, "stage_kind": "confirmation"})
    with pytest.raises(saved.SavedCorpusError, match="saved-corpus-development-only"):
        saved.verify_and_import_saved_corpus(origin, confirmation, cases, nav)

    collision = saved.SavedCorpusStageBinding(**{**binding.__dict__, "packet_stage_uuid": binding.stage_uuid})
    with pytest.raises(saved.SavedCorpusError, match="current-stage-identities-not-fresh"):
        saved.verify_and_import_saved_corpus(origin, collision, cases, nav)

    (capture_root / "context-0001.json").unlink()
    missing_package = saved.package_sha256(origin.collection_directory, capture_root)
    missing_origin = saved.SavedCorpusOrigin(**{**origin.__dict__, "expected_package_sha256": missing_package})
    missing_binding = saved.SavedCorpusStageBinding(**{**binding.__dict__, "origin_package_sha256": missing_package})
    with pytest.raises(saved.SavedCorpusError, match="saved-corpus-file-unavailable"):
        saved.verify_and_import_saved_corpus(missing_origin, missing_binding, cases, nav)
