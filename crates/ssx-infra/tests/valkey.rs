//! No environment store URL is accepted. Each test owns a newly spawned, private
//! Unix-socket Valkey process and UUID namespace; no global flush is needed.
use serde_json::{Value as Json, json};
use ssx_domain::{JobId, LeaseToken, TenantId, WallClockExpiry, WallClockTimestamp};
use ssx_infra::valkey::{
    scripts::{Reply, SavedCas, SavedCommit, SavedCreate, Script, Staged},
    *,
};
use std::{
    fs,
    io::Read as _,
    num::NonZeroU64,
    path::PathBuf,
    process::{Child, Command, Stdio},
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
        let dir = std::env::temp_dir().join(format!("ssx-f07-{id}"));
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
fn ttl(n: u64) -> NonZeroU64 {
    NonZeroU64::new(n).unwrap()
}
fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|s| s.to_string()).collect()
}
struct Tokens(&'static str);
impl TokenSource<JobId> for Tokens {
    fn next_token(&mut self) -> LeaseToken<JobId> {
        LeaseToken::new(self.0).unwrap()
    }
}
fn write(server: &Server, key: &str, value: &str) {
    redis::cmd("SET")
        .arg(key)
        .arg(value)
        .query::<()>(&mut server.connection())
        .unwrap();
}
fn read(server: &Server, key: &str) -> Option<String> {
    redis::cmd("GET")
        .arg(key)
        .query(&mut server.connection())
        .unwrap()
}
fn staged_record() -> Json {
    json!({"operation_id":"op","plan_digest":"digest","state":"queued","expires_at":1000,"execution_deadline_at":900,"next_stage":0,"budget":{"reserved":0,"limit":2},"objectives":{},"retry_keys":[],"stages":[{"index":0,"state":"queued","scope":{"selected_engines":["fake"]},"enforcement":{},"attempts":[]}]})
}
#[tokio::test]
async fn claims_expiry_fencing_and_independent_clients() {
    let server = Server::start();
    let a = server.store();
    let b = server.store();
    let mut ta = Tokens("a");
    let mut tb = Tokens("b");
    let (ra, rb) = tokio::join!(
        a.acquire("lease", &mut ta, ttl(1)),
        b.acquire("lease", &mut tb, ttl(1))
    );
    assert_ne!(
        ra.as_ref().unwrap().is_some(),
        rb.as_ref().unwrap().is_some()
    );
    let old = ra.unwrap().or(rb.unwrap()).unwrap();
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let new = b
        .acquire("lease", &mut Tokens("new"), ttl(10))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        a.renew("lease", &old, ttl(10)).await.unwrap(),
        Ownership::Lost
    );
    assert_eq!(a.release("lease", &old).await.unwrap(), Ownership::Lost);
    assert_eq!(
        a.fenced_write("lease", "job", &old, "stale", ttl(10))
            .await
            .unwrap(),
        Ownership::Lost
    );
    assert_eq!(
        a.fenced_write("lease", "job", &new, "current", ttl(10))
            .await
            .unwrap(),
        Ownership::Applied
    );
    assert_eq!(read(&server, "job").as_deref(), Some("current"));
    assert_eq!(
        a.renew("lease", &new, ttl(10)).await.unwrap(),
        Ownership::Applied
    );
    assert_eq!(a.release("lease", &new).await.unwrap(), Ownership::Applied);
}
#[tokio::test]
async fn reads_wrong_types_storage_loss_reconnect_and_terminal_close() {
    let mut server = Server::start();
    let store = server.store();
    assert_eq!(store.get_object("missing").await, Read::Missing);
    for body in ["null", "[]", "3", "\"s\"", "{bad"] {
        write(&server, "bad", body);
        assert_eq!(store.get_object("bad").await, Read::Malformed);
    }
    redis::cmd("LPUSH")
        .arg("list")
        .arg("x")
        .query::<i64>(&mut server.connection())
        .unwrap();
    assert_eq!(store.get("list").await, Read::Malformed);
    store.put("record", b"{}", ttl(10)).await.unwrap();
    assert_eq!(
        store.get_object("record").await,
        Read::Found(ssx_json::from_str("{}").unwrap())
    );
    server.stop();
    assert_eq!(store.get("record").await, Read::Unavailable);
    assert_eq!(
        store.put("record", b"x", ttl(10)).await,
        Err(Failure::Unavailable)
    );
    server.restart();
    store.ping().await.unwrap();
    store.close().await;
    store.close().await;
    assert_eq!(store.get("record").await, Read::Closed);
    assert_eq!(store.ping().await, Err(Failure::Closed));
    assert_eq!(
        store.put("record", b"x", ttl(10)).await,
        Err(Failure::Closed)
    );
}
#[tokio::test]
async fn persisted_expiry_restart_clock_jumps_and_server_time() {
    let mut server = Server::start();
    let store = server.store();
    let expiry = WallClockExpiry::new(500.).unwrap();
    let at = |n| WallClockTimestamp::new(n).unwrap();
    assert_eq!(retained_ttl(expiry, at(490.9)).unwrap().get(), 309);
    assert_eq!(retained_ttl(expiry, at(800.)).unwrap().get(), 1);
    store
        .put("record", br#"{"expires_at":500}"#, ttl(300))
        .await
        .unwrap();
    redis::cmd("SAVE")
        .query::<()>(&mut server.connection())
        .unwrap();
    store.close().await;
    server.stop();
    server.restart();
    let fresh = server.store();
    let Read::Found(record) = fresh.get_object("record").await else {
        panic!("persisted record missing")
    };
    assert_eq!(record["expires_at"], 500);
    assert!(!expiry.expired_at(at(499.)));
    assert!(expiry.expired_at(at(501.)));
    assert!(!expiry.expired_at(at(100.)));
    assert!(fresh.quota_time().await.unwrap().milliseconds() > 1_000_000);
    let physical: i64 = redis::cmd("TTL")
        .arg("record")
        .query(&mut server.connection())
        .unwrap();
    assert!(physical > 290 && physical <= 300);
}
#[test]
fn namespace_bytes_and_ttl_contracts() {
    let t = TenantId::new("ü:team[*]?").unwrap();
    assert_eq!(
        keys::research("job", &t, "id").unwrap(),
        "mcp:job:ü:team[*]?:id"
    );
    assert_eq!(keys::snapshot(&t, "s"), "mcp:snapshot:ü:team[*]?:s");
    assert_eq!(
        keys::saved("lease", &t, Some("s")).unwrap(),
        "mcp:saved:v1:lease:ü:team[*]?:s"
    );
    let t = TenantId::new("default").unwrap();
    assert_eq!(
        keys::staged("record", &t, Some("op")).unwrap(),
        "mcp:staged:v1:record:37a8eec1ce19687d132fe290:op"
    );
    assert_eq!(
        keys::staged("idem", &t, Some("abc")).unwrap(),
        "mcp:staged:v1:idem:37a8eec1ce19687d132fe290:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(keys::literal_scan_prefix("a[*]?\\"), "a\\[\\*\\]\\?\\\\*");
}
#[tokio::test]
async fn research_create_ready_indexes_and_durable_cancel_are_independent() {
    let server = Server::start();
    let store = server.store();
    let a = args(&[
        "100",
        "{\"tenant\":\"t\",\"state\":\"queued\"}",
        "{\"job_id\":\"j\"}",
        "j",
    ]);
    assert_eq!(
        store
            .run(Script::ResearchIdempotentCreate, &["idem", "job"], &a)
            .await
            .unwrap(),
        Reply::ResearchCreated {
            created: true,
            job_id: "j".into()
        }
    );
    assert_eq!(
        store
            .run(Script::ResearchIdempotentCreate, &["idem", "job"], &a)
            .await
            .unwrap(),
        Reply::ResearchCreated {
            created: false,
            job_id: "j".into()
        }
    );
    write(&server, "idem", "null");
    assert_eq!(
        store
            .run(Script::ResearchIdempotentCreate, &["idem", "job"], &a)
            .await
            .unwrap(),
        Reply::ResearchIdempotencyMalformed
    );
    store
        .run(
            Script::ResearchReadyRefresh,
            &["job", "lease", "ready:tenant:t", "tenants"],
            &args(&["t", "j"]),
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .run(
                Script::ResearchReadyTake,
                &["tenants", "sequence"],
                &args(&["ready"])
            )
            .await
            .unwrap(),
        Reply::Ready(Some(("t".into(), "j".into())))
    );
    store.put("cancel", b"1", ttl(10)).await.unwrap();
    let owner = store
        .acquire("lease", &mut Tokens("owner"), ttl(10))
        .await
        .unwrap()
        .unwrap();
    store
        .fenced_write("lease", "job", &owner, "{}", ttl(100))
        .await
        .unwrap();
    assert_eq!(read(&server, "cancel").as_deref(), Some("1"));
}
#[tokio::test]
async fn saved_distinct_codes_repair_and_revision_policy_fencing() {
    let server = Server::start();
    let store = server.store();
    let def=json!({"tenant":"t","search_id":"s","expires_at":1000,"revision":1,"next_due":0,"policy_fingerprint":"p","paused":false,"deleted":false}).to_string();
    let create = vec![
        "1".into(),
        "2".into(),
        "1299".into(),
        def.clone(),
        "1000".into(),
        "s".into(),
        "0".into(),
        "t".into(),
        "definition:*".into(),
        "t".into(),
    ];
    assert_eq!(
        store
            .run(
                Script::SavedCreate,
                &["definition:s", "index", "due", "tenants", "count"],
                &create
            )
            .await
            .unwrap(),
        Reply::SavedCreate(SavedCreate::Created)
    );
    assert_eq!(
        store
            .run(
                Script::SavedCreate,
                &["definition:s", "index", "due", "tenants", "count"],
                &create
            )
            .await
            .unwrap(),
        Reply::SavedCreate(SavedCreate::Exists)
    );
    let claim = store
        .run(
            Script::SavedClaim,
            &["definition:s", "lease"],
            &args(&["1", "owner", "60", "1299"]),
        )
        .await
        .unwrap();
    assert!(matches!(claim, Reply::SavedClaim(Some(_))));
    let commit = vec![
        "owner".into(),
        "1".into(),
        "2".into(),
        "wrong-policy".into(),
        "100".into(),
        "{}".into(),
        "2".into(),
        "run".into(),
        "2".into(),
        "report:".into(),
        def.clone(),
        "1298".into(),
        "3".into(),
        "s".into(),
        "".into(),
        "100".into(),
        "".into(),
        "0-0".into(),
        "100".into(),
        "100".into(),
    ];
    let ks = [
        "definition:s",
        "lease",
        "report:run",
        "reports",
        "due",
        "baseline",
        "events",
    ];
    assert_eq!(
        store.run(Script::SavedCommit, &ks, &commit).await.unwrap(),
        Reply::SavedCommit {
            status: SavedCommit::Conflict,
            cursor: "".into()
        }
    );
    let mut commit = commit;
    commit[3] = "p".into();
    assert_eq!(
        store.run(Script::SavedCommit, &ks, &commit).await.unwrap(),
        Reply::SavedCommit {
            status: SavedCommit::Applied,
            cursor: "".into()
        }
    );
    assert_eq!(
        store.run(Script::SavedCommit, &ks, &commit).await.unwrap(),
        Reply::SavedCommit {
            status: SavedCommit::OwnershipLost,
            cursor: "".into()
        }
    );
    assert_eq!(
        store
            .run(
                Script::SavedCas,
                &["definition:s", "due", "lease", "baseline", "events"],
                &args(&[
                    "2", "2", "1298", &def, "paused", "s", "2", "", "0-0", "100", "100"
                ])
            )
            .await
            .unwrap(),
        Reply::SavedCas(SavedCas::Revision(1))
    );
    // Lost derived index must be reconstructed from authoritative definitions.
    redis::cmd("DEL")
        .arg("index")
        .arg("count")
        .query::<i64>(&mut server.connection())
        .unwrap();
    let mut other = create.clone();
    other[3] = def.replace("\"s\"", "\"s2\"");
    other[5] = "s2".into();
    other[1] = "1".into();
    assert_eq!(
        store
            .run(
                Script::SavedCreate,
                &["definition:s2", "index", "due", "tenants", "count"],
                &other
            )
            .await
            .unwrap(),
        Reply::SavedCreate(SavedCreate::Quota)
    );
    assert!(read(&server, "definition:s2").is_none());
}
#[tokio::test]
async fn staged_reservations_unknown_attempts_and_retry_conserve_budget() {
    let server = Server::start();
    let store = server.store();
    let record = staged_record().to_string();
    let admit = vec![
        "digest".into(),
        "1".into(),
        "32".into(),
        "1299".into(),
        record,
        "1000".into(),
        "record:*".into(),
        "op".into(),
    ];
    let ks = ["record:op", "idem", "index", "ready", "marker"];
    assert!(matches!(
        store.run(Script::StagedAdmit, &ks, &admit).await.unwrap(),
        Reply::Staged {
            status: Staged::Created,
            ..
        }
    ));
    assert!(matches!(
        store.run(Script::StagedAdmit, &ks, &admit).await.unwrap(),
        Reply::Staged {
            status: Staged::Replay,
            ..
        }
    ));
    let claim = store
        .run(
            Script::StagedClaim,
            &["record:op", "lease", "ready", "running"],
            &args(&["2", "owner", "1298", "60", "attempt", "2"]),
        )
        .await
        .unwrap();
    let Reply::Staged {
        status: Staged::Claimed,
        record: Some(claimed),
    } = claim
    else {
        panic!("claim failed")
    };
    assert_eq!(claimed["budget"]["reserved"], 1);
    redis::cmd("DEL")
        .arg("lease")
        .query::<i64>(&mut server.connection())
        .unwrap();
    assert_eq!(
        store
            .run(
                Script::StagedInterrupt,
                &["record:op", "lease", "running"],
                &args(&["3", "1297"])
            )
            .await
            .unwrap(),
        Reply::Flag(true)
    );
    let interrupted: Json = serde_json::from_str(&read(&server, "record:op").unwrap()).unwrap();
    assert_eq!(interrupted["budget"]["reserved"], 1);
    assert_eq!(
        interrupted["stages"][0]["attempts"][0]["state"],
        "interrupted"
    );
    assert!(matches!(
        store
            .run(
                Script::StagedRetry,
                &["record:op", "ready"],
                &args(&["4", "retry", "1296"])
            )
            .await
            .unwrap(),
        Reply::Staged {
            status: Staged::Queued,
            ..
        }
    ));
    let retry = store
        .run(
            Script::StagedClaim,
            &["record:op", "lease", "ready", "running"],
            &args(&["5", "new", "1295", "60", "attempt2", "5"]),
        )
        .await
        .unwrap();
    let Reply::Staged {
        record: Some(retry),
        ..
    } = retry
    else {
        panic!()
    };
    assert_eq!(retry["budget"]["reserved"], 2);
    assert_eq!(
        store
            .run(
                Script::StagedOwnedSave,
                &["record:op", "lease", "ready", "running"],
                &args(&["owner", "100", "{}", "queued", "5"])
            )
            .await
            .unwrap(),
        Reply::Flag(false)
    );
}
#[test]
fn exact_resp_shape_decoder_rejects_malformed_values() {
    use redis::Value::*;
    for v in [
        Nil,
        Int(2),
        Array(vec![]),
        BulkString(b"1".to_vec()),
        Array(vec![Int(1), Int(2)]),
    ] {
        assert!(Script::ResearchLeaseSave.decode(v).is_err());
    }
    for raw in ["null", "[]", "2", "\"x\""] {
        assert!(
            Script::SavedClaim
                .decode(BulkString(raw.as_bytes().to_vec()))
                .is_err()
        );
    }
    assert!(
        Script::SavedCommit
            .decode(Array(vec![Int(1), BulkString(b"bad".to_vec())]))
            .is_err()
    );
    assert!(
        Script::StagedClaim
            .decode(Array(vec![
                BulkString(b"claimed".to_vec()),
                BulkString(b"".to_vec())
            ]))
            .is_err()
    );
}
#[tokio::test]
async fn lua_errors_are_partial_and_ambiguous_writes_are_not_retried() {
    let server = Server::start();
    let store = Valkey::new(&server.url, Duration::from_millis(40)).unwrap();
    store.ping().await.unwrap();
    // The source script writes a record before encountering WRONGTYPE on ZADD.
    let body = staged_record().to_string();
    write(&server, "ready", "wrong-type");
    let a = vec![
        "d".into(),
        "1".into(),
        "32".into(),
        "1299".into(),
        body,
        "1000".into(),
        "record:*".into(),
        "op".into(),
    ];
    assert_eq!(
        store
            .run(
                Script::StagedAdmit,
                &["record:op", "idem", "index", "ready", "marker"],
                &a
            )
            .await,
        Err(Failure::ScriptErrorMayHaveWritten)
    );
    assert!(read(&server, "record:op").is_some());
    store.ping().await.unwrap();
    redis::cmd("CLIENT")
        .arg("PAUSE")
        .arg(150)
        .arg("ALL")
        .query::<()>(&mut server.connection())
        .unwrap();
    assert_eq!(
        store.put("late", b"committed", ttl(10)).await,
        Err(Failure::CompletionUnknown)
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(read(&server, "late").as_deref(), Some("committed"));
}
#[tokio::test]
async fn cancellation_close_race_never_reopens_client() {
    let server = Server::start();
    let store = std::sync::Arc::new(server.store());
    store.ping().await.unwrap();
    redis::cmd("CLIENT")
        .arg("PAUSE")
        .arg(150)
        .arg("ALL")
        .query::<()>(&mut server.connection())
        .unwrap();
    let writer = store.clone();
    let task = tokio::spawn(async move { writer.put("cancelled", b"maybe", ttl(10)).await });
    tokio::time::sleep(Duration::from_millis(20)).await;
    task.abort();
    let _ = task.await;
    store.close().await;
    assert_eq!(store.ping().await, Err(Failure::Closed));
    tokio::time::sleep(Duration::from_millis(200)).await;
    let clients: String = redis::cmd("CLIENT")
        .arg("LIST")
        .query(&mut server.connection())
        .unwrap();
    assert_eq!(
        clients.lines().count(),
        1,
        "closed client driver retained a socket"
    );
    assert_eq!(
        store.put("after-close", b"no", ttl(1)).await,
        Err(Failure::Closed)
    );
    assert!(read(&server, "after-close").is_none());
}
#[tokio::test]
async fn lossless_durable_numbers_and_malformed_cjson_records() {
    let server = Server::start();
    let store = server.store();
    let raw = r#"{"large":184467440737095516160000001,"negative":-184467440737095516160000001}"#;
    store.put("numbers", raw.as_bytes(), ttl(60)).await.unwrap();
    let Read::Found(v) = store.get_object("numbers").await else {
        panic!()
    };
    assert_eq!(ssx_json::to_string(&v).unwrap(), raw);
    for bad in [
        "null",
        "[]",
        "1",
        "{\"expires_at\":null}",
        "{\"expires_at\":10,\"state\":\"queued\"}",
    ] {
        write(&server, "broken", bad);
        assert!(
            matches!(
                store
                    .run(
                        Script::StagedClaim,
                        &["broken", "lease", "ready", "running"],
                        &args(&["1", "owner", "10", "2", "a", "1"])
                    )
                    .await,
                Err(Failure::ScriptErrorMayHaveWritten)
                    | Err(Failure::Malformed)
                    | Ok(Reply::Staged {
                        status: Staged::NotQueued,
                        record: None
                    })
            ),
            "unexpected source outcome for {bad}"
        );
    }
}
#[tokio::test]
async fn index_ttl_event_cursor_and_all_remaining_script_contracts() {
    use ssx_infra::valkey::scripts::{SavedAck, SavedExpiry};
    let server = Server::start();
    let store = server.store();
    for script in [Script::ResearchRecentRecord, Script::SavedRecentRecord] {
        assert_eq!(
            store
                .run(script, &["recent"], &args(&["1", "a", "1", "60"]))
                .await
                .unwrap(),
            Reply::Flag(true)
        );
        store
            .run(script, &["recent"], &args(&["2", "b", "1", "60"]))
            .await
            .unwrap();
        let members: Vec<String> = redis::cmd("ZRANGE")
            .arg("recent")
            .arg(0)
            .arg(-1)
            .query(&mut server.connection())
            .unwrap();
        assert_eq!(members, vec!["b"]);
    }
    write(&server, "lease", "owner");
    assert_eq!(
        store
            .run(Script::SavedRenew, &["lease"], &args(&["owner", "10"]))
            .await
            .unwrap(),
        Reply::Flag(true)
    );
    assert_eq!(
        store
            .run(Script::SavedRelease, &["lease"], &args(&["old"]))
            .await
            .unwrap(),
        Reply::Flag(false)
    );
    assert_eq!(
        store
            .run(Script::SavedRelease, &["lease"], &args(&["owner"]))
            .await
            .unwrap(),
        Reply::Flag(true)
    );
    write(&server, "lease", "owner");
    assert_eq!(
        store
            .run(
                Script::StagedRenew,
                &["lease", "running"],
                &args(&["owner", "10", "11", "record:s"])
            )
            .await
            .unwrap(),
        Reply::Flag(true)
    );
    let def = json!({"expires_at":0,"revision":1,"search_id":"s"}).to_string();
    write(&server, "def", &def);
    assert_eq!(
        store
            .run(
                Script::SavedPublishExpiry,
                &["def", "expired", "events"],
                &args(&["1", "0-0", "2", "{}", "100"])
            )
            .await
            .unwrap(),
        Reply::SavedExpiry(SavedExpiry::Published)
    );
    assert_eq!(
        store
            .run(
                Script::SavedPublishExpiry,
                &["def", "expired", "events"],
                &args(&["1", "0-0", "2", "{}", "100"])
            )
            .await
            .unwrap(),
        Reply::SavedExpiry(SavedExpiry::Unchanged)
    );
    let Reply::SavedEvents {
        first_cursor,
        entries,
    } = store
        .run(
            Script::SavedReadEvents,
            &["events"],
            &args(&["0-0", "0-0", "10"]),
        )
        .await
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].0, first_cursor);
    assert_eq!(entries[0].1, vec![("payload".into(), "{}".into())]);
    let ack = vec![
        "1".into(),
        first_cursor,
        "100".into(),
        "101".into(),
        "consumer".into(),
        "2".into(),
        "0-0".into(),
    ];
    assert_eq!(
        store
            .run(Script::SavedAckEvent, &["ack", "consumers", "events"], &ack)
            .await
            .unwrap(),
        Reply::SavedAck(SavedAck::Advanced)
    );
    assert_eq!(
        store
            .run(Script::SavedAckEvent, &["ack", "consumers", "events"], &ack)
            .await
            .unwrap(),
        Reply::SavedAck(SavedAck::Unchanged)
    );
    assert_eq!(
        store
            .run(
                Script::SavedDelete,
                &[
                    "def", "index", "due", "deleted", "lease", "reports", "baseline", "extra"
                ],
                &args(&["1", "s", "100", "report:"])
            )
            .await
            .unwrap(),
        Reply::SavedCas(SavedCas::Applied)
    );
    // Saved CAS success and outbox capacity are distinct from revision conflicts.
    write(&server, "def", r#"{"expires_at":1000,"revision":1}"#);
    let mut cas = args(&[
        "1",
        "1",
        "100",
        r#"{"expires_at":1000,"revision":2}"#,
        "paused",
        "s",
        "1",
        "{}",
        "0-0",
        "1",
        "100",
    ]);
    assert_eq!(
        store
            .run(
                Script::SavedCas,
                &["def", "due", "lease", "baseline", "events"],
                &cas
            )
            .await
            .unwrap(),
        Reply::SavedCas(SavedCas::Capacity)
    );
    cas[7] = "".into();
    assert_eq!(
        store
            .run(
                Script::SavedCas,
                &["def", "due", "lease", "baseline", "events"],
                &cas
            )
            .await
            .unwrap(),
        Reply::SavedCas(SavedCas::Applied)
    );
}
