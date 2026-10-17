use super::*;
use std::{
    future::Future,
    num::NonZeroUsize,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
};
struct NoHttp(AtomicUsize);
impl crate::http::Transport for NoHttp {
    fn exchange(&self, _: Request) -> crate::http::ExchangeFuture<'_> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { panic!("unexpected HTTP dispatch") })
    }
}
fn pool() -> AtomPool {
    AtomPool::new(
        PathBuf::from("/no-helper-may-be-spawned-in-these-pre-http-controls"),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap()
}
fn fixture(config: ArxivConfig) -> (ArxivAdapter, Arc<NoHttp>, AtomPool) {
    let http = Arc::new(AdapterHttp::default());
    let calls = Arc::new(NoHttp(AtomicUsize::new(0)));
    http.set_transport(calls.clone()).unwrap();
    let pool = pool();
    (
        ArxivAdapter::new(
            http,
            Arc::new(super::super::AllowAllAdmission),
            config,
            pool.clone(),
        ),
        calls,
        pool,
    )
}
#[tokio::test]
async fn actual_runtime_allocation_refusal_stops_before_http_and_restores_admission() {
    let (adapter, calls, pool) = fixture(ArxivConfig::default());
    let budget = Budget::new(65536).unwrap();
    let request = SearchRequest {
        query: "a ".repeat(500_000),
        ..Default::default()
    };
    let result = adapter
        .run_budgeted(
            &request,
            Some(SearchContext {
                deadline: Instant::now() + Duration::from_secs(30),
            }),
            &budget,
        )
        .await;
    assert_eq!(result.status, EngineStatus::Error);
    assert_eq!(
        result.error_message.as_deref(),
        Some("arXiv native resource limit exceeded")
    );
    assert_eq!(calls.0.load(Ordering::SeqCst), 0);
    let snapshot = budget.snapshot().unwrap();
    assert!(snapshot.peak > 0);
    assert_eq!(snapshot.used, 0);
    assert_eq!(pool.available_operations(), 1);
    assert_eq!(pool.available_slots(), 1);
    pool.shutdown().await.unwrap();
}
#[tokio::test]
async fn actual_runtime_query_copy_cancellation_and_real_deadline_release_charged_buffers() {
    for expire in [false, true] {
        let (adapter, calls, pool) = fixture(ArxivConfig::default());
        let budget = Budget::new(16 * 1024 * 1024).unwrap();
        let request = SearchRequest {
            query: "alpha beta ".repeat(150_000),
            ..Default::default()
        };
        let deadline = Instant::now() + Duration::from_secs(if expire { 2 } else { 30 });
        let mut future =
            Box::pin(adapter.run_budgeted(&request, Some(SearchContext { deadline }), &budget));
        let mut context = Context::from_waker(Waker::noop());
        let mut observed = 0;
        for _ in 0..20000 {
            assert!(
                matches!(future.as_mut().poll(&mut context), Poll::Pending),
                "operation completed before charged source-copy phase"
            );
            if budget.snapshot().unwrap().used > 0 {
                observed += 1;
                if observed == 16 {
                    break;
                }
            }
        }
        assert_eq!(observed, 16);
        assert!(budget.snapshot().unwrap().used > 0);
        assert_eq!(calls.0.load(Ordering::SeqCst), 0);
        assert_eq!(pool.available_operations(), 0);
        if expire {
            tokio::time::sleep_until(deadline.into()).await;
            let result = future.await;
            assert_eq!(result.status, EngineStatus::Timeout);
        } else {
            drop(future);
        }
        assert_eq!(budget.snapshot().unwrap().used, 0);
        assert_eq!(calls.0.load(Ordering::SeqCst), 0);
        assert_eq!(pool.available_operations(), 1);
        pool.shutdown().await.unwrap();
    }
}
#[tokio::test]
async fn direct_calls_require_explicit_finite_operation_policy_for_unbounded_source_timeout() {
    for value in [f64::INFINITY, f64::NAN] {
        let (adapter, calls, pool) = fixture(ArxivConfig {
            timeout_ms: Scalar::Float(value),
            ..Default::default()
        });
        let response = adapter.search(&SearchRequest::default()).await;
        assert_eq!(response.status, EngineStatus::Error);
        assert_eq!(
            response.error_message.as_deref(),
            Some("arXiv requires a finite execution deadline")
        );
        assert_eq!(calls.0.load(Ordering::SeqCst), 0);
        pool.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn actual_source_query_encoder_checks_its_own_real_deadline_after_charged_pending_polls() {
    let budget = Budget::new(16 * 1024 * 1024).unwrap();
    let query = "alpha beta ".repeat(150_000);
    let limit = Scalar::Integer(5);
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut future = Box::pin(scholarly_wire::arxiv_parameters_budgeted(
        &budget, &query, &limit, deadline,
    ));
    let mut context = Context::from_waker(Waker::noop());
    let mut observed = 0;
    for _ in 0..20000 {
        assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
        if budget.snapshot().unwrap().used > 0 {
            observed += 1;
            if observed == 16 {
                break;
            }
        }
    }
    assert_eq!(observed, 16);
    assert!(budget.snapshot().unwrap().used > 0);
    tokio::time::sleep_until(deadline.into()).await;
    assert!(matches!(future.await, Err(BudgetError::Deadline)));
    assert_eq!(budget.snapshot().unwrap().used, 0);
}

#[tokio::test]
async fn redirect_raw_copy_cancellation_and_real_deadline_restore_shared_budget() {
    for expire in [false, true] {
        let location = format!("https://{}Σ.example/next", "A\u{301}".repeat(131072));
        let budget = Budget::new(4 * 1024 * 1024).unwrap();
        let deadline = Instant::now() + Duration::from_secs(if expire { 2 } else { 30 });
        let mut future = Box::pin(redirect::target(
            "https://export.arxiv.org/api/query",
            &location,
            &budget,
            deadline,
        ));
        let mut context = Context::from_waker(Waker::noop());
        let mut observed = 0;
        for _ in 0..100000 {
            assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
            if budget.snapshot().unwrap().used > 0 {
                observed += 1;
                if observed == 16 {
                    break;
                }
            }
        }
        assert_eq!(observed, 16);
        if expire {
            tokio::time::sleep_until(deadline.into()).await;
            assert!(matches!(future.await, Err(Failure::Deadline)));
        } else {
            drop(future);
        }
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
}

#[tokio::test]
async fn source_invalid_base_url_is_transport_not_resource_before_http_or_parser() {
    let corpus: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../../docs/rust-rewrite/a06/source-arxiv-invalid-base.json"
    ))
    .unwrap();
    let row = &corpus["records"][0];
    assert_eq!(row["id"].as_str(), Some("arxiv.invalid_base_ipv6"));
    assert_eq!(row["requests"].as_array().unwrap().len(), 0);
    assert_eq!(
        row["output"]["error_message"].as_str(),
        Some("Invalid IPv6 address: '[invalid]'")
    );
    let config = ArxivConfig {
        base_url: Arc::from(row["config"]["base_url"].as_str().unwrap()),
        ..Default::default()
    };
    let (adapter, calls, pool) = fixture(config);
    let budget = Budget::new(1024 * 1024).unwrap();
    let actual = adapter
        .run_budgeted(
            &SearchRequest {
                query: row["query"].as_str().unwrap().to_owned(),
                ..Default::default()
            },
            Some(SearchContext {
                deadline: Instant::now() + Duration::from_secs(30),
            }),
            &budget,
        )
        .await;
    assert_eq!(actual.status, EngineStatus::Error);
    // Explicit F08 diagnostic boundary: retain source and native spellings.
    assert_eq!(
        actual.error_message.as_deref(),
        Some("arXiv invalid HTTP request")
    );
    assert_ne!(
        actual.error_message.as_deref(),
        row["output"]["error_message"].as_str()
    );
    assert_eq!(calls.0.load(Ordering::SeqCst), 0);
    assert_eq!(budget.snapshot().unwrap().used, 0);
    assert_eq!(pool.available_operations(), 1);
    assert_eq!(pool.available_slots(), 1);
    pool.shutdown().await.unwrap();
}

#[tokio::test]
async fn invalid_injected_response_status_is_transport_not_resource() {
    struct InvalidStatus(u16, Arc<AtomicUsize>);
    impl crate::http::Transport for InvalidStatus {
        fn exchange(&self, request: Request) -> crate::http::ExchangeFuture<'_> {
            self.1.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                Ok(Response {
                    status: self.0,
                    url: request.url,
                    headers: Default::default(),
                    body: Vec::new(),
                })
            })
        }
    }
    for status in [0, 99, 1000, u16::MAX] {
        let calls = Arc::new(AtomicUsize::new(0));
        let http = Arc::new(AdapterHttp::default());
        http.set_transport(Arc::new(InvalidStatus(status, calls.clone())))
            .unwrap();
        let pool = pool();
        let adapter = ArxivAdapter::new(
            http,
            Arc::new(super::super::AllowAllAdmission),
            ArxivConfig::default(),
            pool.clone(),
        );
        let budget = Budget::new(1024 * 1024).unwrap();
        let actual = adapter
            .run_budgeted(
                &SearchRequest::default(),
                Some(SearchContext {
                    deadline: Instant::now() + Duration::from_secs(30),
                }),
                &budget,
            )
            .await;
        assert_eq!(actual.status, EngineStatus::Error);
        assert_eq!(
            actual.error_message.as_deref(),
            Some("arXiv invalid HTTP response")
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(budget.snapshot().unwrap().used, 0);
        assert_eq!(pool.available_operations(), 1);
        assert_eq!(pool.available_slots(), 1);
        pool.shutdown().await.unwrap();
    }
}
