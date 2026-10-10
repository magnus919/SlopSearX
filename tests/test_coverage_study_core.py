import hashlib
import json
import unittest
import uuid

from scripts import coverage_study_core as core


def canonical(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


STAGE = str(uuid.UUID(int=101))
OLD_STAGE = str(uuid.UUID(int=100))
DEV_STAGE = str(uuid.UUID(int=99))
SOURCE_REVISION = "a" * 40
TASKS = tuple([f"research-{i}" for i in range(1, 9)] + [f"navigation-{i}" for i in range(1, 6)])
NAV_SKIPS = (None, None, None, None, None)


def make_fixture(*, stage_uuid=STAGE, stage_kind="development", navigation_skips=NAV_SKIPS):
    tasks = TASKS
    coverage_source = b"pinned coverage source bytes"
    production_source = b"pinned production rerank bytes"
    dependency_lock = b"pinned dependency closure bytes"
    source_pins = {
        "coverage_source": digest(coverage_source),
        "production_rerank_source": digest(production_source),
        "dependency_lock": digest(dependency_lock),
    }
    source_closure = canonical(
        {
            "schema": "coverage-study-qualified-source-closure/1",
            "source_revision": SOURCE_REVISION,
            "material_pins": source_pins,
        }
    )
    protocol = b"frozen protocol bytes"
    research_tasks = []
    for task_index in range(1, 9):
        candidates = [
            {
                "id": f"c{task_index:02d}-{card_index:03d}",
                "title": f"Title {task_index}-{card_index}",
                "url": f"https://example.test/{task_index}/{card_index}",
                "snippet": f"Synthetic card {task_index}-{card_index}",
            }
            for card_index in range(41 if task_index <= 4 else 20)
        ]
        task = {
            "task_id": f"research-{task_index}",
            "query": f"synthetic query {task_index}",
            "purpose": f"synthetic purpose {task_index}",
            "facets": [
                {"id": "facet-a", "description": "Synthetic facet A"},
                {"id": "facet-b", "description": "Synthetic facet B"},
            ],
            "candidates": candidates,
            "incumbent_order": [candidate["id"] for candidate in candidates],
        }
        compiled = core.coverage.compile_request(
            query=task["query"],
            purpose=task["purpose"],
            facets=task["facets"],
            candidates=task["candidates"],
            incumbent_order=task["incumbent_order"],
        )
        task["compiled_request_sha256"] = digest(compiled.body)
        research_tasks.append(task)
    navigation_tasks = [
        {
            "task_id": f"navigation-{task_index}",
            "query": f"synthetic navigation query {task_index}",
            "target_candidate_id": f"target-{task_index}",
            "pool_ids": [f"target-{task_index}", f"other-{task_index}"],
            "w0_order": [f"target-{task_index}", f"other-{task_index}"],
        }
        for task_index in range(1, 6)
    ]
    input_manifest = canonical(
        {
            "schema": core.INPUT_SCHEMA,
            "stage_uuid": stage_uuid,
            "research_tasks": research_tasks,
            "navigation_tasks": navigation_tasks,
        }
    )
    source_capture = canonical({"schema": "coverage-source-capture-manifest/1", "stage_uuid": stage_uuid})
    references = canonical({"schema": "coverage-blind-reference-manifest/1", "stage_uuid": stage_uuid})
    answer_plan = canonical({"schema": "coverage-answer-assessment-plan/1", "stage_uuid": stage_uuid})
    acquisition_plan = canonical(
        {
            "schema": core.ACQUISITION_SCHEMA,
            "stage_uuid": stage_uuid,
            "tasks": [
                {
                    "task_id": task,
                    "engine_calls": {"wikipedia": 1, "arxiv": 1, "github": 0, "openalex": 0},
                }
                for task in tasks
            ],
        }
    )
    capture_plan = canonical(
        {
            "schema": core.CAPTURE_SCHEMA,
            "stage_uuid": stage_uuid,
            "tasks": [{"task_id": task, "scraper_calls": 1} for task in tasks],
            "health_calls": 1,
        }
    )
    materials = {
        "protocol": protocol,
        "qualified_source_closure": source_closure,
        "coverage_source": coverage_source,
        "production_rerank_source": production_source,
        "dependency_lock": dependency_lock,
        "task_input_manifest": input_manifest,
        "source_capture_manifest": source_capture,
        "reference_manifest": references,
        "answer_assessment_plan": answer_plan,
        "acquisition_plan": acquisition_plan,
        "capture_plan": capture_plan,
    }
    pins = {name: digest(raw) for name, raw in materials.items()}
    operations = core.selector_inventory(navigation_skips)
    registration = canonical(
        {
            "schema": core.REGISTRATION_SCHEMA,
            "stage_uuid": stage_uuid,
            "stage_kind": stage_kind,
            "source_revision": SOURCE_REVISION,
            "pins": pins,
            "operation_ids": [item.operation_id for item in operations],
            "navigation_skip_reasons": list(navigation_skips),
            "task_ids": list(tasks),
        }
    )
    args = {
        "registration_bytes": registration,
        "expected_registration_sha256": digest(registration),
        "expected_stage_uuid": stage_uuid,
        "forbidden_stage_uuids": (OLD_STAGE,),
        "expected_source_revision": SOURCE_REVISION,
        "observed_source_revision": SOURCE_REVISION,
        "materials": materials,
        "expected_pins": pins,
        "acquisition_plan_bytes": acquisition_plan,
        "capture_plan_bytes": capture_plan,
        "task_ids": tasks,
        "navigation_skip_reasons": navigation_skips,
    }
    return args, materials, pins, registration


def reseal_fixture(args, materials, *, nav_skips=NAV_SKIPS, stage_kind="development", stage_uuid=STAGE):
    pins = {name: digest(raw) for name, raw in materials.items()}
    ops = core.selector_inventory(nav_skips)
    registration = canonical(
        {
            "schema": core.REGISTRATION_SCHEMA,
            "stage_uuid": stage_uuid,
            "stage_kind": stage_kind,
            "source_revision": SOURCE_REVISION,
            "pins": pins,
            "operation_ids": [call.operation_id for call in ops],
            "navigation_skip_reasons": list(nav_skips),
            "task_ids": list(TASKS),
        }
    )
    args.update(
        registration_bytes=registration,
        expected_registration_sha256=digest(registration),
        materials=materials,
        expected_pins=pins,
        acquisition_plan_bytes=materials["acquisition_plan"],
        capture_plan_bytes=materials["capture_plan"],
        navigation_skip_reasons=nav_skips,
    )
    return args


def make_run(navigation_skips=NAV_SKIPS):
    args, *_ = make_fixture(navigation_skips=navigation_skips)
    prepared = core.preflight_stage(**args)
    return core.StudyRun(prepared), prepared.operation_ids


class RegistrationPreflightTests(unittest.TestCase):
    def test_valid_development_is_preflight_only_and_never_authorizes(self):
        args, *_ = make_fixture()
        prepared = core.preflight_stage(**args)
        self.assertEqual(prepared.status, "preflight-verified-not-admitted")
        self.assertFalse(prepared.fresh_execution_authorized)
        self.assertEqual(len(prepared.operation_ids), 39)
        self.assertEqual(prepared.acquisition_calls, 26)
        self.assertEqual(prepared.scraper_calls, 13)
        self.assertEqual(prepared.health_calls, 1)
        self.assertFalse(prepared.selector_input_map_required)
        self.assertFalse(hasattr(prepared, "clock"))
        self.assertFalse(hasattr(prepared, "permit"))

    def test_versioned_v2_protocol_persistently_requires_selector_input_map(self):
        args, materials, _, _ = make_fixture()
        materials["protocol"] = canonical(
            {
                "schema": "coverage-first-study-protocol/2-draft",
                "selector_input_map_schema": "coverage-selector-input-map/2-draft",
            }
        )
        reseal_fixture(args, materials)
        prepared = core.preflight_stage(**args)
        self.assertTrue(prepared.selector_input_map_required)

        args, materials, _, _ = make_fixture()
        materials["protocol"] = canonical({"selector_input_map_schema": "unsupported-map/99"})
        reseal_fixture(args, materials)
        with self.assertRaisesRegex(core.StudyError, "selector-input-map-schema-unsupported"):
            core.preflight_stage(**args)

    def test_registered_map_schema_requires_registered_status_and_external_pin(self):
        args, materials, _, _ = make_fixture()
        materials["protocol"] = canonical(
            {
                "schema": "coverage-first-study-protocol/2",
                "selector_input_map_schema": "coverage-selector-input-map/2-registered",
                "selector_input_map_status": "registered",
            }
        )
        reseal_fixture(args, materials)
        prepared = core.preflight_stage(**args)
        self.assertTrue(prepared.selector_input_map_required)
        self.assertEqual(prepared.selector_input_map_schema, "coverage-selector-input-map/2-registered")
        self.assertFalse(prepared.fresh_execution_authorized)

        args, materials, _, _ = make_fixture()
        materials["protocol"] = canonical(
            {"selector_input_map_schema": "coverage-selector-input-map/2-registered"}
        )
        reseal_fixture(args, materials)
        with self.assertRaisesRegex(core.StudyError, "selector-input-map-registration-status-required"):
            core.preflight_stage(**args)

    def test_registration_and_materials_require_external_hashes(self):
        args, materials, _, _ = make_fixture()
        changed = dict(materials)
        changed["coverage_source"] = b"changed code bytes"
        args["materials"] = changed
        with self.assertRaisesRegex(core.StudyError, "material-pin-mismatch:coverage_source"):
            core.preflight_stage(**args)

        args, *_ = make_fixture()
        args["expected_registration_sha256"] = "0" * 64
        with self.assertRaisesRegex(core.StudyError, "registration-digest-mismatch"):
            core.preflight_stage(**args)

    def test_old_uuid_and_extra_clock_authority_are_rejected(self):
        args, *_ = make_fixture()
        args["expected_stage_uuid"] = OLD_STAGE
        with self.assertRaisesRegex(core.StudyError, "stage-uuid-not-fresh"):
            core.preflight_stage(**args)

        args, materials, _, registration = make_fixture()
        document = json.loads(registration)
        document["legacy_clock"] = {"stage_uuid": OLD_STAGE}
        registration = canonical(document)
        args["registration_bytes"] = registration
        args["expected_registration_sha256"] = digest(registration)
        with self.assertRaisesRegex(core.StudyError, "registration-schema"):
            core.preflight_stage(**args)

        args, _, _, registration = make_fixture()
        document = json.loads(registration)
        document["operation_ids"][0] = False
        registration = canonical(document)
        args["registration_bytes"] = registration
        args["expected_registration_sha256"] = digest(registration)
        with self.assertRaisesRegex(core.StudyError, "operation-inventory-mismatch"):
            core.preflight_stage(**args)

    def test_source_revision_is_only_a_label_without_matching_sealed_closure(self):
        args, materials, _, _ = make_fixture()
        closure = json.loads(materials["qualified_source_closure"])
        closure["source_revision"] = "b" * 40
        materials["qualified_source_closure"] = canonical(closure)
        reseal_fixture(args, materials)
        with self.assertRaisesRegex(core.StudyError, "qualified-source-revision-mismatch"):
            core.preflight_stage(**args)

    def test_source_closure_rejects_resealed_dependency_pin_drift(self):
        args, materials, _, _ = make_fixture()
        closure = json.loads(materials["qualified_source_closure"])
        closure["material_pins"]["dependency_lock"] = "f" * 64
        materials["qualified_source_closure"] = canonical(closure)
        reseal_fixture(args, materials)
        with self.assertRaisesRegex(core.StudyError, "qualified-source-material-pin-mismatch"):
            core.preflight_stage(**args)

    def test_input_manifest_must_be_stage_bound_and_exact_task_inventory(self):
        args, materials, _, _ = make_fixture()
        bad = json.loads(materials["task_input_manifest"])
        bad["research_tasks"][-1]["task_id"] = "old-research"
        materials["task_input_manifest"] = canonical(bad)
        reseal_fixture(args, materials)
        with self.assertRaisesRegex(core.StudyError, "input-research-task-shape"):
            core.preflight_stage(**args)

        args, materials, _, _ = make_fixture()
        bad = json.loads(materials["reference_manifest"])
        bad["stage_uuid"] = OLD_STAGE
        materials["reference_manifest"] = canonical(bad)
        reseal_fixture(args, materials)
        with self.assertRaisesRegex(core.StudyError, "reference_manifest-stage-mismatch"):
            core.preflight_stage(**args)

    def test_input_preflight_requires_four_natural_size_41_to_80_candidate_pools(self):
        args, materials, _, _ = make_fixture()
        doc = json.loads(materials["task_input_manifest"])
        for row in doc["research_tasks"][:4]:
            row["candidates"].pop()
            row["incumbent_order"].pop()
            compiled = core.coverage.compile_request(
                query=row["query"],
                purpose=row["purpose"],
                facets=row["facets"],
                candidates=row["candidates"],
                incumbent_order=row["incumbent_order"],
            )
            row["compiled_request_sha256"] = digest(compiled.body)
        materials["task_input_manifest"] = canonical(doc)
        reseal_fixture(args, materials)
        with self.assertRaisesRegex(core.StudyError, "input-large-pool-minimum"):
            core.preflight_stage(**args)

    def test_strict_json_rejects_duplicate_nonfinite_and_large_integer_failures(self):
        for raw in (
            b'{"schema":"x","schema":"y"}',
            b'{"schema":"x","v":NaN}',
            b'{"schema":"x","v":' + b"9" * 5000 + b"}",
        ):
            with self.subTest(raw=raw[:32]):
                with self.assertRaises(core.StudyError):
                    core._strict_json(raw, "test")

    def test_confirmation_requires_separately_pinned_development_pass(self):
        args, materials, pins, _ = make_fixture(stage_uuid=STAGE, stage_kind="confirmation")
        with self.assertRaisesRegex(core.StudyError, "confirmation-needs-sealed-development-pass"):
            core.preflight_stage(**args)

        development = canonical(
            {
                "schema": core.DEVELOPMENT_RECEIPT_SCHEMA,
                "stage_uuid": DEV_STAGE,
                "stage_kind": "development",
                "status": "passed",
                "protocol_sha256": pins["protocol"],
                "source_revision": SOURCE_REVISION,
                "source_closure_sha256": pins["qualified_source_closure"],
                "result_sha256": "e" * 64,
            }
        )
        args["development_receipt_bytes"] = development
        args["expected_development_receipt_sha256"] = digest(development)
        args["expected_development_stage_uuid"] = DEV_STAGE
        prepared = core.preflight_stage(**args)
        self.assertFalse(prepared.fresh_execution_authorized)
        self.assertEqual(prepared.stage_kind, "confirmation")

        failed = json.loads(development)
        failed["status"] = "inconclusive"
        failed_bytes = canonical(failed)
        args["development_receipt_bytes"] = failed_bytes
        args["expected_development_receipt_sha256"] = digest(failed_bytes)
        with self.assertRaisesRegex(core.StudyError, "development-stage-not-passed"):
            core.preflight_stage(**args)

        wrong_source = json.loads(development)
        wrong_source["status"] = "passed"
        wrong_source["source_revision"] = "b" * 40
        wrong_source_bytes = canonical(wrong_source)
        args["development_receipt_bytes"] = wrong_source_bytes
        args["expected_development_receipt_sha256"] = digest(wrong_source_bytes)
        with self.assertRaisesRegex(core.StudyError, "development-source-revision-mismatch"):
            core.preflight_stage(**args)

        args["development_receipt_bytes"] = development
        args["expected_development_receipt_sha256"] = digest(development)
        args["expected_development_stage_uuid"] = OLD_STAGE
        with self.assertRaisesRegex(core.StudyError, "development-stage-not-passed"):
            core.preflight_stage(**args)


class ScheduleAndPlanTests(unittest.TestCase):
    def test_frozen_stability_task_indices_are_1_4_7_8(self):
        calls = core.selector_inventory([None] * 5)
        self.assertEqual(len(calls), 39)
        variants = {call.operation_id.split("-")[1] for call in calls if "-repeat-" in call.operation_id}
        self.assertEqual(variants, {"01", "04", "07", "08"})
        self.assertEqual(sum(call.family == "navigation" for call in calls), 5)
        self.assertLessEqual(len(calls), 44)
        with self.assertRaisesRegex(core.StudyError, "navigation-skip-not-production-guard"):
            core.selector_inventory(["native-top1", None, None, None, None])

    def test_acquisition_plan_uses_normalized_names_and_per_task_limits(self):
        args, materials, pins, _ = make_fixture()
        self.assertEqual(
            core.validate_acquisition_plan(
                materials["acquisition_plan"],
                expected_sha256=pins["acquisition_plan"],
                expected_stage_uuid=STAGE,
                expected_task_ids=TASKS,
            ),
            26,
        )
        doc = json.loads(materials["acquisition_plan"])
        doc["tasks"][0]["engine_calls"]["wiki"] = 1
        del doc["tasks"][0]["engine_calls"]["wikipedia"]
        bad = canonical(doc)
        with self.assertRaisesRegex(core.StudyError, "acquisition-engine-names"):
            core.validate_acquisition_plan(
                bad,
                expected_sha256=digest(bad),
                expected_stage_uuid=STAGE,
                expected_task_ids=TASKS,
            )

    def test_acquisition_plan_has_53_call_stage_limit_and_exact_task_rows(self):
        args, materials, pins, _ = make_fixture()
        doc = json.loads(materials["acquisition_plan"])
        for row in doc["tasks"]:
            row["engine_calls"] = {"wikipedia": 2, "arxiv": 2, "github": 1, "openalex": 1}
        over = canonical(doc)
        with self.assertRaisesRegex(core.StudyError, "acquisition-stage-limit"):
            core.validate_acquisition_plan(
                over,
                expected_sha256=digest(over),
                expected_stage_uuid=STAGE,
                expected_task_ids=TASKS,
            )
        doc["tasks"].pop()
        missing = canonical(doc)
        with self.assertRaisesRegex(core.StudyError, "acquisition-task-count"):
            core.validate_acquisition_plan(
                missing,
                expected_sha256=digest(missing),
                expected_stage_uuid=STAGE,
                expected_task_ids=TASKS,
            )

    def test_capture_plan_enforces_640_plus_one_separate_health(self):
        args, materials, pins, _ = make_fixture()
        self.assertEqual(
            core.validate_capture_plan(
                materials["capture_plan"],
                expected_sha256=pins["capture_plan"],
                expected_stage_uuid=STAGE,
                expected_task_ids=TASKS,
            ),
            13,
        )
        doc = json.loads(materials["capture_plan"])
        for row in doc["tasks"]:
            row["scraper_calls"] = 50
        over = canonical(doc)
        with self.assertRaisesRegex(core.StudyError, "capture-stage-limit"):
            core.validate_capture_plan(
                over,
                expected_sha256=digest(over),
                expected_stage_uuid=STAGE,
                expected_task_ids=TASKS,
            )
        doc["health_calls"] = 0
        bad_health = canonical(doc)
        with self.assertRaisesRegex(core.StudyError, "capture-health-call-count"):
            core.validate_capture_plan(
                bad_health,
                expected_sha256=digest(bad_health),
                expected_stage_uuid=STAGE,
                expected_task_ids=TASKS,
            )


class OneShotBudgetAndInventoryTests(unittest.TestCase):
    def test_ledger_requires_a_pinned_preflight_schedule(self):
        with self.assertRaisesRegex(core.StudyError, "prepared-stage-required"):
            core.StudyRun(["arbitrary-operation"])

        prepared, *_ = make_fixture()
        stage = core.preflight_stage(**prepared)
        altered = core.PreparedStage(
            stage_uuid=stage.stage_uuid,
            stage_kind=stage.stage_kind,
            source_revision=stage.source_revision,
            registration_sha256=stage.registration_sha256,
            pins=stage.pins,
            navigation_skip_reasons=stage.navigation_skip_reasons,
            operation_ids=("arbitrary-operation",),
            acquisition_calls=stage.acquisition_calls,
            scraper_calls=stage.scraper_calls,
            health_calls=stage.health_calls,
        )
        with self.assertRaisesRegex(core.StudyError, "prepared-operation-inventory-mismatch"):
            core.StudyRun(altered)

    def test_full_registered_schedule_closes_only_after_all_ordered_successes(self):
        run, ids = make_run()
        for operation_id in ids:
            run.begin(operation_id)
            run.complete(operation_id, input_tokens=10, output_tokens=20)
        result = run.close()
        self.assertEqual(result.status, "all-registered-calls-complete")
        self.assertTrue(result.all_calls_complete)
        self.assertEqual(len(result.rows), 39)
        self.assertEqual(result.input_tokens, 390)
        self.assertEqual(result.output_tokens, 780)

    def test_unknown_usage_is_terminal_and_all_later_slots_are_uninvoked(self):
        run, ids = make_run()
        run.begin(ids[0])
        with self.assertRaisesRegex(core.StudyError, "unknown-or-invalid-usage"):
            run.complete(ids[0], input_tokens=None, output_tokens=0)
        with self.assertRaisesRegex(core.StudyError, "stage-terminal"):
            run.begin(ids[1])
        result = run.close()
        self.assertFalse(result.all_calls_complete)
        self.assertEqual(result.rows[0].state, "partial-or-interrupted")
        self.assertTrue(all(row.state == "not-invoked-after-terminal-stop" for row in result.rows[1:]))

    def test_aggregate_output_reserve_failure_retains_stop_and_no_partial_credit(self):
        run, ids = make_run()
        for operation_id in ids[:8]:
            run.begin(operation_id)
            run.complete(operation_id, input_tokens=1, output_tokens=core.MAX_CALL_OUTPUT)
        with self.assertRaisesRegex(core.StudyError, "output-reserve-failed"):
            run.begin(ids[8])
        result = run.close()
        self.assertFalse(result.all_calls_complete)
        self.assertEqual(result.rows[8].state, "preflight-budget-stop")
        self.assertTrue(all(row.state == "not-invoked-after-terminal-stop" for row in result.rows[9:]))
        self.assertEqual(result.output_tokens, core.MAX_OUTPUT_TOTAL)

    def test_external_failure_is_terminal_no_retry_and_no_partial_science(self):
        run, ids = make_run()
        run.begin(ids[0])
        with self.assertRaisesRegex(core.StudyError, "external-failure"):
            run.external_failure(ids[0], state="transport-or-http-failure")
        result = run.close()
        self.assertFalse(result.all_calls_complete)
        self.assertEqual(result.rows[0].state, "transport-or-http-failure")
        with self.assertRaisesRegex(core.StudyError, "stage-terminal"):
            run.begin(ids[0])

    def test_invalid_complete_response_and_out_of_order_are_terminal(self):
        run, ids = make_run()
        run.begin(ids[0])
        with self.assertRaisesRegex(core.StudyError, "invalid-complete-response"):
            run.complete(ids[0], input_tokens=1, output_tokens=1, response_state="complete-invalid-response")
        self.assertFalse(run.close().all_calls_complete)

        other, _ = make_run()
        with self.assertRaisesRegex(core.StudyError, "operation-order-mismatch"):
            other.begin(ids[1])
        closure = other.close()
        self.assertFalse(closure.all_calls_complete)
        self.assertEqual(closure.rows[0].state, "preflight-integrity-stop")

    def test_early_close_cannot_return_partial_science(self):
        run, _ = make_run()
        with self.assertRaisesRegex(core.StudyError, "schedule-incomplete"):
            run.close()
        result = run.close()
        self.assertFalse(result.all_calls_complete)
        self.assertEqual(result.rows[0].state, "preflight-integrity-stop")


if __name__ == "__main__":
    unittest.main()
