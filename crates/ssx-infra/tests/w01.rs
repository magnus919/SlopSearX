//! W01 authoritative contracts run only against a private disposable Valkey process.
use ssx_domain::{AttemptId, JobId, LeaseToken, TenantId, WallClockTimestamp, research::*};
use ssx_infra::valkey::{research::*, *};
use std::{
    fs,
    io::Read as _,
    num::NonZeroU64,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Barrier},
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
        let dir = std::env::temp_dir().join(format!("ssx-w01-{id}"));
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
fn now(n: f64) -> WallClockTimestamp {
    WallClockTimestamp::new(n).unwrap()
}
fn id() -> JobId {
    JobId::new("job-test").unwrap()
}
fn aid() -> AttemptId {
    AttemptId::new("recovered-attempt").unwrap()
}
fn job() -> ResearchJob {
    ResearchJob::from_payload(ssx_json::json!({"job_id":"job-test","tenant":"org:team","question":"q","strategy":"broad","created_at":1,"deadline":1000,"idempotency_key":"request-1","queries":[{"index":0,"query":"q","intent":"general","engines":["fake"]}]})).unwrap()
}
fn scoped(server: &Server) -> ResearchStore {
    ResearchStore::new(Arc::new(server.store()), TenantId::new("org:team").unwrap())
}
async fn found(s: &ResearchStore) -> ResearchJob {
    match s.load(&id()).await {
        JobRead::Found(v) => *v,
        other => panic!("expected authoritative job: {other:?}"),
    }
}
async fn claim(s: &ResearchStore, token: &'static str, seconds: u64) -> Claim {
    s.claim(
        &id(),
        "worker",
        ttl(seconds),
        &mut Tokens(token),
        now(10.0),
        aid,
    )
    .await
    .unwrap()
    .unwrap()
}
fn initialize(j: &mut ResearchJob) -> Result<(), ssx_domain::DomainError> {
    j.initialize_budget(ResearchCeilings {
        queries: 2,
        engines_per_query: 1,
        results: 3,
    })
}
#[tokio::test]
async fn tenants_identity_idempotency_and_malformed_records() {
    let server = Server::start();
    let a = scoped(&server);
    let b = a.for_tenant(TenantId::new("other").unwrap());
    let (created, yes) = a.create(&job()).await.unwrap();
    assert!(yes);
    assert_eq!(created.tenant, "org:team");
    let mut replay = job();
    replay.job_id = "different".into();
    assert!(!a.create(&replay).await.unwrap().1);
    assert!(matches!(b.load(&id()).await, JobRead::Missing));
    assert!(b.create(&job()).await.is_err());
    let mut collision = job();
    collision.idempotency_key = Some("request-2".into());
    assert_eq!(a.create(&collision).await, Err(Error::Conflict));
    let owner = claim(&a, "tenant-owner", 10).await;
    let forged_scope = a.for_tenant(TenantId::new("org").unwrap());
    let forged_id = JobId::new("team:job-test").unwrap();
    assert!(
        forged_scope
            .renew(&forged_id, &owner.token, ttl(10))
            .await
            .is_err()
    );
    assert!(
        forged_scope
            .release(&forged_id, &owner.token)
            .await
            .is_err()
    );
    assert_eq!(
        a.renew(&id(), &owner.token, ttl(10)).await.unwrap(),
        Ownership::Applied
    );
    let mut malformed = job().to_payload().unwrap();
    malformed["job_id"] = "wrong".into();
    write(
        &server,
        "mcp:job:org:team:job-test",
        &ssx_json::to_string(&malformed).unwrap(),
    );
    assert!(matches!(a.load(&id()).await, JobRead::Malformed));
    malformed["job_id"] = "job-test".into();
    malformed["tenant"] = "other".into();
    write(
        &server,
        "mcp:job:org:team:job-test",
        &ssx_json::to_string(&malformed).unwrap(),
    );
    assert!(matches!(a.load(&id()).await, JobRead::Malformed));
    write(&server, "mcp:job:org:team:job-test", "[]");
    assert!(matches!(a.load(&id()).await, JobRead::Malformed));
    assert!(matches!(
        a.load(&JobId::new("team:job-test").unwrap()).await,
        JobRead::Malformed
    ));
}
#[tokio::test]
async fn reservation_crash_reclaim_and_completed_evidence() {
    let server = Server::start();
    let a = scoped(&server);
    let b = scoped(&server);
    a.create(&job()).await.unwrap();
    let first = claim(&a, "first", 1).await;
    a.mutate(&id(), &first.token, |j| {
        initialize(j)?;
        j.reserve_attempt(0, AttemptId::new("a")?, now(11.0))?;
        Ok(())
    })
    .await
    .unwrap();
    // No dispatch/settlement occurs: the durable reservation must survive expiry.
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let second = claim(&b, "second", 10).await;
    assert_eq!(second.job.budget_used["attempts"], 1);
    assert_eq!(second.job.queries[0].attempts[0].state, "interrupted");
    assert!(matches!(
        a.mutate(&id(), &first.token, |_| Ok(())).await,
        Err(Error::OwnershipLost)
    ));
    assert_eq!(
        a.renew(&id(), &first.token, ttl(5)).await.unwrap(),
        Ownership::Lost
    );
    assert_eq!(
        a.release(&id(), &first.token).await.unwrap(),
        Ownership::Lost
    );
    let done = b
        .mutate(&id(), &second.token, |j| {
            assert!(j.reserve_attempt(0, AttemptId::new("b")?, now(12.0))?);
            j.queries[0].state = "done".into();
            j.queries[0].cursor = Some("snap".into());
            j.queries[0].result_count = 2;
            let lead = LeadId::new(format!("lead-v1-{}", "a".repeat(64)))?;
            j.finish_attempt(
                0,
                &AttemptId::new("b")?,
                Some(&[lead.clone(), lead]),
                now(13.0),
            )?;
            j.state = "partial".into();
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(done.budget_used["results"], 2);
    assert_eq!(done.seen_lead_ids.len(), 1);
    assert!(
        b.mutate(&id(), &second.token, |j| {
            j.queries[0].attempts[1].cursor = Some("rewrite".into());
            Ok(())
        })
        .await
        .is_err()
    );
    assert!(
        b.mutate(&id(), &second.token, |j| {
            j.budget_used.insert("attempts".into(), 0);
            Ok(())
        })
        .await
        .is_err()
    );
    let retained = found(&b).await;
    assert_eq!(retained, done);
    let rejected = b
        .mutate(&id(), &second.token, |j| {
            j.queries[0].state = "pending".into(); // Admit another attempt before testing its exhausted budget.
            assert!(!j.reserve_attempt(0, AttemptId::new("c")?, now(14.0))?);
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(
        rejected.stop_reason.as_deref(),
        Some("attempt_budget_exhausted")
    );
    assert_eq!(rejected.queries[0].attempts, done.queries[0].attempts);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn same_token_concurrent_writers_cannot_lose_history() {
    let server = Server::start();
    let s = scoped(&server);
    s.create(&job()).await.unwrap();
    let owner = claim(&s, "same", 10).await;
    s.mutate(&id(), &owner.token, initialize).await.unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let mut tasks = vec![];
    for name in ["left", "right"] {
        let s = scoped(&server);
        let token = owner.token.clone();
        let barrier = barrier.clone();
        let runtime = tokio::runtime::Handle::current();
        tasks.push(tokio::task::spawn_blocking(move || {
            runtime.block_on(async move {
                s.mutate(&id(), &token, |j| {
                    barrier.wait();
                    j.reserve_attempt(0, AttemptId::new(name)?, now(11.0))?;
                    Ok(())
                })
                .await
            })
        }));
    }
    let mut passed = 0;
    let mut conflicts = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(_) => passed += 1,
            Err(Error::Conflict) => conflicts += 1,
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!((passed, conflicts), (1, 1));
    let j = found(&s).await;
    assert_eq!(j.budget_used["attempts"], 1);
    assert_eq!(j.queries[0].attempts.len(), 1);
}
#[tokio::test]
async fn cancellation_and_absolute_expiry_survive_restart() {
    let mut server = Server::start();
    let s = scoped(&server);
    s.create(&job()).await.unwrap();
    let owner = claim(&s, "owner", 10).await;
    s.mutate(&id(), &owner.token, |j| {
        initialize(j)?;
        j.reserve_attempt(0, AttemptId::new("a")?, now(11.0))?;
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(
        s.request_cancel(&id(), now(12.0), &mut Tokens("cancel-live"), aid)
            .await
            .unwrap(),
        "running"
    );
    assert!(found(&s).await.cancel_requested);
    s.release(&id(), &owner.token).await.unwrap();
    assert_eq!(
        s.request_cancel(&id(), now(13.0), &mut Tokens("cancel"), aid)
            .await
            .unwrap(),
        "cancelled"
    );
    let cancelled = found(&s).await;
    assert_eq!(cancelled.budget_used["attempts"], 1);
    assert_eq!(cancelled.queries[0].attempts[0].state, "interrupted");
    assert_eq!(
        s.request_cancel(&id(), now(14.0), &mut Tokens("cancel-again"), aid)
            .await
            .unwrap(),
        "cancelled"
    );
    let mut exp = job();
    exp.job_id = "expires".into();
    exp.idempotency_key = None;
    exp.deadline = ssx_domain::FiniteF64::new(100.0).unwrap();
    s.create(&exp).await.unwrap();
    redis::cmd("SAVE")
        .query::<()>(&mut server.connection())
        .unwrap();
    server.stop();
    server.restart();
    let s = scoped(&server);
    let expires = JobId::new("expires").unwrap();
    assert!(
        s.claim(
            &expires,
            "worker",
            ttl(10),
            &mut Tokens("at-deadline"),
            now(100.0),
            aid
        )
        .await
        .unwrap()
        .is_none()
    );
    match s.load(&expires).await {
        JobRead::Found(v) => assert_eq!(v.state, "expired"),
        _ => panic!("missing expiry"),
    }
    assert!(
        s.claim(
            &expires,
            "worker",
            ttl(10),
            &mut Tokens("clock-back"),
            now(50.0),
            aid
        )
        .await
        .unwrap()
        .is_none()
    );
    assert_eq!(
        found(&s).await.queries[0].attempts,
        cancelled.queries[0].attempts
    );
}
#[tokio::test]
async fn ready_recent_repair_and_tenant_scoping() {
    let server = Server::start();
    let a = scoped(&server);
    a.create(&job()).await.unwrap();
    let b = a.for_tenant(TenantId::new("other").unwrap());
    let mut other = job();
    other.tenant = "other".into();
    other.job_id = "other-job".into();
    b.create(&other).await.unwrap();
    redis::cmd("DEL")
        .arg("mcp:ready:v1:tenant:org:team")
        .arg("mcp:recent:v1:org:team")
        .query::<()>(&mut server.connection())
        .unwrap();
    a.reconcile().await.unwrap();
    assert_eq!(a.list_recent(None, 20).await.unwrap().len(), 1);
    let claim = a
        .claim_next("scoped", ttl(10), &mut Tokens("scope"), now(10.0), aid)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claim.job.tenant, "org:team");
    let any = a
        .claim_next_any_tenant("any", ttl(10), &mut Tokens("any"), now(10.0), aid)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(any.job.tenant, "other");
    write(&server, "mcp:job:org:team:job-test", "{}");
    assert!(a.list_recent(None, 20).await.is_err());
}
#[tokio::test]
async fn unknown_mutation_completion_requires_authoritative_reconciliation() {
    let server = Server::start();
    let s = scoped(&server);
    s.create(&job()).await.unwrap();
    let owner = claim(&s, "owner", 10).await;
    s.mutate(&id(), &owner.token, initialize).await.unwrap();
    let short = ResearchStore::new(
        Arc::new(Valkey::new(&server.url, Duration::from_millis(30)).unwrap()),
        TenantId::new("org:team").unwrap(),
    );
    let mut paused = false;
    let result = short
        .mutate(&id(), &owner.token, |j| {
            j.reserve_attempt(0, AttemptId::new("unknown-dispatch")?, now(11.0))?;
            redis::cmd("CLIENT")
                .arg("PAUSE")
                .arg(150)
                .arg("WRITE")
                .query::<()>(&mut server.connection())
                .unwrap();
            paused = true;
            Ok(())
        })
        .await;
    assert!(paused);
    assert_eq!(result, Err(Error::Store(Failure::CompletionUnknown)));
    tokio::time::sleep(Duration::from_millis(250)).await;
    let authoritative = found(&s).await;
    assert_eq!(authoritative.budget_used["attempts"], 1);
    assert_eq!(
        authoritative.queries[0].attempts[0].attempt_id,
        "unknown-dispatch"
    );
    // Reconciliation observes committed work; no automatic second reservation.
    assert_eq!(authoritative.budget_used["engine_attempts"], 1);
}
#[tokio::test]
async fn prepared_mutation_reloads_terminal_evidence_and_preserves_guards() {
    let server = Server::start();
    let s = scoped(&server);
    s.create(&job()).await.unwrap();
    let owner = claim(&s, "worker", 10).await;
    assert!(
        s.claim_prepared(
            &id(),
            "retry",
            ttl(10),
            &mut Tokens("denied"),
            now(20.0),
            aid,
            false,
            |_| panic!("live owner excludes mutation")
        )
        .await
        .unwrap()
        .is_none()
    );
    s.mutate(&id(), &owner.token, |j| {
        initialize(j)?;
        j.reserve_attempt(0, AttemptId::new("historical")?, now(11.0))?;
        j.queries[0].state = "failed".into();
        j.queries[0].cursor = Some("old-evidence".into());
        j.finish_attempt(0, &AttemptId::new("historical")?, None, now(12.0))?;
        j.state = "partial".into();
        Ok(())
    })
    .await
    .unwrap();
    s.release(&id(), &owner.token).await.unwrap();
    let direct = s
        .claim_prepared(
            &id(),
            "retry",
            ttl(10),
            &mut Tokens("direct"),
            now(20.0),
            aid,
            false,
            |j| {
                j.reset_retryable_queries();
                Ok(())
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(direct.job.state, "running");
    assert_eq!(
        direct.job.queries[0].attempts[0].cursor.as_deref(),
        Some("old-evidence")
    );
    s.mutate(&id(), &direct.token, |j| {
        j.caller_completed = true;
        j.state = "partial".into();
        Ok(())
    })
    .await
    .unwrap();
    s.release(&id(), &direct.token).await.unwrap();
    let complete = s
        .claim_prepared(
            &id(),
            "metadata",
            ttl(10),
            &mut Tokens("metadata"),
            now(2000.0),
            aid,
            true,
            |j| {
                j.completion_rationale = Some("complete".into());
                Ok(())
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert!(complete.job.caller_completed);
    assert_eq!(
        complete.job.completion_rationale.as_deref(),
        Some("complete")
    );
    s.clear_ownership(&id(), &complete.token).await.unwrap();
    s.release(&id(), &complete.token).await.unwrap();
    let preserved = s
        .claim_prepared(
            &id(),
            "retry",
            ttl(10),
            &mut Tokens("no-resurrection"),
            now(2000.0),
            aid,
            false,
            |_| panic!("completed caller judgment retained"),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(preserved.job.state, "partial");
}

#[tokio::test]
async fn prepared_claim_returns_fenced_terminal_state_without_mutation() {
    let server = Server::start();
    let s = scoped(&server);

    // A regular worker claim treats terminal state as no work. A prepared claim
    // instead returns the authoritative terminal record under a fresh lease so
    // its caller can clear ownership/release without applying retry mutation.
    s.create(&job()).await.unwrap();
    assert_eq!(
        s.request_cancel(&id(), now(20.0), &mut Tokens("cancel"), aid)
            .await
            .unwrap(),
        "cancelled"
    );
    assert!(
        s.claim(
            &id(),
            "worker",
            ttl(10),
            &mut Tokens("worker"),
            now(21.0),
            aid
        )
        .await
        .unwrap()
        .is_none()
    );
    let cancelled = s
        .claim_prepared(
            &id(),
            "reader",
            ttl(10),
            &mut Tokens("prepared-cancelled"),
            now(21.0),
            aid,
            false,
            |_| panic!("terminal cancellation must not invoke prepared mutation"),
        )
        .await
        .unwrap()
        .expect("terminal state is returned under a lease");
    assert_eq!(cancelled.job.state, "cancelled");
    assert_eq!(cancelled.job.owner_id.as_deref(), Some("reader"));
    assert_eq!(
        cancelled.job.lease_token.as_deref(),
        Some(cancelled.token.as_str())
    );
    s.clear_ownership(&id(), &cancelled.token).await.unwrap();
    assert_eq!(
        s.release(&id(), &cancelled.token).await.unwrap(),
        Ownership::Applied
    );

    let mut expired_job = job();
    expired_job.job_id = "expired-prepared".into();
    expired_job.idempotency_key = None;
    let (expired_job, _) = s.create(&expired_job).await.unwrap();
    let expired_id = JobId::new("expired-prepared").unwrap();
    assert!(
        s.claim(
            &expired_id,
            "worker",
            ttl(10),
            &mut Tokens("expire"),
            now(1000.0),
            aid
        )
        .await
        .unwrap()
        .is_none()
    );
    let expired = s
        .claim_prepared(
            &expired_id,
            "reader",
            ttl(10),
            &mut Tokens("prepared-expired"),
            now(21.0),
            aid,
            false,
            |_| panic!("terminal expiry must not invoke prepared mutation"),
        )
        .await
        .unwrap()
        .expect("terminal state is returned under a lease");
    assert_eq!(expired.job.job_id, expired_job.job_id);
    assert_eq!(expired.job.state, "expired");
    assert_eq!(expired.job.owner_id.as_deref(), Some("reader"));
    assert_eq!(
        expired.job.lease_token.as_deref(),
        Some(expired.token.as_str())
    );
    s.clear_ownership(&expired_id, &expired.token)
        .await
        .unwrap();
    assert_eq!(
        s.release(&expired_id, &expired.token).await.unwrap(),
        Ownership::Applied
    );
}

#[tokio::test]
async fn authoritative_mutations_cannot_bypass_conservation_helpers() {
    let server = Server::start();
    let s = scoped(&server);
    s.create(&job()).await.unwrap();
    let owner = claim(&s, "worker", 10).await;
    assert!(
        s.mutate(&id(), &owner.token, |j| {
            j.queries[0].attempts.push(ResearchQueryAttempt {
                attempt_id: "before-accounting".into(),
                state: "running".into(),
                ..Default::default()
            });
            Ok(())
        })
        .await
        .is_err()
    );
    s.mutate(&id(), &owner.token, initialize).await.unwrap();
    assert!(
        s.mutate(&id(), &owner.token, |j| {
            j.queries[0].attempts.push(ResearchQueryAttempt {
                attempt_id: "uncharged".into(),
                state: "running".into(),
                ..Default::default()
            });
            Ok(())
        })
        .await
        .is_err()
    );
    assert!(
        s.mutate(&id(), &owner.token, |j| {
            j.budget_used.insert("attempts".into(), 100);
            Ok(())
        })
        .await
        .is_err()
    );
    s.mutate(&id(), &owner.token, |j| {
        j.reserve_attempt(0, AttemptId::new("reserved")?, now(11.0))?;
        Ok(())
    })
    .await
    .unwrap();
    assert!(
        s.mutate(&id(), &owner.token, |j| {
            let a = &mut j.queries[0].attempts[0];
            a.state = "done".into();
            a.admitted_result_ids.push("snap:0".into());
            Ok(())
        })
        .await
        .is_err()
    );
    let before = found(&s).await;
    let lowered = s
        .mutate(&id(), &owner.token, |j| {
            j.initialize_budget(ResearchCeilings {
                queries: 0,
                engines_per_query: 0,
                results: 0,
            })
        })
        .await
        .unwrap();
    assert_eq!(before.budget_used, lowered.budget_used);
    assert!(
        s.mutate(&id(), &owner.token, |j| {
            j.queries[0].attempts.push(ResearchQueryAttempt {
                attempt_id: "over-cap".into(),
                state: "running".into(),
                ..Default::default()
            });
            *j.budget_used.get_mut("attempts").unwrap() += 1;
            *j.budget_used.get_mut("engine_attempts").unwrap() += 1;
            Ok(())
        })
        .await
        .is_err()
    );
}
#[tokio::test]
async fn initialization_and_work_cannot_evade_scope_or_legacy_charges() {
    let server = Server::start();
    let s = scoped(&server);
    let mut initial = job();
    initial.queries[0].engines = vec!["one".into(), "two".into()];
    s.create(&initial).await.unwrap();
    let owner = claim(&s, "worker", 10).await;
    assert!(
        s.mutate(&id(), &owner.token, |j| {
            j.initialize_budget(ResearchCeilings {
                queries: 4,
                engines_per_query: 1,
                results: 3,
            })?;
            j.queries[0].attempts.push(ResearchQueryAttempt {
                attempt_id: "overscope".into(),
                state: "running".into(),
                ..Default::default()
            });
            j.budget_used.insert("attempts".into(), 1);
            j.budget_used.insert("engine_attempts".into(), 2);
            Ok(())
        })
        .await
        .is_err()
    );
    assert!(found(&s).await.budget_limits.is_empty());
    // A pre-ledger implicit completed attempt must not disappear into a newly
    // appended explicit attempt when initialization and work share one write.
    let mut old = job();
    old.job_id = "legacy-done".into();
    old.idempotency_key = None;
    old.state = "partial".into();
    old.queries[0].state = "done".into();
    old.queries[0].result_count = 2;
    s.create(&old).await.unwrap();
    let legacy = JobId::new("legacy-done").unwrap();
    let denied = s
        .claim_prepared(
            &legacy,
            "direct",
            ttl(10),
            &mut Tokens("legacy-direct"),
            now(20.0),
            aid,
            false,
            |j| {
                j.initialize_budget(ResearchCeilings {
                    queries: 4,
                    engines_per_query: 1,
                    results: 5,
                })?;
                j.queries[0].attempts.push(ResearchQueryAttempt {
                    attempt_id: "new".into(),
                    state: "running".into(),
                    ..Default::default()
                });
                j.budget_used.insert("attempts".into(), 1);
                j.budget_used.insert("engine_attempts".into(), 1);
                Ok(())
            },
        )
        .await;
    assert!(denied.is_err());
}
#[tokio::test]
async fn legacy_retry_materializes_evidence_before_reset_and_accounting() {
    let server = Server::start();
    let s = scoped(&server);
    let mut initial = job();
    initial.state = "partial".into();
    initial.queries[0].state = "done".into();
    initial.queries[0].cursor = Some("legacy-snapshot".into());
    initial.queries[0].query_id = Some("legacy-query".into());
    initial.queries[0].result_count = 0;
    s.create(&initial).await.unwrap();
    let prepared = s
        .claim_prepared(
            &id(),
            "retry",
            ttl(10),
            &mut Tokens("legacy"),
            now(20.0),
            aid,
            false,
            |j| {
                j.reset_retryable_queries();
                Ok(())
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(prepared.job.queries[0].state, "pending");
    assert_eq!(prepared.job.queries[0].attempts.len(), 1);
    assert_eq!(
        prepared.job.queries[0].attempts[0].cursor.as_deref(),
        Some("legacy-snapshot")
    );
    let reserved = s
        .mutate(&id(), &prepared.token, |j| {
            initialize(j)?;
            assert!(j.reserve_attempt(0, AttemptId::new("retry-attempt")?, now(21.0))?);
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(reserved.budget_used["attempts"], 2);
    assert_eq!(reserved.budget_used["engine_attempts"], 2);
    assert_eq!(
        reserved.queries[0].attempts[0].query_id.as_deref(),
        Some("legacy-query")
    );
    // A generic caller cannot clear an unmaterialized legacy attempt or seed
    // smaller counters before the explicit normalization boundary.
    let mut raw = initial;
    raw.job_id = "raw-legacy".into();
    raw.idempotency_key = None;
    raw.owner_id = Some("owner".into());
    raw.lease_token = Some("raw-token".into());
    write(
        &server,
        "mcp:job:org:team:raw-legacy",
        &ssx_json::to_string(&raw.to_payload().unwrap()).unwrap(),
    );
    write(&server, "mcp:joblease:org:team:raw-legacy", "raw-token");
    let raw_id = JobId::new("raw-legacy").unwrap();
    let token = LeaseToken::new("raw-token").unwrap();
    assert!(
        s.mutate(&raw_id, &token, |j| {
            j.reset_retryable_queries();
            Ok(())
        })
        .await
        .is_err()
    );
    assert!(s.mutate(&raw_id, &token, initialize).await.is_err());
    let migrated = s
        .mutate(&raw_id, &token, |j| {
            j.normalize_legacy_evidence();
            initialize(j)?;
            j.reset_retryable_queries();
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(migrated.budget_used["attempts"], 1);
    assert_eq!(
        migrated.queries[0].attempts[0].cursor.as_deref(),
        Some("legacy-snapshot")
    );
}
#[tokio::test]
async fn terminal_and_caller_intent_cannot_be_revoked_by_owned_mutation() {
    let server = Server::start();
    let s = scoped(&server);
    s.create(&job()).await.unwrap();
    let owner = claim(&s, "owner", 10).await;
    s.mutate(&id(), &owner.token, |j| {
        initialize(j)?;
        j.cancel_requested = true;
        j.state = "cancelled".into();
        Ok(())
    })
    .await
    .unwrap();
    assert!(
        s.mutate(&id(), &owner.token, |j| {
            j.cancel_requested = false;
            Ok(())
        })
        .await
        .is_err()
    );
    assert!(
        s.mutate(&id(), &owner.token, |j| {
            j.state = "running".into();
            Ok(())
        })
        .await
        .is_err()
    );
    assert!(
        s.mutate(&id(), &owner.token, |j| {
            j.deadline = ssx_domain::FiniteF64::new(9000.0)?;
            Ok(())
        })
        .await
        .is_err()
    );
    let mut completed = job();
    completed.job_id = "complete".into();
    completed.idempotency_key = None;
    completed.state = "partial".into();
    completed.caller_completed = true;
    s.create(&completed).await.unwrap();
    let complete = JobId::new("complete").unwrap();
    let owner = s
        .claim_prepared(
            &complete,
            "metadata",
            ttl(10),
            &mut Tokens("complete"),
            now(20.0),
            aid,
            true,
            |_| Ok(()),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(
        s.mutate(&complete, &owner.token, |j| {
            j.caller_completed = false;
            Ok(())
        })
        .await
        .is_err()
    );
}
#[tokio::test]
async fn observed_side_key_cancellation_cannot_be_erased_by_callback() {
    let server = Server::start();
    let s = scoped(&server);
    s.create(&job()).await.unwrap();
    let owner = claim(&s, "owner", 10).await;
    assert_eq!(
        s.request_cancel(&id(), now(20.0), &mut Tokens("external"), aid)
            .await
            .unwrap(),
        "running"
    );
    assert!(
        s.mutate(&id(), &owner.token, |j| {
            assert!(j.cancel_requested);
            j.cancel_requested = false;
            Ok(())
        })
        .await
        .is_err()
    );
    assert!(found(&s).await.cancel_requested);
    let observed = s
        .mutate(&id(), &owner.token, |j| {
            assert!(j.cancel_requested);
            j.caller_completed = true;
            j.state = "partial".into();
            Ok(())
        })
        .await
        .unwrap();
    assert!(observed.cancel_requested);
    s.release(&id(), &owner.token).await.unwrap();
    assert!(
        s.claim_prepared(
            &id(),
            "metadata",
            ttl(10),
            &mut Tokens("metadata"),
            now(21.0),
            aid,
            true,
            |j| {
                assert!(j.cancel_requested);
                j.cancel_requested = false;
                Ok(())
            }
        )
        .await
        .is_err()
    );
    assert!(found(&s).await.cancel_requested);
    let mut metadata = job();
    metadata.job_id = "metadata-side".into();
    metadata.idempotency_key = None;
    metadata.caller_completed = true;
    s.create(&metadata).await.unwrap();
    let metadata_id = JobId::new("metadata-side").unwrap();
    let owner = s
        .claim(
            &metadata_id,
            "worker",
            ttl(10),
            &mut Tokens("metadata-owner"),
            now(10.0),
            aid,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        s.request_cancel(
            &metadata_id,
            now(20.0),
            &mut Tokens("metadata-external"),
            aid
        )
        .await
        .unwrap(),
        "running"
    );
    s.release(&metadata_id, &owner.token).await.unwrap();
    assert!(
        s.claim_prepared(
            &metadata_id,
            "metadata",
            ttl(10),
            &mut Tokens("metadata-fresh"),
            now(21.0),
            aid,
            true,
            |j| {
                assert!(j.cancel_requested);
                j.cancel_requested = false;
                Ok(())
            }
        )
        .await
        .is_err()
    );
}
#[tokio::test]
async fn fresh_identity_and_lead_history_require_authoritative_admissions() {
    let server = Server::start();
    let s = scoped(&server);
    s.create(&job()).await.unwrap();
    let owner = claim(&s, "review-fix", 30).await;
    s.mutate(&id(), &owner.token, |j| {
        j.initialize_budget(ResearchCeilings {
            queries: 2,
            engines_per_query: 1,
            results: 1,
        })
    })
    .await
    .unwrap();
    for attempt_id in ["", "bad\nidentity"] {
        assert!(
            s.mutate(&id(), &owner.token, |j| {
                j.queries[0].state = "running".into();
                j.queries[0].attempts.push(ResearchQueryAttempt {
                    attempt_id: attempt_id.into(),
                    state: "running".into(),
                    ..Default::default()
                });
                j.budget_used.insert("attempts".into(), 1);
                j.budget_used.insert("engine_attempts".into(), 1);
                Ok(())
            })
            .await
            .is_err()
        );
    }
    s.mutate(&id(), &owner.token, |j| {
        j.reserve_attempt(0, AttemptId::new("valid")?, now(11.0))?;
        Ok(())
    })
    .await
    .unwrap();
    let leads = vec![
        format!("lead-v1-{}", "a".repeat(64)),
        format!("lead-v1-{}", "b".repeat(64)),
    ];
    assert!(
        s.mutate(&id(), &owner.token, |j| {
            j.queries[0].attempts[0].new_lead_ids = leads.clone();
            j.seen_lead_ids = leads.clone();
            Ok(())
        })
        .await
        .is_err()
    );
    assert!(
        s.mutate(&id(), &owner.token, |j| {
            j.seen_lead_ids = vec![leads[0].clone()];
            Ok(())
        })
        .await
        .is_err()
    );
    assert!(
        s.mutate(&id(), &owner.token, |j| {
            let a = &mut j.queries[0].attempts[0];
            a.state = "done".into();
            a.cursor = Some("snap".into());
            a.admitted_result_ids = vec!["snap:0".into()];
            a.new_lead_ids = vec![leads[0].clone()];
            j.budget_used.insert("results".into(), 1);
            Ok(())
        })
        .await
        .is_err()
    );
    assert!(
        s.mutate(&id(), &owner.token, |j| {
            let a = &mut j.queries[0].attempts[0];
            a.state = "done".into();
            a.cursor = Some("snap".into());
            a.admitted_result_ids = vec!["snap:0".into()];
            a.new_lead_ids = vec![leads[0].clone()];
            j.seen_lead_ids = vec![leads[0].clone(), leads[0].clone()];
            j.budget_used.insert("results".into(), 1);
            Ok(())
        })
        .await
        .is_err()
    );
    let settled = s
        .mutate(&id(), &owner.token, |j| {
            j.queries[0].state = "done".into();
            j.queries[0].cursor = Some("snap".into());
            j.queries[0].result_count = 2;
            j.finish_attempt(
                0,
                &AttemptId::new("valid")?,
                Some(&[
                    LeadId::new(leads[0].clone())?,
                    LeadId::new(leads[1].clone())?,
                ]),
                now(12.0),
            )
        })
        .await
        .unwrap();
    assert_eq!(settled.budget_used["results"], 1);
    assert_eq!(settled.seen_lead_ids, vec![leads[0].clone()]);
    assert!(
        s.mutate(&id(), &owner.token, |j| {
            j.queries[0].attempts.push(ResearchQueryAttempt {
                attempt_id: "valid".into(),
                state: "running".into(),
                ..Default::default()
            });
            j.budget_used.insert("attempts".into(), 2);
            j.budget_used.insert("engine_attempts".into(), 2);
            Ok(())
        })
        .await
        .is_err()
    );
}
#[tokio::test]
async fn existing_empty_legacy_identity_survives_only_faithful_materialization() {
    let server = Server::start();
    let s = scoped(&server);
    let mut legacy = job();
    legacy.state = "partial".into();
    legacy.queries[0].state = "failed".into();
    legacy.queries[0].cursor = Some("legacy-cursor".into());
    legacy.queries[0].result_count = 1;
    s.create(&legacy).await.unwrap();
    let owner = s
        .claim_prepared(
            &id(),
            "legacy-owner",
            ttl(30),
            &mut Tokens("legacy-owner"),
            now(10.0),
            aid,
            false,
            |j| {
                j.initialize_budget(ResearchCeilings {
                    queries: 3,
                    engines_per_query: 1,
                    results: 3,
                })?;
                j.reset_retryable_queries();
                Ok(())
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert!(owner.job.queries[0].attempts[0].attempt_id.is_empty());
    assert_eq!(
        owner.job.queries[0].attempts[0].cursor.as_deref(),
        Some("legacy-cursor")
    );
    let preserved = s
        .mutate(&id(), &owner.token, |j| {
            j.reserve_attempt(0, AttemptId::new("fresh-id")?, now(11.0))?;
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(
        preserved.queries[0].attempts[0],
        owner.job.queries[0].attempts[0]
    );
    assert_eq!(preserved.budget_used["attempts"], 2);
}

#[tokio::test]
async fn actual_url_settlement_is_fenced_charged_and_retained_across_retry() {
    let server = Server::start();
    let store = scoped(&server);
    store.create(&job()).await.unwrap();
    let owner = claim(&store, "identity-owner", 30).await;
    store
        .mutate(&id(), &owner.token, |job| {
            initialize(job)?;
            job.reserve_attempt(0, AttemptId::new("first-url-attempt")?, now(11.0))?;
            Ok(())
        })
        .await
        .unwrap();
    let response = ssx_domain::SearchResponse {
        results: [
            "https://example.com/p?utm_source=x",
            "https://example.com/p",
        ]
        .into_iter()
        .map(|url| ssx_domain::SearchResult {
            url: url.into(),
            ..Default::default()
        })
        .collect(),
        ..Default::default()
    };
    let reserved = found(&store).await;
    assert!(
        store
            .mutate(&id(), &owner.token, |job| {
                let attempt = &mut job.queries[0].attempts[0];
                attempt.state = "done".into();
                attempt.cursor = Some(String::new());
                attempt.admitted_result_ids = vec![":0".into()];
                let lead = ssx_application::research::lead_id(&response.results[0].url)
                    .as_str()
                    .to_owned();
                attempt.new_lead_ids = vec![lead.clone()];
                job.seen_lead_ids = vec![lead];
                job.budget_used.insert("results".into(), 1);
                Ok(())
            })
            .await
            .is_err()
    );
    assert_eq!(found(&store).await, reserved);
    let done = store
        .mutate(&id(), &owner.token, |job| {
            job.queries[0].state = "done".into();
            job.queries[0].cursor = Some("snapshot-one".into());
            job.queries[0].result_count = 2;
            ssx_application::research::finish_attempt(
                job,
                0,
                &AttemptId::new("first-url-attempt")?,
                Some(&response),
                now(12.0),
            )
        })
        .await
        .unwrap();
    let identity = ssx_application::research::lead_id(&response.results[0].url)
        .as_str()
        .to_owned();
    assert_eq!(done.seen_lead_ids, vec![identity.clone()]);
    assert_eq!(done.budget_used["results"], 2);
    let first = done.queries[0].attempts[0].clone();
    store
        .mutate(&id(), &owner.token, |job| {
            job.queries[0].state = "pending".into(); // Explicit retry admission; completed history remains immutable.
            job.reserve_attempt(0, AttemptId::new("second-url-attempt")?, now(13.0))?;
            Ok(())
        })
        .await
        .unwrap();
    let done = store
        .mutate(&id(), &owner.token, |job| {
            job.queries[0].state = "done".into();
            job.queries[0].cursor = Some("snapshot-two".into());
            job.queries[0].result_count = 2;
            ssx_application::research::finish_attempt(
                job,
                0,
                &AttemptId::new("second-url-attempt")?,
                Some(&response),
                now(14.0),
            )
        })
        .await
        .unwrap();
    assert_eq!(done.budget_used["results"], 3);
    assert_eq!(done.seen_lead_ids, vec![identity]);
    assert_eq!(done.queries[0].attempts[0], first);
    assert!(done.queries[0].attempts[1].new_lead_ids.is_empty());
    assert_eq!(
        done.queries[0].attempts[1].admitted_result_ids,
        vec!["snapshot-two:0"]
    );
    assert_eq!(done.stop_reason.as_deref(), Some("result_budget_exhausted"));
    assert_eq!(found(&store).await, done);
}

fn request_cancel_between_reads_and_commit(server: &Server, tenant: &str, job_id: &JobId) {
    let url = server.url.clone();
    let tenant = tenant.to_owned();
    let job_id = job_id.clone();
    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let store = ResearchStore::new(
                    Arc::new(Valkey::new(&url, Duration::from_millis(300)).unwrap()),
                    TenantId::new(tenant).unwrap(),
                );
                assert_eq!(
                    store
                        .request_cancel(&job_id, now(11.0), &mut Tokens("race-cancel"), aid)
                        .await
                        .unwrap(),
                    "running"
                );
            });
    })
    .join()
    .unwrap();
}

#[tokio::test]
async fn cancel_signal_between_observation_and_owned_commit_rejects_reservation() {
    let server = Server::start();
    let store = scoped(&server);
    store.create(&job()).await.unwrap();
    let owner = claim(&store, "race-owner", 30).await;
    let result = store
        .mutate(&id(), &owner.token, |job| {
            initialize(job)?;
            job.reserve_attempt(0, AttemptId::new("cancel-race-attempt")?, now(11.0))?;
            request_cancel_between_reads_and_commit(&server, "org:team", &id());
            Ok(())
        })
        .await;
    assert!(
        matches!(result, Err(Error::Conflict)),
        "cancellation race accepted: {result:?}"
    );
    assert!(found(&store).await.queries[0].attempts.is_empty());
}

#[tokio::test]
async fn colon_containing_tenants_and_idempotency_components_are_independent() {
    let server = Server::start();
    let a = scoped(&server);
    let b = a.for_tenant(TenantId::new("org:team:x").unwrap());
    let mut first = job();
    first.job_id = "job-first".into();
    first.idempotency_key = Some("x:y".into());
    let mut second = first.clone();
    second.job_id = "job-second".into();
    second.tenant = "org:team:x".into();
    second.idempotency_key = Some("y".into());
    assert!(a.create(&first).await.unwrap().1);
    assert!(b.create(&second).await.unwrap().1);
    assert!(
        matches!(a.find_by_idempotency("x:y").await, JobRead::Found(j) if j.job_id == "job-first")
    );
    assert!(
        matches!(b.find_by_idempotency("y").await, JobRead::Found(j) if j.job_id == "job-second")
    );
}

#[tokio::test]
async fn cancellation_snapshot_fences_claim_and_prepared_claim() {
    for prepared in [false, true] {
        let server = Server::start();
        let store = scoped(&server);
        let mut initial = job();
        if !prepared {
            initial.state = "running".into();
            initial.queries[0].state = "running".into();
        }
        store.create(&initial).await.unwrap();
        let result = if prepared {
            store
                .claim_prepared(
                    &id(),
                    "race-owner",
                    ttl(30),
                    &mut Tokens("prepared-race"),
                    now(10.0),
                    aid,
                    false,
                    |_| {
                        request_cancel_between_reads_and_commit(&server, "org:team", &id());
                        Ok(())
                    },
                )
                .await
        } else {
            store
                .claim(
                    &id(),
                    "race-owner",
                    ttl(30),
                    &mut Tokens("claim-race"),
                    now(10.0),
                    || {
                        request_cancel_between_reads_and_commit(&server, "org:team", &id());
                        aid()
                    },
                )
                .await
        };
        assert!(
            matches!(result, Err(Error::Conflict)),
            "claim cancellation race accepted: {result:?}"
        );
        let persisted = found(&store).await;
        assert!(persisted.cancel_requested);
        assert!(persisted.queries[0].attempts.is_empty());
        let exists: bool = redis::cmd("EXISTS")
            .arg("mcp:joblease:org:team:job-test")
            .query(&mut server.connection())
            .unwrap();
        assert!(!exists);
        assert!(
            store
                .claim(
                    &id(),
                    "retry-owner",
                    ttl(30),
                    &mut Tokens("after-cancel"),
                    now(12.0),
                    aid
                )
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(found(&store).await.state, "cancelled");
    }
}

#[tokio::test]
async fn changed_true_cancellation_bytes_conflict_even_when_next_flag_is_true() {
    let server = Server::start();
    let store = scoped(&server);
    store.create(&job()).await.unwrap();
    let owner = claim(&store, "true-owner", 30).await;
    request_cancel_between_reads_and_commit(&server, "org:team", &id());
    let result = store
        .mutate(&id(), &owner.token, |_| {
            write(
                &server,
                "mcp:jobcancel:org:team:job-test",
                "{\"cancel_requested\":true,\"requested_at\":12}",
            );
            Ok(())
        })
        .await;
    assert!(matches!(result, Err(Error::Conflict)));
    assert!(found(&store).await.cancel_requested);
}

#[tokio::test]
async fn verified_legacy_idempotency_adoption_preserves_record_and_remaining_ttl() {
    let server = Server::start();
    let store = scoped(&server);
    let old = job();
    let record = ssx_json::to_string(&old.to_payload().unwrap()).unwrap();
    let legacy_key = "mcp:idem:org:team:request-1";
    redis::cmd("SETEX")
        .arg("mcp:job:org:team:job-test")
        .arg(60)
        .arg(&record)
        .query::<()>(&mut server.connection())
        .unwrap();
    redis::cmd("PSETEX")
        .arg(legacy_key)
        .arg(5000)
        .arg("{\"job_id\":\"job-test\"}")
        .query::<()>(&mut server.connection())
        .unwrap();
    assert!(
        matches!(store.find_by_idempotency("request-1").await, JobRead::Found(j) if j.job_id == "job-test")
    );
    let mut candidate = old.clone();
    candidate.job_id = "replacement".into();
    let (reused, created) = store.create(&candidate).await.unwrap();
    assert!(!created);
    assert_eq!(reused.job_id, "job-test");
    let new_key = ssx_infra::valkey::keys::research_idempotency_v2(store.tenant(), "request-1");
    let remaining: i64 = redis::cmd("PTTL")
        .arg(&new_key)
        .query(&mut server.connection())
        .unwrap();
    assert!((1..=5000).contains(&remaining));
    let unchanged: String = redis::cmd("GET")
        .arg("mcp:job:org:team:job-test")
        .query(&mut server.connection())
        .unwrap();
    assert_eq!(unchanged, record);
    let absent: bool = redis::cmd("EXISTS")
        .arg("mcp:job:org:team:replacement")
        .query(&mut server.connection())
        .unwrap();
    assert!(!absent);
}

#[tokio::test]
async fn malformed_v2_never_falls_back_and_legacy_aliases_require_verified_binding() {
    for legacy_value in [
        "not-json",
        "{\"job_id\":\"absent\"}",
        "{\"job_id\":\"foreign\"}",
    ] {
        let server = Server::start();
        let store = scoped(&server);
        write(&server, "mcp:idem:org:team:request-1", legacy_value);
        let mut foreign = job();
        foreign.job_id = "foreign".into();
        foreign.idempotency_key = Some("different".into());
        write(
            &server,
            "mcp:job:org:team:foreign",
            &ssx_json::to_string(&foreign.to_payload().unwrap()).unwrap(),
        );
        assert!(matches!(
            store.find_by_idempotency("request-1").await,
            JobRead::Missing
        ));
        assert!(store.create(&job()).await.unwrap().1);
    }
    let server = Server::start();
    let store = scoped(&server);
    let record = ssx_json::to_string(&job().to_payload().unwrap()).unwrap();
    write(&server, "mcp:job:org:team:job-test", &record);
    write(
        &server,
        "mcp:idem:org:team:request-1",
        "{\"job_id\":\"job-test\"}",
    );
    let key = ssx_infra::valkey::keys::research_idempotency_v2(store.tenant(), "request-1");
    for value in ["not-json", "{}", "{\"job_id\":\"absent\"}"] {
        write(&server, &key, value);
        assert!(matches!(
            store.find_by_idempotency("request-1").await,
            JobRead::Malformed
        ));
        let mut candidate = job();
        candidate.job_id = "must-not-create".into();
        assert!(store.create(&candidate).await.is_err());
        let exists: bool = redis::cmd("EXISTS")
            .arg("mcp:job:org:team:must-not-create")
            .query(&mut server.connection())
            .unwrap();
        assert!(!exists);
    }
}

#[tokio::test]
async fn expired_legacy_idempotency_is_not_refreshed_or_adopted() {
    let server = Server::start();
    let store = scoped(&server);
    write(
        &server,
        "mcp:job:org:team:old-job",
        &ssx_json::to_string(&{
            let mut j = job();
            j.job_id = "old-job".into();
            j.to_payload().unwrap()
        })
        .unwrap(),
    );
    let old_key = "mcp:idem:org:team:request-1";
    redis::cmd("PSETEX")
        .arg(old_key)
        .arg(1)
        .arg("{\"job_id\":\"old-job\"}")
        .query::<()>(&mut server.connection())
        .unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(matches!(
        store.find_by_idempotency("request-1").await,
        JobRead::Missing
    ));
    assert!(store.create(&job()).await.unwrap().1);
    let exists: bool = redis::cmd("EXISTS")
        .arg(old_key)
        .query(&mut server.connection())
        .unwrap();
    assert!(!exists);
}

#[tokio::test]
async fn cancellation_intent_rejects_new_reservations_but_retains_existing_settlement() {
    for observed in [false, true] {
        let server = Server::start();
        let store = scoped(&server);
        store.create(&job()).await.unwrap();
        let owner = claim(&store, "cancel-guard-owner", 30).await;
        store
            .mutate(&id(), &owner.token, |job| {
                initialize(job)?;
                Ok(())
            })
            .await
            .unwrap();
        if observed {
            request_cancel_between_reads_and_commit(&server, "org:team", &id());
        }
        let result = store
            .mutate(&id(), &owner.token, |job| {
                job.reserve_attempt(0, AttemptId::new("must-not-reserve")?, now(12.0))?;
                job.cancel_requested = true;
                Ok(())
            })
            .await;
        assert!(matches!(result, Err(Error::Invalid(_))));
        assert_eq!(found(&store).await.budget_used["attempts"], 0);
    }
    let server = Server::start();
    let store = scoped(&server);
    store.create(&job()).await.unwrap();
    let owner = claim(&store, "settlement-owner", 30).await;
    store
        .mutate(&id(), &owner.token, |job| {
            initialize(job)?;
            job.reserve_attempt(0, AttemptId::new("already-reserved")?, now(11.0))?;
            Ok(())
        })
        .await
        .unwrap();
    request_cancel_between_reads_and_commit(&server, "org:team", &id());
    let settled = store
        .mutate(&id(), &owner.token, |job| {
            job.queries[0].state = "done".into();
            job.finish_attempt(0, &AttemptId::new("already-reserved")?, None, now(12.0))
        })
        .await
        .unwrap();
    assert!(settled.cancel_requested);
    assert_eq!(settled.budget_used["attempts"], 1);
    assert_eq!(settled.queries[0].attempts[0].state, "done");
}

#[tokio::test]
async fn immortal_legacy_alias_or_record_is_not_lookup_or_admission_authority() {
    for immortal_mapping in [false, true] {
        let server = Server::start();
        let store = scoped(&server);
        let mut old = job();
        old.job_id = "legacy-old".into();
        let record = ssx_json::to_string(&old.to_payload().unwrap()).unwrap();
        redis::cmd("SETEX")
            .arg("mcp:job:org:team:legacy-old")
            .arg(60)
            .arg(&record)
            .query::<()>(&mut server.connection())
            .unwrap();
        redis::cmd("SETEX")
            .arg("mcp:idem:org:team:request-1")
            .arg(60)
            .arg("{\"job_id\":\"legacy-old\"}")
            .query::<()>(&mut server.connection())
            .unwrap();
        let immortal = if immortal_mapping {
            "mcp:idem:org:team:request-1"
        } else {
            "mcp:job:org:team:legacy-old"
        };
        redis::cmd("PERSIST")
            .arg(immortal)
            .query::<i64>(&mut server.connection())
            .unwrap();
        assert!(matches!(
            store.find_by_idempotency("request-1").await,
            JobRead::Missing
        ));
        assert!(store.create(&job()).await.unwrap().1);
        let ttl: i64 = redis::cmd("PTTL")
            .arg(immortal)
            .query(&mut server.connection())
            .unwrap();
        assert_eq!(ttl, -1);
    }
}

#[tokio::test]
async fn scoped_ready_selection_preserves_microseconds_and_excludes_future_scores() {
    let server = Server::start();
    let mut connection = server.connection();
    let source = include_str!("../src/valkey/research_scripts/ready_scoped.lua");
    assert_eq!(source.matches("redis.call('TIME')").count(), 1);
    // Inject only the clock at the exact production Lua boundary; real Valkey
    // still performs numeric conversion, score comparison, ordering and limit.
    let fixed = source.replace("redis.call('TIME')", "{'100','123456'}");
    let key = "test:ready:precision";
    redis::cmd("ZADD")
        .arg(key)
        .arg("100.123456")
        .arg("due-now")
        .arg("100.123457")
        .arg("future")
        .query::<i64>(&mut connection)
        .unwrap();
    let old: Vec<String> = redis::cmd("ZRANGEBYSCORE")
        .arg(key)
        .arg("-inf")
        .arg(100123f64 / 1000.0)
        .arg("LIMIT")
        .arg(0)
        .arg(16)
        .query(&mut connection)
        .unwrap();
    assert!(
        old.is_empty(),
        "old millisecond cutoff loses currently due entry"
    );
    let exact: Vec<String> = redis::cmd("EVAL")
        .arg(&fixed)
        .arg(1)
        .arg(key)
        .query(&mut connection)
        .unwrap();
    assert_eq!(exact, ["due-now"]);
    for index in 0..30 {
        redis::cmd("ZADD")
            .arg(key)
            .arg("100.1")
            .arg(format!("due-{index:02}"))
            .query::<i64>(&mut connection)
            .unwrap();
    }
    let bounded: Vec<String> = redis::cmd("EVAL")
        .arg(&fixed)
        .arg(1)
        .arg(key)
        .query(&mut connection)
        .unwrap();
    assert_eq!(bounded.len(), 16);
    assert!(!bounded.iter().any(|id| id == "future"));
}

#[tokio::test]
async fn same_lease_active_dispatch_rejects_a_second_reservation() {
    let server = Server::start();
    let store = scoped(&server);
    store.create(&job()).await.unwrap();
    let owner = claim(&store, "overlap-owner", 30).await;
    store.mutate(&id(), &owner.token, initialize).await.unwrap();
    let first_store = scoped(&server);
    let token = owner.token.clone();
    let (reserved_tx, reserved_rx) = tokio::sync::oneshot::channel();
    let (complete_tx, complete_rx) = tokio::sync::oneshot::channel();
    let first = tokio::spawn(async move {
        first_store
            .mutate(&id(), &token, |job| {
                job.reserve_attempt(0, AttemptId::new("overlap-first")?, now(11.0))?;
                Ok(())
            })
            .await
            .unwrap();
        reserved_tx.send(()).unwrap();
        complete_rx.await.unwrap();
        first_store
            .mutate(&id(), &token, |job| {
                job.queries[0].state = "done".into();
                ssx_application::research::finish_attempt(
                    job,
                    0,
                    &AttemptId::new("overlap-first")?,
                    None,
                    now(13.0),
                )
            })
            .await
            .unwrap()
    });
    reserved_rx.await.unwrap(); // First dispatch is outstanding while caller two loads its reservation.
    let reserved = found(&store).await;
    let second = scoped(&server)
        .mutate(&id(), &owner.token, |job| {
            job.reserve_attempt(0, AttemptId::new("overlap-second")?, now(12.0))?;
            Ok(())
        })
        .await;
    assert!(
        matches!(
            second,
            Err(Error::Invalid(ssx_domain::DomainError(
                "reservation requires a pending query without an active attempt"
            )))
        ),
        "unexpected reservation result: {second:?}"
    );
    assert_eq!(found(&store).await, reserved);
    complete_tx.send(()).unwrap();
    let done = first.await.unwrap();
    assert_eq!(done.queries[0].attempts.len(), 1);
    assert_eq!(done.queries[0].attempts[0].attempt_id, "overlap-first");
    assert_eq!(done.queries[0].attempts[0].state, "done");
    assert_eq!(done.budget_used["attempts"], 1);
}

#[tokio::test]
async fn recovered_attempt_rejects_stale_application_completion_with_current_lease() {
    let server = Server::start();
    let store = scoped(&server);
    store.create(&job()).await.unwrap();
    let first = claim(&store, "old-worker", 1).await;
    store
        .mutate(&id(), &first.token, |job| {
            initialize(job)?;
            job.reserve_attempt(0, AttemptId::new("stale-first")?, now(11.0))?;
            Ok(())
        })
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let successor = scoped(&server);
    let next = claim(&successor, "new-worker", 30).await;
    assert_eq!(next.job.queries[0].state, "pending");
    assert_eq!(next.job.queries[0].attempts[0].state, "interrupted");
    successor
        .mutate(&id(), &next.token, |job| {
            job.reserve_attempt(0, AttemptId::new("fresh-second")?, now(12.0))?;
            Ok(())
        })
        .await
        .unwrap();
    let reserved = found(&successor).await;
    assert!(matches!(
        store.mutate(&id(), &first.token, |_| Ok(())).await,
        Err(Error::OwnershipLost)
    ));
    let response = ssx_domain::SearchResponse {
        results: vec![ssx_domain::SearchResult {
            url: "https://stale.example/".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let stale = successor
        .mutate(&id(), &next.token, |job| {
            job.queries[0].state = "done".into();
            job.queries[0].cursor = Some("stale-snapshot".into());
            job.queries[0].result_count = 1;
            ssx_application::research::finish_attempt(
                job,
                0,
                &AttemptId::new("stale-first")?,
                Some(&response),
                now(13.0),
            )
        })
        .await;
    assert!(
        matches!(
            stale,
            Err(Error::Invalid(ssx_domain::DomainError(
                "research attempt identity changed before settlement"
            )))
        ),
        "unexpected stale completion result: {stale:?}"
    );
    assert_eq!(
        found(&successor).await,
        reserved,
        "stale response fields must not persist"
    );
    let mut fresh_response = response.clone();
    fresh_response.results[0].url = "https://fresh.example/".into();
    let done = successor
        .mutate(&id(), &next.token, |job| {
            job.queries[0].state = "done".into();
            job.queries[0].cursor = Some("fresh-snapshot".into());
            job.queries[0].result_count = 1;
            ssx_application::research::finish_attempt(
                job,
                0,
                &AttemptId::new("fresh-second")?,
                Some(&fresh_response),
                now(14.0),
            )
        })
        .await
        .unwrap();
    assert_eq!(done.queries[0].attempts[0], reserved.queries[0].attempts[0]);
    assert_eq!(done.queries[0].attempts[1].state, "done");
    assert_eq!(
        done.queries[0].attempts[1].cursor.as_deref(),
        Some("fresh-snapshot")
    );
    assert_eq!(done.budget_used["attempts"], 2);
    assert_eq!(done.budget_used["results"], 1);
    assert_eq!(
        done.seen_lead_ids,
        vec![
            ssx_application::research::lead_id("https://fresh.example/")
                .as_str()
                .to_owned()
        ]
    );
}
