"""Cross-file source binding for the current prospective research method."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path


def test_current_task_use_method_binds_checked_out_candidate_source():
    root = Path(__file__).parents[1]
    materials = root / "docs/experiments/evidence/coverage-first-study/research-task-use-method-materials"
    protocol = json.loads((materials / "protocol.json").read_bytes())
    candidate_sha = hashlib.sha256((root / "scripts/intent_ranking_coverage.py").read_bytes()).hexdigest()
    assert protocol["candidate_source_sha256"] == candidate_sha


def test_corrected_method_changes_only_two_development_queries():
    root = Path(__file__).parents[1]
    evidence = root / "docs/experiments/evidence/coverage-first-study"
    original = evidence / "research-task-use-method-materials"
    corrected = evidence / "input-corrected-method-materials"
    old = json.loads((original / "cohorts.json").read_bytes())
    new = json.loads((corrected / "cohorts.json").read_bytes())
    assert new["stages"][1] == old["stages"][1]
    development = new["stages"][0]
    research = next(case for case in development["research_cases"] if case["task_id"] == "D27-R05")
    navigation = next(case for case in development["navigation_targets"] if case["target_id"] == "D27-N03")
    assert research["search_query"] == "container registry"
    assert navigation["search_query"] == "backstage"
    research["search_query"] = "artifact registry"
    navigation["search_query"] = "backstage platform"
    assert new == old

    protocol = json.loads((corrected / "protocol.json").read_bytes())
    old_protocol = json.loads((original / "protocol.json").read_bytes())
    protocol["cohorts_sha256"] = old_protocol["cohorts_sha256"]
    assert protocol == old_protocol


def test_corrected_method_locks_materials_and_unchanged_quality_contract():
    from scripts import coverage_gate_calculation as gates

    root = Path(__file__).parents[1]
    materials = root / "docs/experiments/evidence/coverage-first-study/input-corrected-method-materials"
    lock = json.loads((materials / "material-lock.json").read_bytes())
    for name, key in (
        ("protocol.json", "protocol_sha256"),
        ("cohorts.json", "cohorts_sha256"),
        ("band-companion-contract.json", "band_companion_sha256"),
    ):
        assert hashlib.sha256((materials / name).read_bytes()).hexdigest() == lock[key]
    protocol = json.loads((materials / "protocol.json").read_bytes())
    assert protocol["cohorts_sha256"] == lock["cohorts_sha256"]
    assert gates.quality_task_use_sha256(protocol) == lock["quality_task_use_sha256"]
    assert lock["quality_task_use_sha256"] == gates.TASK_USE_V4_QUALITY_TASK_USE_SHA256
    candidate_sha = hashlib.sha256((root / "scripts/intent_ranking_coverage.py").read_bytes()).hexdigest()
    assert protocol["candidate_source_sha256"] == candidate_sha
