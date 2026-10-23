use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchEngine};
use ssx_config::ConfigScalar;
use ssx_domain::{EngineStatus, SearchRequest};
use ssx_infra::{
    adapters::{
        AllowAllAdmission,
        clinicaltrials::{BiomedicalConfig, ClinicalTrialsAdapter},
        europepmc::EuropePMCAdapter,
        pubchem::PubChemAdapter,
        pubmed::PubMedAdapter,
        uniprot::UniProtAdapter,
    },
    http::AdapterHttp,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
const CT: &str =
    include_str!("../../../docs/rust-rewrite/a07/fixtures/clinicaltrials-success.json");
const PC_SEARCH: &str = include_str!("../../../docs/rust-rewrite/a07/fixtures/pubchem-search.json");
const PC_DETAIL: &str = include_str!("../../../docs/rust-rewrite/a07/fixtures/pubchem-detail.json");
const PM_SEARCH: &str = include_str!("../../../docs/rust-rewrite/a07/fixtures/pubmed-search.json");
const PM_SUMMARY: &str =
    include_str!("../../../docs/rust-rewrite/a07/fixtures/pubmed-summary.json");
const UP: &str = include_str!("../../../docs/rust-rewrite/a07/fixtures/uniprot-success.json");
const EPMC: &str =
    include_str!("../../../docs/rust-rewrite/a07/fixtures/europepmc-biomedical.json");
const PPR: &str = include_str!("../../../docs/rust-rewrite/a07/fixtures/europepmc-preprints.json");
async fn server(
    replies: Vec<(u16, String, Duration)>,
) -> (String, Arc<Mutex<Vec<String>>>, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let requests = seen.clone();
    let task = tokio::spawn(async move {
        for (status, body, delay) in replies {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut b = [0; 4096];
                let n = stream.read(&mut b).await.unwrap();
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&b[..n]);
                if bytes.windows(4).any(|v| v == b"\r\n\r\n") {
                    break;
                }
            }
            requests
                .lock()
                .unwrap()
                .push(String::from_utf8(bytes).unwrap());
            tokio::time::sleep(delay).await;
            let header = format!(
                "HTTP/1.1 {status} Status\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(header.as_bytes()).await;
            let _ = stream.write_all(body.as_bytes()).await;
        }
    });
    (format!("http://{addr}"), seen, task)
}
fn engine(
    name: &str,
    base: &str,
    config: Option<BiomedicalConfig>,
    admission: Option<Arc<dyn SearchAdmission>>,
) -> Box<dyn SearchEngine> {
    let c = config.unwrap_or_else(|| BiomedicalConfig::defaults(base));
    let h = Arc::new(AdapterHttp::default());
    let a = admission.unwrap_or_else(|| Arc::new(AllowAllAdmission));
    match name {
        "clinicaltrials" => Box::new(ClinicalTrialsAdapter::new(h, a, c)),
        "europepmc" => Box::new(EuropePMCAdapter::new(h, a, c)),
        "pubchem" => Box::new(PubChemAdapter::new(h, a, c)),
        "pubmed" => Box::new(PubMedAdapter::new(h, a, c)),
        "uniprot" => Box::new(UniProtAdapter::new(h, a, c)),
        _ => panic!("unknown engine"),
    }
}
fn q(query: &str) -> SearchRequest {
    SearchRequest {
        query: query.into(),
        page: 8,
        max_results: Some(1),
        date_from: Some("2024-01-01".into()),
        ..SearchRequest::default()
    }
}
fn reply(body: &str) -> (u16, String, Duration) {
    (200, body.into(), Duration::ZERO)
}
#[tokio::test]
async fn clinicaltrials_preserves_request_and_full_normalization() {
    let (base, seen, task) = server(vec![reply(CT)]).await;
    let out = engine("clinicaltrials", &base, None, None)
        .search(&q("heart disease"))
        .await;
    task.await.unwrap();
    assert_eq!(out.status, EngineStatus::Ok);
    let r = &out.results[0];
    assert_eq!(r.url, "https://clinicaltrials.gov/study/NCT00000001");
    assert_eq!(r.title, "Study of heart disease prevention");
    assert_eq!(
        r.content,
        "RECRUITING — PHASE3 — Heart Disease; Cardiovascular"
    );
    assert_eq!(r.position, 1);
    assert_eq!(r.score.get(), 1.);
    assert!(r.payload.is_none() && r.media.is_none());
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert!(seen[0].starts_with(
        "GET /?query.term=heart+disease&pageSize=10&format=json&sort=%40relevance HTTP/1.1"
    ));
    assert!(!seen[0].to_ascii_lowercase().contains("authorization:"));
}
#[tokio::test]
async fn europepmc_lite_preserves_attribution_preprints_and_payload() {
    for (body, source) in [(EPMC, "MED"), (PPR, "PPR")] {
        let (base, seen, task) = server(vec![reply(body)]).await;
        let out = engine("europepmc", &base, None, None)
            .search(&q("test"))
            .await;
        task.await.unwrap();
        assert_eq!(out.status, EngineStatus::Ok);
        assert_eq!(out.results.len(), 5);
        let r = &out.results[0];
        assert!(
            r.url
                .starts_with(&format!("https://europepmc.org/article/{source}/"))
        );
        assert!(
            r.content
                .starts_with("Source: Europe PMC / EMBL-EBI (https://europepmc.org)")
        );
        assert_eq!(r.position, 0);
        assert_eq!(r.score.get(), 1.);
        if source == "PPR" {
            assert!(r.content.contains("Preprint (not peer reviewed)"));
        }
        let p = r.payload.as_ref().unwrap();
        assert_eq!(p["domain"], "science");
        assert_eq!(p["provenance"]["engine"], "europepmc");
        assert_eq!(p["data"]["source"], source);
        assert!(p["data"].get("abstract").is_none());
        let wire = &seen.lock().unwrap()[0];
        assert!(wire.starts_with(
            "GET /search?query=test&format=json&resultType=lite&pageSize=10 HTTP/1.1"
        ));
        assert!(!wire.contains("date_from") && !wire.contains("cursorMark"));
    }
}
#[tokio::test]
async fn pubchem_two_stage_request_count_order_and_compound_fallback() {
    let (base, seen, task) = server(vec![reply(PC_SEARCH), reply(PC_DETAIL)]).await;
    let out = engine("pubchem", &base, None, None)
        .search(&q("aspirin/a b"))
        .await;
    task.await.unwrap();
    assert_eq!(out.status, EngineStatus::Ok);
    assert_eq!(out.results.len(), 2);
    assert_eq!(out.results[0].title, "aspirin");
    assert_eq!(out.results[0].content, "Formula: C9H8O4 | MW: 180.16");
    assert_eq!(out.results[1].title, "Compound CID 962");
    assert_eq!(out.results[1].content, "Formula: C17H21NO4 | MW: 303.35");
    let wires = seen.lock().unwrap();
    assert_eq!(wires.len(), 2);
    assert!(wires[0].starts_with("GET /compound/name/aspirin/a%20b/cids/JSON "));
    assert!(wires[1].starts_with("GET /compound/cid/2244,962/JSON "));
}
#[tokio::test]
async fn pubmed_two_stage_success_and_complete_payload() {
    let (base, seen, task) = server(vec![reply(PM_SEARCH), reply(PM_SUMMARY)]).await;
    let out = engine("pubmed", &base, None, None)
        .search(&q("covid vaccine"))
        .await;
    task.await.unwrap();
    assert_eq!(out.status, EngineStatus::Ok);
    assert_eq!(out.results.len(), 2);
    let r = &out.results[0];
    assert_eq!(r.url, "https://pubmed.ncbi.nlm.nih.gov/12345/");
    assert_eq!(r.title, "COVID-19 vaccine efficacy");
    assert_eq!(
        r.content,
        "New England Journal of Medicine — Smith J, Doe A"
    );
    assert_eq!(r.published_date.as_deref(), Some("2024 Jan"));
    let payload = r.payload.as_ref().unwrap();
    assert_eq!(
        payload["data"]["authors"],
        ssx_json::json!(["Smith J", "Doe A"])
    );
    assert_eq!(payload["data"]["pmid"], "12345");
    let wires = seen.lock().unwrap();
    assert_eq!(wires.len(), 2);
    assert!(wires[0].starts_with(
        "GET /esearch.fcgi?db=pubmed&term=covid+vaccine&retmax=10&retmode=json&sort=relevance "
    ));
    assert!(wires[1].starts_with("GET /esummary.fcgi?db=pubmed&id=12345%2C67890&retmode=json "));
    for wire in wires.iter() {
        assert!(wire.to_lowercase().contains("user-agent: slopsearx/0.1.0"));
        assert!(!wire.to_lowercase().contains("authorization:"));
    }
}
#[tokio::test]
async fn uniprot_simple_gene_overfetch_and_content() {
    let (base, seen, task) = server(vec![reply(UP)]).await;
    let out = engine("uniprot", &base, None, None).search(&q("p53")).await;
    task.await.unwrap();
    assert_eq!(out.status, EngineStatus::Ok);
    assert_eq!(out.results[0].title, "P04637 — Cellular tumor antigen p53");
    assert_eq!(out.results[0].content, "Homo sapiens — Gene: TP53");
    assert_eq!(
        out.results[0].url,
        "https://www.uniprot.org/uniprotkb/P04637/entry"
    );
    assert_eq!(out.results[0].score.get(), 1.);
    let wires = seen.lock().unwrap();
    assert!(wires[0].starts_with("GET /?query=%28p53%29+OR+%28gene%3Ap53%29&size=30&format=json "));
    assert!(wires[0].to_lowercase().contains("accept: application/json"));
}
#[tokio::test]
async fn valid_empty_and_schema_failure_never_fabricate_results() {
    for (name, empty, bad) in [
        ("clinicaltrials", r#"{"studies":[]}"#, r#"{"studies":null}"#),
        (
            "europepmc",
            r#"{"resultList":{"result":[]}}"#,
            r#"{"resultList":{}}"#,
        ),
        (
            "pubchem",
            r#"{"IdentifierList":{"CID":[]}}"#,
            r#"{"IdentifierList":null}"#,
        ),
        (
            "pubmed",
            r#"{"esearchresult":{"idlist":[]}}"#,
            r#"{"esearchresult":{"idlist":[1]}}"#,
        ),
        ("uniprot", r#"{"results":[]}"#, r#"{"results":null}"#),
    ] {
        for (body, status) in [
            (empty, EngineStatus::Ok),
            (bad, EngineStatus::Error),
            ("{", EngineStatus::Error),
        ] {
            let (base, seen, task) = server(vec![reply(body)]).await;
            let out = engine(name, &base, None, None).search(&q("x")).await;
            task.await.unwrap();
            assert_eq!(out.status, status, "{name}: {body}");
            assert!(out.results.is_empty());
            assert_eq!(seen.lock().unwrap().len(), 1);
        }
    }
}
#[tokio::test]
async fn all_engines_classify_http_failures_and_timeout() {
    for name in [
        "clinicaltrials",
        "europepmc",
        "pubchem",
        "pubmed",
        "uniprot",
    ] {
        for (status, expected) in [
            (429, EngineStatus::RateLimited),
            (503, EngineStatus::Error),
            (
                404,
                if name == "pubchem" {
                    EngineStatus::Ok
                } else {
                    EngineStatus::Error
                },
            ),
        ] {
            let (base, _, task) = server(vec![(status, String::new(), Duration::ZERO)]).await;
            let out = engine(name, &base, None, None).search(&q("x")).await;
            task.await.unwrap();
            assert_eq!(out.status, expected, "{name} {status}");
            assert!(out.results.is_empty());
        }
        let (base, _, task) = server(vec![(200, "{}".into(), Duration::from_millis(200))]).await;
        let mut c = BiomedicalConfig::defaults(&base);
        c.timeout_ms = ConfigScalar::Integer(20);
        let out = engine(name, &base, Some(c), None).search(&q("x")).await;
        assert_eq!(out.status, EngineStatus::Timeout, "{name}");
        task.abort();
    }
}
struct Deny;
impl SearchAdmission for Deny {
    fn admit<'a>(&'a self, _: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async { AdmissionDecision::Denied })
    }
}
#[tokio::test]
async fn admission_denial_precedes_invalid_configuration_and_dispatch() {
    for name in [
        "clinicaltrials",
        "europepmc",
        "pubchem",
        "pubmed",
        "uniprot",
    ] {
        let out = engine(name, "not a url", None, Some(Arc::new(Deny)))
            .search(&q("x"))
            .await;
        assert_eq!(out.status, EngineStatus::RateLimited);
        assert_eq!(out.error_message.as_deref(), Some("rate limited"));
        assert!(out.results.is_empty());
    }
}
#[tokio::test]
async fn pubmed_aggregate_deadline_bounds_second_request() {
    let (base, seen, task) = server(vec![
        (200, PM_SEARCH.into(), Duration::from_millis(100)),
        (200, PM_SUMMARY.into(), Duration::from_millis(800)),
    ])
    .await;
    let mut c = BiomedicalConfig::defaults(&base);
    c.timeout_ms = ConfigScalar::Integer(1000);
    c.aggregate_timeout_ms = Some(ConfigScalar::Integer(500));
    let start = std::time::Instant::now();
    let out = engine("pubmed", &base, Some(c), None).search(&q("x")).await;
    assert_eq!(out.status, EngineStatus::Timeout);
    assert!(start.elapsed() < Duration::from_millis(1500));
    assert!(out.results.is_empty());
    assert_eq!(seen.lock().unwrap().len(), 2);
    task.abort();
}
#[tokio::test]
async fn pubmed_missing_first_summary_preserves_position_and_all_authors() {
    let summary = r#"{"result":{"67890":{"title":"Two","authors":[{"name":"A"},{"name":"B"},{"name":"C"},{"name":"D"},{"name":"E"},{"name":"F"}],"articleids":[{"idtype":"doi","value":"10/x"},{"idtype":"pmc","value":"PMC2"}],"pubtype":["Journal Article"]}}}"#;
    let (base, _, task) = server(vec![reply(PM_SEARCH), reply(summary)]).await;
    let out = engine("pubmed", &base, None, None).search(&q("x")).await;
    task.await.unwrap();
    assert_eq!(out.status, EngineStatus::Ok);
    let r = &out.results[0];
    assert_eq!(r.position, 2);
    assert_eq!(r.content, " — A, B, C");
    let p = r.payload.as_ref().unwrap();
    assert_eq!(p["data"]["authors"].as_array().unwrap().len(), 6);
    assert_eq!(p["data"]["doi"], "10/x");
    assert_eq!(p["data"]["pmcid"], "PMC2");
}

#[tokio::test]
async fn dropping_each_adapter_search_closes_the_active_socket() {
    for name in [
        "clinicaltrials",
        "europepmc",
        "pubchem",
        "pubmed",
        "uniprot",
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (accepted, received) = tokio::sync::oneshot::channel();
        let peer = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut data = Vec::new();
            loop {
                let mut buf = [0; 4096];
                let n = stream.read(&mut buf).await.unwrap();
                assert_ne!(n, 0);
                data.extend_from_slice(&buf[..n]);
                if data.windows(4).any(|b| b == b"\r\n\r\n") {
                    break;
                }
            }
            accepted.send(()).unwrap();
            let mut b = [0; 1];
            tokio::time::timeout(Duration::from_secs(2), stream.read(&mut b))
                .await
                .unwrap()
                .unwrap()
        });
        let e = engine(name, &base, None, None);
        let search = tokio::spawn(async move { e.search(&q("cancel")).await });
        tokio::time::timeout(Duration::from_secs(2), received)
            .await
            .unwrap()
            .unwrap();
        search.abort();
        assert!(search.await.unwrap_err().is_cancelled());
        assert_eq!(
            peer.await.unwrap(),
            0,
            "{name} socket stayed open after search drop"
        );
    }
}

#[tokio::test]
async fn pubchem_second_stage_has_independent_timeout_and_404_is_error() {
    let (base, seen, task) = server(vec![
        (200, PC_SEARCH.into(), Duration::from_millis(300)),
        (200, PC_DETAIL.into(), Duration::from_millis(300)),
    ])
    .await;
    let mut c = BiomedicalConfig::defaults(&base);
    c.timeout_ms = ConfigScalar::Integer(500);
    let out = engine("pubchem", &base, Some(c), None)
        .search(&q("aspirin"))
        .await;
    let mut task = task;
    match tokio::time::timeout(Duration::from_secs(5), &mut task).await {
        Ok(joined) => joined.unwrap(),
        Err(_) => {
            task.abort();
            let _ = task.await;
            panic!(
                "owned PubChem fixture did not finish; response: {:?}",
                out.status
            );
        }
    }
    assert_eq!(out.status, EngineStatus::Ok);
    assert_eq!(out.results.len(), 2);
    assert_eq!(seen.lock().unwrap().len(), 2);
    let (base, seen, task) =
        server(vec![reply(PC_SEARCH), (404, String::new(), Duration::ZERO)]).await;
    let out = engine("pubchem", &base, None, None)
        .search(&q("aspirin"))
        .await;
    let mut task = task;
    match tokio::time::timeout(Duration::from_secs(5), &mut task).await {
        Ok(joined) => joined.unwrap(),
        Err(_) => {
            task.abort();
            let _ = task.await;
            panic!(
                "owned PubChem fixture did not finish; response: {:?}",
                out.status
            );
        }
    }
    assert_eq!(out.status, EngineStatus::Error);
    assert!(out.results.is_empty());
    assert_eq!(seen.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn pubmed_invalid_timeout_settings_dispatch_nothing() {
    for (field, value) in [
        ("timeout", ConfigScalar::Boolean(true)),
        ("timeout", ConfigScalar::Integer(0)),
        ("timeout", ConfigScalar::Text("bad".into())),
        ("aggregate", ConfigScalar::Integer(0)),
        ("aggregate", ConfigScalar::Integer(-1)),
        ("aggregate", ConfigScalar::Float(f64::NAN)),
        ("aggregate", ConfigScalar::Integer(6000)),
        ("aggregate", ConfigScalar::Boolean(true)),
    ] {
        let mut c = BiomedicalConfig::defaults("http://127.0.0.1:1");
        if field == "timeout" {
            c.timeout_ms = value;
        } else {
            c.aggregate_timeout_ms = Some(value);
        }
        let out = engine("pubmed", "", Some(c), None).search(&q("test")).await;
        assert_eq!(out.status, EngineStatus::Error);
        assert!(out.results.is_empty());
        assert!(out.error_message.unwrap().contains("timeout"));
    }
}

#[tokio::test]
async fn uniprot_exact_gene_identifier_and_stable_overfetch_promotion() {
    let body = r#"{"results":[{"primaryAccession":"Q99999","uniProtkbId":"Q99999_TEST","genes":[{"geneName":{"value":"VIMBP"}}]},{"primaryAccession":"P20152","uniProtkbId":"P20152_TEST","genes":[{"geneName":{"value":"Vim"}}]},{"primaryAccession":"P08670","uniProtkbId":"P08670_TEST","genes":[{"geneName":{"value":"VIM"}}]}]}"#;
    for (query, target, ids, scores) in [
        (
            "VIM",
            "%28VIM%29+OR+%28gene%3AVIM%29",
            vec!["P20152", "P08670"],
            vec![2., 2.],
        ),
        (
            "P08670",
            "%28P08670%29+OR+%28accession%3AP08670%29",
            vec!["P08670", "Q99999"],
            vec![3., 1.],
        ),
        (
            "gene:VIM",
            "gene%3AVIM",
            vec!["P20152", "P08670"],
            vec![2., 2.],
        ),
        (
            "id:P08670_TEST",
            "id%3AP08670_TEST",
            vec!["P08670", "Q99999"],
            vec![3., 1.],
        ),
        (
            "(gene:VIM) AND organism_id:9606",
            "%28gene%3AVIM%29+AND+organism_id%3A9606",
            vec!["Q99999", "P20152"],
            vec![1., 1.],
        ),
    ] {
        let (base, seen, task) = server(vec![reply(body)]).await;
        let mut c = BiomedicalConfig::defaults(&base);
        c.max_results = ConfigScalar::Integer(2);
        let out = engine("uniprot", &base, Some(c), None)
            .search(&q(query))
            .await;
        task.await.unwrap();
        assert_eq!(out.status, EngineStatus::Ok);
        assert_eq!(
            out.results
                .iter()
                .map(|r| r.url.split('/').nth_back(1).unwrap())
                .collect::<Vec<_>>(),
            ids
        );
        assert_eq!(
            out.results
                .iter()
                .map(|r| r.score.get())
                .collect::<Vec<_>>(),
            scores
        );
        assert_eq!(
            out.results.iter().map(|r| r.position).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert!(
            seen.lock().unwrap()[0]
                .starts_with(&format!("GET /?query={target}&size=6&format=json "))
        );
    }
}

#[tokio::test]
async fn europepmc_skips_incomplete_records_and_parses_source_dates_without_claiming_filters() {
    for (date, expected) in [
        ("2026-99-99", None),
        ("20260102", Some("2026-01-02")),
        ("2026-W01-1", Some("2025-12-29")),
        ("2026-W01", Some("2025-12-29")),
    ] {
        let body = ssx_json::json!({"resultList":{"result":[{}, {"source":"MED","id":"1","title":"title","firstPublicationDate":date,"journalTitle":[],"authorString":{"name":"x"},"pmcid":"PMC1","doi":"10.1/test"}]}});
        let (base, _, task) = server(vec![reply(&ssx_json::to_string(&body).unwrap())]).await;
        let out = engine("europepmc", &base, None, None)
            .search(&q("test"))
            .await;
        task.await.unwrap();
        assert_eq!(out.status, EngineStatus::Ok);
        assert_eq!(out.results.len(), 1);
        let r = &out.results[0];
        assert_eq!(r.published_date.as_deref(), expected);
        assert_eq!(
            r.content,
            "Source: Europe PMC / EMBL-EBI (https://europepmc.org)"
        );
        assert_eq!(
            r.payload.as_ref().unwrap()["data"]["journal"],
            ssx_json::json!([])
        );
        assert_eq!(r.payload.as_ref().unwrap()["data"]["pmid"], "1");
        assert_eq!(r.payload.as_ref().unwrap()["data"]["pmcid"], "PMC1");
    }
}

use ssx_infra::http as replay_http;
#[path = "support/a07_source_replay.rs"]
mod source_replay;
use source_replay::CapturedTransport;
#[tokio::test]
async fn actual_source_matrix_preserves_requests_and_full_normalized_results() {
    let matrix: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a07/source_matrix.json"
    ))
    .unwrap();
    let mut moved = std::collections::BTreeSet::new();
    let mut replayed = 0usize;
    let mut diagnostics = Vec::new();
    let mut executed = std::collections::BTreeSet::new();
    let supplement: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a07/source_supplement.json"
    ))
    .unwrap();
    let numeric: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a07/source_numeric.json"
    ))
    .unwrap();
    let digits: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a07/source_digits.json"
    ))
    .unwrap();
    let complete: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a07/source_complete.json"
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
        "../../../docs/rust-rewrite/a07/source_gene_shapes.json"
    ))
    .unwrap();
    let dates: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a07/source_date_grammar.json"
    ))
    .unwrap();
    let sequences: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a07/source_sequences.json"
    ))
    .unwrap();
    let empty_iterables: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a07/source_empty_iterables.json"
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
        if source_replay::MOVED.contains(&id) {
            assert_eq!(&source_replay::source_record(id), record);
            moved.insert(id);
            continue;
        }
        let transport = source_replay::transport(record);
        let http = Arc::new(AdapterHttp::default());
        http.set_transport(transport.clone()).unwrap();
        diagnostics.extend(source_replay::replay_record(record, http, transport).await);
        replayed += 1;
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
        "../../../docs/rust-rewrite/a07/proposed-differences.json"
    ))
    .unwrap();
    let approved_shape=ledger["records"].as_array().unwrap().iter().map(|r|ssx_json::json!({"id":r["id"],"source_error":r["source_error"],"source_raised":r["source_raised"],"native_status":r["native_status"],"native_error":r["native_error"]})).collect::<Vec<_>>();
    assert_eq!(
        diagnostics, approved_shape,
        "diagnostic drift requires an explicit per-case ledger update and review; pending entries are not acceptance"
    );
    assert_eq!(replayed, 168);
    assert_eq!(moved, source_replay::MOVED.into_iter().collect());
}

#[tokio::test]
async fn resolved_yaml_environment_scalars_reach_engine_specific_semantics() {
    use std::collections::BTreeMap;
    let defaults = ssx_config::load_config_with(
        Some(std::path::Path::new("/nonexistent/a07-config")),
        &BTreeMap::new(),
    )
    .unwrap();
    assert!(!defaults.engines["europepmc"].enabled);
    for (name, yaml, env_value, expected) in [
        ("clinicaltrials", "true", "false", EngineStatus::Ok),
        ("clinicaltrials", "'2'", "3", EngineStatus::Error),
        ("europepmc", "'2'", "3", EngineStatus::Ok),
    ] {
        let body = if name == "europepmc" { EPMC } else { CT };
        let (base, seen, task) = server(vec![reply(body)]).await;
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("a07-{}-{unique}.yaml", std::process::id()));
        std::fs::write(&path,format!("engines:\n  {name}:\n    enabled: true\n    base_url: '{base}'\n    max_results: {yaml}\n")).unwrap();
        let env = BTreeMap::from([(
            format!("ENGINE_{}_MAX_RESULTS", name.to_uppercase()),
            env_value.into(),
        )]);
        let resolved = ssx_config::load_config_with(Some(&path), &env).unwrap();
        std::fs::remove_file(&path).unwrap();
        let c = BiomedicalConfig::from_resolved(&resolved.engines[name]);
        if yaml == "true" {
            assert_eq!(c.max_results, ConfigScalar::Boolean(false));
        } else {
            assert_eq!(c.max_results, ConfigScalar::Text("3".into()));
        }
        let out = engine(name, &base, Some(c), None)
            .search(&q("config"))
            .await;
        task.await.unwrap();
        assert_eq!(out.status, expected);
        if name == "europepmc" {
            assert_eq!(out.results.len(), 3);
        } else if yaml == "true" {
            assert!(out.results.is_empty());
            assert!(seen.lock().unwrap()[0].contains("pageSize=false"));
        }
    }
}

#[tokio::test]
async fn pubmed_both_stage_http_and_summary_schema_regressions() {
    for (code, status) in [(429, EngineStatus::RateLimited), (500, EngineStatus::Error)] {
        for stage in [0, 1] {
            let replies = if stage == 0 {
                vec![(code, String::new(), Duration::ZERO)]
            } else {
                vec![reply(PM_SEARCH), (code, String::new(), Duration::ZERO)]
            };
            let (base, seen, task) = server(replies).await;
            let out = engine("pubmed", &base, None, None).search(&q("x")).await;
            task.await.unwrap();
            assert_eq!(out.status, status);
            assert_eq!(seen.lock().unwrap().len(), stage + 1);
            assert!(out.results.is_empty());
            if code == 429 {
                assert!(out.error_message.is_none());
            }
        }
    }
    for (body, message) in [
        ("[]", "ESummary payload must be an object"),
        (r#"{"result":null}"#, "ESummary payload is missing result"),
        (r#"{"result":[]}"#, "ESummary payload is missing result"),
        (
            r#"{"result":{"12345":null}}"#,
            "ESummary entry for PMID 12345 must be an object",
        ),
        (
            r#"{"result":{"12345":{"title":1}}}"#,
            "ESummary field title must be text",
        ),
        (
            r#"{"result":{"12345":{"authors":null}}}"#,
            "ESummary authors for PMID 12345 must be a list",
        ),
        (
            r#"{"result":{"12345":{"authors":[null]}}}"#,
            "ESummary author for PMID 12345 must be an object",
        ),
        (
            r#"{"result":{"12345":{"authors":[{"name":1}]}}}"#,
            "ESummary field name must be text",
        ),
    ] {
        let (base, _, task) = server(vec![reply(PM_SEARCH), reply(body)]).await;
        let out = engine("pubmed", &base, None, None).search(&q("x")).await;
        task.await.unwrap();
        assert_eq!(out.status, EngineStatus::Error, "{body}");
        assert_eq!(out.error_message.as_deref(), Some(message));
        assert!(out.results.is_empty());
    }
}

#[tokio::test]
async fn local_denial_has_source_diagnostic_and_zero_transport_exchanges() {
    for name in [
        "clinicaltrials",
        "europepmc",
        "pubchem",
        "pubmed",
        "uniprot",
    ] {
        let transport = Arc::new(CapturedTransport {
            replies: Mutex::new(Default::default()),
            seen: Mutex::new(Vec::new()),
        });
        let h = Arc::new(AdapterHttp::default());
        h.set_transport(transport.clone()).unwrap();
        let a: Arc<dyn SearchAdmission> = Arc::new(Deny);
        let c = BiomedicalConfig::defaults("http://127.0.0.1:1");
        let e: Box<dyn SearchEngine> = match name {
            "clinicaltrials" => Box::new(ClinicalTrialsAdapter::new(h.clone(), a, c)),
            "europepmc" => Box::new(EuropePMCAdapter::new(h.clone(), a, c)),
            "pubchem" => Box::new(PubChemAdapter::new(h.clone(), a, c)),
            "pubmed" => Box::new(PubMedAdapter::new(h.clone(), a, c)),
            "uniprot" => Box::new(UniProtAdapter::new(h.clone(), a, c)),
            _ => unreachable!(),
        };
        let out = e.search(&q("denied")).await;
        assert_eq!(out.status, EngineStatus::RateLimited);
        assert_eq!(out.error_message.as_deref(), Some("rate limited"));
        assert!(out.results.is_empty());
        assert!(transport.seen.lock().unwrap().is_empty());
        h.shutdown_and_wait().await;
    }
}

#[tokio::test]
async fn uniprot_default_window_preserves_all_related_records_and_species_ties() {
    let source: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a07/source_complete.json"
    ))
    .unwrap();
    let original = source["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "up-query-VIM")
        .unwrap();
    let body = ssx_json::to_string(&original["responses"][0]["body"]).unwrap();
    for (query, ids, scores) in [
        ("VIM", vec!["P20152", "P08670", "Q99999"], vec![2., 2., 1.]),
        (
            "P08670",
            vec!["P08670", "Q99999", "P20152"],
            vec![3., 1., 1.],
        ),
        (
            "gene:VIM",
            vec!["P20152", "P08670", "Q99999"],
            vec![2., 2., 1.],
        ),
    ] {
        let (base, seen, task) = server(vec![reply(&body)]).await;
        let out = engine("uniprot", &base, None, None).search(&q(query)).await;
        task.await.unwrap();
        assert_eq!(out.status, EngineStatus::Ok);
        assert_eq!(
            out.results
                .iter()
                .map(|r| r.url.split('/').nth_back(1).unwrap())
                .collect::<Vec<_>>(),
            ids
        );
        assert_eq!(
            out.results
                .iter()
                .map(|r| r.score.get())
                .collect::<Vec<_>>(),
            scores
        );
        assert!(seen.lock().unwrap()[0].contains("size=30"));
        if query != "P08670" {
            assert_eq!(out.results[0].content, "Mus musculus — Gene: Vim");
            assert_eq!(out.results[1].content, "Homo sapiens — Gene: VIM");
        }
    }
}
