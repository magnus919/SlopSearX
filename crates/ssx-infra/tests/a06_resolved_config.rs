//! Real YAML/environment -> F06 -> scholarly settings -> registered factories.
#[path = "support/a06_case.rs"]
mod a06_case;
use base64::Engine as _;
use ssx_application::SearchContext;
use ssx_domain::AdapterResponse;
use ssx_infra::{
    adapters::{
        AllowAllAdmission,
        arxiv_expat::AtomPool,
        scholarly_config::Scalar,
        scholarly_runtime::{self, ScholarlySettings},
    },
    http::{AdapterHttp, ExchangeFuture, Request, Response, Transport},
};
use ssx_json::{Value, json};
use std::{
    collections::BTreeMap,
    num::NonZeroUsize,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
fn normalized(r: &AdapterResponse) -> Value {
    assert!(r.results.iter().all(|v| v.media.is_none()));
    json!({"results":r.results.iter().map(|v|json!({"url":v.url,"title":v.title,"content":v.content,"engine":v.engine,"engines":v.engines.iter().collect::<Vec<_>>(),"score":v.score,"position":v.position,"category":v.category,"published_date":v.published_date,"thumbnail":v.thumbnail,"img_src":v.img_src,"tier":v.tier,"media":null,"payload":v.payload,"work_group":v.work_group})).collect::<Vec<_>>(),"status":r.status,"error_message":r.error_message,"answers":r.answers,"corrections":r.corrections,"infoboxes":r.infoboxes,"synthetic":r.synthetic})
}
fn scalar(v: &Scalar) -> Value {
    match v {
        Scalar::Boolean(v) => json!({"kind":"bool","value":v}),
        Scalar::Integer(v) => json!({"kind":"int","value":v}),
        Scalar::Text(v) => json!({"kind":"str","value":v.as_ref()}),
        Scalar::Float(v) => {
            json!({"kind":"float","value":if v.is_finite(){json!(v)}else{json!({"$source_nonfinite_float":if v.is_nan(){"nan"}else if *v>0.{"inf"}else{"-inf"}})}})
        }
    }
}
struct Replay {
    row: Value,
    calls: AtomicUsize,
}
impl Transport for Replay {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            let expected = &self.row["requests"][n];
            assert_eq!(
                request.method.as_str(),
                expected["method"].as_str().unwrap()
            );
            assert_eq!(
                request.url.as_str(),
                expected["url"].as_str().unwrap(),
                "{}",
                self.row["id"]
            );
            let key = expected["headers"]["x-api-key"].as_str();
            assert_eq!(
                request
                    .headers
                    .get("x-api-key")
                    .map(|v| v.to_str().unwrap()),
                key
            );
            for secret in ["offline-file-secret", "offline-env-secret"] {
                assert!(!request.url.as_str().contains(secret));
            }
            if self.row["engine"] == "openalex" {
                assert_eq!(expected["headers"]["user-agent"], "python-httpx/0.28.1");
                assert_eq!(
                    request.headers["user-agent"],
                    ssx_infra::http::DEFAULT_USER_AGENT
                );
            } else {
                assert_eq!(
                    request.headers["user-agent"].to_str().unwrap(),
                    expected["headers"]["user-agent"].as_str().unwrap()
                );
            }
            Ok(Response {
                status: 200,
                url: request.url,
                headers: Default::default(),
                body: base64::engine::general_purpose::STANDARD
                    .decode(self.row["response_body_b64"].as_str().unwrap())
                    .unwrap(),
            })
        })
    }
}
struct ConfigFile(PathBuf);
impl Drop for ConfigFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[tokio::test]
async fn actual_source_yaml_env_precedence_raw_scalars_base_defaults_and_credentials_reach_factories()
 {
    let source: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a06/source-resolved-config.json"
    ))
    .unwrap();
    let mut ids = std::collections::BTreeSet::new();
    for row in source["records"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        assert!(ids.insert(id));
        let name = row["engine"].as_str().unwrap();
        let temp = ConfigFile(std::env::temp_dir().join(format!(
                "ssx-a06-config-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )));
        std::fs::create_dir(&temp.0).unwrap();
        let file = temp.0.join("config.yaml");
        std::fs::write(&file, row["yaml"].as_str().unwrap()).unwrap();
        let env: BTreeMap<_, _> = row["env"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
            .collect();
        let resolved = ssx_config::load_config_with(Some(&file), &env).unwrap();
        let redacted = format!("{resolved:?}");
        assert!(!redacted.contains("offline-file-secret"));
        assert!(!redacted.contains("offline-env-secret"));
        assert_eq!(
            resolved.engines[name].enabled,
            row["resolved"]["enabled"].as_bool().unwrap()
        );
        assert_eq!(
            resolved.engines[name].base_url,
            row["resolved"]["base_url"].as_str().unwrap()
        );
        let settings = ScholarlySettings::from_resolved(&resolved);
        let (timeout, max, base) = match name {
            "arxiv" => (
                &settings.arxiv.timeout_ms,
                &settings.arxiv.max_results,
                &settings.arxiv.base_url,
            ),
            "openalex" => (
                &settings.openalex.timeout_ms,
                &settings.openalex.max_results,
                &settings.openalex.base_url,
            ),
            _ => (
                &settings.semanticscholar.timeout_ms,
                &settings.semanticscholar.max_results,
                &settings.semanticscholar.base_url,
            ),
        };
        assert_eq!(scalar(timeout), row["resolved"]["timeout_ms"], "{id}");
        assert_eq!(scalar(max), row["resolved"]["max_results"], "{id}");
        if row["source_service_omitted_base"] == false {
            assert_eq!(base.as_ref(), row["resolved"]["base_url"].as_str().unwrap());
        }
        assert_eq!(
            format!("{resolved:?}"),
            redacted,
            "settings construction preserves F06 config"
        );
        let replay = Arc::new(Replay {
            row: row.clone(),
            calls: AtomicUsize::new(0),
        });
        let http = Arc::new(AdapterHttp::default());
        http.set_transport(replay.clone()).unwrap();
        let parser = AtomPool::new(
            PathBuf::from(env!("CARGO_BIN_EXE_ssx-atom-parser")),
            NonZeroUsize::new(1).unwrap(),
        )
        .unwrap();
        let mut registry = ssx_registry::Registry::builtins().unwrap();
        let metadata = registry.descriptor(name).unwrap().clone();
        scholarly_runtime::register(
            &mut registry,
            http.clone(),
            Arc::new(AllowAllAdmission),
            settings,
            parser.clone(),
        )
        .unwrap();
        assert_eq!(registry.descriptor(name).unwrap(), &metadata);
        assert!(registry.is_available(name));
        let request = ssx_domain::SearchRequest {
            query: row["query"].as_str().unwrap().into(),
            ..Default::default()
        };
        let out = registry
            .instantiate(name)
            .unwrap()
            .search_with_context(
                &request,
                SearchContext {
                    deadline: Instant::now() + Duration::from_secs(30),
                },
            )
            .await;
        assert_eq!(normalized(&out), row["output"], "{id}");
        assert_eq!(
            replay.calls.load(Ordering::SeqCst),
            row["requests"].as_array().unwrap().len()
        );
        assert_eq!(parser.available_operations(), 1);
        assert_eq!(parser.available_slots(), 1);
        parser.shutdown().await.unwrap();
        http.shutdown_and_wait().await;
        let path = temp.0.clone();
        drop(temp);
        assert!(!path.exists());
        a06_case::completed(
            "ssx-infra/a06_resolved_config/actual_source_yaml_env_precedence_raw_scalars_base_defaults_and_credentials_reach_factories",
            "docs/rust-rewrite/a06/source-resolved-config.json",
            id,
            "default",
            "exact_outcome_request_config",
        );
    }
    assert_eq!(ids.len(), 36);
}
