"""Offline engineering tests for the private one-shot coverage issuer.

All receipts and source inputs here are synthetic fixtures. Passing these tests
is not qualification evidence and does not execute a study or contact providers.
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import subprocess
import sys
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import pytest

SOURCE = Path(__file__).resolve().parents[1]
ISSUER_SCRIPT = SOURCE / "scripts/coverage_external_issuer.py"
SCOPES = (
    "frozen-pool-collection",
    "late-registration",
    "protected-source-capture",
    "source-capture",
    "selector-map-admission",
    "selector-operation-permits",
    "answer-execution",
)


def sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def canonical(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def load_issuer_module():
    spec = importlib.util.spec_from_file_location("coverage_fresh_issue_phase_test", ISSUER_SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@dataclass
class FakePlan:
    stage_uuid: str
    stage_kind: str
    source_revision: str
    source_closure_sha256: str
    pool_input_mode: str
    stage_started_monotonic: float
    stage_deadline_monotonic: float
    protocol_bytes: bytes
    cohorts_bytes: bytes
    acquisition_plan_bytes: bytes
    acquisition_manifest_bytes: bytes
    candidate_identity_bytes: bytes
    candidate_endpoint_sha256: str
    expected_acquisition_manifest_sha256: str
    research_cases: tuple = ()
    navigation_targets: tuple = ()
    band_companion_bytes: bytes = b"fixture-companion"
    band_companion_sha256: str = "a" * 64
    band_companion_source_pins: dict | None = None
    initial_registration_sha256: str | None = None


def make_runner(base: Path, revision: str):
    import scripts.coverage_source_capture as capture

    stage_uuid = "00000000-0000-4000-8000-000000000821"
    protocol = canonical({"fixture": "protocol"})
    cohorts = canonical({"fixture": "cohorts"})
    acquisition_plan = canonical({"fixture": "acquisition-plan"})
    acquisition_manifest = canonical({"fixture": "legacy-manifest"})
    identity = canonical({"runtime": {"revision": "a" * 40}})
    _health, _scrape, endpoint_sha = capture._candidate_endpoints("https://candidate.example")
    closure = "b" * 64
    plan = FakePlan(
        stage_uuid=stage_uuid,
        stage_kind="development",
        source_revision=revision,
        source_closure_sha256=closure,
        pool_input_mode="frozen-pool/1",
        stage_started_monotonic=time.monotonic() - 10,
        stage_deadline_monotonic=time.monotonic() + 3600,
        protocol_bytes=protocol,
        cohorts_bytes=cohorts,
        acquisition_plan_bytes=acquisition_plan,
        acquisition_manifest_bytes=acquisition_manifest,
        candidate_identity_bytes=identity,
        candidate_endpoint_sha256=endpoint_sha,
        expected_acquisition_manifest_sha256=sha(acquisition_manifest),
    )
    handoff_root = base / "handoff"
    directories = {
        "operator_handoff": handoff_root,
        "stage_inventory": base / "inventory",
        "capture_receipts": base / "capture-receipts",
        "selector_results": base / "selector-results",
        "selector_archive": base / "selector-archive",
    }
    for path in directories.values():
        path.mkdir(mode=0o700, parents=True, exist_ok=True)
    registration = canonical({"fixture": "initial-registration"})
    environment = canonical({"schema": "coverage-active-python-environment/1", "fixture": True})
    return SimpleNamespace(
        plan=plan,
        config={
            "candidate_base_url": "https://candidate.example",
            "answer_endpoint": "https://candidate.example/answer",
            "forbidden_stage_uuids": [],
        },
        directories=directories,
        private_paths={},
        candidate_identity={"runtime": {"revision": "a" * 40}},
        initial_registration=registration,
        initial_registration_sha256=sha(registration),
        _active_registration_bytes=registration,
        active_environment_receipt_bytes=environment,
        static_materials={"qualified_source_closure": canonical({"source_revision": revision})},
    )


def _request_artifacts(scope: str, runner) -> dict[str, bytes]:
    clock = canonical(
        {
            "schema": "coverage-frozen-collection-stage-clock/1",
            "stage_started_monotonic": runner.plan.stage_started_monotonic,
            "stage_deadline_monotonic": runner.plan.stage_deadline_monotonic,
            "stage_started_utc": "2026-10-13T00:00:00Z",
            "stage_deadline_utc": "2026-10-13T08:00:00Z",
            "clock_origin_sha256": "c" * 64,
        }
    )
    artifacts = {
        "source-closure": canonical({"source_revision": runner.plan.source_revision}),
        "initial-registration": runner.initial_registration,
        "active-registration": runner.initial_registration,
        "stage-clock": clock,
    }
    if scope == "answer-execution":
        artifacts["answer-task-material"] = canonical({"fixture": "answer-task-material"})
    return artifacts


def _request_bindings(runner) -> dict[str, object]:
    return {
        "source_closure_sha256": runner.plan.source_closure_sha256,
        "active_python_environment": json.loads(runner.active_environment_receipt_bytes),
        "active_python_environment_sha256": sha(runner.active_environment_receipt_bytes),
    }


def _write_private(path: Path, raw: bytes) -> None:
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "wb") as stream:
        stream.write(raw)
        stream.flush()
        os.fsync(stream.fileno())


def _run_main(module, *, source_revision: str, config: Path, scope: str, active: Path, qualification: Path):
    argv = [
        "coverage_fresh_issue_phase.py",
        "--source",
        str(SOURCE),
        "--source-revision",
        source_revision,
        "--config",
        str(config),
        "--config-sha256",
        "d" * 64,
        "--scope",
        scope,
    ]
    if scope not in {"frozen-pool-collection", "late-registration"}:
        argv += ["--active-registration", str(active), "--active-registration-sha256", sha(active.read_bytes())]
    if scope == "protected-source-capture":
        argv += [
            "--qualification-evidence",
            str(qualification),
            "--qualification-evidence-sha256",
            sha(qualification.read_bytes()),
        ]
    with mock.patch.object(sys, "argv", argv):
        module.main()


@pytest.fixture
def offline_issuer(monkeypatch, request, tmp_path):
    """Use the pinned real modules and real file handoff, with issuer dispatches stubbed."""
    source_revision = "a" * 40

    def fixture_checkout(command, **_kwargs):
        if command == ["git", "-C", str(SOURCE), "rev-parse", "HEAD"]:
            return source_revision + "\n"
        if command == ["git", "-C", str(SOURCE), "status", "--porcelain"]:
            return ""
        raise AssertionError("unexpected subprocess in offline fixture")

    monkeypatch.setattr(subprocess, "check_output", fixture_checkout)
    import scripts.coverage_frozen_pool_collect as collector
    import scripts.coverage_late_registration as late
    import scripts.coverage_local_authority as authority
    import scripts.coverage_operator_handoff as handoff
    import scripts.coverage_operator_runner as runner_module
    import scripts.coverage_selector_input_map as selector_map

    runner = make_runner(tmp_path, source_revision)
    monkeypatch.setattr(
        runner_module.OperatorStageRunner,
        "preflight_from_file",
        classmethod(lambda _cls, _path, _sha: runner),
    )
    monkeypatch.setattr(collector, "_validate_clock", lambda *_args, **_kwargs: None)
    module = load_issuer_module()
    config = tmp_path / "config.json"
    active = tmp_path / "active-registration.json"
    qualification = tmp_path / "qualification-evidence.json"
    _write_private(config, b"{}")
    _write_private(active, runner.initial_registration)
    _write_private(qualification, canonical({"evidence": {"fixture": True}}))

    # The registered dispatch functions are autospecced so the test checks the
    # helper's keyword contract against the pinned source signatures.
    dispatch_names = (
        "frozen_pool_collection",
        "late_registration",
        "protected_capture_qualification",
        "source_capture",
        "selector_map_admission",
        "selector_operation_permits",
    )
    patches = [
        mock.patch.object(authority.PhaseArtifactReceiptIssuer, name, autospec=True, return_value=b"fixture-receipt")
        for name in dispatch_names
    ]
    dispatch_mocks = []
    for patcher in patches:
        dispatch_mocks.append(patcher.start())
        request.addfinalizer(patcher.stop)
    answer_patch = mock.patch.object(authority.PhaseArtifactReceiptIssuer, "answer_execution", autospec=True)
    answer_mock = answer_patch.start()

    async def issue_answer_fixture(**_kwargs):
        return b"fixture-receipt"

    answer_mock.side_effect = issue_answer_fixture
    request.addfinalizer(answer_patch.stop)

    rebuild = mock.patch.object(
        authority.PhaseArtifactReceiptIssuer,
        "rebuild_frozen_registration_materials",
        autospec=True,
        return_value=(
            {"task_input_manifest": b"{}"},
            {"capture_manifest_bytes": b"fixture-capture-manifest", "tasks": []},
            SimpleNamespace(
                snapshots_directory=tmp_path,
                snapshot_index_sha256="e" * 64,
                pool_manifest_sha256=sha(runner.plan.acquisition_manifest_bytes),
            ),
        ),
    )
    rebuild_mock = rebuild.start()
    request.addfinalizer(rebuild.stop)
    late_verify = mock.patch.object(
        late,
        "verify_late_registration",
        autospec=True,
        return_value=SimpleNamespace(prepared=object()),
    )
    late_verify.start()
    request.addfinalizer(late_verify.stop)
    selector_build = mock.patch.object(
        selector_map, "build_selector_input_map", autospec=True, return_value=b"fixture-map"
    )
    selector_build.start()
    request.addfinalizer(selector_build.stop)

    return SimpleNamespace(
        runner=runner,
        module=module,
        config=config,
        active=active,
        qualification=qualification,
        source_revision=source_revision,
        handoff=handoff,
        authority=authority,
        dispatch_patches=patches,
        dispatch_mocks=dispatch_mocks,
        answer_mock=answer_mock,
        rebuild_mock=rebuild_mock,
    )


@pytest.mark.parametrize("scope", SCOPES)
def test_main_loads_real_scoped_artifact_bundle_and_dispatches_once(offline_issuer, scope):
    fixture = offline_issuer
    runner = fixture.runner
    handoff = fixture.handoff
    scope_path = (
        Path(runner.directories["operator_handoff"]) / runner.plan.stage_uuid / scope / "requests" / "permit.json"
    )
    seen_scopes: list[str] = []
    real_loader = handoff.load_request_artifacts

    def observed_loader(**kwargs):
        seen_scopes.append(kwargs.get("scope"))
        return real_loader(**kwargs)

    # This calls the real bounded loader with the caller's actual scope. Before
    # the scope fix, the helper's common call omitted it and the loader rejects.
    with mock.patch.object(handoff, "load_request_artifacts", side_effect=observed_loader):
        exchange = handoff.OperatorReceiptHandoff(runner.directories["operator_handoff"], poll_interval=0.02)
        result: list[object] = []
        worker = threading.Thread(
            target=lambda: result.append(
                exchange.request(
                    stage_uuid=runner.plan.stage_uuid,
                    scope=scope,
                    request_id="permit",
                    bindings=_request_bindings(runner),
                    deadline_monotonic=time.monotonic() + 30,
                    artifacts=_request_artifacts(scope, runner),
                )
            )
        )
        worker.start()
        for _ in range(500):
            if scope_path.is_file():
                break
            time.sleep(0.005)
        assert scope_path.is_file()
        _run_main(
            fixture.module,
            source_revision=fixture.source_revision,
            config=fixture.config,
            scope=scope,
            active=fixture.active,
            qualification=fixture.qualification,
        )
        worker.join(timeout=3)
        assert not worker.is_alive()
    assert seen_scopes == [scope]
    assert len(result) == 1 and result[0][0] == b"fixture-receipt"
    receipt = scope_path.parent.parent / "responses" / "permit.receipt"
    pin = scope_path.parent.parent / "pins" / "permit.sha256"
    assert receipt.read_bytes() == b"fixture-receipt"
    assert pin.read_text().strip() == sha(b"fixture-receipt")


def test_main_refuses_duplicate_receipt_and_preserves_existing_pin(offline_issuer):
    fixture = offline_issuer
    runner = fixture.runner
    scope = "frozen-pool-collection"
    request_path = (
        Path(runner.directories["operator_handoff"]) / runner.plan.stage_uuid / scope / "requests" / "permit.json"
    )
    exchange = fixture.handoff.OperatorReceiptHandoff(runner.directories["operator_handoff"], poll_interval=0.02)
    result: list[object] = []
    worker = threading.Thread(
        target=lambda: result.append(
            exchange.request(
                stage_uuid=runner.plan.stage_uuid,
                scope=scope,
                request_id="permit",
                bindings=_request_bindings(runner),
                deadline_monotonic=time.monotonic() + 30,
                artifacts=_request_artifacts(scope, runner),
            )
        )
    )
    worker.start()
    for _ in range(500):
        if request_path.is_file():
            break
        time.sleep(0.005)
    assert request_path.is_file()
    _run_main(
        fixture.module,
        source_revision=fixture.source_revision,
        config=fixture.config,
        scope=scope,
        active=fixture.active,
        qualification=fixture.qualification,
    )
    worker.join(timeout=3)
    assert not worker.is_alive()
    response = request_path.parent.parent / "responses" / "permit.receipt"
    pin = request_path.parent.parent / "pins" / "permit.sha256"
    original = (response.read_bytes(), pin.read_bytes())
    dispatch_count_before_duplicate = fixture.dispatch_mocks[0].call_count
    with pytest.raises(ValueError, match="receipt-already-issued"):
        _run_main(
            fixture.module,
            source_revision=fixture.source_revision,
            config=fixture.config,
            scope=scope,
            active=fixture.active,
            qualification=fixture.qualification,
        )
    assert (response.read_bytes(), pin.read_bytes()) == original
    assert fixture.dispatch_mocks[0].call_count == dispatch_count_before_duplicate == 1


def test_external_issuer_rejects_confirmation_with_missing_or_malformed_receipt():
    module = load_issuer_module()
    with pytest.raises(ValueError, match="confirmation-development-receipt-required"):
        module._require_confirmation_receipt(
            SimpleNamespace(
                development_receipt_bytes=None,
                development_receipt_sha256=None,
                development_stage_uuid=None,
            )
        )
    with pytest.raises(ValueError, match="confirmation-development-receipt-required"):
        module._require_confirmation_receipt(
            SimpleNamespace(
                development_receipt_bytes=b"receipt",
                development_receipt_sha256="0" * 64,
                development_stage_uuid="00000000-0000-0000-0000-000000000001",
            )
        )


def test_main_rejects_request_scope_tampering_before_dispatch(offline_issuer):
    fixture = offline_issuer
    runner = fixture.runner
    scope = "frozen-pool-collection"
    request_path = (
        Path(runner.directories["operator_handoff"]) / runner.plan.stage_uuid / scope / "requests" / "permit.json"
    )
    exchange = fixture.handoff.OperatorReceiptHandoff(runner.directories["operator_handoff"], poll_interval=0.02)
    worker_error: list[object] = []

    def wait_for_permit():
        try:
            exchange.request(
                stage_uuid=runner.plan.stage_uuid,
                scope=scope,
                request_id="permit",
                bindings=_request_bindings(runner),
                deadline_monotonic=time.monotonic() + 30,
                artifacts=_request_artifacts(scope, runner),
            )
        except Exception as exc:  # fixture thread is released below
            worker_error.append(exc)

    worker = threading.Thread(target=wait_for_permit)
    worker.start()
    for _ in range(500):
        if request_path.is_file():
            break
        time.sleep(0.005)
    envelope = json.loads(request_path.read_bytes())
    envelope["scope"] = "source-capture"
    _write_private(request_path, canonical(envelope))
    with pytest.raises(Exception):
        _run_main(
            fixture.module,
            source_revision=fixture.source_revision,
            config=fixture.config,
            scope=scope,
            active=fixture.active,
            qualification=fixture.qualification,
        )
    assert all(dispatch.call_count == 0 for dispatch in fixture.dispatch_mocks)
    assert fixture.answer_mock.call_count == 0
    response = request_path.parent.parent / "responses" / "permit.receipt"
    pin = request_path.parent.parent / "pins" / "permit.sha256"
    _write_private(response, b"fixture-release")
    _write_private(pin, (sha(b"fixture-release") + "\n").encode())
    worker.join(timeout=3)
    assert not worker.is_alive()
