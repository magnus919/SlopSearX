//! Test-only in-process HTTP and admission witnesses; no sockets or providers.
use super::register;
use crate::{
    http::{AdapterHttp, ExchangeFuture, Request, Response, Transport},
    search::SearchSettings,
};
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission};
use ssx_config::{Config, ConfigScalar, EngineConfig, Secret};
use ssx_domain::{EngineStatus, SearchRequest};
use ssx_registry::Registry;
use std::sync::{Arc, Mutex};

const ENGINES: [&str; 4] = ["abuseipdb", "greynoise", "otx", "urlhaus"];

struct Admission {
    allowed: bool,
    names: Mutex<Vec<&'static str>>,
}
impl SearchAdmission for Admission {
    fn admit<'a>(&'a self, name: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async move {
            self.names.lock().unwrap().push(name);
            if self.allowed {
                AdmissionDecision::Allowed
            } else {
                AdmissionDecision::Denied
            }
        })
    }
}
#[derive(Default)]
struct Recorder {
    requests: Mutex<Vec<Request>>,
}
impl Transport for Recorder {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            let url = request.url.clone();
            self.requests.lock().unwrap().push(request);
            // All four real adapters classify this before body normalization.
            Ok(Response {
                status: 429,
                url,
                headers: Default::default(),
                body: Vec::new(),
            })
        })
    }
}
fn config(timeout: ConfigScalar) -> Config {
    Config {
        engines: ENGINES
            .into_iter()
            .map(|name| {
                (
                    name.into(),
                    EngineConfig {
                        enabled: true,
                        base_url: format!("http://127.0.0.1:1/{name}"),
                        api_key: Secret::new(Some(format!("fixture-only-{name}"))),
                        timeout_ms: timeout.clone(),
                        max_results: ConfigScalar::Boolean(true),
                        ..Default::default()
                    },
                )
            })
            .collect(),
        ..Default::default()
    }
}
fn registry(config: &Config, http: Arc<AdapterHttp>, admission: Arc<Admission>) -> Registry {
    let mut registry = Registry::builtins().unwrap();
    let settings = SearchSettings::from_environment(&Default::default(), "presence").unwrap();
    register(&mut registry, http, admission, config, &settings).unwrap();
    registry
}
fn query() -> SearchRequest {
    SearchRequest {
        query: "1.2.3.4".into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn four_real_factories_use_resolved_endpoints_credentials_and_shared_ports() {
    let http = Arc::new(AdapterHttp::default());
    let recorder = Arc::new(Recorder::default());
    http.set_transport(recorder.clone()).unwrap();
    let admission = Arc::new(Admission {
        allowed: true,
        names: Mutex::new(Vec::new()),
    });
    let registry = registry(
        &config(ConfigScalar::Float(1500.5)),
        http.clone(),
        admission.clone(),
    );
    let query = query();
    for name in ENGINES {
        let engine = registry
            .instantiate(name)
            .expect("real attached A11 factory");
        let result = engine.search(&query).await;
        assert_eq!(result.status, EngineStatus::RateLimited);
        assert!(result.results.is_empty());
    }
    http.shutdown_and_wait().await;
    assert_eq!(*admission.names.lock().unwrap(), ENGINES);
    let requests = recorder.requests.lock().unwrap();
    assert_eq!(
        requests.len(),
        4,
        "all factories must use the injected shared HTTP owner"
    );
    let expected = [
        ("/abuseipdb/check", "key"),
        ("/greynoise/v3/enterprise/1.2.3.4", "key"),
        ("/otx/indicators/IPv4/1.2.3.4/general", "x-otx-api-key"),
        ("/urlhaus/v1/host/", "auth-key"),
    ];
    for ((name, request), (path, header)) in ENGINES.into_iter().zip(requests.iter()).zip(expected)
    {
        assert_eq!(request.url.host_str(), Some("127.0.0.1"));
        assert_eq!(request.url.port(), Some(1));
        assert_eq!(request.url.path(), path);
        assert_eq!(
            request.headers[header].to_str().unwrap(),
            format!("fixture-only-{name}")
        );
    }
    assert_eq!(
        requests[0].url.query(),
        Some("ipAddress=1.2.3.4&maxAgeInDays=90&verbose=")
    );
    assert_eq!(requests[3].method, reqwest::Method::POST);
    assert_eq!(requests[3].body, b"host=1.2.3.4");
}

#[tokio::test]
async fn four_factories_obey_shared_denial_and_preserve_known_unmounted_catalog() {
    let http = Arc::new(AdapterHttp::default());
    let recorder = Arc::new(Recorder::default());
    http.set_transport(recorder.clone()).unwrap();
    let admission = Arc::new(Admission {
        allowed: false,
        names: Mutex::new(Vec::new()),
    });
    let registry = registry(
        &config(ConfigScalar::Integer(1500)),
        http.clone(),
        admission.clone(),
    );
    assert_eq!(registry.len(), 51);
    let available = registry
        .descriptors()
        .filter(|d| registry.is_available(&d.name))
        .count();
    assert_eq!(available, 33);
    assert!(registry.descriptor("google").is_some());
    assert!(!registry.is_available("google"));
    assert!(registry.instantiate("google").is_none());
    let query = query();
    for name in ENGINES {
        let result = registry.instantiate(name).unwrap().search(&query).await;
        assert_eq!(result.status, EngineStatus::RateLimited);
        assert_eq!(result.error_message.as_deref(), Some("rate limited"));
    }
    http.shutdown_and_wait().await;
    assert_eq!(*admission.names.lock().unwrap(), ENGINES);
    assert!(
        recorder.requests.lock().unwrap().is_empty(),
        "denied scope must not enter HTTP transport"
    );
}

#[tokio::test]
async fn resolved_numeric_text_timeout_is_not_coerced_by_factory_wiring() {
    let http = Arc::new(AdapterHttp::default());
    let recorder = Arc::new(Recorder::default());
    http.set_transport(recorder.clone()).unwrap();
    let admission = Arc::new(Admission {
        allowed: true,
        names: Mutex::new(Vec::new()),
    });
    let registry = registry(
        &config(ConfigScalar::Text("1500.5".into())),
        http.clone(),
        admission.clone(),
    );
    let query = query();
    for name in ENGINES {
        let result = registry.instantiate(name).unwrap().search(&query).await;
        assert_eq!(result.status, EngineStatus::Error);
        assert_eq!(result.error_message.as_deref(), Some("invalid timeout_ms"));
    }
    http.shutdown_and_wait().await;
    assert_eq!(*admission.names.lock().unwrap(), ENGINES);
    assert!(recorder.requests.lock().unwrap().is_empty());
}
