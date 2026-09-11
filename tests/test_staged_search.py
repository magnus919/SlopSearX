"""Contract tests for the staged MCP search workflow."""

from __future__ import annotations

import asyncio
import copy
import json

import pytest

from slopsearx import staged as staged_mod
from slopsearx.capabilities import MCPPolicy
from slopsearx.mcp import staged_tools
from slopsearx.mcp.state import set_state
from slopsearx.staged import StagedSearchRunner, StagedSearchStore
from tests.test_mcp_tools import _build_state, _FakeStore, _MockEngine


@pytest.fixture
def staged_state():
    policy = MCPPolicy()
    policy.enabled_tools["staged_search"] = True
    state = _build_state(["wikipedia", "brave"], policy=policy)
    shared = _FakeStore()
    state.ctx.cache = shared
    state.snapshots._store = shared
    store = StagedSearchStore(shared)
    runner = StagedSearchRunner(state.service, store, state.snapshots, staged_tools._policy_check)
    state.staged_store = store
    state.staged_runner = runner
    set_state(state)
    yield state
    set_state(None)


async def test_preview_has_no_dispatch_and_stable_digest(staged_state) -> None:
    arguments = {
        "query": "durable evidence",
        "objectives": {"deadline_ms": 5000, "max_engine_calls": 2},
        "initial_scope": {"engines": ["wikipedia"]},
    }
    first = await staged_tools.slopsearx_preview_staged_search(**arguments)
    second = await staged_tools.slopsearx_preview_staged_search(**arguments)
    assert first["dispatch"] is False
    assert first["plan_digest"] == second["plan_digest"]
    assert staged_state.ctx.active_engines["wikipedia"].calls == 0


async def test_nonempty_initial_stage_skips_fallback_and_replays(staged_state) -> None:
    arguments = {
        "query": "durable evidence",
        "objectives": {"deadline_ms": 5000, "max_engine_calls": 2},
        "initial_scope": {"engines": ["wikipedia"]},
        "fallback_scope": {"engines": ["brave"]},
        "allow_scope_expansion": True,
        "idempotency_key": "request-1",
    }
    accepted = await staged_tools.slopsearx_search_staged(**arguments)
    await staged_state.staged_runner.run_one("default", accepted["operation_id"])
    completed = await staged_tools.slopsearx_get_staged_search(accepted["operation_id"])
    replayed = await staged_tools.slopsearx_search_staged(**arguments)
    assert completed["state"] == "completed"
    assert completed["artifact"]["kind"] == "staged_search"
    assert completed["meta"]["artifact"]["kind"] == "snapshot"
    assert completed["stop_reason"] == "initial_nonempty"
    assert completed["budget"]["reserved"] == 1
    assert completed["stages"][1]["state"] == "skipped"
    assert completed["results"]
    assert replayed["operation_id"] == accepted["operation_id"]
    assert staged_state.ctx.active_engines["wikipedia"].calls == 1
    assert staged_state.ctx.active_engines["brave"].calls == 0


async def test_clean_empty_runs_disjoint_fallback(staged_state) -> None:
    staged_state.ctx.active_engines["wikipedia"] = _MockEngine("wikipedia", count=0)
    arguments = {
        "query": "fallback evidence",
        "objectives": {"deadline_ms": 5000, "max_engine_calls": 2},
        "initial_scope": {"engines": ["wikipedia"]},
        "fallback_scope": {"engines": ["brave"]},
        "allow_scope_expansion": True,
        "idempotency_key": "request-2",
    }
    accepted = await staged_tools.slopsearx_search_staged(**arguments)
    await staged_state.staged_runner.run_one("default", accepted["operation_id"])
    await staged_state.staged_runner.run_one("default", accepted["operation_id"])
    completed = await staged_tools.slopsearx_get_staged_search(accepted["operation_id"])
    assert completed["stop_reason"] == "fallback_nonempty"
    assert completed["budget"]["reserved"] == 2
    assert completed["stages"][0]["attempts"][0]["result_count"] == 0
    assert completed["stages"][1]["attempts"][0]["result_count"] > 0


async def test_cached_empty_initial_stage_does_not_authorize_fallback(staged_state) -> None:
    staged_state.ctx.active_engines["wikipedia"] = _MockEngine("wikipedia", count=0)
    warm = await staged_tools.slopsearx_search_staged(
        "cached empty evidence",
        {"deadline_ms": 5000, "max_engine_calls": 1},
        {"engines": ["wikipedia"]},
        "cache-warm",
    )
    await staged_state.staged_runner.run_one("default", warm["operation_id"])

    accepted = await staged_tools.slopsearx_search_staged(
        "cached empty evidence",
        {"deadline_ms": 5000, "max_engine_calls": 2},
        {"engines": ["wikipedia"]},
        "cache-reuse",
        fallback_scope={"engines": ["brave"]},
        allow_scope_expansion=True,
    )
    await staged_state.staged_runner.run_one("default", accepted["operation_id"])
    completed = await staged_tools.slopsearx_get_staged_search(accepted["operation_id"])

    assert completed["state"] == "failed"
    assert completed["stop_reason"] == "execution_failed"
    assert completed["stages"][0]["attempts"][0]["cached"] is True
    assert completed["stages"][1]["state"] == "pending"
    assert completed["objectives"]["unmet"] == [{"objective": "fallback", "reason": "initial_stage_not_clean"}]
    assert staged_state.ctx.active_engines["wikipedia"].calls == 1
    assert staged_state.ctx.active_engines["brave"].calls == 0


async def test_validation_rejects_overlap_and_strict_safesearch(staged_state) -> None:
    overlap = await staged_tools.slopsearx_preview_staged_search(
        "query",
        {"deadline_ms": 1000, "max_engine_calls": 2},
        {"engines": ["wikipedia"]},
        {"engines": ["wikipedia"]},
        True,
    )
    strict = await staged_tools.slopsearx_preview_staged_search(
        "query",
        {"deadline_ms": 1000, "max_engine_calls": 1},
        {"engines": ["wikipedia"]},
        filters={"safesearch": "strict"},
    )
    assert overlap["error"]["code"] == "objective_conflict"
    assert strict["error"]["code"] == "policy_rejected"


async def test_background_queue_executes_once(staged_state) -> None:
    task = asyncio.create_task(staged_state.staged_runner.run_forever())
    try:
        accepted = await staged_tools.slopsearx_search_staged(
            "queued",
            {"deadline_ms": 5000, "max_engine_calls": 1},
            {"engines": ["wikipedia"]},
            "request-3",
        )
        await asyncio.wait_for(staged_state.staged_runner._queue.join(), timeout=1)
        completed = await staged_tools.slopsearx_get_staged_search(accepted["operation_id"])
        assert completed["state"] == "completed"
        assert staged_state.ctx.active_engines["wikipedia"].calls == 1
    finally:
        task.cancel()
        with pytest.raises(asyncio.CancelledError):
            await task


async def test_retry_preserves_unknown_aggregate_accounting(staged_state) -> None:
    accepted = await staged_tools.slopsearx_search_staged(
        "retry",
        {"deadline_ms": 5000, "max_engine_calls": 2},
        {"engines": ["wikipedia"]},
        "request-retry",
    )
    read = await staged_state.staged_store.read("default", accepted["operation_id"])
    record = read.record
    assert record is not None
    record["state"] = "interrupted"
    record["stop_reason"] = "interrupted"
    record["budget"]["reserved"] = 1
    record["budget"]["observed"] = None
    record["stages"][0]["state"] = "interrupted"
    await staged_state.staged_store.save("default", record)
    retried = await staged_tools.slopsearx_retry_staged_search(accepted["operation_id"], "retry-1")
    assert retried["state"] == "queued"
    await staged_state.staged_runner.run_one("default", accepted["operation_id"])
    completed = await staged_tools.slopsearx_get_staged_search(accepted["operation_id"])
    assert completed["state"] == "completed"
    assert completed["budget"]["reserved"] == 2
    assert completed["budget"]["observed"] is None


async def test_orphan_recovery_records_interrupted_outcome(staged_state, monkeypatch: pytest.MonkeyPatch) -> None:
    transitions: list[tuple[str | None, str]] = []
    terminal: list[str] = []

    class ExpiredLeaseClient:
        def __init__(self, record_key: str) -> None:
            self.record_key = record_key

        async def zrangebyscore(self, *_args, **_kwargs):
            return [self.record_key]

        async def eval(self, *_args):
            return 1

        async def zrem(self, *_args):
            return 1

    record_key = staged_state.staged_store._key("default", "op-recovered")
    staged_state.ctx.cache._data[record_key] = {
        "tenant": "default",
        "operation_id": "op-recovered",
        "expires_at": 9999999999.0,
    }
    client = ExpiredLeaseClient(record_key)
    monkeypatch.setattr(staged_state.staged_store, "_client", lambda: client)
    monkeypatch.setattr(
        staged_mod.m,
        "transition_workflow",
        lambda _workflow, previous, current: transitions.append((previous, current)),
    )
    monkeypatch.setattr(
        staged_mod.m,
        "record_workflow_terminal",
        lambda _workflow, outcome, *_args: terminal.append(outcome),
    )

    assert await staged_state.staged_store.recover_orphans() == 1
    assert transitions == [("running", "interrupted")]
    assert terminal == ["interrupted"]


@pytest.mark.parametrize("status", ["deadline", "budget"])
async def test_valkey_claim_terminal_status_balances_queued_metrics(
    staged_state, monkeypatch: pytest.MonkeyPatch, status: str
) -> None:
    accepted = await staged_tools.slopsearx_search_staged(
        "terminal claim",
        {"deadline_ms": 5000, "max_engine_calls": 1},
        {"engines": ["wikipedia"]},
        f"terminal-claim-{status}",
    )
    read = await staged_state.staged_store.read("default", accepted["operation_id"])
    assert read.record is not None
    terminal_record = copy.deepcopy(read.record)
    terminal_record.update(
        state="failed", stop_reason="deadline_expired" if status == "deadline" else "budget_exhausted"
    )

    class TerminalClaimClient:
        async def eval(self, *_args):
            return [status, json.dumps(terminal_record)]

    transitions: list[tuple[str | None, str]] = []
    terminal: list[str] = []
    expiries: list[str] = []
    rejections: list[str] = []
    monkeypatch.setattr(staged_state.staged_store, "_client", lambda: TerminalClaimClient())
    monkeypatch.setattr(
        staged_mod.m,
        "transition_workflow",
        lambda _workflow, previous, current: transitions.append((previous, current)),
    )
    monkeypatch.setattr(
        staged_mod.m,
        "record_workflow_terminal",
        lambda _workflow, outcome, *_args: terminal.append(outcome),
    )
    monkeypatch.setattr(
        staged_mod.m,
        "record_workflow_expiry",
        lambda _workflow, artifact, *_args: expiries.append(artifact),
    )
    monkeypatch.setattr(
        staged_mod.m,
        "record_workflow_rejection",
        lambda _workflow, reason, *_args: rejections.append(reason),
    )

    assert await staged_state.staged_store.claim("default", accepted["operation_id"]) is None
    assert transitions == [("queued", "failed")]
    assert terminal == ["failed"]
    assert expiries == (["operation"] if status == "deadline" else [])
    assert rejections == (["budget"] if status == "budget" else [])


async def test_absolute_deadline_terminalizes_claimed_attempt(staged_state) -> None:
    accepted = await staged_tools.slopsearx_search_staged(
        "deadline",
        {"deadline_ms": 1, "max_engine_calls": 1},
        {"engines": ["wikipedia"]},
        "request-deadline",
    )
    await asyncio.sleep(0.01)
    await staged_state.staged_runner.run_one("default", accepted["operation_id"])
    failed = await staged_tools.slopsearx_get_staged_search(accepted["operation_id"])
    assert failed["state"] == "failed"
    assert failed["stop_reason"] == "deadline_expired"
    assert failed["stages"][0]["attempts"][0]["state"] == "failed"
    assert failed["stages"][0]["attempts"][0]["observed_engine_calls"] == 0
    assert {item["objective"] for item in failed["objectives"]["unmet"]} == {"deadline_ms"}


async def test_deadline_after_renewal_records_terminal_metrics(staged_state, monkeypatch: pytest.MonkeyPatch) -> None:
    accepted = await staged_tools.slopsearx_search_staged(
        "deadline after renewal",
        {"deadline_ms": 5000, "max_engine_calls": 1},
        {"engines": ["wikipedia"]},
        "request-deadline-renewal",
    )
    read = await staged_state.staged_store.read("default", accepted["operation_id"])
    assert read.record is not None
    clock = [float(read.record["execution_deadline_at"]) - 1]
    monkeypatch.setattr(staged_mod.time, "time", lambda: clock[0])
    original_renew = staged_state.staged_store.renew

    async def renew_then_expire(*args):
        renewed = await original_renew(*args)
        clock[0] += 2
        return renewed

    terminal: list[str] = []
    monkeypatch.setattr(staged_state.staged_store, "renew", renew_then_expire)
    monkeypatch.setattr(
        staged_mod.m,
        "record_workflow_terminal",
        lambda _workflow, outcome, *_args: terminal.append(outcome),
    )

    await staged_state.staged_runner.run_one("default", accepted["operation_id"])

    assert terminal == ["failed"]


async def test_policy_revocation_hides_retained_results_and_replay(staged_state) -> None:
    arguments = {
        "query": "revoke",
        "objectives": {"deadline_ms": 5000, "max_engine_calls": 1},
        "initial_scope": {"engines": ["wikipedia"]},
        "idempotency_key": "request-revoke",
    }
    accepted = await staged_tools.slopsearx_search_staged(**arguments)
    await staged_state.staged_runner.run_one("default", accepted["operation_id"])
    staged_state.policy.sensitive_engines.add("wikipedia")
    staged_state.policy.targeted_sensitive_allowed = False
    read = await staged_tools.slopsearx_get_staged_search(accepted["operation_id"])
    replay = await staged_tools.slopsearx_search_staged(**arguments)
    assert read["error"]["code"] == "policy_rejected"
    assert replay["error"]["code"] == "policy_rejected"
    assert "results" not in read and "results" not in replay
