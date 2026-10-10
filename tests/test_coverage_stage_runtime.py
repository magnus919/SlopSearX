from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest import mock

from scripts import coverage_answer_execution as answer_execution
from scripts import coverage_stage_orchestration as orchestration
from scripts import coverage_stage_runtime as runtime
from scripts.coverage_native_grader_handoff import NativeGraderHandoff
from tests.test_coverage_answer_execution import _tasks
from tests.test_coverage_stage_orchestration import plan_fixture


def _bindings(root: Path, *, acquisition_authority=None, capture_authority=None, answer_authority=None):
    async def verify_acquisition(_plan, _evidence):
        return True

    async def prepare_after_acquisition(_plan, _acquisition, _capture):
        raise AssertionError("unused in this test")

    async def verify_late_preflight(_plan, _evidence):
        return True

    async def selector_permit_resolver(**_kwargs):
        return {}

    async def selector_admission_verifier(**_kwargs):
        raise AssertionError("unused in this test")

    def selector_evidence_builder(*_args, **_kwargs):
        raise AssertionError("unused in this test")

    return runtime.RuntimeBindings(
        acquisition_authority=acquisition_authority or (lambda _plan: {}),
        verify_acquisition=verify_acquisition,
        capture_authority=capture_authority or (lambda _plan, _inputs: {}),
        prepare_after_acquisition=prepare_after_acquisition,
        verify_late_preflight=verify_late_preflight,
        selector_permit_resolver=selector_permit_resolver,
        selector_admission_verifier=selector_admission_verifier,
        selector_evidence_builder=selector_evidence_builder,
        answer_authority=answer_authority or (lambda _plan, _tasks: {}),
        selector_api_key="synthetic-test-key",
        selector_lease_root=root / "selector-leases",
        selector_archive_root=root / "selector-archives",
        selector_result_root=root / "selector-results",
        native_handoff=NativeGraderHandoff(root / "grader-handoff"),
    )


class StageRuntimeWiringTests(unittest.IsolatedAsyncioTestCase):
    async def test_operator_receipt_authority_callbacks_may_wait_asynchronously(self):
        with tempfile.TemporaryDirectory() as temporary:
            plan = plan_fixture()
            calls = []

            async def acquisition_authority(_plan):
                calls.append("acquisition")
                return {"one_shot_lease": object()}

            async def capture_authority(_plan, _inputs):
                calls.append("capture")
                return {"transport": object()}

            async def answer_authority(_plan, _tasks):
                calls.append("answer")
                return {"resolver": object()}

            executors = runtime.build_stage_executors(
                plan,
                _bindings(
                    Path(temporary),
                    acquisition_authority=acquisition_authority,
                    capture_authority=capture_authority,
                    answer_authority=answer_authority,
                ),
            )
            with (
                mock.patch.object(runtime.coverage_live_acquire, "acquire_live_coverage_stage") as acquire,
                mock.patch.object(runtime.coverage_source_capture, "capture_sources_once") as capture,
                mock.patch.object(answer_execution, "execute_answer_stage") as answer,
            ):
                with self.assertRaisesRegex(runtime.RuntimeWiringError, "acquisition-authority-shape-invalid"):
                    await executors.acquire(plan)
                with self.assertRaisesRegex(runtime.RuntimeWiringError, "capture-authority-shape-invalid"):
                    await executors.capture(plan, {})
                with self.assertRaisesRegex(runtime.RuntimeWiringError, "answer-authority-shape-invalid"):
                    await executors.answerer(plan, _tasks())
            self.assertEqual(calls, ["acquisition", "capture", "answer"])
            acquire.assert_not_called()
            capture.assert_not_called()
            answer.assert_not_called()

    async def test_factory_wires_ordered_executors_without_minting_authority(self):
        with tempfile.TemporaryDirectory() as temporary:
            plan = plan_fixture(selector_map_registered=True)
            executors = runtime.build_stage_executors(plan, _bindings(Path(temporary)))

            self.assertIsInstance(executors, orchestration.StageExecutors)
            self.assertEqual(executors.acquire.__name__, "acquire")
            self.assertEqual(executors.capture.__name__, "capture")
            self.assertEqual(executors.answerer.__name__, "answerer")
            self.assertEqual(executors.grade_references.__name__, "grade_references")
            self.assertEqual(executors.grade_answers.__name__, "grade_answers")
            self.assertIsNotNone(executors.verify_selector_admission)

    async def test_incomplete_acquisition_authority_fails_before_transport(self):
        with tempfile.TemporaryDirectory() as temporary:
            plan = plan_fixture()
            executors = runtime.build_stage_executors(
                plan,
                _bindings(Path(temporary), acquisition_authority=lambda _plan: {"one_shot_lease": object()}),
            )
            with mock.patch.object(runtime.coverage_live_acquire, "acquire_live_coverage_stage") as acquire:
                with self.assertRaisesRegex(runtime.RuntimeWiringError, "acquisition-authority-shape-invalid"):
                    await executors.acquire(plan)
                acquire.assert_not_called()

    async def test_runtime_rejects_injected_transport_and_resolver_overrides(self):
        with tempfile.TemporaryDirectory() as temporary:
            plan = plan_fixture()
            root = Path(temporary)
            acq_fields = {
                "permit_receipt_bytes": b"",
                "expected_permit_receipt_sha256": "0" * 64,
                "permit_verifier": object(),
                "one_shot_lease": object(),
                "receipt_directory": root / "acquisition",
                "test_transport": object(),
            }
            capture_fields = {
                "candidate_base_url": "https://candidate.invalid",
                "operator_token": None,
                "permit_receipt_bytes": b"",
                "expected_permit_receipt_sha256": "0" * 64,
                "permit_verifier": object(),
                "one_shot_lease": object(),
                "receipt_root": root / "capture",
                "transport": object(),
            }
            tasks = _tasks()
            answer_fields = {
                "endpoint": "https://model.invalid/v1/chat/completions",
                "api_key": "synthetic-only",
                "permit_bytes": b"",
                "expected_permit_sha256": "0" * 64,
                "permit_verifier": object(),
                "one_shot_lease": object(),
                "lease_root": root / "answer-lease",
                "archive_root": root / "answer-archive",
                "result_root": root / "answer-result",
                "stage_started_utc": "2026-01-01T00:00:00Z",
                "stage_deadline_utc": "2026-01-01T01:00:00Z",
                "answer_manifest_sha256": "0" * 64,
                "resolver": object(),
            }
            bindings = _bindings(
                root,
                acquisition_authority=lambda _plan: acq_fields,
                capture_authority=lambda _plan, _inputs: capture_fields,
                answer_authority=lambda _plan, _tasks: answer_fields,
            )
            executors = runtime.build_stage_executors(plan, bindings)
            with (
                mock.patch.object(runtime.coverage_live_acquire, "acquire_live_coverage_stage") as acquire,
                mock.patch.object(runtime.coverage_source_capture, "capture_sources_once") as capture,
                mock.patch.object(answer_execution, "execute_answer_stage") as answer,
            ):
                with self.assertRaisesRegex(runtime.RuntimeWiringError, "acquisition-authority-shape-invalid"):
                    await executors.acquire(plan)
                acquire.assert_not_called()

                with self.assertRaisesRegex(runtime.RuntimeWiringError, "capture-authority-shape-invalid"):
                    await executors.capture(plan, {})
                capture.assert_not_called()

                with self.assertRaisesRegex(runtime.RuntimeWiringError, "answer-authority-shape-invalid"):
                    await executors.answerer(plan, tasks)
                answer.assert_not_called()

    async def test_reference_grader_handoff_uses_packet_stage_identity(self):
        with tempfile.TemporaryDirectory() as temporary:
            plan = plan_fixture()
            bindings = _bindings(Path(temporary))
            expected = (object(),)
            with mock.patch.object(bindings.native_handoff, "collect", return_value=expected) as collect:
                executors = runtime.build_stage_executors(plan, bindings)
                prepared_packets = object()
                result = await executors.grade_references(prepared_packets)

            self.assertIs(result, expected)
            self.assertEqual(collect.call_args.kwargs["stage_uuid"], plan.packet_stage_uuid)
            self.assertEqual(collect.call_args.kwargs["phase"], "references")
            self.assertIs(collect.call_args.kwargs["prepared_packets"], prepared_packets)

    async def test_answer_manifest_must_match_external_authority_before_dispatch(self):
        with tempfile.TemporaryDirectory() as temporary:
            plan = plan_fixture()
            tasks = _tasks()
            authority = {
                "endpoint": "https://model.invalid/v1/chat/completions",
                "api_key": "synthetic-only",
                "permit_bytes": b"unused",
                "expected_permit_sha256": "0" * 64,
                "permit_verifier": object(),
                "one_shot_lease": object(),
                "lease_root": Path(temporary) / "lease",
                "archive_root": Path(temporary) / "archive",
                "result_root": Path(temporary) / "result",
                "stage_started_utc": "2026-01-01T00:00:00Z",
                "stage_deadline_utc": "2026-01-01T01:00:00Z",
                "answer_manifest_sha256": "0" * 64,
            }
            executors = runtime.build_stage_executors(
                plan, _bindings(Path(temporary), answer_authority=lambda _plan, _tasks: authority)
            )
            with mock.patch.object(answer_execution, "execute_answer_stage") as dispatch:
                with self.assertRaisesRegex(runtime.RuntimeWiringError, "answer-operation-manifest-authority-mismatch"):
                    await executors.answerer(plan, tasks)
                dispatch.assert_not_called()


if __name__ == "__main__":
    unittest.main()
