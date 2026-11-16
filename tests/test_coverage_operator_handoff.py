from __future__ import annotations

import asyncio
import base64
import hashlib
import json
import os
import stat
import tempfile
import threading
import time
import unittest
import uuid
from datetime import datetime, timedelta, timezone
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

from scripts import coverage_jev_execution as jev_execution
from scripts import coverage_local_authority as authority
from scripts import coverage_operator_handoff as handoff
from scripts import coverage_selector_input_map
from tests.test_coverage_selector_input_map import _materials


def test_confirmation_receipt_context_requires_plan_from_registered_stage_kind() -> None:
    from types import SimpleNamespace

    registration = {"stage_kind": "confirmation"}
    with unittest.TestCase().assertRaisesRegex(
        authority.LocalAuthorityError, "phase-artifact-confirmation-plan-required"
    ):
        authority._verified_development_receipt_context(None, registration, {}, {})

    development = {"stage_kind": "development"}
    assert authority._verified_development_receipt_context(None, development, {}, {}) == {}

    current_stage = str(uuid.UUID(int=731))
    development_stage = str(uuid.UUID(int=730))
    protocol = canonical({"protocol": "fixed"})
    closure = "a" * 64
    receipt = canonical(
        {
            "schema": "coverage-study-development-gate-receipt/1",
            "stage_uuid": development_stage,
            "stage_kind": "development",
            "status": "passed",
            "protocol_sha256": digest(protocol),
            "source_revision": "1" * 40,
            "source_closure_sha256": closure,
            "result_sha256": "2" * 64,
        }
    )
    plan = SimpleNamespace(
        stage_kind="confirmation",
        stage_uuid=current_stage,
        development_receipt_bytes=receipt,
        development_receipt_sha256=digest(receipt),
        development_stage_uuid=development_stage,
        protocol_bytes=protocol,
        source_revision="1" * 40,
        source_closure_sha256=closure,
    )
    bindings = {"development_receipt_sha256": digest(receipt), "development_stage_uuid": development_stage}
    artifacts = {"development-pass-receipt": receipt}
    assert authority._verified_development_receipt_context(plan, registration, bindings, artifacts) == bindings
    with unittest.TestCase().assertRaisesRegex(authority.LocalAuthorityError, "development-receipt-mismatch"):
        authority._verified_development_receipt_context(
            plan,
            registration,
            {**bindings, "development_receipt_sha256": "0" * 64},
            artifacts,
        )
    with unittest.TestCase().assertRaisesRegex(authority.LocalAuthorityError, "development-receipt-mismatch"):
        authority._verified_development_receipt_context(plan, registration, bindings, {})


def canonical(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def digest(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def registered_context(
    stage: str,
    source_revision: str,
    protocol: bytes,
    closure_sha256: str,
    extra_pins: dict[str, str] | None = None,
    stage_kind: str = "development",
) -> bytes:
    from scripts import coverage_study_core as core

    pins = {name: digest(name.encode()) for name in core.PIN_NAMES}
    pins["protocol"] = digest(protocol)
    pins["qualified_source_closure"] = closure_sha256
    pins.update(extra_pins or {})
    return canonical(
        {
            "schema": core.REGISTRATION_SCHEMA,
            "stage_uuid": stage,
            "stage_kind": stage_kind,
            "source_revision": source_revision,
            "pins": pins,
            "operation_ids": [],
            "navigation_skip_reasons": [],
            "task_ids": [],
        }
    )


def selector_permit_batch(tmp_path: Path) -> bytes:
    """Build a full-shaped batch from the registered operation schedule fixture."""
    tmp_path.mkdir(mode=0o700)
    operation_materials: dict[str, object] = {}
    inputs = _materials(tmp_path)
    inputs["operation_materials"] = operation_materials
    map_bytes = coverage_selector_input_map.build_selector_input_map(**inputs)
    map_doc = json.loads(map_bytes)
    prepared = inputs["prepared"]
    rows = []
    for operation in map_doc["operations"]:
        permit = jev_execution._canonical(
            {
                "schema": jev_execution.PERMIT_SCHEMA,
                "status": "verified-admitted",
                "stage_uuid": prepared.stage_uuid,
                "operation_id": operation["operation_id"],
                "parser_mode": operation["parser_mode"],
                "source_revision": prepared.source_revision,
                "protocol_sha256": prepared.pins["protocol"],
                "source_closure_sha256": prepared.pins["qualified_source_closure"],
                "operation_input_sha256": operation["operation_input_sha256"],
                "request_body_sha256": operation["request_body_sha256"],
            }
        )
        rows.append(
            {
                "operation_id": operation["operation_id"],
                "permit_base64": base64.b64encode(permit).decode("ascii"),
                "permit_sha256": digest(permit),
            }
        )
        if len(permit) > handoff.MAX_RECEIPT_BYTES:
            raise AssertionError("fixture must stay inside the individual-permit cap")
    return canonical(
        {
            "schema": "coverage-selector-permit-batch/1",
            "stage_uuid": prepared.stage_uuid,
            "selector_input_map_sha256": hashlib.sha256(map_bytes).hexdigest(),
            "operations": rows,
        }
    )


class OperatorReceiptHandoffTests(unittest.TestCase):
    def test_file_handoff_issuer_recomputes_source_capture_from_artifact(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "handoff"
            exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
            stage = str(uuid.UUID(int=8125))
            cohorts_bytes = canonical({"cohorts": "test-only"})
            protocol_bytes = canonical({"protocol": "test-only", "cohorts_sha256": digest(cohorts_bytes)})
            closure_bytes = canonical({"source_revision": "b" * 40})
            closure = digest(closure_bytes)
            manifest = {
                "stage_uuid": stage,
                "protocol_sha256": digest(protocol_bytes),
                "source_revision": "b" * 40,
                "cohorts_sha256": digest(cohorts_bytes),
                "candidate_identity_sha256": "d" * 64,
                "candidate_endpoint_sha256": "e" * 64,
                "sources": [{"source_id": "s1"}],
            }
            manifest_bytes = canonical(manifest)
            registration = registered_context(
                stage,
                manifest["source_revision"],
                protocol_bytes,
                closure,
                {"source_capture_manifest": digest(manifest_bytes)},
            )
            bindings = {
                "stage_uuid": stage,
                "manifest_sha256": digest(manifest_bytes),
                "protocol_sha256": manifest["protocol_sha256"],
                "source_revision": manifest["source_revision"],
                "cohorts_sha256": manifest["cohorts_sha256"],
                "candidate_identity_sha256": manifest["candidate_identity_sha256"],
                "candidate_endpoint_sha256": manifest["candidate_endpoint_sha256"],
                "source_count": 1,
                "max_owned_calls": authority.capture.MAX_OWNED_CALLS,
                "max_health_calls": authority.capture.MAX_HEALTH_CALLS,
                "max_scrape_calls": authority.capture.MAX_SCRAPE_CALLS,
                "timeout_seconds": authority.capture.REQUEST_TIMEOUT_SECONDS,
                "response_bytes": authority.capture.MAX_RESPONSE_BYTES,
                "source_context_characters": authority.capture.MAX_SOURCE_CHARS,
                "ca_bundle_sha256": None,
                "source_closure_sha256": closure,
            }
            result: list[object] = []

            def run_request():
                try:
                    result.append(
                        exchange.request(
                            stage_uuid=stage,
                            scope="source-capture",
                            request_id="permit",
                            bindings=bindings,
                            deadline_monotonic=time.monotonic() + 2,
                            artifacts={
                                "capture-manifest": manifest_bytes,
                                "protocol": protocol_bytes,
                                "cohorts": cohorts_bytes,
                                "source-closure": closure_bytes,
                                "initial-registration": registration,
                                "active-registration": registration,
                            },
                        )
                    )
                except Exception as exc:
                    result.append(exc)

            worker = threading.Thread(target=run_request)
            worker.start()
            request_path = root / stage / "source-capture" / "requests" / "permit.json"
            for _ in range(100):
                if request_path.exists():
                    break
                time.sleep(0.005)
            request_bytes = request_path.read_bytes()
            receipt = authority.PhaseArtifactReceiptIssuer.source_capture(
                root=root,
                stage_uuid=stage,
                request_id="permit",
                request_bytes=request_bytes,
                expected_source_closure_sha256=closure,
                expected_initial_registration_sha256=digest(registration),
                expected_active_registration_sha256=digest(registration),
                expected_capture_manifest_bytes=manifest_bytes,
            )
            with self.assertRaisesRegex(authority.LocalAuthorityError, "registered-context-pin-mismatch"):
                authority.PhaseArtifactReceiptIssuer.source_capture(
                    root=root,
                    stage_uuid=stage,
                    request_id="permit",
                    request_bytes=request_bytes,
                    expected_source_closure_sha256=closure,
                    expected_initial_registration_sha256="0" * 64,
                    expected_active_registration_sha256=digest(registration),
                    expected_capture_manifest_bytes=manifest_bytes,
                )
            authority.SourceCaptureReceiptVerifier().verify(manifest_bytes, receipt, digest(receipt), None)
            response = request_path.parent.parent / "responses" / "permit.receipt"
            pin = request_path.parent.parent / "pins" / "permit.sha256"
            response.write_bytes(receipt)
            response.chmod(0o600)
            pin.write_text(digest(receipt) + "\n", encoding="ascii")
            pin.chmod(0o600)
            worker.join(timeout=2)
            self.assertFalse(worker.is_alive())
            self.assertEqual(result, [(receipt, digest(receipt))])

    def test_file_handoff_issuer_rebuilds_answer_manifest_from_tasks(self):
        from scripts import coverage_answer_execution as answer
        from scripts import coverage_operator_runner as runner
        from tests.test_coverage_answer_execution import _tasks

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "handoff"
            exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
            stage = str(uuid.UUID(int=8126))
            source_revision = "9" * 40
            from dataclasses import replace

            base_tasks = _tasks()
            cohort_cases = []
            tasks = []
            for task in base_tasks:
                facets = [
                    {
                        "facet_id": f"{task.task_id}-F{facet_index}",
                        "definition": f"Registered facet {facet_index}",
                        "critical_checks": [
                            f"Registered check {facet_index}.1",
                            f"Registered check {facet_index}.2",
                        ],
                    }
                    for facet_index in (1, 2)
                ]
                checks = tuple(
                    {"id": f"{facet['facet_id']}-C{index}", "description": text}
                    for facet in facets
                    for index, text in enumerate(facet["critical_checks"], start=1)
                )
                cohort_cases.append(
                    {
                        "task_id": task.task_id,
                        "search_query": task.query,
                        "purpose": task.purpose,
                        "critical_facets": facets,
                    }
                )
                answers = []
                for item in task.answers:
                    payload = json.loads(item.body)
                    payload["task"]["critical_checks"] = list(checks)
                    body = canonical(payload)
                    answers.append(replace(item, body=body, body_sha256=digest(body)))
                tasks.append(replace(task, critical_checks=checks, answers=tuple(answers)))
            tasks = tuple(tasks)
            cohorts = canonical({"stages": [{"stage": "development", "research_cases": cohort_cases}]})
            protocol = canonical({"registered": "protocol", "cohorts_sha256": digest(cohorts)})
            closure = canonical({"schema": "closure-test", "source_revision": source_revision})
            closure_sha = digest(closure)
            input_manifest = canonical(
                {
                    "schema": "coverage-study-input-manifest/1",
                    "stage_uuid": stage,
                    "research_tasks": [
                        {
                            "task_id": row.task_id,
                            "query": row.query,
                            "purpose": row.purpose,
                            "facets": [],
                            "candidates": [],
                            "incumbent_order": [],
                            "compiled_request_sha256": "0" * 64,
                        }
                        for row in tasks
                    ],
                    "navigation_tasks": [],
                }
            )
            registration = registered_context(
                stage,
                source_revision,
                protocol,
                closure_sha,
                {"task_input_manifest": digest(input_manifest)},
            )
            task_bytes = runner._answer_task_artifact_bytes(stage, tasks)
            requests = answer._make_requests(tasks)
            manifest, manifest_sha = answer._request_manifest(requests, endpoint_security_mode="https-required")
            endpoint = "https://answer.example/v1"
            resolved_sha = answer._sha(answer._canonical({"mode": "https-required"}))
            endpoint_sha = answer._sha(
                answer._canonical(
                    {
                        "endpoint": endpoint,
                        "security_mode": "https-required",
                        "resolved_destination_sha256": resolved_sha,
                    }
                )
            )
            bindings = {
                "stage_uuid": stage,
                "source_revision": source_revision,
                "protocol_sha256": digest(protocol),
                "cohorts_sha256": digest(cohorts),
                "operation_manifest_sha256": manifest_sha,
                "endpoint_sha256": endpoint_sha,
                "resolved_destination_sha256": resolved_sha,
                "endpoint_security_mode": "https-required",
                "operation_ids": [row.operation_id for row in requests],
                "source_closure_sha256": closure_sha,
            }
            artifacts = {
                "protocol": protocol,
                "cohorts": cohorts,
                "source-closure": closure,
                "initial-registration": registration,
                "active-registration": registration,
                "task-input-manifest": input_manifest,
                "answer-request-manifest": manifest,
                "answer-task-material": task_bytes,
            }
            result: list[object] = []

            def run_request():
                try:
                    result.append(
                        exchange.request(
                            stage_uuid=stage,
                            scope="answer-execution",
                            request_id="permit",
                            bindings=bindings,
                            deadline_monotonic=time.monotonic() + 4,
                            artifacts=artifacts,
                        )
                    )
                except Exception as exc:
                    result.append(exc)

            worker = threading.Thread(target=run_request)
            worker.start()
            request_path = root / stage / "answer-execution" / "requests" / "permit.json"
            for _ in range(100):
                if request_path.exists():
                    break
                time.sleep(0.005)
            request_bytes = request_path.read_bytes()
            receipt = asyncio.run(
                authority.PhaseArtifactReceiptIssuer.answer_execution(
                    root=root,
                    stage_uuid=stage,
                    request_id="permit",
                    request_bytes=request_bytes,
                    expected_source_closure_sha256=closure_sha,
                    endpoint=endpoint,
                    expected_initial_registration_sha256=digest(registration),
                    expected_active_registration_sha256=digest(registration),
                    allow_trusted_private_http=False,
                    expected_task_material_bytes=task_bytes,
                )
            )
            with self.assertRaisesRegex(authority.LocalAuthorityError, "registered-context-pin-mismatch"):
                asyncio.run(
                    authority.PhaseArtifactReceiptIssuer.answer_execution(
                        root=root,
                        stage_uuid=stage,
                        request_id="permit",
                        request_bytes=request_bytes,
                        expected_source_closure_sha256=closure_sha,
                        endpoint=endpoint,
                        expected_initial_registration_sha256=digest(registration),
                        expected_active_registration_sha256="0" * 64,
                        allow_trusted_private_http=False,
                        expected_task_material_bytes=task_bytes,
                    )
                )
            public_bindings = {key: value for key, value in bindings.items() if key != "source_closure_sha256"}
            typed = authority.AnswerReceiptVerifier().verify(receipt, digest(receipt), public_bindings)
            self.assertEqual(typed.operation_manifest_sha256, manifest_sha)
            response = request_path.parent.parent / "responses" / "permit.receipt"
            pin = request_path.parent.parent / "pins" / "permit.sha256"
            response.write_bytes(receipt)
            response.chmod(0o600)
            pin.write_text(digest(receipt) + "\n", encoding="ascii")
            pin.chmod(0o600)
            worker.join(timeout=4)
            self.assertFalse(worker.is_alive())
            self.assertEqual(result, [(receipt, digest(receipt))])

    def test_file_handoff_selector_admission_requires_rebuilt_map_and_registered_context(self):
        from scripts import coverage_selector_admission as admission
        from scripts import coverage_study_core as core

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "operator-handoff"
            exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
            stage = str(uuid.UUID(int=8127))
            revision = "8" * 40
            cohorts = canonical({"cohort": "registered"})
            protocol = canonical(
                {
                    "cohorts_sha256": digest(cohorts),
                    "selector_input_map_schema": "coverage-selector-input-map/2-registered",
                    "selector_input_map_status": "registered",
                }
            )
            closure_bytes = canonical({"source_revision": revision})
            closure = digest(closure_bytes)
            registration = registered_context(stage, revision, protocol, closure)
            prepared = core.PreparedStage(
                stage_uuid=stage,
                stage_kind="development",
                source_revision=revision,
                registration_sha256=digest(registration),
                pins={"protocol": digest(protocol), "qualified_source_closure": closure},
                navigation_skip_reasons=(None,) * 5,
                operation_ids=("research-01-base-w0",),
                acquisition_calls=1,
                scraper_calls=0,
                health_calls=0,
                selector_input_map_schema="coverage-selector-input-map/2-registered",
            )
            map_bytes = canonical({"schema": prepared.selector_input_map_schema, "operations": []})
            bindings = {
                "stage_uuid": stage,
                "source_revision": revision,
                "protocol_sha256": digest(protocol),
                "registration_sha256": digest(registration),
                "source_closure_sha256": closure,
                "selector_input_map_sha256": digest(map_bytes),
            }
            result: list[object] = []

            def run_request():
                try:
                    result.append(
                        exchange.request(
                            stage_uuid=stage,
                            scope="selector-map-admission",
                            request_id="map",
                            bindings=bindings,
                            deadline_monotonic=time.monotonic() + 3,
                            artifacts={
                                "protocol": protocol,
                                "cohorts": cohorts,
                                "source-closure": closure_bytes,
                                "initial-registration": registration,
                                "active-registration": registration,
                                "selector-map": map_bytes,
                            },
                        )
                    )
                except Exception as exc:
                    result.append(exc)

            worker = threading.Thread(target=run_request)
            worker.start()
            request_path = root / stage / "selector-map-admission" / "requests" / "map.json"
            for _ in range(100):
                if request_path.exists():
                    break
                time.sleep(0.005)
            request_bytes = request_path.read_bytes()
            receipt = authority.PhaseArtifactReceiptIssuer.selector_map_admission(
                root=root,
                stage_uuid=stage,
                request_id="map",
                request_bytes=request_bytes,
                expected_source_closure_sha256=closure,
                expected_initial_registration_sha256=digest(registration),
                expected_active_registration_sha256=digest(registration),
                prepared=prepared,
                expected_selector_map_bytes=map_bytes,
            )
            admission.verify_selector_map_admission(
                receipt_bytes=receipt,
                expected_receipt_sha256=digest(receipt),
                protocol_bytes=protocol,
                prepared=prepared,
                selector_input_map_sha256=digest(map_bytes),
            )
            response = request_path.parent.parent / "responses" / "map.receipt"
            response.write_bytes(receipt)
            response.chmod(0o600)
            (request_path.parent.parent / "pins" / "map.sha256").write_text(digest(receipt) + "\n")
            (request_path.parent.parent / "pins" / "map.sha256").chmod(0o600)
            worker.join(timeout=3)
            self.assertFalse(worker.is_alive())
            self.assertEqual(result, [(receipt, digest(receipt))])

    def test_file_handoff_frozen_collection_permit_uses_registered_manifest_and_host_clock(self):
        from scripts import coverage_frozen_pool_collect as collector

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "operator-handoff"
            exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
            stage = str(uuid.UUID(int=8128))
            revision = "7" * 40
            cohorts = canonical({"cohorts": "fixed"})
            protocol = canonical({"cohorts_sha256": digest(cohorts), "pool_input_mode": "frozen-pool/1"})
            closure_raw = canonical({"source_revision": revision})
            closure = digest(closure_raw)
            acquisition_plan = canonical({"plan": "registered"})
            identity = canonical({"candidate": "identity"})
            endpoint_sha = "e" * 64
            started = time.monotonic() - 1
            deadline = started + collector.MAX_STAGE_WALL_SECONDS
            started_utc = datetime.now(timezone.utc) - timedelta(seconds=1)
            clock_doc = {
                "schema": "coverage-frozen-collection-stage-clock/1",
                "stage_started_monotonic": started,
                "stage_deadline_monotonic": deadline,
                "stage_started_utc": started_utc.isoformat().replace("+00:00", "Z"),
                "stage_deadline_utc": (started_utc + timedelta(seconds=collector.MAX_STAGE_WALL_SECONDS))
                .isoformat()
                .replace("+00:00", "Z"),
                "clock_origin_sha256": "a" * 64,
            }
            clock_raw = collector._canonical(clock_doc)
            from scripts import coverage_live_acquire as live

            acquisition_manifest = live._canonical(
                {
                    "stage_uuid": stage,
                    "stage": "confirmation",
                    "source_revision": revision,
                    "source_closure_sha256": closure,
                    "protocol_sha256": digest(protocol),
                    "cohorts_sha256": digest(cohorts),
                    "input_manifest_sha256": "f" * 64,
                    "acquisition_plan_sha256": digest(acquisition_plan),
                }
            )
            registration = registered_context(stage, revision, protocol, closure, stage_kind="confirmation")
            development_stage = str(uuid.UUID(int=8127))
            development_receipt = canonical(
                {
                    "schema": "coverage-study-development-gate-receipt/1",
                    "stage_uuid": development_stage,
                    "stage_kind": "development",
                    "status": "passed",
                    "protocol_sha256": digest(protocol),
                    "source_revision": revision,
                    "source_closure_sha256": closure,
                    "result_sha256": "9" * 64,
                }
            )
            plan = SimpleNamespace(
                stage_uuid=stage,
                stage_kind="confirmation",
                source_revision=revision,
                source_closure_sha256=closure,
                pool_input_mode="frozen-pool/1",
                protocol_bytes=protocol,
                cohorts_bytes=cohorts,
                acquisition_plan_bytes=acquisition_plan,
                acquisition_manifest_bytes=acquisition_manifest,
                candidate_identity_bytes=identity,
                candidate_endpoint_sha256=endpoint_sha,
                stage_started_monotonic=started,
                stage_deadline_monotonic=deadline,
                expected_acquisition_manifest_sha256=digest(acquisition_manifest),
                development_receipt_bytes=development_receipt,
                development_receipt_sha256=digest(development_receipt),
                development_stage_uuid=development_stage,
            )
            bindings = {
                "stage_uuid": stage,
                "stage_kind": plan.stage_kind,
                "source_revision": revision,
                "source_closure_sha256": closure,
                "protocol_sha256": digest(protocol),
                "cohorts_sha256": digest(cohorts),
                "input_manifest_sha256": "f" * 64,
                "acquisition_plan_sha256": digest(acquisition_plan),
                "legacy_manifest_sha256": digest(acquisition_manifest),
                "pool_input_mode": "frozen-pool/1",
                "stage_clock_sha256": digest(clock_raw),
                "candidate_identity_sha256": digest(identity),
                "candidate_endpoint_sha256": endpoint_sha,
                "development_receipt_sha256": digest(development_receipt),
                "development_stage_uuid": development_stage,
            }
            result: list[object] = []

            def run_request():
                try:
                    result.append(
                        exchange.request(
                            stage_uuid=stage,
                            scope="frozen-pool-collection",
                            request_id="permit",
                            bindings=bindings,
                            deadline_monotonic=time.monotonic() + 3,
                            artifacts={
                                "protocol": protocol,
                                "cohorts": cohorts,
                                "source-closure": closure_raw,
                                "initial-registration": registration,
                                "active-registration": registration,
                                "acquisition-plan": acquisition_plan,
                                "acquisition-manifest": acquisition_manifest,
                                "stage-clock": clock_raw,
                                "development-pass-receipt": development_receipt,
                            },
                        )
                    )
                except Exception as exc:
                    result.append(exc)

            worker = threading.Thread(target=run_request)
            worker.start()
            request_path = root / stage / "frozen-pool-collection" / "requests" / "permit.json"
            for _ in range(100):
                if request_path.exists():
                    break
                time.sleep(0.005)
            request_bytes = request_path.read_bytes()
            with mock.patch.object(collector, "_local_clock_origin_sha256", return_value="a" * 64):
                receipt = authority.PhaseArtifactReceiptIssuer.frozen_pool_collection(
                    root=root,
                    stage_uuid=stage,
                    request_id="permit",
                    request_bytes=request_bytes,
                    expected_source_closure_sha256=closure,
                    expected_initial_registration_sha256=digest(registration),
                    expected_active_registration_sha256=digest(registration),
                    plan=plan,
                )
            response = request_path.parent.parent / "responses" / "permit.receipt"
            response.write_bytes(receipt)
            response.chmod(0o600)
            (request_path.parent.parent / "pins" / "permit.sha256").write_text(digest(receipt) + "\n")
            (request_path.parent.parent / "pins" / "permit.sha256").chmod(0o600)
            worker.join(timeout=3)
            self.assertFalse(worker.is_alive())
            self.assertEqual(result, [(receipt, digest(receipt))])

    def test_late_registration_rebuild_uses_distinct_handoff_and_inventory_roots(self):
        """Compose fresh registered-v2 plan, MockTransport collection, and real late preflight."""
        import httpx

        from scripts import coverage_frozen_pool_collect as collector
        from scripts import coverage_frozen_pool_inputs as frozen
        from scripts import coverage_operator_runner as runner
        from scripts import coverage_resource_evidence as resource_evidence
        from scripts import coverage_stage_orchestration as orchestration
        from tests.test_coverage_frozen_pool_collect import _handler, _Lease, _Pacer, _PermitVerifier
        from tests.test_coverage_operator_runner import OperatorRunnerConfigTests

        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            config_root = base / "config"
            config_root.mkdir(mode=0o700)
            config_path, config_sha = OperatorRunnerConfigTests()._registered_companion_config(config_root)
            with (
                mock.patch.object(runner, "_verify_checkout_sources"),
                mock.patch.object(runner, "_require_source_bootstrap"),
                mock.patch.object(runner.runtime_environment, "verify_active_environment", return_value=b"{}"),
            ):
                source_runner = runner.OperatorStageRunner.from_file(config_path, config_sha)
            plan = source_runner.plan
            static_materials = source_runner.static_materials
            expected_phase_task_ids = tuple(row["task_id"] for row in plan.research_cases) + tuple(
                row["target_id"] for row in plan.navigation_targets
            )
            self.assertEqual(
                tuple(row["task_id"] for row in json.loads(static_materials["capture_plan"])["tasks"]),
                expected_phase_task_ids,
            )
            handoff_root = Path(source_runner.directories["operator_handoff"])
            inventory_root = Path(source_runner.directories["stage_inventory"])
            stage_directory = inventory_root / plan.stage_uuid
            stage_directory.mkdir(mode=0o700)

            stage_clock = collector.FrozenCollectionStageClock(
                stage_started_monotonic=plan.stage_started_monotonic,
                stage_deadline_monotonic=plan.stage_deadline_monotonic,
                stage_started_utc=source_runner.started_utc.isoformat().replace("+00:00", "Z"),
                stage_deadline_utc=source_runner.deadline_utc.isoformat().replace("+00:00", "Z"),
                clock_origin_sha256=digest(b"test-only-clock-origin"),
            )
            pacer = _Pacer()
            pacer.elapsed = plan.stage_started_monotonic
            calls = []
            from urllib.parse import parse_qs

            base_handler = _handler(calls)

            def registered_handler(request):
                query = parse_qs(request.url.query.decode()).get("q", [""])[0]
                if request.url.host == "api.github.com":
                    for target in plan.navigation_targets:
                        if target["search_query"].casefold() in query.casefold():
                            url = target["target_url"]
                            owner_repo = url.removeprefix("https://github.com/")
                            calls.append(request)
                            return httpx.Response(
                                200,
                                json={
                                    "total_count": 1,
                                    "items": [
                                        {
                                            "full_name": owner_repo,
                                            "html_url": url,
                                            "description": "registered navigation fixture",
                                            "stargazers_count": 1,
                                            "language": "Python",
                                        },
                                        {
                                            "full_name": f"fixture/alternate-{target['target_id']}",
                                            "html_url": f"https://github.com/fixture/alternate-{target['target_id']}",
                                            "description": "secondary registered navigation fixture",
                                            "stargazers_count": 0,
                                            "language": "Python",
                                        },
                                    ],
                                },
                                request=request,
                            )
                return base_handler(request)

            with mock.patch.object(
                collector, "_local_clock_origin_sha256", return_value=stage_clock.clock_origin_sha256
            ):
                result = asyncio.run(
                    collector.collect_frozen_pool_stage(
                        stage_manifest_bytes=plan.acquisition_manifest_bytes,
                        acquisition_plan_bytes=plan.acquisition_plan_bytes,
                        stage_clock=stage_clock,
                        candidate_identity_sha256=digest(plan.candidate_identity_bytes),
                        candidate_endpoint_sha256=plan.candidate_endpoint_sha256,
                        permit_receipt_bytes=b"mock permit",
                        expected_permit_receipt_sha256=digest(b"mock permit"),
                        permit_verifier=_PermitVerifier(),
                        one_shot_lease=_Lease(),
                        stage_inventory_directory=stage_directory,
                        test_transport=httpx.MockTransport(registered_handler),
                        pacer=pacer,
                    )
                )
            collection_summary = json.loads((result.collection_directory / "collection-summary.json").read_text())
            frozen_index = json.loads((result.collection_directory / "frozen-pool-index.json").read_text())
            self.assertEqual(
                result.status,
                "complete",
                collection_summary["operation_statuses"]
                + [
                    json.loads((result.collection_directory / row["file"]).read_text())["operation"]
                    .get("canonical_response", {})
                    .get("results", [{}])[0]
                    .get("url")
                    for row in frozen_index["operations"]
                    if row["kind"] == "navigation"
                ],
            )
            collection = result.collection_directory
            evidence = frozen.FrozenPoolEvidence(
                snapshots_directory=collection,
                evidence_bytes=(collection / "frozen-pool-evidence.json").read_bytes(),
                expected_evidence_sha256=digest((collection / "frozen-pool-evidence.json").read_bytes()),
                collector_receipt_bytes=(collection / "collector-receipt.json").read_bytes(),
                expected_collector_receipt_sha256=digest((collection / "collector-receipt.json").read_bytes()),
                availability_bytes=(collection / "engine-availability.json").read_bytes(),
                expected_availability_sha256=digest((collection / "engine-availability.json").read_bytes()),
            )
            verified = frozen.verify_frozen_pool_evidence(
                evidence,
                expected_stage_uuid=plan.stage_uuid,
                expected_stage_kind=plan.stage_kind,
                expected_source_revision=plan.source_revision,
                expected_source_closure_sha256=plan.source_closure_sha256,
                expected_protocol_sha256=digest(plan.protocol_bytes),
                expected_cohorts_sha256=digest(plan.cohorts_bytes),
                expected_candidate_identity_sha256=digest(plan.candidate_identity_bytes),
                expected_candidate_endpoint_sha256=plan.candidate_endpoint_sha256,
                expected_acquisition_plan_sha256=digest(plan.acquisition_plan_bytes),
                expected_task_ids=tuple(row["task_id"] for row in plan.research_cases),
                expected_navigation_ids=tuple(row["target_id"] for row in plan.navigation_targets),
                stage_started_monotonic=plan.stage_started_monotonic,
                stage_deadline_monotonic=plan.stage_deadline_monotonic,
                now_monotonic=pacer.monotonic(),
            )
            pipeline_inputs = frozen.build_pipeline_inputs_from_frozen(
                list(plan.research_cases),
                evidence=evidence,
                verified=verified,
                expected_stage_uuid=plan.stage_uuid,
                expected_source_revision=plan.source_revision,
                expected_protocol_sha256=digest(plan.protocol_bytes),
                expected_cohorts_sha256=digest(plan.cohorts_bytes),
                expected_candidate_identity_sha256=digest(plan.candidate_identity_bytes),
                expected_candidate_endpoint_sha256=plan.candidate_endpoint_sha256,
            )
            registration_context = frozen.FrozenPoolRegistrationContext(
                stage_uuid=plan.stage_uuid,
                stage_kind=plan.stage_kind,
                source_revision=plan.source_revision,
                source_closure_sha256=plan.source_closure_sha256,
                protocol_sha256=digest(plan.protocol_bytes),
                cohorts_sha256=digest(plan.cohorts_bytes),
                candidate_identity_sha256=digest(plan.candidate_identity_bytes),
                candidate_endpoint_sha256=plan.candidate_endpoint_sha256,
                acquisition_plan_sha256=digest(plan.acquisition_plan_bytes),
                task_ids=tuple(row["task_id"] for row in plan.research_cases),
                navigation_ids=tuple(row["target_id"] for row in plan.navigation_targets),
                evidence_sha256=verified.evidence_sha256,
                collector_receipt_sha256=verified.collector_receipt_sha256,
                availability_sha256=verified.availability_sha256,
            )
            frozen_stage_evidence = orchestration.FrozenPoolStageEvidence(
                collection=orchestration.FrozenPoolCollectionEvidence(
                    evidence=evidence,
                    registration=registration_context,
                    receipt_directory=collection,
                    receipt_inventory_sha256=resource_evidence.receipt_inventory_sha256(collection),
                    collector_status=result.status,
                ),
                phase_result=frozen.FrozenPoolPhaseResult(verified=verified, pipeline_inputs=pipeline_inputs),
            )
            source_runner._frozen_stage_clock_bytes = collector._canonical(
                {
                    "schema": "coverage-frozen-collection-stage-clock/1",
                    **stage_clock.__dict__,
                }
            )
            worker_result: list[object] = []

            async def prepare_late():
                try:
                    worker_result.append(
                        await source_runner._prepare_after_frozen_pool(
                            plan,
                            frozen_stage_evidence,
                            pipeline_inputs,
                        )
                    )
                except Exception as exc:
                    worker_result.append(exc)

            worker = threading.Thread(target=lambda: asyncio.run(prepare_late()))
            worker.start()
            request_path = handoff_root / plan.stage_uuid / "late-registration" / "requests" / "registration-1.json"
            for _ in range(200):
                if request_path.exists():
                    break
                time.sleep(0.005)
            if not request_path.exists():
                worker.join(timeout=2)
                self.fail(f"production late request did not persist: {worker_result!r}")
            self.assertTrue(request_path.exists())
            request_bytes = request_path.read_bytes()
            with mock.patch.object(
                collector, "_local_clock_origin_sha256", return_value=stage_clock.clock_origin_sha256
            ):
                receipt = authority.PhaseArtifactReceiptIssuer.late_registration(
                    root=handoff_root,
                    inventory_root=inventory_root,
                    stage_uuid=plan.stage_uuid,
                    request_id="registration-1",
                    request_bytes=request_bytes,
                    expected_source_closure_sha256=plan.source_closure_sha256,
                    expected_initial_registration_sha256=source_runner.initial_registration_sha256,
                    expected_active_registration_sha256=source_runner.initial_registration_sha256,
                    plan=plan,
                    static_materials=static_materials,
                    forbidden_stage_uuids=tuple(source_runner.config["forbidden_stage_uuids"]),
                    now_monotonic=pacer.monotonic(),
                )
            response = request_path.parent.parent / "responses" / "registration-1.receipt"
            response.write_bytes(receipt)
            response.chmod(0o600)
            pin_path = request_path.parent.parent / "pins" / "registration-1.sha256"
            pin_path.write_text(digest(receipt) + "\n", encoding="ascii")
            pin_path.chmod(0o600)
            worker.join(timeout=10)
            self.assertFalse(worker.is_alive())
            collection_summary = json.loads((collection / "collection-summary.json").read_bytes())
            self.assertEqual(len(calls), collection_summary["physical_request_count"])
            self.assertEqual(len(worker_result), 1)
            self.assertIsInstance(worker_result[0], orchestration.LatePreparationEvidence)
            self.assertEqual(worker_result[0].prepared.stage_uuid, plan.stage_uuid)
            self.assertEqual(worker_result[0].prepared.fresh_execution_authorized, False)

            from scripts import coverage_selector_input_map

            operation_materials: dict[str, dict[str, object]] = {}
            map_bytes = coverage_selector_input_map.build_selector_input_map(
                prepared=worker_result[0].prepared,
                task_input_manifest_bytes=worker_result[0].input_manifest_bytes,
                pipeline_tasks=pipeline_inputs["tasks"],
                navigation_targets=plan.navigation_targets,
                snapshots_directory=verified.snapshots_directory,
                acquisition_snapshot_index_sha256=verified.snapshot_index_sha256,
                acquisition_manifest_bytes=plan.acquisition_manifest_bytes,
                acquisition_manifest_sha256=verified.pool_manifest_sha256,
                verified_pool_source=verified,
                neutral_fixture_bytes=(
                    Path(__file__).parents[1]
                    / "docs/experiments/evidence/coverage-first-study/draft-v2/selector-readiness-inputs.json"
                ).read_bytes(),
                frozen_v1_source_bytes=(
                    Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study/w0-rerank-v1.py.txt"
                ).read_bytes(),
                current_service_source_bytes=(Path(__file__).parents[1] / "slopsearx/service.py").read_bytes(),
                expected_builder_source_sha256=digest(Path(coverage_selector_input_map.__file__).read_bytes()),
                operation_materials=operation_materials,
            )
            selector_result: list[object] = []

            async def resolve_selector_permits():
                try:
                    selector_result.append(
                        await source_runner._selector_permits(
                            plan,
                            worker_result[0].prepared,
                            map_bytes,
                            digest(map_bytes),
                            operation_materials,
                        )
                    )
                except Exception as exc:
                    selector_result.append(exc)

            selector_worker = threading.Thread(target=lambda: asyncio.run(resolve_selector_permits()))
            selector_worker.start()
            selector_request_path = (
                handoff_root / plan.stage_uuid / "selector-operation-permits" / "requests" / "batch.json"
            )
            for _ in range(300):
                if selector_request_path.exists():
                    break
                time.sleep(0.005)
            if not selector_request_path.exists():
                selector_worker.join(timeout=2)
                self.fail(f"production selector request did not persist: {selector_result!r}")
            selector_request_bytes = selector_request_path.read_bytes()
            selector_error = None
            try:
                with mock.patch.object(
                    collector, "_local_clock_origin_sha256", return_value=stage_clock.clock_origin_sha256
                ):
                    selector_receipt = authority.PhaseArtifactReceiptIssuer.selector_operation_permits(
                        root=handoff_root,
                        inventory_root=inventory_root,
                        stage_uuid=plan.stage_uuid,
                        request_id="batch",
                        request_bytes=selector_request_bytes,
                        expected_source_closure_sha256=plan.source_closure_sha256,
                        expected_initial_registration_sha256=source_runner.initial_registration_sha256,
                        expected_active_registration_sha256=digest(source_runner._active_registration_bytes),
                        plan=plan,
                        static_materials=static_materials,
                        forbidden_stage_uuids=tuple(source_runner.config["forbidden_stage_uuids"]),
                        now_monotonic=pacer.monotonic(),
                    )
            except Exception as exc:
                selector_error = exc
                selector_receipt = b"{}"
            selector_response = selector_request_path.parent.parent / "responses" / "batch.receipt"
            selector_response.write_bytes(selector_receipt)
            selector_response.chmod(0o600)
            selector_pin = selector_request_path.parent.parent / "pins" / "batch.sha256"
            selector_pin.write_text(digest(selector_receipt) + "\n", encoding="ascii")
            selector_pin.chmod(0o600)
            selector_worker.join(timeout=15)
            self.assertFalse(selector_worker.is_alive())
            if selector_error is not None:
                raise selector_error
            self.assertEqual(len(selector_result), 1)
            self.assertIsInstance(selector_result[0], dict)
            self.assertEqual(set(selector_result[0]), set(worker_result[0].prepared.operation_ids))

            # Execute the registered selector inventory against deterministic
            # HTTPX fixtures, then reconstruct the answer phase from the
            # durable source/capture/selector artifacts through the real
            # runner and issuer path.
            from scripts import coverage_assessment_packets as packets
            from scripts import coverage_jev_execution as jev
            from scripts import coverage_selector_admission as selector_admission
            from scripts import coverage_source_capture as source_capture
            from scripts import coverage_stage_orchestration as orchestration
            from scripts import coverage_study_core as core
            from scripts import intent_ranking_coverage as coverage

            prepared = worker_result[0].prepared
            selector_results = Path(source_runner.directories["selector_results"])
            selector_archives = Path(source_runner.directories["selector_archive"])
            for directory in (selector_results, selector_archives):
                directory.mkdir(mode=0o700, parents=True, exist_ok=True)
                directory.chmod(0o700)
            selector_admission_receipt = canonical(
                {
                    "schema": selector_admission.SCHEMA,
                    "status": "admitted",
                    "stage_uuid": plan.stage_uuid,
                    "source_revision": plan.source_revision,
                    "protocol_sha256": digest(plan.protocol_bytes),
                    "registration_sha256": prepared.registration_sha256,
                    "source_closure_sha256": plan.source_closure_sha256,
                    "selector_input_map_sha256": digest(map_bytes),
                }
            )
            verified_admission = selector_admission.verify_selector_map_admission(
                receipt_bytes=selector_admission_receipt,
                expected_receipt_sha256=digest(selector_admission_receipt),
                protocol_bytes=plan.protocol_bytes,
                prepared=prepared,
                selector_input_map_sha256=digest(map_bytes),
            )
            jev._bind_selector_input_map(prepared, map_bytes, digest(map_bytes), admission=verified_admission)
            selector_ledger = core.StudyRun(prepared)

            def selector_handler(operation_id):
                def handle(request):
                    material = operation_materials[operation_id]
                    _body, compiled = jev.build_selector_request(
                        operation_id=operation_id,
                        operation_input_bytes=material["operation_input_bytes"],
                        legacy_control=material.get("legacy_control"),
                    )
                    if compiled is None:
                        ids = [f"c{index}" for index in range(len(material["legacy_control"]["canonical_pool"]))]
                        payload = {
                            "model": coverage.MODEL,
                            "answers": {card_id: {"type": "score", "score": 5} for card_id in ids},
                            "usage": {"input_tokens": 20, "output_tokens": 10},
                        }
                    else:
                        answers = {}
                        for question_id in compiled.question_ids:
                            if question_id.startswith("score:"):
                                answers[question_id] = {
                                    "type": "score",
                                    "score": 5,
                                    "legend": {str(index): label for index, label in enumerate(coverage.LEVELS)},
                                    "probabilities": {str(index): (1.0 if index == 5 else 0.0) for index in range(10)},
                                    "confidence": 1.0,
                                }
                            else:
                                options = compiled.profile_options[question_id]
                                answers[question_id] = {
                                    "type": "choice",
                                    "choice": options[0],
                                    "probabilities": {
                                        option: (1.0 if option == options[0] else 0.0) for option in options
                                    },
                                    "confidence": 1.0,
                                }
                        payload = {
                            "model": coverage.MODEL,
                            "answers": answers,
                            "usage": {"input_tokens": 20, "output_tokens": 10},
                        }
                    return httpx.Response(200, json=payload, request=request)

                return handle

            call_results = []
            for operation_id in prepared.operation_ids:
                material = operation_materials[operation_id]
                permit_bytes = selector_result[0][operation_id][0]
                transport = httpx.MockTransport(selector_handler(operation_id))
                call_results.append(
                    asyncio.run(
                        jev.execute_selector_call(
                            prepared=prepared,
                            ledger=selector_ledger,
                            operation_id=operation_id,
                            operation_input_bytes=material["operation_input_bytes"],
                            permit_bytes=permit_bytes,
                            expected_permit_sha256=selector_result[0][operation_id][1],
                            api_key="synthetic-only",
                            lease_root=source_runner.directories["lease_root"],
                            archive_root=selector_archives,
                            result_root=selector_results,
                            transport=transport,
                            legacy_control=material.get("legacy_control"),
                            selector_input_map_bytes=map_bytes,
                            expected_selector_input_map_sha256=digest(map_bytes),
                        )
                    )
                )
            terminal = jev.close_stage(prepared=prepared, ledger=selector_ledger, result_root=selector_results)
            self.assertEqual(terminal["status"], "all-registered-calls-complete")
            self.assertTrue(terminal["all_calls_complete"])
            self.assertEqual(len(call_results), len(prepared.operation_ids))
            self.assertIsInstance(operation_materials["research-01-base-w0"].get("legacy_control"), dict)

            capture_root = Path(source_runner.directories["capture_receipts"])
            capture_stage = capture_root / plan.stage_uuid
            capture_stage.mkdir(mode=0o700, parents=True)
            capture_stage.chmod(0o700)
            capture_manifest_bytes = pipeline_inputs["capture_manifest_bytes"]
            source_capture._write_once(capture_stage / "source-manifest.json", capture_manifest_bytes)
            capture_manifest = json.loads(capture_manifest_bytes)
            capture_rows = []
            captured_source_ids = set()
            for source_index, source_row in enumerate(capture_manifest["sources"], start=1):
                if source_row["source_id"] in captured_source_ids:
                    capture_rows.append({**source_row, "status": "duplicate_url_not_attempted", "attempted": False})
                else:
                    captured_source_ids.add(source_row["source_id"])
                    context_meta = source_capture._context_artifact(
                        capture_stage,
                        source_index,
                        source_row,
                        f"Synthetic captured context for {source_row['source_id']}.",
                    )
                    capture_rows.append(
                        {
                            **source_row,
                            "status": "captured",
                            "attempted": True,
                            "context_artifact": context_meta["relative_path"],
                            "context_sha256": context_meta["context_sha256"],
                            "context_characters": context_meta["context_characters"],
                        }
                    )
            capture_inventory = {
                "schema": "coverage-source-capture-inventory/1",
                "stage_uuid": plan.stage_uuid,
                "source_revision": plan.source_revision,
                "source_manifest_sha256": digest(capture_manifest_bytes),
                "protocol_sha256": digest(plan.protocol_bytes),
                "cohorts_sha256": digest(plan.cohorts_bytes),
                "candidate_identity_sha256": digest(plan.candidate_identity_bytes),
                "status": "complete",
                "owned_http_calls": len(capture_rows),
                "health_calls": 1,
                "scrape_calls": len(capture_rows),
                "sources": capture_rows,
            }
            capture_inventory_bytes = source_capture._canonical_json(capture_inventory)
            (capture_stage / "inventory.json").write_bytes(capture_inventory_bytes)
            (capture_stage / "inventory.json").chmod(0o600)

            answer_inputs = pipeline_inputs
            prepared_packets = packets.prepare_assessment_packets(
                packet_stage_uuid=plan.packet_stage_uuid,
                pipeline_inputs=answer_inputs,
                research_cases=list(plan.research_cases),
                capture_inventory_bytes=capture_inventory_bytes,
                expected_capture_inventory_sha256=digest(capture_inventory_bytes),
                context_artifacts={path.name: path.read_bytes() for path in capture_stage.glob("context-*.json")},
            )
            selector_evidence = source_runner._selector_evidence(
                plan,
                worker_result[0].prepared,
                {
                    **pipeline_inputs,
                    "selector_operation_materials": operation_materials,
                    "task_input_manifest_bytes": worker_result[0].input_manifest_bytes,
                },
                SimpleNamespace(receipt_sha256=digest(b"synthetic-reference-closure")),
                call_results,
                terminal,
            )
            result_statuses = {row.status for row in call_results}
            self.assertEqual(result_statuses, {"complete-original-v1", "selected"})
            self.assertEqual(
                {row["state"] for row in selector_evidence.operation_rows},
                {"complete-success"},
            )
            w0_tail_checks = []
            for index, task in enumerate(pipeline_inputs["tasks"], start=1):
                operation_id = f"research-{index:02d}-base-w0"
                material = operation_materials[operation_id]["legacy_control"]
                canonical_pool = material["canonical_pool"]
                if len(task["native_order"]) > 40:
                    w0_order = selector_evidence.research_orders[task["task_id"]]["w0"]
                    expected_prefix = tuple(f"c{position}" for position in range(len(canonical_pool)))
                    self.assertEqual(w0_order[: len(expected_prefix)], expected_prefix)
                    self.assertEqual(
                        w0_order[len(expected_prefix) :], tuple(task["native_order"][len(expected_prefix) :])
                    )
                    self.assertEqual(len(w0_order), len(task["native_order"]))
                    w0_tail_checks.append(task["task_id"])
            self.assertTrue(w0_tail_checks, "fixture must exercise a W0 task with an untouched native tail")
            answer_tasks, _answer_bodies = orchestration._prepare_answer_tasks(
                plan, pipeline_inputs, prepared_packets, selector_evidence, capture_inventory
            )

            answer_result: list[object] = []

            async def request_answer():
                try:
                    answer_result.append(await source_runner._answer_authority(plan, answer_tasks))
                except Exception as exc:
                    answer_result.append(exc)

            real_request_async = source_runner.handoff.request_async

            async def bounded_request_async(**kwargs):
                kwargs["deadline_monotonic"] = min(kwargs["deadline_monotonic"], time.monotonic() + 15.0)
                return await real_request_async(**kwargs)

            source_runner.handoff.request_async = bounded_request_async
            self.addCleanup(setattr, source_runner.handoff, "request_async", real_request_async)
            answer_worker = threading.Thread(target=lambda: asyncio.run(request_answer()))
            answer_worker.start()
            self.addCleanup(answer_worker.join, 16)
            answer_request_path = handoff_root / plan.stage_uuid / "answer-execution" / "requests" / "permit.json"
            for _ in range(300):
                if answer_request_path.exists():
                    break
                time.sleep(0.005)
            self.assertTrue(answer_request_path.exists())
            answer_request_bytes = answer_request_path.read_bytes()
            answer_artifacts = handoff.load_request_artifacts(
                root=handoff_root,
                stage_uuid=plan.stage_uuid,
                scope="answer-execution",
                request_id="permit",
                request_bytes=answer_request_bytes,
                expected_source_closure_sha256=plan.source_closure_sha256,
            )
            with mock.patch.object(
                collector, "_local_clock_origin_sha256", return_value=stage_clock.clock_origin_sha256
            ):
                answer_receipt = asyncio.run(
                    authority.PhaseArtifactReceiptIssuer.answer_execution(
                        root=handoff_root,
                        stage_uuid=plan.stage_uuid,
                        request_id="permit",
                        request_bytes=answer_request_bytes,
                        expected_source_closure_sha256=plan.source_closure_sha256,
                        expected_initial_registration_sha256=source_runner.initial_registration_sha256,
                        expected_active_registration_sha256=digest(source_runner._active_registration_bytes),
                        endpoint=source_runner.config["answer_endpoint"],
                        allow_trusted_private_http=source_runner.config.get("allow_trusted_private_http", False),
                        expected_task_material_bytes=answer_artifacts["answer-task-material"],
                        forbidden_stage_uuids=tuple(source_runner.config["forbidden_stage_uuids"]),
                        plan=plan,
                        static_materials=static_materials,
                        inventory_root=inventory_root,
                        capture_receipts_root=capture_root,
                        selector_results_root=selector_results,
                        selector_archive_root=selector_archives,
                        now_monotonic=pacer.monotonic(),
                    )
                )
            response = answer_request_path.parent.parent / "responses" / "permit.receipt"
            response.write_bytes(answer_receipt)
            response.chmod(0o600)
            answer_pin = answer_request_path.parent.parent / "pins" / "permit.sha256"
            answer_pin.write_text(digest(answer_receipt) + "\n", encoding="ascii")
            answer_pin.chmod(0o600)
            answer_worker.join(timeout=16)
            self.assertFalse(answer_worker.is_alive())
            self.assertEqual(len(answer_result), 1)
            self.assertIsInstance(answer_result[0], dict)

            # Make the catalog mutation internally self-consistent with its
            # task-material digest and request artifact manifest. The issuer
            # must still reject it by rebuilding from registered source data.
            answer_envelope = json.loads(answer_request_bytes)
            task_artifact = answer_request_path.parent.parent / "artifacts" / "permit" / "answer-task-material.bin"
            original_task_bytes = task_artifact.read_bytes()
            task_doc = json.loads(original_task_bytes)
            first_catalog = base64.b64decode(task_doc["tasks"][0]["catalog_base64"], validate=True)
            changed_catalog = first_catalog + b" "
            task_doc["tasks"][0]["catalog_base64"] = base64.b64encode(changed_catalog).decode("ascii")
            task_doc["tasks"][0]["catalog_sha256"] = digest(changed_catalog)
            changed_task_bytes = canonical(task_doc)
            changed_task_artifact_manifest = dict(answer_envelope["phase_artifacts"])
            changed_task_artifact_manifest["files"] = [
                (
                    {**row, "sha256": digest(changed_task_bytes), "bytes": len(changed_task_bytes)}
                    if row["name"] == "answer-task-material"
                    else row
                )
                for row in changed_task_artifact_manifest["files"]
            ]
            changed_envelope = dict(answer_envelope)
            changed_envelope["phase_artifacts"] = changed_task_artifact_manifest
            changed_envelope["phase_artifacts_sha256"] = digest(handoff._canonical(changed_task_artifact_manifest))
            changed_request_bytes = handoff._canonical(changed_envelope)
            task_artifact.write_bytes(changed_task_bytes)
            answer_request_path.write_bytes(changed_request_bytes)
            with (
                mock.patch.object(
                    collector, "_local_clock_origin_sha256", return_value=stage_clock.clock_origin_sha256
                ),
                self.assertRaisesRegex(authority.LocalAuthorityError, "phase-artifact-answer-source-rebuild-mismatch"),
            ):
                asyncio.run(
                    authority.PhaseArtifactReceiptIssuer.answer_execution(
                        root=handoff_root,
                        stage_uuid=plan.stage_uuid,
                        request_id="permit",
                        request_bytes=changed_request_bytes,
                        expected_source_closure_sha256=plan.source_closure_sha256,
                        expected_initial_registration_sha256=source_runner.initial_registration_sha256,
                        expected_active_registration_sha256=digest(source_runner._active_registration_bytes),
                        endpoint=source_runner.config["answer_endpoint"],
                        allow_trusted_private_http=source_runner.config.get("allow_trusted_private_http", False),
                        expected_task_material_bytes=changed_task_bytes,
                        forbidden_stage_uuids=tuple(source_runner.config["forbidden_stage_uuids"]),
                        plan=plan,
                        static_materials=static_materials,
                        inventory_root=inventory_root,
                        capture_receipts_root=capture_root,
                        selector_results_root=selector_results,
                        selector_archive_root=selector_archives,
                        now_monotonic=pacer.monotonic(),
                    )
                )
            task_artifact.write_bytes(original_task_bytes)
            answer_request_path.write_bytes(answer_request_bytes)

            context_path = capture_stage / capture_rows[0]["context_artifact"]
            original_context = context_path.read_bytes()
            tampered_context = json.loads(original_context)
            tampered_context["context"] += " altered"
            context_path.write_bytes(source_capture._canonical_json(tampered_context))
            with (
                mock.patch.object(
                    collector, "_local_clock_origin_sha256", return_value=stage_clock.clock_origin_sha256
                ),
                self.assertRaises(Exception),
            ):
                asyncio.run(
                    authority.PhaseArtifactReceiptIssuer.answer_execution(
                        root=handoff_root,
                        stage_uuid=plan.stage_uuid,
                        request_id="permit",
                        request_bytes=answer_request_bytes,
                        expected_source_closure_sha256=plan.source_closure_sha256,
                        expected_initial_registration_sha256=source_runner.initial_registration_sha256,
                        expected_active_registration_sha256=digest(source_runner._active_registration_bytes),
                        endpoint=source_runner.config["answer_endpoint"],
                        allow_trusted_private_http=source_runner.config.get("allow_trusted_private_http", False),
                        expected_task_material_bytes=answer_artifacts["answer-task-material"],
                        forbidden_stage_uuids=tuple(source_runner.config["forbidden_stage_uuids"]),
                        plan=plan,
                        static_materials=static_materials,
                        inventory_root=inventory_root,
                        capture_receipts_root=capture_root,
                        selector_results_root=selector_results,
                        selector_archive_root=selector_archives,
                        now_monotonic=pacer.monotonic(),
                    )
                )
            context_path.write_bytes(original_context)
            task_artifact = answer_request_path.parent.parent / "artifacts" / "permit" / "answer-task-material.bin"
            task_original = task_artifact.read_bytes()
            task_artifact.write_bytes(task_original + b" ")
            with self.assertRaisesRegex(handoff.OperatorHandoffError, "handoff-artifact-pin-mismatch"):
                handoff.load_request_artifacts(
                    root=handoff_root,
                    stage_uuid=plan.stage_uuid,
                    scope="answer-execution",
                    request_id="permit",
                    request_bytes=answer_request_bytes,
                    expected_source_closure_sha256=plan.source_closure_sha256,
                )

    def test_capture_authority_issues_both_permits_through_file_handoff(self):
        from scripts import coverage_operator_runner as runner
        from scripts import coverage_runtime_environment as runtime_environment
        from scripts import coverage_source_capture as capture
        from tests.test_coverage_operator_runner import OperatorRunnerConfigTests

        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            config_root = base / "config"
            config_root.mkdir(mode=0o700)
            config_path, config_sha = OperatorRunnerConfigTests()._registered_companion_config(config_root)
            environment_bytes = canonical({"schema": runtime_environment.SCHEMA, "fixture": True})
            with (
                mock.patch.object(runner, "_verify_checkout_sources"),
                mock.patch.object(runner, "_require_source_bootstrap"),
                mock.patch.object(
                    runner.runtime_environment,
                    "verify_active_environment",
                    return_value=environment_bytes,
                ),
            ):
                stage_runner = runner.OperatorStageRunner.from_file(config_path, config_sha)

            plan = stage_runner.plan
            development_stage = str(uuid.UUID(int=9127))
            development_receipt = canonical(
                {
                    "schema": "coverage-study-development-gate-receipt/1",
                    "stage_uuid": development_stage,
                    "stage_kind": "development",
                    "status": "passed",
                    "protocol_sha256": digest(plan.protocol_bytes),
                    "source_revision": plan.source_revision,
                    "source_closure_sha256": plan.source_closure_sha256,
                    "result_sha256": "8" * 64,
                }
            )
            from dataclasses import replace

            plan = replace(
                plan,
                stage_kind="confirmation",
                development_receipt_bytes=development_receipt,
                development_receipt_sha256=digest(development_receipt),
                development_stage_uuid=development_stage,
            )
            stage_runner.plan = plan
            initial_registration_doc = json.loads(stage_runner.initial_registration)
            initial_registration_doc["stage_kind"] = "confirmation"
            stage_runner.initial_registration = canonical(initial_registration_doc)
            stage_runner.initial_registration_sha256 = digest(stage_runner.initial_registration)
            active_registration_doc = json.loads(stage_runner.initial_registration)
            active_registration_doc["stage_kind"] = "confirmation"
            stage_runner._active_registration_bytes = canonical(active_registration_doc)
            endpoint = stage_runner.config["candidate_base_url"]
            _health_url, _scrape_url, endpoint_sha = capture._candidate_endpoints(endpoint)
            manifest = {
                "schema": "coverage-source-capture-manifest/1",
                "stage": plan.stage_kind,
                "stage_uuid": plan.stage_uuid,
                "source_revision": plan.source_revision,
                "protocol_sha256": digest(plan.protocol_bytes),
                "cohorts_sha256": digest(plan.cohorts_bytes),
                "candidate_identity_sha256": digest(plan.candidate_identity_bytes),
                "candidate_endpoint_sha256": endpoint_sha,
                "sources": [],
            }
            manifest_bytes = capture._canonical_json(manifest)
            active_registration = json.loads(stage_runner.initial_registration)
            active_registration["pins"]["source_capture_manifest"] = digest(manifest_bytes)
            stage_runner._active_registration_bytes = canonical(active_registration)
            handoff_root = Path(stage_runner.directories["operator_handoff"])
            qualification_request_path = (
                handoff_root / plan.stage_uuid / "protected-source-capture" / "requests" / "permit.json"
            )
            source_request_path = handoff_root / plan.stage_uuid / "source-capture" / "requests" / "permit.json"
            result: list[object] = []

            async def invoke_capture_authority():
                try:
                    result.append(
                        await stage_runner._capture_authority(plan, {"capture_manifest_bytes": manifest_bytes})
                    )
                except Exception as exc:
                    result.append(exc)

            worker = threading.Thread(target=lambda: asyncio.run(invoke_capture_authority()))
            worker.start()

            def wait_for_request(path: Path) -> bytes:
                for _ in range(400):
                    if path.is_file():
                        return path.read_bytes()
                    time.sleep(0.01)
                self.fail(f"file handoff request was not created: {path.parent.parent.name}")

            qualification_request = wait_for_request(qualification_request_path)
            qualification_envelope = json.loads(qualification_request)
            expected_active_sha = digest(stage_runner._active_registration_bytes)
            qualification_args = {
                "root": handoff_root,
                "stage_uuid": plan.stage_uuid,
                "request_id": "permit",
                "expected_source_closure_sha256": plan.source_closure_sha256,
                "expected_initial_registration_sha256": stage_runner.initial_registration_sha256,
                "expected_active_registration_sha256": expected_active_sha,
                "candidate_base_url": endpoint,
                "ca_bundle_sha256": None,
                "plan": plan,
            }
            evidence = {
                "grok_image_digest": "sha256:" + "a" * 64,
                "grok_config_sha256": "b" * 64,
                "protected_profile_sha256": "c" * 64,
                "full_boundary_evidence_sha256": "d" * 64,
            }
            scope_bindings = {
                key: value
                for key, value in qualification_envelope["bindings"].items()
                if key
                not in {
                    "source_closure_sha256",
                    "active_python_environment",
                    "active_python_environment_sha256",
                    "development_receipt_sha256",
                    "development_stage_uuid",
                }
            }
            qualification_receipt = canonical(
                {
                    "schema": authority.SCHEMA,
                    "decision": "allow",
                    "scope": "protected-source-capture",
                    "bindings": scope_bindings,
                    "evidence": evidence,
                }
            )

            bad_closure = json.loads(qualification_request)
            bad_closure["bindings"]["source_closure_sha256"] = "f" * 64
            bad_closure["request_sha256"] = digest(canonical(bad_closure["bindings"]))
            with self.assertRaisesRegex(handoff.OperatorHandoffError, "handoff-artifact-binding-mismatch"):
                authority.PhaseArtifactReceiptIssuer.protected_capture_qualification(
                    request_bytes=canonical(bad_closure),
                    qualification_receipt_bytes=qualification_receipt,
                    expected_qualification_receipt_sha256=digest(qualification_receipt),
                    **qualification_args,
                )

            bad_extra = json.loads(qualification_request)
            bad_extra["bindings"]["unregistered_extra"] = "rejected"
            bad_extra["request_sha256"] = digest(canonical(bad_extra["bindings"]))
            with self.assertRaisesRegex(
                authority.LocalAuthorityError, "phase-artifact-request-context-binding-mismatch"
            ):
                authority.PhaseArtifactReceiptIssuer.protected_capture_qualification(
                    request_bytes=canonical(bad_extra),
                    qualification_receipt_bytes=qualification_receipt,
                    expected_qualification_receipt_sha256=digest(qualification_receipt),
                    **qualification_args,
                )

            bad_environment = json.loads(qualification_request)
            bad_environment["bindings"]["active_python_environment_sha256"] = "e" * 64
            bad_environment["request_sha256"] = digest(canonical(bad_environment["bindings"]))
            with self.assertRaisesRegex(
                authority.LocalAuthorityError, "phase-artifact-active-environment-binding-invalid"
            ):
                authority.PhaseArtifactReceiptIssuer.protected_capture_qualification(
                    request_bytes=canonical(bad_environment),
                    qualification_receipt_bytes=qualification_receipt,
                    expected_qualification_receipt_sha256=digest(qualification_receipt),
                    **qualification_args,
                )

            issued_qualification = authority.PhaseArtifactReceiptIssuer.protected_capture_qualification(
                request_bytes=qualification_request,
                qualification_receipt_bytes=qualification_receipt,
                expected_qualification_receipt_sha256=digest(qualification_receipt),
                **qualification_args,
            )
            qualification_scope = handoff_root / plan.stage_uuid / "protected-source-capture"
            qualification_response = qualification_scope / "responses" / "permit.receipt"
            qualification_pin = qualification_scope / "pins" / "permit.sha256"
            qualification_response.write_bytes(issued_qualification)
            qualification_response.chmod(0o600)
            qualification_pin.write_text(digest(issued_qualification) + "\n")
            qualification_pin.chmod(0o600)

            source_request = wait_for_request(source_request_path)
            issued_capture = authority.PhaseArtifactReceiptIssuer.source_capture(
                root=handoff_root,
                stage_uuid=plan.stage_uuid,
                request_id="permit",
                request_bytes=source_request,
                expected_source_closure_sha256=plan.source_closure_sha256,
                expected_initial_registration_sha256=stage_runner.initial_registration_sha256,
                expected_active_registration_sha256=expected_active_sha,
                expected_capture_manifest_bytes=manifest_bytes,
                plan=plan,
            )
            source_scope = handoff_root / plan.stage_uuid / "source-capture"
            source_response = source_scope / "responses" / "permit.receipt"
            source_pin = source_scope / "pins" / "permit.sha256"
            source_response.write_bytes(issued_capture)
            source_response.chmod(0o600)
            source_pin.write_text(digest(issued_capture) + "\n")
            source_pin.chmod(0o600)
            worker.join(timeout=5)
            self.assertFalse(worker.is_alive())
            self.assertEqual(len(result), 1)
            self.assertIsInstance(result[0], dict)
            authority_result = result[0]

            qualified = authority_result["qualification_verifier"].verify(
                authority_result["qualification_receipt_bytes"],
                authority_result["expected_qualification_receipt_sha256"],
                capture.ProtectedCaptureQualificationBindings(**scope_bindings),
            )
            admitted = authority_result["permit_verifier"].verify(
                manifest_bytes,
                authority_result["permit_receipt_bytes"],
                authority_result["expected_permit_receipt_sha256"],
            )
            self.assertEqual(qualified.status, "verified-qualified")
            self.assertEqual(admitted.status, "verified-admitted")
            self.assertEqual(admitted.manifest_sha256, digest(manifest_bytes))

    def test_phase_artifact_bundle_is_persisted_private_and_source_bound(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "handoff"
            exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
            stage = str(uuid.UUID(int=8123))
            closure = "d" * 64
            bindings = {"source_revision": "a" * 40, "source_closure_sha256": closure}
            result: list[object] = []

            def run_request():
                try:
                    result.append(
                        exchange.request(
                            stage_uuid=stage,
                            scope="selector-map-admission",
                            request_id="map-1",
                            bindings=bindings,
                            deadline_monotonic=time.monotonic() + 2,
                            artifacts={"selector-map": b"canonical-map", "protocol": b"registered-protocol"},
                        )
                    )
                except Exception as exc:
                    result.append(exc)

            worker = threading.Thread(target=run_request)
            worker.start()
            request_path = root / stage / "selector-map-admission" / "requests" / "map-1.json"
            for _ in range(100):
                if request_path.exists():
                    break
                time.sleep(0.005)
            envelope_bytes = request_path.read_bytes()
            artifacts = handoff.load_request_artifacts(
                root=root,
                stage_uuid=stage,
                scope="selector-map-admission",
                request_id="map-1",
                request_bytes=envelope_bytes,
                expected_source_closure_sha256=closure,
            )
            self.assertEqual(artifacts, {"protocol": b"registered-protocol", "selector-map": b"canonical-map"})
            artifact_dir = request_path.parent.parent / "artifacts" / "map-1"
            self.assertEqual(stat.S_IMODE(artifact_dir.stat().st_mode), 0o700)
            self.assertEqual(stat.S_IMODE((artifact_dir / "selector-map.bin").stat().st_mode), 0o600)
            receipt = canonical({"admitted": True})
            response = request_path.parent.parent / "responses" / "map-1.receipt"
            pin = request_path.parent.parent / "pins" / "map-1.sha256"
            response.write_bytes(receipt)
            response.chmod(0o600)
            pin.write_text(digest(receipt) + "\n", encoding="ascii")
            pin.chmod(0o600)
            worker.join(timeout=2)
            self.assertEqual(result, [(receipt, digest(receipt))])
            (artifact_dir / "selector-map.bin").write_bytes(b"tampered-map")
            with self.assertRaisesRegex(handoff.OperatorHandoffError, "handoff-artifact-pin-mismatch"):
                handoff.load_request_artifacts(
                    root=root,
                    stage_uuid=stage,
                    scope="selector-map-admission",
                    request_id="map-1",
                    request_bytes=envelope_bytes,
                    expected_source_closure_sha256=closure,
                )

    def test_phase_artifacts_reject_symlinks_and_oversize_payloads(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "handoff"
            root.mkdir(mode=0o700)
            stage = str(uuid.UUID(int=8124))
            with self.assertRaisesRegex(handoff.OperatorHandoffError, "handoff-artifact-over-cap"):
                handoff._artifact_manifest(
                    stage,
                    "selector-map-admission",
                    "map-1",
                    {
                        "selector-map": b"x" * (handoff.MAX_ARTIFACT_BYTES + 1),
                    },
                )
            base = root / stage / "selector-map-admission" / "artifacts" / "map-1"
            base.mkdir(mode=0o700, parents=True)
            for directory in (root / stage, root / stage / "selector-map-admission", base.parent, base):
                directory.chmod(0o700)
            secret = Path(temporary) / "outside"
            secret.write_bytes(b"linked")
            os.symlink(secret, base / "selector-map.bin")
            manifest = {
                "schema": "coverage-operator-phase-artifacts/1",
                "stage_uuid": stage,
                "scope": "selector-map-admission",
                "request_id": "map-1",
                "files": [{"name": "selector-map", "sha256": digest(b"linked"), "bytes": 6}],
            }
            bindings = {"source_closure_sha256": "e" * 64}
            envelope = {
                "schema": handoff.SCHEMA,
                "stage_uuid": stage,
                "scope": "selector-map-admission",
                "request_id": "map-1",
                "request_sha256": digest(canonical(bindings)),
                "bindings": bindings,
                "phase_artifacts": manifest,
                "phase_artifacts_sha256": digest(canonical(manifest)),
            }
            with self.assertRaisesRegex(handoff.OperatorHandoffError, "handoff-artifacts-unavailable"):
                handoff.load_request_artifacts(
                    root=root,
                    stage_uuid=stage,
                    scope="selector-map-admission",
                    request_id="map-1",
                    request_bytes=canonical(envelope),
                    expected_source_closure_sha256="e" * 64,
                )
            with self.assertRaisesRegex(handoff.OperatorHandoffError, "handoff-artifact-request-identity-invalid"):
                handoff.load_request_artifacts(
                    root=root,
                    stage_uuid=stage,
                    scope="../outside",
                    request_id="map-1",
                    request_bytes=b"{}",
                    expected_source_closure_sha256="e" * 64,
                )

    def test_waits_for_separate_operator_receipt_and_pin(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "handoff"
            exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
            stage = str(uuid.UUID(int=8001))
            bindings = {"source_revision": "a" * 40, "protocol_sha256": "b" * 64, "input_sha256": "c" * 64}
            result: list[object] = []

            def run_request():
                try:
                    result.append(
                        exchange.request(
                            stage_uuid=stage,
                            scope="selector-admission",
                            request_id="late-map-1",
                            bindings=bindings,
                            deadline_monotonic=time.monotonic() + 2,
                        )
                    )
                except Exception as exc:  # surfaced in assertion below
                    result.append(exc)

            worker = threading.Thread(target=run_request)
            worker.start()
            request_path = root / stage / "selector-admission" / "requests" / "late-map-1.json"
            for _ in range(100):
                if request_path.exists():
                    break
                time.sleep(0.005)
            self.assertTrue(request_path.is_file())
            envelope = json.loads(request_path.read_bytes())
            self.assertEqual(envelope["bindings"], bindings)
            self.assertEqual(envelope["request_sha256"], digest(canonical(bindings)))
            self.assertEqual(stat.S_IMODE(request_path.stat().st_mode), 0o600)
            self.assertEqual(stat.S_IMODE(root.stat().st_mode), 0o700)
            receipt = canonical({"externally_issued": True, "opaque": "receipt-bytes"})
            response = root / stage / "selector-admission" / "responses" / "late-map-1.receipt"
            pin = root / stage / "selector-admission" / "pins" / "late-map-1.sha256"
            response.write_bytes(receipt)
            response.chmod(0o600)
            pin.write_text(digest(receipt) + "\n", encoding="ascii")
            pin.chmod(0o600)
            worker.join(timeout=2)
            self.assertFalse(worker.is_alive())
            self.assertEqual(result, [(receipt, digest(receipt))])

    def test_selector_batch_accepts_exact_39_operation_receipt(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "handoff"
            exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
            stage = str(uuid.UUID(int=8005))
            receipt = selector_permit_batch(Path(temporary) / "fixture")
            self.assertGreater(len(receipt), handoff.MAX_RECEIPT_BYTES)
            self.assertLessEqual(len(receipt), handoff.MAX_SELECTOR_PERMIT_BATCH_BYTES)
            batch = json.loads(receipt)
            self.assertEqual(len(batch["operations"]), 39)
            permits = [json.loads(base64.b64decode(row["permit_base64"])) for row in batch["operations"]]
            self.assertEqual(
                [row["operation_id"] for row in batch["operations"]],
                [permit["operation_id"] for permit in permits],
            )
            self.assertTrue(all(permit["schema"] == "coverage-jev-operation-permit/1" for permit in permits))
            self.assertTrue(all(permit["status"] == "verified-admitted" for permit in permits))
            self.assertTrue(all(len(permit) == 10 for permit in permits))
            self.assertTrue(all(permit["parser_mode"] in {"original-v1", "coverage"} for permit in permits))
            result: list[object] = []

            def run_request():
                try:
                    result.append(
                        exchange.request(
                            stage_uuid=stage,
                            scope="selector-operation-permits",
                            request_id="batch",
                            bindings={"operation_count": 39, "selector_input_map_sha256": "f" * 64},
                            deadline_monotonic=time.monotonic() + 2,
                        )
                    )
                except Exception as exc:  # surfaced in assertions below
                    result.append(exc)

            worker = threading.Thread(target=run_request)
            worker.start()
            request_path = root / stage / "selector-operation-permits" / "requests" / "batch.json"
            for _ in range(100):
                if request_path.exists():
                    break
                time.sleep(0.005)
            response = root / stage / "selector-operation-permits" / "responses" / "batch.receipt"
            pin = root / stage / "selector-operation-permits" / "pins" / "batch.sha256"
            response.write_bytes(receipt)
            response.chmod(0o600)
            pin.write_text(digest(receipt) + "\n", encoding="ascii")
            pin.chmod(0o600)
            worker.join(timeout=2)
            self.assertFalse(worker.is_alive())
            self.assertEqual(result, [(receipt, digest(receipt))])

    def test_selector_batch_over_cap_is_terminal_and_not_reissued(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "handoff"
            exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
            stage = str(uuid.UUID(int=8006))
            kwargs = {
                "stage_uuid": stage,
                "scope": "selector-operation-permits",
                "request_id": "batch",
                "bindings": {"operation_count": 39},
                "deadline_monotonic": time.monotonic() + 2,
            }
            result: list[object] = []

            def run_request():
                try:
                    result.append(exchange.request(**kwargs))
                except Exception as exc:
                    result.append(exc)

            worker = threading.Thread(target=run_request)
            worker.start()
            response = root / stage / "selector-operation-permits" / "responses" / "batch.receipt"
            for _ in range(100):
                if response.parent.is_dir():
                    break
                time.sleep(0.005)
            oversized = b"x" * (handoff.MAX_SELECTOR_PERMIT_BATCH_BYTES + 1)
            response.write_bytes(oversized)
            response.chmod(0o600)
            pin = root / stage / "selector-operation-permits" / "pins" / "batch.sha256"
            pin.write_text(digest(oversized) + "\n", encoding="ascii")
            pin.chmod(0o600)
            worker.join(timeout=2)
            self.assertFalse(worker.is_alive())
            self.assertEqual(len(result), 1)
            self.assertIsInstance(result[0], handoff.OperatorHandoffError)
            self.assertEqual(str(result[0]), "handoff-response-over-cap")
            with self.assertRaisesRegex(handoff.OperatorHandoffError, "handoff-request-already-created-no-resume"):
                exchange.request(**kwargs)

    def test_other_receipt_scopes_keep_the_original_cap(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "handoff"
            exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
            stage = str(uuid.UUID(int=8007))
            kwargs = {
                "stage_uuid": stage,
                "scope": "selector-map-admission",
                "request_id": "map",
                "bindings": {"selector_input_map_sha256": "a" * 64},
                "deadline_monotonic": time.monotonic() + 2,
            }
            result: list[object] = []

            def run_request():
                try:
                    result.append(exchange.request(**kwargs))
                except Exception as exc:
                    result.append(exc)

            worker = threading.Thread(target=run_request)
            worker.start()
            response = root / stage / "selector-map-admission" / "responses" / "map.receipt"
            for _ in range(100):
                if response.parent.is_dir():
                    break
                time.sleep(0.005)
            oversized = b"x" * (handoff.MAX_RECEIPT_BYTES + 1)
            response.write_bytes(oversized)
            response.chmod(0o600)
            pin = root / stage / "selector-map-admission" / "pins" / "map.sha256"
            pin.write_text(digest(oversized) + "\n", encoding="ascii")
            pin.chmod(0o600)
            worker.join(timeout=2)
            self.assertFalse(worker.is_alive())
            self.assertEqual(len(result), 1)
            self.assertIsInstance(result[0], handoff.OperatorHandoffError)
            self.assertEqual(str(result[0]), "handoff-response-over-cap")

    def test_bad_pin_is_terminal_and_request_cannot_be_reissued(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "handoff"
            exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
            stage = str(uuid.UUID(int=8002))
            response = root / stage / "capture" / "responses" / "receipt-1.receipt"
            pin = root / stage / "capture" / "pins" / "receipt-1.sha256"
            (root / stage / "capture" / "responses").mkdir(parents=True, mode=0o700)
            (root / stage / "capture").chmod(0o700)
            (root / stage).chmod(0o700)
            (root / stage / "capture" / "pins").mkdir(mode=0o700)
            response.write_bytes(b"receipt")
            response.chmod(0o600)
            pin.write_text("0" * 64, encoding="ascii")
            pin.chmod(0o600)
            kwargs = {
                "stage_uuid": stage,
                "scope": "capture",
                "request_id": "receipt-1",
                "bindings": {"manifest_sha256": "a" * 64},
                "deadline_monotonic": time.monotonic() + 1,
            }
            with self.assertRaisesRegex(handoff.OperatorHandoffError, "handoff-pin-mismatch"):
                exchange.request(**kwargs)
            with self.assertRaisesRegex(handoff.OperatorHandoffError, "handoff-request-already-created-no-resume"):
                exchange.request(**kwargs)

    def test_timeout_leaves_request_and_never_claims_receipt(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "handoff"
            exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
            stage = str(uuid.UUID(int=8003))
            with self.assertRaisesRegex(handoff.OperatorHandoffError, "handoff-receipt-timeout-no-resume"):
                exchange.request(
                    stage_uuid=stage,
                    scope="answer",
                    request_id="answer-1",
                    bindings={"operation_manifest_sha256": "d" * 64},
                    deadline_monotonic=time.monotonic() + 0.07,
                )
            self.assertTrue((root / stage / "answer" / "requests" / "answer-1.json").is_file())

    def test_scope_request_and_deadline_are_strict(self):
        with tempfile.TemporaryDirectory() as temporary:
            exchange = handoff.OperatorReceiptHandoff(Path(temporary) / "handoff")
            base = {
                "stage_uuid": str(uuid.UUID(int=8004)),
                "scope": "answer",
                "request_id": "safe-id",
                "bindings": {"x": 1},
                "deadline_monotonic": time.monotonic() + 1,
            }
            for change in (
                {"scope": "../escape"},
                {"request_id": "../escape"},
                {"deadline_monotonic": time.monotonic() + handoff.MAX_WAIT_SECONDS + 1},
                {"bindings": {"bad": float("nan")}},
                {"bindings": {"api_key": "never-persist"}},
            ):
                with self.subTest(change=change), self.assertRaises(handoff.OperatorHandoffError):
                    exchange.request(**(base | change))


if __name__ == "__main__":
    unittest.main()
