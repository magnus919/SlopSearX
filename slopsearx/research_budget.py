"""Durable research accounting; reservations are conservative after crashes."""

from __future__ import annotations

import hashlib
import time
import uuid
from typing import Any

from slopsearx.capabilities import MCPPolicy
from slopsearx.merger import _normalise_url
from slopsearx.research_models import ResearchJob, ResearchQuery, ResearchQueryAttempt
from slopsearx.service import SearchResponse


class ResearchMutationError(ValueError):
    """An authoritative leased mutation failed without changing the record."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def initialize_budget(job: ResearchJob, policy: MCPPolicy) -> None:
    """Persist default legacy accounting once and only lower stored ceilings."""
    ceilings = {
        "queries": policy.job_max_queries,
        "attempts": policy.job_max_queries,
        "engines_per_query": policy.job_max_engines_per_query,
        "engine_attempts": policy.job_max_queries * policy.job_max_engines_per_query,
        "results": policy.job_max_results,
    }
    legacy = not job.budget_limits
    for key, ceiling in ceilings.items():
        job.budget_limits[key] = min(job.budget_limits.get(key, ceiling), ceiling)
    if legacy:
        # Completed legacy attempts count even when no explicit attempt ID exists.
        attempts = [
            (query, max(len(query.attempts), int(query.state in {"done", "failed", "running"})))
            for query in job.queries
        ]
        job.budget_used = {
            "attempts": sum(count for _, count in attempts),
            "engine_attempts": sum(count * len(query.engines) for query, count in attempts),
            "results": sum(
                sum(a.result_count for a in query.attempts) if query.attempts else query.result_count
                for query in job.queries
            ),
        }
    for key in ("attempts", "engine_attempts", "results"):
        job.budget_used.setdefault(key, 0)


def budget_summary(job: ResearchJob) -> dict[str, Any]:
    used = {**job.budget_used, "queries": len(job.queries)}
    return {
        "limits": dict(job.budget_limits),
        "used": used,
        "remaining": {
            key: max(0, limit - used.get(key, 0))
            for key, limit in job.budget_limits.items()
            if key != "engines_per_query"
        },
        "accounting": "Attempts and engine slots are reserved before dispatch; admitted leads each use one slot.",
    }


def reserve_attempt(job: ResearchJob, query: ResearchQuery) -> ResearchQueryAttempt | None:
    """Prepare one attempt under the caller's lease, without performing I/O."""

    def reject(reason: str) -> None:
        job.stop_reason = reason
        query.state = "failed"
        query.error = reason

    if query.attempts and query.attempts[-1].state == "running":
        interrupted = query.attempts[-1]
        interrupted.state = "interrupted"
        interrupted.finished_at = time.time()
        interrupted.error = "Previous dispatch outcome is unknown after worker recovery; reservation retained"
    for key, amount, reason in (
        ("attempts", 1, "attempt_budget_exhausted"),
        ("engine_attempts", len(query.engines), "engine_budget_exhausted"),
        ("results", 1, "result_budget_exhausted"),
    ):
        if job.budget_used[key] + amount > job.budget_limits[key]:
            reject(reason)
            return None
    if len(query.engines) > job.budget_limits["engines_per_query"]:
        reject("engine_budget_exhausted")
        return None
    query.query_id = query.cursor = query.error = None
    query.result_count = 0
    query.engine_coverage = []
    query.enforcement = {}
    query.state = "running"
    attempt = ResearchQueryAttempt(attempt_id=f"attempt-{uuid.uuid4().hex}", state="running")
    query.attempts.append(attempt)
    job.budget_used["attempts"] += 1
    job.budget_used["engine_attempts"] += len(query.engines)
    return attempt


def finish_attempt(job: ResearchJob, query: ResearchQuery, response: SearchResponse | None = None) -> None:
    """Finalize the active attempt once; do not modify historical evidence."""
    attempt = query.attempts[-1]
    if attempt.state != "running":
        raise ValueError("Only a running attempt may be finalized")
    attempt.finished_at = time.time()
    attempt.state = query.state
    attempt.cursor, attempt.query_id = query.cursor, query.query_id
    attempt.result_count, attempt.error = query.result_count, query.error
    attempt.engine_coverage = list(query.engine_coverage)
    attempt.enforcement = dict(query.enforcement)
    if response is not None and query.cursor:
        available = max(0, job.budget_limits["results"] - job.budget_used["results"])
        admitted = response.results[:available]
        if len(response.results) > available:
            job.stop_reason = "result_budget_exhausted"
        known = set(job.seen_lead_ids)
        for index, result in enumerate(admitted):
            attempt.admitted_result_ids.append(f"{query.cursor}:{index}")
            lead_id = "lead-v1-" + hashlib.sha256(_normalise_url(result.url).encode()).hexdigest()
            if lead_id not in known:
                known.add(lead_id)
                job.seen_lead_ids.append(lead_id)
                attempt.new_lead_ids.append(lead_id)
        job.budget_used["results"] += len(admitted)
