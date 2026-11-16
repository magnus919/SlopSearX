from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path

import pytest

from scripts import coverage_band_companion as companion
from scripts import coverage_gate_calculation as gates
from scripts import coverage_study_acquire as acquire
from tests import test_coverage_stage_finalization as finalization_fixtures
from tests.test_coverage_gate_calculation import _fixture

ROOT = Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study"
SAVED = ROOT / "saved-development-method-materials"
SAVED_WIRE_COMPAT = ROOT / "saved-development-wire-compatibility-method-materials"
SAVED_WIRE_FLOAT = ROOT / "saved-development-wire-float-method-materials"
ORIGINAL = ROOT / "frozen-input-method-materials"


def _sha(raw):
    return hashlib.sha256(raw).hexdigest()


def test_saved_method_preserves_candidate_quality_and_usage_limits():
    old = json.loads((ORIGINAL / "protocol.json").read_bytes())
    new = json.loads((SAVED / "protocol.json").read_bytes())
    for key in (
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
    ):
        assert new[key] == old[key], key
    assert (SAVED / "cohorts.json").read_bytes() == (ORIGINAL / "cohorts.json").read_bytes()
    assert _sha((SAVED / "protocol.json").read_bytes()) == companion.SAVED_PROTOCOL_SHA256
    assert _sha((SAVED / "band-companion-contract.json").read_bytes()) == companion.SAVED_CONTRACT_SHA256
    assert new["saved_development_origin"]["historical_grades_reused"] is False
    assert new["saved_development_origin"]["original_quality_credit"] is False
    assert acquire.pool_input_mode_for_stage(new, "development") == "saved-corpus/1"
    assert acquire.pool_input_mode_for_stage(new, "confirmation") == "frozen-pool/1"
    with pytest.raises(ValueError, match="stage kind required"):
        acquire.pool_input_mode_for_stage(new, None)


def test_wire_compatibility_method_changes_only_candidate_parser_pin():
    old = json.loads((SAVED / "protocol.json").read_bytes())
    new_bytes = (SAVED_WIRE_COMPAT / "protocol.json").read_bytes()
    new = json.loads(new_bytes)
    changed = {key for key in old if old[key] != new.get(key)}
    assert changed == {"candidate_source_sha256"}
    assert new["candidate_source_sha256"] == "126528a56d7d0179a46fe111673b0be191569d64bbad196921b182a211cb03d6"
    assert (SAVED_WIRE_COMPAT / "cohorts.json").read_bytes() == (SAVED / "cohorts.json").read_bytes()
    lock = json.loads((SAVED_WIRE_COMPAT / "material-lock.json").read_bytes())
    companion_bytes = (SAVED_WIRE_COMPAT / "band-companion-contract.json").read_bytes()
    assert lock["candidate_source_sha256"] == new["candidate_source_sha256"]
    assert _sha(new_bytes) == companion.SAVED_WIRE_COMPAT_PROTOCOL_SHA256
    assert _sha((SAVED_WIRE_COMPAT / "cohorts.json").read_bytes()) == companion.FROZEN_COHORTS_SHA256
    assert _sha(companion_bytes) == companion.SAVED_WIRE_COMPAT_CONTRACT_SHA256
    assert lock["protocol_sha256"] == _sha(new_bytes)
    assert lock["band_companion_sha256"] == _sha(companion_bytes)
    execution_keys = (
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
    assert _sha(gates._canonical({key: new[key] for key in execution_keys})) == lock["execution_contract_sha256"]
    assert lock["numeric_quality_and_task_use_unchanged"] is True
    assert lock["parser_wire_compatibility"]["ranking_instructions_changed"] is False
    assert lock["parser_wire_compatibility"]["quality_thresholds_changed"] is False
    assert lock["parser_wire_compatibility"]["existing_results_rescued"] is False


def test_wire_float_method_changes_only_candidate_parser_pin_and_locks_exact_pair():
    previous = json.loads((SAVED_WIRE_COMPAT / "protocol.json").read_bytes())
    raw = (SAVED_WIRE_FLOAT / "protocol.json").read_bytes()
    protocol = json.loads(raw)
    assert {key for key in previous if previous[key] != protocol.get(key)} == {"candidate_source_sha256"}
    assert protocol["candidate_source_sha256"] == (
        "3be05771ebd32fcf18433ee87db031bd190ee9615237a95e4bbd96108487067a"
    )
    cohort_bytes = (SAVED_WIRE_FLOAT / "cohorts.json").read_bytes()
    assert cohort_bytes == (SAVED_WIRE_COMPAT / "cohorts.json").read_bytes()
    contract_bytes = (SAVED_WIRE_FLOAT / "band-companion-contract.json").read_bytes()
    lock = json.loads((SAVED_WIRE_FLOAT / "material-lock.json").read_bytes())
    assert _sha(raw) == companion.SAVED_WIRE_FLOAT_PROTOCOL_SHA256
    assert _sha(cohort_bytes) == companion.FROZEN_COHORTS_SHA256
    assert _sha(contract_bytes) == companion.SAVED_WIRE_FLOAT_CONTRACT_SHA256
    assert lock["protocol_sha256"] == _sha(raw)
    assert lock["cohorts_sha256"] == _sha(cohort_bytes)
    assert lock["band_companion_sha256"] == _sha(contract_bytes)
    assert lock["candidate_source_sha256"] == protocol["candidate_source_sha256"]
    assert lock["execution_contract_sha256"] == _sha(
        gates._canonical(
            {
                key: protocol[key]
                for key in (
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
            }
        )
    )
    assert lock["reported_scores_and_ranking_instructions_unchanged"] is True
    assert lock["parser_wire_compatibility"]["score_policy_changed"] is False
    assert lock["parser_wire_compatibility"]["existing_results_rescued"] is False

    inputs = _fixture()
    inputs.update(
        protocol=protocol,
        protocol_sha256=_sha(raw),
        cohorts_sha256=_sha(cohort_bytes),
        stage_kind="development",
    )
    inputs["resource_evidence"].update(
        candidate_source_sha256=protocol["candidate_source_sha256"],
        acquisition_timeout_seconds=protocol["acquisition"]["timeout_seconds"],
        acquisition_retries=protocol["acquisition"]["retries"],
        acquisition_pacing_seconds=protocol["acquisition"]["pacing_seconds"],
        arxiv_pacing_seconds=protocol["acquisition"]["arxiv_physical_pacing_seconds"],
    )
    report = gates.calculate_coverage_gate_report(**inputs)
    assert report.evaluation.status == "pass"
    contract = json.loads(contract_bytes)
    validated = companion.validate_contract(
        contract,
        stage_kind="development",
        protocol_sha256=_sha(raw),
        cohorts_sha256=_sha(cohort_bytes),
    )
    assert validated["groups"] == companion.FROZEN_GROUPS

    changed = copy.deepcopy(inputs)
    changed["protocol"]["candidate_source_sha256"] = "0" * 64
    with pytest.raises(gates.GateCalculationError, match="frozen-execution-contract-mismatch"):
        gates.calculate_coverage_gate_report(**changed)
    with pytest.raises(companion.BandCompanionError, match="companion-material-pin-mismatch"):
        companion.validate_contract(
            contract,
            stage_kind="development",
            protocol_sha256=companion.SAVED_WIRE_COMPAT_PROTOCOL_SHA256,
            cohorts_sha256=companion.FROZEN_COHORTS_SHA256,
        )
    assert (
        companion.contract_sha256_for(
            companion.SAVED_WIRE_FLOAT_PROTOCOL_SHA256,
            companion.FROZEN_COHORTS_SHA256,
        )
        == companion.SAVED_WIRE_FLOAT_CONTRACT_SHA256
    )


def test_wire_compatibility_method_uses_exact_gate_and_band_allowlists():
    raw = (SAVED_WIRE_COMPAT / "protocol.json").read_bytes()
    protocol = json.loads(raw)
    inputs = _fixture()
    inputs.update(
        protocol=protocol,
        protocol_sha256=_sha(raw),
        cohorts_sha256=companion.FROZEN_COHORTS_SHA256,
        stage_kind="development",
    )
    inputs["resource_evidence"].update(
        candidate_source_sha256=protocol["candidate_source_sha256"],
        acquisition_timeout_seconds=protocol["acquisition"]["timeout_seconds"],
        acquisition_retries=protocol["acquisition"]["retries"],
        acquisition_pacing_seconds=protocol["acquisition"]["pacing_seconds"],
        arxiv_pacing_seconds=protocol["acquisition"]["arxiv_physical_pacing_seconds"],
    )
    report = gates.calculate_coverage_gate_report(**copy.deepcopy(inputs))
    assert report.evaluation.status == "pass"
    old_raw = (SAVED / "protocol.json").read_bytes()
    old_protocol = json.loads(old_raw)
    old_inputs = _fixture()
    old_inputs.update(
        protocol=old_protocol,
        protocol_sha256=_sha(old_raw),
        cohorts_sha256=companion.FROZEN_COHORTS_SHA256,
    )
    old_inputs["resource_evidence"].update(
        acquisition_timeout_seconds=old_protocol["acquisition"]["timeout_seconds"],
        acquisition_retries=old_protocol["acquisition"]["retries"],
        acquisition_pacing_seconds=old_protocol["acquisition"]["pacing_seconds"],
        arxiv_pacing_seconds=old_protocol["acquisition"]["arxiv_physical_pacing_seconds"],
    )
    old_report = gates.calculate_coverage_gate_report(**old_inputs)
    assert old_report.evaluation.status == "pass"
    assert report.metrics["ranking"] == old_report.metrics["ranking"]
    assert report.metrics["task_use_R1"] == old_report.metrics["task_use_R1"]
    assert report.metrics["task_use_R2"] == old_report.metrics["task_use_R2"]

    contract = json.loads((SAVED_WIRE_COMPAT / "band-companion-contract.json").read_bytes())
    validated = companion.validate_contract(
        contract,
        stage_kind="development",
        protocol_sha256=_sha(raw),
        cohorts_sha256=companion.FROZEN_COHORTS_SHA256,
    )
    assert validated["groups"]["research_le_40"]["development"] == ["D26-R01", "D26-R02", "D26-R03", "D26-R04"]

    changed = copy.deepcopy(inputs)
    changed["protocol"]["candidate_source_sha256"] = "0" * 64
    with pytest.raises(gates.GateCalculationError, match="frozen-execution-contract-mismatch"):
        gates.calculate_coverage_gate_report(**changed)

    with pytest.raises(companion.BandCompanionError, match="companion-material-pin-mismatch"):
        companion.validate_contract(
            contract,
            stage_kind="development",
            protocol_sha256=companion.SAVED_PROTOCOL_SHA256,
            cohorts_sha256=companion.FROZEN_COHORTS_SHA256,
        )
    changed_contract = copy.deepcopy(contract)
    changed_contract["protocol_sha256"] = companion.SAVED_PROTOCOL_SHA256
    with pytest.raises(companion.BandCompanionError, match="companion-material-pin-mismatch"):
        companion.validate_contract(
            changed_contract,
            stage_kind="development",
            protocol_sha256=companion.SAVED_WIRE_COMPAT_PROTOCOL_SHA256,
            cohorts_sha256=companion.FROZEN_COHORTS_SHA256,
        )
    assert (
        companion.contract_sha256_for(
            companion.SAVED_WIRE_COMPAT_PROTOCOL_SHA256,
            companion.FROZEN_COHORTS_SHA256,
        )
        == companion.SAVED_WIRE_COMPAT_CONTRACT_SHA256
    )
    assert (
        companion.contract_sha256_for(
            companion.SAVED_PROTOCOL_SHA256,
            companion.FROZEN_COHORTS_SHA256,
        )
        == companion.SAVED_CONTRACT_SHA256
    )
    assert (
        companion.contract_sha256_for(
            companion.SAVED_WIRE_COMPAT_PROTOCOL_SHA256,
            "0" * 64,
        )
        is None
    )


def test_saved_method_reaches_unchanged_quality_calculator_and_rejects_retuning():
    raw = (SAVED / "protocol.json").read_bytes()
    protocol = json.loads(raw)
    inputs = _fixture()
    inputs.update(
        protocol=protocol,
        protocol_sha256=_sha(raw),
        cohorts_sha256=companion.FROZEN_COHORTS_SHA256,
    )
    inputs["resource_evidence"].update(
        acquisition_timeout_seconds=protocol["acquisition"]["timeout_seconds"],
        acquisition_retries=protocol["acquisition"]["retries"],
        acquisition_pacing_seconds=protocol["acquisition"]["pacing_seconds"],
        arxiv_pacing_seconds=protocol["acquisition"]["arxiv_physical_pacing_seconds"],
    )
    report = gates.calculate_coverage_gate_report(**inputs)
    assert report.evaluation.status == "pass"
    changed = copy.deepcopy(inputs)
    changed["protocol"]["consumer"]["answerer_calls_maximum"] += 1
    with pytest.raises(gates.GateCalculationError, match="frozen-execution-contract-mismatch"):
        gates.calculate_coverage_gate_report(**changed)
    changed = copy.deepcopy(inputs)
    changed["protocol_sha256"] = "0" * 64
    with pytest.raises(gates.GateCalculationError, match="frozen-execution-contract-mismatch"):
        gates.calculate_coverage_gate_report(**changed)


def test_saved_method_reaches_independent_finalizer(tmp_path):
    helper = finalization_fixtures.StageFinalizationTests()
    paths, plan = helper._fixture(
        tmp_path,
        elapsed=7.5,
        protocol_bytes_override=(SAVED / "protocol.json").read_bytes(),
        cohorts_bytes_override=(SAVED / "cohorts.json").read_bytes(),
    )
    result = helper._verify(paths, plan)
    assert result.status == "gates-pass-awaiting-independent-decision"
    assert json.loads(result.receipt_bytes)["product_authorized"] is False
