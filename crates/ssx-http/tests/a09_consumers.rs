//! Actual named adapter -> accepted ranking/parser -> HTTP/MCP projections.
use ssx_application::{
    SearchEngine,
    ranking::{Feeds, rank},
};
use ssx_domain::{EngineStatus, SearchRequest};
use ssx_infra::{
    adapters::{
        AllowAllAdmission,
        openlibrary::{OpenLibraryAdapter, OpenLibraryConfig},
        tmdb::{TmdbAdapter, TmdbConfig},
    },
    http::{AdapterHttp, ExchangeFuture, Request, Response, Transport},
};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
};
struct Replay(Mutex<VecDeque<String>>);
impl Transport for Replay {
    fn exchange(&self, r: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            Ok(Response {
                url: r.url,
                status: 200,
                headers: Default::default(),
                body: self
                    .0
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("unexpected request")
                    .into_bytes(),
            })
        })
    }
}
async fn actual(engine: &str, id: &str) -> (Vec<ssx_domain::SearchResult>, Arc<AdapterHttp>) {
    let doc: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a09/regressions.json"
    ))
    .unwrap();
    let case = doc["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id)
        .unwrap();
    let transport = Arc::new(Replay(Mutex::new(
        case["responses"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["wire_body"].as_str().unwrap().to_owned())
            .collect(),
    )));
    let h = Arc::new(AdapterHttp::default());
    h.set_transport(transport.clone()).unwrap();
    let adapter: Box<dyn SearchEngine> = if engine == "openlibrary" {
        Box::new(OpenLibraryAdapter::new(
            h.clone(),
            Arc::new(AllowAllAdmission),
            OpenLibraryConfig::defaults(ssx_infra::adapters::openlibrary::ENDPOINT),
        ))
    } else {
        Box::new(TmdbAdapter::new(
            h.clone(),
            Arc::new(AllowAllAdmission),
            TmdbConfig {
                api_key: ssx_config::Secret::new(Some("synthetic-tmdb-key".into())),
                ..Default::default()
            },
        ))
    };
    let response = adapter
        .search(&SearchRequest {
            query: case["query"].as_str().unwrap().into(),
            ..Default::default()
        })
        .await;
    assert_eq!(response.status, EngineStatus::Ok);
    assert!(transport.0.lock().unwrap().is_empty());
    (response.results, h)
}
#[tokio::test]
async fn openlibrary_actual_rank_and_http_json_preserve_original_identity_and_parsed_url() {
    let node = "tests/test_openlibrary_url_contract.py::test_distinct_urls_survive_merger_and_json_formatting";
    let id = format!("{node}#0");
    let (results, http) = actual("openlibrary", &id).await;
    let mut feeds = Feeds::new();
    feeds.insert("openlibrary".into(), results);
    let ranked = rank(&feeds, "presence", &BTreeMap::new());
    let doc: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a09/regressions.json"
    ))
    .unwrap();
    let source = &doc["consumer_records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["node"] == node)
        .unwrap()["output"]["results"];
    assert_eq!(ranked.len(), source.as_array().unwrap().len());
    for (row, expected) in ranked.iter().zip(source.as_array().unwrap()) {
        let metadata = ssx_wire::HttpResultMetadata {
            parsed_url: ssx_application::parsed_url(&row.url),
            ..Default::default()
        };
        let projected = ssx_http::projection::result_json(row, &metadata);
        let actual = serde_json::to_value(projected).unwrap();
        for field in [
            "url",
            "parsed_url",
            "title",
            "content",
            "engine",
            "engines",
            "thumbnail",
            "payload",
            "media",
        ] {
            assert_eq!(actual[field], expected[field], "{field}");
        }
    }
    http.shutdown_and_wait().await;
}
#[tokio::test]
async fn tmdb_actual_complete_payload_and_thumbnail_survive_rank_and_http_projection() {
    let id = "tests/test_payload.py::TestMediaPayload::test_tmdb_payload#0";
    let (results, http) = actual("tmdb", id).await;
    let mut feeds = Feeds::new();
    feeds.insert("tmdb".into(), results);
    let ranked = rank(&feeds, "presence", &BTreeMap::new());
    assert_eq!(ranked.len(), 2);
    let movie = ssx_http::projection::result_json(&ranked[0], &Default::default());
    let person = ssx_http::projection::result_json(&ranked[1], &Default::default());
    let movie = serde_json::to_value(movie).unwrap();
    let person = serde_json::to_value(person).unwrap();
    assert_eq!(
        movie["thumbnail"],
        "https://image.tmdb.org/t/p/w200/testposter.jpg"
    );
    assert_eq!(movie["payload"]["data"]["vote_average"], 7.5);
    assert_eq!(movie["payload"]["provenance"]["engine"], "tmdb");
    assert_eq!(person["payload"]["data"]["media_type"], "person");
    assert!(movie["media"].is_null());
    assert!(movie["img_src"].is_null());
    http.shutdown_and_wait().await;
}
#[test]
fn parsed_url_accessor_six_components_and_malformed_authority_boundary() {
    assert_eq!(
        ssx_application::parsed_url("HTTPS://Example.COM/path;params?q=x#frag"),
        Some(
            ["https", "Example.COM", "/path", "params", "q=x", "frag"]
                .map(str::to_owned)
                .to_vec()
        )
    );
    assert!(ssx_application::parsed_url("https://[broken/path").is_none());
    assert_eq!(
        ssx_application::parsed_url("/relative"),
        Some(
            ["", "", "/relative", "", "", ""]
                .map(str::to_owned)
                .to_vec()
        )
    );
}
