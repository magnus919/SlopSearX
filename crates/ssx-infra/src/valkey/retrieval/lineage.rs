//! Readonly adapters over accepted typed stores. Index members are only hints;
//! W04 recent_report remains the retained-report disclosure authority.
use super::super::{
    Failure as StoreFailure, Valkey, keys,
    research::{JobRead, ResearchStore},
    saved::{Error as SavedError, RecentReport, SavedStore},
    staged::{RecordRead, StagedStore},
};
use ssx_application::{
    PortFuture,
    lineage::{
        Failure,
        workflows::{Read, Records},
    },
    policy::PolicyService,
    receipts::Cancellation,
    saved::{SavedLimits, SavedPolicy},
};
use ssx_domain::{
    JobId, SavedSearchId, StagedSearchId, TenantId, WallClockTimestamp, research::ResearchJob,
    saved::SavedDefinition, staged::Record,
};
use ssx_json::Value;
use std::{collections::BTreeMap, sync::Arc};

/// One coherent current composition snapshot. Credential VALUES are excluded.
/// W04 recent_report consumes search, saved_granted and limits.reports; its
/// retained disclosure contract does not gate runtime activity or auth readiness.
pub struct AuthoritySnapshot {
    pub search: PolicyService,
    pub limits: SavedLimits,
    pub auth_configured: BTreeMap<String, bool>,
}
/// Composition supplies live operator state, rather than a stored definition's
/// historical fingerprint or an untrusted index member. No resources are owned.
pub trait Authority: Send + Sync {
    fn current(&self) -> AuthoritySnapshot;
}
pub struct NativeRecords {
    pub backend: Arc<Valkey>,
    pub authority: Arc<dyn Authority>,
}
fn check(cancel: &Cancellation) -> Result<(), Failure> {
    if cancel.is_cancelled() {
        Err(Failure::Cancelled)
    } else {
        Ok(())
    }
}
fn store_failure(error: StoreFailure) -> Failure {
    match error {
        StoreFailure::Closed => Failure::Closed,
        StoreFailure::Unavailable | StoreFailure::CompletionUnknown => Failure::Unavailable,
        _ => Failure::Malformed,
    }
}
fn saved_failure<T>(error: SavedError) -> Result<Read<T>, Failure> {
    match error {
        SavedError::Missing => Ok(Read::Missing),
        SavedError::Invalid(_) | SavedError::Conflict => Ok(Read::PolicyDenied),
        SavedError::Store(
            StoreFailure::Unavailable | StoreFailure::Closed | StoreFailure::CompletionUnknown,
        ) => Ok(Read::Unavailable),
        _ => Err(Failure::Malformed),
    }
}
impl Records for NativeRecords {
    fn research<'a>(
        &'a self,
        tenant: &'a TenantId,
        id: &'a str,
        cancel: &'a Cancellation,
    ) -> PortFuture<'a, Result<Read<ResearchJob>, Failure>> {
        Box::pin(async move {
            check(cancel)?;
            let Ok(id) = JobId::new(id) else {
                return Ok(Read::Missing);
            };
            // Await the owning store read to completion before cancellation is
            // observed. No select/spawn/abandoned read or extra driver owner.
            let result = ResearchStore::new(self.backend.clone(), tenant.clone())
                .load(&id)
                .await;
            check(cancel)?;
            match result {
                JobRead::Found(job) => Ok(Read::Found(*job)),
                JobRead::Missing => Ok(Read::Missing),
                JobRead::Unavailable | JobRead::Closed => Ok(Read::Unavailable),
                JobRead::Malformed => Err(Failure::Malformed),
            }
        })
    }
    fn staged<'a>(
        &'a self,
        tenant: &'a TenantId,
        id: &'a str,
        now: WallClockTimestamp,
        cancel: &'a Cancellation,
    ) -> PortFuture<'a, Result<Read<Record>, Failure>> {
        Box::pin(async move {
            check(cancel)?;
            let Ok(id) = StagedSearchId::new(id) else {
                return Ok(Read::Missing);
            };
            let result = StagedStore::new(&self.backend).read(tenant, &id, now).await;
            check(cancel)?;
            match result {
                Ok(RecordRead::Found(record)) => Ok(Read::Found(record)),
                Ok(RecordRead::Missing) => Ok(Read::Missing),
                Ok(RecordRead::Expired { expires_at }) => Ok(Read::Expired(
                    WallClockTimestamp::new(expires_at).map_err(|_| Failure::Malformed)?,
                )),
                Err(
                    StoreFailure::Unavailable
                    | StoreFailure::Closed
                    | StoreFailure::CompletionUnknown,
                ) => Ok(Read::Unavailable),
                Err(error) => Err(store_failure(error)),
            }
        })
    }
    fn saved<'a>(
        &'a self,
        tenant: &'a TenantId,
        id: &'a str,
        cancel: &'a Cancellation,
    ) -> PortFuture<'a, Result<Read<SavedDefinition>, Failure>> {
        Box::pin(async move {
            check(cancel)?;
            let Ok(id) = SavedSearchId::new(id) else {
                return Ok(Read::Missing);
            };
            let result = SavedStore::new(self.backend.clone(), tenant.clone())
                .load(&id)
                .await;
            check(cancel)?;
            match result {
                Ok(Some(definition)) => Ok(Read::Found(definition)),
                Ok(None) | Err(SavedError::Missing) => Ok(Read::Missing),
                Err(SavedError::Store(
                    StoreFailure::Unavailable
                    | StoreFailure::Closed
                    | StoreFailure::CompletionUnknown,
                )) => Ok(Read::Unavailable),
                // load is a typed read, not policy validation.
                Err(_) => Err(Failure::Malformed),
            }
        })
    }
    fn reports<'a>(
        &'a self,
        tenant: &'a TenantId,
        definition: &'a SavedDefinition,
        _now: WallClockTimestamp,
        limit: usize,
        selected_run: Option<&'a str>,
        cancel: &'a Cancellation,
    ) -> PortFuture<'a, Result<Read<Vec<Value>>, Failure>> {
        Box::pin(async move {
            check(cancel)?;
            if definition.tenant != tenant.as_str() || !(1..=100).contains(&limit) {
                return Err(Failure::Malformed);
            }
            let id = SavedSearchId::new(&definition.search_id).map_err(|_| Failure::Malformed)?;
            let initial = self.authority.current();
            // Same grant/limit ordering as recent_report, even for empty hints.
            if !initial.search.policy.grants.contains("saved_searches")
                || !(1..=100).contains(&initial.limits.reports)
            {
                return Ok(Read::PolicyDenied);
            }
            let bound = limit
                .min(definition.max_reports as usize)
                .min(initial.limits.reports as usize);
            if bound == 0 {
                return Err(Failure::Malformed);
            }
            let key = keys::saved("reports", tenant, Some(id.as_str())).expect("fixed key kind");
            let mut command = redis::cmd("ZREVRANGE");
            command.arg(key).arg(0).arg(bound - 1);
            let reply = self.backend.execute(command, false).await;
            check(cancel)?;
            let reply = match reply {
                Ok(reply) => reply,
                Err(
                    StoreFailure::Unavailable
                    | StoreFailure::Closed
                    | StoreFailure::CompletionUnknown,
                ) => return Ok(Read::Unavailable),
                Err(error) => return Err(store_failure(error)),
            };
            let ids: Vec<String> =
                redis::from_redis_value(reply).map_err(|_| Failure::Malformed)?;
            if ids.len() > bound {
                return Err(Failure::Malformed);
            }
            let store = SavedStore::new(self.backend.clone(), tenant.clone());
            let mut reports = Vec::with_capacity(ids.len());
            for run in ids {
                check(cancel)?;
                let current = self.authority.current();
                let policy = SavedPolicy {
                    search: &current.search,
                    saved_granted: current.search.policy.grants.contains("saved_searches"),
                    auth_configured: &current.auth_configured,
                    limits: &current.limits,
                };
                // Re-reads authoritative current definition, current bounded
                // membership and retained row; validates binding/expiry/raw CAS.
                // Preserve W04 errors before filtering the selected projection.
                let read = store.recent_report(&id, &run, &policy).await;
                check(cancel)?;
                match read {
                    Ok(Some(
                        RecentReport::Found(mut value)
                        | RecentReport::Incomparable(mut value)
                        | RecentReport::Truncated(mut value),
                    )) => {
                        let object = value.as_object_mut().ok_or(Failure::Malformed)?;
                        let mut projection = ssx_json::Map::new();
                        projection.insert(
                            "run_id".into(),
                            object.swap_remove("run_id").unwrap_or(Value::Null),
                        );
                        if selected_run == Some(run.as_str()) {
                            for field in ["status", "finished_at", "expires_at"] {
                                projection.insert(
                                    field.into(),
                                    object.swap_remove(field).unwrap_or(Value::Null),
                                );
                            }
                        }
                        reports.push(Value::Object(projection));
                    }
                    Ok(None) | Err(SavedError::Missing) => {}
                    Err(error) => return saved_failure(error),
                }
            }
            check(cancel)?;
            Ok(Read::Found(reports))
        })
    }
}
