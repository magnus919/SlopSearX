//! Full selected report reads for composition. The W07 lineage projection stays
//! unchanged. SavedStore::recent_report is the sole membership/expiry authority.
use crate::valkey::{
    Failure as StoreFailure, Valkey,
    retrieval::lineage::Authority as SavedAuthority,
    saved::{Error as SavedError, RecentReport, SavedStore},
};
use ssx_application::{
    PortFuture,
    composition::{Authority, SavedReports},
    lineage::{Failure, workflows::Read},
    receipts::Cancellation,
    saved::SavedPolicy,
};
use ssx_domain::{SavedSearchId, TenantId, saved::SavedDefinition};
use ssx_json::Value;
use std::sync::Arc;
pub struct NativeSavedReports {
    pub backend: Arc<Valkey>,
    pub authority: Arc<dyn SavedAuthority>,
    pub principal: Arc<dyn Authority>,
}
impl NativeSavedReports {
    fn allowed(
        &self,
        tenant: &TenantId,
        definition: &SavedDefinition,
        engines: &[String],
        bound: usize,
    ) -> bool {
        let principal = self.principal.current();
        let current = self.authority.current();
        principal.tenant.as_ref() == Some(tenant)
            && definition.tenant == tenant.as_str()
            && principal.policy.policy.grants.contains("saved_searches")
            && current.search.policy.grants.contains("saved_searches")
            && (1..=100).contains(&bound)
            && principal.saved_max_reports >= bound
            && current.limits.reports >= bound as u64
            && principal
                .policy
                .retained_engine_rejection(engines, "engines")
                .is_ok()
            && current
                .search
                .retained_engine_rejection(engines, "engines")
                .is_ok()
    }
}
impl SavedReports for NativeSavedReports {
    fn selected<'a>(
        &'a self,
        tenant: &'a TenantId,
        definition: &'a SavedDefinition,
        run: &'a str,
        limit: usize,
        cancel: &'a Cancellation,
    ) -> PortFuture<'a, Result<Read<Value>, Failure>> {
        Box::pin(async move {
            if cancel.is_cancelled() {
                return Err(Failure::Cancelled);
            }
            let bound = limit.min(usize::try_from(definition.max_reports).unwrap_or(usize::MAX));
            if !self.allowed(tenant, definition, &definition.engines, bound) {
                return Ok(Read::PolicyDenied);
            }
            let id = SavedSearchId::new(&definition.search_id).map_err(|_| Failure::Malformed)?;
            let principal = self.principal.current();
            let current = self.authority.current();
            let mut search = principal.policy;
            // The owning reader can re-read a changed definition internally.
            // Restrict its immutable policy to the initial eligible intersection
            // and retain every eligible engine as a post-await witness, exactly
            // as the accepted W07 report fence guards hidden definition reads.
            let eligible = search
                .catalog
                .engines()
                .iter()
                .filter(|engine| {
                    let names = std::slice::from_ref(&engine.name);
                    search.retained_engine_rejection(names, "engines").is_ok()
                        && current
                            .search
                            .retained_engine_rejection(names, "engines")
                            .is_ok()
                })
                .cloned()
                .collect::<Vec<_>>();
            let mut engines = eligible
                .iter()
                .map(|engine| engine.name.clone())
                .collect::<Vec<_>>();
            if !self.allowed(tenant, definition, &engines, bound) {
                return Ok(Read::PolicyDenied);
            }
            search.catalog = ssx_application::scope::CatalogSnapshot::new(eligible)
                .map_err(|_| Failure::Malformed)?;
            let mut limits = current.limits.clone();
            limits.reports = bound as u64;
            let policy = SavedPolicy {
                search: &search,
                saved_granted: search.policy.grants.contains("saved_searches")
                    && current.search.policy.grants.contains("saved_searches"),
                auth_configured: &current.auth_configured,
                limits: &limits,
            };
            let read = SavedStore::new(self.backend.clone(), tenant.clone())
                .recent_report(&id, run, &policy)
                .await;
            if cancel.is_cancelled() {
                return Err(Failure::Cancelled);
            }
            if !self.allowed(tenant, definition, &engines, bound) {
                return Ok(Read::PolicyDenied);
            }
            let value = match read {
                Ok(Some(
                    RecentReport::Found(value)
                    | RecentReport::Incomparable(value)
                    | RecentReport::Truncated(value),
                )) => value,
                Ok(None) | Err(SavedError::Missing) => return Ok(Read::Missing),
                Err(SavedError::Invalid(_) | SavedError::Conflict) => {
                    return Ok(Read::PolicyDenied);
                }
                Err(SavedError::Store(
                    StoreFailure::Unavailable
                    | StoreFailure::Closed
                    | StoreFailure::CompletionUnknown,
                )) => return Ok(Read::Unavailable),
                Err(_) => return Err(Failure::Malformed),
            };
            engines.extend(definition.engines.clone());
            if let Some(names) = value["observation"]["window"]["engines"].as_array() {
                for name in names {
                    if let Some(name) = name.as_str() {
                        engines.push(name.into());
                    }
                }
            }
            if !self.allowed(tenant, definition, &engines, bound) {
                return Ok(Read::PolicyDenied);
            }
            Ok(Read::Found(value))
        })
    }
}
