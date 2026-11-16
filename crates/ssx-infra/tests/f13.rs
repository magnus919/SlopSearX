//! F13 private real-Valkey qualification; no providers or shared state.
use ssx_application::{cache::*, snapshot::*};
use ssx_domain::*;
use ssx_infra::valkey::{Create, Valkey, cache::ValkeyCache, snapshot::ValkeySnapshots};
use ssx_json::Value;
use std::{
    fs,
    io::Read as _,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};
use std::{
    num::NonZeroU64,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
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
        let dir = std::env::temp_dir().join(format!("ssx-f13-{id}"));
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
        Valkey::new(&self.url, Duration::from_millis(300)).unwrap()
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

fn ttl(value: u64) -> NonZeroU64 {
    NonZeroU64::new(value).unwrap()
}
fn tenant(value: &str) -> TenantId {
    TenantId::new(value).unwrap()
}
fn id(value: &str) -> SnapshotId {
    SnapshotId::new(value).unwrap()
}
struct Clock(AtomicU64);
impl Clock {
    fn new(value: f64) -> Self {
        Self(AtomicU64::new(value.to_bits()))
    }
    fn set(&self, value: f64) {
        self.0.store(value.to_bits(), Ordering::SeqCst);
    }
}
impl SnapshotClock for Clock {
    fn now(&self) -> WallClockTimestamp {
        WallClockTimestamp::new(f64::from_bits(self.0.load(Ordering::SeqCst))).unwrap()
    }
}
struct Ids {
    calls: AtomicUsize,
}
impl Ids {
    fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
        }
    }
}
impl SnapshotIds for Ids {
    fn next(&self) -> Result<SnapshotId, SnapshotFailure> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(id(&format!("snap-{n:012x}")))
    }
}
fn canonical() -> CanonicalResponse {
    CanonicalResponse::new(SearchResponse {
        query: "q".into(),
        query_id: "ssx-q".into(),
        results: (0..5)
            .map(|i| SearchResult {
                url: format!("https://example.com/{i}"),
                title: format!("Title{i}"),
                engine: "a".into(),
                engines: ["b".into(), "a".into()].into_iter().collect(),
                payload: Some(ssx_json::json!({"nested":{"list":["original"]}})),
                ..Default::default()
            })
            .collect(),
        answers: vec![ssx_json::json!({"nested":{"x":"original"}})],
        suggestions: vec!["suggestion".into()],
        ..Default::default()
    })
}
fn service(store: Arc<Valkey>, clock: Arc<Clock>, ids: Arc<Ids>) -> SnapshotService {
    SnapshotService::new(
        Arc::new(ValkeySnapshots::new(store)),
        clock,
        ids,
        tenant("tenant-a"),
        SnapshotTtl::new(1).unwrap(),
    )
}
fn put_raw(server: &Server, key: &str, raw: &[u8]) {
    redis::cmd("SET")
        .arg(key)
        .arg(raw)
        .arg("EX")
        .arg(600)
        .query::<()>(&mut server.connection())
        .unwrap();
}
fn raw(server: &Server, key: &str) -> Option<Vec<u8>> {
    redis::cmd("GET")
        .arg(key)
        .query(&mut server.connection())
        .unwrap()
}

#[tokio::test]
async fn nx_two_independent_clients_preserve_first_value_and_retention() {
    let server = Server::start();
    let a = server.store();
    let b = server.store();
    let (one, two) = tokio::join!(
        a.put_if_absent("immutable", b"first", ttl(60)),
        b.put_if_absent("immutable", b"second", ttl(120))
    );
    let one = one.unwrap();
    let two = two.unwrap();
    assert!(
        (one == Create::Stored && two == Create::Exists)
            || (one == Create::Exists && two == Create::Stored)
    );
    let expected = if one == Create::Stored {
        b"first".as_slice()
    } else {
        b"second".as_slice()
    };
    assert_eq!(raw(&server, "immutable").as_deref(), Some(expected));
    let before: i64 = redis::cmd("PTTL")
        .arg("immutable")
        .query(&mut server.connection())
        .unwrap();
    assert!(before > if one == Create::Stored { 59000 } else { 119000 });
    assert_eq!(
        b.put_if_absent("immutable", b"overwrite", ttl(1))
            .await
            .unwrap(),
        Create::Exists
    );
    let after: i64 = redis::cmd("PTTL")
        .arg("immutable")
        .query(&mut server.connection())
        .unwrap();
    assert!(after <= before && after > before - 5000);
    assert_eq!(raw(&server, "immutable").as_deref(), Some(expected));
}
#[tokio::test]
async fn immutable_cache_views_admission_answer_and_negative_namespaces_are_separate() {
    let server = Server::start();
    let store = Arc::new(server.store());
    let repo = Arc::new(ValkeyCache::new(store.clone()));
    let cache = CanonicalCache::new(repo.clone(), CacheTtls::default());
    let input = CacheKeyInput {
        query: " q ".into(),
        ..Default::default()
    };
    let key = source_base_key(&input);
    let canonical = canonical();
    for decision in [
        CacheAdmission::AllUnresponsive,
        CacheAdmission::DeadlineTruncated,
        CacheAdmission::RerankFallback,
        CacheAdmission::Rejected,
    ] {
        assert_eq!(
            cache.write(&key, &canonical, &[], decision).await,
            CacheWriteOutcome::Skipped
        );
        assert!(raw(&server, key.as_str()).is_none());
    }
    assert_eq!(
        cache
            .write(&key, &canonical, &[], CacheAdmission::Complete)
            .await,
        CacheWriteOutcome::Stored
    );
    let CacheEntry::Canonical(hit) = cache.read(&key, false).await.unwrap() else {
        panic!()
    };
    let first = hit.view(["results".into()].into_iter().collect(), Some(1), true);
    let second = hit.view(
        ["results".into(), "suggestions".into()]
            .into_iter()
            .collect(),
        Some(5),
        true,
    );
    assert!(first.canonical().shares_evidence(second.canonical()));
    assert_eq!(first.results().len(), 1);
    assert_eq!(second.results().len(), 5);
    assert!(second.cached());
    assert_eq!(second.suggestions(), ["suggestion"]);
    let mut owned = first.to_response();
    owned.answers.clear();
    owned.results.clear();
    assert_eq!(second.canonical().response().answers.len(), 1);
    assert_eq!(canonical.response().results.len(), 5);
    assert!(cache.read(&key, true).await.is_none());
    let answers = AnswerCache::new(repo.clone(), ttl(60));
    answers
        .set(" q ", &ssx_json::json!({"answer":"separate"}))
        .await
        .unwrap();
    assert_eq!(
        answers.get("q").await.unwrap()["answer"],
        ssx_json::json!("separate")
    );
    assert!(matches!(
        repo.read(&answer_key("q")).await,
        Err(CacheFailure::InvalidInput)
    ));
    assert_eq!(
        cache.read(&key, false).await.unwrap(),
        CacheEntry::Canonical(canonical.clone())
    );
    cache.write_negative(&key, 100.9).await.unwrap();
    assert_eq!(
        cache.read(&key, false).await.unwrap(),
        CacheEntry::Negative {
            timestamp: Some(ssx_domain::WallClockTimestamp::new(100.0).unwrap())
        }
    );
    let corpus: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/f13/source.json")).unwrap();
    let writes = corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["kind"] == ssx_json::json!("cache-write"))
        .collect::<Vec<_>>();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[1]["expected"]["writes"], ssx_json::json!([]));
    let source_writes = writes[0]["expected"]["writes"].as_array().unwrap();
    assert_eq!(source_writes.len(), 2);
    assert_eq!(source_writes[0][1], ssx_json::json!(60));
    let source_negative: Value = ssx_json::from_str(source_writes[0][2].as_str().unwrap()).unwrap();
    assert_eq!(
        source_negative,
        ssx_json::json!({"_error":true,"timestamp":100})
    );
    assert_eq!(source_writes[1][0], ssx_json::json!(answer_key("q")));
    assert_eq!(source_writes[1][1], ssx_json::json!(3600));
    let stored: Value = ssx_json::from_slice(&raw(&server, key.as_str()).unwrap()).unwrap();
    assert_eq!(stored, ssx_json::json!({"_error":true,"timestamp":100}));
    assert_eq!(
        answers.get("q").await.unwrap()["answer"],
        ssx_json::json!("separate")
    );
    drop(cache);
    drop(answers);
    drop(repo);
    store.ping().await.unwrap();
}
#[tokio::test]
async fn source_snapshot_legacy_records_integrity_and_error_states_do_not_mutate() {
    let server = Server::start();
    let store = Arc::new(server.store());
    let clock = Arc::new(Clock::new(100.0));
    let svc = service(store.clone(), clock.clone(), Arc::new(Ids::new()));
    let corpus: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/f13/source.json")).unwrap();
    let mut count = 0;
    for case in corpus["cases"].as_array().unwrap().iter().filter(|c| {
        matches!(
            c["kind"].as_str(),
            Some("snapshot-read" | "snapshot-integrity")
        )
    }) {
        let input = &case["input"];
        clock.set(input["now"].as_f64().unwrap());
        let sid = id(input["id"].as_str().unwrap());
        let key = ssx_infra::valkey::keys::snapshot(&tenant("tenant-a"), sid.as_str());
        let bytes = ssx_json::to_vec(&input["payload"]).unwrap();
        put_raw(&server, &key, &bytes);
        let result = svc.read(&sid).await;
        let hardened = case["kind"] == ssx_json::json!("snapshot-integrity");
        match case["expected"]["state"].as_str().unwrap() {
            "found" if !hardened => {
                let SnapshotLookup::Found(s) = result else {
                    panic!("{}", case["id"])
                };
                assert_eq!(s.id().as_str(), case["expected"]["id"].as_str().unwrap());
                assert_eq!(
                    s.total() as u64,
                    case["expected"]["total"].as_u64().unwrap()
                );
                assert_eq!(
                    s.lineage_available(),
                    case["expected"]["lineage_available"].as_bool().unwrap()
                );
            }
            "expired" if !hardened => assert!(matches!(result, SnapshotLookup::Expired { .. })),
            _ => assert_eq!(result, SnapshotLookup::Missing, "{}", case["id"]),
        };
        assert_eq!(
            raw(&server, &key),
            Some(bytes),
            "read must not rewrite {} ",
            case["id"]
        );
        count += 1;
    }
    assert_eq!(count, 18);
    assert_eq!(svc.read(&id("snap-missing")).await, SnapshotLookup::Missing);
}
#[tokio::test]
async fn logical_equality_real_expiry_retention_and_tenant_isolation() {
    let server = Server::start();
    let store = Arc::new(server.store());
    let clock = Arc::new(Clock::new(100.0));
    let svc = service(store.clone(), clock.clone(), Arc::new(Ids::new()));
    let sid = svc.create(&canonical(), &[]).await.unwrap();
    let key = ssx_infra::valkey::keys::snapshot(&tenant("tenant-a"), sid.as_str());
    let retention: i64 = redis::cmd("PTTL")
        .arg(&key)
        .query(&mut server.connection())
        .unwrap();
    assert!(retention > 300000 && retention <= 301000);
    clock.set(101.0);
    assert!(matches!(svc.read(&sid).await, SnapshotLookup::Found(_)));
    clock.set(101.0001);
    assert!(matches!(
        svc.read(&sid).await,
        SnapshotLookup::Expired { .. }
    ));
    assert!(raw(&server, &key).is_some());
    clock.set(100.0);
    assert!(
        matches!(svc.read(&sid).await, SnapshotLookup::Found(_)),
        "source wall clock reversal remains wall-epoch based"
    );
    clock.set(101.0001);
    assert_eq!(
        svc.for_tenant(tenant("tenant-b")).read(&sid).await,
        SnapshotLookup::Missing
    );
    let system = SnapshotService::new(
        Arc::new(ValkeySnapshots::new(store.clone())),
        Arc::new(ssx_infra::valkey::snapshot::SystemSnapshotClock),
        Arc::new(ssx_infra::valkey::snapshot::RandomSnapshotIds),
        tenant("real-clock"),
        SnapshotTtl::new(1).unwrap(),
    );
    let real = system.create(&canonical(), &[]).await.unwrap();
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(matches!(
        system.read(&real).await,
        SnapshotLookup::Expired { .. }
    ));
    redis::cmd("PEXPIRE")
        .arg(&key)
        .arg(5)
        .query::<()>(&mut server.connection())
        .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(svc.read(&sid).await, SnapshotLookup::Missing);
}
#[tokio::test]
async fn cache_and_snapshot_reconnect_distinguish_missing_malformed_outage_and_closed() {
    let mut server = Server::start();
    let store = Arc::new(server.store());
    let svc = service(
        store.clone(),
        Arc::new(Clock::new(100.0)),
        Arc::new(Ids::new()),
    );
    let repo = ValkeyCache::new(store.clone());
    let key = source_base_key(&CacheKeyInput::default());
    let sid = svc.create(&canonical(), &[]).await.unwrap();
    let skey = ssx_infra::valkey::keys::snapshot(&tenant("tenant-a"), sid.as_str());
    for bytes in [b"null".as_slice(), b"[]", b"{", b"{\"snapshot_id\":null}"] {
        put_raw(&server, &skey, bytes);
        assert_eq!(svc.read(&sid).await, SnapshotLookup::Missing);
        assert_eq!(raw(&server, &skey).as_deref(), Some(bytes));
    }
    redis::cmd("DEL")
        .arg(&skey)
        .query::<()>(&mut server.connection())
        .unwrap();
    redis::cmd("LPUSH")
        .arg(&skey)
        .arg("wrongtype")
        .query::<()>(&mut server.connection())
        .unwrap();
    assert_eq!(svc.read(&sid).await, SnapshotLookup::Missing);
    put_raw(&server, key.as_str(), b"[]");
    assert_eq!(repo.read(key.as_str()).await, Err(CacheFailure::Malformed));
    let corpus: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/f13/source.json")).unwrap();
    let quirks = corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == ssx_json::json!("source-quirk"))
        .collect::<Vec<_>>();
    assert_eq!(quirks.len(), 2);
    assert_eq!(
        quirks[0]["expected"],
        ssx_json::json!({"unavailable":false,"snapshot":null})
    );
    assert_eq!(
        quirks[1]["expected"]["id"],
        ssx_json::json!("snap-0123456789ab")
    );
    server.stop();
    assert_eq!(svc.read(&sid).await, SnapshotLookup::Unavailable);
    assert_eq!(
        svc.create(&canonical(), &[]).await,
        Err(SnapshotFailure::Unavailable)
    );
    let optional = CanonicalCache::new(
        Arc::new(ValkeyCache::new(store.clone())),
        CacheTtls::default(),
    );
    assert!(optional.read(&key, false).await.is_none());
    assert_eq!(
        optional
            .write(&key, &canonical(), &[], CacheAdmission::Complete)
            .await,
        CacheWriteOutcome::NotStored(CacheFailure::Unavailable)
    );
    assert_eq!(
        repo.read(key.as_str()).await,
        Err(CacheFailure::Unavailable)
    );
    server.restart();
    assert_eq!(svc.read(&sid).await, SnapshotLookup::Missing);
    assert!(repo.read(key.as_str()).await.unwrap().is_none());
    let fresh = svc.create(&canonical(), &[]).await.unwrap();
    assert!(matches!(svc.read(&fresh).await, SnapshotLookup::Found(_)));
    store.close().await;
    assert_eq!(svc.read(&fresh).await, SnapshotLookup::Unavailable);
    assert_eq!(repo.read(key.as_str()).await, Err(CacheFailure::Closed));
}

async fn reply_proxy(
    server_path: PathBuf,
    malformed: bool,
) -> (String, tokio::task::JoinHandle<usize>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("redis://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut client, _) = listener.accept().await.unwrap();
        let mut backend = tokio::net::UnixStream::connect(server_path).await.unwrap();
        let mut sent = Vec::new();
        let mut mutating = false;
        let mut from_client = [0; 8192];
        let mut from_backend = [0; 8192];
        loop {
            tokio::select! {
                         read=client.read(&mut from_client)=>{let n=read.unwrap();if n==0{break;}sent.extend_from_slice(&from_client[..n]);mutating|=sent.windows(9).any(|v|v==b"$3\r\nSET\r\n");backend.write_all(&from_client[..n]).await.unwrap();},
                         read=backend.read(&mut from_backend)=>{let n=read.unwrap();if n==0{break;}
            if mutating {assert_eq!(&from_backend[..n],b"+OK\r\n");if malformed{client.write_all(b":9\r\n").await.unwrap();}break;}client.write_all(&from_backend[..n]).await.unwrap();}
                        }
        }
        sent.windows(9).filter(|v| *v == b"$3\r\nSET\r\n").count()
    });
    (url, task)
}
#[tokio::test]
async fn lost_or_malformed_create_reply_never_publishes_or_issues_another_id() {
    for malformed in [false, true] {
        let server = Server::start();
        let (url, proxy) = reply_proxy(server.dir.join("s.sock"), malformed).await;
        let store = Arc::new(Valkey::new(&url, Duration::from_secs(2)).unwrap());
        let ids = Arc::new(Ids::new());
        let svc = service(store, Arc::new(Clock::new(100.0)), ids.clone());
        let outcome = svc.create(&canonical(), &[]).await;
        assert_eq!(
            outcome,
            Err(if malformed {
                SnapshotFailure::Malformed
            } else {
                SnapshotFailure::CompletionUnknown
            })
        );
        assert_eq!(ids.calls.load(Ordering::SeqCst), 1);
        assert_eq!(proxy.await.unwrap(), 1);
        let key = ssx_infra::valkey::keys::snapshot(&tenant("tenant-a"), "snap-000000000000");
        assert!(
            raw(&server, &key).is_some(),
            "write executed even though caller never received success"
        );
        let retained: i64 = redis::cmd("PTTL")
            .arg(&key)
            .query(&mut server.connection())
            .unwrap();
        assert!(retained > 299000);
    }
}
#[tokio::test]
async fn known_id_collision_retries_only_before_success_without_replacing_evidence() {
    let server = Server::start();
    let store = Arc::new(server.store());
    let clock = Arc::new(Clock::new(100.0));
    let first = service(store.clone(), clock.clone(), Arc::new(Ids::new()));
    let one = first.create(&canonical(), &[]).await.unwrap();
    let key = ssx_infra::valkey::keys::snapshot(&tenant("tenant-a"), one.as_str());
    let original = raw(&server, &key).unwrap();
    let ids = Arc::new(Ids::new());
    let successor = service(store.clone(), clock, ids.clone());
    let two = successor.create(&canonical(), &[]).await.unwrap();
    assert_ne!(one, two);
    assert_eq!(ids.calls.load(Ordering::SeqCst), 2);
    assert_eq!(raw(&server, &key), Some(original));
    let SnapshotLookup::Found(snapshot) = successor.read(&two).await else {
        panic!()
    };
    let page = SnapshotPage::new(snapshot.clone(), 1, 1).unwrap();
    assert_eq!(page.results().len(), 1);
    assert_eq!(page.snapshot().total(), 5);
    assert_eq!(page.result_id(0).unwrap().to_string(), format!("{two}:0"));
    let later = SnapshotPage::new(snapshot, 2, 3).unwrap();
    assert_eq!(later.result_id(0).unwrap().to_string(), format!("{two}:3"));
    assert_eq!(later.results().len(), 2);
    // Optional migration evidence export: native tests/gates never invoke Python.
    if let Ok(path) = std::env::var("F13_EXPORT_NATIVE_RECORD") {
        fs::write(
            path,
            raw(
                &server,
                &ssx_infra::valkey::keys::snapshot(&tenant("tenant-a"), two.as_str()),
            )
            .unwrap(),
        )
        .unwrap();
    }
}
#[tokio::test]
async fn canonical_cache_source_reads_and_source_snapshot_writer_are_replayed() {
    let server = Server::start();
    let store = Arc::new(server.store());
    let repo = Arc::new(ValkeyCache::new(store.clone()));
    let cache = CanonicalCache::new(repo.clone(), CacheTtls::default());
    let key = source_base_key(&CacheKeyInput {
        query: "q".into(),
        ..Default::default()
    });
    let corpus: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/f13/source.json")).unwrap();
    let mut reads = 0;
    for case in corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["kind"] == ssx_json::json!("cache-read"))
    {
        if let Some(raw) = case["input"]["raw"].as_str() {
            put_raw(&server, key.as_str(), raw.as_bytes());
        } else {
            redis::cmd("DEL")
                .arg(key.as_str())
                .query::<()>(&mut server.connection())
                .unwrap();
        }
        let actual = cache.read(&key, false).await;
        let expected = &case["expected"];
        if expected.is_null() {
            assert!(actual.is_none());
        } else if expected["_error"].as_bool() == Some(true) {
            assert_eq!(
                actual,
                Some(CacheEntry::Negative {
                    timestamp: Some(ssx_domain::WallClockTimestamp::new(100.0).unwrap())
                })
            );
        } else {
            assert_eq!(
                actual,
                Some(CacheEntry::Canonical(CanonicalResponse::new(
                    storage::decode_response(expected.clone()).unwrap()
                )))
            );
        }
        reads += 1;
    }
    assert_eq!(reads, 10);
    let source = corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["kind"] == ssx_json::json!("snapshot-create"))
        .unwrap();
    let snapshot = storage::snapshot::decode(source["expected"]["payload"].clone()).unwrap();
    assert_eq!(
        storage::snapshot::encode(&snapshot).unwrap(),
        source["expected"]["payload"]
    );
    let snapshots = ValkeySnapshots::new(store);
    assert_eq!(
        snapshots
            .create(
                &snapshot,
                ttl(source["expected"]["physical_ttl"].as_u64().unwrap())
            )
            .await
            .unwrap(),
        SnapshotWrite::Stored
    );
    let bytes = raw(&server, source["expected"]["key"].as_str().unwrap()).unwrap();
    assert_eq!(
        ssx_json::from_slice::<Value>(&bytes).unwrap(),
        source["expected"]["payload"]
    );
    assert_eq!(
        ResultId::new(snapshot.id().clone(), 2).to_string(),
        source["expected"]["result_id"].as_str().unwrap()
    );
}
#[test]
fn python_reverse_reader_observation_matches_exact_native_created_record() {
    let raw = include_bytes!("../../xtask/fixtures/f13/native-record.json");
    let record: Value = ssx_json::from_slice(raw).unwrap();
    let source: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/f13/reverse.json")).unwrap();
    let snapshot = storage::snapshot::decode(record.clone()).unwrap();
    let mut projected = storage::snapshot::encode(&snapshot).unwrap();
    projected["lineage_available"] = ssx_json::json!(snapshot.lineage_available());
    assert_eq!(projected, source["reverse_read"]["projection"]);
    let expected = ssx_json::to_vec(&storage::snapshot::encode(&snapshot).unwrap()).unwrap();
    assert_eq!(expected.as_slice(), raw);
}
#[tokio::test]
async fn malformed_negative_metadata_is_unknown_and_nonobject_answers_do_not_overwrite() {
    let server = Server::start();
    let store = Arc::new(server.store());
    let repo = Arc::new(ValkeyCache::new(store.clone()));
    let cache = CanonicalCache::new(repo.clone(), CacheTtls::default());
    let answers = AnswerCache::new(repo.clone(), ttl(60));
    let key = source_base_key(&CacheKeyInput {
        query: "q".into(),
        ..Default::default()
    });
    let corpus: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/f13/source.json")).unwrap();
    let mut checked = 0;
    for case in corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == ssx_json::json!("negative-metadata"))
    {
        let value = ssx_json::json!({"_error":true,"timestamp":case["input"]["timestamp"]});
        let bytes = ssx_json::to_vec(&value).unwrap();
        put_raw(&server, key.as_str(), &bytes);
        let entry = cache.read(&key, false).await.unwrap();
        let expected_timestamp = case["input"]["timestamp"]
            .as_f64()
            .map(|v| WallClockTimestamp::new(v).unwrap());
        assert_eq!(
            entry,
            CacheEntry::Negative {
                timestamp: expected_timestamp
            }
        );
        let view = entry.view(CachedViewContext {
            query: "q".into(),
            query_id: QueryId::new("ssx-current").unwrap(),
            ranking_explanation: "rank".into(),
            include: Default::default(),
            max_results: None,
        });
        assert_eq!(view.cached(), case["expected"]["cached"].as_bool().unwrap());
        assert_eq!(
            view.to_response().cached_error,
            case["expected"]["cached_error"].as_bool().unwrap()
        );
        assert_eq!(raw(&server, key.as_str()), Some(bytes));
        checked += 1;
    }
    answers
        .set("q", &ssx_json::json!({"answer":"preserved"}))
        .await
        .unwrap();
    let before = raw(&server, &answer_key("q")).unwrap();
    for case in corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == ssx_json::json!("answer-nonobject"))
    {
        assert_eq!(case["expected"]["written"], case["input"]["value"]);
        assert!(case["expected"]["read"].is_null());
        assert_eq!(
            answers.set("q", &case["input"]["value"]).await,
            Err(CacheFailure::Malformed)
        );
        assert_eq!(raw(&server, &answer_key("q")), Some(before.clone()));
        checked += 1;
    }
    assert_eq!(checked, 10);
}
#[tokio::test]
async fn fresh_unsliced_evidence_uses_byte_bounds_without_arbitrary_result_cap() {
    let server = Server::start();
    let store = Arc::new(Valkey::new(&server.url, Duration::from_secs(5)).unwrap());
    let cache = CanonicalCache::new(
        Arc::new(ValkeyCache::new(store.clone())),
        CacheTtls::default(),
    );
    let ids = Arc::new(Ids::new());
    let snapshots = service(store.clone(), Arc::new(Clock::new(100.0)), ids.clone());
    let corpus: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/f13/source.json")).unwrap();
    let mut checked = 0;
    for case in corpus["cases"].as_array().unwrap().iter().filter(|v| {
        v["kind"] == ssx_json::json!("storage-bounds") && v["input"]["count"].is_number()
    }) {
        let count = case["input"]["count"].as_u64().unwrap() as usize;
        let canonical = CanonicalResponse::new(SearchResponse {
            query_id: "ssx-large".into(),
            results: vec![SearchResult::default(); count],
            ..Default::default()
        });
        let key = source_base_key(&CacheKeyInput {
            query: format!("count-{count}"),
            ..Default::default()
        });
        assert_eq!(
            cache
                .write(&key, &canonical, &[], CacheAdmission::Complete)
                .await,
            CacheWriteOutcome::Stored
        );
        let sid = snapshots.create(&canonical, &[]).await.unwrap();
        let SnapshotLookup::Found(stored) = snapshots.read(&sid).await else {
            panic!()
        };
        assert_eq!(
            stored.total() as u64,
            case["expected"]["read_total"].as_u64().unwrap()
        );
        assert_eq!(
            canonical
                .view(Default::default(), None, false)
                .to_response()
                .results
                .len(),
            count
        );
        checked += 1;
    }
    assert_eq!(checked, 2);
    let key = source_base_key(&CacheKeyInput {
        query: "oversized-bytes".into(),
        ..Default::default()
    });
    let result = SearchResult {
        content: "x".repeat(ssx_json::MAX_JSON_BYTES + 1),
        ..Default::default()
    };
    let large = CanonicalResponse::new(SearchResponse {
        query_id: "ssx-large-bytes".into(),
        results: vec![result],
        ..Default::default()
    });
    let before = ids.calls.load(Ordering::SeqCst);
    assert_eq!(
        cache
            .write(&key, &large, &[], CacheAdmission::Complete)
            .await,
        CacheWriteOutcome::NotStored(CacheFailure::Malformed)
    );
    assert_eq!(
        snapshots.create(&large, &[]).await,
        Err(SnapshotFailure::Malformed)
    );
    assert_eq!(ids.calls.load(Ordering::SeqCst), before + 1);
    assert!(raw(&server, key.as_str()).is_none());
    let absent =
        ssx_infra::valkey::keys::snapshot(&tenant("tenant-a"), &format!("snap-{before:012x}"));
    assert!(raw(&server, &absent).is_none());
    assert_eq!(
        large
            .view(Default::default(), None, false)
            .to_response()
            .results[0]
            .content
            .len(),
        ssx_json::MAX_JSON_BYTES + 1
    );
    assert_eq!(
        large.response().results[0].content.len(),
        ssx_json::MAX_JSON_BYTES + 1
    );
    let source = corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == ssx_json::json!("source-bytes-over-4mib"))
        .unwrap();
    assert_eq!(source["expected"]["written"], ssx_json::json!(true));
    assert_eq!(
        cache
            .write(&key, &canonical(), &[], CacheAdmission::Complete)
            .await,
        CacheWriteOutcome::Stored
    );
    let previous_cache = raw(&server, key.as_str()).unwrap();
    let aggregate = CanonicalResponse::new(SearchResponse {
        query_id: "ssx-aggregate".into(),
        results: vec![SearchResult::default(); 20_000],
        ..Default::default()
    });
    assert_eq!(
        cache
            .write(&key, &aggregate, &[], CacheAdmission::Complete)
            .await,
        CacheWriteOutcome::NotStored(CacheFailure::Malformed)
    );
    assert_eq!(raw(&server, key.as_str()), Some(previous_cache));
    let before = ids.calls.load(Ordering::SeqCst);
    assert_eq!(
        snapshots.create(&aggregate, &[]).await,
        Err(SnapshotFailure::Malformed)
    );
    assert_eq!(ids.calls.load(Ordering::SeqCst), before + 1);
    let absent =
        ssx_infra::valkey::keys::snapshot(&tenant("tenant-a"), &format!("snap-{before:012x}"));
    assert!(raw(&server, &absent).is_none());
    assert_eq!(
        aggregate
            .view(Default::default(), None, false)
            .to_response()
            .results
            .len(),
        20_000
    );
}
#[tokio::test]
async fn injected_f05_payload_bounds_apply_only_at_persistence_boundary() {
    let server = Server::start();
    let store = Arc::new(server.store());
    let limits = storage::StorageLimits {
        payload_max_bytes: 0,
        work_group_max_bytes: 0,
    };
    let repo = Arc::new(ValkeyCache::with_limits(store.clone(), limits));
    let cache = CanonicalCache::new(repo, CacheTtls::default());
    let response = canonical();
    let key = source_base_key(&CacheKeyInput {
        query: "bounds".into(),
        ..Default::default()
    });
    assert_eq!(
        cache
            .write(&key, &response, &[], CacheAdmission::Complete)
            .await,
        CacheWriteOutcome::Stored
    );
    let CacheEntry::Canonical(stored) = cache.read(&key, false).await.unwrap() else {
        panic!()
    };
    assert!(
        stored
            .response()
            .results
            .iter()
            .all(|r| r.payload.is_none())
    );
    assert!(
        response
            .response()
            .results
            .iter()
            .all(|r| r.payload.is_some())
    );
    let svc = SnapshotService::new(
        Arc::new(ValkeySnapshots::with_limits(store, limits)),
        Arc::new(Clock::new(100.0)),
        Arc::new(Ids::new()),
        tenant("tenant-a"),
        SnapshotTtl::new(10).unwrap(),
    );
    let sid = svc.create(&response, &[]).await.unwrap();
    let SnapshotLookup::Found(snapshot) = svc.read(&sid).await else {
        panic!()
    };
    assert!(snapshot.results().iter().all(|r| r.payload.is_none()));
    assert!(
        response
            .response()
            .results
            .iter()
            .all(|r| r.payload.is_some())
    );
}
#[tokio::test]
async fn cache_admission_uses_news_and_partial_retention_without_changing_evidence() {
    let server = Server::start();
    let store = Arc::new(server.store());
    let cache = CanonicalCache::new(
        Arc::new(ValkeyCache::new(store)),
        CacheTtls::new(3600, 7, 5).unwrap(),
    );
    let response = canonical();
    for (query, categories, admission, seconds) in [
        ("general", vec![], CacheAdmission::Complete, 3600),
        (
            "news",
            vec!["notnews".into()],
            CacheAdmission::Complete,
            300,
        ),
        ("partial", vec!["news".into()], CacheAdmission::Partial, 7),
    ] {
        let key = source_base_key(&CacheKeyInput {
            query: query.into(),
            ..Default::default()
        });
        assert_eq!(
            cache.write(&key, &response, &categories, admission).await,
            CacheWriteOutcome::Stored
        );
        let actual: i64 = redis::cmd("PTTL")
            .arg(key.as_str())
            .query(&mut server.connection())
            .unwrap();
        assert!(actual <= seconds * 1000 && actual > seconds * 1000 - 2000);
        let CacheEntry::Canonical(hit) = cache.read(&key, false).await.unwrap() else {
            panic!()
        };
        assert_eq!(hit, response);
    }
}
#[tokio::test]
async fn result_lookup_keeps_snapshot_identity_and_never_recovers_by_requery() {
    let mut server = Server::start();
    let store = Arc::new(server.store());
    let clock = Arc::new(Clock::new(100.0));
    let ids = Arc::new(Ids::new());
    let svc = service(store.clone(), clock.clone(), ids.clone());
    let sid = svc.create(&canonical(), &[]).await.unwrap();
    let result = ResultId::new(sid.clone(), 3);
    let SnapshotResultLookup::Found(found) = svc.read_result(&result).await else {
        panic!()
    };
    assert_eq!(found.result(), &canonical().response().results[3]);
    assert_eq!(
        svc.read_result(&ResultId::new(sid.clone(), 5)).await,
        SnapshotResultLookup::InvalidIndex
    );
    assert_eq!(
        svc.read_result(&ResultId::new(id("absent"), 0)).await,
        SnapshotResultLookup::Missing
    );
    assert_eq!(
        svc.for_tenant(tenant("other")).read_result(&result).await,
        SnapshotResultLookup::Missing
    );
    // A new repository/service instance uses the persisted wall epoch, not process uptime.
    let restarted = service(store, Arc::new(Clock::new(100.5)), Arc::new(Ids::new()));
    assert!(matches!(
        restarted.read_result(&result).await,
        SnapshotResultLookup::Found(_)
    ));
    clock.set(101.1);
    assert!(matches!(
        svc.read_result(&result).await,
        SnapshotResultLookup::Expired { .. }
    ));
    server.stop();
    assert_eq!(
        svc.read_result(&result).await,
        SnapshotResultLookup::Unavailable
    );
    assert_eq!(ids.calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn durable_byte_and_depth_refusal_is_nonmutating() {
    let server = Server::start();
    let store = Arc::new(Valkey::new(&server.url, Duration::from_secs(5)).unwrap());
    let cache = ValkeyCache::new(store.clone());
    let snapshots = ValkeySnapshots::new(store);
    let key = source_base_key(&CacheKeyInput::default());
    let sid = id("snap-bounded");
    let tenant = tenant("tenant-a");
    let skey = ssx_infra::valkey::keys::snapshot(&tenant, sid.as_str());
    // Deliberately bypass the bounded writer to inject adversarial durable bytes.
    let oversized = format!(
        "{{\"padding\":\"{}\"}}",
        "x".repeat(ssx_json::MAX_JSON_BYTES)
    )
    .into_bytes();
    let nested = format!("{}0{}", "[".repeat(101), "]".repeat(101)).into_bytes();
    for bytes in [oversized, nested] {
        put_raw(&server, key.as_str(), &bytes);
        put_raw(&server, &skey, &bytes);
        assert_eq!(cache.read(key.as_str()).await, Err(CacheFailure::Malformed));
        assert_eq!(
            snapshots.read(&tenant, &sid).await,
            Err(SnapshotFailure::Malformed)
        );
        assert_eq!(raw(&server, key.as_str()), Some(bytes.clone()));
        assert_eq!(raw(&server, &skey), Some(bytes));
    }
}
#[tokio::test]
async fn snapshot_local_legacy_iterables_preserve_valid_scope_without_rewriting() {
    let server = Server::start();
    let store = Arc::new(server.store());
    let repo = ValkeySnapshots::new(store);
    let corpus: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/f13/source.json")).unwrap();
    let mut checked = 0;
    for case in corpus["cases"].as_array().unwrap().iter().filter(|v| {
        matches!(
            v["kind"].as_str(),
            Some("snapshot-scope" | "snapshot-lineage")
        )
    }) {
        let input = &case["input"];
        let sid = id(input["id"].as_str().unwrap());
        let tenant = tenant("tenant-a");
        let key = ssx_infra::valkey::keys::snapshot(&tenant, sid.as_str());
        let bytes = ssx_json::to_vec(&input["payload"]).unwrap();
        put_raw(&server, &key, &bytes);
        let result = repo.read(&tenant, &sid).await;
        if case["expected"]["state"] == ssx_json::json!("found") {
            let snapshot = result.unwrap().unwrap();
            if case["kind"] == ssx_json::json!("snapshot-scope") {
                assert_eq!(
                    storage::snapshot::encode(&snapshot).unwrap()["scope"],
                    case["expected"]["scope"],
                    "{}",
                    case["id"]
                );
            } else {
                assert_eq!(
                    snapshot.lineage(),
                    case["expected"]["lineage"].as_array().unwrap()
                );
                assert_eq!(
                    snapshot.lineage_available(),
                    case["expected"]["lineage_available"].as_bool().unwrap()
                );
            }
        } else {
            assert_eq!(result, Err(SnapshotFailure::Malformed));
        }
        assert_eq!(raw(&server, &key), Some(bytes));
        checked += 1;
    }
    assert_eq!(checked, 13);
}
