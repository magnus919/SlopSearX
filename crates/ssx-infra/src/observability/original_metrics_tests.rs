//! Exact assertions from the42-owned original functions; HTTP rows stay T02 handoffs.
//! Fixed native family names replace source helper arbitrary names explicitly.
use super::*;
fn complete(node: &str, consumer: &str) {
    println!(
        "O01_ORIGINAL_COMPLETE:{}",
        ssx_json::json!({"node":node,"consumer":consumer,"class":"native-original-assertions-fixed-registry"})
    );
}
fn scalar(m: &Metrics, name: &str, labels: &[&str]) -> f64 {
    let index = DEFINITIONS.iter().position(|d| d.name == name).unwrap();
    m.series.lock().unwrap()[&(
        index,
        labels.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    )]
        .value
}
#[test]
fn original_counter_and_gauge_assertions() {
    const C: &str = concat!(module_path!(), "::original_counter_and_gauge_assertions");
    let m = Metrics::new(true, vec!["brave".into()], vec![]).unwrap();
    for n in [1., 2.] {
        m.update("slopsearx_engine_queries_total", &[("engine", "brave")], n)
            .unwrap();
    }
    assert_eq!(scalar(&m, "slopsearx_engine_queries_total", &["brave"]), 3.);
    assert!(
        m.render()
            .contains("slopsearx_engine_queries_total{engine=\"brave\"} 3\n")
    );
    complete("tests/test_metrics.py::TestCounter::test_increment", C);
    let m = Metrics::new(true, vec!["brave".into(), "wikipedia".into()], vec![]).unwrap();
    for engine in ["brave", "wikipedia", "brave"] {
        m.update("slopsearx_engine_queries_total", &[("engine", engine)], 1.)
            .unwrap();
    }
    assert_eq!(scalar(&m, "slopsearx_engine_queries_total", &["brave"]), 2.);
    assert_eq!(
        scalar(&m, "slopsearx_engine_queries_total", &["wikipedia"]),
        1.
    );
    complete(
        "tests/test_metrics.py::TestCounter::test_multiple_labels",
        C,
    );
    let mut header = String::new();
    render_header(&mut header, "test_total", "Test counter", Kind::Counter);
    assert_eq!(
        header,
        "# HELP test_total Test counter\n# TYPE test_total counter\n"
    );
    complete("tests/test_metrics.py::TestCounter::test_header", C);
    let m = Metrics::new(true, vec!["brave".into(), "google".into()], vec![]).unwrap();
    m.update("slopsearx_engine_status", &[("engine", "brave")], 0.)
        .unwrap();
    m.update("slopsearx_engine_status", &[("engine", "google")], 1.)
        .unwrap();
    let text = m.render();
    assert!(text.contains("slopsearx_engine_status{engine=\"brave\"} 0\n"));
    assert!(text.contains("slopsearx_engine_status{engine=\"google\"} 1\n"));
    complete("tests/test_metrics.py::TestGauge::test_set_and_render", C);
    m.update("slopsearx_engine_status", &[("engine", "brave")], 2.)
        .unwrap();
    assert_eq!(scalar(&m, "slopsearx_engine_status", &["brave"]), 2.);
    assert!(
        m.render()
            .contains("slopsearx_engine_status{engine=\"brave\"} 2\n")
    );
    complete("tests/test_metrics.py::TestGauge::test_overwrite", C);
}
#[test]
fn original_histogram_assertions() {
    const C: &str = concat!(module_path!(), "::original_histogram_assertions");
    let bounds = BucketBounds::new(&[0.1, 0.5, 5.]).unwrap();
    let mut state = Series::default();
    for n in [0., 0.1, 0.2, 0.5, 6.] {
        state.observe(bounds, n).unwrap();
    }
    assert_eq!(state.buckets, vec![2, 4, 4]);
    assert_eq!(state.count, 5);
    assert!((state.value - 6.8).abs() < f64::EPSILON * 8.);
    let mut text = String::new();
    render_header(&mut text, "custom", "custom histogram", Kind::Histogram);
    render_histogram(&mut text, "custom", "engine=\"fixture\"", bounds, &state);
    let source: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../xtask/fixtures/o01/source-supplement.json"
    ))
    .unwrap();
    let custom = source["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "histogram.buckets.38")
        .unwrap();
    assert_eq!(text, custom["output"]["render"].as_str().unwrap());
    // Exact golden text plus structured counts/sum/type proves the original parser
    // assertions without introducing a second production OpenMetrics parser.
    complete(
        "tests/test_metrics.py::TestHistogram::test_cumulative_buckets_and_parser",
        C,
    );
    let bounds = BucketBounds::new(LATENCY).unwrap();
    let mut state = Series::default();
    for _ in 0..100_000 {
        state.observe(bounds, 0.25).unwrap();
    }
    assert_eq!(state.buckets.len(), LATENCY.len());
    assert_eq!(state.count, 100_000);
    assert_eq!(state.value, 25_000.);
    let mut text = String::new();
    render_histogram(&mut text, "latency", "", bounds, &state);
    assert_eq!(text.lines().count(), LATENCY.len() + 3);
    complete(
        "tests/test_metrics.py::TestHistogram::test_bounded_retention",
        C,
    );
    let m = Metrics::new(true, vec!["brave".into()], vec![]).unwrap();
    for n in [f64::NAN, f64::INFINITY, -1.] {
        m.update(
            "slopsearx_engine_latency_seconds",
            &[("engine", "brave")],
            n,
        )
        .unwrap();
    }
    assert!(m.series.lock().unwrap().is_empty());
    assert!(!m.render().contains("engine=\"brave\""));
    complete(
        "tests/test_metrics.py::TestHistogram::test_nonfinite_and_negative_latency_ignored",
        C,
    );
    for (i, values) in [vec![1., 1.], vec![2., 1.], vec![-1.], vec![f64::INFINITY]]
        .iter()
        .enumerate()
    {
        assert_eq!(
            BucketBounds::new(values).err(),
            Some("Histogram buckets must be finite, nonnegative, and strictly increasing")
        );
        complete(
            &format!("tests/test_metrics.py::TestHistogram::test_invalid_buckets[buckets{i}]"),
            C,
        );
    }
    let text = m.render();
    assert!(text.contains("# HELP slopsearx_engine_latency_seconds "));
    assert!(text.contains("# TYPE slopsearx_engine_latency_seconds histogram\n"));
    assert!(
        !text
            .lines()
            .any(|l| l.starts_with("slopsearx_engine_latency_seconds"))
    );
    complete(
        "tests/test_metrics.py::TestHistogram::test_empty_no_output",
        C,
    );
}
#[test]
fn original_render_escape_and_error_assertions() {
    const C: &str = concat!(
        module_path!(),
        "::original_render_escape_and_error_assertions"
    );
    let m = Metrics::new(true, vec!["brave".into()], vec![]).unwrap();
    for (name, value) in [
        ("slopsearx_engine_queries_total", 1.),
        ("slopsearx_engine_latency_seconds", 0.34),
        ("slopsearx_engine_status", 0.),
    ] {
        m.update(name, &[("engine", "brave")], value).unwrap();
    }
    m.update("slopsearx_cache_hit_total", &[("type", "hit")], 1.)
        .unwrap();
    m.update("slopsearx_server_requests_total", &[], 1.)
        .unwrap();
    let text = m.render();
    let original: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../xtask/fixtures/o01/original-render-source.json"
    ))
    .unwrap();
    assert_eq!(text, original["render"].as_str().unwrap());
    assert!(original["parsed_family_count"].as_i64().unwrap() > 0);
    for name in [
        "slopsearx_engine_queries_total",
        "slopsearx_engine_latency_seconds",
        "slopsearx_engine_status",
        "slopsearx_cache_hit_total",
        "slopsearx_server_requests_total",
    ] {
        assert!(text.contains(name));
    }
    assert!(text.lines().any(|l| !l.starts_with('#') && !l.is_empty()));
    complete(
        "tests/test_metrics.py::TestRenderMetrics::test_render_produces_valid_openmetrics",
        C,
    );
    assert!(text.matches("# HELP").count() >= 5);
    assert!(text.matches("# TYPE").count() >= 5);
    complete(
        "tests/test_metrics.py::TestRenderMetrics::test_render_includes_help_and_type",
        C,
    );
    let label = "quotes\" slash\\\\ and\nnewline";
    let expected = "engine=\"quotes\\\" slash\\\\\\\\ and\\nnewline\"";
    for name in [
        "slopsearx_engine_queries_total",
        "slopsearx_engine_status",
        "slopsearx_engine_latency_seconds",
    ] {
        let m = Metrics::new(true, vec![label.into()], vec![]).unwrap();
        m.update(name, &[("engine", label)], 1.).unwrap();
        let text = m.render();
        let samples = text
            .lines()
            .filter(|l| l.starts_with(name))
            .collect::<Vec<_>>();
        assert!(!samples.is_empty());
        assert!(samples.iter().all(|l| l.contains(expected)));
        let series = m.series.lock().unwrap();
        assert!(
            series
                .keys()
                .all(|(_, values)| values == &vec![label.to_owned()])
        );
    }
    complete("tests/test_metrics.py::test_label_escaping_roundtrips", C);
    let m = Metrics::new(true, vec!["example".into()], vec![]).unwrap();
    for status in [
        ssx_domain::EngineStatus::Ok,
        ssx_domain::EngineStatus::RateLimited,
        ssx_domain::EngineStatus::Blocked,
        ssx_domain::EngineStatus::Error,
        ssx_domain::EngineStatus::Timeout,
        ssx_domain::EngineStatus::Unavailable,
    ] {
        m.engine_outcome(
            "example",
            &AdapterResponse {
                status,
                ..Default::default()
            },
            OutcomeOrigin::AdapterReturned,
        );
    }
    assert_eq!(
        scalar(&m, "slopsearx_engine_errors_total", &["example"]),
        5.
    );
    assert_eq!(
        scalar(&m, "slopsearx_engine_queries_total", &["example"]),
        6.
    );
    complete(
        "tests/test_metrics.py::test_engine_error_counter_counts_failures_not_empty",
        C,
    );
}
#[test]
fn original_saved_event_metric_assertions() {
    const C: &str = concat!(module_path!(), "::original_saved_event_metric_assertions");
    let m = Metrics::new(true, vec![], vec![]).unwrap();
    m.update(
        "slopsearx_saved_search_event_publications_total",
        &[("event_type", "report_created")],
        1.,
    )
    .unwrap();
    m.saved_event_read("gap", Some(42.)).unwrap();
    m.update(
        "slopsearx_saved_search_event_acknowledgements_total",
        &[("outcome", "advanced")],
        1.,
    )
    .unwrap();
    m.update(
        "slopsearx_saved_search_event_capacity_rejections_total",
        &[("resource", "stream")],
        1.,
    )
    .unwrap();
    for (name, labels) in [
        (
            "slopsearx_saved_search_event_publications_total",
            vec!["report_created"],
        ),
        ("slopsearx_saved_search_event_reads_total", vec!["gap"]),
        ("slopsearx_saved_search_event_retention_gaps_total", vec![]),
        (
            "slopsearx_saved_search_event_acknowledgements_total",
            vec!["advanced"],
        ),
        (
            "slopsearx_saved_search_event_capacity_rejections_total",
            vec!["stream"],
        ),
    ] {
        assert_eq!(scalar(&m, name, &labels), 1.);
    }
    assert_eq!(
        scalar(&m, "slopsearx_saved_search_event_backlog_age_seconds", &[]),
        42.
    );
    complete(
        "tests/test_saved_event_metrics.py::test_publication_read_ack_gap_capacity_and_age_are_recorded",
        C,
    );
    for outcome in ["empty", "gap"] {
        m.saved_event_read("delivered", Some(42.)).unwrap();
        m.saved_event_read(outcome, None).unwrap();
        assert_eq!(
            scalar(&m, "slopsearx_saved_search_event_backlog_age_seconds", &[]),
            0.
        );
        complete(
            &format!(
                "tests/test_saved_event_metrics.py::test_read_without_events_clears_stale_backlog_age[{outcome}]"
            ),
            C,
        );
    }
    for (helper, secret, name, label) in [
        (
            "record_saved_event_publication",
            "tenant-secret",
            "slopsearx_saved_search_event_publications_total",
            "event_type",
        ),
        (
            "record_saved_event_read",
            "consumer-secret",
            "slopsearx_saved_search_event_reads_total",
            "outcome",
        ),
        (
            "record_saved_event_ack",
            "event-secret",
            "slopsearx_saved_search_event_acknowledgements_total",
            "outcome",
        ),
        (
            "record_saved_event_capacity",
            "query-secret",
            "slopsearx_saved_search_event_capacity_rejections_total",
            "resource",
        ),
    ] {
        let before = m.render();
        assert!(m.update(name, &[(label, secret)], 1.).is_err());
        assert_eq!(m.render(), before);
        assert!(!m.render().contains(secret));
        complete(
            &format!(
                "tests/test_saved_event_metrics.py::test_user_or_stored_values_cannot_become_labels[{helper}-{secret}]"
            ),
            C,
        );
    }
    let m = Metrics::new(true, vec![], vec![]).unwrap();
    for (name, label, value) in [
        (
            "slopsearx_saved_search_event_publications_total",
            "event_type",
            "definition_expired",
        ),
        (
            "slopsearx_saved_search_event_reads_total",
            "outcome",
            "empty",
        ),
        (
            "slopsearx_saved_search_event_acknowledgements_total",
            "outcome",
            "idempotent",
        ),
        (
            "slopsearx_saved_search_event_capacity_rejections_total",
            "resource",
            "consumer",
        ),
    ] {
        m.update(name, &[(label, value)], 1.).unwrap();
    }
    let text = m.render();
    for value in ["definition_expired", "idempotent", "consumer"] {
        assert!(text.contains(value));
    }
    for secret in ["tenant", "query", "saved-", "run-", "event-"] {
        assert!(!text.contains(secret));
    }
    complete(
        "tests/test_saved_event_metrics.py::test_rendered_metrics_expose_only_closed_operational_dimensions",
        C,
    );
}
#[test]
fn original_workflow_metric_assertions() {
    const C: &str = concat!(module_path!(), "::original_workflow_metric_assertions");
    let kinds = [
        "research",
        "dependency_dossier",
        "staged_search",
        "saved_search",
        "retrieval_receipt",
    ];
    for workflow in kinds {
        let m = Metrics::new(true, vec![], vec![]).unwrap();
        m.workflow_accepted(
            workflow,
            if workflow == "retrieval_receipt" {
                "immediate"
            } else {
                "durable_leased"
            },
        )
        .unwrap();
        m.workflow_transition(workflow, None, "queued").unwrap();
        m.workflow_transition(workflow, Some("queued"), "running")
            .unwrap();
        m.update(
            "slopsearx_workflow_lease_recoveries_total",
            &[("workflow", workflow)],
            1.,
        )
        .unwrap();
        // The source helper's omitted reason argument defaults to this closed value.
        m.update(
            "slopsearx_workflow_retries_total",
            &[("workflow", workflow), ("reason", "manual")],
            1.,
        )
        .unwrap();
        m.update(
            "slopsearx_workflow_rejections_total",
            &[("workflow", workflow), ("reason", "policy")],
            1.,
        )
        .unwrap();
        m.update(
            "slopsearx_workflow_expired_total",
            &[
                ("workflow", workflow),
                (
                    "artifact",
                    if workflow == "retrieval_receipt" {
                        "receipt"
                    } else {
                        "job"
                    },
                ),
            ],
            1.,
        )
        .unwrap();
        m.workflow_transition(workflow, Some("running"), "succeeded")
            .unwrap();
        m.workflow_terminal(workflow, "succeeded", Some(0.25))
            .unwrap();
        for name in [
            "slopsearx_workflow_queued",
            "slopsearx_workflow_running",
            "slopsearx_workflow_active_leases",
        ] {
            assert_eq!(scalar(&m, name, &[workflow]), 0.);
        }
        assert!(m.render().contains("_count"));
        complete(
            &format!(
                "tests/test_workflow_metrics.py::test_every_workflow_uses_the_closed_lifecycle_vocabulary[{workflow}]"
            ),
            C,
        );
    }
    let m = Metrics::new(true, vec![], vec![]).unwrap();
    m.workflow_transition("research", None, "queued").unwrap();
    m.workflow_transition("research", Some("queued"), "running")
        .unwrap();
    m.workflow_reconcile("research", 3, 1, 1, 45.).unwrap();
    for (name, n) in [
        ("slopsearx_workflow_queued", 3.),
        ("slopsearx_workflow_running", 1.),
        ("slopsearx_workflow_active_leases", 1.),
    ] {
        assert_eq!(scalar(&m, name, &["research"]), n);
    }
    complete(
        "tests/test_workflow_metrics.py::test_reconciliation_replaces_stale_gauges_after_recovery",
        C,
    );
    let m = Metrics::new(true, vec![], vec![]).unwrap();
    m.workflow_transition("staged_search", None, "running")
        .unwrap();
    m.workflow_transition("staged_search", Some("running"), "interrupted")
        .unwrap();
    m.workflow_terminal("staged_search", "interrupted", None)
        .unwrap();
    for name in [
        "slopsearx_workflow_running",
        "slopsearx_workflow_active_leases",
    ] {
        assert_eq!(scalar(&m, name, &["staged_search"]), 0.);
    }
    assert_eq!(
        scalar(
            &m,
            "slopsearx_workflow_terminal_total",
            &["interrupted", "staged_search"]
        ),
        1.
    );
    assert!(!m.render().contains("outcome=\"failed\""));
    complete(
        "tests/test_workflow_metrics.py::test_interrupted_is_a_terminal_outcome_without_becoming_failure",
        C,
    );
    let before = m.render();
    assert!(
        m.workflow_accepted("tenant-query-job-url-secret", "durable")
            .is_err()
    );
    assert!(
        m.update(
            "slopsearx_workflow_rejections_total",
            &[
                ("workflow", "research"),
                ("reason", "tenant-query-job-url-secret")
            ],
            1.
        )
        .is_err()
    );
    assert_eq!(m.render(), before);
    assert!(!m.render().contains("tenant-query-job-url-secret"));
    complete(
        "tests/test_workflow_metrics.py::test_user_data_cannot_become_a_metric_label",
        C,
    );
    m.workflow_reconcile("staged_search", 4, 2, 2, 301.)
        .unwrap();
    let availability = kinds
        .into_iter()
        .map(|kind| (kind, kind != "saved_search"))
        .collect();
    let health = Metrics::workflow_health_summary(&availability);
    assert_eq!(
        health["staged_search"],
        ssx_json::json!({"available":true,"status":"available"})
    );
    assert_eq!(
        health["saved_search"],
        ssx_json::json!({"available":false,"status":"unavailable"})
    );
    for value in health.as_object().unwrap().values() {
        assert_eq!(
            value
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["available", "status"]
        );
    }
    complete(
        "tests/test_workflow_metrics.py::test_tenant_facing_health_has_states_without_activity_counts",
        C,
    );
}
