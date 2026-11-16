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
fn publication(claim: &Claim, now: f64) -> (ssx_json::Value, Option<ssx_json::Value>, f64) {
    let d = claim.definition();
    let slot = d.slot(now).unwrap();
    let observation = observe(
        d,
        &ssx_domain::SearchResponse {
            scope: ssx_domain::ScopeDecision {
                selected_engines: d.engines.clone(),
                ..Default::default()
            },
            engine_outcomes: vec![ssx_domain::EngineOutcome {
                engine: "brave".into(),
                status: "ok".into(),
                ..Default::default()
            }],
            ..Default::default()
        },
        now,
    )
    .unwrap();
    let (mut report, baseline) = compare_observations(
        d,
        observation,
        &slot.run_id,
        slot.scheduled_at.get(),
        now,
        now,
        slot.missed_slots,
    )
    .unwrap();
    report["slot"] = json!(slot.slot);
    (report, baseline, slot.following_due.get())
}
#[tokio::test]
async fn publication_is_atomic_fenced_and_tenant_bound() {
    let server = Server::start();
    let backend = Arc::new(server.store());
    let store = SavedStore::new(backend.clone(), TenantId::new("tenant").unwrap());
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let prepared = create_at(&p, "tenant", "saved-a", store.now().await.unwrap());
    store.create(&prepared).await.unwrap();
    let claim = store
        .claim(&id("saved-a"), &mut Tokens("one"))
        .await
        .unwrap()
        .unwrap();
    let (report, baseline, next) = publication(&claim, store.now().await.unwrap());
    let (first, duplicate) = tokio::join!(
        store.commit(
            &claim,
            &report,
            baseline.as_ref(),
            next,
            &claim.definition().policy_fingerprint
        ),
        store.commit(
            &claim,
            &report,
            baseline.as_ref(),
            next,
            &claim.definition().policy_fingerprint
        )
    );
    assert_eq!(
        usize::from(first.unwrap()) + usize::from(duplicate.unwrap()),
        1
    );
    let loaded = store.load(&id("saved-a")).await.unwrap().unwrap();
    assert_eq!(loaded.baseline, baseline);
    assert_eq!(loaded.latest_run_id.as_deref(), report["run_id"].as_str());
    assert_eq!(
        store
            .report(&id("saved-a"), report["run_id"].as_str().unwrap(), &p)
            .await
            .unwrap(),
        Some(report.clone())
    );
    let other = store.for_tenant(TenantId::new("other").unwrap());
    assert!(
        other
            .commit(
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
        other
            .report(&id("saved-a"), report["run_id"].as_str().unwrap(), &p)
            .await
            .unwrap()
            .is_none()
    );
    let denied = SavedPolicy {
        saved_granted: false,
        ..p
    };
    assert!(
        store
            .report(&id("saved-a"), report["run_id"].as_str().unwrap(), &denied)
            .await
            .is_err()
    );
    backend.close().await;
}
#[tokio::test]
async fn update_pause_delete_reclaim_and_malformed_key_preflight_prevent_partial_writes() {
    let server = Server::start();
    let backend = Arc::new(server.store());
    let store = SavedStore::new(backend.clone(), TenantId::new("tenant").unwrap());
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    for action in ["update", "pause", "delete", "reclaim", "malformed"] {
        let name = format!("saved-{action}");
        let saved = id(&name);
        store
            .create(&create_at(&p, "tenant", &name, store.now().await.unwrap()))
            .await
            .unwrap();
        let claim = store
            .claim(&saved, &mut Tokens("old"))
            .await
            .unwrap()
            .unwrap();
        let (report, baseline, next) = publication(&claim, store.now().await.unwrap());
        match action {
            "update" => {
                store
                    .update(&saved, 1, &json!({"query":"new"}), &p)
                    .await
                    .unwrap();
            }
            "pause" => {
                store.pause(&saved, 1, true, &p).await.unwrap();
            }
            "delete" => {
                store.delete(&saved, 1).await.unwrap();
            }
            "reclaim" => {
                redis::cmd("DEL")
                    .arg(format!("mcp:saved:v1:lease:tenant:{name}"))
                    .query::<()>(&mut server.connection())
                    .unwrap();
                assert!(
                    store
                        .claim(&saved, &mut Tokens("new"))
                        .await
                        .unwrap()
                        .is_some()
                );
            }
            "malformed" => {
                redis::cmd("SET")
                    .arg("mcp:saved:v1:due:tenant")
                    .arg("wrong-kind")
                    .query::<()>(&mut server.connection())
                    .unwrap();
            }
            _ => unreachable!(),
        }
        let result = store
            .commit(
                &claim,
                &report,
                baseline.as_ref(),
                next,
                &claim.definition().policy_fingerprint,
            )
            .await;
        assert!(!matches!(result, Ok(true)), "{action}");
        let key = format!(
            "mcp:saved:v1:report:tenant:{name}:{}",
            report["run_id"].as_str().unwrap()
        );
        assert_eq!(
            redis::cmd("EXISTS")
                .arg(key)
                .query::<u64>(&mut server.connection())
                .unwrap(),
            0
        );
    }
    backend.close().await;
}

use ssx_application::{NoopSink, PortFuture, SearchEngine};
use ssx_domain::{AdapterResponse, SearchRequest, SearchResult};
use ssx_infra::{saved_runner::*, search::*};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::sync::{Notify, watch};
fn capability(name: &str) -> EngineCapability {
    EngineCapability {
        name: name.into(),
        enabled: true,
        active: true,
        available: true,
        categories: vec!["general".into()],
        media_types: vec![],
        auth_ready: true,
        circuit_open: false,
        cost_class: "free".into(),
        health_status: "unknown".into(),
        health_stale: true,
        enforced_filters: BTreeMap::new(),
    }
}

fn dependencies(engines: Vec<(&str, Arc<dyn SearchEngine>)>) -> SearchDependencies {
    SearchDependencies {
        policy: PolicyService {
            policy: SearchPolicy::default(),
            catalog: CatalogSnapshot::new(engines.iter().map(|(n, _)| capability(n)).collect())
                .unwrap(),
            resolver: ScopeResolver {
                catalog_routing: false,
                ..Default::default()
            },
        },
        engines: engines
            .into_iter()
            .map(|(n, engine)| {
                (
                    n.into(),
                    EngineBinding {
                        engine,
                        generation: "fixture-v1".into(),
                        timeout: Duration::from_secs(3),
                        scrape: false,
                    },
                )
            })
            .collect(),
        routing_identity: RoutingIdentity {
            catalog: None,
            sensitive: ["hibp", "dehashed"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            tier1: ScopeResolver::default().tier1,
            budget: None,
            ranking_strategy: "presence".into(),
            work_group_version: "scholarly-work-v3".into(),
            rerank_identity: None,
            routing_extension: None,
        },
        cache: None,
        client_admission: None,
        suggestions: None,
        jev: None,
        routing_cache: None,
        metrics: Arc::new(NoopSink),
        supervisor: Arc::new(NoopSink),
        audit: Arc::new(NoopSink),
        cooldowns: None,
        owned_http: vec![],
        owned_stores: vec![],
        clock: Arc::new(SystemSearchClock),
        reranker: None,
    }
}
struct HeldEngine {
    entered: Notify,
    release: Notify,
    drops: AtomicUsize,
    calls: AtomicUsize,
}
struct DropGuard<'a>(&'a AtomicUsize);
impl Drop for DropGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
impl SearchEngine for HeldEngine {
    fn search<'a>(&'a self, _: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let _guard = DropGuard(&self.drops);
            self.entered.notify_one();
            self.release.notified().await;
            AdapterResponse {
                results: vec![SearchResult {
                    url: "https://source.test".into(),
                    engine: "brave".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }
        })
    }
}
struct MutableAuthority {
    enabled: AtomicBool,
    fingerprint: String,
}
struct PopulatedCache(AtomicUsize);
impl ssx_application::cache::CacheRepository for PopulatedCache {
    fn read<'a>(
        &'a self,
        _: &'a str,
    ) -> PortFuture<
        'a,
        Result<Option<ssx_application::cache::CacheEntry>, ssx_application::cache::CacheFailure>,
    > {
        Box::pin(async move {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(Some(ssx_application::cache::CacheEntry::Canonical(
                ssx_application::cache::CanonicalResponse::new(
                    ssx_domain::SearchResponse::default(),
                ),
            )))
        })
    }
    fn write<'a>(
        &'a self,
        _: &'a str,
        _: &'a ssx_application::cache::CacheEntry,
        _: std::num::NonZeroU64,
    ) -> PortFuture<'a, Result<(), ssx_application::cache::CacheFailure>> {
        Box::pin(async { Ok(()) })
    }
}
impl SavedAuthority for MutableAuthority {
    fn fingerprint(
        &self,
        _: &ssx_domain::saved::SavedDefinition,
    ) -> Result<String, ssx_domain::DomainError> {
        if self.enabled.load(Ordering::SeqCst) {
            Ok(self.fingerprint.clone())
        } else {
            Err(ssx_domain::DomainError("revoked"))
        }
    }
}
#[tokio::test]
async fn runner_revocation_shutdown_outer_drop_and_uncached_execution() {
    for mode in [
        "denied",
        "fingerprint",
        "revoked",
        "shutdown",
        "outerdrop",
        "success",
    ] {
        let server = Server::start();
        let backend = Arc::new(server.store());
        let store = SavedStore::new(backend.clone(), TenantId::new("tenant").unwrap());
        let service = policy();
        let limits = SavedLimits::default();
        let auth = BTreeMap::from([("brave".into(), false)]);
        let p = SavedPolicy {
            search: &service,
            saved_granted: true,
            auth_configured: &auth,
            limits: &limits,
        };
        let prepared = create_at(&p, "tenant", "saved-a", store.now().await.unwrap());
        store.create(&prepared).await.unwrap();
        let authority = Arc::new(MutableAuthority {
            enabled: AtomicBool::new(mode != "denied"),
            fingerprint: if mode == "fingerprint" {
                "changed".into()
            } else {
                prepared.definition().policy_fingerprint.clone()
            },
        });
        let engine = Arc::new(HeldEngine {
            entered: Notify::new(),
            release: Notify::new(),
            drops: AtomicUsize::new(0),
            calls: AtomicUsize::new(0),
        });
        let cached = Arc::new(PopulatedCache(AtomicUsize::new(0)));
        let mut deps = dependencies(vec![("brave", engine.clone())]);
        deps.cache = Some(Arc::new(ssx_application::cache::CanonicalCache::new(
            cached.clone(),
            Default::default(),
        )));
        let search = SearchService::new(SearchConfig::default(), deps).unwrap();
        let runner = SavedRunner::new(
            store.clone(),
            search.clone(),
            authority.clone(),
            Duration::from_secs(2),
        )
        .unwrap();
        let (stop, rx) = watch::channel(false);
        let task = tokio::spawn(async move { runner.run_one(id("saved-a"), rx).await });
        if matches!(mode, "denied" | "fingerprint") {
            assert!(matches!(task.await.unwrap(), Err(RunnerError::Policy)));
            assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
            let loaded = store.load(&id("saved-a")).await.unwrap().unwrap();
            assert!(loaded.baseline.is_none() && loaded.latest_run_id.is_none());
            search.shutdown().await;
            backend.close().await;
            continue;
        }
        tokio::time::timeout(Duration::from_secs(2), engine.entered.notified())
            .await
            .unwrap();
        match mode {
            "revoked" => {
                authority.enabled.store(false, Ordering::SeqCst);
                engine.release.notify_one();
            }
            "shutdown" => {
                stop.send_replace(true);
            }
            "outerdrop" => {
                task.abort();
            }
            _ => engine.release.notify_one(),
        }
        if mode != "outerdrop" {
            let result = task.await.unwrap();
            if mode == "success" {
                let report = result.unwrap().unwrap();
                assert_eq!(report["status"], "baseline_initialized");
                assert_eq!(report["observation"]["cached"], false);
                assert_eq!(report["observation"]["discovered_count"], 1);
            } else {
                assert!(matches!(
                    result,
                    Err(RunnerError::Policy | RunnerError::Shutdown)
                ));
            }
        } else {
            let _ = task.await;
        }
        tokio::time::timeout(Duration::from_secs(2), async {
            while engine.drops.load(Ordering::SeqCst) != 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let loaded = store.load(&id("saved-a")).await.unwrap().unwrap();
        assert_eq!(loaded.latest_run_id.is_some(), mode == "success");
        assert_eq!(loaded.baseline.is_some(), mode == "success");
        assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            cached.0.load(Ordering::SeqCst),
            0,
            "scheduled acquisition bypasses populated canonical cache"
        );
        search.shutdown().await;
        backend.close().await;
    }
}

use ssx_application::cache::RoutingIdentity;
struct SocketEngine {
    url: String,
}
impl SearchEngine for SocketEngine {
    fn search<'a>(&'a self, _: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            let _ = reqwest::Client::new().get(&self.url).send().await;
            AdapterResponse::default()
        })
    }
}
#[tokio::test]
async fn held_loopback_renews_authority_and_lease_loss_joins_only_owned_socket() {
    use tokio::{io::AsyncReadExt, net::TcpListener};
    for lose in [false, true] {
        let server = Server::start();
        let backend = Arc::new(server.store());
        let store = SavedStore::new(backend.clone(), TenantId::new("tenant").unwrap());
        let service = policy();
        let limits = SavedLimits::default();
        let auth = BTreeMap::from([("brave".into(), false)]);
        let p = SavedPolicy {
            search: &service,
            saved_granted: true,
            auth_configured: &auth,
            limits: &limits,
        };
        let prepared = create_at(&p, "tenant", "saved-a", store.now().await.unwrap());
        store.create(&prepared).await.unwrap();
        let authority = Arc::new(MutableAuthority {
            enabled: AtomicBool::new(true),
            fingerprint: prepared.definition().policy_fingerprint.clone(),
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let engine = Arc::new(SocketEngine {
            url: format!("http://{address}/held"),
        });
        let search = SearchService::new(
            SearchConfig::default(),
            dependencies(vec![("brave", engine)]),
        )
        .unwrap();
        let runner = SavedRunner::new(
            store.clone(),
            search.clone(),
            authority,
            Duration::from_secs(3),
        )
        .unwrap()
        .with_renewal_interval(Duration::from_millis(40))
        .unwrap();
        let (stop, rx) = watch::channel(false);
        let task = tokio::spawn(async move { runner.run_one(id("saved-a"), rx).await });
        let (mut socket, _) = tokio::time::timeout(Duration::from_secs(2), listener.accept())
            .await
            .unwrap()
            .unwrap();
        let mut buf = [0; 2048];
        assert!(socket.read(&mut buf).await.unwrap() > 0);
        if lose {
            redis::cmd("SET")
                .arg("mcp:saved:v1:lease:tenant:saved-a")
                .arg("replacement")
                .arg("EX")
                .arg(10)
                .query::<()>(&mut server.connection())
                .unwrap();
        } else {
            redis::cmd("EXPIRE")
                .arg("mcp:saved:v1:lease:tenant:saved-a")
                .arg(1)
                .query::<()>(&mut server.connection())
                .unwrap();
            tokio::time::sleep(Duration::from_millis(1150)).await;
            assert!(
                redis::cmd("TTL")
                    .arg("mcp:saved:v1:lease:tenant:saved-a")
                    .query::<i64>(&mut server.connection())
                    .unwrap()
                    > 50
            );
            stop.send_replace(true);
        }
        let result = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            result,
            Err(RunnerError::LeaseLost | RunnerError::Shutdown)
        ));
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), socket.read(&mut buf))
                .await
                .unwrap()
                .unwrap(),
            0
        );
        if lose {
            assert_eq!(
                redis::cmd("GET")
                    .arg("mcp:saved:v1:lease:tenant:saved-a")
                    .query::<String>(&mut server.connection())
                    .unwrap(),
                "replacement"
            );
        }
        assert!(
            store
                .load(&id("saved-a"))
                .await
                .unwrap()
                .unwrap()
                .baseline
                .is_none()
        );
        search.shutdown().await;
        backend.close().await;
    }
}
#[test]
fn killed_worker_probe() {
    let Ok(url) = std::env::var("SSX_W04_CHILD_URL") else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let store = SavedStore::new(
            Arc::new(Valkey::new(&url, Duration::from_secs(1)).unwrap()),
            TenantId::new("tenant").unwrap(),
        );
        let claim = store
            .claim(&id("saved-a"), &mut Tokens("killed-owner"))
            .await
            .unwrap()
            .unwrap();
        fs::write(
            std::env::var("SSX_W04_CHILD_READY").unwrap(),
            claim
                .definition()
                .slot(store.now().await.unwrap())
                .unwrap()
                .run_id,
        )
        .unwrap();
        std::future::pending::<()>().await;
    });
}
#[tokio::test]
async fn killed_process_recovery_preserves_slot_and_publishes_once() {
    let server = Server::start();
    let backend = Arc::new(server.store());
    let store = SavedStore::new(backend.clone(), TenantId::new("tenant").unwrap());
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    store
        .create(&create_at(
            &p,
            "tenant",
            "saved-a",
            store.now().await.unwrap(),
        ))
        .await
        .unwrap();
    let ready = server.dir.join("ready");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "killed_worker_probe", "--nocapture"])
        .env("SSX_W04_CHILD_URL", &server.url)
        .env("SSX_W04_CHILD_READY", &ready)
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while !ready.exists() {
            assert!(child.try_wait().unwrap().is_none());
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let run = fs::read_to_string(&ready).unwrap();
    child.kill().unwrap();
    child.wait().unwrap();
    redis::cmd("PEXPIRE")
        .arg("mcp:saved:v1:lease:tenant:saved-a")
        .arg(30)
        .query::<()>(&mut server.connection())
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    let claim = store
        .claim(&id("saved-a"), &mut Tokens("recovered"))
        .await
        .unwrap()
        .unwrap();
    let (report, baseline, next) = publication(&claim, store.now().await.unwrap());
    assert_eq!(report["run_id"], run.as_str());
    assert!(
        store
            .commit(
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
        !store
            .commit(
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
        redis::cmd("ZCARD")
            .arg("mcp:saved:v1:reports:tenant:saved-a")
            .query::<u64>(&mut server.connection())
            .unwrap(),
        1
    );
    backend.close().await;
}
#[tokio::test]
async fn retained_baseline_and_report_eviction_keep_independent_horizons() {
    let server = Server::start();
    let backend = Arc::new(server.store());
    let store = SavedStore::new(backend.clone(), TenantId::new("tenant").unwrap());
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
    store.create(&prepare_create(&p,"tenant","saved-long",&json!({"query":"q","engines":["brave"],"interval_seconds":60,"start_immediately":true,"expires_in_seconds":3600}),now).unwrap()).await.unwrap();
    store.create(&prepare_create(&p,"tenant","saved-short",&json!({"query":"q","engines":["brave"],"interval_seconds":60,"start_immediately":true,"expires_in_seconds":600,"max_reports":1}),now).unwrap()).await.unwrap();
    let claim = store
        .claim(&id("saved-short"), &mut Tokens("first"))
        .await
        .unwrap()
        .unwrap();
    let (report, baseline, next) = publication(&claim, store.now().await.unwrap());
    store
        .commit(
            &claim,
            &report,
            baseline.as_ref(),
            next,
            &claim.definition().policy_fingerprint,
        )
        .await
        .unwrap();
    assert!(
        redis::cmd("TTL")
            .arg("mcp:saved:v1:due:tenant")
            .query::<i64>(&mut server.connection())
            .unwrap()
            > 3500
    );
    // Advance only this deterministic fixture's due instant, without advancing
    // real Valkey time or sleeping a schedule interval.
    let mut record = store.load(&id("saved-short")).await.unwrap().unwrap();
    record.next_due = ssx_domain::FiniteF64::new(now - 1.0).unwrap();
    redis::cmd("SET")
        .arg("mcp:saved:v1:definition:tenant:saved-short")
        .arg(ssx_json::to_vec(&record.stored_payload().unwrap()).unwrap())
        .arg("EX")
        .arg(900)
        .query::<()>(&mut server.connection())
        .unwrap();
    redis::cmd("EXPIRE")
        .arg("mcp:saved:v1:baseline:tenant:saved-short")
        .arg(10)
        .query::<()>(&mut server.connection())
        .unwrap();
    redis::cmd("EXPIRE")
        .arg("mcp:saved:v1:reports:tenant:saved-short")
        .arg(2000)
        .query::<()>(&mut server.connection())
        .unwrap();
    redis::cmd("PERSIST")
        .arg("mcp:saved:v1:due:tenant")
        .query::<()>(&mut server.connection())
        .unwrap();
    let claim = store
        .claim(&id("saved-short"), &mut Tokens("second"))
        .await
        .unwrap()
        .unwrap();
    let (mut second, _, next) = publication(&claim, store.now().await.unwrap());
    second["status"] = json!("incomparable");
    second["incomparable_reasons"] = json!(["cached_observation"]);
    assert!(
        store
            .commit(
                &claim,
                &second,
                None,
                next,
                &claim.definition().policy_fingerprint
            )
            .await
            .unwrap()
    );
    assert_eq!(
        store
            .load(&id("saved-short"))
            .await
            .unwrap()
            .unwrap()
            .baseline,
        baseline
    );
    assert!(
        redis::cmd("TTL")
            .arg("mcp:saved:v1:baseline:tenant:saved-short")
            .query::<i64>(&mut server.connection())
            .unwrap()
            <= 10
    );
    assert!(
        redis::cmd("TTL")
            .arg("mcp:saved:v1:reports:tenant:saved-short")
            .query::<i64>(&mut server.connection())
            .unwrap()
            > 1900
    );
    assert_eq!(
        redis::cmd("TTL")
            .arg("mcp:saved:v1:due:tenant")
            .query::<i64>(&mut server.connection())
            .unwrap(),
        -1
    );
    assert_eq!(
        redis::cmd("EXISTS")
            .arg(format!(
                "mcp:saved:v1:report:tenant:saved-short:{}",
                report["run_id"].as_str().unwrap()
            ))
            .query::<i64>(&mut server.connection())
            .unwrap(),
        0
    );
    assert_eq!(
        redis::cmd("ZCARD")
            .arg("mcp:saved:v1:reports:tenant:saved-short")
            .query::<i64>(&mut server.connection())
            .unwrap(),
        1
    );
    backend.close().await;
}
#[tokio::test]
async fn retained_old_scope_is_revalidated_after_definition_changes() {
    let server = Server::start();
    let backend = Arc::new(server.store());
    let store = SavedStore::new(backend.clone(), TenantId::new("tenant").unwrap());
    let mut service = policy();
    service.catalog =
        CatalogSnapshot::new(vec![capability("brave"), capability("public")]).unwrap();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false), ("public".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    store
        .create(&create_at(
            &p,
            "tenant",
            "saved-a",
            store.now().await.unwrap(),
        ))
        .await
        .unwrap();
    let claim = store
        .claim(&id("saved-a"), &mut Tokens("owner"))
        .await
        .unwrap()
        .unwrap();
    let (report, baseline, next) = publication(&claim, store.now().await.unwrap());
    store
        .commit(
            &claim,
            &report,
            baseline.as_ref(),
            next,
            &claim.definition().policy_fingerprint,
        )
        .await
        .unwrap();
    store
        .update(&id("saved-a"), 1, &json!({"engines":["public"]}), &p)
        .await
        .unwrap();
    let mut revoked = service.clone();
    revoked.policy.sensitive_engines.insert("brave".into());
    revoked.policy.sensitive_allowed = false;
    let denied = SavedPolicy {
        search: &revoked,
        ..p
    };
    assert!(denied.validate_scope("q", &["public".into()]).is_ok());
    assert!(
        store
            .report(&id("saved-a"), report["run_id"].as_str().unwrap(), &denied)
            .await
            .is_err()
    );
    backend.close().await;
}
struct ImmediateEngine;
impl SearchEngine for ImmediateEngine {
    fn search<'a>(&'a self, _: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async {
            AdapterResponse {
                results: (0..2)
                    .map(|i| SearchResult {
                        url: format!("https://source{i}.test"),
                        engine: "brave".into(),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }
        })
    }
}
#[tokio::test]
async fn scheduled_due_batch_preserves_missed_slots_and_unsliced_observation() {
    let server = Server::start();
    let backend = Arc::new(server.store());
    let store = SavedStore::new(backend.clone(), TenantId::new("tenant").unwrap());
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
    let prepared=prepare_create(&p,"tenant","saved-a",&json!({"query":"q","engines":["brave"],"interval_seconds":60,"start_immediately":true,"expires_in_seconds":600,"max_results":1}),now-305.0).unwrap();
    store.create(&prepared).await.unwrap();
    let authority = Arc::new(StaticSavedAuthority {
        search: service.clone(),
        saved_granted: true,
        auth_configured: auth,
        limits,
    });
    let search = SearchService::new(
        SearchConfig::default(),
        dependencies(vec![("brave", Arc::new(ImmediateEngine))]),
    )
    .unwrap();
    let runner = SavedRunner::new(
        store.clone(),
        search.clone(),
        authority,
        Duration::from_secs(2),
    )
    .unwrap();
    let (_stop, rx) = watch::channel(false);
    let reports = runner.run_due(rx).await.unwrap();
    assert_eq!(reports.len(), 1);
    let report = &reports[0];
    assert_eq!(report["missed_slots"], 4);
    assert_eq!(report["slot"], 5);
    assert_eq!(report["observation"]["discovered_count"], 2);
    assert_eq!(
        report["observation"]["records"].as_object().unwrap().len(),
        1
    );
    assert_eq!(
        report["incomparable_reasons"],
        json!(["observation_truncated"])
    );
    let stored = store.load(&id("saved-a")).await.unwrap().unwrap();
    assert!(stored.next_due.get() > now);
    assert!(stored.baseline.is_none());
    search.shutdown().await;
    backend.close().await;
}
#[tokio::test]
async fn shutdown_classified_during_stalled_renewal_io() {
    let server = Server::start();
    let backend = Arc::new(server.store());
    let store = SavedStore::new(backend.clone(), TenantId::new("tenant").unwrap());
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let prepared = create_at(&p, "tenant", "saved-a", store.now().await.unwrap());
    store.create(&prepared).await.unwrap();
    let authority = Arc::new(MutableAuthority {
        enabled: AtomicBool::new(true),
        fingerprint: prepared.definition().policy_fingerprint.clone(),
    });
    let engine = Arc::new(HeldEngine {
        entered: Notify::new(),
        release: Notify::new(),
        drops: AtomicUsize::new(0),
        calls: AtomicUsize::new(0),
    });
    let search = SearchService::new(
        SearchConfig::default(),
        dependencies(vec![("brave", engine.clone())]),
    )
    .unwrap();
    let runner = SavedRunner::new(store, search.clone(), authority, Duration::from_secs(3))
        .unwrap()
        .with_renewal_interval(Duration::from_millis(40))
        .unwrap();
    let (stop, rx) = watch::channel(false);
    let task = tokio::spawn(async move { runner.run_one(id("saved-a"), rx).await });
    tokio::time::timeout(Duration::from_secs(2), engine.entered.notified())
        .await
        .unwrap();
    redis::cmd("CLIENT")
        .arg("PAUSE")
        .arg(2000)
        .arg("ALL")
        .query::<()>(&mut server.connection())
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let started = Instant::now();
    stop.send_replace(true);
    let result = tokio::time::timeout(Duration::from_millis(800), task)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(result, Err(RunnerError::Shutdown)), "{result:?}");
    assert!(started.elapsed() < Duration::from_millis(800));
    assert_eq!(engine.drops.load(Ordering::SeqCst), 1);
    search.shutdown().await;
    backend.close().await;
}
#[tokio::test]
async fn shutdown_classified_during_post_acquisition_time_read() {
    let server = Server::start();
    let backend = Arc::new(server.store());
    let store = SavedStore::new(backend.clone(), TenantId::new("tenant").unwrap());
    let service = policy();
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("brave".into(), false)]);
    let p = SavedPolicy {
        search: &service,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let prepared = create_at(&p, "tenant", "saved-a", store.now().await.unwrap());
    store.create(&prepared).await.unwrap();
    let authority = Arc::new(MutableAuthority {
        enabled: AtomicBool::new(true),
        fingerprint: prepared.definition().policy_fingerprint.clone(),
    });
    let engine = Arc::new(HeldEngine {
        entered: Notify::new(),
        release: Notify::new(),
        drops: AtomicUsize::new(0),
        calls: AtomicUsize::new(0),
    });
    let search = SearchService::new(
        SearchConfig::default(),
        dependencies(vec![("brave", engine.clone())]),
    )
    .unwrap();
    let runner =
        SavedRunner::new(store, search.clone(), authority, Duration::from_secs(3)).unwrap();
    let (stop, rx) = watch::channel(false);
    let task = tokio::spawn(async move { runner.run_one(id("saved-a"), rx).await });
    tokio::time::timeout(Duration::from_secs(2), engine.entered.notified())
        .await
        .unwrap();
    redis::cmd("CLIENT")
        .arg("PAUSE")
        .arg(2000)
        .arg("ALL")
        .query::<()>(&mut server.connection())
        .unwrap();
    engine.release.notify_one();
    tokio::time::timeout(Duration::from_secs(1), async {
        while engine.drops.load(Ordering::SeqCst) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    stop.send_replace(true);
    let result = tokio::time::timeout(Duration::from_millis(800), task)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(result, Err(RunnerError::Shutdown)), "{result:?}");
    search.shutdown().await;
    backend.close().await;
}
