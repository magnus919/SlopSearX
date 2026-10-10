"""Mock-only tests for map-bound serial selector dispatch."""

from __future__ import annotations

import hashlib
from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace

import httpx
import pytest

from scripts import coverage_jev_execution as execution
from scripts import coverage_selector_input_map as input_map
from scripts import coverage_stage_orchestration as orchestration
from scripts import coverage_study_core as core
from tests.test_coverage_jev_execution import PrivateRoots, permit_for, valid_response
from tests.test_coverage_legacy_control import _response as legacy_response
from tests.test_coverage_selector_input_map import _materials


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _schedule(tmp_path: Path):
    kwargs = _materials(tmp_path)
    prepared = replace(kwargs["prepared"], selector_input_map_required=True)
    kwargs["prepared"] = prepared
    operation_materials = {}
    kwargs["operation_materials"] = operation_materials
    map_bytes = input_map.build_selector_input_map(**kwargs)
    map_sha = _sha(map_bytes)
    execution._bind_selector_input_map(prepared, map_bytes, map_sha)
    ledger = core.StudyRun(prepared)
    permits = {}
    responses = {}
    for operation_id in prepared.operation_ids:
        material = operation_materials[operation_id]
        input_bytes = material["operation_input_bytes"]
        legacy_control = material["legacy_control"]
        request_body, compiled = execution.build_selector_request(
            operation_id=operation_id,
            operation_input_bytes=input_bytes,
            legacy_control=legacy_control,
        )
        permits[operation_id] = permit_for(
            prepared,
            operation_id,
            input_bytes,
            request_body,
            parser_mode="coverage" if compiled is not None else "original-v1",
        )
        if compiled is not None:
            responses[operation_id] = valid_response(compiled)
        else:
            pool = legacy_control["canonical_pool"]
            responses[operation_id] = legacy_response({f"c{index}": 7 for index in range(len(pool))})
    return prepared, ledger, operation_materials, map_bytes, map_sha, permits, responses


@pytest.mark.asyncio
async def test_all_registered_operations_are_preflighted_then_serially_dispatched(tmp_path):
    prepared, ledger, materials, map_bytes, map_sha, permits, responses = _schedule(tmp_path)
    roots = PrivateRoots()
    dispatched = []

    def transport_factory(operation_id):
        async def handler(request):
            dispatched.append(operation_id)
            return httpx.Response(200, content=responses[operation_id], request=request)

        return httpx.MockTransport(handler)

    results = await execution.execute_registered_selector_schedule(
        prepared=prepared,
        ledger=ledger,
        selector_input_map_bytes=map_bytes,
        expected_selector_input_map_sha256=map_sha,
        operation_materials=materials,
        permits=permits,
        api_key="synthetic-map-test-key",
        lease_root=roots.lease,
        archive_root=roots.archive,
        result_root=roots.result,
        transport_factory=transport_factory,
    )
    closed = ledger.close()
    assert len(results) == len(prepared.operation_ids) == 39
    assert dispatched == list(prepared.operation_ids)
    assert closed.all_calls_complete is True
    assert sum(result.dispatch_count or 0 for result in results) == 39
    roots.close()


@pytest.mark.asyncio
async def test_late_bad_permit_fails_entire_schedule_before_any_transport_or_claim(tmp_path):
    prepared, ledger, materials, map_bytes, map_sha, permits, _responses = _schedule(tmp_path)
    roots = PrivateRoots()
    dispatched = []
    final_id = prepared.operation_ids[-1]
    permits[final_id] = (permits[final_id][0], "0" * 64)

    def transport_factory(operation_id):
        dispatched.append(operation_id)
        return httpx.MockTransport(lambda request: httpx.Response(200, content=b"{}", request=request))

    with pytest.raises(execution.JevExecutionError, match="external-permit-pin-mismatch"):
        await execution.execute_registered_selector_schedule(
            prepared=prepared,
            ledger=ledger,
            selector_input_map_bytes=map_bytes,
            expected_selector_input_map_sha256=map_sha,
            operation_materials=materials,
            permits=permits,
            api_key="synthetic-map-test-key",
            lease_root=roots.lease,
            archive_root=roots.archive,
            result_root=roots.result,
            transport_factory=transport_factory,
        )
    assert dispatched == []
    assert list(roots.lease.iterdir()) == []
    assert list(roots.archive.iterdir()) == []
    assert ledger.rows == []
    roots.close()


@pytest.mark.asyncio
async def test_first_terminal_call_stops_schedule_and_leaves_later_slots_uninvoked(tmp_path):
    prepared, ledger, materials, map_bytes, map_sha, permits, responses = _schedule(tmp_path)
    roots = PrivateRoots()
    responses[prepared.operation_ids[0]] = b"{}"
    dispatched = []

    def transport_factory(operation_id):
        async def handler(request):
            dispatched.append(operation_id)
            return httpx.Response(200, content=responses[operation_id], request=request)

        return httpx.MockTransport(handler)

    results = await execution.execute_registered_selector_schedule(
        prepared=prepared,
        ledger=ledger,
        selector_input_map_bytes=map_bytes,
        expected_selector_input_map_sha256=map_sha,
        operation_materials=materials,
        permits=permits,
        api_key="synthetic-map-test-key",
        lease_root=roots.lease,
        archive_root=roots.archive,
        result_root=roots.result,
        transport_factory=transport_factory,
    )
    closure = ledger.close()
    assert len(results) == 1
    assert dispatched == [prepared.operation_ids[0]]
    assert closure.all_calls_complete is False
    assert len(closure.rows) == len(prepared.operation_ids)
    assert all(row.state == "not-invoked-after-terminal-stop" for row in closure.rows[1:])
    roots.close()


@pytest.mark.asyncio
async def test_factory_wires_guarded_schedule_into_stage_selector_callback(tmp_path):
    prepared, _ledger, materials, map_bytes, map_sha, permits, responses = _schedule(tmp_path)
    roots = PrivateRoots()
    dispatched = []
    reference = SimpleNamespace(receipt_sha256="f" * 64)

    def transport_factory(operation_id):
        async def handler(request):
            dispatched.append(operation_id)
            return httpx.Response(200, content=responses[operation_id], request=request)

        return httpx.MockTransport(handler)

    def evidence_builder(_plan, stage_summary, _inputs, ref_closure, results, terminal):
        assert len(results) == len(stage_summary.operation_ids)
        terminal_bytes = (roots.result / f"{stage_summary.stage_uuid}.terminal-inventory.json").read_bytes()
        return orchestration.SelectorEvidence(
            stage_uuid=stage_summary.stage_uuid,
            terminal_inventory_sha256=terminal["sha256"],
            operation_rows=tuple(
                {"operation_id": operation_id, "state": "complete-success"}
                for operation_id in stage_summary.operation_ids
            ),
            research_orders={},
            navigation_rank_one={},
            reference_grade_receipt_sha256=ref_closure.receipt_sha256,
            usage_status="known",
            terminal_inventory_bytes=terminal_bytes,
            result_root=roots.result,
            archive_root=roots.archive,
        )

    selector_callback = orchestration.registered_selector_executor_factory(
        permits=permits,
        api_key="synthetic-map-test-key",
        lease_root=roots.lease,
        archive_root=roots.archive,
        result_root=roots.result,
        evidence_builder=evidence_builder,
        transport_factory=transport_factory,
    )
    result = await selector_callback(
        SimpleNamespace(),
        prepared,
        {
            "selector_input_map_bytes": map_bytes,
            "selector_input_map_sha256": map_sha,
            "selector_operation_materials": materials,
        },
        reference,
    )
    assert isinstance(selector_callback, orchestration.RegisteredSelectorStageExecutor)
    assert result.terminal_inventory_sha256
    assert dispatched == list(prepared.operation_ids)
    roots.close()
