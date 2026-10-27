//! Native durable dossier service. Search execution and discovery use W02's
//! existing store/runner; report/preview methods have no provider-call path.
use crate::{
    research_runner::{ResearchRunner, SourceValidator},
    valkey::research::{JobRead, ResearchStore},
};
use ssx_application::{
    PortFuture,
    composition::{Destination, Error, SourceResolver},
    dependency_dossier::{self as dossier, Identity, Limits, Section},
    receipts::{Cancellation, service::Clock},
    snapshot::SnapshotLookup,
};
use ssx_domain::{
    ArtifactRef, JobId, SearchResult, TenantId, WallClockTimestamp, research::ResearchJob,
};
use ssx_json::{Value, json};
use std::sync::Arc;
pub trait JobIds: Send + Sync {
    fn next(&self) -> Result<JobId, Error>;
}
pub struct RandomJobIds;
impl JobIds for RandomJobIds {
    fn next(&self) -> Result<JobId, Error> {
        let mut bytes = [0u8; 32];
        aws_lc_rs::rand::fill(&mut bytes)
            .map_err(|_| Error::new("store_unavailable", "job identity entropy unavailable"))?;
        JobId::new(bytes.iter().map(|b| format!("{b:02x}")).collect::<String>())
            .map_err(|_| Error::new("invalid_input", "invalid job identity"))
    }
}
pub struct Input<'a> {
    pub ecosystem: &'a str,
    pub package: &'a str,
    pub version: Option<&'a str>,
    pub repository: Option<&'a str>,
    pub requested_deadline: Option<&'a str>,
    pub deadline: WallClockTimestamp,
    pub idempotency_key: Option<&'a str>,
    pub source: Option<&'a ArtifactRef>,
}
/// Dossier-specific admission/read seam. The native implementation delegates
/// exclusively to W01's existing atomic create/load authority.
pub trait DossierJobs: Send + Sync {
    fn create<'a>(
        &'a self,
        tenant: &'a TenantId,
        candidate: &'a ResearchJob,
    ) -> PortFuture<'a, Result<(ResearchJob, bool), Error>>;
    fn load<'a>(
        &'a self,
        tenant: &'a TenantId,
        id: &'a JobId,
    ) -> PortFuture<'a, Result<Option<ResearchJob>, Error>>;
}
impl DossierJobs for ResearchStore {
    fn create<'a>(
        &'a self,
        tenant: &'a TenantId,
        candidate: &'a ResearchJob,
    ) -> PortFuture<'a, Result<(ResearchJob, bool), Error>> {
        Box::pin(async move {
            self.for_tenant(tenant.clone())
                .create(candidate)
                .await
                .map_err(|e| {
                    Error::new(
                        "store_unavailable",
                        format!("dependency dossier admission could not be persisted: {e:?}"),
                    )
                })
        })
    }
    fn load<'a>(
        &'a self,
        tenant: &'a TenantId,
        id: &'a JobId,
    ) -> PortFuture<'a, Result<Option<ResearchJob>, Error>> {
        Box::pin(async move {
            match self.for_tenant(tenant.clone()).load(id).await {
                JobRead::Found(job) => Ok(Some(*job)),
                JobRead::Missing => Ok(None),
                JobRead::Malformed => Err(Error::new(
                    "source_malformed",
                    "dependency dossier record is malformed",
                )),
                JobRead::Unavailable | JobRead::Closed => Err(Error::new(
                    "store_unavailable",
                    "dependency dossier store is unavailable",
                )),
            }
        })
    }
}
pub struct DossierService {
    pub jobs: Arc<dyn DossierJobs>,
    pub sources: Arc<SourceResolver>,
    pub clock: Arc<dyn Clock>,
    pub ids: Arc<dyn JobIds>,
    pub limits: Limits,
}
impl DossierService {
    fn check(
        &self,
        tenant: &TenantId,
        job: Option<&ResearchJob>,
        cancel: &Cancellation,
    ) -> Result<(), Error> {
        if cancel.is_cancelled() {
            return Err(Error::new("cancelled", "dossier operation cancelled"));
        }
        let current = self.sources.authority.current();
        if current.tenant.as_ref() != Some(tenant) {
            return Err(Error::new(
                "source_policy_denied",
                "current principal no longer authorizes this tenant",
            ));
        }
        if ["dependency_dossier", "research", "security"]
            .iter()
            .any(|g| !current.policy.policy.grants.contains(*g))
        {
            return Err(Error::new(
                "tool_disabled",
                "dependency dossier requires dependency_dossier, research and security grants",
            ));
        }
        if let Some(job) = job {
            if job.tenant != tenant.as_str() {
                return Err(Error::new("source_malformed", "dossier tenant mismatch"));
            }
            current
                .policy
                .engine_rejection(
                    &job.queries
                        .iter()
                        .flat_map(|q| q.engines.clone())
                        .collect::<Vec<_>>(),
                    "engines",
                )
                .map_err(|e| Error::new("policy_rejected", e.message))?;
            current
                .policy
                .validate_research_workflow(job)
                .map_err(|e| Error::new("policy_rejected", e.message))?;
        }
        Ok(())
    }
    pub fn runner(&self, runner: ResearchRunner) -> ResearchRunner {
        runner.with_source_validator(Arc::new(NativeSourceValidator {
            sources: self.sources.clone(),
        }))
    }
    pub async fn start(
        &self,
        tenant: &TenantId,
        input: Input<'_>,
        cancel: &Cancellation,
    ) -> Result<Value, Error> {
        self.check(tenant, None, cancel)?;
        let identity = Identity::new(
            input.ecosystem,
            input.package,
            input.version,
            input.repository,
            input.requested_deadline,
        )?;
        let source = if let Some(source) = input.source {
            Some(
                self.sources
                    .resolve(tenant, source, Destination::DependencyDossier, cancel)
                    .await?,
            )
        } else {
            None
        };
        self.check(tenant, None, cancel)?;
        let candidate = dossier::plan(
            identity,
            tenant,
            self.ids.next()?.as_str(),
            self.clock.now(),
            input.deadline,
            input.idempotency_key,
            source.as_ref(),
            &self.limits,
        )?;
        self.check(tenant, Some(&candidate), cancel)?;
        // The worker revalidates composed sources before each new reservation;
        // expiry or revocation during this awaited create cannot authorize work.
        let admitted_result = self.jobs.create(tenant, &candidate).await;
        self.check(tenant, None, cancel)?;
        let (admitted, created) = admitted_result?;
        self.check(tenant, Some(&admitted), cancel)?;
        if !dossier::same_request(&admitted, &candidate) {
            return Err(Error::new(
                "idempotency_conflict",
                "idempotency key refers to a different workflow request or composition source",
            ));
        }
        if let Some(source) = input.source {
            self.sources
                .resolve(tenant, source, Destination::DependencyDossier, cancel)
                .await?;
            self.check(tenant, Some(&admitted), cancel)?;
        }
        Ok(dossier::start_envelope(&admitted, !created))
    }
    /// Supply the accepted native public card projection at the final transport
    /// boundary. Reads/attribution remain complete regardless of its card limit.
    pub async fn get(
        &self,
        tenant: &TenantId,
        id: &JobId,
        max_results: Option<usize>,
        card: &(dyn Fn(&SearchResult, &str) -> Value + Send + Sync),
        cancel: &Cancellation,
    ) -> Result<Value, Error> {
        self.check(tenant, None, cancel)?;
        self.limits.validate()?;
        if max_results == Some(0) {
            return Err(Error::new(
                "invalid_input",
                "max_results must be a positive integer",
            ));
        }
        let read = self.jobs.load(tenant, id).await;
        self.check(tenant, None, cancel)?;
        let job = read?
            .filter(|job| job.workflow.get("kind") == Some(&json!("dependency_dossier")))
            .ok_or_else(|| Error::new("invalid_job_id", "unknown dependency dossier"))?;
        self.check(tenant, Some(&job), cancel)?;
        let indexes = job
            .workflow
            .get("section_queries")
            .ok_or_else(|| Error::new("source_malformed", "dossier section indexes missing"))?;
        let mut sections = vec![];
        let mut read_engines = vec![];
        for name in [
            "package_information",
            "repository_records",
            "advisory_leads",
        ] {
            let index = if indexes[name].is_null() {
                None
            } else {
                Some(
                    indexes[name]
                        .as_u64()
                        .and_then(|n| usize::try_from(n).ok())
                        .filter(|i| *i < job.queries.len())
                        .ok_or_else(|| {
                            Error::new("source_malformed", "invalid dossier section index")
                        })?,
                )
            };
            let query = index.map(|i| &job.queries[i]);
            let mut section = dossier::section_without_snapshot(query);
            if let (Some(index), Some(query)) = (index, query)
                && !matches!(query.state.as_str(), "pending" | "running" | "failed")
                && let Some(cursor) = query.cursor.as_deref()
            {
                let id = ssx_domain::SnapshotId::new(cursor)
                    .map_err(|_| Error::new("source_malformed", "invalid dossier snapshot"))?;
                let read = self
                    .sources
                    .snapshots
                    .for_tenant(tenant.clone())
                    .read(&id)
                    .await;
                self.check(tenant, Some(&job), cancel)?;
                match read {
                    SnapshotLookup::Found(snapshot) => {
                        read_engines.extend(snapshot.scope().selected_engines.clone());
                        self.sources
                            .authority
                            .current()
                            .policy
                            .retained_engine_rejection(&read_engines, "engines")
                            .map_err(|_| {
                                Error::new(
                                    "policy_rejected",
                                    "current policy denies dossier snapshot evidence",
                                )
                            })?;
                        section = dossier::section_from_snapshot(
                            &job,
                            index,
                            &snapshot,
                            max_results
                                .unwrap_or(self.limits.max_results)
                                .min(self.limits.max_results),
                            if name == "repository_records" {
                                job.workflow["identity"]["repository"].as_str()
                            } else {
                                None
                            },
                            card,
                        )?;
                    }
                    SnapshotLookup::Expired { .. } => {
                        section.value["state"] = json!("expired");
                        section.value["error"] = json!("expired_handle");
                    }
                    SnapshotLookup::Unavailable => {
                        section.value["state"] = json!("unavailable");
                        section.value["error"] = json!("unavailable");
                    }
                    SnapshotLookup::Missing => {
                        section.value["state"] = json!("failed");
                        section.value["error"] = json!("not_executed");
                    }
                }
            }
            sections.push(section);
        }
        self.check(tenant, Some(&job), cancel)?;
        self.sources
            .authority
            .current()
            .policy
            .retained_engine_rejection(&read_engines, "engines")
            .map_err(|_| {
                Error::new(
                    "policy_rejected",
                    "current policy denies dossier snapshot evidence",
                )
            })?;
        let sections: [Section; 3] = sections
            .try_into()
            .map_err(|_| Error::new("source_malformed", "invalid dossier section count"))?;
        dossier::report(&job, sections)
    }
}
pub struct NativeSourceValidator {
    pub sources: Arc<SourceResolver>,
}
impl NativeSourceValidator {
    fn check_current(&self, tenant: &TenantId, job: &ResearchJob) -> Result<(), Error> {
        let current = self.sources.authority.current();
        let dossier = job.workflow.get("kind") == Some(&json!("dependency_dossier"));
        let required = if dossier {
            &["research", "dependency_dossier", "security"][..]
        } else {
            &["research"][..]
        };
        if current.tenant.as_ref() != Some(tenant)
            || job.tenant != tenant.as_str()
            || required
                .iter()
                .any(|g| !current.policy.policy.grants.contains(*g))
        {
            return Err(Error::new(
                "source_policy_denied",
                "current destination authority denies composed work",
            ));
        }
        let engines = job
            .queries
            .iter()
            .flat_map(|q| q.engines.clone())
            .collect::<Vec<_>>();
        current
            .policy
            .engine_rejection(&engines, "engines")
            .map_err(|_| {
                Error::new(
                    "source_policy_denied",
                    "current destination policy denies the complete workflow plan",
                )
            })?;
        current
            .policy
            .validate_research_workflow(job)
            .map_err(|_| {
                Error::new(
                    "source_policy_denied",
                    "current destination grants deny the workflow",
                )
            })?;
        for query in &job.queries {
            current
                .policy
                .authorize(ssx_application::policy::ScopeProposal::from_research_query(
                    query,
                ))
                .map_err(|_| {
                    Error::new(
                        "source_policy_denied",
                        "current destination policy denies a workflow query",
                    )
                })?;
        }
        Ok(())
    }
}
impl SourceValidator for NativeSourceValidator {
    fn validate<'a>(
        &'a self,
        tenant: &'a TenantId,
        job: &'a ResearchJob,
    ) -> PortFuture<'a, Result<(), Error>> {
        Box::pin(async move {
            self.check_current(tenant, job)?;
            let composition = job
                .workflow
                .get("composition")
                .ok_or_else(|| Error::new("source_malformed", "composition descriptor missing"))?;
            let stored = &composition["source"];
            let artifact = ArtifactRef::from_value(&stored["artifact"]).map_err(|_| {
                Error::new(
                    "source_malformed",
                    "composition artifact reference is malformed",
                )
            })?;
            let destination = if job.workflow.get("kind") == Some(&json!("dependency_dossier")) {
                Destination::DependencyDossier
            } else {
                Destination::Research
            };
            let resolved = self
                .sources
                .resolve(tenant, &artifact, destination, &Cancellation::default())
                .await?;
            // Immutable reference binding and selected result identity must agree;
            // retained query/detail strings cannot inject dispatch authority.
            if stored != &resolved.stored() {
                return Err(Error::new(
                    "source_malformed",
                    "composition source binding differs from retained evidence",
                ));
            }
            self.check_current(tenant, job)?;
            Ok(())
        })
    }
}
