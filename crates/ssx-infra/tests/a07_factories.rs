//! Actual factory/config/scope boundary; source replay remains a07_biomedical.
use ssx_application::{
    SearchContext,
    scope::{CatalogSnapshot, EngineCapability, ScopeResolver},
};
use ssx_config::{ConfigScalar, load_config_with};
use ssx_domain::{EngineStatus, SearchRequest};
use ssx_infra::{
    adapters::{AllowAllAdmission, BiomedicalSettings, register_biomedical},
    http::{AdapterHttp, ExchangeFuture, Request, Response, Transport},
};
use ssx_registry::{Registry, RegistryError};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
const NAMES: [&str; 5] = [
    "clinicaltrials",
    "europepmc",
    "pubmed",
    "pubchem",
    "uniprot",
];
fn config(yaml: &str, env: BTreeMap<String, String>) -> ssx_config::Config {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "a07-factory-{}-{}.yaml",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::write(&path, yaml).unwrap();
    let result = load_config_with(Some(&path), &env);
    std::fs::remove_file(path).unwrap();
    result.unwrap()
}
struct Replay {
    bodies: Mutex<VecDeque<Vec<u8>>>,
    requests: Mutex<Vec<Request>>,
}
impl Transport for Replay {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            let url = request.url.clone();
            self.requests.lock().unwrap().push(request);
            Ok(Response {
                url,
                status: 200,
                headers: Default::default(),
                body: self
                    .bodies
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("unexpected factory request"),
            })
        })
    }
}
fn wired(config: &ssx_config::Config, bodies: &[&str]) -> (Registry, Arc<Replay>) {
    let replay = Arc::new(Replay {
        bodies: Mutex::new(bodies.iter().map(|s| s.as_bytes().to_vec()).collect()),
        requests: Mutex::new(Vec::new()),
    });
    let http = Arc::new(AdapterHttp::default());
    http.set_transport(replay.clone()).unwrap();
    let mut registry = Registry::builtins().unwrap();
    register_biomedical(
        &mut registry,
        http,
        Arc::new(AllowAllAdmission),
        BiomedicalSettings::from_resolved(config),
    )
    .unwrap();
    (registry, replay)
}
#[test]
fn real_f06_scalars_and_source_base_url_normalization() {
    for raw in ["true", "2", "2.0", "'2'"] {
        let c = config(
            &format!(
                "engines:\n  clinicaltrials:\n    base_url: '\u{2003} '\n    max_results: {raw}\n"
            ),
            BTreeMap::new(),
        );
        let settings = BiomedicalSettings::from_resolved(&c);
        assert_eq!(
            settings.clinicaltrials.max_results,
            c.engines["clinicaltrials"].max_results
        );
        assert_eq!(
            settings.clinicaltrials.base_url,
            "https://clinicaltrials.gov/api/v2/studies"
        );
    }
    let c = config(
        "engines:\n  pubmed:\n    base_url: ' https://example.test/raw '\n",
        BTreeMap::from([("ENGINE_PUBMED_TIMEOUT_MS".into(), "5_000".into())]),
    );
    let s = BiomedicalSettings::from_resolved(&c);
    assert_eq!(s.pubmed.base_url, " https://example.test/raw ");
    assert_eq!(s.pubmed.timeout_ms, c.engines["pubmed"].timeout_ms);
    assert_eq!(s.pubchem.max_results, ConfigScalar::Integer(10));
}
#[test]
fn every_factory_preserves_real_yaml_and_environment_scalar_variants() {
    for name in NAMES {
        for raw in ["true", "2", "2.0", "'2'"] {
            let c = config(
                &format!("engines:\n  {name}:\n    max_results: {raw}\n    timeout_ms: {raw}\n"),
                BTreeMap::new(),
            );
            let settings = BiomedicalSettings::from_resolved(&c);
            let actual = match name {
                "clinicaltrials" => settings.clinicaltrials,
                "europepmc" => settings.europepmc,
                "pubmed" => settings.pubmed,
                "pubchem" => settings.pubchem,
                _ => settings.uniprot,
            };
            assert_eq!(actual.max_results, c.engines[name].max_results);
            assert_eq!(actual.timeout_ms, c.engines[name].timeout_ms);
        }
        let c = config(
            &format!("engines:\n  {name}: {{}}\n"),
            BTreeMap::from([
                (
                    format!("ENGINE_{}_MAX_RESULTS", name.to_uppercase()),
                    "2".into(),
                ),
                (
                    format!("ENGINE_{}_TIMEOUT_MS", name.to_uppercase()),
                    "7000".into(),
                ),
            ]),
        );
        let settings = BiomedicalSettings::from_resolved(&c);
        let actual = match name {
            "clinicaltrials" => settings.clinicaltrials,
            "europepmc" => settings.europepmc,
            "pubmed" => settings.pubmed,
            "pubchem" => settings.pubchem,
            _ => settings.uniprot,
        };
        assert_eq!(actual.max_results, ConfigScalar::Integer(2));
        assert_eq!(actual.timeout_ms, ConfigScalar::Integer(7000));
    }
}
#[tokio::test]
async fn all_five_factories_execute_actual_adapters_and_keep_context() {
    let cases: [(&str, Vec<&str>); 5] = [
        (
            "clinicaltrials",
            vec![include_str!(
                "../../../docs/rust-rewrite/a07/fixtures/clinicaltrials-success.json"
            )],
        ),
        (
            "europepmc",
            vec![include_str!(
                "../../../docs/rust-rewrite/a07/fixtures/europepmc-preprints.json"
            )],
        ),
        (
            "pubmed",
            vec![
                include_str!("../../../docs/rust-rewrite/a07/fixtures/pubmed-search.json"),
                include_str!("../../../docs/rust-rewrite/a07/fixtures/pubmed-summary.json"),
            ],
        ),
        (
            "pubchem",
            vec![
                include_str!("../../../docs/rust-rewrite/a07/fixtures/pubchem-search.json"),
                include_str!("../../../docs/rust-rewrite/a07/fixtures/pubchem-detail.json"),
            ],
        ),
        (
            "uniprot",
            vec![include_str!(
                "../../../docs/rust-rewrite/a07/fixtures/uniprot-success.json"
            )],
        ),
    ];
    for (name, bodies) in cases {
        let c = config("", BTreeMap::new());
        let (registry, replay) = wired(&c, &bodies);
        assert!(registry.is_available(name));
        let adapter = registry.instantiate(name).unwrap();
        let result = adapter
            .search_with_context(
                &SearchRequest {
                    query: "aspirin".into(),
                    ..Default::default()
                },
                SearchContext {
                    deadline: Instant::now() + Duration::from_secs(5),
                },
            )
            .await;
        assert_eq!(
            result.status,
            EngineStatus::Ok,
            "{name}: {:?}",
            result.error_message
        );
        assert!(!result.results.is_empty(), "{name}");
        assert_eq!(replay.requests.lock().unwrap().len(), bodies.len());
        assert!(
            replay
                .requests
                .lock()
                .unwrap()
                .iter()
                .all(|r| r.method == "GET")
        );
        let before = replay.requests.lock().unwrap().len();
        let expired = adapter
            .search_with_context(
                &SearchRequest::default(),
                SearchContext {
                    deadline: Instant::now() - Duration::from_millis(1),
                },
            )
            .await;
        assert_eq!(expired.status, EngineStatus::Timeout);
        assert!(expired.results.is_empty());
        assert_eq!(replay.requests.lock().unwrap().len(), before);
    }
}
#[tokio::test]
async fn europepmc_selection_requires_resolved_opt_in_despite_factory_availability() {
    for enabled in [false, true] {
        let c = config(
            if enabled {
                "engines:\n  europepmc:\n    enabled: true\n"
            } else {
                ""
            },
            BTreeMap::new(),
        );
        let body = include_str!("../../../docs/rust-rewrite/a07/fixtures/europepmc-preprints.json");
        let (registry, replay) = wired(&c, &[body]);
        assert!(registry.is_available("europepmc"));
        let d = registry.descriptor("europepmc").unwrap();
        let (resolved, _, _, categories) = ssx_config::capability_configured(d, &c);
        assert_eq!(resolved, enabled);
        let catalog = CatalogSnapshot::new(vec![EngineCapability {
            name: "europepmc".into(),
            enabled: resolved,
            active: true,
            available: registry.is_available("europepmc"),
            categories,
            media_types: vec![],
            auth_ready: true,
            circuit_open: false,
            cost_class: "free".into(),
            health_status: "healthy".into(),
            health_stale: false,
            enforced_filters: BTreeMap::new(),
        }])
        .unwrap();
        let request = SearchRequest {
            query: "aspirin".into(),
            engines: Some(vec!["europepmc".into()]),
            ..Default::default()
        };
        let scope = ScopeResolver::default().resolve(&request, &catalog);
        assert_eq!(
            scope.selected_engines,
            if enabled { vec!["europepmc"] } else { vec![] }
        );
        for name in scope.selected_engines {
            assert_eq!(
                registry
                    .instantiate(&name)
                    .unwrap()
                    .search(&request)
                    .await
                    .status,
                EngineStatus::Ok
            );
        }
        assert_eq!(replay.requests.lock().unwrap().len(), usize::from(enabled));
    }
}
#[test]
fn actual_factory_registration_preserves_duplicate_and_unknown_refusals() {
    let c = config("", BTreeMap::new());
    let (mut registry, _) = wired(&c, &[]);
    for (name, expected) in [
        ("clinicaltrials", vec!["medical", "health", "science"]),
        (
            "europepmc",
            vec!["science", "reference", "medical", "health"],
        ),
        ("pubmed", vec!["science", "reference", "medical", "health"]),
        (
            "pubchem",
            vec!["science", "reference", "chemistry", "medical"],
        ),
        (
            "uniprot",
            vec!["science", "reference", "biology", "medical"],
        ),
    ] {
        assert!(registry.instantiate(name).is_some());
        assert_eq!(registry.descriptor(name).unwrap().categories, expected);
    }
    assert!(matches!(
        register_biomedical(
            &mut registry,
            Arc::new(AdapterHttp::default()),
            Arc::new(AllowAllAdmission),
            BiomedicalSettings::from_resolved(&c)
        ),
        Err(RegistryError::Duplicate(_))
    ));
    assert!(matches!(
        registry.register("unknown-a07", Arc::new(|| panic!("must never instantiate"))),
        Err(RegistryError::Unknown(_))
    ));
    assert!(registry.instantiate("unknown-a07").is_none());
}

#[tokio::test]
async fn actual_factory_admission_denial_preserves_diagnostic_without_dispatch() {
    struct Denied;
    impl ssx_application::SearchAdmission for Denied {
        fn admit<'a>(
            &'a self,
            _: &'static str,
        ) -> ssx_application::PortFuture<'a, ssx_application::AdmissionDecision> {
            Box::pin(async { ssx_application::AdmissionDecision::Denied })
        }
    }
    let c = config("", BTreeMap::new());
    let replay = Arc::new(Replay {
        bodies: Mutex::new(VecDeque::new()),
        requests: Mutex::new(Vec::new()),
    });
    let http = Arc::new(AdapterHttp::default());
    http.set_transport(replay.clone()).unwrap();
    let mut registry = Registry::builtins().unwrap();
    register_biomedical(
        &mut registry,
        http,
        Arc::new(Denied),
        BiomedicalSettings::from_resolved(&c),
    )
    .unwrap();
    for name in NAMES {
        let response = registry
            .instantiate(name)
            .unwrap()
            .search(&SearchRequest::default())
            .await;
        assert_eq!(response.status, EngineStatus::RateLimited);
        assert_eq!(response.error_message.as_deref(), Some("rate limited"));
        assert!(response.results.is_empty());
    }
    assert!(replay.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn registered_specialist_cards_and_categories_reach_actual_science_selection() {
    let c = config(
        "engines:\n  europepmc:\n    enabled: true\n",
        BTreeMap::new(),
    );
    let (registry, replay) = wired(&c, &[]);
    let cards = [
        (
            "clinicaltrials",
            "registered clinical trials",
            "the query seeks trials, interventions, or enrollment",
        ),
        (
            "europepmc",
            "biomedical publication and preprint metadata",
            "the query seeks biomedical literature including preprints beyond PubMed",
        ),
        (
            "pubmed",
            "biomedical literature",
            "the query seeks medical or life-science publications",
        ),
        (
            "pubchem",
            "chemical compounds and bioassays",
            "the query seeks a compound, structure, or chemical identifier",
        ),
        (
            "uniprot",
            "protein sequences and annotations",
            "the query seeks a protein, gene product, or accession",
        ),
    ];
    let mut capabilities = Vec::new();
    for (name, purpose, use_when) in cards {
        assert!(registry.instantiate(name).is_some());
        let descriptor = registry.descriptor(name).unwrap();
        assert_eq!(
            descriptor.routing.role,
            ssx_registry::RoutingRole::Specialist
        );
        assert_eq!(descriptor.routing.purpose.as_deref(), Some(purpose));
        assert_eq!(descriptor.routing.use_when.as_deref(), Some(use_when));
        let (enabled, _, _, categories) = ssx_config::capability_configured(descriptor, &c);
        assert!(enabled);
        assert!(categories.iter().any(|x| x == "science"));
        capabilities.push(EngineCapability {
            name: name.into(),
            enabled,
            active: true,
            available: registry.is_available(name),
            categories,
            media_types: vec![],
            auth_ready: true,
            circuit_open: false,
            cost_class: "free".into(),
            health_status: "healthy".into(),
            health_stale: false,
            enforced_filters: descriptor.enforced_filters.clone(),
        });
    }
    let catalog = CatalogSnapshot::new(capabilities).unwrap();
    let request = SearchRequest {
        query: "science".into(),
        categories: Some(vec!["science".into()]),
        ..Default::default()
    };
    let decision = ScopeResolver::default().resolve(&request, &catalog);
    assert_eq!(
        decision
            .selected_engines
            .iter()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        NAMES.into_iter().collect()
    );
    for name in decision.selected_engines {
        let out = registry
            .instantiate(&name)
            .unwrap()
            .search_with_context(
                &request,
                SearchContext {
                    deadline: Instant::now() - Duration::from_millis(1),
                },
            )
            .await;
        assert_eq!(out.status, EngineStatus::Timeout);
        assert!(out.results.is_empty());
    }
    assert!(replay.requests.lock().unwrap().is_empty());
}
