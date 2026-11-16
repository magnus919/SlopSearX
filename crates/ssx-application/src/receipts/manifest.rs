//! W07-only readonly research_manifest source transition. This does not admit a
//! new workflow; W09 owns the remaining cross-workflow composition transitions.
use super::{
    Cancellation,
    service::{ManifestSources, ResolvedManifestSource, error},
};
use crate::{
    PortFuture,
    lineage::{
        input,
        resolver::Policy,
        workflows::{Read, Records},
    },
    snapshot::{SnapshotProjectionLookup, SnapshotService},
    staged,
};
use ssx_domain::{ArtifactKind, ArtifactRef, SnapshotId, TenantId};
use ssx_json::{Value, json};
use std::{collections::BTreeSet, sync::Arc};
pub struct Sources {
    pub records: Arc<dyn Records>,
    pub snapshots: SnapshotService,
    pub policy: Arc<dyn Policy>,
    pub clock: Arc<dyn super::service::Clock>,
    pub staged_limits: staged::Limits,
}
fn unsupported(source: &ArtifactRef) -> Value {
    let kind = source.to_value()["kind"].as_str().unwrap().to_owned();
    let mut e = error(
        "unsupported_transition",
        &format!("{kind} cannot be used as a source for research_manifest"),
    );
    e["error"]["source_kind"] = Value::String(kind);
    e["error"]["destination"] = json!("research_manifest");
    e
}
fn store_error(message: &str) -> Value {
    error("source_unavailable", message)
}
fn policy_error() -> Value {
    error(
        "source_policy_denied",
        "current policy denies the source artifact",
    )
}
struct Selection {
    ids: Vec<String>,
    query: String,
    total: usize,
    query_id: String,
}
impl Sources {
    async fn snapshot(
        &self,
        tenant: &TenantId,
        id: &str,
        cancel: &Cancellation,
    ) -> Result<Selection, Value> {
        let id = SnapshotId::new(id)
            .map_err(|_| error("source_not_found", "source artifact was not found"))?;
        if cancel.is_cancelled() {
            return Err(store_error("source artifact store is unavailable"));
        }
        match self
            .snapshots
            .for_tenant(tenant.clone())
            .read_projected(&id)
            .await
        {
            SnapshotProjectionLookup::Missing => {
                Err(error("source_not_found", "source artifact was not found"))
            }
            SnapshotProjectionLookup::Unavailable => {
                Err(store_error("source artifact store is unavailable"))
            }
            SnapshotProjectionLookup::Expired { expires_at } => {
                let mut e = error("source_expired", "source artifact has expired");
                e["error"]["expires_at"] = json!(expires_at.timestamp().seconds());
                Err(e)
            }
            SnapshotProjectionLookup::Found(rows) => {
                let s = rows.summary();
                self.policy
                    .current()
                    .retained_engine_rejection(&s.scope().selected_engines, "engines")
                    .map_err(|_| policy_error())?;
                Ok(Selection {
                    ids: (0..s.total().min(25))
                        .map(|i| format!("{}:{i}", s.id()))
                        .collect(),
                    query: s.query().into(),
                    total: s.total(),
                    query_id: s.query_id().as_str().into(),
                })
            }
        }
    }
    fn research_allowed(&self, job: &ssx_domain::research::ResearchJob) -> Result<(), Value> {
        let policy = self.policy.current();
        if !policy.policy.grants.contains("research") {
            return Err(policy_error());
        }
        policy
            .retained_engine_rejection(
                &job.queries
                    .iter()
                    .flat_map(|q| q.engines.clone())
                    .collect::<Vec<_>>(),
                "engines",
            )
            .map_err(|_| policy_error())?;
        policy
            .validate_research_workflow(job)
            .map_err(|_| policy_error())?;
        Ok(())
    }
    async fn research(
        &self,
        tenant: &TenantId,
        root: &ArtifactRef,
        cancel: &Cancellation,
    ) -> Result<ResolvedManifestSource, Value> {
        if !self.policy.current().policy.grants.contains("research") {
            return Err(error(
                "source_policy_denied",
                "research access is disabled by current policy",
            ));
        }
        let attempt_only = root.kind() == ArtifactKind::ResearchAttempt;
        let parts = if attempt_only {
            input::composite(root.id(), 2)
                .map_err(|_| error("source_not_found", "source research attempt was not found"))?
        } else {
            vec![root.id().into()]
        };
        let read = self
            .records
            .research(tenant, &parts[0], cancel)
            .await
            .map_err(|_| store_error("research job store is unavailable"))?;
        let job = match read {
            Read::Found(job) => job,
            Read::Unavailable => return Err(store_error("research job store is unavailable")),
            _ => {
                return Err(error(
                    "source_not_found",
                    "source research artifact was not found",
                ));
            }
        };
        if job.tenant != tenant.as_str()
            || job.job_id != parts[0]
            || job.workflow.get("kind").and_then(Value::as_str) == Some("dependency_dossier")
        {
            return Err(error(
                "source_not_found",
                "source research artifact was not found",
            ));
        }
        self.research_allowed(&job)?;
        let attempts = job
            .queries
            .iter()
            .flat_map(|q| &q.attempts)
            .filter(|a| !attempt_only || a.attempt_id == parts[1])
            .collect::<Vec<_>>();
        if attempt_only && attempts.is_empty() {
            return Err(error(
                "source_not_found",
                "source research attempt was not found",
            ));
        }
        let cursors = attempts
            .iter()
            .filter_map(|a| a.cursor.as_deref())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        if cursors.is_empty() {
            return Err(error(
                "source_partial",
                "source research artifact has no captured result snapshot",
            ));
        }
        let mut ids = vec![];
        let mut seen = BTreeSet::new();
        let mut total = 0usize;
        for cursor in cursors {
            let selected = self.snapshot(tenant, cursor, cancel).await?;
            self.research_allowed(&job)?;
            total = total
                .checked_add(selected.total)
                .ok_or_else(|| error("resource_limit", "source result count overflow"))?;
            for id in selected.ids {
                if seen.insert(id.clone()) && ids.len() < 25 {
                    ids.push(id);
                }
            }
        }
        let status = if total > ids.len() {
            "truncated"
        } else if job.state == "succeeded" {
            "live"
        } else {
            "partial"
        };
        let stored = json!({"artifact":root.to_value(),"status":status,"result_ids":ids,"query":job.question,"details":{"job_id":job.job_id,"job_state":job.state,"result_count":total,"included_result_count":ids.len()}});
        Ok(ResolvedManifestSource {
            artifact: root.clone(),
            result_ids: ids,
            stored,
        })
    }
    async fn staged(
        &self,
        tenant: &TenantId,
        root: &ArtifactRef,
        cancel: &Cancellation,
    ) -> Result<ResolvedManifestSource, Value> {
        if !self
            .policy
            .current()
            .policy
            .grants
            .contains("staged_search")
        {
            return Err(error(
                "source_policy_denied",
                "staged-search access is disabled by current policy",
            ));
        }
        let read = self
            .records
            .staged(tenant, root.id(), self.clock.now(), cancel)
            .await
            .map_err(|_| store_error("staged-search store is unavailable"))?;
        let record = match read {
            Read::Found(r) => r,
            Read::Unavailable => return Err(store_error("staged-search store is unavailable")),
            Read::Expired(at) => {
                let mut e = error("source_expired", "source staged search has expired");
                e["error"]["expires_at"] = json!(at.seconds());
                return Err(e);
            }
            _ => {
                return Err(error(
                    "source_not_found",
                    "source staged search was not found",
                ));
            }
        };
        if record.tenant() != *tenant || record.id().as_str() != root.id() {
            return Err(error(
                "source_not_found",
                "source staged search was not found",
            ));
        }
        let (policy, limits, _) = self.policy.current_with_limits(&self.staged_limits, 0);
        if !policy.policy.grants.contains("staged_search")
            || staged::revalidate(&policy, &limits, &record).is_err()
        {
            return Err(error(
                "source_policy_denied",
                "current policy denies the source staged search",
            ));
        }
        let v = record.value();
        let selected = v["stages"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|s| s["attempts"].as_array().into_iter().flatten())
            .find(|a| a["attempt_id"] == v["selected_result_attempt_id"]);
        let Some((attempt, cursor)) = selected.and_then(|a| {
            a["cursor"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(|c| (a, c))
        }) else {
            return Err(error(
                "source_partial",
                "source staged search has no selected result snapshot",
            ));
        };
        let selected = self.snapshot(tenant, cursor, cancel).await?;
        let (policy, limits, _) = self.policy.current_with_limits(&self.staged_limits, 0);
        if !policy.policy.grants.contains("staged_search")
            || staged::revalidate(&policy, &limits, &record).is_err()
        {
            return Err(policy_error());
        }
        let status = if selected.total > selected.ids.len() {
            "truncated"
        } else {
            "live"
        };
        let stored = json!({"artifact":root.to_value(),"status":status,"result_ids":selected.ids,"query":selected.query,"details":{"operation_id":root.id(),"selected_attempt_id":attempt["attempt_id"],"selected_snapshot":cursor,"query_id":selected.query_id,"result_count":selected.total,"included_result_count":selected.ids.len()}});
        Ok(ResolvedManifestSource {
            artifact: root.clone(),
            result_ids: selected.ids,
            stored,
        })
    }
}
impl ManifestSources for Sources {
    fn resolve<'a>(
        &'a self,
        tenant: &'a TenantId,
        source: &'a ArtifactRef,
        cancel: &'a Cancellation,
    ) -> PortFuture<'a, Result<ResolvedManifestSource, Value>> {
        Box::pin(async move {
            match source.kind() {
                ArtifactKind::ResearchJob | ArtifactKind::ResearchAttempt => {
                    self.research(tenant, source, cancel).await
                }
                ArtifactKind::StagedSearch => self.staged(tenant, source, cancel).await,
                _ => Err(unsupported(source)),
            }
        })
    }
}
