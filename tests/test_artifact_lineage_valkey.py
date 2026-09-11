"""Artifact lineage persistence against an explicitly selected disposable Valkey."""

from __future__ import annotations

import os
import uuid

import pytest
import valkey.asyncio as valkey

from slopsearx import snapshot
from slopsearx.adapter import SearchResult
from slopsearx.artifacts import artifact_ref
from slopsearx.service import ScopeDecision
from slopsearx.snapshot import SnapshotStore
from tests.test_research_queue_integration import Store

pytestmark = pytest.mark.skipif(
    not os.environ.get("SLOPSEARX_TEST_VALKEY_URL"),
    reason="explicit test Valkey required",
)


@pytest.fixture
async def backend(monkeypatch: pytest.MonkeyPatch):
    prefix = f"test-artifact-lineage-{uuid.uuid4().hex}"
    monkeypatch.setattr(snapshot, "SNAPSHOT_KEY_PREFIX", prefix)
    client = valkey.from_url(os.environ["SLOPSEARX_TEST_VALKEY_URL"])
    await client.ping()
    store = Store(client)
    try:
        yield store, client, prefix
    finally:
        keys = [key async for key in client.scan_iter(match=f"{prefix}:*")]
        if keys:
            await client.delete(*keys)
        await client.aclose()


async def test_lineage_reads_across_store_instances_without_extending_ttl(backend) -> None:
    store, client, prefix = backend
    first = SnapshotStore(store, tenant="tenant", ttl_seconds=60)
    second = SnapshotStore(store, tenant="tenant", ttl_seconds=60)
    parent = artifact_ref("research_attempt", "attempt-parent")
    snapshot_id = await first.create(
        "query",
        "query-id",
        [SearchResult(url="https://example.test", title="Example", engine="wikipedia")],
        ScopeDecision(selected_engines=["wikipedia"]),
        derived_from=[parent],
    )
    assert snapshot_id is not None
    key = f"{prefix}:tenant:{snapshot_id}"
    before = await client.ttl(key)

    read = await second.read(snapshot_id)
    after = await client.ttl(key)

    assert read.snapshot is not None
    assert read.snapshot.lineage[0]["to"] == parent
    assert after <= before
