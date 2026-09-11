"""Atomic saved-search event outbox behavior against an isolated real Valkey."""

from __future__ import annotations

import asyncio
import os
import time
import uuid

import pytest
import valkey.asyncio as valkey

from slopsearx import saved_store as storage
from slopsearx.saved_events import definition_event, event_for_report
from slopsearx.saved_models import SavedDefinition
from slopsearx.saved_store import OutboxCapacityError, SavedSearchStore
from tests.test_research_queue_integration import Store

pytestmark = pytest.mark.skipif(not os.environ.get("SLOPSEARX_TEST_VALKEY_URL"), reason="explicit test Valkey required")


@pytest.fixture
async def backend(monkeypatch):
    prefix = f"test-saved-events-{uuid.uuid4().hex}"
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


def _definition(now: float) -> SavedDefinition:
    return SavedDefinition(
        "saved-events",
        "tenant",
        "query",
        ["wikipedia"],
        60,
        600,
        10,
        10,
        now,
        now + 600,
        now,
        policy_fingerprint="policy",
    )


def _report(identity: str, slot: int, now: float) -> dict:
    return {
        "run_id": identity,
        "slot": slot,
        "scheduled_at": now,
        "started_at": now,
        "finished_at": now,
        "missed_slots": 0,
        "status": "compared",
        "incomparable_reasons": [],
        "observation": {"coverage": {"wikipedia": "ok"}, "discovered_count": 1},
        "events": [],
        "expires_at": now + 500,
    }


async def _commit(store: SavedSearchStore, definition: SavedDefinition, identity: str, now: float) -> bool:
    claimed = await store.claim(definition.search_id, now=now)
    assert claimed
    report = _report(identity, int((now - definition.created_at) // 60), now)
    return await store.commit(
        claimed,
        report,
        None,
        now=now,
        next_due=now + 60,
        policy_fingerprint="policy",
        event=event_for_report(claimed, report),
    )


async def test_competing_workers_publish_exactly_one_report_event_pair(backend):
    now = time.time()
    store = SavedSearchStore(backend, "tenant")
    definition = _definition(now)
    assert await store.create(definition, 20, now=now) == "created"
    claims = await asyncio.gather(*(store.claim(definition.search_id, now=now) for _ in range(8)))
    owners = [item for item in claims if item]
    assert len(owners) == 1
    report = _report("run-authority", 0, now)
    assert await store.commit(
        owners[0],
        report,
        None,
        now=now,
        next_due=now + 60,
        policy_fingerprint="policy",
        event=event_for_report(owners[0], report),
    )
    assert await backend._client.xlen(store._event_stream()) == 1
    assert [item["run_id"] for item in await store.reports(definition.search_id, now=now)] == ["run-authority"]


async def test_capacity_rejection_writes_neither_report_nor_event(backend):
    now = time.time()
    store = SavedSearchStore(backend, "tenant", event_capacity=1, event_retention_seconds=600)
    definition = _definition(now)
    assert await store.create(definition, 20, now=now) == "created"
    assert await _commit(store, definition, "run-one", now)
    with pytest.raises(OutboxCapacityError):
        await _commit(store, definition, "run-two", now + 60)
    assert await backend._client.xlen(store._event_stream()) == 1
    reports = await store.reports(definition.search_id, now=now + 60)
    assert [item["run_id"] for item in reports] == ["run-one"]


async def test_ordered_read_ack_restart_and_tenant_isolation(backend):
    now = time.time()
    root = SavedSearchStore(backend)
    store = root.for_tenant("tenant")
    definition = _definition(now)
    assert await store.create(definition, 20, now=now) == "created"
    assert await _commit(store, definition, "run-one", now)
    assert await _commit(store, definition, "run-two", now + 60)
    batch = await store.read_events("agent", cursor=None, limit=1)
    assert len(batch["events"]) == 1
    cursor = batch["next_cursor"]
    assert await store.acknowledge_event("agent", cursor, now=now + 60) == cursor
    assert await store.acknowledge_event("agent", cursor, now=now + 60) == cursor
    restarted = SavedSearchStore(backend).for_tenant("tenant")
    assert (await restarted.read_events("agent", cursor=None, limit=10))["events"][0][1]["run_id"] == "run-two"
    assert (await root.for_tenant("other").read_events("agent", cursor=None, limit=10))["events"] == []


async def test_pause_and_expiry_publication_are_atomic_and_idempotent(backend):
    now = time.time()
    root = SavedSearchStore(backend)
    store = root.for_tenant("tenant")
    definition = _definition(now)
    assert await store.create(definition, 20, now=now) == "created"
    current = await store.load(definition.search_id, now=now)
    assert current
    current.paused = True
    await store.compare_and_set(
        current,
        1,
        now=now,
        event=definition_event(current, "definition_paused", now),
    )
    assert (await store.load(definition.search_id, now=now)).paused is True
    assert await backend._client.xlen(store._event_stream()) == 1

    expiring = _definition(now)
    expiring.search_id = "saved-expiring"
    expiring.expires_at = now + 1
    assert await store.create(expiring, 20, now=now) == "created"
    assert await root.publish_expired_events(now=now + 2) == 1
    assert await root.publish_expired_events(now=now + 2) == 0
    assert await backend._client.xlen(store._event_stream()) == 2


async def test_read_atomically_trims_expired_events_without_a_publisher(backend):
    now = time.time()
    store = SavedSearchStore(backend, "tenant", event_retention_seconds=120)
    stale_cursor = f"{int((now - 300) * 1000)}-0"
    live_cursor = f"{int((now - 60) * 1000)}-0"
    await backend._client.xadd(
        store._event_stream(),
        {"payload": '{"event_id":"stale"}'},
        id=stale_cursor,
    )
    await backend._client.xadd(
        store._event_stream(),
        {"payload": '{"event_id":"live"}'},
        id=live_cursor,
    )

    batch = await store.read_events("agent", cursor=stale_cursor, limit=10, now=now)

    assert batch["events"] == [(live_cursor, {"event_id": "live"})]
    assert batch["gap"] == {
        "detected": True,
        "reason": "retention_expired",
        "first_available_cursor": live_cursor,
    }
    assert await backend._client.xlen(store._event_stream()) == 1


async def test_ack_atomically_rejects_an_expired_untrimmed_cursor(backend):
    now = time.time()
    store = SavedSearchStore(backend, "tenant", event_retention_seconds=120)
    stale_cursor = f"{int((now - 300) * 1000)}-0"
    live_cursor = f"{int((now - 60) * 1000)}-0"
    for cursor in (stale_cursor, live_cursor):
        await backend._client.xadd(
            store._event_stream(),
            {"payload": '{"event_id":"event"}'},
            id=cursor,
        )

    with pytest.raises(LookupError, match="expired"):
        await store.acknowledge_event("agent", stale_cursor, now=now)

    assert await backend._client.xlen(store._event_stream()) == 1
