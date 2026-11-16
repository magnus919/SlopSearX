"""Companion qualification for the two frozen research-size bands.

This is an additive, opt-in registered contract. The original mixed-workload
calculator and its protocol thresholds remain unchanged. The companion uses
the same closed grades and the same ranking/task-use implementations, split
only by the predeclared cohort membership.
"""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from types import SimpleNamespace

from scripts.coverage_gate_calculation import (
    Endpoint,
    GateCalculationError,
    _ranking_endpoint,
    _task_use,
    _validated_useful_count_role,
    quality_task_use_sha256,
)

SCHEMA = "coverage-first-band-companion/1"
ARCHIVAL_PROTOCOL_SHA256 = "567e337d41268f380348abc70ab1bf4a0249852f237797fa361295f1e34afb06"
ARCHIVAL_COHORTS_SHA256 = "36d4cbff9c93555d35d7269525616f009a21727c8d1141a8a2a61934791038f0"
# Preserve the established names for the retained archival contract.
PROTOCOL_SHA256 = ARCHIVAL_PROTOCOL_SHA256
COHORTS_SHA256 = ARCHIVAL_COHORTS_SHA256
FROZEN_PROTOCOL_SHA256 = "e4f7a7eaf7b78c9e662e02b48f9672eac368db7bef2992fe4e4c11443500b604"
FROZEN_COHORTS_SHA256 = "377450c59f2b8498c8a7f923bb7b6d05a0b780300467cc9b9f8ef0bd69522595"
FROZEN_CONTRACT_SHA256 = "359f0899a2afcec711352fa5bc673de6cb94d308cf69e013347e24aebb4814e7"
SAVED_PROTOCOL_SHA256 = "647d1a09b9594f04798a9b891e609ed710c37111da62ecbf3e8e34d1b183c2bb"
SAVED_CONTRACT_SHA256 = "e71660f3dbabe03c7d15e983f3c8b8cc666560d5f900a3323508b407cdef87b6"
SAVED_WIRE_COMPAT_PROTOCOL_SHA256 = "5b40c497b0871a575c07eff18b8c8f8a54dcb417d1028e61815ab9a77887a604"
SAVED_WIRE_COMPAT_CONTRACT_SHA256 = "8d807e655098848762c6c76ed7f862d24a8d256b38b7ada55d246a0fb01561fc"
SAVED_WIRE_FLOAT_PROTOCOL_SHA256 = "65b5515eb161c849c54f196c300f03b3965cbdbd4162682131b13ccecd0911a8"
SAVED_WIRE_FLOAT_CONTRACT_SHA256 = "268a26679401cd728808ba24a589643c31770e3a747b0d33462f5ba2563fbfec"
TASK_USE_V4_PROTOCOL_SHA256 = "9278fa98aa0de47fcb9a83a0f7067ef002671012e7995a6907b144ce66ecd37f"
TASK_USE_V4_COHORTS_SHA256 = "4ae302eaebd54d9fe2c3bbd49d243811d07ffd2135c207698db1742644355b72"
TASK_USE_V4_CONTRACT_SHA256 = "84a536d20291749232e8a07f1f10adc66f87a5e15ac16bfad6a574dfc691fd7c"
INPUT_CORRECTED_PROTOCOL_SHA256 = "cfac29e7d406dc4c7e45030fafeb6ba466923cfeb19b34e427d344507c9d9ba9"
INPUT_CORRECTED_COHORTS_SHA256 = "ad731b5d99907242dbc7c6be0c671049e179df76d50a2148886e7e34b6b28b30"
INPUT_CORRECTED_CONTRACT_SHA256 = "5b802406687af0a3c1cd318a4f5dfc6250f2643e81547d2739f7521e81740ed2"
ARCHIVAL_GROUPS = {
    "research_le_40": {
        "development": ["D-R01", "D-R02", "D-R03", "D-R04"],
        "confirmation": ["C-R01", "C-R02", "C-R03", "C-R04"],
    },
    "research_41_80": {
        "development": ["D-R05", "D-R06", "D-R07", "D-R08"],
        "confirmation": ["C-R05", "C-R06", "C-R07", "C-R08"],
    },
}
FROZEN_GROUPS = {
    "research_le_40": {
        "development": ["D26-R01", "D26-R02", "D26-R03", "D26-R04"],
        "confirmation": ["C26-R01", "C26-R02", "C26-R03", "C26-R04"],
    },
    "research_41_80": {
        "development": ["D26-R05", "D26-R06", "D26-R07", "D26-R08"],
        "confirmation": ["C26-R05", "C26-R06", "C26-R07", "C26-R08"],
    },
}
TASK_USE_V4_GROUPS = {
    "research_le_40": {
        "development": ["D27-R01", "D27-R02", "D27-R03", "D27-R04"],
        "confirmation": ["C27-R01", "C27-R02", "C27-R03", "C27-R04"],
    },
    "research_41_80": {
        "development": ["D27-R05", "D27-R06", "D27-R07", "D27-R08"],
        "confirmation": ["C27-R05", "C27-R06", "C27-R07", "C27-R08"],
    },
}
# Backward-compatible name for archived-contract tests and consumers.
GROUPS = ARCHIVAL_GROUPS


def contract_sha256_for(protocol_sha256: str, cohorts_sha256: str) -> str | None:
    """Return the exact allowlisted companion digest for a material pair."""
    allowed = {
        (FROZEN_PROTOCOL_SHA256, FROZEN_COHORTS_SHA256): FROZEN_CONTRACT_SHA256,
        (SAVED_PROTOCOL_SHA256, FROZEN_COHORTS_SHA256): SAVED_CONTRACT_SHA256,
        (SAVED_WIRE_COMPAT_PROTOCOL_SHA256, FROZEN_COHORTS_SHA256): SAVED_WIRE_COMPAT_CONTRACT_SHA256,
        (SAVED_WIRE_FLOAT_PROTOCOL_SHA256, FROZEN_COHORTS_SHA256): SAVED_WIRE_FLOAT_CONTRACT_SHA256,
        (TASK_USE_V4_PROTOCOL_SHA256, TASK_USE_V4_COHORTS_SHA256): TASK_USE_V4_CONTRACT_SHA256,
        (INPUT_CORRECTED_PROTOCOL_SHA256, INPUT_CORRECTED_COHORTS_SHA256): INPUT_CORRECTED_CONTRACT_SHA256,
    }
    return allowed.get((protocol_sha256, cohorts_sha256))


class BandCompanionError(ValueError):
    """The registered companion declaration or its closed inputs are invalid."""


@dataclass(frozen=True)
class BandCompanionResult:
    status: str
    groups: Mapping[str, object]
    gates: Mapping[str, bool | None]


def validate_contract(contract: object, *, stage_kind: str, protocol_sha256: str, cohorts_sha256: str) -> dict:
    if type(stage_kind) is not str or stage_kind not in {"development", "confirmation"}:
        raise BandCompanionError("companion-stage-kind-invalid")
    if type(contract) is not dict or set(contract) != {
        "schema",
        "stage_kind",
        "protocol_sha256",
        "cohorts_sha256",
        "groups",
        "claim_scope",
    }:
        raise BandCompanionError("companion-contract-shape")
    if contract["schema"] != SCHEMA or contract["stage_kind"] != "development_and_confirmation":
        raise BandCompanionError("companion-contract-identity")
    if (contract["protocol_sha256"], contract["cohorts_sha256"]) == (
        ARCHIVAL_PROTOCOL_SHA256,
        ARCHIVAL_COHORTS_SHA256,
    ) and (protocol_sha256, cohorts_sha256) == (ARCHIVAL_PROTOCOL_SHA256, ARCHIVAL_COHORTS_SHA256):
        expected_groups = ARCHIVAL_GROUPS
    elif (contract["protocol_sha256"], contract["cohorts_sha256"]) == (
        FROZEN_PROTOCOL_SHA256,
        FROZEN_COHORTS_SHA256,
    ) and (protocol_sha256, cohorts_sha256) == (FROZEN_PROTOCOL_SHA256, FROZEN_COHORTS_SHA256):
        expected_groups = FROZEN_GROUPS
    elif (contract["protocol_sha256"], contract["cohorts_sha256"]) == (
        SAVED_PROTOCOL_SHA256,
        FROZEN_COHORTS_SHA256,
    ) and (protocol_sha256, cohorts_sha256) == (SAVED_PROTOCOL_SHA256, FROZEN_COHORTS_SHA256):
        expected_groups = FROZEN_GROUPS
    elif (contract["protocol_sha256"], contract["cohorts_sha256"]) == (
        SAVED_WIRE_COMPAT_PROTOCOL_SHA256,
        FROZEN_COHORTS_SHA256,
    ) and (protocol_sha256, cohorts_sha256) == (SAVED_WIRE_COMPAT_PROTOCOL_SHA256, FROZEN_COHORTS_SHA256):
        expected_groups = FROZEN_GROUPS
    elif (contract["protocol_sha256"], contract["cohorts_sha256"]) == (
        SAVED_WIRE_FLOAT_PROTOCOL_SHA256,
        FROZEN_COHORTS_SHA256,
    ) and (protocol_sha256, cohorts_sha256) == (SAVED_WIRE_FLOAT_PROTOCOL_SHA256, FROZEN_COHORTS_SHA256):
        expected_groups = FROZEN_GROUPS
    elif (contract["protocol_sha256"], contract["cohorts_sha256"]) == (
        TASK_USE_V4_PROTOCOL_SHA256,
        TASK_USE_V4_COHORTS_SHA256,
    ) and (protocol_sha256, cohorts_sha256) == (TASK_USE_V4_PROTOCOL_SHA256, TASK_USE_V4_COHORTS_SHA256):
        expected_groups = TASK_USE_V4_GROUPS
    elif (contract["protocol_sha256"], contract["cohorts_sha256"]) == (
        INPUT_CORRECTED_PROTOCOL_SHA256,
        INPUT_CORRECTED_COHORTS_SHA256,
    ) and (protocol_sha256, cohorts_sha256) == (
        INPUT_CORRECTED_PROTOCOL_SHA256,
        INPUT_CORRECTED_COHORTS_SHA256,
    ):
        expected_groups = TASK_USE_V4_GROUPS
    else:
        raise BandCompanionError("companion-material-pin-mismatch")
    if contract["groups"] != expected_groups:
        raise BandCompanionError("companion-group-inventory-mismatch")
    if contract["claim_scope"] != "fixed-frozen-cases-only; no population or causal-depth claim":
        raise BandCompanionError("companion-claim-scope-invalid")
    return contract


def _validate_frozen_recipes(
    quality: Mapping[str, object], task_use: Mapping[str, object], protocol: Mapping[str, object]
) -> str:
    try:
        raw = json.dumps(
            {"quality": quality, "task_use": task_use},
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8")
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise BandCompanionError("companion-gate-recipe-invalid") from exc
    try:
        useful_count_role = _validated_useful_count_role(protocol, quality)
    except GateCalculationError as exc:
        # Keep this module's public error vocabulary stable for callers.
        raise BandCompanionError("companion-useful-count-policy-invalid") from exc
    try:
        expected_recipe_sha256 = quality_task_use_sha256(protocol)
    except GateCalculationError as exc:
        raise BandCompanionError("companion-frozen-gate-recipe-mismatch") from exc
    if hashlib.sha256(raw).hexdigest() != expected_recipe_sha256:
        raise BandCompanionError("companion-frozen-gate-recipe-mismatch")
    return useful_count_role


def calculate(
    *,
    contract: Mapping[str, object],
    stage_kind: str,
    protocol_sha256: str,
    cohorts_sha256: str,
    quality: Mapping[str, object],
    task_use: Mapping[str, object],
    protocol: Mapping[str, object],
    task_inputs: Sequence[Mapping[str, object]],
    orders: Mapping[str, object],
    records_by_task: Mapping[str, object],
    answer_scores: object,
    prepared_assessments: object,
) -> BandCompanionResult:
    useful_count_role = _validate_frozen_recipes(quality, task_use, protocol)
    validated_contract = validate_contract(
        dict(contract), stage_kind=stage_kind, protocol_sha256=protocol_sha256, cohorts_sha256=cohorts_sha256
    )
    groups: dict[str, object] = {}
    gates: dict[str, bool | None] = {}
    observed = {row.get("task_id"): row for row in task_inputs if type(row) is dict}
    catalog = getattr(prepared_assessments, "task_catalogs", None)
    for band, stage_ids in validated_contract["groups"].items():
        task_ids = stage_ids[stage_kind]
        rows = [observed.get(task_id) for task_id in task_ids]
        valid = (
            len(observed) == len(task_inputs) == 8
            and all(type(row) is dict and row.get("band") == band for row in rows)
            and all(row.get("intent") in {"evidence_seeking", "source_discovery"} for row in rows)
            and sum(row.get("intent") == "evidence_seeking" for row in rows) == 2
            and sum(row.get("intent") == "source_discovery" for row in rows) == 2
        )
        if not valid:
            for assessor in quality["independent_references"]:
                gates[f"band_{band}_{assessor}_ranking"] = None
            for assessor in task_use["independent_assessors"]:
                gates[f"band_{band}_{assessor}_task_use"] = None
            groups[band] = {"status": "inconclusive", "reason": "registered-band-membership-incomplete"}
            continue
        ranking = {
            assessor: _ranking_endpoint(
                assessor, rows, orders, records_by_task, quality, useful_count_role=useful_count_role
            )
            for assessor in quality["independent_references"]
        }
        band_answers = (
            {
                task_id: answer_scores[task_id]
                for task_id in task_ids
                if type(answer_scores) is dict and task_id in answer_scores
            }
            if type(answer_scores) is dict
            else None
        )
        band_catalogs = (
            {task_id: catalog[task_id] for task_id in task_ids if type(catalog) is dict and task_id in catalog}
            if type(catalog) is dict
            else None
        )
        band_prepared = SimpleNamespace(task_catalogs=band_catalogs)
        task_results: dict[str, Endpoint] = {}
        if type(band_answers) is dict:
            for assessor in task_use["independent_assessors"]:
                filtered = {
                    task_id: {
                        arm: ({assessor: graders[assessor]} if type(graders) is dict and assessor in graders else {})
                        for arm, graders in arms.items()
                    }
                    for task_id, arms in band_answers.items()
                    if type(arms) is dict
                }
                task_results[assessor] = _task_use(
                    filtered,
                    band_prepared,
                    task_ids,
                    task_use["additional_completed_tasks_each_minimum"],
                    protocol,
                    (assessor,),
                )
        else:
            task_results = {
                assessor: Endpoint(None, {"reason": "answer-output-unavailable"})
                for assessor in task_use["independent_assessors"]
            }
        for assessor, result in ranking.items():
            gates[f"band_{band}_{assessor}_ranking"] = result.passed
        for assessor, result in task_results.items():
            gates[f"band_{band}_{assessor}_task_use"] = result.passed
        values = [*ranking.values(), *task_results.values()]
        group_status = (
            "inconclusive"
            if any(item.passed is None for item in values)
            else "fail"
            if any(item.passed is False for item in values)
            else "pass"
        )
        groups[band] = {
            "status": group_status,
            "task_ids": task_ids,
            "ranking": {key: value.metrics for key, value in ranking.items()},
            "task_use": {key: value.metrics for key, value in task_results.items()},
        }
    status = (
        "inconclusive"
        if any(value is None for value in gates.values())
        else "fail"
        if any(value is False for value in gates.values())
        else "pass"
    )
    return BandCompanionResult(status=status, groups=groups, gates=gates)
