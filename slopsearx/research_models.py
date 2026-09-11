"""Research records, coverage classification, retry state and wire serialization.

This domain module owns no shared store or search execution state.
"""

from __future__ import annotations

import dataclasses
import secrets
import time
import uuid
from dataclasses import dataclass, field
from typing import Any

from slopsearx.adapter import EngineStatus


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
# (``ok``/``rate_limited``/``blocked``/``error``/``timeout``/``unavailable``)
# come from ``EngineStatus``; ``auth_required`` is the single derived token
# coming from a credential check, never from ``AdapterResponse.status``.
FAILURE_CLASS_TOKENS: tuple[str, ...] = (
    "ok",
    "rate_limited",
    "blocked",
    "error",
    "timeout",
    "unavailable",
    "auth_required",
)


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
    # Structured filter-enforcement report for this subquery (issue 187). Uses
    # the same schema/vocabulary as the MCP search envelope so the research
    # path preserves the same enforcement truth as generic/targeted/specialist
    # searches.
    enforcement: dict[str, Any] = field(default_factory=dict)


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
    # Optional additive metadata for bounded workflows built on the durable
    # research runner. Ordinary research jobs keep this empty.
    workflow: dict[str, Any] = field(default_factory=dict)
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
            enforcement=dict(item.get("enforcement") or {}),
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
        workflow=dict(payload.get("workflow") or {}),
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
