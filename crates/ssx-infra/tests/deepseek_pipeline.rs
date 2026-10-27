//! Offline provider-contract tests. Inject the shared HTTP transport, never a provider key.
use reqwest::{
    Method,
    header::{HeaderMap, HeaderValue},
};
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchContext};
use ssx_config::{EngineConfig, Secret};
use ssx_domain::{EngineStatus, SearchRequest};
use ssx_infra::{adapters::register_deepseek, http::*, search::EngineBinding};
use ssx_registry::Registry;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Default)]
struct Wire {
    calls: Mutex<Vec<Request>>,
    replies: Mutex<VecDeque<(u16, Vec<u8>, HeaderMap)>>,
    hang: bool,
    network_failure: bool,
}
impl Transport for Wire {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            let url = request.url.clone();
            self.calls.lock().unwrap().push(request);
            if self.network_failure {
                return Err(Error::Network);
            }
            if self.hang {
                std::future::pending::<()>().await;
            }
            let (status, body, headers) = self.replies.lock().unwrap().pop_front().unwrap_or((
                200,
                br#"{"results":[]}"#.to_vec(),
                HeaderMap::new(),
            ));
            Ok(Response {
                status,
                body,
                headers,
                url,
            })
        })
    }
}
struct Admission {
    decision: AdmissionDecision,
    calls: Mutex<usize>,
    cooldown: Mutex<Vec<f64>>,
}
impl Default for Admission {
    fn default() -> Self {
        Self {
            decision: AdmissionDecision::Allowed,
            calls: Mutex::new(0),
            cooldown: Mutex::new(vec![]),
        }
    }
}
impl SearchAdmission for Admission {
    fn admit<'a>(&'a self, name: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async move {
            assert_eq!(name, "deepseek");
            *self.calls.lock().unwrap() += 1;
            self.decision
        })
    }
    fn set_upstream_cooldown<'a>(&'a self, name: &'static str, seconds: f64) -> PortFuture<'a, ()> {
        Box::pin(async move {
            assert_eq!(name, "deepseek");
            self.cooldown.lock().unwrap().push(seconds);
        })
    }
}
fn config() -> EngineConfig {
    let descriptor = Registry::builtins()
        .unwrap()
        .descriptor("deepseek")
        .unwrap()
        .clone();
    EngineConfig {
        enabled: true,
        base_url: descriptor.defaults.base_url,
        api_key: Secret::new(Some("synthetic-secret".into())),
        ..Default::default()
    }
}
fn request(query: &str) -> SearchRequest {
    SearchRequest {
        query: query.into(),
        ..Default::default()
    }
}
fn setup(
    config: &EngineConfig,
    wire: Arc<Wire>,
    admission: Arc<Admission>,
) -> (Arc<AdapterHttp>, EngineBinding) {
    let http = Arc::new(AdapterHttp::default());
    http.set_transport(wire).unwrap();
    let binding = register_deepseek(
        &mut Registry::builtins().unwrap(),
        http.clone(),
        admission,
        config,
        "test-generation-v1",
    )
    .unwrap()
    .unwrap();
    (http, binding)
}
fn reply(status: u16, body: &str) -> (u16, Vec<u8>, HeaderMap) {
    (status, body.as_bytes().to_vec(), HeaderMap::new())
}
#[tokio::test]
async fn exact_post_and_normalization_keep_snippet_positions_dates_and_tier() {
    let body = r#"{"results":[{"url":"https://example.test/a","title":"<script>x</script>","snippet":"display & text","summary":"never display","authority":4,"rerank_score":3,"date":"2026-10-27T12:00:00Z"},{"url":"javascript:alert(1)","title":"bad","snippet":"bad"},{"url":"https://example.test/b","title":"B","snippet":"b","date":"not-a-date"},{"url":"https://user:pass@example.test/","title":"credential URL","snippet":"bad"},{"url":"https://example.test/c","title":"C","snippet":"c"},null]}"#;
    let wire = Arc::new(Wire {
        replies: Mutex::new([reply(200, body)].into()),
        ..Default::default()
    });
    let (http, binding) = setup(&config(), wire.clone(), Arc::new(Admission::default()));
    let mut req = request("😀 site:example.test lang:en after:2025 -\"exact phrase\"");
    req.max_results = Some(1);
    req.client_identifier = Some("private-ip-or-client".into());
    let result = binding.engine.search(&req).await;
    assert_eq!(result.status, EngineStatus::Ok);
    assert_eq!(
        result
            .results
            .iter()
            .map(|r| r.position)
            .collect::<Vec<_>>(),
        [1, 3, 5]
    );
    assert_eq!(result.results[0].content, "display & text");
    assert_eq!(
        result.results[0].published_date.as_deref(),
        Some("2026-10-27T12:00:00Z")
    );
    assert!(result.results[1].published_date.is_none());
    assert!(result.results[2].published_date.is_none());
    for r in &result.results {
        assert_eq!(r.tier, 2);
        assert_eq!(r.engine, "deepseek");
        assert_eq!(r.category, "general");
        assert_eq!(r.score.get(), 0.);
    }
    let calls = wire.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    let sent = &calls[0];
    assert_eq!(sent.method, Method::POST);
    assert_eq!(
        sent.url.as_str(),
        "https://api.deepseek.com/v1/tools/search"
    );
    assert_eq!(sent.headers["authorization"], "Bearer synthetic-secret");
    assert!(sent.headers["authorization"].is_sensitive());
    assert!(!format!("{sent:?}").contains("synthetic-secret"));
    assert_eq!(sent.headers["content-type"], "application/json");
    let body: serde_json::Value = serde_json::from_slice(&sent.body).unwrap();
    assert_eq!(
        body,
        serde_json::json!({"query":req.query,"count":10,"need_content":false})
    );
    http.shutdown_and_wait().await;
}
#[tokio::test]
async fn query_count_and_pagination_boundaries_dispatch_only_valid_requests() {
    for query in [
        "".into(),
        " \n\t".into(),
        "😀".repeat(129),
        "e\u{301}".repeat(65),
    ] {
        let wire = Arc::new(Wire::default());
        let admission = Arc::new(Admission::default());
        let (http, binding) = setup(&config(), wire.clone(), admission.clone());
        assert_eq!(
            binding.engine.search(&request(&query)).await.status,
            EngineStatus::Error
        );
        assert!(wire.calls.lock().unwrap().is_empty());
        assert_eq!(*admission.calls.lock().unwrap(), 0);
        http.shutdown_and_wait().await;
    }
    for count in [1, 50] {
        for query in ["x".into(), "😀".repeat(128), "e\u{301}".repeat(64)] {
            let wire = Arc::new(Wire::default());
            let mut c = config();
            c.max_results = count.into();
            let (http, binding) = setup(&c, wire.clone(), Arc::new(Admission::default()));
            assert_eq!(
                binding.engine.search(&request(&query)).await.status,
                EngineStatus::Ok
            );
            let body: serde_json::Value =
                serde_json::from_slice(&wire.calls.lock().unwrap()[0].body).unwrap();
            assert_eq!(body["count"], count);
            http.shutdown_and_wait().await;
        }
    }
    let wire = Arc::new(Wire::default());
    let admission = Arc::new(Admission::default());
    let (http, binding) = setup(&config(), wire.clone(), admission.clone());
    let mut req = request("page");
    req.page = 2;
    let result = binding.engine.search(&req).await;
    assert_eq!(result.status, EngineStatus::Error);
    assert!(
        result
            .error_message
            .unwrap()
            .contains("pagination unsupported")
    );
    assert!(wire.calls.lock().unwrap().is_empty());
    assert_eq!(*admission.calls.lock().unwrap(), 0);
    http.shutdown_and_wait().await;
}
#[tokio::test]
async fn configuration_is_opt_in_validated_redacted_and_factory_backed() {
    let http = Arc::new(AdapterHttp::default());
    let mut registry = Registry::builtins().unwrap();
    let admission = Arc::new(Admission::default());
    let mut disabled = config();
    disabled.enabled = false;
    disabled.api_key = Secret::default();
    disabled.max_results = 0.into();
    assert!(
        register_deepseek(
            &mut registry,
            http.clone(),
            admission.clone(),
            &disabled,
            "test-generation-v1"
        )
        .unwrap()
        .is_none()
    );
    assert!(!registry.is_available("deepseek"));
    for generation in ["", "   "] {
        assert!(
            register_deepseek(
                &mut registry,
                http.clone(),
                admission.clone(),
                &config(),
                generation
            )
            .is_err()
        );
    }
    for key in [
        None,
        Some(""),
        Some("  "),
        Some("bad\r\nvalue"),
        Some(" key"),
        Some("é"),
    ] {
        let mut c = config();
        c.api_key = Secret::new(key.map(str::to_owned));
        assert!(
            register_deepseek(
                &mut registry,
                http.clone(),
                admission.clone(),
                &c,
                "test-generation-v1"
            )
            .is_err()
        );
    }
    for count in [0, 51, -1] {
        let mut c = config();
        c.max_results = count.into();
        assert!(
            register_deepseek(
                &mut registry,
                http.clone(),
                admission.clone(),
                &c,
                "test-generation-v1"
            )
            .is_err()
        );
    }
    for timeout in [0, -1, 300_001, i64::MAX] {
        let mut c = config();
        c.timeout_ms = timeout.into();
        assert!(
            register_deepseek(
                &mut registry,
                http.clone(),
                admission.clone(),
                &c,
                "test-generation-v1"
            )
            .is_err()
        );
    }
    assert!(!format!("{:?}", config()).contains("synthetic-secret"));
    assert!(
        !serde_json::to_string(&config())
            .unwrap()
            .contains("synthetic-secret")
    );
    let wire = Arc::new(Wire::default());
    http.set_transport(wire.clone()).unwrap();
    register_deepseek(
        &mut registry,
        http.clone(),
        admission.clone(),
        &config(),
        "test-generation-v1",
    )
    .unwrap();
    assert!(registry.is_available("deepseek"));
    assert_eq!(
        registry
            .instantiate("deepseek")
            .unwrap()
            .search(&request("factory"))
            .await
            .status,
        EngineStatus::Ok
    );
    assert!(
        register_deepseek(
            &mut registry,
            http.clone(),
            admission,
            &config(),
            "test-generation-v1"
        )
        .is_err()
    );
    http.shutdown_and_wait().await;
}
#[tokio::test]
async fn failures_do_not_echo_provider_bodies_and_only_transient_statuses_retry() {
    for (status, expected, attempts) in [
        (401, EngineStatus::Blocked, 1),
        (402, EngineStatus::Blocked, 1),
        (400, EngineStatus::Error, 1),
        (403, EngineStatus::Error, 1),
        (429, EngineStatus::RateLimited, 3),
        (408, EngineStatus::Error, 3),
        (500, EngineStatus::Error, 3),
        (504, EngineStatus::Error, 3),
    ] {
        let wire = Arc::new(Wire {
            replies: Mutex::new(
                (0..3)
                    .map(|_| reply(status, "synthetic-secret upstream body"))
                    .collect(),
            ),
            ..Default::default()
        });
        let admission = Arc::new(Admission::default());
        let (http, binding) = setup(&config(), wire.clone(), admission.clone());
        let result = binding.engine.search(&request("fault")).await;
        assert_eq!(result.status, expected, "{status}");
        assert_eq!(wire.calls.lock().unwrap().len(), attempts, "{status}");
        assert!(!format!("{result:?}").contains("synthetic-secret"));
        assert_eq!(
            admission.cooldown.lock().unwrap().len(),
            if status == 429 { 3 } else { 0 }
        );
        http.shutdown_and_wait().await;
    }
    for body in ["not JSON", "{}", r#"{"results":null}"#, r#"{"results":{}}"#] {
        let wire = Arc::new(Wire {
            replies: Mutex::new([reply(200, body)].into()),
            ..Default::default()
        });
        let (http, binding) = setup(&config(), wire.clone(), Arc::new(Admission::default()));
        assert_eq!(
            binding.engine.search(&request("malformed")).await.status,
            EngineStatus::Error
        );
        assert_eq!(wire.calls.lock().unwrap().len(), 1);
        http.shutdown_and_wait().await;
    }
}
#[tokio::test]
async fn retry_then_success_preserves_body_and_admits_each_attempt() {
    let wire = Arc::new(Wire {
        replies: Mutex::new([reply(500, "{}"), reply(200, r#"{"results":[]}"#)].into()),
        ..Default::default()
    });
    let admission = Arc::new(Admission::default());
    let (http, binding) = setup(&config(), wire.clone(), admission.clone());
    assert_eq!(
        binding.engine.search(&request("retry")).await.status,
        EngineStatus::Ok
    );
    assert_eq!(*admission.calls.lock().unwrap(), 2);
    let calls = wire.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].body, calls[1].body);
    http.shutdown_and_wait().await;
}
#[tokio::test]
async fn retry_after_and_caller_deadline_prevent_early_second_request() {
    let mut headers = HeaderMap::new();
    headers.insert("retry-after", HeaderValue::from_static("60"));
    let wire = Arc::new(Wire {
        replies: Mutex::new([(429, b"{}".to_vec(), headers)].into()),
        ..Default::default()
    });
    let admission = Arc::new(Admission::default());
    let (http, binding) = setup(&config(), wire.clone(), admission.clone());
    let result = binding
        .engine
        .search_with_context(
            &request("deadline"),
            SearchContext {
                deadline: Instant::now() + Duration::from_millis(100),
            },
        )
        .await;
    assert_eq!(result.status, EngineStatus::Timeout);
    assert_eq!(wire.calls.lock().unwrap().len(), 1);
    assert_eq!(*admission.cooldown.lock().unwrap(), [60.]);
    http.shutdown_and_wait().await;
}
#[tokio::test]
async fn timeout_and_cancelled_backoff_do_not_dispatch_later_attempts() {
    let wire = Arc::new(Wire {
        hang: true,
        ..Default::default()
    });
    let (http, binding) = setup(&config(), wire.clone(), Arc::new(Admission::default()));
    let result = binding
        .engine
        .search_with_context(
            &request("hang"),
            SearchContext {
                deadline: Instant::now() + Duration::from_millis(30),
            },
        )
        .await;
    assert_eq!(result.status, EngineStatus::Timeout);
    http.shutdown_and_wait().await;
    let wire = Arc::new(Wire {
        replies: Mutex::new([reply(500, "{}")].into()),
        ..Default::default()
    });
    let (http, binding) = setup(&config(), wire.clone(), Arc::new(Admission::default()));
    {
        let query = request("cancel");
        let future = binding.engine.search(&query);
        tokio::pin!(future);
        tokio::select! { _=&mut future=>panic!("retry completed before cancel"), _=tokio::time::sleep(Duration::from_millis(20))=>{} }
    } // The entire future is dropped, rather than merely dropping a pinned reference.
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(wire.calls.lock().unwrap().len(), 1);
    http.shutdown_and_wait().await;
}
#[tokio::test]
async fn denied_or_unavailable_admission_never_calls_provider() {
    for decision in [AdmissionDecision::Denied, AdmissionDecision::Unavailable] {
        let wire = Arc::new(Wire::default());
        let admission = Arc::new(Admission {
            decision,
            ..Default::default()
        });
        let (http, binding) = setup(&config(), wire.clone(), admission);
        assert_ne!(
            binding.engine.search(&request("deny")).await.status,
            EngineStatus::Ok
        );
        assert!(wire.calls.lock().unwrap().is_empty());
        http.shutdown_and_wait().await;
    }
}

#[tokio::test]
async fn transport_failure_oversize_and_redirects_are_bounded_without_retries() {
    let mut redirect = HeaderMap::new();
    redirect.insert(
        "location",
        HeaderValue::from_static("https://unexpected.example/"),
    );
    for wire in [
        Wire {
            network_failure: true,
            ..Default::default()
        },
        Wire {
            replies: Mutex::new([(200, vec![b'x'; 2 * 1024 * 1024 + 1], HeaderMap::new())].into()),
            ..Default::default()
        },
        Wire {
            replies: Mutex::new([(302, vec![], redirect)].into()),
            ..Default::default()
        },
    ] {
        let wire = Arc::new(wire);
        let (http, binding) = setup(&config(), wire.clone(), Arc::new(Admission::default()));
        assert_eq!(
            binding.engine.search(&request("bounded")).await.status,
            EngineStatus::Error
        );
        assert_eq!(wire.calls.lock().unwrap().len(), 1);
        http.shutdown_and_wait().await;
    }
}
#[tokio::test]
async fn clipped_retry_after_never_sends_an_early_retry() {
    let mut headers = HeaderMap::new();
    headers.insert("retry-after", HeaderValue::from_static("600"));
    let wire = Arc::new(Wire {
        replies: Mutex::new([(429, vec![], headers)].into()),
        ..Default::default()
    });
    let (http, binding) = setup(&config(), wire.clone(), Arc::new(Admission::default()));
    assert_eq!(
        binding.engine.search(&request("long delay")).await.status,
        EngineStatus::RateLimited
    );
    assert_eq!(wire.calls.lock().unwrap().len(), 1);
    http.shutdown_and_wait().await;
}

/// Faithful expiring store for the real F10 policy/adapter bridge. This test
/// proves cooldown coordination, not distributed Valkey quota correctness.
#[derive(Default)]
struct ExpiringRateStore {
    until: Mutex<Option<Instant>>,
    published: Mutex<Vec<f64>>,
    acquisitions: std::sync::atomic::AtomicUsize,
}
impl ssx_application::rate::RateStore for ExpiringRateStore {
    fn acquire<'a>(
        &'a self,
        engine: &'a ssx_domain::rate::EngineKey,
        _: u64,
    ) -> PortFuture<
        'a,
        std::result::Result<ssx_domain::rate::QuotaReceipt, ssx_domain::rate::RateFailure>,
    > {
        Box::pin(async move {
            assert_eq!(engine.as_str(), "deepseek");
            let count = self
                .acquisitions
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1;
            Ok(ssx_domain::rate::QuotaReceipt {
                allowed: true,
                count: count as u64,
                bucket_key: "offline-deepseek".into(),
                server_time: ssx_domain::ValkeyQuotaTime::from_milliseconds(0),
                remaining_ttl_ms: 1,
            })
        })
    }
    fn cooldown<'a>(
        &'a self,
        engine: &'a ssx_domain::rate::EngineKey,
    ) -> PortFuture<
        'a,
        std::result::Result<ssx_domain::rate::SharedCooldown, ssx_domain::rate::RateFailure>,
    > {
        Box::pin(async move {
            assert_eq!(engine.as_str(), "deepseek");
            let remaining = self.until.lock().unwrap().map_or(Duration::ZERO, |until| {
                until.saturating_duration_since(Instant::now())
            });
            Ok(if remaining.is_zero() {
                ssx_domain::rate::SharedCooldown::Missing
            } else {
                ssx_domain::rate::SharedCooldown::Active {
                    seconds: remaining.as_secs_f64().ceil() as u64,
                }
            })
        })
    }
    fn publish_cooldown<'a>(
        &'a self,
        engine: &'a ssx_domain::rate::EngineKey,
        seconds: f64,
    ) -> PortFuture<'a, std::result::Result<u64, ssx_domain::rate::RateFailure>> {
        Box::pin(async move {
            assert_eq!(engine.as_str(), "deepseek");
            self.published.lock().unwrap().push(seconds);
            // DistributedQuota publishes whole-second TTLs through math.ceil.
            *self.until.lock().unwrap() =
                Some(Instant::now() + Duration::from_secs_f64(seconds.ceil()));
            Ok(seconds.ceil() as u64)
        })
    }
    fn warmup(&self) -> PortFuture<'_, std::result::Result<(), ssx_domain::rate::RateFailure>> {
        Box::pin(async { Ok(()) })
    }
    fn shutdown(&self) -> PortFuture<'_, ()> {
        Box::pin(async {})
    }
}
#[tokio::test]
async fn retry_delay_matches_actual_f10_minimum_and_whole_second_shared_cooldowns() {
    for (retry_after, expected_seconds) in [(None, 1.0), (Some("1.01"), 2.0)] {
        let store = Arc::new(ExpiringRateStore::default());
        let limiter = Arc::new(tokio::sync::Mutex::new(
            ssx_application::rate::RateLimiter::new(store.clone(), true),
        ));
        let admission = Arc::new(ssx_infra::adapters::F10Admission::new(limiter));
        let mut first = reply(429, "{}");
        if let Some(value) = retry_after {
            first
                .2
                .insert("retry-after", HeaderValue::from_str(value).unwrap());
        }
        let wire = Arc::new(Wire {
            replies: Mutex::new([first, reply(200, r#"{"results":[]}"#)].into()),
            ..Default::default()
        });
        let http = Arc::new(AdapterHttp::default());
        http.set_transport(wire.clone()).unwrap();
        let binding = register_deepseek(
            &mut Registry::builtins().unwrap(),
            http.clone(),
            admission,
            &config(),
            "f10-cooldown-regression",
        )
        .unwrap()
        .unwrap();
        let outcome = binding
            .engine
            .search(&request("rate-limited then successful"))
            .await;
        http.shutdown_and_wait().await;
        assert_eq!(
            outcome.status,
            EngineStatus::Ok,
            "Retry-After: {retry_after:?}"
        );
        let calls = wire.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert!(calls.iter().all(|call| call.method == Method::POST));
        assert_eq!(calls[0].body, calls[1].body);
        assert_eq!(calls[0].url, calls[1].url);
        assert_eq!(
            store.acquisitions.load(std::sync::atomic::Ordering::SeqCst),
            2
        );
        assert_eq!(*store.published.lock().unwrap(), [expected_seconds]);
    }
}
