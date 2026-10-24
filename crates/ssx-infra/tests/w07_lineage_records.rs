//! New NativeRecords controls, separate from all accepted inventories and tests.
//! Qualification must use the accepted pinned fixture and its original Child.
#[path = "w07_support/valkey.rs"]
mod private_valkey;
use ssx_application::{
    lineage::{
        Failure,
        workflows::{Read, Records},
    },
    policy::{PolicyService, SearchPolicy},
    receipts::Cancellation,
    saved::{SavedLimits, SavedPolicy, prepare_create},
    scope::{CatalogSnapshot, EngineCapability, ScopeResolver},
};
use ssx_domain::{
    SavedSearchId, TenantId, WallClockTimestamp, research::ResearchJob, saved::SavedDefinition,
    staged::Record,
};
use ssx_infra::valkey::{
    keys,
    retrieval::lineage::{Authority, AuthoritySnapshot, NativeRecords},
    saved::SavedStore,
};
use ssx_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

struct Current {
    calls: AtomicUsize,
    deny_at: usize,
    reports: u64,
    shrink_after_hint: bool,
    enabled: bool,
}
impl Current {
    fn normal() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            deny_at: usize::MAX,
            reports: 2,
            shrink_after_hint: false,
            enabled: true,
        }
    }
}
impl Authority for Current {
    fn current(&self) -> AuthoritySnapshot {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        let mut policy = SearchPolicy::default();
        if call < self.deny_at {
            policy.grants.insert("saved_searches".into());
        }
        AuthoritySnapshot {
            search: PolicyService {
                policy,
                resolver: ScopeResolver::default(),
                catalog: CatalogSnapshot::new(vec![EngineCapability {
                    name: "brave".into(),
                    enabled: self.enabled,
                    active: false,
                    available: false,
                    categories: vec!["general".into()],
                    media_types: vec![],
                    auth_ready: false,
                    circuit_open: false,
                    cost_class: "free".into(),
                    health_status: "unavailable".into(),
                    health_stale: false,
                    enforced_filters: BTreeMap::new(),
                }])
                .unwrap(),
            },
            limits: SavedLimits {
                reports: if self.shrink_after_hint && call > 0 {
                    1
                } else {
                    self.reports
                },
                ..Default::default()
            },
            auth_configured: [("brave".into(), false)].into_iter().collect(),
        }
    }
}
fn tenant() -> TenantId {
    TenantId::new("org:team").unwrap()
}
fn at(n: f64) -> WallClockTimestamp {
    WallClockTimestamp::new(n).unwrap()
}
fn repo(server: &private_valkey::Server, authority: Arc<Current>) -> NativeRecords {
    NativeRecords {
        backend: Arc::new(server.store()),
        authority,
    }
}
fn set(server: &private_valkey::Server, key: &str, value: &Value) {
    redis::cmd("SET")
        .arg(key)
        .arg(ssx_json::to_vec(value).unwrap())
        .arg("EX")
        .arg(600)
        .query::<()>(&mut server.connection())
        .unwrap();
}
fn dump(server: &private_valkey::Server, keys: &[String]) -> Vec<(Vec<u8>, i64)> {
    keys.iter()
        .map(|key| {
            let raw = redis::cmd("DUMP")
                .arg(key)
                .query(&mut server.connection())
                .unwrap();
            let ttl = redis::cmd("PTTL")
                .arg(key)
                .query(&mut server.connection())
                .unwrap();
            (raw, ttl)
        })
        .collect()
}
fn unchanged(before: Vec<(Vec<u8>, i64)>, after: Vec<(Vec<u8>, i64)>) {
    assert_eq!(before.len(), after.len());
    for ((raw, ttl), (next_raw, next_ttl)) in before.into_iter().zip(after) {
        assert_eq!(raw, next_raw, "read changed stored bytes");
        assert!(
            next_ttl > 0 && next_ttl <= ttl && next_ttl >= ttl - 5000,
            "read extended/removed TTL"
        );
    }
}
async fn saved(
    server: &private_valkey::Server,
    backend: Arc<ssx_infra::valkey::Valkey>,
) -> SavedDefinition {
    let store = SavedStore::new(backend, tenant());
    let now = store.now().await.unwrap();
    // Prepare with an acquisition-capable catalog solely to build valid stored
    // fixture bytes; the tested retained reader receives inactive/no-auth state.
    let mut current = Current::normal().current();
    let mut capability = current.search.catalog.get("brave").unwrap().clone();
    capability.active = true;
    capability.auth_ready = true;
    current.search.catalog = CatalogSnapshot::new(vec![capability]).unwrap();
    let policy = SavedPolicy {
        search: &current.search,
        saved_granted: true,
        auth_configured: &current.auth_configured,
        limits: &current.limits,
    };
    let prepared = prepare_create(&policy, tenant().as_str(), "saved-1", &json!({"query":"q","engines":["brave"],"interval_seconds":60,"start_immediately":true,"expires_in_seconds":600,"max_reports":2}), now).unwrap();
    set(
        server,
        &keys::saved("definition", &tenant(), Some("saved-1")).unwrap(),
        &prepared.definition().stored_payload().unwrap(),
    );
    store
        .load(&SavedSearchId::new("saved-1").unwrap())
        .await
        .unwrap()
        .unwrap()
}
fn reports(server: &private_valkey::Server, definition: &SavedDefinition) -> Vec<String> {
    let index = keys::saved("reports", &tenant(), Some("saved-1")).unwrap();
    let mut keys_out = vec![
        keys::saved("definition", &tenant(), Some("saved-1")).unwrap(),
        index.clone(),
    ];
    for (run, score) in [("new", 2), ("old", 1)] {
        redis::cmd("ZADD")
            .arg(&index)
            .arg(score)
            .arg(run)
            .query::<()>(&mut server.connection())
            .unwrap();
        let key = keys::saved("report", &tenant(), Some(&format!("saved-1:{run}"))).unwrap();
        set(
            server,
            &key,
            &json!({"search_id":"saved-1","run_id":run,"status":"incomparable","finished_at":definition.created_at.get(),"expires_at":definition.expires_at.get(),"incomparable_reasons":["fixture"],"private_payload":"must not project"}),
        );
        keys_out.push(key);
    }
    redis::cmd("EXPIRE")
        .arg(&index)
        .arg(600)
        .query::<()>(&mut server.connection())
        .unwrap();
    keys_out
}
#[tokio::test]
async fn typed_stores_tenant_binding_expiry_missing_and_readonly_ttls() {
    let server = private_valkey::Server::start();
    let r = repo(&server, Arc::new(Current::normal()));
    let job = ResearchJob::from_payload(json!({"job_id":"job-test","tenant":"org:team","question":"q","strategy":"broad","created_at":1,"deadline":1000,"queries":[{"index":0,"query":"q","intent":"general","engines":["fake"]}]})).unwrap();
    let job_key = keys::research("job", &tenant(), "job-test").unwrap();
    set(&server, &job_key, &job.to_payload().unwrap());
    // Reuse the original W11 fixture as source evidence, without modifying it.
    let fixture: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/w11/corpus.json")).unwrap();
    let record = Record::from_value(fixture["record"].clone()).unwrap();
    let staged_key = keys::staged("record", &tenant(), Some(record.id().as_str())).unwrap();
    set(&server, &staged_key, record.value());
    let definition = saved(&server, r.backend.clone()).await;
    let definition_key = keys::saved("definition", &tenant(), Some("saved-1")).unwrap();
    let keys = vec![job_key.clone(), staged_key.clone(), definition_key];
    let before = dump(&server, &keys);
    let c = Cancellation::default();
    assert!(matches!(
        r.research(&tenant(), "job-test", &c).await.unwrap(),
        Read::Found(_)
    ));
    assert!(matches!(
        r.staged(&tenant(), record.id().as_str(), at(101.), &c)
            .await
            .unwrap(),
        Read::Found(_)
    ));
    assert!(matches!(
        r.staged(&tenant(), record.id().as_str(), at(record.expires_at()), &c)
            .await
            .unwrap(),
        Read::Expired(_)
    ));
    assert!(matches!(
        r.saved(&tenant(), &definition.search_id, &c).await.unwrap(),
        Read::Found(_)
    ));
    let other = TenantId::new("other").unwrap();
    assert!(matches!(
        r.research(&other, "job-test", &c).await.unwrap(),
        Read::Missing
    ));
    assert!(matches!(
        r.staged(&other, record.id().as_str(), at(101.), &c)
            .await
            .unwrap(),
        Read::Missing
    ));
    assert!(matches!(
        r.saved(&other, "saved-1", &c).await.unwrap(),
        Read::Missing
    ));
    unchanged(before, dump(&server, &keys));
    let mut foreign = job.to_payload().unwrap();
    foreign["tenant"] = json!("other");
    set(&server, &job_key, &foreign);
    assert!(matches!(
        r.research(&tenant(), "job-test", &c).await,
        Err(Failure::Malformed)
    ));
    let mut foreign = record.value().clone();
    foreign["tenant"] = json!("other");
    set(&server, &staged_key, &foreign);
    assert!(matches!(
        r.staged(&tenant(), record.id().as_str(), at(101.), &c)
            .await,
        Err(Failure::Malformed)
    ));
    let mut foreign = definition.stored_payload().unwrap();
    foreign["tenant"] = json!("other");
    set(&server, &keys[2], &foreign);
    assert!(matches!(
        r.saved(&tenant(), "saved-1", &c).await,
        Err(Failure::Malformed)
    ));
    r.backend.close().await;
    assert!(matches!(
        r.research(&tenant(), "job-test", &c).await.unwrap(),
        Read::Unavailable
    ));
    assert!(matches!(
        r.staged(&tenant(), record.id().as_str(), at(101.), &c)
            .await
            .unwrap(),
        Read::Unavailable
    ));
    assert!(matches!(
        r.saved(&tenant(), "saved-1", &c).await.unwrap(),
        Read::Unavailable
    ));
}
#[tokio::test]
async fn hints_cannot_bypass_live_policy_and_projection_preserves_error_order() {
    let server = private_valkey::Server::start();
    let r = repo(&server, Arc::new(Current::normal()));
    let d = saved(&server, r.backend.clone()).await;
    let keys = reports(&server, &d);
    let before = dump(&server, &keys);
    let c = Cancellation::default();
    let Read::Found(rows) = r
        .reports(&tenant(), &d, at(0.), 2, Some("old"), &c)
        .await
        .unwrap()
    else {
        panic!("expected retained rows")
    };
    assert_eq!(rows[0], json!({"run_id":"new"}));
    assert_eq!(
        rows[1],
        json!({"run_id":"old","status":"incomparable","finished_at":d.created_at.get(),"expires_at":d.expires_at.get()})
    );
    unchanged(before, dump(&server, &keys));
    // Initial hint and first row grant; the second row must see revocation.
    let denied = repo(
        &server,
        Arc::new(Current {
            deny_at: 2,
            ..Current::normal()
        }),
    );
    assert!(matches!(
        denied
            .reports(&tenant(), &d, at(0.), 2, Some("old"), &c)
            .await
            .unwrap(),
        Read::PolicyDenied
    ));
    let disabled = repo(
        &server,
        Arc::new(Current {
            enabled: false,
            ..Current::normal()
        }),
    );
    assert!(matches!(
        disabled
            .reports(&tenant(), &d, at(0.), 2, None, &c)
            .await
            .unwrap(),
        Read::PolicyDenied
    ));
    // A malformed nonselected first row remains an error; selection is a view.
    set(
        &server,
        &keys[2],
        &json!({"search_id":"other","run_id":"new","expires_at":d.expires_at.get()}),
    );
    assert!(matches!(
        r.reports(&tenant(), &d, at(0.), 2, Some("old"), &c).await,
        Err(Failure::Malformed)
    ));
    // Each independently created backend remains owned by this test.
    r.backend.close().await;
    denied.backend.close().await;
    disabled.backend.close().await;
}
#[tokio::test]
async fn fresh_report_bound_and_authoritative_definition_override_stale_hints() {
    let server = private_valkey::Server::start();
    let r = repo(
        &server,
        Arc::new(Current {
            shrink_after_hint: true,
            ..Current::normal()
        }),
    );
    let mut d = saved(&server, r.backend.clone()).await;
    let keys = reports(&server, &d);
    // The caller's retained snapshot cannot grant scope or report membership.
    d.engines = vec!["untrusted-stale-engine".into()];
    let c = Cancellation::default();
    assert!(
        matches!(
            r.reports(&tenant(), &d, at(0.), 2, None, &c).await.unwrap(),
            Read::PolicyDenied
        ),
        "a narrower final bound must discard the original enumeration"
    );
    // A bound fixed before enumeration still uses W04's stored definition, never
    // the caller's deliberately stale engine list.
    let bounded = repo(
        &server,
        Arc::new(Current {
            reports: 1,
            ..Current::normal()
        }),
    );
    let Read::Found(rows) = bounded
        .reports(&tenant(), &d, at(0.), 2, None, &c)
        .await
        .unwrap()
    else {
        panic!("expected bounded rows")
    };
    assert_eq!(rows, vec![json!({"run_id":"new"})]);
    redis::cmd("DEL")
        .arg(&keys[2])
        .query::<()>(&mut server.connection())
        .unwrap();
    let Read::Found(rows) = bounded
        .reports(&tenant(), &d, at(0.), 2, None, &c)
        .await
        .unwrap()
    else {
        panic!("expected empty rows")
    };
    assert!(rows.is_empty(), "stale report-index hint became authority");
    r.backend.close().await;
    bounded.backend.close().await;
    assert!(matches!(
        bounded
            .reports(&tenant(), &d, at(0.), 2, None, &c)
            .await
            .unwrap(),
        Read::Unavailable
    ));
}
// Finite RESP2 relay for this fixture only. It forwards real commands/replies;
// the one exact missing-job GET reply can be held after Valkey has answered.
// A joined thread makes relay retirement synchronous even during test unwinding.
mod cancellation_reply_barrier {
    use std::{
        fs,
        io::{self, Read, Write},
        os::unix::{
            ffi::OsStrExt,
            fs::PermissionsExt,
            net::{UnixListener, UnixStream},
        },
        path::PathBuf,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
            mpsc,
        },
        thread::{self, JoinHandle},
        time::Duration,
    };

    const MAX_FRAME: usize = 65_536;
    static NEXT: AtomicUsize = AtomicUsize::new(0);

    fn invalid(message: &'static str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, message)
    }
    fn append(stream: &mut UnixStream, out: &mut Vec<u8>, count: usize) -> io::Result<()> {
        let end = out
            .len()
            .checked_add(count)
            .ok_or_else(|| invalid("frame overflow"))?;
        if end > MAX_FRAME {
            return Err(invalid("fixture frame too large"));
        }
        let mut remaining = count;
        let mut chunk = [0; 4096];
        while remaining != 0 {
            let size = remaining.min(chunk.len());
            match stream.read(&mut chunk[..size]) {
                Ok(0) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
                Ok(read) => {
                    out.extend_from_slice(&chunk[..read]);
                    remaining -= read;
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
    fn value(stream: &mut UnixStream, out: &mut Vec<u8>, depth: usize) -> io::Result<()> {
        if depth > 4 {
            return Err(invalid("fixture nesting too deep"));
        }
        let start = out.len();
        append(stream, out, 1)?;
        loop {
            append(stream, out, 1)?;
            if out.ends_with(b"\r\n") {
                break;
            }
            if out.len() - start > 1024 {
                return Err(invalid("fixture header too long"));
            }
        }
        let number = || -> io::Result<i64> {
            std::str::from_utf8(&out[start + 1..out.len() - 2])
                .map_err(|_| invalid("non-ASCII RESP count"))?
                .parse()
                .map_err(|_| invalid("invalid RESP count"))
        };
        match out[start] {
            b'+' | b'-' | b':' => (),
            b'$' => {
                let count = number()?;
                if count < -1 {
                    return Err(invalid("invalid bulk length"));
                }
                if count >= 0 {
                    let count = usize::try_from(count).map_err(|_| invalid("bulk overflow"))?;
                    append(
                        stream,
                        out,
                        count
                            .checked_add(2)
                            .ok_or_else(|| invalid("bulk overflow"))?,
                    )?;
                    if !out.ends_with(b"\r\n") {
                        return Err(invalid("bulk trailer"));
                    }
                }
            }
            b'*' => {
                let count = number()?;
                if !(-1..=16).contains(&count) {
                    return Err(invalid("fixture array bound"));
                }
                for _ in 0..count.max(0) {
                    value(stream, out, depth + 1)?;
                }
            }
            _ => return Err(invalid("fixture expects RESP2 only")),
        }
        Ok(())
    }
    fn frame(stream: &mut UnixStream) -> io::Result<Option<Vec<u8>>> {
        let mut out = Vec::new();
        match value(stream, &mut out, 0) {
            Ok(()) => Ok(Some(out)),
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof && out.is_empty() => {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
    fn register(
        stream: &UnixStream,
        sockets: &Mutex<Vec<UnixStream>>,
        stop: &AtomicBool,
    ) -> io::Result<bool> {
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        stream.set_write_timeout(Some(Duration::from_secs(30)))?;
        let mut registered = sockets.lock().unwrap();
        if stop.load(Ordering::SeqCst) {
            let _ = stream.shutdown(std::net::Shutdown::Both);
            return Ok(false);
        }
        registered.push(stream.try_clone()?);
        Ok(true)
    }

    pub struct Barrier {
        dir: PathBuf,
        socket: PathBuf,
        url: String,
        stop: Arc<AtomicBool>,
        sockets: Arc<Mutex<Vec<UnixStream>>>,
        observed: Arc<Mutex<Vec<Vec<u8>>>>,
        accepted: Arc<AtomicUsize>,
        armed: Arc<AtomicBool>,
        held: Arc<AtomicBool>,
        reached: Option<tokio::sync::oneshot::Receiver<()>>,
        release: Option<mpsc::Sender<()>>,
        task: Option<JoinHandle<io::Result<()>>>,
    }
    impl Barrier {
        pub async fn start(upstream_url: &str, exact_get: Vec<u8>) -> Self {
            Self::start_reply(upstream_url, exact_get, b"$-1\r\n".to_vec(), 1).await
        }
        pub async fn start_reply(
            upstream_url: &str,
            exact_get: Vec<u8>,
            expected_reply: Vec<u8>,
            occurrence: usize,
        ) -> Self {
            assert!((1..=6).contains(&occurrence));
            let upstream = PathBuf::from(
                upstream_url
                    .strip_prefix("unix://")
                    .expect("private UDS URL"),
            );
            let dir = std::env::temp_dir().join(format!(
                "w07p-{:x}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ));
            let socket = dir.join("p.sock");
            assert!(
                socket.as_os_str().as_bytes().len() < 104,
                "relay UDS path bound"
            );
            fs::create_dir(&dir).unwrap();
            // Own cleanup before any fallible setup after exclusive creation.
            let mut owner = Self {
                dir,
                socket,
                url: String::new(),
                stop: Arc::new(AtomicBool::new(false)),
                sockets: Arc::new(Mutex::new(Vec::new())),
                observed: Arc::new(Mutex::new(Vec::new())),
                accepted: Arc::new(AtomicUsize::new(0)),
                armed: Arc::new(AtomicBool::new(false)),
                held: Arc::new(AtomicBool::new(false)),
                reached: None,
                release: None,
                task: None,
            };
            fs::set_permissions(&owner.dir, fs::Permissions::from_mode(0o700)).unwrap();
            let listener = UnixListener::bind(&owner.socket).unwrap();
            fs::set_permissions(&owner.socket, fs::Permissions::from_mode(0o700)).unwrap();
            owner.url = format!("unix://{}", owner.socket.display());
            // The caller guards this async connect. Register the sole upstream
            // before spawning: shutdown can never race a worker-side connect.
            let mut upstream = tokio::net::UnixStream::connect(upstream)
                .await
                .unwrap()
                .into_std()
                .unwrap();
            upstream.set_nonblocking(false).unwrap();
            assert!(register(&upstream, &owner.sockets, &owner.stop).unwrap());
            let (reached_tx, reached_rx) = tokio::sync::oneshot::channel();
            let (release_tx, release_rx) = mpsc::channel();
            owner.reached = Some(reached_rx);
            owner.release = Some(release_tx);
            let stop = owner.stop.clone();
            let sockets = owner.sockets.clone();
            let observed = owner.observed.clone();
            let accepted = owner.accepted.clone();
            let armed = owner.armed.clone();
            let held = owner.held.clone();
            owner.task = Some(thread::spawn(move || {
                let run = || -> io::Result<()> {
                    let mut reached_tx = Some(reached_tx);
                    let mut matching_replies = 0;
                    loop {
                        let (mut client, _) = listener.accept()?;
                        if stop.load(Ordering::SeqCst) {
                            return Ok(());
                        }
                        if accepted.fetch_add(1, Ordering::SeqCst) != 0 {
                            return Err(invalid("unexpected backend reconnect"));
                        }
                        if !register(&client, &sockets, &stop)? {
                            return Ok(());
                        }
                        while let Some(command) = frame(&mut client)? {
                            upstream.write_all(&command)?;
                            {
                                let mut seen = observed.lock().unwrap();
                                if seen.len() >= 32 {
                                    return Err(invalid("fixture command-count bound"));
                                }
                                seen.push(command.clone());
                            }
                            let reply = frame(&mut upstream)?
                                .ok_or_else(|| invalid("missing real reply"))?;
                            if command == exact_get && armed.load(Ordering::SeqCst) {
                                matching_replies += 1;
                            }
                            if command == exact_get
                                && matching_replies == occurrence
                                && armed.swap(false, Ordering::SeqCst)
                            {
                                if reply != expected_reply {
                                    return Err(invalid(
                                        "actual held reply differs from exact fixture bytes",
                                    ));
                                }
                                held.store(true, Ordering::SeqCst);
                                reached_tx
                                    .take()
                                    .ok_or_else(|| invalid("duplicate reply barrier"))?
                                    .send(())
                                    .map_err(|_| invalid("barrier observer dropped"))?;
                                // Explicit release or RAII cleanup only: no timer can release a reply.
                                release_rx
                                    .recv()
                                    .map_err(|_| invalid("reply release dropped"))?;
                                held.store(false, Ordering::SeqCst);
                                if stop.load(Ordering::SeqCst) {
                                    return Ok(());
                                }
                            }
                            client.write_all(&reply)?;
                        }
                    }
                };
                match run() {
                    Err(error)
                        if stop.load(Ordering::SeqCst)
                            && matches!(
                                error.kind(),
                                io::ErrorKind::UnexpectedEof
                                    | io::ErrorKind::BrokenPipe
                                    | io::ErrorKind::ConnectionReset
                                    | io::ErrorKind::NotConnected
                                    | io::ErrorKind::Interrupted
                            ) =>
                    {
                        Ok(())
                    }
                    result => result,
                }
            }));
            owner
        }
        pub fn url(&self) -> &str {
            &self.url
        }
        pub fn requests(&self) -> Vec<Vec<u8>> {
            self.observed.lock().unwrap().clone()
        }
        pub fn connections(&self) -> usize {
            self.accepted.load(Ordering::SeqCst)
        }
        pub fn arm(&self) {
            assert!(!self.armed.swap(true, Ordering::SeqCst));
        }
        pub async fn reached(&mut self) {
            self.reached
                .take()
                .expect("one reply barrier")
                .await
                .expect("relay failed before real reply");
        }
        pub fn is_held(&self) -> bool {
            self.held.load(Ordering::SeqCst)
        }
        pub fn release(&mut self) {
            assert!(self.is_held(), "real reply must still be held");
            self.release
                .take()
                .expect("release once")
                .send(())
                .expect("relay release receiver");
        }
        pub fn finish(&mut self) -> io::Result<()> {
            self.stop.store(true, Ordering::SeqCst);
            if let Some(release) = self.release.take() {
                let _ = release.send(());
            }
            for socket in self.sockets.lock().unwrap().iter() {
                let _ = socket.shutdown(std::net::Shutdown::Both);
            }
            // Wake listener.accept if the backend has already closed its stream.
            let _ = UnixStream::connect(&self.socket);
            let joined = self.task.take().map(|task| task.join());
            self.sockets.lock().unwrap().clear();
            // Join before removing only our owned socket and exclusively-created directory.
            let result = match joined {
                Some(Ok(result)) => result,
                Some(Err(_)) => Err(invalid("relay thread panicked")),
                None => Ok(()),
            };
            match fs::remove_file(&self.socket) {
                Ok(()) => (),
                Err(error) if error.kind() == io::ErrorKind::NotFound => (),
                Err(error) => return Err(error),
            }
            fs::remove_dir(&self.dir)?;
            result
        }
    }
    impl Drop for Barrier {
        fn drop(&mut self) {
            if (self.task.is_some() || self.dir.exists())
                && let Err(error) = self.finish()
            {
                if std::thread::panicking() {
                    eprintln!("W07 reply relay retirement failed: {error}");
                } else {
                    panic!("W07 reply relay retirement failed: {error}");
                }
            }
        }
    }
}

#[tokio::test]
async fn cancellation_before_reads_and_after_dispatched_read_is_drained() {
    use std::{future::poll_fn, task::Poll};
    let server = private_valkey::Server::start();
    let tenant = tenant();
    assert!(ssx_domain::JobId::new("job-test").is_ok());
    assert!(ssx_domain::StagedSearchId::new("stage").is_ok());
    assert!(SavedSearchId::new("saved-1").is_ok());
    let job_key = keys::research("job", &tenant, "job-test").unwrap();
    let mut command = redis::cmd("GET");
    command.arg(&job_key);
    let exact_get = command.get_packed_command();
    let mut relay = tokio::time::timeout(
        Duration::from_secs(5),
        cancellation_reply_barrier::Barrier::start(server.url(), exact_get.clone()),
    )
    .await
    .expect("bounded relay and upstream UDS setup");
    // Fixture-only owner budget: not a production/default-latency assertion.
    // Each causal phase has a shorter five-second harness guard.
    let r = NativeRecords {
        backend: Arc::new(
            ssx_infra::valkey::Valkey::new(relay.url(), Duration::from_secs(30)).unwrap(),
        ),
        authority: Arc::new(Current::normal()),
    };
    let d = tokio::time::timeout(Duration::from_secs(5), saved(&server, r.backend.clone()))
        .await
        .expect("bounded saved fixture setup");
    tokio::time::timeout(Duration::from_secs(5), r.backend.ping())
        .await
        .unwrap()
        .unwrap();
    let before = relay.requests();
    let c = Cancellation::default();
    c.cancel();
    tokio::time::timeout(Duration::from_secs(5), async {
        assert!(matches!(
            r.research(&tenant, "job-test", &c).await,
            Err(Failure::Cancelled)
        ));
        assert!(matches!(
            r.staged(&tenant, "stage", at(1.), &c).await,
            Err(Failure::Cancelled)
        ));
        assert!(matches!(
            r.saved(&tenant, "saved-1", &c).await,
            Err(Failure::Cancelled)
        ));
        assert!(matches!(
            r.reports(&tenant, &d, at(0.), 2, None, &c).await,
            Err(Failure::Cancelled)
        ));
        // Same backend/stream FIFO fence: all preceding commands must be observed.
        r.backend.ping().await.unwrap();
    })
    .await
    .expect("bounded pre-cancelled calls and FIFO fence");
    let after = relay.requests();
    assert_eq!(&after[..before.len()], before.as_slice());
    assert_eq!(
        &after[before.len()..],
        &[redis::cmd("PING").get_packed_command()],
        "pre-cancelled ports dispatched application commands before the PING fence"
    );
    assert_eq!(relay.connections(), 1, "one primed backend connection");

    // job-test is absent: ResearchStore::load executes exactly this one GET;
    // the Found-job merge-cancel GET is not covered by this held-reply control.
    let c = Cancellation::default();
    let mut read = r.research(&tenant, "job-test", &c);
    relay.arm();
    tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! {
            biased;
            result = &mut read => panic!("read completed before the real reply barrier: {result:?}"),
            () = relay.reached() => (),
        }
        assert!(relay.is_held(), "actual Nil reply must remain withheld");
        c.cancel();
        // AtomicBool cancellation does not wake the future: explicitly repoll it.
        // A premature cancellation return must fail while the reply is still held.
        poll_fn(|cx| {
            assert!(std::future::Future::poll(read.as_mut(), cx).is_pending(), "cancelled read returned before draining held reply");
            Poll::Ready(())
        }).await;
        tokio::task::yield_now().await;
        poll_fn(|cx| {
            assert!(std::future::Future::poll(read.as_mut(), cx).is_pending(), "cancelled read returned after yield before reply release");
            Poll::Ready(())
        }).await;
        assert!(relay.is_held(), "no implicit timed release is permitted");
        relay.release();
        assert!(matches!(read.await, Err(Failure::Cancelled)));
        assert!(matches!(r.research(&tenant, "missing", &Cancellation::default()).await.unwrap(), Read::Missing));
    }).await.expect("bounded dispatch, held-reply cancellation, drain and reuse");
    let mut reuse = redis::cmd("GET");
    reuse.arg(keys::research("job", &tenant, "missing").unwrap());
    let final_requests = relay.requests();
    assert_eq!(&final_requests[..after.len()], after.as_slice());
    assert_eq!(
        &final_requests[after.len()..],
        &[exact_get, reuse.get_packed_command()],
        "expected held missing-job GET then subsequent reuse GET only"
    );
    assert_eq!(relay.connections(), 1, "drain/reuse must not reconnect");
    r.backend.close().await;
    relay
        .finish()
        .expect("joined relay and removed owned UDS scratch");
    // The unchanged fixture retains and retires its original private Valkey Child.
}

// Explicit mutation while an empty initial hint or a final W04 CAS reply is
// withheld. This does not replace the existing cancellation/Nil barrier control.
struct LiveAuthority {
    mutation: AtomicUsize,
    samples: AtomicUsize,
    expand_engine: bool,
    wide_membership: bool,
}
impl Authority for LiveAuthority {
    fn current(&self) -> AuthoritySnapshot {
        let sample = self.samples.fetch_add(1, Ordering::SeqCst);
        let mut snapshot = Current::normal().current();
        if self.wide_membership {
            snapshot.limits.reports = 4;
        }
        if self.expand_engine && sample == 0 {
            let mut engine = snapshot.search.catalog.get("brave").unwrap().clone();
            engine.enabled = false;
            snapshot.search.catalog = CatalogSnapshot::new(vec![engine]).unwrap();
        }
        match self.mutation.load(Ordering::SeqCst) {
            0 => (),
            1 => {
                snapshot.search.policy.grants.remove("saved_searches");
            }
            2 => {
                let mut engine = snapshot.search.catalog.get("brave").unwrap().clone();
                engine.enabled = false;
                snapshot.search.catalog = CatalogSnapshot::new(vec![engine]).unwrap();
            }
            3 => snapshot.limits.reports = if self.wide_membership { 3 } else { 1 },
            _ => panic!("unknown finite policy mutation"),
        }
        snapshot
    }
}
async fn held_report_case(mutation: usize, empty: bool) {
    use std::{future::poll_fn, task::Poll};
    let server = private_valkey::Server::start();
    let fixture_backend = Arc::new(server.store());
    let d = saved(&server, fixture_backend.clone()).await;
    let keys = reports(&server, &d);
    fixture_backend.close().await;
    let (exact_command, expected_reply, occurrence) = if empty {
        redis::cmd("DEL")
            .arg(&keys[1])
            .query::<()>(&mut server.connection())
            .unwrap();
        let mut command = redis::cmd("ZREVRANGE");
        command.arg(&keys[1]).arg(0).arg(1);
        (command.get_packed_command(), b"*0\r\n".to_vec(), 1)
    } else {
        // W04 reads each definition three times: initial raw, load, final CAS.
        // Two retained rows are enumerated; hold the old selected run's final
        // (sixth) raw read, after its real report and TIME replies completed.
        let raw: Vec<u8> = redis::cmd("GET")
            .arg(&keys[0])
            .query(&mut server.connection())
            .unwrap();
        let mut reply = format!("${}\r\n", raw.len()).into_bytes();
        reply.extend_from_slice(&raw);
        reply.extend_from_slice(b"\r\n");
        let mut command = redis::cmd("GET");
        command.arg(&keys[0]);
        (
            command.get_packed_command(),
            reply,
            if mutation == 5 { 3 } else { 6 },
        )
    };
    let watched = keys
        .iter()
        .enumerate()
        .filter(|(i, _)| !empty || *i != 1)
        .map(|(_, key)| key.clone())
        .collect::<Vec<_>>();
    let before = dump(&server, &watched);
    let mut relay = tokio::time::timeout(
        Duration::from_secs(5),
        cancellation_reply_barrier::Barrier::start_reply(
            server.url(),
            exact_command.clone(),
            expected_reply,
            occurrence,
        ),
    )
    .await
    .expect("bounded exact-reply relay setup");
    let authority = Arc::new(LiveAuthority {
        mutation: AtomicUsize::new(0),
        samples: AtomicUsize::new(0),
        expand_engine: mutation == 4,
        wide_membership: mutation == 5,
    });
    let r = NativeRecords {
        backend: Arc::new(
            ssx_infra::valkey::Valkey::new(relay.url(), Duration::from_secs(30)).unwrap(),
        ),
        authority: authority.clone(),
    };
    tokio::time::timeout(Duration::from_secs(5), r.backend.ping())
        .await
        .unwrap()
        .unwrap();
    let mut empty_hint_requests = relay.requests();
    empty_hint_requests.push(exact_command.clone());
    relay.arm();
    let tenant = tenant();
    let cancel = Cancellation::default();
    let mut read = r.reports(
        &tenant,
        &d,
        at(0.),
        if mutation == 5 { 1 } else { 2 },
        Some(if mutation == 5 { "new" } else { "old" }),
        &cancel,
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! {
            biased;
            result = &mut read => panic!("reports completed before final actual reply: {result:?}"),
            () = relay.reached() => (),
        }
        assert!(relay.is_held());
        assert_eq!(
            relay
                .requests()
                .iter()
                .filter(|command| **command == exact_command)
                .count(),
            occurrence
        );
        if empty {
            assert_eq!(
                authority.samples.load(Ordering::SeqCst),
                1,
                "only initial authority is sampled before the empty hint reply drains"
            );
            assert_eq!(
                relay.requests(),
                empty_hint_requests,
                "empty hint must precede every W04 or metadata read"
            );
        } else {
            assert!(
                authority.samples.load(Ordering::SeqCst) >= 2,
                "initial hint and actual W04 authority samples must precede held final reply"
            );
        }
        if mutation == 5 {
            let mut hint = redis::cmd("ZREVRANGE");
            hint.arg(&keys[1]).arg(0).arg(0);
            let mut membership = redis::cmd("ZREVRANGE");
            membership.arg(&keys[1]).arg(0).arg(3);
            let requests = relay.requests();
            assert!(
                requests.contains(&hint.get_packed_command()),
                "hint bound must be one"
            );
            assert!(
                requests.contains(&membership.get_packed_command()),
                "W04 must actually use membership bound four"
            );
        }
        authority.mutation.store(
            match mutation {
                4 => 2,
                5 => 3,
                other => other,
            },
            Ordering::SeqCst,
        );
        poll_fn(|cx| {
            assert!(
                std::future::Future::poll(read.as_mut(), cx).is_pending(),
                "reports escaped held real reply"
            );
            Poll::Ready(())
        })
        .await;
        relay.release();
        let result = read.await.unwrap();
        if mutation == 0 {
            let Read::Found(rows) = result else {
                panic!("unchanged authority lost retained rows")
            };
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[1]["run_id"], json!("old"));
            assert_eq!(rows[1]["status"], json!("incomparable"));
        } else {
            assert!(
                matches!(result, Read::PolicyDenied),
                "superseded authority disclosed report metadata: {result:?}"
            );
        }
        if empty {
            assert_eq!(
                authority.samples.load(Ordering::SeqCst),
                2,
                "empty hint must reach the final live authority fence after draining"
            );
            assert_eq!(
                relay.requests(),
                empty_hint_requests,
                "empty hint must not dispatch W04 or metadata reads"
            );
        }
        r.backend.ping().await.unwrap();
    })
    .await
    .expect("bounded held final reply, policy mutation, drain and reuse");
    unchanged(before, dump(&server, &watched));
    assert_eq!(
        relay.connections(),
        1,
        "post-read fence must not reconnect/retry"
    );
    r.backend.close().await;
    relay
        .finish()
        .expect("joined final-reply relay and retired owned scratch");
}
#[tokio::test]
async fn held_final_recent_report_reply_rechecks_saved_grant() {
    held_report_case(1, false).await;
}
#[tokio::test]
async fn held_final_recent_report_reply_rechecks_retained_engine() {
    held_report_case(2, false).await;
}
#[tokio::test]
async fn held_final_recent_report_reply_rejects_bound_shrink_for_old_selected_run() {
    held_report_case(3, false).await;
}
#[tokio::test]
async fn held_empty_hint_reply_rechecks_saved_grant_without_metadata() {
    held_report_case(1, true).await;
}
#[tokio::test]
async fn unchanged_authority_after_held_final_recent_report_preserves_projection() {
    held_report_case(0, false).await;
}

#[tokio::test]
async fn held_final_report_rejects_engine_allowed_after_hint_then_revoked() {
    held_report_case(4, false).await;
}
#[tokio::test]
async fn held_final_report_rejects_membership_shrink_still_above_hint_bound() {
    held_report_case(5, false).await;
}
