"""Atomic retrieval-receipt invariants against isolated real Valkey."""

from __future__ import annotations

import asyncio
import os
import uuid

import pytest
import valkey.asyncio as valkey

from slopsearx import retrieval_receipts as storage
from slopsearx.retrieval_receipts import ReceiptStore
from tests.test_research_queue_integration import Store

pytestmark = pytest.mark.skipif(not os.environ.get("SLOPSEARX_TEST_VALKEY_URL"), reason="explicit test Valkey required")


@pytest.fixture
async def backend(monkeypatch):
    prefix = f"test-receipts-{uuid.uuid4().hex}"
    monkeypatch.setattr(storage, "RECEIPT_PREFIX", prefix)
    client = valkey.from_url(os.environ["SLOPSEARX_TEST_VALKEY_URL"])
    await client.ping()
    store = Store(client)
    try:
        yield store
    finally:
        async for key in client.scan_iter(match=f"{prefix}:*"):
            await client.delete(key)
        await client.aclose()


def receipt(index: int = 0) -> dict:
    return {
        "contract": "slopsearx.retrieval_receipt",
        "version": 1,
        "result_id": "snap-source:0",
        "attribution": {"retriever": "reader", "authenticated_tenant": "tenant"},
        "observation": {"status": "failed", "failure": {"code": f"failure-{index}"}},
        "discovery": {"result_id": "snap-source:0", "verified": False},
        "observations_verified": False,
    }


async def test_atomic_replay_conflict_capacity_fixed_horizon_and_tenant_isolation(backend):
    store = ReceiptStore(backend, "tenant")
    attempts = await asyncio.gather(
        *(store.submit("snap-source:0", "same-key", "same-digest", receipt(), now=1000) for _ in range(8))
    )
    assert [status for status, _value in attempts].count("created") == 1
    assert [status for status, _value in attempts].count("replayed") == 7
    assert (await store.submit("snap-source:0", "same-key", "different", receipt(), now=1001))[0] == "conflict"
    first_expiry = attempts[0][1]["expires_at"]
    for index in range(1, 20):
        status, value = await store.submit(
            "snap-source:0", f"key-{index}", f"digest-{index}", receipt(index), now=1000 + index
        )
        assert status == "created"
        assert value["expires_at"] == first_expiry
    receipt_ttl = await backend._client.ttl(store._receipt_key("snap-source:0", value["receipt_id"]))
    assert receipt_ttl <= int(first_expiry - (1000 + index)) + storage.RECEIPT_STORE_MARGIN_SECONDS
    assert (await store.submit("snap-source:0", "overflow", "overflow", receipt(21), now=1021))[0] == "capacity"
    values, total = await store.read("snap-source:0", now=1021, limit=5)
    assert len(values) == 5 and total == 20
    assert await ReceiptStore(backend, "other").read("snap-source:0", now=1021) == ([], 0)
    renewed, value = await store.submit("snap-source:0", "same-key", "new-era", receipt(99), now=first_expiry + 1)
    assert renewed == "created"
    assert value["expires_at"] == first_expiry + 1 + storage.RECEIPT_RETENTION_SECONDS


async def test_oversize_rejection_is_atomic(backend):
    store = ReceiptStore(backend, "tenant")
    status, value = await store.submit(
        "snap-source:0", "oversize", "digest", {"large": "x" * storage.MAX_RECEIPT_BYTES}, now=1000
    )
    assert status == "size"
    assert value is None
    assert await store.read("snap-source:0", now=1000) == ([], 0)
