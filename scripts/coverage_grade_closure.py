"""All-or-nothing structural closure for fresh coverage grader submissions.

This module validates exact prepared packet bindings and frozen JSON Schemas. It
preserves grader judgments as received; it does not calculate correctness,
consensus, completion, or study quality.
"""

from __future__ import annotations

import hashlib
import json
import math
import re
from dataclasses import dataclass
from typing import Mapping

from jsonschema import Draft202012Validator  # type: ignore[import-untyped]

from scripts.coverage_assessment_packets import (
    MAX_GRADER_SUBMISSIONS,
    MAX_PACKET_BYTES,
    PreparedAnswerPackets,
    PreparedAssessmentPackets,
)

MAX_RESPONSE_BYTES = 2_000_000
# A closed view combines responses and, for answer closure, the previously
# closed references. Source grades occur in both chunk and consolidated views;
# restored identities come from bounded input packets. The provider limit above
# remains per submission, not a limit on this derived aggregate.
MAX_CLOSED_OUTPUT_BYTES = MAX_GRADER_SUBMISSIONS * (2 * MAX_RESPONSE_BYTES + MAX_PACKET_BYTES)
MAX_JSON_DEPTH = 64
_SHA = re.compile(r"[0-9a-f]{64}\Z")


class GradeClosureError(ValueError):
    """A grader phase is incomplete, malformed, or does not match its packets."""


@dataclass(frozen=True)
class GradeSubmission:
    packet_id: str
    task_id: str
    stage_uuid: str
    role: str
    assessor_id: str
    input_sha256: str
    response_bytes: bytes


@dataclass(frozen=True)
class SubmissionHashReceipt:
    packet_id: str
    task_id: str
    stage_uuid: str
    role: str
    assessor_id: str
    input_sha256: str
    response_sha256: str
    response_byte_count: int


@dataclass(frozen=True)
class GradeClosure:
    phase: str
    stage_uuid: str
    receipt_bytes: bytes
    receipt_sha256: str
    submission_receipts: tuple[SubmissionHashReceipt, ...]
    _outputs_bytes: bytes
    _source_outputs_bytes: bytes | None = None

    def outputs(self) -> dict[str, object]:
        """Return a fresh copy of closed grades with identities restored."""
        value = _strict_json(self._outputs_bytes, "closed-output", max_bytes=MAX_CLOSED_OUTPUT_BYTES)
        if type(value) is not dict:
            raise GradeClosureError("closed-output-shape")
        return value

    def source_outputs_for_answer(self) -> dict[str, dict[str, list[dict[str, object]]]]:
        """Expose complete closed R1/R2 chunk outputs for answer-packet joining."""
        if self.phase != "preassessment" or self._source_outputs_bytes is None:
            raise GradeClosureError("preassessment-closure-required")
        value = _strict_json(self._source_outputs_bytes, "closed-source-output", max_bytes=MAX_CLOSED_OUTPUT_BYTES)
        if type(value) is not dict:
            raise GradeClosureError("closed-source-output-shape")
        return value  # freshly parsed, caller mutations cannot change closure receipts


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    try:
        return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode(
            "utf-8"
        )
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise GradeClosureError("closure-json-invalid") from exc


def _depth_guard(raw: bytes) -> None:
    depth = 0
    in_string = False
    escaped = False
    for byte in raw:
        if in_string:
            if escaped:
                escaped = False
            elif byte == 0x5C:
                escaped = True
            elif byte == 0x22:
                in_string = False
        elif byte == 0x22:
            in_string = True
        elif byte in (0x7B, 0x5B):
            depth += 1
            if depth > MAX_JSON_DEPTH:
                raise GradeClosureError("submission-json-depth")
        elif byte in (0x7D, 0x5D):
            depth -= 1
            if depth < 0:
                raise GradeClosureError("submission-json-invalid")
    if depth != 0 or in_string or escaped:
        raise GradeClosureError("submission-json-invalid")


def _strict_json(raw: bytes, label: str, *, max_bytes: int = MAX_RESPONSE_BYTES) -> object:
    if type(raw) is not bytes or not raw or len(raw) > max_bytes:
        raise GradeClosureError(f"{label}-bytes-invalid")
    _depth_guard(raw)

    def pairs(items: list[tuple[str, object]]) -> dict[str, object]:
        result: dict[str, object] = {}
        for key, value in items:
            if key in result:
                raise GradeClosureError(f"{label}-duplicate-key")
            result[key] = value
        return result

    def parse_int(value: str) -> int:
        if len(value.lstrip("-")) > 128:
            raise GradeClosureError(f"{label}-integer-bound")
        return int(value)

    def parse_float(value: str) -> float:
        result = float(value)
        if not math.isfinite(result):
            raise GradeClosureError(f"{label}-nonfinite")
        return result

    def reject_constant(_value: str) -> object:
        raise GradeClosureError(f"{label}-nonfinite")

    try:
        return json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=pairs,
            parse_int=parse_int,
            parse_float=parse_float,
            parse_constant=reject_constant,
        )
    except GradeClosureError:
        raise
    except (UnicodeDecodeError, json.JSONDecodeError, RecursionError, ValueError) as exc:
        raise GradeClosureError(f"{label}-json-invalid") from exc


def _expected_packets(packets: tuple[dict[str, object], ...], *, answer_phase: bool) -> dict[str, dict[str, object]]:
    expected: dict[str, dict[str, object]] = {}
    for packet in packets:
        raw = packet.get("bytes")
        if type(raw) is not bytes or len(raw) > MAX_PACKET_BYTES or _sha(raw) != packet.get("sha256"):
            raise GradeClosureError("prepared-packet-integrity")
        envelope = _strict_json(raw, "prepared-packet")
        if type(envelope) is not dict or type(envelope.get("output_schema")) is not dict:
            raise GradeClosureError("prepared-packet-envelope")
        if answer_phase:
            if packet.get("role") != "answer":
                raise GradeClosureError("prepared-answer-role")
        elif packet.get("role") not in {"card", "source"}:
            raise GradeClosureError("prepared-preassessment-role")
        packet_id = packet.get("packet_id")
        if type(packet_id) is not str or packet_id in expected:
            raise GradeClosureError("prepared-packet-id")
        if envelope.get("packet_id") != packet_id or envelope.get("task_id") != packet.get("task_id"):
            raise GradeClosureError("prepared-packet-metadata-binding")
        if envelope.get("assessor_id") != packet.get("assessor_id"):
            raise GradeClosureError("prepared-packet-role-binding")
        if answer_phase:
            if envelope.get("packet_kind") != "answer":
                raise GradeClosureError("prepared-answer-role-binding")
            expected_schema = _derive_answer_schema(envelope)
            if envelope.get("output_schema") != expected_schema:
                raise GradeClosureError("prepared-dynamic-schema-mismatch")
        elif envelope.get("role") != packet.get("role"):
            raise GradeClosureError("prepared-packet-role-binding")
        else:
            expected_schema = _derive_preassessment_schema(envelope, str(packet.get("role")))
            if envelope.get("output_schema") != expected_schema:
                raise GradeClosureError("prepared-dynamic-schema-mismatch")
        expected[packet_id] = {**packet, "envelope": envelope}
    return expected


def _derive_answer_schema(envelope: dict[str, object]) -> dict[str, object]:
    from scripts import coverage_assessment_packets as packet_prep

    task_id = envelope.get("task_id")
    packet_id = envelope.get("packet_id")
    assessor_id = envelope.get("assessor_id")
    blind_arm_id = envelope.get("blind_arm_id")
    answer = envelope.get("answer")
    if (
        type(task_id) is not str
        or type(packet_id) is not str
        or type(assessor_id) is not str
        or type(blind_arm_id) is not str
        or type(answer) is not list
    ):
        raise GradeClosureError("prepared-answer-assignment")
    check_ids = [row.get("facet_id") for row in answer if type(row) is dict]
    if len(check_ids) != len(answer):
        raise GradeClosureError("prepared-answer-assignment")
    source_inventory = envelope.get("pool_source_inventory")
    if type(source_inventory) is not list:
        raise GradeClosureError("prepared-answer-assignment")
    source_ids = [row.get("source_id") for row in source_inventory if type(row) is dict]
    if len(source_ids) != len(source_inventory):
        raise GradeClosureError("prepared-answer-assignment")
    return packet_prep._answer_schema(task_id, packet_id, assessor_id, blind_arm_id, check_ids, source_ids)


def _derive_preassessment_schema(envelope: dict[str, object], role: str) -> dict[str, object]:
    """Rebuild the per-packet schema from frozen EXP-077 schema and assignment."""
    from scripts import coverage_assessment_packets as packet_prep

    model_input = envelope.get("model_input")
    task_id = envelope.get("task_id")
    assessor_id = envelope.get("assessor_id")
    if type(model_input) is not dict or type(task_id) is not str or type(assessor_id) is not str:
        raise GradeClosureError("prepared-packet-assignment")
    if role == "card":
        base, _raw = packet_prep._reference("card-assessment.schema.json")
        schema = json.loads(json.dumps(base))
        cards = model_input.get("cards")
        facets = model_input.get("facets")
        if type(cards) is not list or type(facets) is not list:
            raise GradeClosureError("prepared-card-assignment")
        card_hashes = [row.get("card_sha256") for row in cards if type(row) is dict]
        facet_ids = [row.get("id") for row in facets if type(row) is dict]
        if len(card_hashes) != len(cards) or len(facet_ids) != len(facets):
            raise GradeClosureError("prepared-card-assignment")
        records = schema["properties"]["records"]
        records["minItems"] = len(cards)
        records["maxItems"] = len(cards)
        record_props = records["items"]["properties"]
        record_props["card_sha256"] = {"enum": card_hashes}
        for key in ("facets", "uncertain"):
            record_props[key]["items"] = {"enum": facet_ids}
        record_props["facet_anchors"]["propertyNames"] = {"enum": facet_ids}
        return schema
    if role != "source":
        raise GradeClosureError("prepared-packet-role-invalid")
    base, _raw = packet_prep._reference("source-chunk.schema.json")
    schema = json.loads(json.dumps(base))
    chunk_id = model_input.get("chunk_id")
    facets = model_input.get("facets")
    sources = model_input.get("sources")
    if type(chunk_id) is not str or type(facets) is not list or type(sources) is not list:
        raise GradeClosureError("prepared-source-assignment")
    checks = [row.get("id") for row in facets if type(row) is dict]
    source_ids = [row.get("source_id") for row in sources if type(row) is dict]
    if len(checks) != len(facets) or len(source_ids) != len(sources):
        raise GradeClosureError("prepared-source-assignment")
    schema["properties"]["task_id"] = {"const": task_id}
    schema["properties"]["assessor_id"] = {"const": assessor_id}
    schema["properties"]["chunk_id"] = {"const": chunk_id}
    assessments = schema["properties"]["source_assessments"]
    assessments["minItems"] = len(sources)
    assessments["maxItems"] = len(sources)
    assessments["items"]["properties"]["source_id"] = {"enum": source_ids}
    facet_schema = assessments["items"]["properties"]["facets"]
    facet_schema["minItems"] = len(checks)
    facet_schema["maxItems"] = len(checks)
    facet_schema["items"]["properties"]["facet_id"] = {"enum": checks}
    return schema


def _validate_preparation_pins(
    prepared: PreparedAssessmentPackets,
    *,
    expected_manifest_sha256: str,
    expected_private_binding_sha256: str,
) -> None:
    """Require caller-held pins and rederive frozen references/assignment bindings."""
    from scripts import coverage_assessment_packets as packet_prep

    if (
        type(expected_manifest_sha256) is not str
        or not _SHA.fullmatch(expected_manifest_sha256)
        or _sha(prepared.preassessment_manifest_bytes) != expected_manifest_sha256
    ):
        raise GradeClosureError("trusted-preassessment-manifest-pin")
    if (
        type(expected_private_binding_sha256) is not str
        or not _SHA.fullmatch(expected_private_binding_sha256)
        or _sha(prepared.private_binding_bytes) != expected_private_binding_sha256
    ):
        raise GradeClosureError("trusted-private-binding-pin")
    binding = _strict_json(prepared.private_binding_bytes, "private-binding")
    if (
        type(binding) is not dict
        or binding.get("schema") != "coverage-assessment-private-bindings/1"
        or binding.get("packet_stage_uuid") != prepared.packet_stage_uuid
        or binding.get("source_stage_uuid") != prepared.source_stage_uuid
        or binding.get("blind_arm_labels") != prepared.blind_arm_labels
        or binding.get("task_card_identity_maps") != prepared.card_identity_maps
        or binding.get("task_catalogs") != prepared.task_catalogs
        or binding.get("manifest_sha256") != expected_manifest_sha256
        or binding.get("arm_label_map_sha256") != _sha(_canonical(prepared.blind_arm_labels))
    ):
        raise GradeClosureError("private-binding-assignment-mismatch")
    expected_refs: dict[str, str] = {}
    for name in (
        "card-assessment.schema.json",
        "source-chunk.schema.json",
        "answer-assessment.schema.json",
        "card-assessor-prompt.txt",
        "source-assessor-prompt.txt",
        "answer-assessor-prompt.txt",
    ):
        _value, raw = packet_prep._reference(name)
        expected_refs[name] = _sha(raw)
    if dict(prepared.reference_sha256) != expected_refs:
        raise GradeClosureError("frozen-reference-digest-mismatch")
    manifest = _strict_json(prepared.preassessment_manifest_bytes, "prepared-manifest")
    if type(manifest) is not dict or manifest.get("reference_sha256") != expected_refs:
        raise GradeClosureError("manifest-reference-digest-mismatch")
    for packet in prepared.preassessment_packets:
        envelope = _strict_json(packet.get("bytes"), "prepared-packet")
        if type(envelope) is not dict:
            raise GradeClosureError("prepared-packet-envelope")
        role = packet.get("role")
        expected_schema = _derive_preassessment_schema(envelope, str(role))
        if envelope.get("output_schema") != expected_schema:
            raise GradeClosureError("prepared-dynamic-schema-mismatch")


def _required_preassessment_packets(prepared: PreparedAssessmentPackets) -> tuple[dict[str, object], ...]:
    rows = []
    for packet in prepared.preassessment_packets:
        if packet.get("role") != "source":
            rows.append(packet)
            continue
        envelope = _strict_json(packet.get("bytes"), "prepared-source-packet")
        model_input = envelope.get("model_input") if type(envelope) is dict else None
        if type(model_input) is not dict or type(model_input.get("model_call_required")) is not bool:
            raise GradeClosureError("prepared-source-call-binding")
        if model_input["model_call_required"]:
            rows.append(packet)
    return tuple(rows)


def _verify_manifest_packets(
    manifest_bytes: bytes,
    manifest_sha256: str | None,
    packets: tuple[dict[str, object], ...],
    phase: str,
    stage_uuid: str,
    expected_manifest_sha256: str,
) -> None:
    if (
        type(expected_manifest_sha256) is not str
        or not _SHA.fullmatch(expected_manifest_sha256)
        or _sha(manifest_bytes) != expected_manifest_sha256
        or (manifest_sha256 is not None and manifest_sha256 != expected_manifest_sha256)
    ):
        raise GradeClosureError("prepared-manifest-digest")
    manifest = _strict_json(manifest_bytes, "prepared-manifest")
    if type(manifest) is not dict:
        raise GradeClosureError("prepared-manifest-shape")
    expected_schema = (
        "coverage-assessment-preparation-manifest/1"
        if phase == "preassessment"
        else "coverage-answer-assessment-manifest/1"
    )
    if (
        manifest.get("schema") != expected_schema
        or manifest.get("stage_uuid", manifest.get("packet_stage_uuid")) != stage_uuid
        or manifest.get("model_calls_performed") != 0
        or manifest.get("admission_created") is not False
        or manifest.get("quality_credit") is not False
        or manifest.get("status") == "incomplete"
    ):
        raise GradeClosureError("prepared-manifest-schema")
    rows = manifest.get("packets")
    if type(rows) is not list:
        raise GradeClosureError("prepared-manifest-packets")
    observed = {
        packet.get("packet_id"): {
            "packet_id": packet.get("packet_id"),
            "task_id": packet.get("task_id"),
            "assessor_id": packet.get("assessor_id"),
            "role": packet.get("role"),
            "sha256": packet.get("sha256"),
            "byte_count": packet.get("byte_count"),
        }
        for packet in packets
    }
    expected_rows = {}
    for row in rows:
        if type(row) is not dict or type(row.get("packet_id")) is not str:
            raise GradeClosureError("prepared-manifest-packet-row")
        packet_id = row["packet_id"]
        if packet_id in expected_rows:
            raise GradeClosureError("prepared-manifest-packet-duplicate")
        expected_rows[packet_id] = {
            "packet_id": packet_id,
            "task_id": row.get("task_id"),
            "assessor_id": row.get("assessor_id"),
            "role": row.get("role", "answer" if phase == "answer" else None),
            "sha256": row.get("sha256"),
            "byte_count": row.get("byte_count"),
        }
    if (
        len(observed) != len(packets)
        or len(expected_rows) != len(rows)
        or observed != expected_rows
        or manifest.get("grader_packet_count") != len(packets)
    ):
        raise GradeClosureError("prepared-manifest-packet-binding")


def _check_local_refs(schema: dict[str, object], node: object | None = None) -> None:
    """Reject remote schema references so validation cannot perform network I/O."""
    current = schema if node is None else node
    if type(current) is dict:
        ref = current.get("$ref")
        if ref is not None and (type(ref) is not str or not ref.startswith("#")):
            raise GradeClosureError("output-schema-external-ref")
        for value in current.values():
            _check_local_refs(schema, value)
    elif type(current) is list:
        for value in current:
            _check_local_refs(schema, value)


def _validate_schema(output: object, schema: dict[str, object]) -> None:
    _check_local_refs(schema)
    try:
        Draft202012Validator.check_schema(schema)
        errors = next(Draft202012Validator(schema).iter_errors(output), None)
    except Exception as exc:
        raise GradeClosureError("output-schema-invalid") from exc
    if errors:
        raise GradeClosureError("submission-schema-invalid")


def _validate_pre_assignment(packet: dict[str, object], output: object) -> dict[str, object]:
    envelope = packet["envelope"]
    schema = envelope["output_schema"]
    _validate_schema(output, schema)
    if type(output) is not dict:
        raise GradeClosureError("submission-output-object-required")
    model_input = envelope["model_input"]
    role = packet["role"]
    if role == "card":
        if type(model_input) is not dict or type(model_input.get("cards")) is not list:
            raise GradeClosureError("card-assignment-malformed")
        records = output.get("records")
        if type(records) is not list:
            raise GradeClosureError("card-record-list-required")
        expected = [row.get("card_sha256") for row in model_input["cards"] if type(row) is dict]
        observed = [row.get("card_sha256") for row in records if type(row) is dict]
        if len(expected) != len(model_input["cards"]) or observed != expected or len(set(observed)) != len(expected):
            raise GradeClosureError("card-record-assignment-coverage")
        return {"records": records}

    if role != "source" or type(model_input) is not dict:
        raise GradeClosureError("preassessment-role-invalid")
    if not model_input.get("model_call_required"):
        raise GradeClosureError("source-packet-not-callable")
    rows = output.get("source_assessments")
    expected_sources = [row.get("source_id") for row in model_input.get("sources", []) if type(row) is dict]
    if (
        type(rows) is not list
        or [row.get("source_id") if type(row) is dict else None for row in rows] != expected_sources
    ):
        raise GradeClosureError("source-assessment-coverage")
    check_ids = [row.get("id") for row in model_input.get("facets", []) if type(row) is dict]
    if len(check_ids) != 4:
        raise GradeClosureError("source-check-assignment")
    for row in rows:
        facets = row.get("facets")
        if (
            type(facets) is not list
            or [item.get("facet_id") if type(item) is dict else None for item in facets] != check_ids
        ):
            raise GradeClosureError("source-check-coverage")
        codes = {
            passage.get("evidence_id")
            for source in model_input["sources"]
            if source.get("source_id") == row["source_id"]
            for passage in source.get("passages", [])
            if type(passage) is dict
        }
        for facet in facets:
            evidence_ids = facet.get("evidence_ids") if type(facet) is dict else None
            if type(evidence_ids) is not list or not set(evidence_ids).issubset(codes):
                raise GradeClosureError("source-evidence-assignment")
    return {
        "task_id": packet["task_id"],
        "assessor_id": packet["assessor_id"],
        "chunk_id": model_input["chunk_id"],
        "source_assessments": rows,
    }


def _validate_answer_assignment(
    packet: dict[str, object], output: object, prepared: PreparedAssessmentPackets
) -> dict[str, object]:
    envelope = packet["envelope"]
    schema = envelope["output_schema"]
    _validate_schema(output, schema)
    if type(output) is not dict or type(output.get("tasks")) is not list or len(output["tasks"]) != 1:
        raise GradeClosureError("answer-task-assignment")
    task = output["tasks"][0]
    task_id = packet["task_id"]
    if (
        type(task) is not dict
        or task.get("task_id") != task_id
        or type(task.get("arms")) is not list
        or len(task["arms"]) != 1
    ):
        raise GradeClosureError("answer-task-assignment")
    answer_packet = envelope
    arm = task["arms"][0]
    blind_arm_id = answer_packet.get("blind_arm_id")
    if type(arm) is not dict or arm.get("arm_id") != blind_arm_id:
        raise GradeClosureError("answer-arm-assignment")
    checks = [row["id"] for row in prepared.task_checks[task_id]]
    facets = arm.get("facet_assessments")
    if type(facets) is not list or [row.get("facet_id") if type(row) is dict else None for row in facets] != checks:
        raise GradeClosureError("answer-check-coverage")
    source_ids = [row["source_id"] for row in prepared.task_metadata[task_id]["source_inventory"]]
    catalog_ids = {row["original_evidence_id"] for row in prepared.task_catalogs[task_id].values()}
    for facet_index, facet in enumerate(facets):
        dispositions = facet.get("evidence_dispositions")
        if (
            type(dispositions) is not list
            or [row.get("source_id") if type(row) is dict else None for row in dispositions] != source_ids
        ):
            raise GradeClosureError("answer-source-coverage")
        for disposition in dispositions:
            if type(disposition) is not dict or type(disposition.get("evidence_ids")) is not list:
                raise GradeClosureError("answer-source-evidence-assignment")
            source_catalog_ids = {
                row["original_evidence_id"]
                for row in prepared.task_catalogs[task_id].values()
                if row["source_id"] == disposition["source_id"]
            }
            if not set(disposition["evidence_ids"]).issubset(source_catalog_ids):
                raise GradeClosureError("answer-source-evidence-assignment")
        frozen_claims = answer_packet["answer"][facet_index]["claims"]
        claims = facet.get("claims")
        expected_claims = [(row["claim_id"], row["text"]) for row in frozen_claims]
        observed_claims = (
            [(row.get("claim_id"), row.get("text")) if type(row) is dict else (None, None) for row in claims]
            if type(claims) is list
            else []
        )
        if observed_claims != expected_claims:
            raise GradeClosureError("answer-frozen-claim-binding")
        for claim, frozen_claim in zip(claims, frozen_claims):
            frozen_evidence_ids = frozen_claim.get("evidence_ids")
            if (
                type(frozen_evidence_ids) is not list
                or claim.get("evidence_ids") != frozen_evidence_ids
                or not set(claim.get("evidence_ids", [])).issubset(catalog_ids)
            ):
                raise GradeClosureError("answer-claim-evidence-assignment")
        unsupported = facet.get("unsupported_claim_indices")
        if any(type(index) is not int or index < 0 or index >= len(expected_claims) for index in unsupported):
            raise GradeClosureError("answer-unsupported-claim-index")
    logical_arm = next(
        (arm_name for arm_name, label in prepared.blind_arm_labels[task_id].items() if label == blind_arm_id), None
    )
    if logical_arm is None:
        raise GradeClosureError("answer-blind-label-unmapped")
    return {"logical_arm": logical_arm, "assessor_id": packet["assessor_id"], "output": output}


def _validate_submissions(
    *, phase: str, stage_uuid: str, expected: dict[str, dict[str, object]], submissions: list[GradeSubmission]
) -> tuple[list[SubmissionHashReceipt], dict[str, object], dict[str, dict[str, object]]]:
    if type(submissions) not in {list, tuple}:
        raise GradeClosureError("submission-list-required")
    by_id: dict[str, GradeSubmission] = {}
    for submission in submissions:
        if (
            type(submission) is not GradeSubmission
            or type(submission.packet_id) is not str
            or submission.packet_id in by_id
        ):
            raise GradeClosureError("submission-duplicate-or-type")
        by_id[submission.packet_id] = submission
    if len(by_id) != len(expected) or set(by_id) != set(expected):
        raise GradeClosureError("submission-inventory-mismatch")
    receipts = []
    outputs: dict[str, object] = {}
    raw_by_id: dict[str, dict[str, object]] = {}
    for packet_id in sorted(expected):
        packet = expected[packet_id]
        submission = by_id[packet_id]
        required = {
            "task_id": packet["task_id"],
            "stage_uuid": stage_uuid,
            "role": packet["role"],
            "assessor_id": packet["assessor_id"],
            "input_sha256": packet["sha256"],
        }
        for field, expected_value in required.items():
            if getattr(submission, field) != expected_value:
                raise GradeClosureError(f"submission-{field}-binding")
        output = _strict_json(submission.response_bytes, "submission")
        _validate_schema(output, packet["envelope"]["output_schema"])
        receipts.append(
            SubmissionHashReceipt(
                packet_id=packet_id,
                task_id=submission.task_id,
                stage_uuid=submission.stage_uuid,
                role=submission.role,
                assessor_id=submission.assessor_id,
                input_sha256=submission.input_sha256,
                response_sha256=_sha(submission.response_bytes),
                response_byte_count=len(submission.response_bytes),
            )
        )
        outputs[packet_id] = output
        raw_by_id[packet_id] = packet
    return receipts, outputs, raw_by_id


def _closure(
    *,
    phase: str,
    stage_uuid: str,
    receipts: list[SubmissionHashReceipt],
    exposed_outputs: dict[str, object],
    source_outputs: dict[str, dict[str, list[dict[str, object]]]] | None = None,
) -> GradeClosure:
    exposed_bytes = _canonical(exposed_outputs)
    source_bytes = _canonical(source_outputs) if source_outputs is not None else None
    if len(exposed_bytes) > MAX_CLOSED_OUTPUT_BYTES or (
        source_bytes is not None and len(source_bytes) > MAX_CLOSED_OUTPUT_BYTES
    ):
        raise GradeClosureError("closed-output-bytes-invalid")
    receipt_body = {
        "schema": "coverage-grade-closure-receipt/1",
        "phase": phase,
        "stage_uuid": stage_uuid,
        "status": "complete",
        "submissions": [receipt.__dict__ for receipt in receipts],
        "closed_outputs_sha256": _sha(exposed_bytes),
        "closed_source_outputs_sha256": _sha(source_bytes) if source_bytes is not None else None,
        "semantic_grade": False,
        "quality_credit": False,
    }
    receipt_bytes = _canonical(receipt_body)
    return GradeClosure(
        phase=phase,
        stage_uuid=stage_uuid,
        receipt_bytes=receipt_bytes,
        receipt_sha256=_sha(receipt_bytes),
        submission_receipts=tuple(receipts),
        _outputs_bytes=exposed_bytes,
        _source_outputs_bytes=source_bytes,
    )


def close_preassessment(
    prepared: PreparedAssessmentPackets,
    submissions: list[GradeSubmission],
    *,
    expected_manifest_sha256: str,
    expected_private_binding_sha256: str,
) -> GradeClosure:
    """Close all required card and successful-source grader submissions."""
    if type(prepared) is not PreparedAssessmentPackets:
        raise GradeClosureError("prepared-assessment-required")
    _validate_preparation_pins(
        prepared,
        expected_manifest_sha256=expected_manifest_sha256,
        expected_private_binding_sha256=expected_private_binding_sha256,
    )
    _verify_manifest_packets(
        prepared.preassessment_manifest_bytes,
        None,
        prepared.preassessment_packets,
        "preassessment",
        prepared.packet_stage_uuid,
        expected_manifest_sha256,
    )
    packets = _required_preassessment_packets(prepared)
    expected = _expected_packets(packets, answer_phase=False)
    receipts, raw_outputs, packet_by_id = _validate_submissions(
        phase="preassessment", stage_uuid=prepared.packet_stage_uuid, expected=expected, submissions=submissions
    )
    cards: dict[str, dict[str, list[dict[str, object]]]] = {}
    source_chunks: list[dict[str, object]] = []
    source_outputs: dict[str, dict[str, list[dict[str, object]]]] = {}
    for packet_id, output in raw_outputs.items():
        packet = packet_by_id[packet_id]
        validated = _validate_pre_assignment(packet, output)
        task_id = str(packet["task_id"])
        assessor = str(packet["assessor_id"])
        if packet["role"] == "card":
            reverse = {fingerprint: card_id for fingerprint, card_id in prepared.card_identity_maps[task_id].items()}
            records = []
            for record in validated["records"]:
                fingerprint = record["card_sha256"]
                card_id = reverse.get(fingerprint)
                if card_id is None:
                    raise GradeClosureError("card-fingerprint-unmapped")
                records.append({**record, "card_id": card_id})
            cards.setdefault(task_id, {})[assessor] = records
        else:
            chunk = {
                "task_id": task_id,
                "assessor_id": assessor,
                "chunk_id": validated["chunk_id"],
                "source_assessments": validated["source_assessments"],
            }
            source_chunks.append(chunk)
            source_outputs.setdefault(task_id, {}).setdefault(assessor, []).append(chunk)
    for task_id in prepared.task_metadata:
        if set(cards.get(task_id, {})) != {"A", "B"}:
            raise GradeClosureError("card-assessor-coverage")
        if set(source_outputs.get(task_id, {})) != {
            assessor
            for assessor in ("R1", "R2")
            if any(
                packet.get("task_id") == task_id
                and packet.get("role") == "source"
                and packet.get("assessor_id") == assessor
                and json.loads(packet["bytes"])["model_input"]["model_call_required"]
                for packet in prepared.preassessment_packets
            )
        }:
            raise GradeClosureError("source-assessor-coverage")
        source_outputs.setdefault(task_id, {})
    # Ensure source-output map includes explicit empty R1/R2 lists for every task.
    source_for_answer = {
        task_id: {
            assessor: sorted(source_outputs.get(task_id, {}).get(assessor, []), key=lambda row: row["chunk_id"])
            for assessor in ("R1", "R2")
        }
        for task_id in prepared.task_metadata
    }
    source_grades = {
        task_id: {
            assessor: [
                source_row
                for chunk in source_for_answer[task_id][assessor]
                for source_row in chunk["source_assessments"]
            ]
            for assessor in ("R1", "R2")
        }
        for task_id in prepared.task_metadata
    }
    exposed = {
        "cards": cards,
        "source_chunks": sorted(source_chunks, key=lambda row: row["chunk_id"]),
        "sources": source_grades,
    }
    return _closure(
        phase="preassessment",
        stage_uuid=prepared.packet_stage_uuid,
        receipts=receipts,
        exposed_outputs=exposed,
        source_outputs=source_for_answer,
    )


def _consolidated_source_view(
    source_outputs: Mapping[str, Mapping[str, list[dict[str, object]]]], task_id: str, assessor: str
) -> dict[str, object]:
    rows = [
        row for row in source_outputs.get(task_id, {}).get(assessor, []) for row in row.get("source_assessments", [])
    ]
    return {
        "packet_kind": "source",
        "assessor_id": assessor,
        "tasks": [{"task_id": task_id, "source_assessments": rows}],
    }


def close_answer_assessments(
    prepared: PreparedAssessmentPackets,
    prepared_answers: PreparedAnswerPackets,
    pre_closure: GradeClosure,
    submissions: list[GradeSubmission],
    *,
    expected_preassessment_manifest_sha256: str,
    expected_private_binding_sha256: str,
    expected_answer_manifest_sha256: str,
) -> GradeClosure:
    """Close all 32 R1/R2 answer assessments and restore blind arm labels."""
    if (
        type(prepared) is not PreparedAssessmentPackets
        or type(prepared_answers) is not PreparedAnswerPackets
        or type(pre_closure) is not GradeClosure
        or pre_closure.phase != "preassessment"
        or pre_closure.stage_uuid != prepared.packet_stage_uuid
    ):
        raise GradeClosureError("matching-preassessment-closure-required")
    _validate_preparation_pins(
        prepared,
        expected_manifest_sha256=expected_preassessment_manifest_sha256,
        expected_private_binding_sha256=expected_private_binding_sha256,
    )
    _verify_manifest_packets(
        prepared_answers.manifest_bytes,
        prepared_answers.manifest_sha256,
        prepared_answers.packets,
        "answer",
        prepared.packet_stage_uuid,
        expected_answer_manifest_sha256,
    )
    answer_manifest = _strict_json(prepared_answers.manifest_bytes, "prepared-manifest")
    if (
        type(answer_manifest) is not dict
        or answer_manifest.get("private_binding_sha256") != expected_private_binding_sha256
    ):
        raise GradeClosureError("answer-manifest-private-binding")
    source_outputs = pre_closure.source_outputs_for_answer()
    expected = _expected_packets(prepared_answers.packets, answer_phase=True)
    if len(expected) != 32:
        raise GradeClosureError("answer-packet-count")
    receipts, raw_outputs, packet_by_id = _validate_submissions(
        phase="answer", stage_uuid=prepared.packet_stage_uuid, expected=expected, submissions=submissions
    )
    answer_grades: dict[str, dict[str, dict[str, object]]] = {}
    for packet_id, output in raw_outputs.items():
        packet = packet_by_id[packet_id]
        envelope = packet["envelope"]
        model_input = envelope
        task_id = str(packet["task_id"])
        assessor = str(packet["assessor_id"])
        if type(model_input) is not dict:
            raise GradeClosureError("answer-input-assignment")
        expected_source_view = _consolidated_source_view(source_outputs, task_id, assessor)
        if model_input.get("same_assessor_source_assessment") != expected_source_view:
            raise GradeClosureError("answer-source-grade-join")
        validated = _validate_answer_assignment(packet, output, prepared)
        logical_arm = validated["logical_arm"]
        if assessor in answer_grades.setdefault(task_id, {}).setdefault(logical_arm, {}):
            raise GradeClosureError("answer-assessor-duplicate")
        answer_grades[task_id][logical_arm][assessor] = output
    for task_id in prepared.task_metadata:
        if set(answer_grades.get(task_id, {})) != {"w0", "candidate"}:
            raise GradeClosureError("answer-arm-coverage")
        if any(set(answer_grades[task_id][arm]) != {"R1", "R2"} for arm in ("w0", "candidate")):
            raise GradeClosureError("answer-assessor-coverage")
    exposed = {
        "cards": pre_closure.outputs()["cards"],
        "source_chunks": pre_closure.outputs()["source_chunks"],
        "sources": pre_closure.outputs()["sources"],
        "answers": answer_grades,
        "preassessment_receipt_sha256": pre_closure.receipt_sha256,
    }
    return _closure(
        phase="answer",
        stage_uuid=prepared.packet_stage_uuid,
        receipts=receipts,
        exposed_outputs=exposed,
    )


__all__ = [
    "GradeClosure",
    "GradeClosureError",
    "GradeSubmission",
    "SubmissionHashReceipt",
    "close_answer_assessments",
    "close_preassessment",
]
