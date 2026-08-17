"""Deterministic tests for durable research execution across replicas.

These cover the lease/claim model in :mod:`slopsearx.research` without a
live Valkey: an atomic, TTL-aware in-memory store stands in for Valkey
(``SET NX`` semantics), and a fake Valkey-like client exercises the
production ``_client`` code paths. Scenarios: worker loss / lease expiry,
duplicate delivery, cancellation across owners, tenant isolation, and
request-scoped tenant identity.

No live network, no Valkey.
"""

from __future__ import annotations

import asyncio
import json
import time
from typing import Any

import pytest

import engines  # noqa: F401 — populates the engine registry
from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult
from slopsearx.capabilities import CapabilityCatalog, load_mcp_policy
from slopsearx.config import load_config
from slopsearx.mcp import state as state_mod
from slopsearx.mcp import tools as t
from slopsearx.mcp.state import McpState, set_state, tenant_scope
from slopsearx.research import (
    LEASE_KEY_PREFIX,
    ResearchJob,
    ResearchJobRunner,
    ResearchJobStore,
    ResearchQuery,
    generate_job_id,
)
from slopsearx.service import AppContext, ScopeDecision, SearchService
from slopsearx.snapshot import SnapshotStore

# ---------------------------------------------------------------------------
# Deterministic store backends
# ---------------------------------------------------------------------------


class AtomicStore:
    """TTL-aware, atomic in-memory key/value store (faithful Valkey stand-in).

    ``set_nx``/``acquire_lease``/``renew_lease``/``release_lease`` mutate a
    single dict synchronously, so they are atomic under asyncio (no ``await``
    between check and set).
    """

    def __init__(self) -> None:
        self.is_connected = True
        self._data: dict[str, Any] = {}
        self._expiry: dict[str, float | None] = {}
        self.set_ttls: list[int] = []

    def _expired(self, key: str) -> bool:
        exp = self._expiry.get(key)
        return exp is not None and exp <= time.time()

    def _purge(self, key: str) -> None:
        if self._expired(key):
            self._data.pop(key, None)
            self._expiry.pop(key, None)

    async def get(self, key: str) -> Any | None:
        self._purge(key)
        return self._data.get(key)

    async def set(self, key: str, value: Any, ttl: int = 300) -> None:
        self._data[key] = value
        self._expiry[key] = time.time() + ttl if ttl and ttl > 0 else None
        self.set_ttls.append(ttl)

    async def keys(self, pattern: str) -> list[str]:
        prefix = pattern.rstrip("*")
        return [key for key in self._data if key.startswith(prefix) and not self._expired(key)]

    async def delete(self, key: str) -> None:
        self._data.pop(key, None)
        self._expiry.pop(key, None)

    async def set_nx(self, key: str, value: dict[str, Any], ttl: int = 300) -> bool:
        self._purge(key)
        if key in self._data:
            return False
        self._data[key] = value
        self._expiry[key] = time.time() + ttl if ttl and ttl > 0 else None
        self.set_ttls.append(ttl)
        return True

    async def acquire_lease(self, key: str, token: str, ttl: int) -> bool:
        return await self.set_nx(key, {"token": token}, ttl)

    async def renew_lease(self, key: str, token: str, ttl: int) -> bool:
        self._purge(key)
        current = self._data.get(key)
        if current is None or not isinstance(current, dict) or current.get("token") != token:
            return False
        self._data[key] = {"token": token}
        self._expiry[key] = time.time() + ttl
        self.set_ttls.append(ttl)
        return True

    async def release_lease(self, key: str, token: str) -> bool:
        self._purge(key)
        current = self._data.get(key)
        if current is None or not isinstance(current, dict) or current.get("token") != token:
            return False
        self._data.pop(key, None)
        self._expiry.pop(key, None)
        return True


class _ValkeyLikeClient:
    """Minimal async client mirroring the valkey-py surface used by leases."""

    def __init__(self) -> None:
        self._data: dict[str, bytes] = {}
        self._ttl: dict[str, float | None] = {}

    def _expired(self, key: str) -> bool:
        exp = self._ttl.get(key)
        return exp is not None and exp <= time.time()

    async def get(self, key: str) -> bytes | None:
        if self._expired(key):
            self._data.pop(key, None)
            self._ttl.pop(key, None)
            return None
        return self._data.get(key)

    async def set(
        self,
        key: str,
        value: Any,
        nx: bool = False,
        xx: bool = False,
        ex: int | None = None,
        px: int | None = None,
    ) -> bool | None:
        if self._expired(key):
            self._data.pop(key, None)
            self._ttl.pop(key, None)
        if nx and key in self._data:
            return None
        if xx and key not in self._data:
            return None
        self._data[key] = value if isinstance(value, bytes) else str(value).encode()
        ttl = ex if ex is not None else (px / 1000.0 if px is not None else None)
        self._ttl[key] = time.time() + ttl if ttl is not None else None
        return True

    async def delete(self, key: str) -> bool:
        existed = key in self._data
        self._data.pop(key, None)
        self._ttl.pop(key, None)
        return existed

    async def keys(self, pattern: str) -> list[bytes]:
        prefix = pattern.rstrip("*")
        return [key.encode() for key in self._data if key.startswith(prefix) and not self._expired(key)]


class ValkeyLikeStore:
    """A store whose ``_client`` exposes the valkey-py surface.

    Job records are JSON-serialized through ``get``/``set`` (like
    :class:`slopsearx.cache.SearchCache`); lease keys are written raw by the
    lease primitives via ``_client``.
    """

    def __init__(self) -> None:
        self.is_connected = True
        self._client = _ValkeyLikeClient()

    async def get(self, key: str) -> dict[str, Any] | None:
        raw = await self._client.get(key)
        if raw is None:
            return None
        return json.loads(raw.decode())

    async def set(self, key: str, value: dict[str, Any], ttl: int = 300) -> None:
        await self._client.set(key, json.dumps(value, default=str), ex=ttl)


class _ScanPreferredClient:
    """Client exposing both ``scan_iter`` and ``keys``; tracks ``KEYS`` misuse."""

    def __init__(self) -> None:
        self._data: dict[bytes, bytes] = {
            b"mcp:job:default:job-a": b"{}",
            b"mcp:job:default:job-b": b"{}",
        }
        self.keys_calls = 0

    async def scan_iter(self, match: str | None = None) -> Any:
        prefix = (match or "").rstrip("*").encode()
        for key in list(self._data):
            if key.startswith(prefix):
                yield key

    async def keys(self, pattern: str) -> list[bytes]:
        del pattern
        self.keys_calls += 1
        return []


class ScanPreferredStore:
    """A store whose ``_client`` can SCAN but should never be driven via KEYS."""

    def __init__(self) -> None:
        self.is_connected = True
        self._client = _ScanPreferredClient()

    async def get(self, key: str) -> dict[str, Any] | None:
        del key
        return None

    async def set(self, key: str, value: dict[str, Any], ttl: int = 300) -> None:
        del key, value, ttl


class _MockEngine(EngineAdapter):
    """Deterministic engine with a call counter."""

    def __init__(self, name: str, status: EngineStatus = EngineStatus.OK, count: int = 2) -> None:
        super().__init__()
        self.name = name
        self._status = status
        self._count = count
        self.calls = 0

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        del query, params
        self.calls += 1
        if self._status != EngineStatus.OK:
            return AdapterResponse(results=[], status=self._status, error_message="boom", latency_ms=1.0)
        return AdapterResponse(
            results=[
                SearchResult(
                    url=f"https://{self.name}{i}.example",
                    title=f"{self.name} {i}",
                    content=f"Content {i}.",
                    engine=self.name,
                )
                for i in range(self._count)
            ],
            status=EngineStatus.OK,
            latency_ms=1.0,
        )


class _SlowEngine(EngineAdapter):
    """Engine whose search blocks long enough to outlive a short lease."""

    def __init__(self, name: str, delay: float) -> None:
        super().__init__()
        self.name = name
        self.delay = delay
        self.calls = 0

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        del query, params
        self.calls += 1
        await asyncio.sleep(self.delay)
        return AdapterResponse(
            results=[
                SearchResult(
                    url=f"https://{self.name}.example",
                    title=self.name,
                    content="Content.",
                    engine=self.name,
                )
            ],
            status=EngineStatus.OK,
            latency_ms=self.delay * 1000,
        )


def _build_state(
    *,
    store: AtomicStore | None = None,
    owner_id: str = "worker-test",
    lease_ttl: int = 60,
    max_concurrent_jobs: int = 2,
) -> tuple[McpState, AtomicStore]:
    engine_names = ["wikipedia", "brave"]
    engines_map = {name: _MockEngine(name) for name in engine_names}
    policy = load_mcp_policy(config_path=None)
    policy.enabled_tools["research"] = True
    store = store or AtomicStore()
    ctx = AppContext(
        active_engines=engines_map,
        cache=store,
        tier1_engines=set(engine_names),
        sensitive_engines=policy.sensitive_engines,
    )
    catalog = CapabilityCatalog(config=load_config())
    service = SearchService(ctx)
    snapshots = SnapshotStore(store, ttl_seconds=policy.snapshot_ttl_seconds)
    job_store = ResearchJobStore(store)
    runner = ResearchJobRunner(
        service,
        job_store,
        snapshots,
        catalog,
        policy,
        owner_id=owner_id,
        lease_ttl=lease_ttl,
        max_concurrent_jobs=max_concurrent_jobs,
    )
    state = McpState(
        ctx=ctx,
        policy=policy,
        catalog=catalog,
        service=service,
        snapshots=snapshots,
        job_store=job_store,
        runner=runner,
        version="test",
    )
    return state, store


def _job(
    *,
    job_id: str | None = None,
    state: str = "queued",
    tenant: str = "default",
    queries: list[ResearchQuery] | None = None,
    deadline: float | None = None,
) -> ResearchJob:
    return ResearchJob(
        job_id=job_id or generate_job_id(),
        question="q",
        strategy="triangulate",
        state=state,
        tenant=tenant,
        queries=queries or [],
        deadline=deadline if deadline is not None else time.time() + 3600,
    )


def _lease_key(tenant: str, job_id: str) -> str:
    return f"{LEASE_KEY_PREFIX}:{tenant}:{job_id}"


# ---------------------------------------------------------------------------
# Claim / lease semantics
# ---------------------------------------------------------------------------


class TestClaim:
    async def test_claim_queued_job_sets_running_and_lease(self) -> None:
        _, store = _build_state()
        job_store = ResearchJobStore(store)
        job = _job(queries=[ResearchQuery(index=0, query="q", intent="web", engines=["wikipedia"])])
        await job_store.save(job)

        claimed = await job_store.claim(job.job_id, "w1", 60)

        assert claimed is not None
        assert claimed.state == "running"
        assert claimed.owner_id == "w1"
        assert claimed.lease_token is not None
        assert claimed.lease_expires_at > time.time()
        assert await job_store._lease_get(_lease_key("default", job.job_id)) == claimed.lease_token

    async def test_claim_terminal_job_returns_none(self) -> None:
        _, store = _build_state()
        job_store = ResearchJobStore(store)
        job = _job(state="succeeded")
        await job_store.save(job)

        assert await job_store.claim(job.job_id, "w1", 60) is None

    async def test_claim_owned_job_returns_none(self) -> None:
        _, store = _build_state()
        job_store = ResearchJobStore(store)
        job = _job()
        await job_store.save(job)
        first = await job_store.claim(job.job_id, "w1", 60)
        assert first is not None
        # Second worker cannot claim while w1's lease is live.
        assert await job_store.claim(job.job_id, "w2", 60) is None

    async def test_duplicate_delivery_exactly_one_wins(self) -> None:
        _, store = _build_state()
        job_store = ResearchJobStore(store)
        job = _job()
        await job_store.save(job)

        results = await asyncio.gather(
            job_store.claim(job.job_id, "w1", 60),
            job_store.claim(job.job_id, "w2", 60),
        )
        winners = [r for r in results if r is not None]
        assert len(winners) == 1
        assert {winners[0].owner_id} <= {"w1", "w2"}

    async def test_claim_resets_running_query_on_orphan_recovery(self) -> None:
        _, store = _build_state()
        job_store = ResearchJobStore(store)
        job = _job(
            state="running",
            queries=[
                ResearchQuery(index=0, query="a", intent="web", engines=["wikipedia"], state="done", cursor="snap-old"),
                ResearchQuery(index=1, query="b", intent="web", engines=["wikipedia"], state="running"),
            ],
        )
        await job_store.save(job)

        claimed = await job_store.claim(job.job_id, "w2", 60)

        assert claimed is not None
        assert claimed.queries[0].state == "done"
        assert claimed.queries[0].cursor == "snap-old"
        assert claimed.queries[1].state == "pending"

    async def test_lease_expiry_makes_job_reclaimable(self) -> None:
        _, store = _build_state()
        job_store = ResearchJobStore(store)
        job = _job()
        await job_store.save(job)
        first = await job_store.claim(job.job_id, "w1", 60)
        assert first is not None

        # Simulate the visibility timeout expiring (w1 died without release).
        store._expiry[_lease_key("default", job.job_id)] = time.time() - 1

        second = await job_store.claim(job.job_id, "w2", 60)
        assert second is not None
        assert second.owner_id == "w2"
        assert second.lease_token != first.lease_token


class TestLeasePrimitives:
    async def test_renew_and_release_are_token_guarded(self) -> None:
        _, store = _build_state()
        job_store = ResearchJobStore(store)
        job = _job()
        await job_store.save(job)
        claimed = await job_store.claim(job.job_id, "w1", 60)
        assert claimed is not None
        token = claimed.lease_token
        assert token is not None

        assert await job_store.renew(job.job_id, token, 60) is True
        assert await job_store.renew(job.job_id, "wrong-token", 60) is False

        # A foreign token cannot release the lease.
        assert await job_store._lease_release(_lease_key("default", job.job_id), "wrong-token") is False
        await job_store.release(job.job_id, token)
        assert await job_store._lease_get(_lease_key("default", job.job_id)) is None

    async def test_claim_next_and_scan_tenants(self) -> None:
        _, store = _build_state()
        job_store = ResearchJobStore(store)
        job = _job(tenant="tenant-b")
        await job_store.for_tenant("tenant-b").save(job)

        assert await job_store.scan_tenants() == ["tenant-b"]
        claimed = await job_store.claim_next_any_tenant("w1", 60)
        assert claimed is not None
        assert claimed.job_id == job.job_id
        assert claimed.tenant == "tenant-b"

    async def test_scan_tenants_is_sorted_and_deduped(self) -> None:
        _, store = _build_state()
        job_store = ResearchJobStore(store)
        await job_store.for_tenant("b").save(_job(tenant="b"))
        await job_store.for_tenant("a").save(_job(tenant="a"))
        await job_store.for_tenant("a").save(_job(tenant="a"))
        assert await job_store.scan_tenants() == ["a", "b"]


class TestValkeyLikeBackend:
    """Exercise the production ``_client`` lease code paths."""

    async def test_claim_renew_release_over_valkey_client(self) -> None:
        store = ValkeyLikeStore()
        job_store = ResearchJobStore(store)
        job = _job()
        await job_store.save(job)

        claimed = await job_store.claim(job.job_id, "w1", 60)
        assert claimed is not None
        token = claimed.lease_token
        assert token is not None
        assert await job_store._lease_get(_lease_key("default", job.job_id)) == token

        assert await job_store.renew(job.job_id, token, 60) is True
        assert await job_store.renew(job.job_id, "bad", 60) is False

        await job_store.release(job.job_id, token)
        assert await job_store._lease_get(_lease_key("default", job.job_id)) is None

    async def test_claim_next_over_valkey_client(self) -> None:
        store = ValkeyLikeStore()
        job_store = ResearchJobStore(store)
        job = _job()
        await job_store.save(job)
        claimed = await job_store.claim_next("w1", 60)
        assert claimed is not None
        assert claimed.job_id == job.job_id


# ---------------------------------------------------------------------------
# Cancellation across owners
# ---------------------------------------------------------------------------


class TestCancellation:
    async def test_cancel_unowned_job_finalizes_immediately(self) -> None:
        _, store = _build_state()
        job_store = ResearchJobStore(store)
        job = _job(queries=[ResearchQuery(index=0, query="q", intent="web", engines=["wikipedia"])])
        await job_store.save(job)

        assert await job_store.request_cancel(job.job_id) == "cancelled"
        loaded = await job_store.load(job.job_id)
        assert loaded is not None
        assert loaded.state == "cancelled"
        assert loaded.cancel_requested is True
        assert loaded.queries[0].state == "cancelled"

    async def test_cancel_owned_job_is_observed_by_owner(self) -> None:
        state, store = _build_state()
        job_store = state.job_store
        job = _job(queries=[ResearchQuery(index=0, query="q", intent="web", engines=["wikipedia"])])
        await job_store.save(job)
        claimed = await job_store.claim(job.job_id, "w1", 60)
        assert claimed is not None

        # Owner holds the lease → cancel records the flag but does not fight
        # the owner's intermediate writes.
        assert await job_store.request_cancel(job.job_id) == "running"
        loaded = await job_store.load(job.job_id)
        assert loaded is not None
        assert loaded.cancel_requested is True
        assert loaded.state == "running"

        # The owner observes the flag and finalizes, preserving nothing (no
        # completed evidence yet, so all queries are cancelled).
        finalized = await state.runner.run_pending(claimed)
        assert finalized.state == "cancelled"
        assert finalized.queries[0].state == "cancelled"

    async def test_cancel_preserves_completed_evidence_across_owners(self) -> None:
        state, store = _build_state()
        job_store = state.job_store
        job = _job(
            state="running",
            queries=[
                ResearchQuery(
                    index=0, query="done", intent="web", engines=["wikipedia"], state="done", cursor="snap-done"
                ),
                ResearchQuery(index=1, query="pending", intent="web", engines=["wikipedia"]),
            ],
        )
        await job_store.save(job)
        claimed = await job_store.claim(job.job_id, "w1", 60)
        assert claimed is not None

        await job_store.request_cancel(job.job_id)
        finalized = await state.runner.run_pending(claimed)

        assert finalized.state == "cancelled"
        assert finalized.queries[0].state == "done"
        assert finalized.queries[0].cursor == "snap-done"
        assert finalized.queries[1].state == "cancelled"


# ---------------------------------------------------------------------------
# Runner execution with leases
# ---------------------------------------------------------------------------


class TestRunnerExecution:
    async def test_execute_claimed_runs_and_releases_lease(self) -> None:
        state, store = _build_state()
        job_store = state.job_store
        job = _job(
            queries=[
                ResearchQuery(index=0, query="q", intent="web", engines=["wikipedia"]),
            ]
        )
        await job_store.save(job)
        claimed = await job_store.claim(job.job_id, "w1", 60)
        assert claimed is not None

        await state.runner._execute_claimed(claimed)

        loaded = await job_store.load(job.job_id)
        assert loaded is not None
        assert loaded.state == "succeeded"
        assert await job_store._lease_get(_lease_key("default", job.job_id)) is None

    async def test_worker_loss_orphan_recovery_preserves_evidence(self) -> None:
        state, store = _build_state()
        job_store = state.job_store
        job = _job(
            state="running",
            queries=[
                ResearchQuery(
                    index=0,
                    query="done",
                    intent="web",
                    engines=["wikipedia"],
                    state="done",
                    cursor="snap-done",
                    query_id="ssx-done",
                ),
                ResearchQuery(index=1, query="pending", intent="web", engines=["wikipedia"], state="running"),
            ],
        )
        await job_store.save(job)

        # Another replica claims the abandoned job and resumes only the
        # unfinished subquery.
        claimed = await job_store.claim(job.job_id, "w2", 60)
        assert claimed is not None
        await state.runner._execute_claimed(claimed)

        loaded = await job_store.load(job.job_id)
        assert loaded is not None
        assert loaded.state == "succeeded"
        assert loaded.queries[0].cursor == "snap-done"
        assert loaded.queries[1].state == "done"
        assert loaded.queries[1].cursor is not None

    async def test_duplicate_delivery_does_not_duplicate_evidence(self) -> None:
        state, store = _build_state()
        job_store = state.job_store
        engine = state.ctx.active_engines["wikipedia"]
        job = _job(
            queries=[
                ResearchQuery(index=0, query="q", intent="web", engines=["wikipedia"]),
            ]
        )
        await job_store.save(job)

        winners = [
            r
            for r in await asyncio.gather(
                job_store.claim(job.job_id, "w1", 60),
                job_store.claim(job.job_id, "w2", 60),
            )
            if r is not None
        ]
        assert len(winners) == 1

        await state.runner._execute_claimed(winners[0])

        loaded = await job_store.load(job.job_id)
        assert loaded is not None
        assert loaded.state == "succeeded"
        # Exactly one execution: no duplicate subquery dispatch.
        assert engine.calls == 1
        assert len(loaded.queries[0].attempts) == 1

    async def test_enqueue_fast_path_carries_tenant(self) -> None:
        state, store = _build_state()
        state.runner.enqueue("job-123", tenant="tenant-x")
        assert state.runner._next_local() == ("tenant-x", "job-123")
        assert state.runner._next_local() is None


# ---------------------------------------------------------------------------
# Direct runs after durable execution (retry/extend on stale lease fields)
# ---------------------------------------------------------------------------


class TestDirectRunsAfterDurableExecution:
    async def test_retry_clears_stale_lease_and_reruns(self) -> None:
        state, store = _build_state()
        job_store = state.job_store
        state.ctx.active_engines["wikipedia"] = _MockEngine("wikipedia", status=EngineStatus.ERROR)
        job = _job(queries=[ResearchQuery(index=0, query="q", intent="web", engines=["wikipedia"])])
        await job_store.save(job)

        claimed = await job_store.claim(job.job_id, "w1", 60)
        assert claimed is not None
        await state.runner._execute_claimed(claimed)

        loaded = await job_store.load(job.job_id)
        assert loaded is not None
        assert loaded.state == "failed"
        # The lease key is gone but the record fields survive the release.
        assert loaded.lease_token is not None
        assert await job_store._lease_get(_lease_key("default", job.job_id)) is None

        # A retry of a previously durable-executed job must not raise
        # LeaseLostError against the released lease and must re-run the work.
        state.ctx.active_engines["wikipedia"] = _MockEngine("wikipedia")
        result = await state.runner.retry(job.job_id, tenant="default")

        assert result is not None
        assert result.state == "succeeded"
        assert result.queries[0].state == "done"
        assert result.owner_id is None
        assert result.lease_token is None


# ---------------------------------------------------------------------------
# Lease ownership guard for subquery persistence
# ---------------------------------------------------------------------------


class TestLeaseOwnershipGuard:
    async def test_stale_owner_does_not_persist_after_lease_reclaimed(self) -> None:
        state, store = _build_state(lease_ttl=1)
        job_store = state.job_store
        state.ctx.active_engines["wikipedia"] = _SlowEngine("wikipedia", delay=2.0)
        job = _job(queries=[ResearchQuery(index=0, query="q", intent="web", engines=["wikipedia"])])
        await job_store.save(job)

        claimed = await job_store.claim(job.job_id, "w1", 1)
        assert claimed is not None

        task = asyncio.create_task(state.runner._execute_claimed(claimed))

        # Let the original owner start the subquery and let its lease expire
        # (the engine call takes 2s vs a 1s lease).
        await asyncio.sleep(1.5)

        # Another replica reclaims the abandoned job: it resets the in-flight
        # subquery to pending and installs a fresh lease token.
        second = await job_store.claim(job.job_id, "w2", 60)
        assert second is not None

        await task

        loaded = await job_store.load(job.job_id)
        assert loaded is not None
        # The stale owner must not have written its done result over the
        # reclaimer's reset; the job is still pending under w2.
        assert loaded.queries[0].state == "pending"
        assert loaded.owner_id == "w2"
        assert loaded.lease_token == second.lease_token


# ---------------------------------------------------------------------------
# Key scanning backend selection
# ---------------------------------------------------------------------------


class TestScanKeys:
    async def test_scan_keys_prefers_scan_iter_over_keys(self) -> None:
        store = ScanPreferredStore()
        job_store = ResearchJobStore(store)

        ids = await job_store._scan_job_ids()

        assert ids == ["job-a", "job-b"]
        assert store._client.keys_calls == 0


# ---------------------------------------------------------------------------
# Tenant identity and isolation
# ---------------------------------------------------------------------------


class TestTenantIdentity:
    def test_current_tenant_default_and_override(self) -> None:
        assert state_mod.current_tenant() == "default"
        with tenant_scope("tenant-x"):
            assert state_mod.current_tenant() == "tenant-x"
        assert state_mod.current_tenant() == "default"

    def test_current_tenant_derives_from_oauth_client_id(self, monkeypatch: pytest.MonkeyPatch) -> None:
        class _FakeToken:
            client_id = "client-abc"
            subject = "operator"

        monkeypatch.setattr(state_mod, "_access_token", lambda: _FakeToken())
        assert state_mod.current_tenant() == "client-abc"

    def test_current_tenant_override_wins_over_oauth(self, monkeypatch: pytest.MonkeyPatch) -> None:
        class _FakeToken:
            client_id = "client-abc"

        monkeypatch.setattr(state_mod, "_access_token", lambda: _FakeToken())
        with tenant_scope("tenant-override"):
            assert state_mod.current_tenant() == "tenant-override"


class TestTenantIsolation:
    async def test_for_tenant_scopes_jobs(self) -> None:
        _, store = _build_state()
        base = ResearchJobStore(store)
        job_a = _job(tenant="a")
        await base.for_tenant("a").save(job_a)

        assert await base.for_tenant("a").load(job_a.job_id) is not None
        assert await base.for_tenant("b").load(job_a.job_id) is None
        assert await base.load(job_a.job_id) is None  # default tenant

    async def test_start_research_tenant_isolated(self) -> None:
        state, _store = _build_state()
        set_state(state)
        try:
            with tenant_scope("tenant-a"):
                r_a = await t.slopsearx_start_research("q", max_queries=1, max_engines_per_query=1)
            with tenant_scope("tenant-b"):
                r_b = await t.slopsearx_start_research("q", max_queries=1, max_engines_per_query=1)
            assert "error" not in r_a and "error" not in r_b
            assert r_a["job_id"] != r_b["job_id"]

            with tenant_scope("tenant-a"):
                got_a = await t.slopsearx_get_job(r_a["job_id"])
                assert "error" not in got_a
                got_b = await t.slopsearx_get_job(r_b["job_id"])
                assert got_b["error"]["code"] == "invalid_job_id"
            with tenant_scope("tenant-b"):
                got_b = await t.slopsearx_get_job(r_b["job_id"])
                assert "error" not in got_b
        finally:
            set_state(None)

    async def test_snapshot_tenant_isolation(self) -> None:
        _, store = _build_state()
        snapshots = SnapshotStore(store)
        snapshot_id = await snapshots.for_tenant("a").create("q", "ssx-1", [], ScopeDecision())
        assert snapshot_id is not None
        assert await snapshots.for_tenant("a").get(snapshot_id) is not None
        assert await snapshots.for_tenant("b").get(snapshot_id) is None
