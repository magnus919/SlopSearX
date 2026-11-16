//! Actual private Valkey authority; no providers, external URLs or Python.
#[path = "w07_support/valkey.rs"]
mod private_valkey;
use sha2::{Digest, Sha256};
use ssx_application::receipts::*;
use ssx_domain::{TenantId, WallClockTimestamp};
use ssx_infra::valkey::{Valkey, retrieval::ValkeyReceipts};
use ssx_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
#[derive(Default)]
struct Ids(AtomicUsize);
impl ReceiptIds for Ids {
    fn next(&self) -> Result<String, Failure> {
        Ok(format!(
            "receipt-{:024}",
            self.0.fetch_add(1, Ordering::SeqCst) + 1
        ))
    }
}
fn now(value: f64) -> WallClockTimestamp {
    WallClockTimestamp::new(value).unwrap()
}
fn tenant(value: &str) -> TenantId {
    TenantId::new(value).unwrap()
}
fn body() -> Value {
    json!({"contract":CONTRACT,"version":1,"result_id":"snap-source:0","attribution":{"retriever":"reader","authenticated_tenant":"tenant"},"observation":{"status":"failed","failure":{"code":"blocked","message":null},"passage_refs":[]},"discovery":{"result_id":"snap-source:0","verified":false},"observations_verified":false})
}
fn token(s: &str) -> String {
    Sha256::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn key(kind: &str, id: Option<&str>) -> String {
    let p = format!(
        "mcp:retrieval-receipt:v1:{kind}:tenant:{}",
        token("snap-source:0")
    );
    match id {
        Some(s) => format!("{p}:{s}"),
        None => p,
    }
}
#[tokio::test]
async fn atomic_replay_conflict_capacity_fixed_horizon_and_tenant_isolation() {
    let server = private_valkey::Server::start();
    let backend = Arc::new(server.store());
    let repo = Arc::new(ValkeyReceipts::new(
        backend.clone(),
        Arc::new(Ids::default()),
    ));
    let t = tenant("tenant");
    let c = Cancellation::default();
    let receipt = body();
    let mut attempts = vec![];
    for _ in 0..8 {
        let (repo, t, c, receipt) = (repo.clone(), t.clone(), c.clone(), receipt.clone());
        attempts.push(tokio::spawn(async move {
            repo.submit(Insert {
                tenant: &t,
                result_id: "snap-source:0",
                idempotency_key: "same-key",
                digest: "same-digest",
                receipt: &receipt,
                now: now(1000.25),
                cancellation: &c,
            })
            .await
            .unwrap()
        }))
    }
    let mut created = vec![];
    let mut replayed = vec![];
    for task in attempts {
        match task.await.unwrap() {
            Submission::Created(v) => created.push(v),
            Submission::Replayed(v) => replayed.push(v),
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(created.len(), 1);
    assert_eq!(replayed.len(), 7);
    assert!(replayed.iter().all(|r| r == &created[0]));
    let expiry = created[0]["expires_at"].as_f64().unwrap();
    assert_eq!(expiry, 87400.25);
    assert_eq!(
        repo.replay(
            &t,
            "snap-source:0",
            "same-key",
            "different",
            now(1001.0),
            &c
        )
        .await
        .unwrap(),
        Replay::Conflict
    );
    assert!(matches!(
        repo.replay(
            &t,
            "snap-source:0",
            "same-key",
            "same-digest",
            now(1001.0),
            &c
        )
        .await
        .unwrap(),
        Replay::Replayed(_)
    ));
    let first_id = created[0]["receipt_id"].as_str().unwrap();
    let first_ttl: i64 = redis::cmd("TTL")
        .arg(key("receipt", Some(first_id)))
        .query(&mut server.connection())
        .unwrap();
    for i in 1..20 {
        let idem = format!("key-{i}");
        let digest = format!("digest-{i}");
        let result = repo
            .submit(Insert {
                tenant: &t,
                result_id: "snap-source:0",
                idempotency_key: &idem,
                digest: &digest,
                receipt: &receipt,
                now: now(1000.25 + i as f64),
                cancellation: &c,
            })
            .await
            .unwrap();
        let Submission::Created(v) = result else {
            panic!("{result:?}")
        };
        assert_eq!(v["expires_at"].as_f64(), Some(expiry));
    }
    assert_eq!(
        repo.submit(Insert {
            tenant: &t,
            result_id: "snap-source:0",
            idempotency_key: "overflow",
            digest: "new",
            receipt: &receipt,
            now: now(1020.0),
            cancellation: &c
        })
        .await
        .unwrap(),
        Submission::Capacity
    );
    let remaining: i64 = redis::cmd("TTL")
        .arg(key("receipt", Some(first_id)))
        .query(&mut server.connection())
        .unwrap();
    assert!(
        remaining <= first_ttl,
        "later receipts extended the first receipt TTL"
    );
    let read = repo
        .read(&t, "snap-source:0", now(1020.0), 5, &c)
        .await
        .unwrap();
    assert_eq!((read.receipts.len(), read.total), (5, 20));
    let ids: Vec<_> = read
        .receipts
        .iter()
        .map(|r| r["receipt_id"].as_str().unwrap())
        .collect();
    assert!(ids.windows(2).all(|v| v[0] > v[1]));
    assert_eq!(
        repo.read(&tenant("other"), "snap-source:0", now(1020.0), 20, &c)
            .await
            .unwrap(),
        Retained {
            receipts: vec![],
            total: 0
        }
    );
    assert_eq!(
        repo.read(&t, "snap-source:0", now(expiry), 20, &c)
            .await
            .unwrap()
            .total,
        0,
        "receipt horizon uses <= at the exact boundary"
    );
    let Submission::Created(renewed) = repo
        .submit(Insert {
            tenant: &t,
            result_id: "snap-source:0",
            idempotency_key: "same-key",
            digest: "new-era",
            receipt: &receipt,
            now: now(expiry + 1.0),
            cancellation: &c,
        })
        .await
        .unwrap()
    else {
        panic!("new horizon not created")
    };
    assert_eq!(
        renewed["expires_at"].as_f64(),
        Some(expiry + 1.0 + RETENTION_SECONDS as f64)
    );
    assert_eq!(
        repo.read(&t, "snap-source:0", now(expiry + 1.0), 20, &c)
            .await
            .unwrap()
            .total,
        1
    );
    let original: Option<Vec<u8>> = redis::cmd("GET")
        .arg(key("receipt", Some(first_id)))
        .query(&mut server.connection())
        .unwrap();
    assert!(
        original.is_none(),
        "new horizon retained an obsolete receipt key"
    );
    backend.close().await;
}
#[tokio::test]
async fn oversize_is_rejected_without_fresh_writes_and_store_close_is_terminal() {
    let server = private_valkey::Server::start();
    let backend = Arc::new(server.store());
    let ids = Arc::new(Ids::default());
    let repo = ValkeyReceipts::new(backend.clone(), ids.clone());
    let t = tenant("tenant");
    let c = Cancellation::default();
    let receipt = json!({"large":"x".repeat(MAX_RECEIPT_BYTES)});
    assert_eq!(
        repo.submit(Insert {
            tenant: &t,
            result_id: "snap-source:0",
            idempotency_key: "oversize",
            digest: "digest",
            receipt: &receipt,
            now: now(1000.0),
            cancellation: &c
        })
        .await
        .unwrap(),
        Submission::Size
    );
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg("*")
        .query(&mut server.connection())
        .unwrap();
    assert!(keys.is_empty());
    c.cancel();
    assert_eq!(
        repo.read(&t, "snap-source:0", now(1000.0), 20, &c).await,
        Err(Failure::Cancelled)
    );
    backend.close().await;
    assert_eq!(
        repo.read(
            &t,
            "snap-source:0",
            now(1000.0),
            20,
            &Cancellation::default()
        )
        .await,
        Err(Failure::Closed)
    );
    assert_eq!(ids.0.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn corrupt_index_preflight_preserves_expired_idempotency_without_partial_cleanup() {
    let server = private_valkey::Server::start();
    let backend = Arc::new(Valkey::new(server.url(), Duration::from_millis(300)).unwrap());
    let ids = Arc::new(Ids::default());
    let repo = ValkeyReceipts::new(backend.clone(), ids.clone());
    let t = tenant("tenant");
    let c = Cancellation::default();
    let idem = key("idem", Some(&token("key")));
    redis::cmd("SET")
        .arg(&idem)
        .arg(r#"{"expires_at":0,"digest":"old","receipt_id":"receipt-old"}"#)
        .query::<()>(&mut server.connection())
        .unwrap();
    redis::cmd("SET")
        .arg(key("horizon", None))
        .arg(r#"{"expires_at":2000}"#)
        .query::<()>(&mut server.connection())
        .unwrap();
    redis::cmd("SET")
        .arg(key("index", None))
        .arg("wrongtype")
        .query::<()>(&mut server.connection())
        .unwrap();
    let receipt = body();
    assert_eq!(
        repo.submit(Insert {
            tenant: &t,
            result_id: "snap-source:0",
            idempotency_key: "key",
            digest: "digest",
            receipt: &receipt,
            now: now(1000.0),
            cancellation: &c
        })
        .await,
        Err(Failure::Malformed)
    );
    assert_eq!(
        redis::cmd("EXISTS")
            .arg(idem)
            .query::<usize>(&mut server.connection())
            .unwrap(),
        1
    );
    assert_eq!(
        ids.0.load(Ordering::SeqCst),
        1,
        "known corrupt preflight must not retry with another ID"
    );
    backend.close().await;
}

struct FixedIds {
    calls: AtomicUsize,
}
impl ReceiptIds for FixedIds {
    fn next(&self) -> Result<String, Failure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok("receipt-CCCCCCCCCCCCCCCCCCCCCCCC".into())
    }
}
fn retained_bytes(server: &private_valkey::Server) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut conn = server.connection();
    let keys: Vec<String> = redis::cmd("KEYS").arg("*").query(&mut conn).unwrap();
    keys.into_iter()
        .map(|key| {
            let bytes = redis::cmd("DUMP").arg(&key).query(&mut conn).unwrap();
            (key, bytes)
        })
        .collect()
}
fn set(server: &private_valkey::Server, key: &str, value: &str) {
    redis::cmd("SET")
        .arg(key)
        .arg(value)
        .arg("EX")
        .arg(600)
        .query::<()>(&mut server.connection())
        .unwrap();
}
#[tokio::test]
async fn same_id_collision_never_overwrites_and_valid_replay_precedes_corrupt_unrelated_index() {
    let server = private_valkey::Server::start();
    let backend = Arc::new(server.store());
    let ids = Arc::new(FixedIds {
        calls: AtomicUsize::new(0),
    });
    let repo = ValkeyReceipts::new(backend.clone(), ids.clone());
    let t = tenant("tenant");
    let c = Cancellation::default();
    let a = body();
    let mut b = a.clone();
    b["observation"]["capture_ref"] = json!("capture://B");
    let first = repo
        .submit(Insert {
            tenant: &t,
            result_id: "snap-source:0",
            idempotency_key: "A",
            digest: "a",
            receipt: &a,
            now: now(1000.),
            cancellation: &c,
        })
        .await
        .unwrap();
    let Submission::Created(first) = first else {
        panic!("first must be created")
    };
    let before = retained_bytes(&server);
    assert_eq!(
        repo.submit(Insert {
            tenant: &t,
            result_id: "snap-source:0",
            idempotency_key: "B",
            digest: "b",
            receipt: &b,
            now: now(1001.),
            cancellation: &c
        })
        .await,
        Err(Failure::Collision)
    );
    assert_eq!(
        retained_bytes(&server),
        before,
        "collision must not mutate existing or create new keys"
    );
    assert_eq!(
        repo.replay(&t, "snap-source:0", "A", "a", now(1002.), &c)
            .await
            .unwrap(),
        Replay::Replayed(first.clone())
    );
    // A completed idempotent replay is readonly and remains valid despite
    // unrelated corrupt index metadata and a colliding next generated ID.
    set(&server, &key("index", None), "wrong-type");
    let before = retained_bytes(&server);
    assert_eq!(
        repo.submit(Insert {
            tenant: &t,
            result_id: "snap-source:0",
            idempotency_key: "A",
            digest: "a",
            receipt: &a,
            now: now(1003.),
            cancellation: &c
        })
        .await
        .unwrap(),
        Submission::Replayed(first)
    );
    assert_eq!(
        repo.submit(Insert {
            tenant: &t,
            result_id: "snap-source:0",
            idempotency_key: "A",
            digest: "other",
            receipt: &a,
            now: now(1004.),
            cancellation: &c
        })
        .await
        .unwrap(),
        Submission::Conflict
    );
    assert_eq!(retained_bytes(&server), before);
    assert_eq!(ids.calls.load(Ordering::SeqCst), 4);
    backend.close().await;
}
#[tokio::test]
async fn every_known_corrupt_preflight_is_atomic_and_never_renews_retention() {
    for case in [
        "horizon_type",
        "index_type",
        "receipt_type",
        "idem_type",
        "horizon_json",
        "idem_json",
        "idem_field",
        "prior_type",
        "prior_identity",
        "old_cleanup_type",
        "orphan_index_collision",
    ] {
        let server = private_valkey::Server::start();
        let backend = Arc::new(server.store());
        let ids = Arc::new(Ids::default());
        let repo = ValkeyReceipts::new(backend.clone(), ids.clone());
        let t = tenant("tenant");
        let c = Cancellation::default();
        let idem = key("idem", Some(&token("key")));
        let horizon = key("horizon", None);
        let index = key("index", None);
        let candidate = key("receipt", Some("receipt-000000000000000000000001"));
        let prior = key("receipt", Some("receipt-old"));
        set(
            &server,
            &idem,
            r#"{"expires_at":0,"digest":"old","receipt_id":"receipt-old"}"#,
        );
        set(&server, &horizon, r#"{"expires_at":2000}"#);
        let wrong_type = match case {
            "horizon_type" => Some(&horizon),
            "index_type" => Some(&index),
            "receipt_type" => Some(&candidate),
            "idem_type" => Some(&idem),
            _ => None,
        };
        if let Some(key) = wrong_type {
            redis::cmd("DEL")
                .arg(key)
                .query::<()>(&mut server.connection())
                .unwrap();
            redis::cmd("HSET")
                .arg(key)
                .arg("field")
                .arg("value")
                .query::<()>(&mut server.connection())
                .unwrap();
        }
        match case {
            "horizon_json" => set(&server, &horizon, "not-json"),
            "idem_json" => set(&server, &idem, "not-json"),
            "idem_field" => set(
                &server,
                &idem,
                r#"{"expires_at":0,"digest":7,"receipt_id":"receipt-old"}"#,
            ),
            "prior_type" | "prior_identity" => {
                set(
                    &server,
                    &idem,
                    r#"{"expires_at":2000,"digest":"digest","receipt_id":"receipt-old"}"#,
                );
                if case == "prior_type" {
                    redis::cmd("HSET")
                        .arg(&prior)
                        .arg("field")
                        .arg("value")
                        .query::<()>(&mut server.connection())
                        .unwrap();
                } else {
                    set(&server, &prior, r#"{"receipt_id":"different"}"#);
                }
            }
            "old_cleanup_type" => {
                set(&server, &horizon, r#"{"expires_at":0}"#);
                redis::cmd("ZADD")
                    .arg(&index)
                    .arg(0)
                    .arg("receipt-old")
                    .query::<()>(&mut server.connection())
                    .unwrap();
                redis::cmd("HSET")
                    .arg(&prior)
                    .arg("field")
                    .arg("value")
                    .query::<()>(&mut server.connection())
                    .unwrap();
            }
            "orphan_index_collision" => {
                redis::cmd("ZADD")
                    .arg(&index)
                    .arg(0)
                    .arg("receipt-000000000000000000000001")
                    .query::<()>(&mut server.connection())
                    .unwrap();
            }
            _ => {}
        }
        let before = retained_bytes(&server);
        let ttl: i64 = redis::cmd("PTTL")
            .arg(&horizon)
            .query(&mut server.connection())
            .unwrap();
        let receipt = body();
        let result = repo
            .submit(Insert {
                tenant: &t,
                result_id: "snap-source:0",
                idempotency_key: "key",
                digest: "digest",
                receipt: &receipt,
                now: now(1000.),
                cancellation: &c,
            })
            .await;
        assert_eq!(
            result,
            Err(if case == "orphan_index_collision" {
                Failure::Collision
            } else {
                Failure::Malformed
            }),
            "{case}"
        );
        assert_eq!(retained_bytes(&server), before, "{case}");
        let after: i64 = redis::cmd("PTTL")
            .arg(horizon)
            .query(&mut server.connection())
            .unwrap();
        assert!(after <= ttl, "{case} must not renew retention");
        assert_eq!(ids.0.load(Ordering::SeqCst), 1, "{case} no retry");
        backend.close().await;
    }
}
#[tokio::test]
async fn capacity_and_size_rejections_preserve_expired_idempotency_and_its_ttl() {
    for capacity in [false, true] {
        let server = private_valkey::Server::start();
        let backend = Arc::new(server.store());
        let ids = Arc::new(Ids::default());
        let repo = ValkeyReceipts::new(backend.clone(), ids);
        let t = tenant("tenant");
        let c = Cancellation::default();
        let idem = key("idem", Some(&token("key")));
        set(
            &server,
            &idem,
            r#"{"expires_at":0,"digest":"old","receipt_id":"receipt-old"}"#,
        );
        set(&server, &key("horizon", None), r#"{"expires_at":2000}"#);
        if capacity {
            for i in 0..20 {
                redis::cmd("ZADD")
                    .arg(key("index", None))
                    .arg(i)
                    .arg(format!("old-{i}"))
                    .query::<()>(&mut server.connection())
                    .unwrap();
            }
        }
        let before = retained_bytes(&server);
        let ttl: i64 = redis::cmd("PTTL")
            .arg(&idem)
            .query(&mut server.connection())
            .unwrap();
        let receipt = json!({"large":"x".repeat(MAX_RECEIPT_BYTES)});
        assert_eq!(
            repo.submit(Insert {
                tenant: &t,
                result_id: "snap-source:0",
                idempotency_key: "key",
                digest: "digest",
                receipt: &receipt,
                now: now(1000.),
                cancellation: &c
            })
            .await
            .unwrap(),
            if capacity {
                Submission::Capacity
            } else {
                Submission::Size
            }
        );
        assert_eq!(retained_bytes(&server), before);
        let after: i64 = redis::cmd("PTTL")
            .arg(idem)
            .query(&mut server.connection())
            .unwrap();
        assert!(after <= ttl);
        backend.close().await;
    }
}
#[tokio::test]
async fn dispatched_timeout_is_unknown_and_never_retries_with_another_id() {
    let server = private_valkey::Server::start();
    let backend = Arc::new(Valkey::new(server.url(), Duration::from_millis(100)).unwrap());
    let ids = Arc::new(Ids::default());
    let repo = ValkeyReceipts::new(backend.clone(), ids.clone());
    let t = tenant("tenant");
    let c = Cancellation::default();
    let receipt = body();
    redis::cmd("CLIENT")
        .arg("PAUSE")
        .arg(10000)
        .arg("WRITE")
        .query::<()>(&mut server.connection())
        .unwrap();
    let result = repo
        .submit(Insert {
            tenant: &t,
            result_id: "snap-source:0",
            idempotency_key: "key",
            digest: "digest",
            receipt: &receipt,
            now: now(1000.),
            cancellation: &c,
        })
        .await;
    // Explicit release follows the bounded operation; no wall-clock delay is
    // used to infer when the server wrote or to extend the write deadline.
    redis::cmd("CLIENT")
        .arg("UNPAUSE")
        .query::<()>(&mut server.connection())
        .unwrap();
    assert_eq!(result, Err(Failure::CompletionUnknown));
    assert_eq!(ids.0.load(Ordering::SeqCst), 1);
    backend.close().await;
}

#[test]
fn fixed_script_fresh_record_identity_faults_are_rejected_before_mutation() {
    // Deliberate qualification-only fault injection below the repository input
    // constructor. Production never exposes this EVAL or caller-built arguments.
    let server = private_valkey::Server::start();
    for id in [Value::Null, json!(7), json!(""), json!("receipt-other")] {
        let before = retained_bytes(&server);
        let receipt = json!({"receipt_id":id,"submitted_at":1000,"observation":{}});
        let raw: String = redis::cmd("EVAL")
            .arg(include_str!("../src/valkey/retrieval_scripts/insert.lua"))
            .arg(4)
            .arg(key("horizon", None))
            .arg(key("index", None))
            .arg(key("receipt", Some("receipt-candidate")))
            .arg(key("idem", Some(&token("key"))))
            .arg("1000.0")
            .arg("digest")
            .arg(format!("{}:", key("receipt", None)))
            .arg(RETENTION_SECONDS)
            .arg(STORE_MARGIN_SECONDS)
            .arg(MAX_RECEIPTS_PER_RESULT)
            .arg(ssx_json::to_string(&receipt).unwrap())
            .arg("1000.0")
            .arg("receipt-candidate")
            .arg(MAX_RECEIPT_BYTES)
            .query(&mut server.connection())
            .unwrap();
        assert_eq!(
            ssx_json::from_str::<Value>(&raw).unwrap()["status"],
            "corrupt"
        );
        assert_eq!(retained_bytes(&server), before);
    }
}

#[tokio::test]
async fn competing_different_digests_commit_one_receipt_without_orphan_keys() {
    let server = private_valkey::Server::start();
    // Separate owners establish independent connections to the same authority.
    let backends = [Arc::new(server.store()), Arc::new(server.store())];
    for backend in &backends {
        backend.ping().await.unwrap();
    }
    let repos: Vec<_> = backends
        .iter()
        .enumerate()
        .map(|(owner, backend)| {
            Arc::new(ValkeyReceipts::new(
                backend.clone(),
                Arc::new(Ids(AtomicUsize::new(owner * 100))),
            ))
        })
        .collect();
    let ready = Arc::new(tokio::sync::Barrier::new(8));
    let mut attempts = vec![];
    for i in 0..8 {
        let repo = repos[i % repos.len()].clone();
        let ready = ready.clone();
        attempts.push(tokio::spawn(async move {
            ready.wait().await;
            repo.submit(Insert {
                tenant: &tenant("tenant"),
                result_id: "snap-source:0",
                idempotency_key: "shared",
                digest: &format!("digest-{i}"),
                receipt: &body(),
                now: now(1000.),
                cancellation: &Cancellation::default(),
            })
            .await
            .unwrap()
        }));
    }
    let mut created = 0;
    let mut conflicts = 0;
    for attempt in attempts {
        match attempt.await.unwrap() {
            Submission::Created(receipt) => {
                created += 1;
                assert_eq!(receipt["observations_verified"], false);
                assert_eq!(receipt["discovery"]["verified"], false);
            }
            Submission::Conflict => conflicts += 1,
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!((created, conflicts), (1, 7));
    assert_eq!(
        retained_bytes(&server).len(),
        4,
        "conflicts must not leave receipt or idempotency orphans"
    );
    let repo = &repos[0];
    let retained = repo
        .read(
            &tenant("tenant"),
            "snap-source:0",
            now(1001.),
            20,
            &Cancellation::default(),
        )
        .await
        .unwrap();
    assert_eq!((retained.total, retained.receipts.len()), (1, 1));
    for key in retained_bytes(&server).keys() {
        let ttl: i64 = redis::cmd("TTL")
            .arg(key)
            .query(&mut server.connection())
            .unwrap();
        assert!((1..=(RETENTION_SECONDS + STORE_MARGIN_SECONDS) as i64).contains(&ttl));
    }
    assert_eq!(
        repo.replay(
            &tenant("other"),
            "snap-source:0",
            "shared",
            "digest-0",
            now(1001.),
            &Cancellation::default()
        )
        .await
        .unwrap(),
        Replay::Missing
    );
    // Closing one owner must not close the other connection.
    backends[0].close().await;
    assert_eq!(
        repos[1]
            .read(
                &tenant("tenant"),
                "snap-source:0",
                now(1001.),
                20,
                &Cancellation::default()
            )
            .await
            .unwrap(),
        retained
    );
    backends[1].close().await;
}

#[tokio::test]
async fn missing_receipts_preserve_index_count_and_malformed_references_fail_without_mutation() {
    let server = private_valkey::Server::start();
    let backend = Arc::new(server.store());
    let repo = ValkeyReceipts::new(backend.clone(), Arc::new(Ids::default()));
    let t = tenant("tenant");
    let c = Cancellation::default();
    let Submission::Created(created) = repo
        .submit(Insert {
            tenant: &t,
            result_id: "snap-source:0",
            idempotency_key: "key",
            digest: "digest",
            receipt: &body(),
            now: now(1000.),
            cancellation: &c,
        })
        .await
        .unwrap()
    else {
        panic!("receipt must be created")
    };
    redis::cmd("DEL")
        .arg(key("receipt", created["receipt_id"].as_str()))
        .query::<()>(&mut server.connection())
        .unwrap();
    assert_eq!(
        repo.read(&t, "snap-source:0", now(1001.), 20, &c)
            .await
            .unwrap(),
        Retained {
            receipts: vec![],
            total: 1
        }
    );
    assert_eq!(
        repo.replay(&t, "snap-source:0", "key", "digest", now(1001.), &c)
            .await
            .unwrap(),
        Replay::Missing
    );
    // Stored references are fault-injected in this isolated server only.
    for malformed in [
        r#"{"expires_at":2000,"digest":"digest","receipt_id":7}"#,
        r#"{"expires_at":2000,"digest":7,"receipt_id":"receipt-old"}"#,
    ] {
        set(&server, &key("idem", Some(&token("key"))), malformed);
        let before = retained_bytes(&server);
        assert_eq!(
            repo.replay(&t, "snap-source:0", "key", "digest", now(1001.), &c)
                .await,
            Err(Failure::Malformed)
        );
        assert_eq!(retained_bytes(&server), before);
    }
    redis::cmd("ZADD")
        .arg(key("index", None))
        .arg(1001)
        .arg("")
        .query::<()>(&mut server.connection())
        .unwrap();
    let before = retained_bytes(&server);
    assert_eq!(
        repo.read(&t, "snap-source:0", now(1001.), 20, &c).await,
        Err(Failure::Malformed)
    );
    assert_eq!(retained_bytes(&server), before);
    backend.close().await;
}

#[tokio::test]
async fn bounded_inputs_fail_before_ids_or_writes_and_unavailable_is_not_missing() {
    let server = private_valkey::Server::start();
    let backend = Arc::new(server.store());
    let ids = Arc::new(Ids::default());
    let repo = ValkeyReceipts::new(backend.clone(), ids.clone());
    let t = tenant("tenant");
    let c = Cancellation::default();
    for invalid in [String::new(), "x".repeat(8193)] {
        assert_eq!(
            repo.submit(Insert {
                tenant: &t,
                result_id: &invalid,
                idempotency_key: "key",
                digest: "digest",
                receipt: &body(),
                now: now(1000.),
                cancellation: &c,
            })
            .await,
            Err(Failure::InvalidInput)
        );
        assert_eq!(
            repo.read(&t, &invalid, now(1000.), 20, &c).await,
            Err(Failure::InvalidInput)
        );
    }
    assert_eq!(
        repo.replay(
            &t,
            "snap-source:0",
            &"k".repeat(513),
            "digest",
            now(1000.),
            &c
        )
        .await,
        Err(Failure::InvalidInput)
    );
    assert_eq!(
        repo.replay(&t, "snap-source:0", "key", &"d".repeat(257), now(1000.), &c)
            .await,
        Err(Failure::InvalidInput)
    );
    assert_eq!(ids.0.load(Ordering::SeqCst), 0);
    assert!(retained_bytes(&server).is_empty());
    backend.close().await;
    // This endpoint belonged to our joined private server; there is no external
    // URL, port selection, or accidental production-store fallback.
    let url = server.url().to_owned();
    drop(server);
    let unavailable = Arc::new(Valkey::new(&url, Duration::from_millis(100)).unwrap());
    let repo = ValkeyReceipts::new(unavailable.clone(), ids.clone());
    assert_eq!(
        repo.read(&t, "snap-source:0", now(1000.), 20, &c).await,
        Err(Failure::Unavailable)
    );
    assert_eq!(
        repo.replay(&t, "snap-source:0", "key", "digest", now(1000.), &c)
            .await,
        Err(Failure::Unavailable)
    );
    assert_eq!(
        repo.submit(Insert {
            tenant: &t,
            result_id: "snap-source:0",
            idempotency_key: "key",
            digest: "digest",
            receipt: &body(),
            now: now(1000.),
            cancellation: &c,
        })
        .await,
        Err(Failure::Unavailable)
    );
    assert_eq!(
        ids.0.load(Ordering::SeqCst),
        1,
        "unavailable submission must not generate retry IDs"
    );
    unavailable.close().await;
}
