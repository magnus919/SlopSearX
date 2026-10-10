from __future__ import annotations

import asyncio
import hashlib
import json
import tempfile
import unittest
import uuid
from datetime import datetime, timedelta, timezone
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

from scripts import coverage_answer_execution as answer
from scripts import coverage_operator_runner as runner
from tests.test_coverage_answer_execution import _tasks
from tests.test_coverage_stage_orchestration import plan_fixture


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()


def digest(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


class OperatorRunnerConfigTests(unittest.TestCase):
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

    def test_unregistered_or_draft_protocol_fails_before_clock_or_handoff_side_effects(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            protocol = canonical({"selector_input_map_schema": "coverage-selector-input-map/2-draft"})
            config, pin = self._config(root, protocol)
            with (
                mock.patch.object(runner.time, "monotonic", side_effect=AssertionError("clock must not start")),
                mock.patch.object(
                    runner.operator_handoff, "OperatorReceiptHandoff", side_effect=AssertionError("no handoff")
                ),
            ):
                with self.assertRaisesRegex(runner.OperatorRunnerError, "registered-source-materials-required"):
                    runner.OperatorStageRunner.from_file(config, pin)
            self.assertFalse((root / "operator_handoff").exists())

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

                async def request(scope, request_id, bindings):
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
