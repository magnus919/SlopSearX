"""Artifact-to-workflow composition contracts and atomic rejection paths."""

from __future__ import annotations

import copy
import time

import pytest

from slopsearx.adapter import SearchResult
from slopsearx.artifacts import artifact_ref, composite_artifact_id
from slopsearx.mcp import dependency_tools, lineage_tools, receipt_tools, staged_tools
from slopsearx.mcp import tools as core
from slopsearx.mcp.entity_projection import entity_groups
from slopsearx.mcp.state import set_state, tenant_scope
from slopsearx.retrieval_receipts import ReceiptStore
from slopsearx.saved_runner import SavedSearchRunner
from slopsearx.saved_store import SavedSearchStore
from slopsearx.service import ScopeDecision
from slopsearx.staged import StagedSearchRunner, StagedSearchStore
from tests.test_research_retry_followup import _build_state


@pytest.fixture
def state():
    value = _build_state(["wikipedia", "brave", "pypi", "github", "nvd"])
    value.policy.enabled_tools.update(
        {
            "saved_searches": True,
            "staged_search": True,
            "retrieval_receipts": True,
            "dependency_dossier": True,
            "security": True,
        }
    )
    value.receipt_store = ReceiptStore(value.ctx.cache)
    value.saved_store = SavedSearchStore(value.ctx.cache)
    value.saved_runner = SavedSearchRunner(
        value.service,
        value.saved_store,
        value.policy,
        policy_check=lambda definition: core._saved_policy_error(value, definition),
        policy_fingerprint=lambda definition: core._saved_policy_fingerprint(value, definition),
    )
    value.staged_store = StagedSearchStore(value.ctx.cache)
    value.staged_runner = StagedSearchRunner(
        value.service,
        value.staged_store,
        value.snapshots,
        staged_tools._policy_check,
    )
    set_state(value)
    yield value
    set_state(None)


async def _snapshot_source() -> dict:
    response = await core.slopsearx_search("composition evidence", engines=["wikipedia"])
    assert "error" not in response
    return response["meta"]["artifact"]


async def _saved_report_source(state) -> dict:
    created = await core.slopsearx_create_saved_search(
        "saved evidence",
        ["wikipedia"],
        60,
        expires_in_seconds=3600,
        start_immediately=True,
    )
    report = await state.saved_runner.run_one("default", created["search_id"])
    assert report is not None
    reports = await core.slopsearx_read_saved_search_reports(created["search_id"])
    return reports["reports"][0]["artifact"]


async def test_snapshot_to_research_is_idempotent_and_traversable(state) -> None:
    source = await _snapshot_source()
    calls = state.ctx.active_engines["wikipedia"].calls
    arguments = {
        "question": "continue from retained evidence",
        "initial_plan": [{"query": "explicit destination query", "engines": ["wikipedia"]}],
        "max_queries": 1,
        "max_attempts": 1,
        "idempotency_key": "composed-research",
        "source": source,
    }
    started = await core.slopsearx_start_research(**arguments)
    replay = await core.slopsearx_start_research(**arguments)
    assert started["source"]["artifact"] == source
    assert replay["job_id"] == started["job_id"]
    assert state.ctx.active_engines["wikipedia"].calls == calls
    lineage = await lineage_tools.slopsearx_get_artifact_lineage(started["artifact"], direction="outgoing")
    assert any(edge["to"] == source and edge["relation"] == "derived_from" for edge in lineage["edges"])

    other = await _snapshot_source()
    conflict = await core.slopsearx_start_research(**{**arguments, "source": other})
    assert conflict["error"]["code"] == "idempotency_conflict"


async def test_saved_report_composes_into_research_and_explicit_staged_plan(state) -> None:
    source = await _saved_report_source(state)
    research = await core.slopsearx_start_research(
        "explain the change",
        initial_plan=[{"query": "destination objective", "engines": ["wikipedia"]}],
        max_queries=1,
        max_attempts=1,
        source=source,
    )
    assert research["source"]["artifact"] == source

    calls = state.ctx.active_engines["wikipedia"].calls
    staged = await staged_tools.slopsearx_search_staged(
        objectives={"deadline_ms": 5000, "max_engine_calls": 1},
        initial_scope={"engines": ["wikipedia"]},
        idempotency_key="saved-to-staged",
        source=source,
    )
    assert staged["plan"]["query"] == "saved evidence"
    assert staged["source"]["artifact"] == source
    assert state.ctx.active_engines["wikipedia"].calls == calls
    conflict = await staged_tools.slopsearx_search_staged(
        query="copied query",
        objectives={"deadline_ms": 5000, "max_engine_calls": 1},
        initial_scope={"engines": ["wikipedia"]},
        idempotency_key="saved-to-staged-2",
        source=source,
    )
    assert conflict["error"]["code"] == "input_conflict"
    assert state.ctx.active_engines["wikipedia"].calls == calls


async def test_snapshot_result_and_entity_group_seed_dossiers_without_dispatch(state) -> None:
    snapshot = await _snapshot_source()
    snapshot_id = snapshot["id"]
    result = artifact_ref("result", f"{snapshot_id}:0")
    cve = SearchResult(
        url="https://nvd.nist.gov/vuln/detail/CVE-2026-1234",
        title="CVE-2026-1234",
        content="retained source evidence",
        engine="nvd",
        payload={
            "schema_version": 1,
            "domain": "security",
            "type": "vulnerability",
            "data": {"cve_id": "CVE-2026-1234"},
            "provenance": {"engine": "nvd", "adapter_fields": ["cve_id"], "inferred_fields": []},
        },
    )
    entity_snapshot_id = await state.snapshots.create(
        "entity seed",
        "query-entity",
        [cve],
        ScopeDecision(selected_engines=["nvd"], resolved_categories=[], routing_rule="explicit"),
    )
    assert entity_snapshot_id is not None
    read = await state.snapshots.read(entity_snapshot_id)
    group = entity_groups(read.snapshot)[0]
    entity = artifact_ref("entity_group", composite_artifact_id(entity_snapshot_id, group["entity_id"]))
    calls = {name: engine.calls for name, engine in state.ctx.active_engines.items()}
    for index, source in enumerate((snapshot, result, entity)):
        dossier = await dependency_tools.slopsearx_start_dependency_dossier(
            "pypi", "requests", idempotency_key=f"seed-{index}", source=source
        )
        assert dossier["source"]["artifact"] == source
    assert {name: engine.calls for name, engine in state.ctx.active_engines.items()} == calls


async def test_staged_and_research_sources_export_bounded_manifests(state) -> None:
    staged = await staged_tools.slopsearx_search_staged(
        "staged source",
        {"deadline_ms": 5000, "max_engine_calls": 1},
        {"engines": ["wikipedia"]},
        "staged-source",
    )
    await state.staged_runner.run_one("default", staged["operation_id"])
    staged_manifest = await receipt_tools.slopsearx_export_research_manifest(source=staged["artifact"])
    assert staged_manifest["source"]["artifact"] == staged["artifact"]
    assert len(staged_manifest["items"]) == 3

    research = await core.slopsearx_start_research(
        "manifest source",
        initial_plan=[{"query": "manifest source", "engines": ["wikipedia"]}],
        max_queries=1,
        max_attempts=1,
    )
    job = await state.job_store.load(research["job_id"])
    await state.runner.run_direct(job)
    completed = await core.slopsearx_get_job(research["job_id"])
    attempt = completed["queries"][0]["attempts"][0]["artifact"]
    for source in (attempt, completed["artifact"]):
        manifest = await receipt_tools.slopsearx_export_research_manifest(source=source)
        assert manifest["source"]["artifact"] == source
        assert manifest["items"]
        assert any(edge["relation"] == "derived_from" and edge["to"] == source for edge in manifest["lineage"])


async def test_source_lifecycle_policy_and_contract_fail_closed_before_dispatch(state) -> None:
    source = await _snapshot_source()
    calls = state.ctx.active_engines["wikipedia"].calls
    before = copy.deepcopy(state.job_store._store._data)
    malformed = await core.slopsearx_start_research(
        "q",
        initial_plan=[{"query": "q", "engines": ["wikipedia"]}],
        source={**source, "version": 0},
    )
    unsupported = await dependency_tools.slopsearx_start_dependency_dossier(
        "pypi", "requests", source=artifact_ref("research_job", "unknown")
    )
    with tenant_scope("other"):
        missing = await core.slopsearx_start_research(
            "q",
            initial_plan=[{"query": "q", "engines": ["wikipedia"]}],
            source=source,
        )
    state.policy.sensitive_engines.add("wikipedia")
    denied = await core.slopsearx_start_research(
        "q",
        initial_plan=[{"query": "q", "engines": ["wikipedia"]}],
        source=source,
    )
    assert malformed["error"]["code"] == "unsupported_source_contract"
    assert unsupported["error"]["code"] == "unsupported_transition"
    assert missing["error"]["code"] == "source_not_found"
    assert denied["error"]["code"] == "source_policy_denied"
    assert state.ctx.active_engines["wikipedia"].calls == calls
    assert state.job_store._store._data == before

    state.policy.sensitive_engines.clear()
    key = next(key for key in state.snapshots._store._data if key.endswith(source["id"]))
    state.snapshots._store._data[key]["expires_at"] = time.time() - 1
    expired = await core.slopsearx_start_research(
        "q",
        initial_plan=[{"query": "q", "engines": ["wikipedia"]}],
        source=source,
    )
    assert expired["error"]["code"] == "source_expired"


async def test_partial_incomparable_truncated_and_conflicting_sources_are_explicit(state) -> None:
    staged = await staged_tools.slopsearx_search_staged(
        "not executed",
        {"deadline_ms": 5000, "max_engine_calls": 1},
        {"engines": ["wikipedia"]},
        "partial-source",
    )
    calls = state.ctx.active_engines["wikipedia"].calls
    partial = await receipt_tools.slopsearx_export_research_manifest(source=staged["artifact"])
    assert partial["error"]["code"] == "source_partial"
    assert state.ctx.active_engines["wikipedia"].calls == calls

    saved = await _saved_report_source(state)
    report_key = next(key for key in state.ctx.cache._data if key.startswith("mcp:saved:v1:report:"))
    report = state.ctx.cache._data[report_key]
    report["status"] = "incomparable"
    report["incomparable_reasons"] = ["dispatch_failed"]
    incomparable = await core.slopsearx_start_research(
        "q",
        initial_plan=[{"query": "q", "engines": ["wikipedia"]}],
        source=saved,
    )
    assert incomparable["error"]["code"] == "source_incomparable"
    report["incomparable_reasons"] = ["observation_truncated"]
    truncated = await core.slopsearx_start_research(
        "q",
        initial_plan=[{"query": "q", "engines": ["wikipedia"]}],
        source=saved,
    )
    assert truncated["error"]["code"] == "source_truncated"

    snapshot = await _snapshot_source()
    conflict = await receipt_tools.slopsearx_export_research_manifest(
        result_ids=[f"{snapshot['id']}:0"],
        source=artifact_ref("staged_search", staged["operation_id"]),
    )
    assert conflict["error"]["code"] == "input_conflict"


async def test_research_rejects_truncated_snapshot_and_staged_snapshot_sources(state) -> None:
    results = [
        SearchResult(
            url=f"https://example.test/{index}",
            title=f"Evidence {index}",
            content="retained evidence",
            engine="wikipedia",
        )
        for index in range(26)
    ]
    snapshot_id = await state.snapshots.create(
        "bounded evidence",
        "query-bounded",
        results,
        ScopeDecision(selected_engines=["wikipedia"], resolved_categories=[], routing_rule="explicit"),
    )
    assert snapshot_id is not None
    calls = state.ctx.active_engines["wikipedia"].calls
    jobs_before = copy.deepcopy(state.job_store._store._data)
    truncated_snapshot = await core.slopsearx_start_research(
        "continue",
        initial_plan=[{"query": "continue", "engines": ["wikipedia"]}],
        source=artifact_ref("snapshot", snapshot_id),
    )
    assert truncated_snapshot["error"]["code"] == "source_truncated"
    assert state.job_store._store._data == jobs_before

    staged = await staged_tools.slopsearx_search_staged(
        "staged evidence",
        {"deadline_ms": 5000, "max_engine_calls": 1},
        {"engines": ["wikipedia"]},
        "truncated-staged-source",
    )
    await state.staged_runner.run_one("default", staged["operation_id"])
    read = await state.staged_store.read("default", staged["operation_id"])
    assert read.record is not None
    attempt = read.record["stages"][0]["attempts"][0]
    key = next(key for key in state.snapshots._store._data if key.endswith(str(attempt["cursor"])))
    stored_snapshot = state.snapshots._store._data[key]
    stored_snapshot["results"] = [copy.deepcopy(stored_snapshot["results"][0]) for _ in range(26)]
    stored_snapshot["total"] = 26
    calls_after_staged = state.ctx.active_engines["wikipedia"].calls
    jobs_before_staged_research = copy.deepcopy(state.job_store._store._data)
    truncated_staged = await core.slopsearx_start_research(
        "continue staged",
        initial_plan=[{"query": "continue staged", "engines": ["wikipedia"]}],
        source=staged["artifact"],
    )
    assert truncated_staged["error"]["code"] == "source_truncated"
    assert state.ctx.active_engines["wikipedia"].calls == calls_after_staged
    assert calls_after_staged == calls + 1
    assert state.job_store._store._data == jobs_before_staged_research
