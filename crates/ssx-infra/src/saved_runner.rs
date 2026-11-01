//! Saved execution owns acquisition cancellation and lease renewal. A cancelled
//! provider request may already have reached its remote peer; no exactly-once
//! external execution guarantee is made.
use crate::{
    search::{SearchCancellation, SearchService},
    valkey::saved::{Claim, Error as StoreError, SavedStore},
};
use ssx_application::{
    policy::{PolicyService, ScopeProposal},
    saved::{SavedLimits, SavedPolicy, compare_observations, observe},
};
use ssx_domain::{DomainError, SavedSearchId, SearchRequest, saved::SavedDefinition};
use ssx_json::Value;
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tokio::sync::watch;

/// Re-read authority at both boundaries. Deployments with dynamic grants must
/// implement this port using their live grant/config authority.
pub trait SavedAuthority: Send + Sync {
    fn fingerprint(&self, definition: &SavedDefinition) -> Result<String, DomainError>;
}
/// Concrete immutable composition matching the existing native service lifetime.
/// Replacing this value requires replacing the runner. It does not hot-reload.
pub struct StaticSavedAuthority {
    pub search: PolicyService,
    pub saved_granted: bool,
    pub auth_configured: BTreeMap<String, bool>,
    pub limits: SavedLimits,
}
impl SavedAuthority for StaticSavedAuthority {
    fn fingerprint(&self, definition: &SavedDefinition) -> Result<String, DomainError> {
        let policy = SavedPolicy {
            search: &self.search,
            saved_granted: self.saved_granted,
            auth_configured: &self.auth_configured,
            limits: &self.limits,
        };
        policy.validate_scope(&definition.query, &definition.engines)?;
        policy.fingerprint(&definition.engines)
    }
}
#[derive(Debug)]
pub enum RunnerError {
    Store(StoreError),
    Invalid(DomainError),
    Policy,
    LeaseLost,
    Shutdown,
    Task,
}
impl From<StoreError> for RunnerError {
    fn from(e: StoreError) -> Self {
        Self::Store(e)
    }
}
impl From<DomainError> for RunnerError {
    fn from(e: DomainError) -> Self {
        Self::Invalid(e)
    }
}
#[derive(Clone)]
pub struct SavedRunner {
    store: SavedStore,
    search: SearchService,
    authority: Arc<dyn SavedAuthority>,
    timeout: Duration,
    renew_every: Duration,
    events: Option<crate::saved_events::EventPublisher>,
}
struct OwnedExecution {
    cancel: watch::Sender<bool>,
}
impl Drop for OwnedExecution {
    fn drop(&mut self) {
        self.cancel.send_replace(true);
    }
}
impl SavedRunner {
    pub fn with_events(mut self, events: crate::saved_events::EventPublisher) -> Self {
        self.events = Some(events);
        self
    }
    async fn publish(
        &self,
        claim: &Claim,
        report: &Value,
        baseline: Option<&Value>,
        next_due: f64,
        fingerprint: &str,
    ) -> Result<bool, StoreError> {
        if let Some(events) = &self.events {
            return events
                .publish(&self.store, claim, report, baseline, next_due, fingerprint)
                .await;
        }
        self.store
            .commit(claim, report, baseline, next_due, fingerprint)
            .await
    }

    /// Poll bounded pages with explicit joined shutdown. Discovery failures are
    /// retried on the next poll; persisted claims remain recovery authority.
    pub async fn run(
        &self,
        poll: Duration,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), RunnerError> {
        if poll.is_zero() || tokio::time::Instant::now().checked_add(poll).is_none() {
            return Err(DomainError("invalid saved poll interval").into());
        }
        let mut cursor = 0;
        let mut expiry_cursor = 0;
        loop {
            if *shutdown.borrow() {
                return Ok(());
            }
            if let Some(events) = &self.events {
                let expiry = tokio::select! {biased; _ = stopped(&mut shutdown) => return Ok(()), result = events.publish_expired_page(&self.store, expiry_cursor, 128) => result};
                match expiry {
                    Ok(next) => expiry_cursor = next,
                    Err(_) => {
                        // Source run_forever skips due acquisition after an
                        // expiry publication failure, then waits one poll.
                        tokio::select! {biased; _ = stopped(&mut shutdown) => return Ok(()), _ = tokio::time::sleep(poll) => ()}
                        continue;
                    }
                }
            }
            let page = tokio::select! {biased; _ = stopped(&mut shutdown) => return Ok(()), page = self.store.scan_tenants(cursor,128) => page};
            if let Ok(page) = page {
                cursor = page.cursor;
                for tenant in page.values {
                    let mut runner = self.clone();
                    runner.store = self.store.for_tenant(tenant);
                    let _ = runner.run_due(shutdown.clone()).await;
                    if *shutdown.borrow() {
                        return Ok(());
                    }
                }
            }
            tokio::select! {biased; _ = stopped(&mut shutdown) => return Ok(()), _ = tokio::time::sleep(poll) => ()}
        }
    }
    /// Repair/discover one bounded tenant batch; Valkey remains scheduling authority.
    pub async fn run_due(
        &self,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<Vec<Value>, RunnerError> {
        let ids = tokio::select! {biased; _ = stopped(&mut shutdown) => return Err(RunnerError::Shutdown), ids = async {self.store.repair_indexes().await?;self.store.due_ids(128).await} => ids?};
        let mut pending = tokio::task::JoinSet::new();
        let mut reports = Vec::new();
        for id in ids {
            if *shutdown.borrow() {
                break;
            }
            if pending.len() == 2
                && let Some(Ok(Ok(Some(report)))) = pending.join_next().await
            {
                reports.push(report);
            }
            let runner = self.clone();
            let stop = shutdown.clone();
            pending.spawn(async move { runner.run_one(id, stop).await });
        }
        while let Some(result) = pending.join_next().await {
            if let Ok(Ok(Some(report))) = result {
                reports.push(report);
            }
        }
        Ok(reports)
    }
    /// Bounded operator tuning; renew at least three times per 60 second lease.
    pub fn with_renewal_interval(mut self, interval: Duration) -> Result<Self, RunnerError> {
        if interval.is_zero() || interval > Duration::from_secs(20) {
            return Err(DomainError("invalid saved renewal interval").into());
        }
        self.renew_every = interval;
        Ok(self)
    }
    pub fn new(
        store: SavedStore,
        search: SearchService,
        authority: Arc<dyn SavedAuthority>,
        timeout: Duration,
    ) -> Result<Self, RunnerError> {
        if timeout.is_zero() || tokio::time::Instant::now().checked_add(timeout).is_none() {
            return Err(DomainError("invalid saved dispatch timeout").into());
        }
        Ok(Self {
            store,
            search,
            authority,
            timeout,
            renew_every: Duration::from_secs(20),
            events: None,
        })
    }
    /// An outer dropped future signals its owned task. That task cancels and
    /// joins acquisition and releases authority; it never shuts down shared search.
    pub async fn run_one(
        &self,
        id: SavedSearchId,
        shutdown: watch::Receiver<bool>,
    ) -> Result<Option<Value>, RunnerError> {
        let (cancel, local) = watch::channel(false);
        let guard = OwnedExecution { cancel };
        let runner = self.clone();
        let task = tokio::spawn(async move { runner.execute(id, shutdown, local).await });
        let result = task.await.map_err(|_| RunnerError::Task)?;
        drop(guard);
        result
    }
    fn revalidate(&self, d: &SavedDefinition) -> Result<String, RunnerError> {
        let fingerprint = self
            .authority
            .fingerprint(d)
            .map_err(|_| RunnerError::Policy)?;
        if fingerprint != d.policy_fingerprint {
            return Err(RunnerError::Policy);
        }
        Ok(fingerprint)
    }
    async fn execute(
        &self,
        id: SavedSearchId,
        mut shutdown: watch::Receiver<bool>,
        mut local: watch::Receiver<bool>,
    ) -> Result<Option<Value>, RunnerError> {
        if *shutdown.borrow() || *local.borrow() {
            return Err(RunnerError::Shutdown);
        }
        let instant = tokio::select! {biased;
            _ = stopped(&mut shutdown) => return Err(RunnerError::Shutdown),
            _ = stopped(&mut local) => return Err(RunnerError::Shutdown),
            instant = self.store.now() => instant?,
        };
        let claim = tokio::select! {biased;
            _ = stopped(&mut shutdown) => return Err(RunnerError::Shutdown),
            _ = stopped(&mut local) => return Err(RunnerError::Shutdown),
            claim = self.store.claim_fresh(&id) => claim?,
        };
        let Some(claim) = claim else {
            return Ok(None);
        };
        let result = self
            .execute_claim(&claim, instant, &mut shutdown, &mut local)
            .await;
        // Commit deletes the lease; release then harmlessly returns false.
        // Do not retry failed writes whose completion is unknown.
        // Cleanup is best effort and bounded. A timed-out release may have been
        // accepted; authoritative lease expiry/recovery handles the unknown.
        let _ = tokio::time::timeout(Duration::from_millis(250), self.store.release(&claim)).await;
        result.map(Some)
    }
    async fn execute_claim(
        &self,
        claim: &Claim,
        instant: f64,
        shutdown: &mut watch::Receiver<bool>,
        local: &mut watch::Receiver<bool>,
    ) -> Result<Value, RunnerError> {
        let d = claim.definition();
        let started = self.now_owned(shutdown, local).await?;
        let slot = d.slot(instant)?;
        self.revalidate(d)?;
        let cancellation = SearchCancellation::default();
        let search = self.search.search_with_cancellation(
            ScopeProposal::generic(SearchRequest {
                query: d.query.clone(),
                engines: Some(d.engines.clone()),
                page: ssx_domain::SearchPage::default(),
                max_results: None,
                include: ["results".into(), "engine_status".into()]
                    .into_iter()
                    .collect(),
                freshness: "prefer_fresh".into(),
                client_identifier: Some(format!("mcp-saved:{}", self.store.tenant())),
                ..Default::default()
            }),
            cancellation.clone(),
        );
        tokio::pin!(search);
        let deadline = tokio::time::sleep(self.timeout);
        tokio::pin!(deadline);
        let mut renew = tokio::time::interval(self.renew_every);
        renew.tick().await;
        let response = loop {
            tokio::select! { biased;
                _ = stopped(shutdown) => { cancellation.cancel(); let _ = search.await; return Err(RunnerError::Shutdown); }
                _ = stopped(local) => { cancellation.cancel(); let _ = search.await; return Err(RunnerError::Shutdown); }
                _ = renew.tick() => {
                    let renewed = tokio::select! {biased;
                        _ = stopped(shutdown) => Err(RunnerError::Shutdown),
                        _ = stopped(local) => Err(RunnerError::Shutdown),
                        _ = &mut deadline => {cancellation.cancel(); let _ = search.await; break None;},
                        result = self.store.renew(claim) => result.map_err(RunnerError::Store),
                    };
                    if !matches!(renewed, Ok(true)) {cancellation.cancel(); let _ = search.await; return Err(renewed.err().unwrap_or(RunnerError::LeaseLost));}
                }
                result = &mut search => break Some(result),
                _ = &mut deadline => {cancellation.cancel(); let _ = search.await; break None;}
            }
        };
        let observed = self.now_owned(shutdown, local).await?;
        let observation = match response {
            Some(Ok(response)) => observe(d, &response, observed)?,
            other => {
                ssx_json::json!({"window":d.window()?,"observed_at":observed,"query_id":"","cached":false,"coverage":{},"discovered_count":0,"records":{},"incomparable_reasons":["dispatch_failed"],"operational_error":if other.is_none(){"TimeoutError"}else{"SearchError"}})
            }
        };
        let finished = self.now_owned(shutdown, local).await?;
        let (mut report, baseline) = compare_observations(
            d,
            observation,
            &slot.run_id,
            slot.scheduled_at.get(),
            started,
            finished,
            slot.missed_slots,
        )?;
        report["slot"] =
            ssx_json::to_value(slot.slot).map_err(|_| DomainError("invalid saved slot"))?;
        let fingerprint = self.revalidate(d)?;
        // Renewal remains owned through publication. Store fencing is final;
        // cancellation during an accepted write cannot retract that write.
        let committed = {
            let publication = self.publish(
                claim,
                &report,
                baseline.as_ref(),
                slot.following_due.get(),
                &fingerprint,
            );
            tokio::pin!(publication);
            loop {
                tokio::select! { biased;
                    _ = stopped(shutdown) => return Err(RunnerError::Shutdown),
                    _ = stopped(local) => return Err(RunnerError::Shutdown),
                    _ = renew.tick() => {
                        let renewed = tokio::select! {biased;
                            _ = stopped(shutdown) => return Err(RunnerError::Shutdown),
                            _ = stopped(local) => return Err(RunnerError::Shutdown),
                            result = self.store.renew(claim) => result?,
                        };
                        if !renewed {return Err(RunnerError::LeaseLost);}
                    },
                    committed = &mut publication => break committed?,
                }
            }
        };
        if committed {
            Ok(report)
        } else {
            Err(RunnerError::LeaseLost)
        }
    }
    async fn now_owned(
        &self,
        shutdown: &mut watch::Receiver<bool>,
        local: &mut watch::Receiver<bool>,
    ) -> Result<f64, RunnerError> {
        tokio::select! {biased;
            _ = stopped(shutdown) => Err(RunnerError::Shutdown),
            _ = stopped(local) => Err(RunnerError::Shutdown),
            result = self.store.now() => Ok(result?),
        }
    }
}
async fn stopped(rx: &mut watch::Receiver<bool>) {
    loop {
        if *rx.borrow_and_update() {
            return;
        }
        if rx.changed().await.is_err() {
            return;
        }
    }
}
