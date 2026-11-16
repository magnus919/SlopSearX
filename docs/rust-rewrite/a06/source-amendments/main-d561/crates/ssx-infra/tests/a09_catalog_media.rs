//! Actual A09 adapter/factory replay against frozen offline original outcomes.
use serde_json::{Value, json};
use ssx_domain::{AdapterResponse, SearchRequest};
use ssx_infra::{
    adapters::{self, AllowAllAdmission, CatalogMediaSettings},
    http::{AdapterHttp, ExchangeFuture, Request, Response, Transport},
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
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
                .expect("unexpected provider call");
            match r["exception"].as_str() {
                Some("timeout") => return Err(ssx_infra::http::Error::Deadline),
                Some(_) => return Err(ssx_infra::http::Error::Network),
                None => {}
            }
            Ok(Response {
                url,
                status: r["status"].as_u64().unwrap() as u16,
                headers: Default::default(),
                body: r["wire_body"]
                    .as_str()
                    .or(r["raw"].as_str())
                    .map(|s| s.as_bytes().to_vec())
                    .unwrap_or_else(|| serde_json::to_vec(&r["body"]).unwrap()),
            })
        })
    }
}
fn scalar(v: &Value) -> ssx_config::ConfigScalar {
    match v {
        Value::Bool(v) => ssx_config::ConfigScalar::Boolean(*v),
        Value::Number(n) if n.is_i64() => ssx_config::ConfigScalar::Integer(n.as_i64().unwrap()),
        Value::Number(n) => ssx_config::ConfigScalar::Float(n.as_f64().unwrap()),
        Value::String(v) => ssx_config::ConfigScalar::Text(v.clone()),
        _ => panic!("bad scalar"),
    }
}
fn config(case: &Value, base: &str) -> adapters::openlibrary::OpenLibraryConfig {
    let mut c = adapters::openlibrary::OpenLibraryConfig::defaults(base);
    if let Some(v) = case["config"].get("max_results") {
        c.max_results = scalar(v)
    }
    if let Some(v) = case["config"].get("timeout_ms") {
        c.timeout_ms = scalar(v)
    }
    if let Some(v) = case["config"]["base_url"].as_str() {
        c.base_url = v.into()
    }
    c
}
fn settings(case: &Value) -> CatalogMediaSettings {
    let key = case["config"]["api_key"]
        .as_str()
        .filter(|s| !s.is_empty())
        .or(case["config"]["ENGINE_TMDB_API_KEY"]
            .as_str()
            .filter(|s| !s.is_empty()))
        .or(case["config"]["environment_api_key"].as_str())
        .map(str::to_owned);
    let archive_config = if case["config"].get("timeout_ms").is_some() {
        config(case, adapters::internetarchive::ENDPOINT)
    } else {
        let mut c = config(case, adapters::internetarchive::ENDPOINT);
        c.timeout_ms = ssx_config::ConfigScalar::Integer(10000);
        c
    };
    CatalogMediaSettings {
        openlibrary: config(case, adapters::openlibrary::ENDPOINT),
        musicbrainz: config(case, adapters::musicbrainz::ENDPOINT),
        tmdb: adapters::tmdb::TmdbConfig {
            settings: config(case, adapters::tmdb::ENDPOINT),
            api_key: ssx_config::Secret::new(key),
        },
        nominatim: config(case, adapters::nominatim::ENDPOINT),
        internetarchive: adapters::internetarchive::InternetArchiveConfig {
            settings: archive_config,
            cdx_url: None,
            availability_url: None,
        },
    }
}
fn output(r: &AdapterResponse) -> Value {
    let status = serde_json::to_value(&r.status).unwrap();
    json!({"status":status,"error_message":r.error_message,"results":r.results.iter().map(|r|json!({"url":r.url,"title":r.title,"content":r.content,"engine":r.engine,"position":r.position,"score":r.score.get(),"category":r.category,"published_date":r.published_date,"thumbnail":r.thumbnail,"img_src":r.img_src,"media":({assert!(r.media.is_none());Value::Null}),"payload":r.payload.as_ref().map(|v|serde_json::from_str::<Value>(&v.to_string()).unwrap())})).collect::<Vec<_>>()})
}
#[tokio::test]
async fn all_engine_source_replay() {
    let mut source: Value =
        serde_json::from_str(include_str!("../../../docs/rust-rewrite/a09/source.json")).unwrap();
    let regressions: Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a09/regressions.json"
    ))
    .unwrap();
    source["records"]
        .as_array_mut()
        .unwrap()
        .extend(regressions["records"].as_array().unwrap().clone());
    let differences: Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a09/differences.json"
    ))
    .unwrap();
    let headers: Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a09/header-differences.json"
    ))
    .unwrap();
    let mut used_headers = std::collections::BTreeSet::new();
    let mut observed = vec![];
    let mut mismatch = vec![];
    let mut used = std::collections::BTreeSet::new();
    for case in source["records"].as_array().unwrap() {
        let replay = Arc::new(Replay {
            replies: Mutex::new(case["responses"].as_array().unwrap().clone().into()),
            requests: Mutex::new(vec![]),
        });
        let http = Arc::new(AdapterHttp::default());
        http.set_transport(replay.clone()).unwrap();
        let mut registry = ssx_registry::Registry::builtins().unwrap();
        adapters::register_catalog_media(
            &mut registry,
            http.clone(),
            Arc::new(AllowAllAdmission),
            settings(case),
        )
        .unwrap();
        let engine = registry
            .instantiate(case["engine"].as_str().unwrap())
            .unwrap();
        let response = engine
            .search(&SearchRequest {
                query: case["query"].as_str().unwrap().into(),
                page: 8,
                max_results: Some(1),
                language: "fr".into(),
                ..Default::default()
            })
            .await;
        let actual = output(&response);
        let requests=replay.requests.lock().unwrap().iter().map(|r|json!({"method":r.method.as_str(),"url":r.url.as_str(),"body":String::from_utf8(r.body.clone()).unwrap(),"headers":r.headers.iter().filter(|(k,_)|["user-agent","accept","authorization","content-type"].contains(&k.as_str())).map(|(k,v)|(k.to_string(),Value::String(if case.get("node").is_some(){v.to_str().unwrap().replace("synthetic-tmdb-key","<synthetic-key>")}else{v.to_str().unwrap().to_owned()}))).collect::<serde_json::Map<_,_>>()})).collect::<Vec<_>>();
        let expected_requests = if let Some(header) = headers["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|h| h["id"] == case["id"])
        {
            assert_eq!(header["source_requests"], case["requests"]);
            used_headers.insert(case["id"].as_str().unwrap().to_owned());
            header["native_requests"].clone()
        } else {
            case["requests"].clone()
        };
        let difference = differences["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["id"] == case["id"]);
        let expected = if let Some(d) = difference {
            assert_eq!(d["source_output"], case["output"]);
            used.insert(case["id"].as_str().unwrap().to_owned());
            &d["native_output"]
        } else {
            &case["output"]
        };
        if actual != *expected || json!(requests) != expected_requests {
            mismatch.push(json!({"id":case["id"],"source":case["output"],"native":actual,"source_requests":expected_requests,"native_requests":requests}));
        }
        observed.push(json!({"id":case["id"],"output":actual,"requests":requests}));
        drop(engine);
        http.shutdown_and_wait().await;
        // Completion marker only follows real assertions, full request comparison and shutdown.
        if actual == *expected && json!(requests) == expected_requests {
            assert_eq!(actual, *expected);
            assert_eq!(json!(requests), expected_requests);
            println!("\nA09_CASE {}", case["id"]);
        }
    }
    std::fs::write(
        std::env::temp_dir().join(format!("a09-native-observed-{}.json", std::process::id())),
        serde_json::to_vec_pretty(&observed).unwrap(),
    )
    .unwrap();
    std::fs::write(
        std::env::temp_dir().join(format!("a09-native-mismatches-{}.json", std::process::id())),
        serde_json::to_vec_pretty(&mismatch).unwrap(),
    )
    .unwrap();
    assert_eq!(
        used_headers.len(),
        headers["records"].as_array().unwrap().len(),
        "stale header difference"
    );
    assert_eq!(
        used.len(),
        differences["records"].as_array().unwrap().len(),
        "stale difference"
    );
    assert!(
        mismatch.is_empty(),
        "{} source mismatches, inspect a09-native-mismatches.json",
        mismatch.len()
    );
}
#[tokio::test]
async fn actual_yaml_env_tmdb_key_precedence_is_bearer_only_and_redacted() {
    for (i, (yaml, env, ready)) in [
        ("yaml-secret", None, true),
        ("yaml-secret", Some("env-secret"), true),
        ("yaml-secret", Some(""), false),
        ("yaml-secret", Some("   "), false),
        ("", None, false),
        ("   ", None, false),
    ]
    .into_iter()
    .enumerate()
    {
        let p =
            std::env::temp_dir().join(format!("a09-tmdb-secret-{}-{i}.yaml", std::process::id()));
        std::fs::write(&p, format!("engines:\n  tmdb:\n    api_key: '{yaml}'\n")).unwrap();
        let envs = env
            .map(|key| {
                std::collections::BTreeMap::from([("ENGINE_TMDB_API_KEY".into(), key.into())])
            })
            .unwrap_or_default();
        let config = ssx_config::load_config_with(Some(&p), &envs).unwrap();
        std::fs::remove_file(p).unwrap();
        let settings = CatalogMediaSettings::from_resolved(&config);
        assert!(!format!("{settings:?}").contains("yaml-secret"));
        assert!(!format!("{settings:?}").contains("env-secret"));
        let transport = Arc::new(Replay {
            replies: Mutex::new(VecDeque::from([
                json!({"status":500,"body":{"secret":"must never appear"}}),
            ])),
            requests: Mutex::new(vec![]),
        });
        let h = Arc::new(AdapterHttp::default());
        h.set_transport(transport.clone()).unwrap();
        let mut registry = ssx_registry::Registry::builtins().unwrap();
        adapters::register_catalog_media(
            &mut registry,
            h.clone(),
            Arc::new(AllowAllAdmission),
            settings,
        )
        .unwrap();
        let response = registry
            .instantiate("tmdb")
            .unwrap()
            .search(&SearchRequest {
                query: "test".into(),
                ..Default::default()
            })
            .await;
        assert_eq!(response.status, ssx_domain::EngineStatus::Error);
        {
            let requests = transport.requests.lock().unwrap();
            assert_eq!(requests.len(), usize::from(ready));
            if ready {
                let r = &requests[0];
                assert_eq!(
                    r.headers["authorization"],
                    format!("Bearer {}", env.unwrap_or(yaml))
                );
                assert!(!r.url.as_str().contains("api_key"));
            }
        }
        let diagnostic = response.error_message.unwrap();
        assert!(!diagnostic.contains("yaml-secret"));
        assert!(!diagnostic.contains("env-secret"));
        assert!(!diagnostic.contains("must never appear"));
        h.shutdown_and_wait().await;
    }
}
