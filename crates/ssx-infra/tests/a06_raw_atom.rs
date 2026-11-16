//! Actual same-process raw-header/body Atom path, not HTTP/factory acceptance.
use base64::Engine as _;
use ssx_domain::AdapterResponse;
use ssx_infra::adapters::{
    arxiv_expat::{AtomPool, RawAdmissionError},
    scholarly_frame::Header,
};
use ssx_json::{Value, json};
use ssx_owned_budget::Budget;
use std::{num::NonZeroUsize, path::PathBuf, time::Duration};
use tokio::time::Instant;
fn pool() -> AtomPool {
    AtomPool::new(
        PathBuf::from(env!("CARGO_BIN_EXE_ssx-atom-parser")),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap()
}
fn normalized(response: &AdapterResponse) -> Value {
    assert!(
        response.results.iter().all(|result| result.media.is_none()),
        "actual science result unexpectedly contains media"
    );
    json!({"results":response.results.iter().map(|r|json!({"url":r.url,"title":r.title,"content":r.content,"engine":r.engine,"engines":r.engines.iter().collect::<Vec<_>>(),"score":r.score,"position":r.position,"category":r.category,"published_date":r.published_date,"thumbnail":r.thumbnail,"img_src":r.img_src,"tier":r.tier,"media":null,"payload":r.payload,"work_group":r.work_group})).collect::<Vec<_>>(),"status":response.status,"error_message":response.error_message,"answers":response.answers,"corrections":response.corrections,"infoboxes":response.infoboxes,"synthetic":response.synthetic})
}
fn headers(reply: &Value) -> Vec<(Vec<u8>, Vec<u8>)> {
    if let Some(pairs) = reply["headers_b64"].as_array() {
        pairs
            .iter()
            .map(|pair| {
                (
                    base64::engine::general_purpose::STANDARD
                        .decode(pair[0].as_str().unwrap())
                        .unwrap(),
                    base64::engine::general_purpose::STANDARD
                        .decode(pair[1].as_str().unwrap())
                        .unwrap(),
                )
            })
            .collect()
    } else {
        reply["headers"]
            .as_object()
            .map(|values| {
                values
                    .iter()
                    .map(|(k, v)| {
                        (
                            k.as_bytes().to_vec(),
                            v.as_str().unwrap().as_bytes().to_vec(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}
#[tokio::test]
async fn same_helper_complete_source_outcomes_include_charset_and_surrogate_precedence() {
    use sha2::Digest;
    let selection: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a06/atom-raw-selected-ids.json"
    ))
    .unwrap();
    let mut unique_ids = std::collections::BTreeSet::new();
    let pool = pool();
    let mut counts = Vec::new();
    let mut deltas = Vec::new();
    for (corpus_index, source) in [
        include_str!("../../../docs/rust-rewrite/a06/source-charset.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-atom-decode.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-xml.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-initial.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-regressions.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-shared.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-xml-grammar.json"),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            sha2::Sha256::digest(source.as_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            selection["corpora"][corpus_index]["sha256"]
                .as_str()
                .unwrap()
        );
        let corpus: Value = ssx_json::from_str(source).unwrap();
        let mut selected_ids = Vec::new();
        let mut count = 0;
        for row in corpus["records"].as_array().unwrap() {
            if row["engine"] != json!("arxiv") {
                continue;
            }
            // Declared fixture responses can include an unused200 after a
            // refused redirect. Select only the last actually captured request.
            let requests = row["requests"].as_array().unwrap().len();
            if requests == 0 {
                continue;
            }
            let reply = &row["responses"].as_array().unwrap()[requests - 1];
            if reply["status"] != json!(200) || reply["body_b64"].as_str().is_none() {
                continue;
            }
            let body = base64::engine::general_purpose::STANDARD
                .decode(reply["body_b64"].as_str().unwrap())
                .unwrap();
            let headers = headers(reply);
            let budget = Budget::new(512 * 1024 * 1024).unwrap();
            // Fixture inputs are borrowed existing owners. Retain their capacity
            // charge throughout the operation; this is not transport precharge.
            let input_bytes = body.capacity()
                + headers.capacity() * std::mem::size_of::<(Vec<u8>, Vec<u8>)>()
                + headers
                    .iter()
                    .map(|(k, v)| k.capacity() + v.capacity())
                    .sum::<usize>();
            let input_charge = budget.reserve(input_bytes).unwrap();
            let op = pool
                .begin_raw(budget.clone(), Instant::now() + Duration::from_secs(10))
                .unwrap();
            let response = op
                .parse(
                    headers.iter().map(|(name, value)| Header { name, value }),
                    &body,
                )
                .await;
            assert_eq!(
                budget.snapshot().unwrap().used,
                input_charge.bytes(),
                "{}",
                row["id"]
            );
            let id = row["id"].as_str().unwrap();
            assert!(
                unique_ids.insert(id.to_owned()),
                "duplicate selected ID {id}"
            );
            selected_ids.push(id.to_owned());
            if ["arxiv.grammar.declaration_2", "arxiv.grammar.declaration_3"].contains(&id) {
                assert_eq!(
                    normalized(&response),
                    normalized(&AdapterResponse::default()),
                    "{id}"
                );
                deltas.push(id.to_owned());
            } else {
                assert_eq!(normalized(&response), row["output"], "{id}");
            }
            drop(input_charge);
            assert_eq!(budget.snapshot().unwrap().used, 0);
            count += 1;
        }
        assert_eq!(
            json!(selected_ids),
            selection["corpora"][corpus_index]["selected_ids"],
            "exact selected IDs for corpus {corpus_index}"
        );
        counts.push(count);
    }
    assert_eq!(unique_ids.len(), 269);
    assert_eq!(selection["unique_count"], json!(269));
    eprintln!("raw source counts={counts:?} approved version deltas={deltas:?}");
    assert_eq!(counts, vec![45, 8, 24, 16, 10, 1, 165]);
    assert_eq!(
        deltas,
        vec!["arxiv.grammar.declaration_2", "arxiv.grammar.declaration_3"]
    );
    pool.shutdown().await.unwrap();
}
#[tokio::test]
async fn pre_http_admission_uses_existing_operation_and_shutdown_scope() {
    let pool = pool();
    let budget = Budget::new(512 * 1024 * 1024).unwrap();
    assert!(matches!(
        pool.begin_raw(budget.clone(), Instant::now() - Duration::from_secs(1)),
        Err(RawAdmissionError::Deadline)
    ));
    let op = pool
        .begin_raw(budget.clone(), Instant::now() + Duration::from_secs(10))
        .unwrap();
    assert_eq!(pool.available_operations(), 0);
    assert_eq!(pool.available_slots(), 1);
    assert!(matches!(
        pool.begin_raw(budget.clone(), Instant::now() + Duration::from_secs(10)),
        Err(RawAdmissionError::Saturated)
    ));
    drop(op);
    assert_eq!(pool.available_operations(), 1);
    assert_eq!(budget.snapshot().unwrap().used, 0);
    pool.shutdown().await.unwrap();
    assert!(matches!(
        pool.begin_raw(budget, Instant::now() + Duration::from_secs(10)),
        Err(RawAdmissionError::Shutdown)
    ));
}

#[cfg(unix)]
mod lifecycle {
    use super::*;
    use std::{
        fs,
        io::Write,
        os::unix::fs::{DirBuilderExt, OpenOptionsExt},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };
    static ID: AtomicU64 = AtomicU64::new(0);
    struct Fixture {
        directory: PathBuf,
        path: PathBuf,
        pid: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let directory = (0..64)
                .find_map(|_| {
                    let path = std::env::temp_dir().join(format!(
                        "a06-raw-atom-{}-{stamp}-{}",
                        std::process::id(),
                        ID.fetch_add(1, Ordering::Relaxed)
                    ));
                    match fs::DirBuilder::new().mode(0o700).create(&path) {
                        Ok(()) => Some(path),
                        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                        Err(error) => panic!("private fixture: {error}"),
                    }
                })
                .expect("private fixture collision bound");
            let path = directory.join("helper");
            let pid = directory.join("pid");
            let quote = |value: &std::path::Path| {
                format!("'{}'", value.to_str().unwrap().replace('\'', "'\"'\"'"))
            };
            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o700)
                .open(&path)
                .unwrap();
            writeln!(
                file,
                "#!/bin/sh\nprintf '%s\\n' \"$$\" > {}\nexec {}",
                quote(&pid),
                quote(std::path::Path::new(env!("CARGO_BIN_EXE_ssx-atom-parser")))
            )
            .unwrap();
            Self {
                directory,
                path,
                pid,
            }
        }
        fn pool(&self) -> AtomPool {
            AtomPool::new(self.path.clone(), NonZeroUsize::new(1).unwrap()).unwrap()
        }
        async fn pid(&self) -> String {
            let until = std::time::Instant::now() + Duration::from_secs(3);
            loop {
                if let Ok(pid) = fs::read_to_string(&self.pid)
                    && !pid.trim().is_empty()
                {
                    return pid.trim().to_owned();
                }
                assert!(
                    std::time::Instant::now() < until,
                    "actual helper PID handshake timed out"
                );
                tokio::task::yield_now().await;
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
            let _ = fs::remove_file(&self.pid);
            let _ = fs::remove_dir(&self.directory);
        }
    }
    fn gone(pid: &str) -> bool {
        !std::process::Command::new("/bin/kill")
            .args(["-0", pid])
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success()
    }
    async fn drained(pool: &AtomPool) {
        let until = std::time::Instant::now() + Duration::from_secs(3);
        while pool.available_operations() != 1 || pool.available_slots() != 1 {
            assert!(
                std::time::Instant::now() < until,
                "actual helper disposal timed out"
            );
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test]
    async fn scarce_budget_refuses_before_helper_spawn_and_preserves_operation_cleanup() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let budget = Budget::new(1).unwrap();
        let op = pool
            .begin_raw(budget.clone(), Instant::now() + Duration::from_secs(10))
            .unwrap();
        let response = op.parse(std::iter::empty(), b"<x/>").await;
        assert_eq!(response.status, ssx_domain::EngineStatus::Error);
        assert!(response.error_message.unwrap().contains("resource limit"));
        assert!(!fixture.pid.exists());
        assert_eq!(budget.snapshot().unwrap().used, 0);
        assert_eq!(pool.available_operations(), 1);
        assert_eq!(pool.available_slots(), 1);
        pool.shutdown().await.unwrap();
    }
    #[tokio::test(flavor = "current_thread")]
    async fn actual_raw_helper_cancellation_reaps_pid_before_releasing_child_allowance() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let budget = Budget::new(512 * 1024 * 1024).unwrap();
        let body = format!("<x>{}</x>", " ".repeat(24 * 1024 * 1024)).into_bytes();
        let input_charge = budget.reserve(body.capacity()).unwrap();
        let op = pool
            .begin_raw(budget.clone(), Instant::now() + Duration::from_secs(30))
            .unwrap();
        let job = tokio::spawn(async move { op.parse(std::iter::empty(), &body).await });
        let pid = fixture.pid().await;
        assert!(!job.is_finished());
        assert_eq!(pool.available_operations(), 0);
        assert!(
            budget.snapshot().unwrap().used
                >= input_charge.bytes()
                    + ssx_infra::adapters::scholarly_frame::CANDIDATE_CHILD_BYTES
        );
        job.abort();
        assert!(job.await.unwrap_err().is_cancelled());
        drained(&pool).await;
        assert!(gone(&pid));
        assert_eq!(budget.snapshot().unwrap().used, input_charge.bytes());
        drop(input_charge);
        assert_eq!(budget.snapshot().unwrap().used, 0);
        pool.shutdown().await.unwrap();
    }
    #[tokio::test(flavor = "current_thread")]
    async fn actual_parent_projection_cancellation_releases_rows_after_native_reap() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let budget = Budget::new(512 * 1024 * 1024).unwrap();
        let body = format!(
            "<feed xmlns='http://www.w3.org/2005/Atom'><entry><summary>{}</summary></entry></feed>",
            "a".repeat(8 * 1024 * 1024)
        )
        .into_bytes();
        let input_charge = budget.reserve(body.capacity()).unwrap();
        let op = pool
            .begin_raw(budget.clone(), Instant::now() + Duration::from_secs(30))
            .unwrap();
        let job = tokio::spawn(async move { op.parse(std::iter::empty(), &body).await });
        let pid = fixture.pid().await;
        tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                assert!(
                    !job.is_finished(),
                    "projection completed before observation"
                );
                let used = budget.snapshot().unwrap().used;
                // Reply's full64MiB reservation is gone; entry reservation remains.
                // With worker slot restored and operation stillheld, this is the
                // actual post-decode projection phase, not child or wire decoding.
                if pool.available_slots() == 1
                    && pool.available_operations() == 0
                    && used > input_charge.bytes() + 64 * 1024 * 1024
                    && used < input_charge.bytes() + 128 * 1024 * 1024
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(gone(&pid));
        job.abort();
        assert!(job.await.unwrap_err().is_cancelled());
        drained(&pool).await;
        assert_eq!(budget.snapshot().unwrap().used, input_charge.bytes());
        drop(input_charge);
        assert_eq!(budget.snapshot().unwrap().used, 0);
        pool.shutdown().await.unwrap();
    }
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn actual_pid_parent_tokio_deadline_reaps_and_restores_mirrored_allowance() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let budget = Budget::new(512 * 1024 * 1024).unwrap();
        let body = format!("<x>{}</x>", " ".repeat(24 * 1024 * 1024)).into_bytes();
        let input_charge = budget.reserve(body.capacity()).unwrap();
        let op = pool
            .begin_raw(budget.clone(), Instant::now() + Duration::from_secs(3600))
            .unwrap();
        let job = tokio::spawn(async move { op.parse(std::iter::empty(), &body).await });
        let pid = fixture.pid().await;
        assert!(!job.is_finished());
        assert_eq!(pool.available_operations(), 0);
        // Only the parent's Tokio timer is advanced. Frame std::Instant remains
        // in the future and child checkpoints remain supervisor-owned. This is
        // not evidence of child CPU or std::Instant deadline enforcement.
        tokio::time::advance(Duration::from_secs(3601)).await;
        let response = job.await.unwrap();
        assert_eq!(response.status, ssx_domain::EngineStatus::Timeout);
        drained(&pool).await;
        assert!(gone(&pid));
        assert_eq!(budget.snapshot().unwrap().used, input_charge.bytes());
        drop(input_charge);
        assert_eq!(budget.snapshot().unwrap().used, 0);
        pool.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn actual_produced_large_abstract_survives_shared_payload_boundary() {
    let pool = pool();
    let budget = Budget::new(512 * 1024 * 1024).unwrap();
    let abstract_text = "a".repeat(5 * 1024 * 1024);
    let body=format!("<feed xmlns='http://www.w3.org/2005/Atom'><entry><id>https://arxiv.org/abs/1</id><title>Big</title><summary>{abstract_text}</summary></entry></feed>").into_bytes();
    let input_charge = budget.reserve(body.capacity()).unwrap();
    let response = pool
        .begin_raw(budget.clone(), Instant::now() + Duration::from_secs(30))
        .unwrap()
        .parse(std::iter::empty(), &body)
        .await;
    assert_eq!(response.status, ssx_domain::EngineStatus::Ok);
    assert_eq!(response.results.len(), 1);
    assert_eq!(
        response.results[0].payload.as_ref().unwrap()["data"]["abstract"]
            .as_str()
            .unwrap(),
        abstract_text
    );
    assert_eq!(response.results[0].content, format!("{}…", "a".repeat(300)));
    eprintln!(
        "raw large result requested parent operation peak={} input={} limit={}; retained public response lifetime remains G02",
        budget.snapshot().unwrap().peak,
        input_charge.bytes(),
        budget.snapshot().unwrap().limit
    );
    assert_eq!(budget.snapshot().unwrap().used, input_charge.bytes());
    drop(input_charge);
    pool.shutdown().await.unwrap();
    assert_eq!(budget.snapshot().unwrap().used, 0);
}
