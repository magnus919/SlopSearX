"""Synthetic-only coverage gate calculation tests; no provider calls or prior grades."""

from __future__ import annotations

import hashlib
import json
import random
import statistics
import uuid
from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace

from scripts import coverage_assessment_packets as packet_prep
from scripts import coverage_gate_calculation as gates
from scripts import coverage_grade_closure as closure
from scripts.coverage_grade_closure import GradeClosure
from scripts.coverage_stage_orchestration import SelectorEvidence
from scripts.coverage_study_core import selector_inventory
from tests.test_coverage_assessment_packets import _answer_stage
from tests.test_coverage_assessment_packets import _fixture as _packet_fixture
from tests.test_coverage_grade_closure import (
    _answer_outputs,
    _answer_pins,
    _card_output,
    _closed_prepared,
    _output_bytes,
    _pre_pins,
)


def _canonical(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _closure(phase: str, stage_uuid: str, outputs: dict) -> GradeClosure:
    outputs_bytes = _canonical(outputs)
    receipt_bytes = _canonical(
        {
            "schema": "synthetic-closed-output-test-receipt/1",
            "phase": phase,
            "stage_uuid": stage_uuid,
            "closed_outputs_sha256": _sha(outputs_bytes),
        }
    )
    return GradeClosure(
        phase=phase,
        stage_uuid=stage_uuid,
        receipt_bytes=receipt_bytes,
        receipt_sha256=_sha(receipt_bytes),
        submission_receipts=(),
        _outputs_bytes=outputs_bytes,
    )


def _fixture():
    protocol_path = Path(gates.__file__).parents[1] / "docs/experiments/evidence/coverage-first-study/protocol.json"
    protocol = json.loads(protocol_path.read_text(encoding="utf-8"))
    source_stage_uuid = str(uuid.UUID(int=41001))
    packet_stage_uuid = str(uuid.UUID(int=41002))
    task_inputs = []
    orders = {}
    reference_cards = {}
    task_catalogs = {}
    answer_outputs = {}
    for task_index in range(1, 9):
        task_id = f"D-R{task_index:02d}"
        band = "research_le_40" if task_index <= 4 else "research_41_80"
        count = 20 if task_index <= 4 else 41
        card_ids = [f"c{task_index:02d}-{card:02d}" for card in range(1, count + 1)]
        facet_ids = [f"F{task_index:02d}-A", f"F{task_index:02d}-B"]
        task_inputs.append(
            {
                "task_id": task_id,
                "card_ids": card_ids,
                "facet_ids": facet_ids,
                "band": band,
                "intent": "evidence_seeking" if task_index % 2 else "source_discovery",
            }
        )
        # Natural/W0 has grade-0 cards first. The candidate promotes the
        # independently useful grade-3 cards, retaining the full pool.
        records = []
        for index, card_id in enumerate(card_ids):
            lead = 0 if index < 10 else 3 if index < 20 else 1
            records.append({"card_id": card_id, "lead": lead, "facets": facet_ids if lead >= 2 else []})
        reference_cards[task_id] = {assessor: [dict(row) for row in records] for assessor in ("A", "B")}
        w0 = list(card_ids)
        candidate = card_ids[10:20] + card_ids[:10] + card_ids[20:]
        orders[task_id] = {"w0": w0, "candidate": candidate}

        source_id = f"S-{task_id}"
        evidence_id = f"E-{task_id}"
        code = f"D{task_index}E0001"
        task_catalogs[task_id] = {code: {"code": code, "original_evidence_id": evidence_id, "source_id": source_id}}
        by_arm = {}
        for arm in ("w0", "candidate"):
            by_assessor = {}
            for assessor in ("R1", "R2"):
                should_complete = task_index <= (5 if arm == "candidate" else 2)
                facets = []
                for check_id in [f"{facet}-C{check}" for facet in facet_ids for check in (1, 2)]:
                    facets.append(
                        {
                            "facet_id": check_id,
                            "status": "complete" if should_complete else "partial",
                            "scope_match": "match",
                            "entailment": "supports",
                            "applicability": "applicable",
                            "unsupported_material_claims": [],
                            "missed_qualifications": [],
                            "unsupported_claim_indices": [],
                            "claims": [{"evidence_ids": [evidence_id]}],
                            "evidence_dispositions": [
                                {
                                    "source_id": source_id,
                                    "disposition": "used_with_support",
                                    "evidence_ids": [evidence_id],
                                }
                            ],
                        }
                    )
                by_assessor[assessor] = {
                    "tasks": [{"task_id": task_id, "arms": [{"arm_id": f"blind-{arm}", "facet_assessments": facets}]}]
                }
            by_arm[arm] = by_assessor
        answer_outputs[task_id] = by_arm

    reference = _closure("preassessment", packet_stage_uuid, {"cards": reference_cards})
    answer = _closure(
        "answer",
        packet_stage_uuid,
        {"answers": answer_outputs, "preassessment_receipt_sha256": reference.receipt_sha256},
    )
    operation_ids = [call.operation_id for call in selector_inventory((None,) * 5)]
    selector = SelectorEvidence(
        stage_uuid=source_stage_uuid,
        terminal_inventory_sha256="a" * 64,
        operation_rows=tuple({"operation_id": op, "state": "complete-success"} for op in operation_ids),
        research_orders=orders,
        navigation_rank_one={f"N{index:02d}": True for index in range(1, 6)},
        reference_grade_receipt_sha256=reference.receipt_sha256,
        usage_status="known",
    )
    stability = {}
    for index in (1, 4, 7, 8):
        task = task_inputs[index - 1]
        task_id = task["task_id"]
        stability[task_id] = {
            arm: {"base": orders[task_id][arm], "repeat": orders[task_id][arm], "rotate": orders[task_id][arm]}
            for arm in ("w0", "candidate")
        }
    navigation_inputs = [
        {"task_id": f"N{index:02d}", "target_url": f"https://github.com/example/repo-{index}"} for index in range(1, 6)
    ]
    navigation_observations = {
        row["task_id"]: {"acquired_urls": [row["target_url"]], "candidate_top1_url": row["target_url"]}
        for row in navigation_inputs
    }
    resource = {
        "terminal_inventory_complete": True,
        "selector_elapsed_ms": 500,
        "stage_elapsed_seconds": 60,
        "selector_usage": [
            {"operation_id": op, "status": "reported", "input_tokens": 10, "output_tokens": 1} for op in operation_ids
        ],
        "acquisition_physical_http_calls": 53,
        "acquisition_engine_calls": {"wikipedia": 2, "arxiv": 2, "github": 1, "openalex": 1, "brave": 0},
        "capture_owned_http_calls": 245,
        "capture_scrape_calls": 244,
        "capture_health_calls": 1,
        "capture_internal_fanout": None,
        "answerer_calls": 18,
        "grader_submissions": 112,
        "max_grader_request_bytes": 383_000,
        "max_grader_response_bytes": 1_900_000,
        "atomic_fallback_verified": True,
        "w0_control_source_sha256": protocol["primary_control"]["rerank_source_sha256"],
        "w0_source_revision": protocol["primary_control"]["source_revision"],
        "w0_model": protocol["primary_control"]["model"],
        "w0_provider_configured": protocol["primary_control"]["provider_configured"],
        "w0_ranking_strategy": protocol["primary_control"]["ranking_strategy"],
        "w0_parser_parity_verified": True,
        "candidate_source_sha256": protocol["candidate_source_sha256"],
        "selector_concurrency": protocol["provider_schedule"]["concurrency"],
        "selector_retries": protocol["provider_schedule"]["retries"],
        "acquisition_timeout_seconds": protocol["acquisition"]["timeout_seconds"],
        "acquisition_retries": protocol["acquisition"]["retries"],
        "acquisition_pacing_seconds": protocol["acquisition"]["pacing_seconds"],
        "arxiv_pacing_seconds": protocol["acquisition"]["arxiv_physical_pacing_seconds"],
        "capture_timeout_seconds": protocol["capture"]["timeout_seconds"],
        "capture_retries": protocol["capture"]["retries"],
        "capture_max_response_bytes": 1_900_000,
        "capture_max_context_characters": protocol["capture"]["context_unicode_characters"],
        "capture_unique_public_urls": 244,
        "capture_attempts": 244,
        "answerer_timeout_seconds": 30,
        "answerer_retries": 0,
        "answerer_output_tokens_requested": protocol["consumer"]["answerer_output_tokens_requested"],
        "answerer_max_request_bytes": 383_000,
        "answerer_max_response_bytes": 1_900_000,
        "answerer_max_words": protocol["consumer"]["answer_words_maximum"],
        "grader_concurrency": protocol["consumer"]["assessor_concurrency_maximum"],
        "max_sources_per_task": protocol["consumer"]["maximum_successful_sources"],
    }
    inputs = {
        "protocol": protocol,
        "task_inputs": task_inputs,
        "navigation_inputs": navigation_inputs,
        "prepared_assessments": SimpleNamespace(
            task_catalogs=task_catalogs,
            source_stage_uuid=source_stage_uuid,
            packet_stage_uuid=packet_stage_uuid,
        ),
        "reference_closure": reference,
        "answer_closure": answer,
        "selector_evidence": selector,
        "stability_orders": stability,
        "navigation_observations": navigation_observations,
        "resource_evidence": resource,
        "input_manifest_sha256": "b" * 64,
    }
    return inputs


def _supported_preassessment_submissions(prepared):
    submissions = []
    for packet in prepared.preassessment_packets:
        envelope = json.loads(packet["bytes"])
        model_input = envelope["model_input"]
        if packet["role"] == "card":
            output = _card_output(packet)
        else:
            sources = []
            for source in model_input["sources"]:
                evidence_ids = [row["evidence_id"] for row in source["passages"]]
                sources.append(
                    {
                        "source_id": source["source_id"],
                        "facets": [
                            {
                                "facet_id": check["id"],
                                "disposition": "evidence_available",
                                "scope_match": "match",
                                "entailment": "supports",
                                "applicability": "applicable",
                                "not_assessable_reason": "",
                                "evidence_ids": evidence_ids,
                                "reason": "Synthetic source fixture.",
                                "limitations": [],
                            }
                            for check in model_input["facets"]
                        ],
                    }
                )
            output = {
                "task_id": packet["task_id"],
                "assessor_id": packet["assessor_id"],
                "chunk_id": model_input["chunk_id"],
                "source_assessments": sources,
            }
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


def _positive_answer_submissions(prepared_answers, prepared):
    submissions = []
    for packet in prepared_answers.packets:
        model_input = json.loads(packet["bytes"])
        task_id = packet["task_id"]
        catalog = prepared.task_catalogs[task_id]
        source_ids = list(prepared.task_metadata[task_id]["source_ids"])
        facets = []
        for frozen in model_input["answer"]:
            claims = frozen["claims"]
            evidence_ids = sorted({item for claim in claims for item in claim["evidence_ids"]})
            evidence_by_source = {
                source_id: [
                    evidence_id
                    for evidence_id in evidence_ids
                    if catalog[
                        next(code for code, row in catalog.items() if row["original_evidence_id"] == evidence_id)
                    ]["source_id"]
                    == source_id
                ]
                for source_id in source_ids
            }
            dispositions = [
                {
                    "source_id": source_id,
                    "disposition": "used_with_support" if evidence_by_source[source_id] else "delivered_unused",
                    "not_assessable_reason": "",
                    "evidence_ids": evidence_by_source[source_id],
                    "reason": "Synthetic supported answer fixture.",
                }
                for source_id in source_ids
            ]
            facets.append(
                {
                    "facet_id": frozen["facet_id"],
                    "status": "complete",
                    "not_assessable_reason": "",
                    "conclusion": frozen["conclusion"],
                    "claims": claims,
                    "unsupported_claim_indices": [],
                    "unsupported_material_claims": [],
                    "missed_qualifications": [],
                    "evidence_found_but_unused": [],
                    "evidence_dispositions": dispositions,
                    "scope_match": "match",
                    "entailment": "supports",
                    "applicability": "applicable",
                    "reason": "Synthetic supported answer fixture.",
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
                    "arms": [{"arm_id": model_input["blind_arm_id"], "facet_assessments": facets, "limitations": []}],
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


def test_full_frozen_gate_calculation_is_deterministic_and_separate_by_assessor():
    inputs = _fixture()
    first = gates.calculate_coverage_gate_report(**inputs)
    second = gates.calculate_coverage_gate_report(**inputs)
    assert first.evaluation.status == "pass"
    assert all(first.evaluation.gates.values())
    assert first.evaluation.metrics_sha256 == second.evaluation.metrics_sha256
    assert first.receipt_bytes == second.receipt_bytes
    assert first.metrics["ranking"]["A"]["mean_delta"] == first.metrics["ranking"]["B"]["mean_delta"]
    assert first.metrics["task_use_R1"]["additional_completed"] == 3
    assert first.metrics["task_use_R2"]["additional_completed"] == 3
    assert first.metrics["resources"]["capture_internal_fanout_status"] == "unknown"
    assert first.evaluation.scope == "mixed-workload-only"


def test_unknown_usage_and_incomplete_inventory_are_inconclusive():
    for field, value in (
        ("terminal_inventory_complete", False),
        ("selector_usage", [{"operation_id": "unknown", "status": "unavailable"}]),
    ):
        inputs = _fixture()
        inputs["resource_evidence"][field] = value
        result = gates.calculate_coverage_gates(**inputs)
        assert result.status == "inconclusive"
        assert result.gates["resource_and_terminal_guards"] is None


def test_brave_calls_require_explicit_zero_evidence():
    missing = _fixture()
    del missing["resource_evidence"]["acquisition_engine_calls"]["brave"]
    result = gates.calculate_coverage_gates(**missing)
    assert result.status == "inconclusive"
    assert result.gates["resource_and_terminal_guards"] is None

    observed = _fixture()
    observed["resource_evidence"]["acquisition_engine_calls"]["brave"] = 1
    result = gates.calculate_coverage_gates(**observed)
    assert result.status == "fail"
    assert result.gates["resource_and_terminal_guards"] is False


def test_navigation_miss_is_inconclusive_and_no_url_is_replaced():
    inputs = _fixture()
    target = inputs["navigation_inputs"][0]
    inputs["navigation_observations"][target["task_id"]]["acquired_urls"] = []
    result = gates.calculate_coverage_gates(**inputs)
    assert result.status == "inconclusive"
    assert result.gates["exact_navigation_top1"] is None


def test_candidate_answer_hazard_fails_only_affected_independent_assessor_gate():
    inputs = _fixture()
    output = inputs["answer_closure"].outputs()
    facet = output["answers"]["D-R01"]["candidate"]["R1"]["tasks"][0]["arms"][0]["facet_assessments"][0]
    facet["unsupported_material_claims"] = ["unsupported synthetic assertion"]
    mutated = _closure(
        "answer",
        inputs["reference_closure"].stage_uuid,
        {"answers": output["answers"], "preassessment_receipt_sha256": inputs["reference_closure"].receipt_sha256},
    )
    inputs["answer_closure"] = mutated
    result = gates.calculate_coverage_gates(**inputs)
    assert result.status == "fail"
    assert result.gates["answer_R1_task_use"] is False
    assert result.gates["answer_R2_task_use"] is True


def test_invalid_grade_or_control_binding_never_counts_as_pass():
    inputs = _fixture()
    inputs["resource_evidence"]["w0_source_revision"] = "f" * 40
    result = gates.calculate_coverage_gates(**inputs)
    assert result.status == "fail"
    assert result.gates["resource_and_terminal_guards"] is False


def test_changed_quality_or_task_use_thresholds_are_rejected():
    inputs = _fixture()
    inputs["protocol"]["quality"]["mean_ndcg10_gain_minimum"] = 0
    try:
        gates.calculate_coverage_gates(**inputs)
    except gates.GateCalculationError as exc:
        assert str(exc) == "frozen-quality-task-use-contract-mismatch"
    else:
        raise AssertionError("modified thresholds must not be treated as the frozen gate")


def test_changed_resource_or_schedule_contract_is_rejected():
    inputs = _fixture()
    inputs["protocol"]["acquisition"]["max_physical_http_calls"] += 1
    try:
        gates.calculate_coverage_gates(**inputs)
    except gates.GateCalculationError as exc:
        assert str(exc) == "frozen-execution-contract-mismatch"
    else:
        raise AssertionError("modified resource limits must not be treated as frozen")


def test_ranking_bootstrap_matches_frozen_exp077_choices_recipe():
    values = [-0.03, 0.0, 0.01, 0.02, 0.04, 0.01, -0.01, 0.03]
    rng = random.Random(7701)
    samples = sorted(statistics.mean(rng.choices(values, k=8)) for _ in range(10_000))
    assert gates._bootstrap(values, 10_000, 7701) == (statistics.mean(values), [samples[249], samples[9749]])


def _use_real_prepared_stage(inputs, prepared, reference_closure):
    inputs["reference_closure"] = reference_closure
    inputs["prepared_assessments"] = prepared
    inputs["task_inputs"] = [
        {
            "task_id": task_id,
            "card_ids": list(metadata["card_ids"]),
            "facet_ids": [facet["id"] for facet in metadata["facets"]],
            "band": "research_le_40" if index <= 4 else "research_41_80",
            "intent": "evidence_seeking" if index % 2 else "source_discovery",
        }
        for index, (task_id, metadata) in enumerate(prepared.task_metadata.items(), start=1)
    ]
    orders = {
        task["task_id"]: {"w0": task["card_ids"], "candidate": list(reversed(task["card_ids"]))}
        for task in inputs["task_inputs"]
    }
    inputs["selector_evidence"] = SelectorEvidence(
        stage_uuid=prepared.source_stage_uuid,
        terminal_inventory_sha256="a" * 64,
        operation_rows=tuple(
            {"operation_id": op, "state": "complete-success"}
            for op in [call.operation_id for call in selector_inventory((None,) * 5)]
        ),
        research_orders=orders,
        navigation_rank_one={f"N{index:02d}": True for index in range(1, 6)},
        reference_grade_receipt_sha256=reference_closure.receipt_sha256,
        usage_status="known",
    )
    inputs["stability_orders"] = {
        task["task_id"]: {
            arm: {
                "base": orders[task["task_id"]][arm],
                "repeat": orders[task["task_id"]][arm],
                "rotate": orders[task["task_id"]][arm],
            }
            for arm in ("w0", "candidate")
        }
        for task in inputs["task_inputs"]
    }
    return inputs


def test_real_grade_closure_outputs_join_without_manual_closure_construction():
    prepared, reference_closure, prepared_answers = _closed_prepared()
    answer_closure = closure.close_answer_assessments(
        prepared,
        prepared_answers,
        reference_closure,
        _answer_outputs(prepared_answers, prepared),
        **_answer_pins(prepared, prepared_answers),
    )
    inputs = _fixture()
    inputs["answer_closure"] = answer_closure
    inputs = _use_real_prepared_stage(inputs, prepared, reference_closure)
    result = gates.calculate_coverage_gate_report(**inputs)
    assert result.metrics["ranking"]["A"]["cases"][0]["task_id"] == "D-R01"
    assert result.metrics["ranking"]["B"]["cases"][0]["task_id"] == "D-R01"
    assert result.metrics["ranking"]["A"]["cases"][0]["w0"]["useful_count"] == 6
    assert result.evaluation.gates["complete_eligible_membership"] is None
    assert result.evaluation.gates["answer_R1_task_use"] is None
    assert prepared.source_stage_uuid != prepared.packet_stage_uuid
    assert result.metrics["source_stage_uuid"] == prepared.source_stage_uuid
    assert result.metrics["packet_stage_uuid"] == prepared.packet_stage_uuid


def test_selector_source_stage_and_reference_receipt_must_match():
    inputs = _fixture()
    bad_stage = dict(inputs)
    bad_stage["selector_evidence"] = replace(
        inputs["selector_evidence"], stage_uuid=inputs["reference_closure"].stage_uuid
    )
    try:
        gates.calculate_coverage_gates(**bad_stage)
    except gates.GateCalculationError as exc:
        assert str(exc) == "selector-evidence-stage-mismatch"
    else:
        raise AssertionError("packet-stage selector evidence must not be accepted as source-stage evidence")

    bad_receipt = dict(inputs)
    bad_receipt["selector_evidence"] = replace(inputs["selector_evidence"], reference_grade_receipt_sha256="c" * 64)
    try:
        gates.calculate_coverage_gates(**bad_receipt)
    except gates.GateCalculationError as exc:
        assert str(exc) == "selector-evidence-stage-mismatch"
    else:
        raise AssertionError("selector evidence for another reference closure must be rejected")


def test_real_restored_original_evidence_ids_support_positive_completion():
    prepared, captures = _packet_fixture()
    reference_closure = closure.close_preassessment(
        prepared,
        _supported_preassessment_submissions(prepared),
        **_pre_pins(prepared),
    )
    answer_inputs, answer_outputs, _unused = _answer_stage(prepared, captures)
    prepared_answers = packet_prep.prepare_answer_assessment_packets(
        prepared=prepared,
        answer_inputs=answer_inputs,
        answer_outputs=answer_outputs,
        source_outputs=reference_closure.source_outputs_for_answer(),
    )
    answer_closure = closure.close_answer_assessments(
        prepared,
        prepared_answers,
        reference_closure,
        _positive_answer_submissions(prepared_answers, prepared),
        **_answer_pins(prepared, prepared_answers),
    )
    inputs = _use_real_prepared_stage(_fixture(), prepared, reference_closure)
    inputs["answer_closure"] = answer_closure
    report = gates.calculate_coverage_gate_report(**inputs)
    for assessor in ("R1", "R2"):
        task_use = report.metrics[f"task_use_{assessor}"]
        assert task_use["candidate_completed"] == 8
        assert task_use["w0_completed"] == 8
        assert report.evaluation.gates[f"answer_{assessor}_task_use"] is False


def test_wrong_pool_band_is_inconclusive_and_no_trimming_is_accepted():
    inputs = _fixture()
    inputs["task_inputs"][0]["card_ids"] = inputs["task_inputs"][0]["card_ids"][:10]
    result = gates.calculate_coverage_gates(**inputs)
    assert result.status == "inconclusive"
    assert result.gates["candidate_complete_pool_permutation"] is False


def test_inconclusive_packet_status_precedes_known_failed_gate_but_preserves_both():
    inputs = _fixture()
    inputs["selector_evidence"].research_orders["D-R01"]["candidate"] = ["missing"]
    inputs["navigation_observations"]["N01"]["acquired_urls"] = []
    result = gates.calculate_coverage_gates(**inputs)
    assert result.status == "inconclusive"
    assert result.gates["candidate_complete_pool_permutation"] is False
    assert result.gates["exact_navigation_top1"] is None
