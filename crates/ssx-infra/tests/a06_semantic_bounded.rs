//! Bounded Semantic row operations; actual HTTP/factory qualification is separate.
use base64::Engine as _;
use ssx_domain::{AdapterResponse, EngineStatus};
use ssx_infra::adapters::{
    semantic_projection::{self, Error},
    semantic_scholar::{self, ParseError},
    source_json,
    source_payload::Outcome,
};
use ssx_json::{Value, json};
use ssx_owned_budget::Budget;
use std::time::{Duration, Instant};
fn normalized(response: &AdapterResponse) -> Value {
    assert!(response.results.iter().all(|r| r.media.is_none()));
    json!({"results":response.results.iter().map(|r|json!({"url":r.url,"title":r.title,"content":r.content,"engine":r.engine,"engines":r.engines.iter().collect::<Vec<_>>(),"score":r.score,"position":r.position,"category":r.category,"published_date":r.published_date,"thumbnail":r.thumbnail,"img_src":r.img_src,"tier":r.tier,"media":null,"payload":r.payload,"work_group":r.work_group})).collect::<Vec<_>>(),"status":response.status,"error_message":response.error_message,"answers":response.answers,"corrections":response.corrections,"infoboxes":response.infoboxes,"synthetic":response.synthetic})
}
#[tokio::test]
async fn bounded_rows_preserve_original_outcomes_and_accepted_typed_refusals() {
    let mut exact = Vec::new();
    let mut typed = Vec::new();
    let mut outside = Vec::new();
    let mut unique = std::collections::BTreeSet::new();
    for raw in [
        include_str!("../../../docs/rust-rewrite/a06/source-initial.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-regressions.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-shared.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-json-matrix.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-selected-values.json"),
    ] {
        let corpus: Value = ssx_json::from_str(raw).unwrap();
        for row in corpus["records"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["engine"].as_str() == Some("semanticscholar"))
        {
            let id = row["id"].as_str().unwrap();
            assert!(unique.insert(id.to_owned()));
            let response = row["responses"].as_array().unwrap().last().unwrap();
            if response["status"].as_u64() != Some(200) {
                outside.push(id.to_owned());
                continue;
            }
            let body = base64::engine::general_purpose::STANDARD
                .decode(response["body_b64"].as_str().unwrap())
                .unwrap();
            let budget = Budget::new(256 * 1024 * 1024).unwrap();
            let input = budget.reserve(body.capacity()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(30);
            let source =
                match source_json::from_slice_budgeted(&body, Some(deadline), &budget).await {
                    Ok(value) => value,
                    Err(_) => {
                        outside.push(id.to_owned());
                        continue;
                    }
                };
            // Retained legacy source seam is a test-only policy comparator; the
            // new actual path never executes it or clones its result batch.
            let legacy = semantic_scholar::parse(source.value(), deadline).await;
            let actual = semantic_projection::parse(source.value(), &budget, deadline).await;
            match legacy {
                Ok(_) => {
                    let batch = actual.unwrap_or_else(|error| panic!("{id}: {error:?}"));
                    let actual = batch
                        .into_adapter_response(Outcome {
                            status: EngineStatus::Ok,
                            error_message: None,
                            latency_ms: 0.0,
                            synthetic: false,
                        })
                        .await
                        .unwrap();
                    assert_eq!(normalized(&actual), row["output"], "{id}");
                    exact.push(id.to_owned());
                }
                Err(ParseError::Source(expected)) => {
                    let Err(Error::Source(actual)) = actual else {
                        panic!("{id}: wrong source error stage")
                    };
                    assert_eq!(actual.as_str(), expected, "{id}");
                    assert_eq!(
                        actual.as_str(),
                        row["output"]["error_message"].as_str().unwrap(),
                        "{id}"
                    );
                    exact.push(id.to_owned());
                }
                Err(ParseError::Output(expected)) => {
                    let Err(Error::Output(actual)) = actual else {
                        panic!("{id}: wrong typed error stage")
                    };
                    assert_eq!(actual, expected, "{id}");
                    typed.push(id.to_owned());
                }
                Err(error) => panic!("unexpected legacy operational failure {id}: {error:?}"),
            }
            drop(source);
            drop(input);
            assert_eq!(budget.snapshot().unwrap().used, 0, "{id}");
        }
    }
    eprintln!(
        "exact={} typed={} outside={} exact_ids={exact:?} typed_ids={typed:?} outside_ids={outside:?}",
        exact.len(),
        typed.len(),
        outside.len()
    );
    assert_eq!((exact.len(), typed.len(), outside.len()), (108, 22, 18));
}

#[tokio::test]
async fn actual_projection_owned_overlap_cancellation_deadline_and_root_pressure_release() {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    let author_chars = 512 * 1024;
    let body = format!("{{\"data\":[{{\"title\":\"Paper\",\"url\":\"https://paper.test/\",\"citationCount\":1,\"authors\":[{{\"name\":\"{}\"}}]}}]}}", "x".repeat(author_chars)).into_bytes();
    for expire in [false, true] {
        let budget = Budget::new(128 * 1024 * 1024).unwrap();
        let input = budget.reserve(body.capacity()).unwrap();
        let source = source_json::from_slice_budgeted(
            &body,
            Some(Instant::now() + Duration::from_secs(30)),
            &budget,
        )
        .await
        .unwrap();
        let baseline = budget.snapshot().unwrap().used;
        let deadline = Instant::now() + Duration::from_secs(if expire { 2 } else { 30 });
        let mut future = Box::pin(semantic_projection::parse(
            source.value(),
            &budget,
            deadline,
        ));
        let mut context = Context::from_waker(Waker::noop());
        let mut observed = 0;
        for _ in 0..100000 {
            assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
            // Output/scratch growth coexists with retained raw input and DOM.
            if budget.snapshot().unwrap().used >= baseline + author_chars * 4 {
                observed += 1;
                if observed == 16 {
                    break;
                }
            }
        }
        assert_eq!(observed, 16);
        if expire {
            tokio::time::sleep_until(deadline.into()).await;
            assert!(matches!(future.await, Err(Error::Deadline)));
        } else {
            drop(future);
        }
        assert_eq!(budget.snapshot().unwrap().used, baseline);
        // A held caller reservation injects pressure into the SAME root, after
        // successful DOM parse; this is a quota test, not an RSS-limit claim.
        let pressure = budget
            .reserve(budget.snapshot().unwrap().limit - baseline - 1024)
            .unwrap();
        let retained = budget.snapshot().unwrap().used;
        assert!(matches!(
            semantic_projection::parse(
                source.value(),
                &budget,
                Instant::now() + Duration::from_secs(30)
            )
            .await,
            Err(Error::Resource)
        ));
        assert_eq!(budget.snapshot().unwrap().used, retained);
        drop(pressure);
        drop(source);
        drop(input);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
}
