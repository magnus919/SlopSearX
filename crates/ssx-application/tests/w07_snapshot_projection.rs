//! Additive F13 projected read controls; no acquisition or alternate authority.
use ssx_application::{PortFuture, snapshot::*};
use ssx_domain::{
    QueryId, SearchResult, SearchSnapshot, SnapshotId, TenantId, WallClockExpiry,
    WallClockTimestamp,
};
use std::{
    num::NonZeroU64,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
struct Store {
    result: Result<Option<SearchSnapshot>, SnapshotFailure>,
    reads: AtomicUsize,
}
impl SnapshotRepository for Store {
    fn create<'a>(
        &'a self,
        _: &'a SearchSnapshot,
        _: NonZeroU64,
    ) -> PortFuture<'a, Result<SnapshotWrite, SnapshotFailure>> {
        Box::pin(async { panic!("readonly projection must not create") })
    }
    fn read<'a>(
        &'a self,
        _: &'a TenantId,
        _: &'a SnapshotId,
    ) -> PortFuture<'a, Result<Option<SearchSnapshot>, SnapshotFailure>> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { self.result.clone() })
    }
}
struct Clock;
impl SnapshotClock for Clock {
    fn now(&self) -> WallClockTimestamp {
        WallClockTimestamp::new(100.).unwrap()
    }
}
struct Ids;
impl SnapshotIds for Ids {
    fn next(&self) -> Result<SnapshotId, SnapshotFailure> {
        panic!("projection must not acquire/requery")
    }
}
fn tenant() -> TenantId {
    TenantId::new("tenant-a").unwrap()
}
fn id() -> SnapshotId {
    SnapshotId::new("snap-projection").unwrap()
}
fn snapshot(which: &str) -> SearchSnapshot {
    SearchSnapshot::new(
        if which == "wrong-id" {
            SnapshotId::new("other").unwrap()
        } else {
            id()
        },
        "q".into(),
        QueryId::new("query").unwrap(),
        vec![
            SearchResult {
                url: "https://example.test/0".into(),
                ..Default::default()
            },
            SearchResult::default(),
        ],
        Default::default(),
        if which == "wrong-tenant" {
            TenantId::new("tenant-b").unwrap()
        } else {
            tenant()
        },
        WallClockTimestamp::new(1.).unwrap(),
        Some(WallClockExpiry::new(if which == "expired" { 99. } else { 200. }).unwrap()),
    )
    .unwrap()
}
fn state(lookup: SnapshotLookup) -> &'static str {
    match lookup {
        SnapshotLookup::Found(_) => "found",
        SnapshotLookup::Missing => "missing",
        SnapshotLookup::Expired { .. } => "expired",
        SnapshotLookup::Unavailable => "unavailable",
    }
}
#[test]
fn default_repository_projection_preserves_single_read_and_all_service_identity_expiry_failures() {
    let mut operation = std::pin::pin!(async {
        let mut cases = vec![
            ("found", Ok(Some(snapshot("live")))),
            ("wrong-id", Ok(Some(snapshot("wrong-id")))),
            ("wrong-tenant", Ok(Some(snapshot("wrong-tenant")))),
            ("expired", Ok(Some(snapshot("expired")))),
            ("missing", Ok(None)),
        ];
        for (name, error) in [
            ("malformed", SnapshotFailure::Malformed),
            ("invalid", SnapshotFailure::InvalidInput),
            ("collision", SnapshotFailure::Collision),
            ("closed", SnapshotFailure::Closed),
            ("unavailable", SnapshotFailure::Unavailable),
            ("unknown", SnapshotFailure::CompletionUnknown),
        ] {
            cases.push((name, Err(error)));
        }
        assert_eq!(cases.len(), 11);
        for (name, result) in cases {
            let store = Arc::new(Store {
                result,
                reads: AtomicUsize::new(0),
            });
            let service = SnapshotService::new(
                store.clone(),
                Arc::new(Clock),
                Arc::new(Ids),
                tenant(),
                SnapshotTtl::new(60).unwrap(),
            );
            let before = state(service.read(&id()).await);
            assert_eq!(store.reads.load(Ordering::SeqCst), 1);
            let projected = service.read_projected(&id()).await;
            assert_eq!(store.reads.load(Ordering::SeqCst), 2);
            let mut visited = 0;
            let after = match projected {
                SnapshotProjectionLookup::Found(owner) => {
                    assert_eq!(
                        owner.summary(),
                        &SnapshotSummary::from_snapshot(&snapshot("live"))
                    );
                    owner
                        .visit_results(&mut |index, row| {
                            assert_eq!(index, visited);
                            assert_eq!(row, &snapshot("live").results()[index]);
                            visited += 1;
                        })
                        .unwrap();
                    "found"
                }
                SnapshotProjectionLookup::Missing => "missing",
                SnapshotProjectionLookup::Expired { expires_at } => {
                    assert_eq!(expires_at.timestamp().seconds(), 99.);
                    "expired"
                }
                SnapshotProjectionLookup::Unavailable => "unavailable",
            };
            assert_eq!(before, after, "{name}");
            assert_eq!(visited, if name == "found" { 2 } else { 0 });
            assert_eq!(
                store.reads.load(Ordering::SeqCst),
                2,
                "visitation reread repository"
            );
            eprintln!(
                "W07_F13_DEFAULT_PROJECTION case={name} state={after} read_calls=2 callbacks={visited}"
            );
        }
    });
    use std::future::Future as _;
    let polled = operation
        .as_mut()
        .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()));
    assert!(
        matches!(polled, std::task::Poll::Ready(())),
        "injected immediate read unexpectedly suspended"
    );
}
