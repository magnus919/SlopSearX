//! Actual factory timer policy; original MockTransport successes stay immutable.
use crate::adapters::AllowAllAdmission;
use crate::adapters::a06_case;
use crate::{
    adapters::{
        arxiv_expat::AtomPool,
        arxiv_runtime::{self, ArxivConfig},
        openalex_runtime::{self, OpenAlexConfig},
        scholarly_config::Scalar,
        semantic_runtime::{self, SemanticConfig},
    },
    http::AdapterHttp,
};
use ssx_application::SearchContext;
use ssx_domain::{EngineStatus, SearchRequest};
use ssx_json::Value;
use std::{
    num::NonZeroUsize,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
fn scalar(v: &Value) -> Option<Scalar> {
    Some(match v {
        Value::Bool(v) => Scalar::Boolean(*v),
        Value::String(v) => Scalar::Text(Arc::from(v.as_str())),
        Value::Number(v) if !v.is_f64() => Scalar::Integer(v.as_str().parse().ok()?),
        Value::Number(v) => Scalar::Float(v.as_str().parse().ok()?),
        Value::Object(v) => Scalar::Float(match v.get("$source_nonfinite_float")?.as_str()? {
            "nan" => f64::NAN,
            "inf" => f64::INFINITY,
            "-inf" => f64::NEG_INFINITY,
            _ => return None,
        }),
        _ => return None,
    })
}
#[tokio::test]
async fn controlled_all_42_timeout_scalars_full_factory_pending_exchange() {
    matrix(true).await;
}
#[tokio::test]
async fn real_system_all_42_timeout_scalars_full_factory_observed_expiry() {
    matrix(false).await;
}
async fn matrix(controlled: bool) {
    let corpus: Value = ssx_json::from_str(include_str!(
        "../../../../docs/rust-rewrite/a06/source-config.json"
    ))
    .unwrap();
    let mut ids = std::collections::BTreeSet::new();
    for row in corpus["records"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        if !id.contains(".timeout_ms.")
            || row["native_reachability"]
                .as_str()
                .unwrap()
                .starts_with("direct-source-only")
        {
            continue;
        }
        assert!(ids.insert(id));
        let timer = scalar(&row["config"]["timeout_ms"]).unwrap();
        let millis = match &timer {
            Scalar::Boolean(v) => f64::from(u8::from(*v)),
            Scalar::Integer(v) => *v as f64,
            Scalar::Float(v) => *v,
            Scalar::Text(_) => f64::NAN,
        };
        for with_context in [false, true] {
            let pool = AtomPool::new(
                PathBuf::from("/nonexistent-timeout-no-parser-dispatch"),
                NonZeroUsize::new(1).unwrap(),
            )
            .unwrap();
            let (http, transport, driver) = AdapterHttp::pending_replay_owner(controlled);
            let http = Arc::new(http);
            let mut registry = ssx_registry::Registry::builtins().unwrap();
            let admission = Arc::new(AllowAllAdmission);
            let name = row["engine"].as_str().unwrap();
            match name {
                "arxiv" => arxiv_runtime::register(
                    &mut registry,
                    http.clone(),
                    admission,
                    ArxivConfig {
                        timeout_ms: timer.clone(),
                        ..Default::default()
                    },
                    pool.clone(),
                )
                .unwrap(),
                "openalex" => openalex_runtime::register(
                    &mut registry,
                    http.clone(),
                    admission,
                    OpenAlexConfig {
                        timeout_ms: timer.clone(),
                        ..Default::default()
                    },
                    pool.clone(),
                )
                .unwrap(),
                "semanticscholar" => semantic_runtime::register(
                    &mut registry,
                    http.clone(),
                    admission,
                    SemanticConfig {
                        timeout_ms: timer.clone(),
                        ..Default::default()
                    },
                    pool.clone(),
                )
                .unwrap(),
                _ => panic!("unexpected engine"),
            }
            let engine = registry.instantiate(name).unwrap();
            let request = SearchRequest {
                query: "offline config".into(),
                ..Default::default()
            };
            let context = if with_context {
                Some(if let Some(driver) = &driver {
                    transport.context(driver, Duration::from_millis(30))
                } else {
                    SearchContext {
                        deadline: Instant::now() + Duration::from_millis(30),
                    }
                })
            } else {
                None
            };
            let text = matches!(timer, Scalar::Text(_));
            let nonpositive = matches!(
                millis.partial_cmp(&0.),
                Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
            );
            let finite_required = !text && !millis.is_finite() && !nonpositive && !with_context;
            let eligible = !text && !finite_required && !nonpositive;
            let mut response_future = if let Some(context) = context {
                engine.search_with_context(&request, context)
            } else {
                engine.search(&request)
            };
            if let Some(driver) = &driver
                && eligible
            {
                tokio::time::timeout(Duration::from_secs(5), async {
                        tokio::select! { biased;
                            response=&mut response_future=>panic!("completed before actual pending exchange: {id} {response:?}"),
                            _=transport.started_signal()=>{}
                        }
                    }).await.expect("real fixture handshake watchdog");
                let (network, operation) = transport.deadlines();
                let expiry = network.min(operation);
                let origin = driver.now();
                let duration = expiry.duration_since(origin);
                assert!(duration > Duration::ZERO, "positive fixture interval {id}");
                driver.advance_to(duration - Duration::from_nanos(1));
                let pending = std::future::poll_fn(|cx| {
                    std::task::Poll::Ready(std::future::Future::poll(response_future.as_mut(), cx))
                })
                .await;
                assert!(
                    pending.is_pending(),
                    "finished before original deadline {id}"
                );
                assert_eq!(transport.started(), 1);
                assert_eq!(transport.dropped(), 0);
                driver.advance_to(duration);
            }
            let response = tokio::time::timeout(Duration::from_secs(5), response_future)
                .await
                .expect("real fixture completion watchdog");
            let expected_dispatch = usize::from(eligible);
            let actual = transport.started();
            if controlled || !eligible {
                assert_eq!(actual, expected_dispatch, "{id} context={with_context}");
            } else {
                assert!(actual <= 1, "{id}");
                if actual == 0 {
                    let (deadline,observed)=transport.hit().expect("zero-dispatch positive Timeout requires actual owner deadline-hit observation");
                    assert!(observed >= deadline, "no invented expiry {id}");
                }
            }
            assert_eq!(
                transport.dropped(),
                actual,
                "pending exchange released {id}"
            );
            assert!(
                response.answers.is_empty()
                    && response.corrections.is_empty()
                    && response.infoboxes.is_empty()
            );
            assert!(!response.synthetic);
            assert!(response.results.is_empty());
            if text {
                assert_eq!(response.status, EngineStatus::Error, "{id}");
                assert_eq!(
                    response.error_message.as_deref(),
                    row["output"]["error_message"].as_str(),
                    "{id}"
                );
            } else if finite_required {
                assert_eq!(response.status, EngineStatus::Error, "{id}");
                assert!(
                    response
                        .error_message
                        .as_deref()
                        .unwrap()
                        .contains("requires a finite execution deadline")
                );
            } else {
                assert_eq!(response.status, EngineStatus::Timeout, "{id}");
                assert!(response.error_message.is_none(), "{id}");
                assert_eq!(
                    row["output"]["status"].as_str(),
                    Some("ok"),
                    "immutable mock result differs explicitly"
                );
            }
            assert_eq!(pool.available_slots(), 1);
            assert_eq!(pool.available_operations(), 1);
            pool.shutdown().await.unwrap();
            http.shutdown_and_wait().await;
            a06_case::completed(
                if controlled {
                    "ssx-infra/lib/http::scholarly_timeout_clock::controlled_all_42_timeout_scalars_full_factory_pending_exchange"
                } else {
                    "ssx-infra/lib/http::scholarly_timeout_clock::real_system_all_42_timeout_scalars_full_factory_observed_expiry"
                },
                "docs/rust-rewrite/a06/source-config.json",
                id,
                if with_context {
                    if controlled {
                        "controlled_finite_context"
                    } else {
                        "finite_context"
                    }
                } else {
                    if controlled {
                        "controlled_direct"
                    } else {
                        "direct"
                    }
                },
                if text {
                    "exact_operand_error"
                } else if finite_required {
                    "finite_policy_refusal"
                } else {
                    "deadline_policy"
                },
            );
        }
    }
    assert_eq!(ids.len(), 42);
    let ledger: Value = ssx_json::from_str(include_str!(
        "../../../../docs/rust-rewrite/a06/timeout-config-classification.json"
    ))
    .unwrap();
    assert_eq!(
        ids,
        ledger["unique_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect()
    );
}

#[tokio::test]
async fn real_system_one_ms_preparation_expiry_does_not_promise_dispatch() {
    for name in ["arxiv", "openalex", "semanticscholar"] {
        for with_context in [false, true] {
            let pool = AtomPool::new(
                PathBuf::from("/nonexistent-real-system-early-expiry"),
                NonZeroUsize::new(1).unwrap(),
            )
            .unwrap();
            let (http, observer, driver) = AdapterHttp::pending_replay_owner(false);
            assert!(driver.is_none());
            let http = Arc::new(http);
            let mut registry = ssx_registry::Registry::builtins().unwrap();
            let admission = Arc::new(AllowAllAdmission);
            match name {
                "arxiv" => arxiv_runtime::register(
                    &mut registry,
                    http.clone(),
                    admission,
                    ArxivConfig {
                        timeout_ms: Scalar::Boolean(true),
                        ..Default::default()
                    },
                    pool.clone(),
                )
                .unwrap(),
                "openalex" => openalex_runtime::register(
                    &mut registry,
                    http.clone(),
                    admission,
                    OpenAlexConfig {
                        timeout_ms: Scalar::Boolean(true),
                        ..Default::default()
                    },
                    pool.clone(),
                )
                .unwrap(),
                _ => semantic_runtime::register(
                    &mut registry,
                    http.clone(),
                    admission,
                    SemanticConfig {
                        timeout_ms: Scalar::Boolean(true),
                        ..Default::default()
                    },
                    pool.clone(),
                )
                .unwrap(),
            }
            let engine = registry.instantiate(name).unwrap();
            let request = SearchRequest {
                query: "offline config".into(),
                ..Default::default()
            };
            let mut future = if with_context {
                engine.search_with_context(
                    &request,
                    SearchContext {
                        deadline: Instant::now() + Duration::from_millis(30),
                    },
                )
            } else {
                engine.search(&request)
            };
            let first = std::future::poll_fn(|cx| {
                std::task::Poll::Ready(std::future::Future::poll(future.as_mut(), cx))
            })
            .await;
            assert_eq!(
                observer.started(),
                0,
                "first cooperative preparation poll {name}"
            );
            let response = match first {
                std::task::Poll::Ready(response) => response,
                std::task::Poll::Pending => {
                    // Intentionally expire the unchanged1ms real limit while the
                    // actual preparation future is held; not a latency threshold.
                    std::thread::sleep(Duration::from_millis(5));
                    tokio::time::timeout(Duration::from_secs(5), future)
                        .await
                        .unwrap()
                }
            };
            assert_eq!(response.status, EngineStatus::Timeout);
            assert!(response.error_message.is_none());
            assert!(response.results.is_empty());
            assert_eq!(observer.started(), 0);
            assert_eq!(observer.dropped(), 0);
            let (deadline, now) = observer.hit().expect("actual real deadline hit");
            assert!(now >= deadline);
            assert_eq!(pool.available_operations(), 1);
            assert_eq!(pool.available_slots(), 1);
            pool.shutdown().await.unwrap();
            http.shutdown_and_wait().await;
        }
    }
}
#[tokio::test]
async fn sealed_pending_owner_rejects_replacement_context_and_worker_entry() {
    struct Reply;
    impl crate::http::Transport for Reply {
        fn exchange(&self, _: crate::http::Request) -> crate::http::ExchangeFuture<'_> {
            panic!("replacement transport must never be installed")
        }
    }
    let (http, observer, driver) = AdapterHttp::pending_replay_owner(true);
    assert!(http.set_transport(Arc::new(Reply)).is_err());
    let (unsupported, unsupported_driver) = AdapterHttp::replay_owner(Arc::new(Reply));
    let unsupported_pool = AtomPool::new(
        PathBuf::from("/nonexistent-general-manual"),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap();
    let unsupported_deadline = unsupported_driver.now() + Duration::from_secs(1);
    let mut unsupported_admission = Box::pin(unsupported_pool.begin_raw_for_http(
        ssx_owned_budget::Budget::new(512 * 1024 * 1024).unwrap(),
        unsupported_deadline.into(),
        unsupported_deadline.into(),
        &unsupported,
    ));
    // Async construction does not enter the owner check: poll it inside the
    // panic boundary so the original forbidden-owner assertion still executes.
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
            std::future::Future::poll(unsupported_admission.as_mut(), &mut cx)
        }))
        .is_err()
    );
    drop(unsupported_admission);
    unsupported_pool.shutdown().await.unwrap();
    unsupported.shutdown_and_wait().await;
    let (_, _, other) = AdapterHttp::pending_replay_owner(true);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || observer.context(other.as_ref().unwrap(), Duration::from_millis(30))
        ))
        .is_err()
    );
    let pool = AtomPool::new(
        PathBuf::from("/nonexistent-manual-worker-forbidden"),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap();
    let deadline = driver.as_ref().unwrap().now() + Duration::from_secs(1);
    let operation = pool
        .begin_raw_for_http(
            ssx_owned_budget::Budget::new(512 * 1024 * 1024).unwrap(),
            deadline.into(),
            deadline.into(),
            &http,
        )
        .await
        .unwrap();
    let child = tokio::spawn(async move { operation.parse(std::iter::empty(), b"").await });
    assert!(child.await.unwrap_err().is_panic());
    assert_eq!(observer.started(), 0);
    assert_eq!(pool.available_slots(), 1);
    assert_eq!(pool.available_operations(), 1);
    pool.shutdown().await.unwrap();
    http.shutdown_and_wait().await;
}

#[tokio::test]
async fn pending_owner_early_tick_isolation_and_caller_drop_are_bounded() {
    let (http, observer, driver) = AdapterHttp::pending_replay_owner(true);
    let driver = driver.unwrap();
    let (other_http, other_observer, other_driver) = AdapterHttp::pending_replay_owner(true);
    let other_driver = other_driver.unwrap();
    let original = driver.now();
    other_driver.advance_to(Duration::from_secs(7));
    assert_eq!(driver.now(), original);
    let http = Arc::new(http);
    let pool = AtomPool::new(
        PathBuf::from("/nonexistent-caller-drop-worker"),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap();
    let mut registry = ssx_registry::Registry::builtins().unwrap();
    arxiv_runtime::register(
        &mut registry,
        http.clone(),
        Arc::new(AllowAllAdmission),
        ArxivConfig {
            timeout_ms: Scalar::Boolean(true),
            ..Default::default()
        },
        pool.clone(),
    )
    .unwrap();
    let engine = registry.instantiate("arxiv").unwrap();
    let request = SearchRequest {
        query: "offline config".into(),
        ..Default::default()
    };
    let mut future = engine.search(&request);
    let first = std::future::poll_fn(|cx| {
        std::task::Poll::Ready(std::future::Future::poll(future.as_mut(), cx))
    })
    .await;
    assert!(first.is_pending());
    assert_eq!(pool.available_operations(), 0);
    assert_eq!(observer.started(), 0);
    drop(future);
    assert_eq!(pool.available_operations(), 1);
    assert_eq!(pool.available_slots(), 1);
    // Tick already occurred before first sleep subscription/public factory poll.
    let context = observer.context(&driver, Duration::from_millis(1));
    driver.advance_to(Duration::from_millis(1));
    let response = tokio::time::timeout(
        Duration::from_secs(5),
        engine.search_with_context(&request, context),
    )
    .await
    .unwrap();
    assert_eq!(response.status, EngineStatus::Timeout);
    assert!(response.error_message.is_none());
    assert_eq!(observer.started(), 0);
    assert_eq!(pool.available_operations(), 1);
    assert_eq!(pool.available_slots(), 1);
    pool.shutdown().await.unwrap();
    http.shutdown_and_wait().await;
    drop(engine);
    drop(registry);
    drop(http);
    drop(observer);
    assert_eq!(driver.owners(), 1);
    other_http.shutdown_and_wait().await;
    drop(other_http);
    drop(other_observer);
    assert_eq!(other_driver.owners(), 1);
}

/// Causal detector for using System at any of the three query-preparation seams.
/// The unchanged 1ms System timestamp expires while the actual preparation is
/// held at its first cooperative yield; Manual remains before that deadline.
#[tokio::test]
async fn manual_owner_preparation_survives_real_expiry_for_all_three_factories() {
    for name in ["arxiv", "openalex", "semanticscholar"] {
        for with_context in [false, true] {
            let pool = AtomPool::new(
                PathBuf::from("/nonexistent-manual-preparation-no-parser"),
                NonZeroUsize::new(1).unwrap(),
            )
            .unwrap();
            let (http, observer, driver) = AdapterHttp::pending_replay_owner(true);
            let driver = driver.unwrap();
            let http = Arc::new(http);
            let mut registry = ssx_registry::Registry::builtins().unwrap();
            let admission = Arc::new(AllowAllAdmission);
            match name {
                "arxiv" => arxiv_runtime::register(
                    &mut registry,
                    http.clone(),
                    admission,
                    ArxivConfig {
                        timeout_ms: Scalar::Boolean(true),
                        ..Default::default()
                    },
                    pool.clone(),
                )
                .unwrap(),
                "openalex" => openalex_runtime::register(
                    &mut registry,
                    http.clone(),
                    admission,
                    OpenAlexConfig {
                        timeout_ms: Scalar::Boolean(true),
                        ..Default::default()
                    },
                    pool.clone(),
                )
                .unwrap(),
                _ => semantic_runtime::register(
                    &mut registry,
                    http.clone(),
                    admission,
                    SemanticConfig {
                        timeout_ms: Scalar::Boolean(true),
                        ..Default::default()
                    },
                    pool.clone(),
                )
                .unwrap(),
            }
            let engine = registry.instantiate(name).unwrap();
            let request = SearchRequest {
                query: "offline preparation clock".into(),
                ..Default::default()
            };
            let origin = driver.now();
            let mut future = if with_context {
                engine.search_with_context(
                    &request,
                    observer.context(&driver, Duration::from_millis(30)),
                )
            } else {
                engine.search(&request)
            };
            // Poll actual public factory once, acquiring its original operation
            // lease and stopping at query preparation's first existing yield.
            let first = std::future::poll_fn(|cx| {
                std::task::Poll::Ready(std::future::Future::poll(future.as_mut(), cx))
            })
            .await;
            assert!(
                first.is_pending(),
                "first preparation poll {name} context={with_context}"
            );
            assert_eq!(observer.started(), 0);
            assert_eq!(observer.dropped(), 0);
            assert_eq!(pool.available_operations(), 0);
            assert_eq!(pool.available_slots(), 1);
            let (network, operation) = observer.deadlines();
            assert_eq!(network, origin + Duration::from_millis(1));
            assert_eq!(
                operation,
                origin
                    + if with_context {
                        Duration::from_millis(30)
                    } else {
                        Duration::from_millis(1)
                    }
            );
            // Wait for the actual unchanged System timestamp, without polling
            // preparation or advancing this owner's Manual clock. The real
            // watchdog bounds the fixture; elapsed time is not a pass threshold.
            tokio::time::timeout(Duration::from_secs(5), async {
                while Instant::now() <= network {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("real System timestamp expiry watchdog");
            assert!(Instant::now() > network);
            assert_eq!(driver.now(), origin);
            assert!(driver.now() < network);
            assert!(observer.hit().is_none());
            // A reverted System preparation check completes with Timeout here,
            // before any exchange, and must fail this causal detector.
            tokio::time::timeout(Duration::from_secs(5), async {
                tokio::select! { biased;
                    response = &mut future => panic!("preparation used a foreign/System clock: {name} context={with_context} {response:?}"),
                    _ = observer.started_signal() => {}
                }
            }).await.expect("actual pending exchange watchdog");
            assert_eq!(observer.started(), 1);
            assert_eq!(observer.dropped(), 0);
            driver.advance_to(Duration::from_millis(1) - Duration::from_nanos(1));
            let before = std::future::poll_fn(|cx| {
                std::task::Poll::Ready(std::future::Future::poll(future.as_mut(), cx))
            })
            .await;
            assert!(before.is_pending(), "original Manual deadline not reached");
            assert_eq!(observer.dropped(), 0);
            driver.advance_to(Duration::from_millis(1));
            let response = tokio::time::timeout(Duration::from_secs(5), future)
                .await
                .expect("exact Manual expiry watchdog");
            assert_eq!(response.status, EngineStatus::Timeout);
            assert!(response.error_message.is_none());
            assert!(
                response.results.is_empty()
                    && response.answers.is_empty()
                    && response.corrections.is_empty()
                    && response.infoboxes.is_empty()
            );
            assert!(!response.synthetic);
            assert_eq!(observer.started(), 1);
            assert_eq!(observer.dropped(), 1);
            let (hit, observed) = observer.hit().expect("actual Manual expiry trace");
            assert_eq!(hit, network);
            assert_eq!(observed, network);
            assert_eq!(pool.available_operations(), 1);
            assert_eq!(pool.available_slots(), 1);
            pool.shutdown().await.unwrap();
            http.shutdown_and_wait().await;
            drop(engine);
            drop(registry);
            drop(http);
            drop(observer);
            assert_eq!(driver.owners(), 1);
        }
    }
}
