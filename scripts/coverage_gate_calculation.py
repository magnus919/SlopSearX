"""Pure deterministic calculator for the frozen coverage-first study gates.

Consumes already-closed fresh grade objects and recorded selector, navigation,
and resource observations. It does not validate their receipts, infer missing
measurements, or create study authority. Missing required observations are
inconclusive.
"""

from __future__ import annotations

import hashlib
import json
import math
import random
import statistics
from collections.abc import Mapping, Sequence
from dataclasses import dataclass

from scripts.coverage_grade_closure import GradeClosure
from scripts.coverage_stage_orchestration import GateEvaluation, SelectorEvidence
from scripts.coverage_study_core import selector_inventory

SCHEMA = "coverage-first-gate-calculation/1"
FROZEN_QUALITY_TASK_USE_SHA256 = "b6353b480fd88543da530ef6b03129d26743f8f273e8a80d543320167399f7dd"
FROZEN_EXECUTION_CONTRACT_SHA256 = "44cee41bc453581d4d0e00e9d3081332dc0467d64a73b9fd70a1ef3515548858"
GATE_NAMES = frozenset(
    {
        "complete_eligible_membership",
        "candidate_complete_pool_permutation",
        "reference_A_ranking",
        "reference_B_ranking",
        "answer_R1_task_use",
        "answer_R2_task_use",
        "candidate_repeat_rotation_overlap",
        "exact_navigation_top1",
        "resource_and_terminal_guards",
    }
)


class GateCalculationError(ValueError):
    """The calculator was given unclosed grades or an invalid protocol."""


@dataclass(frozen=True)
class Endpoint:
    passed: bool | None
    metrics: Mapping[str, object]


@dataclass(frozen=True)
class CoverageGateReport:
    evaluation: GateEvaluation
    metrics: Mapping[str, object]
    receipt_bytes: bytes


def _canonical(value: object) -> bytes:
    try:
        return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise GateCalculationError("gate-json-invalid") from exc


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _finite(value: object) -> bool:
    return type(value) in {int, float} and math.isfinite(value)


def _closed_outputs(reference: GradeClosure, answers: GradeClosure) -> tuple[dict, dict]:
    if type(reference) is not GradeClosure or reference.phase != "preassessment":
        raise GateCalculationError("closed-preassessment-required")
    if type(answers) is not GradeClosure or answers.phase != "answer":
        raise GateCalculationError("closed-answer-assessment-required")
    if reference.stage_uuid != answers.stage_uuid:
        raise GateCalculationError("grade-closure-stage-mismatch")
    try:
        ref, answer = reference.outputs(), answers.outputs()
    except Exception as exc:
        raise GateCalculationError("grade-closure-output-invalid") from exc
    if type(ref) is not dict or type(answer) is not dict:
        raise GateCalculationError("grade-closure-output-shape")
    if answer.get("preassessment_receipt_sha256") != reference.receipt_sha256:
        raise GateCalculationError("grade-closure-chain-mismatch")
    return ref, answer


def _dcg(order: Sequence[str], grades: Mapping[str, int]) -> float:
    return sum((2 ** grades[item] - 1) / math.log2(rank + 2) for rank, item in enumerate(order[:10]))


def _ranking_case(
    task: Mapping[str, object], orders: Mapping[str, object], records: object, *, useful_min: int, case_min: float
) -> Endpoint:
    task_id = task["task_id"]
    ids = task.get("card_ids")
    if (
        type(ids) not in (list, tuple)
        or not ids
        or any(type(item) is not str for item in ids)
        or len(ids) != len(set(ids))
    ):
        return Endpoint(None, {"task_id": task_id, "reason": "pool-membership-invalid"})
    if type(orders) is not dict or set(orders) != {"w0", "candidate"}:
        return Endpoint(None, {"task_id": task_id, "reason": "base-order-missing"})
    for order in orders.values():
        if (
            type(order) not in (list, tuple)
            or any(type(item) is not str for item in order)
            or len(order) != len(ids)
            or set(order) != set(ids)
        ):
            return Endpoint(None, {"task_id": task_id, "reason": "complete-order-unavailable"})
    facet_ids = task.get("facet_ids")
    if type(facet_ids) not in (list, tuple) or len(facet_ids) != 2 or len(set(facet_ids)) != 2:
        return Endpoint(None, {"task_id": task_id, "reason": "caller-facets-invalid"})
    if type(records) not in (list, tuple):
        return Endpoint(None, {"task_id": task_id, "reason": "reference-records-unavailable"})
    by_id = {}
    for row in records:
        if type(row) is not dict or type(row.get("card_id")) is not str or row["card_id"] in by_id:
            return Endpoint(None, {"task_id": task_id, "reason": "reference-record-invalid"})
        lead, facets = row.get("lead"), row.get("facets")
        if type(lead) is not int or not 0 <= lead <= 3 or type(facets) is not list:
            return Endpoint(None, {"task_id": task_id, "reason": "reference-grade-invalid"})
        if any(type(facet) is not str or facet not in facet_ids for facet in facets) or len(facets) != len(set(facets)):
            return Endpoint(None, {"task_id": task_id, "reason": "reference-facets-invalid"})
        by_id[row["card_id"]] = row
    if set(by_id) != set(ids):
        return Endpoint(None, {"task_id": task_id, "reason": "reference-pool-coverage-incomplete"})
    grades = {item: by_id[item]["lead"] for item in ids}
    ideal = _dcg(sorted(ids, key=lambda item: -grades[item]), grades)

    def view(order):
        useful = {item for item in order[:10] if grades[item] >= useful_min}
        return {
            "ndcg10": _dcg(order, grades) / ideal if ideal else 0.0,
            "useful_count": len(useful),
            "useful_facets": sorted({facet for item in useful for facet in by_id[item]["facets"]}),
            "useful_card_ids": sorted(useful),
        }

    w0, candidate = view(orders["w0"]), view(orders["candidate"])
    delta = float(candidate["ndcg10"]) - float(w0["ndcg10"])
    lost = sorted(set(w0["useful_facets"]) - set(candidate["useful_facets"]))
    passed = delta >= case_min and candidate["useful_count"] >= w0["useful_count"] and not lost
    return Endpoint(
        passed,
        {
            "task_id": task_id,
            "w0": w0,
            "candidate": candidate,
            "delta": delta,
            "lost_facets": lost,
            "per_case_pass": passed,
        },
    )


def _bootstrap(values: Sequence[float], draws: int, seed: int) -> tuple[float, list[float]]:
    rng = random.Random(seed)
    samples = sorted(statistics.mean(rng.choices(list(values), k=len(values))) for _ in range(draws))
    return statistics.mean(values), [samples[249], samples[9749]]


def _ranking_endpoint(assessor, tasks, orders, records_by_task, quality) -> Endpoint:
    if type(records_by_task) is not dict:
        return Endpoint(None, {"assessor": assessor, "reason": "independent-reference-missing"})
    result = []
    for task in tasks:
        task_id = task["task_id"]
        task_records = records_by_task.get(task_id)
        if type(task_records) is not dict or assessor not in task_records:
            return Endpoint(None, {"assessor": assessor, "task_id": task_id, "reason": "independent-reference-missing"})
        case = _ranking_case(
            task,
            orders.get(task_id),
            task_records[assessor],
            useful_min=quality["useful_lead_grade_minimum"],
            case_min=quality["per_case_delta_minimum"],
        )
        if case.passed is None:
            return Endpoint(None, {"assessor": assessor, **case.metrics})
        result.append(case.metrics)
    deltas = [float(row["delta"]) for row in result]
    mean, interval = _bootstrap(deltas, quality["bootstrap_draws"], quality["bootstrap_seed"])
    passed = (
        mean >= quality["mean_ndcg10_gain_minimum"] and interval[0] > 0 and all(row["per_case_pass"] for row in result)
    )
    return Endpoint(
        passed, {"assessor": assessor, "mean_delta": mean, "bootstrap_95": interval, "cases": result, "pass": passed}
    )


def _completion(arm: object, catalog: Mapping[str, Mapping[str, object]]) -> tuple[bool | None, list[object]]:
    if type(arm) is not dict or type(arm.get("facet_assessments")) is not list or not arm["facet_assessments"]:
        return None, [{"kind": "answer-facet-inventory-invalid"}]
    hazards, unknown = [], []
    complete = True
    for facet in arm["facet_assessments"]:
        if type(facet) is not dict:
            return None, [{"kind": "answer-facet-invalid"}]
        for field in ("unsupported_material_claims", "missed_qualifications"):
            items = facet.get(field)
            if type(items) is not list:
                return None, [{"kind": "answer-hazard-field-invalid", "field": field}]
            if items:
                hazards.append({"facet_id": facet.get("facet_id"), "kind": field, "items": items})
        if facet.get("scope_match") == "mismatch" or facet.get("applicability") == "not_applicable":
            hazards.append({"facet_id": facet.get("facet_id"), "kind": "scope-or-applicability"})
        if facet.get("status") == "not_assessable" or any(
            facet.get(field) in {"uncertain", "not_assessable"}
            for field in ("scope_match", "entailment", "applicability")
        ):
            unknown.append(facet.get("facet_id"))
        dispositions, claims, unsupported = (
            facet.get("evidence_dispositions"),
            facet.get("claims"),
            facet.get("unsupported_claim_indices"),
        )
        if type(dispositions) is not list or type(claims) is not list or type(unsupported) is not list:
            return None, [{"kind": "answer-evidence-structure-invalid"}]
        if any(type(index) is not int or index < 0 or index >= len(claims) for index in unsupported):
            return None, [{"kind": "answer-unsupported-claim-index-invalid"}]
        if any(type(row) is not dict for row in dispositions):
            return None, [{"kind": "answer-disposition-invalid"}]
        if any(row.get("disposition") == "not_assessable" for row in dispositions):
            unknown.append(facet.get("facet_id"))
        supported = set()
        for row in dispositions:
            if row.get("disposition") == "used_with_support":
                source_id, evidence_ids = row.get("source_id"), row.get("evidence_ids")
                if type(source_id) is not str or type(evidence_ids) is not list:
                    return None, [{"kind": "answer-supported-evidence-invalid"}]
                supported.update((source_id, evidence_id) for evidence_id in evidence_ids if type(evidence_id) is str)
        anchored = False
        for index, claim in enumerate(claims):
            if type(claim) is not dict or type(claim.get("evidence_ids")) is not list:
                return None, [{"kind": "answer-claim-invalid"}]
            if index in unsupported:
                continue
            for evidence_id in claim["evidence_ids"]:
                source_id = catalog.get(evidence_id, {}).get("source_id")
                anchored |= source_id is not None and (source_id, evidence_id) in supported
        facet_complete = (
            anchored
            and facet.get("status") == "complete"
            and facet.get("scope_match") == "match"
            and facet.get("applicability") == "applicable"
            and facet.get("entailment") in {"supports", "qualifies", "contradicts"}
        )
        complete &= facet_complete
    if unknown:
        return None, [{"kind": "unknown-critical-facet", "facet_ids": sorted(set(unknown))}, *hazards]
    if hazards:
        return False, hazards
    return bool(complete), []


def _task_use(
    answers: object,
    prepared: object,
    task_ids: Sequence[str],
    minimum: int,
    protocol: Mapping[str, object],
    assessors: Sequence[str],
) -> Endpoint:
    if type(answers) is not dict or set(answers) != set(task_ids):
        return Endpoint(None, {"reason": "answer-task-coverage-incomplete"})
    raw_catalogs = getattr(prepared, "task_catalogs", None)
    if type(raw_catalogs) is not dict or set(raw_catalogs) != set(task_ids):
        return Endpoint(None, {"reason": "answer-catalog-unavailable"})
    catalogs = {}
    for task_id in task_ids:
        rows = raw_catalogs[task_id]
        if type(rows) is not dict or not rows:
            return Endpoint(None, {"reason": "answer-catalog-invalid", "task_id": task_id})
        by_original_id = {}
        for code, row in rows.items():
            if (
                type(code) is not str
                or type(row) is not dict
                or row.get("code") != code
                or type(row.get("original_evidence_id")) is not str
                or not row["original_evidence_id"]
                or type(row.get("source_id")) is not str
                or not row["source_id"]
                or row["original_evidence_id"] in by_original_id
            ):
                return Endpoint(None, {"reason": "answer-catalog-invalid", "task_id": task_id})
            by_original_id[row["original_evidence_id"]] = {"source_id": row["source_id"]}
        catalogs[task_id] = by_original_id
    wins = losses = ties = w0_completed = candidate_completed = 0
    unknown, hazards, deltas = [], [], []
    for task_id in task_ids:
        arms = answers[task_id]
        if type(arms) is not dict or set(arms) != {"w0", "candidate"}:
            return Endpoint(None, {"reason": "answer-arm-coverage-incomplete", "task_id": task_id})
        arm_completed = {}
        arm_unknown = False
        for arm_name in ("w0", "candidate"):
            graders = arms[arm_name]
            if type(graders) is not dict or set(graders) != set(assessors):
                return Endpoint(None, {"reason": "answer-assessor-coverage-incomplete", "task_id": task_id})
            states = []
            for assessor in assessors:
                output = graders[assessor]
                task_rows = output.get("tasks") if type(output) is dict else None
                if (
                    type(task_rows) is not list
                    or len(task_rows) != 1
                    or type(task_rows[0]) is not dict
                    or task_rows[0].get("task_id") != task_id
                ):
                    return Endpoint(None, {"reason": "answer-packet-invalid", "task_id": task_id})
                task_arms = task_rows[0].get("arms")
                if type(task_arms) is not list or len(task_arms) != 1:
                    return Endpoint(None, {"reason": "answer-arm-packet-invalid", "task_id": task_id})
                complete, problems = _completion(task_arms[0], catalogs[task_id])
                states.append(complete)
                if arm_name == "candidate":
                    hazards.extend(
                        {"task_id": task_id, "assessor": assessor, **problem}
                        for problem in problems
                        if type(problem) is dict and problem.get("kind") != "unknown-critical-facet"
                    )
            if any(state is None for state in states):
                arm_completed[arm_name] = None
                arm_unknown = True
            else:
                arm_completed[arm_name] = all(state is True for state in states)
        if arm_unknown:
            unknown.append(task_id)
            deltas.append(0)
            continue
        w0, candidate = arm_completed["w0"], arm_completed["candidate"]
        w0_completed += int(w0)
        candidate_completed += int(candidate)
        wins += int(candidate and not w0)
        losses += int(w0 and not candidate)
        ties += int(candidate == w0)
        deltas.append(int(candidate) - int(w0))
    rng = random.Random(protocol["quality"]["bootstrap_seed"])
    samples = sorted(sum(rng.choices(deltas, k=len(deltas))) for _ in range(protocol["quality"]["bootstrap_draws"]))
    ci = [samples[249], samples[9749]]
    if losses > 0 or hazards:
        passed: bool | None = False
    elif unknown:
        passed = None
    else:
        passed = wins - losses >= minimum
    return Endpoint(
        passed,
        {
            "wins": wins,
            "ties": ties,
            "losses": losses,
            "w0_completed": w0_completed,
            "candidate_completed": candidate_completed,
            "additional_completed": wins - losses,
            "additional_completed_bootstrap_95_diagnostic": ci,
            "unknown_tasks": unknown,
            "candidate_material_failures": hazards,
            "pass": passed,
        },
    )


def _stability(task_ids, orders, variants, minimum) -> Endpoint:
    if type(variants) is not dict:
        return Endpoint(None, {"reason": "stability-orders-missing"})
    results = []
    for task_id in task_ids:
        base = orders.get(task_id)
        row = variants.get(task_id)
        if type(base) is not dict or type(row) is not dict or set(row) != {"w0", "candidate"}:
            return Endpoint(None, {"reason": "stability-task-missing", "task_id": task_id})
        pool = base.get("candidate")
        if type(pool) not in (list, tuple):
            return Endpoint(None, {"reason": "stability-base-missing", "task_id": task_id})
        comparisons = []
        for arm in ("w0", "candidate"):
            actual = row[arm]
            if type(actual) is not dict or set(actual) != {"base", "repeat", "rotate"}:
                return Endpoint(None, {"reason": "stability-variant-inventory", "task_id": task_id})
            if list(actual["base"]) != list(base[arm]):
                return Endpoint(None, {"reason": "stability-base-order-mismatch", "task_id": task_id})
            for variant in ("repeat", "rotate"):
                order = actual[variant]
                if type(order) not in (list, tuple) or len(order) != len(pool) or set(order) != set(pool):
                    return Endpoint(None, {"reason": "stability-order-invalid", "task_id": task_id})
                denominator = min(10, len(pool))
                overlap = len(set(actual["base"][:10]) & set(order[:10])) / denominator
                comparisons.append({"arm": arm, "variant": variant, "top10_overlap": overlap})
        candidate_checks = [item for item in comparisons if item["arm"] == "candidate"]
        results.append(
            {
                "task_id": task_id,
                "comparisons": comparisons,
                "pass": all(item["top10_overlap"] >= minimum for item in candidate_checks),
            }
        )
    return Endpoint(all(item["pass"] for item in results), {"cases": results})


def _resources(protocol: Mapping[str, object], observed: object, expected_operation_ids: Sequence[str]) -> Endpoint:
    fields = {
        "terminal_inventory_complete",
        "selector_elapsed_ms",
        "stage_elapsed_seconds",
        "selector_usage",
        "acquisition_physical_http_calls",
        "acquisition_engine_calls",
        "capture_owned_http_calls",
        "capture_scrape_calls",
        "capture_health_calls",
        "capture_internal_fanout",
        "answerer_calls",
        "grader_submissions",
        "max_grader_request_bytes",
        "max_grader_response_bytes",
        "atomic_fallback_verified",
        "w0_control_source_sha256",
        "w0_source_revision",
        "w0_model",
        "w0_provider_configured",
        "w0_ranking_strategy",
        "w0_parser_parity_verified",
        "candidate_source_sha256",
        "selector_concurrency",
        "selector_retries",
        "acquisition_timeout_seconds",
        "acquisition_retries",
        "acquisition_pacing_seconds",
        "arxiv_pacing_seconds",
        "capture_timeout_seconds",
        "capture_retries",
        "capture_max_response_bytes",
        "capture_max_context_characters",
        "capture_unique_public_urls",
        "capture_attempts",
        "answerer_timeout_seconds",
        "answerer_retries",
        "answerer_output_tokens_requested",
        "answerer_max_request_bytes",
        "answerer_max_response_bytes",
        "answerer_max_words",
        "grader_concurrency",
        "max_sources_per_task",
    }
    if type(observed) is not dict:
        return Endpoint(None, {"reason": "resource-evidence-missing"})
    missing = sorted(fields - set(observed))
    if missing:
        return Endpoint(None, {"reason": "resource-evidence-incomplete", "missing": missing})
    if observed["terminal_inventory_complete"] is not True:
        return Endpoint(None, {"reason": "terminal-inventory-incomplete"})
    if not _finite(observed["selector_elapsed_ms"]) or not _finite(observed["stage_elapsed_seconds"]):
        return Endpoint(None, {"reason": "timing-evidence-invalid"})
    count_fields = fields - {
        "terminal_inventory_complete",
        "selector_elapsed_ms",
        "stage_elapsed_seconds",
        "selector_usage",
        "acquisition_engine_calls",
        "capture_internal_fanout",
        "atomic_fallback_verified",
        "w0_control_source_sha256",
        "w0_source_revision",
        "w0_model",
        "w0_provider_configured",
        "w0_ranking_strategy",
        "w0_parser_parity_verified",
        "candidate_source_sha256",
        "selector_concurrency",
        "selector_retries",
        "acquisition_timeout_seconds",
        "acquisition_retries",
        "acquisition_pacing_seconds",
        "arxiv_pacing_seconds",
        "capture_timeout_seconds",
        "capture_retries",
        "capture_max_response_bytes",
        "capture_max_context_characters",
        "answerer_timeout_seconds",
        "answerer_retries",
        "answerer_output_tokens_requested",
        "answerer_max_request_bytes",
        "answerer_max_response_bytes",
        "answerer_max_words",
        "grader_concurrency",
    }
    if any(type(observed[name]) is not int or observed[name] < 0 for name in count_fields):
        return Endpoint(None, {"reason": "resource-count-invalid"})
    if observed["atomic_fallback_verified"] is not True:
        return Endpoint(False, {"reason": "atomic-fallback-not-verified"})
    usage = observed["selector_usage"]
    if type(usage) not in (list, tuple) or not usage:
        return Endpoint(None, {"reason": "selector-usage-unavailable"})
    input_total = output_total = 0
    seen = set()
    observed_operation_ids = []
    for row in usage:
        if type(row) is not dict or type(row.get("operation_id")) is not str or row["operation_id"] in seen:
            return Endpoint(None, {"reason": "selector-usage-row-invalid"})
        seen.add(row["operation_id"])
        observed_operation_ids.append(row["operation_id"])
        if row.get("status") != "reported":
            return Endpoint(None, {"reason": "selector-usage-unknown", "operation_id": row["operation_id"]})
        input_tokens, output_tokens = row.get("input_tokens"), row.get("output_tokens")
        if any(type(x) is not int or x < 0 for x in (input_tokens, output_tokens)):
            return Endpoint(None, {"reason": "selector-token-count-invalid"})
        per_call_input = protocol["provider_schedule"]["call_input_maximum_and_reserve"]
        per_call_output = protocol["provider_schedule"]["call_output_maximum_and_reserve"]
        if input_tokens > per_call_input or output_tokens > per_call_output:
            return Endpoint(False, {"reason": "selector-per-call-cap-exceeded"})
        if (
            input_total + per_call_input > protocol["provider_schedule"]["stage_token_input_maximum"]
            or output_total + per_call_output > protocol["provider_schedule"]["stage_token_output_maximum"]
        ):
            return Endpoint(False, {"reason": "selector-reserve-unavailable"})
        input_total += input_tokens
        output_total += output_tokens
    if observed_operation_ids != list(expected_operation_ids):
        return Endpoint(None, {"reason": "selector-usage-operation-inventory-mismatch"})
    if (
        input_total > protocol["provider_schedule"]["stage_token_input_maximum"]
        or output_total > protocol["provider_schedule"]["stage_token_output_maximum"]
    ):
        return Endpoint(False, {"reason": "selector-stage-token-cap-exceeded"})
    acq, cap, consumer, phase = (
        protocol["acquisition"],
        protocol["capture"],
        protocol["consumer"],
        protocol["phase_limits"],
    )
    engine_calls = observed["acquisition_engine_calls"]
    if type(engine_calls) is not dict:
        return Endpoint(None, {"reason": "acquisition-engine-counts-invalid"})
    expected_engines = set(acq["adapter_calls_maximum"]) | {"brave"}
    if set(engine_calls) != expected_engines:
        return Endpoint(None, {"reason": "acquisition-engine-inventory-incomplete"})
    if any(type(count) is not int or count < 0 for count in engine_calls.values()):
        return Endpoint(None, {"reason": "acquisition-engine-counts-invalid"})
    engine_ok = (
        all(engine_calls[name] <= maximum for name, maximum in acq["adapter_calls_maximum"].items())
        and engine_calls["brave"] == 0
    )
    fanout = observed["capture_internal_fanout"]
    if fanout is not None and (type(fanout) is not int or fanout < 0):
        return Endpoint(None, {"reason": "capture-fanout-invalid"})
    control = protocol["primary_control"]
    if (
        observed["w0_control_source_sha256"] != control["rerank_source_sha256"]
        or observed["w0_source_revision"] != control["source_revision"]
        or observed["w0_model"] != control["model"]
        or observed["w0_provider_configured"] is not control["provider_configured"]
        or observed["w0_ranking_strategy"] != control["ranking_strategy"]
        or observed["w0_parser_parity_verified"] is not True
        or observed["candidate_source_sha256"] != protocol["candidate_source_sha256"]
    ):
        return Endpoint(False, {"reason": "frozen-control-or-candidate-binding-mismatch"})
    checks = {
        "selector_concurrency": observed["selector_concurrency"] == protocol["provider_schedule"]["concurrency"],
        "selector_retries": observed["selector_retries"] == protocol["provider_schedule"]["retries"],
        "acquisition_timeout": observed["acquisition_timeout_seconds"] == acq["timeout_seconds"],
        "acquisition_retries": observed["acquisition_retries"] == acq["retries"],
        "acquisition_pacing": observed["acquisition_pacing_seconds"] == acq["pacing_seconds"],
        "arxiv_pacing": observed["arxiv_pacing_seconds"] == acq["arxiv_physical_pacing_seconds"],
        "selector_time": observed["selector_elapsed_ms"] <= phase["selector_phase_ms"],
        "stage_wall": observed["stage_elapsed_seconds"] <= phase["stage_wall_seconds"],
        "acquisition_calls": observed["acquisition_physical_http_calls"] <= acq["max_physical_http_calls"],
        "acquisition_engine_caps": engine_ok,
        "capture_owned_calls": observed["capture_owned_http_calls"] <= cap["max_owned_http_calls"],
        "capture_scrape_calls": observed["capture_scrape_calls"] <= cap["max_owned_scrape_calls"],
        "capture_health_calls": observed["capture_health_calls"] <= cap["max_health_calls"],
        "capture_timeout": observed["capture_timeout_seconds"] == cap["timeout_seconds"],
        "capture_retries": observed["capture_retries"] == cap["retries"],
        "capture_response_bytes": observed["capture_max_response_bytes"] <= cap["response_bytes"],
        "capture_context_chars": observed["capture_max_context_characters"] <= cap["context_unicode_characters"],
        "capture_one_shot_urls": observed["capture_attempts"] == observed["capture_unique_public_urls"],
        "capture_url_budget": observed["capture_unique_public_urls"] <= cap["max_owned_scrape_calls"],
        "answerer_calls": observed["answerer_calls"] == consumer["answerer_calls_maximum"],
        "answerer_timeout": observed["answerer_timeout_seconds"] == 30,
        "answerer_retries": observed["answerer_retries"] == 0,
        "answerer_output_tokens": observed["answerer_output_tokens_requested"]
        == protocol["consumer"]["answerer_output_tokens_requested"],
        "answerer_request_bytes": observed["answerer_max_request_bytes"] <= consumer["grader_input_bytes_maximum"],
        "answerer_response_bytes": observed["answerer_max_response_bytes"] <= cap["response_bytes"],
        "answerer_word_limit": observed["answerer_max_words"] <= consumer["answer_words_maximum"],
        "grader_concurrency": observed["grader_concurrency"] <= protocol["consumer"]["assessor_concurrency_maximum"],
        "source_success_budget": observed["max_sources_per_task"] <= consumer["maximum_successful_sources"],
        "grader_submissions": observed["grader_submissions"] <= consumer["max_logical_grader_submissions"],
        "grader_input_bytes": observed["max_grader_request_bytes"] <= consumer["grader_input_bytes_maximum"],
        "grader_output_bytes": observed["max_grader_response_bytes"] <= consumer["grader_output_bytes_maximum"],
    }
    return Endpoint(
        all(checks.values()),
        {
            "limits": checks,
            "selector_input_tokens": input_total,
            "selector_output_tokens": output_total,
            "capture_internal_fanout": fanout,
            "capture_internal_fanout_status": "unknown" if fanout is None else "reported",
        },
    )


def calculate_coverage_gate_report(
    *,
    protocol: Mapping[str, object],
    task_inputs: Sequence[Mapping[str, object]],
    navigation_inputs: Sequence[Mapping[str, object]],
    prepared_assessments: object,
    reference_closure: GradeClosure,
    answer_closure: GradeClosure,
    selector_evidence: SelectorEvidence,
    stability_orders: object,
    navigation_observations: object,
    resource_evidence: object,
    input_manifest_sha256: str,
) -> CoverageGateReport:
    """Calculate frozen mixed-workload gates from closed grades and observations.

    A task row requires task_id, complete card_ids, band and intent. A
    navigation row requires task_id and exact target_url. Stability rows map
    each of the four frozen task IDs to W0/candidate base/repeat/rotate full
    orders. Resource evidence keys are exported as RESOURCE_EVIDENCE_FIELDS.
    """
    if type(protocol) is not dict or protocol.get("schema") not in {
        "coverage-first-study-protocol/1",
        "coverage-first-study-protocol/2",
    }:
        raise GateCalculationError("frozen-protocol-required")
    if protocol.get("status") not in {"draft_not_registered_not_admitted", "registered"}:
        raise GateCalculationError("protocol-status-invalid")
    if (
        type(input_manifest_sha256) is not str
        or len(input_manifest_sha256) != 64
        or any(char not in "0123456789abcdef" for char in input_manifest_sha256)
    ):
        raise GateCalculationError("gate-input-manifest-pin-invalid")
    quality, task_use = protocol.get("quality"), protocol.get("task_use")
    if type(quality) is not dict or type(task_use) is not dict:
        raise GateCalculationError("frozen-gate-contract-missing")
    if _sha(_canonical({"quality": quality, "task_use": task_use})) != FROZEN_QUALITY_TASK_USE_SHA256:
        raise GateCalculationError("frozen-quality-task-use-contract-mismatch")
    execution_contract_keys = (
        "quality",
        "task_use",
        "provider_schedule",
        "acquisition",
        "capture",
        "consumer",
        "phase_limits",
        "variants",
        "primary_control",
        "candidate_source_sha256",
    )
    try:
        execution_contract = {key: protocol[key] for key in execution_contract_keys}
    except KeyError as exc:
        raise GateCalculationError("frozen-execution-contract-missing") from exc
    if _sha(_canonical(execution_contract)) != FROZEN_EXECUTION_CONTRACT_SHA256:
        raise GateCalculationError("frozen-execution-contract-mismatch")
    refs, answers = _closed_outputs(reference_closure, answer_closure)
    source_stage_uuid = getattr(prepared_assessments, "source_stage_uuid", None)
    packet_stage_uuid = getattr(prepared_assessments, "packet_stage_uuid", None)
    if (
        type(source_stage_uuid) is not str
        or type(packet_stage_uuid) is not str
        or packet_stage_uuid != reference_closure.stage_uuid
    ):
        raise GateCalculationError("prepared-stage-identity-mismatch")
    if (
        type(selector_evidence) is not SelectorEvidence
        or selector_evidence.stage_uuid != source_stage_uuid
        or selector_evidence.reference_grade_receipt_sha256 != reference_closure.receipt_sha256
    ):
        raise GateCalculationError("selector-evidence-stage-mismatch")
    if type(task_inputs) not in (list, tuple) or len(task_inputs) != 8:
        raise GateCalculationError("eight-task-inputs-required")
    task_ids = [row.get("task_id") if type(row) is dict else None for row in task_inputs]
    if any(type(value) is not str for value in task_ids) or len(set(task_ids)) != 8:
        raise GateCalculationError("task-input-identities-invalid")
    if type(navigation_inputs) not in (list, tuple) or len(navigation_inputs) != 5:
        raise GateCalculationError("five-navigation-inputs-required")
    navigation_ids = [row.get("task_id") if type(row) is dict else None for row in navigation_inputs]
    if any(type(value) is not str for value in navigation_ids) or len(set(navigation_ids)) != 5:
        raise GateCalculationError("navigation-input-identities-invalid")

    orders = selector_evidence.research_orders if type(selector_evidence.research_orders) is dict else {}
    membership = []
    for row in task_inputs:
        ids, band = row.get("card_ids"), row.get("band")
        if type(ids) not in (list, tuple) or not ids or len(ids) != len(set(ids)):
            membership.append(None)
            continue
        count = len(ids)
        valid = (
            1 <= count <= 40 if band == "research_le_40" else 41 <= count <= 80 if band == "research_41_80" else False
        )
        membership.append(True if valid else None)
    bands = [row.get("band") for row in task_inputs]
    membership_gate = (
        True
        if bands.count("research_le_40") == 4
        and bands.count("research_41_80") == 4
        and all(item is True for item in membership)
        else None
    )
    complete_order: bool | None = True if set(orders) == set(task_ids) else None
    if complete_order is True:
        for row in task_inputs:
            task_orders = orders.get(row["task_id"])
            ids = row["card_ids"]
            if type(task_orders) is not dict or set(task_orders) != {"w0", "candidate"}:
                complete_order = None
                break
            if any(
                type(order) not in (list, tuple) or len(order) != len(ids) or set(order) != set(ids)
                for order in task_orders.values()
            ):
                complete_order = False
                break
    if membership_gate is None:
        complete_order = None

    cards = refs.get("cards") if type(refs.get("cards")) is dict else {}
    ranking = {
        assessor: _ranking_endpoint(assessor, task_inputs, orders, cards, quality)
        for assessor in quality["independent_references"]
    }
    answer_scores = answers.get("answers")
    answer_aggregate = _task_use(
        answer_scores,
        prepared_assessments,
        task_ids,
        task_use["additional_completed_tasks_each_minimum"],
        protocol,
        task_use["independent_assessors"],
    )
    by_assessor = {}
    if type(answer_scores) is dict:
        for assessor in task_use["independent_assessors"]:
            filtered = {}
            for task_id, arms in answer_scores.items():
                if type(arms) is dict:
                    filtered[task_id] = {
                        arm: ({assessor: graders[assessor]} if type(graders) is dict and assessor in graders else {})
                        for arm, graders in arms.items()
                    }
            by_assessor[assessor] = _task_use(
                filtered,
                prepared_assessments,
                task_ids,
                task_use["additional_completed_tasks_each_minimum"],
                protocol,
                (assessor,),
            )
    else:
        by_assessor = {
            assessor: Endpoint(None, {"reason": "answer-output-unavailable"})
            for assessor in task_use["independent_assessors"]
        }

    stability_ids = [task_ids[index - 1] for index in protocol["variants"]["task_indices"]]
    stability = _stability(stability_ids, orders, stability_orders, quality["candidate_top10_overlap_minimum"])
    nav_obs = navigation_observations if type(navigation_observations) is dict else {}
    nav_rank_one = selector_evidence.navigation_rank_one if type(selector_evidence.navigation_rank_one) is dict else {}
    nav_values = []
    for row in navigation_inputs:
        obs = nav_obs.get(row["task_id"])
        target = row.get("target_url")
        if (
            type(obs) is not dict
            or type(target) is not str
            or target not in obs.get("acquired_urls", [])
            or type(obs.get("candidate_top1_url")) is not str
            or type(nav_rank_one.get(row["task_id"])) is not bool
        ):
            nav_values.append(None)
        else:
            nav_values.append(obs["candidate_top1_url"] == target and nav_rank_one[row["task_id"]])
    nav_gate = None if any(value is None for value in nav_values) else all(nav_values)
    planned_operations = selector_inventory((None,) * len(navigation_inputs))
    expected_operation_ids = [operation.operation_id for operation in planned_operations]
    operation_rows = selector_evidence.operation_rows
    if any(type(row) is not dict for row in operation_rows):
        operation_inventory_ok = False
        observed_operation_ids = []
    else:
        observed_operation_ids = [row.get("operation_id") for row in operation_rows]
    if observed_operation_ids != expected_operation_ids:
        operation_inventory_ok = False
    else:
        operation_inventory_ok = all(
            row.get("state") in {"complete-success", "complete-invalid-response"}
            for row in selector_evidence.operation_rows
        )
    resource = _resources(protocol, resource_evidence, expected_operation_ids)
    if selector_evidence.usage_status != "known" or not operation_inventory_ok:
        resource = Endpoint(None, {"reason": "selector-terminal-inventory-incomplete"})

    gates = {
        "complete_eligible_membership": membership_gate,
        "candidate_complete_pool_permutation": complete_order,
        "reference_A_ranking": ranking.get("A", Endpoint(None, {})).passed,
        "reference_B_ranking": ranking.get("B", Endpoint(None, {})).passed,
        "answer_R1_task_use": by_assessor.get("R1", Endpoint(None, {})).passed,
        "answer_R2_task_use": by_assessor.get("R2", Endpoint(None, {})).passed,
        "candidate_repeat_rotation_overlap": stability.passed,
        "exact_navigation_top1": nav_gate,
        "resource_and_terminal_guards": resource.passed,
    }
    status = (
        "inconclusive"
        if any(value is None for value in gates.values())
        else "fail"
        if any(value is False for value in gates.values())
        else "pass"
    )
    metrics = {
        "schema": SCHEMA,
        "scope": "mixed-workload-only",
        "source_stage_uuid": source_stage_uuid,
        "packet_stage_uuid": packet_stage_uuid,
        "gates": gates,
        "pool_sizes": {
            row["task_id"]: len(row["card_ids"]) if type(row.get("card_ids")) in (list, tuple) else None
            for row in task_inputs
        },
        "bands_and_intents": {
            band: {
                "task_count": sum(row.get("band") == band for row in task_inputs),
                "intents": {
                    intent: sum(row.get("band") == band and row.get("intent") == intent for row in task_inputs)
                    for intent in ("evidence_seeking", "source_discovery")
                },
            }
            for band in ("research_le_40", "research_41_80")
        },
        "ranking": {key: value.metrics for key, value in ranking.items()},
        "task_use_R1": by_assessor.get("R1", Endpoint(None, {})).metrics,
        "task_use_R2": by_assessor.get("R2", Endpoint(None, {})).metrics,
        "task_use_aggregate_diagnostic": answer_aggregate.metrics,
        "stability": stability.metrics,
        "navigation": {row["task_id"]: nav_values[index] for index, row in enumerate(navigation_inputs)},
        "resources": resource.metrics,
        "status": status,
    }
    metrics_sha = _sha(_canonical(metrics))
    threshold_sha = _sha(_canonical({"quality": quality, "task_use": task_use}))
    receipt = {
        "schema": "coverage-first-gate-calculation-receipt/1",
        "source_stage_uuid": source_stage_uuid,
        "packet_stage_uuid": packet_stage_uuid,
        "input_manifest_sha256": input_manifest_sha256,
        "threshold_sha256": threshold_sha,
        "metrics_sha256": metrics_sha,
        "status": status,
        "gates": gates,
        "quality_credit": False,
        "admission_created": False,
    }
    receipt_bytes = _canonical(receipt)
    evaluation = GateEvaluation(
        status=status,
        scope="mixed-workload-only",
        input_manifest_sha256=input_manifest_sha256,
        threshold_sha256=threshold_sha,
        gates=gates,
        band_diagnostics=metrics["bands_and_intents"],
        metrics_sha256=metrics_sha,
        calculation_receipt_sha256=_sha(receipt_bytes),
    )
    return CoverageGateReport(evaluation=evaluation, metrics=metrics, receipt_bytes=receipt_bytes)


def calculate_coverage_gates(**kwargs) -> GateEvaluation:
    """Return the coordinator-compatible evaluation from a full calculation report."""
    return calculate_coverage_gate_report(**kwargs).evaluation


RESOURCE_EVIDENCE_FIELDS = frozenset(
    {
        "terminal_inventory_complete",
        "selector_elapsed_ms",
        "stage_elapsed_seconds",
        "selector_usage",
        "acquisition_physical_http_calls",
        "acquisition_engine_calls",
        "capture_owned_http_calls",
        "capture_scrape_calls",
        "capture_health_calls",
        "capture_internal_fanout",
        "answerer_calls",
        "grader_submissions",
        "max_grader_request_bytes",
        "max_grader_response_bytes",
        "atomic_fallback_verified",
        "w0_control_source_sha256",
        "w0_source_revision",
        "w0_model",
        "w0_provider_configured",
        "w0_ranking_strategy",
        "w0_parser_parity_verified",
        "candidate_source_sha256",
        "selector_concurrency",
        "selector_retries",
        "acquisition_timeout_seconds",
        "acquisition_retries",
        "acquisition_pacing_seconds",
        "arxiv_pacing_seconds",
        "capture_timeout_seconds",
        "capture_retries",
        "capture_max_response_bytes",
        "capture_max_context_characters",
        "capture_unique_public_urls",
        "capture_attempts",
        "answerer_timeout_seconds",
        "answerer_retries",
        "answerer_output_tokens_requested",
        "answerer_max_request_bytes",
        "answerer_max_response_bytes",
        "answerer_max_words",
        "grader_concurrency",
        "max_sources_per_task",
    }
)


__all__ = [
    "CoverageGateReport",
    "Endpoint",
    "GateCalculationError",
    "RESOURCE_EVIDENCE_FIELDS",
    "calculate_coverage_gate_report",
    "calculate_coverage_gates",
]
