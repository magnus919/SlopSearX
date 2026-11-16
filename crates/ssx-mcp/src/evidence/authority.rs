//! Trusted owner publication, not a reload service or a second policy gate.
use ssx_application::{policy::PolicyService, saved::SavedLimits, staged};
use ssx_infra::{
    search::CurrentPolicy,
    valkey::{
        Valkey,
        retrieval::lineage::{Authority, AuthoritySnapshot},
    },
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone)]
pub struct EvidenceAuthoritySnapshot {
    pub search: PolicyService,
    pub saved_limits: SavedLimits,
    pub staged_limits: staged::Limits,
    pub auth_configured: BTreeMap<String, bool>,
    /// Established-owner readiness; the actual bounded operation remains authoritative.
    pub valkey_connected: bool,
}
pub trait EvidenceAuthority: Send + Sync {
    /// One complete, secret-free, in-memory snapshot. No clock capture or I/O.
    fn snapshot(&self) -> EvidenceAuthoritySnapshot;
}
fn generation(policy: &PolicyService) -> String {
    let mut fixed = policy.clone();
    fixed.policy.grants.clear();
    fixed.policy.sensitive_allowed = false;
    let mut engines = fixed.catalog.engines().to_vec();
    for engine in &mut engines {
        engine.enabled = false;
    }
    fixed.catalog =
        ssx_application::scope::CatalogSnapshot::new(engines).expect("validated catalog");
    // Internal same-generation equality only, never wire/storage identity. This
    // includes every immutable catalog/resolver field; only the named live
    // grants/sensitive flag/enabled fields are deliberately normalized away.
    format!("{fixed:?}")
}
fn validate(s: &EvidenceAuthoritySnapshot) -> Result<(), &'static str> {
    let l = &s.saved_limits;
    if l.definitions == 0
        || l.engines == 0
        || l.rows == 0
        || !(1..=100).contains(&l.reports)
        || l.default_reports == 0
        || l.default_reports > l.reports
        || l.retention_seconds == 0
        || l.default_retention_seconds == 0
        || l.default_retention_seconds > l.retention_seconds
        || l.minimum_interval == 0
        || l.minimum_interval > l.maximum_interval
        || s.staged_limits.deadline_ms == 0
        || s.staged_limits.engine_calls == 0
        || s.staged_limits.granted != s.search.policy.grants.contains("staged_search")
        || s.auth_configured.len() != s.search.catalog.engines().len()
        || s.search
            .catalog
            .engines()
            .iter()
            .any(|e| !s.auth_configured.contains_key(&e.name))
    {
        return Err("invalid effective evidence configuration");
    }
    Ok(())
}
struct State {
    value: RwLock<EvidenceAuthoritySnapshot>,
    generation: String,
}
impl EvidenceAuthority for State {
    fn snapshot(&self) -> EvidenceAuthoritySnapshot {
        // Publication replaces a complete prevalidated value, never individual
        // fields. Poison recovery can therefore only observe a complete value.
        self.value.read().unwrap_or_else(|p| p.into_inner()).clone()
    }
}
/// Writer capability belongs to the native composition owner. The runtime
/// graph receives only authority(), never this owner or its backend lifecycle.
pub struct EvidenceOwner {
    state: Arc<State>,
    backend: Arc<Valkey>,
    admission_closed: AtomicBool,
}
impl EvidenceOwner {
    pub fn new(
        mut initial: EvidenceAuthoritySnapshot,
        backend: Arc<Valkey>,
    ) -> Result<Self, &'static str> {
        initial.valkey_connected = false;
        validate(&initial)?;
        let generation = generation(&initial.search);
        Ok(Self {
            state: Arc::new(State {
                value: RwLock::new(initial),
                generation,
            }),
            backend,
            admission_closed: AtomicBool::new(false),
        })
    }
    pub fn authority(&self) -> Arc<dyn EvidenceAuthority> {
        self.state.clone()
    }
    /// Only the native composition publisher may install trusted effective
    /// configuration. This is not exposed through the runtime/tool graph.
    #[allow(
        dead_code,
        reason = "crate-private native configuration publication is consumed by the later owner integration; tests exercise it now"
    )]
    pub(crate) fn publish(&self, next: EvidenceAuthoritySnapshot) -> Result<(), &'static str> {
        validate(&next)?;
        if generation(&next.search) != self.state.generation {
            return Err("new service generation required");
        }
        let mut value = self.state.value.write().unwrap_or_else(|p| p.into_inner());
        if next.valkey_connected != value.valkey_connected {
            return Err("readiness is owner controlled");
        }
        *value = next;
        Ok(())
    }
    /// One explicit, bounded startup probe on the SAME borrowed backend. No
    /// request-time probe, retry loop, driver, or terminal-close replacement.
    pub async fn establish_readiness(&self) -> Result<(), ssx_infra::valkey::Failure> {
        if self.admission_closed.load(Ordering::SeqCst) || self.backend.is_closed() {
            return Err(ssx_infra::valkey::Failure::Closed);
        }
        self.backend.ping().await?;
        let mut value = self.state.value.write().unwrap_or_else(|p| p.into_inner());
        if self.admission_closed.load(Ordering::SeqCst) || self.backend.is_closed() {
            return Err(ssx_infra::valkey::Failure::Closed);
        }
        value.valkey_connected = true;
        Ok(())
    }
    /// Native owner calls before cancelling/joining handlers and then closing
    /// its existing SearchService. Evidence never closes the borrowed backend.
    pub fn stop_admission(&self) {
        let mut value = self.state.value.write().unwrap_or_else(|p| p.into_inner());
        self.admission_closed.store(true, Ordering::SeqCst);
        value.valkey_connected = false;
    }
}
#[derive(Clone)]
pub struct SearchPolicyAdapter {
    pub authority: Arc<dyn EvidenceAuthority>,
}
impl CurrentPolicy for SearchPolicyAdapter {
    fn current(&self) -> PolicyService {
        self.authority.snapshot().search
    }
}
pub struct RecordAuthorityAdapter {
    pub authority: Arc<dyn EvidenceAuthority>,
}
impl Authority for RecordAuthorityAdapter {
    fn current(&self) -> AuthoritySnapshot {
        let s = self.authority.snapshot();
        AuthoritySnapshot {
            search: s.search,
            limits: s.saved_limits,
            auth_configured: s.auth_configured,
        }
    }
}
pub struct LineagePolicyAdapter {
    pub authority: Arc<dyn EvidenceAuthority>,
}
impl ssx_application::lineage::resolver::Policy for LineagePolicyAdapter {
    fn current(&self) -> PolicyService {
        self.authority.snapshot().search
    }
    fn current_with_limits(
        &self,
        _: &staged::Limits,
        _: usize,
    ) -> (PolicyService, staged::Limits, usize) {
        let s = self.authority.snapshot();
        (s.search, s.staged_limits, s.saved_limits.reports as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ssx_application::{
        policy::SearchPolicy,
        scope::{CatalogSnapshot, ScopeResolver},
    };
    use std::time::Duration;
    fn initial() -> EvidenceAuthoritySnapshot {
        EvidenceAuthoritySnapshot {
            search: PolicyService {
                policy: SearchPolicy::default(),
                catalog: CatalogSnapshot::new(vec![]).unwrap(),
                resolver: ScopeResolver::default(),
            },
            saved_limits: SavedLimits::default(),
            staged_limits: staged::Limits::default(),
            auth_configured: BTreeMap::new(),
            valkey_connected: false,
        }
    }
    fn owner() -> EvidenceOwner {
        EvidenceOwner::new(
            initial(),
            Arc::new(Valkey::new("redis://127.0.0.1:1/", Duration::from_millis(50)).unwrap()),
        )
        .unwrap()
    }
    #[test]
    fn one_publication_is_one_complete_policy_limit_pair() {
        let owner = Arc::new(owner());
        let writer = owner.clone();
        let task = std::thread::spawn(move || {
            for i in 0..1000 {
                let mut next = writer.state.snapshot();
                let enabled = i % 2 == 0;
                next.search.policy.sensitive_allowed = enabled;
                next.saved_limits.reports = if enabled { 2 } else { 1 };
                next.saved_limits.default_reports = 1;
                writer.publish(next).unwrap();
            }
        });
        for _ in 0..1000 {
            let s = owner.authority().snapshot();
            assert!(matches!(
                (s.search.policy.sensitive_allowed, s.saved_limits.reports),
                (false, 100) | (false, 1) | (true, 2)
            ));
        }
        task.join().unwrap();
    }
    #[test]
    fn invalid_generation_or_limit_publication_is_atomic_and_keeps_old_state() {
        let owner = owner();
        let before = owner.state.snapshot();
        let mut invalid = before.clone();
        invalid.saved_limits.reports = 0;
        assert!(owner.publish(invalid).is_err());
        let mut changed = before.clone();
        changed
            .search
            .resolver
            .tier1
            .insert("new-generation".into());
        assert!(owner.publish(changed).is_err());
        let mut forged_readiness = before.clone();
        forged_readiness.valkey_connected = true;
        assert!(owner.publish(forged_readiness).is_err());
        let after = owner.state.snapshot();
        assert_eq!(after.saved_limits, before.saved_limits);
        assert_eq!(generation(&after.search), generation(&before.search));
        assert!(!after.valkey_connected);
    }
    #[test]
    fn record_adapter_samples_once_and_search_lineage_use_same_authority() {
        use ssx_application::lineage::resolver::Policy;
        let owner = owner();
        let authority = owner.authority();
        let search = SearchPolicyAdapter {
            authority: authority.clone(),
        };
        let records = RecordAuthorityAdapter {
            authority: authority.clone(),
        };
        let lineage = LineagePolicyAdapter { authority };
        let mut next = owner.state.snapshot();
        next.search
            .policy
            .grants
            .insert("retrieval_receipts".into());
        next.saved_limits.reports = 1;
        next.saved_limits.default_reports = 1;
        owner.publish(next).unwrap();
        let record = records.current();
        assert_eq!(record.limits.reports, 1);
        assert!(record.search.policy.grants.contains("retrieval_receipts"));
        assert_eq!(
            search.current().policy.grants,
            lineage.current().policy.grants
        );
    }
    #[tokio::test]
    async fn configured_url_is_not_readiness_and_terminal_close_is_observed() {
        use ssx_application::receipts::service::Access;
        let owner = owner();
        let access = crate::evidence::EvidenceAccess {
            authority: owner.authority(),
            backend: owner.backend.clone(),
        };
        assert!(!access.connected());
        // Simulate the owner having already established readiness, without a
        // network claim in this terminal-observation control.
        owner.state.value.write().unwrap().valkey_connected = true;
        assert!(access.connected());
        owner.backend.close().await;
        assert!(!access.connected());
        assert!(matches!(
            owner.establish_readiness().await,
            Err(ssx_infra::valkey::Failure::Closed)
        ));
        owner.stop_admission();
        assert!(!owner.authority().snapshot().valkey_connected);
    }
    /// Publication occurs in the future returned by the authoritative record
    /// read, after entry authorization and before post-read revalidation.
    struct PublishingRecords {
        owner: Arc<EvidenceOwner>,
        next: EvidenceAuthoritySnapshot,
        record: ssx_domain::staged::Record,
        report_limit: std::sync::atomic::AtomicUsize,
        saved_reads: std::sync::atomic::AtomicUsize,
        selected_reports: std::sync::atomic::AtomicUsize,
        research_reads: std::sync::atomic::AtomicUsize,
        publish_on_research_read: usize,
        publish_on_saved_read: usize,
    }
    impl ssx_application::lineage::workflows::Records for PublishingRecords {
        fn research<'a>(
            &'a self,
            tenant: &'a ssx_domain::TenantId,
            id: &'a str,
            _: &'a ssx_application::receipts::Cancellation,
        ) -> ssx_application::PortFuture<
            'a,
            Result<
                ssx_application::lineage::workflows::Read<ssx_domain::research::ResearchJob>,
                ssx_application::lineage::Failure,
            >,
        > {
            Box::pin(async move {
                let read = self
                    .research_reads
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                    + 1;
                if read == self.publish_on_research_read {
                    self.owner.publish(self.next.clone()).unwrap();
                }
                Ok(ssx_application::lineage::workflows::Read::Found(
                    ssx_domain::research::ResearchJob {
                        tenant: tenant.as_str().into(),
                        job_id: id.into(),
                        queries: vec![ssx_domain::research::ResearchQuery {
                            engines: vec!["fixture".into()],
                            attempts: vec![ssx_domain::research::ResearchQueryAttempt {
                                attempt_id: "attempt-a".into(),
                                ..Default::default()
                            }],
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                ))
            })
        }
        fn staged<'a>(
            &'a self,
            _: &'a ssx_domain::TenantId,
            _: &'a str,
            _: ssx_domain::WallClockTimestamp,
            _: &'a ssx_application::receipts::Cancellation,
        ) -> ssx_application::PortFuture<
            'a,
            Result<
                ssx_application::lineage::workflows::Read<ssx_domain::staged::Record>,
                ssx_application::lineage::Failure,
            >,
        > {
            Box::pin(async move {
                self.owner.publish(self.next.clone()).unwrap();
                Ok(ssx_application::lineage::workflows::Read::Found(
                    self.record.clone(),
                ))
            })
        }
        fn saved<'a>(
            &'a self,
            tenant: &'a ssx_domain::TenantId,
            id: &'a str,
            _: &'a ssx_application::receipts::Cancellation,
        ) -> ssx_application::PortFuture<
            'a,
            Result<
                ssx_application::lineage::workflows::Read<ssx_domain::saved::SavedDefinition>,
                ssx_application::lineage::Failure,
            >,
        > {
            Box::pin(async move {
                let read = self
                    .saved_reads
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                    + 1;
                if read == self.publish_on_saved_read {
                    self.owner.publish(self.next.clone()).unwrap();
                }
                let corpus: ssx_json::Value = ssx_json::from_str(include_str!(
                    "../../../../docs/rust-rewrite/w00/source.json"
                ))
                .unwrap();
                let mut definition = ssx_domain::saved::SavedDefinition::from_payload(
                    &corpus["observations"][0]["input"],
                )
                .unwrap();
                definition.tenant = tenant.as_str().into();
                definition.search_id = id.into();
                definition.engines.clear();
                Ok(ssx_application::lineage::workflows::Read::Found(definition))
            })
        }
        fn reports<'a>(
            &'a self,
            _: &'a ssx_domain::TenantId,
            _: &'a ssx_domain::saved::SavedDefinition,
            _: ssx_domain::WallClockTimestamp,
            limit: usize,
            selected_run: Option<&'a str>,
            _: &'a ssx_application::receipts::Cancellation,
        ) -> ssx_application::PortFuture<
            'a,
            Result<
                ssx_application::lineage::workflows::Read<Vec<ssx_json::Value>>,
                ssx_application::lineage::Failure,
            >,
        > {
            Box::pin(async move {
                if selected_run.is_some() {
                    self.selected_reports
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
                self.report_limit
                    .store(limit, std::sync::atomic::Ordering::SeqCst);
                Ok(ssx_application::lineage::workflows::Read::Found(vec![]))
            })
        }
    }
    struct LegacyPolicy(Arc<dyn EvidenceAuthority>);
    impl ssx_application::lineage::resolver::Policy for LegacyPolicy {
        fn current(&self) -> PolicyService {
            self.0.snapshot().search
        }
    }
    struct FixedClock;
    impl ssx_application::receipts::service::Clock for FixedClock {
        fn now(&self) -> ssx_domain::WallClockTimestamp {
            ssx_domain::WallClockTimestamp::new(100.).unwrap()
        }
    }
    fn staged_owner_and_record() -> (Arc<EvidenceOwner>, ssx_domain::staged::Record) {
        use ssx_json::json;
        let mut s = initial();
        s.search
            .policy
            .grants
            .extend(["staged_search".into(), "saved_searches".into()]);
        s.staged_limits.granted = true;
        s.search.catalog = CatalogSnapshot::new(vec![ssx_application::scope::EngineCapability {
            name: "fixture".into(),
            enabled: true,
            active: true,
            available: true,
            categories: vec!["general".into()],
            media_types: vec![],
            auth_ready: true,
            circuit_open: false,
            cost_class: "free".into(),
            health_status: "healthy".into(),
            health_stale: false,
            enforced_filters: BTreeMap::new(),
        }])
        .unwrap();
        s.auth_configured.insert("fixture".into(), true);
        let record = staged::Plan::build(
            &s.search,
            &s.staged_limits,
            &staged::Input {
                query: "fixture".into(),
                objectives: json!({"deadline_ms":1000,"max_engine_calls":1}),
                initial_scope: json!({"engines":["fixture"]}),
                fallback_scope: None,
                allow_scope_expansion: false,
                filters: None,
            },
        )
        .unwrap()
        .record(
            &ssx_domain::TenantId::new("tenant-a").unwrap(),
            &ssx_domain::StagedSearchId::new("staged-a").unwrap(),
            100.,
        )
        .unwrap();
        let owner = Arc::new(
            EvidenceOwner::new(
                s,
                Arc::new(Valkey::new("redis://127.0.0.1:1/", Duration::from_millis(50)).unwrap()),
            )
            .unwrap(),
        );
        (owner, record)
    }
    #[tokio::test]
    async fn publication_during_read_refreshes_staged_pair_and_saved_report_bound() {
        use ssx_application::lineage::{Resolver, Status, workflows::WorkflowResolver};
        use ssx_application::receipts::Cancellation;
        use ssx_domain::{ArtifactKind, ArtifactRef, TenantId};
        for revoke in [false, true] {
            let (owner, record) = staged_owner_and_record();
            let mut next = owner.state.snapshot();
            if revoke {
                next.search.policy.grants.remove("staged_search");
                next.staged_limits.granted = false;
            } else {
                next.staged_limits.deadline_ms = 1;
            }
            let records = Arc::new(PublishingRecords {
                owner: owner.clone(),
                next,
                record,
                report_limit: std::sync::atomic::AtomicUsize::new(0),
                saved_reads: std::sync::atomic::AtomicUsize::new(0),
                selected_reports: std::sync::atomic::AtomicUsize::new(0),
                research_reads: std::sync::atomic::AtomicUsize::new(0),
                publish_on_research_read: 0,
                publish_on_saved_read: 1,
            });
            let resolver = WorkflowResolver {
                records,
                policy: Arc::new(LineagePolicyAdapter {
                    authority: owner.authority(),
                }),
                clock: Arc::new(FixedClock),
                staged_limits: staged::Limits::default(),
                saved_max_reports: 100,
            };
            let result = resolver
                .resolve(
                    &TenantId::new("tenant-a").unwrap(),
                    &ArtifactRef::new(ArtifactKind::StagedSearch, "staged-a").unwrap(),
                    &Cancellation::default(),
                )
                .await
                .unwrap();
            assert_eq!(result.node.status, Status::PolicyDenied);
            assert!(result.edges.is_empty());
        }
        for revoke in [false, true] {
            let (owner, record) = staged_owner_and_record();
            let mut next = owner.state.snapshot();
            next.saved_limits.reports = 1;
            next.saved_limits.default_reports = 1;
            if revoke {
                next.search.policy.grants.remove("saved_searches");
            }
            let records = Arc::new(PublishingRecords {
                owner: owner.clone(),
                next,
                record,
                report_limit: std::sync::atomic::AtomicUsize::new(0),
                saved_reads: std::sync::atomic::AtomicUsize::new(0),
                selected_reports: std::sync::atomic::AtomicUsize::new(0),
                research_reads: std::sync::atomic::AtomicUsize::new(0),
                publish_on_research_read: 0,
                publish_on_saved_read: 1,
            });
            let resolver = WorkflowResolver {
                records: records.clone(),
                policy: Arc::new(LineagePolicyAdapter {
                    authority: owner.authority(),
                }),
                clock: Arc::new(FixedClock),
                staged_limits: staged::Limits::default(),
                saved_max_reports: 100,
            };
            let result = resolver
                .resolve(
                    &TenantId::new("tenant-a").unwrap(),
                    &ArtifactRef::new(ArtifactKind::SavedSearch, "saved-a").unwrap(),
                    &Cancellation::default(),
                )
                .await
                .unwrap();
            assert_eq!(
                result.node.status,
                if revoke {
                    Status::PolicyDenied
                } else {
                    Status::Live
                }
            );
            assert_eq!(
                records
                    .report_limit
                    .load(std::sync::atomic::Ordering::SeqCst),
                usize::from(!revoke)
            );
        }
    }
    #[tokio::test]
    async fn saved_report_second_definition_read_refreshes_grant_and_report_bound() {
        use ssx_application::lineage::{Resolver, Status, workflows::WorkflowResolver};
        use ssx_application::receipts::Cancellation;
        use ssx_domain::{ArtifactKind, ArtifactRef, TenantId, canonical::composite_artifact_id};
        for revoke in [false, true] {
            let (owner, record) = staged_owner_and_record();
            let mut next = owner.state.snapshot();
            next.saved_limits.reports = 1;
            next.saved_limits.default_reports = 1;
            if revoke {
                next.search.policy.grants.remove("saved_searches");
            }
            let records = Arc::new(PublishingRecords {
                owner: owner.clone(),
                next,
                record,
                report_limit: std::sync::atomic::AtomicUsize::new(0),
                saved_reads: std::sync::atomic::AtomicUsize::new(0),
                selected_reports: std::sync::atomic::AtomicUsize::new(0),
                research_reads: std::sync::atomic::AtomicUsize::new(0),
                publish_on_research_read: 0,
                publish_on_saved_read: 2,
            });
            let resolver = WorkflowResolver {
                records: records.clone(),
                policy: Arc::new(LineagePolicyAdapter {
                    authority: owner.authority(),
                }),
                clock: Arc::new(FixedClock),
                staged_limits: staged::Limits::default(),
                saved_max_reports: 100,
            };
            let id = composite_artifact_id(&["saved-a", "run-a"]).unwrap();
            let result = resolver
                .resolve(
                    &TenantId::new("tenant-a").unwrap(),
                    &ArtifactRef::new(ArtifactKind::SavedReport, id).unwrap(),
                    &Cancellation::default(),
                )
                .await
                .unwrap();
            assert_eq!(
                records
                    .saved_reads
                    .load(std::sync::atomic::Ordering::SeqCst),
                2
            );
            assert_eq!(
                result.node.status,
                if revoke {
                    Status::PolicyDenied
                } else {
                    Status::Missing
                }
            );
            assert_eq!(
                records
                    .selected_reports
                    .load(std::sync::atomic::Ordering::SeqCst),
                usize::from(!revoke)
            );
            // Revocation prevents the selected report read; the parent read
            // already observed 100. Otherwise the selected read observes 1.
            assert_eq!(
                records
                    .report_limit
                    .load(std::sync::atomic::Ordering::SeqCst),
                if revoke { 100 } else { 1 }
            );
        }
    }
    #[tokio::test]
    async fn default_policy_port_still_rechecks_staged_revocation_after_read() {
        use ssx_application::lineage::{Resolver, Status, workflows::WorkflowResolver};
        use ssx_application::receipts::Cancellation;
        use ssx_domain::{ArtifactKind, ArtifactRef, TenantId};
        let (owner, record) = staged_owner_and_record();
        let mut next = owner.state.snapshot();
        next.search.policy.grants.remove("staged_search");
        next.staged_limits.granted = false;
        let records = Arc::new(PublishingRecords {
            owner: owner.clone(),
            next,
            record,
            report_limit: std::sync::atomic::AtomicUsize::new(0),
            saved_reads: std::sync::atomic::AtomicUsize::new(0),
            selected_reports: std::sync::atomic::AtomicUsize::new(0),
            research_reads: std::sync::atomic::AtomicUsize::new(0),
            publish_on_research_read: 0,
            publish_on_saved_read: 1,
        });
        let resolver = WorkflowResolver {
            records,
            policy: Arc::new(LegacyPolicy(owner.authority())),
            clock: Arc::new(FixedClock),
            staged_limits: staged::Limits::default(),
            saved_max_reports: 100,
        };
        let result = resolver
            .resolve(
                &TenantId::new("tenant-a").unwrap(),
                &ArtifactRef::new(ArtifactKind::StagedSearch, "staged-a").unwrap(),
                &Cancellation::default(),
            )
            .await
            .unwrap();
        assert_eq!(result.node.status, Status::PolicyDenied);
        assert!(result.edges.is_empty());
    }
    #[tokio::test]
    async fn research_publication_during_each_read_revokes_grant_engine_and_sensitive_access() {
        use ssx_application::lineage::{Resolver, Status, workflows::WorkflowResolver};
        use ssx_application::receipts::{
            Cancellation, manifest::Sources, service::ManifestSources,
        };
        use ssx_domain::{ArtifactKind, ArtifactRef, TenantId, canonical::composite_artifact_id};
        for boundary in [1, 2] {
            for mutation in ["grant", "engine", "sensitive"] {
                let (_, record) = staged_owner_and_record();
                let mut initial = staged_owner_and_record().0.state.snapshot();
                initial.search.policy.grants.insert("research".into());
                initial
                    .search
                    .policy
                    .sensitive_engines
                    .insert("fixture".into());
                initial.search.policy.sensitive_allowed = true;
                let owner = Arc::new(
                    EvidenceOwner::new(
                        initial,
                        Arc::new(
                            Valkey::new("redis://127.0.0.1:1/", Duration::from_millis(50)).unwrap(),
                        ),
                    )
                    .unwrap(),
                );
                let mut next = owner.state.snapshot();
                match mutation {
                    "grant" => {
                        next.search.policy.grants.remove("research");
                    }
                    "engine" => {
                        let mut engines = next.search.catalog.engines().to_vec();
                        engines[0].enabled = false;
                        next.search.catalog = CatalogSnapshot::new(engines).unwrap();
                    }
                    _ => next.search.policy.sensitive_allowed = false,
                }
                let records = Arc::new(PublishingRecords {
                    owner: owner.clone(),
                    next,
                    record,
                    report_limit: std::sync::atomic::AtomicUsize::new(0),
                    saved_reads: std::sync::atomic::AtomicUsize::new(0),
                    selected_reports: std::sync::atomic::AtomicUsize::new(0),
                    research_reads: std::sync::atomic::AtomicUsize::new(0),
                    publish_on_research_read: boundary,
                    publish_on_saved_read: 0,
                });
                let policy = Arc::new(LineagePolicyAdapter {
                    authority: owner.authority(),
                });
                let resolver = WorkflowResolver {
                    records: records.clone(),
                    policy: policy.clone(),
                    clock: Arc::new(FixedClock),
                    staged_limits: staged::Limits::default(),
                    saved_max_reports: 100,
                };
                let root = if boundary == 1 {
                    ArtifactRef::new(ArtifactKind::ResearchJob, "job-a").unwrap()
                } else {
                    ArtifactRef::new(
                        ArtifactKind::ResearchAttempt,
                        composite_artifact_id(&["job-a", "attempt-a"]).unwrap(),
                    )
                    .unwrap()
                };
                let result = resolver
                    .resolve(
                        &TenantId::new("tenant-a").unwrap(),
                        &root,
                        &Cancellation::default(),
                    )
                    .await
                    .unwrap();
                assert_eq!(
                    result.node.status,
                    Status::PolicyDenied,
                    "{boundary}/{mutation}"
                );
                assert!(result.edges.is_empty());
                assert_eq!(
                    records
                        .research_reads
                        .load(std::sync::atomic::Ordering::SeqCst),
                    boundary
                );
                assert_eq!(
                    records
                        .saved_reads
                        .load(std::sync::atomic::Ordering::SeqCst),
                    0
                );
                assert_eq!(
                    records
                        .report_limit
                        .load(std::sync::atomic::Ordering::SeqCst),
                    0
                );
                // Re-enable the same complete original publication, then test
                // manifest source resolution's own post-read denial boundary.
                if boundary == 1 {
                    let mut initial = owner.state.snapshot();
                    initial.search.policy.grants.insert("research".into());
                    initial.search.policy.sensitive_allowed = true;
                    let mut engines = initial.search.catalog.engines().to_vec();
                    engines[0].enabled = true;
                    initial.search.catalog = CatalogSnapshot::new(engines).unwrap();
                    owner.publish(initial).unwrap();
                    records
                        .research_reads
                        .store(0, std::sync::atomic::Ordering::SeqCst);
                    let snapshots = ssx_application::snapshot::SnapshotService::new(
                        Arc::new(ssx_infra::valkey::snapshot::ValkeySnapshots::new(
                            owner.backend.clone(),
                        )),
                        Arc::new(ssx_infra::valkey::snapshot::SystemSnapshotClock),
                        Arc::new(ssx_infra::valkey::snapshot::RandomSnapshotIds),
                        TenantId::new("tenant-a").unwrap(),
                        ssx_application::snapshot::SnapshotTtl::new(60).unwrap(),
                    );
                    let sources = Sources {
                        records: records.clone(),
                        snapshots,
                        policy,
                        clock: Arc::new(FixedClock),
                        staged_limits: staged::Limits::default(),
                    };
                    let result = sources
                        .resolve(
                            &TenantId::new("tenant-a").unwrap(),
                            &root,
                            &Cancellation::default(),
                        )
                        .await
                        .unwrap_err();
                    assert_eq!(
                        result["error"]["code"], "source_policy_denied",
                        "manifest/{mutation}"
                    );
                    assert_eq!(
                        records
                            .research_reads
                            .load(std::sync::atomic::Ordering::SeqCst),
                        1
                    );
                }
            }
        }
    }
}
