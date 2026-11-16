//! The same full public SearchEngine replay is compiled into lib and integration tests.
use super::replay_http as http;
use super::{
    AdapterHttp, AllowAllAdmission, BiomedicalConfig, ClinicalTrialsAdapter, EuropePMCAdapter,
    PubChemAdapter, PubMedAdapter, UniProtAdapter,
};
use ssx_application::{SearchAdmission, SearchEngine};
use ssx_config::ConfigScalar;
use ssx_domain::SearchRequest;
use std::sync::{Arc, Mutex};
pub(super) const MOVED: [&str; 4] = [
    "clinicaltrials-config-timeout_ms-bool-True",
    "europepmc-config-timeout_ms-bool-True",
    "pubchem-config-timeout_ms-bool-True",
    "uniprot-config-timeout_ms-bool-True",
];
pub(super) fn source_record(id: &str) -> ssx_json::Value {
    let source: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../../docs/rust-rewrite/a07/source_complete.json"
    ))
    .unwrap();
    source["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"].as_str() == Some(id))
        .unwrap()
        .clone()
}
pub(super) fn transport(record: &ssx_json::Value) -> Arc<CapturedTransport> {
    Arc::new(CapturedTransport {
        replies: Mutex::new(record["responses"].as_array().unwrap().clone().into()),
        seen: Mutex::new(Vec::new()),
    })
}
pub(super) fn assert_consumed(transport: &CapturedTransport) {
    assert!(
        transport.replies.lock().unwrap().is_empty(),
        "unconsumed captured responses"
    );
}
fn completion(id: &str) {
    use std::io::Write;
    static RECEIPT: Mutex<()> = Mutex::new(());
    if let Some(path) = std::env::var_os("A07_COMPLETED_CASES") {
        let _guard = RECEIPT.lock().unwrap();
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        let mut line=ssx_json::to_vec(&ssx_json::json!({"id":id,"kind":"full_public_SearchEngine","compared":"original_config_status_requests_complete_output"})).unwrap();
        line.push(b'\n');
        file.write_all(&line).unwrap();
    }
}
fn q(query: &str) -> SearchRequest {
    SearchRequest {
        query: query.into(),
        page: ssx_domain::SearchPage::from_u64(8).expect("positive fixture page"),
        max_results: Some(1),
        date_from: Some("2024-01-01".into()),
        ..SearchRequest::default()
    }
}
pub(super) struct CapturedTransport {
    pub(super) replies: Mutex<std::collections::VecDeque<ssx_json::Value>>,
    pub(super) seen: Mutex<Vec<http::Request>>,
}
impl http::Transport for CapturedTransport {
    fn exchange(&self, request: http::Request) -> http::ExchangeFuture<'_> {
        Box::pin(async move {
            let url = request.url.clone();
            self.seen.lock().unwrap().push(request);
            let reply = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .ok_or(http::Error::RequestLimit)?;
            Ok(http::Response {
                status: reply["status"].as_u64().unwrap() as u16,
                url,
                headers: Default::default(),
                body: ssx_json::to_vec(&reply["body"]).unwrap(),
            })
        })
    }
}
fn raw_scalar(value: &ssx_json::Value) -> ConfigScalar {
    match value {
        ssx_json::Value::Bool(v) => ConfigScalar::Boolean(*v),
        ssx_json::Value::String(v) => ConfigScalar::Text(v.clone()),
        ssx_json::Value::Number(n) if n.is_f64() => ConfigScalar::Float(n.as_f64().unwrap()),
        ssx_json::Value::Number(n) => ConfigScalar::Integer(n.as_i64().unwrap()),
        _ => panic!("fixture config must be scalar"),
    }
}
fn normalized_results(results: &[ssx_domain::SearchResult]) -> ssx_json::Value {
    ssx_json::Value::Array(results.iter().map(|r|ssx_json::json!({"url":r.url,"title":r.title,"content":r.content,"engine":r.engine,"position":r.position,"score":r.score.get(),"category":r.category,"published_date":r.published_date,"thumbnail":r.thumbnail,"img_src":r.img_src,"media":r.media.as_ref().map(|m|ssx_json::json!({"media_type":m.media_type,"url":m.url,"thumbnail":m.thumbnail,"source":m.source,"width":m.width,"height":m.height,"duration":m.duration.as_ref()})),"payload":r.payload,"engines":r.engines,"tier":r.tier,"work_group":r.work_group})).collect())
}
pub(super) async fn replay_record(
    record: &ssx_json::Value,
    http: Arc<AdapterHttp>,
    transport: Arc<CapturedTransport>,
) -> Vec<ssx_json::Value> {
    let id = record["id"].as_str().unwrap();
    let mut mismatches = Vec::new();
    let mut diagnostics = Vec::new();
    let name = record["engine"].as_str().unwrap();
    let base = match name {
        "clinicaltrials" => "https://clinicaltrials.gov/api/v2/studies",
        "europepmc" => "https://www.ebi.ac.uk/europepmc/webservices/rest",
        "pubchem" => "https://pubchem.ncbi.nlm.nih.gov/rest/pug",
        "pubmed" => "https://eutils.ncbi.nlm.nih.gov/entrez/eutils",
        "uniprot" => "https://rest.uniprot.org/uniprotkb/search",
        _ => panic!(),
    };
    let mut config = BiomedicalConfig::defaults(base);
    for (key, value) in record["config"].as_object().unwrap() {
        match key.as_str() {
            "timeout_ms" => config.timeout_ms = raw_scalar(value),
            "max_results" => config.max_results = raw_scalar(value),
            "aggregate_timeout_ms" => config.aggregate_timeout_ms = Some(raw_scalar(value)),
            _ => panic!("unknown config fixture"),
        }
    }
    let a: Arc<dyn SearchAdmission> = Arc::new(AllowAllAdmission);
    let e: Box<dyn SearchEngine> = match name {
        "clinicaltrials" => Box::new(ClinicalTrialsAdapter::new(http.clone(), a, config)),
        "europepmc" => Box::new(EuropePMCAdapter::new(http.clone(), a, config)),
        "pubchem" => Box::new(PubChemAdapter::new(http.clone(), a, config)),
        "pubmed" => Box::new(PubMedAdapter::new(http.clone(), a, config)),
        "uniprot" => Box::new(UniProtAdapter::new(http.clone(), a, config)),
        _ => panic!(),
    };
    let out = e.search(&q(record["query"].as_str().unwrap())).await;
    let expected_status = record["output"]["status"].as_str().unwrap_or("error");
    let status = ssx_json::to_value(&out.status).unwrap();
    if status.as_str() != Some(expected_status) {
        mismatches.push(format!(
            "{id}: status expected {expected_status}, got {status}"
        ));
    }
    let expected_results = record["output"]
        .get("results")
        .cloned()
        .unwrap_or_else(|| ssx_json::json!([]));
    if record["output"].get("raised").is_none() {
        assert_eq!(
            ssx_json::json!({"answers":out.answers,"corrections":out.corrections,"infoboxes":out.infoboxes,"synthetic":out.synthetic}),
            ssx_json::json!({"answers":record["output"]["answers"],"corrections":record["output"]["corrections"],"infoboxes":record["output"]["infoboxes"],"synthetic":record["output"]["synthetic"]})
        );
    }
    let actual = normalized_results(&out.results);
    if actual != expected_results {
        mismatches.push(format!(
            "{id}: complete normalized results differ: expected {expected_results}; actual {actual}"
        ));
    }
    let seen = transport.seen.lock().unwrap().clone();
    let expected = record["requests"].as_array().unwrap();
    if seen.len() != expected.len() {
        mismatches.push(format!(
            "{id}: request count expected {}, got {}",
            expected.len(),
            seen.len()
        ));
    }
    for (actual, expected) in seen.iter().zip(expected) {
        if actual.method.as_str() != expected["method"].as_str().unwrap()
            || actual.url.as_str() != expected["url"].as_str().unwrap()
        {
            mismatches.push(format!(
                "{id}: request expected {} {}, got {} {}",
                expected["method"], expected["url"], actual.method, actual.url
            ));
        }
        for key in ["authorization", "user-agent", "accept"] {
            let expected = expected["headers"][key].as_str();
            if key == "user-agent" && expected.is_some_and(|s| s.starts_with("python-httpx/")) {
                continue;
            }
            let actual = actual.headers.get(key).map(|s| s.to_str().unwrap());
            if actual != expected {
                mismatches.push(format!(
                    "{id}: {key}: expected {expected:?}, got {actual:?}"
                ));
            }
        }
    }
    if out.error_message.as_deref() != record["output"]["error_message"].as_str() {
        diagnostics.push(ssx_json::json!({"id":id,"source_error":record["output"]["error_message"],"source_raised":record["output"].get("raised"),"native_status":out.status,"native_error":out.error_message}));
    }
    http.shutdown_and_wait().await;
    assert_eq!(
        transport.replies.lock().unwrap().len(),
        record["responses"].as_array().unwrap().len()
            - record["requests"].as_array().unwrap().len()
    );
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    if record["responses"].as_array().unwrap().len() == record["requests"].as_array().unwrap().len()
    {
        assert_consumed(&transport);
    }
    completion(id);
    diagnostics
}
