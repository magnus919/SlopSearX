"""Closed-cardinality lifecycle telemetry for durable agent workflows."""

from __future__ import annotations

import pytest
from prometheus_client.parser import text_string_to_metric_families

from slopsearx import metrics as m


@pytest.fixture
def workflow_metrics(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(m, "_workflow_oldest_started", {})
    replacements = {
        "workflow_accepted": m.Counter("slopsearx_workflow_accepted_total", "accepted"),
        "workflow_terminal": m.Counter("slopsearx_workflow_terminal_total", "terminal"),
        "workflow_retries": m.Counter("slopsearx_workflow_retries_total", "retries"),
        "workflow_lease_recoveries": m.Counter("slopsearx_workflow_lease_recoveries_total", "recoveries"),
        "workflow_rejections": m.Counter("slopsearx_workflow_rejections_total", "rejections"),
        "workflow_expired": m.Counter("slopsearx_workflow_expired_total", "expired"),
        "workflow_queued": m.Gauge("slopsearx_workflow_queued", "queued"),
        "workflow_running": m.Gauge("slopsearx_workflow_running", "running"),
        "workflow_active_leases": m.Gauge("slopsearx_workflow_active_leases", "leases"),
        "workflow_oldest_claimable_age": m.Gauge("slopsearx_workflow_oldest_claimable_age_seconds", "age"),
        "workflow_execution": m.Histogram("slopsearx_workflow_execution_seconds", "execution"),
    }
    for name, value in replacements.items():
        monkeypatch.setattr(m, name, value)


@pytest.mark.parametrize("workflow", m.WORKFLOW_KINDS)
def test_every_workflow_uses_the_closed_lifecycle_vocabulary(workflow_metrics: None, workflow: m.WorkflowKind) -> None:
    mode = "immediate" if workflow == "retrieval_receipt" else "durable_leased"
    m.record_workflow_accepted(workflow, mode)
    m.transition_workflow(workflow, None, "queued")
    m.transition_workflow(workflow, "queued", "running")
    m.record_workflow_recovery(workflow)
    m.record_workflow_retry(workflow)
    m.record_workflow_rejection(workflow, "policy")
    m.record_workflow_expiry(workflow, "receipt" if workflow == "retrieval_receipt" else "job")
    m.transition_workflow(workflow, "running", "succeeded")
    m.record_workflow_terminal(workflow, "succeeded", 0.25)

    label = f'workflow="{workflow}"'
    assert m.workflow_queued._values[label] == 0
    assert m.workflow_running._values[label] == 0
    assert m.workflow_active_leases._values[label] == 0
    assert list(text_string_to_metric_families(m.render_metrics()))


def test_reconciliation_replaces_stale_gauges_after_recovery(workflow_metrics: None) -> None:
    m.transition_workflow("research", None, "queued")
    m.transition_workflow("research", "queued", "running")
    m.reconcile_workflow("research", queued=3, running=1, active_leases=1, oldest_claimable_age_seconds=45)
    assert m.workflow_queued._values['workflow="research"'] == 3
    assert m.workflow_running._values['workflow="research"'] == 1
    assert m.workflow_active_leases._values['workflow="research"'] == 1


def test_interrupted_is_a_terminal_outcome_without_becoming_failure(workflow_metrics: None) -> None:
    m.transition_workflow("staged_search", None, "running")
    m.transition_workflow("staged_search", "running", "interrupted")
    m.record_workflow_terminal("staged_search", "interrupted")

    assert m.workflow_running._values['workflow="staged_search"'] == 0
    assert m.workflow_active_leases._values['workflow="staged_search"'] == 0
    assert m.workflow_terminal._values['outcome="interrupted",workflow="staged_search"'] == 1
    assert 'outcome="failed",workflow="staged_search"' not in m.workflow_terminal._values


def test_user_data_cannot_become_a_metric_label(workflow_metrics: None) -> None:
    secret = "tenant-query-job-url-secret"
    with pytest.raises(ValueError):
        m.record_workflow_accepted(secret, "durable")  # type: ignore[arg-type]
    with pytest.raises(ValueError):
        m.record_workflow_rejection("research", secret)
    assert secret not in m.render_metrics()


def test_tenant_facing_health_has_states_without_activity_counts(workflow_metrics: None) -> None:
    m.reconcile_workflow("staged_search", queued=4, running=2, active_leases=2, oldest_claimable_age_seconds=301)
    availability = {workflow: workflow != "saved_search" for workflow in m.WORKFLOW_KINDS}
    health = m.workflow_health_summary(availability=availability)
    assert health["staged_search"] == {"available": True, "status": "available"}
    assert health["saved_search"] == {"available": False, "status": "unavailable"}
    assert all(set(item) == {"available", "status"} for item in health.values())
