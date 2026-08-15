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
import logging
import time
import uuid
from dataclasses import dataclass, field
from typing import Any

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
JOB_RETENTION_SECONDS = 86_400  # 24 hours

JOB_STATES = frozenset({"queued", "running", "partial", "succeeded", "failed", "cancelled", "expired"})
QUERY_STATES = frozenset({"pending", "running", "done", "failed", "cancelled"})
STRATEGIES = ("triangulate", "broad", "fresh", "counterevidence")


# ---------------------------------------------------------------------------
# Model
# ---------------------------------------------------------------------------


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
    """Valkey-backed persistence for research jobs."""

    def __init__(self, store: KeyValueStore | None, tenant: str = "default") -> None:
        self._store = store
        self._tenant = tenant

    @property
    def available(self) -> bool:
        return self._store is not None and self._store.is_connected

    def _key(self, job_id: str) -> str:
        return f"{JOB_KEY_PREFIX}:{self._tenant}:{job_id}"

    def _idem_key(self, idempotency_key: str) -> str:
        return f"{IDEMPOTENCY_PREFIX}:{self._tenant}:{idempotency_key}"

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

    async def load(self, job_id: str) -> ResearchJob | None:
        """Load a job by ID, or None when missing/unavailable."""
        store = self._store
        if store is None or not store.is_connected:
            return None
        payload = await store.get(self._key(job_id))
        if payload is None:
            return None
        job = _job_from_payload(payload)
        if job.tenant != self._tenant:
            return None
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

    async def expire_stale_running(self) -> int:
        """Mark jobs left in ``running`` by a dead process as ``expired``.

        Returns the number of jobs expired. Called at MCP server startup.
        """
        store = self._store
        if store is None or not store.is_connected:
            return 0
        # Enumerate running jobs by scanning tenant keys (bounded by
        # prefix scan in Valkey).
        expired = 0
        try:
            client = getattr(store, "_client", None)
            if client is None:
                return 0
            pattern = f"{JOB_KEY_PREFIX}:{self._tenant}:*"
            keys = await client.keys(pattern)
            for raw_key in keys:
                key = raw_key.decode() if isinstance(raw_key, bytes) else str(raw_key)
                payload = await store.get(key)
                if payload is None:
                    continue
                job = _job_from_payload(payload)
                if job.state == "running":
                    job.state = "expired"
                    for query in job.queries:
                        if query.state in ("pending", "running"):
                            query.state = "cancelled"
                    await self.save(job)
                    expired += 1
        except Exception as exc:  # noqa: BLE001 — graceful degradation
            logger.warning("ResearchJobStore: stale-job scan failed: %s", exc)
        return expired


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
    """Background executor for research jobs (single in-process worker).

    Jobs are processed one at a time, queries sequentially, respecting
    each job's deadline. Cancellation stops undispatched work; in-flight
    engine calls complete and their results are preserved.
    """

    def __init__(
        self,
        service: SearchService,
        job_store: ResearchJobStore,
        snapshot_store: SnapshotStore,
        catalog: CapabilityCatalog,
        policy: MCPPolicy,
    ) -> None:
        self._service = service
        self._jobs = job_store
        self._snapshots = snapshot_store
        self._catalog = catalog
        self._policy = policy
        self._queue: asyncio.Queue[str] = asyncio.Queue()

    def enqueue(self, job_id: str) -> None:
        """Queue a job for execution."""
        self._queue.put_nowait(job_id)

    async def run_forever(self) -> None:
        """Process queued jobs until cancelled."""
        while True:
            job_id = await self._queue.get()
            try:
                await self._run_job(job_id)
            except asyncio.CancelledError:
                raise
            except Exception as exc:  # noqa: BLE001 — never let a job kill the worker
                logger.exception("ResearchJobRunner: job %s crashed: %s", job_id, exc)
            finally:
                self._queue.task_done()

    async def _run_job(self, job_id: str) -> None:
        job = await self._jobs.load(job_id)
        if job is None:
            return

        if job.state in ("cancelled", "expired"):
            # Already finalized by cancellation or a deadline expiry —
            # make sure any still-pending queries are marked cancelled.
            if job.cancel_requested:
                for query in job.queries:
                    if query.state in ("pending", "running"):
                        query.state = "cancelled"
                await self._jobs.save(job)
            return

        if time.time() >= job.deadline:
            job.state = "expired"
            await self._jobs.save(job)
            return

        job.state = "running"
        await self._jobs.save(job)

        completed = 0
        for query in job.queries:
            if job.cancel_requested:
                query.state = "cancelled"
                continue
            if time.time() >= job.deadline:
                break

            query.state = "running"
            await self._jobs.save(job)
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
                await self._jobs.save(job)
                continue

            query.query_id = response.query_id
            query.result_count = len(response.results)
            query.cursor = await self._snapshots.create(
                response.query,
                response.query_id,
                response.results,
                response.scope,
            )
            if response.all_unresponsive:
                query.state = "failed"
                query.error = "no engines responded"
            else:
                query.state = "done"
                completed += 1
            await self._jobs.save(job)

        # Reload to observe any cancellation that landed mid-run.
        job = await self._jobs.load(job_id)
        if job is None:
            return

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
        await self._jobs.save(job)


# ---------------------------------------------------------------------------
# Serialization
# ---------------------------------------------------------------------------


def _job_to_payload(job: ResearchJob) -> dict[str, Any]:
    return dataclasses.asdict(job)


def _job_from_payload(payload: dict[str, Any]) -> ResearchJob:
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
    )


def generate_job_id() -> str:
    """Generate a short, traceable research job identifier."""
    return f"job-{uuid.uuid4().hex[:12]}"
