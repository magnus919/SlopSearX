//! Native transport-stage controls, not new original-provider observations.
use super::*;
use base64::Engine;
use std::{
    future::Future,
    sync::atomic::{AtomicUsize, Ordering},
    task::Poll,
};

async fn within<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), future)
        .await
        .expect("owner-clock staging watchdog")
}
struct Captured {
    seen: Mutex<Vec<Request>>,
    status: u16,
    headers: HeaderMap,
    body: Vec<u8>,
}
impl Transport for Captured {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            let url = request.url.clone();
            self.seen.lock().unwrap().push(request);
            Ok(Response {
                status: self.status,
                url,
                headers: self.headers.clone(),
                body: self.body.clone(),
            })
        })
    }
}
fn captured() -> (Arc<Captured>, Request) {
    // Same typed boundary-corpus decode as vulnerability_tests::BoundaryCases:
    // retain source-only config/output text raw, including unpaired surrogates.
    #[derive(serde::Deserialize)]
    struct Cases {
        records: Vec<Case>,
    }
    #[derive(serde::Deserialize)]
    struct Case {
        id: String,
        config: Box<serde_json::value::RawValue>,
        exchanges: Vec<serde_json::Value>,
        #[serde(rename = "output")]
        _output: Option<Box<serde_json::value::RawValue>>,
    }
    let source: Cases = serde_json::from_str(include_str!(
        "../../../../docs/rust-rewrite/a10/source-boundaries-v2.json"
    ))
    .unwrap();
    let record = source
        .records
        .iter()
        .find(|r| r.id == "a10-boundary/cve/timeout-0.25")
        .unwrap();
    let config: serde_json::Value = serde_json::from_str(record.config.get()).unwrap();
    assert_eq!(config["timeout_ms"], 0.25);
    assert_eq!(record.exchanges.len(), 1);
    let exchange = &record.exchanges[0];
    let mut request = Request::get(exchange["request"]["url"].as_str().unwrap()).unwrap();
    request.method = exchange["request"]["method"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    for pair in exchange["request"]["headers"].as_array().unwrap() {
        request.headers.append(
            header::HeaderName::from_bytes(pair[0].as_str().unwrap().as_bytes()).unwrap(),
            pair[1].as_str().unwrap().parse().unwrap(),
        );
    }
    request.body = base64::engine::general_purpose::STANDARD
        .decode(exchange["request"]["body_base64"].as_str().unwrap())
        .unwrap();
    let mut headers = HeaderMap::new();
    for pair in exchange["response"]["headers"].as_array().unwrap() {
        headers.append(
            header::HeaderName::from_bytes(pair[0].as_str().unwrap().as_bytes()).unwrap(),
            pair[1].as_str().unwrap().parse().unwrap(),
        );
    }
    assert!(
        !headers.contains_key(header::CONTENT_ENCODING),
        "captured CVE uses real identity decoder"
    );
    let body = base64::engine::general_purpose::STANDARD
        .decode(exchange["response"]["body_base64"].as_str().unwrap())
        .unwrap();
    (
        Arc::new(Captured {
            seen: Mutex::new(vec![]),
            status: exchange["response"]["status"].as_u64().unwrap() as u16,
            headers,
            body,
        }),
        request,
    )
}
fn complete_response(transport: &Captured, request: &Request, response: Response) {
    assert_eq!(response.status, transport.status);
    assert_eq!(response.url, request.url);
    assert_eq!(response.headers, transport.headers);
    assert_eq!(response.body, transport.body);
    let seen = transport.seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].method, request.method);
    assert_eq!(seen[0].url, request.url);
    assert_eq!(seen[0].headers, request.headers);
    assert_eq!(seen[0].body, request.body);
}
async fn staged_session(manual: bool) {
    let (transport, request) = captured();
    // Pause before creating the owner's origin. Even host preemption during
    // later setup cannot put the System sleep past its deadline before staging.
    tokio::time::pause();
    let (http, driver) = if manual {
        let (owner, driver) = AdapterHttp::replay_owner(transport.clone());
        (owner, Some(driver))
    } else {
        let owner = AdapterHttp::default();
        owner.set_transport(transport.clone()).unwrap();
        (owner, None)
    };
    let permits = http
        .decoder
        .slots
        .clone()
        .acquire_many_owned(DECODE_CONCURRENCY as u32)
        .await
        .unwrap();
    // Pause only Tokio staging in this isolated test. No System c::check is
    // bypassed: this test enters the real Session directly, not an adapter.
    let deadline = http.deadline_now() + Duration::from_micros(250);
    let mut session = http
        .session(Options::default(), deadline.into(), Cancellation::default())
        .unwrap();
    let mut execute = Box::pin(session.execute(request.clone()));
    let state = std::future::poll_fn(|cx| Poll::Ready(execute.as_mut().poll(cx))).await;
    assert!(state.is_pending());
    assert_eq!(transport.seen.lock().unwrap().len(), 1, "response consumed");
    assert_eq!(http.decoder.state.lock().unwrap().active, 0);
    assert_eq!(http.decoder.slots.available_permits(), 0);
    println!(
        "A10_OWNER_STAGE manual={manual} site=post_exchange_decoder_admission requests={} active={} available_slots={}",
        transport.seen.lock().unwrap().len(),
        http.decoder.state.lock().unwrap().active,
        http.decoder.slots.available_permits()
    );
    // Captured::exchange has no suspension. Once it recorded its request, the
    // same execute poll can suspend only at Decoder::run's unavailable slots.
    let advance = tokio::time::Instant::from_std(deadline)
        .saturating_duration_since(tokio::time::Instant::now())
        + Duration::from_micros(1);
    tokio::time::advance(advance).await;
    assert!(tokio::time::Instant::now() >= deadline.into());
    if manual {
        let state = std::future::poll_fn(|cx| Poll::Ready(execute.as_mut().poll(cx))).await;
        assert!(
            state.is_pending(),
            "Manual owner used the System session timer"
        );
        assert_eq!(
            driver.as_ref().unwrap().now() + Duration::from_micros(250),
            deadline
        );
        tokio::time::resume();
        drop(permits);
        let response = within(execute).await.expect("Manual source HTTP response");
        complete_response(&transport, &request, response);
    } else {
        tokio::time::resume();
        assert_eq!(within(execute).await.unwrap_err(), Error::Deadline);
        drop(permits);
        assert_eq!(transport.seen.lock().unwrap().len(), 1);
    }
    drop(session);
    within(http.shutdown_and_wait()).await;
    assert_eq!(http.decoder.state.lock().unwrap().active, 0);
    if let Some(driver) = driver {
        assert_eq!(driver.owners(), 2);
    }
}
#[tokio::test]
async fn manual_session_survives_elapsed_tokio_after_confirmed_post_exchange_decoder_wait() {
    staged_session(true).await;
    println!("A10_OWNER_CLOCK_COMPLETE manual-session-after-decoder-wait");
}
#[tokio::test]
async fn system_session_expires_after_confirmed_post_exchange_decoder_wait() {
    staged_session(false).await;
    println!("A10_OWNER_CLOCK_COMPLETE system-session-expiry-after-decoder-wait");
}

struct Gate(Arc<(Mutex<bool>, std::sync::Condvar)>);
impl Gate {
    fn release(&self) {
        let (lock, notify) = &*self.0;
        *lock.lock().unwrap() = true;
        notify.notify_all();
    }
}
impl Drop for Gate {
    fn drop(&mut self) {
        self.release();
    }
}
#[tokio::test]
async fn identity_decoder_uses_owner_after_confirmed_worker_start_and_elapsed_wall_time() {
    let (transport, request) = captured();
    let (http, driver) = AdapterHttp::replay_owner(transport.clone());
    let started = Arc::new(Notify::new());
    let gate = Gate(Arc::new((Mutex::new(false), std::sync::Condvar::new())));
    let worker_started = started.clone();
    let worker_gate = gate.0.clone();
    *http.decoder.before_work.lock().unwrap() = Some(Arc::new(move || {
        worker_started.notify_one();
        let (lock, notify) = &*worker_gate;
        let mut open = lock.lock().unwrap();
        while !*open {
            open = notify.wait(open).unwrap();
        }
    }));
    let deadline = driver.now() + Duration::from_micros(250);
    let mut session = http
        .session(Options::default(), deadline.into(), Cancellation::default())
        .unwrap();
    let mut execute = Box::pin(session.execute(request.clone()));
    within(async {
        tokio::select! {
            result=&mut execute=>panic!("completed before identity worker gate: {result:?}"),
            _=started.notified()=>{}
        }
    })
    .await;
    assert_eq!(transport.seen.lock().unwrap().len(), 1);
    assert_eq!(http.decoder.state.lock().unwrap().active, 1);
    assert_eq!(
        driver.owners(),
        5,
        "HTTP, driver, Session, execute timer and blocking worker"
    );
    println!(
        "A10_OWNER_STAGE site=identity_worker_started requests={} active={} clock_owners={}",
        transport.seen.lock().unwrap().len(),
        http.decoder.state.lock().unwrap().active,
        driver.owners()
    );
    std::thread::sleep(Duration::from_millis(5));
    assert!(
        Instant::now() >= deadline,
        "wall delay must cross actual configured deadline"
    );
    assert_eq!(driver.now() + Duration::from_micros(250), deadline);
    println!(
        "A10_OWNER_DELAY site=identity_reader wall_expired={} owner_expired={}",
        Instant::now() >= deadline,
        driver.now() >= deadline
    );
    gate.release();
    let response = within(execute)
        .await
        .expect("Manual identity decoder source response");
    complete_response(&transport, &request, response);
    drop(session);
    within(http.shutdown_and_wait()).await;
    assert_eq!(http.decoder.state.lock().unwrap().active, 0);
    assert_eq!(driver.owners(), 2);
    drop(gate);
    drop(driver);
    drop(http);
    println!("A10_OWNER_CLOCK_COMPLETE identity-reader-wall-delay");
}

struct Active<'a>(&'a AtomicUsize);
impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[derive(Default)]
struct Pending {
    started: Notify,
    entered: AtomicUsize,
    dropped: AtomicUsize,
}
impl Transport for Pending {
    fn exchange(&self, _: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            self.entered.fetch_add(1, Ordering::SeqCst);
            let _active = Active(&self.dropped);
            self.started.notify_one();
            std::future::pending().await
        })
    }
}
#[tokio::test]
async fn manual_pending_exchange_expires_at_exact_250us_and_drops_once() {
    let transport = Arc::new(Pending::default());
    let (http, driver) = AdapterHttp::replay_owner(transport.clone());
    let deadline = driver.now() + Duration::from_micros(250);
    let mut session = http
        .session(Options::default(), deadline.into(), Cancellation::default())
        .unwrap();
    let mut execute = Box::pin(session.execute(Request::get("https://example.invalid").unwrap()));
    within(async {
        tokio::select! {
            result=&mut execute=>panic!("completed before pending exchange: {result:?}"),
            _=transport.started.notified()=>{}
        }
    })
    .await;
    driver.advance_to(Duration::from_micros(249));
    assert!(
        std::future::poll_fn(|cx| Poll::Ready(execute.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    driver.advance_to(Duration::from_micros(250));
    assert_eq!(within(execute).await.unwrap_err(), Error::Deadline);
    assert_eq!(transport.entered.load(Ordering::SeqCst), 1);
    assert_eq!(transport.dropped.load(Ordering::SeqCst), 1);
    drop(session);
    within(http.shutdown_and_wait()).await;
    assert_eq!(driver.owners(), 2);
    drop(driver);
    drop(http);
    println!("A10_OWNER_CLOCK_COMPLETE pending-exact-250us");
}
#[tokio::test]
async fn unmocked_system_expiry_and_expired_search_context_dispatch_nothing() {
    use ssx_application::SearchEngine;
    let http = AdapterHttp::default();
    assert!(!http.clock.is_manual());
    let deadline = Instant::now() - Duration::from_micros(250);
    let mut session = http
        .session(Options::default(), deadline.into(), Cancellation::default())
        .unwrap();
    assert_eq!(
        within(session.execute(Request::get("https://example.invalid").unwrap()))
            .await
            .unwrap_err(),
        Error::Deadline
    );
    assert!(
        session
            .pool
            .slots
            .lock()
            .unwrap()
            .iter()
            .all(|(busy, client)| !busy && client.is_none()),
        "expired unmocked HTTP never checks out/builds a client"
    );
    drop(session);
    within(http.shutdown_and_wait()).await;
    drop(http);
    let (transport, _) = captured();
    let http = Arc::new(AdapterHttp::default());
    http.set_transport(transport.clone()).unwrap();
    let mut settings = crate::adapters::cve::CveConfig::defaults(crate::adapters::cve::ENDPOINT);
    settings.timeout_ms = ssx_config::ConfigScalar::Float(0.25);
    let adapter = crate::adapters::cve::CveAdapter::new(
        http.clone(),
        Arc::new(crate::adapters::AllowAllAdmission),
        settings,
    );
    let request = ssx_domain::SearchRequest {
        query: "CVE-2024-12345".into(),
        ..Default::default()
    };
    let response =
        within(adapter.search_with_context(&request, ssx_application::SearchContext { deadline }))
            .await;
    assert_eq!(response.status, ssx_domain::EngineStatus::Timeout);
    assert!(response.results.is_empty());
    assert!(transport.seen.lock().unwrap().is_empty());
    drop(adapter);
    within(http.shutdown_and_wait()).await;
    drop(http);
    println!("A10_OWNER_CLOCK_COMPLETE system-unmocked-and-context-expiry");
}
#[tokio::test]
async fn manual_direct_replay_rejects_absolute_search_context() {
    use ssx_application::SearchEngine;
    let (transport, _) = captured();
    let (http, driver) = AdapterHttp::replay_owner(transport.clone());
    let http = Arc::new(http);
    let settings = crate::adapters::cve::CveConfig::defaults(crate::adapters::cve::ENDPOINT);
    let adapter = crate::adapters::cve::CveAdapter::new(
        http.clone(),
        Arc::new(crate::adapters::AllowAllAdmission),
        settings,
    );
    let request = ssx_domain::SearchRequest {
        query: "CVE-2024-12345".into(),
        ..Default::default()
    };
    let mut search = adapter.search_with_context(
        &request,
        ssx_application::SearchContext {
            deadline: Instant::now() + Duration::from_secs(60),
        },
    );
    let rejected = within(std::future::poll_fn(|cx| {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| search.as_mut().poll(cx))) {
            Ok(Poll::Pending) => Poll::Pending,
            Ok(Poll::Ready(response)) => panic!(
                "manual/context mixture returned {:?} instead of rejecting",
                response.status
            ),
            Err(reason) => Poll::Ready(reason),
        }
    }))
    .await;
    let message = rejected
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| rejected.downcast_ref::<&str>().copied())
        .unwrap_or("");
    assert!(message.contains("manual HTTP replay is direct-only"));
    assert!(transport.seen.lock().unwrap().is_empty());
    drop(search);
    drop(adapter);
    within(http.shutdown_and_wait()).await;
    assert_eq!(driver.owners(), 2);
    drop(driver);
    drop(http);
    println!("A10_OWNER_CLOCK_COMPLETE manual-context-refused");
}
