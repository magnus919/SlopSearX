"""Distributed staged-search invariants against an explicit disposable Valkey."""

from __future__ import annotations

import asyncio
import json
import os
import time
import uuid
from typing import Any

import pytest
import valkey.asyncio as valkey

from slopsearx import staged

pytestmark = pytest.mark.skipif(not os.environ.get("SLOPSEARX_TEST_VALKEY_URL"), reason="explicit test Valkey required")


class Store:
    is_connected = True

    def __init__(self, client: Any) -> None:
        self._client = client

    async def get(self, key: str) -> dict[str, Any] | None:
        raw = await self._client.get(key)
        return json.loads(raw) if raw else None

    async def set(self, key: str, value: dict[str, Any], ttl: int = 300) -> None:
        await self._client.set(key, json.dumps(value), ex=ttl)


@pytest.fixture
async def stores(monkeypatch: pytest.MonkeyPatch):
    prefix = f"test-staged-{uuid.uuid4().hex}"
    monkeypatch.setattr(staged, "PREFIX", prefix)
    client = valkey.from_url(os.environ["SLOPSEARX_TEST_VALKEY_URL"])
    await client.ping()
    backend = Store(client)
    try:
        yield staged.StagedSearchStore(backend), staged.StagedSearchStore(backend)
    finally:
        keys = [key async for key in client.scan_iter(match=f"{prefix}:*")]
        if keys:
            await client.delete(*keys)
        await client.aclose()


def operation(operation_id: str) -> dict[str, Any]:
    now = time.time()
    scope = {"selected_engines": ["wikipedia"], "excluded_engines": []}
    return {
        "operation_id": operation_id,
        "tenant": "tenant-a",
        "plan_digest": "same-plan",
        "accepted_at": now,
        "execution_deadline_at": now + 300,
        "expires_at": now + 3600,
        "state": "queued",
        "stop_reason": None,
        "next_stage": 0,
        "plan": {"initial_scope": {"selected_engines": ["wikipedia"], "excluded_engines": []}},
        "objectives": {"unmet": []},
        "budget": {"limit": 2, "reserved": 0, "observed": 0},
        "retry_keys": [],
        "stages": [
            {
                "index": 0,
                "state": "pending",
                "scope": scope,
                "attempts": [],
                "enforcement": {"language": {"enforced_by": []}},
            }
        ],
    }


async def test_atomic_admission_claim_fencing_and_retry(stores) -> None:
    first, second = stores
    records = [operation(str(uuid.uuid4())) for _ in range(8)]
    admissions = await asyncio.gather(
        *(
            store.admit("tenant-a", "same-key", "same-plan", record)
            for store, record in zip([first, second] * 4, records)
        )
    )
    assert [status for status, _ in admissions].count("created") == 1
    operation_ids = {record["operation_id"] for _, record in admissions if record}
    assert len(operation_ids) == 1
    operation_id = operation_ids.pop()
    record_key = first._key("tenant-a", operation_id)
    admitted_ttl = await first._client().ttl(record_key)

    claims = await asyncio.gather(*(store.claim("tenant-a", operation_id) for store in (first, second)))
    owned = [claim for claim in claims if claim]
    assert len(owned) == 1
    record, token = owned[0]
    assert record["budget"]["reserved"] == 1
    assert not await second.save_owned("tenant-a", record, "stale-owner")

    record["state"] = "failed"
    record["stop_reason"] = "execution_failed"
    record["stages"][0]["state"] = "failed"
    record["stages"][0]["attempts"][-1]["state"] = "failed"
    assert await first.save_owned("tenant-a", record, token)
    assert 0 < await first._client().ttl(record_key) <= admitted_ttl
    assert 0 < await first._client().ttl(first._idem_key("tenant-a", "same-key")) <= admitted_ttl
    retries = await asyncio.gather(
        first.request_retry("tenant-a", operation_id, "retry-a"),
        second.request_retry("tenant-a", operation_id, "retry-b"),
    )
    assert sorted(status for status, _ in retries) == ["busy", "queued"]
    persisted = await first.read("tenant-a", operation_id)
    assert persisted.record and persisted.record["budget"]["reserved"] == 1


async def test_reconcile_orphan_preserves_unknown_accounting_and_array_shapes(stores) -> None:
    first, second = stores
    item = operation(str(uuid.uuid4()))
    assert (await first.admit("tenant-a", "orphan-key", "same-plan", item))[0] == "created"
    await first._client().delete(first._ready_key())
    await second.reconcile_indexes()
    candidate = await second.claim_next()
    assert candidate == ("tenant-a", item["operation_id"])
    claimed = await second.claim(*candidate)
    assert claimed is not None
    record, _token = claimed
    await first._client().delete(first._lease_key("tenant-a", item["operation_id"]))
    await first._client().zadd(first._running_key(), {first._key("tenant-a", item["operation_id"]): 0})
    assert await first.recover_orphans() == 1
    interrupted = await first.read("tenant-a", item["operation_id"])
    assert interrupted.record is not None
    assert interrupted.record["budget"]["observed"] is None
    assert interrupted.record["retry_keys"] == []
    assert interrupted.record["objectives"]["unmet"] == []
    assert interrupted.record["plan"]["initial_scope"]["excluded_engines"] == []
    assert interrupted.record["stages"][0]["enforcement"]["language"]["enforced_by"] == []
    assert interrupted.record["stages"][0]["attempts"][0]["engine_outcomes"] == []
    assert interrupted.record["stages"][0]["attempts"][0]["enforcement"]["language"]["enforced_by"] == []

    status, queued = await first.request_retry("tenant-a", item["operation_id"], "retry-orphan")
    assert status == "queued" and queued is not None
    retried = await second.claim("tenant-a", item["operation_id"])
    assert retried is not None
    assert retried[0]["budget"]["reserved"] == 2
    assert retried[0]["budget"]["observed"] is None
