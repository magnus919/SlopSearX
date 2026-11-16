//! Exact original controls execute the full public adapter with a sealed owner clock.
use super::*;
use crate::{
    adapters::{
        AllowAllAdmission, europepmc::EuropePMCAdapter, pubchem::PubChemAdapter,
        pubmed::PubMedAdapter, uniprot::UniProtAdapter,
    },
    http as replay_http,
};
use ssx_application::SearchEngine;
use std::{
    future::Future,
    sync::atomic::{AtomicUsize, Ordering},
    task::Poll,
};
#[path = "a07_source_replay.rs"]
mod source_replay;

async fn full_source(id: &str) {
    assert!(source_replay::MOVED.contains(&id));
    let record = source_replay::source_record(id);
    assert_eq!(record["config"], ssx_json::json!({"timeout_ms":true}));
    let transport = source_replay::transport(&record);
    let (http, driver) = AdapterHttp::replay_owner(transport.clone());
    let origin = driver.now();
    let http = Arc::new(http);
    let diagnostics = tokio::time::timeout(
        Duration::from_secs(5),
        source_replay::replay_record(&record, http.clone(), transport.clone()),
    )
    .await
    .unwrap();
    assert!(diagnostics.is_empty());
    source_replay::assert_consumed(&transport);
    assert_eq!(driver.now(), origin, "only the driver advances owner time");
    assert_eq!(http.replay_workers(), (0, 0));
    assert_eq!(driver.owners(), 2, "only HTTP owner and test driver remain");
}
#[tokio::test]
async fn clinicaltrials_bool_true_full_source_replay() {
    full_source(source_replay::MOVED[0]).await;
}
#[tokio::test]
async fn europepmc_bool_true_full_source_replay() {
    full_source(source_replay::MOVED[1]).await;
}
#[tokio::test]
async fn pubchem_bool_true_full_source_replay() {
    full_source(source_replay::MOVED[2]).await;
}
#[tokio::test]
async fn uniprot_bool_true_full_source_replay() {
    full_source(source_replay::MOVED[3]).await;
}

struct Active<'a>(&'a AtomicUsize);
impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[derive(Default)]
struct Stall {
    started: tokio::sync::Notify,
    seen: std::sync::Mutex<Vec<crate::http::Request>>,
    dropped: AtomicUsize,
}
impl crate::http::Transport for Stall {
    fn exchange(&self, request: crate::http::Request) -> crate::http::ExchangeFuture<'_> {
        Box::pin(async move {
            self.seen.lock().unwrap().push(request);
            let _active = Active(&self.dropped);
            self.started.notify_one();
            std::future::pending().await
        })
    }
}
fn full_adapter(name: &str, http: Arc<AdapterHttp>) -> Box<dyn SearchEngine> {
    let mut config = BiomedicalConfig::defaults("https://example.invalid");
    config.timeout_ms = ssx_config::ConfigScalar::Boolean(true);
    let admission = Arc::new(AllowAllAdmission);
    match name {
        "clinicaltrials" => Box::new(ClinicalTrialsAdapter::new(http, admission, config)),
        "europepmc" => Box::new(EuropePMCAdapter::new(http, admission, config)),
        "pubchem" => Box::new(PubChemAdapter::new(http, admission, config)),
        "uniprot" => Box::new(UniProtAdapter::new(http, admission, config)),
        _ => panic!("bounded four-adapter fixture"),
    }
}
#[tokio::test]
async fn all_four_boolean_timers_expire_at_one_ms_after_one_confirmed_pending_exchange() {
    for name in ["clinicaltrials", "europepmc", "pubchem", "uniprot"] {
        let transport = Arc::new(Stall::default());
        let (http, driver) = AdapterHttp::replay_owner(transport.clone());
        let http = Arc::new(http);
        let adapter = full_adapter(name, http.clone());
        let request = SearchRequest {
            query: "test".into(),
            ..SearchRequest::default()
        };
        let config = BiomedicalConfig {
            timeout_ms: ssx_config::ConfigScalar::Boolean(true),
            ..BiomedicalConfig::defaults("https://example.invalid")
        };
        assert_eq!(
            network_deadline_for_http(&http, &config, None).unwrap() - driver.now(),
            Duration::from_millis(1)
        );
        let mut search = Box::pin(adapter.search(&request));
        tokio::time::timeout(Duration::from_secs(5),async{
            tokio::select!{
                result=&mut search=>panic!("{name}: completed before pending exchange {:?}",result.status),
                _=transport.started.notified()=>{}
            }
        }).await.unwrap();
        assert_eq!(transport.seen.lock().unwrap().len(), 1);
        driver.advance_to(Duration::from_micros(999));
        let state = std::future::poll_fn(|cx| Poll::Ready(search.as_mut().poll(cx))).await;
        assert!(state.is_pending(), "{name}: timer expired early");
        driver.advance_to(Duration::from_millis(1));
        let out = tokio::time::timeout(Duration::from_secs(5), search)
            .await
            .unwrap();
        assert_eq!(out.status, EngineStatus::Timeout, "{name}");
        assert!(out.results.is_empty());
        assert_eq!(
            out.error_message.as_deref(),
            if name == "europepmc" {
                Some("biomedical provider request timed out")
            } else {
                None
            }
        );
        assert_eq!(transport.seen.lock().unwrap().len(), 1);
        assert_eq!(transport.dropped.load(Ordering::SeqCst), 1);
        http.shutdown_and_wait().await;
        assert_eq!(http.replay_workers(), (0, 0));
        assert_eq!(driver.owners(), 2);
    }
}
#[tokio::test]
async fn owner_get_precheck_rejects_expiry_before_any_exchange() {
    let transport = Arc::new(Stall::default());
    let (http, driver) = AdapterHttp::replay_owner(transport.clone());
    let deadline = driver.now() + Duration::from_millis(1);
    let mut session = session(&http, deadline).unwrap();
    driver.advance_to(Duration::from_millis(1));
    let result = get(
        &mut session,
        request("https://example.invalid", &[], false, false).unwrap(),
        deadline,
        false,
        false,
        None,
    )
    .await;
    assert_eq!(result.unwrap_err().status, EngineStatus::Timeout);
    assert!(transport.seen.lock().unwrap().is_empty());
    drop(session);
    http.shutdown_and_wait().await;
    assert_eq!(http.replay_workers(), (0, 0));
    assert_eq!(driver.owners(), 2);
}
