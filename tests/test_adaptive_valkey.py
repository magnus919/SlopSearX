"""Adaptive research lease races and recovery against isolated real Valkey."""

import asyncio
import os

import pytest

from slopsearx.mcp import tools as t
from slopsearx.mcp.state import set_state
from slopsearx.research import LeaseLostError, ResearchJobRunner, ResearchJobStore
from slopsearx.snapshot import SnapshotStore
from tests.test_adaptive_research import start
from tests.test_research_queue_integration import backend  # noqa: F401 — isolated Valkey fixture
from tests.test_research_retry_followup import _build_state

pytestmark = pytest.mark.skipif(not os.environ.get("SLOPSEARX_TEST_VALKEY_URL"), reason="explicit test Valkey required")


@pytest.fixture
def state(backend):  # noqa: F811 — shared pytest fixture
    value = _build_state(["wikipedia"])
    value.job_store = ResearchJobStore(backend)
    value.snapshots = SnapshotStore(backend)
    value.runner = ResearchJobRunner(value.service, value.job_store, value.snapshots, value.catalog, value.policy)
    set_state(value)
    yield value
    set_state(None)


async def test_completed_job_race_reserves_last_slot_once(state):
    first = await start(state, max_queries=2, max_attempts=2)
    entered, release = asyncio.Event(), asyncio.Event()
    original = state.service.search

    async def blocked(request):
        entered.set()
        await release.wait()
        return await original(request)

    state.service.search = blocked
    task = asyncio.create_task(t.slopsearx_extend_research(first["job_id"], "follow-up", engines=["wikipedia"]))
    await asyncio.wait_for(entered.wait(), 5)
    first_runner = state.runner
    state.runner = ResearchJobRunner(state.service, state.job_store, state.snapshots, state.catalog, state.policy)
    try:
        competing = await t.slopsearx_extend_research(first["job_id"], "other", engines=["wikipedia"])
        assert competing["state"] == "running"
    finally:
        state.runner = first_runner
        release.set()
    finished = await task
    assert len(finished["queries"]) == 2
    assert finished["budgets"]["used"]["attempts"] == 2
    assert finished["queries"][0]["attempts"] == first["queries"][0]["attempts"]
    assert (await t.slopsearx_extend_research(first["job_id"], "third", engines=["wikipedia"]))["error"][
        "code"
    ] == "job_budget_exceeded"


@pytest.mark.parametrize("max_attempts", [1, 2])
async def test_recovery_charges_uncertain_attempt_and_fences_same_owner_token(
    state,
    backend,  # noqa: F811 — shared pytest fixture
    max_attempts,
):
    created = await t.slopsearx_start_research(
        "q",
        initial_plan=[{"query": "q", "engines": ["wikipedia"]}],
        max_attempts=max_attempts,
    )
    job = await state.job_store.load(created["job_id"])
    entered, release = asyncio.Event(), asyncio.Event()
    original = state.service.search
    calls = 0

    async def interrupted(request):
        nonlocal calls
        calls += 1
        response = await original(request)
        if calls == 1:
            entered.set()
            await release.wait()
        return response

    state.service.search = interrupted
    old = asyncio.create_task(state.runner.run_direct(job))
    await asyncio.wait_for(entered.wait(), 5)
    reserved = await state.job_store.load(job.job_id)
    old_attempt = reserved.queries[0].attempts[0].attempt_id
    assert reserved.budget_used["attempts"] == 1
    # Simulate lease expiry; same process owner ID must not defeat token fencing.
    await backend._client.delete(state.job_store._lease_key(job.job_id))
    replacement = ResearchJobRunner(
        state.service,
        state.job_store,
        state.snapshots,
        state.catalog,
        state.policy,
        owner_id=state.runner.worker_id,
    )
    recovered = await replacement.run_direct(reserved)
    evidence = [a.attempt_id for a in recovered.queries[0].attempts]
    assert evidence[0] == old_attempt
    assert recovered.queries[0].attempts[0].state == "interrupted"
    assert len(evidence) == max_attempts
    assert recovered.budget_used["attempts"] == max_attempts
    assert calls == max_attempts
    release.set()
    with pytest.raises(LeaseLostError):
        await old
    final = await state.job_store.load(job.job_id)
    assert final.queries[0].attempts == recovered.queries[0].attempts
    if max_attempts == 1:
        assert final.stop_reason == "attempt_budget_exhausted"


@pytest.mark.parametrize("competitor", ["same_key", "retry", "complete", "cancel"])
async def test_extension_serializes_with_lifecycle_requests(state, competitor):
    state.ctx.active_engines["wikipedia"]._count = 0
    first = await start(state, max_queries=3, max_attempts=3)
    entered, release = asyncio.Event(), asyncio.Event()
    original = state.service.search

    async def blocked(request):
        entered.set()
        await release.wait()
        return await original(request)

    state.service.search = blocked
    arguments = dict(job_id=first["job_id"], query="follow-up", engines=["wikipedia"], continuation_key="same")
    work = asyncio.create_task(t.slopsearx_extend_research(**arguments))
    await asyncio.wait_for(entered.wait(), 5)
    owner = state.runner
    state.runner = ResearchJobRunner(state.service, state.job_store, state.snapshots, state.catalog, state.policy)
    try:
        if competitor == "same_key":
            competing = await t.slopsearx_extend_research(**arguments)
        elif competitor == "retry":
            competing = await t.slopsearx_retry_research(first["job_id"])
        elif competitor == "complete":
            competing = await t.slopsearx_update_research(first["job_id"], {}, complete=True)
        else:
            competing = await t.slopsearx_cancel_job(first["job_id"])
        if competitor == "complete":
            assert competing["error"]["code"] == "job_busy"
        else:
            assert competing["state"] == "running"
    finally:
        state.runner = owner
        release.set()
    await work
    final = await state.job_store.load(first["job_id"])
    assert len(final.queries) == 2
    assert final.budget_used["attempts"] == 2
    assert len(final.queries[1].attempts) == 1
    assert final.queries[1].attempts[0].state == "done"
    assert final.queries[0].attempts[0].attempt_id == first["queries"][0]["attempts"][0]["attempt_id"]
    assert final.caller_completed is False
    assert final.state == ("cancelled" if competitor == "cancel" else "succeeded")
    if competitor == "same_key":
        replay = await t.slopsearx_extend_research(**arguments)
        assert len(replay["queries"]) == 2
        assert replay["budgets"]["used"]["attempts"] == 2
