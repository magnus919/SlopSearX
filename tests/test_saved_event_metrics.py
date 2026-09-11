"""Closed-cardinality operational metrics for the saved-search outbox."""

from __future__ import annotations

import pytest

from slopsearx import metrics as m


@pytest.fixture
def event_metrics(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(m, "saved_event_publications", m.Counter("publication", "publication"))
    monkeypatch.setattr(m, "saved_event_reads", m.Counter("reads", "reads"))
    monkeypatch.setattr(m, "saved_event_acknowledgements", m.Counter("acks", "acks"))
    monkeypatch.setattr(m, "saved_event_retention_gaps", m.Counter("gaps", "gaps"))
    monkeypatch.setattr(m, "saved_event_capacity_rejections", m.Counter("capacity", "capacity"))
    monkeypatch.setattr(m, "saved_event_backlog_age", m.Gauge("age", "age"))


def test_publication_read_ack_gap_capacity_and_age_are_recorded(event_metrics: None) -> None:
    m.record_saved_event_publication("report_created")
    m.record_saved_event_read("gap", oldest_age_seconds=42)
    m.record_saved_event_ack("advanced")
    m.record_saved_event_capacity("stream")

    assert m.saved_event_publications._values['event_type="report_created"'] == 1
    assert m.saved_event_reads._values['outcome="gap"'] == 1
    assert m.saved_event_retention_gaps._values[""] == 1
    assert m.saved_event_acknowledgements._values['outcome="advanced"'] == 1
    assert m.saved_event_capacity_rejections._values['resource="stream"'] == 1
    assert m.saved_event_backlog_age._values[""] == 42


@pytest.mark.parametrize("outcome", ["empty", "gap"])
def test_read_without_events_clears_stale_backlog_age(event_metrics: None, outcome: str) -> None:
    m.record_saved_event_read("delivered", oldest_age_seconds=42)
    m.record_saved_event_read(outcome)

    assert m.saved_event_backlog_age._values[""] == 0


@pytest.mark.parametrize(
    ("helper", "value"),
    [
        (m.record_saved_event_publication, "tenant-secret"),
        (m.record_saved_event_read, "consumer-secret"),
        (m.record_saved_event_ack, "event-secret"),
        (m.record_saved_event_capacity, "query-secret"),
    ],
)
def test_user_or_stored_values_cannot_become_labels(event_metrics: None, helper, value) -> None:
    with pytest.raises(ValueError):
        helper(value)


def test_rendered_metrics_expose_only_closed_operational_dimensions(event_metrics: None) -> None:
    m.record_saved_event_publication("definition_expired")
    m.record_saved_event_read("empty")
    m.record_saved_event_ack("idempotent")
    m.record_saved_event_capacity("consumer")
    rendered = "".join(
        metric.render()
        for metric in (
            m.saved_event_publications,
            m.saved_event_reads,
            m.saved_event_acknowledgements,
            m.saved_event_capacity_rejections,
        )
    )
    assert "definition_expired" in rendered
    assert "idempotent" in rendered
    assert "consumer" in rendered
    assert not any(secret in rendered for secret in ("tenant", "query", "saved-", "run-", "event-"))
