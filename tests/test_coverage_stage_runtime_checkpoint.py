"""Offline runtime wiring tests for pre-grade and closed-grade checkpoints."""

from __future__ import annotations

import json
from dataclasses import replace
from pathlib import Path
from unittest import mock

import pytest

from scripts import coverage_assessment_checkpoint as checkpoint
from scripts import coverage_grade_closure as grades
from scripts import coverage_stage_runtime as runtime
from tests.test_coverage_assessment_packets import _fixture
from tests.test_coverage_grade_closure import _pre_pins, _pre_submissions
from tests.test_coverage_stage_orchestration import plan_fixture
from tests.test_coverage_stage_runtime import _bindings


@pytest.mark.asyncio
async def test_runtime_checkpoints_before_native_collect_and_after_closure(tmp_path: Path) -> None:
    root = tmp_path / "runtime"
    root.mkdir(mode=0o700)
    prepared, _captures = _fixture()
    submissions = _pre_submissions(prepared)
    closure = grades.close_preassessment(prepared, submissions, **_pre_pins(prepared))
    bindings = _bindings(root)

    async def collect(**kwargs):
        assert kwargs["phase"] == "references"
        assert kwargs["prepared_packets"] is prepared
        saved_index = (
            bindings.native_handoff.root
            / "assessment-checkpoints"
            / prepared.packet_stage_uuid
            / "prepared"
            / "prepared-index.json"
        )
        assert saved_index.is_file()
        assert (saved_index.parent / "private-binding.json").read_bytes() == prepared.private_binding_bytes
        return tuple(submissions)

    with mock.patch.object(bindings.native_handoff, "collect", side_effect=collect) as collect_mock:
        plan = replace(plan_fixture(), packet_stage_uuid=prepared.packet_stage_uuid)
        executors = runtime.build_stage_executors(plan, bindings)
        with pytest.raises(runtime.RuntimeWiringError, match="checkpoint-required"):
            await executors.persist_reference_closure(prepared, submissions, closure)

        result = await executors.grade_references(prepared)
        assert result == tuple(submissions)
        collect_mock.assert_awaited_once()

        closure_sha = await executors.persist_reference_closure(prepared, submissions, closure)

    closed_dir = bindings.native_handoff.root / "assessment-checkpoints" / prepared.packet_stage_uuid / "closed"
    closure_index = json.loads((closed_dir / "closure-index.json").read_bytes())
    assert closure_index["schema"] == checkpoint.CLOSED_SCHEMA
    assert closure_index["grade_closure_receipt_sha256"] == closure.receipt_sha256
    assert len(list((closed_dir / "submissions").glob("*.response.json"))) == len(submissions)
    assert len(closure_sha) == 64


@pytest.mark.asyncio
async def test_checkpoint_failure_prevents_native_reference_dispatch(tmp_path: Path) -> None:
    root = tmp_path / "runtime"
    root.mkdir(mode=0o700)
    prepared, _captures = _fixture()
    malformed = type(prepared)(
        **{
            **prepared.__dict__,
            "private_binding_bytes": prepared.private_binding_bytes + b" ",
        }
    )
    bindings = _bindings(root)
    with mock.patch.object(bindings.native_handoff, "collect", new_callable=mock.AsyncMock) as collect_mock:
        plan = replace(plan_fixture(), packet_stage_uuid=prepared.packet_stage_uuid)
        executors = runtime.build_stage_executors(plan, bindings)
        with pytest.raises(runtime.RuntimeWiringError, match="prepared-assessment-checkpoint-failed"):
            await executors.grade_references(malformed)
        collect_mock.assert_not_awaited()


@pytest.mark.asyncio
async def test_packet_stage_mismatch_is_rejected_before_checkpoint_or_dispatch(tmp_path: Path) -> None:
    root = tmp_path / "runtime"
    root.mkdir(mode=0o700)
    prepared, _captures = _fixture()
    bindings = _bindings(root)
    with mock.patch.object(bindings.native_handoff, "collect", new_callable=mock.AsyncMock) as collect_mock:
        executors = runtime.build_stage_executors(plan_fixture(), bindings)
        with pytest.raises(runtime.RuntimeWiringError, match="stage-binding-invalid"):
            await executors.grade_references(prepared)
        collect_mock.assert_not_awaited()
    assert not (bindings.native_handoff.root / "assessment-checkpoints" / prepared.packet_stage_uuid).exists()
