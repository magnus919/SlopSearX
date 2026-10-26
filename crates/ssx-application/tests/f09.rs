use ssx_application::{filters::*, policy::*, scope::*};
use ssx_domain::{ScopeDecision, SearchRequest};
use ssx_json::{Value, json};

fn text(v: &Value) -> String {
    v.as_str().unwrap().into()
}
fn strings(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(text).collect()
}
fn fixture() -> Value {
    ssx_json::from_str(include_str!("../../xtask/fixtures/f09/oracle.json")).unwrap()
}
fn catalog() -> CatalogSnapshot {
    CatalogSnapshot::new(
        fixture()["engines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| EngineCapability {
                name: text(&e["name"]),
                enabled: e["enabled"].as_bool().unwrap(),
                active: e["active"].as_bool().unwrap(),
                available: e["available"].as_bool().unwrap(),
                categories: strings(&e["categories"]),
                media_types: strings(&e["media_types"]),
                auth_ready: e["auth_ready"].as_bool().unwrap(),
                circuit_open: e["circuit_open"].as_bool().unwrap(),
                cost_class: text(&e["cost_class"]),
                health_status: text(&e["health_status"]),
                health_stale: e["health_stale"].as_bool().unwrap(),
                enforced_filters: e["enforced_filters"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(k, v)| (k.clone(), text(v)))
                    .collect(),
            })
            .collect(),
    )
    .unwrap()
}
fn service() -> PolicyService {
    PolicyService {
        policy: SearchPolicy::default(),
        catalog: catalog(),
        resolver: ScopeResolver::default(),
    }
}
fn proposal(names: &[&str]) -> ScopeProposal {
    ScopeProposal::generic(SearchRequest {
        query: "query".into(),
        engines: Some(names.iter().map(|n| (*n).into()).collect()),
        ..Default::default()
    })
}
#[test]
fn source_scope_replay() {
    for case in fixture()["scopes"].as_array().unwrap() {
        let mut r = ScopeResolver {
            tier1: ["alpha", "beta"].into_iter().map(str::to_owned).collect(),
            catalog_routing: case["catalog_routing"].as_bool().unwrap(),
            ..Default::default()
        };
        r.topics = case["topics"].as_array().map(|a| {
            a.iter()
                .map(|t| Topic {
                    name: text(&t["name"]),
                    keywords: strings(&t["keywords"]),
                    engines: strings(&t["engines"]),
                })
                .collect()
        });
        if !case["budget"].is_null() {
            r.budget = Some(RoutingBudget {
                max_engines: case["budget"]["max_engines"].as_u64().unwrap() as usize,
                max_cost_class: text(&case["budget"]["max_cost_class"]),
                coverage_target: case["budget"]["coverage_target"].as_u64().unwrap() as usize,
            });
        }
        let q = &case["request"];
        let request = SearchRequest {
            query: text(&q["query"]),
            engines: q["engines"].as_array().map(|_| strings(&q["engines"])),
            categories: q["categories"]
                .as_array()
                .map(|_| strings(&q["categories"])),
            media_type: q["media_type"].as_str().map(str::to_owned),
            ..Default::default()
        };
        let actual = r.resolve(&request, &catalog());
        let expected: ScopeDecision = ssx_json::from_value(case["expected"].clone()).unwrap();
        assert_eq!(actual, expected, "{}", case["name"]);
    }
}
#[test]
fn source_filter_policy_date_replay() {
    let f = fixture();
    let s = service();
    for c in f["filters"].as_array().unwrap() {
        let actual = resolve_filter_enforcement(
            &strings(&c["selected"]),
            c["field"].as_str().unwrap(),
            c["requested"].clone(),
            &s.catalog.declarations(),
        );
        assert_eq!(actual.to_value(), c["expected"]);
    }
    for c in f["policy"].as_array().unwrap() {
        let mut s = s.clone();
        s.policy.sensitive_allowed = c["allowed"].as_bool().unwrap();
        let actual = s
            .engine_rejection(&strings(&c["names"]), "engines")
            .err()
            .map(|e| {
                let mut v = e.to_value()["error"].clone();
                v.as_object_mut().unwrap().shift_remove("field");
                v
            })
            .unwrap_or(Value::Null);
        assert_eq!(actual, c["expected"]);
    }
    for c in f["dates"].as_array().unwrap() {
        match PublicationBounds::new(c["start"].as_str(), c["end"].as_str()) {
            Ok(b) => {
                let contains: Vec<_> = [
                    None,
                    Some("2024-02-29"),
                    Some("2024-03-01"),
                    Some("20240101"),
                    Some("bad"),
                ]
                .into_iter()
                .map(|v| b.contains(v))
                .collect();
                assert_eq!(json!(contains), c["expected"]["contains"]);
            }
            Err(e) => {
                assert_eq!(e.message, text(&c["expected"]["error"]));
                assert_eq!(e.field, c["expected"]["field"].as_str().unwrap());
            }
        }
    }
    for c in f["relative"].as_array().unwrap() {
        let day = publication_day(c["today"].as_str().unwrap()).unwrap();
        assert_eq!(
            json!(within_time_range(
                c["value"].as_str(),
                c["range"].as_str().unwrap(),
                day
            )),
            c["expected"],
            "{}",
            c["value"]
        );
    }
}
#[test]
fn denied_paths_never_dispatch_and_preview_has_no_authority() {
    let mut s = service();
    let mut calls = 0;
    for surface in [
        SearchSurface::Generic,
        SearchSurface::Targeted,
        SearchSurface::Jobs,
        SearchSurface::Security,
        SearchSurface::Science,
        SearchSurface::Research,
        SearchSurface::Preview,
        SearchSurface::Worker,
    ] {
        let mut p = proposal(&["alpha", "hibp"]);
        p.surface = surface;
        if let Ok(token) = s.authorize(p) {
            let _ = s.with_revalidated(&token, |_| calls += 1);
        }
    }
    assert_eq!(calls, 0);
    let token = s.authorize(proposal(&["alpha"])).unwrap();
    s.with_revalidated(&token, |_| calls += 1).unwrap();
    assert_eq!(calls, 1);
    let mut e = s.catalog.engines().to_vec();
    e[0].enabled = false;
    s.catalog = CatalogSnapshot::new(e).unwrap();
    assert!(s.with_revalidated(&token, |_| calls += 1).is_err());
    assert_eq!(calls, 1);
    let mut s = service();
    s.policy.grants.insert("research".into());
    let mut p = proposal(&["alpha"]);
    p.surface = SearchSurface::Research;
    p.intent = "web".into();
    let token = s.authorize(p).unwrap();
    s.policy.grants.clear();
    assert!(s.with_revalidated(&token, |_| calls += 1).is_err());
    assert_eq!(calls, 1);
    let mut p = proposal(&["alpha"]);
    p.surface = SearchSurface::Preview;
    assert!(s.preview(&p).is_ok());
    assert!(s.authorize(p).is_err());
}
#[test]
fn http_preserves_unknown_and_explicit_sensitive_semantics() {
    let s = service();
    let mut p = proposal(&["missing", "alpha", "hibp"]);
    assert!(s.preview(&p).is_err());
    p.surface = SearchSurface::Http;
    let preview = s.preview(&p).unwrap();
    assert_eq!(preview.scope.selected_engines, ["alpha", "hibp"]);
    assert!(s.authorize(p).is_ok());
}
#[test]
fn strict_filters_invalid_bounds_and_availability_fail_closed() {
    let s = service();
    for (names, range, from, to, safe) in [
        (vec!["alpha", "beta"], None, None, None, 2),
        (vec!["beta"], Some("all"), None, None, 0),
        (vec!["alpha"], None, Some("2023-02-29"), None, 0),
        (
            vec!["alpha"],
            None,
            Some("2024-03-01"),
            Some("2024-02-29"),
            0,
        ),
    ] {
        let mut p = proposal(&names);
        p.request.time_range = range.map(str::to_owned);
        p.request.date_from = from.map(str::to_owned);
        p.request.date_to = to.map(str::to_owned);
        p.request.safesearch = safe;
        assert!(s.authorize(p).is_err());
    }
    let mut s = service();
    let mut engines = s.catalog.engines().to_vec();
    engines[0].available = false;
    s.catalog = CatalogSnapshot::new(engines).unwrap();
    assert!(s.preview(&proposal(&["alpha"])).is_ok());
    assert!(s.authorize(proposal(&["alpha"])).is_err());
}
#[test]
fn revalidation_detects_enforcement_and_scope_changes() {
    let mut s = service();
    let token = s.authorize(proposal(&["alpha"])).unwrap();
    let mut engines = s.catalog.engines().to_vec();
    engines[0]
        .enforced_filters
        .insert("language".into(), "upstream".into());
    s.catalog = CatalogSnapshot::new(engines).unwrap();
    assert!(s.with_revalidated(&token, |_| panic!("dispatch")).is_err());
}
#[test]
fn specialist_grants_remain_independent_of_sensitive_grant_and_origin() {
    let mut s = service();
    for surface in [
        SearchSurface::Jobs,
        SearchSurface::Security,
        SearchSurface::Science,
        SearchSurface::Research,
    ] {
        let mut p = proposal(&["alpha"]);
        p.surface = surface;
        if surface == SearchSurface::Research {
            p.intent = "web".into();
        }
        assert!(s.authorize(p.clone()).is_err());
        let grant = match surface {
            SearchSurface::Jobs => "jobs",
            SearchSurface::Security => "security",
            SearchSurface::Science => "science",
            _ => "research",
        };
        s.policy.grants.insert(grant.into());
        let token = s.authorize(p).unwrap();
        s.with_revalidated(&token, |_| ()).unwrap();
        s.policy.grants.remove(grant);
        assert!(
            s.with_revalidated(&token, |_| panic!("revoked specialist dispatched"))
                .is_err()
        );
    }
    s.policy
        .grants
        .extend(["jobs", "security", "science", "research"].map(str::to_owned));
    let mut p = proposal(&["alpha", "hibp"]);
    p.surface = SearchSurface::Security;
    assert!(s.authorize(p.clone()).is_err());
    s.policy.sensitive_allowed = true;
    assert!(s.authorize(p).is_ok());
    let mut p = proposal(&[]);
    p.surface = SearchSurface::Worker;
    assert!(s.authorize(p).is_err());
    let mut p = proposal(&["alpha"]);
    p.intent = "jobs".into();
    s.policy.grants.clear();
    assert!(
        s.authorize(p).is_ok(),
        "explicit engine precedence must match source generic intent handling"
    );
}
#[test]
fn strict_positive_and_truthful_local_filter_layers() {
    let s = service();
    let mut p = proposal(&["beta"]);
    p.request.safesearch = 2;
    let t = s.authorize(p).unwrap();
    assert_eq!(
        t.preview().enforcement["safesearch"].status,
        EnforcementStatus::Enforced
    );
    let p = proposal(&["alpha"]);
    let t = s.authorize(p).unwrap();
    assert_eq!(
        t.preview().enforcement["language"].status,
        EnforcementStatus::Unsupported
    );
    assert!(t.preview().enforcement["language"].enforced_by.is_empty());
}
#[test]
fn research_intent_and_captured_evidence_are_revalidated() {
    use ssx_domain::research::ResearchQuery;
    let mut s = service();
    s.policy
        .grants
        .extend(["research", "science"].map(str::to_owned));
    let q = ResearchQuery {
        query: "query".into(),
        intent: "science".into(),
        engines: vec!["alpha".into()],
        requires_intent_grant: true,
        planning_method: Some("evidence_followup".into()),
        evidence_result_ids: vec!["result-id".into()],
        evidence_engines: vec!["beta".into()],
        evidence_intent: Some("science".into()),
        parent_attempt_id: Some("attempt".into()),
        rationale: Some("follow up".into()),
        ..Default::default()
    };
    let token = s.authorize(ScopeProposal::from_research_query(&q)).unwrap();
    s.policy.grants.remove("science");
    assert!(
        s.with_revalidated(&token, |_| panic!("revoked intent"))
            .is_err()
    );
    s.policy.grants.insert("science".into());
    let mut q = q.clone();
    q.intent = "web".into();
    let token = s.authorize(ScopeProposal::from_research_query(&q)).unwrap();
    s.policy.grants.remove("science");
    assert!(
        s.with_revalidated(&token, |_| panic!("revoked evidence"))
            .is_err()
    );
    s.policy.grants.insert("science".into());
    q.evidence_engines = vec!["hibp".into()];
    assert!(s.authorize(ScopeProposal::from_research_query(&q)).is_err());
    q.evidence_engines = vec![];
    assert!(s.authorize(ScopeProposal::from_research_query(&q)).is_err());
    q.evidence_result_ids.clear();
    q.planning_method = None;
    let mut p = ScopeProposal::from_research_query(&q);
    p.surface = SearchSurface::Http;
    assert!(s.authorize(p).is_err());
}
#[test]
fn validated_command_preserves_specialist_values_and_pagination_truth() {
    let mut s = service();
    s.policy.grants.insert("jobs".into());
    let mut p = proposal(&["alpha"]);
    p.surface = SearchSurface::Jobs;
    p.job_location = Some("Zürich".into());
    p.job_employment_type = Some("full_time".into());
    let token = s.authorize(p).unwrap();
    s.with_revalidated(&token, |command| {
        assert_eq!(command.origin(), SearchSurface::Jobs);
        assert_eq!(command.job_location(), Some("Zürich"));
        assert_eq!(command.job_employment_type(), Some("full_time"));
        assert_eq!(command.request().query, "query");
        assert_eq!(
            command.preview().enforcement["location"].status,
            EnforcementStatus::Unsupported
        );
    })
    .unwrap();
    let mut p = proposal(&["alpha"]);
    p.request.page = ssx_domain::SearchPage::from_u64(2).unwrap();
    let t = s.authorize(p).unwrap();
    assert_eq!(
        t.preview().enforcement["pagination"].to_value()["enforced_by"],
        json!(["upstream:alpha"])
    );
}
#[test]
fn local_time_postfilter_requires_actual_publication_evidence() {
    for invalid in ["+001W011", "2024W+11", "2024-W+1"] {
        assert_eq!(
            within_time_range(
                Some(invalid),
                "week",
                publication_day("2024-03-01").unwrap()
            ),
            None
        );
    }
    assert_eq!(
        within_time_range(Some("2024-03-01"), "week", i64::MIN),
        None
    );
    let today = publication_day("2024-03-01").unwrap();
    let results = [
        None,
        Some("2024-02-29T12:00Z"),
        Some("2024-02-21"),
        Some("not-a-date"),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, d)| ssx_domain::SearchResult {
        url: format!("https://example.com/2024-03-01/{i}"),
        published_date: d.map(str::to_owned),
        ..Default::default()
    })
    .collect::<Vec<_>>();
    assert_eq!(
        filter_results_by_time_range(&results, "week", today),
        vec![results[1].clone()]
    );
    assert_eq!(
        filter_results_by_time_range(&results, "all", today),
        results
    );
}
#[test]
fn source_public_denials_and_research_dispatch_policy() {
    let corpus: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/f09/authority.json")).unwrap();
    for c in corpus["surface_denials"].as_array().unwrap() {
        let mut p = proposal(&["alpha", "hibp"]);
        p.surface = match c["surface"].as_str().unwrap() {
            "generic" => SearchSurface::Generic,
            "targeted" => SearchSurface::Targeted,
            "preview" => SearchSurface::Preview,
            "jobs" => SearchSurface::Jobs,
            "security" => SearchSurface::Security,
            "science" => SearchSurface::Science,
            _ => SearchSurface::Research,
        };
        assert_eq!(
            service().preview(&p).unwrap_err().to_value(),
            c["expected"],
            "{}",
            c["surface"]
        );
    }
    for c in corpus["workers"].as_array().unwrap() {
        let mut s = service();
        s.policy.grants = strings(&c["grants"]).into_iter().collect();
        let q: ssx_domain::research::ResearchQuery =
            ssx_json::from_value(c["query"].clone()).unwrap();
        let p = ScopeProposal::from_research_query(&q);
        let result = s.authorize(p);
        assert_eq!(
            result
                .err()
                .map(|e| json!(e.message))
                .unwrap_or(Value::Null),
            c["expected"],
            "{}",
            c["name"]
        );
    }
}

#[test]
fn default_template_and_explicit_research_intent_grants_remain_distinct() {
    let mut s = service();
    s.policy.grants.insert("research".into());
    let mut q = ssx_domain::research::ResearchQuery {
        query: "query".into(),
        intent: "science".into(),
        engines: vec!["alpha".into()],
        requires_intent_grant: false,
        ..Default::default()
    };
    // Source-generated default templates need research, not a specialist grant.
    // Caller-provided queries set the flag true at the admission boundary.
    let mut calls = 0;
    let template = s.authorize(ScopeProposal::from_research_query(&q)).unwrap();
    s.with_revalidated(&template, |_| calls += 1).unwrap();
    q.requires_intent_grant = true;
    assert!(s.authorize(ScopeProposal::from_research_query(&q)).is_err());
    s.policy.grants.insert("science".into());
    let explicit = s.authorize(ScopeProposal::from_research_query(&q)).unwrap();
    s.with_revalidated(&explicit, |_| calls += 1).unwrap();
    s.policy.grants.remove("research");
    assert!(s.with_revalidated(&template, |_| calls += 1).is_err());
    assert!(s.with_revalidated(&explicit, |_| calls += 1).is_err());
    s.policy.grants.insert("research".into());
    s.policy.sensitive_engines.insert("alpha".into());
    assert!(s.with_revalidated(&template, |_| calls += 1).is_err());
    assert!(s.with_revalidated(&explicit, |_| calls += 1).is_err());
    assert_eq!(calls, 2);
}

#[test]
fn explicit_inactive_members_reject_atomically_without_changing_http_or_preview_availability() {
    let capture: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/f09/inactive.json")).unwrap();
    assert!(capture["source"]["validation"].is_null());
    assert_eq!(strings(&capture["source"]["selected_engines"]), ["alpha"]);
    for available in [true, false] {
        let mut s = service();
        s.policy
            .grants
            .extend(["research", "science"].map(str::to_owned));
        let token = s.authorize(proposal(&["alpha", "beta"])).unwrap();
        let mut engines = s.catalog.engines().to_vec();
        let beta = engines.iter_mut().find(|e| e.name == "beta").unwrap();
        beta.active = false;
        beta.available = available;
        s.catalog = CatalogSnapshot::new(engines).unwrap();
        let mut calls = 0;
        assert!(s.with_revalidated(&token, |_| calls += 1).is_err());
        let mixed = proposal(&["alpha", "beta"]);
        let error = s.preview(&mixed).unwrap_err();
        assert_eq!(error.code, "invalid_scope");
        assert!(!strings(&error.details["valid_alternatives"]).contains(&"beta".into()));
        let admission = s.authorize(mixed.clone());
        assert!(admission.is_err());
        if let Ok(t) = admission {
            s.with_revalidated(&t, |_| calls += 1).unwrap();
        }
        let mut http = mixed;
        http.surface = SearchSurface::Http;
        assert_eq!(s.preview(&http).unwrap().scope.selected_engines, ["alpha"]);
        let q = ssx_domain::research::ResearchQuery {
            query: "query".into(),
            intent: "science".into(),
            engines: vec!["alpha".into()],
            planning_method: Some("evidence_followup".into()),
            evidence_result_ids: vec!["result-id".into()],
            evidence_engines: vec!["beta".into()],
            evidence_intent: Some("science".into()),
            parent_attempt_id: Some("attempt".into()),
            rationale: Some("follow up".into()),
            ..Default::default()
        };
        let evidence_error = s
            .authorize(ScopeProposal::from_research_query(&q))
            .unwrap_err();
        assert_eq!(evidence_error.code, "invalid_scope");
        assert_eq!(evidence_error.field, "engines");
        assert_eq!(
            evidence_error.message,
            "unknown or inactive engines: beta (inactive)"
        );
        assert_eq!(calls, 0);
    }
    let mut s = service();
    let mut engines = s.catalog.engines().to_vec();
    engines
        .iter_mut()
        .find(|e| e.name == "beta")
        .unwrap()
        .available = false;
    s.catalog = CatalogSnapshot::new(engines).unwrap();
    assert!(s.preview(&proposal(&["alpha", "beta"])).is_ok());
    let unavailable = s.authorize(proposal(&["alpha", "beta"])).unwrap_err();
    assert_eq!(unavailable.code, "invalid_scope");
    assert_eq!(unavailable.field, "engines");
    assert_eq!(
        unavailable.message,
        "selected native engine implementation is unavailable"
    );
    assert_eq!(strings(&unavailable.details["engines"]), ["beta"]);
}

#[test]
fn recovered_research_rejects_unknown_and_media_intents_before_dispatch() {
    let mut s = service();
    s.policy.grants.insert("research".into());
    let mut calls = 0;
    for intent in [
        "",
        "auto",
        "unknown",
        "security ",
        " Security",
        "SECURITY",
        "images",
        "videos",
    ] {
        let q = ssx_domain::research::ResearchQuery {
            query: "query".into(),
            intent: intent.into(),
            engines: vec!["alpha".into()],
            ..Default::default()
        };
        let result = s.authorize(ScopeProposal::from_research_query(&q));
        if let Ok(token) = &result {
            let _ = s.with_revalidated(token, |_| calls += 1);
        }
        assert!(
            result.is_err(),
            "accepted invalid research intent {intent:?}"
        );
    }
    assert_eq!(calls, 0);
    let q = ssx_domain::research::ResearchQuery {
        query: "query".into(),
        intent: "web".into(),
        engines: vec!["alpha".into()],
        ..Default::default()
    };
    let token = s.authorize(ScopeProposal::from_research_query(&q)).unwrap();
    s.with_revalidated(&token, |_| calls += 1).unwrap();
    assert_eq!(calls, 1);
    for intent in ["jobs", "security", "science"] {
        let mut specialist = q.clone();
        specialist.intent = intent.into();
        assert!(
            s.authorize(ScopeProposal::from_research_query(&specialist))
                .is_err()
        );
        s.policy.grants.insert(intent.into());
        let token = s
            .authorize(ScopeProposal::from_research_query(&specialist))
            .unwrap();
        s.policy.grants.remove(intent);
        assert!(s.with_revalidated(&token, |_| calls += 1).is_err());
    }
    assert_eq!(calls, 1);
}

#[test]
fn high_page_policy_preserves_exact_request_and_numeric_enforcement_value() {
    let service = service();
    let raw = "170141183460469231731687303715884105728";
    let mut p = proposal(&["alpha"]);
    p.surface = SearchSurface::Http;
    p.request.page = ssx_domain::SearchPage::new(raw).unwrap();
    let token = service.authorize(p).unwrap();
    service
        .with_revalidated(&token, |command| {
            assert_eq!(command.request().page.as_str(), raw);
            assert_eq!(
                command.preview().enforcement["pagination"].to_value()["requested"].to_string(),
                raw
            );
        })
        .unwrap();
}
