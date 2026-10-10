"""Verify the externally pinned admission for a registered selector input map.

The map builder and this module never create permission. An external authority
supplies the expected receipt digest after verifying the registered protocol
and current post-acquisition map. The receipt must bind the current stage,
qualified source, registration, and recomputed map before dispatch.
"""

from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass

from scripts import coverage_study_core as core

SCHEMA = "coverage-selector-map-admission/1"
_SHA256 = re.compile(r"[0-9a-f]{64}\Z")


class SelectorAdmissionError(ValueError):
    """The pinned selector admission is missing, stale, or misbound."""


@dataclass(frozen=True)
class VerifiedSelectorAdmission:
    receipt_sha256: str
    stage_uuid: str
    source_revision: str
    protocol_sha256: str
    registration_sha256: str
    source_closure_sha256: str
    selector_input_map_sha256: str


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _strict(raw: bytes) -> object:
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise SelectorAdmissionError("duplicate-key")
            result[key] = value
        return result

    try:
        return json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=pairs,
            parse_constant=lambda value: (_ for _ in ()).throw(ValueError(value)),
        )
    except (UnicodeDecodeError, ValueError, RecursionError) as exc:
        raise SelectorAdmissionError("receipt-json-invalid") from exc


def verify_selector_map_admission(
    *,
    receipt_bytes: bytes | None,
    expected_receipt_sha256: str | None,
    protocol_bytes: bytes,
    prepared: core.PreparedStage,
    selector_input_map_sha256: str,
) -> VerifiedSelectorAdmission:
    """Check a receipt pinned by the externally verified registered protocol."""
    if (
        type(prepared) is not core.PreparedStage
        or prepared.fresh_execution_authorized is not False
        or prepared.selector_input_map_schema != "coverage-selector-input-map/2-registered"
        or type(receipt_bytes) is not bytes
        or type(expected_receipt_sha256) is not str
        or not _SHA256.fullmatch(expected_receipt_sha256)
        or _sha(receipt_bytes) != expected_receipt_sha256
        or type(protocol_bytes) is not bytes
        or type(selector_input_map_sha256) is not str
        or not _SHA256.fullmatch(selector_input_map_sha256)
    ):
        raise SelectorAdmissionError("admission-input-binding-invalid")
    protocol = _strict(protocol_bytes)
    value = _strict(receipt_bytes)
    if (
        type(protocol) is not dict
        or protocol.get("selector_input_map_schema") != prepared.selector_input_map_schema
        or protocol.get("selector_input_map_status") != "registered"
    ):
        raise SelectorAdmissionError("admission-not-pinned-by-registered-protocol")
    expected = {
        "schema": SCHEMA,
        "status": "admitted",
        "stage_uuid": prepared.stage_uuid,
        "source_revision": prepared.source_revision,
        "protocol_sha256": _sha(protocol_bytes),
        "registration_sha256": prepared.registration_sha256,
        "source_closure_sha256": prepared.pins["qualified_source_closure"],
        "selector_input_map_sha256": selector_input_map_sha256,
    }
    if type(value) is not dict or set(value) != set(expected) or value != expected:
        raise SelectorAdmissionError("admission-receipt-binding-invalid")
    return VerifiedSelectorAdmission(
        receipt_sha256=expected_receipt_sha256,
        stage_uuid=prepared.stage_uuid,
        source_revision=prepared.source_revision,
        protocol_sha256=expected["protocol_sha256"],
        registration_sha256=prepared.registration_sha256,
        source_closure_sha256=prepared.pins["qualified_source_closure"],
        selector_input_map_sha256=selector_input_map_sha256,
    )
