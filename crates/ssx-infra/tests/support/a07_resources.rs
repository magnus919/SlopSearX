//! Real private adapter consumers; synthetic bounded bodies, no providers.
use super::*;
use crate::{
    adapters::{
        AllowAllAdmission, europepmc::EuropePMCAdapter, pubchem::PubChemAdapter,
        pubmed::PubMedAdapter, uniprot::UniProtAdapter,
    },
    http::{ExchangeFuture, Response, Transport},
};
use std::{
    collections::VecDeque,
    future::{Future, poll_fn},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::Poll,
};
const ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/rust-rewrite/a07/resource-bodies/"
);
struct Replies {
    bodies: Mutex<VecDeque<Vec<u8>>>,
    seen: AtomicUsize,
    requests: Mutex<Vec<(String, String)>>,
}
impl Transport for Replies {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            self.seen.fetch_add(1, Ordering::SeqCst);
            self.requests
                .lock()
                .unwrap()
                .push((request.method.to_string(), request.url.to_string()));
            Ok(Response {
                status: 200,
                url: request.url,
                headers: Default::default(),
                body: self
                    .bodies
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("unexpected request"),
            })
        })
    }
}
enum Engine {
    Ct(ClinicalTrialsAdapter),
    Ep(EuropePMCAdapter),
    Pm(PubMedAdapter),
    Pc(PubChemAdapter),
    Up(UniProtAdapter),
}
impl Engine {
    async fn run<'a>(
        &self,
        q: &SearchRequest,
        deadline: Option<Instant>,
        budget: &'a Budget,
        observer: Observer<'_>,
    ) -> Result<ProducedBatch<'a>> {
        match self {
            Self::Ct(e) => e.run_observed(q, deadline, budget, observer).await,
            Self::Ep(e) => e.run_observed(q, deadline, budget, observer).await,
            Self::Pm(e) => e.run_observed(q, deadline, budget, observer).await,
            Self::Pc(e) => e.run_observed(q, deadline, budget, observer).await,
            Self::Up(e) => e.run_observed(q, deadline, budget, observer).await,
        }
    }
}
fn harness(name: &str, bodies: Vec<Vec<u8>>) -> (Engine, Arc<AdapterHttp>, Arc<Replies>) {
    harness_config(name, bodies, 1000, 5000)
}
fn harness_config(
    name: &str,
    bodies: Vec<Vec<u8>>,
    max: i64,
    timeout: i64,
) -> (Engine, Arc<AdapterHttp>, Arc<Replies>) {
    let replies = Arc::new(Replies {
        bodies: Mutex::new(bodies.into()),
        seen: AtomicUsize::new(0),
        requests: Mutex::new(Vec::new()),
    });
    let http = Arc::new(AdapterHttp::default());
    http.set_transport(replies.clone()).unwrap();
    let base = match name {
        "clinicaltrials" => "https://clinicaltrials.gov/api/v2/studies",
        "europepmc" => "https://www.ebi.ac.uk/europepmc/webservices/rest",
        "pubmed" => "https://eutils.ncbi.nlm.nih.gov/entrez/eutils",
        "pubchem" => "https://pubchem.ncbi.nlm.nih.gov/rest/pug",
        _ => "https://rest.uniprot.org/uniprotkb/search",
    };
    let mut cfg = BiomedicalConfig::defaults(base);
    cfg.max_results = ConfigScalar::Integer(max);
    cfg.timeout_ms = ConfigScalar::Integer(timeout);
    let admission = Arc::new(AllowAllAdmission);
    let engine = match name {
        "clinicaltrials" => Engine::Ct(ClinicalTrialsAdapter::new(http.clone(), admission, cfg)),
        "europepmc" => Engine::Ep(EuropePMCAdapter::new(http.clone(), admission, cfg)),
        "pubmed" => Engine::Pm(PubMedAdapter::new(http.clone(), admission, cfg)),
        "pubchem" => Engine::Pc(PubChemAdapter::new(http.clone(), admission, cfg)),
        "uniprot" => Engine::Up(UniProtAdapter::new(http.clone(), admission, cfg)),
        _ => panic!("unknown engine"),
    };
    (engine, http, replies)
}
fn query() -> SearchRequest {
    SearchRequest {
        query: "context".into(),
        ..Default::default()
    }
}
fn prepared(id: &str) -> (String, Vec<Vec<u8>>) {
    let manifest: ssx_json::Value =
        ssx_json::from_slice(&std::fs::read(format!("{ROOT}manifest.json")).unwrap()).unwrap();
    let case = manifest["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"].as_str() == Some(id))
        .unwrap();
    let bodies = case["response_bodies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            let file = std::path::Path::new(v["path"].as_str().unwrap())
                .file_name()
                .unwrap()
                .to_str()
                .unwrap();
            let bytes = std::fs::read(format!("{ROOT}{file}")).unwrap();
            assert!(bytes.len() <= source_json::MAX_BYTES);
            bytes
        })
        .collect();
    (case["engine"].as_str().unwrap().into(), bodies)
}
async fn until_phase<F: Future>(
    future: std::pin::Pin<&mut F>,
    phase: &AtomicUsize,
    wanted: Phase,
    budget: &Budget,
    baseline: &AtomicUsize,
    extra: usize,
) {
    let mut future = future;
    poll_fn(|cx| match future.as_mut().poll(cx) {
        Poll::Ready(_) => panic!("adapter completed before in-progress phase evidence"),
        Poll::Pending => {
            let reached = phase.load(Ordering::SeqCst) == wanted as usize + 1;
            let used = budget.snapshot().unwrap().used;
            if reached && used >= baseline.load(Ordering::SeqCst) + extra {
                Poll::Ready(())
            } else {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    })
    .await;
}
fn result_value(r: &SearchResult) -> ssx_json::Value {
    ssx_json::json!({"url":r.url,"title":r.title,"content":r.content,"engine":r.engine,"engines":r.engines.iter().collect::<Vec<_>>(),"score":r.score.get(),"position":r.position,"category":r.category,"published_date":r.published_date,"thumbnail":r.thumbnail,"img_src":r.img_src,"tier":r.tier,"media":r.media.as_ref().map(|m|ssx_json::json!({"media_type":m.media_type,"url":m.url,"thumbnail":m.thumbnail,"source":m.source,"width":m.width,"height":m.height,"duration":m.duration.as_ref()})),"payload":r.payload,"work_group":r.work_group})
}
#[tokio::test]
async fn actual_adapter_payload_controls_preserve_full_results_and_later_source_error_precedence() {
    let fixture: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../../docs/rust-rewrite/a06/source-publication-envelope.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let bodies = case["wire_response_bodies"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().as_bytes().to_vec())
            .collect();
        let (engine, http, replies) =
            harness_config(case["engine"].as_str().unwrap(), bodies, 10, 5000);
        let budget = Budget::new(ROW_BYTES).unwrap();
        let mut q = query();
        q.query = "test".into();
        let result = engine.run(&q, None, &budget, None).await;
        let response = finish_batch(result, Instant::now()).await;
        let expected = &case["output"];
        assert_eq!(
            format!("{:?}", response.status).to_lowercase(),
            expected["status"].as_str().unwrap()
        );
        assert_eq!(
            response.error_message.as_deref(),
            expected["error_message"].as_str()
        );
        assert_eq!(
            ssx_json::Value::Array(response.results.iter().map(result_value).collect()),
            expected["results"]
        );
        assert_eq!(
            replies.seen.load(Ordering::SeqCst),
            case["requests"].as_array().unwrap().len()
        );
        let expected_calls = case["requests"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                (
                    v["method"].as_str().unwrap().to_owned(),
                    v["url"].as_str().unwrap().to_owned(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(*replies.requests.lock().unwrap(), expected_calls);
        assert!(
            response.answers.is_empty()
                && response.corrections.is_empty()
                && response.infoboxes.is_empty()
        );
        assert!(!response.synthetic);
        assert_eq!(budget.snapshot().unwrap().used, 0);
        http.shutdown_and_wait().await;
    }
}
#[tokio::test]
async fn all_eight_prepared_bodies_fit_candidate_quota_and_transfer_without_cloning() {
    for (id, count) in [
        ("ct-large-one-row", 1),
        ("pc-large-one-row", 1),
        ("up-large-one-row", 1),
        ("pm-surrogate-author-after-display", 1),
        ("pm-large-one-row", 1),
        ("ep-large-one-row", 1),
        ("ep-1000-realistic", 1000),
        ("pm-200-realistic", 200),
    ] {
        let (name, bodies) = prepared(id);
        let (engine, http, _) = harness(&name, bodies);
        let budget = Budget::new(ROW_BYTES).unwrap();
        let q = query();
        let batch = engine.run(&q, None, &budget, None).await.unwrap();
        assert_eq!(batch.results().len(), count, "{id}");
        assert_eq!(
            budget.snapshot().unwrap().used,
            batch.charged_bytes(),
            "only batch storage survives run"
        );
        let peak = budget.snapshot().unwrap().peak;
        assert!(peak <= ROW_BYTES);
        eprintln!("A07_ENVELOPE {id} peak={peak} cap={ROW_BYTES}");
        let response = finish_batch(Ok(batch), Instant::now()).await;
        assert_eq!(response.status, EngineStatus::Ok, "{id}");
        assert_eq!(response.results.len(), count);
        assert_eq!(budget.snapshot().unwrap().used, 0);
        http.shutdown_and_wait().await;
    }
}
#[tokio::test]
async fn tiny_quota_is_terminal_error_and_never_partial_success() {
    for id in [
        "ct-large-one-row",
        "pc-large-one-row",
        "up-large-one-row",
        "pm-large-one-row",
        "ep-large-one-row",
    ] {
        let (name, bodies) = prepared(id);
        let (engine, http, _) = harness(&name, bodies);
        let budget = Budget::new(1024).unwrap();
        let q = query();
        let result = engine.run(&q, None, &budget, None).await;
        let response = finish_batch(result, Instant::now()).await;
        assert_eq!(response.status, EngineStatus::Error, "{id}");
        assert_eq!(
            response.error_message.as_deref(),
            Some("biomedical normalization resource limit exceeded")
        );
        assert!(response.results.is_empty());
        let snapshot = budget.snapshot().unwrap();
        assert_eq!(snapshot.used, 0);
        assert!(snapshot.peak <= snapshot.limit);
        http.shutdown_and_wait().await;
    }
}
#[tokio::test]
async fn same_root_quota_is_not_reset_per_row_or_payload_field() {
    let mut peaks = Vec::new();
    for count in [1, 2] {
        let body = ssx_json::json!({"resultList":{"result":(0..count).map(|i|ssx_json::json!({"source":"MED","id":i.to_string(),"title":"title","doi":"x".repeat(1024)})).collect::<Vec<_>>()}});
        let (engine, http, _) = harness("europepmc", vec![ssx_json::to_vec(&body).unwrap()]);
        let budget = Budget::new(ROW_BYTES).unwrap();
        let q = query();
        let batch = engine.run(&q, None, &budget, None).await.unwrap();
        peaks.push(budget.snapshot().unwrap().peak);
        drop(batch);
        assert_eq!(budget.snapshot().unwrap().used, 0);
        http.shutdown_and_wait().await;
    }
    assert!(peaks[1] > peaks[0]);
    let cap = peaks[0] + (peaks[1] - peaks[0]) / 2;
    for count in [1, 2] {
        let body = ssx_json::json!({"resultList":{"result":(0..count).map(|i|ssx_json::json!({"source":"MED","id":i.to_string(),"title":"title","doi":"x".repeat(1024)})).collect::<Vec<_>>()}});
        let (engine, http, _) = harness("europepmc", vec![ssx_json::to_vec(&body).unwrap()]);
        let budget = Budget::new(cap).unwrap();
        let q = query();
        let response =
            finish_batch(engine.run(&q, None, &budget, None).await, Instant::now()).await;
        assert_eq!(
            response.status,
            if count == 1 {
                EngineStatus::Ok
            } else {
                EngineStatus::Error
            }
        );
        assert_eq!(response.results.len(), if count == 1 { 1 } else { 0 });
        assert_eq!(budget.snapshot().unwrap().used, 0);
        http.shutdown_and_wait().await;
    }
}
#[tokio::test]
async fn dropping_real_large_inner_loop_futures_releases_all_scratch_and_batch_charges() {
    for id in [
        "ct-large-one-row",
        "pc-large-one-row",
        "up-large-one-row",
        "pm-large-one-row",
    ] {
        let (name, bodies) = prepared(id);
        let (engine, http, _) = harness(&name, bodies);
        let budget = Budget::new(ROW_BYTES).unwrap();
        let q = query();
        let phase = AtomicUsize::new(0);
        let baseline = AtomicUsize::new(0);
        let observer = |p| {
            if p == Phase::Inner {
                phase.store(p as usize + 1, Ordering::SeqCst);
            }
        };
        let mut future = Box::pin(engine.run(&q, None, &budget, Some(&observer)));
        until_phase(future.as_mut(), &phase, Phase::Inner, &budget, &baseline, 1).await;
        assert!(budget.snapshot().unwrap().used > 0);
        drop(future);
        assert_eq!(budget.snapshot().unwrap().used, 0, "{id}");
        http.shutdown_and_wait().await;
    }
}
#[tokio::test]
async fn dropping_actual_payload_projection_after_live_allocation_releases_ownership() {
    for id in ["pm-large-one-row", "ep-large-one-row"] {
        let (name, bodies) = prepared(id);
        let (engine, http, _) = harness(&name, bodies);
        let budget = Budget::new(ROW_BYTES).unwrap();
        let q = query();
        let phase = AtomicUsize::new(0);
        let baseline = AtomicUsize::new(0);
        let observer = |p| {
            if p == Phase::Payload {
                baseline.store(budget.snapshot().unwrap().used, Ordering::SeqCst);
                phase.store(p as usize + 1, Ordering::SeqCst);
            }
        };
        let mut future = Box::pin(engine.run(&q, None, &budget, Some(&observer)));
        until_phase(
            future.as_mut(),
            &phase,
            Phase::Payload,
            &budget,
            &baseline,
            32768,
        )
        .await;
        assert!(budget.snapshot().unwrap().used > baseline.load(Ordering::SeqCst));
        drop(future);
        assert_eq!(budget.snapshot().unwrap().used, 0);
        http.shutdown_and_wait().await;
    }
}
#[tokio::test]
async fn actual_payload_projection_obeys_absolute_context_after_source_rows_finish() {
    for id in ["pm-large-one-row", "ep-large-one-row"] {
        let (name, bodies) = prepared(id);
        let (engine, http, _) = harness(&name, bodies);
        let budget = Budget::new(ROW_BYTES).unwrap();
        let q = query();
        let phase = AtomicUsize::new(0);
        let baseline = AtomicUsize::new(0);
        let observer = |p| {
            if p == Phase::Payload {
                baseline.store(budget.snapshot().unwrap().used, Ordering::SeqCst);
                phase.store(p as usize + 1, Ordering::SeqCst);
            }
        };
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut future = Box::pin(engine.run(&q, Some(deadline), &budget, Some(&observer)));
        until_phase(
            future.as_mut(),
            &phase,
            Phase::Payload,
            &budget,
            &baseline,
            32768,
        )
        .await;
        std::thread::sleep(
            deadline.saturating_duration_since(Instant::now()) + Duration::from_millis(1),
        );
        let result = future.await;
        let response = finish_batch(result, Instant::now()).await;
        assert_eq!(response.status, EngineStatus::Timeout);
        assert!(response.results.is_empty());
        assert_eq!(budget.snapshot().unwrap().used, 0);
        http.shutdown_and_wait().await;
    }
}
#[tokio::test]
async fn source_decode_context_is_enforced_after_network_completion_for_all_five() {
    for name in [
        "clinicaltrials",
        "europepmc",
        "pubchem",
        "pubmed",
        "uniprot",
    ] {
        let mut object = match name {
            "clinicaltrials" => ssx_json::json!({"studies":[]}),
            "europepmc" => ssx_json::json!({"resultList":{"result":[]}}),
            "pubchem" => ssx_json::json!({"IdentifierList":{"CID":[]}}),
            "pubmed" => ssx_json::json!({"esearchresult":{"idlist":[]}}),
            _ => ssx_json::json!({"results":[]}),
        };
        object.as_object_mut().unwrap().insert(
            "ignored".into(),
            ssx_json::Value::String("x".repeat(262144)),
        );
        let (engine, http, replies) = harness(name, vec![ssx_json::to_vec(&object).unwrap()]);
        let budget = Budget::new(ROW_BYTES).unwrap();
        let q = query();
        let phase = AtomicUsize::new(0);
        let baseline = AtomicUsize::new(0);
        let observer = |p| {
            baseline.store(budget.snapshot().unwrap().used, Ordering::SeqCst);
            phase.store(p as usize + 1, Ordering::SeqCst);
        };
        let deadline = Instant::now() + Duration::from_millis(250);
        let mut future = Box::pin(engine.run(&q, Some(deadline), &budget, Some(&observer)));
        until_phase(
            future.as_mut(),
            &phase,
            Phase::Decode,
            &budget,
            &baseline,
            0,
        )
        .await;
        assert_eq!(replies.seen.load(Ordering::SeqCst), 1);
        assert!(replies.bodies.lock().unwrap().is_empty());
        assert_eq!(
            budget.snapshot().unwrap().used,
            baseline.load(Ordering::SeqCst),
            "no new row allocation during source decode; pre-request scratch remains owned"
        );
        std::thread::sleep(
            deadline.saturating_duration_since(Instant::now()) + Duration::from_millis(1),
        );
        let response = finish_batch(future.await, Instant::now()).await;
        assert_eq!(response.status, EngineStatus::Timeout, "{name}");
        assert!(response.results.is_empty());
        assert_eq!(budget.snapshot().unwrap().used, 0);
        http.shutdown_and_wait().await;
    }
}
#[tokio::test]
async fn byte_resource_bound_remains_independent_of_generous_row_budget() {
    let bytes = vec![b' '; source_json::MAX_BYTES + 1];
    for name in [
        "clinicaltrials",
        "europepmc",
        "pubchem",
        "pubmed",
        "uniprot",
    ] {
        let (engine, http, _) = harness(name, vec![bytes.clone()]);
        let budget = Budget::new(ROW_BYTES).unwrap();
        let q = query();
        let response =
            finish_batch(engine.run(&q, None, &budget, None).await, Instant::now()).await;
        assert_eq!(response.status, EngineStatus::Error);
        assert!(response.results.is_empty());
        assert_eq!(budget.snapshot().unwrap().used, 0);
        http.shutdown_and_wait().await;
    }
}
#[tokio::test]
async fn source_repr_profiles_match_legacy_with_charged_cooperative_output() {
    for (depth, points, surrogate) in [
        (1, 1000, false),
        (10, 1000, false),
        (90, 1000, false),
        (1, 30000, false),
        (1, 30000, true),
    ] {
        let mut value = Value::String(if surrogate {
            Text::from_points(vec![0xd800; points])
        } else {
            Text::from("\u{200b}".repeat(points).as_str())
        });
        for _ in 0..depth {
            value = Value::Array(vec![value]);
        }
        let budget = Budget::new(ROW_BYTES).unwrap();
        let start = Instant::now();
        let result = owned_stringify(&budget, &value, None).await;
        let elapsed = start.elapsed();
        let expected = value.repr();
        match result {
            Ok(out) => {
                assert_eq!(out.as_str(), expected);
                eprintln!(
                    "A07_REPR depth={depth} points={points} surrogate={surrogate} elapsed_us={} output={} peak={}",
                    elapsed.as_micros(),
                    out.len(),
                    budget.snapshot().unwrap().peak
                );
                drop(out);
            }
            Err(f) => {
                assert_eq!(f.status, EngineStatus::Error);
                eprintln!(
                    "A07_REPR_REFUSED depth={depth} points={points} elapsed_us={}",
                    elapsed.as_micros()
                );
            }
        }
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
}

#[tokio::test]
async fn legacy_direct_normalization_does_not_reuse_expired_network_timer() {
    for name in ["clinicaltrials", "europepmc", "pubchem", "uniprot"] {
        for contextual in [false, true] {
            let body = match name {
                "clinicaltrials" => br#"{"studies":[]}"#.to_vec(),
                "europepmc" => br#"{"resultList":{"result":[]}}"#.to_vec(),
                "pubchem" => br#"{"IdentifierList":{"CID":[]}}"#.to_vec(),
                _ => br#"{"results":[]}"#.to_vec(),
            };
            let (engine, http, replies) = harness_config(name, vec![body], 10, 50);
            let budget = Budget::new(ROW_BYTES).unwrap();
            let q = query();
            let observer = |phase| {
                if phase == Phase::Decode {
                    std::thread::sleep(Duration::from_millis(60));
                }
            };
            let deadline = contextual.then(|| Instant::now() + Duration::from_millis(50));
            let response = finish_batch(
                engine.run(&q, deadline, &budget, Some(&observer)).await,
                Instant::now(),
            )
            .await;
            assert_eq!(replies.seen.load(Ordering::SeqCst), 1);
            assert_eq!(
                response.status,
                if contextual {
                    EngineStatus::Timeout
                } else {
                    EngineStatus::Ok
                },
                "{name}/{contextual}"
            );
            assert_eq!(budget.snapshot().unwrap().used, 0);
            http.shutdown_and_wait().await;
        }
    }
}
#[tokio::test]
async fn five_concurrent_realistic_operation_owners_have_explicit_aggregate_envelope() {
    let (n1, b1) = prepared("ct-large-one-row");
    let (n2, b2) = prepared("pc-large-one-row");
    let (n3, b3) = prepared("up-large-one-row");
    let (n4, b4) = prepared("pm-200-realistic");
    let (n5, b5) = prepared("ep-1000-realistic");
    let (e1, h1, _) = harness(&n1, b1);
    let (e2, h2, _) = harness(&n2, b2);
    let (e3, h3, _) = harness(&n3, b3);
    let (e4, h4, _) = harness(&n4, b4);
    let (e5, h5, _) = harness(&n5, b5);
    let budgets = [
        Budget::new(ROW_BYTES).unwrap(),
        Budget::new(ROW_BYTES).unwrap(),
        Budget::new(ROW_BYTES).unwrap(),
        Budget::new(ROW_BYTES).unwrap(),
        Budget::new(ROW_BYTES).unwrap(),
    ];
    let q = query();
    let (r1, r2, r3, r4, r5) = tokio::join!(
        e1.run(&q, None, &budgets[0], None),
        e2.run(&q, None, &budgets[1], None),
        e3.run(&q, None, &budgets[2], None),
        e4.run(&q, None, &budgets[3], None),
        e5.run(&q, None, &budgets[4], None)
    );
    let batches = [
        r1.unwrap(),
        r2.unwrap(),
        r3.unwrap(),
        r4.unwrap(),
        r5.unwrap(),
    ];
    let peak_sum: usize = budgets.iter().map(|b| b.snapshot().unwrap().peak).sum();
    let retained: usize = budgets.iter().map(|b| b.snapshot().unwrap().used).sum();
    assert_eq!(
        retained,
        batches
            .iter()
            .map(ProducedBatch::charged_bytes)
            .sum::<usize>()
    );
    assert!(peak_sum <= 5 * ROW_BYTES);
    eprintln!(
        "A07_AGGREGATE sum_of_individual_peaks={peak_sum} retained_batch_bytes={retained} operation_allowance_sum={} input_and_G02_excluded=true",
        5 * ROW_BYTES
    );
    drop(batches);
    assert!(budgets.iter().all(|b| b.snapshot().unwrap().used == 0));
    for http in [h1, h2, h3, h4, h5] {
        http.shutdown_and_wait().await;
    }
}
#[tokio::test]
async fn first_phase_row_scratch_quota_failure_precedes_any_payload_projection() {
    for id in ["pm-large-one-row", "ep-large-one-row"] {
        let (name, bodies) = prepared(id);
        let (engine, http, _) = harness(&name, bodies);
        let budget = Budget::new(1024).unwrap();
        let phase = AtomicUsize::new(0);
        let observer = |p| {
            phase.store(p as usize + 1, Ordering::SeqCst);
        };
        let q = query();
        let response = finish_batch(
            engine.run(&q, None, &budget, Some(&observer)).await,
            Instant::now(),
        )
        .await;
        assert_eq!(response.status, EngineStatus::Error);
        assert!(response.results.is_empty());
        assert_ne!(phase.load(Ordering::SeqCst), Phase::Payload as usize + 1);
        assert_eq!(budget.snapshot().unwrap().used, 0);
        http.shutdown_and_wait().await;
    }
}

#[tokio::test]
async fn source_sized_pubmed_error_text_keeps_charge_until_response_handoff() {
    let id = "1".repeat(100000);
    let search = ssx_json::json!({"esearchresult":{"idlist":[id]}});
    let mut entries = ssx_json::Map::<String, ssx_json::Value>::new();
    entries.insert(id.clone(), ssx_json::json!([]));
    let summary = ssx_json::json!({"result":entries});
    let (engine, http, _) = harness(
        "pubmed",
        vec![
            ssx_json::to_vec(&search).unwrap(),
            ssx_json::to_vec(&summary).unwrap(),
        ],
    );
    let budget = Budget::new(ROW_BYTES).unwrap();
    let q = query();
    let result = engine.run(&q, None, &budget, None).await;
    let failure = result.as_ref().err().expect("source shape error");
    assert_eq!(failure.status, EngineStatus::Error);
    assert_eq!(
        failure.message.as_deref(),
        Some(format!("ESummary entry for PMID {id} must be an object").as_str())
    );
    assert_eq!(
        budget.snapshot().unwrap().used,
        failure.message.as_ref().unwrap().capacity()
    );
    let response = finish_batch(result, Instant::now()).await;
    assert_eq!(response.status, EngineStatus::Error);
    assert!(response.results.is_empty());
    assert_eq!(budget.snapshot().unwrap().used, 0);
    http.shutdown_and_wait().await;
}

#[tokio::test]
async fn actual_pubchem_cid_reference_parts_yield_and_release_before_second_dispatch() {
    for expire in [false, true] {
        let values = (1..=4096).collect::<Vec<_>>();
        let body = ssx_json::to_vec(&ssx_json::json!({"IdentifierList":{"CID":values}})).unwrap();
        assert!(body.len() < source_json::MAX_BYTES);
        let (engine, http, replies) = harness_config("pubchem", vec![body], 4096, 5000);
        let budget = Budget::new(ROW_BYTES).unwrap();
        let phase = AtomicUsize::new(0);
        let baseline = AtomicUsize::new(0);
        let history = AtomicUsize::new(0);
        let observer = |p| {
            if matches!(p, Phase::CidParts | Phase::CidPartsComplete) {
                history.fetch_or(1 << p as usize, Ordering::SeqCst);
                phase.store(p as usize + 1, Ordering::SeqCst);
            }
        };
        let deadline = Instant::now() + Duration::from_secs(2);
        let q = query();
        let mut future = Box::pin(engine.run(&q, Some(deadline), &budget, Some(&observer)));
        let phase_started = Instant::now();
        poll_fn(|cx| match future.as_mut().poll(cx) {
            Poll::Ready(result) => panic!(
                "PubChem completed before phase: error={:?} elapsed={:?} phase={} history={} budget={:?} requests={}",
                result.as_ref().err(), phase_started.elapsed(),
                phase.load(Ordering::SeqCst), history.load(Ordering::SeqCst),
                budget.snapshot().unwrap(), replies.seen.load(Ordering::SeqCst)
            ),
            Poll::Pending => {
                let reached = phase.load(Ordering::SeqCst) == Phase::CidParts as usize + 1;
                let used = budget.snapshot().unwrap().used;
                if reached && used > baseline.load(Ordering::SeqCst) {
                    Poll::Ready(())
                } else {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            }
        }).await;
        assert!(budget.snapshot().unwrap().used > 4096 * std::mem::size_of::<&str>());
        eprintln!(
            "A07_PROVIDER_PHASE pubchem expire={expire} live={} peak={}",
            budget.snapshot().unwrap().used,
            budget.snapshot().unwrap().peak
        );
        assert_eq!(replies.seen.load(Ordering::SeqCst), 1);
        if expire {
            std::thread::sleep(
                deadline.saturating_duration_since(Instant::now()) + Duration::from_millis(1),
            );
            let response = finish_batch(future.await, Instant::now()).await;
            assert_eq!(response.status, EngineStatus::Timeout);
            assert!(response.results.is_empty());
        } else {
            drop(future);
        }
        assert_eq!(budget.snapshot().unwrap().used, 0);
        assert_eq!(replies.seen.load(Ordering::SeqCst), 1);
        http.shutdown_and_wait().await;
    }
}

#[tokio::test]
async fn provider_sized_med_digit_scan_and_uniprot_partition_observe_deadline_and_drop() {
    for name in ["europepmc", "uniprot"] {
        for expire in [false, true] {
            let (body, wanted, complete, max) = if name == "europepmc" {
                (ssx_json::to_vec(&ssx_json::json!({"resultList":{"result":[{"source":"MED","id":"1".repeat(100_000),"title":"Long PMID"}]}})).unwrap(),Phase::MedDigits,Phase::MedDigitsComplete,10)
            } else {
                (ssx_json::to_vec(&ssx_json::json!({"results":(0..4096).map(|_|ssx_json::json!({"primaryAccession":"P12345"})).collect::<Vec<_>>()})).unwrap(),Phase::RankPartition,Phase::RankPartitionComplete,4096)
            };
            assert!(body.len() < source_json::MAX_BYTES);
            let (engine, http, replies) = harness_config(name, vec![body], max, 5000);
            let budget = Budget::new(ROW_BYTES).unwrap();
            let phase = AtomicUsize::new(0);
            let baseline = AtomicUsize::new(0);
            let observer = |p| {
                if p == wanted || p == complete {
                    phase.store(p as usize + 1, Ordering::SeqCst);
                }
            };
            let deadline = Instant::now() + Duration::from_secs(2);
            let q = query();
            let mut future = Box::pin(engine.run(&q, Some(deadline), &budget, Some(&observer)));
            until_phase(future.as_mut(), &phase, wanted, &budget, &baseline, 1).await;
            let live = budget.snapshot().unwrap().used;
            assert!(live > 0);
            if name == "uniprot" {
                assert!(live >= 2 * 4096 * std::mem::size_of::<(u8, usize, &Value)>());
            }
            eprintln!(
                "A07_PROVIDER_PHASE {name} expire={expire} live={live} peak={}",
                budget.snapshot().unwrap().peak
            );
            assert_eq!(replies.seen.load(Ordering::SeqCst), 1);
            if expire {
                std::thread::sleep(
                    deadline.saturating_duration_since(Instant::now()) + Duration::from_millis(1),
                );
                let response = finish_batch(future.await, Instant::now()).await;
                assert_eq!(response.status, EngineStatus::Timeout);
                assert!(response.results.is_empty());
            } else {
                drop(future);
            }
            assert_eq!(budget.snapshot().unwrap().used, 0);
            assert_eq!(replies.seen.load(Ordering::SeqCst), 1);
            http.shutdown_and_wait().await;
        }
    }
}

#[test]
fn complete_result_normalizer_exposes_actual_nonempty_media() {
    let result = SearchResult {
        media: Some(ssx_domain::MediaInfo {
            media_type: "video".into(),
            url: Some("https://example.test/media".into()),
            thumbnail: Some("https://example.test/thumb".into()),
            source: Some("sentinel".into()),
            width: Some(640),
            height: Some(480),
            duration: Some(FiniteF64::new(12.5).unwrap()),
        }),
        ..SearchResult::default()
    };
    let value = result_value(&result);
    assert_eq!(
        value["media"],
        ssx_json::json!({"media_type":"video","url":"https://example.test/media","thumbnail":"https://example.test/thumb","source":"sentinel","width":640,"height":480,"duration":12.5})
    );
    assert_ne!(value["media"], ssx_json::Value::Null);
}
