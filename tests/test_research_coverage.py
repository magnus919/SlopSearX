"""Tests for research per-query/per-engine coverage and the failure-class taxonomy.

Covers VAL-RESEARCH-004, VAL-RESEARCH-005, VAL-RESEARCH-006, VAL-RESEARCH-007,
VAL-RESEARCH-019, and VAL-CROSS-006. The pinned outcome->bucket->token join
(classify_coverage) and the disjoint-bucket summary are pure functions tested
directly; the runner integration tests exercise per-query state and per-engine
coverage through the in-memory store and the MCP get_job surface.
"""

from __future__ import annotations

import time
from typing import Any

import pytest

import engines  # noqa: F401 — triggers @register_engine to populate registry
from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult
from slopsearx.capabilities import CapabilityCatalog, load_mcp_policy
from slopsearx.config import load_config
from slopsearx.mcp import tools as t
from slopsearx.mcp.state import McpState, set_state
from slopsearx.research import (
    COVERAGE_BUCKETS,
    ResearchJob,
    ResearchJobRunner,
    ResearchJobStore,
    ResearchQuery,
    classify_coverage,
    generate_job_id,
    status_token,
    summarize_coverage,
)
from slopsearx.service import AppContext, SearchService
from slopsearx.snapshot import SnapshotStore


class _FakeStore:
    """In-memory key-value store used as cache/snapshot/job storage."""

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
    """Parameterizable mock engine that records invocation count."""

    def __init__(
        self,
        name: str,
        status: EngineStatus = EngineStatus.OK,
        count: int = 2,
    ) -> None:
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


def _build_state(engine_names: list[str] | None = None) -> tuple[McpState, _FakeStore]:
    engine_names = engine_names or ["wikipedia", "arxiv", "stackexchange", "duckduckgo", "reddit", "brave"]
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
    snapshots = SnapshotStore(store, ttl_seconds=policy.snapshot_ttl_seconds)
    job_store = ResearchJobStore(store)
    runner = ResearchJobRunner(service, job_store, snapshots, catalog, policy)
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


def _make_job(state: McpState, queries: list[ResearchQuery]) -> ResearchJob:
    job = ResearchJob(
        job_id=generate_job_id(),
        question="test question",
        strategy="triangulate",
        deadline=time.time() + 3600,
        queries=queries,
    )
    return job


# ---------------------------------------------------------------------------
# VAL-RESEARCH-006: failure classes are machine-readable and stable
# ---------------------------------------------------------------------------


class TestFailureClassTokens:
    def test_status_derived_tokens_are_stable(self) -> None:
        assert status_token(EngineStatus.OK) == "ok"
        assert status_token(EngineStatus.RATE_LIMITED) == "rate_limited"
        assert status_token(EngineStatus.BLOCKED) == "blocked"
        assert status_token(EngineStatus.ERROR) == "error"
        assert status_token(EngineStatus.TIMEOUT) == "timeout"

    def test_token_strings_pass_through(self) -> None:
        assert status_token("ok") == "ok"
        assert status_token("timeout") == "timeout"
        assert status_token(None) is None


# ---------------------------------------------------------------------------
# VAL-RESEARCH-019: pinned outcome -> bucket -> token join
# ---------------------------------------------------------------------------


class TestPinnedOutcomeJoin:
    @pytest.mark.parametrize(
        "status,result_count,credential,dispatched,expected_bucket,expected_class",
        [
            (EngineStatus.OK, 3, False, True, "successful", "ok"),
            (EngineStatus.OK, 0, False, True, "empty", "ok"),
            (EngineStatus.RATE_LIMITED, 0, False, True, "failed", "rate_limited"),
            (EngineStatus.BLOCKED, 0, False, True, "failed", "blocked"),
            (EngineStatus.ERROR, 0, False, True, "failed", "error"),
            (EngineStatus.TIMEOUT, 0, False, True, "failed", "timeout"),
            (EngineStatus.OK, 0, True, True, "unavailable", "auth_required"),
            (None, 0, False, False, "not-selected", None),
        ],
    )
    def test_pinned_mapping(
        self,
        status: EngineStatus,
        result_count: int,
        credential: bool,
        dispatched: bool,
        expected_bucket: str,
        expected_class: str,
    ) -> None:
        cov = classify_coverage(
            engine="e",
            dispatched=dispatched,
            status=status,
            result_count=result_count,
            credential_missing=credential,
        )
        assert cov.bucket == expected_bucket
        assert cov.failure_class == expected_class

    def test_auth_required_derived_not_from_status(self) -> None:
        # Even an ok-with-results engine is unavailable when credentials are
        # missing: the auth_required token comes from the credential check,
        # never from AdapterResponse.status.
        cov = classify_coverage(
            engine="hibp", dispatched=True, status=EngineStatus.OK, result_count=5, credential_missing=True
        )
        assert cov.bucket == "unavailable"
        assert cov.failure_class == "auth_required"

    def test_join_is_deterministic(self) -> None:
        a = classify_coverage(engine="e", dispatched=True, status=EngineStatus.ERROR, result_count=0)
        b = classify_coverage(engine="e", dispatched=True, status=EngineStatus.ERROR, result_count=0)
        assert a.bucket == b.bucket == "failed"
        assert a.failure_class == b.failure_class == "error"


# ---------------------------------------------------------------------------
# VAL-RESEARCH-005: disjoint buckets and the attempted aggregate
# ---------------------------------------------------------------------------


class TestCoverageSummary:
    def test_buckets_disjoint_and_attempted_aggregate(self) -> None:
        coverage = [
            classify_coverage(engine="a", dispatched=True, status=EngineStatus.OK, result_count=3),  # successful
            classify_coverage(engine="b", dispatched=True, status=EngineStatus.OK, result_count=0),  # empty
            classify_coverage(engine="c", dispatched=True, status=EngineStatus.RATE_LIMITED, result_count=0),  # failed
            # unavailable
            classify_coverage(
                engine="d", dispatched=True, status=EngineStatus.OK, result_count=0, credential_missing=True
            ),
            classify_coverage(engine="e", dispatched=False),  # not-selected
            classify_coverage(engine="f", dispatched=True, status=EngineStatus.ERROR, result_count=0),  # failed
        ]
        summary = summarize_coverage(coverage)
        assert summary.successful == 1
        assert summary.empty == 1
        assert summary.failed == 2
        assert summary.unavailable == 1
        assert summary.not_selected == 1
        assert summary.attempted == summary.successful + summary.empty + summary.failed + summary.unavailable
        assert summary.attempted == 5

    def test_each_engine_in_exactly_one_bucket(self) -> None:
        coverage = [
            classify_coverage(engine=f"e{i}", dispatched=True, status=status, result_count=0)
            for i, status in enumerate(
                [
                    EngineStatus.OK,
                    EngineStatus.RATE_LIMITED,
                    EngineStatus.BLOCKED,
                    EngineStatus.ERROR,
                    EngineStatus.TIMEOUT,
                ]
            )
        ]
        coverage.append(classify_coverage(engine="excluded", dispatched=False))
        # Every entry carries a single valid, disjoint bucket and every engine
        # is accounted for exactly once (no engine in two buckets).
        for entry in coverage:
            assert entry.bucket in COVERAGE_BUCKETS
        summary = summarize_coverage(coverage)
        total_bucketed = (
            summary.successful + summary.empty + summary.failed + summary.unavailable + summary.not_selected
        )
        assert total_bucketed == len(coverage)
        # ok+0 results is empty; the four non-ok statuses are failed.
        assert summary.empty == 1
        assert summary.failed == 4
        assert summary.not_selected == 1

    def test_empty_coverage_summary(self) -> None:
        summary = summarize_coverage([])
        assert summary.attempted == 0
        assert summary.successful == 0 and summary.empty == 0 and summary.failed == 0
        assert summary.unavailable == 0 and summary.not_selected == 0


# ---------------------------------------------------------------------------
# VAL-RESEARCH-004: get_job exposes per-query state + per-engine coverage
# ---------------------------------------------------------------------------


class TestGetJobCoverage:
    async def _run_and_get(self, state: McpState, job: ResearchJob, store: _FakeStore) -> dict[str, Any]:
        await state.runner._jobs.save(job)
        await state.runner._run_job(job.job_id)
        set_state(state)
        return await t.slopsearx_get_job(job.job_id)

    async def test_get_job_exposes_per_query_state_and_per_engine_coverage(self) -> None:
        state, store = _build_state()
        job = _make_job(
            state,
            queries=[
                ResearchQuery(index=0, query="q1", intent="web", engines=["wikipedia"]),
            ],
        )
        result = await self._run_and_get(state, job, store)
        assert result["progress"]["completed"] == 1
        query = result["queries"][0]
        assert query["index"] == 0
        assert query["state"] == "done"
        assert query["query_id"] and query["query_id"].startswith("ssx-")
        assert query["cursor"] is not None
        # per-engine coverage entry with the pinned fields
        assert query["engine_coverage"], "expected at least one coverage entry"
        entry = query["engine_coverage"][0]
        assert set(entry) >= {"engine", "status", "result_count", "failure_class"}
        assert entry["engine"] == "wikipedia"
        assert entry["status"] == "ok"
        assert entry["bucket"] == "successful"
        assert entry["failure_class"] == "ok"

    async def test_get_job_coverage_summary_present(self) -> None:
        state, store = _build_state()
        job = _make_job(
            state,
            queries=[
                ResearchQuery(index=0, query="q1", intent="web", engines=["wikipedia", "arxiv"]),
            ],
        )
        result = await self._run_and_get(state, job, store)
        cov = result["queries"][0]["coverage"]
        assert cov["attempted"] == cov["successful"] + cov["empty"] + cov["failed"] + cov["unavailable"]
        assert cov["attempted"] >= 2


# ---------------------------------------------------------------------------
# VAL-RESEARCH-007: subquery state distinguishes successful/empty/failed
# ---------------------------------------------------------------------------


class TestSubqueryState:
    async def _run_job(self, state: McpState, queries: list[ResearchQuery], store: _FakeStore) -> ResearchJob:
        job = _make_job(state, queries)
        await state.runner._jobs.save(job)
        await state.runner._run_job(job.job_id)
        # _run_job mutates a fresh copy loaded from the store; reload it.
        loaded = await state.runner._jobs.load(job.job_id)
        assert loaded is not None
        return loaded

    async def test_successful_subquery(self) -> None:
        state, store = _build_state()
        job = await self._run_job(
            state, [ResearchQuery(index=0, query="q", intent="web", engines=["wikipedia"])], store
        )
        assert job.queries[0].state == "done"
        assert job.queries[0].result_count > 0
        assert job.queries[0].error is None

    async def test_empty_subquery_distinct_from_failed(self) -> None:
        state, store = _build_state()
        # arxiv responds ok but returns zero results -> empty, never failed.
        state.ctx.active_engines["arxiv"] = _MockEngine("arxiv", count=0)
        job = await self._run_job(state, [ResearchQuery(index=0, query="q", intent="web", engines=["arxiv"])], store)
        q = job.queries[0]
        assert q.state == "done"
        assert q.result_count == 0
        assert q.error is None
        assert q.engine_coverage[0].bucket == "empty"

    async def test_failed_subquery(self) -> None:
        state, store = _build_state()
        state.ctx.active_engines["stackexchange"] = _MockEngine("stackexchange", status=EngineStatus.ERROR)
        job = await self._run_job(
            state, [ResearchQuery(index=0, query="q", intent="web", engines=["stackexchange"])], store
        )
        q = job.queries[0]
        assert q.state == "failed"
        assert q.error is not None
        assert q.engine_coverage[0].bucket == "failed"
        assert q.engine_coverage[0].failure_class == "error"

    async def test_mixed_empty_and_failed_is_never_clean_empty(self) -> None:
        state, store = _build_state()
        state.ctx.active_engines["wikipedia"] = _MockEngine("wikipedia", count=0)  # empty
        state.ctx.active_engines["arxiv"] = _MockEngine("arxiv", status=EngineStatus.TIMEOUT)  # failed
        job = await self._run_job(
            state,
            [ResearchQuery(index=0, query="q", intent="web", engines=["wikipedia", "arxiv"])],
            store,
        )
        q = job.queries[0]
        assert q.state == "failed"
        assert q.error is not None
        buckets = {c.bucket for c in q.engine_coverage}
        assert "empty" in buckets
        assert "failed" in buckets

    async def test_unavailable_engine_reported_via_auth_required(self) -> None:
        state, store = _build_state()
        # brave is credential-required and not configured in this environment.
        job = await self._run_job(state, [ResearchQuery(index=0, query="q", intent="web", engines=["brave"])], store)
        q = job.queries[0]
        entry = next(c for c in q.engine_coverage if c.engine == "brave")
        assert entry.bucket == "unavailable"
        assert entry.failure_class == "auth_required"


class TestJobStorePersistence:
    async def test_engine_coverage_round_trips_through_store(self) -> None:
        state, store = _build_state()
        cov = [
            classify_coverage(engine="wikipedia", dispatched=True, status=EngineStatus.OK, result_count=2),
            classify_coverage(engine="arxiv", dispatched=True, status=EngineStatus.TIMEOUT, result_count=0),
        ]
        job = _make_job(
            state,
            queries=[
                ResearchQuery(
                    index=0,
                    query="q",
                    intent="web",
                    engines=["wikipedia", "arxiv"],
                    state="done",
                    result_count=2,
                    engine_coverage=cov,
                )
            ],
        )
        await state.runner._jobs.save(job)
        loaded = await state.runner._jobs.load(job.job_id)
        assert loaded is not None
        restored = loaded.queries[0].engine_coverage
        assert [(c.engine, c.bucket, c.failure_class) for c in restored] == [
            ("wikipedia", "successful", "ok"),
            ("arxiv", "failed", "timeout"),
        ]
        assert restored[0].result_count == 2


# ---------------------------------------------------------------------------
# VAL-CROSS-006: research -> get_job -> subquery snapshot cursor -> expand
# ---------------------------------------------------------------------------


class TestResearchToExpansion:
    async def test_subquery_snapshot_and_expansion(self) -> None:
        state, store = _build_state()
        job = _make_job(state, [ResearchQuery(index=0, query="subquery text", intent="web", engines=["wikipedia"])])
        await state.runner._jobs.save(job)
        await state.runner._run_job(job.job_id)

        set_state(state)
        got = await t.slopsearx_get_job(job.job_id)
        query = got["queries"][0]
        assert query["cursor"] is not None
        assert query["query_id"].startswith("ssx-")
        assert query["query"] == "subquery text"

        # Read the subquery's snapshot via its cursor.
        page = await t.slopsearx_read_results(query["cursor"])
        assert page["query"] == "subquery text"
        assert page["meta"]["query_id"] == query["query_id"]
        assert page["results"], "expected captured results in the subquery snapshot"

        # Expand one result and confirm provenance references the subquery.
        card = page["results"][0]
        record = await t.slopsearx_read_result(card["result_id"])
        assert record["result_id"] == card["result_id"]
        assert record["provenance"]["query"] == "subquery text"
        assert record["provenance"]["query_id"] == query["query_id"]
        assert record["snapshot"]["cursor"] == query["cursor"]
