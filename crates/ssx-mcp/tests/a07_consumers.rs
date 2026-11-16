//! Actual A07 parsers -> accepted ranking -> accepted compact-card projection.
//! Distinct PubMed/Europe PMC URLs preserve attribution; this does not claim
//! first-wins same-URL snippets somehow gain attribution from a later feed.
use ssx_application::{
    SearchEngine,
    ranking::{Feeds, rank},
};
use ssx_domain::{EngineStatus, SearchRequest};
use ssx_infra::{
    adapters::{
        AllowAllAdmission, clinicaltrials::BiomedicalConfig, europepmc::EuropePMCAdapter,
        pubmed::PubMedAdapter,
    },
    http::{AdapterHttp, ExchangeFuture, Request, Response, Transport},
};
use ssx_mcp::projection::result_card;
use ssx_wire::McpRetrievalDto;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
};
struct Recorded {
    replies: Mutex<VecDeque<Vec<u8>>>,
    seen: Mutex<Vec<Request>>,
}
impl Transport for Recorded {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            let url = request.url.clone();
            self.seen.lock().unwrap().push(request);
            let body = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .ok_or(ssx_infra::http::Error::RequestLimit)?;
            Ok(Response {
                status: 200,
                url,
                headers: Default::default(),
                body,
            })
        })
    }
}
fn http(bodies: Vec<Vec<u8>>) -> (Arc<AdapterHttp>, Arc<Recorded>) {
    let recorded = Arc::new(Recorded {
        replies: Mutex::new(bodies.into()),
        seen: Mutex::new(vec![]),
    });
    let http = Arc::new(AdapterHttp::default());
    http.set_transport(recorded.clone()).unwrap();
    (http, recorded)
}
fn retrieval() -> McpRetrievalDto {
    McpRetrievalDto {
        contract: "slopsearx.retrieval_handoff".into(),
        version: 1,
        eligible: true,
        url_status: "ok".into(),
        url_reason: None,
        scheme: Some("https".into()),
    }
}
#[tokio::test]
async fn actual_preprint_adapter_rank_and_compact_card_retain_attribution_notice_and_unicode_cap() {
    let (body, requests) = http(vec![
        include_bytes!("../../../docs/rust-rewrite/a07/fixtures/europepmc-long-preprint.json")
            .to_vec(),
    ]);
    let adapter = EuropePMCAdapter::new(
        body.clone(),
        Arc::new(AllowAllAdmission),
        BiomedicalConfig::defaults("https://www.ebi.ac.uk/europepmc/webservices/rest"),
    );
    let response = adapter
        .search(&SearchRequest {
            query: "test".into(),
            ..Default::default()
        })
        .await;
    assert_eq!(response.status, EngineStatus::Ok);
    let mut feeds = Feeds::new();
    feeds.insert("europepmc".into(), response.results);
    let rows = rank(&feeds, "presence", &BTreeMap::new());
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert!(row.content.chars().count() > 300);
    let card = result_card(row, None, retrieval());
    assert_eq!(card.snippet.chars().count(), 300);
    assert!(
        card.snippet
            .starts_with("Source: Europe PMC / EMBL-EBI (https://europepmc.org)")
    );
    assert!(card.snippet.contains("Preprint (not peer reviewed)"));
    assert_eq!(card.url, "https://europepmc.org/article/PPR/PPR1");
    assert_eq!(card.primary_engine, "europepmc");
    assert_eq!(card.source_engines, vec!["europepmc"]);
    let seen = requests.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 1);
    assert!(seen[0].url.query().unwrap().contains("resultType=lite"));
    assert!(requests.replies.lock().unwrap().is_empty());
    body.shutdown_and_wait().await;
}
#[tokio::test]
async fn actual_pubmed_first_distinct_url_merge_keeps_europepmc_attribution_in_compact_cards() {
    let epmc = include_bytes!("../../../docs/rust-rewrite/a07/fixtures/europepmc-biomedical.json");
    let source: ssx_json::Value = ssx_json::from_slice(epmc).unwrap();
    let pmid = source["resultList"]["result"][0]["id"].as_str().unwrap();
    let search = ssx_json::json!({"esearchresult":{"idlist":[pmid]}});
    let mut result: ssx_json::Map<String, ssx_json::Value> = ssx_json::Map::new();
    result.insert(
        pmid.into(),
        ssx_json::json!({"title":"PubMed article","source":"Journal"}),
    );
    let summary = ssx_json::json!({"result":result});
    let (pm_http, pm_calls) = http(vec![
        ssx_json::to_vec(&search).unwrap(),
        ssx_json::to_vec(&summary).unwrap(),
    ]);
    let (ep_http, ep_calls) = http(vec![epmc.to_vec()]);
    let pm = PubMedAdapter::new(
        pm_http.clone(),
        Arc::new(AllowAllAdmission),
        BiomedicalConfig::defaults("https://eutils.ncbi.nlm.nih.gov/entrez/eutils"),
    );
    let ep = EuropePMCAdapter::new(
        ep_http.clone(),
        Arc::new(AllowAllAdmission),
        BiomedicalConfig::defaults("https://www.ebi.ac.uk/europepmc/webservices/rest"),
    );
    let q = SearchRequest {
        query: "CRISPR sickle cell".into(),
        ..Default::default()
    };
    let pubmed = pm.search(&q).await;
    let europe = ep.search(&q).await;
    assert_eq!(pubmed.status, EngineStatus::Ok);
    assert_eq!(europe.status, EngineStatus::Ok);
    assert_eq!(pubmed.results.len(), 1);
    assert_eq!(europe.results.len(), 5);
    assert_ne!(pubmed.results[0].url, europe.results[0].url);
    let mut feeds = Feeds::new();
    feeds.insert("pubmed".into(), pubmed.results);
    feeds.insert("europepmc".into(), europe.results);
    let merged = rank(&feeds, "presence", &BTreeMap::new());
    assert_eq!(merged.len(), 6);
    let cards = merged
        .iter()
        .map(|row| result_card(row, None, retrieval()))
        .collect::<Vec<_>>();
    assert!(
        cards
            .iter()
            .any(|card| card.url == format!("https://pubmed.ncbi.nlm.nih.gov/{pmid}/"))
    );
    let europe = cards
        .iter()
        .filter(|card| card.primary_engine == "europepmc")
        .collect::<Vec<_>>();
    assert_eq!(europe.len(), 5);
    assert!(europe.iter().all(|card| {
        card.snippet
            .starts_with("Source: Europe PMC / EMBL-EBI (https://europepmc.org)")
    }));
    assert_eq!(pm_calls.seen.lock().unwrap().len(), 2);
    assert_eq!(ep_calls.seen.lock().unwrap().len(), 1);
    pm_http.shutdown_and_wait().await;
    ep_http.shutdown_and_wait().await;
}
