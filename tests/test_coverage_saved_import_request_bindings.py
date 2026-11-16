"""Saved-import pins remain on downstream requests from the real runner builder."""

from __future__ import annotations

import asyncio
import hashlib
from dataclasses import replace
from types import SimpleNamespace

import pytest

from scripts import coverage_operator_runner as runner_module
from scripts.coverage_operator_runner import OperatorRunnerError
from tests.test_coverage_stage_orchestration import plan_fixture


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    return runner_module.core._canonical(value)


class _Exchange:
    def __init__(self) -> None:
        self.calls: list[dict[str, object]] = []

    async def request_async(self, **kwargs):
        self.calls.append(kwargs)
        return b"fixture-receipt", "f" * 64


def _runner(*, saved: bool = True):
    instance = object.__new__(runner_module.OperatorStageRunner)
    base = plan_fixture()
    origin_stage_uuid = "00000000-0000-4000-8000-000000000701"
    origin_package_sha256 = "a" * 64
    origin = SimpleNamespace(
        registration=SimpleNamespace(stage_uuid=origin_stage_uuid),
        expected_package_sha256=origin_package_sha256,
    )
    instance.plan = replace(
        base,
        pool_input_mode="saved-corpus/1" if saved else "frozen-pool/1",
        saved_origin=origin if saved else None,
    )
    instance.active_environment_receipt_bytes = _canonical(
        {"schema": "coverage-active-python-environment/1", "python": "offline-fixture"}
    )
    instance.initial_registration = _canonical({"stage_uuid": base.stage_uuid, "pins": {}})
    instance._active_registration_bytes = instance.initial_registration
    instance._active_input_manifest_bytes = None
    instance._frozen_pool_reference_bytes = None
    instance._saved_corpus_reference_bytes = (
        _canonical(
            {
                "schema": "coverage-saved-corpus-import-reference/1",
                "stage_uuid": base.stage_uuid,
                "origin_stage_uuid": origin_stage_uuid,
                "origin_package_sha256": origin_package_sha256,
                "import_receipt_sha256": "b" * 64,
                "capture_manifest_sha256": "c" * 64,
                "capture_inventory_sha256": "d" * 64,
                "source_rows": 10,
                "captured_contexts": 6,
                "failed_contexts": 4,
                "current_search_calls": 0,
                "current_capture_calls": 0,
            }
        )
        if saved
        else None
    )
    instance.plan = replace(
        instance.plan,
        saved_origin=origin if saved else None,
    )
    instance._frozen_stage_clock_bytes = None
    instance._pipeline_inputs = None
    instance.static_materials = {"qualified_source_closure": b"fixture-closure"}
    instance.handoff = _Exchange()
    return instance


@pytest.mark.parametrize(
    ("scope", "request_id"),
    [
        ("selector-map-admission", "map"),
        ("selector-operation-permits", "batch"),
        ("answer-execution", "permit"),
    ],
)
def test_saved_import_bindings_and_receipt_artifact_are_forwarded(scope, request_id):
    instance = _runner()
    asyncio.run(instance._request(scope, request_id, {}, artifacts={"scope-input": b"fixture"}))
    call = instance.handoff.calls[0]
    assert call["scope"] == scope
    bindings = call["bindings"]
    assert bindings["saved_corpus_import_receipt_sha256"] == "b" * 64
    assert bindings["origin_package_sha256"] == "a" * 64
    artifacts = call["artifacts"]
    assert artifacts["saved-corpus-import-reference"] == instance._saved_corpus_reference_bytes
    assert artifacts["saved-corpus-import-receipt-sha256"] == ("b" * 64).encode("ascii")


def test_saved_import_binding_and_artifact_collisions_refuse_request():
    instance = _runner()
    with pytest.raises(OperatorRunnerError, match="phase-request-context-binding-collision"):
        asyncio.run(
            instance._request(
                "answer-execution",
                "permit",
                {"saved_corpus_import_receipt_sha256": "e" * 64},
                artifacts={},
            )
        )
    with pytest.raises(OperatorRunnerError, match="phase-artifact-dynamic-material-collision"):
        instance._phase_request_context({"saved-corpus-import-receipt-sha256": b"tampered"})


def test_preimport_saved_request_does_not_claim_a_receipt():
    instance = _runner()
    instance._saved_corpus_reference_bytes = None
    bindings, artifacts = instance._phase_request_context({})
    assert "saved_corpus_import_receipt_sha256" not in bindings
    assert "origin_package_sha256" not in bindings
    assert "saved-corpus-import-reference" not in artifacts
    assert "saved-corpus-import-receipt-sha256" not in artifacts


def test_saved_reference_is_forwarded_even_without_scope_specific_artifacts():
    instance = _runner()
    bindings, artifacts = instance._phase_request_context(None)
    assert bindings["saved_corpus_import_receipt_sha256"] == "b" * 64
    assert bindings["origin_package_sha256"] == "a" * 64
    assert artifacts["saved-corpus-import-reference"] == instance._saved_corpus_reference_bytes
    assert artifacts["saved-corpus-import-receipt-sha256"] == ("b" * 64).encode("ascii")


def test_frozen_pool_requests_do_not_gain_saved_import_fields():
    instance = _runner(saved=False)
    asyncio.run(instance._request("answer-execution", "permit", {}, artifacts={"answer-task": b"fixture"}))
    call = instance.handoff.calls[0]
    assert "saved_corpus_import_receipt_sha256" not in call["bindings"]
    assert "origin_package_sha256" not in call["bindings"]
    assert "saved-corpus-import-reference" not in call["artifacts"]
    assert "saved-corpus-import-receipt-sha256" not in call["artifacts"]
