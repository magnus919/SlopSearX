"""Valkey research persistence, ready indexes and fenced lease coordination.

The durable job and lease records remain authoritative; the ready index is a
repairable discovery aid. Execution and planning live in research.py.
"""

from __future__ import annotations

import asyncio
import json
import logging
import time
from dataclasses import replace
from typing import Any, Callable

from slopsearx import metrics as m
from slopsearx.research_models import (
    LeaseLostError,
    ResearchJob,
    _job_from_payload,
    _job_to_payload,
    generate_lease_token,
    recover_orphan_attempts,
)
from slopsearx.snapshot import KeyValueStore

logger = logging.getLogger(__name__)


def _workflow_kind(job: ResearchJob) -> m.WorkflowKind:
    return "dependency_dossier" if job.workflow.get("kind") == "dependency_dossier" else "research"


JOB_KEY_PREFIX = "mcp:job"
IDEMPOTENCY_PREFIX = "mcp:idem"
LEASE_KEY_PREFIX = "mcp:joblease"
CANCEL_KEY_PREFIX = "mcp:jobcancel"
JOB_RETENTION_SECONDS = 86_400  # 24 hours
DEFAULT_JOB_LEASE_TTL_SECONDS = 60
DEFAULT_JOB_POLL_INTERVAL_SECONDS = 1.0


# Ready indexes are derived state; job records and lease tokens remain authority.
READY_PREFIX = "mcp:ready:v1"
RECENT_PREFIX = "mcp:recent:v1"
MAX_RECENT_JOBS = 200
_RECENT_RECORD_SCRIPT = """
redis.call('ZADD', KEYS[1], ARGV[1], ARGV[2])
local count = redis.call('ZCARD', KEYS[1])
if count > tonumber(ARGV[3]) then
  redis.call('ZREMRANGEBYRANK', KEYS[1], 0, count - tonumber(ARGV[3]) - 1)
end
redis.call('EXPIRE', KEYS[1], ARGV[4])
return 1
"""
_RECONCILE_INTERVAL = 10
_RECONCILE_BATCH = 128
_READY_REFRESH_SCRIPT = """
local raw = redis.call('GET', KEYS[1])
local now = redis.call('TIME')
local due = tonumber(now[1]) + tonumber(now[2]) / 1000000
local active = false
if raw then
    local ok, job = pcall(cjson.decode, raw)
    active = ok and type(job) == "table" and job.tenant == ARGV[1] and
        (job.state == 'queued' or job.state == 'running')
end
if active then
    local ttl = redis.call('PTTL', KEYS[2])
    if ttl > 0 then due = due + ttl / 1000 end
    redis.call('ZADD', KEYS[3], due, ARGV[2])
    redis.call('ZADD', KEYS[4], 'NX', 0, ARGV[1])
else
    redis.call('ZREM', KEYS[3], ARGV[2])
    if redis.call('ZCARD', KEYS[3]) == 0 then
        redis.call('ZREM', KEYS[4], ARGV[1])
    end
end
return 1
"""

_READY_TAKE_SCRIPT = """
local now = redis.call('TIME')
local due = tonumber(now[1]) + tonumber(now[2]) / 1000000
for i = 1, 16 do
    local tenants = redis.call('ZRANGE', KEYS[1], 0, 0)
    if #tenants == 0 then return {} end
    local tenant = tenants[1]
    local ready = ARGV[1] .. ':tenant:' .. tenant
    local jobs = redis.call('ZRANGEBYSCORE', ready, '-inf', due, 'LIMIT', 0, 1)
    if redis.call('ZCARD', ready) == 0 then
        redis.call('ZREM', KEYS[1], tenant)
    else
        redis.call('ZADD', KEYS[1], redis.call('INCR', KEYS[2]), tenant)
    end
    if #jobs > 0 then
        -- Reservation is non-destructive: worker death before lease acquisition
        -- makes this candidate visible again without waiting for reconciliation.
        redis.call('ZADD', ready, due + 5, jobs[1])
        return {tenant, jobs[1]}
    end
end
return {}
"""

# Atomic compare-and-set used by :meth:`ResearchJobStore.save_if_owned`.
# KEYS[1] is the lease key, KEYS[2] is the job-record key; ARGV[1] is the
# lease token, ARGV[2] the record TTL, ARGV[3] the serialized job payload.
# The check (does this token still own the lease) and the write happen in a
# single Lua call so a concurrent reclamation cannot race between them.
_LEASE_SAVE_SCRIPT = """
if redis.call('GET', KEYS[1]) ~= ARGV[1] then
    return 0
end
redis.call('SETEX', KEYS[2], ARGV[2], ARGV[3])
return 1
"""

# Atomic lease renewal used by :meth:`ResearchJobStore._lease_renew`.
# KEYS[1] is the lease key; ARGV[1] is the lease token, ARGV[2] the new TTL.
# The check (does this token still own the lease) and the SETEX happen in a
# single Lua call so a concurrent reclamation cannot race between them.
_LEASE_RENEW_SCRIPT = """
if redis.call('GET', KEYS[1]) ~= ARGV[1] then
    return 0
end
redis.call('SETEX', KEYS[1], ARGV[2], ARGV[1])
return 1
"""

# Atomic lease release used by :meth:`ResearchJobStore._lease_release`.
# KEYS[1] is the lease key; ARGV[1] is the lease token. The check (does this
# token still own the lease) and the DEL happen in a single Lua call.
_LEASE_RELEASE_SCRIPT = """
if redis.call('GET', KEYS[1]) ~= ARGV[1] then
    return 0
end
redis.call('DEL', KEYS[1])
return 1
"""

_IDEMPOTENT_CREATE_SCRIPT = """
local existing = redis.call('GET', KEYS[1])
if existing then
    local ok, record = pcall(cjson.decode, existing)
    if ok and type(record) == 'table' and record.job_id then
        return {0, record.job_id}
    end
    return {-1, ''}
end
redis.call('SETEX', KEYS[2], ARGV[1], ARGV[2])
redis.call('SETEX', KEYS[1], ARGV[1], ARGV[3])
return {1, ARGV[4]}
"""


# ---------------------------------------------------------------------------
# Storage
# ---------------------------------------------------------------------------


class ResearchJobStore:
    """Valkey-backed persistence and durable execution coordination.

    Beyond plain persistence, this store implements a distributed
    claim/lease model so multiple replicas can execute research jobs
    without double delivery:

    - :meth:`claim` atomically moves a ``queued`` (or lease-expired
      ``running``) job to ``running`` under a unique lease token. The
      atomic primitive is Valkey ``SET NX`` (or an equivalent in-memory
      store primitive), so exactly one replica wins.
    - :meth:`renew` extends the visibility timeout while a worker is
      still executing; :meth:`release` drops it on completion.
    - :meth:`request_cancel` records a durable, race-free cancellation
      flag and finalizes immediately when no worker holds the lease.
    """

    def __init__(
        self,
        store: KeyValueStore | None,
        tenant: str = "default",
        *,
        admission_locks: dict[str, asyncio.Lock] | None = None,
    ) -> None:
        self._store = store
        self._tenant = tenant
        self._admission_locks = admission_locks if admission_locks is not None else {}

    @property
    def available(self) -> bool:
        return self._store is not None and self._store.is_connected

    @property
    def durable(self) -> bool:
        """Whether the backing store is shared Valkey (durable across replicas)."""
        store = self._store
        if store is None or not store.is_connected:
            return False
        client = getattr(store, "_client", None)
        return client is not None and hasattr(client, "get") and hasattr(client, "set")

    def for_tenant(self, tenant: str) -> "ResearchJobStore":
        """Return a tenant-scoped view sharing the same backing store."""
        if tenant == self._tenant:
            return self
        return ResearchJobStore(self._store, tenant=tenant, admission_locks=self._admission_locks)

    def _key(self, job_id: str) -> str:
        return f"{JOB_KEY_PREFIX}:{self._tenant}:{job_id}"

    def _idem_key(self, idempotency_key: str) -> str:
        return f"{IDEMPOTENCY_PREFIX}:{self._tenant}:{idempotency_key}"

    def _lease_key(self, job_id: str) -> str:
        return f"{LEASE_KEY_PREFIX}:{self._tenant}:{job_id}"

    def _cancel_key(self, job_id: str) -> str:
        return f"{CANCEL_KEY_PREFIX}:{self._tenant}:{job_id}"

    def _recent_key(self) -> str:
        return f"{RECENT_PREFIX}:{self._tenant}"

    async def _record_recent(self, job: ResearchJob) -> None:
        """Maintain the bounded tenant index used by workflow explorers."""
        store = self._store
        if store is None or not store.is_connected:
            return
        client = getattr(store, "_client", None)
        if client is not None and hasattr(client, "eval"):
            await client.eval(
                _RECENT_RECORD_SCRIPT,
                1,
                self._recent_key(),
                str(job.created_at),
                job.job_id,
                str(MAX_RECENT_JOBS),
                str(JOB_RETENTION_SECONDS),
            )
            return
        async with self._admission_locks.setdefault(f"recent:{self._tenant}", asyncio.Lock()):
            current = await store.get(self._recent_key()) or {}
            index = {str(key): float(value) for key, value in current.items()}
            index[job.job_id] = job.created_at
            ordered = sorted(index.items(), key=lambda item: (item[1], item[0]), reverse=True)[:MAX_RECENT_JOBS]
            await store.set(self._recent_key(), dict(ordered), JOB_RETENTION_SECONDS)

    async def save(self, job: ResearchJob) -> None:
        """Persist a job. No-op when the store is unavailable."""
        store = self._store
        if store is None or not store.is_connected:
            return
        payload = _job_to_payload(job)
        await store.set(self._key(job.job_id), payload, JOB_RETENTION_SECONDS)
        await self._record_recent(job)
        await self._refresh_ready(job.job_id)
        if job.idempotency_key:
            await store.set(
                self._idem_key(job.idempotency_key),
                {"job_id": job.job_id},
                JOB_RETENTION_SECONDS,
            )

    async def save_if_owned(self, job: ResearchJob) -> bool:
        """Persist a job only if the caller still owns its lease.

        Returns ``True`` when the job was persisted (or when it has no
        lease, e.g. a direct retry/extend run) and ``False`` when the lease
        was lost and the write was skipped. On Valkey the lease check and the
        record write happen in one atomic Lua call (compare-and-set) so a
        concurrent reclamation cannot race between them. Non-Valkey backends
        must expose the equivalent ``save_if_lease_owner`` atomic primitive;
        unsupported backends fail closed.
        """
        token = job.lease_token
        if not token:
            await self.save(job)
            return True
        store = self._store
        if store is None or not store.is_connected:
            return False
        client = getattr(store, "_client", None)
        eval_method = getattr(client, "eval", None) if client is not None else None
        if eval_method is not None:
            return await self._save_if_owned_valkey(eval_method, job, token)
        compare_and_set = getattr(store, "save_if_lease_owner", None)
        if compare_and_set is None:
            return False
        saved = bool(
            await compare_and_set(
                self._lease_key(job.job_id),
                token,
                self._key(job.job_id),
                _job_to_payload(job),
                JOB_RETENTION_SECONDS,
            )
        )
        if saved:
            await self._refresh_ready(job.job_id)
        return saved

    async def clear_ownership(self, job: ResearchJob) -> bool:
        """Clear record ownership while the old token still fences the write.

        The caller releases that token afterwards. This avoids an unguarded
        record write after another replica can acquire the released lease.
        """
        token = job.lease_token
        if not token or not self.available:
            return False
        cleared = replace(job, owner_id=None, lease_token=None, lease_expires_at=0.0)
        client = getattr(self._store, "_client", None)
        evaluate = getattr(client, "eval", None)
        if evaluate is not None:
            saved = await self._save_if_owned_valkey(evaluate, cleared, token)
        else:
            compare_and_set = getattr(self._store, "save_if_lease_owner", None)
            if compare_and_set is None:
                return False
            saved = bool(
                await compare_and_set(
                    self._lease_key(job.job_id),
                    token,
                    self._key(job.job_id),
                    _job_to_payload(cleared),
                    JOB_RETENTION_SECONDS,
                )
            )
            if saved:
                await self._refresh_ready(job.job_id)
        if saved:
            job.owner_id = job.lease_token = None
            job.lease_expires_at = 0.0
        return saved

    async def _save_if_owned_valkey(self, eval_method: Any, job: ResearchJob, token: str) -> bool:
        """Atomic compare-and-set: persist ``job`` only if ``token`` still owns the lease."""
        payload = json.dumps(_job_to_payload(job), default=str)
        try:
            result = await eval_method(
                _LEASE_SAVE_SCRIPT,
                2,
                self._lease_key(job.job_id),
                self._key(job.job_id),
                token,
                str(JOB_RETENTION_SECONDS),
                payload,
            )
        except Exception:  # noqa: BLE001 — lease loss / transient store error
            return False
        if result:
            await self._refresh_ready(job.job_id)
        return bool(result)

    async def load(self, job_id: str) -> ResearchJob | None:
        """Load a job by ID, merging the durable cancellation flag.

        Returns ``None`` when missing/unavailable or when the record's
        tenant does not match this store's tenant.
        """
        store = self._store
        if store is None or not store.is_connected:
            return None
        payload = await store.get(self._key(job_id))
        if payload is None:
            return None
        job = _job_from_payload(payload)
        if job.tenant != self._tenant:
            return None
        # Merge the race-free cancellation signal (a separate key, so a
        # concurrent worker's job-record writes can never clobber it).
        cancel_payload = await store.get(self._cancel_key(job_id))
        if cancel_payload and cancel_payload.get("cancel_requested"):
            job.cancel_requested = True
        return job

    async def find_by_idempotency(self, idempotency_key: str) -> ResearchJob | None:
        """Return the job previously created with this idempotency key."""
        store = self._store
        if store is None or not store.is_connected or not idempotency_key:
            return None
        payload = await store.get(self._idem_key(idempotency_key))
        if payload is None:
            return None
        job_id = str(payload.get("job_id", ""))
        if not job_id:
            return None
        return await self.load(job_id)

    async def create_idempotent(self, job: ResearchJob) -> tuple[ResearchJob | None, bool]:
        """Atomically create ``job`` or return the record owning its key.

        Real Valkey uses one Lua transaction for the job and idempotency
        records. Lightweight in-memory stores are serialized by a shared
        per-tenant lock so deterministic tests exercise the same admission
        semantics.
        """
        if not job.idempotency_key:
            await self.save(job)
            return job, True
        store = self._store
        if store is None or not store.is_connected:
            return None, False
        client = getattr(store, "_client", None)
        eval_method = getattr(client, "eval", None) if client is not None else None
        if eval_method is not None:
            encoded_job = json.dumps(_job_to_payload(job), default=str)
            encoded_idem = json.dumps({"job_id": job.job_id})
            result = await eval_method(
                _IDEMPOTENT_CREATE_SCRIPT,
                2,
                self._idem_key(job.idempotency_key),
                self._key(job.job_id),
                str(JOB_RETENTION_SECONDS),
                encoded_job,
                encoded_idem,
                job.job_id,
            )
            created = int(result[0]) == 1
            if int(result[0]) < 0:
                return None, False
            raw_job_id = result[1]
            resolved_id = raw_job_id.decode() if isinstance(raw_job_id, bytes) else str(raw_job_id)
            if created:
                await self._refresh_ready(job.job_id)
                await self._record_recent(job)
            return await self.load(resolved_id), created

        lock = self._admission_locks.setdefault(self._tenant, asyncio.Lock())
        async with lock:
            existing = await self.find_by_idempotency(job.idempotency_key)
            if existing is not None:
                return existing, False
            await self.save(job)
            return job, True

    async def list_recent(self, *, before: tuple[float, str] | None = None, limit: int = 20) -> list[ResearchJob]:
        """Read one deterministic page from the maintained tenant index."""
        if not self.available or not 1 <= limit <= MAX_RECENT_JOBS:
            return []
        store = self._store
        assert store is not None
        client = getattr(store, "_client", None)
        pairs: list[tuple[str, float]] = []
        if client is not None and hasattr(client, "zrevrange"):
            raw = await client.zrevrange(self._recent_key(), 0, MAX_RECENT_JOBS - 1, withscores=True)
            pairs = [(item.decode() if isinstance(item, bytes) else str(item), float(score)) for item, score in raw]
        else:
            payload = await store.get(self._recent_key()) or {}
            pairs = sorted(
                ((str(key), float(value)) for key, value in payload.items()),
                key=lambda item: (item[1], item[0]),
                reverse=True,
            )
        if before is not None:
            pairs = [item for item in pairs if (item[1], item[0]) < before]
        jobs: list[ResearchJob] = []
        stale: list[str] = []
        loaded = await asyncio.gather(*(self.load(job_id) for job_id, _score in pairs))
        for (job_id, _score), job in zip(pairs, loaded, strict=True):
            if job is None:
                stale.append(job_id)
                continue
            jobs.append(job)
            if len(jobs) == limit:
                break
        if stale:
            if client is not None and hasattr(client, "zrem"):
                await client.zrem(self._recent_key(), *stale)
            else:
                payload = await store.get(self._recent_key()) or {}
                for job_id in stale:
                    payload.pop(job_id, None)
                await store.set(self._recent_key(), payload, JOB_RETENTION_SECONDS)
        return jobs

    async def _scan_job_ids(self) -> list[str]:
        """List this tenant's persisted job IDs."""
        store = self._store
        if store is None or not store.is_connected:
            return []
        prefix = f"{JOB_KEY_PREFIX}:{self._tenant}:"
        keys = await _scan_keys(store, f"{JOB_KEY_PREFIX}:{self._tenant}:*")
        return [key[len(prefix) :] for key in keys if key.startswith(prefix)]

    async def scan_tenants(self) -> list[str]:
        """Enumerate the distinct tenant namespaces that have job records.

        Job keys are ``{JOB_KEY_PREFIX}:{tenant}:{job_id}`` and job ids never
        contain ``:``, so the tenant is everything between the prefix and the
        FINAL colon. Parsing on the first colon would truncate a tenant whose
        name itself contains ``:`` (e.g. an OAuth ``client_id``), and its jobs
        would never be claimed by the durable poll loop.
        """
        store = self._store
        if store is None or not store.is_connected:
            return []
        prefix = f"{JOB_KEY_PREFIX}:"
        tenants: set[str] = set()
        for key in await _scan_keys(store, f"{JOB_KEY_PREFIX}:*"):
            if not key.startswith(prefix):
                continue
            rest = key[len(prefix) :]
            if ":" in rest:
                tenants.add(rest.rsplit(":", 1)[0])
        return sorted(tenants)

    async def expire_stale_running(self) -> int:
        """Expire unowned ``running`` jobs whose deadline has passed.

        An unowned ``running`` job with a future deadline is left alone so the
        durable worker loop can reclaim and resume it (orphan recovery). A job
        whose deadline has already passed cannot make progress, so it is
        finalized to ``expired`` here. Jobs still held by a live lease are
        never touched. Returns the number expired.
        """
        store = self._store
        if store is None or not store.is_connected:
            return 0
        expired = 0
        now = time.time()
        try:
            for job_id in await self._scan_job_ids():
                job = await self.load(job_id)
                if job is None or job.state != "running":
                    continue
                if job.deadline > now:
                    continue
                claimed = await self._claim_prepared(job, "expiry", DEFAULT_JOB_LEASE_TTL_SECONDS)
                if claimed is None:
                    continue
                token = claimed.lease_token
                try:
                    if claimed.state == "expired":
                        expired += 1
                        workflow = _workflow_kind(claimed)
                        m.record_workflow_recovery(workflow)
                        m.transition_workflow(workflow, "running", "expired")
                        m.record_workflow_expiry(workflow, "job")
                        m.record_workflow_terminal(workflow, "expired")
                    if not await self.clear_ownership(claimed):
                        raise LeaseLostError(job_id)
                finally:
                    if token:
                        await self._lease_release(self._lease_key(job_id), token)

        except Exception as exc:  # noqa: BLE001 — graceful degradation
            logger.warning("ResearchJobStore: stale-job scan failed: %s", exc)
        return expired

    # -- lease primitives -------------------------------------------------

    async def _lease_get(self, key: str) -> str | None:
        """Return the lease token held at ``key``, or None."""
        store = self._store
        if store is None or not store.is_connected:
            return None
        client = getattr(store, "_client", None)
        if client is not None and hasattr(client, "get") and hasattr(client, "set"):
            try:
                raw = await client.get(key)
            except Exception:  # noqa: BLE001 — graceful degradation
                return None
            if raw is None:
                return None
            return raw.decode() if isinstance(raw, bytes) else str(raw)
        value: Any = await store.get(key)
        if value is None:
            return None
        if isinstance(value, dict):
            token = value.get("token")
            return str(token) if token else None
        return str(value)

    async def _lease_acquire(self, key: str, token: str, ttl: int) -> bool:
        """Atomically acquire a lease (SET NX) and return success."""
        store = self._store
        if store is None or not store.is_connected:
            return False
        client = getattr(store, "_client", None)
        if client is not None and hasattr(client, "set") and hasattr(client, "get"):
            try:
                result = await client.set(key, token, nx=True, ex=ttl)
            except Exception:  # noqa: BLE001 — graceful degradation
                return False
            return bool(result)
        method = getattr(store, "acquire_lease", None)
        if method is not None:
            return bool(await method(key, token, ttl))
        method = getattr(store, "set_nx", None)
        if method is not None:
            return bool(await method(key, {"token": token}, ttl))
        # Non-atomic fallback for single-process stores (safe because no
        # concurrent replica shares the process).
        if await self._lease_get(key) is not None:
            return False
        await store.set(key, {"token": token}, ttl)
        return True

    async def _lease_renew(self, key: str, token: str, ttl: int) -> bool:
        """Extend a lease only if it is still held by ``token``.

        On a Valkey client this is a single atomic Lua compare-and-set
        (GET + compare + SETEX in one EVAL), so a stale owner whose lease was
        reclaimed cannot renew over the reclaimer's token. Falls back to the
        non-Lua check-then-set path only when the client has no ``eval``.
        """
        store = self._store
        if store is None or not store.is_connected:
            return False
        client = getattr(store, "_client", None)
        eval_method = getattr(client, "eval", None) if client is not None else None
        if eval_method is not None:
            try:
                return bool(await eval_method(_LEASE_RENEW_SCRIPT, 1, key, token, str(ttl)))
            except Exception:  # noqa: BLE001 — graceful degradation
                return False
        if client is not None and hasattr(client, "get") and hasattr(client, "set"):
            try:
                current = await client.get(key)
                if current is None:
                    return False
                current_s = current.decode() if isinstance(current, bytes) else str(current)
                if current_s != token:
                    return False
                await client.set(key, token, ex=ttl)
                return True
            except Exception:  # noqa: BLE001 — graceful degradation
                return False
        method = getattr(store, "renew_lease", None)
        if method is not None:
            return bool(await method(key, token, ttl))
        if await self._lease_get(key) != token:
            return False
        await store.set(key, {"token": token}, ttl)
        return True

    async def _lease_release(self, key: str, token: str) -> bool:
        """Release a lease only if it is still held by ``token``.

        On a Valkey client this is a single atomic Lua compare-and-delete
        (GET + compare + DEL in one EVAL), so a stale owner whose lease was
        reclaimed cannot delete the reclaimer's lease. Falls back to the
        non-Lua check-then-delete path only when the client has no ``eval``.
        """
        store = self._store
        if store is None or not store.is_connected:
            return False
        client = getattr(store, "_client", None)
        eval_method = getattr(client, "eval", None) if client is not None else None
        if eval_method is not None:
            try:
                return bool(await eval_method(_LEASE_RELEASE_SCRIPT, 1, key, token))
            except Exception:  # noqa: BLE001 — graceful degradation
                return False
        if client is not None and hasattr(client, "get") and hasattr(client, "delete"):
            try:
                current = await client.get(key)
                if current is None:
                    return False
                current_s = current.decode() if isinstance(current, bytes) else str(current)
                if current_s != token:
                    return False
                await client.delete(key)
                return True
            except Exception:  # noqa: BLE001 — graceful degradation
                return False
        method = getattr(store, "release_lease", None)
        if method is not None:
            return bool(await method(key, token))
        if await self._lease_get(key) != token:
            return False
        delete = getattr(store, "delete", None)
        if delete is not None:
            await delete(key)
            return True
        data = getattr(store, "_data", None)
        if data is not None:
            data.pop(key, None)
            return True
        return False

    # -- durable claim / lease lifecycle ----------------------------------

    async def claim(self, job_id: str, owner_id: str, lease_ttl: int) -> ResearchJob | None:
        """Atomically claim a claimable job for ``owner_id``.

        Claimable means ``queued`` or ``running`` with an expired/absent
        lease. On success the job is persisted as ``running`` under a fresh
        lease and any subquery left ``running`` by a dead owner is reset to
        ``pending`` (orphan recovery). Returns ``None`` when the job is not
        claimable or another replica won the race.
        """
        store = self._store
        if store is None or not store.is_connected:
            return None
        # Cheap pre-filter: avoid lease churn on terminal jobs.
        pre = await self.load(job_id)
        if pre is None or pre.state not in ("queued", "running"):
            return None
        token = generate_lease_token()
        if not await self._lease_acquire(self._lease_key(job_id), token, lease_ttl):
            return None
        try:
            # Re-load under the lease; every subsequent write is token-fenced.
            job = await self.load(job_id)
            if job is None or job.state not in ("queued", "running"):
                await self._lease_release(self._lease_key(job_id), token)
                return None
            recover_orphan_attempts(job)
            workflow = _workflow_kind(job)
            metric_previous = pre.state
            if pre.state == "running":
                m.record_workflow_recovery(workflow)
                # A replacement process starts with empty gauges, while a
                # surviving process may still hold the dead owner's running
                # observation. Normalizing through the recovery boundary
                # state makes both cases converge on one active lease.
                m.transition_workflow(workflow, "running", "interrupted")
                metric_previous = "interrupted"
            job.owner_id = owner_id
            job.lease_token = token
            job.lease_expires_at = time.time() + lease_ttl
            if time.time() >= job.deadline:
                for query in job.queries:
                    if query.state in ("pending", "running"):
                        query.state = "cancelled"
                job.state = "expired"
                job.stop_reason = "deadline_expired"
                if not await self.save_if_owned(job):
                    raise LeaseLostError(job_id)
                m.transition_workflow(workflow, metric_previous, "expired")
                m.record_workflow_expiry(workflow, "job")
                m.record_workflow_terminal(workflow, "expired")
                await self._lease_release(self._lease_key(job_id), token)
                return None
            for query in job.queries:
                if query.state == "running":
                    query.state = "pending"
            job.state = "running"
            if not await self.save_if_owned(job):
                raise LeaseLostError(job_id)
            m.transition_workflow(workflow, metric_previous, "running")
            m.workflow_queue_wait.observe({"workflow": workflow}, max(0.0, time.time() - job.created_at))
            return job
        except BaseException:
            await self._lease_release(self._lease_key(job_id), token)
            raise

    async def _claim_prepared(
        self,
        job: ResearchJob,
        owner_id: str,
        lease_ttl: int,
        mutate: Callable[[ResearchJob], None] | None = None,
    ) -> ResearchJob | None:
        """Claim a job under a fresh lease, re-applying the caller's mutation.

        Mirrors :meth:`claim`, but re-applies the caller's direct-run mutation
        (reset-to-pending retry queries or an appended follow-up) to the
        authoritative record *after* re-loading it under the lease, instead of
        persisting the caller-supplied copy. The lease acquisition is the
        atomic exclusion gate; the re-load validates claimability and the
        deadline before writing, so a record concurrently finalized by another
        worker is never clobbered.
        """
        store = self._store
        if store is None or not store.is_connected:
            return None
        token = generate_lease_token()
        if not await self._lease_acquire(self._lease_key(job.job_id), token, lease_ttl):
            return None
        try:
            current = await self.load(job.job_id)
            if current is None:
                await self._lease_release(self._lease_key(job.job_id), token)
                return None
            previous_state = current.state
            recover_orphan_attempts(current)
            current.owner_id = owner_id
            current.lease_token = token
            current.lease_expires_at = time.time() + lease_ttl
            current.cancel_requested = current.cancel_requested or job.cancel_requested
            if current.state in ("cancelled", "expired") or current.caller_completed:
                pass
            elif time.time() >= current.deadline:
                for query in current.queries:
                    if query.state in ("pending", "running"):
                        query.state = "cancelled"
                current.state = "expired"
                current.stop_reason = "deadline_expired"
            elif current.cancel_requested:
                for query in current.queries:
                    if query.state in ("pending", "running"):
                        query.state = "cancelled"
                current.state = "cancelled"
                current.stop_reason = "cancelled"
            else:
                if mutate is not None:
                    mutate(current)
                for query in current.queries:
                    if query.state == "running":
                        query.state = "pending"
                if any(query.state == "pending" for query in current.queries):
                    current.state = "running"
            if not await self.save_if_owned(current):
                raise LeaseLostError(job.job_id)
            if current.state == "running":
                workflow = _workflow_kind(current)
                metric_previous = previous_state
                if previous_state == "running":
                    m.record_workflow_recovery(workflow)
                    m.transition_workflow(workflow, "running", "interrupted")
                    metric_previous = "interrupted"
                m.transition_workflow(workflow, metric_previous, "running")
            return current
        except BaseException:
            await self._lease_release(self._lease_key(job.job_id), token)
            raise

    def _ready_client(self) -> Any:
        """Real sorted-set backend; simple injected stores keep their scan seam."""
        client = getattr(self._store, "_client", None)
        if client is not None and all(hasattr(client, name) for name in ("eval", "scan", "zrangebyscore")):
            return client
        return None

    async def _refresh_ready(self, job_id: str) -> None:
        client = self._ready_client()
        if client is None:
            return
        try:
            await client.eval(
                _READY_REFRESH_SCRIPT,
                4,
                self._key(job_id),
                self._lease_key(job_id),
                f"{READY_PREFIX}:tenant:{self._tenant}",
                f"{READY_PREFIX}:tenants",
                self._tenant,
                job_id,
            )
        except Exception as exc:  # Derived state cannot invalidate an authoritative write/lease.
            logger.warning("Research ready-index refresh failed (%s); reconciliation will retry", type(exc).__name__)

    async def _reconcile_ready(self) -> None:
        """At most one SCAN page per shared interval, not one scan per worker.

        Persist the cursor after indexing so a crash repeats rather than skips
        records. The throttle is advisory: refresh is idempotent and reads the
        authoritative record atomically, making duplicate reconciliation safe.
        """
        client = self._ready_client()
        if client is None or not await client.set(
            f"{READY_PREFIX}:reconcile-lock", "1", nx=True, ex=_RECONCILE_INTERVAL
        ):
            return
        cursor = await client.get(f"{READY_PREFIX}:cursor") or 0
        next_cursor, keys = await client.scan(cursor=cursor, match=f"{JOB_KEY_PREFIX}:*", count=_RECONCILE_BATCH)
        prefix = f"{JOB_KEY_PREFIX}:"
        for raw in keys:
            key = raw.decode() if isinstance(raw, bytes) else str(raw)
            if not key.startswith(prefix) or ":" not in key[len(prefix) :]:
                continue
            tenant, job_id = key[len(prefix) :].rsplit(":", 1)
            await self.for_tenant(tenant)._refresh_ready(job_id)
        await client.set(f"{READY_PREFIX}:cursor", str(next_cursor))

    async def _claim_ready(self, owner_id: str, lease_ttl: int) -> ResearchJob | None:
        client = self._ready_client()
        await self._reconcile_ready()
        candidate = await client.eval(
            _READY_TAKE_SCRIPT, 2, f"{READY_PREFIX}:tenants", f"{READY_PREFIX}:turn", READY_PREFIX
        )
        if not candidate:
            return None
        tenant, job_id = (value.decode() if isinstance(value, bytes) else str(value) for value in candidate)
        scoped = self.for_tenant(tenant)
        job = await scoped.claim(job_id, owner_id, lease_ttl)
        await scoped._refresh_ready(job_id)
        return job

    async def claim_next(self, owner_id: str, lease_ttl: int) -> ResearchJob | None:
        """Claim the next claimable job for this store's tenant."""
        client = self._ready_client()
        if client is not None:
            await self._reconcile_ready()
            # Scoped callers never claim a different tenant's work.
            now = await client.time()
            due = int(now[0]) + int(now[1]) / 1_000_000
            candidates = await client.zrangebyscore(
                f"{READY_PREFIX}:tenant:{self._tenant}", "-inf", due, start=0, num=16
            )
            for raw in candidates:
                job_id = raw.decode() if isinstance(raw, bytes) else str(raw)
                job = await self.claim(job_id, owner_id, lease_ttl)
                await self._refresh_ready(job_id)
                if job is not None:
                    return job
            return None
        for job_id in await self._scan_job_ids():
            job = await self.claim(job_id, owner_id, lease_ttl)
            if job is not None:
                return job
        return None

    async def claim_next_any_tenant(self, owner_id: str, lease_ttl: int) -> ResearchJob | None:
        """Claim the next claimable job across all tenant namespaces."""
        if self._ready_client() is not None:
            return await self._claim_ready(owner_id, lease_ttl)
        for tenant in await self.scan_tenants():
            job = await self.for_tenant(tenant).claim_next(owner_id, lease_ttl)
            if job is not None:
                return job
        return None

    async def renew(self, job_id: str, token: str, lease_ttl: int) -> bool:
        """Extend the lease on ``job_id`` if ``token`` still owns it."""
        if not token:
            return False
        renewed = await self._lease_renew(self._lease_key(job_id), token, lease_ttl)
        if renewed:
            await self._refresh_ready(job_id)
        return renewed

    async def release(self, job_id: str, token: str | None) -> None:
        """Release the lease on ``job_id`` if ``token`` still owns it."""
        if not token:
            return
        await self._lease_release(self._lease_key(job_id), token)
        await self._refresh_ready(job_id)

    async def request_cancel(self, job_id: str) -> str:
        """Record a durable cancellation request and finalize if unowned.

        Returns the resulting job state: ``cancelled`` when finalized here,
        ``running`` when a live worker still owns the lease (it will observe
        the flag and finalize), or the existing terminal state.
        """
        store = self._store
        if store is None or not store.is_connected:
            return "unavailable"
        # Terminal-state gate first: never write a durable cancel flag for a
        # job that is already finished. Otherwise the flag would linger and
        # silently cancel every later retry/extend of that terminal job.
        job = await self.load(job_id)
        if job is None:
            return "unknown"
        if job.state in ("succeeded", "partial", "failed", "cancelled", "expired"):
            return job.state
        # Durable, race-free signal: a separate key so the owner's
        # job-record writes can never clobber the cancellation request.
        await store.set(
            self._cancel_key(job_id),
            {"cancel_requested": True, "requested_at": time.time()},
            JOB_RETENTION_SECONDS,
        )
        # Claim and reload before finalizing; checking absence of a lease
        # followed by an unconditional save races a new worker's reservation.
        claimed = await self._claim_prepared(job, "cancellation", DEFAULT_JOB_LEASE_TTL_SECONDS)
        if claimed is None:
            return "running"
        token = claimed.lease_token
        try:
            if not await self.clear_ownership(claimed):
                raise LeaseLostError(job_id)
            return claimed.state
        finally:
            if token:
                await self._lease_release(self._lease_key(job_id), token)


async def _scan_keys(store: KeyValueStore | None, pattern: str) -> list[str]:
    """List keys matching a glob across the supported store backends.

    Valkey-backed stores are scanned with ``SCAN`` (via ``scan_iter``) so the
    worker's periodic prefix scans never block the single-threaded Valkey
    event loop with an O(N) ``KEYS`` command. In-memory stores either expose
    a ``keys`` method or a ``_data`` dict. Returns decoded string keys (empty
    list when the store is unavailable).
    """
    if store is None or not store.is_connected:
        return []
    keys_method = getattr(store, "keys", None)
    if keys_method is not None:
        raw = await keys_method(pattern)
    else:
        client = getattr(store, "_client", None)
        scan_iter = getattr(client, "scan_iter", None) if client is not None else None
        if scan_iter is not None:
            raw = []
            async for key in scan_iter(match=pattern):
                raw.append(key)
        else:
            keys_client_method = getattr(client, "keys", None) if client is not None else None
            if keys_client_method is not None:
                raw = await keys_client_method(pattern)
            else:
                data = getattr(store, "_data", None)
                if data is None:
                    return []
                prefix = pattern.rstrip("*")
                raw = [key for key in data if key.startswith(prefix)]
    return [key.decode() if isinstance(key, bytes) else str(key) for key in raw]
