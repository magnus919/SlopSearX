//! Original-source discovery replay. A mismatch is a real failure until reviewed.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission};
use ssx_config::{ConfigScalar, Secret};
use ssx_domain::SearchRequest;
use ssx_infra::{
    adapters::{ThreatIntelConfig, ThreatIntelSettings, register_threat_intelligence},
    http::{AdapterHttp, ExchangeFuture, Request, Response, Transport},
};
use ssx_json::Value;
use ssx_registry::Registry;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
#[derive(Deserialize)]
struct Capture {
    records: Vec<Box<serde_json::value::RawValue>>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Difference {
    id: String,
    kind: String,
    expected: Value,
    actual: Value,
}
#[derive(Serialize)]
struct Observation {
    id: String,
    native_output: Value,
    requests: Vec<Value>,
    unused_exchanges: usize,
    admission_calls: Vec<String>,
    completed_before_configured_deadline_floor: bool,
}
#[derive(Serialize)]
struct Discovery {
    profile: &'static str,
    records: Vec<Observation>,
    differences: Vec<Difference>,
}
struct Queue {
    exchanges: Mutex<VecDeque<Value>>,
    requests: Mutex<Vec<Value>>,
}
#[derive(Serialize)]
struct Outbound {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body_base64: String,
}
impl Transport for Queue {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            let outbound = Outbound {
                method: request.method.to_string(),
                url: request.url.to_string(),
                headers: request
                    .headers
                    .iter()
                    .map(|(n, v)| {
                        (
                            n.to_string(),
                            v.to_str().expect("fixture native header").into(),
                        )
                    })
                    .collect(),
                body_base64: STANDARD.encode(&request.body),
            };
            self.requests
                .lock()
                .unwrap()
                .push(ssx_json::to_value(outbound).unwrap());
            let Some(row) = self.exchanges.lock().unwrap().pop_front() else {
                return Err(ssx_infra::http::Error::Network);
            };
            if row.get("exception").is_some() {
                return Err(ssx_infra::http::Error::Deadline);
            }
            let response = &row["response"];
            let mut headers = reqwest::header::HeaderMap::new();
            for pair in response["headers"].as_array().unwrap() {
                headers.append(
                    pair[0]
                        .as_str()
                        .unwrap()
                        .parse::<reqwest::header::HeaderName>()
                        .unwrap(),
                    pair[1].as_str().unwrap().parse().unwrap(),
                );
            }
            Ok(Response {
                status: response["status"].as_u64().unwrap() as u16,
                url: request.url,
                headers,
                body: STANDARD
                    .decode(response["body_base64"].as_str().unwrap())
                    .unwrap(),
            })
        })
    }
}
struct Admission {
    deny: bool,
    calls: Mutex<Vec<String>>,
}
impl SearchAdmission for Admission {
    fn admit<'a>(&'a self, name: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(name.into());
            if self.deny {
                AdmissionDecision::Denied
            } else {
                AdmissionDecision::Allowed
            }
        })
    }
}
fn defaults(name: &str) -> ThreatIntelConfig {
    ThreatIntelConfig::defaults(match name {
        "abuseipdb" => ssx_infra::adapters::abuseipdb::ENDPOINT,
        "greynoise" => ssx_infra::adapters::greynoise::ENDPOINT,
        "otx" => ssx_infra::adapters::otx::ENDPOINT,
        "urlhaus" => ssx_infra::adapters::urlhaus::ENDPOINT,
        _ => panic!("unknown engine"),
    })
}
fn scalar(value: &Value) -> ConfigScalar {
    match value {
        Value::Bool(v) => ConfigScalar::Boolean(*v),
        Value::Number(n) => n
            .as_i64()
            .map(ConfigScalar::Integer)
            .unwrap_or_else(|| ConfigScalar::Float(n.as_f64().unwrap())),
        Value::String(s) => ConfigScalar::Text(s.clone()),
        _ => panic!("unreviewed raw source config"),
    }
}
fn settings(name: &str, source: &Value) -> ThreatIntelSettings {
    let mut target = defaults(name);
    if let Some(v) = source.get("api_key") {
        target.api_key = Secret::new(v.as_str().map(str::to_owned))
    }
    if let Some(v) = source.get("base_url") {
        target.base_url = v.as_str().expect("typed URL").into()
    }
    if let Some(v) = source.get("timeout_ms") {
        target.timeout_ms = scalar(v)
    }
    if let Some(v) = source.get("max_results") {
        target.max_results = scalar(v)
    }
    let mut all = ThreatIntelSettings {
        abuseipdb: defaults("abuseipdb"),
        greynoise: defaults("greynoise"),
        otx: defaults("otx"),
        urlhaus: defaults("urlhaus"),
    };
    match name {
        "abuseipdb" => all.abuseipdb = target,
        "greynoise" => all.greynoise = target,
        "otx" => all.otx = target,
        "urlhaus" => all.urlhaus = target,
        _ => unreachable!(),
    }
    all
}
fn differences(
    id: &str,
    kind: &'static str,
    expected: Value,
    actual: Value,
    output: &mut Vec<Difference>,
) {
    if expected != actual {
        output.push(Difference {
            id: id.into(),
            kind: kind.into(),
            expected,
            actual,
        })
    }
}
#[derive(Serialize)]
struct Normalized<'a> {
    results: Vec<Value>,
    status: &'a ssx_domain::EngineStatus,
    error_message: &'a Option<String>,
    answers: &'a Vec<Value>,
    infoboxes: &'a Vec<Value>,
    corrections: &'a Vec<String>,
    synthetic: bool,
}
fn normalized(response: &ssx_domain::AdapterResponse) -> Value {
    let results = response
        .results
        .iter()
        .map(|result| {
            let encoded = ssx_domain::storage::encode_result(result).unwrap();
            let decoded = ssx_domain::storage::decode_result(encoded.clone()).unwrap();
            assert_eq!(
                &decoded, result,
                "actual storage consumer must retain every field"
            );
            encoded
        })
        .collect();
    ssx_json::to_value(Normalized {
        results,
        status: &response.status,
        error_message: &response.error_message,
        answers: &response.answers,
        infoboxes: &response.infoboxes,
        corrections: &response.corrections,
        synthetic: response.synthetic,
    })
    .unwrap()
}
async fn run(profile: &'static str, source: &str) -> Discovery {
    let capture: Capture = serde_json::from_str(source).unwrap();
    let records: Vec<Value> = capture
        .records
        .iter()
        .map(|r| {
            if profile != "representation10" {
                return ssx_json::from_str(r.get()).unwrap();
            }
            // Preserve exact original internal outputs as literal JSON bytes. Public
            // scalar JSON cannot materialize a lone-surrogate Python string, and
            // tagged nonfinite scores are reference evidence, not wire numbers.
            let mut fields: BTreeMap<String, Box<serde_json::value::RawValue>> =
                serde_json::from_str(r.get()).unwrap();
            let raw = fields.remove("output").unwrap();
            let mut metadata: ssx_json::Map<String, Value> = fields
                .into_iter()
                .map(|(k, v)| (k, ssx_json::from_str(v.get()).unwrap()))
                .collect();
            metadata.insert(
                "output".into(),
                Value::Object(ssx_json::Map::from_iter([(
                    "original_internal_output_json".into(),
                    Value::String(raw.get().to_owned()),
                )])),
            );
            Value::Object(metadata)
        })
        .collect();
    let mut output = Discovery {
        profile,
        records: Vec::new(),
        differences: Vec::new(),
    };
    let mut parent = BTreeMap::new();
    for row in records.iter().filter(|r| {
        r.get("operation")
            .is_none_or(|v| v.as_str() == Some("search"))
    }) {
        let id = row["id"].as_str().unwrap();
        let name = row["engine"].as_str().unwrap();
        let config = settings(name, &row["config"]);
        let timeout = match name {
            "abuseipdb" => &config.abuseipdb.timeout_ms,
            "greynoise" => &config.greynoise.timeout_ms,
            "otx" => &config.otx.timeout_ms,
            _ => &config.urlhaus.timeout_ms,
        };
        let millis = match timeout {
            ConfigScalar::Integer(n) => *n as f64,
            ConfigScalar::Boolean(v) => {
                if *v {
                    1.
                } else {
                    0.
                }
            }
            ConfigScalar::Float(v) => *v,
            ConfigScalar::Text(_) => 0.,
        };
        let queue = Arc::new(Queue {
            exchanges: Mutex::new(
                row["exchanges"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .cloned()
                    .collect(),
            ),
            requests: Mutex::new(Vec::new()),
        });
        let http = Arc::new(AdapterHttp::default());
        http.set_transport(queue.clone()).unwrap();
        let mut registry = Registry::builtins().unwrap();
        let admission = Arc::new(Admission {
            deny: id.contains("admission-denied"),
            calls: Mutex::new(Vec::new()),
        });
        register_threat_intelligence(&mut registry, http.clone(), admission.clone(), config)
            .unwrap();
        let adapter = registry
            .instantiate(name)
            .expect("actual registered factory");
        let mut request = SearchRequest {
            query: row["query"].as_str().unwrap().into(),
            ..Default::default()
        };
        let params = &row["params"];
        if let Some(p) = params.get("page").and_then(Value::as_u64) {
            request.page =
                ssx_domain::SearchPage::from_u64(p).expect("positive source fixture page")
        }
        if let Some(s) = params.get("language").and_then(Value::as_str) {
            request.language = s.into()
        }
        if let Some(s) = params.get("time_range").and_then(Value::as_str) {
            request.time_range = Some(s.into())
        }
        if let Some(s) = params.get("safesearch").and_then(Value::as_u64) {
            request.safesearch = s as u8
        }
        let floor = Duration::try_from_secs_f64(millis / 1000.)
            .ok()
            .and_then(|duration| Instant::now().checked_add(duration));
        let response = adapter.search(&request).await;
        let completed = floor.is_some_and(|d| Instant::now() <= d);
        let actual = normalized(&response);
        http.shutdown_and_wait().await;
        let requests = queue.requests.lock().unwrap().clone();
        let unused = queue.exchanges.lock().unwrap().len();
        let expected_requests = Value::Array(
            row["exchanges"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x["request"].clone())
                .collect(),
        );
        differences(
            id,
            if profile == "representation10" {
                "typed_public_output_refusal"
            } else {
                "output"
            },
            row["output"].clone(),
            actual.clone(),
            &mut output.differences,
        );
        differences(
            id,
            "outbound",
            expected_requests,
            Value::Array(requests.clone()),
            &mut output.differences,
        );
        if unused != 0 {
            output.differences.push(Difference {
                id: id.into(),
                kind: "unused_source_exchanges".into(),
                expected: ssx_json::to_value(0).unwrap(),
                actual: ssx_json::to_value(unused).unwrap(),
            })
        }
        if id.contains("admission-denied") {
            differences(
                id,
                "admission",
                row["admission_calls"].clone(),
                ssx_json::to_value(admission.calls.lock().unwrap().clone()).unwrap(),
                &mut output.differences,
            )
        }
        if let Some(test) = row.get("source_test").and_then(Value::as_str) {
            parent.insert((test.to_owned(), name.to_owned()), actual.clone());
        }
        output.records.push(Observation {
            id: id.into(),
            native_output: actual,
            requests,
            unused_exchanges: unused,
            admission_calls: admission.calls.lock().unwrap().clone(),
            completed_before_configured_deadline_floor: completed,
        });
    }
    for row in records.iter().filter(|r| {
        r.get("operation")
            .is_some_and(|v| v.as_str() != Some("search"))
    }) {
        assert_eq!(row["nested_in_search"].as_bool(), Some(true));
        let test = row["source_test"].as_str().unwrap();
        let name = row["engine"].as_str().unwrap();
        let native = &parent[&(test.into(), name.into())];
        let results = &native["results"];
        let id = row["id"].as_str().unwrap();
        let actual = if let Some(exception) = row.get("exception") {
            let actual = native.clone();
            differences(
                id,
                "nested_source_exception",
                exception.clone(),
                actual.clone(),
                &mut output.differences,
            );
            actual
        } else {
            let actual = if row["operation"].as_str() == Some("_parse_single_url") {
                results[0].clone()
            } else {
                results.clone()
            };
            differences(
                id,
                "nested_parser_output",
                row["output"].clone(),
                actual.clone(),
                &mut output.differences,
            );
            actual
        };
        output.records.push(Observation {
            id: id.into(),
            native_output: actual,
            requests: Vec::new(),
            unused_exchanges: 0,
            admission_calls: Vec::new(),
            completed_before_configured_deadline_floor: false,
        });
    }
    output
}
#[derive(Deserialize)]
struct Ledger {
    entries: Vec<LedgerEntry>,
}
#[derive(Deserialize)]
struct LedgerEntry {
    profile: String,
    exact_difference: Box<serde_json::value::RawValue>,
}
fn approved(profile: &str) -> Vec<Difference> {
    let ledger: Ledger = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a11/native-differences-v1.json"
    ))
    .unwrap();
    ledger
        .entries
        .into_iter()
        .filter(|e| e.profile == profile)
        .map(|e| ssx_json::from_str(e.exact_difference.get()).unwrap())
        .collect()
}
fn counter(rows: &[Difference]) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for row in rows {
        *out.entry(ssx_json::to_string(row).unwrap()).or_default() += 1;
    }
    out
}
fn reviewed(profile: &str, actual: &[Difference]) -> bool {
    if !matches!(
        profile,
        "baseline58" | "boundary250" | "supplement3" | "representation10"
    ) {
        return false;
    }
    let expected = approved(profile);
    !expected.is_empty() && counter(&expected) == counter(actual)
}
fn helper_parents(baseline: &Discovery) {
    let source: Capture = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a11/source-regressions-v1.json"
    ))
    .unwrap();
    let source: Vec<Value> = source
        .records
        .iter()
        .map(|r| ssx_json::from_str(r.get()).unwrap())
        .collect();
    let bindings: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a11/helper-parent-bindings-v1.json"
    ))
    .unwrap();
    assert_eq!(bindings["bindings"].as_array().unwrap().len(), 11);
    for binding in bindings["bindings"].as_array().unwrap() {
        let helper_id = binding["source_helper_identity"].as_str().unwrap();
        let parent_id = binding["source_parent_identity"].as_str().unwrap();
        let original_helper = source
            .iter()
            .find(|r| r["id"].as_str() == Some(helper_id))
            .unwrap();
        let original_parent = source
            .iter()
            .find(|r| r["id"].as_str() == Some(parent_id))
            .unwrap();
        assert_eq!(original_helper["nested_in_search"].as_bool(), Some(true));
        assert_eq!(original_parent["operation"].as_str(), Some("search"));
        assert_eq!(
            original_helper["source_test"],
            original_parent["source_test"]
        );
        assert_eq!(original_helper["engine"], original_parent["engine"]);
        assert_eq!(original_helper["exception"], binding["source_exception"]);
        assert_eq!(
            original_parent["output"],
            binding["source_parent_complete_output"]
        );
        let native_parent = &baseline
            .records
            .iter()
            .find(|r| r.id == parent_id)
            .unwrap()
            .native_output;
        let helper_associated = &baseline
            .records
            .iter()
            .find(|r| r.id == helper_id)
            .unwrap()
            .native_output;
        assert_eq!(native_parent, &original_parent["output"]);
        assert_eq!(
            helper_associated, native_parent,
            "qualified only through the actual associated search; no standalone helper exception API"
        );
    }
}
#[test]
fn exact_difference_checker_refuses_missing_extra_duplicate_and_unbound_tuple_changes() {
    for profile in [
        "baseline58",
        "boundary250",
        "supplement3",
        "representation10",
    ] {
        let expected = approved(profile);
        assert!(reviewed(profile, &expected));
        assert!(!reviewed(profile, &[]));
        let mut missing = expected.clone();
        missing.pop();
        assert!(!reviewed(profile, &missing));
        let mut duplicate = expected.clone();
        duplicate.push(expected[0].clone());
        assert!(!reviewed(profile, &duplicate));
        let mut wrong = expected.clone();
        let row = wrong
            .iter_mut()
            .find(|r| r.kind == "outbound" && !r.actual.as_array().unwrap().is_empty())
            .unwrap();
        row.actual.as_array_mut().unwrap()[0]["url"] =
            Value::String("http://127.0.0.1:1/unbound".into());
        assert!(!reviewed(profile, &wrong));
        let mut no_auth = expected.clone();
        let row = no_auth
            .iter_mut()
            .find(|r| r.kind == "outbound" && !r.actual.as_array().unwrap().is_empty())
            .unwrap();
        row.actual.as_array_mut().unwrap()[0]["headers"] = Value::Array(Vec::new());
        assert!(!reviewed(profile, &no_auth));
    }
    let mut diagnostics = approved("boundary250");
    let row = diagnostics.iter_mut().find(|r| r.kind == "output").unwrap();
    row.actual["error_message"] = Value::String("unreviewed diagnostic".into());
    assert!(!reviewed("boundary250", &diagnostics));
    let mut unused = approved("boundary250");
    let row = unused
        .iter_mut()
        .find(|r| r.kind == "unused_source_exchanges")
        .unwrap();
    row.actual = ssx_json::to_value(0).unwrap();
    assert!(!reviewed("boundary250", &unused));
    let refusals = approved("representation10");
    for original in refusals
        .iter()
        .filter(|row| row.kind == "typed_public_output_refusal")
    {
        let mut broken = refusals.clone();
        let row = broken
            .iter_mut()
            .find(|row| row.id == original.id && row.kind == original.kind)
            .unwrap();
        row.actual["status"] = Value::String("ok".into());
        assert!(!reviewed("representation10", &broken));
        let mut broken = refusals.clone();
        let row = broken
            .iter_mut()
            .find(|row| row.id == original.id && row.kind == original.kind)
            .unwrap();
        row.actual["results"] = ssx_json::from_str("[{}]").unwrap();
        assert!(!reviewed("representation10", &broken));
        let mut broken = refusals.clone();
        let row = broken
            .iter_mut()
            .find(|row| row.id == original.id && row.kind == original.kind)
            .unwrap();
        row.expected["original_internal_output_json"] = Value::String("{}".into());
        assert!(!reviewed("representation10", &broken));
    }
    assert!(!reviewed("unknown", &[]));
    println!("A11_CHECKER_COMPLETE finite-positive-negative-exact-tuples");
}
#[tokio::test]
async fn all_original_observations_full_boundaries_and_supplement_exact() {
    let baseline = run(
        "baseline58",
        include_str!("../../../docs/rust-rewrite/a11/source-regressions-v1.json"),
    )
    .await;
    let boundary = run(
        "boundary250",
        include_str!("../../../docs/rust-rewrite/a11/source-boundaries-v2.json"),
    )
    .await;
    let supplement = run(
        "supplement3",
        include_str!("../../../docs/rust-rewrite/a11/source-supplement-v1.json"),
    )
    .await;
    let representation = run(
        "representation10",
        include_str!("../../../docs/rust-rewrite/a11/source-representation-v1.json"),
    )
    .await;
    assert_eq!(representation.records.len(), 10);
    assert_eq!(baseline.records.len(), 58);
    assert_eq!(boundary.records.len(), 250);
    assert_eq!(supplement.records.len(), 3);
    if let Ok(path) = std::env::var("SSX_A11_DISCOVERY_OUTPUT") {
        use std::io::Write;
        let bytes =
            serde_json::to_vec_pretty(&[&baseline, &boundary, &supplement, &representation])
                .unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap()
            .write_all(&bytes)
            .unwrap();
    }
    helper_parents(&baseline);
    for profile in [&baseline, &boundary, &supplement, &representation] {
        assert!(
            reviewed(profile.profile, &profile.differences),
            "unreviewed full identity-bound source differences: {}",
            profile.profile
        );
    }
    // All request/output/helper-ledger assertions and all engine/HTTP cleanup precede markers.
    for profile in [&baseline, &boundary, &supplement, &representation] {
        for row in &profile.records {
            println!("A11_OBSERVATION_COMPLETE {}:{}", profile.profile, row.id);
        }
    }
}
