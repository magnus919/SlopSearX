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
import logging
import time
from typing import Any, Callable

from slopsearx.capabilities import CapabilityCatalog, MCPPolicy, resolve_intent
from slopsearx.filters import resolve_filter_enforcement
from slopsearx.research_budget import finish_attempt, initialize_budget, reserve_attempt

# Compatibility exports preserve the historical research module API.
from slopsearx.research_models import (
    COVERAGE_BUCKETS as COVERAGE_BUCKETS,
)
from slopsearx.research_models import (
    FAILURE_CLASS_TOKENS as FAILURE_CLASS_TOKENS,
)
from slopsearx.research_models import (
    JOB_STATES as JOB_STATES,
)
from slopsearx.research_models import (
    QUERY_STATES as QUERY_STATES,
)
from slopsearx.research_models import (
    STRATEGIES as STRATEGIES,
)
from slopsearx.research_models import (
    CoverageSummary as CoverageSummary,
)
from slopsearx.research_models import (
    EngineCoverage as EngineCoverage,
)
from slopsearx.research_models import (
    JobStillRunningError as JobStillRunningError,
)
from slopsearx.research_models import (
    LeaseLostError as LeaseLostError,
)
from slopsearx.research_models import (
    ResearchJob as ResearchJob,
)
from slopsearx.research_models import (
    ResearchQuery as ResearchQuery,
)
from slopsearx.research_models import (
    ResearchQueryAttempt as ResearchQueryAttempt,
)
from slopsearx.research_models import (
    _attempt_from_query as _attempt_from_query,
)
from slopsearx.research_models import (
    _job_from_payload as _job_from_payload,
)
from slopsearx.research_models import (
    _job_to_payload as _job_to_payload,
)
from slopsearx.research_models import (
    _reset_retryable_queries as _reset_retryable_queries,
)
from slopsearx.research_models import (
    classify_coverage as classify_coverage,
)
from slopsearx.research_models import (
    generate_job_id as generate_job_id,
)
from slopsearx.research_models import (
    generate_lease_token as generate_lease_token,
)
from slopsearx.research_models import (
    generate_owner_id as generate_owner_id,
)
from slopsearx.research_models import (
    is_retryable_query as is_retryable_query,
)
from slopsearx.research_models import (
    status_token as status_token,
)
from slopsearx.research_models import (
    summarize_coverage as summarize_coverage,
)
from slopsearx.research_store import (
    _LEASE_RELEASE_SCRIPT as _LEASE_RELEASE_SCRIPT,
)
from slopsearx.research_store import (
    _LEASE_RENEW_SCRIPT as _LEASE_RENEW_SCRIPT,
)
from slopsearx.research_store import (
    _LEASE_SAVE_SCRIPT as _LEASE_SAVE_SCRIPT,
)
from slopsearx.research_store import (
    _READY_REFRESH_SCRIPT as _READY_REFRESH_SCRIPT,
)
from slopsearx.research_store import (
    _READY_TAKE_SCRIPT as _READY_TAKE_SCRIPT,
)
from slopsearx.research_store import (
    _RECONCILE_BATCH as _RECONCILE_BATCH,
)
from slopsearx.research_store import (
    _RECONCILE_INTERVAL as _RECONCILE_INTERVAL,
)
from slopsearx.research_store import (
    CANCEL_KEY_PREFIX as CANCEL_KEY_PREFIX,
)
from slopsearx.research_store import (
    DEFAULT_JOB_LEASE_TTL_SECONDS as DEFAULT_JOB_LEASE_TTL_SECONDS,
)
from slopsearx.research_store import (
    DEFAULT_JOB_POLL_INTERVAL_SECONDS as DEFAULT_JOB_POLL_INTERVAL_SECONDS,
)
from slopsearx.research_store import (
    IDEMPOTENCY_PREFIX as IDEMPOTENCY_PREFIX,
)
from slopsearx.research_store import (
    JOB_KEY_PREFIX as JOB_KEY_PREFIX,
)
from slopsearx.research_store import (
    JOB_RETENTION_SECONDS as JOB_RETENTION_SECONDS,
)
from slopsearx.research_store import (
    LEASE_KEY_PREFIX as LEASE_KEY_PREFIX,
)
from slopsearx.research_store import (
    READY_PREFIX as READY_PREFIX,
)
from slopsearx.research_store import (
    ResearchJobStore as ResearchJobStore,
)
from slopsearx.research_store import (
    _scan_keys as _scan_keys,
)
from slopsearx.service import (
    QueryValidationError,
    SearchRequest,
    SearchService,
)
from slopsearx.service import (
    RateLimitExceededError as RateLimitExceededError,
)
from slopsearx.snapshot import SnapshotStore

logger = logging.getLogger(__name__)


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
        dispatch_validator: Callable[[ResearchQuery], str | None] | None = None,
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
        self.dispatch_validator = dispatch_validator

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
        initialize_budget(job, self._policy)
        job.stop_reason = None
        if not await store.save_if_owned(job):
            raise LeaseLostError(job.job_id)
        for index in range(len(job.queries)):
            # Reload to observe cancellation or deadline changes that landed
            # from another request (e.g. the cancel tool) mid-run.
            fresh = await store.load(job.job_id)
            if fresh is not None:
                if fresh.lease_token != job.lease_token or (
                    fresh.owner_id is not None and fresh.owner_id != self._owner_id
                ):
                    raise LeaseLostError(job.job_id)
                job = fresh
            initialize_budget(job, self._policy)
            query = job.queries[index]
            if query.state in ("done", "failed", "cancelled"):
                continue
            if job.cancel_requested:
                query.state = "cancelled"
                if not await store.save_if_owned(job):
                    raise LeaseLostError(job.job_id)
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
            if job.stop_reason and job.stop_reason.endswith("budget_exhausted"):
                break

        # Reload to observe any cancellation/deadline that landed mid-run.
        fresh = await store.load(job.job_id)
        if fresh is not None:
            if fresh.lease_token != job.lease_token:
                raise LeaseLostError(job.job_id)
            job = fresh
        completed = sum(1 for query in job.queries if query.state == "done")
        if job.cancel_requested:
            for query in job.queries:
                if query.state in ("pending", "running"):
                    query.state = "cancelled"
            job.state = "cancelled"
            job.stop_reason = "cancelled"
        elif time.time() >= job.deadline:
            for query in job.queries:
                if query.state in ("pending", "running"):
                    query.state = "cancelled"
            job.state = "partial" if completed else "failed"
            job.stop_reason = "deadline_expired"
        elif all(query.state == "done" for query in job.queries):
            job.state = "succeeded"
        elif any(query.state == "done" for query in job.queries):
            job.state = "partial"
        else:
            job.state = "failed"
        if job.stop_reason is None:
            job.stop_reason = "plan_executed" if job.state == "succeeded" else "execution_failed"
        if not await store.save_if_owned(job):
            raise LeaseLostError(job.job_id)
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
        execute: bool = True,
    ) -> ResearchJob:
        """Apply a mutation and optionally execute while holding a fenced lease.

        Completed jobs use the same exclusion boundary as running jobs. Never
        apply a stale caller copy or write after releasing lease ownership.
        """
        store = self._jobs_for(job.tenant)
        claimed = await store._claim_prepared(job, self._owner_id, self._lease_ttl, mutate=mutate)
        if claimed is None:
            raise JobStillRunningError(job.job_id)
        token = claimed.lease_token
        try:
            if not execute or claimed.state in ("cancelled", "expired") or claimed.caller_completed:
                result = claimed
            else:
                result = await self.run_pending(claimed)
            if not await store.clear_ownership(result):
                raise LeaseLostError(job.job_id)
            return result
        finally:
            await store.release(job.job_id, token)

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
        if job.state in ("cancelled", "expired") or job.caller_completed:
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

    def _build_query_enforcement(self, query: ResearchQuery, response: Any) -> dict[str, Any]:
        """Derive the subquery's filter-enforcement report (issue 187).

        Uses the same shared resolver as the MCP tools, resolved against the
        dispatched scope, so research jobs preserve the same enforcement truth
        as generic/targeted/specialist searches. Only non-default filters the
        subquery actually requested are reported (time_range for the ``fresh``
        strategy; language defaults to ``en`` and safesearch to ``off``).
        """
        report: dict[str, Any] = {}
        if query.time_range:
            report["time_range"] = resolve_filter_enforcement(
                response.scope.selected_engines,
                "time_range",
                query.time_range,
                self._service._ctx.active_engines,
            )
        return report

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
        if reserve_attempt(job, query) is None:
            if not await store.save_if_owned(job):
                raise LeaseLostError(job.job_id)
            return
        if not await store.save_if_owned(job):
            raise LeaseLostError(job.job_id)
        request = SearchRequest(
            query=query.query,
            engines=query.engines,
            time_range=query.time_range,
            include={"results", "engine_status"},
            client_identifier=f"mcp-job:{job.job_id}",
        )
        try:
            if not query.engines:
                raise QueryValidationError("Research query has no permitted engines")
            if self.dispatch_validator is not None:
                rejection = self.dispatch_validator(query)
                if rejection:
                    raise QueryValidationError(rejection)
            response = await self._service.search(request)
        except Exception as exc:  # noqa: BLE001 — persist unexpected execution failures for recovery
            query.state = "failed"
            query.error = str(exc)
            finish_attempt(job, query)
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
            ranking_explanation=response.ranking_explanation,
        )
        # Persist per-engine coverage and the disjoint bucket summary.
        query.engine_coverage = self._build_query_coverage(query, response)
        # Persist the filter-enforcement report (issue 187) so research
        # evidence carries the same enforcement truth as direct searches.
        query.enforcement = self._build_query_enforcement(query, response)
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
        finish_attempt(job, query, response)
        if not await store.save_if_owned(job):
            raise LeaseLostError(job.job_id)
