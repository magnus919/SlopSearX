from __future__ import annotations

import copy
import hashlib
import json
from dataclasses import replace
from pathlib import Path

import pytest

from scripts import coverage_band_companion as companion
from scripts import coverage_gate_calculation as gates
from scripts import coverage_stage_orchestration as orchestration
from tests.test_coverage_gate_calculation import _fixture
from tests.test_coverage_stage_orchestration import plan_fixture

CONTRACT_PATH = (
    Path(__file__).parents[1]
    / "docs/experiments/evidence/coverage-first-study/band-companion/development-contract.json"
)
FRESH_MATERIAL_DIR = (
    Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study/frozen-input-method-materials"
)


def _inputs():
    inputs = _fixture()
    inputs["band_companion_bytes"] = CONTRACT_PATH.read_bytes()
    inputs["stage_kind"] = "development"
    inputs["protocol_sha256"] = companion.PROTOCOL_SHA256
    inputs["cohorts_sha256"] = companion.COHORTS_SHA256
    return inputs


def test_closed_grade_composition_preserves_primary_and_adds_independent_band_gates():
    inputs = _inputs()
    # The 41–80 cohort remains within the per-case floor, but its ranking gain
    # is zero; the full mixed workload still passes on the strong first band.
    for task in inputs["task_inputs"]:
        if task["band"] == "research_41_80":
            row = inputs["selector_evidence"].research_orders[task["task_id"]]
            row["candidate"] = list(row["w0"])
            stability = inputs["stability_orders"].get(task["task_id"])
            if stability is not None:
                for variant in ("base", "repeat", "rotate"):
                    stability["candidate"][variant] = list(row["w0"])

    primary_inputs = {
        key: value
        for key, value in inputs.items()
        if key not in {"band_companion_bytes", "stage_kind", "protocol_sha256", "cohorts_sha256"}
    }
    primary = gates.calculate_coverage_gate_report(**copy.deepcopy(primary_inputs))
    report = gates.calculate_coverage_gate_report(**inputs)
    assert report.evaluation.status == primary.evaluation.status == "pass"
    assert report.evaluation.gates == primary.evaluation.gates
    assert report.evaluation.companion_status == "fail"
    assert report.metrics["band_companion"]["groups"]["research_le_40"]["status"] == "pass"
    assert report.metrics["band_companion"]["groups"]["research_41_80"]["status"] == "fail"
    assert set(report.metrics["band_companion"]["gates"]) == {
        f"band_{band}_{assessor}_ranking" for band in ("research_le_40", "research_41_80") for assessor in ("A", "B")
    } | {
        f"band_{band}_{assessor}_task_use" for band in ("research_le_40", "research_41_80") for assessor in ("R1", "R2")
    }
    without_companion = dict(report.metrics)
    without_companion.pop("band_companion")
    assert without_companion == primary.metrics
    receipt = json.loads(report.receipt_bytes)
    assert receipt["status"] == "pass"
    assert receipt["band_companion_status"] == "fail"


def test_standalone_contract_rejects_unknown_stage_kind():
    contract = json.loads(CONTRACT_PATH.read_bytes())
    contract["stage_kind"] = "experimental"
    with pytest.raises(companion.BandCompanionError, match="stage-kind-invalid"):
        companion.validate_contract(
            contract,
            stage_kind="experimental",
            protocol_sha256=companion.PROTOCOL_SHA256,
            cohorts_sha256=companion.COHORTS_SHA256,
        )
    with pytest.raises(companion.BandCompanionError, match="stage-kind-invalid"):
        companion.validate_contract(
            json.loads(CONTRACT_PATH.read_bytes()),
            stage_kind=[],
            protocol_sha256=companion.PROTOCOL_SHA256,
            cohorts_sha256=companion.COHORTS_SHA256,
        )


def test_standalone_calculator_rejects_altered_frozen_recipe():
    contract = json.loads(CONTRACT_PATH.read_bytes())
    quality = {"bootstrap_seed": 1}
    with pytest.raises(companion.BandCompanionError, match="frozen-gate-recipe-mismatch"):
        companion.calculate(
            contract=contract,
            stage_kind="development",
            protocol_sha256=companion.PROTOCOL_SHA256,
            cohorts_sha256=companion.COHORTS_SHA256,
            quality=quality,
            task_use={},
            protocol={},
            task_inputs=(),
            orders={},
            records_by_task={},
            answer_scores=None,
            prepared_assessments=None,
        )


def test_fresh_frozen_contract_uses_only_locked_d26_c26_groups():
    contract_bytes = (FRESH_MATERIAL_DIR / "band-companion-contract.json").read_bytes()
    contract = json.loads(contract_bytes)
    protocol_bytes = (FRESH_MATERIAL_DIR / "protocol.json").read_bytes()
    cohorts_bytes = (FRESH_MATERIAL_DIR / "cohorts.json").read_bytes()
    protocol_sha = hashlib.sha256(protocol_bytes).hexdigest()
    cohorts_sha = hashlib.sha256(cohorts_bytes).hexdigest()
    lock = json.loads((FRESH_MATERIAL_DIR / "material-lock.json").read_bytes())
    assert hashlib.sha256(contract_bytes).hexdigest() == lock["band_companion_sha256"]
    validated = companion.validate_contract(
        contract,
        stage_kind="development",
        protocol_sha256=protocol_sha,
        cohorts_sha256=cohorts_sha,
    )
    assert validated["groups"] == companion.FROZEN_GROUPS
    assert validated["groups"] is not companion.ARCHIVAL_GROUPS
    confirmation = companion.validate_contract(
        contract,
        stage_kind="confirmation",
        protocol_sha256=protocol_sha,
        cohorts_sha256=cohorts_sha,
    )
    assert confirmation["groups"]["research_le_40"]["confirmation"] == [
        "C26-R01",
        "C26-R02",
        "C26-R03",
        "C26-R04",
    ]

    protocol = json.loads(protocol_bytes)
    inputs = _fixture()
    task_ids = [row["task_id"] for row in inputs["task_inputs"]]
    remap = {task_id: f"D26-R{index:02d}" for index, task_id in enumerate(task_ids, 1)}
    result = companion.calculate(
        contract=contract,
        stage_kind="development",
        protocol_sha256=protocol_sha,
        cohorts_sha256=cohorts_sha,
        quality=protocol["quality"],
        task_use=protocol["task_use"],
        protocol=protocol,
        task_inputs=[{**row, "task_id": remap[row["task_id"]]} for row in inputs["task_inputs"]],
        orders={},
        records_by_task={},
        answer_scores=None,
        prepared_assessments=None,
    )
    assert result.groups["research_le_40"]["task_ids"] == companion.FROZEN_GROUPS["research_le_40"]["development"]
    assert result.groups["research_41_80"]["task_ids"] == companion.FROZEN_GROUPS["research_41_80"]["development"]

    wrong_groups = copy.deepcopy(contract)
    wrong_groups["groups"] = companion.ARCHIVAL_GROUPS
    with pytest.raises(companion.BandCompanionError, match="group-inventory-mismatch"):
        companion.validate_contract(
            wrong_groups,
            stage_kind="development",
            protocol_sha256=protocol_sha,
            cohorts_sha256=cohorts_sha,
        )


def test_task_use_v4_companion_is_exactly_pinned_and_keeps_its_error_surface():
    materials = (
        Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study/research-task-use-method-materials"
    )
    protocol_bytes = (materials / "protocol.json").read_bytes()
    cohorts_bytes = (materials / "cohorts.json").read_bytes()
    contract_bytes = (materials / "band-companion-contract.json").read_bytes()
    protocol = json.loads(protocol_bytes)
    contract = json.loads(contract_bytes)
    protocol_sha = hashlib.sha256(protocol_bytes).hexdigest()
    cohorts_sha = hashlib.sha256(cohorts_bytes).hexdigest()
    assert hashlib.sha256(contract_bytes).hexdigest() == companion.TASK_USE_V4_CONTRACT_SHA256
    validated = companion.validate_contract(
        contract, stage_kind="development", protocol_sha256=protocol_sha, cohorts_sha256=cohorts_sha
    )
    assert validated["groups"] == companion.TASK_USE_V4_GROUPS
    companion._validate_frozen_recipes(protocol["quality"], protocol["task_use"], protocol)

    invalid_quality = copy.deepcopy(protocol["quality"])
    invalid_quality.pop("useful_count_role")
    with pytest.raises(companion.BandCompanionError, match="companion-useful-count-policy-invalid"):
        companion._validate_frozen_recipes(invalid_quality, protocol["task_use"], protocol)

    with pytest.raises(companion.BandCompanionError, match="companion-material-pin-mismatch"):
        companion.validate_contract(
            contract, stage_kind="development", protocol_sha256=protocol_sha, cohorts_sha256="0" * 64
        )


def test_input_corrected_companion_accepts_only_its_exact_protocol_cohort_pair():
    protocol_sha = companion.INPUT_CORRECTED_PROTOCOL_SHA256
    cohorts_sha = companion.INPUT_CORRECTED_COHORTS_SHA256
    contract = {
        "schema": companion.SCHEMA,
        "stage_kind": "development_and_confirmation",
        "protocol_sha256": protocol_sha,
        "cohorts_sha256": cohorts_sha,
        "groups": copy.deepcopy(companion.TASK_USE_V4_GROUPS),
        "claim_scope": "fixed-frozen-cases-only; no population or causal-depth claim",
    }

    assert companion.contract_sha256_for(protocol_sha, cohorts_sha) == companion.INPUT_CORRECTED_CONTRACT_SHA256
    assert (
        companion.validate_contract(
            contract,
            stage_kind="development",
            protocol_sha256=protocol_sha,
            cohorts_sha256=cohorts_sha,
        )["groups"]
        == companion.TASK_USE_V4_GROUPS
    )
    assert (
        companion.validate_contract(
            contract,
            stage_kind="confirmation",
            protocol_sha256=protocol_sha,
            cohorts_sha256=cohorts_sha,
        )["groups"]
        == companion.TASK_USE_V4_GROUPS
    )

    for mismatched_protocol, mismatched_cohorts in (
        (protocol_sha, companion.TASK_USE_V4_COHORTS_SHA256),
        (companion.TASK_USE_V4_PROTOCOL_SHA256, cohorts_sha),
    ):
        assert companion.contract_sha256_for(mismatched_protocol, mismatched_cohorts) is None
        with pytest.raises(companion.BandCompanionError, match="companion-material-pin-mismatch"):
            companion.validate_contract(
                contract,
                stage_kind="development",
                protocol_sha256=mismatched_protocol,
                cohorts_sha256=mismatched_cohorts,
            )

    wrong_groups = copy.deepcopy(contract)
    wrong_groups["groups"] = companion.ARCHIVAL_GROUPS
    with pytest.raises(companion.BandCompanionError, match="companion-group-inventory-mismatch"):
        companion.validate_contract(
            wrong_groups,
            stage_kind="development",
            protocol_sha256=protocol_sha,
            cohorts_sha256=cohorts_sha,
        )


def test_registered_v2_execution_contract_requires_exact_protocol_and_cohort_pins():
    registered_dir = CONTRACT_PATH.parents[1] / "registered-2026-10-12-acquisition-v2"
    protocol_bytes = (registered_dir / "protocol.json").read_bytes()
    cohorts_bytes = (registered_dir / "cohorts.json").read_bytes()
    protocol = json.loads(protocol_bytes)
    inputs = _fixture()
    inputs["protocol"] = protocol
    with pytest.raises(gates.GateCalculationError, match="frozen-execution-contract-mismatch"):
        gates.calculate_coverage_gate_report(**inputs)

    inputs["protocol_sha256"] = hashlib.sha256(protocol_bytes).hexdigest()
    inputs["cohorts_sha256"] = hashlib.sha256(cohorts_bytes).hexdigest()
    mutated = copy.deepcopy(inputs)
    mutated["protocol"]["acquisition"]["timeout_seconds"] += 1
    with pytest.raises(gates.GateCalculationError, match="frozen-execution-contract-mismatch"):
        gates.calculate_coverage_gate_report(**mutated)

    wrong_cohorts = copy.deepcopy(inputs)
    wrong_cohorts["cohorts_sha256"] = "0" * 64
    with pytest.raises(gates.GateCalculationError, match="frozen-execution-contract-mismatch"):
        gates.calculate_coverage_gate_report(**wrong_cohorts)


def test_missing_registered_band_member_is_inconclusive():
    inputs = _inputs()
    inputs["task_inputs"] = [
        {**row, "band": "research_le_40"} if row["task_id"] == "D-R08" else row for row in inputs["task_inputs"]
    ]
    report = gates.calculate_coverage_gate_report(**inputs)
    assert report.evaluation.companion_status == "inconclusive"
    assert report.metrics["band_companion"]["groups"]["research_41_80"]["status"] == "inconclusive"
    assert report.metrics["band_companion"]["groups"]["research_le_40"]["status"] == "pass"


def test_registered_v2_mixed_only_path_keeps_legacy_output_shape():
    """The registered acquisition contract is accepted without enabling bands."""
    registered_dir = CONTRACT_PATH.parents[1] / "registered-2026-10-12-acquisition-v2"
    protocol_bytes = (registered_dir / "protocol.json").read_bytes()
    protocol = json.loads(protocol_bytes)
    inputs = _fixture()
    inputs["protocol"] = protocol
    inputs["protocol_sha256"] = hashlib.sha256(protocol_bytes).hexdigest()
    inputs["cohorts_sha256"] = companion.COHORTS_SHA256
    inputs["resource_evidence"].update(
        acquisition_timeout_seconds=protocol["acquisition"]["timeout_seconds"],
        acquisition_retries=protocol["acquisition"]["retries"],
        acquisition_pacing_seconds=protocol["acquisition"]["pacing_seconds"],
        arxiv_pacing_seconds=protocol["acquisition"]["arxiv_physical_pacing_seconds"],
    )
    report = gates.calculate_coverage_gate_report(**inputs)

    assert report.evaluation.status == "pass"
    assert report.evaluation.scope == "mixed-workload-only"
    assert report.evaluation.companion_status is None
    assert "band_companion" not in report.metrics
    receipt = json.loads(report.receipt_bytes)
    assert receipt["status"] == "pass"
    assert not any("band_companion" in key for key in receipt)


def test_legacy_primary_receipts_omit_all_companion_fields():
    report = gates.calculate_coverage_gate_report(**_fixture())
    receipt = json.loads(report.receipt_bytes)
    assert "band_companion" not in report.metrics
    assert not any("band_companion" in key for key in receipt)


def test_static_identity_omits_companion_nulls_and_rejects_registration_drift():
    plan = plan_fixture()
    identity = orchestration._validate_plan_static(plan)
    assert "band_companion_sha256" not in identity
    assert "initial_registration_sha256" not in identity

    contract_bytes = CONTRACT_PATH.read_bytes()
    contract_sha = hashlib.sha256(contract_bytes).hexdigest()
    registration = {
        "schema": "coverage-study-stage-registration/1",
        "stage_uuid": plan.stage_uuid,
        "stage_kind": plan.stage_kind,
        "source_revision": plan.source_revision,
        "pins": {"protocol": hashlib.sha256(plan.protocol_bytes).hexdigest()},
        "operation_ids": [],
        "navigation_skip_reasons": [],
        "task_ids": [],
        "band_companion_sha256": "f" * 64,
    }
    registration_bytes = json.dumps(registration, sort_keys=True, separators=(",", ":")).encode()
    bound_plan = replace(
        plan,
        band_companion_bytes=contract_bytes,
        band_companion_sha256=contract_sha,
        initial_registration_bytes=registration_bytes,
        initial_registration_sha256=hashlib.sha256(registration_bytes).hexdigest(),
        band_companion_source_pins={"coverage_study_core": "a" * 64},
    )
    with pytest.raises(orchestration.OrchestrationError, match="registration-binding-mismatch"):
        orchestration._validate_plan_static(bound_plan)
