"""Resource accounting consumes actual mocked frozen-collector artifacts."""

import json

import pytest

from scripts import coverage_frozen_pool_inputs as inputs
from scripts import coverage_resource_evidence as resources
from scripts import coverage_study_acquire as acquisition
from tests.test_coverage_frozen_pool_collect import _collect, _manifest


def _verified(result, manifest, pacer):
    root = result.collection_directory
    evidence = inputs.FrozenPoolEvidence(
        root,
        (root / "frozen-pool-evidence.json").read_bytes(),
        result.evidence_sha256,
        (root / "collector-receipt.json").read_bytes(),
        result.collector_receipt_sha256,
        (root / "engine-availability.json").read_bytes(),
        result.availability_sha256,
    )
    return inputs.verify_frozen_pool_evidence(
        evidence,
        expected_stage_uuid=manifest["stage_uuid"],
        expected_stage_kind=manifest["stage"],
        expected_source_revision=manifest["source_revision"],
        expected_source_closure_sha256=manifest["source_closure_sha256"],
        expected_protocol_sha256=manifest["protocol_sha256"],
        expected_cohorts_sha256=manifest["cohorts_sha256"],
        expected_candidate_identity_sha256="5" * 64,
        expected_candidate_endpoint_sha256="6" * 64,
        expected_acquisition_plan_sha256=manifest["acquisition_plan_sha256"],
        expected_task_ids=tuple(row["task_id"] for row in manifest["research_cases"]),
        expected_navigation_ids=tuple(row["target_id"] for row in manifest["navigation_targets"]),
        stage_started_monotonic=0.0,
        stage_deadline_monotonic=28_800.0,
        now_monotonic=pacer.monotonic(),
    )


def _observe(result, manifest, plan, verified):
    root = result.collection_directory
    return resources.collect_acquisition_observations(
        snapshots_directory=root,
        receipt_directory=root,
        expected_receipt_inventory_sha256=resources.receipt_inventory_sha256(root),
        acquisition_manifest_bytes=(root / "acquisition-manifest.json").read_bytes(),
        acquisition_plan_bytes=plan,
        expected_index_sha256=result.pool_index_sha256,
        expected_manifest_sha256=result.stage_manifest_sha256,
        stage_uuid=manifest["stage_uuid"],
        source_revision=manifest["source_revision"],
        protocol_sha256=manifest["protocol_sha256"],
        cohorts_sha256=manifest["cohorts_sha256"],
        source_closure_sha256=manifest["source_closure_sha256"],
        acquisition_plan_sha256=manifest["acquisition_plan_sha256"],
        configured_timeout_seconds=30.0,
        configured_query_pacing_seconds=7.0,
        configured_arxiv_pacing_seconds=3.0,
        expected_execution_profile=acquisition.CONTINUE_ACQUISITION_PROFILE.identifier,
        deadline_monotonic=28_800.0,
        verified_pool_source=verified,
    )


@pytest.mark.asyncio
@pytest.mark.parametrize("rate_limited", [False, True])
async def test_actual_collector_resource_accounting_preserves_unknown_live_controls(tmp_path, rate_limited):
    result, calls, pacer = await _collect(tmp_path, rate_limit_openalex=rate_limited)
    manifest, plan = _manifest()
    observed = _observe(result, manifest, plan, _verified(result, manifest, pacer))
    assert observed["acquisition_physical_http_calls"] == len(calls)
    assert sum(observed["acquisition_engine_calls"].values()) == len(calls)
    assert observed["acquisition_engine_calls"]["brave"] == 0
    assert observed["acquisition_response_bytes_total"] > 0
    assert observed["acquisition_retries"] is None
    assert observed["acquisition_pacing_seconds"] is None
    if rate_limited:
        assert observed["acquisition_engine_calls"]["openalex"] == 1
        summary = json.loads((result.collection_directory / "collection-summary.json").read_bytes())
        assert any(row["status"] == "inconclusive" for row in summary["operation_statuses"])
        assert all(row["pool_status"] == "complete" for row in summary["pool_eligibility"])


@pytest.mark.asyncio
async def test_resealed_inventory_cannot_hide_changed_archived_response(tmp_path):
    result, _calls, pacer = await _collect(tmp_path)
    manifest, plan = _manifest()
    verified = _verified(result, manifest, pacer)
    ledger = json.loads((result.collection_directory / "request-ledger-index.json").read_bytes())
    response = result.collection_directory / ledger["entries"][0]["raw_response_file"]
    response.write_bytes(response.read_bytes() + b"tampered")
    with pytest.raises(resources.ResourceEvidenceError):
        _observe(result, manifest, plan, verified)


@pytest.mark.asyncio
async def test_resealed_inventory_cannot_replace_collection_summary_after_verification(tmp_path):
    result, _calls, pacer = await _collect(tmp_path)
    manifest, plan = _manifest()
    verified = _verified(result, manifest, pacer)
    summary_path = result.collection_directory / "collection-summary.json"
    summary = json.loads(summary_path.read_bytes())
    summary["quality_credit"] = True
    summary_path.write_bytes(
        (json.dumps(summary, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode()
    )
    with pytest.raises(resources.ResourceEvidenceError):
        _observe(result, manifest, plan, verified)
