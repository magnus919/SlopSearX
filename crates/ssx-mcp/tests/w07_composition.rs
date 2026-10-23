//! Component-only evidence graph. No MCP transport/tool/production activation.
#[path = "t06_support/server.rs"]
mod private_valkey;
use sha2::{Digest, Sha256};
use ssx_application::{
    lineage::{
        Resolver,
        resolver::{Entities, ProjectedEntities},
    },
    policy::{PolicyService, SearchPolicy},
    receipts::{
        Cancellation,
        observation::ObservationInput,
        service::{Access, Clock},
    },
    saved::SavedLimits,
    scope::{CatalogSnapshot, ScopeResolver},
    snapshot::{SnapshotProjectionLookup, SnapshotRepository, SnapshotService, SnapshotTtl},
    staged,
};
use ssx_domain::{
    ArtifactKind, ArtifactRef, QueryId, SearchResult, SearchSnapshot, SnapshotId, TenantId,
    WallClockExpiry, WallClockTimestamp,
};
use ssx_infra::{
    search::SearchClock,
    valkey::{
        Valkey,
        snapshot::{RandomSnapshotIds, SystemSnapshotClock, ValkeySnapshots},
    },
};
use ssx_mcp::evidence::{
    EvidenceAccess, EvidenceClock, EvidenceEntities, EvidenceInputs, EvidenceRuntime,
    authority::{EvidenceAuthoritySnapshot, EvidenceOwner},
};
use std::{
    collections::BTreeMap,
    num::NonZeroU64,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
struct Captured(AtomicUsize);
impl SearchClock for Captured {
    fn capture(&self) -> (WallClockTimestamp, String) {
        self.0.fetch_add(1, Ordering::SeqCst);
        (
            WallClockTimestamp::new(1000.0).unwrap(),
            "1970-01-01".into(),
        )
    }
}
fn state() -> EvidenceAuthoritySnapshot {
    let mut policy = SearchPolicy::default();
    policy.grants.insert("retrieval_receipts".into());
    EvidenceAuthoritySnapshot {
        search: PolicyService {
            policy,
            catalog: CatalogSnapshot::new(vec![]).unwrap(),
            resolver: ScopeResolver::default(),
        },
        saved_limits: SavedLimits::default(),
        staged_limits: staged::Limits::default(),
        auth_configured: BTreeMap::new(),
        valkey_connected: true,
    }
}
fn server() -> private_valkey::Server {
    let path =
        std::env::var("SLOPSEARX_TEST_VALKEY_SERVER").expect("explicit qualified private Valkey");
    let expected =
        std::env::var("SLOPSEARX_TEST_VALKEY_SERVER_SHA256").expect("pinned Valkey digest");
    let metadata = std::fs::symlink_metadata(&path).unwrap();
    assert!(metadata.is_file() && !metadata.file_type().is_symlink());
    let actual: String = Sha256::digest(std::fs::read(path).unwrap())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(actual, expected);
    assert!(
        std::env::temp_dir().starts_with("/Volumes/tank01/magnus/rust-builds/slopsearx")
            || !cfg!(target_os = "macos")
    );
    private_valkey::Server::start()
}
fn observation(key: &str) -> ObservationInput {
    ObservationInput {
        retriever: "component".into(),
        idempotency_key: key.into(),
        status: "failed".into(),
        failure_code: Some("not_found".into()),
        ..Default::default()
    }
}
#[tokio::test]
async fn same_backend_readiness_terminal_close_and_single_clock_capture() {
    let server = server();
    let backend = Arc::new(Valkey::new(server.url(), Duration::from_secs(2)).unwrap());
    let owner = EvidenceOwner::new(state(), backend.clone()).unwrap();
    let access = EvidenceAccess {
        authority: owner.authority(),
        backend: backend.clone(),
    };
    assert!(
        !access.connected(),
        "configured URL must not imply startup readiness"
    );
    assert!(access.enabled());
    owner.establish_readiness().await.unwrap();
    assert!(access.connected());
    let clock = Arc::new(Captured(AtomicUsize::new(0)));
    let adapter = EvidenceClock {
        clock: clock.clone(),
    };
    assert_eq!(adapter.now().seconds(), 1000.0);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    assert!(access.connected());
    assert_eq!(
        clock.0.load(Ordering::SeqCst),
        1,
        "readiness/authority must not capture clock"
    );
    owner.stop_admission();
    assert!(!access.connected());
    assert!(matches!(
        owner.establish_readiness().await,
        Err(ssx_infra::valkey::Failure::Closed)
    ));
    backend.close().await;
    drop(owner);
    drop(backend);
    drop(server);
}
#[tokio::test]
async fn real_snapshot_receipt_replay_conflict_tenant_and_projected_entity_graph() {
    let server = server();
    let backend = Arc::new(Valkey::new(server.url(), Duration::from_secs(2)).unwrap());
    let owner = EvidenceOwner::new(state(), backend.clone()).unwrap();
    owner.establish_readiness().await.unwrap();
    let tenant = TenantId::new("org:team").unwrap();
    let repository = Arc::new(ValkeySnapshots::new(backend.clone()));
    let snapshot = SearchSnapshot::new(
        SnapshotId::new("snap-component").unwrap(),
        "q".into(),
        QueryId::new("query-component").unwrap(),
        vec![SearchResult {
            title: "retained".into(),
            url: "https://example.test/no-fetch".into(),
            ..Default::default()
        }],
        Default::default(),
        tenant.clone(),
        WallClockTimestamp::new(1000.0).unwrap(),
        Some(WallClockExpiry::new(4102444800.0).unwrap()),
    )
    .unwrap();
    repository
        .create(&snapshot, NonZeroU64::new(3600).unwrap())
        .await
        .unwrap();
    let snapshots = SnapshotService::new(
        repository,
        Arc::new(SystemSnapshotClock),
        Arc::new(RandomSnapshotIds),
        tenant.clone(),
        SnapshotTtl::new(3600).unwrap(),
    );
    let graph = EvidenceRuntime::new(EvidenceInputs {
        backend: backend.clone(),
        snapshots: snapshots.clone(),
        authority: owner.authority(),
        clock: Arc::new(Captured(AtomicUsize::new(0))),
    });
    let cancel = Cancellation::default();
    let first = graph
        .service
        .submit(&tenant, "snap-component:0", observation("once"), &cancel)
        .await
        .unwrap();
    assert!(first.get("error").is_none(), "{first}");
    let replay = graph
        .service
        .submit(&tenant, "snap-component:0", observation("once"), &cancel)
        .await
        .unwrap();
    assert_eq!(first["state"], "created");
    assert_eq!(replay["state"], "replayed");
    assert_eq!(first["receipt"], replay["receipt"]);
    let mut changed = observation("once");
    changed.failure_code = Some("different".into());
    let conflict = graph
        .service
        .submit(&tenant, "snap-component:0", changed, &cancel)
        .await
        .unwrap();
    assert_eq!(conflict["error"]["code"], "idempotency_conflict");
    let foreign = graph
        .service
        .read(
            &TenantId::new("other").unwrap(),
            "snap-component:0",
            20,
            &cancel,
        )
        .await
        .unwrap();
    assert_eq!(foreign["error"]["code"], "invalid_result_id");
    let read = graph
        .service
        .read(&tenant, "snap-component:0", 20, &cancel)
        .await
        .unwrap();
    assert_eq!(read["total"], 1);
    let projected = match snapshots.read_projected(snapshot.id()).await {
        SnapshotProjectionLookup::Found(s) => s,
        _ => panic!("stored projection missing"),
    };
    for version in [1, 2] {
        let full = Entities::groups(&EvidenceEntities, &snapshot, version);
        let projected = ProjectedEntities::groups(&EvidenceEntities, &projected, version).unwrap();
        assert_eq!(format!("{full:?}"), format!("{projected:?}"));
    }
    let resolved = graph
        .lineage
        .resolve(
            &tenant,
            &ArtifactRef::new(ArtifactKind::Snapshot, snapshot.id().as_str()).unwrap(),
            &cancel,
        )
        .await
        .unwrap();
    assert_eq!(resolved.node.status, ssx_application::lineage::Status::Live);
    cancel.cancel();
    assert!(matches!(
        graph
            .service
            .read(&tenant, "snap-component:0", 20, &cancel)
            .await,
        Err(ssx_application::receipts::Failure::Cancelled)
    ));
    owner.stop_admission();
    drop(graph);
    drop(projected);
    drop(snapshots);
    backend.close().await;
    drop(owner);
    drop(backend);
    drop(server);
}

#[tokio::test]
async fn full_and_f13_projected_entity_ports_match_every_frozen_corpus_case() {
    let server = server();
    let backend = Arc::new(Valkey::new(server.url(), Duration::from_secs(2)).unwrap());
    let repository = Arc::new(ValkeySnapshots::new(backend.clone()));
    let tenant = TenantId::new("entity-corpus").unwrap();
    let snapshots = SnapshotService::new(
        repository.clone(),
        Arc::new(SystemSnapshotClock),
        Arc::new(RandomSnapshotIds),
        tenant.clone(),
        SnapshotTtl::new(3600).unwrap(),
    );
    let corpus: ssx_json::Value =
        ssx_json::from_str(include_str!("../../../docs/rust-rewrite/t06/entities.json")).unwrap();
    let mut resolved = 0;
    for (i, row) in corpus["cases"].as_array().unwrap().iter().enumerate() {
        let snapshot = SearchSnapshot::new(
            SnapshotId::new(format!("entity-corpus-{i}")).unwrap(),
            "q".into(),
            QueryId::new("q1").unwrap(),
            row["results"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| ssx_domain::storage::decode_result(v.clone()).unwrap())
                .collect(),
            Default::default(),
            tenant.clone(),
            WallClockTimestamp::new(1000.).unwrap(),
            Some(WallClockExpiry::new(4102444800.).unwrap()),
        )
        .unwrap();
        repository
            .create(&snapshot, NonZeroU64::new(3600).unwrap())
            .await
            .unwrap();
        let projected = match snapshots.read_projected(snapshot.id()).await {
            SnapshotProjectionLookup::Found(s) => s,
            _ => panic!("missing projected corpus row"),
        };
        let version = row["version"].as_u64().unwrap() as u8;
        let full = Entities::groups(&EvidenceEntities, &snapshot, version);
        let streamed = ProjectedEntities::groups(&EvidenceEntities, &projected, version).unwrap();
        let identity = |groups: Vec<ssx_application::lineage::resolver::EntityGroup>| {
            groups
                .into_iter()
                .map(|g| (g.id, g.namespace, g.result_indices))
                .collect::<Vec<_>>()
        };
        resolved += full.len();
        assert_eq!(
            identity(full),
            identity(streamed),
            "corpus case {}",
            row["case"]
        );
    }
    assert!(
        resolved > 0,
        "must exercise explicit identifiers, not only unresolved rows"
    );
    drop(snapshots);
    backend.close().await;
    drop(backend);
    drop(server);
}
