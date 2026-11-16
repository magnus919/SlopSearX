//! Owned staged execution. Unknown external outcomes remain reserved; a fenced
//! record commit does not imply exactly-once execution at any remote provider.
use crate::{
    search::{SearchCancellation, SearchService},
    valkey::{
        Failure, Valkey,
        staged::{ClaimOutcome, OwnedStage, StagedStore},
    },
};
use ssx_application::{
    policy::{PolicyService, ScopeProposal},
    snapshot::{SnapshotClock, SnapshotService},
    staged::{self, Limits},
};
use ssx_domain::{
    ArtifactKind, ArtifactRef, DomainError, LeaseToken, StagedSearchId, TenantId, staged::Record,
};
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;

pub trait StagedAuthority: Send + Sync {
    fn validate(&self, record: &Record) -> Result<(), staged::Error>;
}
/// Immutable composition. Dynamic deployments must supply their live authority;
/// this implementation never claims hot-reload behavior.
pub struct StaticStagedAuthority {
    pub policy: PolicyService,
    pub limits: Limits,
}
impl StagedAuthority for StaticStagedAuthority {
    fn validate(&self, r: &Record) -> Result<(), staged::Error> {
        staged::revalidate(&self.policy, &self.limits, r)
    }
}
#[derive(Debug)]
pub enum Error {
    Store(Failure),
    Invalid(DomainError),
    LeaseLost,
    Shutdown,
    Task,
    Policy(staged::Error),
}
impl From<Failure> for Error {
    fn from(e: Failure) -> Self {
        Self::Store(e)
    }
}
impl From<DomainError> for Error {
    fn from(e: DomainError) -> Self {
        Self::Invalid(e)
    }
}
/// Events are emitted only from acknowledged transitions. W09 owns concrete O01
/// metric composition; delivery is best effort, not a durable metrics outbox.
#[derive(Debug, Clone)]
pub struct Event {
    pub from: Option<&'static str>,
    pub to: String,
    pub record: Record,
}
pub trait Events: Send + Sync {
    fn acknowledged(&self, event: Event);
}
pub struct NoEvents;
impl Events for NoEvents {
    fn acknowledged(&self, _: Event) {}
}
#[derive(Clone)]
pub struct StagedRunner {
    pub store: Arc<Valkey>,
    pub search: SearchService,
    pub snapshots: SnapshotService,
    pub authority: Arc<dyn StagedAuthority>,
    pub clock: Arc<dyn SnapshotClock>,
    pub events: Arc<dyn Events>,
    renew_every: Duration,
}
struct CancelOnDrop(watch::Sender<bool>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.send_replace(true);
    }
}
fn random_id() -> Result<String, Error> {
    let mut bytes = [0; 32];
    aws_lc_rs::rand::fill(&mut bytes)
        .map_err(|_| Error::Invalid(DomainError("operating system randomness unavailable")))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
impl StagedRunner {
    pub fn new(
        store: Arc<Valkey>,
        search: SearchService,
        snapshots: SnapshotService,
        authority: Arc<dyn StagedAuthority>,
        clock: Arc<dyn SnapshotClock>,
        events: Arc<dyn Events>,
    ) -> Self {
        Self {
            store,
            search,
            snapshots,
            authority,
            clock,
            events,
            renew_every: Duration::from_secs(20),
        }
    }
    pub fn with_renewal_interval(mut self, interval: Duration) -> Result<Self, Error> {
        if interval.is_zero() || interval > Duration::from_secs(20) {
            return Err(DomainError("invalid staged renewal interval").into());
        }
        self.renew_every = interval;
        Ok(self)
    }
    /// Dropping this caller signals the owned task, which cancels and joins its
    /// acquisition. Shared SearchService flights owned by other callers survive.
    pub async fn run_one(
        &self,
        tenant: TenantId,
        id: StagedSearchId,
        shutdown: watch::Receiver<bool>,
    ) -> Result<Option<Record>, Error> {
        let (cancel, local) = watch::channel(false);
        let guard = CancelOnDrop(cancel);
        let runner = self.clone();
        let task = tokio::spawn(async move { runner.execute(tenant, id, shutdown, local).await });
        let result = task.await.map_err(|_| Error::Task)?;
        drop(guard);
        result
    }
    async fn execute(
        &self,
        tenant: TenantId,
        id: StagedSearchId,
        mut shutdown: watch::Receiver<bool>,
        mut local: watch::Receiver<bool>,
    ) -> Result<Option<Record>, Error> {
        let store = StagedStore::new(&self.store);
        let attempt_id = random_id()?;
        let prepared = tokio::select! {biased;
            _=stopped(&mut shutdown)=>return Err(Error::Shutdown),
            _=stopped(&mut local)=>return Err(Error::Shutdown),
            result=store.prepare_claim(&tenant,&id,LeaseToken::new(random_id()?)?,&attempt_id,self.clock.now())=>result?,
        };
        let Some(prepared) = prepared else {
            return Ok(None);
        };
        let outcome = tokio::select! {biased;
            _=stopped(&mut shutdown)=>return Err(Error::Shutdown),
            _=stopped(&mut local)=>return Err(Error::Shutdown),
            result=store.claim_prepared_outcome(prepared,self.clock.now())=>result?,
        };
        let owned = match outcome {
            ClaimOutcome::Claimed(owned) => {
                self.events.acknowledged(Event {
                    from: Some("queued"),
                    to: "running".into(),
                    record: owned.record().clone(),
                });
                owned
            }
            ClaimOutcome::Transition(transition) => {
                if matches!(
                    transition.status,
                    crate::valkey::scripts::Staged::Deadline
                        | crate::valkey::scripts::Staged::Budget
                ) && let Some(record) = transition.record
                {
                    self.events.acknowledged(Event {
                        from: Some("queued"),
                        to: "failed".into(),
                        record,
                    });
                }
                return Ok(None);
            }
        };
        let owned = Arc::new(owned);
        // Keep renewal independent from blocked search/snapshot/save I/O. Every
        // unknown/negative renewal cancels work; never retry an ambiguous write.
        let (finish, mut done) = watch::channel(false);
        let (lost, mut lease_lost) = watch::channel(false);
        let backend = self.store.clone();
        let renewal_owned = owned.clone();
        let clock = self.clock.clone();
        let interval = self.renew_every;
        let renewal = tokio::spawn(async move {
            let mut timer = tokio::time::interval(interval);
            timer.tick().await;
            loop {
                tokio::select! {biased;_=stopped(&mut done)=>break,_=timer.tick()=>{let store=StagedStore::new(&backend);let result=tokio::select!{biased;_=stopped(&mut done)=>break,r=store.renew(&renewal_owned,clock.now())=>r};if !matches!(result,Ok(true)){lost.send_replace(true);break;}}}
            }
        });
        let result = self
            .execute_owned(&owned, &mut shutdown, &mut local, &mut lease_lost)
            .await;
        finish.send_replace(true);
        let _ = renewal.await;
        result.map(Some)
    }
    async fn execute_owned(
        &self,
        owned: &OwnedStage,
        shutdown: &mut watch::Receiver<bool>,
        local: &mut watch::Receiver<bool>,
        lost: &mut watch::Receiver<bool>,
    ) -> Result<Record, Error> {
        let original = owned.record();
        let initial_failure = self.authority.validate(original).err();
        if let Some(rejection) = initial_failure {
            let next = staged::policy_rejected(
                original,
                &rejection.value()["error"],
                self.clock.now().seconds(),
            )?;
            return self
                .publish(owned, next, shutdown, local, lost, false)
                .await;
        }
        let remaining = self.remaining(original);
        if remaining == 0 {
            let next = staged::complete(
                original,
                None,
                None,
                self.clock.now().seconds(),
                Some(("deadline_exceeded", "deadline expired before dispatch")),
            )?;
            return self.publish(owned, next, shutdown, local, lost, true).await;
        }
        let store = StagedStore::new(&self.store);
        let renewed = tokio::select! {biased;_=stopped(shutdown)=>return Err(Error::Shutdown),_=stopped(local)=>return Err(Error::Shutdown),_=stopped(lost)=>return Err(Error::LeaseLost),r=store.renew(owned,self.clock.now())=>r?};
        if !renewed {
            return Err(Error::LeaseLost);
        }
        self.authority.validate(original).map_err(Error::Policy)?;
        let remaining = self.remaining(original);
        if remaining == 0 {
            let next = staged::complete(
                original,
                None,
                None,
                self.clock.now().seconds(),
                Some(("deadline_exceeded", "deadline expired before dispatch")),
            )?;
            return self.publish(owned, next, shutdown, local, lost, true).await;
        }
        let cancellation = SearchCancellation::default();
        let snapshots = self.snapshots.for_tenant(original.tenant());
        let parents = [ArtifactRef::new(
            ArtifactKind::StagedSearch,
            original.id().as_str(),
        )?];
        let search = self.search.search_snapshot_with_cancellation(
            ScopeProposal::generic(staged::dispatch(original, remaining)),
            &snapshots,
            &parents,
            cancellation.clone(),
        );
        tokio::pin!(search);
        let response = tokio::select! {biased;
            _=stopped(shutdown)=>{cancellation.cancel();let _=search.await;return Err(Error::Shutdown)},
            _=stopped(local)=>{cancellation.cancel();let _=search.await;return Err(Error::Shutdown)},
            _=stopped(lost)=>{cancellation.cancel();let _=search.await;return Err(Error::LeaseLost)},
            r=&mut search=>r,
        };
        self.authority.validate(original).map_err(Error::Policy)?;
        let now = self.clock.now().seconds();
        let next = match response {
            Ok(captured) => {
                let mut response = captured.canonical.response().clone();
                response.cached = captured.response.cached;
                staged::complete(
                    original,
                    Some(&response),
                    captured.snapshot.as_ref().ok().map(|id| id.as_str()),
                    now,
                    None,
                )?
            }
            Err(_) => staged::complete(
                original,
                None,
                None,
                now,
                Some(("execution_failed", "shared search acquisition failed")),
            )?,
        };
        self.publish(owned, next, shutdown, local, lost, true).await
    }
    fn remaining(&self, r: &Record) -> u64 {
        ((r.value()["execution_deadline_at"].as_f64().unwrap() - self.clock.now().seconds())
            * 1000.)
            .max(0.) as u64
    }
    async fn publish(
        &self,
        owned: &OwnedStage,
        next: Record,
        shutdown: &mut watch::Receiver<bool>,
        local: &mut watch::Receiver<bool>,
        lost: &mut watch::Receiver<bool>,
        check_policy: bool,
    ) -> Result<Record, Error> {
        if check_policy {
            self.authority
                .validate(owned.record())
                .map_err(Error::Policy)?;
        }
        let store = StagedStore::new(&self.store);
        let saved = tokio::select! {biased;_=stopped(shutdown)=>return Err(Error::Shutdown),_=stopped(local)=>return Err(Error::Shutdown),_=stopped(lost)=>return Err(Error::LeaseLost),r=store.save_owned(owned,&next,self.clock.now())=>r?};
        if !saved {
            return Err(Error::LeaseLost);
        }
        self.events.acknowledged(Event {
            from: Some("running"),
            to: next.state().into(),
            record: next.clone(),
        });
        Ok(next)
    }
    /// Bounded discovery never authorizes execution. Durable records and lease
    /// fencing remain authoritative after index loss and process restart.
    pub async fn run(
        &self,
        poll: Duration,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), Error> {
        if poll.is_zero() || tokio::time::Instant::now().checked_add(poll).is_none() {
            return Err(DomainError("invalid staged polling interval").into());
        }
        let mut cursor = 0;
        loop {
            if *shutdown.borrow() {
                return Ok(());
            }
            let store = StagedStore::new(&self.store);
            let candidate = tokio::select! {biased;_=stopped(&mut shutdown)=>return Ok(()),r=async{let acknowledge=|record:Record|self.events.acknowledged(Event{from:Some("running"),to:"interrupted".into(),record});cursor=store.reconcile_indexes_acknowledged(cursor,self.clock.now(),acknowledge).await?;store.recover_orphans_acknowledged(self.clock.now(),acknowledge).await?;store.claim_next(self.clock.now()).await}=>r};
            if let Ok(Some((tenant, id))) = candidate {
                let _ = self.run_one(tenant, id, shutdown.clone()).await;
                continue;
            }
            tokio::select! {biased;_=stopped(&mut shutdown)=>return Ok(()),_=tokio::time::sleep(poll)=>()}
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
