"""Tests for research jobs: planning, lifecycle, storage, cancellation."""

from __future__ import annotations

import time
from typing import Any

import engines  # noqa: F401
from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult
from slopsearx.capabilities import CapabilityCatalog, load_mcp_policy
from slopsearx.config import load_config
from slopsearx.research import (
    ResearchJob,
    ResearchJobRunner,
    ResearchJobStore,
    ResearchQuery,
    generate_job_id,
    plan_research_queries,
)
from slopsearx.service import AppContext, SearchService
from slopsearx.snapshot import SnapshotStore


class _FakeStore:
    """In-memory key-value store that also supports the stale-job scan."""

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
    """Minimal Valkey-client stand-in exposing keys(pattern)."""

    def __init__(self, store: _FakeStore) -> None:
        self._store = store

    async def keys(self, pattern: str) -> list[str]:
        prefix = pattern.rstrip("*")
        return [key for key in self._store._data if key.startswith(prefix)]


class _MockEngine(EngineAdapter):
    def __init__(self, name: str, status: EngineStatus = EngineStatus.OK, count: int = 2) -> None:
        super().__init__()
        self.name = name
        self._status = status
        self._count = count

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
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


def _make_state(engine_names: list[str] | None = None) -> tuple[Any, _FakeStore]:
    engine_names = engine_names or ["wikipedia", "brave", "arxiv", "duckduckgo", "reddit", "stackexchange"]
    engines_map = {name: _MockEngine(name) for name in engine_names}
    policy = load_mcp_policy(config_path=None)
    store = _FakeStore()
    ctx = AppContext(
        active_engines=engines_map,
        cache=store,
        tier1_engines=set(engine_names),
        sensitive_engines=policy.sensitive_engines,
    )
    catalog = CapabilityCatalog(config=load_config())
    service = SearchService(ctx)
    snapshots = SnapshotStore(store)
    job_store = ResearchJobStore(store)
    runner = ResearchJobRunner(service, job_store, snapshots, catalog, policy)
    return runner, store


class TestPlanning:
    def test_unknown_strategy_warns(self) -> None:
        runner, _ = _make_state()
        queries, warnings = plan_research_queries("q", "bogus", 5, 5, runner._catalog, runner._policy)
        assert queries == []
        assert any("unknown strategy" in w for w in warnings)

    def test_triangulate_plans_three_queries(self) -> None:
        runner, _ = _make_state()
        queries, warnings = plan_research_queries("question", "triangulate", 5, 5, runner._catalog, runner._policy)
        assert len(queries) == 3
        assert [q.intent for q in queries] == ["web", "science", "reference"]
        # Without the sensitive grant, sensitive engines (e.g. hibp, which
        # the reference profile includes) are excluded with an explicit
        # policy warning (VAL-RESEARCH-017), never silently dropped.
        assert warnings and all("sensitive engines excluded by policy" in w for w in warnings)

    def test_max_queries_bounds(self) -> None:
        runner, _ = _make_state()
        queries, _ = plan_research_queries("question", "broad", 2, 5, runner._catalog, runner._policy)
        assert len(queries) == 2

    def test_max_engines_bounds(self) -> None:
        runner, _ = _make_state()
        queries, _ = plan_research_queries("question", "broad", 10, 2, runner._catalog, runner._policy)
        assert all(len(q.engines) <= 2 for q in queries)

    def test_sensitive_engines_dropped_without_grant(self) -> None:
        runner, _ = _make_state()
        queries, _ = plan_research_queries("breach", "broad", 10, 20, runner._catalog, runner._policy)
        all_engines = [name for q in queries for name in q.engines]
        assert "hibp" not in all_engines
        assert "dehashed" not in all_engines


class TestJobLifecycle:
    async def test_full_success(self) -> None:
        runner, store = _make_state()
        job = ResearchJob(
            job_id=generate_job_id(),
            question="test",
            strategy="triangulate",
            deadline=time.time() + 3600,
        )
        queries, warnings = plan_research_queries("test", "triangulate", 3, 5, runner._catalog, runner._policy)
        job.queries = queries
        job.warnings = warnings
        await runner._jobs.save(job)

        await runner._run_job(job.job_id)

        finished = await runner._jobs.load(job.job_id)
        assert finished is not None
        assert finished.state == "succeeded"
        assert all(q.state == "done" for q in finished.queries)
        assert all(q.cursor is not None for q in finished.queries)
        assert all(q.query_id and q.query_id.startswith("ssx-") for q in finished.queries)

    async def test_partial_when_one_query_fails(self) -> None:
        runner, store = _make_state()
        # Fail every engine that appears in the "web" query of the
        # triangulate plan (brave, duckduckgo, reddit) so that query is
        # all-unresponsive; science/reference still have arxiv working.
        for name in ("reddit", "brave", "duckduckgo"):
            runner._service._ctx.active_engines[name] = _MockEngine(name, status=EngineStatus.ERROR)
        job = ResearchJob(
            job_id=generate_job_id(), question="test", strategy="triangulate", deadline=time.time() + 3600
        )
        queries, _ = plan_research_queries("test", "triangulate", 3, 5, runner._catalog, runner._policy)
        job.queries = queries
        await runner._jobs.save(job)

        await runner._run_job(job.job_id)

        finished = await runner._jobs.load(job.job_id)
        assert finished is not None
        assert finished.state == "partial"
        assert any(q.state == "done" for q in finished.queries)
        assert any(q.state == "failed" for q in finished.queries)

    async def test_cancel_stops_undispatched_queries(self) -> None:
        runner, store = _make_state()
        job = ResearchJob(job_id=generate_job_id(), question="test", strategy="broad", deadline=time.time() + 3600)
        queries, _ = plan_research_queries("test", "broad", 4, 5, runner._catalog, runner._policy)
        job.queries = queries
        # Simulate cancellation arriving before the runner starts
        job.cancel_requested = True
        job.state = "cancelled"
        await runner._jobs.save(job)

        await runner._run_job(job.job_id)

        finished = await runner._jobs.load(job.job_id)
        assert finished is not None
        assert finished.state == "cancelled"
        assert all(q.state == "cancelled" for q in finished.queries)

    async def test_expired_when_deadline_passed(self) -> None:
        runner, store = _make_state()
        job = ResearchJob(job_id=generate_job_id(), question="test", strategy="triangulate", deadline=time.time() - 10)
        queries, _ = plan_research_queries("test", "triangulate", 3, 5, runner._catalog, runner._policy)
        job.queries = queries
        await runner._jobs.save(job)

        await runner._run_job(job.job_id)

        finished = await runner._jobs.load(job.job_id)
        assert finished is not None
        assert finished.state == "expired"

    async def test_idempotency_dedupe(self) -> None:
        runner, store = _make_state()
        job = ResearchJob(
            job_id=generate_job_id(),
            question="test",
            strategy="triangulate",
            deadline=time.time() + 3600,
            idempotency_key="same-key",
        )
        queries, _ = plan_research_queries("test", "triangulate", 3, 5, runner._catalog, runner._policy)
        job.queries = queries
        await runner._jobs.save(job)

        found = await runner._jobs.find_by_idempotency("same-key")
        assert found is not None
        assert found.job_id == job.job_id

    async def test_job_store_round_trip(self) -> None:
        runner, store = _make_state()
        job = ResearchJob(
            job_id="job-abc",
            question="q",
            strategy="fresh",
            state="running",
            deadline=1234.5,
            tenant="default",
            queries=[
                ResearchQuery(index=0, query="q", intent="web", engines=["brave"], time_range="month", state="done")
            ],
            warnings=["warn"],
            cancel_requested=False,
        )
        await runner._jobs.save(job)
        loaded = await runner._jobs.load("job-abc")
        assert loaded is not None
        assert loaded.strategy == "fresh"
        assert loaded.queries[0].time_range == "month"
        assert loaded.queries[0].state == "done"

    async def test_expire_stale_running(self) -> None:
        runner, store = _make_state()
        stale = ResearchJob(
            job_id="job-stale", question="q", strategy="broad", state="running", deadline=time.time() - 10
        )
        queries, _ = plan_research_queries("q", "broad", 2, 5, runner._catalog, runner._policy)
        stale.queries = queries
        await runner._jobs.save(stale)

        expired = await runner._jobs.expire_stale_running()

        assert expired == 1
        loaded = await runner._jobs.load("job-stale")
        assert loaded is not None
        assert loaded.state == "expired"
        assert all(q.state == "cancelled" for q in loaded.queries)

    async def test_future_deadline_running_job_left_for_reclaim(self) -> None:
        runner, store = _make_state()
        stale = ResearchJob(
            job_id="job-future", question="q", strategy="broad", state="running", deadline=time.time() + 3600
        )
        queries, _ = plan_research_queries("q", "broad", 2, 5, runner._catalog, runner._policy)
        stale.queries = queries
        await runner._jobs.save(stale)

        # A future-deadline orphan is reclaimable, not expired.
        assert await runner._jobs.expire_stale_running() == 0
        loaded = await runner._jobs.load("job-future")
        assert loaded is not None
        assert loaded.state == "running"
