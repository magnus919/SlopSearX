"""Caller-directed plans and conservative, durable continuation accounting."""

import asyncio
import copy

import pytest

from slopsearx.mcp import tools as t
from slopsearx.mcp.state import set_state, tenant_scope
from slopsearx.research import ResearchJobRunner
from slopsearx.research_models import _job_from_payload, _job_to_payload
from tests.test_research_retry_followup import _build_state


@pytest.fixture
def state():
    value = _build_state(["wikipedia"])
    set_state(value)
    yield value
    set_state(None)


async def start(state, **kwargs):
    args = {
        "question": "investigate",
        "max_queries": 3,
        "max_attempts": 3,
        "initial_plan": [{"query": "first", "engines": ["wikipedia"], "subquestion_id": "a", "rationale": "baseline"}],
        "subquestions": [{"id": "a", "question": "maintenance?"}, {"id": "b", "question": "alternatives?"}],
    }
    args.update(kwargs)
    response = await t.slopsearx_start_research(**args)
    assert "error" not in response, response
    job = await state.job_store.load(response["job_id"])
    await state.runner.run_direct(job)
    return await t.slopsearx_get_job(job.job_id)


async def test_plan_followup_progress_completion_and_immutable_history(state):
    first = await start(state)
    prior = copy.deepcopy(first["queries"][0]["attempts"])
    assert first["stop_reason"] == "plan_executed"
    assert first["unresolved_subquestions"] == ["a", "b"]
    response = await t.slopsearx_extend_research(
        first["job_id"],
        "next",
        engines=["wikipedia"],
        subquestion_id="a",
        rationale="follow the finding",
        parent_attempt_id=prior[0]["attempt_id"],
        continuation_key="step2",
    )
    assert "error" not in response, response
    assert response["queries"][0]["attempts"] == prior
    assert response["queries"][1]["parent_attempt_id"] == prior[0]["attempt_id"]
    assert response["budgets"]["used"]["attempts"] == 2
    again = await t.slopsearx_extend_research(
        first["job_id"],
        "next",
        engines=["wikipedia"],
        subquestion_id="a",
        rationale="follow the finding",
        parent_attempt_id=prior[0]["attempt_id"],
        continuation_key="step2",
    )
    assert again["queries"] == response["queries"]
    conflict = await t.slopsearx_extend_research(
        first["job_id"], "different", engines=["wikipedia"], continuation_key="step2"
    )
    assert conflict["error"]["code"] == "idempotency_conflict"
    complete = await t.slopsearx_update_research(first["job_id"], {"a": "resolved"}, complete=True)
    assert complete["unresolved_subquestions"] == ["b"]
    assert complete["stop_reason"] == "caller_completed"
    assert complete["queries"][0]["attempts"] == prior
    replay_after_completion = await t.slopsearx_extend_research(
        first["job_id"],
        "next",
        engines=["wikipedia"],
        subquestion_id="a",
        rationale="follow the finding",
        parent_attempt_id=prior[0]["attempt_id"],
        continuation_key="step2",
    )
    assert replay_after_completion["caller_completed"] is True
    assert replay_after_completion["queries"] == complete["queries"]
    blocked = await t.slopsearx_extend_research(first["job_id"], "another", engines=["wikipedia"])
    assert blocked["error"]["code"] == "invalid_job_state"
    assert (await t.slopsearx_retry_research(first["job_id"]))["state"] == "succeeded"


@pytest.mark.parametrize(
    "field,value",
    [
        ("max_queries", 0),
        ("max_attempts", -1),
        ("max_attempts", True),
        ("max_results", False),
        ("max_engine_attempts", 0),
        ("initial_plan", []),
        ("initial_plan", [{"query": "x", "engines": []}]),
        ("initial_plan", [{"query": "x", "engines": ["wikipedia"], "subquestion_id": "missing"}]),
        ("initial_plan", [{"query": "x", "engines": ["wikipedia"], "parent_attempt_id": "elsewhere"}]),
        ("subquestions", [{"id": "a", "question": "x"}, {"id": "a", "question": "y"}]),
    ],
)
async def test_invalid_plan_is_atomic(state, field, value):
    before = copy.deepcopy(state.job_store._store._data)
    response = await t.slopsearx_start_research("q", **{field: value})
    assert "error" in response
    assert state.job_store._store._data == before
    assert state.ctx.active_engines["wikipedia"].calls == 0


async def test_result_budget_admits_records_without_truncating_snapshot(state):
    first = await start(state, max_results=1)
    attempt = first["queries"][0]["attempts"][0]
    assert len(attempt["admitted_result_ids"]) == 1
    assert attempt["result_count"] == 3
    assert first["stop_reason"] == "result_budget_exhausted"
    flat = await t.slopsearx_read_results(attempt["cursor"])
    assert len(flat["results"]) == 3
    next_query = await t.slopsearx_extend_research(first["job_id"], "next", engines=["wikipedia"])
    assert next_query["error"]["code"] == "job_budget_exceeded"


async def test_lower_attempt_budget_survives_retry(state):
    state.ctx.active_engines["wikipedia"]._count = 0
    first = await start(state, max_attempts=1)
    prior = copy.deepcopy(first["queries"][0]["attempts"])
    retried = await t.slopsearx_retry_research(first["job_id"])
    assert retried["stop_reason"] == "attempt_budget_exhausted"
    assert retried["queries"][0]["attempts"] == prior
    assert state.ctx.active_engines["wikipedia"].calls == 1
    loaded = await state.job_store.load(first["job_id"])
    assert _job_from_payload(_job_to_payload(loaded)) == loaded


async def test_completed_job_continuation_race_holds_lease(state):
    first = await start(state, max_queries=2)
    other = ResearchJobRunner(state.service, state.job_store, state.snapshots, state.catalog, state.policy)
    entered, release = asyncio.Event(), asyncio.Event()
    original = state.service.search

    async def blocked(request):
        entered.set()
        await release.wait()
        return await original(request)

    state.service.search = blocked
    task = asyncio.create_task(t.slopsearx_extend_research(first["job_id"], "next", engines=["wikipedia"]))
    await entered.wait()
    from slopsearx.research import JobStillRunningError

    with pytest.raises(JobStillRunningError):
        await other.run_direct(await state.job_store.load(first["job_id"]))
    second = await t.slopsearx_extend_research(first["job_id"], "competing", engines=["wikipedia"])
    assert second["state"] == "running"
    release.set()
    response = await task
    assert len(response["queries"]) == 2
    assert [q["index"] for q in response["queries"]] == [0, 1]
    assert response["budgets"]["used"]["attempts"] == 2


async def test_revoked_policy_and_tenant_links(state):
    first = await start(state)
    with tenant_scope("other"):
        assert (await t.slopsearx_update_research(first["job_id"], {}))["error"]["code"] == "invalid_job_id"
    dangling = await t.slopsearx_extend_research(
        first["job_id"], "next", engines=["wikipedia"], parent_attempt_id="foreign"
    )
    assert dangling["error"]["code"] == "invalid_input"
    state.policy.sensitive_engines.add("wikipedia")
    rejected = await t.slopsearx_extend_research(first["job_id"], "next", engines=["wikipedia"])
    assert rejected["error"]["code"] == "tool_disabled"


@pytest.mark.parametrize("prior_history", [False, True])
async def test_legacy_running_attempt_is_charged_once_before_recovery(state, prior_history):
    from slopsearx.research_models import ResearchQueryAttempt

    first = await start(state)
    job = await state.job_store.load(first["job_id"])
    job.state = "running"
    query = job.queries[0]
    query.state = "running"
    query.attempts = [ResearchQueryAttempt(state="failed")] if prior_history else []
    job.budget_limits = {}
    job.budget_used = {}
    state.policy.job_max_queries = 1 + int(prior_history)
    await state.job_store.save(job)
    calls = state.ctx.active_engines["wikipedia"].calls
    recovered = await state.runner.run_direct(job)
    assert recovered.budget_used["attempts"] == 1 + int(prior_history)
    assert recovered.queries[0].attempts[-1].state == "interrupted"
    assert recovered.stop_reason == "attempt_budget_exhausted"
    assert state.ctx.active_engines["wikipedia"].calls == calls
    replay = await state.runner.run_direct(recovered)
    assert replay.budget_used == recovered.budget_used


@pytest.mark.parametrize("action", ["cancel", "expire", "complete"])
async def test_orphan_terminalization_preserves_history_and_closes_attempt(state, action):
    import time

    from slopsearx.research_models import ResearchQueryAttempt

    first = await start(state)
    job = await state.job_store.load(first["job_id"])
    original = copy.deepcopy(job.queries[0].attempts)
    job.state = "running"
    query = job.queries[0]
    query.state = "running"
    query.attempts.append(ResearchQueryAttempt(attempt_id="orphan", state="running"))
    job.budget_used["attempts"] += 1
    job.budget_used["engine_attempts"] += 1
    if action == "expire":
        job.deadline = time.time() - 1
    await state.job_store.save(job)
    calls = state.ctx.active_engines["wikipedia"].calls
    if action == "cancel":
        await state.job_store.request_cancel(job.job_id)
    elif action == "expire":
        assert await state.job_store.expire_stale_running() == 1
    else:
        result = await t.slopsearx_update_research(job.job_id, {}, complete=True)
        assert "error" not in result
    final = await state.job_store.load(job.job_id)
    assert final.queries[0].attempts[:-1] == original
    assert final.queries[0].attempts[-1].state == "interrupted"
    assert final.queries[0].attempts[-1].finished_at is not None
    assert final.queries[0].state == "cancelled"
    assert (
        final.stop_reason
        == {"cancel": "cancelled", "expire": "deadline_expired", "complete": "caller_completed"}[action]
    )
    assert final.budget_used == job.budget_used
    assert state.ctx.active_engines["wikipedia"].calls == calls


@pytest.mark.parametrize("revocation", ["research", "sensitive", "science", "legacy_science"])
async def test_current_policy_rechecked_before_retry_dispatch(state, revocation):
    state.ctx.active_engines["wikipedia"]._count = 0
    state.policy.enabled_tools["science"] = True
    first = await start(state)
    job = await state.job_store.load(first["job_id"])
    original = copy.deepcopy(job.queries[0].attempts)
    if revocation in {"science", "legacy_science"}:
        job.queries[0].intent = "science"
        job.queries[0].requires_intent_grant = True
        payload = _job_to_payload(job)
        if revocation == "legacy_science":
            del payload["queries"][0]["requires_intent_grant"]
        await state.job_store.save(_job_from_payload(payload))
        state.policy.enabled_tools["science"] = False
    elif revocation == "research":
        state.policy.enabled_tools["research"] = False
    else:
        state.policy.sensitive_engines.add("wikipedia")
    await t.slopsearx_retry_research(first["job_id"])
    latest = await state.job_store.load(first["job_id"])
    assert state.ctx.active_engines["wikipedia"].calls == 1
    assert latest.queries[0].attempts[:1] == original
    if revocation != "research":
        assert latest.queries[0].attempts[-1].state == "failed"
        assert latest.queries[0].attempts[-1].cursor is None


async def test_unexpected_dispatch_failure_has_fresh_durable_attempt(state):
    state.ctx.active_engines["wikipedia"]._count = 0
    first = await start(state)
    prior = copy.deepcopy(first["queries"][0]["attempts"])

    async def broken(request):
        raise RuntimeError("deterministic dispatch failure")

    state.service.search = broken
    response = await t.slopsearx_retry_research(first["job_id"])
    attempts = response["queries"][0]["attempts"]
    assert attempts[:-1] == prior
    assert attempts[-1]["state"] == "failed"
    assert attempts[-1]["error"] == "deterministic dispatch failure"
    assert attempts[-1]["cursor"] is None
    assert attempts[-1]["finished_at"] is not None
    assert attempts[-1]["engine_coverage"] == []
    assert response["stop_reason"] == "execution_failed"


@pytest.mark.parametrize("stop", ["deadline", "cancel"])
async def test_lifecycle_during_dispatch_retains_evidence_and_stops_next_query(state, stop):

    started = await t.slopsearx_start_research(
        "investigate",
        max_queries=2,
        initial_plan=[{"query": "first", "engines": ["wikipedia"]}, {"query": "second", "engines": ["wikipedia"]}],
    )
    original = state.service.search

    async def dispatch(request):
        response = await original(request)
        if stop == "cancel":
            await state.job_store.request_cancel(started["job_id"])
        else:
            current = await state.job_store.load(started["job_id"])
            # Simulated clock crossing applies to the in-flight worker copy too.
            monkeypatch_time.setattr("slopsearx.research.time.time", lambda: current.deadline + 1)
        return response

    with pytest.MonkeyPatch.context() as monkeypatch_time:
        state.service.search = dispatch
        response = await state.runner.run_direct(await state.job_store.load(started["job_id"]))
    assert response.stop_reason == ("cancelled" if stop == "cancel" else "deadline_expired")
    assert response.queries[0].attempts[0].cursor is not None
    assert response.queries[0].attempts[0].state == "done"
    assert response.queries[1].attempts == []
    assert state.ctx.active_engines["wikipedia"].calls == 1
