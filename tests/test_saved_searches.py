"""Saved-search CRUD, scheduling, policy and coverage behavior."""

from __future__ import annotations

import pytest

from slopsearx.mcp import tools as t
from slopsearx.mcp.state import set_state, tenant_scope
from slopsearx.saved_runner import SavedSearchRunner
from slopsearx.saved_store import SavedSearchStore
from slopsearx.service import SearchRequest
from tests.test_research_retry_followup import _build_state


@pytest.fixture
def saved_state(monkeypatch):
    now = [1000.0]
    monkeypatch.setattr(t.time, "time", lambda: now[0])
    state = _build_state(["wikipedia", "brave"])
    state.policy.enabled_tools["saved_searches"] = True
    store = SavedSearchStore(state.ctx.cache)
    runner = SavedSearchRunner(
        state.service,
        store,
        state.policy,
        policy_check=lambda definition: t._saved_policy_error(state, definition),
        policy_fingerprint=lambda definition: t._saved_policy_fingerprint(state, definition),
        clock=lambda: now[0],
    )
    state.saved_store = store
    state.saved_runner = runner
    set_state(state)
    yield state, now
    set_state(None)


async def create() -> dict:
    value = await t.slopsearx_create_saved_search("release changes", ["wikipedia"], 60, expires_in_seconds=3600)
    assert "error" not in value, value
    return value


async def test_crud_schedule_and_coverage_aware_events(saved_state):
    state, now = saved_state
    created = await create()
    assert created["revision"] == 1
    assert created["comparison"]["absence_is_deletion"] is False
    assert await state.saved_runner.run_due("default", now=1059) == []
    now[0] = 1060
    baseline = await state.saved_runner.run_due("default")
    assert baseline[0]["status"] == "baseline_initialized"
    assert baseline[0]["observation"]["cached"] is False
    state.ctx.active_engines["wikipedia"]._count = 4
    now[0] = 1120
    changed = await state.saved_runner.run_due("default")
    assert {event["kind"] for event in changed[0]["events"]} == {"added"}
    state.ctx.active_engines["wikipedia"]._count = 0
    now[0] = 1180
    absent = await state.saved_runner.run_due("default")
    assert {event["kind"] for event in absent[0]["events"]} == {"not_observed_in_latest_run"}
    reports = await t.slopsearx_read_saved_search_reports(created["search_id"])
    assert len(reports["reports"]) == 3
    assert "never a deletion" in reports["note"]

    paused = await t.slopsearx_pause_saved_search(created["search_id"], 1)
    assert paused["paused"] is True
    assert paused["revision"] == 2
    assert await state.saved_runner.run_due("default", now=2000) == []
    stale = await t.slopsearx_update_saved_search(created["search_id"], 1, query="stale")
    assert stale["error"]["code"] == "revision_conflict"
    resumed = await t.slopsearx_pause_saved_search(created["search_id"], 2, paused=False)
    assert resumed["revision"] == 3
    assert resumed["latest_run_id"] is None
    deleted = await t.slopsearx_delete_saved_search(created["search_id"], 3)
    assert deleted["state"] == "deleted"
    assert (await t.slopsearx_get_saved_search(created["search_id"]))["error"]["code"] == "invalid_search_id"


@pytest.mark.parametrize("revision", [0, -1])
async def test_pause_rejects_non_positive_revision(saved_state, revision):
    created = await create()
    outcome = await t.slopsearx_pause_saved_search(created["search_id"], revision)
    assert outcome["error"]["code"] == "invalid_input"


async def test_scope_update_resets_baseline_and_enforces_policy(saved_state):
    state, now = saved_state
    created = await create()
    now[0] = 1060
    await state.saved_runner.run_due("default")
    updated = await t.slopsearx_update_saved_search(created["search_id"], 1, engines=["brave"])
    assert updated["revision"] == 2
    assert updated["latest_run_id"] is None
    state.policy.sensitive_engines.add("brave")
    calls = state.ctx.active_engines["brave"].calls
    now[0] = updated["next_due"]
    assert await state.saved_runner.run_due("default") == []
    assert state.ctx.active_engines["brave"].calls == calls


async def test_tenant_isolation_and_grant(saved_state):
    state, _now = saved_state
    created = await create()
    with tenant_scope("other"):
        assert (await t.slopsearx_get_saved_search(created["search_id"]))["error"]["code"] == "invalid_search_id"
        assert (await t.slopsearx_read_saved_search_reports(created["search_id"]))["error"][
            "code"
        ] == "invalid_search_id"
    state.policy.enabled_tools["saved_searches"] = False
    assert (await t.slopsearx_get_saved_search(created["search_id"]))["error"]["code"] == "tool_disabled"


@pytest.mark.parametrize(
    "arguments",
    [
        {"interval_seconds": True},
        {"interval_seconds": 59},
        {"interval_seconds": 86401},
        {"interval_seconds": 60, "retention_seconds": False},
        {"interval_seconds": 60, "max_results": 0},
        {"interval_seconds": 60, "max_reports": 101},
    ],
)
async def test_strict_bounded_inputs_are_atomic(saved_state, arguments):
    state, _now = saved_state
    before = dict(state.ctx.cache._data)
    value = await t.slopsearx_create_saved_search("q", ["wikipedia"], **arguments)
    assert "error" in value
    assert state.ctx.cache._data == before


async def test_mixed_sensitive_scope_is_atomic(saved_state):
    state, _now = saved_state
    state.policy.sensitive_engines.add("brave")
    before = dict(state.ctx.cache._data)
    value = await t.slopsearx_create_saved_search("q", ["wikipedia", "brave"], 60)
    assert value["error"]["code"] == "tool_disabled"
    assert state.ctx.cache._data == before


async def test_scheduled_run_bypasses_canonical_cache(saved_state):
    state, now = saved_state
    cached = await state.service.search(SearchRequest(query="release changes", engines=["wikipedia"]))
    assert len(cached.results) == 3
    state.ctx.active_engines["wikipedia"]._count = 1
    await create()
    now[0] = 1060
    report = (await state.saved_runner.run_due("default"))[0]
    assert report["observation"]["cached"] is False
    assert report["observation"]["discovered_count"] == 1
    assert state.ctx.active_engines["wikipedia"].calls == 2


async def test_midflight_policy_revocation_blocks_evidence_commit(saved_state):
    state, now = saved_state
    created = await create()
    original = state.service.search

    async def revoke(request):
        response = await original(request)
        state.policy.enabled_tools["saved_searches"] = False
        return response

    state.service.search = revoke
    now[0] = 1060
    assert await state.saved_runner.run_due("default") == []
    state.policy.enabled_tools["saved_searches"] = True
    assert (await state.saved_store.load(created["search_id"], now=1060)).baseline is None
    assert await state.saved_store.reports(created["search_id"], now=1060) == []


async def test_current_scope_policy_applies_to_definition_and_report_reads(saved_state):
    state, _now = saved_state
    created = await create()
    state.policy.sensitive_engines.add("wikipedia")
    fetched = await t.slopsearx_get_saved_search(created["search_id"])
    reports = await t.slopsearx_read_saved_search_reports(created["search_id"])
    assert fetched["error"]["code"] == "tool_disabled"
    assert reports["error"]["code"] == "tool_disabled"


async def test_retained_old_scope_report_is_revalidated_after_scope_change(saved_state):
    state, now = saved_state
    state.policy.sensitive_engines.add("wikipedia")
    state.policy.targeted_sensitive_allowed = True
    created = await create()
    now[0] = 1060
    await state.saved_runner.run_due("default")
    updated = await t.slopsearx_update_saved_search(created["search_id"], 1, engines=["brave"])
    assert updated["revision"] == 2
    state.policy.targeted_sensitive_allowed = False
    reports = await t.slopsearx_read_saved_search_reports(created["search_id"])
    assert reports["error"]["code"] == "tool_disabled"


async def test_failed_run_retains_baseline_and_reports_missed_slots(saved_state):
    state, now = saved_state
    created = await create()
    now[0] = 1305
    first = (await state.saved_runner.run_due("default"))[0]
    assert first["missed_slots"] == 4
    baseline = (await state.saved_store.load(created["search_id"], now=1305)).baseline

    async def fail(request):
        raise RuntimeError("upstream failed")

    state.service.search = fail
    now[0] = 1360
    failed = (await state.saved_runner.run_due("default"))[0]
    assert failed["status"] == "incomparable"
    assert failed["incomparable_reasons"] == ["dispatch_failed"]
    assert (await state.saved_store.load(created["search_id"], now=1360)).baseline == baseline


async def test_start_immediately_uses_creation_slot_and_keeps_first_interval(saved_state):
    state, now = saved_state
    created = await t.slopsearx_create_saved_search(
        "release changes", ["wikipedia"], 60, expires_in_seconds=3600, start_immediately=True
    )
    report = await state.saved_runner.run_one("default", created["search_id"], now=1000)
    assert report and report["slot"] == 0
    assert report["scheduled_at"] == 1000
    stored = await state.saved_store.load(created["search_id"], now=1000)
    assert stored and stored.next_due == 1060
