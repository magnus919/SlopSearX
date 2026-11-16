//! Native W00 contract tests use only isolated private Valkey, never engines.
use ssx_application::{
    policy::{PolicyService, SearchPolicy},
    saved::*,
    scope::{CatalogSnapshot, EngineCapability, ScopeResolver},
};
use ssx_domain::{LeaseToken, SavedSearchId, TenantId};
use ssx_infra::valkey::{saved::*, *};
use ssx_json::json;
use std::{
    collections::BTreeMap,
    fs,
    io::Read as _,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};
struct Server {
    child: Child,
    dir: PathBuf,
    url: String,
}
impl Server {
    fn start() -> Self {
        let mut bytes = [0; 16];
        fs::File::open("/dev/urandom")
            .unwrap()
            .read_exact(&mut bytes)
            .unwrap();
        bytes[6] = (bytes[6] & 15) | 64;
        bytes[8] = (bytes[8] & 63) | 128;
        let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let dir = std::env::temp_dir().join(format!("ssx-w00-{id}"));
        fs::create_dir(&dir).unwrap();
        let url = format!("unix://{}", dir.join("s.sock").display());
        let mut s = Self {
            child: Self::spawn(&dir),
            dir,
            url,
        };
        s.wait_ready();
        s
    }
    fn spawn(dir: &std::path::Path) -> Child {
        Command::new(
            std::env::var("SLOPSEARX_TEST_VALKEY_SERVER")
                .unwrap_or_else(|_| "valkey-server".into()),
        )
        .args([
            "--port",
            "0",
            "--bind",
            "127.0.0.1",
            "--save",
            "",
            "--appendonly",
            "no",
            "--daemonize",
            "no",
            "--unixsocketperm",
            "700",
            "--dir",
        ])
        .arg(dir)
        .arg("--unixsocket")
        .arg(dir.join("s.sock"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("disposable valkey-server required")
    }
    fn wait_ready(&mut self) {
        let until = Instant::now() + Duration::from_secs(4);
        loop {
            assert!(
                self.child.try_wait().unwrap().is_none(),
                "Valkey startup failed"
            );
            if redis::Client::open(self.url.as_str())
                .unwrap()
                .get_connection()
                .is_ok()
            {
                break;
            }
            assert!(Instant::now() < until, "Valkey startup deadline");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn connection(&self) -> redis::Connection {
        redis::Client::open(self.url.as_str())
            .unwrap()
            .get_connection()
            .unwrap()
    }
    fn store(&self) -> Valkey {
        Valkey::new(&self.url, Duration::from_secs(2)).unwrap()
    }
    fn stop(&mut self) {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
    }
    fn restart(&mut self) {
        self.child = Self::spawn(&self.dir);
        self.wait_ready();
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}
struct Tokens(&'static str);
impl TokenSource<SavedSearchId> for Tokens {
    fn next_token(&mut self) -> LeaseToken<SavedSearchId> {
        LeaseToken::new(self.0).unwrap()
    }
}
fn id(name: &str) -> SavedSearchId {
    SavedSearchId::new(name).unwrap()
}
fn policy() -> PolicyService {
    PolicyService {
        policy: SearchPolicy::default(),
        resolver: ScopeResolver::default(),
        catalog: CatalogSnapshot::new(vec![EngineCapability {
            name: "brave".into(),
            enabled: true,
            active: true,
            available: false,
            categories: vec!["general".into()],
            media_types: vec![],
            auth_ready: true,
            circuit_open: false,
            cost_class: "free".into(),
            health_status: "ok".into(),
            health_stale: false,
            enforced_filters: BTreeMap::new(),
        }])
        .unwrap(),
    }
}
fn create_at(p: &SavedPolicy<'_>, tenant: &str, name: &str, now: f64) -> PreparedDefinition {
    prepare_create(p,tenant,name,&json!({"query":"q","engines":["brave"],"interval_seconds":60,"start_immediately":true,"expires_in_seconds":600}),now).unwrap()
}
#[tokio::test]
async fn atomic_revision_claim_pause_delete_and_tombstone() {
    let server = Server::start();
    let store = SavedStore::new(Arc::new(server.store()), TenantId::new("a:b[*]").unwrap());
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let prepared = create_at(
        &p,
        store.tenant().as_str(),
        "saved-a",
        store.now().await.unwrap(),
    );
    assert!(store.create(&prepared).await.unwrap());
    let claim = store
        .claim(&id("saved-a"), &mut Tokens("owner-one"))
        .await
        .unwrap()
        .unwrap();
    assert!(!format!("{claim:?}").contains("owner-one"));
    assert!(
        store
            .claim(&id("saved-a"), &mut Tokens("owner-two"))
            .await
            .unwrap()
            .is_none()
    );
    assert!(store.renew(&claim).await.unwrap());
    let saved_id = id("saved-a");
    let first = json!({"query":"first"});
    let second = json!({"query":"second"});
    let (a, b) = tokio::join!(
        store.update(&saved_id, 1, &first, &p),
        store.update(&saved_id, 1, &second, &p)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(matches!(
        a.as_ref().err().or(b.as_ref().err()),
        Some(Error::Conflict)
    ));
    assert!(store.renew(&claim).await.is_err());
    let paused = store.pause(&id("saved-a"), 2, true, &p).await.unwrap();
    assert_eq!(paused.revision, 3);
    assert!(paused.paused);
    assert!(
        store
            .claim(&id("saved-a"), &mut Tokens("owner-three"))
            .await
            .unwrap()
            .is_none()
    );
    store.delete(&id("saved-a"), 3).await.unwrap();
    assert!(store.load(&id("saved-a")).await.unwrap().is_none());
    let tomb: String = redis::cmd("GET")
        .arg("mcp:saved:v1:deleted:a:b[*]:saved-a")
        .query(&mut server.connection())
        .unwrap();
    assert_eq!(tomb, "3");
    let large = create_at(
        &p,
        store.tenant().as_str(),
        "saved-large-revision",
        store.now().await.unwrap(),
    );
    let mut record = large.definition().clone();
    record.revision = 100_000_000_000_000;
    redis::cmd("SET")
        .arg("mcp:saved:v1:definition:a:b[*]:saved-large-revision")
        .arg(ssx_json::to_vec(&record.stored_payload().unwrap()).unwrap())
        .arg("EX")
        .arg(900)
        .query::<()>(&mut server.connection())
        .unwrap();
    store
        .delete(&id("saved-large-revision"), 100_000_000_000_000)
        .await
        .unwrap();
    let exact: String = redis::cmd("GET")
        .arg("mcp:saved:v1:deleted:a:b[*]:saved-large-revision")
        .query(&mut server.connection())
        .unwrap();
    assert_eq!(exact, "100000000000000");
}
#[tokio::test]
async fn quota_repair_tenant_binding_and_full_preflight() {
    let server = Server::start();
    let backend = Arc::new(server.store());
    let store = SavedStore::new(backend.clone(), TenantId::new("a:b[*]").unwrap());
    let service = policy();
    let limits = SavedLimits {
        definitions: 1,
        ..Default::default()
    };
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    assert!(
        store
            .create(&create_at(
                &p,
                store.tenant().as_str(),
                "c",
                store.now().await.unwrap()
            ))
            .await
            .unwrap()
    );
    redis::cmd("DEL")
        .arg("mcp:saved:v1:definitions:a:b[*]")
        .query::<()>(&mut server.connection())
        .unwrap();
    assert_eq!(
        store
            .create(&create_at(
                &p,
                store.tenant().as_str(),
                "saved-second",
                store.now().await.unwrap()
            ))
            .await
            .unwrap_err(),
        Error::QuotaExceeded
    );
    let other = store.for_tenant(TenantId::new("a").unwrap());
    assert!(other.load(&id("b[*]:c")).await.is_err());
    assert!(other.load(&id("c")).await.unwrap().is_none());
    let mut raw = store.load(&id("c")).await.unwrap().unwrap();
    raw.tenant = "wrong".into();
    redis::cmd("SET")
        .arg("mcp:saved:v1:definition:a:b[*]:c")
        .arg(ssx_json::to_string(&raw.stored_payload().unwrap()).unwrap())
        .query::<()>(&mut server.connection())
        .unwrap();
    assert_eq!(
        store.delete(&id("c"), 1).await.unwrap_err(),
        Error::Malformed
    );
    let fresh = store.for_tenant(TenantId::new("fresh").unwrap());
    redis::cmd("SET")
        .arg("mcp:saved:v1:recent:fresh")
        .arg("wrong-type")
        .query::<()>(&mut server.connection())
        .unwrap();
    assert_eq!(
        fresh
            .create(&create_at(
                &p,
                "fresh",
                "saved-new",
                fresh.now().await.unwrap()
            ))
            .await
            .unwrap_err(),
        Error::Malformed
    );
    let exists: bool = redis::cmd("EXISTS")
        .arg("mcp:saved:v1:definition:fresh:saved-new")
        .query(&mut server.connection())
        .unwrap();
    assert!(!exists);
}
#[tokio::test]
async fn server_time_expiry_future_preparation_and_stale_owner_fail_closed() {
    let server = Server::start();
    let store = SavedStore::new(Arc::new(server.store()), TenantId::new("clock").unwrap());
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let now = store.now().await.unwrap();
    assert_eq!(
        store
            .create(&create_at(&p, "clock", "saved-future", now + 120.0))
            .await
            .unwrap_err(),
        Error::Malformed
    );
    assert_eq!(
        store
            .create(&create_at(&p, "clock", "saved-expired", now - 700.0))
            .await
            .unwrap_err(),
        Error::Malformed
    );
    assert!(
        store
            .create(&create_at(&p, "clock", "saved-live", now))
            .await
            .unwrap()
    );
    let old = store
        .claim(&id("saved-live"), &mut Tokens("old-owner"))
        .await
        .unwrap()
        .unwrap();
    let ttl: i64 = redis::cmd("TTL")
        .arg("mcp:saved:v1:lease:clock:saved-live")
        .query(&mut server.connection())
        .unwrap();
    assert!((59..=60).contains(&ttl));
    redis::cmd("PEXPIRE")
        .arg("mcp:saved:v1:lease:clock:saved-live")
        .arg(1)
        .query::<()>(&mut server.connection())
        .unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    let fresh = store
        .claim(&id("saved-live"), &mut Tokens("fresh-owner"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        old.definition().slot(now + 60.0).unwrap().run_id,
        fresh.definition().slot(now + 120.0).unwrap().run_id
    );
    assert!(store.release(&old).await.is_err());
    assert!(store.renew(&fresh).await.unwrap());
    let mut definition = store.load(&id("saved-live")).await.unwrap().unwrap();
    definition.expires_at = ssx_domain::FiniteF64::new(now - 1.0).unwrap();
    redis::cmd("SET")
        .arg("mcp:saved:v1:definition:clock:saved-live")
        .arg(ssx_json::to_string(&definition.stored_payload().unwrap()).unwrap())
        .arg("EX")
        .arg(300)
        .query::<()>(&mut server.connection())
        .unwrap();
    assert!(store.load(&id("saved-live")).await.unwrap().is_none());
    assert!(
        store
            .claim(&id("saved-live"), &mut Tokens("expired-attempt"))
            .await
            .unwrap()
            .is_none()
    );
}
#[tokio::test]
async fn restart_reconcile_due_recent_and_tenant_continuation() {
    let mut server = Server::start();
    let store = SavedStore::new(
        Arc::new(server.store()),
        TenantId::new("tenant[*]:雪").unwrap(),
    );
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let now = store.now().await.unwrap();
    for name in ["saved-z", "saved-a"] {
        assert!(
            store
                .create(&create_at(&p, store.tenant().as_str(), name, now))
                .await
                .unwrap()
        );
    }
    assert_eq!(store.due_ids(128).await.unwrap().len(), 2);
    let recent = store.list_recent(None, 1).await.unwrap();
    assert_eq!(recent[0].search_id, "saved-z");
    let next = store
        .list_recent(Some((recent[0].created_at.get(), &recent[0].search_id)), 1)
        .await
        .unwrap();
    assert_eq!(next[0].search_id, "saved-a");
    redis::cmd("DEL")
        .arg("mcp:saved:v1:definitions:tenant[*]:雪")
        .arg("mcp:saved:v1:due:tenant[*]:雪")
        .arg("mcp:saved:v1:tenants")
        .query::<()>(&mut server.connection())
        .unwrap();
    redis::cmd("SAVE")
        .query::<()>(&mut server.connection())
        .unwrap();
    server.stop();
    server.restart();
    let store = SavedStore::new(Arc::new(server.store()), store.tenant().clone());
    assert!(store.load(&id("saved-a")).await.unwrap().is_some());
    assert_eq!(store.repair_indexes().await.unwrap(), 2);
    assert_eq!(store.list_definitions(128).await.unwrap().len(), 2);
    assert_eq!(store.due_ids(128).await.unwrap().len(), 2);
    let page = store.scan_definitions(0, 128).await.unwrap();
    assert_eq!(page.cursor, 0);
    assert_eq!(page.values.len(), 2);
    let tenants = store.scan_tenants(0, 128).await.unwrap();
    assert_eq!(tenants.cursor, 0);
    assert_eq!(tenants.values, [store.tenant().clone()]);
    let hint: Option<f64> = redis::cmd("ZSCORE")
        .arg(keys::SAVED_TENANTS)
        .arg(store.tenant().as_str())
        .query(&mut server.connection())
        .unwrap();
    assert!(hint.unwrap() > now);
}
fn padded_record(d: &ssx_domain::saved::SavedDefinition, size: usize) -> Vec<u8> {
    let mut value = d.stored_payload().unwrap();
    value["padding"] = json!("");
    let small = ssx_json::to_vec(&value).unwrap();
    value["padding"] = json!("x".repeat(size - small.len()));
    let bytes = ssx_json::to_vec(&value).unwrap();
    assert_eq!(bytes.len(), size);
    bytes
}
#[tokio::test]
async fn repair_byte_boundaries_are_atomic_and_preserve_existing_records() {
    let server = Server::start();
    let store = SavedStore::new(Arc::new(server.store()), TenantId::new("bytes").unwrap());
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let now = store.now().await.unwrap();
    for name in ["saved-a", "saved-b"] {
        let d = create_at(&p, "bytes", name, now);
        let bytes = padded_record(d.definition(), 4 * 1024 * 1024);
        redis::cmd("SET")
            .arg(format!("mcp:saved:v1:definition:bytes:{name}"))
            .arg(bytes)
            .arg("EX")
            .arg(900)
            .query::<()>(&mut server.connection())
            .unwrap();
    }
    assert!(store.load(&id("saved-a")).await.unwrap().is_some());
    assert_eq!(store.repair_indexes().await.unwrap(), 2);
    let indexes: Vec<(String, f64)> = redis::cmd("ZRANGE")
        .arg("mcp:saved:v1:definitions:bytes")
        .arg(0)
        .arg(-1)
        .arg("WITHSCORES")
        .query(&mut server.connection())
        .unwrap();
    let third = create_at(&p, "bytes", "saved-c", now);
    redis::cmd("SET")
        .arg("mcp:saved:v1:definition:bytes:saved-c")
        .arg(ssx_json::to_vec(&third.definition().stored_payload().unwrap()).unwrap())
        .arg("EX")
        .arg(900)
        .query::<()>(&mut server.connection())
        .unwrap();
    assert_eq!(
        store.repair_indexes().await.unwrap_err(),
        Error::RepairIncomplete
    );
    let unchanged: Vec<(String, f64)> = redis::cmd("ZRANGE")
        .arg("mcp:saved:v1:definitions:bytes")
        .arg(0)
        .arg(-1)
        .arg("WITHSCORES")
        .query(&mut server.connection())
        .unwrap();
    assert_eq!(indexes, unchanged);
    redis::cmd("SET")
        .arg("mcp:saved:v1:definition:bytes:saved-c")
        .arg(padded_record(third.definition(), 4 * 1024 * 1024 + 1))
        .arg("EX")
        .arg(900)
        .query::<()>(&mut server.connection())
        .unwrap();
    assert_eq!(
        store.load(&id("saved-c")).await.unwrap_err(),
        Error::RepairIncomplete
    );
    assert_eq!(
        store.repair_indexes().await.unwrap_err(),
        Error::RepairIncomplete
    );
    let count: usize = redis::cmd("ZCARD")
        .arg("mcp:saved:v1:definitions:bytes")
        .query(&mut server.connection())
        .unwrap();
    assert_eq!(count, 2);
}
#[tokio::test]
async fn revoked_engine_control_and_reduced_untouched_limits_match_source() {
    let server = Server::start();
    let store = SavedStore::new(Arc::new(server.store()), TenantId::new("controls").unwrap());
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    assert!(
        store
            .create(&create_at(
                &p,
                "controls",
                "saved-revoked",
                store.now().await.unwrap()
            ))
            .await
            .unwrap()
    );
    let mut revoked = service.clone();
    let mut caps = revoked.catalog.engines().to_vec();
    caps[0].active = false;
    revoked.catalog = CatalogSnapshot::new(caps).unwrap();
    let control = SavedPolicy {
        search: &revoked,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    assert!(
        store
            .pause(&id("saved-revoked"), 1, true, &control)
            .await
            .unwrap()
            .paused
    );
    assert!(
        !store
            .pause(&id("saved-revoked"), 2, false, &control)
            .await
            .unwrap()
            .paused
    );
    let denied = SavedPolicy {
        saved_granted: false,
        ..control
    };
    assert!(
        store
            .pause(&id("saved-revoked"), 3, true, &denied)
            .await
            .is_err()
    );
    assert_eq!(
        store
            .load(&id("saved-revoked"))
            .await
            .unwrap()
            .unwrap()
            .revision,
        3
    );
    assert!(
        store
            .create(&create_at(
                &p,
                "controls",
                "saved-limits",
                store.now().await.unwrap()
            ))
            .await
            .unwrap()
    );
    let lower = SavedLimits {
        rows: 1,
        ..Default::default()
    };
    let reduced = SavedPolicy {
        limits: &lower,
        ..p
    };
    let next = store
        .update(
            &id("saved-limits"),
            1,
            &json!({"query":"changed"}),
            &reduced,
        )
        .await
        .unwrap();
    assert_eq!(next.max_results, 100);
    assert!(
        store
            .update(
                &id("saved-limits"),
                2,
                &json!({"max_results":100}),
                &reduced
            )
            .await
            .is_err()
    );
    assert_eq!(
        store
            .update(&id("saved-limits"), 2, &json!({"max_results":1}), &reduced)
            .await
            .unwrap()
            .max_results,
        1
    );
}
#[tokio::test]
async fn concurrent_quota_and_claim_have_one_owner_and_no_lost_definition() {
    let server = Server::start();
    let backend = Arc::new(server.store());
    let store = SavedStore::new(backend, TenantId::new("races").unwrap());
    let service = policy();
    let limits = SavedLimits {
        definitions: 1,
        ..Default::default()
    };
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let now = store.now().await.unwrap();
    let first = create_at(&p, "races", "saved-a", now);
    let second = create_at(&p, "races", "saved-b", now);
    let independent = SavedStore::new(Arc::new(server.store()), store.tenant().clone());
    let (a, b) = tokio::join!(store.create(&first), independent.create(&second));
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(matches!(
        a.as_ref().err().or(b.as_ref().err()),
        Some(Error::QuotaExceeded)
    ));
    let winner = id(if a.is_ok() { "saved-a" } else { "saved-b" });
    let mut tasks = Vec::new();
    for index in 0..8 {
        let store = SavedStore::new(Arc::new(server.store()), store.tenant().clone());
        let winner = winner.clone();
        tasks.push(tokio::spawn(async move {
            struct One(String);
            impl TokenSource<SavedSearchId> for One {
                fn next_token(&mut self) -> LeaseToken<SavedSearchId> {
                    LeaseToken::new(self.0.clone()).unwrap()
                }
            }
            store
                .claim(&winner, &mut One(format!("owner-{index}")))
                .await
                .unwrap()
        }));
    }
    let mut owners = Vec::new();
    for task in tasks {
        if let Some(claim) = task.await.unwrap() {
            owners.push(claim)
        }
    }
    assert_eq!(owners.len(), 1);
    assert!(store.release(&owners[0]).await.unwrap());
    assert_eq!(store.list_definitions(128).await.unwrap().len(), 1);
}
#[tokio::test]
async fn repair_preserves_longest_expiry_and_rejects_overflow_before_writes() {
    let server = Server::start();
    let store = SavedStore::new(Arc::new(server.store()), TenantId::new("horizon").unwrap());
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let now = store.now().await.unwrap();
    for (name, lifetime) in [("saved-long", 1000), ("saved-short", 100)] {
        let d=prepare_create(&p,"horizon",name,&json!({"query":"q","engines":["brave"],"interval_seconds":60,"expires_in_seconds":lifetime}),now).unwrap();
        assert!(store.create(&d).await.unwrap());
    }
    assert_eq!(store.repair_indexes().await.unwrap(), 2);
    for key in ["definitions", "due", "definition-count"] {
        let ttl: i64 = redis::cmd("TTL")
            .arg(format!("mcp:saved:v1:{key}:horizon"))
            .query(&mut server.connection())
            .unwrap();
        assert!((1298..=1300).contains(&ttl), "{key}:{ttl}");
    }
    let mut corrupt = store.load(&id("saved-short")).await.unwrap().unwrap();
    corrupt.expires_at = ssx_domain::FiniteF64::new(1e308).unwrap();
    redis::cmd("SET")
        .arg("mcp:saved:v1:definition:horizon:saved-short")
        .arg(ssx_json::to_vec(&corrupt.stored_payload().unwrap()).unwrap())
        .arg("EX")
        .arg(900)
        .query::<()>(&mut server.connection())
        .unwrap();
    let before: Vec<(String, f64)> = redis::cmd("ZRANGE")
        .arg("mcp:saved:v1:definitions:horizon")
        .arg(0)
        .arg(-1)
        .arg("WITHSCORES")
        .query(&mut server.connection())
        .unwrap();
    assert_eq!(store.repair_indexes().await.unwrap_err(), Error::Malformed);
    let after: Vec<(String, f64)> = redis::cmd("ZRANGE")
        .arg("mcp:saved:v1:definitions:horizon")
        .arg(0)
        .arg(-1)
        .arg("WITHSCORES")
        .query(&mut server.connection())
        .unwrap();
    assert_eq!(before, after);
}
#[tokio::test]
async fn claimed_cjson_bytes_and_run_ids_match_accepted_source_script() {
    let server = Server::start();
    let store = SavedStore::new(Arc::new(server.store()), TenantId::new("identity").unwrap());
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    for (index, due) in [
        1700000000.123456,
        1.234567890123456e-8,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        -1e15,
    ]
    .into_iter()
    .enumerate()
    {
        let name = format!("saved-precision-{index}");
        let prepared = create_at(&p, "identity", &name, store.now().await.unwrap());
        let mut definition = prepared.definition().clone();
        definition.created_at = ssx_domain::FiniteF64::new(due - 60.0).unwrap();
        definition.next_due = ssx_domain::FiniteF64::new(due).unwrap();
        definition.expires_at = ssx_domain::FiniteF64::new(1e12).unwrap();
        let original = ssx_json::to_vec(&definition.stored_payload().unwrap()).unwrap();
        redis::cmd("SET")
            .arg(format!("mcp:saved:v1:definition:identity:{name}"))
            .arg(&original)
            .arg("EX")
            .arg(900)
            .query::<()>(&mut server.connection())
            .unwrap();
        redis::cmd("SET")
            .arg("w00-source-definition")
            .arg(&original)
            .arg("EX")
            .arg(900)
            .query::<()>(&mut server.connection())
            .unwrap();
        let claim = store
            .claim(&id(&name), &mut Tokens("same-token"))
            .await
            .unwrap()
            .unwrap();
        let source_raw: Vec<u8> = redis::cmd("EVAL")
            .arg(scripts::Script::SavedClaim.source())
            .arg(2)
            .arg("w00-source-definition")
            .arg("w00-source-lease")
            .arg(claim.definition().lease_expires_at.get() - 60.0)
            .arg("same-token")
            .arg(60)
            .arg(900)
            .query(&mut server.connection())
            .unwrap();
        let committed: Vec<u8> = redis::cmd("GET")
            .arg(format!("mcp:saved:v1:definition:identity:{name}"))
            .query(&mut server.connection())
            .unwrap();
        assert_eq!(committed, source_raw, "cjson bytes case{index}");
        let source = ssx_domain::saved::SavedDefinition::from_payload(
            &ssx_json::from_slice(&source_raw).unwrap(),
        )
        .unwrap();
        assert_eq!(
            claim
                .definition()
                .slot(claim.definition().next_due.get())
                .unwrap()
                .run_id,
            source.slot(source.next_due.get()).unwrap().run_id
        );
        redis::cmd("DEL")
            .arg("w00-source-definition")
            .arg("w00-source-lease")
            .query::<()>(&mut server.connection())
            .unwrap();
    }
}
#[tokio::test]
async fn unknown_create_completion_is_not_retried_and_reconciles_authority() {
    let server = Server::start();
    let backend = Arc::new(Valkey::new(&server.url, Duration::from_millis(30)).unwrap());
    backend.ping().await.unwrap();
    let store = SavedStore::new(backend, TenantId::new("unknown").unwrap());
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let prepared = create_at(&p, "unknown", "saved-one", store.now().await.unwrap());
    redis::cmd("CLIENT")
        .arg("PAUSE")
        .arg(150)
        .arg("WRITE")
        .query::<()>(&mut server.connection())
        .unwrap();
    assert_eq!(
        store.create(&prepared).await.unwrap_err(),
        Error::Store(Failure::CompletionUnknown)
    );
    tokio::time::sleep(Duration::from_millis(250)).await;
    let authority = SavedStore::new(Arc::new(server.store()), TenantId::new("unknown").unwrap());
    let committed = authority.load(&id("saved-one")).await.unwrap().unwrap();
    assert_eq!(committed.revision, 1);
    let count: u64 = redis::cmd("GET")
        .arg("mcp:saved:v1:definition-count:unknown")
        .query(&mut server.connection())
        .unwrap();
    assert_eq!(count, 1);
}
#[tokio::test]
async fn authoritative_tenant_enumeration_continues_and_escapes_literal_prefixes() {
    let server = Server::start();
    let store = SavedStore::new(
        Arc::new(server.store()),
        TenantId::new("tenant[*]").unwrap(),
    );
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let now = store.now().await.unwrap();
    let mut expected = std::collections::BTreeSet::new();
    for index in 0..140 {
        let tenant = format!("tenant-{index:03}");
        expected.insert(tenant.clone());
        let definition = create_at(&p, &tenant, "saved-item", now);
        redis::cmd("SET")
            .arg(format!("mcp:saved:v1:definition:{tenant}:saved-item"))
            .arg(ssx_json::to_vec(&definition.definition().stored_payload().unwrap()).unwrap())
            .arg("EX")
            .arg(900)
            .query::<()>(&mut server.connection())
            .unwrap();
    }
    for tenant in ["tenant[*]", "tenantx", "tenant[*]:nested"] {
        expected.insert(tenant.into());
        let definition = create_at(&p, tenant, "saved-item", now);
        redis::cmd("SET")
            .arg(format!("mcp:saved:v1:definition:{tenant}:saved-item"))
            .arg(ssx_json::to_vec(&definition.definition().stored_payload().unwrap()).unwrap())
            .arg("EX")
            .arg(900)
            .query::<()>(&mut server.connection())
            .unwrap();
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut cursor = 0;
    let mut pages = 0;
    loop {
        let page = store.scan_tenants(cursor, 3).await.unwrap();
        seen.extend(page.values.into_iter().map(|t| t.as_str().to_owned()));
        cursor = page.cursor;
        pages += 1;
        if cursor == 0 {
            break;
        }
        assert!(pages < 1024);
    }
    assert!(pages > 1);
    assert_eq!(seen, expected);
    let mut cursor = 0;
    let mut records = Vec::new();
    loop {
        let page = store.scan_definitions(cursor, 3).await.unwrap();
        records.extend(page.values);
        cursor = page.cursor;
        if cursor == 0 {
            break;
        }
    }
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].tenant, "tenant[*]");
}
#[tokio::test]
async fn exact_colon_collision_is_rejected_and_tenants_admit_independently() {
    let server = Server::start();
    let backend = Arc::new(server.store());
    let a = SavedStore::new(backend.clone(), TenantId::new("a:b").unwrap());
    let b = SavedStore::new(backend, TenantId::new("a").unwrap());
    assert_eq!(
        keys::saved("definition", a.tenant(), Some("c")),
        keys::saved("definition", b.tenant(), Some("b:c"))
    );
    let service = policy();
    let limits = SavedLimits {
        definitions: 1,
        ..Default::default()
    };
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    assert!(
        a.create(&create_at(&p, "a:b", "c", a.now().await.unwrap()))
            .await
            .unwrap()
    );
    assert!(b.load(&id("b:c")).await.is_err());
    assert!(b.delete(&id("b:c"), 1).await.is_err());
    assert!(
        b.create(&create_at(&p, "a", "saved-b", b.now().await.unwrap()))
            .await
            .unwrap()
    );
    let claim = a.claim_fresh(&id("c")).await.unwrap().unwrap();
    assert!(b.renew(&claim).await.is_err());
    assert!(b.release(&claim).await.is_err());
    assert!(a.renew(&claim).await.unwrap());
    assert_eq!(a.load(&id("c")).await.unwrap().unwrap().tenant, "a:b");
    assert_eq!(b.load(&id("saved-b")).await.unwrap().unwrap().tenant, "a");
}
#[tokio::test]
async fn recent_index_is_bounded_and_fills_around_stale_records() {
    let server = Server::start();
    let store = SavedStore::new(Arc::new(server.store()), TenantId::new("recent").unwrap());
    let service = policy();
    let limits = SavedLimits {
        definitions: 220,
        ..Default::default()
    };
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    for index in 0..210 {
        let name = format!("saved-{index:03}");
        assert!(
            store
                .create(&create_at(&p, "recent", &name, store.now().await.unwrap()))
                .await
                .unwrap()
        );
    }
    let size: u64 = redis::cmd("ZCARD")
        .arg("mcp:saved:v1:recent:recent")
        .query(&mut server.connection())
        .unwrap();
    assert_eq!(size, 200);
    let first = store.list_recent(None, 50).await.unwrap();
    let marker = first.last().unwrap();
    let second = store
        .list_recent(Some((marker.created_at.get(), &marker.search_id)), 50)
        .await
        .unwrap();
    assert_eq!(first.len(), 50);
    assert_eq!(second.len(), 50);
    assert!(
        first
            .iter()
            .all(|a| second.iter().all(|b| a.search_id != b.search_id))
    );
    for record in &first[..10] {
        redis::cmd("DEL")
            .arg(format!(
                "mcp:saved:v1:definition:recent:{}",
                record.search_id
            ))
            .query::<()>(&mut server.connection())
            .unwrap();
    }
    let filled = store.list_recent(None, 50).await.unwrap();
    assert_eq!(filled.len(), 50);
    assert!(
        filled
            .iter()
            .all(|a| first[..10].iter().all(|b| a.search_id != b.search_id))
    );
}
#[tokio::test]
async fn authoritative_repair_key_budget_refuses_partial_publication() {
    let server = Server::start();
    let store = SavedStore::new(
        Arc::new(server.store()),
        TenantId::new("key-budget").unwrap(),
    );
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let prepared = create_at(&p, "key-budget", "saved-base", store.now().await.unwrap());
    let mut definition = prepared.definition().clone();
    let mut pipeline = redis::pipe();
    for index in 0..4100 {
        definition.search_id = format!("saved-{index:04}");
        pipeline
            .cmd("SET")
            .arg(format!(
                "mcp:saved:v1:definition:key-budget:{}",
                definition.search_id
            ))
            .arg(ssx_json::to_vec(&definition.stored_payload().unwrap()).unwrap())
            .arg("EX")
            .arg(900)
            .ignore();
    }
    pipeline.query::<()>(&mut server.connection()).unwrap();
    redis::cmd("ZADD")
        .arg("mcp:saved:v1:definitions:key-budget")
        .arg(definition.expires_at.get())
        .arg("saved-sentinel")
        .query::<()>(&mut server.connection())
        .unwrap();
    assert_eq!(
        store.repair_indexes().await.unwrap_err(),
        Error::RepairIncomplete
    );
    let unchanged: Vec<String> = redis::cmd("ZRANGE")
        .arg("mcp:saved:v1:definitions:key-budget")
        .arg(0)
        .arg(-1)
        .query(&mut server.connection())
        .unwrap();
    assert_eq!(unchanged, ["saved-sentinel"]);
    assert_eq!(
        store.create(&prepared).await.unwrap_err(),
        Error::RepairIncomplete
    );
    let absent: bool = redis::cmd("EXISTS")
        .arg("mcp:saved:v1:definition:key-budget:saved-base")
        .query(&mut server.connection())
        .unwrap();
    assert!(!absent);
}
#[tokio::test]
async fn due_cleanup_is_bounded_and_logical_expiry_preserves_retention_record() {
    let server = Server::start();
    let store = SavedStore::new(Arc::new(server.store()), TenantId::new("due").unwrap());
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let now = store.now().await.unwrap();
    assert!(
        store
            .create(&create_at(&p, "due", "saved-live", now))
            .await
            .unwrap()
    );
    for index in 0..10 {
        redis::cmd("ZADD")
            .arg("mcp:saved:v1:due:due")
            .arg(0)
            .arg(format!("saved-stale-{index}"))
            .query::<()>(&mut server.connection())
            .unwrap();
    }
    assert!(store.due_ids(3).await.unwrap().is_empty());
    let left: usize = redis::cmd("ZCARD")
        .arg("mcp:saved:v1:due:due")
        .query(&mut server.connection())
        .unwrap();
    assert_eq!(left, 8);
    let mut expired = create_at(&p, "due", "saved-expired", now - 700.0)
        .definition()
        .clone();
    expired.expires_at = ssx_domain::FiniteF64::new(now - 1.0).unwrap();
    redis::cmd("SET")
        .arg("mcp:saved:v1:definition:due:saved-expired")
        .arg(ssx_json::to_vec(&expired.stored_payload().unwrap()).unwrap())
        .arg("EX")
        .arg(300)
        .query::<()>(&mut server.connection())
        .unwrap();
    redis::cmd("ZADD")
        .arg("mcp:saved:v1:due:due")
        .arg(-1)
        .arg("saved-expired")
        .query::<()>(&mut server.connection())
        .unwrap();
    assert!(store.load(&id("saved-expired")).await.unwrap().is_none());
    let due = store.due_ids(128).await.unwrap();
    assert_eq!(due, [id("saved-live")]);
    let physical: bool = redis::cmd("EXISTS")
        .arg("mcp:saved:v1:definition:due:saved-expired")
        .query(&mut server.connection())
        .unwrap();
    assert!(physical);
    redis::cmd("ZADD")
        .arg(keys::SAVED_TENANTS)
        .arg(now - 1.0)
        .arg("expired-tenant")
        .query::<()>(&mut server.connection())
        .unwrap();
    store.scan_tenants(0, 128).await.unwrap();
    let gone: Option<f64> = redis::cmd("ZSCORE")
        .arg(keys::SAVED_TENANTS)
        .arg("expired-tenant")
        .query(&mut server.connection())
        .unwrap();
    assert!(gone.is_none());
}
#[tokio::test]
async fn deletion_with_missing_index_forces_authoritative_quota_repair() {
    let server = Server::start();
    let store = SavedStore::new(
        Arc::new(server.store()),
        TenantId::new("delete-count").unwrap(),
    );
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    for name in ["saved-a", "saved-b"] {
        assert!(
            store
                .create(&create_at(
                    &p,
                    "delete-count",
                    name,
                    store.now().await.unwrap()
                ))
                .await
                .unwrap()
        );
    }
    redis::cmd("DEL")
        .arg("mcp:saved:v1:definitions:delete-count")
        .query::<()>(&mut server.connection())
        .unwrap();
    store.delete(&id("saved-a"), 1).await.unwrap();
    let absent: bool = redis::cmd("EXISTS")
        .arg("mcp:saved:v1:definition-count:delete-count")
        .query(&mut server.connection())
        .unwrap();
    assert!(!absent);
    let lower = SavedLimits {
        definitions: 1,
        ..Default::default()
    };
    let restricted = SavedPolicy {
        limits: &lower,
        ..p
    };
    assert_eq!(
        store
            .create(&create_at(
                &restricted,
                "delete-count",
                "saved-c",
                store.now().await.unwrap()
            ))
            .await
            .unwrap_err(),
        Error::QuotaExceeded
    );
    assert!(store.load(&id("saved-b")).await.unwrap().is_some());
}

#[tokio::test]
async fn probe_claim_hydrates_live_baseline() {
    let server = Server::start();
    let store = SavedStore::new(
        Arc::new(server.store()),
        TenantId::new("baseline-probe").unwrap(),
    );
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let now = store.now().await.unwrap();
    store
        .create(&create_at(&p, "baseline-probe", "saved-one", now))
        .await
        .unwrap();
    let baseline = json!({"expires_at":now+300.0,"records":{},"window":{"query":"q"}});
    redis::cmd("SET")
        .arg("mcp:saved:v1:baseline:baseline-probe:saved-one")
        .arg(ssx_json::to_vec(&baseline).unwrap())
        .arg("EX")
        .arg(300)
        .query::<()>(&mut server.connection())
        .unwrap();
    assert!(
        store
            .load(&id("saved-one"))
            .await
            .unwrap()
            .unwrap()
            .baseline
            .is_some()
    );
    let claim = store.claim_fresh(&id("saved-one")).await.unwrap().unwrap();
    println!("claimed_baseline={:?}", claim.definition().baseline);
    assert!(
        claim.definition().baseline.is_some(),
        "source successful claim hydrates live separate baseline"
    );
}
#[tokio::test]
async fn probe_large_revision_increment() {
    let server = Server::start();
    let store = SavedStore::new(
        Arc::new(server.store()),
        TenantId::new("revision-probe").unwrap(),
    );
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let now = store.now().await.unwrap();
    let prepared = create_at(&p, "revision-probe", "saved-one", now);
    let mut d = prepared.definition().clone();
    d.revision = 9_007_199_254_740_993;
    redis::cmd("SET")
        .arg("mcp:saved:v1:definition:revision-probe:saved-one")
        .arg(ssx_json::to_vec(&d.stored_payload().unwrap()).unwrap())
        .arg("EX")
        .arg(900)
        .query::<()>(&mut server.connection())
        .unwrap();
    let out = store
        .update(
            &id("saved-one"),
            d.revision,
            &json!({"query":"changed"}),
            &p,
        )
        .await;
    println!("large_revision_update={:?}", out);
    assert!(out.is_ok(), "valid i64 exact revision must increment");
}

#[test]
fn probe_configured_query_limit() {
    let mut service = policy();
    service.policy.max_query_length = 1000;
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let out = prepare_create(
        &p,
        "query-probe",
        "saved-one",
        &json!({"query":"q".repeat(600),"engines":["brave"],"interval_seconds":60}),
        1000.0,
    );
    println!(
        "configured_max1000_query600={:?}",
        out.as_ref().map(|v| v.definition())
    );
    assert!(
        out.is_ok(),
        "source configured maxquerylength1000 accepts600chars"
    );
}

#[test]
fn probe_query_limit_does_not_revalidate_untouched_query() {
    let mut service = policy();
    service.policy.max_query_length = 1000;
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let prepared = prepare_create(
        &p,
        "query-probe",
        "saved-one",
        &json!({"query":"q".repeat(20),"engines":["brave"],"interval_seconds":60}),
        1000.0,
    )
    .unwrap();
    let current = prepared.definition().clone();
    service.policy.max_query_length = 10;
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let out = prepare_update(&p, &current, &json!({"max_reports":1}), 1001.0);
    println!("unchanged_query20_loweredmax10={:?}", out);
    assert!(out.is_ok(), "source checks supplied query only");
}

// Synchronous token injection occurs after the baseline read and before TIME/CAS.
// It gives deterministic interleavings without production hooks or sleeps.
struct BeforeClaim<F: FnMut()>(F);
impl<F: FnMut()> TokenSource<SavedSearchId> for BeforeClaim<F> {
    fn next_token(&mut self) -> LeaseToken<SavedSearchId> {
        (self.0)();
        LeaseToken::new("snapshot-owner").unwrap()
    }
}
#[tokio::test]
async fn claim_baseline_snapshot_variants_races_and_fencing() {
    let server = Server::start();
    let store = SavedStore::new(Arc::new(server.store()), TenantId::new("baseline").unwrap());
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let now = store.now().await.unwrap();
    for (name, baseline, expected) in [
        (
            "saved-live",
            Some(json!({"expires_at":now+300.0,"records":{"prior":"opaque"}})),
            true,
        ),
        (
            "saved-expired",
            Some(json!({"expires_at":now-1.0,"records":{}})),
            false,
        ),
        ("saved-missing", None, false),
    ] {
        assert!(
            store
                .create(&create_at(&p, "baseline", name, now))
                .await
                .unwrap()
        );
        if let Some(value) = baseline.as_ref() {
            redis::cmd("SET")
                .arg(format!("mcp:saved:v1:baseline:baseline:{name}"))
                .arg(ssx_json::to_vec(value).unwrap())
                .query::<()>(&mut server.connection())
                .unwrap();
        }
        let claim = store.claim_fresh(&id(name)).await.unwrap().unwrap();
        assert_eq!(
            claim.definition().baseline,
            if expected { baseline } else { None }
        );
        assert!(store.release(&claim).await.unwrap());
        store
            .update(&id(name), 1, &json!({"max_reports":1}), &p)
            .await
            .unwrap();
        assert_eq!(store.renew(&claim).await.unwrap_err(), Error::Conflict);
        assert_eq!(store.release(&claim).await.unwrap_err(), Error::Conflict);
    }
    for (name, bytes) in [
        ("saved-bad", b"not-json".as_slice()),
        (
            "saved-bad-expiry",
            br#"{"expires_at":"invalid"}"#.as_slice(),
        ),
    ] {
        store
            .create(&create_at(&p, "baseline", name, now))
            .await
            .unwrap();
        let key = format!("mcp:saved:v1:definition:baseline:{name}");
        let before: Vec<u8> = redis::cmd("GET")
            .arg(&key)
            .query(&mut server.connection())
            .unwrap();
        redis::cmd("SET")
            .arg(format!("mcp:saved:v1:baseline:baseline:{name}"))
            .arg(bytes)
            .query::<()>(&mut server.connection())
            .unwrap();
        assert_eq!(
            store.claim_fresh(&id(name)).await.unwrap_err(),
            Error::Malformed
        );
        let after: Vec<u8> = redis::cmd("GET")
            .arg(&key)
            .query(&mut server.connection())
            .unwrap();
        assert_eq!(before, after);
        assert!(
            !redis::cmd("EXISTS")
                .arg(format!("mcp:saved:v1:lease:baseline:{name}"))
                .query::<bool>(&mut server.connection())
                .unwrap()
        );
    }
    store
        .create(&create_at(&p, "baseline", "saved-race", now))
        .await
        .unwrap();
    let mut changed = BeforeClaim(|| {
        redis::cmd("SET")
            .arg("mcp:saved:v1:baseline:baseline:saved-race")
            .arg(ssx_json::to_vec(&json!({"expires_at":now+300.0,"records":{}})).unwrap())
            .query::<()>(&mut server.connection())
            .unwrap();
    });
    assert!(
        store
            .claim(&id("saved-race"), &mut changed)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        !redis::cmd("EXISTS")
            .arg("mcp:saved:v1:lease:baseline:saved-race")
            .query::<bool>(&mut server.connection())
            .unwrap()
    );
    let claim = store.claim_fresh(&id("saved-race")).await.unwrap().unwrap();
    assert!(claim.definition().baseline.is_some());
    store.delete(&id("saved-race"), 1).await.unwrap();
    assert_eq!(store.renew(&claim).await.unwrap_err(), Error::Missing);
    assert_eq!(store.release(&claim).await.unwrap_err(), Error::Missing);
}
#[tokio::test]
async fn claim_read_failure_and_unknown_commit_preserve_authority() {
    let server = Server::start();
    let backend = Arc::new(Valkey::new(&server.url, Duration::from_millis(30)).unwrap());
    backend.ping().await.unwrap();
    let store = SavedStore::new(backend, TenantId::new("claim-outcome").unwrap());
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let now = store.now().await.unwrap();
    for name in ["saved-read", "saved-write"] {
        store
            .create(&create_at(&p, "claim-outcome", name, now))
            .await
            .unwrap();
    }
    let mut read_paused = BeforeClaim(|| {
        redis::cmd("CLIENT")
            .arg("PAUSE")
            .arg(150)
            .arg("ALL")
            .query::<()>(&mut server.connection())
            .unwrap();
    });
    assert_eq!(
        store
            .claim(&id("saved-read"), &mut read_paused)
            .await
            .unwrap_err(),
        Error::Store(Failure::Unavailable)
    );
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert!(
        !redis::cmd("EXISTS")
            .arg("mcp:saved:v1:lease:claim-outcome:saved-read")
            .query::<bool>(&mut server.connection())
            .unwrap()
    );
    let baseline = json!({"expires_at":now+300.0,"records":{"previous":"opaque"}});
    redis::cmd("SET")
        .arg("mcp:saved:v1:baseline:claim-outcome:saved-write")
        .arg(ssx_json::to_vec(&baseline).unwrap())
        .query::<()>(&mut server.connection())
        .unwrap();
    let mut write_paused = BeforeClaim(|| {
        redis::cmd("CLIENT")
            .arg("PAUSE")
            .arg(150)
            .arg("WRITE")
            .query::<()>(&mut server.connection())
            .unwrap();
    });
    assert_eq!(
        store
            .claim(&id("saved-write"), &mut write_paused)
            .await
            .unwrap_err(),
        Error::Store(Failure::CompletionUnknown)
    );
    tokio::time::sleep(Duration::from_millis(250)).await;
    let authority = SavedStore::new(
        Arc::new(server.store()),
        TenantId::new("claim-outcome").unwrap(),
    );
    let loaded = authority.load(&id("saved-write")).await.unwrap().unwrap();
    assert_eq!(loaded.baseline, Some(baseline));
    assert_eq!(loaded.lease_token.as_deref(), Some("snapshot-owner"));
    assert!(
        authority
            .claim_fresh(&id("saved-write"))
            .await
            .unwrap()
            .is_none()
    );
}
#[tokio::test]
async fn revision_extremes_and_competing_updates_are_exact() {
    let server = Server::start();
    let store = SavedStore::new(
        Arc::new(server.store()),
        TenantId::new("revision-boundary").unwrap(),
    );
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let now = store.now().await.unwrap();
    for revision in [
        9_007_199_254_740_991_u64,
        9_007_199_254_740_992,
        9_007_199_254_740_993,
        i64::MAX as u64 - 1,
    ] {
        let name = format!("saved-{revision}");
        let mut d = create_at(&p, "revision-boundary", &name, now)
            .definition()
            .clone();
        d.revision = revision;
        let key = format!("mcp:saved:v1:definition:revision-boundary:{name}");
        redis::cmd("SET")
            .arg(&key)
            .arg(ssx_json::to_vec(&d.stored_payload().unwrap()).unwrap())
            .arg("EX")
            .arg(900)
            .query::<()>(&mut server.connection())
            .unwrap();
        let changes = json!({"max_reports":1});
        let identity = id(&name);
        let (a, b) = tokio::join!(
            store.update(&identity, revision, &changes, &p),
            store.update(&identity, revision, &changes, &p)
        );
        assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
        assert!(matches!(a, Err(Error::Conflict)) || matches!(b, Err(Error::Conflict)));
        assert_eq!(
            store.load(&id(&name)).await.unwrap().unwrap().revision,
            revision + 1
        );
        let before: Vec<u8> = redis::cmd("GET")
            .arg(&key)
            .query(&mut server.connection())
            .unwrap();
        assert_eq!(
            store
                .update(&id(&name), revision, &changes, &p)
                .await
                .unwrap_err(),
            Error::Conflict
        );
        if revision == i64::MAX as u64 - 1 {
            assert!(matches!(
                store.update(&id(&name), revision + 1, &changes, &p).await,
                Err(Error::Invalid(_))
            ));
        }
        let after: Vec<u8> = redis::cmd("GET")
            .arg(&key)
            .query(&mut server.connection())
            .unwrap();
        assert_eq!(before, after);
    }
}

#[test]
fn supplied_query_bounds_and_unchanged_query_keep_engine_authority() {
    let mut service = policy();
    service.policy.max_query_length = 1000;
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let input = json!({"query":format!("{}{}"," ".repeat(401),"q".repeat(600)),"engines":["brave"],"interval_seconds":60});
    assert!(prepare_create(&p, "query", "saved-raw", &input, 1000.0).is_err());
    let input =
        json!({"query":format!(" {} ","q".repeat(600)),"engines":["brave"],"interval_seconds":60});
    let current = prepare_create(&p, "query", "saved-good", &input, 1000.0)
        .unwrap()
        .definition()
        .clone();
    assert_eq!(current.query, "q".repeat(600));
    service.policy.max_query_length = 10;
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    assert!(prepare_update(&p, &current, &json!({"query":"q".repeat(600)}), 1001.0).is_err());
    assert!(prepare_update(&p, &current, &json!({"query":"          q"}), 1001.0).is_err());
    assert_eq!(
        prepare_update(&p, &current, &json!({"max_reports":1}), 1001.0)
            .unwrap()
            .query,
        current.query
    );
    let denied = SavedPolicy {
        saved_granted: false,
        ..p
    };
    assert!(prepare_update(&denied, &current, &json!({"max_reports":1}), 1001.0).is_err());
    let mut caps = service.catalog.engines().to_vec();
    caps[0].active = false;
    service.catalog = CatalogSnapshot::new(caps).unwrap();
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    assert!(prepare_update(&p, &current, &json!({"max_reports":1}), 1001.0).is_err());
    let source: ssx_json::Value =
        ssx_json::from_str(include_str!("../../../docs/rust-rewrite/w00/source.json")).unwrap();
    let rows = source["review_observations"].as_array().unwrap();
    assert_eq!(
        rows.iter()
            .find(|r| r["name"] == "configured_query_create")
            .unwrap()["outcome"]["query"]
            .as_str()
            .unwrap(),
        current.query
    );
    assert_eq!(
        rows.iter()
            .find(|r| r["name"] == "untouched_query_lowered_limit")
            .unwrap()["outcome"]["query"]
            .as_str()
            .unwrap(),
        current.query
    );
    assert!(
        rows.iter()
            .find(|r| r["name"] == "raw_query_before_strip")
            .unwrap()["outcome"]["error"]
            .is_object()
    );
    assert!(
        rows.iter()
            .find(|r| r["name"] == "supplied_query_lowered_limit")
            .unwrap()["outcome"]["error"]
            .is_object()
    );
}

#[tokio::test]
async fn independent_max_revision_delete_and_repair() {
    let server = Server::start();
    let store = SavedStore::new(
        Arc::new(server.store()),
        TenantId::new("max-probe").unwrap(),
    );
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let now = store.now().await.unwrap();
    let mut d = create_at(&p, "max-probe", "saved-one", now)
        .definition()
        .clone();
    d.revision = i64::MAX as u64 - 1;
    redis::cmd("SET")
        .arg("mcp:saved:v1:definition:max-probe:saved-one")
        .arg(ssx_json::to_vec(&d.stored_payload().unwrap()).unwrap())
        .arg("EX")
        .arg(900)
        .query::<()>(&mut server.connection())
        .unwrap();
    let updated = store
        .update(&id("saved-one"), d.revision, &json!({"max_reports":1}), &p)
        .await
        .unwrap();
    assert_eq!(updated.revision, i64::MAX as u64);
    let repair = store.repair_indexes().await;
    println!("max_revision_repair={repair:?}");
    let delete = store.delete(&id("saved-one"), updated.revision).await;
    println!("max_revision_delete={delete:?}");
    assert!(delete.is_ok(), "valid max revision remains deletable");
    assert!(
        repair.is_ok(),
        "valid maxrevision must not poison tenant repair"
    );
}
#[tokio::test]
async fn independent_nonobject_baseline_refuses_before_claim() {
    let server = Server::start();
    let store = SavedStore::new(
        Arc::new(server.store()),
        TenantId::new("shape-probe").unwrap(),
    );
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let now = store.now().await.unwrap();
    store
        .create(&create_at(&p, "shape-probe", "saved-one", now))
        .await
        .unwrap();
    redis::cmd("SET")
        .arg("mcp:saved:v1:baseline:shape-probe:saved-one")
        .arg("[]")
        .arg("EX")
        .arg(900)
        .query::<()>(&mut server.connection())
        .unwrap();
    println!(
        "load_array_baseline={:?}",
        store.load(&id("saved-one")).await
    );
    let claim = store.claim_fresh(&id("saved-one")).await;
    println!("claim_array_baseline={claim:?}");
    assert!(
        matches!(claim, Err(Error::Malformed)),
        "malformed baseline must not publish lease"
    );
}

#[tokio::test]
async fn max_revision_full_lifecycle_repair_quota_and_true_overflow() {
    let server = Server::start();
    let store = SavedStore::new(Arc::new(server.store()), TenantId::new("max-life").unwrap());
    let service = policy();
    let limits = SavedLimits {
        definitions: 2,
        ..Default::default()
    };
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let now = store.now().await.unwrap();
    store
        .create(&create_at(&p, "max-life", "saved-repair-placeholder", now))
        .await
        .unwrap();
    let old_claim = store
        .claim_fresh(&id("saved-repair-placeholder"))
        .await
        .unwrap()
        .unwrap();
    let mut d = store
        .load(&id("saved-repair-placeholder"))
        .await
        .unwrap()
        .unwrap();
    d.revision = i64::MAX as u64 - 1;
    let key = "mcp:saved:v1:definition:max-life:saved-repair-placeholder";
    redis::cmd("SET")
        .arg(key)
        .arg(ssx_json::to_vec(&d.stored_payload().unwrap()).unwrap())
        .arg("EX")
        .arg(900)
        .query::<()>(&mut server.connection())
        .unwrap();
    store
        .update(
            &id("saved-repair-placeholder"),
            d.revision,
            &json!({"max_reports":1}),
            &p,
        )
        .await
        .unwrap();
    assert!(
        !store
            .create(&create_at(&p, "max-life", "saved-repair-placeholder", now))
            .await
            .unwrap()
    );
    assert_eq!(store.renew(&old_claim).await.unwrap_err(), Error::Conflict);
    assert_eq!(
        store.release(&old_claim).await.unwrap_err(),
        Error::Conflict
    );
    assert_eq!(store.repair_indexes().await.unwrap(), 1);
    redis::cmd("DEL")
        .arg("mcp:saved:v1:definition-count:max-life")
        .query::<()>(&mut server.connection())
        .unwrap();
    assert!(
        store
            .create(&create_at(&p, "max-life", "saved-ordinary", now))
            .await
            .unwrap()
    );
    assert_eq!(store.repair_indexes().await.unwrap(), 2);
    assert_eq!(
        store
            .create(&create_at(&p, "max-life", "saved-blocked", now))
            .await
            .unwrap_err(),
        Error::QuotaExceeded
    );
    store
        .delete(&id("saved-repair-placeholder"), i64::MAX as u64)
        .await
        .unwrap();
    assert_eq!(
        redis::cmd("GET")
            .arg("mcp:saved:v1:deleted:max-life:saved-repair-placeholder")
            .query::<String>(&mut server.connection())
            .unwrap(),
        i64::MAX.to_string()
    );
    assert!(
        store
            .create(&create_at(&p, "max-life", "saved-replacement", now))
            .await
            .unwrap()
    );
    assert_eq!(store.repair_indexes().await.unwrap(), 2);
    let mut bad = store
        .load(&id("saved-replacement"))
        .await
        .unwrap()
        .unwrap()
        .stored_payload()
        .unwrap();
    bad["revision"] = json!(i64::MAX as u64 + 1);
    let key = "mcp:saved:v1:definition:max-life:saved-replacement";
    let raw = ssx_json::to_vec(&bad).unwrap();
    redis::cmd("SET")
        .arg(key)
        .arg(&raw)
        .arg("EX")
        .arg(900)
        .query::<()>(&mut server.connection())
        .unwrap();
    let indexes = ["definitions", "due", "definition-count"]
        .map(|kind| format!("mcp:saved:v1:{kind}:max-life"));
    let before: Vec<Option<Vec<u8>>> = indexes
        .iter()
        .map(|k| {
            redis::cmd("DUMP")
                .arg(k)
                .query(&mut server.connection())
                .unwrap()
        })
        .collect();
    assert_eq!(store.repair_indexes().await.unwrap_err(), Error::Malformed);
    assert_eq!(
        store
            .delete(&id("saved-replacement"), i64::MAX as u64 + 1)
            .await
            .unwrap_err(),
        Error::Malformed
    );
    let after: Vec<Option<Vec<u8>>> = indexes
        .iter()
        .map(|k| {
            redis::cmd("DUMP")
                .arg(k)
                .query(&mut server.connection())
                .unwrap()
        })
        .collect();
    assert_eq!(before, after);
    assert_eq!(
        redis::cmd("GET")
            .arg(key)
            .query::<Vec<u8>>(&mut server.connection())
            .unwrap(),
        raw
    );
    let source = final_review_rows()
        .into_iter()
        .find(|r| r["name"] == "maximum_revision_lifecycle")
        .unwrap();
    assert_eq!(source["updated_revision"].as_u64(), Some(i64::MAX as u64));
    assert_eq!(
        source["tombstone"].as_str(),
        Some(i64::MAX.to_string().as_str())
    );
    assert_eq!(source["repaired"], json!(1));
    assert_eq!(source["final_count"], json!(2));
    assert_eq!(source["subsequent_admitted"], json!(["created", "created"]));
}
fn repair_eval(
    server: &Server,
    tenant: &str,
    attempt: u8,
    proofs: &[(String, Vec<u8>)],
) -> redis::Value {
    let mut cmd = redis::cmd("EVAL");
    cmd.arg(include_str!("../src/valkey/saved_scripts/transition.lua"))
        .arg(10);
    for kind in [
        "definition",
        "definitions",
        "due",
        "tenants",
        "definition-count",
        "recent",
        "lease",
        "deleted",
        "baseline",
        "reports",
    ] {
        let key = match kind {
            "tenants" => "mcp:saved:v1:tenants".to_string(),
            "definition" | "lease" | "deleted" | "baseline" | "reports" => {
                format!("mcp:saved:v1:{kind}:{tenant}:saved-repair-placeholder")
            }
            _ => format!("mcp:saved:v1:{kind}:{tenant}"),
        };
        cmd.arg(key);
    }
    cmd.arg("repair")
        .arg(tenant)
        .arg("saved-repair-placeholder")
        .arg("")
        .arg("")
        .arg(0)
        .arg("")
        .arg(0)
        .arg(keys::literal_scan_prefix(&format!(
            "mcp:saved:v1:definition:{tenant}:"
        )))
        .arg("")
        .arg("")
        .arg("missing")
        .arg(0)
        .arg(attempt);
    for (key, raw) in proofs {
        cmd.arg(key).arg(raw);
    }
    cmd.query(&mut server.connection()).unwrap()
}
#[tokio::test]
async fn repair_exact_proof_replacement_or_new_boundary_conflicts_without_writes() {
    let server = Server::start();
    let store = SavedStore::new(
        Arc::new(server.store()),
        TenantId::new("proof-race").unwrap(),
    );
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let now = store.now().await.unwrap();
    let mut d = create_at(&p, "proof-race", "saved-one", now)
        .definition()
        .clone();
    d.revision = i64::MAX as u64;
    let key = "mcp:saved:v1:definition:proof-race:saved-one";
    let raw = ssx_json::to_vec(&d.stored_payload().unwrap()).unwrap();
    redis::cmd("SET")
        .arg(key)
        .arg(&raw)
        .arg("EX")
        .arg(900)
        .query::<()>(&mut server.connection())
        .unwrap();
    redis::cmd("SET")
        .arg("mcp:saved:v1:definition-count:proof-race")
        .arg(77)
        .query::<()>(&mut server.connection())
        .unwrap();
    let (status, proofs): (i64, Vec<(String, Vec<u8>)>) =
        redis::from_redis_value(repair_eval(&server, "proof-race", 0, &[])).unwrap();
    assert_eq!(status, -6);
    assert_eq!(proofs, vec![(key.into(), raw.clone())]);
    assert_eq!(
        redis::cmd("GET")
            .arg("mcp:saved:v1:definition-count:proof-race")
            .query::<u64>(&mut server.connection())
            .unwrap(),
        77
    );
    d.query = "replaced".into();
    let replaced = ssx_json::to_vec(&d.stored_payload().unwrap()).unwrap();
    redis::cmd("SET")
        .arg(key)
        .arg(&replaced)
        .arg("EX")
        .arg(900)
        .query::<()>(&mut server.connection())
        .unwrap();
    let (status, _): (i64, Vec<u8>) =
        redis::from_redis_value(repair_eval(&server, "proof-race", 1, &proofs)).unwrap();
    assert_eq!(status, -2);
    assert_eq!(
        redis::cmd("GET")
            .arg(key)
            .query::<Vec<u8>>(&mut server.connection())
            .unwrap(),
        replaced
    );
    redis::cmd("SET")
        .arg(key)
        .arg(&raw)
        .arg("EX")
        .arg(900)
        .query::<()>(&mut server.connection())
        .unwrap();
    d.search_id = "saved-new".into();
    redis::cmd("SET")
        .arg("mcp:saved:v1:definition:proof-race:saved-new")
        .arg(ssx_json::to_vec(&d.stored_payload().unwrap()).unwrap())
        .arg("EX")
        .arg(900)
        .query::<()>(&mut server.connection())
        .unwrap();
    let (status, _): (i64, Vec<u8>) =
        redis::from_redis_value(repair_eval(&server, "proof-race", 1, &proofs)).unwrap();
    assert_eq!(status, -2);
    assert_eq!(
        redis::cmd("GET")
            .arg("mcp:saved:v1:definition-count:proof-race")
            .query::<u64>(&mut server.connection())
            .unwrap(),
        77
    );
    for kind in ["definitions", "due"] {
        assert!(
            !redis::cmd("EXISTS")
                .arg(format!("mcp:saved:v1:{kind}:proof-race"))
                .query::<bool>(&mut server.connection())
                .unwrap()
        );
    }
}
#[tokio::test]
async fn baseline_object_shape_matches_load_before_any_lease() {
    let server = Server::start();
    let store = SavedStore::new(
        Arc::new(server.store()),
        TenantId::new("baseline-shapes").unwrap(),
    );
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let now = store.now().await.unwrap();
    for (n, value) in [
        json!([]),
        json!([1]),
        json!(0),
        json!(true),
        json!(""),
        json!(null),
    ]
    .into_iter()
    .enumerate()
    {
        let row = final_review_rows()
            .into_iter()
            .find(|r| r["input"] == value && r["stored"] == true)
            .unwrap();
        if value.is_null() {
            assert_eq!(row["lease_exists"], json!(true));
        } else {
            assert_eq!(row["outcome"]["error_type"], json!("AttributeError"));
            assert_eq!(row["lease_exists"], json!(false));
        }
        let name = format!("saved-{n}");
        store
            .create(&create_at(&p, "baseline-shapes", &name, now))
            .await
            .unwrap();
        let record = format!("mcp:saved:v1:definition:baseline-shapes:{name}");
        let baseline = format!("mcp:saved:v1:baseline:baseline-shapes:{name}");
        let lease = format!("mcp:saved:v1:lease:baseline-shapes:{name}");
        let before: Vec<u8> = redis::cmd("GET")
            .arg(&record)
            .query(&mut server.connection())
            .unwrap();
        let bytes = ssx_json::to_vec(&value).unwrap();
        redis::cmd("SET")
            .arg(&baseline)
            .arg(&bytes)
            .query::<()>(&mut server.connection())
            .unwrap();
        assert_eq!(store.load(&id(&name)).await.unwrap_err(), Error::Malformed);
        assert_eq!(
            store.claim_fresh(&id(&name)).await.unwrap_err(),
            Error::Malformed
        );
        assert_eq!(
            redis::cmd("GET")
                .arg(&record)
                .query::<Vec<u8>>(&mut server.connection())
                .unwrap(),
            before
        );
        assert_eq!(
            redis::cmd("GET")
                .arg(&baseline)
                .query::<Vec<u8>>(&mut server.connection())
                .unwrap(),
            bytes
        );
        assert!(
            !redis::cmd("EXISTS")
                .arg(&lease)
                .query::<bool>(&mut server.connection())
                .unwrap()
        );
        redis::cmd("SET")
            .arg(&baseline)
            .arg("{}")
            .query::<()>(&mut server.connection())
            .unwrap();
        assert!(
            store
                .load(&id(&name))
                .await
                .unwrap()
                .unwrap()
                .baseline
                .is_none()
        );
        let claim = store.claim_fresh(&id(&name)).await.unwrap().unwrap();
        assert!(claim.definition().baseline.is_none());
        assert!(store.release(&claim).await.unwrap());
    }
    store
        .create(&create_at(&p, "baseline-shapes", "saved-absent", now))
        .await
        .unwrap();
    let claim = store
        .claim_fresh(&id("saved-absent"))
        .await
        .unwrap()
        .unwrap();
    assert!(claim.definition().baseline.is_none());
}

#[tokio::test]
async fn maximum_revision_repair_proof_keeps_exact_byte_budgets() {
    let server = Server::start();
    let store = SavedStore::new(
        Arc::new(server.store()),
        TenantId::new("max-proof-bytes").unwrap(),
    );
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let now = store.now().await.unwrap();
    for name in ["saved-a", "saved-b"] {
        let mut d = create_at(&p, "max-proof-bytes", name, now)
            .definition()
            .clone();
        d.revision = i64::MAX as u64;
        redis::cmd("SET")
            .arg(format!("mcp:saved:v1:definition:max-proof-bytes:{name}"))
            .arg(padded_record(&d, 4 * 1024 * 1024))
            .arg("EX")
            .arg(900)
            .query::<()>(&mut server.connection())
            .unwrap();
    }
    assert_eq!(store.repair_indexes().await.unwrap(), 2);
    let count = "mcp:saved:v1:definition-count:max-proof-bytes";
    let before: Vec<u8> = redis::cmd("DUMP")
        .arg(count)
        .query(&mut server.connection())
        .unwrap();
    let mut d = create_at(&p, "max-proof-bytes", "saved-c", now)
        .definition()
        .clone();
    d.revision = i64::MAX as u64;
    let key = "mcp:saved:v1:definition:max-proof-bytes:saved-c";
    redis::cmd("SET")
        .arg(key)
        .arg(ssx_json::to_vec(&d.stored_payload().unwrap()).unwrap())
        .arg("EX")
        .arg(900)
        .query::<()>(&mut server.connection())
        .unwrap();
    assert_eq!(
        store.repair_indexes().await.unwrap_err(),
        Error::RepairIncomplete
    );
    assert_eq!(
        redis::cmd("DUMP")
            .arg(count)
            .query::<Vec<u8>>(&mut server.connection())
            .unwrap(),
        before
    );
    redis::cmd("SET")
        .arg(key)
        .arg(padded_record(&d, 4 * 1024 * 1024 + 1))
        .arg("EX")
        .arg(900)
        .query::<()>(&mut server.connection())
        .unwrap();
    assert_eq!(
        store.repair_indexes().await.unwrap_err(),
        Error::RepairIncomplete
    );
    assert_eq!(
        redis::cmd("DUMP")
            .arg(count)
            .query::<Vec<u8>>(&mut server.connection())
            .unwrap(),
        before
    );
}

fn final_review_rows() -> Vec<ssx_json::Value> {
    let source: ssx_json::Value =
        ssx_json::from_str(include_str!("../../../docs/rust-rewrite/w00/source.json")).unwrap();
    source["final_review_observations"]
        .as_array()
        .unwrap()
        .clone()
}
