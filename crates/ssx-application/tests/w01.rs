use ssx_domain::{AttemptId, WallClockTimestamp, research::*};
use ssx_json::Value;
fn time(v: f64) -> WallClockTimestamp {
    WallClockTimestamp::new(v).unwrap()
}
#[test]
fn source_bound_research_record_and_budget_goldens() {
    let corpus: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/w01/corpus.json")).unwrap();
    assert_eq!(corpus["cases"].as_array().unwrap().len(), 10);
    let mut hardened_cases = std::collections::BTreeSet::new();
    for case in corpus["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut job = ResearchJob::from_payload(case["input"].clone()).unwrap();
        assert_eq!(job.to_payload().unwrap(), case["initial"], "initial {name}");
        for (operation, expected) in case["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["outputs"].as_array().unwrap())
        {
            let now = time(operation["now"].as_f64().unwrap_or(100.0));
            let index = operation["index"].as_u64().unwrap_or(0) as usize;
            match operation["kind"].as_str().unwrap() {
                "initialize" => {
                    let p = operation["policy"].as_array().unwrap();
                    job.initialize_budget(ResearchCeilings {
                        queries: p[0].as_u64().unwrap(),
                        engines_per_query: p[1].as_u64().unwrap(),
                        results: p[2].as_u64().unwrap(),
                    })
                    .unwrap();
                }
                "reserve" => {
                    // Source recovery helper leaves the query running; the actual
                    // native claim entrypoint resets recovered queries to pending.
                    if job.queries[index].state == "running"
                        && job.queries[index]
                            .attempts
                            .last()
                            .is_some_and(|a| a.state == "interrupted")
                    {
                        job.queries[index].state = "pending".into();
                    }
                    if job.queries[index].state != "pending"
                        || job.queries[index]
                            .attempts
                            .last()
                            .is_some_and(|a| a.state == "running")
                    {
                        assert!(
                            [
                                "reserve-interrupts-before-budget-rejection",
                                "result-exhaustion"
                            ]
                            .contains(&name),
                            "unexpected source difference {name}"
                        );
                        let unchanged = job.clone();
                        let error = job
                            .reserve_attempt(
                                index,
                                AttemptId::new("rejected-source-helper").unwrap(),
                                now,
                            )
                            .unwrap_err();
                        assert_eq!(
                            error.0,
                            "reservation requires a pending query without an active attempt"
                        );
                        assert_eq!(job, unchanged);
                        // Retain the source observation: its raw helper mutates
                        // the query to failed, whereas native admission rejects.
                        assert_eq!(expected["job"]["queries"][index]["state"], "failed");
                        assert_eq!(
                            expected["job"]["budget_used"],
                            ssx_json::to_value(&job.budget_used).unwrap()
                        );
                        assert!(hardened_cases.insert(name.to_owned()));
                        let reason = if name == "result-exhaustion" {
                            "result_budget_exhausted"
                        } else {
                            "attempt_budget_exhausted"
                        };
                        assert_eq!(expected["job"]["stop_reason"], reason);
                        assert_eq!(expected["job"]["queries"][index]["error"], reason);
                        let mut source_projection = job.to_payload().unwrap();
                        assert_ne!(source_projection, expected["job"]);
                        source_projection["stop_reason"] = expected["job"]["stop_reason"].clone();
                        for key in ["state", "error"] {
                            source_projection["queries"].as_array_mut().unwrap()[index][key] =
                                expected["job"]["queries"][index][key].clone();
                        }
                        if name == "reserve-interrupts-before-budget-rejection" {
                            let last = job.queries[index].attempts.len() - 1;
                            let source_attempt =
                                &expected["job"]["queries"][index]["attempts"][last];
                            assert_eq!(source_attempt["state"], "interrupted");
                            assert_eq!(
                                source_attempt["error"],
                                "Previous dispatch outcome is unknown after worker recovery; reservation retained"
                            );
                            assert_eq!(source_attempt["finished_at"].as_f64(), Some(now.seconds()));
                            for key in ["state", "error", "finished_at"] {
                                source_projection["queries"].as_array_mut().unwrap()[index]["attempts"].as_array_mut().unwrap()[last][key] =
                                    source_attempt[key].clone();
                            }
                        }
                        assert_eq!(
                            source_projection, expected["job"],
                            "full source payload {name}"
                        );
                        assert_eq!(
                            ssx_json::to_value(job.budget_summary()).unwrap(),
                            expected["summary"]
                        );
                        assert_eq!(
                            ResearchJob::from_payload(job.to_payload().unwrap()).unwrap(),
                            job
                        );
                        continue;
                    }
                    let accepted = job
                        .reserve_attempt(
                            index,
                            AttemptId::new(operation["attempt_id"].as_str().unwrap_or("unused-id"))
                                .unwrap(),
                            now,
                        )
                        .unwrap();
                    assert_eq!(accepted, !operation["attempt_id"].is_null(), "{name}");
                }
                "finish" => {
                    let expected_attempt = AttemptId::new(
                        job.queries[index]
                            .attempts
                            .last()
                            .unwrap()
                            .attempt_id
                            .clone(),
                    )
                    .unwrap();
                    let mut query = ssx_json::to_value(&job.queries[index]).unwrap();
                    for (key, value) in operation["outcome"].as_object().unwrap() {
                        query[key.as_str()] = value.clone();
                    }
                    job.queries[index] = ssx_json::from_value(query).unwrap();
                    let response =
                        operation["urls"]
                            .as_array()
                            .map(|items| ssx_domain::SearchResponse {
                                results: items
                                    .iter()
                                    .map(|url| ssx_domain::SearchResult {
                                        url: url.as_str().unwrap().into(),
                                        ..Default::default()
                                    })
                                    .collect(),
                                ..Default::default()
                            });
                    ssx_application::research::finish_attempt(
                        &mut job,
                        index,
                        &expected_attempt,
                        response.as_ref(),
                        now,
                    )
                    .unwrap();
                }
                "recover" => {
                    let mut generated = operation["generated_ids"].as_array().unwrap().iter();
                    job.recover_orphan_attempts(now, || {
                        AttemptId::new(generated.next().unwrap().as_str().unwrap()).unwrap()
                    })
                    .unwrap();
                }
                "retry" => job.reset_retryable_queries(),
                _ => panic!("unowned operation"),
            }
            assert_eq!(
                job.to_payload().unwrap(),
                expected["job"],
                "job {name} {}",
                operation["kind"]
            );
            assert_eq!(
                ssx_json::to_value(job.budget_summary()).unwrap(),
                expected["summary"],
                "summary {name}"
            );
            assert_eq!(
                ResearchJob::from_payload(job.to_payload().unwrap()).unwrap(),
                job,
                "round trip {name}"
            );
        }
    }
    assert_eq!(
        hardened_cases,
        [
            "reserve-interrupts-before-budget-rejection".to_owned(),
            "result-exhaustion".to_owned()
        ]
        .into()
    );
}

#[test]
fn source_captured_url_identities_and_settlement() {
    let data: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/w01/identity.json")).unwrap();
    assert_eq!(data["identities"].as_array().unwrap().len(), 382);
    for case in data["identities"].as_array().unwrap() {
        let url = case["url"].as_str().unwrap();
        assert_eq!(
            ssx_application::normalise_url(url),
            case["normalized"].as_str().unwrap()
        );
        assert_eq!(
            ssx_application::research::lead_id(url).as_str(),
            case["expected"].as_str().unwrap(),
            "{url:?}"
        );
    }
    for case in data["settlements"].as_array().unwrap() {
        let mut job = ResearchJob::from_payload(case["input"].clone()).unwrap();
        let response = case["urls"]
            .as_array()
            .map(|urls| ssx_domain::SearchResponse {
                results: urls
                    .iter()
                    .map(|url| ssx_domain::SearchResult {
                        url: url.as_str().unwrap().into(),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            });
        let expected_attempt =
            AttemptId::new(job.queries[0].attempts.last().unwrap().attempt_id.clone()).unwrap();
        ssx_application::research::finish_attempt(
            &mut job,
            0,
            &expected_attempt,
            response.as_ref(),
            time(100.0),
        )
        .unwrap();
        assert_eq!(
            job.to_payload().unwrap(),
            case["expected"],
            "{}",
            case["name"]
        );
        let settled = job.clone();
        assert!(
            ssx_application::research::finish_attempt(
                &mut job,
                0,
                &expected_attempt,
                response.as_ref(),
                time(101.0)
            )
            .is_err()
        );
        assert_eq!(job, settled);
    }
}

#[test]
fn lead_identity_changes_only_with_normalized_url() {
    use ssx_application::research::lead_id;
    assert_eq!(
        lead_id("https://example.com/p?utm_source=x"),
        lead_id("https://example.com/p")
    );
    assert_ne!(
        lead_id("https://EXAMPLE.com/p"),
        lead_id("https://example.com/p")
    );
    assert_ne!(
        lead_id("https://example.com/p#f"),
        lead_id("https://example.com/p")
    );
    assert_ne!(
        lead_id("https://example.com/p?a=1"),
        lead_id("https://example.com/p?a=2")
    );
    assert_ne!(
        lead_id("https://example.com/Case"),
        lead_id("https://example.com/case")
    );
    assert_eq!(
        lead_id("https://example.com/p;"),
        lead_id("https://example.com/p")
    );
    assert_ne!(
        lead_id("https://example.com/p;x"),
        lead_id("https://example.com/p")
    );
}
