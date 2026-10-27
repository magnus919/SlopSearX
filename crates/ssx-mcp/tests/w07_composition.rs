//! Component-only evidence graph. No MCP transport/tool/production activation.
#[path = "t06_support/server.rs"]
mod private_valkey;
use sha2::{Digest, Sha256};
use ssx_application::{
    lineage::resolver::{Entities, ProjectedEntities},
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
        metrics: Arc::new(ssx_application::NoopSink),
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

struct LogicalClock(std::sync::atomic::AtomicU64);
impl SearchClock for LogicalClock {
    fn capture(&self) -> (WallClockTimestamp, String) {
        (
            ssx_application::snapshot::SnapshotClock::now(self),
            "1970-01-01".into(),
        )
    }
}
impl ssx_application::snapshot::SnapshotClock for LogicalClock {
    fn now(&self) -> WallClockTimestamp {
        WallClockTimestamp::new(self.0.load(Ordering::SeqCst) as f64).unwrap()
    }
}
fn stored_snapshot(server: &private_valkey::Server, tenant: &TenantId, id: &str) -> (Vec<u8>, i64) {
    let mut conn = server.connection();
    let key = ssx_infra::valkey::keys::snapshot(tenant, id);
    (
        redis::cmd("DUMP").arg(&key).query(&mut conn).unwrap(),
        redis::cmd("PTTL").arg(&key).query(&mut conn).unwrap(),
    )
}
fn unchanged_snapshot(before: &(Vec<u8>, i64), after: &(Vec<u8>, i64)) {
    assert_eq!(before.0, after.0);
    assert!(
        after.1 > 0 && after.1 <= before.1,
        "snapshot retention was renewed"
    );
}
#[tokio::test]
async fn retained_snapshot_original_bounds_foreign_legacy_and_source_expiry_contracts() {
    use ssx_application::{
        lineage::{self, Direction, Request},
        receipts::service::ManifestDraft,
    };
    use ssx_json::{Value, json};
    let server = server();
    let backend = Arc::new(Valkey::new(server.url(), Duration::from_secs(2)).unwrap());
    let telemetry = ssx_infra::observability::Telemetry::new(
        ssx_infra::observability::Options {
            audit: false,
            ..Default::default()
        },
        None,
        vec!["wikipedia".into()],
        vec!["general".into()],
    )
    .unwrap();
    let expiry_series = || {
        telemetry
            .metrics
            .render()
            .lines()
            .filter(|line| line.starts_with("slopsearx_workflow_expired_total{"))
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let mut current = state();
    current.search.policy.grants.insert("research".into());
    current.search.catalog = CatalogSnapshot::new(vec![ssx_application::scope::EngineCapability {
        name: "wikipedia".into(),
        enabled: true,
        active: true,
        available: true,
        categories: vec!["general".into()],
        media_types: vec![],
        auth_ready: true,
        circuit_open: false,
        cost_class: "free".into(),
        health_status: "unknown".into(),
        health_stale: false,
        enforced_filters: BTreeMap::new(),
    }])
    .unwrap();
    current.auth_configured.insert("wikipedia".into(), false);
    let owner = EvidenceOwner::new(current, backend.clone()).unwrap();
    owner.establish_readiness().await.unwrap();
    let tenant = TenantId::new("source-contract").unwrap();
    let clock = Arc::new(LogicalClock(std::sync::atomic::AtomicU64::new(1000)));
    let repository = Arc::new(ValkeySnapshots::new(backend.clone()));
    let parent = ArtifactRef::new(
        ArtifactKind::ResearchAttempt,
        ssx_domain::canonical::composite_artifact_id(&["parent-job", "attempt-1"]).unwrap(),
    )
    .unwrap();
    let root = ArtifactRef::new(ArtifactKind::Snapshot, "source-contract").unwrap();
    let parent_edge =
        json!({"from":root.to_value(),"relation":"derived_from","to":parent.to_value()});
    let snapshot = SearchSnapshot::new(
        SnapshotId::new("source-contract").unwrap(),
        "q".into(),
        QueryId::new("source-query").unwrap(),
        vec![
            SearchResult {
                title: "one".into(),
                url: "https://example.test/one".into(),
                ..Default::default()
            },
            SearchResult {
                title: "two".into(),
                url: "https://example.test/two".into(),
                ..Default::default()
            },
        ],
        ssx_domain::ScopeDecision {
            selected_engines: vec!["wikipedia".into()],
            ..Default::default()
        },
        tenant.clone(),
        WallClockTimestamp::new(1000.).unwrap(),
        Some(WallClockExpiry::new(1010.).unwrap()),
    )
    .unwrap()
    .with_lineage(
        "tier_then_cross_engine_presence".into(),
        vec![parent_edge.clone()],
        true,
    );
    repository
        .create(&snapshot, NonZeroU64::new(3600).unwrap())
        .await
        .unwrap();
    let snapshots = SnapshotService::new(
        repository,
        clock.clone(),
        Arc::new(RandomSnapshotIds),
        tenant.clone(),
        SnapshotTtl::new(3600).unwrap(),
    );
    let second = SnapshotService::new(
        Arc::new(ValkeySnapshots::new(backend.clone())),
        clock.clone(),
        Arc::new(RandomSnapshotIds),
        tenant.clone(),
        SnapshotTtl::new(3600).unwrap(),
    );
    let job = ssx_domain::research::ResearchJob::from_payload(json!({
        "job_id":"parent-job","tenant":tenant.as_str(),"question":"q","strategy":"broad",
        "state":"succeeded","created_at":1000,"deadline":2000,
        "queries":[{"index":0,"query":"q","intent":"general","engines":["wikipedia"],
            "state":"done","cursor":"source-contract","result_count":2,
            "attempts":[{"attempt_id":"attempt-1","state":"done","attempted_at":1000,
                "finished_at":1001,"cursor":"source-contract","result_count":2}]}]
    }))
    .unwrap();
    let job_root = ArtifactRef::new(ArtifactKind::ResearchJob, "parent-job").unwrap();
    let summary = ssx_infra::research_planning::job_summary(&job);
    assert_eq!(summary["artifact"], job_root.to_value());
    assert_eq!(
        summary["queries"][0]["attempts"][0]["artifact"],
        parent.to_value()
    );
    assert_eq!(
        summary["queries"][0]["attempts"][0]["snapshot_artifact"],
        root.to_value()
    );
    let job_key = ssx_infra::valkey::keys::research("job", &tenant, "parent-job").unwrap();
    let mut conn = server.connection();
    let _: () = redis::cmd("SET")
        .arg(&job_key)
        .arg(ssx_json::to_vec(&job.to_payload().unwrap()).unwrap())
        .arg("PX")
        .arg(3_600_000)
        .query(&mut conn)
        .unwrap();
    let job_before: (Vec<u8>, i64) = (
        redis::cmd("DUMP").arg(&job_key).query(&mut conn).unwrap(),
        redis::cmd("PTTL").arg(&job_key).query(&mut conn).unwrap(),
    );
    let before = stored_snapshot(&server, &tenant, root.id());
    let independent = match second.read(snapshot.id()).await {
        ssx_application::snapshot::SnapshotLookup::Found(s) => s,
        _ => panic!("second snapshot reader lost retained source"),
    };
    assert_eq!(independent.lineage(), std::slice::from_ref(&parent_edge));
    unchanged_snapshot(&before, &stored_snapshot(&server, &tenant, root.id()));
    let runtime = EvidenceRuntime::new(EvidenceInputs {
        backend: backend.clone(),
        snapshots: snapshots.clone(),
        authority: owner.authority(),
        clock: clock.clone(),
        metrics: telemetry.clone(),
    });
    let cancel = Cancellation::default();
    let job_graph = lineage::traverse(
        runtime.lineage.as_ref(),
        &tenant,
        Request {
            root: job_root.clone(),
            direction: Direction::Incoming,
            max_depth: 2,
            max_nodes: 100,
        },
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(
        job_graph
            .nodes
            .iter()
            .map(|n| (&n.artifact, n.status))
            .collect::<Vec<_>>(),
        vec![
            (&job_root, lineage::Status::Live),
            (&parent, lineage::Status::Live),
            (&root, lineage::Status::Live)
        ]
    );
    assert_eq!(
        job_graph
            .edges
            .iter()
            .map(|e| e.to_value())
            .collect::<Vec<_>>(),
        vec![
            json!({"from":parent.to_value(),"relation":"derived_from","to":job_root.to_value()}),
            parent_edge.clone(),
        ]
    );
    let job_after: (Vec<u8>, i64) = (
        redis::cmd("DUMP").arg(&job_key).query(&mut conn).unwrap(),
        redis::cmd("PTTL").arg(&job_key).query(&mut conn).unwrap(),
    );
    unchanged_snapshot(&job_before, &job_after);
    let cut = lineage::traverse(
        runtime.lineage.as_ref(),
        &tenant,
        Request {
            root: root.clone(),
            direction: Direction::Incoming,
            max_depth: 1,
            max_nodes: 2,
        },
        &cancel,
    )
    .await
    .unwrap();
    let result0 = ArtifactRef::new(ArtifactKind::Result, "source-contract:0").unwrap();
    let result1 = ArtifactRef::new(ArtifactKind::Result, "source-contract:1").unwrap();
    assert_eq!(
        cut.nodes.iter().map(|n| &n.artifact).collect::<Vec<_>>(),
        vec![&root, &result0]
    );
    assert!(cut.truncated);
    assert_eq!(
        cut.edges.iter().map(|e| e.to_value()).collect::<Vec<_>>(),
        vec![json!({"from":result0.to_value(),"relation":"derived_from","to":root.to_value()})]
    );
    assert!(
        cut.edges
            .iter()
            .all(|e| cut.nodes.iter().any(|n| n.artifact == e.from)
                && cut.nodes.iter().any(|n| n.artifact == e.to))
    );
    let foreign = lineage::traverse(
        runtime.lineage.as_ref(),
        &TenantId::new("foreign").unwrap(),
        Request {
            root: root.clone(),
            direction: Direction::Both,
            max_depth: 2,
            max_nodes: 100,
        },
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(foreign.nodes.len(), 1);
    assert_eq!(
        foreign.nodes[0].to_value(),
        json!({"artifact":root.to_value(),"status":"missing","created_at":null,"expires_at":null})
    );
    assert!(foreign.edges.is_empty());
    let manifest = runtime
        .service
        .manifest_draft(
            &tenant,
            ManifestDraft {
                result_ids: Some(vec![result0.id().into()]),
                artifacts: Some(vec![Ok(root.clone())]),
                source: None,
                max_depth: 2,
                max_nodes: 100,
            },
            &cancel,
        )
        .await
        .unwrap();
    assert_eq!(manifest["lineage_cuts"][0]["root"], root.to_value());
    assert_eq!(
        manifest["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["result_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![result0.id(), result1.id()]
    );
    unchanged_snapshot(&before, &stored_snapshot(&server, &tenant, root.id()));
    let live_empty = runtime
        .service
        .read(&tenant, result1.id(), 20, &cancel)
        .await
        .unwrap();
    assert_eq!(live_empty["source_snapshot_status"], "live");
    assert_eq!(live_empty["receipts"], json!([]));
    assert!(expiry_series().is_empty());
    let first = runtime
        .service
        .submit(&tenant, result0.id(), observation("expiry-once"), &cancel)
        .await
        .unwrap();
    assert_eq!(first["state"], "created");
    clock.0.store(1011, Ordering::SeqCst);
    let replay = runtime
        .service
        .submit(&tenant, result0.id(), observation("expiry-once"), &cancel)
        .await
        .unwrap();
    assert_eq!(replay["state"], "replayed");
    assert_eq!(replay["receipt"], first["receipt"]);
    let new_receipt = runtime
        .service
        .submit(&tenant, result0.id(), observation("expiry-new"), &cancel)
        .await
        .unwrap();
    assert_eq!(
        new_receipt["error"],
        json!({"code":"expired_handle","message":"originating snapshot has expired","expires_at":1010.})
    );
    let read = runtime
        .service
        .read(&tenant, result0.id(), 20, &cancel)
        .await
        .unwrap();
    assert_eq!(read["source_snapshot_status"], "expired");
    assert_eq!(
        read["receipts"][0]["receipt_id"],
        first["receipt"]["receipt_id"]
    );
    assert_eq!(
        read["receipts"][0]["discovery"],
        first["receipt"]["discovery"]
    );
    unchanged_snapshot(&before, &stored_snapshot(&server, &tenant, root.id()));
    assert!(
        expiry_series().is_empty(),
        "retained expired evidence is not an empty-receipt expiry"
    );
    let mut keys_before: Vec<String> = redis::cmd("KEYS").arg("*").query(&mut conn).unwrap();
    keys_before.sort();
    for count in [1, 2] {
        let empty_expired = runtime
            .service
            .read(&tenant, result1.id(), 20, &cancel)
            .await
            .unwrap();
        assert_eq!(
            empty_expired["error"],
            json!({"code":"expired_handle","message":"originating snapshot has expired","expires_at":1010.})
        );
        assert_eq!(
            expiry_series(),
            vec![format!(
                "slopsearx_workflow_expired_total{{artifact=\"receipt\",workflow=\"retrieval_receipt\"}} {count}"
            )]
        );
    }
    let mut keys_after: Vec<String> = redis::cmd("KEYS").arg("*").query(&mut conn).unwrap();
    keys_after.sort();
    assert_eq!(keys_after, keys_before);
    unchanged_snapshot(&before, &stored_snapshot(&server, &tenant, root.id()));
    // Separate owned fixtures: malformed persisted edge and missing legacy lineage.
    for (id, legacy) in [("corrupt-contract", false), ("legacy-contract", true)] {
        let seeded = SearchSnapshot::new(
            SnapshotId::new(id).unwrap(),
            "q".into(),
            QueryId::new("seed-query").unwrap(),
            vec![],
            ssx_domain::ScopeDecision {
                selected_engines: vec!["wikipedia".into()],
                ..Default::default()
            },
            tenant.clone(),
            WallClockTimestamp::new(1000.).unwrap(),
            Some(WallClockExpiry::new(2000.).unwrap()),
        )
        .unwrap();
        let mut raw = ssx_domain::storage::snapshot::encode(&seeded).unwrap();
        let seeded_root = ArtifactRef::new(ArtifactKind::Snapshot, id).unwrap();
        let edge =
            json!({"from":seeded_root.to_value(),"relation":"derived_from","to":parent.to_value()});
        if legacy {
            raw.as_object_mut().unwrap().swap_remove("lineage");
        } else {
            raw["lineage"] = json!([{"from":{"kind":"bogus","id":"x"},"relation":"derived_from","to":parent.to_value()},edge]);
        }
        let key = ssx_infra::valkey::keys::snapshot(&tenant, id);
        let mut conn = server.connection();
        let _: () = redis::cmd("SET")
            .arg(&key)
            .arg(ssx_json::to_vec(&raw).unwrap())
            .arg("PX")
            .arg(3_600_000)
            .query(&mut conn)
            .unwrap();
        let fence = stored_snapshot(&server, &tenant, id);
        let request = || Request {
            root: seeded_root.clone(),
            direction: Direction::Outgoing,
            max_depth: if legacy { 0 } else { 1 },
            max_nodes: 100,
        };
        let a = lineage::traverse(runtime.lineage.as_ref(), &tenant, request(), &cancel)
            .await
            .unwrap();
        let b = lineage::traverse(runtime.lineage.as_ref(), &tenant, request(), &cancel)
            .await
            .unwrap();
        assert_eq!(a, b);
        if legacy {
            assert_eq!(
                a.nodes[0].to_value()["details"]["lineage_status"],
                "lineage_unavailable"
            );
        } else {
            assert_eq!(
                a.edges.iter().map(|e| e.to_value()).collect::<Vec<_>>(),
                vec![edge]
            );
            assert!(a.nodes.iter().all(|n| {
                n.artifact
                    .to_value()
                    .get("kind")
                    .and_then(Value::as_str)
                    .is_some()
            }));
        }
        unchanged_snapshot(&fence, &stored_snapshot(&server, &tenant, id));
    }
    owner.stop_admission();
    drop(runtime);
    drop(independent);
    drop(second);
    drop(snapshots);
    backend.close().await;
    telemetry.shutdown().await;
    drop(owner);
    drop(backend);
    drop(server);
}
