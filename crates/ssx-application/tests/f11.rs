use ssx_application::{
    normalise_url,
    ranking::{self, Feeds},
    retrieval, scholarly,
};
use ssx_domain::{SearchResult, storage};
use ssx_json::{Value, json};
use std::collections::BTreeMap;
fn result(v: &Value) -> SearchResult {
    let mut r = storage::decode_result(v.clone()).expect("valid fixture result");
    r.payload = v.get("payload").filter(|v| !v.is_null()).cloned();
    r.work_group = v.get("work_group").filter(|v| !v.is_null()).cloned();
    r
}
fn encode(r: &SearchResult) -> Value {
    let mut v = storage::encode_result(r).unwrap();
    v["payload"] = json!(r.payload);
    v["work_group"] = json!(r.work_group);
    v
}
fn decode_feeds(v: &Value) -> Feeds {
    v.as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f[0].as_str().unwrap().into(),
                f[1].as_array().unwrap().iter().map(result).collect(),
            )
        })
        .collect()
}
fn encode_feeds(f: &Feeds) -> Value {
    json!(
        f.iter()
            .map(|(k, rows)| json!([k, rows.iter().map(encode).collect::<Vec<_>>()]))
            .collect::<Vec<_>>()
    )
}
#[test]
fn source_goldens() {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/rust-rewrite/f11");
    let manifest: Value =
        ssx_json::from_str(&std::fs::read_to_string(directory.join("manifest.json")).unwrap())
            .unwrap();
    let mut lines = String::new();
    for shard in manifest["shards"].as_array().unwrap() {
        lines.push_str(&std::fs::read_to_string(directory.join(shard.as_str().unwrap())).unwrap());
    }
    for line in lines.lines() {
        let case: Value = ssx_json::from_str(line).unwrap();
        let actual = match case["kind"].as_str().unwrap() {
            "url" => json!(normalise_url(case["input"].as_str().unwrap())),
            "retrieval" => {
                let r = retrieval::classify_retrieval_url(case["input"].as_str().unwrap());
                json!([r.status, r.reason, r.scheme, r.url])
            }
            "group" => encode_feeds(&scholarly::group_publications(
                &decode_feeds(&case["input"]),
                case["query"].as_str().unwrap(),
            )),
            "rank" => {
                let feeds = decode_feeds(&case["input"]);
                let before = feeds.clone();
                let budgets: BTreeMap<String, i64> =
                    ssx_json::from_value(case["budgets"].clone()).unwrap();
                let out = ranking::rank(&feeds, case["strategy"].as_str().unwrap(), &budgets);
                assert_eq!(feeds, before, "input mutation");
                json!(out.iter().map(encode).collect::<Vec<_>>())
            }
            _ => panic!("unknown case"),
        };
        assert_eq!(actual, case["expected"], "case {}", case["id"]);
    }
}
