use super::*;
use ssx_application::{
    NoopSink,
    policy::SearchPolicy,
    scope::{CatalogSnapshot, ScopeResolver},
};
use ssx_domain::rate::{EngineKey, QuotaReceipt, RateFailure, SharedCooldown};
use std::sync::atomic::AtomicUsize;

#[derive(Default)]
struct Witness {
    entered: Notify,
    terminal: AtomicUsize,
    context: Mutex<Option<Instant>>,
}
struct Terminal(Arc<Witness>);
impl Drop for Terminal {
    fn drop(&mut self) {
        self.0.terminal.fetch_add(1, Ordering::SeqCst);
    }
}
#[derive(Clone, Copy)]
enum Mode {
    Complete,
    Pending,
    Panic,
}
struct Engine {
    name: &'static str,
    mode: Mode,
    witness: Arc<Witness>,
}
impl SearchEngine for Engine {
    fn search<'a>(&'a self, _: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        panic!("context-aware fixture must receive the owner's absolute deadline")
    }
    fn search_with_context<'a>(
        &'a self,
        _: &'a SearchRequest,
        context: SearchContext,
    ) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            let _terminal = Terminal(self.witness.clone());
            *self.witness.context.lock().unwrap() = Some(Instant::from_std(context.deadline));
            self.witness.entered.notify_one();
            match self.mode {
                Mode::Complete => AdapterResponse {
                    results: (0..3)
                        .map(|i| ssx_domain::SearchResult {
                            url: format!("https://{}.invalid/{i}", self.name),
                            title: format!("{} {i}", self.name),
                            engine: self.name.into(),
                            ..Default::default()
                        })
                        .collect(),
                    status: EngineStatus::Ok,
                    latency_ms: ssx_domain::FiniteF64::new(2.0).unwrap(),
                    ..Default::default()
                },
                Mode::Pending => std::future::pending().await,
                Mode::Panic => panic!("controlled adapter task defect"),
            }
        })
    }
}
#[derive(Default)]
struct Sink(
    Mutex<Vec<&'static str>>,
    Mutex<Vec<(String, OutcomeOrigin)>>,
);
impl Metrics for Sink {
    fn increment(&self, name: &'static str) {
        self.0.lock().unwrap().push(name);
    }
    fn engine_outcome(&self, name: &str, _: &AdapterResponse, origin: OutcomeOrigin) {
        self.1.lock().unwrap().push((name.into(), origin));
    }
}
#[derive(Default)]
struct Cooldown {
    entered: Notify,
    release: Notify,
    terminal: AtomicUsize,
}
impl ssx_application::rate::RateStore for Cooldown {
    fn acquire<'a>(
        &'a self,
        _: &'a EngineKey,
        _: u64,
    ) -> PortFuture<'a, Result<QuotaReceipt, RateFailure>> {
        Box::pin(async { Err(RateFailure::InvalidInput) })
    }
    fn cooldown<'a>(
        &'a self,
        key: &'a EngineKey,
    ) -> PortFuture<'a, Result<SharedCooldown, RateFailure>> {
        Box::pin(async move {
            if key.as_str() != "duckduckgo" {
                return Ok(SharedCooldown::Missing);
            }
            self.entered.notify_one();
            self.release.notified().await;
            self.terminal.fetch_add(1, Ordering::SeqCst);
            Ok(SharedCooldown::Active { seconds: 10 })
        })
    }
    fn publish_cooldown<'a>(
        &'a self,
        _: &'a EngineKey,
        _: f64,
    ) -> PortFuture<'a, Result<u64, RateFailure>> {
        Box::pin(async { Err(RateFailure::InvalidInput) })
    }
    fn warmup(&self) -> PortFuture<'_, Result<(), RateFailure>> {
        Box::pin(async { Ok(()) })
    }
    fn shutdown(&self) -> PortFuture<'_, ()> {
        Box::pin(async {})
    }
}

fn service(
    engines: BTreeMap<String, EngineBinding>,
    metrics: Arc<dyn Metrics>,
    cooldowns: Option<Arc<dyn ssx_application::rate::RateStore>>,
) -> SearchService {
    SearchService::new(
        SearchConfig::default(),
        SearchDependencies {
            policy: PolicyService {
                policy: SearchPolicy::default(),
                catalog: CatalogSnapshot::new(vec![]).unwrap(),
                resolver: ScopeResolver::default(),
            },
            engines,
            routing_identity: RoutingIdentity {
                catalog: None,
                sensitive: Default::default(),
                tier1: Default::default(),
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
            metrics,
            supervisor: Arc::new(NoopSink),
            audit: Arc::new(NoopSink),
            cooldowns,
            owned_http: vec![],
            owned_stores: vec![],
            clock: Arc::new(SystemSearchClock),
            reranker: None,
        },
    )
    .unwrap()
}

// These controls execute the real owner; only collection scheduling is held.
// The clock cannot auto-advance while the test is runnable in wait_finished.
async fn wait_finished(pause: &CollectionPause, indices: &[usize]) {
    loop {
        if indices
            .iter()
            .all(|i| pause.tasks.lock().unwrap()[*i].is_finished())
        {
            break;
        }
        tokio::task::yield_now().await;
    }
}
#[derive(Clone, Copy)]
enum Case {
    Complete,
    Pending,
    BindingLate,
    CooldownLate,
    Panic,
}
async fn owner_case(case: Case) {
    let start = Instant::now();
    let deadline = start + Duration::from_millis(1);
    let first = Arc::new(Witness::default());
    let second = Arc::new(Witness::default());
    let cooldown = Arc::new(Cooldown::default());
    let sink = Arc::new(Sink::default());
    let second_mode = match case {
        Case::Pending | Case::BindingLate => Mode::Pending,
        Case::Panic => Mode::Panic,
        _ => Mode::Complete,
    };
    let bindings = BTreeMap::from([
        (
            "wikipedia".into(),
            EngineBinding {
                engine: Arc::new(Engine {
                    name: "wikipedia",
                    mode: Mode::Complete,
                    witness: first.clone(),
                }),
                generation: "deadline-drain".into(),
                timeout: Duration::from_secs(3),
                scrape: false,
            },
        ),
        (
            "duckduckgo".into(),
            EngineBinding {
                engine: Arc::new(Engine {
                    name: "duckduckgo",
                    mode: second_mode,
                    witness: second.clone(),
                }),
                generation: "deadline-drain".into(),
                timeout: if matches!(case, Case::BindingLate) {
                    Duration::from_micros(500)
                } else {
                    Duration::from_secs(3)
                },
                scrape: false,
            },
        ),
    ]);
    let service = service(
        bindings,
        sink.clone(),
        if matches!(case, Case::CooldownLate) {
            Some(cooldown.clone())
        } else {
            None
        },
    );
    let pause = Arc::new(CollectionPause::default());
    if matches!(case, Case::Pending) {
        *pause.hold_completion.lock().unwrap() = Some(1);
    }
    *service.0.collection_pause.lock().unwrap() = Some(pause.clone());
    let owner = service.0.clone();
    let (_sender, receiver) = watch::channel(false);
    let task = tokio::spawn(async move {
        execute(
            &owner,
            SearchRequest {
                query: "q".into(),
                interactive_timeout_ms: Some(1),
                ..Default::default()
            },
            ScopeDecision {
                selected_engines: vec!["wikipedia".into(), "duckduckgo".into()],
                ..Default::default()
            },
            start,
            receiver,
        )
        .await
    });
    pause.entered.notified().await;
    first.entered.notified().await;
    if matches!(case, Case::CooldownLate) {
        cooldown.entered.notified().await;
    } else {
        second.entered.notified().await;
    }
    wait_finished(&pause, &[0]).await;
    if matches!(case, Case::Complete | Case::Panic) {
        wait_finished(&pause, &[1]).await;
    }
    assert_eq!(
        Instant::now(),
        start,
        "owner/engine barriers must precede explicit time advance"
    );
    assert_eq!(*first.context.lock().unwrap(), Some(deadline));
    if !matches!(case, Case::CooldownLate) {
        assert_eq!(
            *second.context.lock().unwrap(),
            Some(if matches!(case, Case::BindingLate) {
                start + Duration::from_micros(500)
            } else {
                deadline
            })
        );
    }
    tokio::time::advance(Duration::from_millis(2)).await;
    if matches!(case, Case::CooldownLate) {
        cooldown.release.notify_one();
        wait_finished(&pause, &[1]).await;
    } else if matches!(case, Case::BindingLate) {
        wait_finished(&pause, &[1]).await;
    }
    let pending_before_owner_abort = if matches!(case, Case::Pending) {
        pause.completion_entered.notified().await;
        !pause.tasks.lock().unwrap()[1].is_finished()
    } else {
        false
    };
    assert_eq!(Instant::now(), start + Duration::from_millis(2));
    pause.release.notify_one();
    let joined = task.await;
    // Owned cleanup precedes primary semantic assertions, including negatives.
    service.shutdown().await;
    assert!(
        pause
            .tasks
            .lock()
            .unwrap()
            .iter()
            .all(tokio::task::AbortHandle::is_finished)
    );
    assert_eq!(first.terminal.load(Ordering::SeqCst), 1);
    assert_eq!(
        second.terminal.load(Ordering::SeqCst),
        usize::from(!matches!(case, Case::CooldownLate))
    );
    if matches!(case, Case::CooldownLate) {
        assert_eq!(cooldown.terminal.load(Ordering::SeqCst), 1);
    }
    let label = match case {
        Case::Complete => "complete",
        Case::Pending => "pending",
        Case::BindingLate => "binding-late",
        Case::CooldownLate => "cooldown-late",
        Case::Panic => "panic",
    };
    println!(
        "T06_1010_DRAIN_CLEANUP {label} tasks={} first={} second={} cooldown={}",
        pause.tasks.lock().unwrap().len(),
        first.terminal.load(Ordering::SeqCst),
        second.terminal.load(Ordering::SeqCst),
        cooldown.terminal.load(Ordering::SeqCst)
    );
    let (canonical, admission) = joined
        .expect("owner task must join without panic")
        .expect("owner returns response");
    let response = canonical.response();
    assert_eq!(
        response.results.len(),
        if matches!(case, Case::Complete) { 6 } else { 3 },
        "pre-deadline completed results survive owner drain after owned cleanup"
    );
    let other = response
        .engine_outcomes
        .iter()
        .find(|row| row.engine == "duckduckgo")
        .unwrap();
    let expected_internal = matches!(case, Case::Panic);
    if matches!(case, Case::Pending) {
        assert!(
            pending_before_owner_abort,
            "real owner must receive an unfinished task"
        );
        assert_eq!(
            pause.cancelled_joins.load(Ordering::SeqCst),
            1,
            "real owner abort must produce one cancelled JoinError before classification"
        );
        println!("T06_1010_OWNER_CANCEL_JOIN cancelled=1 pending_before_abort=true");
    }
    assert_eq!(
        sink.0
            .lock()
            .unwrap()
            .contains(&"search_engine_internal_failure"),
        expected_internal,
        "owner cancellation is not an internal defect; panic remains one"
    );
    if matches!(case, Case::Complete | Case::Panic) {
        assert!(
            !response.deadline_exceeded,
            "collection delay alone is not deadline truncation"
        );
        if matches!(case, Case::Complete) {
            assert!(!response.partial);
            assert_eq!(admission, CacheAdmission::Complete);
            assert_eq!(other.status, "ok");
        } else {
            assert!(response.partial);
            assert_eq!(other.status, "error");
        }
    } else {
        assert_eq!(
            other.status, "unavailable",
            "completion at or after the service deadline is not retained"
        );
        assert!(response.deadline_exceeded);
        assert!(response.partial);
        assert_eq!(admission, CacheAdmission::DeadlineTruncated);
        assert_eq!(
            other.latency_ms.as_ref().map(ssx_domain::FiniteF64::get),
            Some(if matches!(case, Case::CooldownLate) {
                0.0
            } else {
                1.0
            })
        );
    }
    let expected_origin = match case {
        Case::Complete => Some(OutcomeOrigin::AdapterReturned),
        Case::CooldownLate => Some(OutcomeOrigin::Undispatched),
        Case::Panic => None,
        _ => Some(OutcomeOrigin::CallerCancelled),
    };
    assert_eq!(
        sink.1
            .lock()
            .unwrap()
            .iter()
            .find(|(name, _)| name == "duckduckgo")
            .map(|(_, origin)| *origin),
        expected_origin
    );
    assert_eq!(
        response.dispatched_engine_count,
        if matches!(case, Case::CooldownLate) {
            1
        } else {
            2
        }
    );
}
#[tokio::test(start_paused = true)]
async fn predeadline_completed_outputs_survive_delayed_owner_drain() {
    owner_case(Case::Complete).await;
}
#[tokio::test(start_paused = true)]
async fn deadline_abort_drains_pending_without_internal_failure() {
    owner_case(Case::Pending).await;
}
#[tokio::test(start_paused = true)]
async fn earlier_binding_timeout_completed_after_service_deadline_is_truncated() {
    owner_case(Case::BindingLate).await;
}
#[tokio::test(start_paused = true)]
async fn cooldown_completed_after_service_deadline_is_truncated() {
    owner_case(Case::CooldownLate).await;
}
#[tokio::test(start_paused = true)]
async fn completed_task_panic_is_not_hidden_by_owner_abort() {
    owner_case(Case::Panic).await;
}
