//! Actual borrowed OpenAlex row pipeline; HTTP/factory integration follows.
use base64::Engine as _;
use ssx_domain::{AdapterResponse, EngineStatus};
use ssx_infra::adapters::{
    openalex_projection::{self, Error},
    scholarly_config::Scalar,
    scholarly_wire, source_json,
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
async fn borrowed_rows_preserve_complete_original_outcomes_and_explicit_typed_refusals() {
    let (mut exact, mut typed, mut outside) = (Vec::new(), Vec::new(), Vec::new());
    let mut unique = std::collections::BTreeSet::new();
    for raw in [
        include_str!("../../../docs/rust-rewrite/a06/source-initial.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-regressions.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-shared.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-json-matrix.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-openalex-order.json"),
    ] {
        let corpus: Value = ssx_json::from_str(raw).unwrap();
        for row in corpus["records"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["engine"].as_str() == Some("openalex"))
        {
            let id = row["id"].as_str().unwrap();
            assert!(unique.insert(id.to_owned()));
            let request = ssx_domain::SearchRequest {
                date_from: row["params"]["date_from"].as_str().map(str::to_owned),
                date_to: row["params"]["date_to"].as_str().map(str::to_owned),
                time_range: row["params"]["time_range"].as_str().map(str::to_owned),
                ..Default::default()
            };
            let Ok(bounds) = scholarly_wire::openalex_preflight(&request) else {
                outside.push(id.to_owned());
                continue;
            };
            let response = &row["responses"][0];
            if response["status"].as_u64() != Some(200) {
                outside.push(id.to_owned());
                continue;
            }
            let body = base64::engine::general_purpose::STANDARD
                .decode(response["body_b64"].as_str().unwrap())
                .unwrap();
            let budget = Budget::new(512 * 1024 * 1024).unwrap();
            let input = budget.reserve(body.capacity()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(30);
            let Ok(source) = source_json::from_slice_budgeted(&body, Some(deadline), &budget).await
            else {
                outside.push(id.to_owned());
                continue;
            };
            let result = openalex_projection::parse(
                source.value(),
                &Scalar::Integer(10),
                bounds,
                request.date_from.is_some() || request.date_to.is_some(),
                &budget,
                deadline,
            )
            .await;
            match result {
                Ok(batch) => {
                    let actual = batch
                        .into_adapter_response(Outcome {
                            status: EngineStatus::Ok,
                            error_message: None,
                            latency_ms: 0.,
                            synthetic: false,
                        })
                        .await
                        .unwrap();
                    assert_eq!(normalized(&actual), row["output"], "{id}");
                    exact.push(id.to_owned());
                }
                Err(Error::Source(m)) => {
                    assert_eq!(row["output"]["status"].as_str(), Some("error"), "{id}");
                    assert_eq!(
                        Some(m.as_str()),
                        row["output"]["error_message"].as_str(),
                        "{id}"
                    );
                    exact.push(id.to_owned());
                }
                Err(Error::Output(m)) => {
                    assert_eq!(
                        row["output"]["status"].as_str(),
                        Some("ok"),
                        "source error precedence {id}: {m}"
                    );
                    assert!(
                        m.starts_with("OpenAlex result") || m.starts_with("OpenAlex payload"),
                        "{id}"
                    );
                    typed.push((id.to_owned(), m));
                }
                Err(e) => panic!("{id}: {e:?}"),
            }
            drop(source);
            drop(input);
            assert_eq!(budget.snapshot().unwrap().used, 0, "{id}");
        }
    }
    println!(
        "exact {} {:?}; typed {} {:?}; outside {} {:?}",
        exact.len(),
        exact,
        typed.len(),
        typed,
        outside.len(),
        outside
    );
    assert!(!exact.is_empty());
}

#[tokio::test]
async fn full_abstract_overlap_charge_drop_deadline_and_pressure_preserve_same_root() {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    let word_chars = 512 * 1024;
    let body=format!("{{\"results\":[{{\"id\":\"https://openalex.org/W1\",\"title\":\"Paper\",\"abstract_inverted_index\":{{\"{}\":[0]}}}}]}}","x".repeat(word_chars)).into_bytes();
    let bounds = ssx_application::filters::PublicationBounds::new_borrowed(None, None).unwrap();
    for expire in [false, true] {
        let budget = Budget::new(64 * 1024 * 1024).unwrap();
        let input = budget.reserve(body.capacity()).unwrap();
        let data = source_json::from_slice_budgeted(
            &body,
            Some(Instant::now() + Duration::from_secs(30)),
            &budget,
        )
        .await
        .unwrap();
        let baseline = budget.snapshot().unwrap().used;
        let deadline = Instant::now() + Duration::from_secs(if expire { 2 } else { 30 });
        let limit = Scalar::Integer(10);
        let mut future = Box::pin(openalex_projection::parse(
            data.value(),
            &limit,
            bounds,
            false,
            &budget,
            deadline,
        ));
        let mut context = Context::from_waker(Waker::noop());
        let mut observed = 0;
        for _ in 0..100000 {
            assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
            if budget.snapshot().unwrap().used > baseline + word_chars * 4 {
                observed += 1;
                if observed == 16 {
                    break;
                }
            }
        }
        assert_eq!(
            observed, 16,
            "observe full joined abstract owner during copy"
        );
        if expire {
            tokio::time::sleep_until(deadline.into()).await;
            assert!(matches!(future.await, Err(Error::Deadline)));
        } else {
            drop(future);
        }
        assert_eq!(budget.snapshot().unwrap().used, baseline);
        drop(data);
        drop(input);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    let budget = Budget::new(64 * 1024 * 1024).unwrap();
    let input = budget.reserve(body.capacity()).unwrap();
    let data = source_json::from_slice_budgeted(&body, None, &budget)
        .await
        .unwrap();
    let baseline = budget.snapshot().unwrap().used;
    let batch = openalex_projection::parse(
        data.value(),
        &Scalar::Integer(10),
        bounds,
        false,
        &budget,
        Instant::now() + Duration::from_secs(30),
    )
    .await
    .unwrap();
    assert_eq!(batch.results()[0].content, "x".repeat(500));
    assert!(budget.snapshot().unwrap().used > baseline);
    assert!(budget.snapshot().unwrap().peak > baseline + word_chars * 4);
    drop(batch);
    assert_eq!(budget.snapshot().unwrap().used, baseline);
    let pressure = budget.reserve(64 * 1024 * 1024 - baseline - 512).unwrap();
    assert!(matches!(
        openalex_projection::parse(
            data.value(),
            &Scalar::Integer(10),
            bounds,
            false,
            &budget,
            Instant::now() + Duration::from_secs(30)
        )
        .await,
        Err(Error::Resource)
    ));
    assert_eq!(budget.snapshot().unwrap().used, 64 * 1024 * 1024 - 512);
    drop(pressure);
    assert_eq!(budget.snapshot().unwrap().used, baseline);
    drop(data);
    drop(input);
    assert_eq!(budget.snapshot().unwrap().used, 0);
}
