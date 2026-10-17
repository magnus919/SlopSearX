#[path = "../t06_support/fixture.rs"]
pub mod fixture;
use ssx_application::policy::SearchPolicy;
use ssx_domain::{EngineStatus, TenantId, research::ResearchCeilings};
use ssx_infra::{
    research_planning::AdaptiveResearch, research_runner::RunnerConfig,
    valkey::research::ResearchStore,
};
use ssx_mcp::research_planning::PlanningRegistry;
use std::{num::NonZeroU64, time::Duration};
pub async fn setup(url: &str) -> (PlanningRegistry, fixture::Fixture) {
    let fixture = fixture::Fixture::with_policy(
        url,
        SearchPolicy {
            grants: ["research", "science"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            ..Default::default()
        },
        vec![fixture::FakeEngine {
            name: "wikipedia".into(),
            calls: Default::default(),
            dispatch_receipt: None,
            count: 3,
            status: EngineStatus::Ok,
            delay: Duration::ZERO,
            results: None,
        }],
        &[],
    )
    .await;
    let planning = AdaptiveResearch::new(
        ResearchStore::new(fixture.store.clone(), TenantId::new("default").unwrap()),
        fixture.service.clone(),
        fixture.surface.state().snapshots.clone(),
        RunnerConfig {
            owner: "w03-wire".into(),
            lease_seconds: NonZeroU64::new(2).unwrap(),
            poll_interval: Duration::from_millis(20),
            workers: NonZeroU64::new(1).unwrap(),
            ceilings: ResearchCeilings {
                queries: 20,
                engines_per_query: 10,
                results: 500,
            },
        },
        500,
    )
    .unwrap();
    (
        PlanningRegistry::new(planning, fixture.surface.clone()),
        fixture,
    )
}
