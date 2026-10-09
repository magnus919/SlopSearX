"""Synthetic checks for actual selector execution resource observations."""

from __future__ import annotations

import asyncio
import json
from pathlib import Path
from types import SimpleNamespace

import httpx
import pytest

from scripts import coverage_jev_execution as execution
from scripts import coverage_study_core as core
from tests.test_coverage_jev_execution import (
    PrivateRoots,
    candidate_input,
    compile_small,
    permit_for,
    prepared_stage,
    valid_response,
)


def _call_kwargs(prepared, ledger, roots, operation_id, compiled, transport):
    inputs = candidate_input()
    request = compiled.body
    parser_mode = "coverage" if "candidate" in operation_id else "original-v1"
    permit, permit_sha = permit_for(prepared, operation_id, inputs, request, parser_mode=parser_mode)
    return {
        "prepared": prepared,
        "ledger": ledger,
        "operation_id": operation_id,
        "operation_input_bytes": inputs,
        "permit_bytes": permit,
        "expected_permit_sha256": permit_sha,
        "api_key": "Synthetic-Key-Resource-Test",
        "lease_root": roots.lease,
        "archive_root": roots.archive,
        "result_root": roots.result,
        "transport": transport,
        # This resource-only fixture stubs the request compiler. Supply the
        # original slot's synthetic strategy context without claiming native
        # W0 parser or request parity (those have separate contract tests).
        "legacy_control": (
            {"service": SimpleNamespace(_ctx=SimpleNamespace(ranking_strategy="presence"))}
            if parser_mode == "original-v1"
            else None
        ),
    }


def _monkeypatch_candidate_builder(monkeypatch, compiled):
    monkeypatch.setattr(
        execution,
        "build_selector_request",
        lambda **_kwargs: (compiled.body, compiled),
    )


@pytest.mark.asyncio
async def test_terminal_inventory_binds_actual_dispatch_usage_bytes_and_serial_order(monkeypatch) -> None:
    prepared = prepared_stage()
    ledger = core.StudyRun(prepared)
    roots = PrivateRoots()
    compiled = compile_small()
    _monkeypatch_candidate_builder(monkeypatch, compiled)
    response = valid_response(compiled, input_tokens=17, output_tokens=6)
    dispatched = []

    async def handler(request: httpx.Request) -> httpx.Response:
        dispatched.append(request)
        assert request.content == compiled.body
        assert request.headers["authorization"] == "Bearer Synthetic-Key-Resource-Test"
        await asyncio.sleep(0)
        return httpx.Response(200, content=response, request=request)

    async def run(operation_id):
        return await execution.execute_selector_call(
            **_call_kwargs(prepared, ledger, roots, operation_id, compiled, httpx.MockTransport(handler))
        )

    # Concurrent callers are serialized by the per-ledger lock.
    results = list(await asyncio.gather(*(run(operation_id) for operation_id in prepared.operation_ids[:2])))
    results.extend([await run(operation_id) for operation_id in prepared.operation_ids[2:]])
    for index, result in enumerate(results, start=1):
        assert result.dispatch_count == 1
        assert result.request_bytes == len(compiled.body)
        assert result.input_tokens_observed == 17
        assert result.output_tokens_observed == 6
        assert result.observed_elapsed_ms is not None and result.observed_elapsed_ms >= 0
        assert result.serialized_operation_sequence == index
        assert result.max_concurrent_operations_observed == 1

    terminal = execution.close_stage(prepared=prepared, ledger=ledger, result_root=roots.result)
    inventory = json.loads((roots.result / f"{prepared.stage_uuid}.terminal-inventory.json").read_bytes())
    observations = inventory["resource_observations"]
    assert terminal["status"] == "all-registered-calls-complete"
    assert len(dispatched) == len(prepared.operation_ids)
    assert [row["operation_id"] for row in observations] == list(prepared.operation_ids)
    assert all(row["status"] == "observed" for row in observations)
    assert [row["provider_dispatch_count"] for row in observations] == [1] * len(results)
    assert [row["request_bytes"] for row in observations] == [len(compiled.body)] * len(results)
    assert [row["response_bytes"] for row in observations] == [len(response)] * len(results)
    assert [row["input_tokens_observed"] for row in observations] == [17] * len(results)
    assert [row["output_tokens_observed"] for row in observations] == [6] * len(results)
    assert [row["serialized_operation_sequence"] for row in observations] == list(range(1, len(results) + 1))
    assert all(row["max_concurrent_operations_observed"] == 1 for row in observations)
    assert all(row["serialization_scope"] == "same-study-ledger-lock" for row in observations)
    roots.close()


@pytest.mark.asyncio
@pytest.mark.parametrize(("result_delay", "expected_ms"), [(1.1, 1_300), (1.0004, 1_201)])
async def test_elapsed_observation_includes_slow_result_and_correction_fsync(
    monkeypatch, result_delay, expected_ms
) -> None:
    prepared = prepared_stage()
    ledger = core.StudyRun(prepared)
    roots = PrivateRoots()
    compiled = compile_small()
    _monkeypatch_candidate_builder(monkeypatch, compiled)
    response = valid_response(compiled)
    fake_now = [10.0]
    monkeypatch.setattr(execution, "time", SimpleNamespace(monotonic=lambda: fake_now[0]))
    original_write = execution._write_exclusive

    def slow_fsync(root: Path, name: str, body: bytes) -> str:
        digest = original_write(root, name, body)
        if name.endswith(".result.json"):
            fake_now[0] += result_delay
        elif name.endswith(".correction.json"):
            fake_now[0] += 0.2
        return digest

    monkeypatch.setattr(execution, "_write_exclusive", slow_fsync)

    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(200, content=response, request=request)

    operation_id = prepared.operation_ids[0]
    result = await execution.execute_selector_call(
        **_call_kwargs(prepared, ledger, roots, operation_id, compiled, httpx.MockTransport(handler))
    )

    assert result.status == "phase-deadline-exceeded"
    assert result.observed_elapsed_ms == expected_ms
    assert result.dispatch_count == 1
    assert (roots.result / f"{prepared.stage_uuid}.{operation_id}.correction.json").is_file()
    provisional = json.loads((roots.result / f"{prepared.stage_uuid}.{operation_id}.result.json").read_bytes())
    assert provisional["phase_elapsed_ms_before_receipt"] == 0
    assert provisional["provider_dispatch_count"] == 1

    execution.close_stage(prepared=prepared, ledger=ledger, result_root=roots.result)
    inventory = json.loads((roots.result / f"{prepared.stage_uuid}.terminal-inventory.json").read_bytes())
    observations = inventory["resource_observations"]
    assert observations[0]["elapsed_ms_through_final_receipt_fsync"] == expected_ms
    assert observations[0]["provider_dispatch_count"] == 1
    assert observations[0]["input_tokens_observed"] == 10
    assert observations[0]["output_tokens_observed"] == 5
    assert all(row["status"] == "not-observed" for row in observations[1:])
    roots.close()
