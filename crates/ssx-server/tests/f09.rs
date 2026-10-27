use ssx_application::policy::{PolicyService, ScopeProposal, SearchPolicy};
use ssx_config::{Config, load_config_with};
use ssx_domain::{SearchRequest, WallClockTimestamp};
use ssx_registry::Registry;
use ssx_server::{project_catalog, project_resolver};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn load(yaml: &str, env: &BTreeMap<String, String>) -> Config {
    let path = std::env::temp_dir().join(format!(
        "ssx-f09-{}-{}.yaml",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&path, yaml).unwrap();
    let result = load_config_with(Some(&path), env);
    fs::remove_file(path).unwrap();
    result.unwrap()
}
#[test]
fn actual_f06_named_topic_order_aliases_merges_duplicates() {
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("../../xtask/fixtures/f09/topic-order.json")).unwrap();
    let registry = Registry::builtins().unwrap();
    let active = registry.descriptors().map(|d| d.name.clone()).collect();
    for c in corpus["cases"].as_array().unwrap() {
        let cfg = load(c["yaml"].as_str().unwrap(), &BTreeMap::new());
        assert_eq!(serde_json::json!(cfg.routing.named_topic_order), c["order"]);
        let cat = project_catalog(
            &cfg,
            &registry,
            &active,
            &BTreeMap::new(),
            WallClockTimestamp::new(1.0).unwrap(),
            300.0,
        )
        .unwrap();
        let resolver = project_resolver(&cfg, &BTreeMap::new()).unwrap();
        let scope = resolver.resolve(
            &SearchRequest {
                query: "overlap".into(),
                ..Default::default()
            },
            &cat,
        );
        assert_eq!(
            serde_json::json!(scope.selected_engines),
            c["selected"],
            "{}",
            c["name"]
        );
        assert_eq!(serde_json::json!(scope.matched_topic), c["topic"]);
    }
}
#[test]
fn accepted_catalog_projection_keeps_distinct_eligibility_and_redacts() {
    let cfg = load(
        "engines:\n  brave:\n    categories: [custom]\n  wikipedia:\n    enabled: false\n  github:\n    api_key: synthetic-test-only\n",
        &BTreeMap::new(),
    );
    let registry = Registry::builtins().unwrap();
    let active: BTreeSet<_> = registry.descriptors().map(|d| d.name.clone()).collect();
    let catalog = project_catalog(
        &cfg,
        &registry,
        &active,
        &BTreeMap::new(),
        WallClockTimestamp::new(1.0).unwrap(),
        300.0,
    )
    .unwrap();
    assert_eq!(catalog.engines().len(), registry.len());
    let deepseek = catalog.get("deepseek").unwrap();
    assert!(!deepseek.enabled);
    assert!(!deepseek.auth_ready);
    assert!(deepseek.enforced_filters.is_empty());
    assert_eq!(catalog.get("brave").unwrap().categories, ["custom"]);
    assert!(!catalog.get("brave").unwrap().auth_ready);
    assert!(catalog.get("brave").unwrap().enabled);
    assert!(!catalog.get("wikipedia").unwrap().enabled);
    assert!(catalog.get("wikipedia").unwrap().active);
    assert!(catalog.get("github").unwrap().auth_ready);
    assert!(!format!("{catalog:?}").contains("synthetic-test-only"));
    assert!(catalog.engines().iter().all(|e| !e.available));
    let s = PolicyService {
        policy: SearchPolicy::default(),
        catalog,
        resolver: project_resolver(&cfg, &BTreeMap::new()).unwrap(),
    };
    let p = ScopeProposal::generic(SearchRequest {
        query: "query".into(),
        engines: Some(vec!["github".into()]),
        ..Default::default()
    });
    assert!(s.preview(&p).is_ok());
    assert!(s.authorize(p).is_err());
}
