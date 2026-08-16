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
JOB_RETENTION_SECONDS = 86_400  # 24 hours

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
FAILURE_CLASS_TOKENS: tuple[str, ...] = ("ok", "rate_limited", "blocked", "error", "timeout", "auth_required")


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

    async def run_pending(self, job: ResearchJob) -> ResearchJob:
        """Execute any pending/running subqueries, then finalize the job.

        Used by the normal runner loop and by retry/extend so completed
        evidence is preserved and the job's terminal state is recomputed
        consistently (VAL-RESEARCH-008/009/011). Returns the final job.
        """
        completed = 0
        for query in job.queries:
            if query.state in ("done", "failed", "cancelled"):
                if query.state == "done":
                    completed += 1
                continue
            if job.cancel_requested:
                query.state = "cancelled"
                continue
            if time.time() >= job.deadline:
                break
            await self._execute_query(job, query)
            if query.state == "done":
                completed += 1

        # Reload to observe any cancellation/deadline that landed mid-run.
        job = await self._jobs.load(job.job_id) or job
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
        return job

    async def retry(self, job_id: str) -> ResearchJob | None:
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
        """
        job = await self._jobs.load(job_id)
        if job is None:
            return None
        # Terminal-state gate: never resurrect a cancelled or expired job.
        if job.state in ("cancelled", "expired"):
            return job
        retryable = [query for query in job.queries if is_retryable_query(query)]
        if not retryable:
            return job
        # Deadline gate: a deadline-passed retry finalizes to expired, not
        # partial/failed (the run_pending deadline branch would otherwise
        # classify a re-run that breaks on the deadline as partial/failed).
        if time.time() >= job.deadline:
            for query in job.queries:
                if query.state in ("pending", "running"):
                    query.state = "cancelled"
            job.state = "expired"
            await self._jobs.save(job)
            return job
        job.state = "running"
        for query in retryable:
            query.state = "pending"
        await self._jobs.save(job)
        return await self.run_pending(job)

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
        await self.run_pending(job)

    async def _execute_query(self, job: ResearchJob, query: ResearchQuery) -> None:
        """Run one subquery and persist its immutable evidence + attempt.

        Reused by the normal runner loop and by retry, so a retried query
        is executed exactly like the original and appends a new attempt.
        """
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
            query.attempts.append(_attempt_from_query(query))
            await self._jobs.save(job)
            return

        query.query_id = response.query_id
        query.result_count = len(response.results)
        query.cursor = await self._snapshots.create(
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
        await self._jobs.save(job)


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
    )


def generate_job_id() -> str:
    """Generate a short, traceable research job identifier."""
    return f"job-{uuid.uuid4().hex[:12]}"
