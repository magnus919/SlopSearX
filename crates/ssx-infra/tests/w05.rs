//! Original W05 semantics through native codecs, service and private real Valkey.
use ssx_application::{
    NoopSink, PortFuture, SearchEngine,
    cache::RoutingIdentity,
    policy::{PolicyService, SearchPolicy},
    scope::{CatalogSnapshot, EngineCapability, ScopeResolver},
};
use ssx_domain::{AdapterResponse, SearchRequest, SearchResult};
use ssx_infra::{
    saved_runner::{SavedRunner, StaticSavedAuthority},
    search::*,
};
use std::time::Duration;
#[path = "w05_support/search.rs"]
mod scheduler_support;
mod w05_support;
use ssx_application::saved::{SavedLimits, SavedPolicy};
use ssx_domain::{TenantId, saved::SavedDefinition};
use ssx_infra::{
    saved_events::{codec::*, observations::*, *},
    valkey::{
        saved::{events::*, *},
        *,
    },
};
use ssx_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use w05_support::*;
#[derive(Default)]
struct Recording(Mutex<Vec<EventObservation>>);
impl EventObserver for Recording {
    fn observe(&self, event: EventObservation) {
        self.0.lock().unwrap().push(event);
    }
}
struct Authority {
    events: AtomicBool,
    saved: AtomicBool,
    allowed: AtomicBool,
}
impl Default for Authority {
    fn default() -> Self {
        Self {
            events: true.into(),
            saved: true.into(),
            allowed: true.into(),
        }
    }
}
impl EventAuthority for Authority {
    fn events_granted(&self) -> bool {
        self.events.load(Ordering::SeqCst)
    }
    fn saved_granted(&self) -> bool {
        self.saved.load(Ordering::SeqCst)
    }
    fn engines_allowed(&self, _: &[String]) -> bool {
        self.allowed.load(Ordering::SeqCst)
    }
}
fn publisher(limits: EventLimits) -> (EventPublisher, Arc<Authority>, Arc<Recording>) {
    let authority = Arc::new(Authority::default());
    let observer = Arc::new(Recording::default());
    (
        EventPublisher {
            limits,
            authority: authority.clone(),
            observer: observer.clone(),
        },
        authority,
        observer,
    )
}
fn service(store: SavedStore, p: EventPublisher) -> EventService {
    EventService::new(
        store.clone(),
        &EventPrincipal::new(store.tenant().clone(), "fixture-subject".into()).unwrap(),
        p,
    )
}
fn saved_policy<'a>(
    search: &'a ssx_application::policy::PolicyService,
    limits: &'a SavedLimits,
    auth: &'a BTreeMap<String, bool>,
) -> SavedPolicy<'a> {
    SavedPolicy {
        search,
        saved_granted: true,
        auth_configured: auth,
        limits,
    }
}
async fn create(store: &SavedStore, name: &str) -> Claim {
    let p = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = saved_policy(&p, &limits, &auth);
    let prepared = create_at(
        &p,
        store.tenant().as_str(),
        name,
        store.now().await.unwrap(),
    );
    store.create(&prepared).await.unwrap();
    store
        .claim(&id(name), &mut Tokens("owner"))
        .await
        .unwrap()
        .unwrap()
}
async fn publish(store: &SavedStore, p: &EventPublisher, name: &str) -> (Value, Cursor) {
    let claim = create(store, name).await;
    let (report, baseline, next) = publication(&claim, store.now().await.unwrap());
    assert!(
        p.publish(
            store,
            &claim,
            &report,
            baseline.as_ref(),
            next,
            &claim.definition().policy_fingerprint
        )
        .await
        .unwrap()
    );
    let rows = store
        .read_events(&ConsumerId::new("probe").unwrap(), None, 100, p.limits)
        .await
        .unwrap();
    (report, rows.events.last().unwrap().0.clone())
}
fn complete(names: &[&str]) {
    for name in names {
        println!("W05_CASE_COMPLETE:{name}");
    }
}
#[test]
fn original_event_envelopes_and_validation_match_exact_source() {
    let source: Value =
        ssx_json::from_str(include_str!("../../../docs/rust-rewrite/w05/source.json")).unwrap();
    let input: Value =
        ssx_json::from_str(include_str!("../../../docs/rust-rewrite/w05/inputs.json")).unwrap();
    let d = SavedDefinition::from_payload(&input["definition"]).unwrap();
    for row in source["cases"].as_array().unwrap() {
        let name = row["name"].as_str().unwrap();
        let event = if name.starts_with("definition_") {
            Event::definition(
                &d,
                if name == "definition_paused" {
                    EventType::DefinitionPaused
                } else {
                    EventType::DefinitionExpired
                },
                1600.0,
                "event-0123456789abcdefghijklmn",
            )
            .unwrap()
        } else {
            Event::report(&d, &row["report"], "event-0123456789abcdefghijklmn").unwrap()
        };
        let mut actual = event.payload().clone();
        if !name.starts_with("definition_") {
            actual["_unknown_internal"] = json!("private");
        }
        assert_eq!(actual, row["stored"], "{name}");
        let event = Event::from_payload(actual).unwrap();
        let cursor = Cursor::new(if name.starts_with("definition_") {
            "1600000-0"
        } else {
            "1060500-0"
        })
        .unwrap();
        assert_eq!(event.public(&cursor, false), row["public"], "{name}");
        assert_eq!(event.public(&cursor, true), row["redacted"], "{name}");
        complete(&[name]);
    }
    for (index, row) in source["validation"].as_array().unwrap().iter().enumerate() {
        let raw = row["input"].as_str().unwrap();
        let actual = if row["kind"] == "cursor" {
            Cursor::new(raw).map(|v| v.as_str().to_owned())
        } else {
            ConsumerId::new(raw).map(|v| v.as_str().to_owned())
        };
        assert_eq!(
            actual.unwrap_or_else(|_| "invalid_input".into()),
            row["output"].as_str().unwrap()
        );
        complete(&[&format!("validation-{index}")]);
    }
    assert!(
        Cursor::new("9007199254740993-0").unwrap() > Cursor::new("9007199254740992-0").unwrap()
    );
    assert!(Event::definition(&d, EventType::ReportCreated, 1.0, "event-good").is_err());
    let mut large = json!({"event_id":"event-bounded","value":"é".repeat(12000)});
    assert!(ssx_json::to_vec(&large).unwrap().len() < MAX_EVENT_BYTES);
    assert!(Event::from_payload(large.clone()).is_err());
    large["value"] = json!("x".repeat(65497));
    assert!(Event::from_payload(large).is_ok());
    let a = event_id().unwrap();
    let b = event_id().unwrap();
    assert_ne!(a, b);
    assert_eq!(a.len(), 30);
    complete(&[
        "exact-decimal-comparison",
        "ascii-64k-bound",
        "opaque-event-id",
    ]);
}
#[tokio::test]
async fn ordered_read_ack_restart_tenant_capacity_and_revocation() {
    let server = Server::start();
    let store = SavedStore::new(
        Arc::new(server.store()),
        TenantId::new("tenant:alpha").unwrap(),
    );
    let (p, authority, recording) = publisher(EventLimits::new(10, 120, 2).unwrap());
    let api = service(store.clone(), p.clone());
    let (_, first) = publish(&store, &p, "saved-one").await;
    let (_, second) = publish(&store, &p, "saved-two").await;
    let batch = api.read("agent-a", None, 1).await;
    assert_eq!(batch["returned"], json!(1));
    assert_eq!(batch["acknowledged_cursor"], json!("0-0"));
    assert_eq!(batch["next_cursor"], json!(first.as_str()));
    assert_eq!(batch, api.read("agent-a", None, 1).await);
    assert_eq!(
        api.read("agent-a", Some(first.as_str()), 1).await["next_cursor"],
        json!(second.as_str())
    );
    let other = service(
        store.for_tenant(TenantId::new("tenant").unwrap()),
        p.clone(),
    );
    assert_eq!(other.read("agent-a", None, 10).await["events"], json!([]));
    assert_eq!(
        other.ack("agent-a", first.as_str()).await["error"]["code"],
        json!("cursor_expired")
    );
    let a = api.ack("agent-a", second.as_str()).await;
    let b = api.ack("agent-a", second.as_str()).await;
    assert_eq!(a, b);
    assert_eq!(
        api.ack("agent-a", first.as_str()).await["acknowledged_cursor"],
        json!(second.as_str())
    );
    let restarted = service(
        SavedStore::new(Arc::new(server.store()), store.tenant().clone()),
        p.clone(),
    );
    assert_eq!(
        restarted.read("agent-a", None, 10).await["events"],
        json!([])
    );
    assert_eq!(
        api.ack("agent-b", second.as_str()).await["acknowledged_cursor"],
        json!(second.as_str())
    );
    assert_eq!(
        api.ack("agent-c", second.as_str()).await["error"]["code"],
        json!("resource_limit")
    );
    authority.allowed.store(false, Ordering::SeqCst);
    let redacted = api.read("unacked", None, 10).await;
    let e = &redacted["events"][0];
    assert_eq!(e["policy_state"], json!("redacted"));
    assert_eq!(e["summary"], Value::Null);
    assert_eq!(e["coverage_state"], json!("redacted"));
    assert_eq!(e["reason_code"], json!("policy_redacted"));
    assert!(e.as_object().unwrap().keys().all(|k| !k.starts_with('_')));
    authority.allowed.store(true, Ordering::SeqCst);
    authority.saved.store(false, Ordering::SeqCst);
    assert_eq!(api.read("unacked", None, 10).await["redacted"], json!(2));
    authority.events.store(false, Ordering::SeqCst);
    assert_eq!(
        api.read("agent-a", None, 10).await["error"]["code"],
        json!("tool_disabled")
    );
    assert_eq!(
        api.ack("agent-a", second.as_str()).await["error"]["code"],
        json!("tool_disabled")
    );
    let report = publish(&store, &p, "saved-no-event").await.0;
    assert!(
        store
            .load(&id("saved-no-event"))
            .await
            .unwrap()
            .unwrap()
            .latest_run_id
            .is_some()
    );
    authority.events.store(true, Ordering::SeqCst);
    assert_eq!(api.read("unacked", None, 100).await["returned"], json!(2));
    assert_eq!(report["search_id"], json!("saved-no-event"));
    let observed = recording.0.lock().unwrap().clone();
    assert!(observed.contains(&EventObservation::Ack(AckOutcome::Advanced)));
    assert!(observed.contains(&EventObservation::Ack(AckOutcome::Idempotent)));
    assert!(observed.contains(&EventObservation::Capacity(Capacity::Consumer)));
    assert_eq!(
        observed
            .iter()
            .filter(|v| matches!(v, EventObservation::Published(_)))
            .count(),
        2
    );
    drop(restarted);
    drop(api);
    drop(other);
    drop(store);
    drop(server);
    complete(&[
        "ordered-bounded-repeat-no-ack",
        "ack-monotonic-idempotent-restart",
        "tenant-consumer-fences",
        "consumer-capacity",
        "policy-revocation-redaction",
        "separate-event-grant",
        "typed-definite-observations",
    ]);
}
fn get(connection: &mut redis::Connection, key: &str) -> Option<Vec<u8>> {
    redis::cmd("GET").arg(key).query(connection).unwrap()
}
fn event_key(tenant: &TenantId) -> String {
    keys::saved("events", tenant, None).unwrap()
}
fn xlen(connection: &mut redis::Connection, tenant: &TenantId) -> usize {
    redis::cmd("XLEN")
        .arg(event_key(tenant))
        .query(connection)
        .unwrap()
}
fn append(connection: &mut redis::Connection, tenant: &TenantId, cursor: &str, event: &Value) {
    let _: String = redis::cmd("XADD")
        .arg(event_key(tenant))
        .arg(cursor)
        .arg("payload")
        .arg(ssx_json::to_vec(event).unwrap())
        .query(connection)
        .unwrap();
}
#[tokio::test]
async fn competing_claims_fenced_atomic_publication_and_capacity_rejection() {
    let server = Server::start();
    let store = SavedStore::new(Arc::new(server.store()), TenantId::new("tenant").unwrap());
    let (p, _, _) = publisher(EventLimits::new(1, 600, 10).unwrap());
    let initial = create(&store, "saved-first").await;
    store.release(&initial).await.unwrap();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let store = store.clone();
        tasks.spawn(async move { store.claim_fresh(&id("saved-first")).await.unwrap() });
    }
    let mut claims = Vec::new();
    while let Some(c) = tasks.join_next().await {
        if let Some(c) = c.unwrap() {
            claims.push(c);
        }
    }
    assert_eq!(claims.len(), 1);
    let claim = claims.pop().unwrap();
    let (report, baseline, next) = publication(&claim, store.now().await.unwrap());
    let event = Event::report(claim.definition(), &report, &event_id().unwrap()).unwrap();
    let (a, b) = tokio::join!(
        store.commit_event(
            &claim,
            &report,
            baseline.as_ref(),
            next,
            &claim.definition().policy_fingerprint,
            Publication {
                event: &event,
                limits: p.limits
            }
        ),
        store.commit_event(
            &claim,
            &report,
            baseline.as_ref(),
            next,
            &claim.definition().policy_fingerprint,
            Publication {
                event: &event,
                limits: p.limits
            }
        )
    );
    assert_eq!(
        usize::from(a.unwrap().committed) + usize::from(b.unwrap().committed),
        1
    );
    let mut connection = server.connection();
    assert_eq!(xlen(&mut connection, store.tenant()), 1);
    let loaded = store.load(&id("saved-first")).await.unwrap().unwrap();
    assert_eq!(
        loaded.latest_run_id,
        report["run_id"].as_str().map(str::to_owned)
    );
    assert_eq!(loaded.baseline, baseline);
    let blocked = create(&store, "saved-second").await;
    let (report2, baseline2, next2) = publication(&blocked, store.now().await.unwrap());
    let definition_key = keys::saved("definition", store.tenant(), Some("saved-second")).unwrap();
    let before = get(&mut connection, &definition_key);
    let event = Event::report(blocked.definition(), &report2, &event_id().unwrap()).unwrap();
    assert_eq!(
        store
            .commit_event(
                &blocked,
                &report2,
                baseline2.as_ref(),
                next2,
                &blocked.definition().policy_fingerprint,
                Publication {
                    event: &event,
                    limits: p.limits
                }
            )
            .await
            .unwrap_err(),
        Error::OutboxCapacity
    );
    assert_eq!(before, get(&mut connection, &definition_key));
    assert_eq!(xlen(&mut connection, store.tenant()), 1);
    assert!(
        get(
            &mut connection,
            &keys::saved("baseline", store.tenant(), Some("saved-second")).unwrap()
        )
        .is_none()
    );
    assert!(
        get(
            &mut connection,
            &keys::saved(
                "report",
                store.tenant(),
                Some(&format!(
                    "saved-second:{}",
                    report2["run_id"].as_str().unwrap()
                ))
            )
            .unwrap()
        )
        .is_none()
    );
    assert!(store.renew(&blocked).await.unwrap());
    assert_eq!(
        store
            .load(&id("saved-first"))
            .await
            .unwrap()
            .unwrap()
            .baseline,
        baseline
    );
    let wrong_tenant = store.for_tenant(TenantId::new("other").unwrap());
    assert!(
        wrong_tenant
            .commit_event(
                &blocked,
                &report2,
                baseline2.as_ref(),
                next2,
                &blocked.definition().policy_fingerprint,
                Publication {
                    event: &event,
                    limits: p.limits
                }
            )
            .await
            .is_err()
    );
    // Source capacity rejection may trim expired stream entries, while every
    // report/baseline/definition/lease publication remains rejected atomically.
    let _: usize = redis::cmd("DEL")
        .arg(event_key(store.tenant()))
        .query(&mut connection)
        .unwrap();
    let now = store.now().await.unwrap();
    append(
        &mut connection,
        store.tenant(),
        &format!("{}-0", ((now - 1200.0) * 1000.0) as u64),
        event.payload(),
    );
    append(
        &mut connection,
        store.tenant(),
        &format!("{}-0", (now * 1000.0) as u64),
        event.payload(),
    );
    assert_eq!(xlen(&mut connection, store.tenant()), 2);
    assert_eq!(
        store
            .commit_event(
                &blocked,
                &report2,
                baseline2.as_ref(),
                next2,
                &blocked.definition().policy_fingerprint,
                Publication {
                    event: &event,
                    limits: p.limits
                }
            )
            .await
            .unwrap_err(),
        Error::OutboxCapacity
    );
    assert_eq!(xlen(&mut connection, store.tenant()), 1);
    assert_eq!(before, get(&mut connection, &definition_key));
    drop(connection);
    drop(store);
    drop(server);
    complete(&[
        "capacity-rejection-trim-only-side-effect",
        "competing-eight-claims-one-report-event",
        "duplicate-commit-fenced",
        "capacity-report-baseline-definition-event-atomic",
        "claim-tenant-fence",
    ]);
}
#[tokio::test]
async fn pause_expiry_atomic_idempotence_capacity_and_retained_horizons() {
    let server = Server::start();
    let store = SavedStore::new(Arc::new(server.store()), TenantId::new("tenant").unwrap());
    let (p, _, recording) = publisher(EventLimits::new(1, 120, 10).unwrap());
    let api = service(store.clone(), p.clone());
    let claim = create(&store, "saved-pause").await;
    store.release(&claim).await.unwrap();
    let current = store.load(&id("saved-pause")).await.unwrap().unwrap();
    let search = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let policy = saved_policy(&search, &limits, &auth);
    let paused = api
        .pause(&id("saved-pause"), 1, true, &policy)
        .await
        .unwrap();
    assert_eq!(paused.revision, 2);
    assert!(paused.paused);
    assert!(matches!(
        api.pause(&id("saved-pause"), 1, true, &policy).await,
        Err(Error::Conflict)
    ));
    let claim2 = create(&store, "saved-active").await;
    store.release(&claim2).await.unwrap();
    assert_eq!(
        api.pause(&id("saved-active"), 1, true, &policy)
            .await
            .unwrap_err(),
        Error::OutboxCapacity
    );
    let active = store.load(&id("saved-active")).await.unwrap().unwrap();
    assert!(!active.paused);
    assert_eq!(active.revision, 1);
    let mut connection = server.connection();
    let expiry_key = keys::saved("definition", store.tenant(), Some("saved-active")).unwrap();
    let mut raw = active.stored_payload().unwrap();
    let expiry = store.now().await.unwrap() - 1.0;
    raw["expires_at"] = json!(expiry);
    let _: () = redis::cmd("SET")
        .arg(&expiry_key)
        .arg(ssx_json::to_vec(&raw).unwrap())
        .arg("EX")
        .arg(300)
        .query(&mut connection)
        .unwrap();
    let result = store
        .publish_expired_event_page(0, 128, p.limits)
        .await
        .unwrap();
    assert_eq!(result.published, 0);
    assert_eq!(result.failure, Some(Error::OutboxCapacity));
    assert!(
        get(
            &mut connection,
            &keys::saved("event-expired", store.tenant(), Some("saved-active")).unwrap()
        )
        .is_none()
    );
    // Remove the event as a controlled fixture reset, then expiry claims publish once.
    let _: usize = redis::cmd("DEL")
        .arg(event_key(store.tenant()))
        .query(&mut connection)
        .unwrap();
    let result = store
        .publish_expired_event_page(0, 128, p.limits)
        .await
        .unwrap();
    assert!(result.failure.is_none());
    assert_eq!(result.published, 1);
    assert_eq!(
        store
            .publish_expired_event_page(0, 128, p.limits)
            .await
            .unwrap()
            .published,
        0
    );
    assert_eq!(xlen(&mut connection, store.tenant()), 1);
    let batch = api.read("agent", None, 10).await;
    assert_eq!(
        batch["events"][0]["event_type"],
        json!("definition_expired")
    );
    assert_eq!(batch["events"][0]["occurred_at"], json!(expiry));
    assert_eq!(current.expires_at, paused.expires_at);
    let observed = recording.0.lock().unwrap().clone();
    assert!(observed.contains(&EventObservation::Published(EventType::DefinitionPaused)));
    assert!(observed.contains(&EventObservation::Capacity(Capacity::Stream)));
    drop(connection);
    drop(api);
    drop(store);
    drop(server);
    complete(&[
        "pause-revision-idempotence",
        "pause-capacity-active-unchanged",
        "expiry-capacity-no-marker",
        "expiry-atomic-idempotence",
        "expiry-original-time-envelope",
    ]);
}
#[tokio::test]
async fn retention_trim_read_ack_empty_gap_and_exact_large_cursors() {
    let server = Server::start();
    let store = SavedStore::new(Arc::new(server.store()), TenantId::new("tenant").unwrap());
    let (p, authority, recording) = publisher(EventLimits::new(10, 120, 10).unwrap());
    let api = service(store.clone(), p.clone());
    let now = store.now().await.unwrap();
    let stale = format!("{}-0", ((now - 300.0) * 1000.0) as u64);
    let live = format!("{}-0", ((now - 60.0) * 1000.0) as u64);
    let mut connection = server.connection();
    let event = json!({"event_id":"event-retention","occurred_at":now-60.0,"summary":{"private":true},"coverage_state":"complete","reason_code":"none","_policy_engines":["brave"]});
    append(&mut connection, store.tenant(), &stale, &event);
    append(&mut connection, store.tenant(), &live, &event);
    let batch = api.read("agent", Some(&stale), 10).await;
    assert_eq!(batch["returned"], json!(1));
    assert_eq!(
        batch["gap"],
        json!({"detected":true,"reason":"retention_expired","first_available_cursor":live})
    );
    assert_eq!(xlen(&mut connection, store.tenant()), 1);
    assert_eq!(
        api.ack("agent", &stale).await["error"]["code"],
        json!("cursor_expired")
    );
    // Independent fresh stream: ack performs its own trim without prior read.
    let other = store.for_tenant(TenantId::new("other").unwrap());
    append(&mut connection, other.tenant(), &stale, &event);
    append(&mut connection, other.tenant(), &live, &event);
    let other_api = service(other.clone(), p.clone());
    assert_eq!(
        other_api.ack("agent", &stale).await["error"]["code"],
        json!("cursor_expired")
    );
    assert_eq!(xlen(&mut connection, other.tenant()), 1);
    let enormous = format!("{}-{}", "9".repeat(31), "9".repeat(32));
    assert_eq!(enormous.len(), 64);
    assert_eq!(
        api.read("agent", Some(&enormous), 10).await["events"],
        json!([])
    );
    assert_eq!(
        api.ack("agent", &enormous).await["error"]["code"],
        json!("invalid_input")
    );
    assert_eq!(
        api.read(
            "agent",
            Some("0-99999999999999999999999999999999999999999999999999999999999999"),
            10
        )
        .await["returned"],
        json!(1)
    );
    authority.allowed.store(false, Ordering::SeqCst);
    let gap_redacted = api.read("agent", Some(&stale), 10).await;
    assert_eq!(gap_redacted["redacted"], json!(1));
    assert!(recording.0.lock().unwrap().iter().any(|e| matches!(
        e,
        EventObservation::Read {
            outcome: ReadOutcome::Gap,
            ..
        }
    )));
    let _: usize = redis::cmd("DEL")
        .arg(event_key(store.tenant()))
        .query(&mut connection)
        .unwrap();
    let empty = api.read("agent", Some(&live), 10).await;
    assert_eq!(
        empty["gap"],
        json!({"detected":true,"reason":"retention_expired","first_available_cursor":null})
    );
    assert_eq!(empty["next_cursor"], json!(live));
    assert_eq!(
        api.ack("agent", &live).await["error"]["code"],
        json!("cursor_expired")
    );
    drop(connection);
    drop(api);
    drop(other_api);
    drop(store);
    drop(other);
    drop(server);
    complete(&[
        "retention-gap-independent-of-ack",
        "read-trims-without-publisher",
        "ack-trims-without-read",
        "empty-stream-gap-expiry",
        "canonical-64-digit-future-read-ack",
        "oversize-sequence-clamped",
        "gap-before-redaction-observation",
    ]);
}
#[tokio::test]
async fn invalid_inputs_and_wrong_types_preflight_without_partial_publication() {
    let server = Server::start();
    let store = SavedStore::new(Arc::new(server.store()), TenantId::new("tenant").unwrap());
    let (p, _, _) = publisher(EventLimits::default());
    let api = service(store.clone(), p.clone());
    for (index, (consumer, cursor)) in [
        ("".into(), None),
        ("has space".into(), None),
        ("a".repeat(129), None),
        ("agent".into(), Some("bad".into())),
        ("agent".into(), Some("01-0".into())),
        ("agent".into(), Some(format!("{}-0", "1".repeat(65)))),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            api.read(&consumer, cursor.as_deref(), 10).await["error"]["code"],
            json!("invalid_input")
        );
        let count: usize = redis::cmd("DBSIZE")
            .query(&mut server.connection())
            .unwrap();
        assert_eq!(count, 0);
        let _ = index;
    }
    let claim = create(&store, "saved-wrong").await;
    let (report, baseline, next) = publication(&claim, store.now().await.unwrap());
    let event = Event::report(claim.definition(), &report, &event_id().unwrap()).unwrap();
    let mut connection = server.connection();
    let def = keys::saved("definition", store.tenant(), Some("saved-wrong")).unwrap();
    let before = get(&mut connection, &def);
    let stream = event_key(store.tenant());
    let _: () = redis::cmd("SET")
        .arg(&stream)
        .arg("wrong-type")
        .query(&mut connection)
        .unwrap();
    assert_eq!(
        store
            .commit_event(
                &claim,
                &report,
                baseline.as_ref(),
                next,
                &claim.definition().policy_fingerprint,
                Publication {
                    event: &event,
                    limits: p.limits
                }
            )
            .await
            .unwrap_err(),
        Error::Malformed
    );
    assert_eq!(before, get(&mut connection, &def));
    assert_eq!(get(&mut connection, &stream), Some(b"wrong-type".to_vec()));
    assert!(
        get(
            &mut connection,
            &keys::saved("baseline", store.tenant(), Some("saved-wrong")).unwrap()
        )
        .is_none()
    );
    assert!(store.renew(&claim).await.unwrap());
    let _: usize = redis::cmd("DEL")
        .arg(&stream)
        .query(&mut connection)
        .unwrap();
    let max = "18446744073709551615-18446744073709551615";
    append(
        &mut connection,
        store.tenant(),
        max,
        &json!({"event_id":"event-extreme"}),
    );
    assert_eq!(
        store
            .commit_event(
                &claim,
                &report,
                baseline.as_ref(),
                next,
                &claim.definition().policy_fingerprint,
                Publication {
                    event: &event,
                    limits: p.limits
                }
            )
            .await
            .unwrap_err(),
        Error::Malformed
    );
    assert_eq!(before, get(&mut connection, &def));
    assert_eq!(xlen(&mut connection, store.tenant()), 1);
    let _: usize = redis::cmd("DEL")
        .arg(&stream)
        .query(&mut connection)
        .unwrap();
    let search = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let policy = saved_policy(&search, &limits, &auth);
    store
        .pause(&id("saved-wrong"), 1, true, &policy)
        .await
        .unwrap();
    assert!(
        !store
            .commit_event(
                &claim,
                &report,
                baseline.as_ref(),
                next,
                &claim.definition().policy_fingerprint,
                Publication {
                    event: &event,
                    limits: p.limits
                }
            )
            .await
            .unwrap()
            .committed
    );
    assert_eq!(xlen(&mut connection, store.tenant()), 0);
    let (_, _, observer) = publisher(EventLimits::default());
    let closed = Arc::new(server.store());
    let closed_store = SavedStore::new(closed.clone(), store.tenant().clone());
    closed.close().await;
    let publication = EventPublisher {
        limits: p.limits,
        authority: p.authority.clone(),
        observer: observer.clone(),
    };
    assert!(
        publication
            .publish(
                &closed_store,
                &claim,
                &report,
                baseline.as_ref(),
                next,
                &claim.definition().policy_fingerprint
            )
            .await
            .is_err()
    );
    assert!(
        observer
            .0
            .lock()
            .unwrap()
            .iter()
            .all(|v| !matches!(v, EventObservation::Published(_)))
    );
    drop(connection);
    drop(api);
    drop(store);
    drop(server);
    for index in 0..6 {
        complete(&[&format!("invalid-no-mutation-{index}")]);
    }
    complete(&[
        "wrong-type-preflight-no-publication",
        "extreme-stream-id-preflight-no-publication",
        "event-enabled-stale-claim-fenced",
        "unconfirmed-publication-no-success-observation",
    ]);
}
#[tokio::test]
async fn source_precision_clock_and_stream_rollover_boundaries() {
    let server = Server::start();
    let store = SavedStore::new(Arc::new(server.store()), TenantId::new("tenant").unwrap());
    let (p, _, recording) = publisher(EventLimits::new(10, 120, 10).unwrap());
    let api = service(store.clone(), p.clone());
    let now = store.now().await.unwrap();
    let ms = ((now - 1.0) * 1000.0) as u64;
    let first = format!("{ms}-9007199254740992");
    let second = format!("{ms}-9007199254740993");
    let mut connection = server.connection();
    let payload =
        json!({"event_id":"event-precision","occurred_at":now+10000.0,"_policy_engines":["brave"]});
    append(&mut connection, store.tenant(), &first, &payload);
    append(&mut connection, store.tenant(), &second, &payload);
    assert_eq!(
        api.ack("agent", &first).await["acknowledged_cursor"],
        json!(first)
    );
    assert_eq!(
        api.ack("agent", &second).await["acknowledged_cursor"],
        json!(second)
    );
    let count = recording
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|e| matches!(e, EventObservation::Ack(AckOutcome::Advanced)))
        .count();
    assert_eq!(count, 2);
    // Execute the accepted F07 byte-exact original Lua assets on independent
    // private keys: this characterizes the reviewed source differences directly.
    let historical = server.store();
    let original = store.for_tenant(TenantId::new("original-clock").unwrap());
    append(&mut connection, original.tenant(), &first, &payload);
    append(&mut connection, original.tenant(), &second, &payload);
    let ack_key = keys::saved("event-ack", original.tenant(), Some("agent")).unwrap();
    let consumers = keys::saved("event-consumers", original.tenant(), None).unwrap();
    let stream_key = event_key(original.tenant());
    let original_args = |cursor: &str| {
        vec![
            now.to_string(),
            cursor.to_owned(),
            "420".into(),
            (now + 420.0).to_string(),
            "agent".into(),
            "10".into(),
            "0-0".into(),
        ]
    };
    assert_eq!(
        historical
            .run(
                scripts::Script::SavedAckEvent,
                &[&ack_key, &consumers, &stream_key],
                &original_args(&first)
            )
            .await
            .unwrap(),
        scripts::Reply::SavedAck(scripts::SavedAck::Advanced)
    );
    assert_eq!(
        historical
            .run(
                scripts::Script::SavedAckEvent,
                &[&ack_key, &consumers, &stream_key],
                &original_args(&second)
            )
            .await
            .unwrap(),
        scripts::Reply::SavedAck(scripts::SavedAck::Unchanged)
    );
    let cutoff = format!("{}-0", ((now + 10000.0 - 120.0) * 1000.0) as u64);
    let result = historical
        .run(
            scripts::Script::SavedReadEvents,
            &[&stream_key],
            &[cutoff, "0-0".into(), "10".into()],
        )
        .await
        .unwrap();
    assert!(matches!(result,scripts::Reply::SavedEvents{entries,..} if entries.is_empty()));
    assert_eq!(xlen(&mut connection, original.tenant()), 0);
    // Event occurred_at does not control stream retention. Backward/forward event
    // timestamps affect backlog age only, clamped to a validated nonnegative age.
    let batch = api.read("other", None, 100).await;
    assert_eq!(batch["returned"], json!(2));
    let age = recording
        .0
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find_map(|e| {
            if let EventObservation::Read {
                oldest_age_seconds: Some(age),
                ..
            } = e
            {
                Some(age.seconds())
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(age, 0.0);
    assert!(BacklogAge::new(f64::NAN).is_err());
    assert!(BacklogAge::new(f64::INFINITY).is_err());
    assert!(BacklogAge::new(-1.0).is_err());
    let _: usize = redis::cmd("DEL")
        .arg(event_key(store.tenant()))
        .query(&mut connection)
        .unwrap();
    append(
        &mut connection,
        store.tenant(),
        "1-18446744073709551615",
        &payload,
    );
    let claim = create(&store, "saved-rollover").await;
    let (report, baseline, next) = publication(&claim, store.now().await.unwrap());
    let event = Event::report(claim.definition(), &report, &event_id().unwrap()).unwrap();
    let result = store
        .commit_event(
            &claim,
            &report,
            baseline.as_ref(),
            next,
            &claim.definition().policy_fingerprint,
            Publication {
                event: &event,
                limits: p.limits,
            },
        )
        .await
        .unwrap();
    assert!(result.committed);
    let cursor = result.cursor.unwrap();
    assert!(cursor > Cursor::new("1-18446744073709551615").unwrap());
    let ttl: i64 = redis::cmd("TTL")
        .arg(event_key(store.tenant()))
        .query(&mut connection)
        .unwrap();
    assert!((419..=420).contains(&ttl));
    assert_eq!(xlen(&mut connection, store.tenant()), 1);
    drop(connection);
    drop(api);
    drop(store);
    drop(server);
    complete(&[
        "u53-adjacent-ack-cursors-exact",
        "original-lua-precision-difference-characterized",
        "original-caller-clock-jump-trims-native-retains",
        "authoritative-stream-time-not-event-clock",
        "validated-nonnegative-backlog-age",
        "old-ms-max-sequence-valid-rollover",
        "event-retention-plus-margin-ttl",
    ]);
}
#[tokio::test]
async fn acknowledgement_v2_migration_collision_quarantine_and_recovery() {
    let server = Server::start();
    let backend = Arc::new(server.store());
    let plain = SavedStore::new(backend.clone(), TenantId::new("plain").unwrap());
    let now = plain.now().await.unwrap();
    let first = format!("{}-0", ((now - 2.0) * 1000.0) as u64);
    let tail = format!("{}-0", ((now - 1.0) * 1000.0) as u64);
    let mut connection = server.connection();
    let payload =
        json!({"event_id":"event-migration","occurred_at":now,"_policy_engines":["brave"]});
    let consumer = ConsumerId::new("agent").unwrap();
    let (p, _, _) = publisher(EventLimits::new(10, 120, 1).unwrap());
    let plain_api = service(plain.clone(), p.clone());
    append(&mut connection, plain.tenant(), &first, &payload);
    append(&mut connection, plain.tenant(), &tail, &payload);
    let old = keys::saved("event-ack", plain.tenant(), Some(consumer.as_str())).unwrap();
    let new = acknowledgement_key(plain.tenant(), &consumer);
    assert!(legacy_acknowledgement_unambiguous(
        plain.tenant(),
        &consumer
    ));
    let _: () = redis::cmd("SET")
        .arg(&old)
        .arg(&tail)
        .arg("EX")
        .arg(200)
        .query(&mut connection)
        .unwrap();
    let _: usize = redis::cmd("ZADD")
        .arg(keys::saved("event-consumers", plain.tenant(), None).unwrap())
        .arg(now + 420.0)
        .arg(consumer.as_str())
        .query(&mut connection)
        .unwrap();
    let batch = plain_api.read("agent", None, 10).await;
    assert_eq!(batch["acknowledged_cursor"], json!(tail));
    assert_eq!(batch["events"], json!([]));
    assert!(get(&mut connection, &new).is_none());
    assert_eq!(get(&mut connection, &old), Some(tail.as_bytes().to_vec()));
    let migrated = plain_api.ack("agent", &first).await;
    assert_eq!(migrated["acknowledged_cursor"], json!(tail));
    assert_eq!(get(&mut connection, &new), Some(tail.as_bytes().to_vec()));
    assert_eq!(get(&mut connection, &old), Some(tail.as_bytes().to_vec()));
    assert_eq!(
        plain_api.ack("other", &tail).await["error"]["code"],
        json!("resource_limit")
    );
    let restarted = service(
        SavedStore::new(Arc::new(server.store()), plain.tenant().clone()),
        p.clone(),
    );
    assert_eq!(
        restarted.read("agent", None, 10).await["acknowledged_cursor"],
        json!(tail)
    );
    // Malformed v2 authority is never bypassed, even by explicit replay.
    let _: () = redis::cmd("SET")
        .arg(&new)
        .arg("01-0")
        .query(&mut connection)
        .unwrap();
    assert_eq!(
        plain_api.read("agent", Some("0-0"), 10).await["error"]["code"],
        json!("store_unavailable")
    );
    assert_eq!(
        plain_api.ack("agent", &tail).await["error"]["code"],
        json!("store_unavailable")
    );
    assert_eq!(get(&mut connection, &new), Some(b"01-0".to_vec()));
    let _: usize = redis::cmd("DEL").arg(&new).query(&mut connection).unwrap();
    let _: () = redis::cmd("SET")
        .arg(&old)
        .arg("01-0")
        .query(&mut connection)
        .unwrap();
    assert_eq!(
        plain_api.read("agent", Some("0-0"), 10).await["error"]["code"],
        json!("store_unavailable")
    );
    assert_eq!(
        plain_api.ack("agent", &tail).await["error"]["code"],
        json!("store_unavailable")
    );
    assert!(get(&mut connection, &new).is_none());
    let a = plain.for_tenant(TenantId::new("a").unwrap());
    let b = plain.for_tenant(TenantId::new("a:b").unwrap());
    let ca = ConsumerId::new("b:c").unwrap();
    let cb = ConsumerId::new("c").unwrap();
    let old_a = keys::saved("event-ack", a.tenant(), Some(ca.as_str())).unwrap();
    let old_b = keys::saved("event-ack", b.tenant(), Some(cb.as_str())).unwrap();
    assert_eq!(old_a, old_b);
    assert_ne!(
        acknowledgement_key(a.tenant(), &ca),
        acknowledgement_key(b.tenant(), &cb)
    );
    assert!(!legacy_acknowledgement_unambiguous(a.tenant(), &ca));
    assert!(!legacy_acknowledgement_unambiguous(b.tenant(), &cb));
    append(&mut connection, a.tenant(), &tail, &payload);
    append(&mut connection, b.tenant(), &first, &payload);
    let _: () = redis::cmd("SET")
        .arg(&old_a)
        .arg(&tail)
        .arg("EX")
        .arg(200)
        .query(&mut connection)
        .unwrap();
    for (store, id) in [(&a, &ca), (&b, &cb)] {
        let _: usize = redis::cmd("ZADD")
            .arg(keys::saved("event-consumers", store.tenant(), None).unwrap())
            .arg(now + 420.0)
            .arg(id.as_str())
            .query(&mut connection)
            .unwrap();
    }
    let aa = service(a.clone(), p.clone());
    let bb = service(b.clone(), p.clone());
    assert_eq!(
        aa.read("b:c", None, 10).await["error"]["code"],
        json!("cursor_conflict")
    );
    assert_eq!(
        bb.read("c", None, 10).await["error"]["code"],
        json!("cursor_conflict")
    );
    assert!(get(&mut connection, &acknowledgement_key(a.tenant(), &ca)).is_none());
    assert!(get(&mut connection, &acknowledgement_key(b.tenant(), &cb)).is_none());
    assert_eq!(get(&mut connection, &old_a), Some(tail.as_bytes().to_vec()));
    let replay = bb.read("c", Some("0-0"), 10).await;
    assert_eq!(replay["returned"], json!(1));
    assert_eq!(replay["acknowledged_cursor"], json!("0-0"));
    assert_eq!(
        replay["acknowledgement_recovery"],
        json!({"state":"legacy_ambiguous_ignored","action":"acknowledge_a_retained_event"})
    );
    assert!(get(&mut connection, &acknowledgement_key(b.tenant(), &cb)).is_none());
    assert_eq!(
        bb.ack("c", &first).await["acknowledged_cursor"],
        json!(first)
    );
    assert_eq!(bb.read("c", None, 10).await["events"], json!([]));
    assert_eq!(
        aa.read("b:c", None, 10).await["error"]["code"],
        json!("cursor_conflict")
    );
    assert_eq!(
        aa.ack("b:c", &tail).await["acknowledged_cursor"],
        json!(tail)
    );
    assert_eq!(aa.read("b:c", None, 10).await["events"], json!([]));
    assert_eq!(get(&mut connection, &old_a), Some(tail.as_bytes().to_vec()));
    let restart_a = service(
        SavedStore::new(Arc::new(server.store()), a.tenant().clone()),
        p.clone(),
    );
    let restart_b = service(
        SavedStore::new(Arc::new(server.store()), b.tenant().clone()),
        p.clone(),
    );
    assert_eq!(
        restart_a.read("b:c", None, 10).await["acknowledged_cursor"],
        json!(tail)
    );
    assert_eq!(
        restart_b.read("c", None, 10).await["acknowledged_cursor"],
        json!(first)
    );
    let missing = service(
        plain.for_tenant(TenantId::new("missing:legacy").unwrap()),
        p.clone(),
    );
    assert_eq!(
        missing.read("ambiguous:id", None, 10).await["acknowledged_cursor"],
        json!("0-0")
    );
    let mut unique = std::collections::BTreeSet::new();
    for tenant in ["a", "a:b", "a:b:c", "é", "é:a"] {
        for consumer in ["c", "b:c", "a:b:c", "agent"] {
            let tenant = TenantId::new(tenant).unwrap();
            let consumer = ConsumerId::new(consumer).unwrap();
            assert!(unique.insert(acknowledgement_key(&tenant, &consumer)));
        }
    }
    assert_eq!(
        acknowledgement_key(
            &TenantId::new("é").unwrap(),
            &ConsumerId::new("agent").unwrap()
        ),
        "mcp:saved:v2:event-ack:2:é:agent"
    );
    drop(connection);
    drop(plain_api);
    drop(restarted);
    drop(aa);
    drop(bb);
    drop(restart_a);
    drop(restart_b);
    drop(missing);
    drop(plain);
    drop(a);
    drop(b);
    drop(backend);
    drop(server);
    complete(&[
        "legacy-unambiguous-read-no-ack",
        "legacy-monotonic-idempotent-v2-migration",
        "legacy-capacity-slot-reused",
        "malformed-native-ack-explicit-replay-fails",
        "malformed-unambiguous-legacy-fails",
        "legacy-ambiguous-default-quarantined",
        "legacy-explicit-replay-no-ack",
        "legacy-explicit-ack-resolves-only-own-tenant",
        "v2-colliding-owner-restart-isolated",
        "legacy-state-byte-exact-preserved",
        "absent-ambiguous-legacy-normal-zero",
        "v2-utf8-length-key-uniqueness",
    ]);
}
struct CountedSchedulerEngine(std::sync::atomic::AtomicUsize);
impl SearchEngine for CountedSchedulerEngine {
    fn search<'a>(&'a self, _: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            self.0.fetch_add(1, Ordering::SeqCst);
            AdapterResponse {
                results: vec![SearchResult {
                    url: "https://fixture.test/scheduled".into(),
                    engine: "brave".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }
        })
    }
}
#[tokio::test]
async fn scheduler_expiry_capacity_rejects_poll_before_due_and_recovers_same_runner() {
    let server = Server::start();
    let store = SavedStore::new(Arc::new(server.store()), TenantId::new("tenant").unwrap());
    let engine = Arc::new(CountedSchedulerEngine(0.into()));
    let deps = scheduler_support::dependencies(vec![("brave", engine.clone())]);
    let search_policy = deps.policy.clone();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let policy = saved_policy(&search_policy, &limits, &auth);
    let now = store.now().await.unwrap();
    for name in ["saved-expired", "saved-due"] {
        store
            .create(&create_at(&policy, "tenant", name, now))
            .await
            .unwrap();
    }
    let mut connection = server.connection();
    let expired_key = keys::saved("definition", store.tenant(), Some("saved-expired")).unwrap();
    let mut expired: Value =
        ssx_json::from_slice(&get(&mut connection, &expired_key).unwrap()).unwrap();
    expired["expires_at"] = json!(now - 1.0);
    let _: () = redis::cmd("SET")
        .arg(&expired_key)
        .arg(ssx_json::to_vec(&expired).unwrap())
        .arg("EX")
        .arg(300)
        .query(&mut connection)
        .unwrap();
    let payload = json!({"event_id":"event-full","occurred_at":now});
    for seq in [0, 1] {
        append(
            &mut connection,
            store.tenant(),
            &format!("{}-{seq}", ((now - 1.0) * 1000.0) as u64),
            &payload,
        );
    }
    let search = SearchService::new(SearchConfig::default(), deps).unwrap();
    let (p, _, recording) = publisher(EventLimits::new(2, 120, 10).unwrap());
    let authority = Arc::new(StaticSavedAuthority {
        search: search_policy,
        saved_granted: true,
        auth_configured: auth,
        limits,
    });
    let runner = SavedRunner::new(
        store.clone(),
        search.clone(),
        authority,
        Duration::from_secs(2),
    )
    .unwrap()
    .with_events(p.clone());
    let (stop, rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(async move { runner.run(Duration::from_millis(10), rx).await });
    let until = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let failures = recording
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|e| matches!(e, EventObservation::Capacity(Capacity::Stream)))
            .count();
        if failures >= 2 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < until,
            "no actual rejected expiry polls observed"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(
        engine.0.load(Ordering::SeqCst),
        0,
        "expiry rejection must prevent due acquisition for that poll"
    );
    assert!(
        store
            .load(&id("saved-due"))
            .await
            .unwrap()
            .unwrap()
            .latest_run_id
            .is_none()
    );
    let _: usize = redis::cmd("DEL")
        .arg(event_key(store.tenant()))
        .query(&mut connection)
        .unwrap();
    let until = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        let expired = recording
            .0
            .lock()
            .unwrap()
            .contains(&EventObservation::Published(EventType::DefinitionExpired));
        let reported = store
            .load(&id("saved-due"))
            .await
            .unwrap()
            .unwrap()
            .latest_run_id
            .is_some();
        if expired && reported {
            break;
        }
        assert!(
            tokio::time::Instant::now() < until,
            "same joined runner did not recover expiry and due publication"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    stop.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(engine.0.load(Ordering::SeqCst), 1);
    assert_eq!(xlen(&mut connection, store.tenant()), 2);
    let batch = store
        .read_events(&ConsumerId::new("observer").unwrap(), None, 10, p.limits)
        .await
        .unwrap();
    assert!(
        batch
            .events
            .iter()
            .any(|(_, e)| e.kind() == Some(EventType::DefinitionExpired))
    );
    assert!(
        batch
            .events
            .iter()
            .any(|(_, e)| e.kind() == Some(EventType::ReportCreated))
    );
    search.shutdown().await;
    drop(connection);
    drop(store);
    drop(server);
    complete(&[
        "scheduler-expiry-capacity-observed-before-no-dispatch",
        "scheduler-same-runner-capacity-recovery",
        "scheduler-recovery-expiry-report-pair",
        "scheduler-explicit-joined-shutdown",
    ]);
}

#[derive(Default)]
struct SharedCounters {
    names: Mutex<Vec<&'static str>>,
    probe: Option<CommittedMetricProbe>,
}
struct CommittedMetricProbe {
    url: String,
    definition: String,
    report: String,
    baseline: String,
    stream: String,
    ack: String,
    run_id: String,
    ack_cursor: Mutex<Option<String>>,
}
impl ssx_application::Metrics for SharedCounters {
    fn increment(&self, name: &'static str) {
        if let Some(probe) = &self.probe {
            // The only network surface is this fixture's private Unix socket;
            // validate it before creating the actual Redis connection.
            let path = std::path::Path::new(probe.url.strip_prefix("unix://").unwrap());
            assert!(path.is_absolute());
            assert_eq!(path.file_name().unwrap(), "s.sock");
            assert!(
                path.parent()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("ssx-w05-")
            );
            let mut connection = redis::Client::open(probe.url.as_str())
                .unwrap()
                .get_connection()
                .unwrap();
            if name == "saved_event_published_report_created" {
                let definition: Value =
                    ssx_json::from_slice(&get(&mut connection, &probe.definition).unwrap())
                        .unwrap();
                assert_eq!(
                    definition["latest_run_id"].as_str(),
                    Some(probe.run_id.as_str()),
                    "shared success counter preceded committed definition"
                );
                let exists: usize = redis::cmd("EXISTS")
                    .arg(&probe.report)
                    .arg(&probe.baseline)
                    .query(&mut connection)
                    .unwrap();
                assert_eq!(
                    exists, 2,
                    "shared success counter preceded committed report/baseline"
                );
                let len: usize = redis::cmd("XLEN")
                    .arg(&probe.stream)
                    .query(&mut connection)
                    .unwrap();
                assert_eq!(
                    len, 1,
                    "shared success counter preceded atomic stream publication"
                );
            }
            if matches!(
                name,
                "saved_event_ack_advanced" | "saved_event_ack_idempotent"
            ) {
                let stored: Option<String> = redis::cmd("GET")
                    .arg(&probe.ack)
                    .query(&mut connection)
                    .unwrap();
                assert_eq!(
                    &stored,
                    &*probe.ack_cursor.lock().unwrap(),
                    "shared ack counter preceded atomic acknowledgement authority"
                );
                assert!(stored.is_some());
            }
        }
        self.names.lock().unwrap().push(name);
    }
}
#[tokio::test]
async fn shared_metrics_bridge_observes_only_definite_commits_and_acknowledgements() {
    let cases = vec![
        (
            EventObservation::Published(EventType::ReportCreated),
            "saved_event_published_report_created",
        ),
        (
            EventObservation::Published(EventType::RunIncomparable),
            "saved_event_published_run_incomparable",
        ),
        (
            EventObservation::Published(EventType::RunFailed),
            "saved_event_published_run_failed",
        ),
        (
            EventObservation::Published(EventType::DefinitionPaused),
            "saved_event_published_definition_paused",
        ),
        (
            EventObservation::Published(EventType::DefinitionExpired),
            "saved_event_published_definition_expired",
        ),
        (
            EventObservation::Read {
                outcome: ReadOutcome::Delivered,
                oldest_age_seconds: None,
            },
            "saved_event_read_delivered",
        ),
        (
            EventObservation::Read {
                outcome: ReadOutcome::Empty,
                oldest_age_seconds: None,
            },
            "saved_event_read_empty",
        ),
        (
            EventObservation::Read {
                outcome: ReadOutcome::Gap,
                oldest_age_seconds: Some(BacklogAge::new(12.5).unwrap()),
            },
            "saved_event_read_gap",
        ),
        (
            EventObservation::Read {
                outcome: ReadOutcome::Redacted,
                oldest_age_seconds: None,
            },
            "saved_event_read_redacted",
        ),
        (
            EventObservation::Read {
                outcome: ReadOutcome::Rejected,
                oldest_age_seconds: None,
            },
            "saved_event_read_rejected",
        ),
        (
            EventObservation::Ack(AckOutcome::Advanced),
            "saved_event_ack_advanced",
        ),
        (
            EventObservation::Ack(AckOutcome::Idempotent),
            "saved_event_ack_idempotent",
        ),
        (
            EventObservation::Ack(AckOutcome::Rejected),
            "saved_event_ack_rejected",
        ),
        (
            EventObservation::Capacity(Capacity::Stream),
            "saved_event_capacity_stream",
        ),
        (
            EventObservation::Capacity(Capacity::Consumer),
            "saved_event_capacity_consumer",
        ),
    ];
    let finite = Arc::new(SharedCounters::default());
    let forwarded = Arc::new(Recording::default());
    let bridge = SharedEventObserver::new(finite.clone(), forwarded.clone());
    for (event, _) in &cases {
        bridge.observe(event.clone());
    }
    let names = cases.iter().map(|(_, name)| *name).collect::<Vec<_>>();
    assert_eq!(*finite.names.lock().unwrap(), names);
    assert_eq!(
        names
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        15
    );
    assert_eq!(
        *forwarded.0.lock().unwrap(),
        cases
            .iter()
            .map(|(event, _)| event.clone())
            .collect::<Vec<_>>()
    );

    let server = Server::start();
    let store = SavedStore::new(
        Arc::new(server.store()),
        TenantId::new("tenant-private-metric").unwrap(),
    );
    let claim = create(&store, "saved-private-metric").await;
    let (report, baseline, next) = publication(&claim, store.now().await.unwrap());
    assert!(baseline.is_some());
    let consumer = ConsumerId::new("consumer-private-metric").unwrap();
    let metrics = Arc::new(SharedCounters {
        probe: Some(CommittedMetricProbe {
            url: server.url.clone(),
            definition: keys::saved("definition", store.tenant(), Some("saved-private-metric"))
                .unwrap(),
            report: keys::saved(
                "report",
                store.tenant(),
                Some(&format!(
                    "saved-private-metric:{}",
                    report["run_id"].as_str().unwrap()
                )),
            )
            .unwrap(),
            baseline: keys::saved("baseline", store.tenant(), Some("saved-private-metric"))
                .unwrap(),
            stream: event_key(store.tenant()),
            ack: acknowledgement_key(store.tenant(), &consumer),
            run_id: report["run_id"].as_str().unwrap().into(),
            ack_cursor: Mutex::new(None),
        }),
        ..Default::default()
    });
    let typed = Arc::new(Recording::default());
    let p = EventPublisher {
        limits: EventLimits::new(1, 600, 1).unwrap(),
        authority: Arc::new(Authority::default()),
        observer: Arc::new(SharedEventObserver::new(metrics.clone(), typed.clone())),
    };
    assert!(metrics.names.lock().unwrap().is_empty());
    assert!(
        p.publish(
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
    assert_eq!(
        *metrics.names.lock().unwrap(),
        vec!["saved_event_published_report_created"]
    );
    assert!(
        !p.publish(
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
    let api = service(store.clone(), p.clone());
    let batch = api.read(consumer.as_str(), None, 1).await;
    assert_eq!(batch["returned"], 1);
    let cursor = batch["next_cursor"].as_str().unwrap();
    *metrics.probe.as_ref().unwrap().ack_cursor.lock().unwrap() = Some(cursor.into());
    assert_eq!(
        api.ack(consumer.as_str(), cursor).await["acknowledged_cursor"],
        cursor
    );
    assert_eq!(
        api.ack(consumer.as_str(), cursor).await["acknowledged_cursor"],
        cursor
    );
    assert_eq!(
        api.ack(consumer.as_str(), "bad").await["error"]["code"],
        "invalid_input"
    );
    assert_eq!(
        api.ack("other-private-metric", cursor).await["error"]["code"],
        "resource_limit"
    );
    let blocked = create(&store, "saved-capacity-metric").await;
    let (report2, baseline2, next2) = publication(&blocked, store.now().await.unwrap());
    assert_eq!(
        p.publish(
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
        p.publish(
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
    let observed = metrics.names.lock().unwrap().clone();
    let count = |name| observed.iter().filter(|n| **n == name).count();
    assert_eq!(count("saved_event_published_report_created"), 1);
    assert_eq!(count("saved_event_ack_advanced"), 1);
    assert_eq!(count("saved_event_ack_idempotent"), 1);
    assert_eq!(count("saved_event_ack_rejected"), 2);
    assert_eq!(count("saved_event_capacity_consumer"), 1);
    assert_eq!(count("saved_event_capacity_stream"), 1);
    assert!(observed.iter().all(|name| names.contains(name)));
    for secret in [
        store.tenant().as_str(),
        consumer.as_str(),
        "saved-private-metric",
        "fixture-subject",
        "alpha",
        report["run_id"].as_str().unwrap(),
    ] {
        assert!(observed.iter().all(|name| !name.contains(secret)));
    }
    let mut connection = server.connection();
    assert_eq!(xlen(&mut connection, store.tenant()), 1);
    assert_eq!(
        get(&mut connection, &metrics.probe.as_ref().unwrap().ack),
        Some(cursor.as_bytes().to_vec())
    );
    drop(connection);
    drop(api);
    drop(unavailable);
    drop(store);
    drop(server);
    complete(&[
        "shared-metrics-finite-counter-vocabulary",
        "shared-metrics-validated-age-forwarded",
        "shared-metrics-success-after-atomic-publication",
        "shared-metrics-ack-after-atomic-authority",
        "shared-metrics-errors-duplicate-capacity-no-success",
        "shared-metrics-no-identifier-labels",
    ]);
}
