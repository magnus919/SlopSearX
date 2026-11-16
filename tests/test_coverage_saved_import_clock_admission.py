"""Offline authority-path coverage for saved-import clock admission.

The fixture reuses synthetic archive bytes while adjusting descriptor pins to
exercise the issuer boundary. It is not a complete saved-corpus pipeline test
or evidence that an archive's original metadata was produced by a collector.
"""

from __future__ import annotations

import dataclasses
import hashlib
import json
import os
import subprocess
import sys
import threading
import time
import uuid
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import pytest

from scripts import coverage_external_issuer as external
from scripts import coverage_frozen_pool_collect as collector
from scripts import coverage_local_authority as authority
from scripts import coverage_operator_handoff as handoff
from scripts import coverage_saved_corpus as saved
from scripts import coverage_stage_orchestration as orchestration
from tests.test_coverage_saved_corpus import _origin_package

SOURCE = Path(__file__).resolve().parents[1]
MATERIALS = SOURCE / "docs/experiments/evidence/coverage-first-study/saved-development-method-materials"


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def _private(path: Path, raw: bytes) -> None:
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "wb") as stream:
        stream.write(raw)
        stream.flush()
        os.fsync(stream.fileno())


def _clock(
    *, stale: bool = False, tamper_origin: bool = False, origin_sha256: str = "c" * 64
) -> tuple[collector.FrozenCollectionStageClock, bytes]:
    now_mono = time.monotonic()
    now_utc = time.time()
    start_mono = now_mono - (28_801 if stale else 2)
    start_utc = now_utc - (28_801 if stale else 2)
    clock = collector.FrozenCollectionStageClock(
        stage_started_monotonic=start_mono,
        stage_deadline_monotonic=start_mono + collector.MAX_STAGE_WALL_SECONDS,
        stage_started_utc=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(start_utc)),
        stage_deadline_utc=time.strftime(
            "%Y-%m-%dT%H:%M:%SZ", time.gmtime(start_utc + collector.MAX_STAGE_WALL_SECONDS)
        ),
        clock_origin_sha256=("0" * 64 if tamper_origin else origin_sha256),
    )
    doc = {"schema": "coverage-frozen-collection-stage-clock/1", **dataclasses.asdict(clock)}
    return clock, _canonical(doc)


def _registered_runner(tmp_path: Path, *, clock_bytes: bytes):
    import scripts.coverage_operator_runner as runner_module

    origin, cases, nav, _collection, _capture = _origin_package(tmp_path)
    revision = "b" * 40
    protocol = (MATERIALS / "protocol.json").read_bytes()
    cohorts = (MATERIALS / "cohorts.json").read_bytes()
    acquisition_plan = _canonical({"fixture": "registered acquisition plan"})
    acquisition_manifest = _canonical({"fixture": "saved import has no live acquisition"})
    identity = _canonical({"runtime": {"revision": revision}})
    endpoint_sha = "4" * 64
    origin_registration = dataclasses.replace(
        origin.registration,
        cohorts_sha256=_sha(cohorts),
        candidate_identity_sha256=_sha(identity),
        candidate_endpoint_sha256=endpoint_sha,
    )
    origin = dataclasses.replace(origin, registration=origin_registration)
    stage_uuid = str(uuid.UUID(int=1701))
    packet_uuid = str(uuid.UUID(int=1702))
    closure_sha = "5" * 64
    start_clock = collector.FrozenCollectionStageClock(
        **{key: value for key, value in json.loads(clock_bytes).items() if key != "schema"}
    )
    initial = {
        "schema": "coverage-stage-initial-registration/1",
        "stage_uuid": stage_uuid,
        "stage_kind": "development",
        "source_revision": revision,
        "pins": {
            "protocol": _sha(protocol),
            "qualified_source_closure": closure_sha,
        },
    }
    initial_raw = _canonical(initial)
    plan = orchestration.StagePlan(
        stage_uuid=stage_uuid,
        stage_kind="development",
        source_revision=revision,
        protocol_bytes=protocol,
        cohorts_bytes=cohorts,
        acquisition_plan_bytes=acquisition_plan,
        acquisition_manifest_bytes=acquisition_manifest,
        source_closure_sha256=closure_sha,
        research_cases=tuple(cases),
        navigation_targets=tuple(nav),
        candidate_identity_bytes=identity,
        candidate_endpoint_sha256=endpoint_sha,
        packet_stage_uuid=packet_uuid,
        stage_started_monotonic=0.0,
        stage_deadline_monotonic=0.0,
        expected_acquisition_manifest_sha256=_sha(acquisition_manifest),
        pool_input_mode="saved-corpus/1",
        saved_origin=origin,
    )
    environment = _canonical({"schema": "coverage-active-python-environment/1", "python": "fixture"})
    handoff_root = tmp_path / "handoff"
    handoff_root.mkdir(mode=0o700)
    directories = {"operator_handoff": handoff_root, "stage_inventory": tmp_path / "inventory"}
    runner = SimpleNamespace(
        plan=plan,
        config={"forbidden_stage_uuids": []},
        directories=directories,
        static_materials={},
        initial_registration=initial_raw,
        initial_registration_sha256=_sha(initial_raw),
        active_environment_receipt_bytes=environment,
    )
    return runner, runner_module, initial, clock_bytes, start_clock


def _artifact_bundle(runner, initial: dict, clock_bytes: bytes) -> tuple[dict[str, object], dict[str, bytes]]:
    plan = runner.plan
    package_sha = plan.saved_origin.expected_package_sha256
    reference = {
        "schema": "coverage-saved-corpus-origin-reference/1",
        "origin_stage_uuid": plan.saved_origin.registration.stage_uuid,
        "origin_source_revision": plan.saved_origin.registration.source_revision,
        "origin_package_sha256": package_sha,
        "collection_receipt_inventory_sha256": saved.resource_evidence.receipt_inventory_sha256(
            plan.saved_origin.collection_directory
        ),
        "capture_inventory_sha256": _sha(
            saved._private_file(plan.saved_origin.capture_stage_directory / "inventory.json", max_bytes=4_000_000)
        ),
        "capture_manifest_sha256": _sha(
            saved._private_file(plan.saved_origin.capture_stage_directory / "source-manifest.json", max_bytes=4_000_000)
        ),
    }
    artifacts = {
        "source-closure": _canonical({"source_revision": plan.source_revision}),
        "initial-registration": _canonical(initial),
        "active-registration": _canonical(initial),
        "stage-clock": clock_bytes,
        "protocol": plan.protocol_bytes,
        "cohorts": plan.cohorts_bytes,
        "acquisition-plan": plan.acquisition_plan_bytes,
        "saved-corpus-origin-reference": authority._canonical(reference),
    }
    bindings = {
        "stage_uuid": plan.stage_uuid,
        "packet_stage_uuid": plan.packet_stage_uuid,
        "source_revision": plan.source_revision,
        "source_closure_sha256": plan.source_closure_sha256,
        "protocol_sha256": _sha(plan.protocol_bytes),
        "cohorts_sha256": _sha(plan.cohorts_bytes),
        "candidate_identity_sha256": _sha(plan.candidate_identity_bytes),
        "candidate_endpoint_sha256": plan.candidate_endpoint_sha256,
        "origin_package_sha256": package_sha,
        "active_python_environment": json.loads(runner.active_environment_receipt_bytes),
        "active_python_environment_sha256": _sha(runner.active_environment_receipt_bytes),
    }
    return bindings, artifacts


def _invoke_issuer(
    tmp_path: Path, monkeypatch, *, stale: bool = False, tamper_origin: bool = False, tamper_cohorts: bool = False
):
    import pathlib

    boot_id = b"offline-test-boot-id"
    origin_sha256 = _sha(boot_id)
    original_read_text = pathlib.Path.read_text

    def read_text(path, *args, **kwargs):
        if str(path) == "/proc/sys/kernel/random/boot_id":
            return boot_id.decode("ascii")
        return original_read_text(path, *args, **kwargs)

    monkeypatch.setattr(pathlib.Path, "read_text", read_text)
    monkeypatch.setattr(collector, "_local_clock_origin_sha256", lambda: origin_sha256)
    _clock_doc, clock_bytes = _clock(stale=stale, tamper_origin=tamper_origin, origin_sha256=origin_sha256)
    runner, runner_module, initial, _clock_raw, _ = _registered_runner(tmp_path, clock_bytes=clock_bytes)
    if tamper_cohorts:
        protocol = json.loads(runner.plan.protocol_bytes)
        protocol["cohorts_sha256"] = "0" * 64
        raw = _canonical(protocol)
        runner.plan = dataclasses.replace(runner.plan, protocol_bytes=raw)
        initial["pins"]["protocol"] = _sha(raw)
        runner.initial_registration_sha256 = _sha(_canonical(initial))
    monkeypatch.setattr(
        subprocess,
        "check_output",
        lambda command, **kwargs: runner.plan.source_revision + "\n" if command[-2:] == ["rev-parse", "HEAD"] else "",
    )
    monkeypatch.setattr(
        runner_module.OperatorStageRunner,
        "preflight_from_file",
        classmethod(lambda _cls, _path, _sha256: runner),
    )
    observed_plans = []
    original_saved_import = authority.PhaseArtifactReceiptIssuer.saved_corpus_import

    def issue_saved_import(**kwargs):
        observed_plans.append(kwargs["plan"])
        return original_saved_import(**kwargs)

    monkeypatch.setattr(authority.PhaseArtifactReceiptIssuer, "saved_corpus_import", staticmethod(issue_saved_import))
    bindings, artifacts = _artifact_bundle(runner, initial, clock_bytes)
    exchange = handoff.OperatorReceiptHandoff(runner.directories["operator_handoff"], poll_interval=0.02)
    result: list[object] = []

    def request_once():
        try:
            result.append(
                exchange.request(
                    stage_uuid=runner.plan.stage_uuid,
                    scope="saved-corpus-import",
                    request_id="permit",
                    bindings=bindings,
                    deadline_monotonic=time.monotonic() + (1 if stale or tamper_origin or tamper_cohorts else 10),
                    artifacts=artifacts,
                )
            )
        except Exception as exc:
            result.append(exc)

    worker = threading.Thread(target=request_once)
    worker.start()
    request_path = (
        runner.directories["operator_handoff"]
        / runner.plan.stage_uuid
        / "saved-corpus-import"
        / "requests"
        / "permit.json"
    )
    deadline = time.monotonic() + 5
    while not request_path.exists() and time.monotonic() < deadline:
        time.sleep(0.005)
    assert request_path.exists()
    config = tmp_path / "config.json"
    config.write_bytes(b"{}")
    argv = [
        "coverage_external_issuer.py",
        "--source",
        str(SOURCE),
        "--source-revision",
        runner.plan.source_revision,
        "--config",
        str(config),
        "--config-sha256",
        _sha(config.read_bytes()),
        "--scope",
        "saved-corpus-import",
    ]
    error = None
    try:
        with mock.patch.object(sys, "argv", argv):
            external.main()
    except Exception as exc:  # expected on deliberately invalid clocks
        error = exc
    worker.join(timeout=5)
    assert not worker.is_alive()
    if error is not None:
        raise error
    assert len(result) == 1
    assert len(observed_plans) == 1
    receipt, receipt_sha = result[0]
    return runner, observed_plans[0], receipt, receipt_sha


def test_external_saved_import_uses_live_clock_and_accepts_distinct_origin_revision(monkeypatch, tmp_path):
    runner, issued_plan, receipt, receipt_sha = _invoke_issuer(tmp_path, monkeypatch)
    assert issued_plan.stage_started_monotonic > 0
    assert issued_plan.stage_deadline_monotonic - issued_plan.stage_started_monotonic == pytest.approx(
        collector.MAX_STAGE_WALL_SECONDS
    )
    assert issued_plan.source_revision != issued_plan.saved_origin.registration.source_revision
    permit = authority.PinnedReceiptAuthorities.saved_corpus_import(issued_plan, receipt, receipt_sha)
    assert permit.source_revision == issued_plan.source_revision
    assert permit.status == "verified-admitted"


@pytest.mark.parametrize("clock_mode", ["stale", "tampered-origin"])
def test_external_saved_import_rejects_stale_or_tampered_clock(monkeypatch, tmp_path, clock_mode):
    with pytest.raises(collector.FrozenCollectionError):
        _invoke_issuer(
            tmp_path,
            monkeypatch,
            stale=clock_mode == "stale",
            tamper_origin=clock_mode == "tampered-origin",
        )


def test_pinned_saved_import_rejects_malformed_current_revision(monkeypatch, tmp_path):
    runner, issued_plan, receipt, receipt_sha = _invoke_issuer(tmp_path, monkeypatch)
    malformed = dataclasses.replace(issued_plan, source_revision="not-a-revision")
    with pytest.raises(authority.LocalAuthorityError, match="saved-corpus-import-plan-invalid"):
        authority.PinnedReceiptAuthorities.saved_corpus_import(malformed, receipt, receipt_sha)
    request_bytes = (
        Path(runner.directories["operator_handoff"])
        / issued_plan.stage_uuid
        / "saved-corpus-import"
        / "requests"
        / "permit.json"
    ).read_bytes()
    with pytest.raises(authority.LocalAuthorityError, match="saved-corpus-import-context-invalid"):
        authority.PhaseArtifactReceiptIssuer.saved_corpus_import(
            root=runner.directories["operator_handoff"],
            stage_uuid=issued_plan.stage_uuid,
            request_id="permit",
            request_bytes=request_bytes,
            expected_source_closure_sha256=issued_plan.source_closure_sha256,
            expected_initial_registration_sha256=runner.initial_registration_sha256,
            expected_active_registration_sha256=runner.initial_registration_sha256,
            plan=malformed,
        )


def test_external_saved_import_rejects_wrong_protocol_cohort_hash(monkeypatch, tmp_path):
    with pytest.raises(authority.LocalAuthorityError, match="saved-corpus-import-registration-mismatch"):
        _invoke_issuer(tmp_path, monkeypatch, tamper_cohorts=True)
