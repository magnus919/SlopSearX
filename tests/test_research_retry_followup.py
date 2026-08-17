"""Tests for the research lifecycle: idempotent bounded start, get_job
echoing key/deadline, selective retry, bounded extend, cancel preserving
evidence, expiration/stale cleanup, and honest no-store degradation.

Tools are plain async callables tested directly (FastMCP-free), exactly
like tests/test_mcp_tools.py and tests/test_policy_gate.py. Deterministic
in-memory stores + mock engines; no live engines, no Valkey.
"""

from __future__ import annotations

import time
from typing import Any

import pytest

import engines  # noqa: F401 — populates the engine registry
from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult
from slopsearx.capabilities import CapabilityCatalog, load_mcp_policy
from slopsearx.config import load_config
from slopsearx.mcp import tools as t
from slopsearx.mcp.state import McpState, set_state
from slopsearx.research import (
    ResearchJob,
    ResearchJobRunner,
    ResearchJobStore,
    ResearchQuery,
    generate_job_id,
)
from slopsearx.service import AppContext, SearchService
from slopsearx.snapshot import SnapshotStore


class _FakeStore:
    """In-memory key-value store (Valkey stand-in)."""

    def __init__(self) -> None:
        self.is_connected = True
        self._data: dict[str, dict[str, Any]] = {}
        self._client = _FakeClient(self)

    async def get(self, key: str) -> dict[str, Any] | None:
        return self._data.get(key)

    async def set(self, key: str, value: dict[str, Any], ttl: int = 300) -> None:
        del ttl
        self._data[key] = value


class _FakeClient:
    """Minimal client exposing keys(pattern) for the stale-job scan."""

    def __init__(self, store: _FakeStore) -> None:
        self._store = store

    async def keys(self, pattern: str) -> list[str]:
        prefix = pattern.rstrip("*")
        return [key for key in self._store._data if key.startswith(prefix)]


class _MockEngine(EngineAdapter):
    """Parameterizable mock engine with a real registry name and a call counter."""

    def __init__(self, name: str, status: EngineStatus = EngineStatus.OK, count: int = 3) -> None:
        super().__init__()
        self.name = name
        self._status = status
        self._count = count
        self.calls = 0

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        self.calls += 1
        if self._status != EngineStatus.OK:
            return AdapterResponse(results=[], status=self._status, error_message="boom", latency_ms=1.0)
        return AdapterResponse(
            results=[
                SearchResult(
                    url=f"https://{self.name}{i}.com",
                    title=f"{self.name} {i}",
                    content=f"Content {i}.",
                    engine=self.name,
                )
                for i in range(self._count)
            ],
            status=EngineStatus.OK,
            latency_ms=1.0,
        )


def _build_state(
    engine_names: list[str] | None = None,
    *,
    research_enabled: bool = True,
) -> McpState:
    engine_names = engine_names or ["wikipedia", "brave", "duckduckgo", "arxiv"]
    engines_map = {name: _MockEngine(name) for name in engine_names}
    policy = load_mcp_policy(config_path=None)
    policy.enabled_tools["research"] = research_enabled
    ctx = AppContext(
        active_engines=engines_map,
        cache=_FakeStore(),
        tier1_engines=set(engine_names),
        sensitive_engines=policy.sensitive_engines,
    )
    catalog = CapabilityCatalog(config=load_config())
    service = SearchService(ctx)
    store = _FakeStore()
    snapshots = SnapshotStore(store, ttl_seconds=policy.snapshot_ttl_seconds)
    job_store = ResearchJobStore(store)
    runner = ResearchJobRunner(service, job_store, snapshots, catalog, policy)
    return McpState(
        ctx=ctx,
        policy=policy,
        catalog=catalog,
        service=service,
        snapshots=snapshots,
        job_store=job_store,
        runner=runner,
        version="test",
    )


@pytest.fixture
def state() -> McpState:
    state_obj = _build_state()
    set_state(state_obj)
    yield state_obj
    set_state(None)


async def _persist_and_run(state: McpState, queries: list[ResearchQuery], **job_kwargs: Any) -> ResearchJob:
    """Save a job with the given queries, run it through the runner, return it."""
    job = ResearchJob(job_id=generate_job_id(), question="q", strategy="triangulate", deadline=time.time() + 3600)
    for k, v in job_kwargs.items():
        setattr(job, k, v)
    job.queries = queries
    await state.job_store.save(job)
    await state.runner._run_job(job.job_id)
    loaded = await state.job_store.load(job.job_id)
    assert loaded is not None
    return loaded


def _run_query(job: ResearchJob, index: int) -> ResearchQuery:
    return next(q for q in job.queries if q.index == index)


# ---------------------------------------------------------------------------
# VAL-RESEARCH-001 / 022 — bounded start handle; get_job echoes key + deadline
# ---------------------------------------------------------------------------


class TestStartHandle:
    async def test_start_returns_full_bounded_handle(self, state: McpState) -> None:
        """VAL-RESEARCH-001 — handle carries all fields, bounded by policy clamps."""
        result = await t.slopsearx_start_research(
            "some question",
            strategy="triangulate",
            max_queries=2,
            max_engines_per_query=2,
            deadline="2030-01-01T00:00:00Z",
            idempotency_key="key-1",
        )
        assert "error" not in result
        for field in ("job_id", "state", "question", "strategy", "progress", "deadline", "idempotency_key", "queries"):
            assert field in result, f"missing handle field {field}"
        assert result["idempotency_key"] == "key-1"
        assert result["deadline"]
        assert result["progress"]["total"] == 2
        assert all(len(q["engines"]) <= 2 for q in result["queries"])
        assert result["job_id"].startswith("job-")
        for q in result["queries"]:
            for field in ("index", "query", "intent", "engines", "state"):
                assert field in q

    async def test_independent_starts_have_unique_job_ids(self, state: McpState) -> None:
        """VAL-RESEARCH-001 — job_id is unique across independent starts."""
        r1 = await t.slopsearx_start_research("q1", idempotency_key="a")
        r2 = await t.slopsearx_start_research("q2", idempotency_key="b")
        assert r1["job_id"] != r2["job_id"]

    async def test_get_job_echoes_idempotency_key_and_deadline(self, state: McpState) -> None:
        """VAL-RESEARCH-022 — get_job echoes the submitted key and resolved deadline."""
        r = await t.slopsearx_start_research("q", deadline="2031-01-01T00:00:00Z", idempotency_key="echo-key")
        got = await t.slopsearx_get_job(r["job_id"])
        assert "error" not in got
        assert got["idempotency_key"] == "echo-key"
        assert got["deadline"] == r["deadline"]
        assert got["deadline"]


# ---------------------------------------------------------------------------
# VAL-RESEARCH-002 / 003 — idempotency and honest degradation
# ---------------------------------------------------------------------------


class TestIdempotency:
    async def test_repeated_start_same_key_returns_same_job(self, state: McpState) -> None:
        """VAL-RESEARCH-002 — a duplicate key returns the same job, one stored record."""
        r1 = await t.slopsearx_start_research("q", idempotency_key="dup")
        r2 = await t.slopsearx_start_research("q", idempotency_key="dup")
        assert "error" not in r1 and "error" not in r2
        assert r1["job_id"] == r2["job_id"]
        found = await state.job_store.find_by_idempotency("dup")
        assert found is not None
        assert found.job_id == r1["job_id"]
        # second call returns the existing job, not a duplicate run
        assert any("existing job" in (r2.get("note") or "") for _ in [0])

    async def test_fresh_key_store_available_creates_one_job(self, state: McpState) -> None:
        """VAL-RESEARCH-003 — a fresh key yields exactly one new job (store available)."""
        r = await t.slopsearx_start_research("q", idempotency_key="fresh-avail")
        assert "error" not in r
        found = await state.job_store.find_by_idempotency("fresh-avail")
        assert found is not None
        assert found.job_id == r["job_id"]

    async def test_start_store_unavailable_is_degraded_and_ephemeral(self, state: McpState) -> None:
        """VAL-RESEARCH-003 — no store => explicit degraded/ephemeral, no idempotency claim."""
        state.job_store._store.is_connected = False  # type: ignore[attr-defined]
        result = await t.slopsearx_start_research("q", idempotency_key="fresh-unavail")
        assert "error" not in result
        assert result.get("degraded") is True
        assert result.get("ephemeral") is True
        assert "not persisted" in (result.get("note") or "")
        assert "will not be executed" in (result.get("note") or "")


# ---------------------------------------------------------------------------
# VAL-RESEARCH-008 / 021 / VAL-CROSS-007 — selective retry preserves evidence
# ---------------------------------------------------------------------------


class TestRetry:
    async def test_retry_reruns_only_failed_and_preserves_successful(self, state: McpState) -> None:
        """VAL-RESEARCH-008 + VAL-CROSS-007 — successful subquery untouched, failed re-run."""
        state.ctx.active_engines["wikipedia"] = _MockEngine("wikipedia", status=EngineStatus.ERROR)
        job = await _persist_and_run(
            state,
            [
                ResearchQuery(index=0, query="ok", intent="web", engines=["brave"]),
                ResearchQuery(index=1, query="fail", intent="web", engines=["wikipedia"]),
            ],
        )
        assert _run_query(job, 0).state == "done"
        assert _run_query(job, 1).state == "failed"
        success_cursor = _run_query(job, 0).cursor
        failed_cursor = _run_query(job, 1).cursor
        assert success_cursor is not None and failed_cursor is not None
        brave_calls = state.ctx.active_engines["brave"].calls
        wiki_calls = state.ctx.active_engines["wikipedia"].calls

        # Read the successful cursor before retry (must be byte-identical after).
        before = await t.slopsearx_read_results(success_cursor)
        assert "error" not in before

        result = await t.slopsearx_retry_research(job.job_id)
        assert "error" not in result
        assert result.get("retried") == [1]

        job = await state.job_store.load(job.job_id)
        assert job is not None
        # Successful subquery was NOT re-executed and its cursor is unchanged.
        assert state.ctx.active_engines["brave"].calls == brave_calls
        assert _run_query(job, 0).cursor == success_cursor
        # Failed subquery gained a new attempt/cursor.
        assert state.ctx.active_engines["wikipedia"].calls > wiki_calls
        assert _run_query(job, 1).cursor != failed_cursor

        after = await t.slopsearx_read_results(success_cursor)
        assert "error" not in after
        assert after == before

    async def test_retry_preserves_original_failed_attempt_evidence(self, state: McpState) -> None:
        """VAL-RESEARCH-021 — the original failed attempt's cursor stays readable."""
        state.ctx.active_engines["wikipedia"] = _MockEngine("wikipedia", status=EngineStatus.ERROR)
        job = await _persist_and_run(
            state,
            [
                ResearchQuery(index=0, query="ok", intent="web", engines=["brave"]),
                ResearchQuery(index=1, query="fail", intent="web", engines=["wikipedia"]),
            ],
        )
        failed_cursor = _run_query(job, 1).cursor
        assert failed_cursor is not None
        pre = await t.slopsearx_read_results(failed_cursor)
        assert "error" not in pre

        await t.slopsearx_retry_research(job.job_id)
        job = await state.job_store.load(job.job_id)
        assert job is not None
        # Old cursor still resolves, and the query now carries both cursors.
        post = await t.slopsearx_read_results(failed_cursor)
        assert "error" not in post
        assert post == pre
        cursors = [a.cursor for a in _run_query(job, 1).attempts]
        assert failed_cursor in cursors
        assert len(cursors) == 2
        assert len(set(cursors)) == 2

    async def test_retry_no_retryable_work_is_explicit(self, state: McpState) -> None:
        """VAL-RESEARCH-016 — fully succeeded job => structured no_retryable_work signal."""
        job = await _persist_and_run(state, [ResearchQuery(index=0, query="ok", intent="web", engines=["brave"])])
        assert _run_query(job, 0).state == "done"
        result = await t.slopsearx_retry_research(job.job_id)
        assert result["error"]["code"] == "no_retryable_work"
        # zero re-execution
        assert state.ctx.active_engines["brave"].calls == 1

    async def test_retry_deadline_passed_finalizes_to_expired(self, state: McpState) -> None:
        """research-snapshot-hardening (c) — a deadline-passed retry finalizes to expired.

        Instead of re-running (which would end in ``partial``/``failed`` when
        the deadline check fires mid-run), a retry on a job whose deadline has
        already passed must finalize the job to ``expired`` and re-execute no
        subqueries.
        """
        state.ctx.active_engines["wikipedia"] = _MockEngine("wikipedia", status=EngineStatus.ERROR)
        job = await _persist_and_run(
            state,
            [ResearchQuery(index=0, query="fail", intent="web", engines=["wikipedia"])],
        )
        assert _run_query(job, 0).state == "failed"
        wiki_calls = state.ctx.active_engines["wikipedia"].calls

        # Push the deadline into the past, then retry.
        job.deadline = time.time() - 10
        await state.job_store.save(job)

        result = await t.slopsearx_retry_research(job.job_id)
        assert "error" not in result
        assert result["state"] == "expired"
        assert "deadline" in result["note"]

        job = await state.job_store.load(job.job_id)
        assert job is not None
        assert job.state == "expired"
        # No subquery was re-executed and the failed query is unchanged.
        assert state.ctx.active_engines["wikipedia"].calls == wiki_calls
        assert _run_query(job, 0).state == "failed"

    async def test_retry_terminal_cancelled_job_not_resurrected(self, state: McpState) -> None:
        """research-snapshot-hardening (c) — a cancelled job is never resurrected by retry."""
        state.ctx.active_engines["wikipedia"] = _MockEngine("wikipedia", status=EngineStatus.ERROR)
        job = await _persist_and_run(
            state,
            [ResearchQuery(index=0, query="fail", intent="web", engines=["wikipedia"])],
        )
        assert _run_query(job, 0).state == "failed"
        # Mark the whole job cancelled (terminal) even though a failed query exists.
        job.state = "cancelled"
        await state.job_store.save(job)
        wiki_calls = state.ctx.active_engines["wikipedia"].calls

        result = await t.slopsearx_retry_research(job.job_id)
        assert "error" not in result
        assert result["state"] == "cancelled"
        assert "not retried" in result["note"]
        # No re-execution happened.
        assert state.ctx.active_engines["wikipedia"].calls == wiki_calls


# ---------------------------------------------------------------------------
# VAL-RESEARCH-009 / 015 / 020 — bounded, validated follow-up
# ---------------------------------------------------------------------------


class TestExtend:
    async def test_extend_appends_within_budget(self, state: McpState) -> None:
        """VAL-RESEARCH-009 — one new query, incremented total, prior evidence untouched."""
        job = await _persist_and_run(state, [ResearchQuery(index=0, query="ok", intent="web", engines=["brave"])])
        prev_cursor = _run_query(job, 0).cursor
        total_before = len(job.queries)

        result = await t.slopsearx_extend_research(job.job_id, "followup", intent="web")
        assert "error" not in result
        assert result["progress"]["total"] == total_before + 1

        job = await state.job_store.load(job.job_id)
        assert job is not None
        assert len(job.queries) == total_before + 1
        assert _run_query(job, 0).cursor == prev_cursor
        new_q = job.queries[-1]
        assert new_q.query == "followup"
        assert new_q.index == total_before
        assert new_q.intent == "web"
        assert new_q.state in ("done", "failed")
        assert new_q.engine_coverage

    async def test_extend_over_budget_rejected_without_corruption(self, state: McpState) -> None:
        """VAL-RESEARCH-015 — at query budget, extend is rejected and job unchanged."""
        state.policy.job_max_queries = 1
        job = await _persist_and_run(state, [ResearchQuery(index=0, query="q", intent="web", engines=["brave"])])
        total_before = len(job.queries)
        prev_cursor = _run_query(job, 0).cursor

        result = await t.slopsearx_extend_research(job.job_id, "more")
        assert result["error"]["code"] == "job_budget_exceeded"

        job = await state.job_store.load(job.job_id)
        assert job is not None
        assert len(job.queries) == total_before
        assert _run_query(job, 0).cursor == prev_cursor

    async def test_extend_deadline_passed_rejected(self, state: McpState) -> None:
        """VAL-RESEARCH-015 — past deadline, extend is rejected without corrupting."""
        job = await _persist_and_run(
            state,
            [ResearchQuery(index=0, query="q", intent="web", engines=["brave"])],
            deadline=time.time() - 10,
        )
        result = await t.slopsearx_extend_research(job.job_id, "more")
        assert result["error"]["code"] == "deadline_exceeded"

    async def test_extend_empty_query_rejected(self, state: McpState) -> None:
        """VAL-RESEARCH-020 — empty follow-up query rejected as invalid_input."""
        job = await _persist_and_run(state, [ResearchQuery(index=0, query="q", intent="web", engines=["brave"])])
        result = await t.slopsearx_extend_research(job.job_id, "   ")
        assert result["error"]["code"] == "invalid_input"
        assert result["error"]["field"] == "query"

    async def test_extend_overlong_query_rejected(self, state: McpState) -> None:
        """VAL-RESEARCH-020 — over-long follow-up query rejected."""
        job = await _persist_and_run(state, [ResearchQuery(index=0, query="q", intent="web", engines=["brave"])])
        result = await t.slopsearx_extend_research(job.job_id, "x" * 600)
        assert result["error"]["code"] == "invalid_input"
        assert result["error"]["field"] == "query"

    async def test_extend_sensitive_engine_blocked_without_grant(self, state: McpState) -> None:
        """VAL-RESEARCH-020 — a follow-up scope hitting a sensitive engine is denied."""
        job = await _persist_and_run(state, [ResearchQuery(index=0, query="q", intent="web", engines=["brave"])])
        result = await t.slopsearx_extend_research(job.job_id, "breach", engines=["hibp"])
        assert result["error"]["code"] == "tool_disabled"
        job = await state.job_store.load(job.job_id)
        assert job is not None
        assert len(job.queries) == 1

    async def test_extend_explicit_engines_capped_to_per_query_bound(self, state: McpState) -> None:
        """research-snapshot-hardening (b) — explicit engines list is capped.

        In parity with the intent path, an explicit engine list in extend must
        be capped to ``job_max_engines_per_query`` rather than appended whole.
        """
        state.policy.job_max_engines_per_query = 2
        job = await _persist_and_run(state, [ResearchQuery(index=0, query="q", intent="web", engines=["brave"])])
        result = await t.slopsearx_extend_research(
            job.job_id,
            "followup",
            engines=["brave", "wikipedia", "duckduckgo", "arxiv"],
        )
        assert "error" not in result
        job = await state.job_store.load(job.job_id)
        assert job is not None
        new_q = job.queries[-1]
        assert len(new_q.engines) == 2
        assert new_q.engines == ["brave", "wikipedia"]
        assert len(set(new_q.engines)) == 2

    async def test_extend_invalid_intent_rejected(self, state: McpState) -> None:
        """VAL-RESEARCH-020 — unknown follow-up intent rejected with alternatives."""
        job = await _persist_and_run(state, [ResearchQuery(index=0, query="q", intent="web", engines=["brave"])])
        result = await t.slopsearx_extend_research(job.job_id, "more", intent="nope")
        assert result["error"]["code"] == "invalid_input"
        assert result["error"]["field"] == "intent"
        assert "web" in result["error"]["valid_alternatives"]

    async def test_extend_after_durable_execution_clears_stale_lease(self, state: McpState) -> None:
        """A durable-executed job keeps stale lease fields; extend must not raise.

        ``release`` deletes the Valkey lease key but not the record fields, so
        a job that finished under the durable worker still carries a stale
        ``lease_token``. Extend must clear those fields before ``run_pending``
        (which otherwise tries to renew the missing lease and raises
        ``LeaseLostError``).
        """
        job = ResearchJob(
            job_id=generate_job_id(),
            question="q",
            strategy="triangulate",
            state="succeeded",
            deadline=time.time() + 3600,
            owner_id="worker-old",
            lease_token="stale-lease-token",
            queries=[
                ResearchQuery(index=0, query="done", intent="web", engines=["brave"], state="done", cursor="snap-old"),
            ],
        )
        await state.job_store.save(job)

        result = await t.slopsearx_extend_research(job.job_id, "followup", intent="web")

        assert "error" not in result
        job = await state.job_store.load(job.job_id)
        assert job is not None
        assert len(job.queries) == 2
        assert job.queries[0].cursor == "snap-old"
        assert job.queries[1].state == "done"
        assert job.owner_id is None
        assert job.lease_token is None


# ---------------------------------------------------------------------------
# VAL-RESEARCH-010 / 011 — cancel and expiration preserve completed evidence
# ---------------------------------------------------------------------------


class TestCancelAndExpiry:
    async def test_cancel_preserves_completed_evidence(self, state: McpState) -> None:
        """VAL-RESEARCH-010 — cancel preserves completed evidence; a finished partial job stays partial."""
        state.ctx.active_engines["wikipedia"] = _MockEngine("wikipedia", status=EngineStatus.ERROR)
        job = await _persist_and_run(
            state,
            [
                ResearchQuery(index=0, query="ok", intent="web", engines=["brave"]),
                ResearchQuery(index=1, query="fail", intent="web", engines=["wikipedia"]),
            ],
        )
        done_cursor = _run_query(job, 0).cursor
        assert done_cursor is not None

        result = await t.slopsearx_cancel_job(job.job_id)
        # A finished partial job is not rewritten to cancelled: it keeps its
        # partial state so the failed subquery stays retryable.
        assert result["state"] == "partial"

        re = await t.slopsearx_read_results(done_cursor)
        assert "error" not in re
        job = await state.job_store.load(job.job_id)
        assert job is not None
        assert job.state == "partial"
        assert _run_query(job, 0).state == "done"
        assert _run_query(job, 1).state == "failed"

    async def test_cancel_finished_partial_job_reports_already_finished(self, state: McpState) -> None:
        """Cancelling a finished ``partial`` job returns the "already finished"
        note, not "best-effort cancellation requested"."""
        state.ctx.active_engines["wikipedia"] = _MockEngine("wikipedia", status=EngineStatus.ERROR)
        job = await _persist_and_run(
            state,
            [
                ResearchQuery(index=0, query="ok", intent="web", engines=["brave"]),
                ResearchQuery(index=1, query="fail", intent="web", engines=["wikipedia"]),
            ],
        )
        assert job.state == "partial"

        result = await t.slopsearx_cancel_job(job.job_id)

        assert result["state"] == "partial"
        assert "already finished" in result["note"]
        assert "best-effort cancellation requested" not in result["note"]
        loaded = await state.job_store.load(job.job_id)
        assert loaded is not None
        assert loaded.state == "partial"
        assert loaded.cancel_requested is False

    async def test_deadline_passed_finalizes_to_expired(self, state: McpState) -> None:
        """VAL-RESEARCH-011 — a deadline-passed job is finalized (not left running)."""
        job = await _persist_and_run(
            state,
            [ResearchQuery(index=0, query="q", intent="web", engines=["brave"])],
            deadline=time.time() - 10,
        )
        assert job.state == "expired"

    async def test_stale_running_cleanup_marks_expired(self, state: McpState) -> None:
        """VAL-RESEARCH-011 — startup cleanup expires deadline-passed orphans only."""
        stale = ResearchJob(
            job_id="job-stale-cleanup",
            question="q",
            strategy="broad",
            state="running",
            deadline=time.time() - 10,
        )
        stale.queries = [ResearchQuery(index=0, query="q", intent="web", engines=["brave"])]
        await state.job_store.save(stale)

        expired = await state.job_store.expire_stale_running()
        assert expired == 1
        loaded = await state.job_store.load("job-stale-cleanup")
        assert loaded is not None
        assert loaded.state == "expired"
        assert all(q.state == "cancelled" for q in loaded.queries)

    async def test_future_deadline_running_job_left_for_reclaim(self, state: McpState) -> None:
        """VAL-RESEARCH-011 — a future-deadline orphan stays reclaimable."""
        stale = ResearchJob(
            job_id="job-reclaimable",
            question="q",
            strategy="broad",
            state="running",
            deadline=time.time() + 3600,
        )
        stale.queries = [ResearchQuery(index=0, query="q", intent="web", engines=["brave"])]
        await state.job_store.save(stale)

        assert await state.job_store.expire_stale_running() == 0
        loaded = await state.job_store.load("job-reclaimable")
        assert loaded is not None
        assert loaded.state == "running"


# ---------------------------------------------------------------------------
# VAL-RESEARCH-012 / 013 / 014 / 018 — errors, grant gate, degradation
# ---------------------------------------------------------------------------


class TestResearchErrorsAndDegradation:
    async def test_unknown_job_id_structured_error_on_every_operation(self, state: McpState) -> None:
        """VAL-RESEARCH-012 — get/cancel/retry/extend all return invalid_job_id."""
        cases = [
            t.slopsearx_get_job("nope"),
            t.slopsearx_cancel_job("nope"),
            t.slopsearx_retry_research("nope"),
            t.slopsearx_extend_research("nope", "q"),
        ]
        for result in await _gather(*cases):
            assert result["error"]["code"] == "invalid_job_id"
        # no partial state was created
        assert await state.job_store.load("nope") is None

    async def test_disabled_grant_fails_closed(self, state: McpState) -> None:
        """VAL-RESEARCH-013 — start/retry/extend are tool_disabled without the research grant."""
        state.policy.enabled_tools["research"] = False
        assert (await t.slopsearx_start_research("q"))["error"]["code"] == "tool_disabled"
        assert (await t.slopsearx_retry_research("job-x"))["error"]["code"] == "tool_disabled"
        assert (await t.slopsearx_extend_research("job-x", "q"))["error"]["code"] == "tool_disabled"

    async def test_invalid_strategy_rejected_with_alternatives(self, state: McpState) -> None:
        """VAL-RESEARCH-014 — an unknown strategy is rejected with valid alternatives."""
        result = await t.slopsearx_start_research("q", strategy="bogus")
        assert result["error"]["code"] == "invalid_input"
        assert result["error"]["field"] == "strategy"
        assert set(result["error"]["valid_alternatives"]) == {"triangulate", "broad", "fresh", "counterevidence"}

    async def test_lifecycle_degrades_honestly_when_store_unavailable(self, state: McpState) -> None:
        """VAL-RESEARCH-018 — every operation degrades honestly, never claims durability."""
        state.job_store._store.is_connected = False  # type: ignore[attr-defined]
        start = await t.slopsearx_start_research("q", idempotency_key="x")
        assert start.get("degraded") is True or start.get("ephemeral") is True
        assert (await t.slopsearx_get_job("any"))["error"]["code"] == "store_unavailable"
        assert (await t.slopsearx_cancel_job("any"))["error"]["code"] == "store_unavailable"
        assert (await t.slopsearx_retry_research("any"))["error"]["code"] == "store_unavailable"
        assert (await t.slopsearx_extend_research("any", "q"))["error"]["code"] == "store_unavailable"


async def _gather(*coros: Any) -> list[Any]:
    import asyncio

    return list(await asyncio.gather(*coros))
