//! Durable research execution. Valkey owns admission, leases and publication;
//! cancelling local I/O cannot retract an already accepted provider request.
use crate::{
    search::{SearchCancellation, SearchClock, SearchError, SearchService, SystemSearchClock},
    valkey::{
        Ownership, TokenSource,
        research::{Claim, Error as StoreError, JobRead, ResearchStore},
    },
};
use ssx_application::{
    policy::ScopeProposal,
    snapshot::{SnapshotFailure, SnapshotService},
};
use ssx_domain::{
    ArtifactKind, ArtifactRef, AttemptId, DomainError, JobId, LeaseToken, TenantId,
    WallClockTimestamp,
    research::{
        ResearchCeilings, ResearchJob, ResearchQuery, classify_coverage, summarize_coverage,
    },
};
use std::{num::NonZeroU64, sync::Arc, time::Duration};
use tokio::{sync::watch, task::JoinSet};

#[derive(Clone)]
pub struct RunnerConfig {
    pub owner: String,
    pub lease_seconds: NonZeroU64,
    pub poll_interval: Duration,
    pub workers: NonZeroU64,
    pub ceilings: ResearchCeilings,
}
#[derive(Debug)]
pub enum RunnerError {
    Store(StoreError),
    Invalid(DomainError),
    AuthorityLost,
    Shutdown,
    NoRetryableWork,
    LiveOwner,
    Snapshot(SnapshotFailure),
    Policy(SearchError),
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

/// Trusted W08 read-only composition admission. It cannot dispatch or persist.
pub trait SourceValidator: Send + Sync {
    fn validate<'a>(
        &'a self,
        tenant: &'a TenantId,
        job: &'a ResearchJob,
    ) -> ssx_application::PortFuture<'a, Result<(), ssx_application::composition::Error>>;
}
/// Composition owns the search service; a runner never shuts down shared search.

#[derive(Clone)]
pub struct ResearchRunner {
    jobs: ResearchStore,
    search: SearchService,
    snapshots: SnapshotService,
    config: RunnerConfig,
    clock: Arc<dyn SearchClock>,
    source_validator: Option<Arc<dyn SourceValidator>>,
}
struct OwnedRenewal {
    cancellation: SearchCancellation,
    task: tokio::task::JoinHandle<Result<(), RunnerError>>,
}
impl Drop for OwnedRenewal {
    fn drop(&mut self) {
        self.cancellation.cancel();
        self.task.abort();
    }
}
struct RandomTokens;
impl TokenSource<JobId> for RandomTokens {
    fn next_token(&mut self) -> LeaseToken<JobId> {
        LeaseToken::new(random_id()).expect("random canonical identity")
    }
}
fn random_id() -> String {
    let mut bytes = [0u8; 32];
    aws_lc_rs::rand::fill(&mut bytes).expect("operating system randomness");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn attempt() -> AttemptId {
    AttemptId::new(random_id()).expect("random canonical identity")
}
impl ResearchRunner {
    pub fn new(
        jobs: ResearchStore,
        search: SearchService,
        snapshots: SnapshotService,
        config: RunnerConfig,
    ) -> Result<Self, RunnerError> {
        let instant = tokio::time::Instant::now();
        let lease = Duration::from_secs(config.lease_seconds.get());
        // Redis EX is converted to signed milliseconds, including server epoch.
        // Reject unrepresentable configuration rather than panicking or clamping.
        let epoch_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |v| v.as_millis());
        let valkey_expiry = u128::from(config.lease_seconds.get())
            .checked_mul(1000)
            .and_then(|ttl| ttl.checked_add(epoch_ms));
        if config.owner.is_empty()
            || config.poll_interval.is_zero()
            || config.workers.get() > 128
            || valkey_expiry.is_none_or(|expiry| expiry > i64::MAX as u128)
            || instant.checked_add(lease).is_none()
            || instant.checked_add(config.poll_interval).is_none()
        {
            return Err(DomainError("invalid research runner configuration").into());
        }
        Ok(Self {
            jobs,
            search,
            snapshots,
            config,
            clock: Arc::new(SystemSearchClock),
            source_validator: None,
        })
    }
    /// Uncomposed jobs need no source port. Composed work fails closed until
    /// the native composition root supplies the same resolver as admission.
    pub fn with_source_validator(mut self, validator: Arc<dyn SourceValidator>) -> Self {
        self.source_validator = Some(validator);
        self
    }
    fn now(&self) -> WallClockTimestamp {
        self.clock.capture().0
    }
    /// Polling is recoverable discovery, never an authoritative in-memory queue.
    /// Transient job/store failures leave the pool alive and reservations intact.
    pub async fn run(&self, shutdown: watch::Receiver<bool>) {
        let mut workers = JoinSet::new();
        for _ in 0..self.config.workers.get() {
            let runner = self.clone();
            let mut stop = shutdown.clone();
            workers.spawn(async move {
                loop {
                    if *stop.borrow() { break; }
                    let _ = runner.run_once(stop.clone()).await;
                    tokio::select! { biased; _ = stopped(&mut stop) => break, _ = tokio::time::sleep(runner.config.poll_interval) => () }
                }
            });
        }
        while workers.join_next().await.is_some() {}
    }
    pub async fn run_once(
        &self,
        shutdown: watch::Receiver<bool>,
    ) -> Result<Option<ResearchJob>, RunnerError> {
        if *shutdown.borrow() {
            return Err(RunnerError::Shutdown);
        }
        let claim = self
            .jobs
            .claim_next_any_tenant(
                &self.config.owner,
                self.config.lease_seconds,
                &mut RandomTokens,
                self.now(),
                attempt,
            )
            .await?;
        match claim {
            Some(claim) => self.execute_claim(claim, shutdown).await.map(Some),
            None => Ok(None),
        }
    }
    /// Retry and extension mutations are applied only after exclusive authority
    /// is obtained. A live owner wins over deadline and caller validation.
    pub async fn retry(
        &self,
        tenant: TenantId,
        id: &JobId,
        shutdown: watch::Receiver<bool>,
    ) -> Result<ResearchJob, RunnerError> {
        self.direct(tenant, id, shutdown, |job| {
            if !job.queries.iter().any(ResearchQuery::retryable) {
                return Err(DomainError("no retryable work"));
            }
            job.reset_retryable_queries();
            Ok(())
        })
        .await
    }
    pub async fn extend(
        &self,
        tenant: TenantId,
        id: &JobId,
        mut query: ResearchQuery,
        shutdown: watch::Receiver<bool>,
    ) -> Result<ResearchJob, RunnerError> {
        // The policy service performs the same fail-closed query/engine checks as
        // normal dispatch; validate before mutation, then again before execution.
        if query.state != "pending"
            || !query.attempts.is_empty()
            || query.cursor.is_some()
            || query.query_id.is_some()
            || query.result_count != 0
            || query.error.is_some()
            || !query.engine_coverage.is_empty()
            || !query.enforcement.is_empty()
        {
            return Err(DomainError("extension must contain only pending query input").into());
        }
        self.search
            .prepare_scope(&ScopeProposal::from_research_query(&query))
            .await
            .map_err(RunnerError::Policy)?;
        self.direct(tenant, id, shutdown, move |job| {
            job.initialize_budget(self.config.ceilings)?;
            if job.queries.len() as u64 >= job.budget_limits["queries"] {
                return Err(DomainError("query budget exhausted"));
            }
            // W09 validates wire input; this boundary still applies the
            // persisted ceiling in original order after full-list policy denial.
            let mut seen = std::collections::BTreeSet::new();
            query.engines.retain(|engine| seen.insert(engine.clone()));
            query.engines.truncate(
                usize::try_from(job.budget_limits["engines_per_query"]).unwrap_or(usize::MAX),
            );
            query.index = job.queries.len() as u64;
            job.queries.push(query);
            Ok(())
        })
        .await
    }
    async fn direct(
        &self,
        tenant: TenantId,
        id: &JobId,
        shutdown: watch::Receiver<bool>,
        mutation: impl FnOnce(&mut ResearchJob) -> Result<(), DomainError>,
    ) -> Result<ResearchJob, RunnerError> {
        if *shutdown.borrow() {
            return Err(RunnerError::Shutdown);
        }
        let scoped = self.jobs.for_tenant(tenant);
        let claim = scoped
            .claim_prepared(
                id,
                &self.config.owner,
                self.config.lease_seconds,
                &mut RandomTokens,
                self.now(),
                attempt,
                false,
                |job| {
                    job.normalize_legacy_evidence();
                    job.initialize_budget(self.config.ceilings)?;
                    mutation(job)
                },
            )
            .await
            .map_err(|e| match e {
                StoreError::Invalid(DomainError("no retryable work")) => {
                    RunnerError::NoRetryableWork
                }
                other => RunnerError::Store(other),
            })?
            .ok_or(RunnerError::LiveOwner)?;
        self.execute_claim(claim, shutdown).await
    }
    pub async fn execute_claim(
        &self,
        claim: Claim,
        shutdown: watch::Receiver<bool>,
    ) -> Result<ResearchJob, RunnerError> {
        let tenant = TenantId::new(claim.job.tenant.clone())?;
        let store = self.jobs.for_tenant(tenant.clone());
        let id = JobId::new(claim.job.job_id.clone())?;
        let token = claim.token;
        let cancellation = SearchCancellation::default();
        let (done_tx, mut done) = watch::channel(false);
        let monitor_store = store.clone();
        let monitor_id = id.clone();
        let monitor_token = token.clone();
        let ttl = self.config.lease_seconds;
        let cancel = cancellation.clone();
        let mut stop = shutdown.clone();
        // Renewal is supervised for the entire claim, including snapshot writes.
        // Any inability to establish authority stops owned work conservatively.
        let monitor = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(ttl.get()) / 3);
            loop {
                tokio::select! { biased;
                    _ = stopped(&mut done) => return Ok(()),
                    _ = stopped(&mut stop) => { cancel.cancel(); return Err(RunnerError::Shutdown); },
                    _ = interval.tick() => {
                        let check = async {
                            match monitor_store.renew(&monitor_id, &monitor_token, ttl).await? {
                                Ownership::Applied => {
                                    #[cfg(test)]
                                    if let Some(probe) = monitor_store.completion_probe() {
                                        probe.renewed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                    }
                                },
                                Ownership::Lost => return Err(RunnerError::AuthorityLost),
                            }
                            let job = read_owned(&monitor_store, &monitor_id, &monitor_token).await?;
                            #[cfg(test)]
                            if let Some(probe) = monitor_store.completion_probe() {
                                probe.validated_owned.add_permits(1);
                            }
                            Ok(job.cancel_requested)
                        };
                        let result = tokio::select! { biased;
                            _ = stopped(&mut done) => return Ok(()),
                            _ = stopped(&mut stop) => { cancel.cancel(); return Err(RunnerError::Shutdown); },
                            result = check => result,
                        };
                        match result {
                            Ok(true) => cancel.cancel(),
                            Ok(false) => (),
                            Err(error) => { cancel.cancel(); return Err(error); },
                        }
                    }
                }
            }
        });
        let mut renewal = OwnedRenewal {
            cancellation: cancellation.clone(),
            task: monitor,
        };
        #[cfg(test)]
        if let Some(probe) = store.completion_probe() {
            // Fixture-only participation barrier: the actual monitor has renewed
            // and successfully validated this owned record before execution.
            probe
                .validated_owned
                .acquire()
                .await
                .expect("owned fixture handshake")
                .forget();
        }
        let result = self
            .execute(&store, &id, &token, tenant, cancellation, shutdown)
            .await;
        done_tx.send_replace(true);
        let monitored = (&mut renewal.task)
            .await
            .map_err(|_| RunnerError::AuthorityLost)
            .and_then(|r| r);
        #[cfg(test)]
        if let Some(probe) = store.completion_probe() {
            probe
                .monitor_joined
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
        // All execution and snapshot writes have finished. Join the authority
        // monitor before clearing the job token that its reads validate.
        let result = match result {
            Ok(_) if monitored.is_ok() => {
                #[cfg(test)]
                if let Some(probe) = store.completion_probe()
                    && probe.pause_before_clear
                {
                    probe.before_clear.add_permits(1);
                    probe
                        .release_before_clear
                        .acquire()
                        .await
                        .expect("owned fixture gate")
                        .forget();
                }
                store
                    .clear_ownership(&id, &token)
                    .await
                    .map_err(RunnerError::Store)
            }
            other => other,
        };
        // Release never deletes a reclaimer's lease. Do not hide unknown writes.
        let released = store.release(&id, &token).await;
        monitored?;
        released?;
        result
    }
    async fn execute(
        &self,
        store: &ResearchStore,
        id: &JobId,
        token: &LeaseToken<JobId>,
        tenant: TenantId,
        cancellation: SearchCancellation,
        shutdown: watch::Receiver<bool>,
    ) -> Result<ResearchJob, RunnerError> {
        let mut job = store
            .mutate(id, token, |job| {
                job.normalize_legacy_evidence();
                job.initialize_budget(self.config.ceilings)?;
                if !job.caller_completed && !["cancelled", "expired"].contains(&job.state.as_str())
                {
                    job.stop_reason = None;
                }
                Ok(())
            })
            .await?;
        if job.caller_completed || ["cancelled", "expired"].contains(&job.state.as_str()) {
            return Ok(job);
        }
        for index in 0..job.queries.len() {
            job = self.load_owned(store, id, token).await?;
            if *shutdown.borrow() {
                return Err(RunnerError::Shutdown);
            }
            if job.cancel_requested || self.now().seconds() >= job.deadline.get() {
                break;
            }
            if job.queries[index].state != "pending" {
                continue;
            }
            if job.workflow.contains_key("composition") {
                let validation = self
                    .validate_source(&tenant, &job, &cancellation, &shutdown)
                    .await;
                if let Err(error) = validation {
                    // No reservation exists: denied sources consume no attempt
                    // or adapter-call budget and cannot look like execution.
                    store
                        .mutate(id, token, |j| {
                            if j.cancel_requested || self.now().seconds() >= j.deadline.get() {
                                return Ok(());
                            }
                            let reason = format!("source_rejected: {}", error.code);
                            j.stop_reason = Some(reason.clone());
                            for query in &mut j.queries {
                                if query.state == "pending" {
                                    query.state = "failed".into();
                                    query.error = Some(reason.clone());
                                }
                            }
                            Ok(())
                        })
                        .await?;
                    break;
                }
            }
            if let Err(error) = self.search.validate_research_workflow(&job) {
                let denied_attempt = attempt();
                store
                    .mutate(id, token, |j| {
                        let now = self.now();
                        if j.cancel_requested || now.seconds() >= j.deadline.get() {
                            return Ok(());
                        }
                        // Source records a failed predispatch audit. Native W01
                        // requires every appended attempt to conserve reservation
                        // charges; no actual dossier adapter call is charged here.
                        if j.reserve_attempt(index, denied_attempt.clone(), now)? {
                            let q = &mut j.queries[index];
                            q.state = "failed".into();
                            q.error = Some(format!("policy_rejected: {error:?}"));
                            q.engine_coverage = q
                                .engines
                                .iter()
                                .map(|name| classify_coverage(name.clone(), false, None, 0, false))
                                .collect();
                            ssx_application::research::finish_attempt(
                                j,
                                index,
                                &denied_attempt,
                                None,
                                now,
                            )?;
                        }
                        Ok(())
                    })
                    .await?;
                continue;
            }
            let acknowledged = attempt();

            job = store
                .mutate(id, token, |j| {
                    if j.cancel_requested || self.now().seconds() >= j.deadline.get() {
                        return Ok(());
                    }
                    if dossier(j) {
                        let (used, max_calls, captured, max_results) =
                            match workflow_budget(j, self.config.ceilings.results) {
                                Ok(values) => values,
                                Err(error) => {
                                    j.queries[index].state = "failed".into();
                                    j.queries[index].error = Some(error.0.into());
                                    return Ok(());
                                }
                            };
                        j.workflow
                            .entry("budget".into())
                            .or_insert_with(|| ssx_json::json!({}));
                        let cost = j.queries[index].engines.len() as u64;
                        if used.checked_add(cost).is_none_or(|next| next > max_calls)
                            || captured >= max_results
                        {
                            j.queries[index].state = "failed".into();
                            j.queries[index].error =
                                Some("budget_exceeded: workflow budget exhausted".into());
                            return Ok(());
                        }
                    }
                    if j.reserve_attempt(index, acknowledged.clone(), self.now())? && dossier(j) {
                        let budget = j
                            .workflow
                            .get_mut("budget")
                            .expect("validated workflow budget");
                        let used = budget["used_adapter_calls"].as_u64().unwrap_or(0);
                        budget["used_adapter_calls"] =
                            ssx_json::json!(used + j.queries[index].engines.len() as u64);
                    }
                    Ok(())
                })
                .await?;
            if job.queries[index].state != "running" {
                break;
            }
            job = self.load_owned(store, id, token).await?;
            if store.renew(id, token, self.config.lease_seconds).await? != Ownership::Applied {
                return Err(RunnerError::AuthorityLost);
            }
            if job.cancel_requested || self.now().seconds() >= job.deadline.get() {
                cancellation.cancel();
            }
            // The first check avoided unnecessary reservations. Reads/writes and
            // lease renewal have awaited since then, so admit the source again
            // at the actual dispatch boundary. Nothing awaits between a valid
            // concrete source witness and construction/polling of the search.
            if let Err(error) = self
                .validate_source(&tenant, &job, &cancellation, &shutdown)
                .await
            {
                store
                    .mutate(id, token, |j| {
                        let now = self.now();
                        let reason = format!("source_rejected_before_dispatch: {}", error.code);
                        let q = &mut j.queries[index];
                        q.state = "failed".into();
                        q.error = Some(reason.clone());
                        q.engine_coverage = q
                            .engines
                            .iter()
                            .map(|name| classify_coverage(name.clone(), false, None, 0, false))
                            .collect();
                        ssx_application::research::finish_attempt(
                            j,
                            index,
                            &acknowledged,
                            None,
                            now,
                        )?;
                        j.stop_reason = Some(reason.clone());
                        for q in &mut j.queries {
                            if q.state == "pending" {
                                q.state = "failed".into();
                                q.error = Some(reason.clone());
                            }
                        }
                        // W01 reservations are conservative, monotonic evidence.
                        // This known unexecuted attempt retains its reservation;
                        // there is no response, new lead or actual adapter request.
                        Ok(())
                    })
                    .await?;
                break;
            }
            let mut proposal = ScopeProposal::from_research_query(&job.queries[index]);

            proposal.request.client_identifier = Some(format!("mcp-job:{}", id.as_str()));
            proposal.request.execution_isolation_key =
                Some(ssx_domain::canonical::composite_artifact_id(&[
                    tenant.as_str(),
                    id.as_str(),
                    acknowledged.as_str(),
                ])?);
            proposal.request.generate_suggestions = false;
            let parents = [ArtifactRef::new(
                ArtifactKind::ResearchAttempt,
                ssx_domain::canonical::composite_artifact_id(&[
                    id.as_str(),
                    acknowledged.as_str(),
                ])?,
            )?];
            let snapshots = self.snapshots.for_tenant(tenant.clone());
            let execution = self.search.search_snapshot_with_cancellation(
                proposal.clone(),
                &snapshots,
                &parents,
                cancellation.clone(),
            );
            tokio::pin!(execution);
            // A finite persisted deadline can exceed Duration/Instant range.
            // Such a horizon remains valid; normal engine deadlines still bound
            // execution, and absolute wall time is checked at each mutation.
            let timer =
                Duration::try_from_secs_f64((job.deadline.get() - self.now().seconds()).max(0.0))
                    .ok()
                    .and_then(|remaining| tokio::time::Instant::now().checked_add(remaining));
            let capture = tokio::select! { biased;
                result = &mut execution => result,
                _ = wait_deadline(timer) => { cancellation.cancel(); execution.await }
            };
            let policy = match self.search.validate_research_workflow(&job) {
                Ok(()) => self.search.prepare_scope(&proposal).await,
                Err(error) => Err(error),
            };
            let response = capture.as_ref().ok().map(|c| c.canonical.response());
            let cursor = capture
                .as_ref()
                .ok()
                .and_then(|c| c.snapshot.as_ref().ok())
                .map(|s| s.as_str().to_owned());
            job = store
                .mutate(id, token, |j| {
                    let guarded_policy =
                        policy.as_ref().map_err(Clone::clone).and_then(|authority| {
                            self.search.validate_research_workflow(j)?;
                            self.search.revalidate_research_scope(authority)
                        });
                    let now = self.now();
                    let stopped = j.cancel_requested || now.seconds() >= j.deadline.get();
                    let q = &mut j.queries[index];
                    if stopped {
                        q.state = "cancelled".into();
                        q.error = Some(
                            if j.cancel_requested {
                                "cancelled"
                            } else {
                                "deadline_expired"
                            }
                            .into(),
                        );
                    } else if let Err(error) = &guarded_policy {
                        q.state = "failed".into();
                        q.error = Some(format!("policy_rejected: {error:?}"));
                    } else if let Some(response) = response {
                        q.query_id = Some(response.query_id.clone());
                        q.result_count = response.results.len() as u64;
                        q.cursor = cursor.clone();
                        q.engine_coverage = self
                            .search
                            .research_coverage(q, response)
                            .map_err(|_| DomainError("research coverage policy unavailable"))?;
                        if let Ok(authority) = &policy {
                            q.enforcement = authority
                                .preview()
                                .enforcement
                                .iter()
                                .filter(|(k, _)| {
                                    k.as_str() == "time_range" && q.time_range.is_some()
                                })
                                .map(|(k, v)| (k.clone(), v.to_value()))
                                .collect();
                        }
                        let coverage = summarize_coverage(&q.engine_coverage)?;
                        q.state = if response.all_unresponsive
                            || coverage.failed > 0 && q.result_count == 0
                        {
                            "failed"
                        } else {
                            "done"
                        }
                        .into();
                        q.error = if response.all_unresponsive {
                            Some("no engines responded".into())
                        } else if coverage.failed > 0 && q.result_count == 0 {
                            Some("some engines failed and none returned results".into())
                        } else {
                            None
                        };
                        if let Some(error) = capture
                            .as_ref()
                            .ok()
                            .and_then(|c| c.snapshot.as_ref().err())
                        {
                            q.state = "failed".into();
                            q.error = Some(format!("snapshot_failed: {error:?}"));
                        }
                    } else {
                        q.state = "failed".into();
                        q.error = Some(format!("search_failed: {:?}", capture.as_ref().err()));
                    }
                    if !stopped
                        && guarded_policy.is_ok()
                        && let Some(response) = response
                        && dossier(j)
                    {
                        let (_, _, captured, max_results) =
                            workflow_budget(j, self.config.ceilings.results)?;
                        let budget = j
                            .workflow
                            .get_mut("budget")
                            .expect("reserved workflow budget");
                        let admitted = (response.results.len() as u64)
                            .min(max_results.saturating_sub(captured));
                        budget["captured_results"] = ssx_json::json!(captured + admitted);
                        if budget.get("admitted_results").is_none() {
                            budget["admitted_results"] = ssx_json::json!({});
                        }
                        budget["admitted_results"][index.to_string().as_str()] =
                            ssx_json::json!(admitted);
                    }
                    ssx_application::research::finish_attempt(
                        j,
                        index,
                        &acknowledged,
                        if stopped || guarded_policy.is_err() {
                            None
                        } else {
                            response
                        },
                        now,
                    )
                })
                .await?;
            if job
                .stop_reason
                .as_deref()
                .is_some_and(|r| r.ends_with("budget_exhausted"))
            {
                break;
            }
        }
        let now = self.now();
        job = store
            .mutate(id, token, |j| {
                let completed = j.queries.iter().filter(|q| q.state == "done").count();
                if j.cancel_requested || now.seconds() >= j.deadline.get() {
                    for q in &mut j.queries {
                        if ["pending", "running"].contains(&q.state.as_str()) {
                            q.state = "cancelled".into();
                        }
                    }
                    j.state = if j.cancel_requested {
                        "cancelled"
                    } else if completed > 0 {
                        "partial"
                    } else {
                        "failed"
                    }
                    .into();
                    j.stop_reason = Some(
                        if j.cancel_requested {
                            "cancelled"
                        } else {
                            "deadline_expired"
                        }
                        .into(),
                    );
                } else {
                    j.state = if j.queries.iter().all(|q| q.state == "done") {
                        "succeeded"
                    } else if completed > 0 {
                        "partial"
                    } else {
                        "failed"
                    }
                    .into();
                    if j.stop_reason.is_none() {
                        j.stop_reason = Some(
                            if j.state == "succeeded" {
                                if !dossier(j)
                                    && j.queries.len() as u64 >= j.budget_limits["queries"]
                                {
                                    "query_budget_exhausted"
                                } else {
                                    "plan_executed"
                                }
                            } else {
                                "execution_failed"
                            }
                            .into(),
                        );
                    }
                }
                Ok(())
            })
            .await?;
        Ok(job)
    }
    async fn validate_source(
        &self,
        tenant: &TenantId,
        job: &ResearchJob,
        cancellation: &SearchCancellation,
        shutdown: &watch::Receiver<bool>,
    ) -> Result<(), ssx_application::composition::Error> {
        use ssx_application::composition::Error;
        if !job.workflow.contains_key("composition") {
            return Ok(());
        }
        let validator = self.source_validator.as_ref().ok_or_else(|| {
            Error::new(
                "source_unavailable",
                "composition source validator is unavailable",
            )
        })?;
        let remaining = (job.deadline.get() - self.now().seconds()).clamp(0.0, 30.0);
        let mut stop = shutdown.clone();
        tokio::select! {biased;
            _=stopped(&mut stop)=>Err(Error::new("cancelled","source admission interrupted by shutdown")),
            _=cancellation.cancelled()=>Err(Error::new("cancelled","source admission lost worker authority")),
            result=tokio::time::timeout(Duration::from_secs_f64(remaining),validator.validate(tenant,job))=>result.unwrap_or_else(|_|Err(Error::new("source_unavailable","source admission deadline exceeded"))),
        }
    }
    async fn load_owned(
        &self,
        store: &ResearchStore,
        id: &JobId,
        token: &LeaseToken<JobId>,
    ) -> Result<ResearchJob, RunnerError> {
        read_owned(store, id, token).await
    }
}
async fn stopped(receiver: &mut watch::Receiver<bool>) {
    loop {
        if *receiver.borrow_and_update() {
            return;
        }
        if receiver.changed().await.is_err() {
            return;
        }
    }
}

fn dossier(job: &ResearchJob) -> bool {
    job.workflow.get("kind").and_then(ssx_json::Value::as_str) == Some("dependency_dossier")
}

async fn read_owned(
    store: &ResearchStore,
    id: &JobId,
    token: &LeaseToken<JobId>,
) -> Result<ResearchJob, RunnerError> {
    match store.load(id).await {
        JobRead::Found(job) if job.lease_token.as_deref() == Some(token.as_str()) => Ok(*job),
        JobRead::Found(_job) => {
            #[cfg(test)]
            if _job.lease_token.is_none()
                && let Some(probe) = store.completion_probe()
            {
                probe.read_cleared.add_permits(1);
            }
            Err(RunnerError::AuthorityLost)
        }
        JobRead::Missing => Err(StoreError::Missing.into()),
        JobRead::Malformed => Err(StoreError::Store(crate::valkey::Failure::Malformed).into()),
        JobRead::Closed => Err(StoreError::Store(crate::valkey::Failure::Closed).into()),
        JobRead::Unavailable => Err(StoreError::Store(crate::valkey::Failure::Unavailable).into()),
    }
}

async fn wait_deadline(timer: Option<tokio::time::Instant>) {
    match timer {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending::<()>().await,
    }
}

fn workflow_budget(
    job: &ResearchJob,
    default_results: u64,
) -> Result<(u64, u64, u64, u64), DomainError> {
    let Some(budget) = job.workflow.get("budget") else {
        return Ok((0, job.queries.len() as u64, 0, default_results));
    };
    let budget = budget
        .as_object()
        .ok_or(DomainError("invalid workflow budget"))?;
    let count = |field: &str, default| -> Result<u64, DomainError> {
        match budget.get(field) {
            None => Ok(default),
            Some(value) => value
                .as_u64()
                .ok_or(DomainError("invalid workflow budget counter")),
        }
    };
    if budget
        .get("admitted_results")
        .is_some_and(|value| !value.is_object())
    {
        return Err(DomainError("invalid workflow admitted results"));
    }
    Ok((
        count("used_adapter_calls", 0)?,
        count("max_adapter_calls", job.queries.len() as u64)?,
        count("captured_results", 0)?,
        count("max_results", default_results)?,
    ))
}

#[cfg(test)]
pub(crate) mod completion_tests;
