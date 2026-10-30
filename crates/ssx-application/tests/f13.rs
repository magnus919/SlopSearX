use ssx_application::{cache::*, snapshot::SnapshotPage};
use ssx_domain::{SearchResponse, SearchResult, storage};
use ssx_json::Value;
use std::collections::BTreeSet;
const CORPUS: &str = include_str!("../../xtask/fixtures/f13/source.json");
#[test]
fn source_base_keys_answers_ttls_and_views_replay_exactly() {
    let corpus: Value = ssx_json::from_str(CORPUS).unwrap();
    let cases = corpus["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 130);
    let mut checked = 0;
    for case in cases {
        let input = &case["input"];
        match case["kind"].as_str().unwrap() {
            "key" => {
                let mut key = CacheKeyInput {
                    query: input["query"].as_str().unwrap().into(),
                    ..Default::default()
                };
                if let Some(v) = input["language"].as_str() {
                    key.language = v.into();
                }
                if let Some(v) = input["safesearch"].as_i64() {
                    key.safesearch = v;
                }
                if let Some(v) = input["pageno"].as_i64() {
                    key.page = ssx_json::Number::new(&v.to_string()).unwrap();
                }
                for (field, dst) in [
                    ("categories", &mut key.categories),
                    ("engines", &mut key.engines),
                ] {
                    if let Some(v) = input[field].as_array() {
                        *dst = v.iter().map(|x| x.as_str().unwrap().into()).collect();
                    }
                }
                key.time_range = input["time_range"].as_str().map(str::to_owned);
                key.media_type = input["media_type"].as_str().map(str::to_owned);
                key.date_from = input["date_from"].as_str().map(str::to_owned);
                key.date_to = input["date_to"].as_str().map(str::to_owned);
                // Captured source calendar windows are explicit, frozen authority inputs.
                key.relative_window = match case["id"].as_str().unwrap() {
                    "relative-day" => Some(("2026-02-28".into(), "2026-03-01".into())),
                    "relative-next-day" => Some(("2026-03-01".into(), "2026-03-02".into())),
                    "relative-month" => Some(("2026-01-30".into(), "2026-03-01".into())),
                    _ => None,
                };
                assert_eq!(
                    source_base_key(&key).as_str(),
                    case["expected"].as_str().unwrap(),
                    "{}",
                    case["id"]
                );
                checked += 1;
            }
            "answer-key" => {
                assert_eq!(
                    answer_key(input["query"].as_str().unwrap()),
                    case["expected"].as_str().unwrap()
                );
                checked += 1;
            }
            "ttl" => {
                let parsed = CacheTtls::from_text(
                    input["normal"].as_str().unwrap(),
                    input["partial_ttl"].as_str().unwrap(),
                    "60",
                );
                if case["id"] == ssx_json::json!("ttl-14") {
                    assert!(parsed.is_err());
                    assert_eq!(case["expected"]["ttl"].as_i64(), Some(3600));
                } else if case["expected"]["error"].is_string() {
                    assert!(parsed.is_err());
                } else {
                    let categories = input["categories"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|x| x.as_str().unwrap().into())
                        .collect::<Vec<_>>();
                    assert_eq!(
                        parsed
                            .unwrap()
                            .for_response(&categories, input["partial"].as_bool().unwrap())
                            .get(),
                        case["expected"]["ttl"].as_u64().unwrap()
                    );
                }
                checked += 1;
            }
            "negative-view" => {
                let context = CachedViewContext {
                    query: input["query"].as_str().unwrap().into(),
                    query_id: ssx_domain::QueryId::new(input["query_id"].as_str().unwrap())
                        .unwrap(),
                    ranking_explanation: input["ranking_explanation"].as_str().unwrap().into(),
                    include: input["include"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_str().unwrap().into())
                        .collect(),
                    max_results: input["max_results"].as_i64(),
                };
                let view = CacheEntry::Negative {
                    timestamp: Some(ssx_domain::WallClockTimestamp::new(100.0).unwrap()),
                }
                .view(context);
                assert_eq!(view.cached(), case["expected"]["cached"].as_bool().unwrap());
                assert_eq!(
                    storage::encode_response(&view.to_response()).unwrap(),
                    case["expected"]["payload"]
                );
                checked += 1;
            }
            "view" => {
                let response = storage::decode_response(input["canonical"].clone()).unwrap();
                let canonical = CanonicalResponse::new(response);
                let include = input["include"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_str().unwrap().into())
                    .collect();
                let view = canonical.view(include, input["max_results"].as_i64(), false);
                assert!(canonical.shares_evidence(view.canonical()));
                assert_eq!(
                    storage::encode_response(&view.to_response()).unwrap(),
                    case["expected"]
                );
                checked += 1;
            }
            _ => {}
        }
    }
    assert_eq!(checked, 57);
}
#[test]
fn canonical_nested_views_and_snapshot_pages_cannot_contaminate_evidence() {
    let canonical = CanonicalResponse::new(SearchResponse {
        query: "q".into(),
        query_id: "ssx-q".into(),
        results: (0..5)
            .map(|i| SearchResult {
                url: format!("https://e/{i}"),
                payload: Some(ssx_json::json!({"nested":{"x":"original"}})),
                ..Default::default()
            })
            .collect(),
        answers: vec![ssx_json::json!({"nested":["original"]})],
        ..Default::default()
    });
    let first = canonical.view(BTreeSet::new(), Some(1), false);
    let second = canonical.view(BTreeSet::new(), Some(5), true);
    assert!(first.canonical().shares_evidence(second.canonical()));
    let mut owned = first.to_response();
    owned.answers[0]["nested"] = ssx_json::json!(["changed"]);
    owned.results[0].payload = Some(ssx_json::json!({"changed":true}));
    assert_eq!(second.results().len(), 5);
    assert_eq!(
        second.canonical().response().answers[0]["nested"],
        ssx_json::json!(["original"])
    );
    assert_eq!(canonical.response().results.len(), 5);
    let snapshot = ssx_domain::SearchSnapshot::new(
        ssx_domain::SnapshotId::new("snap-one").unwrap(),
        "q".into(),
        ssx_domain::QueryId::new("ssx-q").unwrap(),
        canonical.response().results.clone(),
        Default::default(),
        ssx_domain::TenantId::new("t").unwrap(),
        ssx_domain::WallClockTimestamp::new(0.0).unwrap(),
        None,
    )
    .unwrap();
    let snapshot = std::sync::Arc::new(snapshot);
    let page = SnapshotPage::new(snapshot.clone(), 2, 2).unwrap();
    assert!(std::sync::Arc::ptr_eq(&snapshot, page.snapshot()));
    assert_eq!(page.result_id(0).unwrap().to_string(), "snap-one:2");
    assert_eq!(page.results().len(), 2);
    assert!(page.has_more());
    assert_eq!(snapshot.total(), 5);
    assert!(SnapshotPage::new(snapshot.clone(), 0, 2).is_err());
    assert!(SnapshotPage::new(snapshot.clone(), 1, 0).is_err());
    assert!(
        SnapshotPage::new(snapshot, u64::MAX, u64::MAX)
            .unwrap()
            .results()
            .is_empty()
    );
}
#[test]
fn routing_identity_uses_every_source_contributor_and_conditional_health() {
    let corpus: Value = ssx_json::from_str(CORPUS).unwrap();
    let observed = corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["kind"] == ssx_json::json!("routing"))
        .collect::<Vec<_>>();
    assert_eq!(observed.len(), 10);
    let native = observed
        .iter()
        .map(|case| {
            let cap = &case["input"]["cap"];
            RoutingIdentity {
                catalog: Some(vec![RoutingEngineIdentity {
                    name: "a".into(),
                    active: true,
                    enabled: cap["enabled"].as_bool().unwrap(),
                    auth_class: cap["auth_class"].as_str().unwrap().into(),
                    auth_configured: cap["auth_configured"].as_bool().unwrap(),
                    cost_class: cap["cost_class"].as_str().unwrap().into(),
                    circuit_open: cap["circuit_open"].as_bool().unwrap(),
                    observed_status: cap["last_known_status"].as_str().unwrap().into(),
                    observed_stale: cap["last_known_status_stale"].as_bool().unwrap(),
                }]),
                sensitive: ["s".into()].into_iter().collect(),
                tier1: ["a".into()].into_iter().collect(),
                budget: Some(RoutingBudgetIdentity {
                    max_engines: case["input"]["max_engines"].as_u64().unwrap() as usize,
                    max_cost_class: "paid".into(),
                    coverage_target: 0,
                }),
                ranking_strategy: "presence".into(),
                work_group_version: "v1".into(),
                rerank_identity: None,
                routing_extension: None,
            }
        })
        .collect::<Vec<_>>();
    // Source helper accepts a dynamic coverage label; native uses F09's typed
    // coverage count. Fingerprint bytes are intentionally versioned; dependency
    // equality effects must still exactly match these source observations.
    for i in 0..10 {
        for j in 0..10 {
            assert_eq!(
                observed[i]["expected"] == observed[j]["expected"],
                native[i].fingerprint() == native[j].fingerprint(),
                "{i}/{j}"
            );
        }
    }
    let base = &native[0];
    let key = CacheKeyInput {
        query: "q".into(),
        ..Default::default()
    };
    let fingerprint = base.fingerprint();
    let mut mutations = Vec::new();
    let mut v = base.clone();
    v.catalog = None;
    mutations.push(v);
    let mut v = base.clone();
    v.sensitive.insert("new".into());
    mutations.push(v);
    let mut v = base.clone();
    v.tier1.clear();
    mutations.push(v);
    let mut v = base.clone();
    v.budget = None;
    mutations.push(v);
    let mut v = base.clone();
    v.budget.as_mut().unwrap().max_cost_class = "free".into();
    mutations.push(v);
    let mut v = base.clone();
    v.budget.as_mut().unwrap().coverage_target = 2;
    mutations.push(v);
    let mut v = base.clone();
    v.ranking_strategy = "rrf".into();
    mutations.push(v);
    let mut v = base.clone();
    v.work_group_version = "v2".into();
    mutations.push(v);
    let mut v = base.clone();
    v.rerank_identity = Some(("provider".into(), "v1".into()));
    mutations.push(v);
    let mut v = base.clone();
    v.routing_extension = Some("jev:identity:captured-scope".into());
    mutations.push(v);
    for change in 0..3 {
        let mut v = base.clone();
        let engine = &mut v.catalog.as_mut().unwrap()[0];
        match change {
            0 => engine.name = "different".into(),
            1 => engine.auth_class = "api_key".into(),
            _ => engine.active = false,
        }
        mutations.push(v);
    }
    for v in mutations {
        assert_ne!(v.fingerprint(), fingerprint);
        assert_ne!(
            composed_key(&key, &v, None, true),
            composed_key(&key, base, None, true)
        );
    }
    assert_ne!(
        composed_key(&key, base, None, true),
        composed_key(&key, base, Some(100), true)
    );
    assert_ne!(
        composed_key(&key, base, None, true),
        composed_key(&key, base, None, false)
    );
}
#[test]
fn composed_cache_identity_preserves_source_timeout_and_suggestion_distinctions() {
    let corpus: Value = ssx_json::from_str(CORPUS).unwrap();
    let cases = corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == ssx_json::json!("composed-key"))
        .collect::<Vec<_>>();
    assert_eq!(cases.len(), 4);
    let identity = RoutingIdentity {
        catalog: None,
        sensitive: Default::default(),
        tier1: Default::default(),
        budget: None,
        ranking_strategy: "presence".into(),
        work_group_version: "v1".into(),
        rerank_identity: None,
        routing_extension: Some("routing:fixed".into()),
    };
    let keys = cases
        .iter()
        .map(|c| {
            composed_key(
                &CacheKeyInput {
                    query: c["input"]["query"].as_str().unwrap().into(),
                    ..Default::default()
                },
                &identity,
                c["input"]["interactive_timeout_ms"].as_u64(),
                c["input"]["generate_suggestions"].as_bool().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    for i in 0..4 {
        for j in 0..4 {
            assert_eq!(
                keys[i] == keys[j],
                cases[i]["expected"] == cases[j]["expected"]
            );
        }
    }
}

#[test]
fn native_v1_identity_bytes_survive_tuple_encoding_change() {
    let expected = include_str!("../../xtask/fixtures/f13/native-v1-identity-before.txt")
        .lines()
        .collect::<Vec<_>>();
    assert_eq!(expected.len(), 100);
    for i in 0..100 {
        let input = CacheKeyInput {
            query: format!(" \u{1c} q 🦀 café / + % {i} \u{1f}"),
            subreddit: None,
            language: "fr".into(),
            safesearch: i % 3,
            categories: vec!["news".into(), format!("z{i}"), "a".into()],
            engines: vec!["b".into(), "a".into()],
            page: ssx_json::Number::new(&(i + 1).to_string()).unwrap(),
            time_range: if i % 2 == 0 {
                Some("week".into())
            } else {
                None
            },
            media_type: Some("image".into()),
            date_from: Some("2020-01-01".into()),
            date_to: None,
            relative_window: Some(("2020-01-02".into(), "2020-01-09".into())),
        };
        let routing = RoutingIdentity {
            catalog: Some(vec![RoutingEngineIdentity {
                name: "a".into(),
                active: true,
                enabled: true,
                auth_class: "key".into(),
                auth_configured: i % 2 == 0,
                cost_class: "free".into(),
                circuit_open: i % 3 == 0,
                observed_status: "ok".into(),
                observed_stale: i % 2 == 1,
            }]),
            sensitive: ["s".into()].into(),
            tier1: ["a".into()].into(),
            budget: Some(RoutingBudgetIdentity {
                max_engines: i as usize,
                max_cost_class: "free".into(),
                coverage_target: 2,
            }),
            ranking_strategy: format!("rank{i}"),
            work_group_version: "v1".into(),
            rerank_identity: Some(("model".into(), "policy".into())),
            routing_extension: Some("a\n🦀".into()),
        };
        assert_eq!(
            format!(
                "{} {} {}",
                source_base_key(&input),
                routing.fingerprint(),
                composed_key(&input, &routing, Some(i as u64), i % 2 == 0)
            ),
            expected[i as usize]
        );
    }
    use sha2::{Digest as _, Sha256};
    let reference = |value: Value| {
        Sha256::digest(ssx_json::to_vec(&value).unwrap())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let none = RoutingIdentity {
        catalog: None,
        sensitive: Default::default(),
        tier1: Default::default(),
        budget: None,
        ranking_strategy: String::new(),
        work_group_version: String::new(),
        rerank_identity: None,
        routing_extension: None,
    };
    assert_eq!(
        none.fingerprint(),
        reference(ssx_json::json!([null, [], [], null, "", "", null, null]))
    );
    let mut empty = none.clone();
    empty.catalog = Some(Vec::new());
    assert_eq!(
        empty.fingerprint(),
        reference(ssx_json::json!([[], [], [], null, "", "", null, null]))
    );
    assert_ne!(none.fingerprint(), empty.fingerprint());
    let engine = |name: &str| RoutingEngineIdentity {
        name: name.into(),
        active: true,
        enabled: true,
        auth_class: "none".into(),
        auth_configured: false,
        cost_class: "free".into(),
        circuit_open: false,
        observed_status: "ok".into(),
        observed_stale: false,
    };
    let mut ordered = empty;
    ordered.catalog = Some(vec![engine("b"), engine("a")]);
    ordered.sensitive = ["b".into(), "a".into()].into();
    ordered.tier1 = ["z".into(), "s".into()].into();
    ordered.budget = Some(RoutingBudgetIdentity {
        max_engines: 1,
        max_cost_class: "free".into(),
        coverage_target: 0,
    });
    ordered.rerank_identity = Some((String::new(), String::new()));
    ordered.routing_extension = Some(String::new());
    assert_eq!(
        ordered.fingerprint(),
        reference(ssx_json::json!([
            [
                ["a", "none", false, "free", false, ["ok", false]],
                ["b", "none", false, "free", false, ["ok", false]]
            ],
            ["a", "b"],
            ["s", "z"],
            [1, "free", 0],
            "",
            "",
            ["", ""],
            ""
        ]))
    );
    let expected = ordered.fingerprint();
    ordered.catalog.as_mut().unwrap().reverse();
    assert_eq!(ordered.fingerprint(), expected);
}
#[test]
fn identity_hashing_does_not_apply_optional_storage_bounds_or_panic() {
    let input = CacheKeyInput {
        query: "x".repeat(ssx_json::MAX_JSON_BYTES + 1),
        ..Default::default()
    };
    let key = source_base_key(&input);
    assert!(key.as_str().starts_with("search:v2:"));
    assert_eq!(key.as_str().len(), 74);
    let identity = RoutingIdentity {
        catalog: None,
        sensitive: Default::default(),
        tier1: Default::default(),
        budget: None,
        ranking_strategy: "presence".into(),
        work_group_version: "v1".into(),
        rerank_identity: None,
        routing_extension: Some(input.query.clone()),
    };
    assert_eq!(identity.fingerprint().len(), 64);
    assert!(
        composed_key(&input, &identity, None, false)
            .as_str()
            .starts_with("search:native:v1:")
    );
}
