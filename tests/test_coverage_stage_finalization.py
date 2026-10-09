from __future__ import annotations

import hashlib
import json
import os
import stat
import tempfile
import unittest
from pathlib import Path

from scripts import coverage_stage_finalization as finalization
from tests.test_coverage_stage_orchestration import plan_fixture


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
            paths, plan = self._fixture(Path(temporary), elapsed=28_800, late=True)
            with self.assertRaisesRegex(finalization.StageFinalizationError, "finalization-late-stage"):
                self._verify(paths, plan)

    def test_pending_time_tampering_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            paths, plan = self._fixture(Path(temporary), elapsed=7.5)
            paths["pending"].write_bytes(paths["pending"].read_bytes() + b" ")
            with self.assertRaisesRegex(finalization.StageFinalizationError, "pending-pin-mismatch"):
                self._verify(paths, plan)

    def _fixture(self, root: Path, *, elapsed: float, late: bool = False):
        os.chmod(root, 0o700)
        plan = plan_fixture()
        stage_id = plan.stage_uuid
        source = plan.source_revision
        protocol_sha = _sha(plan.protocol_bytes)
        cohorts_sha = _sha(plan.cohorts_bytes)
        binding = _sha(b"preacquisition-binding")
        thresholds = _sha(
            _canonical(
                {
                    "quality": json.loads(plan.protocol_bytes)["quality"],
                    "task_use": json.loads(plan.protocol_bytes)["task_use"],
                }
            )
        )
        input_sha = _sha(b"gate-input-manifest")
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
            "stage_deadline_seconds": 28_800.0,
            "pending_elapsed_seconds": pending_elapsed,
            "meaning": "lower bound only; later calculation and final inventory fsync are excluded",
        }
        pending_bytes = _canonical(pending)
        self._write_private(root / "stage-pending-time.json", pending_bytes)
        gates = {
            name: True for name in ("ranking_A", "ranking_B", "task_use_R1", "task_use_R2", "navigation", "resources")
        }
        metrics = {"gates": gates, "status": "pass"}
        gate = {
            "schema": "coverage-first-gate-calculation-receipt/1",
            "source_stage_uuid": stage_id,
            "packet_stage_uuid": plan.packet_stage_uuid,
            "input_manifest_sha256": input_sha,
            "threshold_sha256": thresholds,
            "metrics_sha256": _sha(_canonical(metrics)),
            "status": "pass",
            "gates": gates,
            "quality_credit": False,
            "admission_created": False,
        }
        gate_bytes = _canonical(gate)
        self._write_private(root / "gate-calculation-receipt.json", gate_bytes)
        resource_sha = _sha(b"resource evidence")
        gate_input_bytes = _canonical(
            {
                "stage_uuid": stage_id,
                "protocol_sha256": protocol_sha,
                "cohorts_sha256": cohorts_sha,
                "scope": "mixed-workload-only",
                "pending_time_receipt_sha256": _sha(pending_bytes),
                "resource_evidence": {"stage_elapsed_seconds": pending_elapsed},
                "resource_evidence_receipt_sha256": resource_sha,
            }
        )
        self._write_private(root / "gate-input-manifest.json", gate_input_bytes)
        input_sha = _sha(gate_input_bytes)
        gate["input_manifest_sha256"] = input_sha
        gate_bytes = _canonical(gate)
        self._write_private(root / "gate-calculation-receipt.json", gate_bytes)
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
            "pending_time_receipt_file": "stage-pending-time.json",
            "pending_time_receipt_sha256": _sha(pending_bytes),
            "pending_elapsed_seconds": pending_elapsed,
            "pending_elapsed_meaning": "lower-bound-only",
            "gate_calculation_receipt_file": "gate-calculation-receipt.json",
            "gate_calculation_receipt_sha256": _sha(gate_bytes),
            "gate_status": "pass",
            "gate_scope": "mixed-workload-only",
            "scope": "mixed-workload-only",
            "gate_metrics": metrics,
            "gate_result_authoritative": False,
            "stage_started_monotonic": 100.0,
            "stage_deadline_seconds": 28_800.0,
            "status": "terminal-incomplete" if late else "pending-independent-closeout",
            "terminal_reason": "stage-deadline-exceeded-after-final-fsync" if late else None,
        }
        inventory_bytes = _canonical(inventory)
        inventory_sha = _sha(inventory_bytes)
        self._write_private(root / "inventory.json", inventory_bytes)
        deadline = 28_800.0
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
