//! Actual A08 factories, parsers and request transport; source-bound replay.
use serde_json::{Value, json};
use ssx_application::SearchEngine;
use ssx_domain::{EngineStatus, SearchRequest};
use ssx_infra::{
    adapters::{
        AllowAllAdmission,
        edgar::{EdgarAdapter, EdgarConfig},
    },
    http::{AdapterHttp, ExchangeFuture, Request, Response, Transport},
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
const HEADER_DIFFERENCES: &str = include_str!(
    "../../../docs/rust-rewrite/a08/source-amendments/default-user-agent-effective-v1.json"
);
const DIFFERENCES: &str = include_str!("../../../docs/rust-rewrite/a08/differences.json");
const REGRESSIONS: &str = include_str!("../../../docs/rust-rewrite/a08/regressions.json");
const SOURCE: &str = include_str!("../../../docs/rust-rewrite/a08/source.json");
struct Replay {
    replies: Mutex<VecDeque<Value>>,
    requests: Mutex<Vec<Request>>,
}
impl Transport for Replay {
    fn exchange(&self, req: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            let url = req.url.clone();
            self.requests.lock().unwrap().push(req);
            let r = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected request");
            if r["exception"].is_string() && r["exception"] != "timeout" {
                return Err(ssx_infra::http::Error::Network);
            }
            if r["exception"] == "timeout" {
                return Err(ssx_infra::http::Error::Deadline);
            }
            Ok(Response {
                url,
                status: r["status"].as_u64().unwrap() as u16,
                headers: Default::default(),
                body: r["wire_body"]
                    .as_str()
                    .or_else(|| r["raw"].as_str())
                    .map(|s| s.as_bytes().to_vec())
                    .unwrap_or_else(|| serde_json::to_vec(&r["body"]).unwrap()),
            })
        })
    }
}
fn scalar(v: &Value) -> ssx_config::ConfigScalar {
    if let Some(value) = v["$nonfinite"].as_str() {
        return ssx_config::ConfigScalar::Float(match value {
            "nan" => f64::NAN,
            "inf" => f64::INFINITY,
            "-inf" => f64::NEG_INFINITY,
            _ => panic!("bad nonfinite config"),
        });
    }
    match v {
        Value::Bool(b) => ssx_config::ConfigScalar::Boolean(*b),
        Value::Number(n) if n.is_i64() => ssx_config::ConfigScalar::Integer(n.as_i64().unwrap()),
        Value::Number(n) => ssx_config::ConfigScalar::Float(n.as_f64().unwrap()),
        Value::String(s) => ssx_config::ConfigScalar::Text(s.clone()),
        _ => panic!("bad config"),
    }
}
fn config(case: &Value, base: &str) -> EdgarConfig {
    let mut c = EdgarConfig::defaults(base);
    if let Some(v) = case["config"].get("max_results") {
        c.max_results = scalar(v)
    }
    if let Some(v) = case["config"].get("timeout_ms") {
        c.timeout_ms = scalar(v)
    }
    if let Some(s) = case["config"]["base_url"].as_str() {
        c.base_url = s.into()
    }
    c
}
fn output(response: &ssx_domain::AdapterResponse) -> Value {
    json!({"status":match response.status{EngineStatus::Ok=>"ok",EngineStatus::Error=>"error",EngineStatus::Timeout=>"timeout",EngineStatus::RateLimited=>"rate_limited",_=>panic!("unexpected")},"error_message":response.error_message,"results":response.results.iter().map(|r|{assert!(r.media.is_none());json!({"url":r.url,"title":r.title,"content":r.content,"engine":r.engine,"position":r.position,"score":r.score.get(),"category":r.category,"published_date":r.published_date,"thumbnail":r.thumbnail,"img_src":r.img_src,"media":Value::Null,"payload":r.payload.as_ref().map(|v|serde_json::from_str::<Value>(&v.to_string()).unwrap())})}).collect::<Vec<_>>()})
}
#[tokio::test]
async fn all_engine_source_replay() {
    let mut source: Value = serde_json::from_str(SOURCE).unwrap();
    let regression: Value = serde_json::from_str(REGRESSIONS).unwrap();
    source["records"]
        .as_array_mut()
        .unwrap()
        .extend(regression["records"].as_array().unwrap().clone());
    let mut observed = vec![];
    let differences: Value = serde_json::from_str(DIFFERENCES).unwrap();
    let mut used_differences = std::collections::BTreeSet::new();
    let header_differences: Value = serde_json::from_str(HEADER_DIFFERENCES).unwrap();
    let mut used_headers = std::collections::BTreeSet::new();
    for case in source["records"].as_array().unwrap().iter() {
        if let Some(difference) = differences["records"].as_array().unwrap().iter().find(|r| {
            r["id"] == case["id"] && r["category"] == "F06-typed-config-rejection-supersession"
        }) {
            assert_eq!(difference["source_output"], case["output"]);
            let path =
                std::env::temp_dir().join(format!("a08-raw-config-{}.yaml", std::process::id()));
            std::fs::write(
                &path,
                format!(
                    "engines:\n  fred:\n    api_key: {}\n",
                    case["config"]["api_key"]
                ),
            )
            .unwrap();
            assert!(ssx_config::load_config_with(Some(&path), &Default::default()).is_err());
            std::fs::remove_file(path).unwrap();
            used_differences.insert(case["id"].as_str().unwrap().to_owned());
            observed
                .push(json!({"id":case["id"],"output":difference["native_output"],"requests":[]}));
            println!("\nA08_CASE {}", json!({"id":case["id"],"kind":"loader"}));
            continue;
        }
        let replay = Arc::new(Replay {
            replies: Mutex::new(case["responses"].as_array().unwrap().clone().into()),
            requests: Mutex::new(vec![]),
        });
        let http = Arc::new(AdapterHttp::default());
        http.set_transport(replay.clone()).unwrap();
        let name = case["engine"].as_str().unwrap();
        let adapter: Box<dyn SearchEngine> = match name {
            "edgar" => Box::new(EdgarAdapter::new(
                http.clone(),
                Arc::new(AllowAllAdmission),
                config(case, ssx_infra::adapters::edgar::ENDPOINT),
            )),
            "fred" => Box::new(ssx_infra::adapters::fred::FredAdapter::new(
                http.clone(),
                Arc::new(AllowAllAdmission),
                ssx_infra::adapters::fred::FredConfig {
                    settings: config(case, ssx_infra::adapters::fred::ENDPOINT),
                    api_key: ssx_config::Secret::new(
                        case["config"]["api_key"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                            .or(case["config"]["ENGINE_FRED_API_KEY"]
                                .as_str()
                                .filter(|s| !s.is_empty()))
                            .or(case["config"]["environment_api_key"].as_str())
                            .map(str::to_owned),
                    ),
                },
            )),
            "oyez" => Box::new(ssx_infra::adapters::oyez::OyezAdapter::new(
                http.clone(),
                Arc::new(AllowAllAdmission),
                ssx_infra::adapters::oyez::OyezConfig {
                    settings: config(case, ""),
                    search_url: case["config"]["search_url"].as_str().map(str::to_owned),
                },
            )),
            "openfda" => Box::new(ssx_infra::adapters::openfda::OpenFdaAdapter::new(
                http.clone(),
                Arc::new(AllowAllAdmission),
                config(case, ssx_infra::adapters::openfda::ENDPOINT),
            )),
            _ => panic!("unknown engine"),
        };
        let request = SearchRequest {
            query: case["query"].as_str().unwrap().into(),
            page: ssx_domain::SearchPage::from_u64(8).expect("positive fixture page"),
            ..Default::default()
        };
        let result = if case["responses"].as_array().unwrap().is_empty()
            && case["output"]["status"] == "timeout"
        {
            adapter
                .search_with_context(
                    &request,
                    ssx_application::SearchContext {
                        deadline: std::time::Instant::now() - std::time::Duration::from_secs(1),
                    },
                )
                .await
        } else {
            adapter.search(&request).await
        };
        let actual = output(&result);
        let mut expected_output = case["output"].clone();
        if let Some(rows) = expected_output
            .get_mut("results")
            .and_then(Value::as_array_mut)
        {
            for row in rows {
                if let Some(n) = row["score"].as_f64() {
                    row["score"] = json!(n)
                }
            }
        }
        observed.push(json!({"id":case["id"],"output":actual}));
        let requests: Vec<_> = {
            let captured = replay.requests.lock().unwrap();
            captured.iter().map(|r|json!({"method":r.method.as_str(),"url":r.url.as_str().replace("synthetic-fred-key","<synthetic-key>"),"body":std::str::from_utf8(&r.body).unwrap(),"headers":r.headers.iter().filter(|(k,_)|["accept","content-type","user-agent"].contains(&k.as_str())).map(|(k,v)|(k.as_str(),v.to_str().unwrap())).collect::<std::collections::BTreeMap<_,_>>()})).collect()
        };
        observed.last_mut().unwrap()["requests"] = json!(requests);
        let mut expected_requests = case["requests"].as_array().unwrap().clone();
        if let Some(header) = header_differences["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == case["id"])
        {
            assert_eq!(
                header["source_values"],
                json!(
                    expected_requests
                        .iter()
                        .map(|r| r["headers"]
                            .get("user-agent")
                            .cloned()
                            .unwrap_or(Value::Null))
                        .collect::<Vec<_>>()
                )
            );
            assert_eq!(
                header["native_values"],
                json!(
                    requests
                        .iter()
                        .map(|r| r["headers"]
                            .get("user-agent")
                            .cloned()
                            .unwrap_or(Value::Null))
                        .collect::<Vec<_>>()
                )
            );
            used_headers.insert(case["id"].as_str().unwrap().to_owned());
            // Both identities were asserted above; derive the explicit native
            // request view without changing captured source/native observations.
            for (request, native) in expected_requests
                .iter_mut()
                .zip(header["native_values"].as_array().unwrap())
            {
                let headers = request["headers"].as_object_mut().unwrap();
                if native.is_null() {
                    headers.remove("user-agent");
                } else {
                    headers.insert("user-agent".into(), native.clone());
                }
            }
        }
        if let Some(difference) = differences["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == case["id"])
        {
            assert_eq!(
                difference["source_output"], expected_output,
                "{} source difference binding",
                case["id"]
            );
            used_differences.insert(case["id"].as_str().unwrap().to_owned());
            expected_output = difference["native_output"].clone();
            if difference["category"] == "mock-timer-superseded-by-real-loopback" {
                assert!(requests.is_empty());
                assert_eq!(actual["status"], "timeout");
            } else {
                assert_eq!(requests, expected_requests, "{} requests", case["id"]);
            }
        } else {
            assert_eq!(requests, expected_requests, "{} requests", case["id"]);
        }
        assert_eq!(actual, expected_output, "{} complete output", case["id"]);
        assert!(!actual.to_string().contains("synthetic-fred-key"));
        http.shutdown_and_wait().await;
        println!("\nA08_CASE {}", json!({"id":case["id"],"kind":"adapter"}));
    }
    assert_eq!(
        used_differences.len(),
        differences["records"].as_array().unwrap().len()
    );
    assert_eq!(
        used_headers.len(),
        header_differences["records"].as_array().unwrap().len()
    );
    if let Ok(path) = std::env::var("A08_OBSERVED") {
        std::fs::write(path, serde_json::to_vec_pretty(&observed).unwrap()).unwrap();
    }
}
