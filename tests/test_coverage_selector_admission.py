import hashlib
import json

import pytest

from scripts import coverage_selector_admission as admission
from scripts import coverage_study_core as core
from tests.test_coverage_study_core import make_fixture, reseal_fixture


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def prepared_registered():
    args, materials, _, _ = make_fixture()
    protocol = {
        "schema": "coverage-first-study-protocol/2",
        "selector_input_map_schema": "coverage-selector-input-map/2-registered",
        "selector_input_map_status": "registered",
    }
    materials["protocol"] = canonical(protocol)
    reseal_fixture(args, materials)
    return core.preflight_stage(**args), materials["protocol"]


def receipt(prepared, protocol_bytes, map_sha):
    return canonical(
        {
            "schema": admission.SCHEMA,
            "status": "admitted",
            "stage_uuid": prepared.stage_uuid,
            "source_revision": prepared.source_revision,
            "protocol_sha256": sha(protocol_bytes),
            "registration_sha256": prepared.registration_sha256,
            "source_closure_sha256": prepared.pins["qualified_source_closure"],
            "selector_input_map_sha256": map_sha,
        }
    )


def test_external_receipt_must_bind_registered_source_protocol_and_exact_map():
    prepared, protocol_bytes = prepared_registered()
    map_sha = "a" * 64
    raw = receipt(prepared, protocol_bytes, map_sha)
    verified = admission.verify_selector_map_admission(
        receipt_bytes=raw,
        expected_receipt_sha256=sha(raw),
        protocol_bytes=protocol_bytes,
        prepared=prepared,
        selector_input_map_sha256=map_sha,
    )
    assert verified.selector_input_map_sha256 == map_sha
    assert verified.registration_sha256 == prepared.registration_sha256
    assert verified.receipt_sha256 == sha(raw)


@pytest.mark.parametrize("field,value", [("selector_input_map_sha256", "b" * 64), ("stage_uuid", "0" * 36)])
def test_stale_or_mismatched_admission_fails_closed(field, value):
    prepared, protocol_bytes = prepared_registered()
    map_sha = "a" * 64
    value_obj = json.loads(receipt(prepared, protocol_bytes, map_sha))
    value_obj[field] = value
    raw = canonical(value_obj)
    with pytest.raises(admission.SelectorAdmissionError, match="admission-receipt-binding-invalid"):
        admission.verify_selector_map_admission(
            receipt_bytes=raw,
            expected_receipt_sha256=sha(raw),
            protocol_bytes=protocol_bytes,
            prepared=prepared,
            selector_input_map_sha256=map_sha,
        )


def test_admission_digest_mismatch_and_unregistered_protocol_reject():
    prepared, protocol_bytes = prepared_registered()
    raw = receipt(prepared, protocol_bytes, "a" * 64)
    with pytest.raises(admission.SelectorAdmissionError, match="admission-input-binding-invalid"):
        admission.verify_selector_map_admission(
            receipt_bytes=raw,
            expected_receipt_sha256="0" * 64,
            protocol_bytes=protocol_bytes,
            prepared=prepared,
            selector_input_map_sha256="a" * 64,
        )
    draft = canonical(
        {
            "selector_input_map_schema": "coverage-selector-input-map/2-draft",
            "selector_input_map_status": "draft",
        }
    )
    with pytest.raises(admission.SelectorAdmissionError, match="admission-not-pinned-by-registered-protocol"):
        admission.verify_selector_map_admission(
            receipt_bytes=raw,
            expected_receipt_sha256=sha(raw),
            protocol_bytes=draft,
            prepared=prepared,
            selector_input_map_sha256="a" * 64,
        )
