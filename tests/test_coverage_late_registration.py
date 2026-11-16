from __future__ import annotations

import hashlib
import json
import unittest
from pathlib import Path
from types import SimpleNamespace

from scripts import coverage_late_registration as late
from scripts import coverage_study_core as core
from tests.test_coverage_study_core import OLD_STAGE, SOURCE_REVISION, TASKS, canonical, make_fixture, reseal_fixture


def digest(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


class LateRegistrationTests(unittest.TestCase):
    def test_development_receipt_transport_rejects_phase_context_collisions(self):
        raw = b"sealed-test-receipt"
        plan = SimpleNamespace(
            stage_kind="confirmation",
            development_receipt_bytes=raw,
            development_receipt_sha256=digest(raw),
            development_stage_uuid="00000000-0000-0000-0000-000000000099",
        )
        bindings = {"development_receipt_sha256": "0" * 64}
        with self.assertRaisesRegex(late.LateRegistrationError, "binding-collision"):
            late._bind_development_receipt(plan, bindings, {})
        artifacts = {"development-pass-receipt": b"different"}
        with self.assertRaisesRegex(late.LateRegistrationError, "artifact-collision"):
            late._bind_development_receipt(plan, {}, artifacts)
        bindings, artifacts = {}, {}
        late._bind_development_receipt(plan, bindings, artifacts)
        self.assertEqual(bindings["development_receipt_sha256"], digest(raw))
        self.assertEqual(artifacts["development-pass-receipt"], raw)

    def test_composed_confirmation_preflight_requires_pinned_source_bound_development_receipt(self):
        args, materials, _pins, initial = make_fixture(stage_kind="confirmation")
        late_materials = dict(materials)
        reseal_fixture(args, late_materials, stage_kind="confirmation")
        initial = args["registration_bytes"]
        late_registration = initial
        common = dict(
            initial_registration_bytes=initial,
            expected_initial_registration_sha256=digest(initial),
            late_registration_bytes=late_registration,
            expected_late_registration_sha256=digest(late_registration),
            materials=late_materials,
            expected_source_revision=SOURCE_REVISION,
            expected_stage_uuid=args["expected_stage_uuid"],
            forbidden_stage_uuids=(OLD_STAGE,),
            task_ids=TASKS,
            navigation_skip_reasons=args["navigation_skip_reasons"],
        )
        with self.assertRaisesRegex(late.LateRegistrationError, "core-preflight-failed"):
            late.verify_late_registration(**common)

        receipt = canonical(
            {
                "schema": core.DEVELOPMENT_RECEIPT_SCHEMA,
                "stage_uuid": "00000000-0000-0000-0000-000000000099",
                "stage_kind": "development",
                "status": "passed",
                "protocol_sha256": digest(materials["protocol"]),
                "source_revision": SOURCE_REVISION,
                "source_closure_sha256": digest(materials["qualified_source_closure"]),
                "result_sha256": "e" * 64,
            }
        )
        verified = late.verify_late_registration(
            **common,
            development_receipt_bytes=receipt,
            expected_development_receipt_sha256=digest(receipt),
            expected_development_stage_uuid="00000000-0000-0000-0000-000000000099",
        )
        self.assertEqual(verified.prepared.stage_kind, "confirmation")
        self.assertFalse(verified.prepared.fresh_execution_authorized)

        failed = json.loads(receipt)
        failed["status"] = "inconclusive"
        failed_bytes = canonical(failed)
        with self.assertRaisesRegex(late.LateRegistrationError, "core-preflight-failed"):
            late.verify_late_registration(
                **common,
                development_receipt_bytes=failed_bytes,
                expected_development_receipt_sha256=digest(failed_bytes),
                expected_development_stage_uuid=failed["stage_uuid"],
            )

        for field, value in (
            ("source_revision", "b" * 40),
            ("protocol_sha256", "d" * 64),
            ("source_closure_sha256", "c" * 64),
        ):
            with self.subTest(field=field):
                mismatched = json.loads(receipt)
                mismatched[field] = value
                mismatched_bytes = canonical(mismatched)
                with self.assertRaisesRegex(late.LateRegistrationError, "core-preflight-failed"):
                    late.verify_late_registration(
                        **common,
                        development_receipt_bytes=mismatched_bytes,
                        expected_development_receipt_sha256=digest(mismatched_bytes),
                        expected_development_stage_uuid=mismatched["stage_uuid"],
                    )

        with self.assertRaisesRegex(late.LateRegistrationError, "core-preflight-failed"):
            late.verify_late_registration(
                **common,
                development_receipt_bytes=receipt,
                expected_development_receipt_sha256="0" * 64,
                expected_development_stage_uuid="00000000-0000-0000-0000-000000000099",
            )

    def test_composed_v2_initial_to_late_preflight_preserves_companion_and_source_pins(self):
        args, materials, _pins, _legacy_registration = make_fixture()
        evidence_dir = Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study"
        registered_dir = evidence_dir / "registered-2026-10-12-acquisition-v2"
        materials["protocol"] = (registered_dir / "protocol.json").read_bytes()
        companion_bytes = (evidence_dir / "band-companion/development-contract.json").read_bytes()
        companion_sha = digest(companion_bytes)
        source_files = {
            "coverage_study_core": "scripts/coverage_study_core.py",
            "coverage_gate_calculation": "scripts/coverage_gate_calculation.py",
            "coverage_stage_orchestration": "scripts/coverage_stage_orchestration.py",
            "coverage_stage_finalization": "scripts/coverage_stage_finalization.py",
            "coverage_operator_runner": "scripts/coverage_operator_runner.py",
            "coverage_band_companion": "scripts/coverage_band_companion.py",
            "coverage_late_registration": "scripts/coverage_late_registration.py",
        }
        root = Path(__file__).parents[1]
        source_pins = {name: digest((root / path).read_bytes()) for name, path in source_files.items()}
        materials["qualified_source_closure"] = canonical(
            {
                "schema": "coverage-study-qualified-source-closure/2",
                "source_revision": SOURCE_REVISION,
                "material_pins": {
                    "coverage_source": digest(materials["coverage_source"]),
                    "production_rerank_source": digest(materials["production_rerank_source"]),
                    "dependency_lock": digest(materials["dependency_lock"]),
                    "band_companion": companion_sha,
                },
                "source_pins": source_pins,
            }
        )
        reseal_fixture(args, materials)
        initial_doc = json.loads(args["registration_bytes"])
        initial_doc["band_companion_sha256"] = companion_sha
        initial = canonical(initial_doc)

        late_materials = dict(materials)
        late_materials["source_capture_manifest"] = canonical(
            {
                "schema": "coverage-source-capture-manifest/1",
                "stage_uuid": args["expected_stage_uuid"],
                "sources": [],
            }
        )
        reseal_fixture(args, late_materials)
        late_doc = json.loads(args["registration_bytes"])
        late_doc["band_companion_sha256"] = companion_sha
        late_registration = canonical(late_doc)

        verified = late.verify_late_registration(
            initial_registration_bytes=initial,
            expected_initial_registration_sha256=digest(initial),
            late_registration_bytes=late_registration,
            expected_late_registration_sha256=digest(late_registration),
            materials=late_materials,
            expected_source_revision=SOURCE_REVISION,
            expected_stage_uuid=args["expected_stage_uuid"],
            forbidden_stage_uuids=(OLD_STAGE,),
            task_ids=TASKS,
            navigation_skip_reasons=args["navigation_skip_reasons"],
            band_companion_bytes=companion_bytes,
            expected_band_companion_sha256=companion_sha,
            expected_source_pins=source_pins,
        )
        self.assertIsInstance(verified.prepared, core.PreparedStage)
        self.assertEqual(verified.source_capture_manifest_bytes, late_materials["source_capture_manifest"])
        with self.assertRaisesRegex(late.LateRegistrationError, "core-preflight-failed"):
            late.verify_late_registration(
                initial_registration_bytes=initial,
                expected_initial_registration_sha256=digest(initial),
                late_registration_bytes=late_registration,
                expected_late_registration_sha256=digest(late_registration),
                materials=late_materials,
                expected_source_revision=SOURCE_REVISION,
                expected_stage_uuid=args["expected_stage_uuid"],
                forbidden_stage_uuids=(OLD_STAGE,),
                task_ids=TASKS,
                navigation_skip_reasons=args["navigation_skip_reasons"],
                band_companion_bytes=companion_bytes,
                expected_band_companion_sha256=companion_sha,
                expected_source_pins={**source_pins, "coverage_study_core": "0" * 64},
            )

    def test_external_late_registration_may_bind_pool_derived_capture_material(self):
        args, materials, _pins, initial = make_fixture()
        changed_capture = json.dumps(
            {"schema": "coverage-source-capture-manifest/1", "stage_uuid": args["expected_stage_uuid"], "sources": []},
            sort_keys=True,
            separators=(",", ":"),
        ).encode()
        late_materials = dict(materials, source_capture_manifest=changed_capture)
        reseal_fixture(args, late_materials)
        verified = late.verify_late_registration(
            initial_registration_bytes=initial,
            expected_initial_registration_sha256=digest(initial),
            late_registration_bytes=args["registration_bytes"],
            expected_late_registration_sha256=args["expected_registration_sha256"],
            materials=late_materials,
            expected_source_revision=SOURCE_REVISION,
            expected_stage_uuid=args["expected_stage_uuid"],
            forbidden_stage_uuids=(OLD_STAGE,),
            task_ids=TASKS,
            navigation_skip_reasons=args["navigation_skip_reasons"],
        )
        self.assertIsInstance(verified.prepared, core.PreparedStage)
        self.assertFalse(verified.prepared.fresh_execution_authorized)
        self.assertEqual(verified.prepared.status, "preflight-verified-not-admitted")
        self.assertEqual(verified.source_capture_manifest_bytes, changed_capture)
        self.assertEqual(verified.registration_sha256, digest(args["registration_bytes"]))

    def test_changed_source_task_assignment_or_operation_schedule_is_rejected(self):
        for field, value in (
            ("source_revision", "f" * 40),
            ("task_ids", ["different-task"]),
            ("operation_ids", ["different-operation"]),
        ):
            with self.subTest(field=field):
                args, _materials, _pins, initial = make_fixture()
                late_doc = json.loads(initial)
                late_doc[field] = value
                late_bytes = json.dumps(late_doc, sort_keys=True, separators=(",", ":")).encode()
                with self.assertRaisesRegex(late.LateRegistrationError, "late-registration-lineage-drift"):
                    late.verify_late_registration(
                        initial_registration_bytes=initial,
                        expected_initial_registration_sha256=digest(initial),
                        late_registration_bytes=late_bytes,
                        expected_late_registration_sha256=digest(late_bytes),
                        materials=_materials,
                        expected_source_revision=SOURCE_REVISION,
                        expected_stage_uuid=args["expected_stage_uuid"],
                        forbidden_stage_uuids=(OLD_STAGE,),
                        task_ids=TASKS,
                        navigation_skip_reasons=args["navigation_skip_reasons"],
                    )

    def test_frozen_pin_budget_protocol_or_plan_change_is_rejected(self):
        for name in ("protocol", "qualified_source_closure", "acquisition_plan", "capture_plan"):
            with self.subTest(pin=name):
                args, materials, _pins, initial = make_fixture()
                late_materials = dict(materials)
                late_materials[name] += b" changed"
                reseal_fixture(args, late_materials)
                with self.assertRaisesRegex(late.LateRegistrationError, f"frozen-pin-drift:{name}"):
                    late.verify_late_registration(
                        initial_registration_bytes=initial,
                        expected_initial_registration_sha256=digest(initial),
                        late_registration_bytes=args["registration_bytes"],
                        expected_late_registration_sha256=args["expected_registration_sha256"],
                        materials=late_materials,
                        expected_source_revision=SOURCE_REVISION,
                        expected_stage_uuid=args["expected_stage_uuid"],
                        forbidden_stage_uuids=(OLD_STAGE,),
                        task_ids=TASKS,
                        navigation_skip_reasons=args["navigation_skip_reasons"],
                    )

    def test_material_or_expected_hash_drift_fails_before_preflight(self):
        args, materials, _pins, initial = make_fixture()
        reseal_fixture(args, materials)
        changed = dict(materials, source_capture_manifest=b"different but local")
        with self.assertRaisesRegex(late.LateRegistrationError, "material-pin-mismatch:source_capture_manifest"):
            late.verify_late_registration(
                initial_registration_bytes=initial,
                expected_initial_registration_sha256=digest(initial),
                late_registration_bytes=args["registration_bytes"],
                expected_late_registration_sha256=args["expected_registration_sha256"],
                materials=changed,
                expected_source_revision=SOURCE_REVISION,
                expected_stage_uuid=args["expected_stage_uuid"],
                forbidden_stage_uuids=(OLD_STAGE,),
                task_ids=TASKS,
                navigation_skip_reasons=args["navigation_skip_reasons"],
            )


if __name__ == "__main__":
    unittest.main()
