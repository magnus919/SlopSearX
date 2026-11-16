//! Frozen Python is evidence, never runtime. These contracts execute native
//! planning/completion over the actual captured source inputs and full outcomes.
use ssx_application::{policy::*, scope::*, staged::*};
use ssx_domain::{
    canonical::{JsonProfile, canonical_json},
    staged::Record,
    storage,
};
use ssx_json::{Value, json};
use std::collections::BTreeMap;
fn corpus() -> Value {
    ssx_json::from_str(include_str!("../../../docs/rust-rewrite/w06/source.json")).unwrap()
}
fn completed(case: &str, test: &str) {
    println!("W06_CASE_COMPLETE {}", json!({"case_id":case,"test":test}));
}
fn policy(c: &Value) -> PolicyService {
    let context = &c["context"];
    let catalog = context["catalog"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .map(|row| EngineCapability {
                    name: row["name"].as_str().unwrap().into(),
                    enabled: row["enabled"].as_bool().unwrap(),
                    active: row["active"].as_bool().unwrap(),
                    available: true,
                    categories: row["categories"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|s| s.as_str().unwrap().into())
                        .collect(),
                    media_types: vec![],
                    auth_ready: true,
                    circuit_open: false,
                    cost_class: "free".into(),
                    health_status: "ok".into(),
                    health_stale: false,
                    enforced_filters: BTreeMap::new(),
                })
                .collect()
        })
        .unwrap_or_else(|| {
            context["active_engines"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(n, v)| EngineCapability {
                    name: n.clone(),
                    enabled: true,
                    active: true,
                    available: true,
                    categories: v["categories"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|s| s.as_str().unwrap().into())
                        .collect(),
                    media_types: vec![],
                    auth_ready: true,
                    circuit_open: false,
                    cost_class: "free".into(),
                    health_status: "ok".into(),
                    health_stale: false,
                    enforced_filters: BTreeMap::new(),
                })
                .collect()
        });
    PolicyService {
        policy: SearchPolicy {
            max_query_length: context["max_query_length"].as_u64().unwrap() as usize,
            ..Default::default()
        },
        catalog: CatalogSnapshot::new(catalog).unwrap(),
        resolver: ScopeResolver {
            tier1: context["tier1_engines"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_str().unwrap().into())
                .collect(),
            catalog_routing: false,
            ..Default::default()
        },
    }
}
fn input(v: &Value) -> Input {
    Input {
        query: v["query"].as_str().unwrap().into(),
        objectives: v["objectives"].clone(),
        initial_scope: v["initial_scope"].clone(),
        fallback_scope: (!v["fallback_scope"].is_null()).then(|| v["fallback_scope"].clone()),
        allow_scope_expansion: v["allow_scope_expansion"].as_bool().unwrap(),
        filters: (!v["filters"].is_null()).then(|| v["filters"].clone()),
    }
}
#[test]
fn source_plans_preserve_full_outcomes_and_identity_bytes() {
    let c = corpus();
    let policy = policy(&c);
    for row in c["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["id"].as_str().unwrap().starts_with("plan."))
    {
        let id = row["id"].as_str().unwrap();
        let limits = Limits {
            granted: true,
            deadline_ms: row["operator_caps"]["deadline_ms"]
                .as_u64()
                .unwrap_or(30000),
            engine_calls: row["operator_caps"]["max_engine_calls"]
                .as_u64()
                .unwrap_or(64),
        };
        let actual = Plan::build(&policy, &limits, &input(&row["inputs"]));
        if id.contains("language_nonstring") {
            let error = actual.unwrap_err();
            assert_eq!(
                error.value(),
                json!({"error":{"code":"invalid_input","field":"filters.language","message":"language must be a string"}}),
                "{id}"
            );
            assert!(
                !row["output"]["plan"].is_null(),
                "source acceptance must remain captured"
            );
        } else {
            let actual = match actual {
                Ok(plan) => {
                    if let Some(expected) = row["identity_canonical"].as_str() {
                        assert_eq!(
                            canonical_json(&plan.value["identity"], JsonProfile::AsciiCompact)
                                .unwrap(),
                            expected.as_bytes(),
                            "{id}"
                        );
                    }
                    json!({"plan":plan.value,"error":null})
                }
                Err(error) => json!({"plan":null,"error":error.value()}),
            };
            assert_eq!(actual, row["output"], "{id}");
        }
        completed(id, "source_plans_preserve_full_outcomes_and_identity_bytes");
    }
}
fn request_value(r: &ssx_domain::SearchRequest) -> Value {
    json!({"query":r.query,"categories":r.categories,"engines":r.engines,"language":r.language,"page":r.page,"time_range":r.time_range,"safesearch":r.safesearch,"media_type":r.media_type,"max_results":r.max_results,"include":r.include,"freshness":r.freshness,"client_identifier":r.client_identifier,"interactive_timeout_ms":r.interactive_timeout_ms,"generate_suggestions":r.generate_suggestions,"execution_isolation_key":r.execution_isolation_key,"date_from":r.date_from,"date_to":r.date_to,"_time_range_anchor":r.time_range_anchor})
}
#[test]
fn source_completion_preserves_full_records_and_dispatch_requests() {
    let c = corpus();
    for row in c["cases"].as_array().unwrap().iter().filter(|r| {
        r["id"].as_str().unwrap().starts_with("runner.")
            && r["id"].as_str() != Some("runner.renewal_lost")
    }) {
        let id = row["id"].as_str().unwrap();
        let claims = row["claimed"].as_array().unwrap();
        for (index, claimed) in claims.iter().enumerate() {
            let record = Record::from_value(claimed.clone()).unwrap();
            let target = if index + 1 < claims.len() {
                &row["intermediate"]
            } else {
                &row["record"]
            };

            let completion_time = row["completion_time"].as_f64().unwrap();
            let response = row["responses"]
                .as_array()
                .unwrap()
                .get(index)
                .map(|v| storage::decode_response(v.clone()).unwrap());
            let error = row["execution_error"]["message"].as_str();
            let rejection = if row["policy_rejection"].is_null() {
                None
            } else {
                Some(&row["policy_rejection"]["error"])
            };
            let failure = if let Some(error) = error {
                Some(("execution_failed", error))
            } else if let Some(rejection) = rejection {
                Some(("policy_rejected", rejection["message"].as_str().unwrap()))
            } else if response.is_none() {
                Some(("deadline_exceeded", "deadline expired before dispatch"))
            } else {
                None
            };
            let cursor = row["snapshot_ids"][index].as_str();
            let next = if let Some(rejection) = rejection {
                policy_rejected(&record, rejection, completion_time)
            } else {
                complete(&record, response.as_ref(), cursor, completion_time, failure)
            }
            .unwrap();
            let mut next = next.value().clone();
            // The Python in-memory stand-in clears owner_token; real Lua persistence and
            // accepted W11 retain it. Its authorization still comes only from the lease.
            next.as_object_mut().unwrap().shift_remove("owner_token");
            let mut expected = target.clone();
            expected
                .as_object_mut()
                .unwrap()
                .shift_remove("owner_token");
            assert_eq!(next, expected, "{id} stage{index}");
            assert_eq!(
                canonical_json(&next, JsonProfile::AsciiCompact).unwrap(),
                canonical_json(&expected, JsonProfile::AsciiCompact).unwrap(),
                "{id}"
            );
            if let Some(request) = row["requests"].as_array().unwrap().get(index) {
                let remaining = ((claimed["execution_deadline_at"].as_f64().unwrap()
                    - row["request_times"][index].as_f64().unwrap())
                    * 1000.) as u64;
                assert_eq!(
                    request_value(&dispatch(&record, remaining)),
                    *request,
                    "{id}"
                );
            }
        }
        completed(
            id,
            "source_completion_preserves_full_records_and_dispatch_requests",
        );
    }
}
#[test]
fn source_replay_equivalence_preserves_original_requested_objectives() {
    let c = corpus();
    for row in c["cases"].as_array().unwrap().iter().filter(|r| {
        r["id"].as_str().unwrap().starts_with("replay.") && r.get("equivalent").is_some()
    }) {
        let record = Record::from_value(row["record"].clone()).unwrap();
        assert_eq!(
            replay_equivalent(&record, &input(&row["inputs"])),
            row["equivalent"].as_bool().unwrap(),
            "{}",
            row["id"].as_str().unwrap()
        );
        completed(
            row["id"].as_str().unwrap(),
            "source_replay_equivalence_preserves_original_requested_objectives",
        );
    }
}

#[test]
fn source_retained_authority_preserves_full_policy_envelopes() {
    let c: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/w06/evidence/retained-policy-source-outcomes.json"
    ))
    .unwrap();
    for row in c["cases"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        let mutations = &row["mutations"];
        let mut current = c.clone();
        let rows = current["context"]["catalog"].as_array_mut().unwrap();
        rows.retain(|e| {
            !mutations["catalog_remove"]
                .as_array()
                .is_some_and(|names| names.contains(&e["name"]))
        });
        for engine in rows.iter_mut() {
            let name = engine["name"].as_str().unwrap().to_string();
            engine["active"] = json!(
                row["current_active_engines"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(name))
            );
            if let Some(changes) = mutations["catalog_changes"][name.as_str()].as_object() {
                for (key, value) in changes {
                    engine[key.as_str()] = value.clone();
                }
            }
        }
        let mut policy = policy(&current);
        let mut engines = policy.catalog.engines().to_vec();
        for engine in &mut engines {
            let changes = &mutations["catalog_changes"][engine.name.as_str()];
            if changes["auth_class"].as_str() == Some("required") {
                engine.auth_ready = changes["auth_configured"].as_bool().unwrap();
            }
            engine.circuit_open = changes["circuit_open"].as_bool().unwrap_or(false);
            if let Some(status) = changes["last_known_status"].as_str() {
                engine.health_status = status.into();
                engine.available = status != "unavailable";
            }
        }
        policy.catalog = CatalogSnapshot::new(engines).unwrap();
        if let Some(names) = mutations["sensitive_engines"].as_array() {
            policy.policy.sensitive_engines = names
                .iter()
                .map(|n| n.as_str().unwrap().to_string())
                .collect();
        }
        policy.policy.sensitive_allowed = mutations["targeted_sensitive_allowed"]
            .as_bool()
            .unwrap_or(false);
        if mutations["jobs_granted"].as_bool().unwrap_or(true) {
            policy.policy.grants.insert("jobs".into());
        }
        let limits = Limits {
            granted: mutations["staged_granted"].as_bool().unwrap_or(true),
            deadline_ms: mutations["deadline_limit"].as_u64().unwrap_or(30000),
            engine_calls: 64,
        };
        let record = Record::from_value(row["record"].clone()).unwrap();
        let actual = revalidate(&policy, &limits, &record)
            .err()
            .map(|error| error.value())
            .unwrap_or(Value::Null);
        assert_eq!(actual, row["output"], "{id}");
        assert_eq!(
            record.value(),
            &row["record"],
            "retained read must not mutate {id}"
        );
        completed(
            id,
            "source_retained_authority_preserves_full_policy_envelopes",
        );
    }
}
#[test]
fn source_review_plans_preserve_malformed_scope_and_fallback_order() {
    let c: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/w06/evidence/review-public-source-outcomes.json"
    ))
    .unwrap();
    let policy = policy(&c);
    for row in c["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["kind"].as_str() == Some("plan"))
    {
        let actual = match Plan::build(
            &policy,
            &Limits {
                granted: true,
                ..Default::default()
            },
            &input(&row["input"]),
        ) {
            Ok(plan) => json!({"plan":plan.value,"error":null}),
            Err(error) => json!({"plan":null,"error":error.value()}),
        };
        assert_eq!(actual, row["output"], "{}", row["id"].as_str().unwrap());
        completed(
            row["id"].as_str().unwrap(),
            "source_review_plans_preserve_malformed_scope_and_fallback_order",
        );
    }
}
#[test]
fn source_huge_integer_fallback_truthiness_preserves_validation_order() {
    let c: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/w06/evidence/huge-integer-truthiness-source-outcomes.json"
    ))
    .unwrap();
    let policy = policy(&c);
    for row in c["cases"].as_array().unwrap() {
        let mut request = input(&row["input"]);
        // Keep the original JSON spelling so -0 remains an integer zero control.
        request.fallback_scope =
            Some(ssx_json::from_str(row["fallback_json"].as_str().unwrap()).unwrap());
        let actual = match Plan::build(
            &policy,
            &Limits {
                granted: true,
                ..Default::default()
            },
            &request,
        ) {
            Ok(plan) => json!({"plan":plan.value,"error":null}),
            Err(error) => json!({"plan":null,"error":error.value()}),
        };
        assert_eq!(actual, row["output"], "{}", row["id"].as_str().unwrap());
        completed(
            row["id"].as_str().unwrap(),
            "source_huge_integer_fallback_truthiness_preserves_validation_order",
        );
    }
}
