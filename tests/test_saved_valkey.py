"""Distributed saved-search coordination against an isolated real Valkey."""

from __future__ import annotations

import asyncio
import os
import uuid

import pytest
import valkey.asyncio as valkey

from slopsearx import saved_store as storage
from slopsearx.mcp import tools as mcp_tools
from slopsearx.saved_models import SavedDefinition
from slopsearx.saved_runner import SavedSearchRunner
from slopsearx.saved_store import RevisionConflictError, SavedSearchStore
from tests.test_research_queue_integration import Store
from tests.test_research_retry_followup import _build_state

pytestmark = pytest.mark.skipif(not os.environ.get("SLOPSEARX_TEST_VALKEY_URL"), reason="explicit test Valkey required")


@pytest.fixture
async def backend(monkeypatch):
    prefix = f"test-saved-{uuid.uuid4().hex}"
    monkeypatch.setattr(storage, "PREFIX", prefix)
    client = valkey.from_url(os.environ["SLOPSEARX_TEST_VALKEY_URL"])
    await client.ping()
    store = Store(client)
    try:
        yield store
    finally:
        async for key in client.scan_iter(match=f"{prefix}:*"):
            await client.delete(key)
        await client.aclose()


def definition(identity: str, *, now: float = 1000, due: float = 1000) -> SavedDefinition:
    return SavedDefinition(
        identity, "tenant", "query", ["wikipedia"], 60, 600, 10, 2, now, now + 600, due, policy_fingerprint="policy"
    )


def report(identity: str, slot: int, now: float) -> dict:
    return {"run_id": identity, "slot": slot, "finished_at": now, "expires_at": now + 500, "events": []}


async def test_atomic_quota_and_revision_conflict(backend):
    store = SavedSearchStore(backend, "tenant")
    first, second = definition("saved-first"), definition("saved-second")
    outcomes = await asyncio.gather(store.create(first, 1, now=1000), store.create(second, 1, now=1000))
    assert sorted(outcomes) == ["created", "quota_exceeded"]
    winner = first if outcomes[0] == "created" else second
    a = await store.load(winner.search_id, now=1000)
    b = await store.load(winner.search_id, now=1000)
    assert a and b
    a.query = "first update"
    await store.compare_and_set(a, 1, now=1001)
    b.query = "lost update"
    with pytest.raises(RevisionConflictError) as conflict:
        await store.compare_and_set(b, 1, now=1001)
    assert conflict.value.current_revision == 2
    assert (await store.load(winner.search_id, now=1001)).query == "first update"


async def test_atomic_quota_uses_authoritative_records_when_index_is_missing(backend):
    store = SavedSearchStore(backend, "tenant")
    assert await store.create(definition("saved-first"), 1, now=1000) == "created"
    await backend._client.delete(store._definition_index())
    assert await store.create(definition("saved-second"), 1, now=1000) == "quota_exceeded"


async def test_authoritative_quota_scan_treats_tenant_glob_characters_literally(backend):
    tenant = "tenant*[]?"
    store = SavedSearchStore(backend, tenant)
    first = definition("saved-first")
    first.tenant = tenant
    second = definition("saved-second")
    second.tenant = tenant
    assert await store.create(first, 1, now=1000) == "created"
    await backend._client.delete(store._definition_index())
    assert await store.create(second, 1, now=1000) == "quota_exceeded"


async def test_quota_reconciliation_fails_closed_when_one_bounded_scan_cannot_finish(backend):
    store = SavedSearchStore(backend, "tenant")
    assert await store.create(definition("saved-first"), 2, now=1000) == "created"
    await backend._client.delete(store._definition_index(), store._definition_count(), store._tenant_index())
    for index in range(1000):
        await backend._client.set(f"{storage.PREFIX}:noise:{index}", "x", ex=900)
    assert await store.create(definition("saved-second"), 2, now=1000) == "unavailable"


async def test_delete_never_reconstructs_authoritative_count_from_missing_index(backend):
    store = SavedSearchStore(backend, "tenant")
    first, second, third = definition("saved-first"), definition("saved-second"), definition("saved-third")
    assert await store.create(first, 2, now=1000) == "created"
    assert await store.create(second, 2, now=1000) == "created"
    await backend._client.delete(store._definition_index())
    await store.delete(first.search_id, 1, now=1001)
    assert await backend._client.exists(store._definition_count()) == 0
    assert await store.create(third, 1, now=1001) == "quota_exceeded"


async def test_quota_reconciliation_fails_closed_on_malformed_authoritative_record(backend):
    store = SavedSearchStore(backend, "tenant")
    await backend._client.set(f"{storage.PREFIX}:definition:tenant:saved-corrupt", "not-json", ex=900)
    assert await store.create(definition("saved-new"), 2, now=1000) == "unavailable"


async def test_duplicate_claim_stale_owner_and_bounded_reports(backend):
    store = SavedSearchStore(backend, "tenant")
    item = definition("saved-race")
    assert await store.for_tenant("tenant").create(item, 20, now=1000) == "created"
    claims = await asyncio.gather(*(store.claim(item.search_id, now=1000) for _ in range(8)))
    owners = [candidate for candidate in claims if candidate]
    assert len(owners) == 1
    stale = owners[0]
    await backend._client.delete(store._lease_key(item.search_id))
    replacement = await store.claim(item.search_id, now=1001)
    assert replacement and replacement.lease_token != stale.lease_token
    assert not await store.commit(
        stale, report("run-stale", 1, 1001), None, now=1001, next_due=1060, policy_fingerprint="policy"
    )
    assert await store.commit(
        replacement,
        report("run-authority", 1, 1001),
        {"window": {}, "records": {}, "observed_at": 1001, "expires_at": 1501},
        now=1001,
        next_due=1060,
        policy_fingerprint="policy",
    )
    assert [entry["run_id"] for entry in await store.reports(item.search_id, now=1002)] == ["run-authority"]
    for slot in (2, 3):
        claimed = await store.claim(item.search_id, now=1000 + slot * 60)
        assert claimed
        assert await store.commit(
            claimed,
            report(f"run-{slot}", slot, 1000 + slot * 60),
            None,
            now=1000 + slot * 60,
            next_due=1000 + (slot + 1) * 60,
            policy_fingerprint="policy",
        )
    assert [entry["run_id"] for entry in await store.reports(item.search_id, now=1190)] == ["run-3", "run-2"]
    assert await backend._client.exists(store._report_key(item.search_id, "run-authority")) == 0


async def test_report_order_and_retention_use_completion_time_when_expiry_ties(backend):
    store = SavedSearchStore(backend, "tenant")
    item = definition("saved-order")
    await store.create(item, 20, now=1000)
    runs = (("run-z-old", 1, 1010), ("run-a-new", 2, 1070), ("run-m-newest", 3, 1130))
    for run_id, slot, finished_at in runs:
        claimed = await store.claim(item.search_id, now=1000 + (slot - 1) * 60)
        assert claimed
        value = report(run_id, slot, finished_at)
        value["expires_at"] = 1500
        assert await store.commit(
            claimed,
            value,
            None,
            now=finished_at,
            next_due=1000 + slot * 60,
            policy_fingerprint="policy",
        )
    assert [entry["run_id"] for entry in await store.reports(item.search_id, now=1140)] == [
        "run-m-newest",
        "run-a-new",
    ]
    assert await backend._client.exists(store._report_key(item.search_id, "run-z-old")) == 0


@pytest.mark.parametrize("mutation", ["pause", "delete"])
async def test_mutation_during_claim_fences_late_commit(backend, mutation):
    store = SavedSearchStore(backend, "tenant")
    item = definition(f"saved-{mutation}")
    await store.create(item, 20, now=1000)
    owner = await store.claim(item.search_id, now=1000)
    current = await store.load(item.search_id, now=1000)
    assert owner and current
    if mutation == "pause":
        current.paused = True
        await store.compare_and_set(current, 1, now=1001)
    else:
        await store.delete(item.search_id, 1, now=1001)
    assert not await store.commit(
        owner, report("run-late", 1, 1002), None, now=1002, next_due=1060, policy_fingerprint="policy"
    )
    assert await store.reports(item.search_id, now=1002) == []
    if mutation == "delete":
        assert await store.load(item.search_id, now=1002) is None


async def test_index_repair_and_physical_ttl_do_not_extend_lifetime(backend):
    store = SavedSearchStore(backend, "tenant")
    item = definition("saved-repair")
    await store.create(item, 20, now=1000)
    ttl = await backend._client.ttl(store._definition_key(item.search_id))
    assert 899 <= ttl <= 900
    await backend._client.delete(store._due_index())
    assert await store.due_ids(1000) == []
    assert await store.repair_indexes(now=1000) == 1
    assert await store.due_ids(1000) == [item.search_id]
    assert await store.scan_tenants(now=1000) == ["tenant"]
    assert await store.load(item.search_id, now=item.expires_at) is None


async def test_real_valkey_runner_hydrates_baseline_and_emits_delta(backend):
    now = [1000.0]
    state = _build_state(["wikipedia"])
    state.ctx.cache = backend
    state.policy.enabled_tools["saved_searches"] = True
    store = SavedSearchStore(backend)
    runner = SavedSearchRunner(
        state.service,
        store,
        state.policy,
        policy_check=lambda item: mcp_tools._saved_policy_error(state, item),
        policy_fingerprint=lambda item: mcp_tools._saved_policy_fingerprint(state, item),
        clock=lambda: now[0],
    )
    item = definition("saved-real-runner", now=1000, due=1060)
    item.retention_seconds = 500
    item.expires_at = 2000
    item.policy_fingerprint = mcp_tools._saved_policy_fingerprint(state, item)
    assert await store.for_tenant("tenant").create(item, 20, now=1000) == "created"
    now[0] = 1060
    first = await runner.run_one("tenant", item.search_id, now=1060)
    assert first and first["status"] == "baseline_initialized"
    state.ctx.active_engines["wikipedia"]._count = 4
    now[0] = 1120
    second = await runner.run_one("tenant", item.search_id, now=1120)
    assert second and {event["kind"] for event in second["events"]} == {"added"}


async def test_reclaim_keeps_persisted_schedule_identity(backend):
    store = SavedSearchStore(backend, "tenant")
    item = definition("saved-retry", now=1000, due=1060)
    await store.create(item, 20, now=1000)
    first = await store.claim(item.search_id, now=1060)
    assert first
    identity = (first.revision, first.next_due)
    await backend._client.delete(store._lease_key(item.search_id))
    retry = await store.claim(item.search_id, now=1180)
    assert retry and (retry.revision, retry.next_due) == identity


async def test_due_cleanup_and_tenant_expiry_are_bounded(backend):
    store = SavedSearchStore(backend, "tenant")
    item = definition("saved-after-stale", now=1000, due=1000)
    await store.create(item, 20, now=1000)
    stale = {f"saved-stale-{index}": 1 for index in range(128)}
    await backend._client.zadd(store._due_index(), stale)
    assert await store.due_ids(1000, limit=128) == []
    assert await store.due_ids(1000, limit=128) == [item.search_id]
    assert await store.scan_tenants(now=1000) == ["tenant"]
    assert await store.scan_tenants(now=item.expires_at) == []


async def test_authoritative_tenant_scan_repairs_missing_hint_with_continuation(backend):
    store = SavedSearchStore(backend)
    for index in range(140):
        tenant = f"tenant-{index:03d}"
        item = definition(f"saved-{index:03d}", now=1000, due=1060)
        item.tenant = tenant
        assert await store.for_tenant(tenant).create(item, 20, now=1000) == "created"
    await backend._client.delete(store._tenant_index())
    discovered: set[str] = set()
    for _ in range(200):
        discovered.update(await store.scan_tenants(limit=16, now=1000))
        if store._tenant_scan_cursor == 0:
            break
    assert len(discovered) == 140
    assert await backend._client.zcard(store._tenant_index()) == 140
