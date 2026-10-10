from __future__ import annotations

import hashlib
import json
import unittest

from scripts import coverage_late_registration as late
from scripts import coverage_study_core as core
from tests.test_coverage_study_core import OLD_STAGE, SOURCE_REVISION, TASKS, make_fixture, reseal_fixture


def digest(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


class LateRegistrationTests(unittest.TestCase):
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
