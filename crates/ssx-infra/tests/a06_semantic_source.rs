//! Complete source-operation replay; no HTTP/factory implementation claim.
use base64::Engine as _;
use ssx_domain::{AdapterResponse, EngineStatus};
use ssx_infra::adapters::{
    semantic_scholar::{self, ParseError},
    source_json,
};
use ssx_json::{Value, json};
use std::time::{Duration, Instant};
fn normalized(response: &AdapterResponse) -> Value {
    assert!(
        response.results.iter().all(|result| result.media.is_none()),
        "actual science result unexpectedly contains media"
    );
    json!({"results":response.results.iter().map(|r|json!({"url":r.url,"title":r.title,"content":r.content,"engine":r.engine,"engines":r.engines.iter().collect::<Vec<_>>(),"score":r.score,"position":r.position,"category":r.category,"published_date":r.published_date,"thumbnail":r.thumbnail,"img_src":r.img_src,"tier":r.tier,"media":null,"payload":r.payload,"work_group":r.work_group})).collect::<Vec<_>>(),"status":response.status,"error_message":response.error_message,"answers":response.answers,"corrections":response.corrections,"infoboxes":response.infoboxes,"synthetic":response.synthetic})
}
#[tokio::test]
async fn whole_semantic_source_operations_and_later_errors_outrank_pending_refusals() {
    let mut exact = Vec::new();
    let mut typed = Vec::new();
    let mut outside = Vec::new();
    for text in [
        include_str!("../../../docs/rust-rewrite/a06/source-initial.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-regressions.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-shared.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-json-matrix.json"),
    ] {
        let fixture: Value = ssx_json::from_str(text).unwrap();
        for record in fixture["records"].as_array().unwrap() {
            if record["engine"].as_str() != Some("semanticscholar") {
                continue;
            }
            let id = record["id"].as_str().unwrap();
            let response = record["responses"].as_array().unwrap().last().unwrap();
            if response["status"].as_u64() != Some(200) {
                outside.push(id.to_owned());
                continue;
            }
            let body = base64::engine::general_purpose::STANDARD
                .decode(response["body_b64"].as_str().unwrap())
                .unwrap();
            let value = match source_json::from_slice(&body, None).await {
                Ok(v) => v,
                Err(_) => {
                    outside.push(id.to_owned());
                    continue;
                }
            };
            let result =
                semantic_scholar::parse(&value, Instant::now() + Duration::from_secs(10)).await;
            if record["output"]["status"] == json!("error") {
                assert_eq!(
                    result,
                    Err(ParseError::Source(
                        record["output"]["error_message"].as_str().unwrap().into()
                    )),
                    "{id}"
                );
                exact.push(id.to_owned());
            } else {
                assert_eq!(record["output"]["status"], json!("ok"), "{id}");
                let unrepresentable =
                    record["output"]["results"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|r| {
                            r["title"].as_str().is_none()
                                || r["url"].as_str().is_none()
                                || (!r["published_date"].is_null()
                                    && r["published_date"].as_str().is_none())
                        });
                if unrepresentable {
                    assert!(
                        matches!(result, Err(ParseError::Output(_))),
                        "{id}: {result:?}"
                    );
                    typed.push(id.to_owned());
                } else {
                    let out = AdapterResponse {
                        results: result.unwrap_or_else(|e| panic!("{id}: {e:?}")),
                        status: EngineStatus::Ok,
                        ..Default::default()
                    };
                    assert_eq!(normalized(&out), record["output"], "{id}");
                    exact.push(id.to_owned());
                }
            }
        }
    }
    eprintln!(
        "exact={} typed={} outside={} typed_ids={typed:?} outside_ids={outside:?}",
        exact.len(),
        typed.len(),
        outside.len()
    );
    assert!(
        exact
            .iter()
            .any(|id| id.ends_with("deferred_title_late_source_error"))
    );
    assert!(
        exact
            .iter()
            .any(|id| id.ends_with("deferred_url_late_source_error"))
    );
    assert!(
        exact
            .iter()
            .any(|id| id.ends_with("deferred_publicationDate_late_source_error"))
    );
}

#[tokio::test]
async fn quoted_long_decimal_uses_shared_exact_conversion() {
    use sha2::{Digest, Sha256};
    let text = format!("0.{}1e+1990000", "0".repeat(1_990_000));
    assert_eq!(text.len(), 1_990_012);
    assert_eq!(
        Sha256::digest(text.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        "df0527a2aa88b8148e712a258c1897380745434341f64c4bad77c7a01f51e006"
    );
    let body = format!(
        r#"{{"data":[{{"title":"Title","url":"https://paper.test/1","citationCount":"{text}"}}]}}"#
    );
    let value = source_json::from_slice(body.as_bytes(), None)
        .await
        .unwrap();
    let out = semantic_scholar::parse(&value, Instant::now() + Duration::from_secs(30))
        .await
        .unwrap();
    assert_eq!(out.len(), 1);
    let score = &out[0].score;
    assert_eq!(score.get().to_bits(), 0x3fb9_9999_9999_999a);
    assert_eq!(out[0].content.len(), 1_990_022);
    assert_eq!(out[0].content, format!("Cited by: {text}"));
    assert_eq!(out[0].title, "Title");
    assert_eq!(out[0].url, "https://paper.test/1");
}
