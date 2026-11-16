//! Actual A08 adapter -> ranking -> compact JSON. Full portal HTML is T03/G01.
use ssx_application::{
    SearchEngine,
    ranking::{Feeds, rank},
};
use ssx_domain::{EngineStatus, SearchRequest};
use ssx_infra::{
    adapters::{
        AllowAllAdmission,
        openfda::{OpenFdaAdapter, OpenFdaConfig},
        oyez::{OyezAdapter, OyezConfig},
    },
    http::{AdapterHttp, ExchangeFuture, Request, Response, Transport},
};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
};
struct Replay(Mutex<VecDeque<Vec<u8>>>);
impl Transport for Replay {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            Ok(Response {
                url: request.url,
                status: 200,
                headers: Default::default(),
                body: self
                    .0
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("unexpected consumer request"),
            })
        })
    }
}
async fn source(name: &str, id: &str) -> (Vec<ssx_domain::SearchResult>, Arc<AdapterHttp>) {
    let source: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a08/regressions.json"
    ))
    .unwrap();
    let row = source["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id)
        .unwrap();
    let replay = Arc::new(Replay(Mutex::new(
        row["responses"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["wire_body"].as_str().unwrap().as_bytes().to_vec())
            .collect(),
    )));
    let http = Arc::new(AdapterHttp::default());
    http.set_transport(replay.clone()).unwrap();
    let adapter: Box<dyn SearchEngine> = match name {
        "openfda" => Box::new(OpenFdaAdapter::new(
            http.clone(),
            Arc::new(AllowAllAdmission),
            OpenFdaConfig::defaults(ssx_infra::adapters::openfda::ENDPOINT),
        )),
        "oyez" => Box::new(OyezAdapter::new(
            http.clone(),
            Arc::new(AllowAllAdmission),
            OyezConfig::default(),
        )),
        _ => panic!(),
    };
    let response = adapter
        .search(&SearchRequest {
            query: row["query"].as_str().unwrap().into(),
            ..Default::default()
        })
        .await;
    assert_eq!(response.status, EngineStatus::Ok);
    assert!(replay.0.lock().unwrap().is_empty());
    (response.results, http)
}
#[tokio::test]
async fn actual_fda_and_oyez_omit_before_rank_and_compact_json() {
    for (name, id) in [
        (
            "openfda",
            "tests/test_openfda.py::test_identifier_free_labels_are_omitted_before_merge_and_format#0",
        ),
        (
            "oyez",
            "tests/test_oyez_relevance.py::test_unresolved_records_are_omitted_without_empty_url_merge#0",
        ),
    ] {
        let (results, http) = source(name, id).await;
        let expected: Vec<_> = results.iter().map(|r| r.url.clone()).collect();
        if name == "openfda" {
            assert!(expected.is_empty());
        } else {
            assert_eq!(expected.len(), 2);
        }
        assert!(expected.iter().all(|u| !u.is_empty()));
        let mut feeds = Feeds::new();
        feeds.insert(name.into(), results);
        let ranked = rank(&feeds, "presence", &BTreeMap::new());
        assert_eq!(ranked.len(), expected.len());
        let cards: Vec<_> = ranked
            .iter()
            .map(|r| {
                ssx_mcp::projection::result_card(
                    r,
                    None,
                    ssx_wire::McpRetrievalDto {
                        contract: "slopsearx.retrieval_handoff".into(),
                        version: 1,
                        eligible: true,
                        url_status: "ok".into(),
                        url_reason: None,
                        scheme: Some("https".into()),
                    },
                )
            })
            .collect();
        let json = serde_json::to_value(&cards).unwrap();
        let urls: std::collections::BTreeSet<_> = json
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["url"].as_str().unwrap())
            .collect();
        assert_eq!(urls, expected.iter().map(String::as_str).collect());
        assert!(
            cards
                .iter()
                .all(|c| c.primary_engine == name && c.source_engines == vec![name])
        );
        http.shutdown_and_wait().await;
    }
}
#[tokio::test]
async fn actual_fda_full_payload_stays_canonical_while_compact_disclosure_obeys_threshold() {
    let(results,http)=source("openfda","tests/test_openfda.py::test_exact_name_fields_rank_before_related_and_preserve_provenance#0").await;
    assert!(results.iter().all(|r| {
        r.payload
            .as_ref()
            .is_some_and(|p| p["domain"] == "biomedical" && p["type"] == "drug_label")
    }));
    for r in &results {
        let card = ssx_mcp::projection::result_card(
            r,
            None,
            ssx_wire::McpRetrievalDto {
                contract: "slopsearx.retrieval_handoff".into(),
                version: 1,
                eligible: true,
                url_status: "ok".into(),
                url_reason: None,
                scheme: Some("https".into()),
            },
        );
        let bytes = ssx_json::to_vec(r.payload.as_ref().unwrap()).unwrap();
        assert_eq!(card.payload.is_some(), bytes.len() <= 512);
        assert_eq!(card.url, r.url);
        assert!(card.snippet.chars().count() <= 300);
    }
    http.shutdown_and_wait().await;
}
