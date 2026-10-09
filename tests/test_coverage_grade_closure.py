"""Synthetic-only strict closure tests; no provider submissions or prior grades."""

import hashlib
import json

import pytest

from scripts import coverage_assessment_packets as packet_prep
from scripts import coverage_grade_closure as closure
from tests.test_coverage_assessment_packets import _answer_stage, _fixture


def _output_bytes(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def _pre_pins(prepared):
    return {
        "expected_manifest_sha256": hashlib.sha256(prepared.preassessment_manifest_bytes).hexdigest(),
        "expected_private_binding_sha256": prepared.private_binding_sha256,
    }


def _answer_pins(prepared, prepared_answers):
    return {
        "expected_preassessment_manifest_sha256": hashlib.sha256(prepared.preassessment_manifest_bytes).hexdigest(),
        "expected_private_binding_sha256": prepared.private_binding_sha256,
        "expected_answer_manifest_sha256": prepared_answers.manifest_sha256,
    }


def _source_output(packet):
    envelope = json.loads(packet["bytes"])
    model_input = envelope["model_input"]
    return {
        "task_id": packet["task_id"],
        "assessor_id": packet["assessor_id"],
        "chunk_id": model_input["chunk_id"],
        "source_assessments": [
            {
                "source_id": source["source_id"],
                "facets": [
                    {
                        "facet_id": check["id"],
                        "disposition": "not_assessable",
                        "scope_match": "not_assessable",
                        "entailment": "not_assessable",
                        "applicability": "not_assessable",
                        "not_assessable_reason": "Synthetic unresolved fixture.",
                        "evidence_ids": [],
                        "reason": "Synthetic unresolved fixture.",
                        "limitations": [],
                    }
                    for check in model_input["facets"]
                ],
            }
            for source in model_input["sources"]
        ],
    }


def _card_output(packet):
    model_input = json.loads(packet["bytes"])["model_input"]
    return {
        "records": [
            {
                "card_sha256": card["card_sha256"],
                "lead": 2,
                "visible": 1,
                "facets": [],
                "uncertain": [],
                "rationale": "Synthetic typed fixture.",
                "anchors": [{"field": "title", "quote": card["title"]}],
                "facet_anchors": {},
            }
            for card in model_input["cards"]
        ]
    }


def _pre_submissions(prepared):
    submissions = []
    for packet in prepared.preassessment_packets:
        envelope = json.loads(packet["bytes"])
        model_input = envelope["model_input"]
        if packet["role"] == "source" and not model_input["model_call_required"]:
            continue
        output = _card_output(packet) if packet["role"] == "card" else _source_output(packet)
        submissions.append(
            closure.GradeSubmission(
                packet_id=packet["packet_id"],
                task_id=packet["task_id"],
                stage_uuid=prepared.packet_stage_uuid,
                role=packet["role"],
                assessor_id=packet["assessor_id"],
                input_sha256=packet["sha256"],
                response_bytes=_output_bytes(output),
            )
        )
    return submissions


def _answer_outputs(prepared_answers, prepared):
    submissions = []
    for packet in prepared_answers.packets:
        envelope = json.loads(packet["bytes"])
        model_input = envelope
        task_id = packet["task_id"]
        source_ids = [row["source_id"] for row in prepared.task_metadata[task_id]["source_inventory"]]
        facets = []
        for frozen_facet in model_input["answer"]:
            check_id = frozen_facet["facet_id"]
            facets.append(
                {
                    "facet_id": check_id,
                    "status": "not_assessable",
                    "not_assessable_reason": "Synthetic unresolved fixture.",
                    "conclusion": frozen_facet["conclusion"],
                    "claims": frozen_facet["claims"],
                    "unsupported_claim_indices": [],
                    "unsupported_material_claims": [],
                    "missed_qualifications": [],
                    "evidence_found_but_unused": [],
                    "evidence_dispositions": [
                        {
                            "source_id": source_id,
                            "disposition": "not_assessable",
                            "not_assessable_reason": "Synthetic unresolved fixture.",
                            "evidence_ids": [],
                            "reason": "Synthetic unresolved fixture.",
                        }
                        for source_id in source_ids
                    ],
                    "scope_match": "not_assessable",
                    "entailment": "not_assessable",
                    "applicability": "not_assessable",
                    "reason": "Synthetic unresolved fixture.",
                }
            )
        output = {
            "schema": "research-usefulness-assessment/1",
            "packet_id": packet["packet_id"],
            "assessor_id": packet["assessor_id"],
            "packet_kind": "answer",
            "tasks": [
                {
                    "task_id": task_id,
                    "arms": [
                        {
                            "arm_id": model_input["blind_arm_id"],
                            "facet_assessments": facets,
                            "limitations": [],
                        }
                    ],
                }
            ],
        }
        submissions.append(
            closure.GradeSubmission(
                packet_id=packet["packet_id"],
                task_id=task_id,
                stage_uuid=prepared.packet_stage_uuid,
                role="answer",
                assessor_id=packet["assessor_id"],
                input_sha256=packet["sha256"],
                response_bytes=_output_bytes(output),
            )
        )
    return submissions


def _closed_prepared():
    prepared, captures = _fixture()
    pre = closure.close_preassessment(prepared, _pre_submissions(prepared), **_pre_pins(prepared))
    answer_inputs, answer_outputs, _ = _answer_stage(prepared, captures)
    prepared_answers = packet_prep.prepare_answer_assessment_packets(
        prepared=prepared,
        answer_inputs=answer_inputs,
        answer_outputs=answer_outputs,
        source_outputs=pre.source_outputs_for_answer(),
    )
    return prepared, pre, prepared_answers


def test_preassessment_closes_exact_cards_sources_and_restores_card_ids_after_phase():
    prepared, _ = _fixture()
    submissions = _pre_submissions(prepared)
    result = closure.close_preassessment(prepared, submissions, **_pre_pins(prepared))
    out = result.outputs()
    assert result.phase == "preassessment"
    assert len(result.submission_receipts) == len(submissions)
    assert result.receipt_sha256 == hashlib.sha256(result.receipt_bytes).hexdigest()
    assert set(out["cards"]) == set(prepared.task_metadata)
    assert set(out["cards"]["D-R01"]) == {"A", "B"}
    assert {row["card_id"] for row in out["cards"]["D-R01"]["A"]} == set(prepared.task_metadata["D-R01"]["card_ids"])
    assert all(
        row["disposition"] == "not_assessable"
        for chunk in out["source_chunks"]
        for source in chunk["source_assessments"]
        for row in source["facets"]
    )
    source_outputs = result.source_outputs_for_answer()
    assert set(source_outputs["D-R01"]) == {"R1", "R2"}
    source_outputs["D-R01"]["R1"].clear()
    assert result.source_outputs_for_answer()["D-R01"]["R1"]


def test_preassessment_requires_exact_complete_inventory_and_bindings():
    prepared, _ = _fixture()
    submissions = _pre_submissions(prepared)
    with pytest.raises(closure.GradeClosureError, match="submission-inventory-mismatch"):
        closure.close_preassessment(prepared, submissions[:-1], **_pre_pins(prepared))
    with pytest.raises(closure.GradeClosureError, match="submission-duplicate-or-type"):
        closure.close_preassessment(prepared, submissions + [submissions[0]], **_pre_pins(prepared))
    wrong = list(submissions)
    row = wrong[0]
    wrong[0] = closure.GradeSubmission(**{**row.__dict__, "input_sha256": "0" * 64})
    with pytest.raises(closure.GradeClosureError, match="submission-input_sha256-binding"):
        closure.close_preassessment(prepared, wrong, **_pre_pins(prepared))


def test_preassessment_rejects_untrusted_manifest_and_resealed_weak_dynamic_schema():
    from dataclasses import replace

    prepared, _ = _fixture()
    submissions = _pre_submissions(prepared)
    pins = _pre_pins(prepared)
    with pytest.raises(closure.GradeClosureError, match="trusted-preassessment-manifest-pin"):
        closure.close_preassessment(
            prepared,
            submissions,
            **{**pins, "expected_manifest_sha256": "0" * 64},
        )

    # Reseal the packet and both self-describing envelopes with a deliberately
    # weak schema. The caller-held pins are also updated here to prove the
    # frozen-reference/assignment schema derivation catches that forgery.
    packets = list(prepared.preassessment_packets)
    original = packets[0]
    packet_envelope = json.loads(original["bytes"])
    packet_envelope["output_schema"] = {"type": "object"}
    packet_raw = _output_bytes(packet_envelope)
    packets[0] = {
        **original,
        "bytes": packet_raw,
        "sha256": hashlib.sha256(packet_raw).hexdigest(),
        "byte_count": len(packet_raw),
    }
    manifest = json.loads(prepared.preassessment_manifest_bytes)
    row = next(row for row in manifest["packets"] if row["packet_id"] == original["packet_id"])
    row["sha256"] = packets[0]["sha256"]
    row["byte_count"] = packets[0]["byte_count"]
    manifest_raw = _output_bytes(manifest)
    binding = json.loads(prepared.private_binding_bytes)
    binding["manifest_sha256"] = hashlib.sha256(manifest_raw).hexdigest()
    binding_raw = _output_bytes(binding)
    forged = replace(
        prepared,
        preassessment_packets=tuple(packets),
        preassessment_manifest_bytes=manifest_raw,
        private_binding_bytes=binding_raw,
        private_binding_sha256=hashlib.sha256(binding_raw).hexdigest(),
    )
    with pytest.raises(closure.GradeClosureError, match="prepared-dynamic-schema-mismatch"):
        closure.close_preassessment(
            forged,
            _pre_submissions(forged),
            **_pre_pins(forged),
        )
    wrong = list(submissions)
    row = wrong[0]
    wrong[0] = closure.GradeSubmission(**{**row.__dict__, "role": "source"})
    with pytest.raises(closure.GradeClosureError, match="submission-role-binding"):
        closure.close_preassessment(prepared, wrong, **_pre_pins(prepared))


def test_preassessment_rejects_bad_schema_duplicate_keys_and_nonfinite_numbers():
    prepared, _ = _fixture()
    submissions = _pre_submissions(prepared)
    row = submissions[0]
    bad = list(submissions)
    bad[0] = closure.GradeSubmission(**{**row.__dict__, "response_bytes": b'{"records":[],"records":[]}'})
    with pytest.raises(closure.GradeClosureError, match="submission-duplicate-key"):
        closure.close_preassessment(prepared, bad, **_pre_pins(prepared))
    bad[0] = closure.GradeSubmission(**{**row.__dict__, "response_bytes": b'{"records":[NaN]}'})
    with pytest.raises(closure.GradeClosureError, match="submission-nonfinite"):
        closure.close_preassessment(prepared, bad, **_pre_pins(prepared))
    bad[0] = closure.GradeSubmission(**{**row.__dict__, "response_bytes": b'{"records":[]}'})
    with pytest.raises(closure.GradeClosureError, match="submission-schema-invalid"):
        closure.close_preassessment(prepared, bad, **_pre_pins(prepared))


def test_preassessment_rejects_duplicate_assignment_and_wrong_source_chunk():
    prepared, _ = _fixture()
    submissions = _pre_submissions(prepared)
    source_index = next(i for i, row in enumerate(submissions) if row.role == "source")
    row = submissions[source_index]
    output = json.loads(row.response_bytes)
    output["source_assessments"].append(output["source_assessments"][0])
    malformed = list(submissions)
    malformed[source_index] = closure.GradeSubmission(**{**row.__dict__, "response_bytes": _output_bytes(output)})
    with pytest.raises(closure.GradeClosureError, match="submission-schema-invalid|source-assessment-coverage"):
        closure.close_preassessment(prepared, malformed, **_pre_pins(prepared))


def test_answer_phase_requires_preclosure_and_closes_exact_32_with_blind_identity_restoration():
    prepared, pre, prepared_answers = _closed_prepared()
    submissions = _answer_outputs(prepared_answers, prepared)
    result = closure.close_answer_assessments(
        prepared, prepared_answers, pre, submissions, **_answer_pins(prepared, prepared_answers)
    )
    out = result.outputs()
    assert result.phase == "answer"
    assert len(result.submission_receipts) == 32
    assert set(out["answers"]) == set(prepared.task_metadata)
    assert set(out["answers"]["D-R01"]) == {"w0", "candidate"}
    assert set(out["answers"]["D-R01"]["w0"]) == {"R1", "R2"}
    assert (
        out["answers"]["D-R01"]["w0"]["R1"]["tasks"][0]["arms"][0]["facet_assessments"][0]["status"] == "not_assessable"
    )
    with pytest.raises(closure.GradeClosureError, match="matching-preassessment-closure-required"):
        closure.close_answer_assessments(
            prepared,
            prepared_answers,
            closure.GradeClosureError("x"),
            submissions,
            **_answer_pins(prepared, prepared_answers),
        )
    with pytest.raises(closure.GradeClosureError, match="submission-inventory-mismatch"):
        closure.close_answer_assessments(
            prepared, prepared_answers, pre, submissions[:-1], **_answer_pins(prepared, prepared_answers)
        )


def test_answer_phase_rejects_blind_arm_claim_binding_source_join_and_citation_drift():
    prepared, pre, prepared_answers = _closed_prepared()
    submissions = _answer_outputs(prepared_answers, prepared)
    row = submissions[0]
    output = json.loads(row.response_bytes)
    output["tasks"][0]["arms"][0]["arm_id"] = "candidate"
    malformed = list(submissions)
    malformed[0] = closure.GradeSubmission(**{**row.__dict__, "response_bytes": _output_bytes(output)})
    with pytest.raises(closure.GradeClosureError, match="submission-schema-invalid|answer-arm-assignment"):
        closure.close_answer_assessments(
            prepared, prepared_answers, pre, malformed, **_answer_pins(prepared, prepared_answers)
        )
    # Tamper the packet's joined source evaluation while preserving all grader inputs.
    packet_rows = list(prepared_answers.packets)
    original = packet_rows[0]
    envelope = json.loads(original["bytes"])
    envelope["same_assessor_source_assessment"]["tasks"][0]["source_assessments"].pop()
    raw = _output_bytes(envelope)
    packet_sha = hashlib.sha256(raw).hexdigest()
    packet_rows[0] = {**original, "bytes": raw, "sha256": packet_sha, "byte_count": len(raw)}
    manifest = json.loads(prepared_answers.manifest_bytes)
    for manifest_row in manifest["packets"]:
        if manifest_row["packet_id"] == original["packet_id"]:
            manifest_row["sha256"] = packet_sha
            manifest_row["byte_count"] = len(raw)
    manifest_bytes = _output_bytes(manifest)
    tampered_answers = packet_prep.PreparedAnswerPackets(
        tuple(packet_rows), manifest_bytes, hashlib.sha256(manifest_bytes).hexdigest()
    )
    tampered_submissions = list(submissions)
    submission_index = next(
        i for i, submission in enumerate(tampered_submissions) if submission.packet_id == original["packet_id"]
    )
    tampered_submissions[submission_index] = closure.GradeSubmission(
        **{**tampered_submissions[submission_index].__dict__, "input_sha256": packet_sha}
    )
    with pytest.raises(closure.GradeClosureError, match="answer-source-grade-join"):
        closure.close_answer_assessments(
            prepared,
            tampered_answers,
            pre,
            tampered_submissions,
            **_answer_pins(prepared, tampered_answers),
        )

    # A different valid catalog citation still must not replace the frozen
    # citations attached to a claim in the answer packet.
    submissions = _answer_outputs(prepared_answers, prepared)
    row = submissions[0]
    output = json.loads(row.response_bytes)
    facet = output["tasks"][0]["arms"][0]["facet_assessments"][0]
    claim = facet["claims"][0]
    frozen_packet = next(packet for packet in prepared_answers.packets if packet["packet_id"] == row.packet_id)
    frozen_claim = json.loads(frozen_packet["bytes"])["answer"][0]["claims"][0]
    valid_other = next(
        evidence_id
        for evidence_id in {
            entry["original_evidence_id"] for rows in prepared.task_catalogs.values() for entry in rows.values()
        }
        if evidence_id not in frozen_claim["evidence_ids"]
    )
    claim["evidence_ids"] = [valid_other]
    altered = list(submissions)
    altered[0] = closure.GradeSubmission(**{**row.__dict__, "response_bytes": _output_bytes(output)})
    with pytest.raises(closure.GradeClosureError, match="answer-claim-evidence-assignment"):
        closure.close_answer_assessments(
            prepared,
            prepared_answers,
            pre,
            altered,
            **_answer_pins(prepared, prepared_answers),
        )


def test_grade_response_depth_and_size_are_bounded():
    with pytest.raises(closure.GradeClosureError, match="submission-json-depth"):
        closure._strict_json(b"{" * 66 + b"}" * 66, "test")
    with pytest.raises(closure.GradeClosureError, match="test-bytes-invalid"):
        closure._strict_json(b"x" * (closure.MAX_RESPONSE_BYTES + 1), "test")


def test_unavailable_zero_call_packet_is_excluded_and_extra_submission_rejected():
    from dataclasses import replace

    prepared, _ = _fixture()
    base = next(packet for packet in prepared.preassessment_packets if packet["role"] == "source")
    envelope = json.loads(base["bytes"])
    envelope["packet_id"] = "synthetic-unavailable-no-call"
    envelope["model_input"]["chunk_id"] = "synthetic-unavailable-chunk"
    envelope["model_input"]["model_call_required"] = False
    envelope["model_input"]["source_inventory"] = [{"source_id": "s-missing", "state": "not_acquired"}]
    envelope["model_input"]["sources"] = []
    envelope["output_schema"] = closure._derive_preassessment_schema(envelope, "source")
    raw = _output_bytes(envelope)
    no_call = {
        "packet_id": envelope["packet_id"],
        "task_id": base["task_id"],
        "assessor_id": base["assessor_id"],
        "role": "source",
        "bytes": raw,
        "sha256": hashlib.sha256(raw).hexdigest(),
        "byte_count": len(raw),
    }
    packet_rows = (*prepared.preassessment_packets, no_call)
    manifest = json.loads(prepared.preassessment_manifest_bytes)
    manifest["packets"].append(
        {key: no_call[key] for key in ("packet_id", "task_id", "assessor_id", "role", "sha256", "byte_count")}
    )
    manifest["grader_packet_count"] = len(packet_rows)
    manifest_bytes = _output_bytes(manifest)
    binding = json.loads(prepared.private_binding_bytes)
    binding["manifest_sha256"] = hashlib.sha256(manifest_bytes).hexdigest()
    binding_bytes = _output_bytes(binding)
    prepared_with_no_call = replace(
        prepared,
        preassessment_packets=packet_rows,
        preassessment_manifest_bytes=manifest_bytes,
        private_binding_bytes=binding_bytes,
        private_binding_sha256=hashlib.sha256(binding_bytes).hexdigest(),
    )
    submissions = _pre_submissions(prepared_with_no_call)
    result = closure.close_preassessment(prepared_with_no_call, submissions, **_pre_pins(prepared_with_no_call))
    assert len(result.submission_receipts) == len(_pre_submissions(prepared))
    assert no_call["packet_id"] not in {row.packet_id for row in result.submission_receipts}
    extra = closure.GradeSubmission(
        packet_id=no_call["packet_id"],
        task_id=no_call["task_id"],
        stage_uuid=prepared.packet_stage_uuid,
        role="source",
        assessor_id=no_call["assessor_id"],
        input_sha256=no_call["sha256"],
        response_bytes=b"{}",
    )
    with pytest.raises(closure.GradeClosureError, match="submission-inventory-mismatch"):
        closure.close_preassessment(prepared_with_no_call, submissions + [extra], **_pre_pins(prepared_with_no_call))
