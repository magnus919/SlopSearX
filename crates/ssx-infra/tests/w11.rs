//! Mandatory W11 real disposable Valkey contracts; no application URLs/providers.
use ssx_application::{policy::*, scope::*};
use ssx_domain::{
    LeaseToken, SearchRequest, StagedSearchId, TenantId, WallClockTimestamp,
    canonical::plan_digest,
    staged::{Record, identity},
};
use ssx_infra::valkey::{scripts::Staged, staged::*, *};
use ssx_json::{Value, json};
use std::{
    fs,
    io::Read as _,
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
        let dir = std::env::temp_dir().join(format!("ssx-w11-{id}"));
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
fn now(n: f64) -> WallClockTimestamp {
    WallClockTimestamp::new(n).unwrap()
}
fn tenant() -> TenantId {
    TenantId::new("org:team").unwrap()
}
fn id() -> StagedSearchId {
    StagedSearchId::new("legacy:id:雪").unwrap()
}
fn token(s: &str) -> LeaseToken<StagedSearchId> {
    LeaseToken::new(s).unwrap()
}
fn policy() -> PolicyService {
    let f: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/f09/oracle.json")).unwrap();
    let e = &f["engines"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == json!("alpha"))
        .unwrap();
    let engine = EngineCapability {
        name: "alpha".into(),
        enabled: true,
        active: true,
        available: true,
        categories: vec!["general".into()],
        media_types: vec![],
        auth_ready: true,
        circuit_open: false,
        cost_class: "free".into(),
        health_status: "ok".into(),
        health_stale: false,
        enforced_filters: e["enforced_filters"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap().into()))
            .collect(),
    };
    PolicyService {
        policy: SearchPolicy::default(),
        catalog: CatalogSnapshot::new(vec![
            engine.clone(),
            EngineCapability {
                name: "beta".into(),
                ..engine
            },
        ])
        .unwrap(),
        resolver: ScopeResolver::default(),
    }
}
fn scopes() -> ValidatedScope {
    scopes_for("alpha")
}
fn scopes_for(name: &str) -> ValidatedScope {
    let fixture: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/w11/corpus.json")).unwrap();
    policy()
        .authorize(ScopeProposal::generic(SearchRequest {
            query: fixture["record"]["plan"]["query"].as_str().unwrap().into(),
            engines: Some(vec![name.into()]),
            ..Default::default()
        }))
        .unwrap()
}
fn record() -> Record {
    let f: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/w11/corpus.json")).unwrap();
    let mut v = f["record"].clone();
    let s = scopes();
    v["stages"].as_array_mut().unwrap()[0]["enforcement"] = Value::Object(
        s.preview()
            .enforcement
            .iter()
            .map(|(k, v)| (k.clone(), v.to_value()))
            .collect(),
    );
    Record::from_value(v).unwrap()
}
fn finish(owned: &OwnedStage, state: &str, observed: Option<u64>) -> Record {
    let mut v = owned.record().value().clone();
    let index = v["next_stage"].as_u64().unwrap() as usize;
    v["state"] = json!(state);
    v["owner_token"] = Value::Null;
    v["stop_reason"] = json!("execution_failed");
    let stage = &mut v["stages"].as_array_mut().unwrap()[index];
    stage["state"] = json!(if state == "failed" { "failed" } else { "done" });
    let stage_state = stage["state"].clone();
    let attempt = stage["attempts"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap();
    attempt["state"] = stage_state;
    attempt["finished_at"] = json!(102.0);
    attempt["observed_engine_calls"] = json!(observed);
    v["budget"]["observed"] = if let (Some(old), Some(n)) = (
        owned.record().value()["budget"]["observed"].as_u64(),
        observed,
    ) {
        json!(old + n)
    } else {
        Value::Null
    };
    if state == "queued" {
        v["next_stage"] = json!(index + 1);
    }
    Record::from_value(v).unwrap()
}
fn raw_cmd<T: redis::FromRedisValue>(s: &Server, name: &str, args: &[&str]) -> T {
    redis::cmd(name)
        .arg(args)
        .query(&mut s.connection())
        .unwrap()
}
#[tokio::test]
async fn admission_conflict_quota_and_lost_tenant_index() {
    let s = Server::start();
    let client = s.store();
    let store = StagedStore::new(&client);
    let scope = scopes();
    let r = record();
    assert_eq!(
        store
            .admit(&policy(), &r, "idem", &[&scope], now(100.))
            .await
            .unwrap()
            .status,
        Staged::Created
    );
    assert_eq!(
        store
            .admit(&policy(), &r, "idem", &[&scope], now(101.))
            .await
            .unwrap()
            .status,
        Staged::Replay
    );
    let mut v = r.value().clone();
    v["plan"]["query"] = json!("changed");
    v["plan_digest"] = json!(plan_digest(&identity(&v)).unwrap());
    let changed = Record::from_value(v).unwrap();
    let changed_scope = policy()
        .authorize(ScopeProposal::generic(SearchRequest {
            query: "changed".into(),
            engines: Some(vec!["alpha".into()]),
            ..Default::default()
        }))
        .unwrap();
    assert_eq!(
        store
            .admit(&policy(), &changed, "idem", &[&changed_scope], now(101.))
            .await
            .unwrap()
            .status,
        Staged::Conflict
    );
    assert!(matches!(
        store
            .find_by_idempotency(&tenant(), "idem", now(100.))
            .await
            .unwrap(),
        RecordRead::Found(_)
    ));
    assert!(matches!(
        store
            .read(&TenantId::new("other").unwrap(), &id(), now(100.))
            .await
            .unwrap(),
        RecordRead::Missing
    ));
    let index = keys::staged("index", &tenant(), None).unwrap();
    let _: i64 = raw_cmd(&s, "DEL", &[&index]);
    for n in 1..32 {
        let mut v = r.value().clone();
        v["operation_id"] = json!(format!("op-{n}"));
        let r = Record::from_value(v).unwrap();
        assert_eq!(
            store
                .admit(&policy(), &r, &format!("idem-{n}"), &[&scope], now(101.))
                .await
                .unwrap()
                .status,
            Staged::Created
        );
    }
    let mut v = r.value().clone();
    v["operation_id"] = json!("op-over");
    let r = Record::from_value(v).unwrap();
    assert_eq!(
        store
            .admit(&policy(), &r, "over", &[&scope], now(101.))
            .await
            .unwrap()
            .status,
        Staged::Quota
    );
    assert_eq!(
        store
            .list_recent(&tenant(), None, 50, now(101.))
            .await
            .unwrap()
            .len(),
        32
    );
    assert!(matches!(
        store.read(&tenant(), &id(), now(3700.)).await.unwrap(),
        RecordRead::Expired { .. }
    ));
}
#[tokio::test]
async fn competing_claims_stale_tokens_and_immutable_completion() {
    let s = Server::start();
    let a = s.store();
    let b = s.store();
    let store = StagedStore::new(&a);
    let other = StagedStore::new(&b);
    let scope = scopes();
    store
        .admit(&policy(), &record(), "idem", &[&scope], now(100.))
        .await
        .unwrap();
    let tenant = tenant();
    let id = id();
    let (a, b) = tokio::join!(
        store.claim(&tenant, &id, token("a"), "attempt-a", now(101.)),
        other.claim(&tenant, &id, token("b"), "attempt-b", now(101.))
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_eq!(usize::from(a.is_some()) + usize::from(b.is_some()), 1);
    let owner = a.or(b).unwrap();
    let RecordRead::Found(persisted) = store.read(&tenant, &id, now(101.)).await.unwrap() else {
        panic!("claimed record must remain readable");
    };
    assert_eq!(persisted.value()["budget"]["reserved"], json!(1));
    assert_eq!(
        persisted.value()["stages"][0]["attempts"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(owner.record().value()["budget"]["reserved"], json!(1));
    assert!(store.renew(&owner, now(102.)).await.unwrap());
    let mut forged = finish(&owner, "failed", Some(0)).value().clone();
    forged["expires_at"] = json!(3800.);
    assert_eq!(
        store
            .save_owned(&owner, &Record::from_value(forged).unwrap(), now(103.))
            .await,
        Err(Failure::InvalidInput)
    );
    let lease = keys::staged("lease", &tenant, Some(id.as_str())).unwrap();
    let _: () = raw_cmd(&s, "SET", &[&lease, "replacement", "EX", "60"]);
    assert!(!store.renew(&owner, now(103.)).await.unwrap());
    assert!(
        !store
            .save_owned(&owner, &finish(&owner, "failed", Some(0)), now(103.))
            .await
            .unwrap()
    );
}
#[tokio::test]
async fn interruptions_retry_replay_budget_conservation_and_index_repair() {
    let s = Server::start();
    let client = s.store();
    let store = StagedStore::new(&client);
    let scope = scopes();
    store
        .admit(&policy(), &record(), "idem", &[&scope], now(100.))
        .await
        .unwrap();
    let _: i64 = raw_cmd(&s, "DEL", &[keys::STAGED_READY]);
    assert!(store.claim_next(now(100.)).await.unwrap().is_none());
    let mut cursor = 0;
    loop {
        cursor = store.reconcile_indexes(cursor, now(100.)).await.unwrap();
        if cursor == 0 {
            break;
        }
    }
    assert_eq!(
        store.claim_next(now(100.)).await.unwrap(),
        Some((tenant(), id()))
    );
    for n in 0..3 {
        let owner = store
            .claim(
                &tenant(),
                &id(),
                token(&format!("owner-{n}")),
                &format!("attempt-{n}"),
                now(101. + n as f64),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(owner.record().value()["budget"]["reserved"], json!(n + 1));
        let lease = keys::staged("lease", &tenant(), Some(id().as_str())).unwrap();
        let _: i64 = raw_cmd(&s, "DEL", &[&lease]);
        // Lost running index is reconstructed and orphan classified; no new attempt.
        let _: i64 = raw_cmd(&s, "DEL", &[keys::STAGED_RUNNING]);
        let mut cursor = 0;
        loop {
            cursor = store.reconcile_indexes(cursor, now(104.)).await.unwrap();
            if cursor == 0 {
                break;
            }
        }
        assert_eq!(store.recover_orphans(now(200.)).await.unwrap(), 0);
        let RecordRead::Found(r) = store.read(&tenant(), &id(), now(104.)).await.unwrap() else {
            panic!()
        };
        assert_eq!(r.state(), "interrupted");
        assert_eq!(r.value()["budget"]["observed"], Value::Null);
        assert_eq!(r.value()["budget"]["reserved"], json!(n + 1));
        assert!(
            !store
                .save_owned(&owner, &finish_first(&owner), now(104.))
                .await
                .unwrap()
        );
        let result = store
            .request_retry(&tenant(), &id(), &format!("retry-{n}"), now(105.))
            .await
            .unwrap();
        assert_eq!(
            result.status,
            if n == 2 {
                Staged::Budget
            } else {
                Staged::Queued
            }
        );
        if n < 2 {
            assert_eq!(
                store
                    .request_retry(&tenant(), &id(), &format!("retry-{n}"), now(106.))
                    .await
                    .unwrap()
                    .status,
                Staged::Replay
            );
        }
    }
}
fn finish_first(owner: &OwnedStage) -> Record {
    let mut v = owner.record().value().clone();
    v["state"] = json!("failed");
    v["stages"].as_array_mut().unwrap()[0]["state"] = json!("failed");
    let attempts = v["stages"].as_array_mut().unwrap()[0]["attempts"]
        .as_array_mut()
        .unwrap();
    let a = attempts.last_mut().unwrap();
    a["state"] = json!("failed");
    a["finished_at"] = json!(104.);
    v["budget"]["observed"] = Value::Null;
    Record::from_value(v).unwrap()
}
#[tokio::test]
async fn fenced_save_storage_loss_and_partial_script_writes_require_reconciliation() {
    let mut s = Server::start();
    let client = s.store();
    let store = StagedStore::new(&client);
    let scope = scopes();
    store
        .admit(&policy(), &record(), "idem", &[&scope], now(100.))
        .await
        .unwrap();
    let owner = store
        .claim(&tenant(), &id(), token("owner"), "attempt", now(101.))
        .await
        .unwrap()
        .unwrap();
    assert!(
        store
            .save_owned(&owner, &finish(&owner, "failed", Some(0)), now(102.))
            .await
            .unwrap()
    );
    assert_eq!(
        store
            .request_retry(&tenant(), &id(), "retry", now(103.))
            .await
            .unwrap()
            .status,
        Staged::Queued
    );
    let _: i64 = raw_cmd(&s, "DEL", &[keys::STAGED_RUNNING]);
    let _: () = raw_cmd(&s, "SET", &[keys::STAGED_RUNNING, "wrongtype"]);
    assert_eq!(
        store
            .claim(&tenant(), &id(), token("next"), "attempt2", now(104.))
            .await
            .err(),
        Some(Failure::ScriptErrorMayHaveWritten)
    );
    let RecordRead::Found(r) = store.read(&tenant(), &id(), now(105.)).await.unwrap() else {
        panic!()
    };
    assert_eq!(r.state(), "running");
    assert_eq!(r.value()["budget"]["reserved"], json!(2));
    // The command error did not roll back record/lease. Never blindly claim again.
    let _: i64 = raw_cmd(&s, "DEL", &[keys::STAGED_RUNNING]);
    let _: i64 = raw_cmd(
        &s,
        "DEL",
        &[&keys::staged("lease", &tenant(), Some(id().as_str())).unwrap()],
    );
    let mut cursor = 0;
    loop {
        cursor = store.reconcile_indexes(cursor, now(105.)).await.unwrap();
        if cursor == 0 {
            break;
        }
    }
    let RecordRead::Found(r) = store.read(&tenant(), &id(), now(105.)).await.unwrap() else {
        panic!()
    };
    assert_eq!(r.state(), "interrupted");
    assert_eq!(r.value()["budget"]["reserved"], json!(2));
    s.stop();
    assert!(matches!(
        store.read(&tenant(), &id(), now(105.)).await,
        Err(Failure::Unavailable)
    ));
    s.restart();
    assert_eq!(
        store.read(&tenant(), &id(), now(105.)).await.unwrap(),
        RecordRead::Missing
    );
}
fn two_stage_record() -> Record {
    let mut v = record().value().clone();
    let mut fallback = v["plan"]["initial_scope"].clone();
    fallback["selected_engines"] = json!(["beta"]);
    fallback["requested"] = json!({"engines":["beta"]});
    v["plan"]["fallback_scope"] = fallback.clone();
    v["plan"]["allow_scope_expansion"] = json!(true);
    let enforcement = Value::Object(
        scopes_for("beta")
            .preview()
            .enforcement
            .iter()
            .map(|(k, v)| (k.clone(), v.to_value()))
            .collect(),
    );
    v["stages"].as_array_mut().unwrap().push(json!({"index":1,"scope":fallback,"state":"pending","skipped_reason":null,"attempts":[],"enforcement":enforcement}));
    v["plan_digest"] = json!(plan_digest(&identity(&v)).unwrap());
    Record::from_value(v).unwrap()
}
#[tokio::test]
async fn two_stage_owned_completion_and_skipped_fallback_keep_immutable_history() {
    let s = Server::start();
    let client = s.store();
    let store = StagedStore::new(&client);
    let a = scopes();
    let b = scopes_for("beta");
    store
        .admit(&policy(), &two_stage_record(), "two", &[&a, &b], now(100.))
        .await
        .unwrap();
    let owner = store
        .claim(&tenant(), &id(), token("a"), "attempt-a", now(101.))
        .await
        .unwrap()
        .unwrap();
    let mut skipped = finish(&owner, "completed", Some(1)).value().clone();
    let stage = &mut skipped["stages"].as_array_mut().unwrap()[1];
    stage["state"] = json!("skipped");
    stage["skipped_reason"] = json!("initial_nonempty");
    let completed = Record::from_value(skipped).unwrap();
    assert!(
        store
            .save_owned(&owner, &completed, now(102.))
            .await
            .unwrap()
    );
    // A second operation follows initial-clean-empty -> queued fallback -> completed.
    let mut v = two_stage_record().value().clone();
    v["operation_id"] = json!("two-clean");
    let r = Record::from_value(v).unwrap();
    store
        .admit(&policy(), &r, "two-clean", &[&a, &b], now(100.))
        .await
        .unwrap();
    let owner = store
        .claim(&tenant(), &r.id(), token("clean-a"), "clean-a", now(101.))
        .await
        .unwrap()
        .unwrap();
    let queued = finish(&owner, "queued", Some(1));
    assert!(store.save_owned(&owner, &queued, now(102.)).await.unwrap());
    let owner = store
        .claim(&tenant(), &r.id(), token("clean-b"), "clean-b", now(103.))
        .await
        .unwrap()
        .unwrap();
    let completed = finish(&owner, "completed", Some(1));
    assert!(
        store
            .save_owned(&owner, &completed, now(104.))
            .await
            .unwrap()
    );
    assert_eq!(
        completed.value()["budget"],
        json!({"unit":"adapter_search_invocations","limit":3,"reserved":2,"observed":2})
    );
    let mut forged = completed.value().clone();
    forged["stages"].as_array_mut().unwrap()[0]["attempts"]
        .as_array_mut()
        .unwrap()[0]["cursor"] = json!("forged");
    assert!(
        owner
            .record()
            .validate_completion(&Record::from_value(forged).unwrap())
            .is_err()
    );
    let mut incoherent = completed.value().clone();
    incoherent["stages"].as_array_mut().unwrap()[1]["attempts"]
        .as_array_mut()
        .unwrap()[0]["state"] = json!("running");
    assert!(Record::from_value(incoherent).is_err());
}
#[tokio::test]
async fn stale_prepared_claim_cannot_reuse_attempt_after_complete_retry_cycle() {
    let s = Server::start();
    let client = s.store();
    let store = StagedStore::new(&client);
    let scope = scopes();
    store
        .admit(&policy(), &record(), "idem", &[&scope], now(100.))
        .await
        .unwrap();
    let stale = store
        .prepare_claim(&tenant(), &id(), token("stale"), "same-attempt", now(101.))
        .await
        .unwrap()
        .unwrap();
    let owner = store
        .claim(
            &tenant(),
            &id(),
            token("current"),
            "same-attempt",
            now(101.),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(
        store
            .save_owned(&owner, &finish(&owner, "failed", None), now(102.))
            .await
            .unwrap()
    );
    assert_eq!(
        store
            .request_retry(&tenant(), &id(), "retry", now(103.))
            .await
            .unwrap()
            .status,
        Staged::Queued
    );
    assert!(
        store
            .claim_prepared(stale, now(104.))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store
            .claim(&tenant(), &id(), token("reuse"), "same-attempt", now(104.))
            .await
            .err(),
        Some(Failure::InvalidInput)
    );
    let owner = store
        .claim(&tenant(), &id(), token("fresh"), "fresh-attempt", now(104.))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(owner.record().value()["budget"]["reserved"], json!(2));
    assert!(
        store
            .save_owned(&owner, &finish(&owner, "completed", Some(1)), now(105.))
            .await
            .unwrap()
    );
    let RecordRead::Found(r) = store.read(&tenant(), &id(), now(105.)).await.unwrap() else {
        panic!()
    };
    assert_eq!(r.value()["budget"]["observed"], Value::Null);
}
#[tokio::test]
async fn legacy_index_ids_and_stable_cursor_ordering() {
    let s = Server::start();
    let client = s.store();
    let store = StagedStore::new(&client);
    let scope = scopes();
    let r = record();
    store
        .admit(&policy(), &r, "one", &[&scope], now(100.))
        .await
        .unwrap();
    let mut v = r.value().clone();
    v["operation_id"] = json!("z-last");
    let second = Record::from_value(v).unwrap();
    store
        .admit(&policy(), &second, "two", &[&scope], now(100.))
        .await
        .unwrap();
    let index = keys::staged("index", &tenant(), None).unwrap();
    let _: i64 = raw_cmd(&s, "DEL", &[&index]);
    let full = keys::staged("record", &tenant(), Some(id().as_str())).unwrap();
    let foreign = keys::staged(
        "record",
        &TenantId::new("foreign").unwrap(),
        Some("private"),
    )
    .unwrap();
    let _: i64 = raw_cmd(
        &s,
        "ZADD",
        &[
            &index,
            "3700",
            &full,
            "3700",
            "z-last",
            "3700",
            "idem:legacy:slot",
            "3700",
            &foreign,
        ],
    );
    let rows = store
        .list_recent(&tenant(), None, 20, now(101.))
        .await
        .unwrap();
    assert_eq!(
        rows.iter().map(|r| r.id().to_string()).collect::<Vec<_>>(),
        vec!["z-last", id().as_str()]
    );
    let next = store
        .list_recent(&tenant(), Some((100., "z-last".into())), 20, now(101.))
        .await
        .unwrap();
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].id(), id());
}
#[tokio::test]
async fn completion_unknown_and_durable_restart_reconcile_without_refund() {
    let mut s = Server::start();
    let client = s.store();
    let store = StagedStore::new(&client);
    let scope = scopes();
    store
        .admit(&policy(), &record(), "idem", &[&scope], now(100.))
        .await
        .unwrap();
    let owner = store
        .claim(&tenant(), &id(), token("a"), "attempt", now(101.))
        .await
        .unwrap()
        .unwrap();
    let _: () = raw_cmd(&s, "CLIENT", &["PAUSE", "650", "ALL"]);
    assert_eq!(
        store
            .save_owned(&owner, &finish(&owner, "failed", None), now(102.))
            .await,
        Err(Failure::CompletionUnknown)
    );
    tokio::time::sleep(Duration::from_millis(450)).await;
    let independent = s.store();
    let reader = StagedStore::new(&independent);
    let RecordRead::Found(r) = reader.read(&tenant(), &id(), now(103.)).await.unwrap() else {
        panic!()
    };
    assert_eq!(r.state(), "failed");
    assert_eq!(r.value()["budget"]["reserved"], json!(1));
    assert_eq!(r.value()["budget"]["observed"], Value::Null);
    let _: () = raw_cmd(&s, "SAVE", &[]);
    s.stop();
    s.restart();
    assert!(matches!(
        reader.read(&tenant(), &id(), now(103.)).await,
        Err(Failure::Unavailable)
    ));
    let RecordRead::Found(recovered) = reader.read(&tenant(), &id(), now(103.)).await.unwrap()
    else {
        panic!()
    };
    assert_eq!(recovered, r);
    assert_eq!(
        reader
            .request_retry(&tenant(), &id(), "retry", now(103.))
            .await
            .unwrap()
            .status,
        Staged::Queued
    );
    let claimed = reader
        .claim(&tenant(), &id(), token("b"), "attempt2", now(104.))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.record().value()["budget"]["reserved"], json!(2));
}
#[tokio::test]
async fn deadline_terminalization_and_live_orphan_recovery_preserve_charge() {
    let s = Server::start();
    let client = s.store();
    let store = StagedStore::new(&client);
    let scope = scopes();
    store
        .admit(&policy(), &record(), "idem", &[&scope], now(100.))
        .await
        .unwrap();
    assert!(
        store
            .claim(&tenant(), &id(), token("expired"), "expired", now(130.))
            .await
            .unwrap()
            .is_none()
    );
    let RecordRead::Found(r) = store.read(&tenant(), &id(), now(131.)).await.unwrap() else {
        panic!()
    };
    assert_eq!(r.state(), "failed");
    assert_eq!(r.value()["stop_reason"], json!("deadline_expired"));
    assert_eq!(r.value()["budget"]["reserved"], json!(0));
    assert_eq!(
        store
            .request_retry(&tenant(), &id(), "retry", now(131.))
            .await
            .unwrap()
            .status,
        Staged::None
    );
    let mut v = record().value().clone();
    v["operation_id"] = json!("orphan");
    let r = Record::from_value(v).unwrap();
    store
        .admit(&policy(), &r, "orphan", &[&scope], now(100.))
        .await
        .unwrap();
    let owner = store
        .claim(&tenant(), &r.id(), token("orphan"), "orphan", now(101.))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(store.recover_orphans(now(200.)).await.unwrap(), 0); // live lease wins even when hint is due
    let _: i64 = raw_cmd(
        &s,
        "DEL",
        &[&keys::staged("lease", &tenant(), Some(r.id().as_str())).unwrap()],
    );
    assert_eq!(store.recover_orphans(now(200.)).await.unwrap(), 1);
    assert!(
        !store
            .save_owned(&owner, &finish(&owner, "completed", Some(1)), now(104.))
            .await
            .unwrap()
    );
    let RecordRead::Found(r) = store.read(&tenant(), &r.id(), now(201.)).await.unwrap() else {
        panic!()
    };
    assert_eq!(r.value()["budget"]["reserved"], json!(1));
    assert_eq!(
        store
            .request_retry(&tenant(), &r.id(), "deadline", now(201.))
            .await
            .unwrap()
            .status,
        Staged::Deadline
    );
}
#[tokio::test]
async fn admission_rejects_unbound_query_filters_http_origin_and_revoked_scope() {
    let s = Server::start();
    let client = s.store();
    let store = StagedStore::new(&client);
    let scope = scopes();
    for (field, value) in [
        ("query", json!("different")),
        ("language", json!("fr")),
        ("freshness", json!("prefer_fresh")),
    ] {
        let mut v = record().value().clone();
        if field == "query" {
            v["plan"][field] = value;
        } else {
            v["plan"]["filters"][field] = value;
        }
        v["plan_digest"] = json!(plan_digest(&identity(&v)).unwrap());
        let r = Record::from_value(v).unwrap();
        assert_eq!(
            store
                .admit(&policy(), &r, "denied", &[&scope], now(100.))
                .await
                .err(),
            Some(Failure::InvalidInput)
        );
    }
    let mut p = ScopeProposal::generic(SearchRequest {
        query: record().value()["plan"]["query"].as_str().unwrap().into(),
        engines: Some(vec!["alpha".into()]),
        ..Default::default()
    });
    p.surface = SearchSurface::Http;
    let http = policy().authorize(p).unwrap();
    assert_eq!(
        store
            .admit(&policy(), &record(), "http", &[&http], now(100.))
            .await
            .err(),
        Some(Failure::InvalidInput)
    );
    let mut revoked = policy();
    revoked.catalog = CatalogSnapshot::new(vec![]).unwrap();
    assert_eq!(
        store
            .admit(&revoked, &record(), "revoked", &[&scope], now(100.))
            .await
            .err(),
        Some(Failure::InvalidInput)
    );
    assert_eq!(
        store.read(&tenant(), &id(), now(100.)).await.unwrap(),
        RecordRead::Missing
    );
}

#[tokio::test]
async fn retry_completion_preserves_derivable_observed_total() {
    for prior in [Some(0), Some(1), None] {
        let server = Server::start();
        let client = server.store();
        let store = StagedStore::new(&client);
        let scope = scopes();
        store
            .admit(&policy(), &record(), "observed", &[&scope], now(100.))
            .await
            .unwrap();
        let first = store
            .claim(&tenant(), &id(), token("first"), "first", now(101.))
            .await
            .unwrap()
            .unwrap();
        let failed = finish(&first, "failed", prior);
        assert!(store.save_owned(&first, &failed, now(102.)).await.unwrap());
        assert_eq!(
            store
                .request_retry(&tenant(), &id(), "retry", now(103.))
                .await
                .unwrap()
                .status,
            Staged::Queued
        );
        let second = store
            .claim(&tenant(), &id(), token("second"), "second", now(104.))
            .await
            .unwrap()
            .unwrap();
        let completed = finish(&second, "completed", Some(1));
        if prior.is_some() {
            let mut erased = completed.value().clone();
            erased["budget"]["observed"] = Value::Null;
            let erased = Record::from_value(erased).unwrap();
            assert_eq!(
                store.save_owned(&second, &erased, now(105.)).await,
                Err(Failure::InvalidInput)
            );
            let RecordRead::Found(unchanged) =
                store.read(&tenant(), &id(), now(105.)).await.unwrap()
            else {
                panic!("record missing")
            };
            assert_eq!(unchanged.value(), second.record().value());
        }
        assert!(
            store
                .save_owned(&second, &completed, now(105.))
                .await
                .unwrap()
        );
        let RecordRead::Found(saved) = store.read(&tenant(), &id(), now(105.)).await.unwrap()
        else {
            panic!("record missing")
        };
        assert_eq!(
            saved.value()["budget"]["observed"],
            json!(prior.map(|n| n + 1))
        );
        assert_eq!(
            saved.value()["stages"][0]["attempts"][0],
            second.record().value()["stages"][0]["attempts"][0]
        );
    }
}
