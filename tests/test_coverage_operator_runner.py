from __future__ import annotations

import asyncio
import contextlib
import hashlib
import inspect
import io
import json
import os
import stat
import subprocess
import sys
import tempfile
import unittest
import uuid
from dataclasses import replace
from datetime import datetime, timedelta, timezone
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

from scripts import coverage_answer_execution as answer
from scripts import coverage_frozen_pool_inputs, coverage_live_acquire, coverage_saved_corpus
from scripts import coverage_local_authority as local_authority
from scripts import coverage_operator_runner as runner
from tests.test_coverage_answer_execution import _stage_inputs, _tasks
from tests.test_coverage_stage_orchestration import plan_fixture


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()


def digest(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


@contextlib.contextmanager
def source_bootstrap_for_test(root: Path):
    cache = root / "bootstrap-cache"
    cache.mkdir(mode=0o700)
    old_marker = getattr(sys, "_coverage_operator_source_bootstrap", None)
    old_prefix = sys.pycache_prefix
    old_dont_write = sys.dont_write_bytecode
    sys._coverage_operator_source_bootstrap = "coverage-source-bootstrap/1"
    sys.pycache_prefix = str(cache)
    sys.dont_write_bytecode = True
    try:
        yield
    finally:
        if old_marker is None:
            delattr(sys, "_coverage_operator_source_bootstrap")
        else:
            sys._coverage_operator_source_bootstrap = old_marker
        sys.pycache_prefix = old_prefix
        sys.dont_write_bytecode = old_dont_write


class OperatorRunnerConfigTests(unittest.TestCase):
    def test_selector_execution_state_comes_from_exact_terminal_inventory(self):
        from scripts import coverage_jev_execution as jev
        from scripts import intent_ranking_coverage as coverage

        compiled = coverage.compile_request(
            query="registered test query",
            purpose="test the distinction between ranking status and execution state",
            facets=[
                {"id": "f1", "description": "First distinct facet"},
                {"id": "f2", "description": "Second distinct facet"},
            ],
            candidates=[
                {"id": "c0", "title": "First", "url": "https://example.test/0", "snippet": "first"},
                {"id": "c1", "title": "Second", "url": "https://example.test/1", "snippet": "second"},
            ],
            incumbent_order=["c0", "c1"],
        )
        selected = coverage.select(
            compiled,
            coverage.ParsedResponse(
                scores={"c0": 5.0, "c1": 5.0},
                score_audit={},
                selected_facets={"c0": ("f1",), "c1": ("f2",)},
                profile_audit={},
                usage={"input_tokens": 1, "output_tokens": 1},
            ),
        )
        # Keep this state-mapping fixture independent of the prototype's
        # current facet-selection policy.
        fallback = coverage.RankingResult(compiled.incumbent_order, "fallback", "invalid_response", {})
        operation_ids = ("operation-selected", "operation-fallback", "operation-original-v1")
        prepared = SimpleNamespace(
            stage_uuid=str(uuid.uuid4()),
            source_revision="a" * 40,
            registration_sha256="b" * 64,
            pins={"protocol": "c" * 64, "qualified_source_closure": "d" * 64},
            operation_ids=operation_ids,
        )
        statuses = (selected.status, fallback.status, "complete-original-v1")
        self.assertEqual(statuses, ("selected", "fallback", "complete-original-v1"))
        results = [
            SimpleNamespace(operation_id=operation_id, status=status, terminal_reason=None)
            for operation_id, status in zip(operation_ids, statuses)
        ]
        document = {
            "schema": jev.INVENTORY_SCHEMA,
            "stage_uuid": prepared.stage_uuid,
            "source_revision": prepared.source_revision,
            "registration_sha256": prepared.registration_sha256,
            "protocol_sha256": prepared.pins["protocol"],
            "source_closure_sha256": prepared.pins["qualified_source_closure"],
            "status": "all-registered-calls-complete",
            "all_calls_complete": True,
            "input_tokens": 3,
            "output_tokens": 3,
            "operations": [
                {"operation_id": operation_id, "state": "complete-success"} for operation_id in operation_ids
            ],
            "resource_observations": [],
        }
        terminal_bytes = jev._canonical(document)
        terminal = {
            "sha256": digest(terminal_bytes),
            "bytes": len(terminal_bytes),
            "status": document["status"],
            "all_calls_complete": True,
        }
        self.assertEqual(
            runner._verified_selector_operation_states(
                prepared=prepared,
                results=results,
                terminal=terminal,
                terminal_bytes=terminal_bytes,
            ),
            {operation_id: "complete-success" for operation_id in operation_ids},
        )

        for mutate_operations in (
            lambda rows: rows[:-1],
            lambda rows: rows[:-1] + [rows[0]],
            lambda rows: [{**rows[0], "operation_id": "unregistered-operation"}, *rows[1:]],
            lambda rows: [{**rows[0], "state": "complete-invalid-response"}, *rows[1:]],
        ):
            invalid_document = {**document, "operations": mutate_operations(document["operations"])}
            invalid_bytes = jev._canonical(invalid_document)
            invalid_terminal = {**terminal, "sha256": digest(invalid_bytes), "bytes": len(invalid_bytes)}
            with self.assertRaises(runner.OperatorRunnerError):
                runner._verified_selector_operation_states(
                    prepared=prepared,
                    results=results,
                    terminal=invalid_terminal,
                    terminal_bytes=invalid_bytes,
                )

        with self.assertRaisesRegex(runner.OperatorRunnerError, "selector-result-inventory-mismatch"):
            runner._verified_selector_operation_states(
                prepared=prepared,
                results=results[:-1],
                terminal=terminal,
                terminal_bytes=terminal_bytes,
            )
        for invalid_results in (
            [*results[:-1], results[0]],
            [SimpleNamespace(operation_id="unregistered-operation"), *results[1:]],
        ):
            with self.assertRaisesRegex(runner.OperatorRunnerError, "selector-result-inventory-mismatch"):
                runner._verified_selector_operation_states(
                    prepared=prepared,
                    results=invalid_results,
                    terminal=terminal,
                    terminal_bytes=terminal_bytes,
                )

        for invalid_first_result in (
            SimpleNamespace(operation_id=operation_ids[0], status="terminal-failure", terminal_reason=None),
            SimpleNamespace(
                operation_id=operation_ids[0], status=results[0].status, terminal_reason="unexpected-failure"
            ),
            SimpleNamespace(operation_id=operation_ids[0], status="unknown-success-label", terminal_reason=None),
            SimpleNamespace(operation_id=operation_ids[0], terminal_reason=None),
            SimpleNamespace(operation_id=operation_ids[0], status=results[0].status),
        ):
            with self.assertRaisesRegex(runner.OperatorRunnerError, "selector-result-outcome-invalid"):
                runner._verified_selector_operation_states(
                    prepared=prepared,
                    results=[invalid_first_result, *results[1:]],
                    terminal=terminal,
                    terminal_bytes=terminal_bytes,
                )

    def test_acquisition_authority_uses_exact_executor_stage_manifest_digest(self):
        plan = plan_fixture()
        manifest = json.loads(plan.acquisition_manifest_bytes)
        captured = {}

        async def request(_self, scope, request_id, bindings):
            self.assertEqual((scope, request_id), ("acquisition", "permit"))
            captured.update(bindings)
            receipt_bindings = {
                key: manifest[key]
                for key in (
                    "stage_uuid",
                    "source_revision",
                    "source_closure_sha256",
                    "protocol_sha256",
                    "cohorts_sha256",
                    "input_manifest_sha256",
                    "acquisition_plan_sha256",
                )
            }
            receipt_bindings["stage_manifest_sha256"] = bindings["stage_manifest_sha256"]
            raw = local_authority._canonical(
                {
                    "schema": local_authority.SCHEMA,
                    "decision": "allow",
                    "scope": "source-acquisition",
                    "bindings": receipt_bindings,
                }
            )
            return raw, digest(raw)

        instance = object.__new__(runner.OperatorStageRunner)
        with tempfile.TemporaryDirectory() as temporary:
            instance.directories = {"lease_root": temporary, "stage_inventory": temporary}
            with mock.patch.object(runner.OperatorStageRunner, "_request", new=request):
                authority = asyncio.run(runner.OperatorStageRunner._acquisition_authority(instance, plan))

        executor_digest = digest(coverage_live_acquire._canonical(manifest))
        self.assertEqual(plan.acquisition_manifest_bytes[-1:], b"\n")
        self.assertEqual(digest(plan.acquisition_manifest_bytes), executor_digest)
        self.assertEqual(plan.expected_acquisition_manifest_sha256, executor_digest)
        self.assertEqual(captured["acquisition_manifest_sha256"], executor_digest)
        self.assertEqual(captured["stage_manifest_sha256"], executor_digest)
        self.assertEqual(digest(plan.acquisition_manifest_bytes), captured["stage_manifest_sha256"])
        permit = authority["permit_verifier"].verify(
            manifest,
            captured["stage_manifest_sha256"],
            authority["permit_receipt_bytes"],
            authority["expected_permit_receipt_sha256"],
        )
        self.assertEqual(permit.stage_manifest_sha256, executor_digest)

    def _config(self, root: Path, protocol: bytes) -> tuple[Path, str]:
        path_values = {key: root / f"{key}.json" for key in runner._PATH_KEYS}
        for key, path in path_values.items():
            path.write_bytes(protocol if key == "protocol" else b"{}")
        private_paths = {key: root / f"{key}.secret" for key in runner._PRIVATE_PATH_KEYS}
        for path in private_paths.values():
            path.write_bytes(b"synthetic-test-only\n")
            path.chmod(0o600)
        directories = {key: str(root / key) for key in runner._DIRECTORY_KEYS}
        value = {
            "schema": runner.CONFIG_SCHEMA,
            "stage_uuid": str(uuid.UUID(int=501)),
            "packet_stage_uuid": str(uuid.UUID(int=502)),
            "stage_kind": "development",
            "source_revision": "a" * 40,
            "candidate_base_url": "https://candidate.example",
            "answer_endpoint": "https://model.example/v1/chat/completions",
            "initial_registration_sha256": "1" * 64,
            "forbidden_stage_uuids": [str(uuid.UUID(int=100))],
            "paths": {key: str(path) for key, path in path_values.items()},
            "private_paths": {key: str(path) for key, path in private_paths.items()},
            "directories": directories,
        }
        config_path = root / "operator-config.json"
        raw = canonical(value)
        config_path.write_bytes(raw)
        config_path.chmod(0o600)
        return config_path, digest(raw)

    def _registered_config(self, root: Path) -> tuple[Path, str, dict[str, str]]:
        """Make a synthetic, internally pinned config that reaches root prep."""
        bundle = Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study/registered-2026-10-11"
        stage_dir = bundle / "development"
        external_paths = {
            "cohorts": bundle / "cohorts.json",
            "protocol": bundle / "protocol.json",
            "qualified_source_closure": bundle / "current-qualified-source-closure.json",
            "coverage_source": Path(__file__).parents[1] / "scripts/intent_ranking_coverage.py",
            "production_rerank_source": Path(__file__).parents[1] / "slopsearx/rerank.py",
            "dependency_lock": Path(__file__).parents[1] / "uv.lock",
            "reference_manifest": stage_dir / "reference-manifest-shell.json",
            "answer_assessment_plan": stage_dir / "answer-assessment-plan-shell.json",
            "capture_plan": stage_dir / "capture-plan.json",
            "candidate_identity": bundle / "candidate-runtime-identity.json",
        }
        config_path, _ = self._config(root, external_paths["protocol"].read_bytes())
        config = json.loads(config_path.read_bytes())
        config["source_revision"] = subprocess.run(
            ["git", "rev-parse", "--verify", "HEAD^{commit}"],
            cwd=runner._REPOSITORY_ROOT,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
        config["candidate_base_url"] = "https://candidate.example"
        stage_uuid = config["stage_uuid"]
        path_values = {key: Path(value) for key, value in config["paths"].items()}
        material_bytes = {key: path.read_bytes() for key, path in external_paths.items()}
        material_bytes["candidate_identity"] = runner.capture._canonical_json(
            runner.core._strict_json(material_bytes["candidate_identity"], "test-candidate-identity")
        )
        for key in ("coverage_source", "production_rerank_source", "dependency_lock"):
            path_values[key] = external_paths[key]
        material_bytes["qualified_source_closure"] = runner._canonical(
            {
                "schema": "coverage-study-qualified-source-closure/1",
                "source_revision": config["source_revision"],
                "material_pins": {
                    "coverage_source": digest(material_bytes["coverage_source"]),
                    "production_rerank_source": digest(material_bytes["production_rerank_source"]),
                    "dependency_lock": digest(material_bytes["dependency_lock"]),
                },
            }
        )
        for key, raw in material_bytes.items():
            if key not in {"coverage_source", "production_rerank_source", "dependency_lock"}:
                path_values[key].write_bytes(raw)

        cohorts = runner.core._strict_json(material_bytes["cohorts"], "test-cohorts")
        stage_row = runner._stage_cohort(cohorts, config["stage_kind"])
        acquisition_plan = runner._build_acquisition_plan(
            stage_uuid, stage_row["research_cases"], stage_row["navigation_targets"]
        )
        static_materials = {
            "protocol": material_bytes["protocol"],
            "qualified_source_closure": material_bytes["qualified_source_closure"],
            "coverage_source": material_bytes["coverage_source"],
            "production_rerank_source": material_bytes["production_rerank_source"],
            "dependency_lock": material_bytes["dependency_lock"],
            "reference_manifest": material_bytes["reference_manifest"],
            "answer_assessment_plan": material_bytes["answer_assessment_plan"],
            "acquisition_plan": acquisition_plan,
            "capture_plan": material_bytes["capture_plan"],
        }
        registration = runner._canonical(
            {
                "schema": runner.core.REGISTRATION_SCHEMA,
                "stage_uuid": stage_uuid,
                "stage_kind": config["stage_kind"],
                "source_revision": config["source_revision"],
                "pins": {
                    **{key: digest(raw) for key, raw in static_materials.items()},
                    "task_input_manifest": "b" * 64,
                    "source_capture_manifest": "c" * 64,
                },
                "task_ids": [row["task_id"] for row in stage_row["research_cases"]]
                + [row["target_id"] for row in stage_row["navigation_targets"]],
                "navigation_skip_reasons": [None] * len(stage_row["navigation_targets"]),
                "operation_ids": [call.operation_id for call in runner.core.selector_inventory([None] * 5)],
            }
        )
        path_values["initial_registration"].write_bytes(registration)
        config["initial_registration_sha256"] = digest(registration)
        config["paths"] = {key: str(path) for key, path in path_values.items()}
        raw_config = canonical(config)
        config_path.write_bytes(raw_config)
        config_path.chmod(0o600)
        return config_path, digest(raw_config), config["directories"]

    def _registered_companion_config(
        self, root: Path, *, stage_kind: str = "development", schema4: bool = False
    ) -> tuple[Path, str]:
        """Build real registered-v2 materials that traverse runner preflight."""
        repo = Path(__file__).parents[1]
        evidence_dir = repo / "docs/experiments/evidence/coverage-first-study"
        bundle = evidence_dir / ("research-task-use-method-materials" if schema4 else "frozen-input-method-materials")
        stage_dir = evidence_dir / "prospective-2026-10-13-band-companion-v3" / stage_kind
        old_closure_placeholder = evidence_dir / "registered-2026-10-12-acquisition-v2"
        fresh_stage_bundle = evidence_dir if schema4 else evidence_dir / "prospective-2026-10-13-band-companion-v3"
        external_paths = {
            "cohorts": bundle / "cohorts.json",
            "protocol": bundle / "protocol.json",
            # The function replaces this placeholder with a closure rebuilt
            # from the exact checkout sources and the fresh frozen contract.
            "qualified_source_closure": old_closure_placeholder / "current-qualified-source-closure.json",
            "coverage_source": repo / "scripts/intent_ranking_coverage.py",
            "production_rerank_source": evidence_dir / "w0-rerank-v1.py.txt",
            "dependency_lock": repo / "uv.lock",
            "reference_manifest": stage_dir / "reference-manifest-shell.json",
            "answer_assessment_plan": stage_dir / "answer-assessment-plan-shell.json",
            "capture_plan": stage_dir / "capture-plan.json",
            "candidate_identity": fresh_stage_bundle / "candidate-runtime-identity.json",
        }
        config_path, _ = self._config(root, external_paths["protocol"].read_bytes())
        config = json.loads(config_path.read_bytes())
        config["stage_kind"] = stage_kind
        if schema4:
            config["stage_uuid"] = str(uuid.uuid4())
        # Exercise stage-bound shells with the configured registered UUID.
        if not schema4:
            config["stage_uuid"] = json.loads(external_paths["reference_manifest"].read_bytes())["stage_uuid"]
        config["source_revision"] = subprocess.run(
            ["git", "rev-parse", "--verify", "HEAD^{commit}"],
            cwd=repo,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
        path_values = {key: Path(value) for key, value in config["paths"].items()}
        material_bytes = {key: path.read_bytes() for key, path in external_paths.items()}
        material_bytes["candidate_identity"] = runner.capture._canonical_json(
            runner.core._strict_json(material_bytes["candidate_identity"], "test-candidate-identity")
        )
        if schema4:
            for key in ("reference_manifest", "answer_assessment_plan", "capture_plan"):
                document = runner.core._strict_json(material_bytes[key], f"test-{key}")
                document["stage_uuid"] = config["stage_uuid"]
                material_bytes[key] = runner._canonical(document)
        for key in ("coverage_source", "production_rerank_source", "dependency_lock"):
            path_values[key] = external_paths[key]

        companion_path = root / "band-companion.json"
        material_bytes["band_companion"] = (bundle / "band-companion-contract.json").read_bytes()
        companion_path.write_bytes(material_bytes["band_companion"])
        path_values["band_companion"] = companion_path

        source_names = {
            "coverage_study_core": "scripts/coverage_study_core.py",
            "coverage_gate_calculation": "scripts/coverage_gate_calculation.py",
            "coverage_stage_orchestration": "scripts/coverage_stage_orchestration.py",
            "coverage_stage_finalization": "scripts/coverage_stage_finalization.py",
            "coverage_operator_runner": "scripts/coverage_operator_runner.py",
            "coverage_operator_handoff": "scripts/coverage_operator_handoff.py",
            "coverage_atomic_io": "scripts/coverage_atomic_io.py",
            "coverage_stage_runtime": "scripts/coverage_stage_runtime.py",
            "coverage_local_authority": "scripts/coverage_local_authority.py",
            "coverage_frozen_pool_inputs": "scripts/coverage_frozen_pool_inputs.py",
            "coverage_frozen_pool_collect": "scripts/coverage_frozen_pool_collect.py",
            "coverage_resource_evidence": "scripts/coverage_resource_evidence.py",
            "coverage_pipeline_inputs": "scripts/coverage_pipeline_inputs.py",
            "coverage_selector_input_map": "scripts/coverage_selector_input_map.py",
            "coverage_band_companion": "scripts/coverage_band_companion.py",
            "coverage_late_registration": "scripts/coverage_late_registration.py",
        }
        material_bytes["qualified_source_closure"] = runner._canonical(
            {
                "schema": "coverage-study-qualified-source-closure/2",
                "source_revision": config["source_revision"],
                "material_pins": {
                    "coverage_source": digest(material_bytes["coverage_source"]),
                    "production_rerank_source": digest(material_bytes["production_rerank_source"]),
                    "dependency_lock": digest(material_bytes["dependency_lock"]),
                    "band_companion": digest(material_bytes["band_companion"]),
                },
                "source_pins": {
                    name: digest((repo / relative).read_bytes()) for name, relative in source_names.items()
                },
            }
        )
        for key, raw in material_bytes.items():
            if key not in {"coverage_source", "production_rerank_source", "dependency_lock"}:
                path_values[key].write_bytes(raw)

        cohorts = runner.core._strict_json(material_bytes["cohorts"], "test-cohorts")
        stage_row = runner._stage_cohort(cohorts, config["stage_kind"])
        capture_plan = runner.core._strict_json(material_bytes["capture_plan"], "test-capture-plan")
        capture_ids = [row["task_id"] for row in stage_row["research_cases"]] + [
            row["target_id"] for row in stage_row["navigation_targets"]
        ]
        if type(capture_plan) is not dict or type(capture_plan.get("tasks")) is not list:
            raise AssertionError("registered capture fixture is malformed")
        capture_plan["tasks"] = [
            {**row, "task_id": task_id} for row, task_id in zip(capture_plan["tasks"], capture_ids, strict=True)
        ]
        material_bytes["capture_plan"] = runner._canonical(capture_plan)
        path_values["capture_plan"].write_bytes(material_bytes["capture_plan"])
        acquisition_plan = runner._build_acquisition_plan(
            config["stage_uuid"], stage_row["research_cases"], stage_row["navigation_targets"]
        )
        static_materials = {
            "protocol": material_bytes["protocol"],
            "qualified_source_closure": material_bytes["qualified_source_closure"],
            "coverage_source": material_bytes["coverage_source"],
            "production_rerank_source": material_bytes["production_rerank_source"],
            "dependency_lock": material_bytes["dependency_lock"],
            "reference_manifest": material_bytes["reference_manifest"],
            "answer_assessment_plan": material_bytes["answer_assessment_plan"],
            "acquisition_plan": acquisition_plan,
            "capture_plan": material_bytes["capture_plan"],
        }
        registration = runner._canonical(
            {
                "schema": runner.core.REGISTRATION_SCHEMA,
                "stage_uuid": config["stage_uuid"],
                "stage_kind": config["stage_kind"],
                "source_revision": config["source_revision"],
                "pins": {
                    **{key: digest(raw) for key, raw in static_materials.items()},
                    "task_input_manifest": "b" * 64,
                    "source_capture_manifest": "c" * 64,
                },
                "task_ids": [row["task_id"] for row in stage_row["research_cases"]]
                + [row["target_id"] for row in stage_row["navigation_targets"]],
                "navigation_skip_reasons": [None] * len(stage_row["navigation_targets"]),
                "operation_ids": [call.operation_id for call in runner.core.selector_inventory([None] * 5)],
                "band_companion_sha256": digest(material_bytes["band_companion"]),
            }
        )
        path_values["initial_registration"].write_bytes(registration)
        config["initial_registration_sha256"] = digest(registration)
        config["paths"] = {key: str(path) for key, path in path_values.items()}
        raw_config = canonical(config)
        config_path.write_bytes(raw_config)
        config_path.chmod(0o600)
        return config_path, digest(raw_config)

    def test_registered_v2_companion_runner_preflight_binds_closure_registration_and_plan(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config_path, config_sha = self._registered_companion_config(root)
            with (
                mock.patch.object(runner, "_verify_checkout_sources"),
                mock.patch.object(runner, "_require_source_bootstrap"),
                mock.patch.object(runner.runtime_environment, "verify_active_environment", return_value=b"{}"),
            ):
                instance = runner.OperatorStageRunner.preflight_from_file(config_path, config_sha)

            study_dir = Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study"
            registered_dir = study_dir / "frozen-input-method-materials"
            companion_bytes = (registered_dir / "band-companion-contract.json").read_bytes()
            registration_bytes = Path(
                json.loads(config_path.read_bytes())["paths"]["initial_registration"]
            ).read_bytes()
            self.assertEqual(instance.plan.protocol_bytes, (registered_dir / "protocol.json").read_bytes())
            self.assertEqual(instance.plan.cohorts_bytes, (registered_dir / "cohorts.json").read_bytes())
            self.assertEqual(instance.plan.band_companion_bytes, companion_bytes)
            self.assertEqual(instance.plan.band_companion_sha256, digest(companion_bytes))
            self.assertEqual(instance.plan.initial_registration_bytes, registration_bytes)
            self.assertEqual(instance.plan.initial_registration_sha256, digest(registration_bytes))
            self.assertEqual(instance.preflight_validation["band_companion_sha256"], digest(companion_bytes))
            self.assertEqual(instance.preflight_validation["initial_registration_sha256"], digest(registration_bytes))

    def test_registered_v4_frozen_pool_preflight_accepts_both_fresh_stage_kinds(self):
        for stage_kind in ("development", "confirmation"):
            with self.subTest(stage_kind=stage_kind), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                config_path, config_sha = self._registered_companion_config(root, stage_kind=stage_kind, schema4=True)
                with (
                    mock.patch.object(runner, "_verify_checkout_sources"),
                    mock.patch.object(runner, "_require_source_bootstrap"),
                    mock.patch.object(runner.runtime_environment, "verify_active_environment", return_value=b"{}"),
                ):
                    instance = runner.OperatorStageRunner.preflight_from_file(config_path, config_sha)

                self.assertEqual(instance.plan.stage_kind, stage_kind)
                self.assertEqual(instance.plan.pool_input_mode, "frozen-pool/1")
                self.assertIsNone(instance.saved_origin)
                closure = runner.core._strict_json(
                    instance.static_materials["qualified_source_closure"], "test-v4-source-closure"
                )
                self.assertEqual(closure["schema"], "coverage-study-qualified-source-closure/2")
                self.assertEqual(len(closure["source_pins"]), 16)

    def test_saved_import_preflight_uses_orchestration_canonical_manifest_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config_path, _ = self._registered_companion_config(root)
            config = json.loads(config_path.read_bytes())
            repo = Path(__file__).parents[1]
            evidence_dir = repo / "docs/experiments/evidence/coverage-first-study"
            material_dir = evidence_dir / "saved-development-method-materials"
            registered_dir = evidence_dir / "frozen-input-static-registration-2026-10-13/development"
            stage_uuid = json.loads((registered_dir / "reference-manifest-shell.json").read_bytes())["stage_uuid"]
            config["stage_uuid"] = stage_uuid
            config["packet_stage_uuid"] = str(uuid.UUID(int=1502))
            config["paths"]["protocol"] = str(material_dir / "protocol.json")
            config["paths"]["cohorts"] = str(material_dir / "cohorts.json")
            config["paths"]["band_companion"] = str(material_dir / "band-companion-contract.json")
            config["paths"]["reference_manifest"] = str(registered_dir / "reference-manifest-shell.json")
            config["paths"]["answer_assessment_plan"] = str(registered_dir / "answer-assessment-plan-shell.json")
            config["paths"]["capture_plan"] = str(registered_dir / "capture-plan.json")
            config["saved_origin"] = {"fixture": "resolver-isolated"}

            protocol_bytes = Path(config["paths"]["protocol"]).read_bytes()
            cohorts_bytes = Path(config["paths"]["cohorts"]).read_bytes()
            protocol = runner.core._strict_json(protocol_bytes, "saved-test-protocol")
            cohorts = runner.core._strict_json(cohorts_bytes, "saved-test-cohorts")
            stage_row = runner._stage_cohort(cohorts, "development")
            cases = stage_row["research_cases"]
            targets = stage_row["navigation_targets"]
            acquisition_plan_doc = runner.core._strict_json(
                runner._build_acquisition_plan(stage_uuid, cases, targets), "saved-test-acquisition-plan"
            )
            for row in acquisition_plan_doc["tasks"]:
                row["engine_calls"] = {name: 0 for name in row["engine_calls"]}
            acquisition_plan = runner._canonical(acquisition_plan_doc)

            path_values = {key: Path(value) for key, value in config["paths"].items()}
            material_bytes = {key: path.read_bytes() for key, path in path_values.items()}
            material_bytes["candidate_identity"] = runner.capture._canonical_json(
                runner.core._strict_json(material_bytes["candidate_identity"], "saved-test-candidate")
            )
            companion_sha = digest(material_bytes["band_companion"])
            source_names = {
                "coverage_study_core": "scripts/coverage_study_core.py",
                "coverage_gate_calculation": "scripts/coverage_gate_calculation.py",
                "coverage_stage_orchestration": "scripts/coverage_stage_orchestration.py",
                "coverage_stage_finalization": "scripts/coverage_stage_finalization.py",
                "coverage_operator_runner": "scripts/coverage_operator_runner.py",
                "coverage_operator_handoff": "scripts/coverage_operator_handoff.py",
                "coverage_atomic_io": "scripts/coverage_atomic_io.py",
                "coverage_stage_runtime": "scripts/coverage_stage_runtime.py",
                "coverage_local_authority": "scripts/coverage_local_authority.py",
                "coverage_frozen_pool_inputs": "scripts/coverage_frozen_pool_inputs.py",
                "coverage_frozen_pool_collect": "scripts/coverage_frozen_pool_collect.py",
                "coverage_resource_evidence": "scripts/coverage_resource_evidence.py",
                "coverage_pipeline_inputs": "scripts/coverage_pipeline_inputs.py",
                "coverage_selector_input_map": "scripts/coverage_selector_input_map.py",
                "coverage_band_companion": "scripts/coverage_band_companion.py",
                "coverage_late_registration": "scripts/coverage_late_registration.py",
                "coverage_saved_corpus": "scripts/coverage_saved_corpus.py",
                "coverage_assessment_checkpoint": "scripts/coverage_assessment_checkpoint.py",
            }
            material_bytes["qualified_source_closure"] = runner._canonical(
                {
                    "schema": "coverage-study-qualified-source-closure/2",
                    "source_revision": config["source_revision"],
                    "material_pins": {
                        "coverage_source": digest(material_bytes["coverage_source"]),
                        "production_rerank_source": digest(material_bytes["production_rerank_source"]),
                        "dependency_lock": digest(material_bytes["dependency_lock"]),
                        "band_companion": companion_sha,
                    },
                    "source_pins": {
                        name: digest((repo / relative).read_bytes()) for name, relative in source_names.items()
                    },
                }
            )
            for key in ("qualified_source_closure", "candidate_identity"):
                path_values[key].write_bytes(material_bytes[key])
            static_materials = {
                "protocol": material_bytes["protocol"],
                "qualified_source_closure": material_bytes["qualified_source_closure"],
                "coverage_source": material_bytes["coverage_source"],
                "production_rerank_source": material_bytes["production_rerank_source"],
                "dependency_lock": material_bytes["dependency_lock"],
                "reference_manifest": material_bytes["reference_manifest"],
                "answer_assessment_plan": material_bytes["answer_assessment_plan"],
                "acquisition_plan": acquisition_plan,
                "capture_plan": material_bytes["capture_plan"],
            }
            registration = runner._canonical(
                {
                    "schema": runner.core.REGISTRATION_SCHEMA,
                    "stage_uuid": stage_uuid,
                    "stage_kind": "development",
                    "source_revision": config["source_revision"],
                    "pins": {
                        **{key: digest(raw) for key, raw in static_materials.items()},
                        "task_input_manifest": "b" * 64,
                        "source_capture_manifest": "c" * 64,
                    },
                    "task_ids": [row["task_id"] for row in cases] + [row["target_id"] for row in targets],
                    "navigation_skip_reasons": [None] * len(targets),
                    "operation_ids": [call.operation_id for call in runner.core.selector_inventory([None] * 5)],
                    "band_companion_sha256": companion_sha,
                }
            )
            path_values["initial_registration"].write_bytes(registration)
            config["initial_registration_sha256"] = digest(registration)
            config["paths"] = {key: str(path) for key, path in path_values.items()}
            config_path.write_bytes(canonical(config))
            config_path.chmod(0o600)

            origin_doc = protocol["saved_development_origin"]
            origin_collection = root / "origin-collection"
            origin_capture = root / "origin-capture"
            origin_collection.mkdir(mode=0o700)
            origin_capture.mkdir(mode=0o700)
            origin_plan = b'{"schema":"coverage-acquisition-plan/1","operations":[]}'
            (origin_collection / "acquisition-plan.json").write_bytes(origin_plan)
            (origin_collection / "acquisition-plan.json").chmod(0o600)
            origin_source_dir = root / "origin-source"
            origin_source_dir.mkdir(mode=0o700)
            origin_source_bytes = b"# historical operation runner\n"
            origin_source_path = origin_source_dir / "coverage_study_acquire.py"
            origin_source_path.write_bytes(origin_source_bytes)
            origin_source_path.chmod(0o600)
            origin_registration = coverage_frozen_pool_inputs.FrozenPoolRegistrationContext(
                stage_uuid=origin_doc["stage_uuid"],
                stage_kind="development",
                source_revision=origin_doc["source_revision"],
                source_closure_sha256="1" * 64,
                protocol_sha256="2" * 64,
                cohorts_sha256="3" * 64,
                candidate_identity_sha256="4" * 64,
                candidate_endpoint_sha256="5" * 64,
                acquisition_plan_sha256=digest(origin_plan),
                task_ids=tuple(row["task_id"] for row in cases),
                navigation_ids=tuple(row["target_id"] for row in targets),
                evidence_sha256="7" * 64,
                collector_receipt_sha256="8" * 64,
                availability_sha256="9" * 64,
            )
            origin = coverage_saved_corpus.SavedCorpusOrigin(
                registration=origin_registration,
                research_cases=tuple(cases),
                navigation_targets=tuple(targets),
                collection_directory=origin_collection,
                expected_collection_receipt_inventory_sha256="a" * 64,
                capture_stage_directory=origin_capture,
                expected_capture_inventory_sha256="b" * 64,
                expected_capture_manifest_sha256="c" * 64,
                expected_package_sha256=origin_doc["package_sha256"],
                operation_runner_source_sidecar=origin_source_path,
                operation_runner_source_sha256=digest(origin_source_bytes),
            )
            with (
                mock.patch.object(runner, "_verify_checkout_sources"),
                mock.patch.object(runner, "_require_source_bootstrap"),
                mock.patch.object(runner, "_saved_origin_from_config", return_value=origin),
                mock.patch.object(runner.runtime_environment, "verify_active_environment", return_value=b"{}"),
            ):
                instance = runner.OperatorStageRunner.preflight_from_file(config_path, digest(config_path.read_bytes()))

            saved_manifest = runner.core._strict_json(
                instance.plan.acquisition_manifest_bytes, "saved-test-acquisition-manifest"
            )
            self.assertEqual(instance.plan.pool_input_mode, "saved-corpus/1")
            self.assertEqual(
                instance.preflight_validation["acquisition_manifest_sha256"],
                digest(instance.plan.acquisition_manifest_bytes),
            )
            self.assertFalse(instance.plan.acquisition_manifest_bytes.endswith(b"\n"))
            self.assertEqual(
                instance.plan.acquisition_manifest_bytes,
                runner.orchestration._canonical(saved_manifest),
            )

    def test_unregistered_or_draft_protocol_fails_before_clock_or_handoff_side_effects(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            protocol = canonical({"selector_input_map_schema": "coverage-selector-input-map/2-draft"})
            config, pin = self._config(root, protocol)
            with (
                mock.patch.object(runner.time, "monotonic", side_effect=AssertionError("clock must not start")),
                mock.patch.object(
                    runner, "_prepare_private_directories", side_effect=AssertionError("no root preparation")
                ),
                mock.patch.object(
                    runner.operator_handoff, "OperatorReceiptHandoff", side_effect=AssertionError("no handoff")
                ),
                mock.patch.object(runner, "_verify_checkout_sources"),
                mock.patch.object(runner, "_require_source_bootstrap"),
            ):
                with self.assertRaisesRegex(runner.OperatorRunnerError, "registered-source-materials-required"):
                    runner.OperatorStageRunner.from_file(config, pin)
            self.assertFalse((root / "operator_handoff").exists())
            configured_directories = json.loads(config.read_bytes())["directories"].values()
            self.assertTrue(all(not Path(path).exists() for path in configured_directories))

    def test_source_closure_companion_registration_cannot_be_omitted_or_invented(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config_path, _pin, _directories = self._registered_config(root)
            config = json.loads(config_path.read_bytes())
            closure_path = Path(config["paths"]["qualified_source_closure"])
            closure = json.loads(closure_path.read_bytes())
            closure["schema"] = "coverage-study-qualified-source-closure/2"
            closure["source_pins"] = {}
            closure_path.write_bytes(canonical(closure))
            raw_config = canonical(config)
            config_path.write_bytes(raw_config)
            config_path.chmod(0o600)

            with (
                mock.patch.object(runner, "_verify_checkout_sources"),
                mock.patch.object(runner, "_require_source_bootstrap"),
            ):
                with self.assertRaisesRegex(runner.OperatorRunnerError, "source-closure-companion-path-mismatch"):
                    runner.OperatorStageRunner.preflight_from_file(config_path, digest(raw_config))

            config = json.loads(config_path.read_bytes())
            closure = json.loads(closure_path.read_bytes())
            closure["schema"] = "coverage-study-qualified-source-closure/1"
            closure.pop("source_pins")
            closure_path.write_bytes(canonical(closure))
            companion_path = root / "band-companion.json"
            companion_path.write_bytes(b"{}")
            config["paths"]["band_companion"] = str(companion_path)
            raw_config = canonical(config)
            config_path.write_bytes(raw_config)
            config_path.chmod(0o600)
            with (
                mock.patch.object(runner, "_verify_checkout_sources"),
                mock.patch.object(runner, "_require_source_bootstrap"),
            ):
                with self.assertRaisesRegex(runner.OperatorRunnerError, "source-closure-companion-path-mismatch"):
                    runner.OperatorStageRunner.preflight_from_file(config_path, digest(raw_config))

    def test_frozen_pool_operator_rejects_closure_v1_without_companion(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config_path, _pin, _directories = self._registered_config(root)
            config = json.loads(config_path.read_bytes())
            frozen_materials = (
                Path(__file__).parents[1]
                / "docs/experiments/evidence/coverage-first-study/frozen-input-method-materials"
            )
            Path(config["paths"]["protocol"]).write_bytes((frozen_materials / "protocol.json").read_bytes())
            Path(config["paths"]["cohorts"]).write_bytes((frozen_materials / "cohorts.json").read_bytes())
            raw_config = canonical(config)
            config_path.write_bytes(raw_config)
            config_path.chmod(0o600)

            with (
                mock.patch.object(runner, "_verify_checkout_sources"),
                mock.patch.object(runner, "_require_source_bootstrap"),
            ):
                with self.assertRaisesRegex(runner.OperatorRunnerError, "frozen-pool-source-closure-v2-required"):
                    runner.OperatorStageRunner.preflight_from_file(config_path, digest(raw_config))

    def test_registered_config_creates_all_private_roots_before_clock(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config, pin, directories = self._registered_config(root)

            def clock_after_root_preparation():
                for value in directories.values():
                    info = Path(value).lstat()
                    self.assertTrue(stat.S_ISDIR(info.st_mode))
                    self.assertFalse(stat.S_ISLNK(info.st_mode))
                    self.assertEqual(stat.S_IMODE(info.st_mode), 0o700)
                    self.assertEqual(info.st_uid, os.geteuid())
                return 123.0

            with (
                mock.patch.object(runner, "_verify_checkout_sources"),
                mock.patch.object(runner, "_require_source_bootstrap"),
                mock.patch.object(runner.core, "_verify_qualified_source_closure"),
                mock.patch.object(runner.runtime_environment, "verify_active_environment", return_value=b"{}"),
                mock.patch.object(runner.orchestration, "_validate_plan"),
                mock.patch.object(runner.time, "monotonic", side_effect=clock_after_root_preparation),
            ):
                instance = runner.OperatorStageRunner.from_file(config, pin)

            self.assertEqual(instance.started_monotonic, 123.0)
            self.assertEqual(set(directories), runner._DIRECTORY_KEYS)

    def test_from_file_rounds_deadline_toward_start_at_float_boundaries(self):
        starts = (60_000.123456789, 65_535.123456789, 1_234.5)
        for start in starts:
            with self.subTest(start=start), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                config, pin, _directories = self._registered_config(root)
                clock_calls = 0

                def clock():
                    nonlocal clock_calls
                    clock_calls += 1
                    return start if clock_calls == 1 else start + 1.0

                with (
                    mock.patch.object(runner, "_verify_checkout_sources"),
                    mock.patch.object(runner, "_require_source_bootstrap"),
                    mock.patch.object(runner.core, "_verify_qualified_source_closure"),
                    mock.patch.object(runner.runtime_environment, "verify_active_environment", return_value=b"{}"),
                    mock.patch.object(runner.time, "monotonic", side_effect=clock),
                    mock.patch.object(runner.operator_handoff, "OperatorReceiptHandoff"),
                ):
                    instance = runner.OperatorStageRunner.from_file(config, pin)

                span = instance.deadline_monotonic - instance.started_monotonic
                self.assertLessEqual(span, runner.orchestration.MAX_STAGE_WALL_SECONDS)
                self.assertGreater(span, 0)
                self.assertEqual(
                    instance.deadline_utc - instance.started_utc,
                    timedelta(seconds=runner.orchestration.MAX_STAGE_WALL_SECONDS),
                )

    def test_stage_validator_still_rejects_an_extended_deadline(self):
        plan = plan_fixture()
        started = 60_000.123456789
        extended = replace(
            plan,
            stage_started_monotonic=started,
            stage_deadline_monotonic=started + runner.orchestration.MAX_STAGE_WALL_SECONDS + 0.001,
        )
        with (
            mock.patch.object(runner.orchestration, "_monotonic", return_value=started + 1.0),
            self.assertRaisesRegex(runner.orchestration.OrchestrationError, "stage-deadline-invalid"),
        ):
            runner.orchestration._validate_plan(extended)

    def test_registered_pin_mismatch_fails_before_clock_or_root_creation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config_path, _pin, directories = self._registered_config(root)
            config_value = json.loads(config_path.read_bytes())
            registration_path = Path(config_value["paths"]["initial_registration"])
            registration = json.loads(registration_path.read_bytes())
            registration["pins"]["protocol"] = "d" * 64
            registration_raw = canonical(registration)
            registration_path.write_bytes(registration_raw)
            config_value["initial_registration_sha256"] = digest(registration_raw)
            config_raw = canonical(config_value)
            config_path.write_bytes(config_raw)
            config_path.chmod(0o600)

            with (
                mock.patch.object(runner, "_verify_checkout_sources"),
                mock.patch.object(runner, "_require_source_bootstrap"),
                mock.patch.object(runner.core, "_verify_qualified_source_closure"),
                mock.patch.object(runner.runtime_environment, "verify_active_environment", return_value=b"{}"),
                mock.patch.object(runner.time, "monotonic", side_effect=AssertionError("clock must not start")),
            ):
                with self.assertRaisesRegex(
                    runner.OperatorRunnerError, "initial-registration-static-pin-mismatch:protocol"
                ):
                    runner.OperatorStageRunner.from_file(config_path, digest(config_raw))

            self.assertTrue(all(not Path(value).exists() for value in directories.values()))

    def test_static_preflight_runs_capture_protocol_and_identity_validators_before_clock(self):
        cases = ("candidate_identity", "protocol")
        for material_name in cases:
            with self.subTest(material=material_name), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                config_path, pin, directories = self._registered_config(root)
                config = json.loads(config_path.read_bytes())
                material_path = Path(config["paths"][material_name])
                document = json.loads(material_path.read_bytes())
                if material_name == "candidate_identity":
                    material_path.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
                    expected_error = "candidate-identity-preflight-invalid"
                else:
                    document["capture"]["timeout_seconds"] += 1
                    material_path.write_bytes(canonical(document))
                    expected_error = "capture-protocol-preflight-invalid"

                with (
                    mock.patch.object(runner, "_verify_checkout_sources"),
                    mock.patch.object(runner, "_require_source_bootstrap"),
                    mock.patch.object(runner, "_prepare_private_directories") as prepare_roots,
                    mock.patch.object(runner.time, "monotonic", side_effect=AssertionError("clock must not start")),
                ):
                    with self.assertRaisesRegex(runner.OperatorRunnerError, expected_error):
                        runner.OperatorStageRunner.from_file(config_path, pin)

                prepare_roots.assert_not_called()
                self.assertTrue(all(not Path(value).exists() for value in directories.values()))

    @unittest.skipUnless(
        sys.version_info[:2] == (3, 12) and sys.prefix != sys.base_prefix,
        "the registered environment smoke test requires the isolated Python 3.12 study venv",
    )
    def test_preflight_only_runs_static_checks_without_clock_dirs_or_handoff(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config, pin, directories = self._registered_config(root)
            real_subprocess_run = subprocess.run
            real_monotonic = runner.time.monotonic

            def clean_checkout_audit(command, **kwargs):
                if command[:3] == ["git", "status", "--porcelain"]:
                    return SimpleNamespace(stdout="")
                return real_subprocess_run(command, **kwargs)

            def forbid_stage_clock():
                frame = inspect.currentframe().f_back
                if frame.f_globals.get("__name__") == runner.__name__ and frame.f_code.co_name == "_build_plan":
                    raise AssertionError("stage clock must not start")
                return real_monotonic()

            output = io.StringIO()
            with (
                source_bootstrap_for_test(root),
                mock.patch.object(runner.subprocess, "run", side_effect=clean_checkout_audit),
                mock.patch.object(runner.time, "monotonic", side_effect=forbid_stage_clock),
                mock.patch.object(asyncio, "run", side_effect=AssertionError("phase graph must not run")),
                contextlib.redirect_stdout(output),
            ):
                for secret_path in json.loads(config.read_bytes())["private_paths"].values():
                    Path(secret_path).unlink()
                exit_code = runner.main(["--config", str(config), "--config-sha256", pin, "--preflight-only"])

            self.assertEqual(exit_code, 0, output.getvalue())
            receipt = json.loads(output.getvalue())
            self.assertEqual(set(receipt), {"status", "source_revision", "config_sha256", "registration_sha256"})
            self.assertEqual(receipt["status"], "static-preflight-passed")
            self.assertEqual(receipt["config_sha256"], pin)
            config_value = json.loads(config.read_bytes())
            self.assertEqual(receipt["source_revision"], config_value["source_revision"])
            self.assertEqual(receipt["registration_sha256"], config_value["initial_registration_sha256"])
            self.assertTrue(all(not Path(value).exists() for value in directories.values()))

    def test_preflight_rejects_pretty_acquisition_plan_pin_without_side_effects(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config, _pin, directories = self._registered_config(root)
            config_value = json.loads(config.read_bytes())
            registration_path = Path(config_value["paths"]["initial_registration"])
            registration = json.loads(registration_path.read_bytes())
            cohorts = json.loads(Path(config_value["paths"]["cohorts"]).read_bytes())
            stage_row = runner._stage_cohort(cohorts, config_value["stage_kind"])
            compact_plan = runner._build_acquisition_plan(
                config_value["stage_uuid"], stage_row["research_cases"], stage_row["navigation_targets"]
            )
            pretty_plan = json.dumps(json.loads(compact_plan), ensure_ascii=False, sort_keys=True, indent=2).encode(
                "utf-8"
            )
            self.assertNotEqual(digest(compact_plan), digest(pretty_plan))
            registration["pins"]["acquisition_plan"] = digest(pretty_plan)
            registration_raw = canonical(registration)
            registration_path.write_bytes(registration_raw)
            config_value["initial_registration_sha256"] = digest(registration_raw)
            config_raw = canonical(config_value)
            config.write_bytes(config_raw)
            config.chmod(0o600)

            output = io.StringIO()
            with (
                source_bootstrap_for_test(root),
                mock.patch.object(runner, "_verify_checkout_sources"),
                mock.patch.object(runner.runtime_environment, "verify_active_environment", return_value=b"{}"),
                mock.patch.object(runner.time, "monotonic", side_effect=AssertionError("stage clock must not start")),
                contextlib.redirect_stdout(output),
            ):
                exit_code = runner.main(
                    ["--config", str(config), "--config-sha256", digest(config_raw), "--preflight-only"]
                )

            self.assertEqual(exit_code, 1)
            self.assertEqual(
                json.loads(output.getvalue()),
                {
                    "status": "refused-or-terminal",
                    "error_code": "initial-registration-static-pin-mismatch:acquisition_plan",
                },
            )
            self.assertTrue(all(not Path(value).exists() for value in directories.values()))

    def test_preflight_checks_complete_initial_registration_inventory_without_side_effects(self):
        mutations = (
            ("schema", lambda doc: doc.pop("operation_ids"), "initial-registration-schema-invalid"),
            (
                "source revision",
                lambda doc: doc.__setitem__("source_revision", "f" * 40),
                "initial-registration-source-revision-mismatch",
            ),
            (
                "stage kind",
                lambda doc: doc.__setitem__("stage_kind", "confirmation"),
                "initial-registration-stage-kind-mismatch",
            ),
            (
                "task inventory",
                lambda doc: doc["task_ids"].__setitem__(0, "D-N99"),
                "initial-registration-task-inventory-mismatch",
            ),
            (
                "selector inventory",
                lambda doc: doc["operation_ids"].__setitem__(0, "unregistered-operation"),
                "initial-registration-selector-inventory-invalid",
            ),
            (
                "navigation skip declaration",
                lambda doc: doc["navigation_skip_reasons"].__setitem__(0, "not-a-production-guard"),
                "initial-registration-selector-inventory-invalid",
            ),
        )
        for label, mutate, expected_code in mutations:
            with self.subTest(label=label), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                config_path, _pin, directories = self._registered_config(root)
                config = json.loads(config_path.read_bytes())
                registration_path = Path(config["paths"]["initial_registration"])
                registration = json.loads(registration_path.read_bytes())
                mutate(registration)
                registration_raw = canonical(registration)
                registration_path.write_bytes(registration_raw)
                config["initial_registration_sha256"] = digest(registration_raw)
                config_raw = canonical(config)
                config_path.write_bytes(config_raw)
                config_path.chmod(0o600)
                output = io.StringIO()
                with (
                    source_bootstrap_for_test(root),
                    mock.patch.object(runner, "_verify_checkout_sources"),
                    mock.patch.object(runner.runtime_environment, "verify_active_environment", return_value=b"{}"),
                    mock.patch.object(
                        runner.time, "monotonic", side_effect=AssertionError("stage clock must not start")
                    ),
                    contextlib.redirect_stdout(output),
                ):
                    exit_code = runner.main(
                        ["--config", str(config_path), "--config-sha256", digest(config_raw), "--preflight-only"]
                    )
                self.assertEqual(exit_code, 1)
                self.assertEqual(json.loads(output.getvalue())["error_code"], expected_code)
                self.assertTrue(all(not Path(value).exists() for value in directories.values()))

    def test_preflight_refuses_unsafe_existing_root_without_repair(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config, pin, directories = self._registered_config(root)
            unsafe = Path(directories["stage_inventory"])
            unsafe.mkdir(mode=0o755)
            unsafe.chmod(0o755)
            output = io.StringIO()
            with (
                source_bootstrap_for_test(root),
                mock.patch.object(runner, "_verify_checkout_sources"),
                mock.patch.object(runner.core, "_verify_qualified_source_closure"),
                mock.patch.object(runner.runtime_environment, "verify_active_environment", return_value=b"{}"),
                contextlib.redirect_stdout(output),
            ):
                exit_code = runner.main(["--config", str(config), "--config-sha256", pin, "--preflight-only"])

            self.assertEqual(exit_code, 1)
            self.assertEqual(
                json.loads(output.getvalue()),
                {"status": "refused-or-terminal", "error_code": "configured-private-root-unsafe"},
            )
            self.assertEqual(stat.S_IMODE(unsafe.stat().st_mode), 0o755)
            self.assertTrue(
                all(Path(value).exists() is (key == "stage_inventory") for key, value in directories.items())
            )

    def test_unsafe_existing_private_root_fails_before_clock_or_other_creation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config, pin, directories = self._registered_config(root)
            unsafe = Path(directories["stage_inventory"])
            unsafe.mkdir(mode=0o755)
            unsafe.chmod(0o755)

            with (
                mock.patch.object(runner, "_verify_checkout_sources"),
                mock.patch.object(runner, "_require_source_bootstrap"),
                mock.patch.object(runner.core, "_verify_qualified_source_closure"),
                mock.patch.object(runner.runtime_environment, "verify_active_environment", return_value=b"{}"),
                mock.patch.object(runner, "_prepare_private_directories", wraps=runner._prepare_private_directories),
                mock.patch.object(runner.time, "monotonic", side_effect=AssertionError("clock must not start")),
            ):
                with self.assertRaisesRegex(runner.OperatorRunnerError, "configured-private-root-unsafe"):
                    runner.OperatorStageRunner.from_file(config, pin)

            self.assertEqual(stat.S_IMODE(unsafe.stat().st_mode), 0o755)
            self.assertTrue(
                all(Path(value).exists() is (key == "stage_inventory") for key, value in directories.items())
            )

    def test_config_expected_digest_is_external_and_secret_paths_are_private(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config, pin = self._config(root, b"{}")
            with self.assertRaisesRegex(runner.OperatorRunnerError, "config-external-pin-mismatch"):
                runner.OperatorStageRunner.from_file(config, "0" * 64)
            secret = root / "candidate_operator_token.secret"
            secret.chmod(0o644)
            with self.assertRaisesRegex(runner.OperatorRunnerError, "configured-secret-file-not-private"):
                runner._secret(str(secret))

    def test_optional_ca_bundle_is_an_explicit_private_config_path(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config_path, _pin = self._config(root, b"{}")
            config = json.loads(config_path.read_bytes())
            ca_path = root / "capture-ca.pem"
            ca_path.write_text("-----BEGIN CERTIFICATE-----\nsynthetic\n", encoding="ascii")
            ca_path.chmod(0o600)
            config["private_paths"]["capture_ca_bundle"] = str(ca_path)
            raw = canonical(config)
            config_path.write_bytes(raw)
            config_path.chmod(0o600)
            loaded = runner._load_config(config_path, digest(raw))
            self.assertEqual(loaded["private_paths"]["capture_ca_bundle"], str(ca_path))

    def test_confirmation_receipt_config_requires_complete_independent_binding(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config_path, _pin = self._config(root, b"{}")
            config = json.loads(config_path.read_bytes())
            config["stage_kind"] = "confirmation"
            config["private_paths"]["development_pass_receipt"] = str(root / "development-pass.json")
            raw = canonical(config)
            config_path.write_bytes(raw)
            config_path.chmod(0o600)
            with self.assertRaisesRegex(runner.OperatorRunnerError, "config-development-receipt-binding-incomplete"):
                runner._load_config(config_path, digest(raw))

    def test_confirmation_phase_context_transports_receipt_bytes_hash_and_stage(self):
        base_plan = plan_fixture()
        receipt = canonical(
            {
                "schema": runner.core.DEVELOPMENT_RECEIPT_SCHEMA,
                "stage_uuid": str(uuid.UUID(int=8100)),
                "stage_kind": "development",
                "status": "passed",
                "protocol_sha256": digest(base_plan.protocol_bytes),
                "source_revision": base_plan.source_revision,
                "source_closure_sha256": base_plan.source_closure_sha256,
                "result_sha256": "e" * 64,
            }
        )
        plan = replace(
            base_plan,
            stage_kind="confirmation",
            development_receipt_bytes=receipt,
            development_receipt_sha256=digest(receipt),
            development_stage_uuid=str(uuid.UUID(int=8100)),
        )
        instance = object.__new__(runner.OperatorStageRunner)
        instance.plan = plan
        instance.active_environment_receipt_bytes = b"{}"
        instance._saved_corpus_reference_bytes = None
        instance._frozen_pool_reference_bytes = None
        instance._active_registration_bytes = None
        instance._active_input_manifest_bytes = None
        instance._frozen_stage_clock_bytes = None
        instance._pipeline_inputs = None
        instance.static_materials = {"qualified_source_closure": b"closure"}
        instance.initial_registration = b"initial"
        bindings, artifacts = runner.OperatorStageRunner._phase_request_context(instance, {})
        self.assertEqual(bindings["development_receipt_sha256"], digest(receipt))
        self.assertEqual(bindings["development_stage_uuid"], str(uuid.UUID(int=8100)))
        self.assertEqual(artifacts["development-pass-receipt"], receipt)

    def test_fresh_runtime_handoff_requires_native_host_transcripts(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            secret = root / "selector.key"
            secret.write_bytes(b"synthetic-test-only\n")
            secret.chmod(0o600)
            instance = object.__new__(runner.OperatorStageRunner)
            instance.directories = {
                key: str(root / key)
                for key in {
                    "grader_handoff",
                    "lease_root",
                    "selector_archive",
                    "selector_results",
                }
            }
            instance.private_paths = {"selector_api_key": str(secret)}

            bindings = instance.runtime_bindings()

            self.assertTrue(bindings.native_handoff.require_native_host_transcripts)

    def test_checkout_audit_rejects_revision_dirty_tree_and_poisoned_material(self):
        revision = subprocess.run(
            ["git", "rev-parse", "--verify", "HEAD^{commit}"],
            cwd=runner._REPOSITORY_ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
        paths = {key: str(path) for key, path in runner._CHECKOUT_SOURCE_PATHS.items()}
        materials = {key: path.read_bytes() for key, path in runner._CHECKOUT_SOURCE_PATHS.items()}
        clean = subprocess.CompletedProcess([], 0, stdout="", stderr="")
        current = subprocess.CompletedProcess([], 0, stdout=revision + "\n", stderr="")
        with mock.patch.object(runner.subprocess, "run", side_effect=[current, clean]):
            runner._verify_checkout_sources(revision, paths, materials)

        with mock.patch.object(
            runner.subprocess,
            "run",
            return_value=subprocess.CompletedProcess([], 0, stdout="f" * 40, stderr=""),
        ):
            with self.assertRaisesRegex(runner.OperatorRunnerError, "checkout-revision-mismatch"):
                runner._verify_checkout_sources(revision, paths, materials)

        dirty = subprocess.CompletedProcess([], 0, stdout=" M tracked.py\n", stderr="")
        with mock.patch.object(runner.subprocess, "run", side_effect=[current, dirty]):
            with self.assertRaisesRegex(runner.OperatorRunnerError, "checkout-not-clean"):
                runner._verify_checkout_sources(revision, paths, materials)

        poisoned = dict(materials, coverage_source=b"caller-supplied old source")
        with mock.patch.object(runner.subprocess, "run", side_effect=[current, clean]):
            with self.assertRaisesRegex(
                runner.OperatorRunnerError, "checkout-source-material-mismatch:coverage_source"
            ):
                runner._verify_checkout_sources(revision, paths, poisoned)

    def test_source_preflight_failure_precedes_clock_and_handoff_creation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            current_revision = subprocess.run(
                ["git", "rev-parse", "--verify", "HEAD^{commit}"],
                cwd=runner._REPOSITORY_ROOT,
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            config_path, _old_pin = self._config(root, b"{}")
            config = json.loads(config_path.read_bytes())
            config["source_revision"] = current_revision
            raw = canonical(config)
            config_path.write_bytes(raw)
            config_path.chmod(0o600)
            pin = digest(raw)
            current = subprocess.CompletedProcess([], 0, stdout=current_revision + "\n", stderr="")
            clean = subprocess.CompletedProcess([], 0, stdout="", stderr="")
            with (
                mock.patch.object(runner.subprocess, "run", side_effect=[current, clean]),
                mock.patch.object(runner.time, "monotonic", side_effect=AssertionError("clock must not start")),
                mock.patch.object(
                    runner.operator_handoff, "OperatorReceiptHandoff", side_effect=AssertionError("no handoff")
                ),
                mock.patch.object(runner, "_require_source_bootstrap"),
            ):
                with self.assertRaisesRegex(runner.OperatorRunnerError, "checkout-source-path-mismatch"):
                    runner.OperatorStageRunner.from_file(config_path, pin)
            self.assertFalse((root / "operator_handoff").exists())

    def test_confirmation_selects_only_its_explicit_stage_cohort(self):
        cohorts = {
            "stages": [
                {"stage": "development", "research_cases": [{"task_id": "dev"}], "navigation_targets": []},
                {
                    "stage": "confirmation",
                    "research_cases": [{"task_id": "confirm"}],
                    "navigation_targets": [],
                },
            ]
        }
        self.assertEqual(runner._stage_cohort(cohorts, "development")["research_cases"][0]["task_id"], "dev")
        self.assertEqual(runner._stage_cohort(cohorts, "confirmation")["research_cases"][0]["task_id"], "confirm")
        with self.assertRaisesRegex(runner.OperatorRunnerError, "configured-stage-cohort-not-unique"):
            runner._stage_cohort({"stages": [{"stage": "development"}]}, "confirmation")

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config_path, _pin = self._config(root, b"{}")
            config = json.loads(config_path.read_bytes())
            config["stage_kind"] = "confirmation"
            config["forbidden_stage_uuids"].append(config["packet_stage_uuid"])
            raw = canonical(config)
            config_path.write_bytes(raw)
            config_path.chmod(0o600)
            with self.assertRaisesRegex(runner.OperatorRunnerError, "config-forbidden-stage-inventory-invalid"):
                runner._load_config(config_path, digest(raw))

    def test_answer_authority_allows_only_explicitly_opted_in_private_http(self):
        async def scenario(endpoint: str, allow: bool):
            temporary = tempfile.TemporaryDirectory()
            root = Path(temporary.name)
            secret = root / "answer.key"
            secret.write_text("synthetic-test-only\n", encoding="utf-8")
            secret.chmod(0o600)
            instance = object.__new__(runner.OperatorStageRunner)
            instance._test_temporary_directory = temporary
            instance.config = {"answer_endpoint": endpoint, "allow_trusted_private_http": allow}
            instance.private_paths = {"answer_api_key": str(secret)}
            instance.directories = {
                "lease_root": str(root / "leases"),
                "answer_archive": str(root / "archive"),
                "answer_results": str(root / "results"),
            }
            instance.plan = SimpleNamespace(
                stage_uuid=str(uuid.UUID(int=802)),
                source_revision="a" * 40,
                protocol_bytes=b"protocol",
                cohorts_bytes=b"cohorts",
            )
            instance.started_utc = datetime.now(timezone.utc)
            instance.deadline_utc = instance.started_utc + timedelta(hours=8)
            captured = {}

            async def request(scope, request_id, bindings, **kwargs):
                captured.update(scope=scope, request_id=request_id, bindings=bindings)
                return b"synthetic-receipt", "b" * 64

            instance._request = request
            return instance, captured

        async def run_private():
            instance, captured = await scenario("http://llm-svc:4000/v1", True)

            async def resolve(_url):
                return "http://10.0.0.8:4000/v1/chat/completions", "llm-svc:4000", "c" * 64

            with mock.patch.object(answer, "_resolve_private_http_destination", side_effect=resolve):
                authority = await instance._answer_authority(instance.plan, _tasks())
            self.assertTrue(authority["allow_trusted_private_http"])
            self.assertEqual(authority["endpoint"], "http://llm-svc:4000/v1")
            self.assertEqual(captured["bindings"]["endpoint_security_mode"], "trusted-private-http")
            self.assertEqual(captured["bindings"]["resolved_destination_sha256"], "c" * 64)

        async def run_public():
            instance, captured = await scenario("http://public.example/v1", True)
            with self.assertRaisesRegex(answer.AnswerExecutionError, "answer-plaintext-endpoint-not-permitted"):
                await instance._answer_authority(instance.plan, _tasks())
            self.assertEqual(captured, {})

        asyncio.run(run_private())
        asyncio.run(run_public())

    def test_frozen_answer_request_binds_capture_and_selector_terminal_files(self):
        temporary = tempfile.TemporaryDirectory()
        root = Path(temporary.name)
        stage = str(uuid.UUID(int=811))
        capture_root = root / "captures"
        selector_root = root / "selector-results"
        for directory in (capture_root, selector_root):
            directory.mkdir(mode=0o700)
        (capture_root / stage).mkdir(mode=0o700)
        capture_inventory = b"sealed capture inventory fixture"
        selector_inventory = b"sealed selector terminal fixture"
        (capture_root / stage / "inventory.json").write_bytes(capture_inventory)
        (capture_root / stage / "inventory.json").chmod(0o600)
        (selector_root / f"{stage}.terminal-inventory.json").write_bytes(selector_inventory)
        (selector_root / f"{stage}.terminal-inventory.json").chmod(0o600)
        secret = root / "answer.key"
        secret.write_text("synthetic-test-only\n", encoding="utf-8")
        secret.chmod(0o600)
        instance = object.__new__(runner.OperatorStageRunner)
        instance._test_temporary_directory = temporary
        instance.config = {"answer_endpoint": "https://model.example/v1"}
        instance.private_paths = {"answer_api_key": str(secret)}
        instance.directories = {
            "lease_root": str(root / "leases"),
            "answer_archive": str(root / "archive"),
            "answer_results": str(root / "answer-results"),
            "capture_receipts": str(capture_root),
            "selector_results": str(selector_root),
        }
        instance.plan = SimpleNamespace(
            stage_uuid=stage,
            source_revision="a" * 40,
            protocol_bytes=b"protocol",
            cohorts_bytes=b"cohorts",
            pool_input_mode="frozen-pool/1",
        )
        instance.started_utc = datetime.now(timezone.utc)
        instance.deadline_utc = instance.started_utc + timedelta(hours=8)
        captured = {}

        async def request(scope, request_id, bindings, **kwargs):
            captured.update(scope=scope, request_id=request_id, bindings=bindings, artifacts=kwargs["artifacts"])
            return b"receipt", "b" * 64

        instance._request = request
        asyncio.run(instance._answer_authority(instance.plan, _tasks()))
        self.assertEqual(captured["scope"], "answer-execution")
        self.assertNotIn("capture_inventory_sha256", captured["bindings"])
        self.assertNotIn("selector_terminal_inventory_sha256", captured["bindings"])
        source_reference = json.loads(captured["artifacts"]["answer-source-reference"])
        self.assertEqual(source_reference["stage_uuid"], stage)
        self.assertEqual(source_reference["capture_inventory_sha256"], hashlib.sha256(capture_inventory).hexdigest())
        self.assertEqual(
            source_reference["selector_terminal_inventory_sha256"], hashlib.sha256(selector_inventory).hexdigest()
        )

    def test_wrong_answer_security_mode_is_rejected_before_lease(self):
        import pytest

        async def scenario(tmp_path):
            args = _stage_inputs(tmp_path)
            events = []

            class WrongModeVerifier:
                def verify(self, receipt_bytes, expected_receipt_sha256, bindings):
                    events.append("verify")
                    return answer.VerifiedAnswerPermit(
                        status="verified-admitted",
                        stage_uuid=bindings["stage_uuid"],
                        source_revision=bindings["source_revision"],
                        protocol_sha256=bindings["protocol_sha256"],
                        cohorts_sha256=bindings["cohorts_sha256"],
                        operation_manifest_sha256=bindings["operation_manifest_sha256"],
                        endpoint_sha256=bindings["endpoint_sha256"],
                        resolved_destination_sha256=bindings["resolved_destination_sha256"],
                        endpoint_security_mode="trusted-private-http",
                        operation_ids=tuple(bindings["operation_ids"]),
                        max_calls=answer.MAX_CALLS,
                        request_bytes_maximum=answer.MAX_REQUEST_BYTES,
                        response_bytes_maximum=answer.MAX_RESPONSE_BYTES,
                        receipt_sha256=expected_receipt_sha256,
                    )

            class CountLease:
                def consume_once(self, *_args, **_kwargs):
                    events.append("lease")

            args["permit_verifier"] = WrongModeVerifier()
            args["one_shot_lease"] = CountLease()
            with pytest.raises(answer.AnswerExecutionError, match="answer-permit-binding-mismatch"):
                await answer.execute_answer_stage(**args)
            self.assertEqual(events, ["verify"])
            self.assertEqual(list(args["lease_root"].iterdir()), [])

        with tempfile.TemporaryDirectory() as temporary:
            asyncio.run(scenario(Path(temporary)))

    def test_acquisition_plan_matches_fixed_cohort_engine_schedule(self):
        plan = plan_fixture()
        self.assertEqual(
            runner._build_acquisition_plan(plan.stage_uuid, list(plan.research_cases), list(plan.navigation_targets)),
            plan.acquisition_plan_bytes,
        )

    def test_answer_authority_preserves_configured_endpoint_binding(self):
        # The executor hashes the configured endpoint string, then canonicalizes
        # it for dispatch. Base URLs and /v1 URLs must therefore remain the
        # value returned in authority, rather than the appended request URL.
        for endpoint in (
            "https://model.example",
            "https://model.example/v1",
            "https://model.example/v1/chat/completions",
        ):
            with self.subTest(endpoint=endpoint), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                secret = root / "answer.key"
                secret.write_text("synthetic-test-only\n", encoding="utf-8")
                secret.chmod(0o600)
                instance = object.__new__(runner.OperatorStageRunner)
                instance.config = {"answer_endpoint": endpoint}
                instance.private_paths = {"answer_api_key": str(secret)}
                instance.directories = {
                    "lease_root": str(root / "leases"),
                    "answer_archive": str(root / "archive"),
                    "answer_results": str(root / "results"),
                }
                instance.plan = SimpleNamespace(
                    stage_uuid=str(uuid.UUID(int=801)),
                    source_revision="a" * 40,
                    protocol_bytes=b"protocol",
                    cohorts_bytes=b"cohorts",
                )
                instance.started_utc = datetime.now(timezone.utc)
                instance.deadline_utc = instance.started_utc + timedelta(hours=8)
                captured = {}

                async def request(scope, request_id, bindings, **kwargs):
                    captured.update(scope=scope, request_id=request_id, bindings=bindings)
                    return b"synthetic-receipt", "b" * 64

                instance._request = request
                authority = asyncio.run(instance._answer_authority(instance.plan, _tasks()))
                self.assertEqual(authority["endpoint"], endpoint)
                self.assertEqual(captured["scope"], "answer-execution")
                self.assertEqual(
                    captured["bindings"]["endpoint_sha256"],
                    answer._sha(
                        answer._canonical(
                            {
                                "endpoint": endpoint,
                                "security_mode": "https-required",
                                "resolved_destination_sha256": answer._sha(
                                    answer._canonical({"mode": "https-required"})
                                ),
                            }
                        )
                    ),
                )
                self.assertEqual(
                    answer._chat_url(authority["endpoint"])[0], "https://model.example/v1/chat/completions"
                )


if __name__ == "__main__":
    unittest.main()
