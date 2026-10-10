from __future__ import annotations

import asyncio
import hashlib
import json
import os
import tempfile
import time
import unittest
import uuid
from pathlib import Path
from unittest import mock

import httpx

from scripts import coverage_answer_execution as answer_execution
from scripts import (
    coverage_jev_execution,
    coverage_resource_evidence,
)
from scripts import coverage_live_acquire as live_acquire
from scripts import coverage_resource_evidence as resource_evidence
from scripts import coverage_source_capture as source_capture
from scripts import coverage_stage_finalization as stage_finalization
from scripts import coverage_stage_orchestration as stage
from scripts import coverage_study_acquire as acquisition
from scripts import coverage_study_core as core
from slopsearx.adapter import SearchResult
from slopsearx.service import ScopeDecision, SearchResponse
from tests.test_coverage_answer_execution import _answer_content, _response
from tests.test_coverage_answer_execution import _Lease as AnswerLease
from tests.test_coverage_answer_execution import _Verifier as AnswerVerifier
from tests.test_coverage_grade_closure import _answer_outputs, _pre_submissions
from tests.test_coverage_jev_execution import PrivateRoots as SelectorRoots
from tests.test_coverage_jev_execution import permit_for as selector_permit_for
from tests.test_coverage_jev_execution import valid_response as selector_valid_response
from tests.test_coverage_legacy_control import _response as legacy_selector_response
from tests.test_coverage_live_acquire import FakePacer, FakePermitVerifier, OneShot
from tests.test_coverage_source_capture import _Lease as CaptureLease
from tests.test_coverage_source_capture import _Verifier as CaptureVerifier


def canonical(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()


def digest(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _source_materials(*, registered: bool = False) -> dict[str, bytes]:
    if not registered:
        return {
            "coverage_source": b"synthetic coverage code",
            "production_rerank_source": b"synthetic production source",
            "dependency_lock": b"synthetic dependency lock",
        }
    repo = Path(__file__).parents[1]
    return {
        "coverage_source": (repo / "scripts/intent_ranking_coverage.py").read_bytes(),
        "production_rerank_source": (
            repo / "docs/experiments/evidence/coverage-first-study/w0-rerank-v1.py.txt"
        ).read_bytes(),
        "dependency_lock": (repo / "uv.lock").read_bytes(),
    }


def _source_closure_bytes(source_revision: str, *, registered: bool = False) -> bytes:
    materials = _source_materials(registered=registered)
    return canonical(
        {
            "schema": "coverage-study-qualified-source-closure/1",
            "source_revision": source_revision,
            "material_pins": {key: digest(value) for key, value in materials.items()},
        }
    )


def plan_fixture(*, selector_map_registered: bool = False) -> stage.StagePlan:
    stage_id = str(uuid.UUID(int=8001))
    packet_id = str(uuid.UUID(int=8002))
    evidence_dir = Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study"
    protocol_bytes = (evidence_dir / "protocol.json").read_bytes()
    if selector_map_registered:
        protocol_value = json.loads(protocol_bytes)
        protocol_value["selector_input_map_schema"] = "coverage-selector-input-map/2-registered"
        protocol_value["selector_input_map_status"] = "registered"
        protocol_bytes = canonical(protocol_value)
    cohorts_bytes = (evidence_dir / "cohorts.json").read_bytes()
    cohort_doc = json.loads(cohorts_bytes)
    dev = next(row for row in cohort_doc["stages"] if row["stage"] == "development")
    cases, navigation = dev["research_cases"], dev["navigation_targets"]
    acquisition_plan = canonical(
        {
            "schema": core.ACQUISITION_SCHEMA,
            "stage_uuid": stage_id,
            "tasks": [
                {
                    "task_id": row["task_id"],
                    "engine_calls": {
                        engine: (
                            2
                            if engine == "wikipedia" and engine in row["pool_plan"]["engines"]
                            else 1
                            if engine in row["pool_plan"]["engines"]
                            else 0
                        )
                        for engine in ("wikipedia", "arxiv", "github", "openalex")
                    },
                }
                for row in cases
            ]
            + [
                {
                    "task_id": row["target_id"],
                    "engine_calls": {"wikipedia": 0, "arxiv": 0, "github": 1, "openalex": 0},
                }
                for row in navigation
            ],
        }
    )
    source_closure_sha = digest(_source_closure_bytes("a" * 40, registered=selector_map_registered))
    preacquisition_input_sha = digest(
        canonical(
            {
                "schema": "coverage-preacquisition-input-manifest/1",
                "stage_uuid": stage_id,
                "protocol_sha256": digest(protocol_bytes),
                "cohorts_sha256": digest(cohorts_bytes),
                "task_ids": [r["task_id"] for r in cases] + [r["target_id"] for r in navigation],
            }
        )
    )
    acq_manifest = {
        "schema": "coverage-live-acquisition-manifest/1",
        "stage": "development",
        "stage_uuid": stage_id,
        "source_revision": "a" * 40,
        "source_closure_sha256": source_closure_sha,
        "protocol_sha256": digest(protocol_bytes),
        "cohorts_sha256": digest(cohorts_bytes),
        "input_manifest_sha256": preacquisition_input_sha,
        "acquisition_plan_sha256": digest(acquisition_plan),
        "research_cases": cases,
        "navigation_targets": navigation,
    }
    acquisition_manifest_bytes = live_acquire._canonical(acq_manifest)
    now = time.monotonic()
    return stage.StagePlan(
        stage_uuid=stage_id,
        stage_kind="development",
        source_revision="a" * 40,
        protocol_bytes=protocol_bytes,
        cohorts_bytes=cohorts_bytes,
        acquisition_plan_bytes=acquisition_plan,
        acquisition_manifest_bytes=acquisition_manifest_bytes,
        source_closure_sha256=source_closure_sha,
        research_cases=tuple(cases),
        navigation_targets=tuple(navigation),
        candidate_identity_bytes=(
            b'{"runtime":{"model":"free","revision":"cccccccccccccccccccccccccccccccccccccccc"}}\n'
        ),
        candidate_endpoint_sha256=digest(b"https://capture.example"),
        packet_stage_uuid=packet_id,
        stage_started_monotonic=now - 0.1,
        stage_deadline_monotonic=now + 300,
        expected_acquisition_manifest_sha256=digest(acquisition_manifest_bytes),
    )


def _search_result(index: int, *, namespace: str, engines: list[str], url: str | None = None) -> SearchResult:
    return SearchResult(
        url=url or f"https://example.test/{namespace}/{index}",
        title=f"Synthetic card {index}",
        content=f"Synthetic implementation evidence {index}.",
        engine=engines[0],
        engines=set(engines),
        score=float(10 - index / 10),
        position=index + 1,
        category="reference",
        published_date=None,
        thumbnail=None,
        img_src=None,
        tier=1,
        payload={"fixture": True, "index": index},
        work_group={"namespace": namespace},
    )


def _write_synthetic_pool_snapshots(plan: stage.StagePlan, root: Path) -> stage.AcquisitionEvidence:
    root.mkdir(mode=0o700)
    os.chmod(root, 0o700)
    manifest = json.loads(plan.acquisition_manifest_bytes)
    operations = []
    for index, task in enumerate(plan.research_cases):
        engines = task["pool_plan"]["engines"]
        count = 4 if index < 4 else 41
        results = [
            _search_result(card_index, namespace=task["task_id"], engines=engines) for card_index in range(count)
        ]
        scope = ScopeDecision(selected_engines=list(engines))
        response = SearchResponse(
            query=task["search_query"],
            results=results,
            scope=scope,
            engine_outcomes=[],
            ranking_explanation="tier_then_cross_engine_presence",
        )
        operations.append(
            acquisition.AcquisitionOperation(
                operation_id=task["task_id"],
                kind="research",
                query=task["search_query"],
                engines=tuple(engines),
                status="complete",
                pool_count=count,
                band=task["pool_plan"]["band"],
                band_valid=True,
                target_url=None,
                target_found_at_rank1=None,
                scope=scope,
                canonical_response=response,
                exchanges=(),
            )
        )
    for task in plan.navigation_targets:
        target = task["target_url"]
        engines = ["github"]
        results = [
            _search_result(0, namespace=task["target_id"], engines=engines, url=target),
            _search_result(1, namespace=task["target_id"], engines=engines),
        ]
        scope = ScopeDecision(selected_engines=engines)
        response = SearchResponse(
            query=task["search_query"],
            results=results,
            scope=scope,
            engine_outcomes=[],
            ranking_explanation="tier_then_cross_engine_presence",
        )
        operations.append(
            acquisition.AcquisitionOperation(
                operation_id=task["target_id"],
                kind="navigation",
                query=task["search_query"],
                engines=("github",),
                status="complete",
                pool_count=2,
                band=None,
                band_valid=None,
                target_url=target,
                target_found_at_rank1=True,
                scope=scope,
                canonical_response=response,
                exchanges=(),
            )
        )
    manifest_sha = plan.expected_acquisition_manifest_sha256
    index_rows = []
    for operation in operations:
        raw = live_acquire._canonical(live_acquire._operation_snapshot(operation, manifest, manifest_sha))
        filename = live_acquire._snapshot_filename(operation.operation_id)
        path = root / filename
        path.write_bytes(raw)
        os.chmod(path, 0o600)
        index_rows.append(
            {
                "operation_id": operation.operation_id,
                "kind": operation.kind,
                "status": operation.status,
                "file": filename,
                "sha256": digest(raw),
                "bytes": len(raw),
                "card_count": operation.pool_count,
            }
        )
    index = {
        "schema": live_acquire.POOL_INDEX_SCHEMA,
        "stage": plan.stage_kind,
        "stage_uuid": plan.stage_uuid,
        "source_revision": plan.source_revision,
        "source_closure_sha256": plan.source_closure_sha256,
        "protocol_sha256": digest(plan.protocol_bytes),
        "cohorts_sha256": digest(plan.cohorts_bytes),
        "input_manifest_sha256": manifest["input_manifest_sha256"],
        "acquisition_plan_sha256": digest(plan.acquisition_plan_bytes),
        "stage_manifest_sha256": manifest_sha,
        "status": "complete",
        "operations": index_rows,
    }
    index_bytes = live_acquire._canonical(index)
    index_path = root / "pool-snapshot-index.json"
    index_path.write_bytes(index_bytes)
    os.chmod(index_path, 0o600)
    result = acquisition.StageAcquisition(plan.stage_kind, "complete", operations, [], 0, 0, 41)
    return stage.AcquisitionEvidence(
        result=result,
        snapshots_directory=root,
        snapshot_index_sha256=digest(index_bytes),
        acquisition_manifest_sha256=manifest_sha,
        receipt_sha256=digest(b"synthetic durable acquisition receipt"),
        source_bound_receipt_status="synthetic-test-only",
    )


def _late_preflight(plan: stage.StagePlan, evidence: stage.AcquisitionEvidence, pipeline_inputs):
    task_rows = []
    for task in pipeline_inputs["tasks"]:
        compiled = core.coverage.compile_request(
            query=task["query"],
            purpose=task["purpose"],
            facets=task["facets"],
            candidates=task["ranking_candidates"],
            incumbent_order=task["native_order"],
        )
        task_rows.append(
            {
                "task_id": task["task_id"],
                "query": task["query"],
                "purpose": task["purpose"],
                "facets": task["facets"],
                "candidates": task["ranking_candidates"],
                "incumbent_order": task["native_order"],
                "compiled_request_sha256": digest(compiled.body),
            }
        )
    operations = {row.operation_id: row for row in evidence.result.operations}
    navigation_rows = []
    for target in plan.navigation_targets:
        operation = operations[target["target_id"]]
        pool = [f"c{index}" for index in range(operation.pool_count)]
        navigation_rows.append(
            {
                "task_id": target["target_id"],
                "query": target["search_query"],
                "target_candidate_id": pool[0],
                "pool_ids": pool,
                "w0_order": pool,
            }
        )
    input_bytes = canonical(
        {
            "schema": core.INPUT_SCHEMA,
            "stage_uuid": plan.stage_uuid,
            "research_tasks": task_rows,
            "navigation_tasks": navigation_rows,
        }
    )
    registered = json.loads(plan.protocol_bytes).get("selector_input_map_schema") == (
        "coverage-selector-input-map/2-registered"
    )
    source_closure = _source_closure_bytes(plan.source_revision, registered=registered)
    source_materials = _source_materials(registered=registered)
    capture_manifest = pipeline_inputs["capture_manifest_bytes"]
    materials = {
        "protocol": plan.protocol_bytes,
        "qualified_source_closure": source_closure,
        **source_materials,
        "task_input_manifest": input_bytes,
        "source_capture_manifest": capture_manifest,
        "reference_manifest": canonical(
            {"schema": "coverage-blind-reference-manifest/1", "stage_uuid": plan.stage_uuid}
        ),
        "answer_assessment_plan": canonical(
            {"schema": "coverage-answer-assessment-plan/1", "stage_uuid": plan.stage_uuid}
        ),
        "acquisition_plan": plan.acquisition_plan_bytes,
        "capture_plan": canonical(
            {
                "schema": core.CAPTURE_SCHEMA,
                "stage_uuid": plan.stage_uuid,
                "tasks": [{"task_id": task, "scraper_calls": 1} for task in plan_ids(plan)],
                "health_calls": 1,
            }
        ),
    }
    pins = {key: digest(value) for key, value in materials.items()}
    skips = (None,) * 5
    operation_ids = [row.operation_id for row in core.selector_inventory(skips)]
    task_ids = plan_ids(plan)
    registration = canonical(
        {
            "schema": core.REGISTRATION_SCHEMA,
            "stage_uuid": plan.stage_uuid,
            "stage_kind": plan.stage_kind,
            "source_revision": plan.source_revision,
            "pins": pins,
            "operation_ids": operation_ids,
            "navigation_skip_reasons": list(skips),
            "task_ids": task_ids,
        }
    )
    prepared = core.preflight_stage(
        registration_bytes=registration,
        expected_registration_sha256=digest(registration),
        expected_stage_uuid=plan.stage_uuid,
        forbidden_stage_uuids=(str(uuid.UUID(int=9000)),),
        expected_source_revision=plan.source_revision,
        observed_source_revision=plan.source_revision,
        materials=materials,
        expected_pins=pins,
        acquisition_plan_bytes=plan.acquisition_plan_bytes,
        capture_plan_bytes=materials["capture_plan"],
        task_ids=tuple(task_ids),
        navigation_skip_reasons=skips,
    )
    return stage.LatePreparationEvidence(prepared, input_bytes, digest(registration))


def plan_ids(plan: stage.StagePlan) -> list[str]:
    return [row["task_id"] for row in plan.research_cases] + [row["target_id"] for row in plan.navigation_targets]


def _synthetic_capture(plan, pipeline_inputs, root: Path) -> stage.CaptureEvidence:
    rows = []
    artifacts = {}
    for index, source in enumerate(pipeline_inputs["capture_manifest"]["sources"], start=1):
        text = f"Synthetic public source context for {source['source_id']} with implementation notes. " * 45
        context_sha = digest(text.encode())
        artifact_name = f"context-{index:04d}.json"
        artifacts[artifact_name] = canonical(
            {
                "schema": "coverage-source-context/1",
                "task_id": source["task_id"],
                "source_id": source["source_id"],
                "source_url": source["url"],
                "result_index": source["result_index"],
                "title": source["title"],
                "engine": source["engine"],
                "context": text,
                "context_sha256": context_sha,
                "context_characters": len(text),
            }
        )
        rows.append(
            {
                **source,
                "status": "captured",
                "context_artifact": artifact_name,
                "context_sha256": context_sha,
                "context_characters": len(text),
            }
        )
    inventory = {
        "schema": "coverage-source-capture-inventory/1",
        "stage_uuid": plan.stage_uuid,
        "status": "complete",
        "sources": rows,
    }
    inventory_bytes = canonical(inventory)
    result = source_capture.SourceCaptureResult(
        status="complete",
        stage_uuid=plan.stage_uuid,
        source_count=len(rows),
        captured_count=len(rows),
        failed_count=0,
        unattempted_count=0,
        owned_http_calls=0,
        receipt_directory=root,
        private_inventory=tuple(rows),
    )
    return stage.CaptureEvidence(result, inventory_bytes, artifacts, digest(inventory_bytes))


class StageOrchestrationTests(unittest.TestCase):
    def test_unknown_selector_usage_stops_before_downstream_phases(self):
        plan = plan_fixture()
        selector = stage.SelectorEvidence(
            stage_uuid=plan.stage_uuid,
            terminal_inventory_sha256=digest(b"synthetic selector inventory"),
            operation_rows=(),
            research_orders={},
            navigation_rank_one={},
            reference_grade_receipt_sha256=digest(b"reference receipt"),
            usage_status="unavailable",
        )
        with self.assertRaisesRegex(stage.OrchestrationError, "selector-usage-unknown"):
            stage._check_selector(selector, plan, object(), {}, selector.reference_grade_receipt_sha256)

    def test_cohort_digest_mismatch_rejected_before_claim_or_executor(self):
        plan = plan_fixture()
        changed = stage.StagePlan(**{**plan.__dict__, "cohorts_bytes": plan.cohorts_bytes + b" "})
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "private"
            root.mkdir(mode=0o700)
            os.chmod(root, 0o700)
            with self.assertRaisesRegex(stage.OrchestrationError, "protocol-cohorts-binding-mismatch"):
                asyncio.run(stage.coordinate_coverage_stage(plan=changed, executors=None, inventory_root=root))
            self.assertEqual(list(root.iterdir()), [])

    def test_terminal_acquisition_leaves_every_later_phase_uninvoked(self):
        plan = plan_fixture()
        calls: list[str] = []

        async def fail_acquisition(_plan):
            calls.append("acquisition")
            raise RuntimeError("private provider detail must not be stored")

        async def unexpected(*_args):
            calls.append("later")
            raise AssertionError("later phase was invoked")

        executors = stage.StageExecutors(
            acquire=fail_acquisition,
            verify_acquisition=unexpected,
            capture=unexpected,
            prepare_after_acquisition=unexpected,
            verify_late_preflight=unexpected,
            grade_references=unexpected,
            selectors=unexpected,
            answerer=unexpected,
            grade_answers=unexpected,
        )
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "private"
            root.mkdir(mode=0o700)
            os.chmod(root, 0o700)
            result = asyncio.run(stage.coordinate_coverage_stage(plan=plan, executors=executors, inventory_root=root))
            data = json.loads(result.inventory_path.read_bytes())
            inventory_text = result.inventory_path.read_text()
        self.assertEqual(calls, ["acquisition"])
        self.assertEqual(result.status, "terminal-incomplete")
        self.assertEqual(result.terminal_reason, "acquisition-failed")
        self.assertNotIn("private provider detail", inventory_text)
        self.assertEqual(data["phases"][0]["status"], "terminal-failure")
        self.assertTrue(all(row["status"] == "not-invoked" for row in data["phases"][1:]))
        self.assertFalse(data["product_authorized"])
        self.assertFalse(data["admission_created"])

    def test_phase_failure_and_late_final_fsync_closeout_preserve_failure(self):
        plan = plan_fixture()
        calls = []

        async def fail_acquisition(_plan):
            calls.append("acquisition")
            raise RuntimeError("synthetic failure")

        async def unexpected(*_args):
            calls.append("later")
            raise AssertionError("later phase was invoked")

        executors = stage.StageExecutors(
            acquire=fail_acquisition,
            verify_acquisition=unexpected,
            capture=unexpected,
            prepare_after_acquisition=unexpected,
            verify_late_preflight=unexpected,
            grade_references=unexpected,
            selectors=unexpected,
            answerer=unexpected,
            grade_answers=unexpected,
        )
        fake_now = [plan.stage_started_monotonic + 1]
        original_write_inventory = stage._write_inventory
        terminal_writes = 0

        def write_inventory(path, value):
            nonlocal terminal_writes
            result_sha = original_write_inventory(path, value)
            if value.get("status") == "terminal-incomplete":
                terminal_writes += 1
                if terminal_writes == 3:
                    # The final inventory file+directory fsync itself crosses
                    # the stage deadline. No later correction may erase the
                    # acquisition failure that already terminated the stage.
                    fake_now[0] = plan.stage_deadline_monotonic + 1
            return result_sha

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "private"
            root.mkdir(mode=0o700)
            os.chmod(root, 0o700)
            with (
                mock.patch.object(stage, "_monotonic", side_effect=lambda: fake_now[0]),
                mock.patch.object(stage, "_write_inventory", side_effect=write_inventory),
            ):
                result = asyncio.run(
                    stage.coordinate_coverage_stage(plan=plan, executors=executors, inventory_root=root)
                )
            inventory = json.loads(result.inventory_path.read_bytes())
            observed = coverage_resource_evidence.verify_stage_closeout(
                inventory_path=result.inventory_path,
                expected_inventory_sha256=result.inventory_sha256,
                closeout_receipt_path=result.closeout_receipt_path,
                expected_closeout_receipt_sha256=result.closeout_receipt_sha256,
                expected_stage_uuid=plan.stage_uuid,
                expected_source_revision=plan.source_revision,
                protocol_bytes=plan.protocol_bytes,
                cohorts_bytes=plan.cohorts_bytes,
                expected_protocol_sha256=digest(plan.protocol_bytes),
                expected_cohorts_sha256=digest(plan.cohorts_bytes),
            )
        self.assertEqual(calls, ["acquisition"])
        self.assertEqual(result.status, "terminal-incomplete")
        self.assertEqual(result.terminal_reason, "acquisition-failed")
        self.assertEqual(inventory["terminal_reason"], "acquisition-failed")
        self.assertIs(inventory["gate_result_authoritative"], False)
        self.assertGreaterEqual(
            observed.stage_elapsed_seconds,
            plan.stage_deadline_monotonic - plan.stage_started_monotonic,
        )
        self.assertEqual(observed.terminal_reason, "acquisition-failed")
        self.assertEqual(observed.terminal_status, "terminal-incomplete")

    def test_navigation_uses_reverified_snapshot_not_mutable_acquisition_object(self):
        plan = plan_fixture()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "snapshots"
            evidence = _write_synthetic_pool_snapshots(plan, root)
            poisoned_target = plan.navigation_targets[-1]
            selector = stage.SelectorEvidence(
                stage_uuid=plan.stage_uuid,
                terminal_inventory_sha256=digest(b"selector"),
                operation_rows=(),
                research_orders={},
                navigation_rank_one={row["target_id"]: True for row in plan.navigation_targets},
                reference_grade_receipt_sha256=digest(b"reference"),
                usage_status="known",
                navigation_top1_urls={row["target_id"]: row["target_url"] for row in plan.navigation_targets},
            )
            nav_operation = evidence.result.operations[-1]
            nav_operation.canonical_response.results[0].url = "https://poison.example/"
            observations = stage._sealed_navigation_observations(plan, evidence, selector)
            canonical_target = stage.pipeline._canonical_public_url(poisoned_target["target_url"])[1]
            self.assertIn(canonical_target, observations[poisoned_target["target_id"]]["acquired_urls"])
            self.assertNotIn("https://poison.example/", observations[poisoned_target["target_id"]]["acquired_urls"])

            index = json.loads((root / "pool-snapshot-index.json").read_bytes())
            snapshot_path = root / index["operations"][-1]["file"]
            snapshot_path.write_bytes(snapshot_path.read_bytes() + b" ")
            with self.assertRaisesRegex(stage.OrchestrationError, "navigation-snapshot-reverification-failed"):
                stage._sealed_navigation_observations(plan, evidence, selector)

    def test_integrated_synthetic_stage_completes_inconclusive_without_live_authority(self):
        self._run_integrated_synthetic_stage(deadline_cross=False)

    def test_acquisition_receipts_replaced_after_phase_seal_stop_collection(self):
        self._run_integrated_synthetic_stage(deadline_cross=False, poison_receipts=True)

    def test_stage_deadline_overrun_after_final_fsync_corrects_inventory(self):
        self._run_integrated_synthetic_stage(deadline_cross=True)

    def test_stage_closeout_receipt_and_inventory_pins_reject_tampering(self):
        self._run_integrated_synthetic_stage(deadline_cross=False, tamper_closeout=True)

    def _run_integrated_synthetic_stage(
        self,
        *,
        deadline_cross: bool,
        poison_receipts: bool = False,
        tamper_closeout: bool = False,
        selector_map_registered: bool = False,
        admission_mode: str = "valid",
    ):
        plan = plan_fixture(selector_map_registered=selector_map_registered)
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            inventory_root = base / "inventory"
            inventory_root.mkdir(mode=0o700)
            os.chmod(inventory_root, 0o700)
            snapshot_root = base / "snapshots"
            capture_root = base / "capture"
            calls: list[str] = []

            async def acquire(_plan):
                calls.append("acquisition")

                navigation = {row["search_query"]: row["target_url"] for row in plan.navigation_targets}
                research_by_query = {row["search_query"]: row for row in plan.research_cases}

                def count_for(query, engine):
                    task = next(
                        (row for original, row in research_by_query.items() if original == query or original in query),
                        None,
                    )
                    if task is None:
                        return 2
                    engines = task["pool_plan"]["engines"]
                    target = 40 if task["pool_plan"]["band"] == "research_le_40" else 41
                    index = engines.index(engine)
                    quotient, remainder = divmod(target, len(engines))
                    return quotient + (1 if index < remainder else 0)

                def handler(request):
                    host = request.url.host
                    params = dict(request.url.params)
                    if host == "api.github.com":
                        query = params.get("q", "")
                        items = []
                        if query in navigation:
                            target = navigation[query]
                            name = target.removeprefix("https://github.com/")
                            items.append(
                                {
                                    "full_name": name,
                                    "html_url": target,
                                    "description": "Synthetic target repository",
                                    "stargazers_count": 100,
                                    "language": "Python",
                                }
                            )
                        count = 20 if query in navigation else count_for(query, "github")
                        for index in range(count - len(items)):
                            name = f"fixture/repository-{index}"
                            items.append(
                                {
                                    "full_name": name,
                                    "html_url": f"https://github.com/{name}",
                                    "description": "Synthetic repository evidence",
                                    "stargazers_count": index,
                                    "language": "Python",
                                }
                            )
                        return httpx.Response(200, json={"total_count": len(items), "items": items})
                    if host == "api.openalex.org":
                        count = count_for(params.get("search", ""), "openalex")
                        items = [
                            {
                                "id": f"https://openalex.org/W{index}",
                                "doi": f"https://doi.org/10.9999/fixture-{index}",
                                "title": f"Synthetic OpenAlex paper {index}",
                                "publication_date": "2025-01-01",
                                "relevance_score": 20 - index / 100,
                                "abstract_inverted_index": None,
                            }
                            for index in range(count)
                        ]
                        return httpx.Response(200, json={"results": items})
                    if host == "export.arxiv.org":
                        count = count_for(params.get("search_query", ""), "arxiv")
                        entries = "".join(
                            f"<entry><id>https://arxiv.org/abs/2610.{index:05d}</id>"
                            f"<title>Synthetic arXiv paper {index}</title><summary>Fixture abstract</summary>"
                            f"<published>2026-10-01T00:00:00Z</published></entry>"
                            for index in range(count)
                        )
                        return httpx.Response(
                            200,
                            content=(
                                "<?xml version='1.0'?><feed xmlns='http://www.w3.org/2005/Atom'>" + entries + "</feed>"
                            ).encode(),
                            headers={"Content-Type": "application/atom+xml"},
                        )
                    if host == "en.wikipedia.org":
                        if params.get("action") == "opensearch":
                            query = params.get("search", "fixture")
                            count = count_for(query, "wikipedia")
                            titles = [f"{query} topic {i}" for i in range(count)]
                            return httpx.Response(200, json=[query, titles, [""] * count, [""] * count])
                        titles = params.get("titles", "").split("|")
                        pages = {
                            str(index + 1): {"title": title, "extract": "Synthetic encyclopedia passage."}
                            for index, title in enumerate(titles)
                        }
                        return httpx.Response(200, json={"query": {"pages": pages}})
                    raise AssertionError(f"unexpected mock host: {host}")

                permit = b"synthetic-test-permit"
                acquired = await live_acquire.acquire_live_coverage_stage(
                    stage_manifest=json.loads(plan.acquisition_manifest_bytes),
                    acquisition_plan_bytes=plan.acquisition_plan_bytes,
                    permit_receipt_bytes=permit,
                    expected_permit_receipt_sha256=digest(permit),
                    permit_verifier=FakePermitVerifier(),
                    one_shot_lease=OneShot(),
                    receipt_directory=snapshot_root,
                    test_transport=httpx.MockTransport(handler),
                    pacer=FakePacer(),
                )
                evidence = stage.AcquisitionEvidence(
                    result=acquired.stage,
                    snapshots_directory=acquired.receipt_directory,
                    snapshot_index_sha256=acquired.pool_snapshot_index_sha256,
                    acquisition_manifest_sha256=acquired.stage_manifest_sha256,
                    receipt_sha256=acquired.permit_receipt_sha256,
                    source_bound_receipt_status="synthetic-test-only",
                )
                try:
                    stage._validate_acquisition(plan, evidence)
                except Exception as exc:
                    statuses = [
                        (op.operation_id, op.status, op.failure_reasons, op.pool_count)
                        for op in acquired.stage.operations
                    ]
                    raise AssertionError(statuses) from exc
                return evidence

            async def verify_acquisition(_plan, evidence):
                calls.append("verify_acquisition")
                return evidence.source_bound_receipt_status == "synthetic-test-only"

            async def prepare_after_acquisition(_plan, evidence, pipeline_inputs):
                calls.append("late_preflight")
                return _late_preflight(plan, evidence, pipeline_inputs)

            async def verify_late_preflight(_plan, evidence):
                calls.append("verify_late_preflight")
                return evidence.prepared.fresh_execution_authorized is False

            async def capture(plan_arg, pipeline_inputs):
                calls.append("capture")
                capture_root.mkdir(mode=0o700)
                os.chmod(capture_root, 0o700)
                manifest_bytes = pipeline_inputs["capture_manifest_bytes"]
                permit = b"external-permit-receipt"

                def capture_handler(request):
                    if request.method == "GET":
                        return httpx.Response(
                            200,
                            json={
                                "status": "ok",
                                "runtime": {"revision": "c" * 40, "model": "free"},
                            },
                        )
                    return httpx.Response(
                        200,
                        json={"success": True, "data": {"markdown": "Synthetic source passage."}},
                    )

                result = await source_capture.capture_sources_once(
                    protocol_bytes=plan_arg.protocol_bytes,
                    source_manifest_bytes=manifest_bytes,
                    candidate_identity_bytes=plan_arg.candidate_identity_bytes,
                    candidate_base_url="https://capture.example",
                    operator_token="synthetic-capture-token",
                    permit_receipt_bytes=permit,
                    expected_permit_receipt_sha256=digest(permit),
                    permit_verifier=CaptureVerifier(),
                    one_shot_lease=CaptureLease(),
                    receipt_root=capture_root,
                    transport=httpx.MockTransport(capture_handler),
                    stage_started_monotonic=plan_arg.stage_started_monotonic,
                    stage_deadline_monotonic=plan_arg.stage_deadline_monotonic,
                )
                inventory_bytes = (result.receipt_directory / "inventory.json").read_bytes()
                contexts = {}
                for row in result.private_inventory:
                    artifact_name = row.get("context_artifact")
                    if artifact_name:
                        contexts[artifact_name] = (result.receipt_directory / artifact_name).read_bytes()
                return stage.CaptureEvidence(
                    result=result,
                    inventory_bytes=inventory_bytes,
                    context_artifacts=contexts,
                    receipt_sha256=digest(inventory_bytes),
                )

            async def selectors(plan_arg, prepared, pipeline_inputs, closed_references):
                calls.append("selector")
                if poison_receipts:
                    receipt = snapshot_root / "http-0001-start.json"
                    receipt.write_bytes(receipt.read_bytes() + b" ")
                orders = {}
                for task in pipeline_inputs["tasks"]:
                    ids = list(task["native_order"])
                    orders[task["task_id"]] = {"w0": ids, "candidate": list(reversed(ids))}
                operation_rows = tuple(
                    {
                        "operation_id": operation_id,
                        "state": "complete-success",
                        "input_tokens": 9_999,
                        "output_tokens": 9_999,
                    }
                    for operation_id in prepared.operation_ids
                )
                return stage.SelectorEvidence(
                    stage_uuid=plan_arg.stage_uuid,
                    terminal_inventory_sha256=digest(b"synthetic selector inventory"),
                    operation_rows=operation_rows,
                    research_orders=orders,
                    navigation_rank_one={row["target_id"]: True for row in plan_arg.navigation_targets},
                    reference_grade_receipt_sha256=closed_references.receipt_sha256,
                    usage_status="known",
                    navigation_top1_urls={row["target_id"]: row["target_url"] for row in plan_arg.navigation_targets},
                )

            async def answerer(plan_arg, tasks):
                calls.append("answerer")
                from datetime import datetime, timedelta, timezone

                request_rows = answer_execution._make_requests(tasks)
                request_manifest, request_manifest_sha = answer_execution._request_manifest(request_rows)
                dirs = {}
                for name in ("leases", "archives", "results"):
                    path = base / name
                    path.mkdir(mode=0o700)
                    os.chmod(path, 0o700)
                    dirs[name] = path
                now = datetime.now(timezone.utc)
                started = now
                deadline = started + timedelta(hours=8)
                verifier = AnswerVerifier()
                lease = AnswerLease()
                task_map = {task.task_id: task for task in tasks}

                async def handler(request):
                    body = json.loads(request.content)
                    user_content = body["messages"][1]["content"]
                    if user_content.startswith("This is a synthetic endpoint readiness check"):
                        return _response('{"ready":true}')
                    task_id = json.loads(user_content)["task_id"]
                    return _response(_answer_content(task_map[task_id]))

                result = await answer_execution.execute_answer_stage(
                    stage_uuid=plan_arg.stage_uuid,
                    source_revision=plan_arg.source_revision,
                    protocol_sha256=digest(plan_arg.protocol_bytes),
                    cohorts_sha256=digest(plan_arg.cohorts_bytes),
                    answer_manifest_sha256=request_manifest_sha,
                    stage_started_utc=started.isoformat(),
                    stage_deadline_utc=deadline.isoformat(),
                    tasks=tasks,
                    endpoint="https://answer.example/v1",
                    api_key="synthetic-test-key",
                    permit_bytes=b"synthetic-test-permit",
                    expected_permit_sha256=digest(b"synthetic-test-permit"),
                    permit_verifier=verifier,
                    one_shot_lease=lease,
                    lease_root=dirs["leases"],
                    archive_root=dirs["archives"],
                    result_root=dirs["results"],
                    transport=httpx.MockTransport(handler),
                )
                terminal_path = dirs["results"] / f"{plan_arg.stage_uuid}.answer-terminal-inventory.json"
                restored = {task.task_id: {} for task in tasks}
                for task in tasks:
                    for arm in ("w0", "candidate"):
                        op_id = f"answer-{task.task_id}-{arm}"
                        answer_path = dirs["results"] / f"{plan_arg.stage_uuid}.{op_id}.answer.json"
                        restored[task.task_id][arm] = json.loads(answer_path.read_bytes())
                answer_inputs = {task.task_id: {answer.arm: answer.body for answer in task.answers} for task in tasks}
                return stage.AnswerEvidence(
                    result=result,
                    answer_inputs=answer_inputs,
                    restored_answers=restored,
                    terminal_inventory_bytes=terminal_path.read_bytes(),
                    archive_root=dirs["archives"],
                    result_root=dirs["results"],
                )

            async def grade_answers(prepared_answers):
                calls.append("answer_grading")
                return _answer_outputs(prepared_answers, reference_packets)

            # The grader helper needs the exact prepared source packet set. Keep
            # this test executor closure local and bind it before answer grading.
            reference_packets = None

            async def grading_refs(prepared):
                nonlocal reference_packets
                reference_packets = prepared
                calls.append("reference_grading")
                return _pre_submissions(prepared)

            selector_callback = selectors
            verify_admission = None
            selector_roots = None
            if selector_map_registered:
                from scripts import coverage_selector_admission as admission_module

                selector_roots = SelectorRoots()
                response_by_operation = {}
                selector_transport_dispatches = []

                async def resolve_selector_permits(plan_arg, prepared, _map_bytes, _map_sha, operation_materials):
                    permits = {}
                    for operation_id in prepared.operation_ids:
                        material = operation_materials[operation_id]
                        input_bytes = material["operation_input_bytes"]
                        legacy_control = material["legacy_control"]
                        request_body, compiled = coverage_jev_execution.build_selector_request(
                            operation_id=operation_id,
                            operation_input_bytes=input_bytes,
                            legacy_control=legacy_control,
                        )
                        parser_mode = "coverage" if compiled is not None else "original-v1"
                        permits[operation_id] = selector_permit_for(
                            prepared, operation_id, input_bytes, request_body, parser_mode=parser_mode
                        )
                        response_by_operation[operation_id] = (
                            selector_valid_response(compiled)
                            if compiled is not None
                            else legacy_selector_response(
                                {f"c{index}": 7 for index, _ in enumerate(legacy_control["canonical_pool"])}
                            )
                        )
                    return permits

                def selector_transport_factory(operation_id):
                    async def handler(request):
                        selector_transport_dispatches.append(operation_id)
                        return httpx.Response(200, content=response_by_operation[operation_id], request=request)

                    return httpx.MockTransport(handler)

                def selector_evidence_builder(
                    plan_arg, prepared, pipeline_inputs, closed_references, results, terminal
                ):
                    calls.append("selector")
                    terminal_bytes = (
                        selector_roots.result / f"{plan_arg.stage_uuid}.terminal-inventory.json"
                    ).read_bytes()
                    by_operation = {row.operation_id: row for row in results}
                    research_orders = {}
                    for task_index, task in enumerate(pipeline_inputs["tasks"], start=1):
                        candidate = by_operation[f"research-{task_index:02d}-base-candidate"]
                        w0 = by_operation[f"research-{task_index:02d}-base-w0"]
                        candidate_order = candidate.ranking.ordered_ids if candidate.ranking is not None else ()
                        frozen_order = (
                            w0.native_ordered_ids
                            if w0.native_ordered_ids is not None
                            else w0.ranking.ordered_ids if w0.ranking is not None else ()
                        )
                        # W0's frozen 40-card request uses local cN identifiers;
                        # restore those ranks to the sealed pool IDs and append
                        # the untouched pool tail for 41–80-card tasks.
                        w0_order = tuple(
                            task["native_order"][int(card_id.removeprefix("c"))]
                            for card_id in frozen_order
                        ) + tuple(task["native_order"][len(frozen_order) :])
                        research_orders[task["task_id"]] = {
                            "w0": list(w0_order),
                            "candidate": list(candidate_order),
                        }
                        if any(
                            len(research_orders[task["task_id"]][arm]) != len(task["native_order"])
                            or set(research_orders[task["task_id"]][arm]) != set(task["native_order"])
                            for arm in ("w0", "candidate")
                        ):
                            raise AssertionError(
                                f"synthetic selector response did not produce full permutation for {task['task_id']}"
                            )
                    navigation_rank_one = {}
                    navigation_top1_urls = {}
                    for target in plan_arg.navigation_targets:
                        target_id = target["target_id"]
                        navigation_rank_one[target_id] = True
                        navigation_top1_urls[target_id] = target["target_url"]
                    return stage.SelectorEvidence(
                        stage_uuid=plan_arg.stage_uuid,
                        terminal_inventory_sha256=terminal["sha256"],
                        operation_rows=tuple(
                            {"operation_id": operation_id, "state": "complete-success"}
                            for operation_id in prepared.operation_ids
                        ),
                        research_orders=research_orders,
                        navigation_rank_one=navigation_rank_one,
                        reference_grade_receipt_sha256=closed_references.receipt_sha256,
                        usage_status="known",
                        navigation_top1_urls=navigation_top1_urls,
                        terminal_inventory_bytes=terminal_bytes,
                        result_root=selector_roots.result,
                        archive_root=selector_roots.archive,
                    )

                selector_callback = stage.registered_selector_executor_factory(
                    permit_resolver=resolve_selector_permits,
                    api_key="synthetic-registered-selector-key",
                    lease_root=selector_roots.lease,
                    archive_root=selector_roots.archive,
                    result_root=selector_roots.result,
                    evidence_builder=selector_evidence_builder,
                    transport_factory=selector_transport_factory,
                )

                async def verify_admission(plan_arg, prepared, map_bytes, map_sha):
                    calls.append("verify_selector_admission")
                    expected = {
                        "schema": admission_module.SCHEMA,
                        "status": "admitted",
                        "stage_uuid": prepared.stage_uuid,
                        "source_revision": prepared.source_revision,
                        "protocol_sha256": digest(plan_arg.protocol_bytes),
                        "registration_sha256": prepared.registration_sha256,
                        "source_closure_sha256": prepared.pins["qualified_source_closure"],
                        "selector_input_map_sha256": map_sha,
                    }
                    if admission_mode == "wrong":
                        expected["selector_input_map_sha256"] = "0" * 64
                    receipt_bytes = canonical(expected)
                    return admission_module.verify_selector_map_admission(
                        receipt_bytes=receipt_bytes,
                        expected_receipt_sha256=digest(receipt_bytes),
                        protocol_bytes=plan_arg.protocol_bytes,
                        prepared=prepared,
                        selector_input_map_sha256=map_sha,
                    )

                verify_admission = None if admission_mode == "missing" else verify_admission

            executors = stage.StageExecutors(
                acquire=acquire,
                verify_acquisition=verify_acquisition,
                capture=capture,
                prepare_after_acquisition=prepare_after_acquisition,
                verify_late_preflight=verify_late_preflight,
                grade_references=grading_refs,
                selectors=selector_callback,
                answerer=answerer,
                grade_answers=grade_answers,
                verify_selector_admission=verify_admission,
            )
            fake_now = [plan.stage_started_monotonic + 1]
            original_write_inventory = stage._write_inventory

            def write_inventory(path, value):
                result_sha = original_write_inventory(path, value)
                if deadline_cross and value.get("gate_status") == "inconclusive":
                    fake_now[0] = plan.stage_deadline_monotonic + 1
                return result_sha

            with (
                mock.patch.object(stage, "_monotonic", side_effect=lambda: fake_now[0]),
                mock.patch.object(stage, "_write_inventory", side_effect=write_inventory),
            ):
                result = asyncio.run(
                    stage.coordinate_coverage_stage(plan=plan, executors=executors, inventory_root=inventory_root)
                )
            inventory = json.loads(result.inventory_path.read_bytes())
            closeout = resource_evidence.verify_stage_closeout(
                inventory_path=result.inventory_path,
                expected_inventory_sha256=result.inventory_sha256,
                closeout_receipt_path=result.closeout_receipt_path,
                expected_closeout_receipt_sha256=result.closeout_receipt_sha256,
                expected_stage_uuid=plan.stage_uuid,
                expected_source_revision=plan.source_revision,
                protocol_bytes=plan.protocol_bytes,
                cohorts_bytes=plan.cohorts_bytes,
                expected_protocol_sha256=digest(plan.protocol_bytes),
                expected_cohorts_sha256=digest(plan.cohorts_bytes),
            )
            self.assertEqual(closeout.stage_elapsed_seconds, result.stage_elapsed_seconds)
            self.assertEqual(closeout.inventory_sha256, result.inventory_sha256)
            self.assertEqual(closeout.terminal_status, result.status)
            closeout_bytes = result.closeout_receipt_path.read_bytes()
            closeout_document = json.loads(closeout_bytes)
            self.assertEqual(closeout_document["final_inventory_sha256"], result.inventory_sha256)
            self.assertIn("closeout receipt fsync excluded", closeout_document["measurement_basis"])
            if selector_map_registered and admission_mode != "valid":
                self.assertEqual(result.status, "terminal-incomplete", inventory)
                self.assertEqual(inventory["phases"][7]["status"], "not-invoked")
                self.assertEqual(inventory["terminal_reason"], "orchestration-binding-failed")
                self.assertEqual(selector_transport_dispatches, [])
                self.assertEqual(list(selector_roots.lease.iterdir()), [])
                self.assertNotIn("answerer", calls)
                return
            if selector_map_registered:
                self.assertEqual(result.status, "pending-independent-closeout", inventory)
            if not deadline_cross and not poison_receipts and not tamper_closeout:
                finalization = stage_finalization.verify_stage_finalization(
                    inventory_path=result.inventory_path,
                    expected_inventory_sha256=result.inventory_sha256,
                    closeout_receipt_path=result.closeout_receipt_path,
                    expected_closeout_receipt_sha256=result.closeout_receipt_sha256,
                    pending_time_receipt_path=result.inventory_path.parent / inventory["pending_time_receipt_file"],
                    expected_pending_time_receipt_sha256=inventory["pending_time_receipt_sha256"],
                    gate_input_manifest_path=result.inventory_path.parent / inventory["gate_input_manifest_file"],
                    expected_gate_input_manifest_sha256=inventory["gate_input_manifest_sha256"],
                    gate_calculation_receipt_path=result.inventory_path.parent
                    / inventory["gate_calculation_receipt_file"],
                    expected_gate_calculation_receipt_sha256=inventory["gate_calculation_receipt_sha256"],
                    expected_stage_uuid=plan.stage_uuid,
                    expected_source_revision=plan.source_revision,
                    protocol_bytes=plan.protocol_bytes,
                    cohorts_bytes=plan.cohorts_bytes,
                    expected_protocol_sha256=digest(plan.protocol_bytes),
                    expected_cohorts_sha256=digest(plan.cohorts_bytes),
                )
                self.assertEqual(finalization.status, "gates-inconclusive")
                self.assertFalse(finalization.eligible_for_independent_decision)
            if tamper_closeout:
                result.closeout_receipt_path.write_bytes(closeout_bytes + b" ")
                with self.assertRaisesRegex(resource_evidence.ResourceEvidenceError, "receipt-pin-mismatch"):
                    resource_evidence.verify_stage_closeout(
                        inventory_path=result.inventory_path,
                        expected_inventory_sha256=result.inventory_sha256,
                        closeout_receipt_path=result.closeout_receipt_path,
                        expected_closeout_receipt_sha256=result.closeout_receipt_sha256,
                        expected_stage_uuid=plan.stage_uuid,
                        expected_source_revision=plan.source_revision,
                        protocol_bytes=plan.protocol_bytes,
                        cohorts_bytes=plan.cohorts_bytes,
                        expected_protocol_sha256=digest(plan.protocol_bytes),
                        expected_cohorts_sha256=digest(plan.cohorts_bytes),
                    )
                result.closeout_receipt_path.write_bytes(closeout_bytes)
                result.inventory_path.write_bytes(result.inventory_path.read_bytes() + b" ")
                with self.assertRaisesRegex(resource_evidence.ResourceEvidenceError, "inventory-pin-mismatch"):
                    resource_evidence.verify_stage_closeout(
                        inventory_path=result.inventory_path,
                        expected_inventory_sha256=result.inventory_sha256,
                        closeout_receipt_path=result.closeout_receipt_path,
                        expected_closeout_receipt_sha256=result.closeout_receipt_sha256,
                        expected_stage_uuid=plan.stage_uuid,
                        expected_source_revision=plan.source_revision,
                        protocol_bytes=plan.protocol_bytes,
                        cohorts_bytes=plan.cohorts_bytes,
                        expected_protocol_sha256=digest(plan.protocol_bytes),
                        expected_cohorts_sha256=digest(plan.cohorts_bytes),
                    )
                return
            if poison_receipts:
                self.assertEqual(result.status, "terminal-incomplete", inventory)
                self.assertEqual(result.terminal_reason, "resource_evidence_collection-failed")
                self.assertEqual(inventory["phases"][14]["status"], "terminal-failure")
                self.assertRegex(inventory["acquisition_receipt_inventory_sha256"], r"^[0-9a-f]{64}$")
                self.assertFalse((result.inventory_path.parent / "gate-calculation-receipt.json").exists())
                self.assertFalse(result.product_authorized)
                return
            receipt_bytes = (result.inventory_path.parent / inventory["gate_calculation_receipt_file"]).read_bytes()
            resource_bytes = (result.inventory_path.parent / inventory["resource_evidence_receipt_file"]).read_bytes()

        if deadline_cross:
            self.assertEqual(result.status, "terminal-incomplete", inventory)
            self.assertEqual(result.terminal_reason, "stage-deadline-exceeded-after-final-fsync")
            self.assertFalse(inventory["gate_result_authoritative"])
            original_sha = inventory["deadline_overrun_correction"]["original_inventory_sha256"]
            self.assertRegex(original_sha, r"^[0-9a-f]{64}$")
            self.assertNotEqual(original_sha, result.inventory_sha256)
            self.assertGreaterEqual(
                closeout.stage_elapsed_seconds,
                plan.stage_deadline_monotonic - plan.stage_started_monotonic,
            )
        else:
            self.assertEqual(result.status, "pending-independent-closeout", inventory)
            self.assertEqual(result.terminal_reason, "gate-inconclusive")
            self.assertFalse(inventory["gate_result_authoritative"])
        self.assertFalse(result.product_authorized)
        self.assertFalse(result.admission_created)
        self.assertFalse(result.scientific_calls_made_by_coordinator)
        self.assertEqual([row["status"] for row in inventory["phases"]], ["complete"] * 16)
        self.assertEqual(inventory["per_band_qualification"], "diagnostic-only")
        self.assertEqual(len(calls), 10 if selector_map_registered else 9)
        self.assertEqual(inventory["gate_metrics"]["navigation"], {f"D-N0{i}": True for i in range(1, 6)})
        self.assertIsNone(inventory["gate_metrics"]["resources"].get("pass"))
        self.assertEqual(digest(receipt_bytes), inventory["gate_calculation_receipt_sha256"])
        self.assertEqual(json.loads(receipt_bytes)["status"], "inconclusive")
        resource_receipt = json.loads(resource_bytes)
        self.assertEqual(digest(resource_bytes), inventory["resource_evidence_receipt_sha256"])
        self.assertEqual(resource_receipt["schema"], "coverage-resource-evidence/2")
        if not selector_map_registered:
            self.assertIsNone(resource_receipt["observations"].get("selector_elapsed_ms"))
        # The callback's plausible token claims have no archived terminal/result
        # chain and therefore remain unknown; resource collection ignores them.
        if not selector_map_registered:
            self.assertIsNone(resource_receipt["observations"].get("selector_usage"))

    def test_registered_map_executes_full_offline_graph_after_external_admission(self):
        self._run_integrated_synthetic_stage(deadline_cross=False, selector_map_registered=True)

    def test_registered_map_missing_or_wrong_admission_has_zero_selector_dispatch(self):
        for mode in ("missing", "wrong"):
            with self.subTest(mode=mode):
                self._run_integrated_synthetic_stage(
                    deadline_cross=False,
                    selector_map_registered=True,
                    admission_mode=mode,
                )

    def test_gate_threshold_binding_uses_frozen_gate_subset(self):
        plan = plan_fixture()
        protocol = json.loads(plan.protocol_bytes)
        identity = {
            "thresholds_sha256": digest(canonical({"quality": protocol["quality"], "task_use": protocol["task_use"]}))
        }
        expected = digest(b"gate-input")
        evaluation = stage.GateEvaluation(
            status="pass",
            scope="mixed-workload-only",
            input_manifest_sha256=expected,
            threshold_sha256=identity["thresholds_sha256"],
            gates={"g1": True},
            band_diagnostics={"research_le_40": {}, "research_41_80": {}},
            metrics_sha256="a" * 64,
            calculation_receipt_sha256="b" * 64,
        )
        stage._check_gates(evaluation, expected, plan, identity)
        changed = stage.GateEvaluation(**{**evaluation.__dict__, "threshold_sha256": digest(plan.protocol_bytes)})
        with self.assertRaisesRegex(stage.OrchestrationError, "gate-threshold-binding"):
            stage._check_gates(changed, expected, plan, identity)


if __name__ == "__main__":
    unittest.main()
