"""Synthetic tests for the no-call coverage driver boundary."""

from __future__ import annotations

import hashlib
import json
import stat
import tempfile
import unittest
from pathlib import Path

from scripts import coverage_guarded_driver as driver
from tests.test_coverage_study_core import make_fixture


def sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


class SyntheticAdmission:
    def __init__(self, *, result_mutator=None):
        self.calls = 0
        self.result_mutator = result_mutator

    def verify(self, prepared, receipt_bytes, expected_receipt_sha256, *, scope):
        self.calls += 1
        result = driver.VerifiedAdmission(
            status="verified-admitted",
            scope=scope,
            stage_uuid=prepared.stage_uuid,
            registration_sha256=prepared.registration_sha256,
            receipt_sha256=expected_receipt_sha256,
            pins=dict(prepared.pins),
        )
        return self.result_mutator(result) if self.result_mutator else result


class GuardedNoCallDriverTests(unittest.TestCase):
    def setUp(self):
        self.args, *_ = make_fixture()
        self.receipt = b'{"synthetic":"externally sealed admission fixture"}'
        self.receipt_sha = sha(self.receipt)
        self.temp = tempfile.TemporaryDirectory()
        self.state = Path(self.temp.name) / "private-state"
        self.state.mkdir(mode=0o700)
        self.state.chmod(0o700)

    def tearDown(self):
        self.temp.cleanup()

    def invoke(self, verifier, *, receipt=None, receipt_sha=None, state=None):
        return driver.prepare_no_call_selector_stage(
            preflight_kwargs=self.args,
            admission_receipt_bytes=self.receipt if receipt is None else receipt,
            expected_admission_receipt_sha256=self.receipt_sha if receipt_sha is None else receipt_sha,
            admission_verifier=verifier,
            private_state_directory=self.state if state is None else state,
        )

    def test_missing_verifier_fails_before_writing_any_authority_artifacts(self):
        with self.assertRaisesRegex(driver.DriverError, "qualified-admission-verifier-unavailable"):
            self.invoke(None)
        self.assertEqual(list(self.state.iterdir()), [])

    def test_external_receipt_digest_is_checked_before_verifier(self):
        verifier = SyntheticAdmission()
        with self.assertRaisesRegex(driver.DriverError, "admission-receipt-digest-mismatch"):
            self.invoke(verifier, receipt_sha="0" * 64)
        self.assertEqual(verifier.calls, 0)
        self.assertEqual(list(self.state.iterdir()), [])

    def test_wrong_admission_binding_fails_closed(self):
        def drift(result):
            return driver.VerifiedAdmission(**{**result.__dict__, "stage_uuid": "00000000-0000-0000-0000-000000000001"})

        with self.assertRaisesRegex(driver.DriverError, "source-bound-admission-mismatch"):
            self.invoke(SyntheticAdmission(result_mutator=drift))
        self.assertEqual(list(self.state.iterdir()), [])

    def test_dry_run_writes_private_one_shot_lease_and_terminal_inventory(self):
        verifier = SyntheticAdmission()
        result = self.invoke(verifier)
        self.assertEqual(verifier.calls, 1)
        self.assertEqual(result.status, "no-call-skeleton-terminal")
        self.assertFalse(result.provider_dispatch)
        self.assertFalse(result.execution_authorized)
        self.assertEqual(result.operation_count, 39)
        lease_path = self.state / f"{result.stage_uuid}.selector.lease.json"
        terminal_path = self.state / f"{result.stage_uuid}.selector.terminal.json"
        lease_bytes = lease_path.read_bytes()
        terminal_bytes = terminal_path.read_bytes()
        self.assertEqual(sha(lease_bytes), result.lease_sha256)
        self.assertEqual(sha(terminal_bytes), result.terminal_inventory_sha256)
        self.assertEqual(stat.S_IMODE(lease_path.stat().st_mode), 0o600)
        self.assertEqual(stat.S_IMODE(terminal_path.stat().st_mode), 0o600)
        self.assertEqual(stat.S_IMODE(self.state.stat().st_mode), 0o700)
        lease = json.loads(lease_bytes)
        terminal = json.loads(terminal_bytes)
        self.assertEqual(lease["status"], "dry-run-consumed-not-executed")
        self.assertFalse(lease["dispatch_implemented"])
        self.assertEqual(terminal["status"], "no-call-skeleton-terminal")
        self.assertFalse(terminal["provider_dispatch"])
        self.assertEqual(terminal["usage_state"], "not-observed")
        self.assertIsNone(terminal["input_tokens"])
        self.assertIsNone(terminal["output_tokens"])
        self.assertEqual(len(terminal["operations"]), 39)
        self.assertEqual(terminal["operations"][0]["status"], "preflight-integrity-stop")
        self.assertTrue(all(row["status"] == "not-invoked-after-terminal-stop" for row in terminal["operations"][1:]))
        self.assertIn("registered-candidate-native-prejev-fallback-binding", terminal["unmet_requirements"])
        self.assertIn("frozen-first40-and-41to80-scope-decisions", terminal["unmet_requirements"])

    def test_one_shot_artifacts_block_a_second_attempt_without_overwrite(self):
        self.invoke(SyntheticAdmission())
        before = {path.name: path.read_bytes() for path in self.state.iterdir()}
        with self.assertRaisesRegex(driver.DriverError, "one-shot-stage-artifact-already-exists"):
            self.invoke(SyntheticAdmission())
        self.assertEqual(before, {path.name: path.read_bytes() for path in self.state.iterdir()})

    def test_symlink_and_non_private_state_roots_are_rejected(self):
        link = Path(self.temp.name) / "state-link"
        link.symlink_to(self.state, target_is_directory=True)
        with self.assertRaisesRegex(driver.DriverError, "private-state-path-unsafe"):
            self.invoke(SyntheticAdmission(), state=link)

        loose = Path(self.temp.name) / "loose-state"
        loose.mkdir(mode=0o755)
        loose.chmod(0o755)
        with self.assertRaisesRegex(driver.DriverError, "private-state-directory-mode"):
            self.invoke(SyntheticAdmission(), state=loose)
        self.assertEqual(list(loose.iterdir()), [])

    def test_status_cli_reports_blockers_and_never_dispatches(self):
        import contextlib
        import io

        stdout = io.StringIO()
        with contextlib.redirect_stdout(stdout):
            exit_code = driver.main(["--status"])
        self.assertEqual(exit_code, 0)
        status = json.loads(stdout.getvalue())
        self.assertEqual(status["status"], "skeleton-no-call")
        self.assertFalse(status["provider_dispatch"])
        self.assertFalse(status["credential_access"])
        self.assertEqual(status["candidate_native_prejev_fallback_binding"], "implemented-not-registered")
        self.assertEqual(status["configured_v1_control_order"], "separate-and-not-executed")
        self.assertEqual(status["first40_and_41to80_scope_decisions"], "not-frozen")


if __name__ == "__main__":
    unittest.main()
