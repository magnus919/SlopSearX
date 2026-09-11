"""Saved-search event contract, delivery, policy and bounded-state behavior."""

from __future__ import annotations

import copy
import inspect

import pytest

from slopsearx.mcp import tools as t
from slopsearx.mcp.state import set_state, tenant_scope
from slopsearx.saved_events import EVENT_CONTRACT, event_for_report
from slopsearx.saved_models import SavedDefinition
from slopsearx.saved_runner import SavedSearchRunner
from slopsearx.saved_store import SavedSearchStore
from tests.test_research_retry_followup import _build_state


@pytest.fixture
def event_state(monkeypatch: pytest.MonkeyPatch):
    now = [1000.0]
    monkeypatch.setattr(t.time, "time", lambda: now[0])
    state = _build_state(["wikipedia", "brave"])
    state.policy.enabled_tools["saved_searches"] = True
    state.policy.enabled_tools["saved_search_events"] = True
    store = SavedSearchStore(state.ctx.cache, event_capacity=3, event_retention_seconds=120, event_consumers=2)
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


async def _create() -> dict:
    value = await t.slopsearx_create_saved_search(
        "release changes", ["wikipedia"], 60, expires_in_seconds=600, retention_seconds=600
    )
    assert "error" not in value
    return value


async def _run(state, now: list[float], instant: float) -> dict:
    now[0] = instant
    value = await state.saved_runner.run_due("default")
    assert len(value) == 1
    return value[0]


async def test_read_is_ordered_bounded_and_does_not_ack(event_state):
    state, now = event_state
    await _create()
    await _run(state, now, 1060)
    state.ctx.active_engines["wikipedia"]._count = 4
    await _run(state, now, 1120)

    first = await t.slopsearx_read_saved_search_events("agent-a", limit=1)
    assert first["events"][0]["contract"] == EVENT_CONTRACT
    assert first["events"][0]["saved_search"]["kind"] == "saved_search"
    assert first["events"][0]["run"]["kind"] == "saved_report"
    assert first["returned"] == 1
    assert first["acknowledged_cursor"] == "0-0"
    assert first["next_cursor"] == first["events"][0]["cursor"]
    repeated = await t.slopsearx_read_saved_search_events("agent-a", limit=1)
    assert repeated["events"][0]["event_id"] == first["events"][0]["event_id"]

    second = await t.slopsearx_read_saved_search_events("agent-a", cursor=first["next_cursor"], limit=1)
    assert second["events"][0]["cursor"] > first["events"][0]["cursor"]


async def test_ack_is_idempotent_monotonic_and_survives_consumer_restart(event_state):
    state, now = event_state
    await _create()
    await _run(state, now, 1060)
    batch = await t.slopsearx_read_saved_search_events("agent-a")
    cursor = batch["next_cursor"]
    first = await t.slopsearx_ack_saved_search_events("agent-a", cursor)
    second = await t.slopsearx_ack_saved_search_events("agent-a", cursor)
    assert first == second

    restarted = SavedSearchStore(state.ctx.cache).for_tenant("default")
    assert await restarted.acknowledged_cursor("agent-a") == cursor
    resumed = await restarted.read_events("agent-a", cursor=None, limit=10)
    assert resumed["events"] == []


async def test_tenant_and_consumer_fences(event_state):
    state, now = event_state
    await _create()
    await _run(state, now, 1060)
    default_batch = await t.slopsearx_read_saved_search_events("agent-a")
    with tenant_scope("other"):
        other = await t.slopsearx_read_saved_search_events("agent-a")
        assert other["events"] == []
        expired = await t.slopsearx_ack_saved_search_events("agent-a", default_batch["next_cursor"])
        assert expired["error"]["code"] == "cursor_expired"
    assert (await t.slopsearx_read_saved_search_events("agent-a"))["events"]


async def test_consumer_capacity_is_bounded(event_state):
    state, now = event_state
    await _create()
    await _run(state, now, 1060)
    cursor = (await t.slopsearx_read_saved_search_events("agent-a"))["next_cursor"]
    assert "error" not in await t.slopsearx_ack_saved_search_events("agent-a", cursor)
    assert "error" not in await t.slopsearx_ack_saved_search_events("agent-b", cursor)
    rejected = await t.slopsearx_ack_saved_search_events("agent-c", cursor)
    assert rejected["error"]["code"] == "resource_limit"


async def test_capacity_rejection_keeps_report_and_event_atomic(event_state):
    state, now = event_state
    state.saved_store._event_retention_seconds = 1000
    state.saved_store.for_tenant("default")._event_retention_seconds = 1000
    created = await _create()
    await _run(state, now, 1060)
    await _run(state, now, 1120)
    await _run(state, now, 1180)
    before = {
        key: copy.deepcopy(value) for key, value in state.ctx.cache._data.items() if key.startswith("mcp:saved:v1:")
    }
    now[0] = 1240
    assert await state.saved_runner.run_due("default") == []
    after = {key: value for key, value in state.ctx.cache._data.items() if key.startswith("mcp:saved:v1:")}
    assert after == before
    reports = await state.saved_store.for_tenant("default").reports(created["search_id"], now=1240)
    assert all(report["finished_at"] < 1240 for report in reports)


async def test_pause_capacity_rejection_keeps_definition_active(event_state):
    state, now = event_state
    state.saved_store._event_capacity = 1
    state.saved_store.for_tenant("default")._event_capacity = 1
    created = await _create()
    await _run(state, now, 1060)
    paused = await t.slopsearx_pause_saved_search(created["search_id"], 1)
    assert paused["error"]["code"] == "resource_limit"
    current = await t.slopsearx_get_saved_search(created["search_id"])
    assert current["revision"] == 1
    assert current["paused"] is False


async def test_retention_gap_is_explicit_and_does_not_depend_on_ack(event_state):
    state, now = event_state
    await _create()
    await _run(state, now, 1060)
    first = await t.slopsearx_read_saved_search_events("agent-a")
    stale_cursor = first["events"][0]["cursor"]
    now[0] = 1300
    await state.saved_runner.run_due("default")
    batch = await t.slopsearx_read_saved_search_events("agent-a", cursor=stale_cursor)
    assert batch["gap"] == {
        "detected": True,
        "reason": "retention_expired",
        "first_available_cursor": batch["events"][0]["cursor"],
    }
    expired_ack = await t.slopsearx_ack_saved_search_events("agent-a", stale_cursor)
    assert expired_ack["error"]["code"] == "cursor_expired"


async def test_read_enforces_retention_without_a_new_publication(event_state):
    state, now = event_state
    await _create()
    await _run(state, now, 1060)
    first = await t.slopsearx_read_saved_search_events("agent-a")
    stale_cursor = first["events"][0]["cursor"]
    await _run(state, now, 1120)

    now[0] = 1200
    batch = await state.saved_store.for_tenant("default").read_events(
        "agent-a", cursor=stale_cursor, limit=10, now=now[0]
    )

    assert len(batch["events"]) == 1
    live_cursor = batch["events"][0][0]
    assert batch["gap"] == {
        "detected": True,
        "reason": "retention_expired",
        "first_available_cursor": live_cursor,
    }
    stream = await state.ctx.cache.get(state.saved_store.for_tenant("default")._event_stream())
    assert [item["cursor"] for item in stream["entries"]] == [live_cursor]


async def test_ack_rejects_an_expired_cursor_without_a_prior_trim_read(event_state):
    state, now = event_state
    await _create()
    await _run(state, now, 1060)
    stale_cursor = (await t.slopsearx_read_saved_search_events("agent-a"))["events"][0]["cursor"]
    await _run(state, now, 1120)

    now[0] = 1200
    rejected = await t.slopsearx_ack_saved_search_events("agent-a", stale_cursor)

    assert rejected["error"]["code"] == "cursor_expired"


async def test_policy_revocation_redacts_protected_detail_but_explains_state(event_state):
    state, now = event_state
    await _create()
    await _run(state, now, 1060)
    state.policy.sensitive_engines.add("wikipedia")
    batch = await t.slopsearx_read_saved_search_events("agent-a")
    event = batch["events"][0]
    assert event["policy_state"] == "redacted"
    assert event["reason_code"] == "policy_redacted"
    assert event["summary"] is None
    assert "_policy_engines" not in event


async def test_pause_and_expiry_events_are_idempotent(event_state):
    state, now = event_state
    created = await _create()
    paused = await t.slopsearx_pause_saved_search(created["search_id"], 1)
    assert paused["paused"] is True
    batch = await t.slopsearx_read_saved_search_events("agent-a")
    assert [event["event_type"] for event in batch["events"]] == ["definition_paused"]

    now[0] = 1700
    assert await state.saved_store.publish_expired_events(now=1700) == 1
    assert await state.saved_store.publish_expired_events(now=1700) == 0
    batch = await t.slopsearx_read_saved_search_events("agent-a")
    assert [event["event_type"] for event in batch["events"]] == ["definition_expired"]


@pytest.mark.parametrize(
    ("consumer_id", "cursor"),
    [
        ("", None),
        ("has space", None),
        ("a" * 129, None),
        ("agent", "bad"),
        ("agent", "01-0"),
        ("agent", "1" * 65 + "-0"),
    ],
)
async def test_invalid_consumer_and_cursor_are_rejected_without_mutation(event_state, consumer_id, cursor):
    state, _now = event_state
    before = copy.deepcopy(state.ctx.cache._data)
    value = await t.slopsearx_read_saved_search_events(consumer_id, cursor=cursor)
    assert value["error"]["code"] == "invalid_input"
    assert state.ctx.cache._data == before


async def test_separate_grant_stops_reads_and_new_publications(event_state):
    state, now = event_state
    created = await _create()
    state.policy.enabled_tools["saved_search_events"] = False
    await _run(state, now, 1060)
    assert (await t.slopsearx_read_saved_search_events("agent-a"))["error"]["code"] == "tool_disabled"
    state.policy.enabled_tools["saved_search_events"] = True
    assert (await t.slopsearx_read_saved_search_events("agent-a"))["events"] == []
    assert (await t.slopsearx_read_saved_search_reports(created["search_id"]))["reports"]


@pytest.mark.parametrize(
    ("status", "reasons", "operational_error", "event_type"),
    [
        ("compared", [], None, "report_created"),
        ("incomparable", ["incomplete_response"], None, "run_incomparable"),
        ("incomparable", ["dispatch_failed"], "RuntimeError", "run_failed"),
    ],
)
def test_report_event_types_use_closed_reason_codes(status, reasons, operational_error, event_type):
    item = SavedDefinition("saved", "tenant", "query", ["wikipedia"], 60, 600, 10, 10, 1000, 1600, 1060)
    report = {
        "run_id": "run",
        "finished_at": 1060,
        "status": status,
        "incomparable_reasons": reasons,
        "missed_slots": 0,
        "events": [],
        "observation": {
            "coverage": {"wikipedia": "ok"},
            "discovered_count": 1,
            **({"operational_error": operational_error} if operational_error else {}),
        },
    }
    assert event_for_report(item, report)["event_type"] == event_type


def test_event_tools_accept_no_callback_or_outbound_url():
    for tool in (t.slopsearx_read_saved_search_events, t.slopsearx_ack_saved_search_events):
        assert not ({"url", "callback", "callback_url", "webhook"} & set(inspect.signature(tool).parameters))
