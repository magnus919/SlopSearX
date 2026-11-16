"""Regressions for real preregistration shapes in the phase-artifact issuer."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

import pytest

from scripts import coverage_local_authority as authority
from scripts import coverage_study_core as core

STAGE = "00000000-0000-0000-0000-000000008335"
REVISION = "a" * 40


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def context_fixture():
    directory = (
        Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study/frozen-input-method-materials"
    )
    protocol = (directory / "protocol.json").read_bytes()
    cohorts = (directory / "cohorts.json").read_bytes()
    closure = canonical({"schema": "coverage-study-qualified-source-closure/2", "source_revision": REVISION})
    pins = {name: digest(canonical({"fixture": name})) for name in core.PIN_NAMES}
    pins.update(protocol=digest(protocol), qualified_source_closure=digest(closure))
    stage = next(row for row in json.loads(cohorts)["stages"] if row["stage"] == "development")
    registration = canonical(
        {
            "schema": core.REGISTRATION_SCHEMA,
            "stage_uuid": STAGE,
            "stage_kind": "development",
            "source_revision": REVISION,
            "pins": pins,
            "operation_ids": [row.operation_id for row in core.selector_inventory([None] * 5)],
            "navigation_skip_reasons": [None] * 5,
            "task_ids": [row["task_id"] for row in stage["research_cases"]]
            + [row["target_id"] for row in stage["navigation_targets"]],
            "band_companion_sha256": digest((directory / "band-companion-contract.json").read_bytes()),
        }
    )
    artifacts = {
        "protocol": protocol,
        "cohorts": cohorts,
        "source-closure": closure,
        "initial-registration": registration,
        "active-registration": registration,
    }
    return artifacts, {
        "stage_uuid": STAGE,
        "expected_source_closure_sha256": digest(closure),
        "expected_initial_registration_sha256": digest(registration),
        "expected_active_registration_sha256": digest(registration),
    }


def test_real_core_pin_inventory_binds_cohorts_through_locked_protocol():
    artifacts, expected = context_fixture()
    active, protocol = authority.PhaseArtifactReceiptIssuer._registered_context(artifacts, **expected)
    assert set(active["pins"]) == set(core.PIN_NAMES)
    assert "cohorts" not in active["pins"]
    assert protocol["cohorts_sha256"] == digest(artifacts["cohorts"])


def test_changed_cohort_bytes_cannot_preserve_registration_context():
    artifacts, expected = context_fixture()
    artifacts["cohorts"] += b" "
    with pytest.raises(authority.LocalAuthorityError, match="registered-context-invalid"):
        authority.PhaseArtifactReceiptIssuer._registered_context(artifacts, **expected)


def test_even_pinned_malformed_registration_is_rejected_by_core_schema():
    artifacts, expected = context_fixture()
    malformed = json.loads(artifacts["initial-registration"])
    del malformed["schema"]
    raw = canonical(malformed)
    artifacts["initial-registration"] = artifacts["active-registration"] = raw
    expected["expected_initial_registration_sha256"] = digest(raw)
    expected["expected_active_registration_sha256"] = digest(raw)
    with pytest.raises(authority.LocalAuthorityError, match="registration-schema-invalid"):
        authority.PhaseArtifactReceiptIssuer._registered_context(artifacts, **expected)


def test_capture_manifest_must_match_registered_dynamic_pin(tmp_path):
    import threading
    import time

    from scripts import coverage_operator_handoff as handoff

    artifacts, expected = context_fixture()
    manifest = {
        "stage_uuid": STAGE,
        "protocol_sha256": digest(artifacts["protocol"]),
        "source_revision": REVISION,
        "cohorts_sha256": digest(artifacts["cohorts"]),
        "candidate_identity_sha256": "d" * 64,
        "candidate_endpoint_sha256": "e" * 64,
        "sources": [{"source_id": "fixture-source"}],
    }
    manifest_raw = canonical(manifest)
    artifacts["capture-manifest"] = manifest_raw
    bindings = {
        **{key: manifest[key] for key in manifest if key != "sources"},
        "manifest_sha256": digest(manifest_raw),
        "source_count": 1,
        "max_owned_calls": authority.capture.MAX_OWNED_CALLS,
        "max_health_calls": authority.capture.MAX_HEALTH_CALLS,
        "max_scrape_calls": authority.capture.MAX_SCRAPE_CALLS,
        "timeout_seconds": authority.capture.REQUEST_TIMEOUT_SECONDS,
        "response_bytes": authority.capture.MAX_RESPONSE_BYTES,
        "source_context_characters": authority.capture.MAX_SOURCE_CHARS,
        "ca_bundle_sha256": None,
        "source_closure_sha256": expected["expected_source_closure_sha256"],
    }
    root = tmp_path / "handoff"
    exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
    terminal = []

    def request():
        try:
            exchange.request(
                stage_uuid=STAGE,
                scope="source-capture",
                request_id="permit",
                bindings=bindings,
                artifacts=artifacts,
                deadline_monotonic=time.monotonic() + 0.5,
            )
        except handoff.OperatorHandoffError as exc:
            terminal.append(exc)

    worker = threading.Thread(target=request)
    worker.start()
    path = root / STAGE / "source-capture" / "requests" / "permit.json"
    try:
        deadline = time.monotonic() + 0.3
        while not path.exists() and time.monotonic() < deadline:
            time.sleep(0.005)
        assert path.exists()
        # Matching caller-provided expected bytes cannot override the registered pin.
        with pytest.raises(authority.LocalAuthorityError):
            authority.PhaseArtifactReceiptIssuer.source_capture(
                root=root,
                request_id="permit",
                request_bytes=path.read_bytes(),
                expected_capture_manifest_bytes=manifest_raw,
                **expected,
            )
    finally:
        worker.join(timeout=1)
    assert not worker.is_alive()
    assert terminal
