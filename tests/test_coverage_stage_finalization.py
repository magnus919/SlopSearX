from __future__ import annotations

import hashlib
import json
import os
import stat
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

from scripts import coverage_gate_calculation as gate_calculation
from scripts import coverage_stage_finalization as finalization
from tests.test_coverage_gate_calculation import _fixture as gate_fixture


def _canonical(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


class StageFinalizationTests(unittest.TestCase):
    def test_verified_complete_pins_allow_only_independent_decision(self):
        with tempfile.TemporaryDirectory() as temporary:
            paths, plan = self._fixture(Path(temporary), elapsed=7.5)
            result = self._verify(paths, plan)
            self.assertEqual(result.status, "gates-pass-awaiting-independent-decision")
            self.assertTrue(result.eligible_for_independent_decision)
            self.assertFalse(json.loads(result.receipt_bytes)["product_authorized"])
            self.assertFalse(json.loads(result.receipt_bytes)["admission_created"])

    def test_actual_deadline_late_invalidates_preliminary_pass(self):
        with tempfile.TemporaryDirectory() as temporary:
            paths, plan = self._fixture(Path(temporary), elapsed=10.0, late=True, stage_deadline=10.0)
            with self.assertRaisesRegex(finalization.StageFinalizationError, "finalization-late-stage"):
                self._verify(paths, plan)

    def test_pending_time_tampering_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            paths, plan = self._fixture(Path(temporary), elapsed=7.5)
            paths["pending"].write_bytes(paths["pending"].read_bytes() + b" ")
            with self.assertRaisesRegex(finalization.StageFinalizationError, "pending-pin-mismatch"):
                self._verify(paths, plan)

    def test_resource_receipt_body_tampering_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            paths, plan = self._fixture(Path(temporary), elapsed=7.5)
            resource_path = paths["inventory"].parent / "resource-evidence.json"
            resource_path.write_bytes(resource_path.read_bytes() + b" ")
            with self.assertRaisesRegex(finalization.StageFinalizationError, "resource-receipt-pin-mismatch"):
                self._verify(paths, plan)

    def test_unknown_gate_subset_is_rejected_even_when_repinned(self):
        with tempfile.TemporaryDirectory() as temporary:
            paths, plan = self._fixture(Path(temporary), elapsed=7.5)
            gate = json.loads(paths["gate"].read_bytes())
            inventory = json.loads(paths["inventory"].read_bytes())
            metrics = inventory["gate_metrics"]
            gate["gates"] = {"resource_and_terminal_guards": True}
            metrics["gates"] = gate["gates"]
            gate["metrics_sha256"] = _sha(_canonical(metrics))
            self._write_private(paths["gate"], _canonical(gate))
            self._repin_inventory_closeout(paths, gate=gate, inventory=inventory, metrics=metrics)
            with self.assertRaisesRegex(finalization.StageFinalizationError, "finalization-gate-binding-mismatch"):
                self._verify(paths, plan)

    def test_thresholds_are_rederived_from_protocol_even_when_artifacts_match_each_other(self):
        with tempfile.TemporaryDirectory() as temporary:
            paths, plan = self._fixture(Path(temporary), elapsed=7.5)
            gate = json.loads(paths["gate"].read_bytes())
            inventory = json.loads(paths["inventory"].read_bytes())
            gate["threshold_sha256"] = "f" * 64
            inventory["thresholds_sha256"] = "f" * 64
            self._write_private(paths["gate"], _canonical(gate))
            self._repin_inventory_closeout(paths, gate=gate, inventory=inventory, metrics=inventory["gate_metrics"])
            with self.assertRaisesRegex(finalization.StageFinalizationError, "finalization-gate-binding-mismatch"):
                self._verify(paths, plan)

    def test_oversize_inventory_is_rejected_with_bounded_reader_before_allocation(self):
        with tempfile.TemporaryDirectory() as temporary:
            paths, plan = self._fixture(Path(temporary), elapsed=7.5)
            oversized = b"x" * (4_000_001)
            self._write_private(paths["inventory"], oversized)
            paths["inventory_sha"] = _sha(oversized)
            with mock.patch.object(Path, "read_bytes", side_effect=AssertionError("unbounded read")):
                with self.assertRaisesRegex(finalization.StageFinalizationError, "unsafe-or-oversize"):
                    self._verify(paths, plan)

    def test_symlinked_artifact_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            paths, plan = self._fixture(Path(temporary), elapsed=7.5)
            target = paths["gate"].with_name("gate-target.json")
            target.write_bytes(paths["gate"].read_bytes())
            os.chmod(target, 0o600)
            paths["gate"].unlink()
            paths["gate"].symlink_to(target)
            with self.assertRaisesRegex(finalization.StageFinalizationError, "artifact-unsafe"):
                self._verify(paths, plan)

    def test_wrong_file_owner_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            paths, plan = self._fixture(Path(temporary), elapsed=7.5)
            with mock.patch.object(finalization, "_effective_uid", return_value=os.geteuid() + 1):
                with self.assertRaisesRegex(finalization.StageFinalizationError, "artifact-unsafe"):
                    self._verify(paths, plan)

    def test_non_private_stage_parent_is_rejected_before_inventory_read(self):
        with tempfile.TemporaryDirectory() as temporary:
            paths, plan = self._fixture(Path(temporary), elapsed=7.5)
            stage_parent = paths["inventory"].parent
            os.chmod(stage_parent, 0o755)
            with mock.patch.object(Path, "read_bytes", side_effect=AssertionError("inventory read")):
                with self.assertRaisesRegex(finalization.StageFinalizationError, "private-directory-invalid"):
                    self._verify(paths, plan)

    def test_duplicate_keys_and_nonfinite_json_are_rejected(self):
        for raw in (b'{"x":1,"x":2}', b'{"x":NaN}'):
            with self.subTest(raw=raw), self.assertRaisesRegex(finalization.StageFinalizationError, "json-invalid"):
                finalization._strict_loads(raw)

    def _fixture(self, root: Path, *, elapsed: float, late: bool = False, stage_deadline: float = 28_800.0):
        os.chmod(root, 0o700)
        evidence_dir = Path(gate_calculation.__file__).parents[1] / "docs/experiments/evidence/coverage-first-study"
        protocol_bytes = (evidence_dir / "protocol.json").read_bytes()
        cohorts_bytes = (evidence_dir / "cohorts.json").read_bytes()
        calculated = gate_fixture()
        prepared = calculated["prepared_assessments"]
        stage_id = prepared.source_stage_uuid
        packet_stage_id = prepared.packet_stage_uuid
        source = "a" * 40
        plan = SimpleNamespace(
            stage_uuid=stage_id,
            packet_stage_uuid=packet_stage_id,
            source_revision=source,
            protocol_bytes=protocol_bytes,
            cohorts_bytes=cohorts_bytes,
        )
        protocol_sha = _sha(plan.protocol_bytes)
        cohorts_sha = _sha(plan.cohorts_bytes)
        binding = _sha(b"preacquisition-binding")
        protocol = json.loads(plan.protocol_bytes)
        thresholds = _sha(_canonical({"quality": protocol["quality"], "task_use": protocol["task_use"]}))
        pending_elapsed = 5.0
        pending = {
            "schema": "coverage-stage-pending-time/1",
            "stage_uuid": stage_id,
            "stage_kind": "development",
            "source_revision": source,
            "protocol_sha256": protocol_sha,
            "cohorts_sha256": cohorts_sha,
            "preacquisition_binding_sha256": binding,
            "observed_monotonic": 105.0,
            "stage_started_monotonic": 100.0,
            "stage_deadline_seconds": stage_deadline,
            "pending_elapsed_seconds": pending_elapsed,
            "meaning": "lower bound only; later calculation and final inventory fsync are excluded",
        }
        pending_bytes = _canonical(pending)
        self._write_private(root / "stage-pending-time.json", pending_bytes)
        calculated["protocol"] = protocol
        calculated["resource_evidence"]["stage_elapsed_seconds"] = pending_elapsed
        resource_observations = calculated["resource_evidence"]
        resource_provenance = {"quality_credit": False, "admission_created": False}
        collector_resource = _canonical(
            {
                "schema": "coverage-resource-evidence/1",
                "stage_uuid": stage_id,
                "observations": resource_observations,
                "provenance": resource_provenance,
            }
        )
        resource_bytes = _canonical(
            {
                "schema": "coverage-resource-evidence/2",
                "stage_uuid": stage_id,
                "source_revision": source,
                "protocol_sha256": protocol_sha,
                "cohorts_sha256": cohorts_sha,
                "collector_receipt_sha256": _sha(collector_resource),
                "observations": resource_observations,
                "provenance": resource_provenance,
            }
        )
        self._write_private(root / "resource-evidence.json", resource_bytes)
        resource_sha = _sha(resource_bytes)
        gate_input_bytes = _canonical(
            {
                "stage_uuid": stage_id,
                "protocol_sha256": protocol_sha,
                "cohorts_sha256": cohorts_sha,
                "scope": "mixed-workload-only",
                "pending_time_receipt_sha256": _sha(pending_bytes),
                "resource_evidence": resource_observations,
                "resource_evidence_receipt_sha256": resource_sha,
            }
        )
        self._write_private(root / "gate-input-manifest.json", gate_input_bytes)
        input_sha = _sha(gate_input_bytes)
        calculated["input_manifest_sha256"] = input_sha
        report = gate_calculation.calculate_coverage_gate_report(**calculated)
        self.assertEqual(report.evaluation.status, "pass")
        gate_bytes = report.receipt_bytes
        self._write_private(root / "gate-calculation-receipt.json", gate_bytes)
        metrics = report.metrics
        thresholds = report.evaluation.threshold_sha256
        inventory = {
            "stage_uuid": stage_id,
            "stage_kind": "development",
            "source_revision": source,
            "protocol_sha256": protocol_sha,
            "cohorts_sha256": cohorts_sha,
            "preacquisition_binding_sha256": binding,
            "thresholds_sha256": thresholds,
            "gate_input_manifest_sha256": input_sha,
            "gate_input_manifest_file": "gate-input-manifest.json",
            "resource_evidence_receipt_sha256": resource_sha,
            "resource_evidence_receipt_file": "resource-evidence.json",
            "pending_time_receipt_file": "stage-pending-time.json",
            "pending_time_receipt_sha256": _sha(pending_bytes),
            "pending_elapsed_seconds": pending_elapsed,
            "pending_elapsed_meaning": "lower-bound-only",
            "gate_calculation_receipt_file": "gate-calculation-receipt.json",
            "gate_calculation_receipt_sha256": _sha(gate_bytes),
            "gate_status": report.evaluation.status,
            "gate_scope": "mixed-workload-only",
            "scope": "mixed-workload-only",
            "gate_metrics": metrics,
            "gate_result_authoritative": False,
            "stage_started_monotonic": 100.0,
            "stage_deadline_seconds": stage_deadline,
            "status": "terminal-incomplete" if late else "pending-independent-closeout",
            "terminal_reason": "stage-deadline-exceeded-after-final-fsync" if late else None,
        }
        inventory_bytes = _canonical(inventory)
        inventory_sha = _sha(inventory_bytes)
        self._write_private(root / "inventory.json", inventory_bytes)
        deadline = stage_deadline
        closeout = {
            "schema": "coverage-stage-closeout/2",
            "stage_uuid": stage_id,
            "stage_kind": "development",
            "source_revision": source,
            "protocol_sha256": protocol_sha,
            "cohorts_sha256": cohorts_sha,
            "preacquisition_binding_sha256": binding,
            "final_inventory_sha256": inventory_sha,
            "final_inventory_status": inventory["status"],
            "final_inventory_terminal_reason": inventory["terminal_reason"],
            "stage_started_monotonic": 100.0,
            "observed_after_final_inventory_fsync_monotonic": 100.0 + elapsed,
            "stage_elapsed_seconds": elapsed,
            "stage_deadline_seconds": deadline,
            "measurement_basis": (
                "stage start through final decision inventory file and directory fsync; closeout receipt fsync excluded"
            ),
        }
        closeout_bytes = _canonical(closeout)
        self._write_private(root / "stage-closeout.json", closeout_bytes)
        return (
            {
                "inventory": root / "inventory.json",
                "pending": root / "stage-pending-time.json",
                "gate": root / "gate-calculation-receipt.json",
                "gate_input": root / "gate-input-manifest.json",
                "closeout": root / "stage-closeout.json",
                "inventory_sha": inventory_sha,
                "pending_sha": _sha(pending_bytes),
                "gate_sha": _sha(gate_bytes),
                "gate_input_sha": _sha(gate_input_bytes),
                "closeout_sha": _sha(closeout_bytes),
            },
            plan,
        )

    @staticmethod
    def _write_private(path: Path, raw: bytes) -> None:
        path.write_bytes(raw)
        os.chmod(path, stat.S_IRUSR | stat.S_IWUSR)

    def _repin_inventory_closeout(self, paths, *, gate, inventory, metrics):
        gate_bytes = _canonical(gate)
        self._write_private(paths["gate"], gate_bytes)
        paths["gate_sha"] = _sha(gate_bytes)
        inventory["gate_calculation_receipt_sha256"] = paths["gate_sha"]
        inventory["gate_metrics"] = metrics
        inventory_bytes = _canonical(inventory)
        self._write_private(paths["inventory"], inventory_bytes)
        paths["inventory_sha"] = _sha(inventory_bytes)
        closeout = json.loads(paths["closeout"].read_bytes())
        closeout["final_inventory_sha256"] = paths["inventory_sha"]
        closeout_bytes = _canonical(closeout)
        self._write_private(paths["closeout"], closeout_bytes)
        paths["closeout_sha"] = _sha(closeout_bytes)

    def _verify(self, paths, plan):
        return finalization.verify_stage_finalization(
            inventory_path=paths["inventory"],
            expected_inventory_sha256=paths["inventory_sha"],
            closeout_receipt_path=paths["closeout"],
            expected_closeout_receipt_sha256=paths["closeout_sha"],
            pending_time_receipt_path=paths["pending"],
            expected_pending_time_receipt_sha256=paths["pending_sha"],
            gate_input_manifest_path=paths["gate_input"],
            expected_gate_input_manifest_sha256=paths["gate_input_sha"],
            gate_calculation_receipt_path=paths["gate"],
            expected_gate_calculation_receipt_sha256=paths["gate_sha"],
            expected_stage_uuid=plan.stage_uuid,
            expected_source_revision=plan.source_revision,
            protocol_bytes=plan.protocol_bytes,
            cohorts_bytes=plan.cohorts_bytes,
            expected_protocol_sha256=_sha(plan.protocol_bytes),
            expected_cohorts_sha256=_sha(plan.cohorts_bytes),
        )


if __name__ == "__main__":
    unittest.main()
