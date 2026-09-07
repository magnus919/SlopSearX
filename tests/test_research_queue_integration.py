"""Ready-index invariants against isolated real Valkey namespaces; never FLUSH."""

from __future__ import annotations

import asyncio
import json
import os
import time
import uuid
from collections import Counter
from typing import Any

import pytest
import valkey.asyncio as valkey

from slopsearx import research as r

pytestmark = pytest.mark.skipif(not os.environ.get("SLOPSEARX_TEST_VALKEY_URL"), reason="explicit test Valkey required")


class Store:
    is_connected = True

    def __init__(self, client: Any) -> None:
        self._client = client
        self.reads = 0

    async def get(self, key: str) -> Any:
        self.reads += 1
        raw = await self._client.get(key)
        return json.loads(raw) if raw else None

    async def set(self, key: str, value: Any, ttl: int = 300) -> None:
        await self._client.set(key, json.dumps(value), ex=ttl)


@pytest.fixture
async def backend(monkeypatch: Any) -> Any:
    prefix = f"test-ready-{uuid.uuid4().hex}"
    for constant in ("JOB_KEY_PREFIX", "LEASE_KEY_PREFIX", "CANCEL_KEY_PREFIX", "IDEMPOTENCY_PREFIX", "READY_PREFIX"):
        monkeypatch.setattr(r, constant, f"{prefix}:{constant}")
    client = valkey.from_url(os.environ["SLOPSEARX_TEST_VALKEY_URL"])
    await client.ping()
    store = Store(client)
    try:
        yield store
    finally:
        async for key in client.scan_iter(match=f"{prefix}:*"):
            await client.delete(key)
        await client.aclose()


def job(tenant: str = "a", state: str = "queued") -> r.ResearchJob:
    return r.ResearchJob(
        job_id=r.generate_job_id(),
        tenant=tenant,
        question="test",
        strategy="triangulate",
        state=state,
        deadline=time.time() + 3600,
    )


async def test_ready_claim_race_fencing_renewal_and_orphan(backend: Store) -> None:
    store = r.ResearchJobStore(backend, "a")
    item = job()
    await store.save(item)
    claims = await asyncio.gather(*(store.claim_next_any_tenant(str(i), 60) for i in range(8)))
    owned = [claimed for claimed in claims if claimed is not None]
    assert len(owned) == 1
    old = owned[0]
    assert await store.renew(item.job_id, old.lease_token or "", 120)
    score = await backend._client.zscore(f"{r.READY_PREFIX}:tenant:a", item.job_id)
    assert score > time.time() + 100
    # Simulate worker death and lease expiry without sleep or wall-clock races.
    await backend._client.delete(store._lease_key(item.job_id))
    await store._refresh_ready(item.job_id)
    reclaimed = await store.claim_next_any_tenant("replacement", 60)
    assert reclaimed and reclaimed.lease_token != old.lease_token
    old.state = "succeeded"
    assert not await store.save_if_owned(old)
    assert not await store.renew(item.job_id, old.lease_token or "", 60)
    await store.release(item.job_id, old.lease_token)
    assert await store._lease_get(store._lease_key(item.job_id)) == reclaimed.lease_token
    reclaimed.state = "succeeded"
    assert await store.save_if_owned(reclaimed)
    assert await backend._client.zcard(f"{r.READY_PREFIX}:tenant:a") == 0


async def test_tenant_rotation_and_cancel(backend: Store) -> None:
    root = r.ResearchJobStore(backend)
    for tenant in ("busy", "oauth:quiet"):
        for _ in range(4 if tenant == "busy" else 1):
            await root.for_tenant(tenant).save(job(tenant))
    claims = [await root.claim_next_any_tenant("worker", 60) for _ in range(2)]
    assert {claimed.tenant for claimed in claims if claimed} == {"busy", "oauth:quiet"}
    cancelled = job("cancel")
    scoped = root.for_tenant("cancel")
    await scoped.save(cancelled)
    await scoped.request_cancel(cancelled.job_id)
    assert await scoped.claim_next("worker", 60) is None
    assert await backend._client.zcard(f"{r.READY_PREFIX}:tenant:cancel") == 0


async def test_reconciliation_repairs_save_gap_and_reservation_crash(backend: Store, monkeypatch: Any) -> None:
    store = r.ResearchJobStore(backend, "a")
    item = job()
    # Legacy save / crash after record persistence but before index refresh.
    await backend.set(store._key(item.job_id), r._job_to_payload(item))
    await store._reconcile_ready()
    candidate = await backend._client.eval(
        r._READY_TAKE_SCRIPT, 2, f"{r.READY_PREFIX}:tenants", f"{r.READY_PREFIX}:turn", r.READY_PREFIX
    )
    assert candidate == [b"a", item.job_id.encode()]
    # Simulate expiry of the non-destructive five-second reservation.
    await backend._client.zadd(f"{r.READY_PREFIX}:tenant:a", {item.job_id: 0})
    claimed = await store.claim_next_any_tenant("replacement", 60)
    assert claimed and claimed.job_id == item.job_id
    # Late refresh reads terminal authority instead of re-adding stale queued state.
    claimed.state = "cancelled"
    await store.save_if_owned(claimed)
    await store._refresh_ready(item.job_id)
    assert await store.claim_next_any_tenant("worker", 60) is None


async def test_idle_poll_does_not_read_retained_history(backend: Store, monkeypatch: Any) -> None:
    store = r.ResearchJobStore(backend, "a")
    for _ in range(1000):
        await store.save(job(state="succeeded"))
    await store._reconcile_ready()
    counts: Counter[str] = Counter()
    original = backend._client.execute_command

    async def counted(*args: Any, **kwargs: Any) -> Any:
        counts[str(args[0])] += 1
        return await original(*args, **kwargs)

    monkeypatch.setattr(backend._client, "execute_command", counted)
    backend.reads = 0
    for _ in range(20):
        assert await store.claim_next_any_tenant("idle", 60) is None
    assert backend.reads == 0
    assert counts == {"SET": 20, "EVAL": 20}
    print(f"1000 terminal records, 20 idle polls: {dict(counts)}, record reads={backend.reads}")

    class LegacyScanStore(r.ResearchJobStore):
        def _ready_client(self) -> Any:
            return None

    counts.clear()
    assert await LegacyScanStore(backend, "a").claim_next_any_tenant("legacy", 60) is None
    assert backend.reads == 2000
    assert counts["SCAN"] > 0
    print(f"Same history, 1 legacy idle poll: {dict(counts)}, record/cancel reads={backend.reads}")


async def test_actual_lease_expiry_is_discovered_without_scan(backend: Store, monkeypatch: Any) -> None:
    store = r.ResearchJobStore(backend, "a")
    item = job()
    await store.save(item)
    first = await store.claim_next_any_tenant("lost", 1)
    assert first
    await asyncio.sleep(1.05)

    async def forbid_scan(*args: Any, **kwargs: Any) -> Any:
        raise AssertionError("orphan discovery must use the ready index")

    with monkeypatch.context() as scoped_patch:
        scoped_patch.setattr(backend._client, "scan", forbid_scan)
        recovered = await store.claim_next_any_tenant("new", 60)
        assert recovered and recovered.lease_token != first.lease_token
