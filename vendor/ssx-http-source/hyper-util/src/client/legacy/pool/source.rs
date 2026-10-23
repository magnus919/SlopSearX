//! Source HTTP/1 assignment state, owned exclusively by PoolInner's mutex.
//!
//! No IO, timers, notifications, executor work or callbacks run here. Effects
//! describe work to perform after unlocking. This is not yet wired to checkout.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const MAX_ENTRIES: usize = 100;
const MAX_IDLE: usize = 20;
const KEEPALIVE: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct MountId(pub(super) u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct EntryId(u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct WaiterId(u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Lease {
    pub(super) entry: EntryId,
    pub(super) generation: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AssignmentKind {
    Reuse,
    Connect,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Assignment {
    pub(super) lease: Lease,
    pub(super) kind: AssignmentKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RequestState {
    // Required physical close work must finish before a new P may begin.
    PendingCleanup,
    // Epoch starts only after reconciliation leaves the request unassigned.
    Queued { epoch: u64 },
    Assigned(Assignment),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RequestError {
    UnknownMount,
    ClosedMount,
}
#[derive(Debug, Default)]
pub(super) struct Effects {
    pub(super) retire: Vec<Lease>,
    pub(super) notify: Vec<WaiterId>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Reserved,
    Connecting,
    Active,
    Upgraded,
    Idle(Instant),
    Retiring,
}
#[derive(Debug)]
struct Entry<K> {
    origin: K,
    generation: u64,
    owner: Option<WaiterId>,
    state: State,
}
#[derive(Debug)]
struct Request<K> {
    origin: K,
    state: RequestState,
}
#[derive(Debug)]
struct Mount<K> {
    closed: bool,
    entries: BTreeMap<EntryId, Entry<K>>,
    requests: BTreeMap<WaiterId, Request<K>>,
}
#[derive(Debug)]
pub(super) struct SourceLedger<K> {
    mounts: BTreeMap<MountId, Mount<K>>,
    next_entry: u64,
    next_waiter: u64,
    next_epoch: u64,
}

impl<K: Eq + Clone> SourceLedger<K> {
    pub(super) fn new() -> Self {
        Self {
            mounts: BTreeMap::new(),
            next_entry: 0,
            next_waiter: 0,
            next_epoch: 0,
        }
    }

    // Registration is trusted profile configuration, never derived from requests.
    // Duplicate registration cannot reopen a closed mount.
    pub(super) fn register_mount(&mut self, id: MountId) -> bool {
        if self.mounts.contains_key(&id) {
            return false;
        }
        self.mounts.insert(
            id,
            Mount {
                closed: false,
                entries: BTreeMap::new(),
                requests: BTreeMap::new(),
            },
        );
        true
    }

    pub(super) fn request(
        &mut self,
        id: MountId,
        origin: K,
        now: Instant,
    ) -> Result<(WaiterId, Effects), RequestError> {
        let mount = self.mounts.get(&id).ok_or(RequestError::UnknownMount)?;
        if mount.closed {
            return Err(RequestError::ClosedMount);
        }
        let waiter = WaiterId(Self::next(&mut self.next_waiter));
        // Reconcile assigns, waits for required close, or arms a fresh epoch.
        self.mounts.get_mut(&id).unwrap().requests.insert(
            waiter,
            Request {
                origin,
                state: RequestState::PendingCleanup,
            },
        );
        Ok((waiter, self.reconcile(id, now)))
    }

    pub(super) fn state(&self, id: MountId, waiter: WaiterId) -> Option<RequestState> {
        self.mounts.get(&id)?.requests.get(&waiter).map(|r| r.state)
    }

    // Timer owner calls this only after actual P expiry. Stale epochs cannot
    // cancel an assignment or a newer ConnectionNotAvailable wait operation.
    pub(super) fn expire_wait(&mut self, id: MountId, waiter: WaiterId, epoch: u64) -> bool {
        let Some(mount) = self.mounts.get_mut(&id) else {
            return false;
        };
        if mount.requests.get(&waiter).map(|r| r.state) != Some(RequestState::Queued { epoch }) {
            return false;
        }
        mount.requests.remove(&waiter);
        true
    }

    pub(super) fn begin_connect(&mut self, id: MountId, lease: Lease) -> bool {
        self.assignment_kind(id, lease) == Some(AssignmentKind::Connect)
            && self.transition(id, lease, State::Reserved, State::Connecting)
    }

    pub(super) fn activate(&mut self, id: MountId, lease: Lease) -> bool {
        self.transition(id, lease, State::Connecting, State::Active)
    }

    pub(super) fn activate_reused(&mut self, id: MountId, lease: Lease) -> bool {
        self.assignment_kind(id, lease) == Some(AssignmentKind::Reuse)
            && self.transition(id, lease, State::Reserved, State::Active)
    }

    pub(super) fn release_idle(
        &mut self,
        id: MountId,
        lease: Lease,
        now: Instant,
    ) -> Option<Effects> {
        let mount = self.mounts.get_mut(&id)?;
        let entry = mount.entries.get_mut(&lease.entry)?;
        if entry.generation != lease.generation || entry.state != State::Active {
            return None;
        }
        let owner = entry.owner.take()?;
        mount.requests.remove(&owner);
        entry.state = State::Idle(now);
        Some(self.reconcile(id, now))
    }

    pub(super) fn cancel(
        &mut self,
        id: MountId,
        waiter: WaiterId,
        expected: RequestState,
        now: Instant,
    ) -> Effects {
        let mut effects = Effects::default();
        let Some(mount) = self.mounts.get_mut(&id) else {
            return effects;
        };
        if mount.requests.get(&waiter).map(|r| r.state) != Some(expected) {
            return effects;
        }
        if let Some(request) = mount.requests.remove(&waiter)
            && let RequestState::Assigned(assignment) = request.state
        {
            Self::retire(mount, assignment.lease.entry, &mut effects);
        }
        effects.append(self.reconcile(id, now));
        effects
    }

    pub(super) fn unavailable(
        &mut self,
        id: MountId,
        waiter: WaiterId,
        lease: Lease,
        now: Instant,
    ) -> Option<Effects> {
        let mount = self.mounts.get_mut(&id)?;
        let RequestState::Assigned(assignment) = mount.requests.get(&waiter)?.state else {
            return None;
        };
        if assignment.lease != lease {
            return None;
        }
        let mut effects = Effects::default();
        Self::retire(mount, lease.entry, &mut effects);
        mount.requests.get_mut(&waiter)?.state = RequestState::PendingCleanup;
        effects.append(self.reconcile(id, now));
        Some(effects)
    }

    // Only the actual resource owner may report terminal completion. Keeping the
    // retiring entry until this point prevents admission by cancellation alone.
    pub(super) fn finish_retirement(
        &mut self,
        id: MountId,
        lease: Lease,
        now: Instant,
    ) -> Option<Effects> {
        let mount = self.mounts.get_mut(&id)?;
        let entry = mount.entries.get(&lease.entry)?;
        if entry.generation != lease.generation || entry.state != State::Retiring {
            return None;
        }
        mount.entries.remove(&lease.entry);
        Some(self.reconcile(id, now))
    }

    // Ordinary HTTP upgrades retain one entry without a reusable sender.
    pub(super) fn upgrade(&mut self, id: MountId, lease: Lease) -> bool {
        let Some(mount) = self.mounts.get_mut(&id) else {
            return false;
        };
        let Some(entry) = mount.entries.get_mut(&lease.entry) else {
            return false;
        };
        if entry.generation != lease.generation || entry.state != State::Active {
            return false;
        }
        let Some(owner) = entry.owner.take() else {
            return false;
        };
        mount.requests.remove(&owner);
        entry.state = State::Upgraded;
        true
    }

    // IO termination can retire idle/upgraded entries with no request owner.
    pub(super) fn retire_connection(&mut self, id: MountId, lease: Lease) -> Option<Effects> {
        let mount = self.mounts.get_mut(&id)?;
        let entry = mount.entries.get_mut(&lease.entry)?;
        if entry.generation != lease.generation {
            return None;
        }
        if let Some(owner) = entry.owner.take() {
            mount.requests.remove(&owner);
        }
        let mut effects = Effects::default();
        Self::retire(mount, lease.entry, &mut effects);
        Some(effects)
    }

    pub(super) fn close_mount(&mut self, id: MountId) -> Effects {
        let mut effects = Effects::default();
        let Some(mount) = self.mounts.get_mut(&id) else {
            return effects;
        };
        mount.closed = true;
        effects.notify.extend(mount.requests.keys().copied());
        mount.requests.clear();
        let entries: Vec<_> = mount.entries.keys().copied().collect();
        for entry in entries {
            Self::retire(mount, entry, &mut effects);
        }
        effects
    }

    // Repeated notification/reconciliation does not renew an existing epoch.
    pub(super) fn reconcile(&mut self, id: MountId, now: Instant) -> Effects {
        let mut effects = Effects::default();
        let Some(mount) = self.mounts.get_mut(&id) else {
            return effects;
        };
        if mount.closed {
            return effects;
        }
        let expired: Vec<_> = mount
            .entries
            .iter()
            .filter_map(|(id, entry)| match entry.state {
                State::Idle(since) if now.saturating_duration_since(since) > KEEPALIVE => Some(*id),
                _ => None,
            })
            .collect();
        for entry in expired {
            Self::retire(mount, entry, &mut effects);
        }
        let idle: Vec<_> = mount
            .entries
            .iter()
            .filter_map(|(id, entry)| matches!(entry.state, State::Idle(_)).then_some(*id))
            .collect();
        let surplus = idle.len().saturating_sub(MAX_IDLE);
        for entry in idle.into_iter().take(surplus) {
            Self::retire(mount, entry, &mut effects);
        }
        let waiting: Vec<_> = mount
            .requests
            .iter()
            .filter_map(|(id, request)| {
                matches!(
                    request.state,
                    RequestState::PendingCleanup | RequestState::Queued { .. }
                )
                .then_some(*id)
            })
            .collect();
        for waiter in waiting {
            let origin = &mount.requests[&waiter].origin;
            let reusable = mount.entries.iter().find_map(|(id, entry)| {
                (entry.origin == *origin && matches!(entry.state, State::Idle(_))).then_some(*id)
            });
            let assignment = if let Some(entry_id) = reusable {
                let entry = mount.entries.get_mut(&entry_id).unwrap();
                entry.generation = entry
                    .generation
                    .checked_add(1)
                    .expect("source lease generation exhausted");
                entry.owner = Some(waiter);
                entry.state = State::Reserved;
                Some(Assignment {
                    lease: Lease {
                        entry: entry_id,
                        generation: entry.generation,
                    },
                    kind: AssignmentKind::Reuse,
                })
            } else if mount.entries.len() < MAX_ENTRIES {
                let entry_id = EntryId(Self::next(&mut self.next_entry));
                mount.entries.insert(
                    entry_id,
                    Entry {
                        origin: origin.clone(),
                        generation: 1,
                        owner: Some(waiter),
                        state: State::Reserved,
                    },
                );
                Some(Assignment {
                    lease: Lease {
                        entry: entry_id,
                        generation: 1,
                    },
                    kind: AssignmentKind::Connect,
                })
            } else {
                // A single eligible eviction is enough to make progress after
                // its real close completes. Pending retirements already do so.
                if !mount.entries.values().any(|e| e.state == State::Retiring)
                    && let Some(entry) = mount
                        .entries
                        .iter()
                        .find_map(|(id, e)| matches!(e.state, State::Idle(_)).then_some(*id))
                {
                    Self::retire(mount, entry, &mut effects);
                }
                None
            };
            let request = mount.requests.get_mut(&waiter).unwrap();
            if let Some(assignment) = assignment {
                request.state = RequestState::Assigned(assignment);
                effects.notify.push(waiter);
            } else if request.state == RequestState::PendingCleanup
                && !mount.entries.values().any(|e| e.state == State::Retiring)
            {
                request.state = RequestState::Queued {
                    epoch: Self::next(&mut self.next_epoch),
                };
                // Wake a cleanup waiter so its async owner can arm this new P.
                effects.notify.push(waiter);
            }
        }
        effects
    }

    fn assignment_kind(&self, id: MountId, lease: Lease) -> Option<AssignmentKind> {
        let mount = self.mounts.get(&id)?;
        let entry = mount.entries.get(&lease.entry)?;
        if entry.generation != lease.generation {
            return None;
        }
        let RequestState::Assigned(assignment) = mount.requests.get(&entry.owner?)?.state else {
            return None;
        };
        (assignment.lease == lease).then_some(assignment.kind)
    }

    fn transition(&mut self, id: MountId, lease: Lease, from: State, to: State) -> bool {
        let Some(entry) = self
            .mounts
            .get_mut(&id)
            .and_then(|m| m.entries.get_mut(&lease.entry))
        else {
            return false;
        };
        if entry.generation != lease.generation || entry.state != from {
            return false;
        }
        entry.state = to;
        true
    }

    fn retire(mount: &mut Mount<K>, id: EntryId, effects: &mut Effects) {
        let entry = mount.entries.get_mut(&id).unwrap();
        if entry.state == State::Retiring {
            return;
        }
        entry.state = State::Retiring;
        entry.owner = None;
        effects.retire.push(Lease {
            entry: id,
            generation: entry.generation,
        });
    }

    fn next(counter: &mut u64) -> u64 {
        *counter = counter
            .checked_add(1)
            .expect("source pool identity exhausted");
        *counter
    }
}

impl Effects {
    fn append(&mut self, other: Self) {
        self.retire.extend(other.retire);
        self.notify.extend(other.notify);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: MountId = MountId(1);

    fn setup() -> (SourceLedger<u64>, Instant) {
        let mut ledger = SourceLedger::new();
        assert!(ledger.register_mount(M));
        (ledger, Instant::now())
    }

    fn invariant(ledger: &SourceLedger<u64>) {
        for mount in ledger.mounts.values() {
            assert!(mount.entries.len() <= MAX_ENTRIES);
            assert!(
                mount
                    .entries
                    .values()
                    .filter(|e| matches!(e.state, State::Idle(_)))
                    .count()
                    <= MAX_IDLE
            );
            for (id, entry) in &mount.entries {
                assert!(entry.generation > 0);
                if let Some(owner) = entry.owner {
                    assert!(matches!(
                        entry.state,
                        State::Reserved | State::Connecting | State::Active
                    ));
                    let RequestState::Assigned(assignment) = mount.requests[&owner].state else {
                        panic!("entry owner is not assigned");
                    };
                    assert_eq!(
                        assignment.lease,
                        Lease {
                            entry: *id,
                            generation: entry.generation
                        }
                    );
                } else {
                    assert!(matches!(
                        entry.state,
                        State::Idle(_) | State::Retiring | State::Upgraded
                    ));
                }
            }
            for (waiter, request) in &mount.requests {
                match request.state {
                    RequestState::Assigned(assignment) => {
                        let entry = &mount.entries[&assignment.lease.entry];
                        assert_eq!(entry.owner, Some(*waiter));
                        assert_eq!(entry.generation, assignment.lease.generation);
                    }
                    RequestState::Queued { epoch } => assert!(epoch > 0),
                    RequestState::PendingCleanup => {
                        assert!(mount.entries.values().any(|e| e.state == State::Retiring))
                    }
                }
            }
            if mount.closed {
                assert!(mount.requests.is_empty());
            }
        }
    }

    fn assigned(ledger: &SourceLedger<u64>, mount: MountId, waiter: WaiterId) -> Assignment {
        let Some(RequestState::Assigned(assignment)) = ledger.state(mount, waiter) else {
            panic!("request was not assigned");
        };
        assignment
    }

    fn active(
        ledger: &mut SourceLedger<u64>,
        mount: MountId,
        origin: u64,
        now: Instant,
    ) -> (WaiterId, Lease) {
        let (waiter, effects) = ledger.request(mount, origin, now).unwrap();
        assert!(effects.retire.is_empty());
        let assignment = assigned(ledger, mount, waiter);
        assert_eq!(assignment.kind, AssignmentKind::Connect);
        invariant(ledger);
        assert!(ledger.begin_connect(mount, assignment.lease));
        invariant(ledger);
        assert!(ledger.activate(mount, assignment.lease));
        invariant(ledger);
        (waiter, assignment.lease)
    }

    #[test]
    fn source_pool_actual_100_entries_hold_retirement_credit() {
        let (mut ledger, now) = setup();
        let mut leases = Vec::new();
        for origin in 0..MAX_ENTRIES as u64 {
            leases.push(active(&mut ledger, M, origin, now));
        }
        // The same credit follows Active -> Idle -> Reserved without redial.
        assert!(
            ledger
                .release_idle(M, leases[0].1, now)
                .unwrap()
                .retire
                .is_empty()
        );
        let (replacement, _) = ledger.request(M, 0, now).unwrap();
        let reserved = assigned(&ledger, M, replacement);
        assert_eq!(reserved.kind, AssignmentKind::Reuse);
        assert!(!ledger.begin_connect(M, reserved.lease));
        invariant(&ledger);
        let (extra, effects) = ledger.request(M, 999, now).unwrap();
        assert!(effects.retire.is_empty());
        assert!(matches!(
            ledger.state(M, extra),
            Some(RequestState::Queued { .. })
        ));
        let (owner, lease) = leases[1];
        let expected = ledger.state(M, owner).unwrap();
        let effects = ledger.cancel(M, owner, expected, now);
        assert_eq!(effects.retire, vec![lease]);
        invariant(&ledger);
        assert_eq!(ledger.mounts[&M].entries.len(), 100);
        assert!(matches!(
            ledger.state(M, extra),
            Some(RequestState::Queued { .. })
        ));
        let effects = ledger.finish_retirement(M, lease, now).unwrap();
        assert_eq!(effects.notify, vec![extra]);
        assert_eq!(assigned(&ledger, M, extra).kind, AssignmentKind::Connect);
        assert_eq!(ledger.mounts[&M].entries.len(), 100);
        invariant(&ledger);
    }

    #[test]
    fn source_pool_global_idle_20_uses_creation_order() {
        let (mut ledger, now) = setup();
        let leases: Vec<_> = (0..21)
            .map(|origin| active(&mut ledger, M, origin, now).1)
            .collect();
        // Reverse return order distinguishes creation order from idle-return LRU.
        for lease in leases.iter().rev().take(20) {
            assert!(
                ledger
                    .release_idle(M, *lease, now)
                    .unwrap()
                    .retire
                    .is_empty()
            );
            invariant(&ledger);
        }
        let effects = ledger.release_idle(M, leases[0], now).unwrap();
        assert_eq!(effects.retire, vec![leases[0]]);
        assert_eq!(ledger.mounts[&M].entries.len(), 21);
        invariant(&ledger);
        ledger.finish_retirement(M, leases[0], now).unwrap();
        assert_eq!(ledger.mounts[&M].entries.len(), 20);
        invariant(&ledger);
    }

    #[test]
    fn source_pool_expiry_is_strict_and_lazy() {
        let (mut ledger, now) = setup();
        let (_, lease) = active(&mut ledger, M, 0, now);
        ledger.release_idle(M, lease, now).unwrap();
        assert!(ledger.reconcile(M, now + KEEPALIVE).retire.is_empty());
        invariant(&ledger);
        // No timer exists in this state machine; time alone cannot emit work.
        let later = now + KEEPALIVE + Duration::from_nanos(1);
        assert_eq!(
            ledger.mounts[&M].entries[&lease.entry].state,
            State::Idle(now)
        );
        assert_eq!(ledger.reconcile(M, later).retire, vec![lease]);
        invariant(&ledger);
    }

    #[test]
    fn source_pool_mounts_are_finite_independent_and_not_proxy_url_keys() {
        let (mut ledger, now) = setup();
        let other = MountId(2); // Distinct trusted mount even if proxy URLs match.
        assert!(ledger.register_mount(other));
        for mount in [M, other] {
            let mut leases = Vec::new();
            for origin in 0..100 {
                leases.push(active(&mut ledger, mount, origin, now).1);
            }
            for lease in leases.iter().take(20) {
                ledger.release_idle(mount, *lease, now).unwrap();
            }
            let (waiter, effects) = ledger.request(mount, 999, now).unwrap();
            assert_eq!(effects.retire, vec![leases[0]]);
            assert_eq!(
                ledger.state(mount, waiter),
                Some(RequestState::PendingCleanup)
            );
            assert_eq!(ledger.mounts[&mount].entries.len(), 100);
            invariant(&ledger);
        }
        assert_eq!(
            ledger.mounts[&M].entries.len() + ledger.mounts[&other].entries.len(),
            200
        );
        assert!(matches!(
            ledger.request(MountId(3), 0, now),
            Err(RequestError::UnknownMount)
        ));
        invariant(&ledger);
    }

    #[test]
    fn source_pool_wait_epoch_spurious_wake_and_genuine_unavailable() {
        let (mut ledger, now) = setup();
        let leases: Vec<_> = (0..100)
            .map(|origin| active(&mut ledger, M, origin, now))
            .collect();
        let (waiter, _) = ledger.request(M, 999, now).unwrap();
        let first = ledger.state(M, waiter).unwrap();
        let RequestState::Queued { epoch: first_epoch } = first else {
            panic!("expected wait");
        };
        for tick in 1..=3 {
            assert!(
                ledger
                    .reconcile(M, now + Duration::from_secs(tick))
                    .notify
                    .is_empty()
            );
            assert_eq!(ledger.state(M, waiter), Some(first));
            invariant(&ledger);
        }
        let (owner, lease) = leases[0];
        ledger.cancel(M, owner, ledger.state(M, owner).unwrap(), now);
        ledger.finish_retirement(M, lease, now).unwrap();
        let assignment = assigned(&ledger, M, waiter);
        assert!(!ledger.expire_wait(M, waiter, first_epoch));
        ledger
            .unavailable(M, waiter, assignment.lease, now)
            .unwrap();
        assert_eq!(ledger.state(M, waiter), Some(RequestState::PendingCleanup));
        // Required close precedes arming P. This request can be immediately
        // reassigned after close, so it must not manufacture another P timer.
        // A second, genuinely unassigned operation receives its own fresh epoch.
        let (earlier, _) = ledger.request(M, 998, now).unwrap();
        ledger.finish_retirement(M, assignment.lease, now).unwrap();
        assert!(matches!(
            ledger.state(M, waiter),
            Some(RequestState::Assigned(_))
        ));
        let Some(RequestState::Queued { epoch }) = ledger.state(M, earlier) else {
            panic!("expected second wait");
        };
        assert!(epoch > first_epoch);
        assert!(!ledger.expire_wait(M, earlier, first_epoch));
        assert!(ledger.expire_wait(M, earlier, epoch));
        invariant(&ledger);
    }

    #[test]
    fn source_pool_stale_generation_cannot_return_cancel_or_release() {
        let (mut ledger, now) = setup();
        let (old_waiter, old) = active(&mut ledger, M, 0, now);
        let old_state = ledger.state(M, old_waiter).unwrap();
        ledger.release_idle(M, old, now).unwrap();
        let (waiter, _) = ledger.request(M, 0, now).unwrap();
        let current = assigned(&ledger, M, waiter);
        assert_eq!(old.entry, current.lease.entry);
        assert_eq!(current.lease.generation, old.generation + 1);
        assert!(ledger.release_idle(M, old, now).is_none());
        assert!(ledger.finish_retirement(M, old, now).is_none());
        assert!(
            ledger
                .cancel(M, old_waiter, old_state, now)
                .retire
                .is_empty()
        );
        assert!(ledger.cancel(M, waiter, old_state, now).retire.is_empty());
        assert!(ledger.activate_reused(M, current.lease));
        invariant(&ledger);
    }

    #[test]
    fn source_pool_cancel_queue_assignment_and_close_hold_credits() {
        let (mut ledger, now) = setup();
        let leases: Vec<_> = (0..100)
            .map(|origin| active(&mut ledger, M, origin, now))
            .collect();
        let (queued, _) = ledger.request(M, 999, now).unwrap();
        let state = ledger.state(M, queued).unwrap();
        assert!(ledger.cancel(M, queued, state, now).retire.is_empty());
        assert_eq!(ledger.state(M, queued), None);
        let owner = leases[0].0;
        let effects = ledger.cancel(M, owner, ledger.state(M, owner).unwrap(), now);
        assert_eq!(effects.retire, vec![leases[0].1]);
        let effects = ledger.close_mount(M);
        assert_eq!(effects.retire.len(), 99);
        assert_eq!(ledger.mounts[&M].entries.len(), 100);
        assert!(!ledger.register_mount(M));
        assert!(matches!(
            ledger.request(M, 1, now),
            Err(RequestError::ClosedMount)
        ));
        invariant(&ledger);
        for (_, lease) in leases {
            ledger.finish_retirement(M, lease, now).unwrap();
            invariant(&ledger);
        }
        assert!(ledger.mounts[&M].entries.is_empty());
        assert!(ledger.close_mount(M).retire.is_empty());
    }

    #[test]
    fn source_pool_assigned_reservation_excluded_from_expiry_and_eviction() {
        let (mut ledger, now) = setup();
        let (_, old) = active(&mut ledger, M, 0, now);
        ledger.release_idle(M, old, now).unwrap();
        let (waiter, _) = ledger.request(M, 0, now).unwrap();
        let reserved = assigned(&ledger, M, waiter);
        let leases: Vec<_> = (1..=21)
            .map(|origin| active(&mut ledger, M, origin, now).1)
            .collect();
        for lease in leases.iter().take(20) {
            ledger.release_idle(M, *lease, now).unwrap();
        }
        let effects = ledger.release_idle(M, leases[20], now).unwrap();
        assert_eq!(effects.retire, vec![leases[0]]);
        assert!(
            !ledger
                .reconcile(M, now + Duration::from_secs(6))
                .retire
                .contains(&reserved.lease)
        );
        assert_eq!(
            ledger.state(M, waiter),
            Some(RequestState::Assigned(reserved))
        );
        invariant(&ledger);
    }
}

#[cfg(test)]
mod cancellation_transitions {
    use super::*;

    #[test]
    fn source_pool_reserved_and_connecting_cancel_require_terminal_release() {
        let mut ledger = SourceLedger::new();
        let mount = MountId(1);
        let now = Instant::now();
        ledger.register_mount(mount);
        for connecting in [false, true] {
            let (waiter, _) = ledger.request(mount, 0u64, now).unwrap();
            let state = ledger.state(mount, waiter).unwrap();
            let RequestState::Assigned(assignment) = state else {
                panic!("expected reservation");
            };
            if connecting {
                assert!(ledger.begin_connect(mount, assignment.lease));
            }
            assert!(!ledger.activate_reused(mount, assignment.lease));
            assert_eq!(
                ledger.cancel(mount, waiter, state, now).retire,
                vec![assignment.lease]
            );
            assert_eq!(ledger.mounts[&mount].entries.len(), 1);
            assert!(!ledger.begin_connect(mount, assignment.lease));
            assert!(!ledger.activate(mount, assignment.lease));
            assert!(ledger.release_idle(mount, assignment.lease, now).is_none());
            ledger
                .finish_retirement(mount, assignment.lease, now)
                .unwrap();
            assert!(ledger.mounts[&mount].entries.is_empty());
            assert!(
                ledger
                    .finish_retirement(mount, assignment.lease, now)
                    .is_none()
            );
        }
    }
}

#[cfg(test)]
mod cleanup_notification {
    use super::*;

    #[test]
    fn source_pool_cleanup_completion_notifies_new_queued_epoch_once() {
        let mut ledger = SourceLedger::new();
        let mount = MountId(1);
        let now = Instant::now();
        assert!(ledger.register_mount(mount));
        let mut original = Vec::new();
        for origin in 0..100u64 {
            let (waiter, _) = ledger.request(mount, origin, now).unwrap();
            let state = ledger.state(mount, waiter).unwrap();
            let RequestState::Assigned(assignment) = state else {
                panic!("expected reservation");
            };
            original.push((waiter, state, assignment.lease));
        }
        let (owner, state, retiring) = original[0];
        assert_eq!(
            ledger.cancel(mount, owner, state, now).retire,
            vec![retiring]
        );
        let (first, first_effects) = ledger.request(mount, 100, now).unwrap();
        let (second, second_effects) = ledger.request(mount, 101, now).unwrap();
        assert_eq!(
            ledger.state(mount, first),
            Some(RequestState::PendingCleanup)
        );
        assert_eq!(
            ledger.state(mount, second),
            Some(RequestState::PendingCleanup)
        );
        assert!(first_effects.notify.is_empty());
        assert!(second_effects.notify.is_empty());
        assert_eq!(ledger.mounts[&mount].entries.len(), 100);

        let effects = ledger.finish_retirement(mount, retiring, now).unwrap();
        assert!(effects.retire.is_empty());
        assert_eq!(effects.notify, vec![first, second]);
        assert!(matches!(
            ledger.state(mount, first),
            Some(RequestState::Assigned(_))
        ));
        let queued = ledger.state(mount, second).unwrap();
        let RequestState::Queued { epoch } = queued else {
            panic!("expected a fresh P epoch");
        };
        assert!(epoch > 0);
        assert_eq!(ledger.mounts[&mount].entries.len(), 100);
        assert!(
            ledger.mounts[&mount]
                .entries
                .values()
                .all(|entry| entry.state == State::Reserved)
        );
        for tick in 1..=3 {
            let effects = ledger.reconcile(mount, now + Duration::from_secs(tick));
            assert!(effects.notify.is_empty());
            assert!(effects.retire.is_empty());
            assert_eq!(ledger.state(mount, second), Some(queued));
            assert_eq!(ledger.mounts[&mount].entries.len(), 100);
        }
        assert!(!ledger.expire_wait(mount, second, epoch + 1));
        assert!(ledger.expire_wait(mount, second, epoch));
        let effects = ledger.close_mount(mount);
        assert_eq!(effects.retire.len(), 100);
        assert!(ledger.mounts[&mount].requests.is_empty());
        for lease in effects.retire {
            let effects = ledger.finish_retirement(mount, lease, now).unwrap();
            assert!(effects.notify.is_empty());
            assert!(effects.retire.is_empty());
        }
        assert!(ledger.mounts[&mount].entries.is_empty());
    }
}
