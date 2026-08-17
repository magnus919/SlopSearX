"""Async research jobs — Valkey-backed multi-query evidence gathering.

A research job turns one question into a bounded set of scoped searches
(strategy-dependent), executes them through the shared
:class:`~slopsearx.service.SearchService`, and exposes immutable
completed evidence via snapshot cursors.

States (explicit contract): ``queued``, ``running``, ``partial``,
``succeeded``, ``failed``, ``cancelled``, ``expired``. Cancellation is
best-effort: undispatched queries are cancelled, in-flight upstream calls
are not interrupted.
"""

from __future__ import annotations

import asyncio
import dataclasses
import json
import logging
import secrets
import time
import uuid
from dataclasses import dataclass, field
from typing import Any, Callable

from slopsearx.adapter import EngineStatus
from slopsearx.capabilities import CapabilityCatalog, MCPPolicy, resolve_intent
from slopsearx.service import (
    QueryValidationError,
    RateLimitExceededError,
    SearchRequest,
    SearchService,
)
from slopsearx.snapshot import KeyValueStore, SnapshotStore

logger = logging.getLogger(__name__)

JOB_KEY_PREFIX = "mcp:job"
IDEMPOTENCY_PREFIX = "mcp:idem"
LEASE_KEY_PREFIX = "mcp:joblease"
CANCEL_KEY_PREFIX = "mcp:jobcancel"
JOB_RETENTION_SECONDS = 86_400  # 24 hours
DEFAULT_JOB_LEASE_TTL_SECONDS = 60
DEFAULT_JOB_POLL_INTERVAL_SECONDS = 1.0


class LeaseLostError(Exception):
    """Raised when a worker loses ownership of a job mid-execution.

    The runner catches this and stops without finalizing, leaving the job
    ``running`` so the next owner reclaims and resumes the remaining work.
    """


class JobStillRunningError(Exception):
    """Raised when a direct run would race a live worker's execution.

    Retry/extend load a job that may still be actively executed by a worker
    that holds a live lease. Clearing its lease fields and running directly
    would cause concurrent execution, so the caller surfaces "job still
    running" instead of dispatching work.
    """


JOB_STATES = frozenset({"queued", "running", "partial", "succeeded", "failed", "cancelled", "expired"})
QUERY_STATES = frozenset({"pending", "running", "done", "failed", "cancelled"})
STRATEGIES = ("triangulate", "broad", "fresh", "counterevidence")

# Disjoint per-engine coverage buckets (schema pins). Every source is
# classified into exactly one of these; ``attempted`` aggregates all but
# ``not-selected``.
COVERAGE_BUCKETS: tuple[str, ...] = ("successful", "empty", "failed", "unavailable", "not-selected")

# Stable, machine-readable failure-class tokens. Status-derived tokens
# (``ok``/``rate_limited``/``blocked``/``error``/``timeout``) come from
# ``EngineStatus``; ``auth_required`` is the single derived token coming
# from a credential check, never from ``AdapterResponse.status``.
FAILURE_CLASS_TOKENS: tuple[str, ...] = (
    "ok",
    "rate_limited",
    "blocked",
    "error",
    "timeout",
    "unavailable",
    "auth_required",
)

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


# ---------------------------------------------------------------------------
# Model
# ---------------------------------------------------------------------------


@dataclass
class EngineCoverage:
    """Per-engine coverage for one research subquery (VAL-RESEARCH-004).

    ``bucket`` is exactly one of the disjoint :data:`COVERAGE_BUCKETS`;
    ``failure_class`` is a stable token drawn from
    :data:`FAILURE_CLASS_TOKENS` (``None`` for ``not-selected`` sources).
    ``status`` is the ``EngineStatus`` token (``None`` when not attempted).
    """

    engine: str
    bucket: str
    status: str | None = None
    result_count: int = 0
    failure_class: str | None = None


@dataclass
class CoverageSummary:
    """Aggregated per-query/job coverage counts (VAL-RESEARCH-005).

    ``attempted`` is an aggregate count equal to
    ``successful + empty + failed + unavailable``; ``not_selected`` covers
    engines the strategy/plan did not include. Buckets are disjoint: a
    source is counted in exactly one bucket.
    """

    attempted: int = 0
    successful: int = 0
    empty: int = 0
    failed: int = 0
    unavailable: int = 0
    not_selected: int = 0

    def as_dict(self) -> dict[str, int]:
        return dataclasses.asdict(self)


@dataclass
class ResearchQueryAttempt:
    """One execution attempt of a research subquery.

    Research evidence is immutable once written: each attempt records its
    own snapshot cursor, ``query_id``, ``result_count``, ``error``, state,
    and per-engine coverage. Retrying a failed/empty subquery appends a
    NEW attempt and never overwrites an earlier one (VAL-RESEARCH-021).
    """

    cursor: str | None = None
    query_id: str | None = None
    result_count: int = 0
    error: str | None = None
    state: str = "done"
    attempted_at: float = field(default_factory=time.time)
    engine_coverage: list[EngineCoverage] = field(default_factory=list)


@dataclass
class ResearchQuery:
    """One planned search within a research job."""

    index: int
    query: str
    intent: str
    engines: list[str]
    time_range: str | None = None
    state: str = "pending"
    query_id: str | None = None
    result_count: int = 0
    cursor: str | None = None
    error: str | None = None
    engine_coverage: list[EngineCoverage] = field(default_factory=list)
    # Every execution attempt of this subquery, oldest first. The first
    # attempt's cursor is preserved even after a retry overwrites the
    # query's *current* cursor (VAL-RESEARCH-008/021).
    attempts: list[ResearchQueryAttempt] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Coverage classification (pinned join)
# ---------------------------------------------------------------------------


def status_token(status: EngineStatus | str | None) -> str | None:
    """Normalize an ``EngineStatus`` enum or token string to its stable token."""
    if status is None:
        return None
    if isinstance(status, EngineStatus):
        return status.value
    return str(status)


def classify_coverage(
    *,
    engine: str,
    dispatched: bool,
    status: EngineStatus | str | None = None,
    result_count: int = 0,
    credential_missing: bool = False,
) -> EngineCoverage:
    """Pinned outcome -> (bucket, failure_class) join (VAL-RESEARCH-019).

    The mapping is fixed and deterministic:

    - ``ok`` + results > 0          -> ``successful`` / ``ok``
    - ``ok`` + results == 0         -> ``empty``      / ``ok``
    - ``rate_limited|blocked|error|timeout`` -> ``failed`` / same token
    - ``unavailable``               -> ``unavailable`` / ``unavailable``
    - credential missing            -> ``unavailable`` / ``auth_required``
    - not dispatched (scope-excluded) -> ``not-selected`` / ``None``

    ``auth_required`` is derived from the credential check (never from
    ``AdapterResponse.status``), so it takes precedence over any status.
    """
    if not dispatched:
        return EngineCoverage(engine=engine, bucket="not-selected", result_count=0, failure_class=None)
    if credential_missing:
        return EngineCoverage(
            engine=engine,
            bucket="unavailable",
            status=status_token(status),
            result_count=result_count,
            failure_class="auth_required",
        )
    token = status_token(status) or "error"
    if token == "ok":
        bucket = "successful" if result_count > 0 else "empty"
        return EngineCoverage(engine=engine, bucket=bucket, status=token, result_count=result_count, failure_class="ok")
    if token == "unavailable":
        return EngineCoverage(
            engine=engine,
            bucket="unavailable",
            status=token,
            result_count=result_count,
            failure_class="unavailable",
        )
    return EngineCoverage(engine=engine, bucket="failed", status=token, result_count=result_count, failure_class=token)


def summarize_coverage(coverage: list[EngineCoverage]) -> CoverageSummary:
    """Aggregate engine coverage into the disjoint per-query/job summary."""
    counts = {bucket: 0 for bucket in COVERAGE_BUCKETS}
    for entry in coverage:
        counts[entry.bucket] += 1
    not_selected = counts["not-selected"]
    return CoverageSummary(
        attempted=len(coverage) - not_selected,
        successful=counts["successful"],
        empty=counts["empty"],
        failed=counts["failed"],
        unavailable=counts["unavailable"],
        not_selected=not_selected,
    )


def is_retryable_query(query: ResearchQuery) -> bool:
    """Whether a subquery should be re-run by ``retry_research``.

    Retry targets only work classified **failed** or **empty**
    (``done`` + ``result_count == 0`` + no ``error``), never successful
    queries (VAL-RESEARCH-008/016). Empty is distinguished from failed
    exactly as in VAL-RESEARCH-007.
    """
    if query.state == "failed":
        return True
    if query.state == "done" and query.result_count == 0 and not query.error:
        return True
    return False


def _reset_retryable_queries(job: ResearchJob) -> None:
    """Re-apply a retry reset onto an authoritative job record.

    Resets any still-retryable (failed/empty) subqueries to ``pending`` and
    marks the job ``running``. Never resurrects a cancelled/expired job, and
    leaves a record with no retryable work untouched, so a record that was
    concurrently finalized by a durable worker (or a cancel request) is
    preserved as-is.
    """
    if job.state in ("cancelled", "expired"):
        return
    retryable = [query for query in job.queries if is_retryable_query(query)]
    if not retryable:
        return
    job.state = "running"
    for query in retryable:
        query.state = "pending"


def _attempt_from_query(query: ResearchQuery) -> ResearchQueryAttempt:
    """Snapshot the query's current fields as one immutable attempt."""
    return ResearchQueryAttempt(
        cursor=query.cursor,
        query_id=query.query_id,
        result_count=query.result_count,
        error=query.error,
        state=query.state,
        engine_coverage=list(query.engine_coverage),
    )


@dataclass
class ResearchJob:
    """A research job record. Evidence is immutable once written."""

    job_id: str
    question: str
    strategy: str
    state: str = "queued"
    queries: list[ResearchQuery] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)
    created_at: float = field(default_factory=time.time)
    deadline: float = 0.0
    tenant: str = "default"
    idempotency_key: str | None = None
    cancel_requested: bool = False
    # Durable-execution lease fields. ``owner_id`` identifies the replica,
    # ``lease_token`` proves ownership, and ``lease_expires_at`` is the
    # visibility timeout. None/0.0 means the job is not currently leased.
    owner_id: str | None = None
    lease_token: str | None = None
    lease_expires_at: float = 0.0

    @property
    def progress(self) -> tuple[int, int]:
        """(completed_queries, total_queries)."""
        total = len(self.queries)
        completed = sum(1 for q in self.queries if q.state in ("done", "failed", "cancelled"))
        return completed, total


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

    def __init__(self, store: KeyValueStore | None, tenant: str = "default") -> None:
        self._store = store
        self._tenant = tenant

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
        return ResearchJobStore(self._store, tenant=tenant)

    def _key(self, job_id: str) -> str:
        return f"{JOB_KEY_PREFIX}:{self._tenant}:{job_id}"

    def _idem_key(self, idempotency_key: str) -> str:
        return f"{IDEMPOTENCY_PREFIX}:{self._tenant}:{idempotency_key}"

    def _lease_key(self, job_id: str) -> str:
        return f"{LEASE_KEY_PREFIX}:{self._tenant}:{job_id}"

    def _cancel_key(self, job_id: str) -> str:
        return f"{CANCEL_KEY_PREFIX}:{self._tenant}:{job_id}"

    async def save(self, job: ResearchJob) -> None:
        """Persist a job. No-op when the store is unavailable."""
        store = self._store
        if store is None or not store.is_connected:
            return
        payload = _job_to_payload(job)
        await store.set(self._key(job.job_id), payload, JOB_RETENTION_SECONDS)
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
        concurrent reclamation cannot race between them; the in-memory
        fallback is check-then-save, which is atomic under single-threaded
        asyncio (no await between the check and the write).
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
        if await self._lease_get(self._lease_key(job.job_id)) != token:
            return False
        await self.save(job)
        return True

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
                if await self._lease_get(self._lease_key(job_id)) is not None:
                    continue
                if job.deadline > now:
                    continue
                job.state = "expired"
                for query in job.queries:
                    if query.state in ("pending", "running"):
                        query.state = "cancelled"
                await self.save(job)
                expired += 1
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
        # Re-load under the lease (authoritative; another replica may have
        # finalized the job between the pre-filter and the lease win).
        job = await self.load(job_id)
        if job is None or job.state not in ("queued", "running"):
            await self._lease_release(self._lease_key(job_id), token)
            return None
        elif time.time() >= job.deadline:
            # A deadline-passed job can no longer make progress: finalize it to
            # ``expired`` (matching ``_run_job``/``retry``/``expire_stale_running``)
            # instead of handing it to a worker that would classify it
            # ``partial``/``failed`` when the deadline gate fires mid-run.
            for query in job.queries:
                if query.state in ("pending", "running"):
                    query.state = "cancelled"
            job.state = "expired"
            await self.save(job)
            await self._lease_release(self._lease_key(job_id), token)
            return None
        for query in job.queries:
            if query.state == "running":
                query.state = "pending"
        job.state = "running"
        job.owner_id = owner_id
        job.lease_token = token
        job.lease_expires_at = time.time() + lease_ttl
        await self.save(job)
        return job

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
        current = await self.load(job.job_id)
        if current is None:
            await self._lease_release(self._lease_key(job.job_id), token)
            return None
        # Deadline finalization runs BEFORE the caller's mutation is applied or
        # persisted: a claimable job whose deadline lapsed between the caller's
        # deadline gate and this point is finalized to ``expired`` here, so a
        # follow-up that can never run is not written into the record (and is
        # therefore never appended twice by run_direct's terminal path).
        if time.time() >= current.deadline:
            for query in current.queries:
                if query.state in ("pending", "running"):
                    query.state = "cancelled"
            current.state = "expired"
            await self.save(current)
            await self._lease_release(self._lease_key(job.job_id), token)
            return None
        # Re-apply the caller's intended mutation to the freshly loaded record
        # while holding the lease, so a concurrent finalizer cannot interleave.
        if mutate is not None:
            mutate(current)
        if current.state not in ("queued", "running"):
            await self._lease_release(self._lease_key(job.job_id), token)
            return None
        # Preserve a cancellation request that landed after ``job`` was loaded.
        current.cancel_requested = current.cancel_requested or job.cancel_requested
        for query in current.queries:
            if query.state == "running":
                query.state = "pending"
        current.state = "running"
        current.owner_id = owner_id
        current.lease_token = token
        current.lease_expires_at = time.time() + lease_ttl
        await self.save(current)
        return current

    async def claim_next(self, owner_id: str, lease_ttl: int) -> ResearchJob | None:
        """Claim the next claimable job for this store's tenant."""
        for job_id in await self._scan_job_ids():
            job = await self.claim(job_id, owner_id, lease_ttl)
            if job is not None:
                return job
        return None

    async def claim_next_any_tenant(self, owner_id: str, lease_ttl: int) -> ResearchJob | None:
        """Claim the next claimable job across all tenant namespaces."""
        for tenant in await self.scan_tenants():
            job = await self.for_tenant(tenant).claim_next(owner_id, lease_ttl)
            if job is not None:
                return job
        return None

    async def renew(self, job_id: str, token: str, lease_ttl: int) -> bool:
        """Extend the lease on ``job_id`` if ``token`` still owns it."""
        if not token:
            return False
        return await self._lease_renew(self._lease_key(job_id), token, lease_ttl)

    async def release(self, job_id: str, token: str | None) -> None:
        """Release the lease on ``job_id`` if ``token`` still owns it."""
        if not token:
            return
        await self._lease_release(self._lease_key(job_id), token)

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
        # A live owner will observe the flag on its next reload; leave the
        # job record alone so we don't fight its intermediate writes.
        if await self._lease_get(self._lease_key(job_id)) is not None:
            return "running"
        job.cancel_requested = True
        job.state = "cancelled"
        for query in job.queries:
            if query.state in ("pending", "running"):
                query.state = "cancelled"
        await self.save(job)
        return "cancelled"


# ---------------------------------------------------------------------------
# Query planning
# ---------------------------------------------------------------------------


def plan_research_queries(
    question: str,
    strategy: str,
    max_queries: int,
    max_engines_per_query: int,
    catalog: CapabilityCatalog,
    policy: MCPPolicy,
) -> tuple[list[ResearchQuery], list[str]]:
    """Build the ordered query list for a strategy.

    Returns ``(queries, warnings)``. Sensitive engines are dropped unless
    the sensitive-engine grant (``MCP_TARGETED_SENSITIVE_ALLOWED``) is
    enabled, and any such exclusion is reported as an explicit policy
    warning — never silently dropped. Budgets are applied here so the
    runner never exceeds operator limits.
    """
    if strategy not in STRATEGIES:
        return [], [f"unknown strategy '{strategy}'; valid strategies: {', '.join(STRATEGIES)}"]

    plans: dict[str, list[tuple[str, str, str | None]]] = {
        # (intent, query_text, time_range)
        "triangulate": [
            ("web", question, None),
            ("science", question, None),
            ("reference", question, None),
        ],
        "broad": [
            ("web", question, None),
            ("news", question, None),
            ("reference", question, None),
            ("social", question, None),
        ],
        "fresh": [
            ("web", question, "month"),
            ("news", question, "day"),
        ],
        "counterevidence": [
            ("reference", question, None),
            ("science", f"{question} limitations", None),
            ("reference", f"{question} criticism", None),
            ("science", f"{question} counterexample", None),
        ],
    }

    queries: list[ResearchQuery] = []
    warnings: list[str] = []
    sensitive_allowed = policy.targeted_sensitive_allowed
    for intent, text, time_range in plans[strategy]:
        engines, intent_warnings = resolve_intent(intent, catalog)
        warnings.extend(intent_warnings)
        excluded = [name for name in engines if name in policy.sensitive_engines and not sensitive_allowed]
        if excluded:
            engines = [name for name in engines if name not in policy.sensitive_engines]
            warnings.append(
                "sensitive engines excluded by policy (no MCP_TARGETED_SENSITIVE_ALLOWED grant): "
                + ", ".join(sorted(excluded))
            )
        engines = engines[:max_engines_per_query]
        queries.append(
            ResearchQuery(
                index=len(queries),
                query=text,
                intent=intent,
                engines=engines,
                time_range=time_range,
            )
        )
        if len(queries) >= max_queries:
            break
    return queries, warnings


# ---------------------------------------------------------------------------
# Runner
# ---------------------------------------------------------------------------


class ResearchJobRunner:
    """Background executor for research jobs (durable, lease-based worker).

    Each replica runs one or more worker tasks. A worker claims the next
    claimable job from the shared store under an exclusive lease (see
    :meth:`ResearchJobStore.claim`), renews that lease while it executes,
    and releases it on completion. Jobs are processed with bounded
    concurrency; queries run sequentially within a job, respecting each
    job's deadline. Cancellation stops undispatched work; in-flight engine
    calls complete and their results are preserved.
    """

    def __init__(
        self,
        service: SearchService,
        job_store: ResearchJobStore,
        snapshot_store: SnapshotStore,
        catalog: CapabilityCatalog,
        policy: MCPPolicy,
        *,
        owner_id: str | None = None,
        lease_ttl: int = DEFAULT_JOB_LEASE_TTL_SECONDS,
        poll_interval: float = DEFAULT_JOB_POLL_INTERVAL_SECONDS,
        max_concurrent_jobs: int = 1,
    ) -> None:
        self._service = service
        self._jobs = job_store
        self._snapshots = snapshot_store
        self._catalog = catalog
        self._policy = policy
        self._owner_id = owner_id or generate_owner_id()
        self._lease_ttl = lease_ttl
        self._poll_interval = poll_interval
        self._max_concurrent_jobs = max(1, max_concurrent_jobs)
        self._default_tenant = job_store._tenant
        self._queue: asyncio.Queue[tuple[str, str]] = asyncio.Queue()

    @property
    def worker_id(self) -> str:
        """The replica-local worker identifier used for lease ownership."""
        return self._owner_id

    @property
    def lease_ttl(self) -> int:
        """The lease visibility timeout in seconds."""
        return self._lease_ttl

    @property
    def poll_interval(self) -> float:
        """How often an idle worker polls the shared store for claimable jobs."""
        return self._poll_interval

    @property
    def max_concurrent_jobs(self) -> int:
        """The bounded per-replica job-execution concurrency."""
        return self._max_concurrent_jobs

    def _jobs_for(self, tenant: str) -> ResearchJobStore:
        return self._jobs.for_tenant(tenant)

    def _snapshots_for(self, tenant: str) -> SnapshotStore:
        return self._snapshots.for_tenant(tenant)

    def enqueue(self, job_id: str, tenant: str = "default") -> None:
        """Queue a job for immediate execution by this replica (fast path)."""
        self._queue.put_nowait((tenant, job_id))

    async def run_pending(self, job: ResearchJob) -> ResearchJob:
        """Execute any pending/running subqueries, then finalize the job.

        Used by the durable worker loop and by retry/extend so completed
        evidence is preserved and the job's terminal state is recomputed
        consistently (VAL-RESEARCH-008/009/011). Reloads the job before each
        subquery so cancellation/lease changes that land mid-run are observed.
        Returns the final job.
        """
        store = self._jobs_for(job.tenant)
        for index in range(len(job.queries)):
            # Reload to observe cancellation or deadline changes that landed
            # from another request (e.g. the cancel tool) mid-run.
            fresh = await store.load(job.job_id)
            if fresh is not None:
                if fresh.owner_id is not None and fresh.owner_id != self._owner_id:
                    raise LeaseLostError(job.job_id)
                job = fresh
            query = job.queries[index]
            if query.state in ("done", "failed", "cancelled"):
                continue
            if job.cancel_requested:
                query.state = "cancelled"
                await store.save(job)
                continue
            if time.time() >= job.deadline:
                break
            if job.lease_token:
                lease_token: str = job.lease_token
                if not await store.renew(job.job_id, lease_token, self._lease_ttl):
                    raise LeaseLostError(job.job_id)

                async def _keep_alive(job_id: str = job.job_id, token: str = lease_token) -> None:
                    # Renew on a cadence shorter than the lease TTL so a
                    # subquery that outlives the TTL is never reclaimed and
                    # re-executed by another replica mid-flight.
                    while True:
                        await asyncio.sleep(max(self._lease_ttl / 3, 0.5))
                        if not await store.renew(job_id, token, self._lease_ttl):
                            return

                keep_alive = asyncio.create_task(_keep_alive())
                try:
                    await self._execute_query(job, query)
                finally:
                    keep_alive.cancel()
                    try:
                        await keep_alive
                    except asyncio.CancelledError:
                        pass
            else:
                await self._execute_query(job, query)

        # Reload to observe any cancellation/deadline that landed mid-run.
        job = await store.load(job.job_id) or job
        completed = sum(1 for query in job.queries if query.state == "done")
        if job.cancel_requested:
            for query in job.queries:
                if query.state in ("pending", "running"):
                    query.state = "cancelled"
            job.state = "cancelled"
        elif time.time() >= job.deadline:
            for query in job.queries:
                if query.state in ("pending", "running"):
                    query.state = "cancelled"
            job.state = "partial" if completed else "failed"
        elif all(query.state == "done" for query in job.queries):
            job.state = "succeeded"
        elif any(query.state == "done" for query in job.queries):
            job.state = "partial"
        else:
            job.state = "failed"
        await store.save(job)
        return job

    async def _raise_if_live_owned(self, job: ResearchJob) -> None:
        """Raise :class:`JobStillRunningError` if ``job`` is live-lease-owned.

        Liveness is checked against the lease key, not the record's
        ``lease_token``/``owner_id`` fields (which survive a released/expired
        lease), so a lease-expired orphan is not refused.
        """
        if job.state != "running" or not job.lease_token:
            return
        store = self._jobs_for(job.tenant)
        live = await store._lease_get(store._lease_key(job.job_id))
        if live == job.lease_token:
            raise JobStillRunningError(job.job_id)

    async def run_direct(
        self,
        job: ResearchJob,
        *,
        mutate: Callable[[ResearchJob], None] | None = None,
    ) -> ResearchJob:
        """Run a loaded job directly, claiming it first when claimable.

        ``retry``/``extend`` load a job that may still carry lease fields from
        a prior ``claim``/``release`` cycle: ``release`` deletes the Valkey
        lease key but not the record fields. A claimable job (``queued`` or
        ``running``) is claimed under a fresh lease before execution so a
        concurrent durable worker excludes it (exactly-one-owner). Terminal
        jobs are not claimable and still run lease-free exactly as before.

        ``mutate``, when given, is the caller's intended direct-run mutation
        (reset-to-pending retry queries or an appended follow-up). It is
        applied to the freshly loaded record — never the caller's in-memory
        copy — so a record that a durable worker finalized between the
        caller's load and this call is reconciled, not clobbered.

        A job that is still ``running`` under a *live* owner must not be
        cleared or run here: that would race the owner's execution. Liveness
        is checked against the lease key — not the record fields, which survive
        a released/expired lease — so a lease-expired orphan is resumed rather
        than refused. Live-owner calls raise :class:`JobStillRunningError` so
        the tool can surface "job still running".
        """
        store = self._jobs_for(job.tenant)
        fresh = await store.load(job.job_id) or job

        await self._raise_if_live_owned(fresh)

        claimed = await store._claim_prepared(fresh, self._owner_id, self._lease_ttl, mutate=mutate)
        if claimed is not None:
            try:
                result = await self.run_pending(claimed)
            finally:
                await store.release(job.job_id, claimed.lease_token)
            result.owner_id = None
            result.lease_token = None
            result.lease_expires_at = 0.0
            await store.save(result)
            return result

        # Claim returned None. A claimable job that lost the lease race is now
        # owned by another worker; running lease-free would double-execute.
        current = await store.load(job.job_id)
        if current is not None and current.state in ("queued", "running"):
            raise JobStillRunningError(job.job_id)

        # Terminal or non-claimable job: reconcile with the freshly loaded
        # record (never the caller's stale copy) and re-apply the caller's
        # intended mutation before persisting, so completed evidence written
        # by a concurrently finalizing worker is preserved.
        base = current if current is not None else fresh
        if time.time() >= base.deadline:
            # A deadline-passed job can no longer make progress: finalize it to
            # ``expired`` (matching the claim/retry deadline finalization)
            # instead of re-applying the caller's mutation and executing it.
            # Running here would append a follow-up that can never run and let
            # run_pending's mid-run deadline branch re-classify the record as
            # ``partial``/``failed``, losing the ``expired`` terminal state.
            for query in base.queries:
                if query.state in ("pending", "running"):
                    query.state = "cancelled"
            base.state = "expired"
            base.owner_id = None
            base.lease_token = None
            base.lease_expires_at = 0.0
            await store.save(base)
            return base
        if mutate is not None:
            mutate(base)
        base.owner_id = None
        base.lease_token = None
        base.lease_expires_at = 0.0
        await store.save(base)
        return await self.run_pending(base)

    async def retry(self, job_id: str, tenant: str | None = None) -> ResearchJob | None:
        """Re-run only failed/empty subqueries (VAL-RESEARCH-008).

        Gated on the job's deadline/terminal state: a job that is already
        ``cancelled``/``expired`` is left untouched (never resurrected), and a
        job whose deadline has already passed is finalized to ``expired``
        rather than re-run (which would otherwise end in ``partial``/``failed``
        when the deadline check fires mid-run).

        Successful subqueries are never re-executed and their snapshot
        cursors are byte-for-byte unchanged. Each retried subquery gets a
        NEW linked attempt appended; the original attempt's cursor remains
        readable. Returns the updated job, or ``None`` for an unknown id.
        Raises :class:`JobStillRunningError` when the job is still running
        under a live owner and must not be retried concurrently.
        """
        store = self._jobs_for(tenant or self._default_tenant)
        job = await store.load(job_id)
        if job is None:
            return None
        # Terminal-state gate: never resurrect a cancelled or expired job.
        if job.state in ("cancelled", "expired"):
            return job
        retryable = [query for query in job.queries if is_retryable_query(query)]
        if not retryable:
            return job
        # A still-running job under a live owner must not be retried (or have
        # its record rewritten) concurrently. Liveness is checked against the
        # lease key, not the record fields, so a lease-expired orphan proceeds.
        # This runs before the deadline gate so a deadline-passed but still
        # live job surfaces JobStillRunningError instead of being rewritten to
        # ``expired`` mid-run.
        await self._raise_if_live_owned(job)
        # Deadline gate: a deadline-passed retry finalizes to expired, not
        # partial/failed (the run_pending deadline branch would otherwise
        # classify a re-run that breaks on the deadline as partial/failed).
        if time.time() >= job.deadline:
            for query in job.queries:
                if query.state in ("pending", "running"):
                    query.state = "cancelled"
            job.state = "expired"
            await store.save(job)
            return job
        # The retry mutation is applied to the freshly loaded record inside
        # run_direct's claim (exactly-one-owner), never persisted ahead of
        # time. This avoids stripping a concurrently-claiming worker's
        # owner_id/lease_token and avoids clobbering a record a durable worker
        # finalized between the load above and the claim.
        return await self.run_direct(job, mutate=_reset_retryable_queries)

    def _build_query_coverage(self, query: ResearchQuery, response: Any) -> list[EngineCoverage]:
        """Derive per-engine coverage for a completed subquery (VAL-RESEARCH-004).

        Engines actually dispatched appear in ``response.engine_outcomes``;
        engines the scope deliberately excluded are ``not-selected``; any
        planned engine not otherwise accounted for is ``not-selected``.
        Credential state comes from the capability catalog: an engine that
        requires credentials and has none configured is ``unavailable`` with
        the derived ``auth_required`` failure class.
        """
        coverage: dict[str, EngineCoverage] = {}
        for outcome in response.engine_outcomes:
            cap = self._catalog.get(outcome.engine)
            credential_missing = bool(cap is not None and cap.auth_class == "required" and not cap.auth_configured)
            coverage[outcome.engine] = classify_coverage(
                engine=outcome.engine,
                dispatched=True,
                status=outcome.status,
                result_count=outcome.result_count,
                credential_missing=credential_missing,
            )
        for exclusion in response.scope.excluded_engines:
            coverage[exclusion.engine] = classify_coverage(engine=exclusion.engine, dispatched=False)
        for name in query.engines:
            if name not in coverage:
                coverage[name] = classify_coverage(engine=name, dispatched=False)
        # Deterministic order: planned engines first, then any extra excluded.
        ordered = list(dict.fromkeys(list(query.engines) + [e.engine for e in response.scope.excluded_engines]))
        return [coverage[name] for name in ordered if name in coverage]

    async def run_forever(self) -> None:
        """Process claimable jobs until cancelled (bounded worker pool)."""
        workers = [asyncio.create_task(self._worker()) for _ in range(self._max_concurrent_jobs)]
        try:
            await asyncio.gather(*workers)
        except asyncio.CancelledError:
            for worker in workers:
                worker.cancel()
            await asyncio.gather(*workers, return_exceptions=True)
            raise

    async def _worker(self) -> None:
        """One worker task: claim + execute jobs until cancelled."""
        while True:
            try:
                entry = self._next_local()
                if entry is None:
                    # Durable path: claim the next queued/orphaned job in the
                    # shared store (across all tenants).
                    job = await self._jobs.claim_next_any_tenant(self._owner_id, self._lease_ttl)
                else:
                    tenant, job_id = entry
                    job = await self._jobs.for_tenant(tenant).claim(job_id, self._owner_id, self._lease_ttl)
            except asyncio.CancelledError:
                raise
            except Exception as exc:  # noqa: BLE001 — transient claim/store error
                logger.exception("ResearchJobRunner: claim failed: %s", exc)
                await asyncio.sleep(self._poll_interval)
                continue
            if job is None:
                await asyncio.sleep(self._poll_interval)
                continue
            try:
                await self._execute_claimed(job)
            except asyncio.CancelledError:
                raise
            except Exception as exc:  # noqa: BLE001 — never let a job kill the worker
                logger.exception("ResearchJobRunner: job %s crashed: %s", job.job_id, exc)

    def _next_local(self) -> tuple[str, str] | None:
        """Pop the next locally-enqueued ``(tenant, job_id)`` entry, if any."""
        try:
            return self._queue.get_nowait()
        except asyncio.QueueEmpty:
            return None

    async def _execute_claimed(self, job: ResearchJob) -> None:
        """Execute a claimed job and always release its lease afterwards."""
        try:
            await self.run_pending(job)
        except LeaseLostError:
            logger.warning("ResearchJobRunner: lost lease for job %s; abandoning execution", job.job_id)
        finally:
            # Token-guarded: only deletes the lease if we still own it.
            await self._jobs.for_tenant(job.tenant).release(job.job_id, job.lease_token)

    async def _run_job(self, job_id: str, tenant: str | None = None) -> None:
        """Run a single job directly (test/legacy path, no lease claim)."""
        store = self._jobs_for(tenant or self._default_tenant)
        job = await store.load(job_id)
        if job is None:
            return

        if job.state in ("cancelled", "expired"):
            # Already finalized by cancellation or a deadline expiry —
            # make sure any still-pending queries are marked cancelled.
            if job.cancel_requested:
                for query in job.queries:
                    if query.state in ("pending", "running"):
                        query.state = "cancelled"
                await store.save(job)
            return

        if time.time() >= job.deadline:
            job.state = "expired"
            await store.save(job)
            return

        job.state = "running"
        await store.save(job)
        await self.run_pending(job)

    async def _execute_query(self, job: ResearchJob, query: ResearchQuery) -> None:
        """Run one subquery and persist its immutable evidence + attempt.

        Reused by the normal runner loop and by retry, so a retried query
        is executed exactly like the original and appends a new attempt.
        Tenant-scoped so subquery snapshots land in the job's tenant.
        """
        store = self._jobs_for(job.tenant)
        snapshots = self._snapshots_for(job.tenant)
        query.state = "running"
        await store.save(job)
        request = SearchRequest(
            query=query.query,
            engines=query.engines or None,
            time_range=query.time_range,
            include={"results", "engine_status"},
            client_identifier=f"mcp-job:{job.job_id}",
        )
        try:
            response = await self._service.search(request)
        except (QueryValidationError, RateLimitExceededError) as exc:
            query.state = "failed"
            query.error = str(exc)
            query.attempts.append(_attempt_from_query(query))
            if not await store.save_if_owned(job):
                raise LeaseLostError(job.job_id)
            return

        query.query_id = response.query_id
        query.result_count = len(response.results)
        query.cursor = await snapshots.create(
            response.query,
            response.query_id,
            response.results,
            response.scope,
        )
        # Persist per-engine coverage and the disjoint bucket summary.
        query.engine_coverage = self._build_query_coverage(query, response)
        coverage_summary = summarize_coverage(query.engine_coverage)
        # Subquery state distinguishes successful/empty/failed
        # (VAL-RESEARCH-007): empty is done+result_count==0+no error and
        # is never conflated with failed; a mix of empty and failed
        # engines (no results) is classified failed, never clean empty.
        if response.all_unresponsive:
            query.state = "failed"
            query.error = "no engines responded"
        elif coverage_summary.failed > 0 and query.result_count == 0:
            query.state = "failed"
            query.error = "some engines failed and none returned results"
        else:
            query.state = "done"
        query.attempts.append(_attempt_from_query(query))
        if not await store.save_if_owned(job):
            raise LeaseLostError(job.job_id)


# ---------------------------------------------------------------------------
# Serialization
# ---------------------------------------------------------------------------


def _job_to_payload(job: ResearchJob) -> dict[str, Any]:
    return dataclasses.asdict(job)


def _job_from_payload(payload: dict[str, Any]) -> ResearchJob:
    def _coverage(items: Any) -> list[EngineCoverage]:
        return [
            EngineCoverage(
                engine=str(cov.get("engine", "")),
                bucket=str(cov.get("bucket", "not-selected")),
                status=cov.get("status"),
                result_count=int(cov.get("result_count", 0)),
                failure_class=cov.get("failure_class"),
            )
            for cov in (items or [])
        ]

    queries = [
        ResearchQuery(
            index=int(item.get("index", 0)),
            query=str(item.get("query", "")),
            intent=str(item.get("intent", "")),
            engines=[str(name) for name in (item.get("engines") or [])],
            time_range=item.get("time_range"),
            state=str(item.get("state", "pending")),
            query_id=item.get("query_id"),
            result_count=int(item.get("result_count", 0)),
            cursor=item.get("cursor"),
            error=item.get("error"),
            engine_coverage=_coverage(item.get("engine_coverage")),
            attempts=[
                ResearchQueryAttempt(
                    cursor=attempt.get("cursor"),
                    query_id=attempt.get("query_id"),
                    result_count=int(attempt.get("result_count", 0)),
                    error=attempt.get("error"),
                    state=str(attempt.get("state", "done")),
                    attempted_at=float(attempt.get("attempted_at", 0.0)),
                    engine_coverage=_coverage(attempt.get("engine_coverage")),
                )
                for attempt in (item.get("attempts") or [])
            ],
        )
        for item in (payload.get("queries") or [])
    ]
    return ResearchJob(
        job_id=str(payload.get("job_id", "")),
        question=str(payload.get("question", "")),
        strategy=str(payload.get("strategy", "")),
        state=str(payload.get("state", "queued")),
        queries=queries,
        warnings=[str(w) for w in (payload.get("warnings") or [])],
        created_at=float(payload.get("created_at", 0.0)),
        deadline=float(payload.get("deadline", 0.0)),
        tenant=str(payload.get("tenant", "default")),
        idempotency_key=payload.get("idempotency_key"),
        cancel_requested=bool(payload.get("cancel_requested", False)),
        owner_id=payload.get("owner_id"),
        lease_token=payload.get("lease_token"),
        lease_expires_at=float(payload.get("lease_expires_at", 0.0)),
    )


def generate_job_id() -> str:
    """Generate a short, traceable research job identifier."""
    return f"job-{uuid.uuid4().hex[:12]}"


def generate_owner_id() -> str:
    """Generate a stable per-process worker/replica identifier."""
    return f"worker-{uuid.uuid4().hex[:8]}"


def generate_lease_token() -> str:
    """Generate an opaque, unforgeable lease ownership token."""
    return f"lease-{secrets.token_hex(16)}"


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
