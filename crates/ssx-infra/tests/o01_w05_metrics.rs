//! Concrete renderer composition with the accepted W05 producer, private Valkey only.
use ssx_application::saved::{SavedLimits, SavedPolicy};
use ssx_domain::TenantId;
use ssx_infra::{
    observability::{Options, Telemetry},
    saved_events::{codec::EventType, observations::*, *},
    valkey::saved::{events::*, *},
};
use std::{collections::BTreeMap, sync::Arc};
#[path = "w05_support/mod.rs"]
mod support;
use support::*;
fn event_key(tenant: &TenantId) -> String {
    ssx_infra::valkey::keys::saved("events", tenant, None).unwrap()
}
fn sample(text: &str, prefix: &str) -> f64 {
    text.lines()
        .find(|line| line.starts_with(prefix))
        .unwrap()
        .split_whitespace()
        .last()
        .unwrap()
        .parse()
        .unwrap()
}
#[tokio::test]
async fn all_fifteen_callbacks_render_once_and_typed_age_resets_only_source_cases() {
    let telemetry = Telemetry::new(Options::default(), None, vec![], vec![]).unwrap();
    let observer = telemetry.saved_event_observer();
    let cases = [
        (
            EventObservation::Published(EventType::ReportCreated),
            "slopsearx_saved_search_event_publications_total{event_type=\"report_created\"} 1",
        ),
        (
            EventObservation::Published(EventType::RunIncomparable),
            "slopsearx_saved_search_event_publications_total{event_type=\"run_incomparable\"} 1",
        ),
        (
            EventObservation::Published(EventType::RunFailed),
            "slopsearx_saved_search_event_publications_total{event_type=\"run_failed\"} 1",
        ),
        (
            EventObservation::Published(EventType::DefinitionPaused),
            "slopsearx_saved_search_event_publications_total{event_type=\"definition_paused\"} 1",
        ),
        (
            EventObservation::Published(EventType::DefinitionExpired),
            "slopsearx_saved_search_event_publications_total{event_type=\"definition_expired\"} 1",
        ),
        (
            EventObservation::Read {
                outcome: ReadOutcome::Delivered,
                oldest_age_seconds: None,
            },
            "slopsearx_saved_search_event_reads_total{outcome=\"delivered\"} 1",
        ),
        (
            EventObservation::Read {
                outcome: ReadOutcome::Empty,
                oldest_age_seconds: None,
            },
            "slopsearx_saved_search_event_reads_total{outcome=\"empty\"} 1",
        ),
        (
            EventObservation::Read {
                outcome: ReadOutcome::Gap,
                oldest_age_seconds: Some(BacklogAge::new(12.5).unwrap()),
            },
            "slopsearx_saved_search_event_reads_total{outcome=\"gap\"} 1",
        ),
        (
            EventObservation::Read {
                outcome: ReadOutcome::Redacted,
                oldest_age_seconds: None,
            },
            "slopsearx_saved_search_event_reads_total{outcome=\"redacted\"} 1",
        ),
        (
            EventObservation::Read {
                outcome: ReadOutcome::Rejected,
                oldest_age_seconds: None,
            },
            "slopsearx_saved_search_event_reads_total{outcome=\"rejected\"} 1",
        ),
        (
            EventObservation::Ack(AckOutcome::Advanced),
            "slopsearx_saved_search_event_acknowledgements_total{outcome=\"advanced\"} 1",
        ),
        (
            EventObservation::Ack(AckOutcome::Idempotent),
            "slopsearx_saved_search_event_acknowledgements_total{outcome=\"idempotent\"} 1",
        ),
        (
            EventObservation::Ack(AckOutcome::Rejected),
            "slopsearx_saved_search_event_acknowledgements_total{outcome=\"rejected\"} 1",
        ),
        (
            EventObservation::Capacity(Capacity::Stream),
            "slopsearx_saved_search_event_capacity_rejections_total{resource=\"stream\"} 1",
        ),
        (
            EventObservation::Capacity(Capacity::Consumer),
            "slopsearx_saved_search_event_capacity_rejections_total{resource=\"consumer\"} 1",
        ),
    ];
    assert_eq!(cases.len(), 15);
    for (event, _) in &cases {
        observer.observe(event.clone());
    }
    let text = telemetry.metrics.render();
    for (_, line) in &cases {
        assert!(text.lines().any(|actual| actual == *line), "{line}");
    }
    assert_eq!(
        sample(
            &text,
            "slopsearx_saved_search_event_retention_gaps_total{} "
        ),
        1.
    );
    assert_eq!(
        sample(&text, "slopsearx_saved_search_event_backlog_age_seconds{} "),
        12.5
    );
    observer.observe(EventObservation::Read {
        outcome: ReadOutcome::Empty,
        oldest_age_seconds: None,
    });
    assert_eq!(
        sample(
            &telemetry.metrics.render(),
            "slopsearx_saved_search_event_backlog_age_seconds{} "
        ),
        0.
    );
    observer.observe(EventObservation::Read {
        outcome: ReadOutcome::Gap,
        oldest_age_seconds: Some(BacklogAge::new(42.).unwrap()),
    });
    assert_eq!(
        sample(
            &telemetry.metrics.render(),
            "slopsearx_saved_search_event_backlog_age_seconds{} "
        ),
        42.
    );
    observer.observe(EventObservation::Read {
        outcome: ReadOutcome::Gap,
        oldest_age_seconds: None,
    });
    assert_eq!(
        sample(
            &telemetry.metrics.render(),
            "slopsearx_saved_search_event_backlog_age_seconds{} "
        ),
        0.
    );
    assert_eq!(
        sample(
            &telemetry.metrics.render(),
            "slopsearx_saved_search_event_retention_gaps_total{} "
        ),
        3.
    );
    ssx_application::Metrics::increment(&*telemetry, "synthetic-user-unrecognized");
    assert!(
        !telemetry
            .metrics
            .render()
            .contains("synthetic-user-unrecognized")
    );
    assert!(BacklogAge::new(f64::NAN).is_err());
    assert!(BacklogAge::new(-1.).is_err());
    telemetry.shutdown().await;
    let disabled = Telemetry::new(
        Options {
            monitoring: false,
            ..Options::default()
        },
        None,
        vec![],
        vec![],
    )
    .unwrap();
    let observer = disabled.saved_event_observer();
    for (event, _) in cases {
        observer.observe(event);
    }
    assert!(disabled.metrics.render().is_empty());
    disabled.shutdown().await;
}
struct Authority;
impl EventAuthority for Authority {
    fn events_granted(&self) -> bool {
        true
    }
    fn saved_granted(&self) -> bool {
        true
    }
    fn engines_allowed(&self, _: &[String]) -> bool {
        true
    }
}
async fn create(store: &SavedStore, name: &str) -> Claim {
    let search = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let policy = SavedPolicy {
        search: &search,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let prepared = create_at(
        &policy,
        store.tenant().as_str(),
        name,
        store.now().await.unwrap(),
    );
    store.create(&prepared).await.unwrap();
    store
        .claim(&id(name), &mut Tokens("o01-owner"))
        .await
        .unwrap()
        .unwrap()
}
#[tokio::test]
async fn actual_private_publication_read_ack_capacity_and_closed_store_render_no_false_success() {
    let server = Server::start();
    let backend = Arc::new(server.store());
    let store = SavedStore::new(
        backend.clone(),
        TenantId::new("tenant-private-o01").unwrap(),
    );
    let telemetry = Telemetry::new(Options::default(), None, vec!["brave".into()], vec![]).unwrap();
    let publisher = EventPublisher {
        limits: EventLimits::new(1, 600, 1).unwrap(),
        authority: Arc::new(Authority),
        observer: telemetry.saved_event_observer(),
    };
    let claim = create(&store, "saved-private-o01").await;
    let (report, baseline, next) = publication(&claim, store.now().await.unwrap());
    assert!(
        publisher
            .publish(
                &store,
                &claim,
                &report,
                baseline.as_ref(),
                next,
                &claim.definition().policy_fingerprint
            )
            .await
            .unwrap()
    );
    assert!(
        !publisher
            .publish(
                &store,
                &claim,
                &report,
                baseline.as_ref(),
                next,
                &claim.definition().policy_fingerprint
            )
            .await
            .unwrap()
    );
    let principal =
        EventPrincipal::new(store.tenant().clone(), "subject-private-o01".into()).unwrap();
    let api = EventService::new(store.clone(), &principal, publisher.clone());
    let batch = api.read("consumer-private-o01", None, 1).await;
    assert_eq!(batch["returned"], 1);
    let cursor = batch["next_cursor"].as_str().unwrap();
    let age = sample(
        &telemetry.metrics.render(),
        "slopsearx_saved_search_event_backlog_age_seconds{} ",
    );
    assert!(age.is_finite() && age >= 0.);
    assert_eq!(
        api.ack("consumer-private-o01", cursor).await["acknowledged_cursor"],
        cursor
    );
    assert_eq!(
        api.ack("consumer-private-o01", cursor).await["acknowledged_cursor"],
        cursor
    );
    assert_eq!(
        api.ack("consumer-private-o01", "bad").await["error"]["code"],
        "invalid_input"
    );
    assert_eq!(
        api.ack("other-private-o01", cursor).await["error"]["code"],
        "resource_limit"
    );
    let empty = api.read("consumer-private-o01", None, 1).await;
    assert_eq!(empty["returned"], 0);
    assert_eq!(
        sample(
            &telemetry.metrics.render(),
            "slopsearx_saved_search_event_backlog_age_seconds{} "
        ),
        0.
    );
    let blocked = create(&store, "blocked-private-o01").await;
    let (report2, baseline2, next2) = publication(&blocked, store.now().await.unwrap());
    assert_eq!(
        publisher
            .publish(
                &store,
                &blocked,
                &report2,
                baseline2.as_ref(),
                next2,
                &blocked.definition().policy_fingerprint
            )
            .await
            .unwrap_err(),
        Error::OutboxCapacity
    );
    let closed = Arc::new(server.store());
    let unavailable = SavedStore::new(closed.clone(), store.tenant().clone());
    closed.close().await;
    assert!(
        publisher
            .publish(
                &unavailable,
                &blocked,
                &report2,
                baseline2.as_ref(),
                next2,
                &blocked.definition().policy_fingerprint
            )
            .await
            .is_err()
    );
    let text = telemetry.metrics.render();
    for (line, value) in [
        (
            "slopsearx_saved_search_event_publications_total{event_type=\"report_created\"} ",
            1.,
        ),
        (
            "slopsearx_saved_search_event_reads_total{outcome=\"delivered\"} ",
            1.,
        ),
        (
            "slopsearx_saved_search_event_reads_total{outcome=\"empty\"} ",
            1.,
        ),
        (
            "slopsearx_saved_search_event_acknowledgements_total{outcome=\"advanced\"} ",
            1.,
        ),
        (
            "slopsearx_saved_search_event_acknowledgements_total{outcome=\"idempotent\"} ",
            1.,
        ),
        (
            "slopsearx_saved_search_event_acknowledgements_total{outcome=\"rejected\"} ",
            2.,
        ),
        (
            "slopsearx_saved_search_event_capacity_rejections_total{resource=\"stream\"} ",
            1.,
        ),
        (
            "slopsearx_saved_search_event_capacity_rejections_total{resource=\"consumer\"} ",
            1.,
        ),
    ] {
        assert_eq!(sample(&text, line), value, "{line}");
    }
    for secret in [
        "tenant-private-o01",
        "consumer-private-o01",
        "other-private-o01",
        "saved-private-o01",
        "subject-private-o01",
        report["run_id"].as_str().unwrap(),
    ] {
        assert!(!text.contains(secret));
    }
    let mut connection = server.connection();
    let length: usize = redis::cmd("XLEN")
        .arg(event_key(store.tenant()))
        .query(&mut connection)
        .unwrap();
    assert_eq!(length, 1);
    let ack: Option<String> = redis::cmd("GET")
        .arg(acknowledgement_key(
            store.tenant(),
            &ssx_infra::saved_events::codec::ConsumerId::new("consumer-private-o01").unwrap(),
        ))
        .query(&mut connection)
        .unwrap();
    assert_eq!(ack.as_deref(), Some(cursor));
    drop(connection);
    telemetry.shutdown().await;
    backend.close().await;
    drop(api);
    drop(publisher);
    drop(store);
    drop(server);
}
