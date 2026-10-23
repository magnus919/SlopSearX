//! Cold Default remains charged to the real caller context. These new lifecycle
//! controls use their own one-second context; original70/80ms fixtures stay exact.
use super::*;
use crate::adapters::{
    AllowAllAdmission,
    clinicaltrials::{BiomedicalConfig, ClinicalTrialsAdapter},
    pubchem::PubChemAdapter,
    pubmed::PubMedAdapter,
};
use ssx_application::{SearchContext, SearchEngine};
use ssx_config::ConfigScalar;
use ssx_domain::{EngineStatus, SearchRequest};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::AsyncReadExt;

#[tokio::test]
async fn cold_default_context_expiry_retains_real_builder_until_owned_drain() {
    if super::default_ca_test_environment(
        "http::cold_context_tests::cold_default_context_expiry_retains_real_builder_until_owned_drain",
    ) {
        return;
    }
    for name in ["clinicaltrials", "pubchem", "pubmed"] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let accepted = Arc::new(AtomicUsize::new(0));
        let headers = Arc::new(AtomicUsize::new(0));
        let server_accepted = accepted.clone();
        let server_headers = headers.clone();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            server_accepted.fetch_add(1, Ordering::SeqCst);
            let mut bytes = Vec::new();
            loop {
                let mut chunk = [0; 4096];
                let count = stream.read(&mut chunk).await.unwrap();
                if count == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..count]);
                if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                    server_headers.fetch_add(1, Ordering::SeqCst);
                    break;
                }
            }
        });
        // No readiness call. Start from the unchanged cold production owner.
        let http = Arc::new(AdapterHttp::default());
        assert!(http.state.lock().unwrap().pools.is_empty());
        let (started, entered) = tokio::sync::oneshot::channel();
        let (release, held) = tokio::sync::oneshot::channel::<()>();
        let state = Mutex::new(Some((started, held)));
        let construction = Arc::new(AtomicUsize::new(0));
        let construction_result = construction.clone();
        *http.builders.build.lock().unwrap() = Some(Arc::new(move |options| {
            let (started, held) = state.lock().unwrap().take().expect("one cold construction");
            let _ = started.send(());
            // Sender drop also releases on test panic: no blocking-worker hang.
            let _ = held.blocking_recv();
            let result = super::build_client(options);
            construction_result.store(if result.is_ok() { 1 } else { 2 }, Ordering::SeqCst);
            result
        }));
        let mut config = BiomedicalConfig::defaults(&format!("http://{address}"));
        // New lifecycle fixture configuration; context is the operative limit.
        // No original source configuration or70/80ms assertion is modified.
        config.timeout_ms = ConfigScalar::Integer(10_000);
        let engine: Arc<dyn SearchEngine> = match name {
            "clinicaltrials" => Arc::new(ClinicalTrialsAdapter::new(
                http.clone(),
                Arc::new(AllowAllAdmission),
                config,
            )),
            "pubchem" => Arc::new(PubChemAdapter::new(
                http.clone(),
                Arc::new(AllowAllAdmission),
                config,
            )),
            "pubmed" => Arc::new(PubMedAdapter::new(
                http.clone(),
                Arc::new(AllowAllAdmission),
                config,
            )),
            _ => unreachable!(),
        };
        let context = SearchContext {
            deadline: Instant::now() + Duration::from_secs(1),
        };
        let task_engine = engine.clone();
        let task = tokio::spawn(async move {
            let request = SearchRequest {
                query: "cold context ownership".into(),
                ..Default::default()
            };
            task_engine.search_with_context(&request, context).await
        });
        tokio::time::timeout(Duration::from_secs(5), entered)
            .await
            .unwrap()
            .unwrap();
        assert!(
            !task.is_finished(),
            "builder entered before query completion {name}"
        );
        let pool = {
            let state = http.state.lock().unwrap();
            assert_eq!(state.pools.len(), 1);
            state.pools.values().next().unwrap().clone()
        };
        assert_eq!(http.connections.available_permits(), 19);
        assert_eq!(pool.permits.available_permits(), 19);
        assert!(pool.slots.lock().unwrap().iter().any(|(busy, _)| *busy));
        assert_eq!(http.replay_workers(), (0, 1));
        let response = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap();
        assert!(Instant::now() >= context.deadline);
        assert_eq!(response.status, EngineStatus::Timeout, "{name}");
        assert!(response.error_message.is_none());
        assert!(
            response.results.is_empty()
                && response.answers.is_empty()
                && response.corrections.is_empty()
                && response.infoboxes.is_empty()
        );
        assert!(!response.synthetic);
        assert_eq!(accepted.load(Ordering::SeqCst), 0);
        assert_eq!(headers.load(Ordering::SeqCst), 0);
        assert_eq!(construction.load(Ordering::SeqCst), 0);
        // An expired waiter does not release a still-running real builder lease.
        assert_eq!(http.connections.available_permits(), 19);
        assert_eq!(pool.permits.available_permits(), 19);
        let mut drain = Box::pin(http.shutdown_and_wait());
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(drain.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        assert_eq!(http.connections.available_permits(), 19);
        assert_eq!(pool.permits.available_permits(), 19);
        assert!(pool.slots.lock().unwrap().iter().any(|(busy, _)| *busy));
        release.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(5), drain)
            .await
            .unwrap();
        assert_eq!(construction.load(Ordering::SeqCst), 1);
        assert_eq!(http.replay_workers(), (0, 0));
        assert_eq!(http.connections.available_permits(), 20);
        assert_eq!(pool.permits.available_permits(), 20);
        assert!(pool.slots.lock().unwrap().iter().all(|(busy, _)| !*busy));
        assert_eq!(http.idle.available_permits(), 10);
        assert_eq!(accepted.load(Ordering::SeqCst), 0);
        assert_eq!(headers.load(Ordering::SeqCst), 0);
        drop(engine);
        assert_eq!(Arc::strong_count(&http), 1);
        server.abort();
        let _ = server.await;
    }
}
