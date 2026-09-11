"""Source-window deltas must not mistake ranking, cache, or outages for change."""

import copy

import pytest

from slopsearx.adapter import SearchResult
from slopsearx.saved_models import SavedDefinition, compare_observations, observe
from slopsearx.service import EngineOutcome, ScopeDecision, SearchResponse


@pytest.fixture
def definition():
    return SavedDefinition("saved", "tenant", "query", ["wikipedia"], 60, 3600, 100, 20, 0, 3600, 0)


def result(url, title="title", engine="wikipedia"):
    return SearchResult(url, title, "source text", engine)


def response(results, status="ok", cached=False):
    return SearchResponse(
        "query",
        results,
        ScopeDecision(selected_engines=["wikipedia"]),
        [EngineOutcome("wikipedia", status, len(results))],
        cached=cached,
    )


def run(definition, value, now=10):
    return compare_observations(
        definition,
        observe(definition, value, now),
        run_id=f"run-{now}",
        scheduled_at=now,
        started_at=now,
        finished_at=now,
        missed_slots=0,
    )


def baseline(definition, results):
    report, definition.baseline = run(definition, response(results))
    assert report["status"] == "baseline_initialized"
    assert report["events"] == []


def test_added_changed_and_not_observed_events_keep_source_values(definition):
    baseline(definition, [result("https://a.test"), result("https://b.test")])
    report, updated = run(definition, response([result("https://a.test", "new title"), result("https://c.test")]), 20)
    assert report["status"] == "compared"
    assert {event["kind"] for event in report["events"]} == {
        "added",
        "source_field_changed",
        "not_observed_in_latest_run",
    }
    changed = next(event for event in report["events"] if event["kind"] == "source_field_changed")
    assert changed["before"]["fields"]["title"] == "title"
    assert changed["after"]["fields"]["title"] == "new title"
    again, _ = run(definition, response([result("https://c.test"), result("https://a.test", "new title")]), 20)
    assert again["events"] == report["events"]
    assert updated is not None


def test_ranking_changes_are_not_source_changes(definition):
    rows = [result("https://a.test"), result("https://b.test")]
    baseline(definition, rows)
    rows.reverse()
    rows[0].score = 200
    rows[0].tier = 2
    rows[0].position = 10
    report, _ = run(definition, response(rows), 20)
    assert report["status"] == "compared"
    assert report["events"] == []


@pytest.mark.parametrize("reason", ["outage", "cached", "truncated", "source_winner"])
def test_incomparable_observation_never_replaces_good_baseline(definition, reason):
    baseline(definition, [result("https://a.test")])
    previous = copy.deepcopy(definition.baseline)
    if reason == "outage":
        value = response([], status="timeout")
    elif reason == "cached":
        value = response([], cached=True)
    elif reason == "truncated":
        definition.max_results = 1
        value = response([result("https://a.test"), result("https://b.test")])
    else:
        value = response([result("https://a.test", "other winner", engine="brave")])
    report, replacement = run(definition, value, 20)
    assert report["status"] == "incomparable"
    assert report["events"] == []
    assert replacement is None
    assert definition.baseline == previous


def test_expired_baseline_initializes_without_false_absence(definition):
    baseline(definition, [result("https://a.test")])
    definition.baseline["expires_at"] = 15
    report, replacement = run(definition, response([]), 20)
    assert report["status"] == "baseline_initialized"
    assert report["baseline_reason"] == "baseline_expired"
    assert report["events"] == []
    assert replacement["records"] == {}
