"""Retrieval receipt contracts, attribution, isolation and source lifecycle."""

from __future__ import annotations

import copy

import pytest

from slopsearx.mcp import receipt_tools as receipts
from slopsearx.mcp import tools as search_tools
from slopsearx.mcp.state import set_state, tenant_scope
from slopsearx.retrieval_receipts import ReceiptStore
from tests.test_research_retry_followup import _build_state


@pytest.fixture
def state():
    value = _build_state(["wikipedia"])
    value.policy.enabled_tools["retrieval_receipts"] = True
    value.receipt_store = ReceiptStore(value.ctx.cache)
    set_state(value)
    yield value
    set_state(None)


async def result_id() -> str:
    result = await search_tools.slopsearx_search("receipt source", engines=["wikipedia"])
    return result["results"][0]["result_id"]


def success_args(identifier: str) -> dict:
    return {
        "result_id": identifier,
        "retriever": "test-reader",
        "idempotency_key": "capture-1",
        "status": "succeeded",
        "captured_at": "2026-09-11T04:00:00Z",
        "final_url": "https://captured.example/final",
        "content_sha256": "a" * 64,
        "capture_ref": "capture://opaque/1",
        "passage_refs": [{"ref": "passage-1", "label": "supporting text"}],
    }


async def test_success_replay_conflict_failure_and_manifest(state):
    identifier = await result_id()
    snapshot_before = copy.deepcopy(state.ctx.cache._data)
    first = await receipts.slopsearx_submit_retrieval_receipt(**success_args(identifier))
    replay = await receipts.slopsearx_submit_retrieval_receipt(**success_args(identifier))
    conflict_args = success_args(identifier)
    conflict_args["final_url"] = "https://captured.example/different"
    conflict = await receipts.slopsearx_submit_retrieval_receipt(**conflict_args)
    failed = await receipts.slopsearx_submit_retrieval_receipt(
        identifier,
        "test-reader",
        "capture-2",
        "failed",
        failure_code="blocked",
        failure_message="javascript:alert(1) is inert text",
    )
    assert first["state"] == "created"
    assert replay["state"] == "replayed"
    assert replay["receipt"]["receipt_id"] == first["receipt"]["receipt_id"]
    assert conflict["error"]["code"] == "idempotency_conflict"
    assert failed["state"] == "created"
    read = await receipts.slopsearx_read_retrieval_receipts(identifier, 1)
    assert (read["total"], read["returned"], read["has_more"]) == (2, 1, True)
    manifest = await receipts.slopsearx_export_research_manifest([identifier])
    assert manifest["contract"] == "slopsearx.research_manifest"
    assert manifest["observations_verified"] is False
    assert {item["observation"]["status"] for item in manifest["items"][0]["receipts"]} == {
        "succeeded",
        "failed",
    }
    for key, value in snapshot_before.items():
        if key.startswith("mcp:snapshot:"):
            assert state.ctx.cache._data[key] == value


async def test_tenant_grant_and_unknown_handle(state):
    identifier = await result_id()
    with tenant_scope("other"):
        denied = await receipts.slopsearx_submit_retrieval_receipt(**success_args(identifier))
        assert denied["error"]["code"] == "invalid_result_id"
    state.policy.enabled_tools["retrieval_receipts"] = False
    assert (await receipts.slopsearx_read_retrieval_receipts(identifier))["error"]["code"] == "tool_disabled"


@pytest.mark.parametrize(
    "changes",
    [
        {"status": "succeeded", "failure_code": "bad"},
        {"status": "failed", "failure_code": None},
        {"status": "succeeded", "captured_at": "nope"},
        {"status": "succeeded", "content_sha256": "bad"},
        {"status": "succeeded", "passage_refs": [{"ref": "x", "extra": "no"}]},
    ],
)
async def test_contradictory_and_malformed_receipts_are_atomic(state, changes):
    identifier = await result_id()
    args = success_args(identifier)
    args.update(changes)
    before = copy.deepcopy(state.ctx.cache._data)
    outcome = await receipts.slopsearx_submit_retrieval_receipt(**args)
    assert outcome["error"]["code"] == "invalid_input"
    assert state.ctx.cache._data == before


async def test_identical_replay_survives_source_expiry_but_new_receipt_does_not(state, monkeypatch):
    identifier = await result_id()
    snapshot_id = identifier.rsplit(":", 1)[0]
    source = await state.snapshots.read(snapshot_id)
    assert source.snapshot and source.snapshot.expires_at
    clock = [source.snapshot.created_at]
    monkeypatch.setattr(receipts.time, "time", lambda: clock[0])
    import slopsearx.snapshot as snapshot_module

    monkeypatch.setattr(snapshot_module.time, "time", lambda: clock[0])
    first = await receipts.slopsearx_submit_retrieval_receipt(**success_args(identifier))
    assert first["state"] == "created"
    clock[0] = source.snapshot.expires_at + 1
    replay = await receipts.slopsearx_submit_retrieval_receipt(**success_args(identifier))
    assert replay["state"] == "replayed"
    new_args = success_args(identifier)
    new_args["idempotency_key"] = "capture-new"
    assert (await receipts.slopsearx_submit_retrieval_receipt(**new_args))["error"]["code"] == "expired_handle"
    retained = await receipts.slopsearx_read_retrieval_receipts(identifier)
    assert retained["source_snapshot_status"] == "expired"


async def test_receipt_paths_never_dispatch_search(state, monkeypatch):
    identifier = await result_id()

    async def forbidden_dispatch(*args, **kwargs):
        raise AssertionError("receipt paths must not dispatch engines or fetch URLs")

    monkeypatch.setattr(state.service, "search", forbidden_dispatch)
    assert (await receipts.slopsearx_submit_retrieval_receipt(**success_args(identifier)))["state"] == "created"
    assert (await receipts.slopsearx_read_retrieval_receipts(identifier))["total"] == 1
    assert (await receipts.slopsearx_export_research_manifest([identifier]))["items"][0]["total"] == 1


async def test_manifest_rejects_duplicates_and_preserves_per_item_identity(state):
    identifier = await result_id()
    empty = await receipts.slopsearx_export_research_manifest([identifier])
    assert empty["items"][0]["discovery"]["result_id"] == identifier
    assert empty["items"][0]["discovery"]["verified"] is False
    duplicate = await receipts.slopsearx_export_research_manifest([identifier, identifier])
    assert duplicate["error"]["code"] == "invalid_input"
    manifest = await receipts.slopsearx_export_research_manifest([identifier, "unknown"])
    assert manifest["items"][1]["result_id"] == "unknown"
    assert manifest["items"][1]["error"]["code"] == "invalid_result_id"


async def test_logical_bundle_expiry_allows_new_bundle(state):
    identifier = await result_id()
    store = state.receipt_store
    body = {"value": "one"}
    created, first = await store.submit(identifier, "key", "digest", body, now=1000)
    assert created == "created"
    assert await store.read(identifier, now=first["expires_at"] + 1) == ([], 0)
    renewed, second = await store.submit(identifier, "key", "new-digest", body, now=first["expires_at"] + 1)
    assert renewed == "created"
    assert second["expires_at"] > first["expires_at"]


async def test_complete_persisted_receipt_obeys_size_cap(state):
    store = state.receipt_store
    status, value = await store.submit("result", "key", "digest", {"large": "x" * (32 * 1024)}, now=1000)
    assert status == "size"
    assert value is None
    assert await store.read("result", now=1000) == ([], 0)
