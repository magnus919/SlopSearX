"""Artifact identity and bounded lineage contracts."""

from __future__ import annotations

import copy

import pytest

from slopsearx.artifacts import (
    ARTIFACT_CONTRACT,
    artifact_ref,
    composite_artifact_id,
    parse_artifact_ref,
    parse_composite_artifact_id,
)
from slopsearx.mcp import lineage_tools, receipt_tools
from slopsearx.mcp import tools as core
from slopsearx.mcp.state import set_state, tenant_scope
from slopsearx.retrieval_receipts import ReceiptStore
from tests.test_research_retry_followup import _build_state


@pytest.fixture
def state():
    value = _build_state(["wikipedia"])
    value.policy.enabled_tools["research"] = True
    value.policy.enabled_tools["retrieval_receipts"] = True
    value.receipt_store = ReceiptStore(value.ctx.cache)
    set_state(value)
    yield value
    set_state(None)


def test_artifact_reference_and_composite_ids_are_strict_and_round_trip() -> None:
    ref = artifact_ref("research_attempt", composite_artifact_id("job:1", "attempt#2"))
    assert ref["contract"] == ARTIFACT_CONTRACT
    assert parse_artifact_ref(ref) == ref
    assert parse_composite_artifact_id(ref["id"], 2) == ("job:1", "attempt#2")
    with pytest.raises(ValueError):
        parse_artifact_ref({**ref, "extra": True})
    with pytest.raises(ValueError):
        artifact_ref("unknown", "id")


async def test_search_snapshot_and_results_emit_bounded_lineage_without_dispatch(state) -> None:
    search = await core.slopsearx_search("lineage", engines=["wikipedia"])
    snapshot_ref = search["meta"]["artifact"]
    assert snapshot_ref == artifact_ref("snapshot", search["meta"]["cursor"])
    assert search["results"][0]["artifact"]["kind"] == "result"
    calls = state.ctx.active_engines["wikipedia"].calls

    graph = await lineage_tools.slopsearx_get_artifact_lineage(
        snapshot_ref,
        direction="incoming",
        max_depth=1,
        max_nodes=2,
    )

    assert graph["contract"] == "slopsearx.artifact_lineage"
    assert graph["counts"]["nodes"] == 2
    assert graph["truncated"] is True
    assert {node["artifact"]["kind"] for node in graph["nodes"]} == {"snapshot", "result"}
    node_keys = {(node["artifact"]["kind"], node["artifact"]["id"]) for node in graph["nodes"]}
    assert all(
        (edge[side]["kind"], edge[side]["id"]) in node_keys for edge in graph["edges"] for side in ("from", "to")
    )
    assert state.ctx.active_engines["wikipedia"].calls == calls


async def test_research_attempt_snapshot_lineage_survives_serialization(state) -> None:
    started = await core.slopsearx_start_research(
        "investigate",
        max_queries=1,
        max_attempts=1,
        initial_plan=[{"query": "evidence", "engines": ["wikipedia"]}],
    )
    job = await state.job_store.load(started["job_id"])
    assert job is not None
    await state.runner.run_direct(job)
    summary = await core.slopsearx_get_job(job.job_id)
    attempt = summary["queries"][0]["attempts"][0]
    assert summary["artifact"] == artifact_ref("research_job", job.job_id)
    assert attempt["artifact"]["kind"] == "research_attempt"
    assert attempt["snapshot_artifact"]["kind"] == "snapshot"

    graph = await lineage_tools.slopsearx_get_artifact_lineage(summary["artifact"], direction="incoming", max_depth=2)
    kinds = {node["artifact"]["kind"] for node in graph["nodes"]}
    assert {"research_job", "research_attempt", "snapshot"} <= kinds
    snapshot = await state.snapshots.read(summary["queries"][0]["cursor"])
    assert snapshot.snapshot is not None
    assert snapshot.snapshot.lineage[0]["to"] == attempt["artifact"]


async def test_receipt_and_manifest_emit_lineage_and_recheck_policy(state) -> None:
    search = await core.slopsearx_search("receipt", engines=["wikipedia"])
    result_id = search["results"][0]["result_id"]
    submitted = await receipt_tools.slopsearx_submit_retrieval_receipt(
        result_id,
        "reader",
        "capture-1",
        "succeeded",
        captured_at="2026-09-11T12:00:00Z",
        capture_ref="capture://one",
    )
    receipt_ref = submitted["receipt"]["artifact"]
    assert submitted["receipt"]["lineage"][0]["to"] == artifact_ref("result", result_id)
    manifest = await receipt_tools.slopsearx_export_research_manifest([result_id])
    assert manifest["artifact"]["kind"] == "research_manifest"
    assert manifest["lineage"][0]["to"] == artifact_ref("result", result_id)

    graph = await lineage_tools.slopsearx_get_artifact_lineage(receipt_ref, direction="outgoing")
    assert [node["artifact"]["kind"] for node in graph["nodes"][:2]] == [
        "retrieval_receipt",
        "result",
    ]
    state.policy.sensitive_engines.add("wikipedia")
    denied = await lineage_tools.slopsearx_get_artifact_lineage(receipt_ref)
    assert denied["nodes"][0]["status"] == "policy_denied"


async def test_tenant_isolation_and_invalid_bounds_do_not_disclose_or_mutate(state) -> None:
    search = await core.slopsearx_search("private", engines=["wikipedia"])
    snapshot_ref = search["meta"]["artifact"]
    before = copy.deepcopy(state.ctx.cache._data)
    with tenant_scope("other"):
        graph = await lineage_tools.slopsearx_get_artifact_lineage(snapshot_ref)
    assert graph["nodes"] == [
        {
            "artifact": snapshot_ref,
            "status": "missing",
            "created_at": None,
            "expires_at": None,
        }
    ]
    assert (await lineage_tools.slopsearx_get_artifact_lineage(snapshot_ref, max_depth=6))["error"][
        "code"
    ] == "invalid_input"
    assert (await lineage_tools.slopsearx_get_artifact_lineage(snapshot_ref, max_nodes=True))["error"][
        "code"
    ] == "invalid_input"
    assert state.ctx.cache._data == before


async def test_corrupt_snapshot_lineage_is_ignored_and_order_is_deterministic(state) -> None:
    search = await core.slopsearx_search("ordered", engines=["wikipedia"])
    snapshot_id = search["meta"]["cursor"]
    read = await state.snapshots.read(snapshot_id)
    assert read.snapshot is not None
    store = state.snapshots._store
    assert store is not None
    key = state.snapshots._key(snapshot_id)
    payload = await store.get(key)
    assert payload is not None
    payload["lineage"] = [
        {"from": {"invalid": True}, "relation": "derived_from", "to": search["meta"]["artifact"]},
        {
            "from": artifact_ref("snapshot", snapshot_id),
            "relation": "derived_from",
            "to": artifact_ref("research_job", "parent"),
        },
    ]
    await store.set(key, payload, ttl=state.snapshots.store_ttl_seconds)

    first = await lineage_tools.slopsearx_get_artifact_lineage(search["meta"]["artifact"], max_depth=1)
    second = await lineage_tools.slopsearx_get_artifact_lineage(search["meta"]["artifact"], max_depth=1)
    assert first == second
    assert all(edge["from"].get("contract") == ARTIFACT_CONTRACT for edge in first["edges"])


async def test_legacy_snapshot_reports_explicit_lineage_gap(state) -> None:
    search = await core.slopsearx_search("legacy", engines=["wikipedia"])
    snapshot_id = search["meta"]["cursor"]
    store = state.snapshots._store
    assert store is not None
    key = state.snapshots._key(snapshot_id)
    payload = await store.get(key)
    assert payload is not None
    payload.pop("lineage")
    await store.set(key, payload, ttl=state.snapshots.store_ttl_seconds)

    graph = await lineage_tools.slopsearx_get_artifact_lineage(search["meta"]["artifact"], max_depth=0)
    assert graph["nodes"][0]["details"]["lineage_status"] == "lineage_unavailable"


async def test_manifest_accepts_bounded_artifact_selection_without_dispatch(state) -> None:
    search = await core.slopsearx_search("manifest graph", engines=["wikipedia"])
    calls = state.ctx.active_engines["wikipedia"].calls
    manifest = await receipt_tools.slopsearx_export_research_manifest(
        artifacts=[search["meta"]["artifact"]],
        max_depth=1,
        max_nodes=3,
    )
    assert manifest["lineage_cuts"][0]["root"] == search["meta"]["artifact"]
    assert len(manifest["items"]) == 2
    assert state.ctx.active_engines["wikipedia"].calls == calls

    explicit = [search["results"][0]["result_id"]]
    await receipt_tools.slopsearx_export_research_manifest(
        result_ids=explicit,
        artifacts=[search["meta"]["artifact"]],
        max_depth=1,
        max_nodes=2,
    )
    assert explicit == [search["results"][0]["result_id"]]


async def test_cycles_duplicate_edges_and_expired_nodes_are_bounded_and_deterministic(
    state, monkeypatch: pytest.MonkeyPatch
) -> None:
    root = artifact_ref("snapshot", "root")
    child = artifact_ref("result", "root:0")
    expired = artifact_ref("snapshot", "expired")
    root_edge = {"from": child, "relation": "derived_from", "to": root}

    async def resolve(ref):
        if ref == root:
            return lineage_tools._node(ref, "live"), [
                root_edge,
                root_edge,
                {"from": expired, "relation": "derived_from", "to": root},
            ]
        if ref == child:
            return lineage_tools._node(ref, "live"), [{"from": root, "relation": "contains", "to": child}]
        return lineage_tools._node(ref, "expired", expires_at=1.0), []

    monkeypatch.setattr(lineage_tools, "_resolve", resolve)
    first = await lineage_tools.slopsearx_get_artifact_lineage(root, max_depth=5)
    second = await lineage_tools.slopsearx_get_artifact_lineage(root, max_depth=5)
    assert first == second
    assert first["counts"] == {"nodes": 3, "edges": 3}
    assert [node["status"] for node in first["nodes"]] == ["live", "live", "expired"]
