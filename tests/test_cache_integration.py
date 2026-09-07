"""Real Valkey contracts; uses only an explicitly selected disposable test server.

Set SLOPSEARX_TEST_VALKEY_URL, never VALKEY_URL. No FLUSHDB is issued: every
record belongs to a random test namespace and cleanup deletes only its keys.
CI provisions an ephemeral Valkey service for these tests.
"""

from __future__ import annotations

import asyncio
import os
import time
import uuid
from collections.abc import AsyncIterator

import pytest

from slopsearx import research
from slopsearx.cache import SearchCache
from slopsearx.research import ResearchJob, ResearchJobStore

TEST_URL = os.environ.get("SLOPSEARX_TEST_VALKEY_URL", "")
pytestmark = pytest.mark.skipif(not TEST_URL, reason="requires disposable SLOPSEARX_TEST_VALKEY_URL")


@pytest.fixture
async def cache() -> AsyncIterator[SearchCache]:
    instance = SearchCache(TEST_URL)
    await instance.connect()
    assert instance.is_connected, "Disposable Valkey test server must be reachable"
    try:
        yield instance
    finally:
        await instance.close()


@pytest.fixture
async def namespace(cache: SearchCache, monkeypatch: pytest.MonkeyPatch) -> AsyncIterator[str]:
    name = f"integration-{uuid.uuid4().hex}"
    monkeypatch.setattr(research, "READY_PREFIX", f"{name}:ready")
    try:
        yield name
    finally:
        # Namespaces appear in cache keys and tenant-scoped research keys.
        keys = [key async for key in cache._client.scan_iter(match=f"*{name}*")]
        if keys:
            await cache._client.delete(*keys)


async def test_connection_lifecycle(cache: SearchCache) -> None:
    client = cache._client
    await cache.connect()
    assert cache._client is client
    await cache.close()
    assert not cache.is_connected
    replacement = SearchCache(TEST_URL)
    try:
        await replacement.connect()
        assert replacement.is_connected
    finally:
        await replacement.close()


async def test_connection_from_environment(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("VALKEY_URL", TEST_URL)
    instance = SearchCache()
    try:
        await instance.connect()
        assert instance.is_connected
    finally:
        await instance.close()


async def test_roundtrip_overwrite_and_isolation(cache: SearchCache, namespace: str) -> None:
    key = f"{namespace}:one"
    other = f"{namespace}:two"
    payload = {"results": [{"title": "C++", "engines": ["brave", "google"]}], "nested": [True, None, 42]}
    assert await cache.get(key) is None
    await cache.set(key, payload, ttl=60)
    assert await cache.get(key) == payload
    await cache.set(other, {"text": "x" * 100_000}, ttl=60)
    await cache.set(key, {"updated": True}, ttl=60)
    assert await cache.get(key) == {"updated": True}
    assert await cache.get(other) == {"text": "x" * 100_000}


async def test_real_expiration(cache: SearchCache, namespace: str) -> None:
    key = f"{namespace}:expiry"
    await cache.set(key, {"ok": True}, ttl=1)
    assert await cache.get(key) == {"ok": True}
    await asyncio.sleep(1.1)
    assert await cache.get(key) is None


async def test_negative_cache(cache: SearchCache, namespace: str) -> None:
    key = f"{namespace}:negative"
    await cache.set_error(key, ttl=60)
    payload = await cache.get(key)
    assert payload is not None and payload["_error"] is True


def job(namespace: str, suffix: str = "job", *, expired: bool = False) -> ResearchJob:
    return ResearchJob(
        job_id=suffix,
        question="test",
        strategy="broad",
        tenant=namespace,
        deadline=time.time() + (-1 if expired else 120),
    )


async def test_competing_claims_and_tenant_isolation(cache: SearchCache, namespace: str) -> None:
    store = ResearchJobStore(cache, tenant=namespace)
    await store.save(job(namespace))
    claims = await asyncio.gather(*(store.claim("job", f"worker-{i}", 30) for i in range(8)))
    winners = [claim for claim in claims if claim is not None]
    assert len(winners) == 1
    assert await store.for_tenant(namespace + "-other").load("job") is None
    winner = winners[0]
    assert winner.lease_token
    assert await store.renew("job", winner.lease_token, 30)
    assert not await store.renew("job", "stale-owner", 30)
    await store.release("job", "stale-owner")
    assert await store.claim("job", "intruder", 30) is None


async def test_expired_lease_recovery_fences_stale_writes(cache: SearchCache, namespace: str) -> None:
    store = ResearchJobStore(cache, tenant=namespace)
    await store.save(job(namespace))
    old = await store.claim("job", "old", 1)
    assert old is not None
    await asyncio.sleep(1.1)
    current = await store.claim("job", "new", 30)
    assert current is not None and current.lease_token != old.lease_token
    old.state = "succeeded"
    assert not await store.save_if_owned(old)
    await store.release("job", old.lease_token)
    current.state = "succeeded"
    assert await store.save_if_owned(current)
    persisted = await store.load("job")
    assert persisted is not None and persisted.owner_id == "new"


async def test_cancel_signal_survives_owner_write(cache: SearchCache, namespace: str) -> None:
    store = ResearchJobStore(cache, tenant=namespace)
    await store.save(job(namespace))
    owner = await store.claim("job", "owner", 30)
    assert owner is not None
    assert await store.request_cancel("job") == "running"
    assert await store.save_if_owned(owner)
    persisted = await store.load("job")
    assert persisted is not None and persisted.cancel_requested


async def test_unowned_cancellation_and_deadline(cache: SearchCache, namespace: str) -> None:
    store = ResearchJobStore(cache, tenant=namespace)
    await store.save(job(namespace, "cancel"))
    assert await store.request_cancel("cancel") == "cancelled"
    assert await store.claim("cancel", "worker", 30) is None
    await store.save(job(namespace, "expired", expired=True))
    assert await store.claim("expired", "worker", 30) is None
    expired = await store.load("expired")
    assert expired is not None and expired.state == "expired"
