use ssx_application::saved::{compare_observations, observe};
use ssx_domain::{
    EngineOutcome, ScopeDecision, SearchResponse, SearchResult,
    canonical::{JsonProfile, canonical_json},
    saved::SavedDefinition,
};
use ssx_json::Value;
#[test]
fn exact_source_observations_reports_baselines() {
    let source: Value =
        ssx_json::from_str(include_str!("../../../docs/rust-rewrite/w04/source.json")).unwrap();
    for case in source["cases"].as_array().unwrap() {
        let d = SavedDefinition::from_payload(&case["definition"]).unwrap();
        let rows = case["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                let mut result = SearchResult {
                    url: r["url"].as_str().unwrap().into(),
                    title: r["title"].as_str().unwrap().into(),
                    content: r["content"].as_str().unwrap().into(),
                    engine: r["engine"].as_str().unwrap().into(),
                    ..Default::default()
                };
                if let Some(n) = r["score"].as_f64() {
                    result.score = ssx_domain::FiniteF64::new(n).unwrap();
                }
                if let Some(n) = r["position"].as_i64() {
                    result.position = n;
                }
                if let Some(n) = r["tier"].as_i64() {
                    result.tier = n;
                }
                result.published_date = r["published_date"].as_str().map(str::to_owned);
                result.engines = r["engines"]
                    .as_array()
                    .map(|a| a.iter().map(|v| v.as_str().unwrap().to_owned()).collect())
                    .unwrap_or_default();
                result
            })
            .collect::<Vec<_>>();
        let settings = &case["response"];
        let response = SearchResponse {
            engine_outcomes: vec![EngineOutcome {
                engine: "wikipedia".into(),
                status: settings["status"].as_str().unwrap_or("ok").into(),
                result_count: rows.len() as i64,
                ..Default::default()
            }],
            results: rows,
            scope: ScopeDecision {
                selected_engines: settings["scope"]
                    .as_array()
                    .map(|a| a.iter().map(|s| s.as_str().unwrap().into()).collect())
                    .unwrap_or_else(|| vec!["wikipedia".into()]),
                ..Default::default()
            },
            query_id: "q-fixed".into(),
            cached: settings["cached"].as_bool().unwrap_or(false),
            partial: settings["partial"].as_bool().unwrap_or(false),
            deadline_exceeded: settings["deadline_exceeded"].as_bool().unwrap_or(false),
            all_unresponsive: settings["all_unresponsive"].as_bool().unwrap_or(false),
            ..Default::default()
        };
        let observation = observe(&d, &response, 20.0).unwrap();
        assert_eq!(
            observation, case["observation"],
            "{} observation",
            case["name"]
        );
        let (report, replacement) =
            compare_observations(&d, observation, "run-golden", 20.0, 20.0, 20.0, 2).unwrap();
        assert_eq!(report, case["report"], "{} report", case["name"]);
        assert_eq!(
            replacement,
            if case["replacement"].is_null() {
                None
            } else {
                Some(case["replacement"].clone())
            }
        );
        assert_eq!(
            canonical_json(&report, JsonProfile::AsciiSpaced)
                .unwrap()
                .len() as u64,
            case["report_spaced_bytes"].as_u64().unwrap()
        );
        use sha2::Digest;
        let hash: String =
            sha2::Sha256::digest(canonical_json(&report, JsonProfile::AsciiSpaced).unwrap())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
        assert_eq!(hash, case["report_spaced_sha256"].as_str().unwrap());
    }
}
#[test]
fn report_size_uses_ascii_spaced_bytes_and_retains_prior_baseline_without_panicking() {
    let source: Value =
        ssx_json::from_str(include_str!("../../../docs/rust-rewrite/w04/source.json")).unwrap();
    let mut d = SavedDefinition::from_payload(&source["cases"][0]["definition"]).unwrap();
    let mut response = SearchResponse {
        scope: ScopeDecision {
            selected_engines: vec!["wikipedia".into()],
            ..Default::default()
        },
        engine_outcomes: vec![EngineOutcome {
            engine: "wikipedia".into(),
            status: "ok".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    response.results = (0..100)
        .map(|i| SearchResult {
            url: format!("https://{i}.test"),
            title: "雪".repeat(2730),
            content: "雪".repeat(2730),
            published_date: Some("雪".repeat(2730)),
            engine: "wikipedia".into(),
            ..Default::default()
        })
        .collect();
    let observation = observe(&d, &response, 20.0).unwrap();
    assert!(
        observation["incomparable_reasons"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(ssx_json::to_vec(&observation).unwrap().len() < 4 * 1024 * 1024);
    let (report, replacement) =
        compare_observations(&d, observation, "run", 20.0, 20.0, 20.0, 0).unwrap();
    assert_eq!(
        report["incomparable_reasons"],
        ssx_json::json!(["report_size_exceeded"])
    );
    assert!(
        report["observation"]["records"]
            .as_object()
            .unwrap()
            .is_empty()
    );
    assert!(replacement.is_none());
    for row in &mut response.results {
        row.title = "a".repeat(8192);
        row.content = "b".repeat(8192);
        row.published_date = Some("c".repeat(8192));
    }
    let observation = observe(&d, &response, 20.0).unwrap();
    d.baseline = Some({
        let mut v = observation.clone();
        v["expires_at"] = ssx_json::json!(3000.0);
        v
    });
    for row in &mut response.results {
        row.title = "d".repeat(8192);
    }
    let observation = observe(&d, &response, 30.0).unwrap();
    let (report, replacement) =
        compare_observations(&d, observation, "run", 30.0, 30.0, 30.0, 0).unwrap();
    assert_eq!(report["status"], "incomparable");
    assert!(replacement.is_none());
}
