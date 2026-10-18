//! Real process + real disposable Valkey + real npm HTTP adapter. Never providers.
#![cfg(unix)]
use ssx_application::snapshot::{SnapshotLookup, SnapshotService, SnapshotTtl};
use ssx_domain::{
    FiniteF64, JobId, SnapshotId, TenantId,
    research::{ResearchJob, ResearchQuery},
};
use ssx_infra::{
    composition::Resolved,
    search::{SearchClock, SystemSearchClock},
    valkey::{
        Valkey,
        research::{JobRead, ResearchStore},
        snapshot::{RandomSnapshotIds, SystemSnapshotClock, ValkeySnapshots},
    },
};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
struct OwnedChild(Child, Option<std::thread::JoinHandle<String>>);
impl OwnedChild {
    fn new(mut child: Child) -> Self {
        let reader = child.stderr.take().map(|mut stderr| {
            std::thread::spawn(move || {
                let mut text = String::new();
                stderr
                    .read_to_string(&mut text)
                    .expect("owned child stderr drain");
                text
            })
        });
        Self(child, reader)
    }
    fn logs(&mut self) -> String {
        self.1
            .take()
            .map(|reader| reader.join().expect("stderr reader join"))
            .unwrap_or_default()
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
        let _ = self.logs();
    }
}
struct Private {
    path: PathBuf,
}
impl Private {
    fn new() -> Self {
        let mut id = [0; 16];
        aws_lc_rs::rand::fill(&mut id).unwrap();
        let suffix: String = id[..4].iter().map(|b| format!("{b:02x}")).collect();
        let path = std::env::var_os("SLOPSEARX_TEST_WORKDIR")
            .map(PathBuf::from)
            .expect("qualification must supply private tank workdir")
            .join(suffix);
        fs::create_dir(&path).unwrap();
        Self { path }
    }
}
impl Drop for Private {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
fn profile(dir: &Private, base: &str) -> PathBuf {
    let registry = ssx_registry::Registry::builtins().unwrap();
    let mut engines = serde_json::Map::new();
    for d in registry.descriptors() {
        engines.insert(d.name.clone(), serde_json::json!({"enabled":false}));
    }
    engines.insert(
        "npm".into(),
        serde_json::json!({"enabled":true,"base_url":base,"timeout_ms":2000,"max_results":10}),
    );
    let path = dir.path.join("config.json");
    fs::write(&path,serde_json::to_vec(&serde_json::json!({"engines":engines,"mcp":{"enabled_tools":{"research":true},"job_poll_interval_seconds":0.02,"job_lease_ttl_seconds":2}})).unwrap()).unwrap();
    path
}
fn worker(config: &std::path::Path, url: &str, check: bool) -> Command {
    let mut cmd = Command::new(
        std::env::var_os("SLOPSEARX_TEST_WORKER_BIN")
            .expect("actual archived worker binary required"),
    );
    cmd.env_clear()
        .env("VALKEY_URL", url)
        .arg("--config")
        .arg(config);
    if check {
        cmd.arg("--check-config");
    }
    cmd.arg("research")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    cmd
}
async fn wait(child: &mut OwnedChild) -> std::process::ExitStatus {
    let until = Instant::now() + Duration::from_secs(8);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            return status;
        }
        if Instant::now() >= until {
            let _ = child.0.kill();
            let status = child.0.wait().unwrap();
            panic!("process exceeded finite fixture bound, reaped {status}")
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
#[test]
fn composition_preflight_refuses_enabled_unmapped_adapter_without_store_io() {
    let dir = Private::new();
    let config = profile(&dir, "http://127.0.0.1:1");
    let mut v: serde_json::Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    v["engines"]["google"]["enabled"] = serde_json::Value::Bool(true);
    fs::write(&config, serde_json::to_vec(&v).unwrap()).unwrap();
    let result = Resolved::resolve(
        Some(&config),
        &BTreeMap::from([("VALKEY_URL".into(), "redis://127.0.0.1:1".into())]),
    );
    assert!(matches!(result,Err(e) if e.stage=="unavailable_adapter" && e.field=="google"));
}
#[tokio::test]
async fn composition_process_preflight_does_not_claim_readiness_or_require_store() {
    let dir = Private::new();
    let config = profile(&dir, "http://127.0.0.1:1");
    let mut child = OwnedChild::new(
        worker(&config, "redis://127.0.0.1:1", true)
            .spawn()
            .unwrap(),
    );
    let status = wait(&mut child).await;
    let stderr = child.logs();
    drop(child);
    assert!(status.success(), "{stderr}");
    assert!(stderr.contains("worker_configuration_valid"));
    assert!(!stderr.contains("worker_ready"));
}
#[tokio::test]
async fn composition_process_unreachable_store_fails_without_ready() {
    let dir = Private::new();
    let config = profile(&dir, "http://127.0.0.1:1");
    let mut child = OwnedChild::new(
        worker(&config, "redis://127.0.0.1:1", false)
            .spawn()
            .unwrap(),
    );
    let status = wait(&mut child).await;
    let stderr = child.logs();
    drop(child);
    assert!(!status.success());
    assert!(stderr.contains("store:ping"), "{stderr}");
    assert!(!stderr.contains("worker_ready"));
}
#[tokio::test]
async fn composition_process_real_npm_worker_persists_tenant_snapshot_and_joins() {
    let dir = Private::new();
    let socket = dir.path.join("valkey.sock");
    let url = format!("unix://{}", socket.display());
    let mut valkey = OwnedChild::new(
        Command::new(
            std::env::var_os("SLOPSEARX_TEST_VALKEY_SERVER").expect("pinned Valkey required"),
        )
        .args([
            "--port",
            "0",
            "--save",
            "",
            "--appendonly",
            "no",
            "--daemonize",
            "no",
            "--unixsocketperm",
            "700",
            "--unixsocket",
        ])
        .arg(&socket)
        .arg("--dir")
        .arg(&dir.path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap(),
    );
    let store = Arc::new(Valkey::new(&url, Duration::from_millis(300)).unwrap());
    let until = Instant::now() + Duration::from_secs(4);
    while store.ping().await.is_err() {
        assert!(valkey.0.try_wait().unwrap().is_none());
        assert!(Instant::now() < until, "private Valkey startup");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/search", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let accepted = tokio::time::timeout(Duration::from_secs(6), listener.accept()).await;
        let Ok(Ok((mut socket, _))) = accepted else {
            return String::new();
        };
        let mut request = Vec::new();
        let mut part = [0; 1024];
        loop {
            let read = tokio::time::timeout(Duration::from_secs(2), socket.read(&mut part))
                .await
                .unwrap()
                .unwrap();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&part[..read]);
            if request.windows(4).any(|v| v == b"\r\n\r\n") {
                break;
            }
            if request.len() > 16384 {
                break;
            }
        }
        let body = r#"{"objects":[{"package":{"name":"composition-proof","version":"1.0.0","description":"native loopback","links":{"npm":"https://www.npmjs.com/package/composition-proof"}}}],"total":1}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        socket.write_all(response.as_bytes()).await.unwrap();
        socket.shutdown().await.unwrap();
        String::from_utf8(request).unwrap()
    });
    let config = profile(&dir, &base);
    let tenant = TenantId::new("tenant:proof").unwrap();
    let jobs = ResearchStore::new(store.clone(), tenant.clone());
    let now = SystemSearchClock.capture().0.seconds();
    let job = ResearchJob {
        job_id: "job-composition-proof".into(),
        tenant: tenant.as_str().into(),
        question: "composition-proof".into(),
        created_at: FiniteF64::new(now).unwrap(),
        deadline: FiniteF64::new(now + 20.0).unwrap(),
        queries: vec![ResearchQuery {
            query: "composition-proof".into(),
            intent: "web".into(),
            engines: vec!["npm".into()],
            requires_intent_grant: false,
            ..Default::default()
        }],
        ..Default::default()
    };
    jobs.create(&job).await.unwrap();
    let mut process = OwnedChild::new(worker(&config, &url, false).spawn().unwrap());
    let id = JobId::new(&job.job_id).unwrap();
    let until = Instant::now() + Duration::from_secs(7);
    let completed = loop {
        if let JobRead::Found(job) = jobs.load(&id).await
            && job.state == "succeeded"
        {
            break Some(job);
        }
        if Instant::now() >= until || process.0.try_wait().unwrap().is_some() {
            break None;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    let _ = Command::new("/bin/kill")
        .arg("-TERM")
        .arg(process.0.id().to_string())
        .status();
    let status = wait(&mut process).await;
    let stderr = process.logs();
    drop(process);
    let request = server.await.unwrap();
    // A new process must reuse the canonical cache, not its random lease owner.
    // The one-request loopback server/listener has already exited and cannot serve it.
    let mut again = job.clone();
    again.job_id = "job-composition-proof-second".into();
    jobs.create(&again).await.unwrap();
    let mut second = OwnedChild::new(worker(&config, &url, false).spawn().unwrap());
    let second_id = JobId::new(&again.job_id).unwrap();
    let until = Instant::now() + Duration::from_secs(7);
    let reused = loop {
        if let JobRead::Found(job) = jobs.load(&second_id).await
            && job.state == "succeeded"
        {
            break job.queries[0].result_count == 1;
        }
        if Instant::now() >= until || second.0.try_wait().unwrap().is_some() {
            break false;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    let _ = Command::new("/bin/kill")
        .arg("-TERM")
        .arg(second.0.id().to_string())
        .status();
    let second_status = wait(&mut second).await;
    let second_stderr = second.logs();
    drop(second);
    // A changed effective endpoint must invalidate the same retained query cache.
    let changed_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let changed_base = format!("http://{}/search", changed_listener.local_addr().unwrap());
    let changed_server = tokio::spawn(async move {
        let Ok(Ok((mut socket, _))) =
            tokio::time::timeout(Duration::from_secs(6), changed_listener.accept()).await
        else {
            return false;
        };
        let mut bytes = [0; 4096];
        let entered = tokio::time::timeout(Duration::from_secs(2), socket.read(&mut bytes))
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or(0)
            > 0;
        let body = r#"{"objects":[{"package":{"name":"changed-one","version":"2.0.0"}},{"package":{"name":"changed-two","version":"2.0.0"}}],"total":2}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
        entered
    });
    let changed_config = profile(&dir, &changed_base);
    let mut changed = job.clone();
    changed.job_id = "job-composition-proof-changed".into();
    changed.deadline = FiniteF64::new(SystemSearchClock.capture().0.seconds() + 20.0).unwrap();
    jobs.create(&changed).await.unwrap();
    let mut third = OwnedChild::new(worker(&changed_config, &url, false).spawn().unwrap());
    let changed_id = JobId::new(&changed.job_id).unwrap();
    let until = Instant::now() + Duration::from_secs(7);
    let invalidated = loop {
        if let JobRead::Found(job) = jobs.load(&changed_id).await
            && job.state == "succeeded"
        {
            break job.queries[0].result_count == 2;
        }
        if Instant::now() >= until || third.0.try_wait().unwrap().is_some() {
            break false;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    let _ = Command::new("/bin/kill")
        .arg("-TERM")
        .arg(third.0.id().to_string())
        .status();
    let third_status = wait(&mut third).await;
    let third_stderr = third.logs();
    drop(third);
    let changed_io = changed_server.await.unwrap();
    // Read through a fresh independent Valkey client only after actual worker exit.
    let fresh = Arc::new(Valkey::new(&url, Duration::from_secs(1)).unwrap());
    let snapshots = SnapshotService::new(
        Arc::new(ValkeySnapshots::new(fresh.clone())),
        Arc::new(SystemSnapshotClock),
        Arc::new(RandomSnapshotIds),
        tenant,
        SnapshotTtl::new(60).unwrap(),
    );
    let retained = if let Some(ref job) = completed {
        if let Some(ref cursor) = job.queries[0].cursor {
            match snapshots.read(&SnapshotId::new(cursor).unwrap()).await {
                SnapshotLookup::Found(snapshot) => {
                    snapshot.query() == "composition-proof"
                        && snapshot.tenant().as_str() == "tenant:proof"
                        && snapshot.scope().selected_engines == vec!["npm".to_owned()]
                        && snapshot.results().len() == 1
                        && snapshot.results()[0].title == "composition-proof@1.0.0"
                        && snapshot.results()[0].url
                            == "https://www.npmjs.com/package/composition-proof"
                        && snapshot.results()[0].engine == "npm"
                }
                _ => false,
            }
        } else {
            false
        }
    } else {
        false
    };
    fresh.close().await;
    store.close().await;
    let _ = valkey.0.kill();
    let _ = valkey.0.wait();
    drop(valkey);
    assert!(status.success(), "{stderr}");
    assert!(stderr.contains("worker_ready"));
    assert!(stderr.contains("worker_stopped"));
    assert!(
        stderr.find("worker_tasks_entered").unwrap() < stderr.find("worker_ready").unwrap(),
        "readiness preceded actual task-entry acknowledgements: {stderr}"
    );
    assert!(
        completed.is_some(),
        "worker did not complete persisted job: {stderr}"
    );
    assert!(retained, "snapshot absent after worker exit");
    assert!(second_status.success(), "{second_stderr}");
    assert!(
        reused,
        "new worker failed to reuse persisted canonical cache: {second_stderr}"
    );
    assert!(third_status.success(), "{third_stderr}");
    assert!(
        changed_io && invalidated,
        "changed endpoint did not invalidate retained cache: {third_stderr}"
    );
    assert!(request.starts_with("GET /search?"), "{request}");
    assert!(request.contains("composition-proof"));
}

#[test]
fn composition_ordered_os_aliases_preserve_last_entry_before_coercion() {
    let dir = Private::new();
    let config = profile(&dir, "http://127.0.0.1:1");
    let mut entries = vec![
        ("VALKEY_URL".into(), "redis://127.0.0.1:1".into()),
        (
            "ENGINE_NPM_timeout_ms".into(),
            "invalid-losing-alias".into(),
        ),
        ("ENGINE_NPM_TIMEOUT_MS".into(), "1500".into()),
    ];
    assert!(Resolved::resolve_entries(Some(&config), &entries).is_ok());
    entries.swap(1, 2);
    assert!(Resolved::resolve_entries(Some(&config), &entries).is_err());
}
#[test]
fn composition_community_uses_exact_resolver_scalar_contract() {
    let dir = Private::new();
    let config = profile(&dir, "http://127.0.0.1:1");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    value["engines"]["npm"]["enabled"] = false.into();
    value["engines"]["github"] =
        serde_json::json!({"enabled":true,"timeout_ms":1.5,"max_results":10});
    fs::write(&config, serde_json::to_vec(&value).unwrap()).unwrap();
    let env = BTreeMap::from([("VALKEY_URL".into(), "redis://127.0.0.1:1".into())]);
    assert!(
        Resolved::resolve(Some(&config), &env).is_ok(),
        "fractional timeout must survive exact resolver"
    );
    value["engines"]["github"]["max_results"] = serde_json::json!(10.0);
    fs::write(&config, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(
        matches!(Resolved::resolve(Some(&config),&env),Err(e) if e.stage=="config" && e.field=="community_settings")
    );
}
#[test]
fn composition_sentry_mode_is_not_silently_ignored_or_logged() {
    let dir = Private::new();
    let config = profile(&dir, "http://127.0.0.1:1");
    let env = BTreeMap::from([
        ("VALKEY_URL".into(), "redis://127.0.0.1:1".into()),
        (
            "SENTRY_DSN".into(),
            "https://fixture-secret@127.0.0.1/1".into(),
        ),
    ]);
    let error = match Resolved::resolve(Some(&config), &env) {
        Ok(_) => panic!("unsupported explicit Sentry mode accepted"),
        Err(e) => e,
    };
    assert_eq!(error.stage, "unsupported");
    assert_eq!(error.field, "SENTRY_DSN");
    assert!(!error.to_string().contains("fixture-secret"));
}

async fn held_startup_signal(signal: &str) {
    let dir = Private::new();
    let config = profile(&dir, "http://127.0.0.1:1");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("redis://{}", listener.local_addr().unwrap());
    let mut child = OwnedChild::new(worker(&config, &url, false).spawn().unwrap());
    // Witness an actual Redis connection and its handshake bytes before sending
    // the signal. No synthetic startup flag or relaxed operation timer is used.
    let (mut socket, _) = tokio::time::timeout(Duration::from_secs(4), listener.accept())
        .await
        .unwrap()
        .unwrap();
    let mut bytes = [0; 4096];
    let entered = tokio::time::timeout(Duration::from_secs(2), socket.read(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    let signalled = Command::new("/bin/kill")
        .arg(signal)
        .arg(child.0.id().to_string())
        .status()
        .unwrap();
    let eof = tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            match socket.read(&mut bytes).await {
                Ok(0) => break true,
                Ok(_) => {}
                Err(_) => break false,
            }
        }
    })
    .await
    .unwrap_or(false);
    let status = wait(&mut child).await;
    let stderr = child.logs();
    drop(child);
    drop(socket);
    drop(listener);
    assert!(entered > 0, "no actual startup handshake bytes");
    assert!(signalled.success());
    assert!(eof, "startup connection did not close after stop");
    assert!(status.success(), "{stderr}");
    assert!(stderr.contains("worker_startup_stopped"), "{stderr}");
    assert!(stderr.contains("worker_stopped"));
    assert!(!stderr.contains("worker_ready"));
    assert!(!stderr.contains("worker_tasks_entered"));
}
#[tokio::test]
async fn composition_sigint_during_actual_startup_drains_without_ready() {
    held_startup_signal("-INT").await;
}
#[tokio::test]
async fn composition_sigterm_during_actual_startup_drains_without_ready() {
    held_startup_signal("-TERM").await;
}
#[tokio::test]
async fn composition_pre_requested_stop_opens_no_runtime() {
    let dir = Private::new();
    let config = profile(&dir, "http://127.0.0.1:1");
    let env = BTreeMap::from([("VALKEY_URL".into(), "redis://127.0.0.1:1".into())]);
    let resolved = Resolved::resolve(Some(&config), &env).unwrap();
    let (_owner, stop) = tokio::sync::watch::channel(true);
    assert!(matches!(resolved.open_stoppable(stop).await,Err(e) if e.stage=="stopped"));
}
