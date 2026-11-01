//! Deterministic direct-only replay of the complete frozen A07 biomedical matrix.
use crate::{
    adapters::{
        AllowAllAdmission, SearchAdmission,
        clinicaltrials::{BiomedicalConfig, ClinicalTrialsAdapter},
        europepmc::EuropePMCAdapter,
        pubchem::PubChemAdapter,
        pubmed::PubMedAdapter,
        uniprot::UniProtAdapter,
    },
    http::{AdapterHttp, Error, ExchangeFuture, Request, Response, Transport},
};
use ssx_application::SearchEngine;
use ssx_config::ConfigScalar;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

struct CapturedTransport {
    replies: Mutex<VecDeque<ssx_json::Value>>,
    seen: Mutex<Vec<Request>>,
}
impl Transport for CapturedTransport {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            let url = request.url.clone();
            self.seen.lock().unwrap().push(request);
            let reply = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .ok_or(Error::RequestLimit)?;
            Ok(Response {
                status: reply["status"].as_u64().unwrap() as u16,
                url,
                headers: Default::default(),
                body: ssx_json::to_vec(&reply["body"]).unwrap(),
            })
        })
    }
}
fn q(query: &str) -> ssx_domain::SearchRequest {
    ssx_domain::SearchRequest {
        query: query.into(),
        page: ssx_domain::SearchPage::from_u64(8).expect("positive fixture page"),
        max_results: Some(1),
        date_from: Some("2024-01-01".into()),
        ..Default::default()
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

#[tokio::test]
async fn actual_source_matrix_preserves_requests_and_full_normalized_results() {
    let matrix: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../../docs/rust-rewrite/a07/source_matrix.json"
    ))
    .unwrap();
    let mut mismatches = Vec::new();
    let mut diagnostics = Vec::new();
    let mut executed = std::collections::BTreeSet::new();
    let supplement: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../../docs/rust-rewrite/a07/source_supplement.json"
    ))
    .unwrap();
    let numeric: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../../docs/rust-rewrite/a07/source_numeric.json"
    ))
    .unwrap();
    let digits: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../../docs/rust-rewrite/a07/source_digits.json"
    ))
    .unwrap();
    let complete: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../../docs/rust-rewrite/a07/source_complete.json"
    ))
    .unwrap();
    let complete_rows = complete["records"].as_array().unwrap();
    assert_eq!(complete_rows.len(), 127);
    for old in matrix["records"]
        .as_array()
        .unwrap()
        .iter()
        .chain(supplement["records"].as_array().unwrap())
        .chain(numeric["records"].as_array().unwrap())
        .chain(digits["records"].as_array().unwrap())
    {
        let current = complete_rows
            .iter()
            .find(|row| row["id"] == old["id"])
            .expect("original control must survive full-field recapture");
        for key in ["engine", "query", "config", "responses", "requests"] {
            assert_eq!(
                current[key], old[key],
                "source recapture input/request drift at {} {key}",
                old["id"]
            );
        }
        for key in ["status", "error_message", "raised"] {
            assert_eq!(
                current["output"].get(key),
                old["output"].get(key),
                "source recapture status drift"
            );
        }
        if let Some(prior) = old["output"]["results"].as_array() {
            let current = current["output"]["results"].as_array().unwrap();
            assert_eq!(current.len(), prior.len());
            for (row, old) in current.iter().zip(prior) {
                for (key, value) in old.as_object().unwrap() {
                    assert_eq!(
                        &row[key.as_str()],
                        value,
                        "existing captured normalized field changed"
                    );
                }
            }
        }
    }
    let genes: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../../docs/rust-rewrite/a07/source_gene_shapes.json"
    ))
    .unwrap();
    let dates: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../../docs/rust-rewrite/a07/source_date_grammar.json"
    ))
    .unwrap();
    let sequences: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../../docs/rust-rewrite/a07/source_sequences.json"
    ))
    .unwrap();
    let empty_iterables: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../../docs/rust-rewrite/a07/source_empty_iterables.json"
    ))
    .unwrap();
    for record in complete_rows
        .iter()
        .chain(genes["records"].as_array().unwrap())
        .chain(dates["records"].as_array().unwrap())
        .chain(sequences["records"].as_array().unwrap())
        .chain(empty_iterables["records"].as_array().unwrap())
    {
        let id = record["id"].as_str().unwrap();
        assert!(executed.insert(id), "duplicate source control {id}");
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
        let transport = Arc::new(CapturedTransport {
            replies: Mutex::new(record["responses"].as_array().unwrap().clone().into()),
            seen: Mutex::new(Vec::new()),
        });
        let (owner, _driver) = AdapterHttp::replay_owner(transport.clone());
        let http = Arc::new(owner);
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
            mismatches.push(format!("{id}: complete normalized results differ: expected {expected_results}; actual {actual}"));
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
    }
    assert_eq!(
        executed.len(),
        172,
        "source capture must not be silently shortened"
    );
    // Diagnostics are explicit proposed differences, not silently normalized parity.
    if let Ok(path) = std::env::var("A07_DIAGNOSTIC_CAPTURE") {
        std::fs::write(path, ssx_json::to_string(&diagnostics).unwrap()).unwrap();
    }
    let ledger: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../../docs/rust-rewrite/a07/proposed-differences.json"
    ))
    .unwrap();
    let approved_shape=ledger["records"].as_array().unwrap().iter().map(|r|ssx_json::json!({"id":r["id"],"source_error":r["source_error"],"source_raised":r["source_raised"],"native_status":r["native_status"],"native_error":r["native_error"]})).collect::<Vec<_>>();
    assert_eq!(
        diagnostics, approved_shape,
        "diagnostic drift requires an explicit per-case ledger update and review; pending entries are not acceptance"
    );
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
