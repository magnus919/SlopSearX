use super::*;
use crate::search_surface::validation::Arguments;
use ssx_application::{
    PortFuture,
    lineage::{Node, Resolved, Resolver, Status},
    receipts::{
        Insert, ReceiptRepository, Replay, Retained, Submission,
        service::{
            Access, Clock, ManifestSources, ResolvedManifestSource, Service, Source, SourceReader,
        },
    },
};
use ssx_domain::{ArtifactKind, ArtifactRef, WallClockTimestamp};
use ssx_json::json;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

type Row = (String, String, String, String, Value);
#[derive(Default)]
struct Repository {
    rows: Mutex<Vec<Row>>,
    reads: AtomicUsize,
}
impl ReceiptRepository for Repository {
    fn replay<'a>(
        &'a self,
        tenant: &'a TenantId,
        result: &'a str,
        key: &'a str,
        digest: &'a str,
        _: WallClockTimestamp,
        cancel: &'a Cancellation,
    ) -> PortFuture<'a, Result<Replay, Failure>> {
        Box::pin(async move {
            cancel.check()?;
            let rows = self.rows.lock().unwrap();
            Ok(
                match rows
                    .iter()
                    .find(|r| r.0 == tenant.as_str() && r.1 == result && r.2 == key)
                {
                    Some(row) if row.3 == digest => Replay::Replayed(row.4.clone()),
                    Some(_) => Replay::Conflict,
                    None => Replay::Missing,
                },
            )
        })
    }
    fn submit<'a>(&'a self, insert: Insert<'a>) -> PortFuture<'a, Result<Submission, Failure>> {
        Box::pin(async move {
            insert.cancellation.check()?;
            let mut receipt = insert.receipt.clone();
            receipt["receipt_id"] = json!("receipt-one");
            self.rows.lock().unwrap().push((
                insert.tenant.as_str().into(),
                insert.result_id.into(),
                insert.idempotency_key.into(),
                insert.digest.into(),
                receipt.clone(),
            ));
            Ok(Submission::Created(receipt))
        })
    }
    fn read<'a>(
        &'a self,
        tenant: &'a TenantId,
        result: &'a str,
        _: WallClockTimestamp,
        limit: usize,
        cancel: &'a Cancellation,
    ) -> PortFuture<'a, Result<Retained, Failure>> {
        Box::pin(async move {
            cancel.check()?;
            self.reads.fetch_add(1, Ordering::SeqCst);
            let rows = self.rows.lock().unwrap();
            let receipts: Vec<_> = rows
                .iter()
                .filter(|r| r.0 == tenant.as_str() && r.1 == result)
                .map(|r| r.4.clone())
                .collect();
            Ok(Retained {
                total: receipts.len(),
                receipts: receipts.into_iter().take(limit).collect(),
            })
        })
    }
}
struct AccessPort(AtomicBool);
impl Access for AccessPort {
    fn enabled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    fn connected(&self) -> bool {
        true
    }
}
struct ClockPort;
impl Clock for ClockPort {
    fn now(&self) -> WallClockTimestamp {
        WallClockTimestamp::new(1000.0).unwrap()
    }
}
struct Sources;
impl SourceReader for Sources {
    fn discovery<'a>(
        &'a self,
        tenant: &'a TenantId,
        id: &'a str,
        _: &'a Cancellation,
    ) -> PortFuture<'a, Result<Source, Failure>> {
        Box::pin(async move {
            Ok(if tenant.as_str() == "a" && id == "snap:0" {
                Source::Live(
                    json!({"result_id":id,"original_url":"https://source.invalid/inert","verified":false}),
                )
            } else {
                Source::Unknown
            })
        })
    }
}
impl ManifestSources for Sources {
    fn resolve<'a>(
        &'a self,
        _: &'a TenantId,
        _: &'a ArtifactRef,
        _: &'a Cancellation,
    ) -> PortFuture<'a, Result<ResolvedManifestSource, Value>> {
        Box::pin(async { Err(error("source_not_found", "source artifact was not found")) })
    }
}
struct Lineage;
impl Resolver for Lineage {
    fn resolve<'a>(
        &'a self,
        _: &'a TenantId,
        root: &'a ArtifactRef,
        _: &'a Cancellation,
    ) -> PortFuture<'a, Result<Resolved, lineage::Failure>> {
        Box::pin(async move {
            Ok(Resolved {
                node: Node::missing(root.clone(), Status::Missing),
                edges: vec![],
            })
        })
    }
}
fn runtime() -> (EvidenceRuntime, Arc<Repository>, Arc<AccessPort>) {
    let repo = Arc::new(Repository::default());
    let access = Arc::new(AccessPort(AtomicBool::new(true)));
    let lineage = Arc::new(Lineage);
    (
        EvidenceRuntime {
            service: Service::new(
                repo.clone(),
                Arc::new(Sources),
                access.clone(),
                Arc::new(ClockPort),
                lineage.clone(),
                Arc::new(Sources),
                Arc::new(ssx_application::NoopSink),
            ),
            lineage,
        },
        repo,
        access,
    )
}
async fn call(runtime: &EvidenceRuntime, name: &str, args: Value, tenant: &str) -> Value {
    let args = Arguments::new(name, args.as_object().unwrap().clone()).unwrap();
    execute(
        runtime,
        name,
        &args,
        &TenantId::new(tenant).unwrap(),
        &Cancellation::default(),
    )
    .await
    .unwrap()
}
#[tokio::test]
async fn handlers_reuse_application_replay_conflict_tenant_grant_and_manifest_contracts() {
    let (runtime, repo, access) = runtime();
    let observation = json!({"result_id":"snap:0","retriever":"reader","idempotency_key":"once","status":"succeeded","captured_at":"2026-09-11T04:00:00Z","final_url":"https://capture.invalid/inert","capture_ref":"capture://inert"});
    let first = call(&runtime, TOOL_NAMES[0], observation.clone(), "a").await;
    assert_eq!(first["state"], "created");
    assert_eq!(first["receipt"]["attribution"]["authenticated_tenant"], "a");
    assert_eq!(first["receipt"]["observations_verified"], false);
    assert_eq!(
        call(&runtime, TOOL_NAMES[0], observation.clone(), "a").await["state"],
        "replayed"
    );
    let mut changed = observation;
    changed["capture_ref"] = json!("capture://changed");
    assert_eq!(
        call(&runtime, TOOL_NAMES[0], changed, "a").await["error"]["code"],
        "idempotency_conflict"
    );
    assert_eq!(
        call(&runtime, TOOL_NAMES[1], json!({"result_id":"snap:0"}), "b").await["error"]["code"],
        "invalid_result_id"
    );
    let manifest = call(
        &runtime,
        TOOL_NAMES[2],
        json!({"result_ids":["snap:0"]}),
        "a",
    )
    .await;
    assert_eq!(manifest["contract"], "slopsearx.research_manifest");
    assert_eq!(
        manifest["items"][0]["receipts"][0]["receipt_id"],
        first["receipt"]["receipt_id"]
    );
    assert_eq!(manifest["observations_verified"], false);
    let before = repo.reads.load(Ordering::SeqCst);
    access.0.store(false, Ordering::SeqCst);
    for (name, args) in [
        (
            TOOL_NAMES[0],
            json!({"result_id":"snap:0","retriever":"r","idempotency_key":"x","status":"failed"}),
        ),
        (TOOL_NAMES[1], json!({"result_id":"snap:0"})),
        (TOOL_NAMES[2], json!({"result_ids":["snap:0"]})),
    ] {
        assert_eq!(
            call(&runtime, name, args, "a").await["error"]["code"],
            "tool_disabled"
        );
    }
    assert_eq!(repo.reads.load(Ordering::SeqCst), before);
    // Lineage has no standalone receipt grant; its resolver revalidates each artifact.
    let root = ArtifactRef::new(ArtifactKind::Result, "snap:0").unwrap();
    let graph = call(
        &runtime,
        TOOL_NAMES[3],
        json!({"artifact":root.to_value()}),
        "a",
    )
    .await;
    assert_eq!(graph["contract"], "slopsearx.artifact_lineage");
    assert_eq!(graph["nodes"][0]["status"], "missing");
}
#[test]
fn function_boundary_preserves_strict_bounds_nested_types_and_semantic_artifacts() {
    for (name, args, text) in [
        (
            TOOL_NAMES[1],
            json!({"result_id":"r","limit":true}),
            "limit",
        ),
        (TOOL_NAMES[2], json!({"max_depth":"2"}), "max_depth"),
        (
            TOOL_NAMES[3],
            json!({"artifact":[],"max_nodes":1.0}),
            "artifact",
        ),
        (
            TOOL_NAMES[0],
            json!({"result_id":"r","retriever":"r","idempotency_key":"k","status":"failed","passage_refs":[{"id":1}]}),
            "passage_refs.0.id",
        ),
    ] {
        let error = Arguments::new(name, args.as_object().unwrap().clone())
            .err()
            .expect("invalid function type");
        assert!(error.message.contains(text), "{}", error.message);
    }
    assert!(
        Arguments::new(
            TOOL_NAMES[3],
            json!({"artifact":{"unrecognized":true}})
                .as_object()
                .unwrap()
                .clone()
        )
        .is_ok()
    );
    let deferred = Arguments::raw(TOOL_NAMES[3], r#"{"artifact":{"version":1e999}}"#).unwrap();
    assert_eq!(
        deferred.artifact("artifact"),
        Some(Err(
            "artifact must contain exactly contract, version, kind, and id".into()
        ))
    );
    // Typed receipt strings/passage strings and strict integers retain function errors.
    for (name, raw, location) in [
        (
            TOOL_NAMES[0],
            r#"{"result_id":"r","retriever":1e999,"idempotency_key":"k","status":"failed"}"#,
            "retriever",
        ),
        (
            TOOL_NAMES[0],
            r#"{"result_id":"r","retriever":"r","idempotency_key":"k","status":"failed","passage_refs":[{"ref":1e999}]}"#,
            "passage_refs.0.ref",
        ),
        (
            TOOL_NAMES[3],
            r#"{"artifact":{},"max_depth":1e999}"#,
            "max_depth",
        ),
    ] {
        assert!(
            Arguments::raw(name, raw)
                .err()
                .unwrap()
                .message
                .contains(location)
        );
    }
    // Normal arbitrary-size lexical integers remain available to semantic parsing.
    let large = Arguments::raw(TOOL_NAMES[3], r#"{"artifact":{"contract":"slopsearx.artifact_ref","version":18446744073709551617,"kind":"snapshot","id":"s"}}"#).unwrap();
    assert_eq!(
        large.artifact("artifact"),
        Some(Err(
            "unsupported artifact reference contract or version".into()
        ))
    );
    for version in ["1", "1.0", "true"] {
        let raw = format!(
            r#"{{"artifact":{{"contract":"slopsearx.artifact_ref","version":{version},"kind":"snapshot","id":"s"}}}}"#
        );
        assert_eq!(
            Arguments::raw(TOOL_NAMES[3], &raw)
                .unwrap()
                .artifact("artifact"),
            Some(Ok(ArtifactRef::new(ArtifactKind::Snapshot, "s").unwrap()))
        );
    }
}
#[tokio::test]
async fn semantic_errors_keep_source_order_defaults_and_field_attribution() {
    let (runtime, _, _) = runtime();
    let error = call(
        &runtime,
        TOOL_NAMES[3],
        json!({"artifact":{"extra":true},"direction":"wrong","max_depth":6}),
        "a",
    )
    .await;
    assert_eq!(error["error"]["field"], "artifact");
    let root = ArtifactRef::new(ArtifactKind::Result, "snap:0").unwrap();
    let error = call(
        &runtime,
        TOOL_NAMES[3],
        json!({"artifact":root.to_value(),"direction":"wrong","max_depth":6}),
        "a",
    )
    .await;
    assert_eq!(error["error"]["field"], "direction");
    let error = call(
        &runtime,
        TOOL_NAMES[2],
        json!({"source":root.to_value(),"result_ids":[]}),
        "a",
    )
    .await;
    assert_eq!(error["error"]["code"], "input_conflict");
    let graph = call(
        &runtime,
        TOOL_NAMES[3],
        json!({"artifact":root.to_value()}),
        "a",
    )
    .await;
    assert_eq!(graph["direction"], "both");
    assert_eq!(graph["max_depth"], 2);
    assert_eq!(graph["max_nodes"], 100);
    for (variant, code, message) in [
        (
            Failure::Malformed,
            "store_corrupt",
            "retained receipt state is malformed",
        ),
        (
            Failure::Closed,
            "store_unavailable",
            "receipt store is unavailable",
        ),
        (
            Failure::Unavailable,
            "store_unavailable",
            "receipt store is unavailable",
        ),
        (
            Failure::InvalidInput,
            "invalid_input",
            "receipt store rejected the input",
        ),
        (
            Failure::Collision,
            "store_unavailable",
            "receipt identity collision",
        ),
        (
            Failure::CompletionUnknown,
            "completion_unknown",
            "receipt persistence completion is unknown; replay the same idempotency key",
        ),
        (
            Failure::ScriptErrorMayHaveWritten,
            "completion_unknown",
            "receipt persistence completion is unknown; replay the same idempotency key",
        ),
    ] {
        assert_eq!(
            failure(variant).unwrap(),
            json!({"error":{"code":code,"message":message}})
        );
    }
    assert!(failure(Failure::Cancelled).is_err());
}

#[tokio::test]
async fn raw_any_artifacts_preserve_source_semantic_precedence_without_storage() {
    let (runtime, repo, access) = runtime();
    for (name, raw, expected) in [
        (
            TOOL_NAMES[3],
            r#"{"artifact":{"version":1e999}}"#,
            json!({"error":{"code":"invalid_input","message":"artifact must contain exactly contract, version, kind, and id","field":"artifact"}}),
        ),
        (
            TOOL_NAMES[3],
            r#"{"artifact":{"contract":"slopsearx.artifact_ref","version":1e999,"kind":"snapshot","id":"s"}}"#,
            json!({"error":{"code":"invalid_input","message":"unsupported artifact reference contract or version","field":"artifact"}}),
        ),
        (
            TOOL_NAMES[3],
            r#"{"artifact":{"contract":"slopsearx.artifact_ref","version":1,"kind":"snapshot","id":{"nested":[1e999]}}}"#,
            json!({"error":{"code":"invalid_input","message":"artifact kind and id must be strings","field":"artifact"}}),
        ),
        (
            TOOL_NAMES[2],
            r#"{"artifacts":[{"extra":{"nested":[1e999]}}],"max_depth":6}"#,
            json!({"error":{"code":"invalid_input","message":"max_depth must be an integer from 0 to 5"}}),
        ),
        (
            TOOL_NAMES[2],
            r#"{"source":{"version":1e999},"result_ids":[]}"#,
            json!({"error":{"code":"input_conflict","message":"source cannot be combined with result_ids or artifacts"}}),
        ),
        (
            TOOL_NAMES[2],
            r#"{"source":{"version":1e999}}"#,
            json!({"error":{"code":"unsupported_source_contract","message":"artifact must contain exactly contract, version, kind, and id","field":"source"}}),
        ),
        (
            TOOL_NAMES[2],
            r#"{"artifacts":[{"version":1e999}]}"#,
            json!({"error":{"code":"invalid_input","message":"artifact must contain exactly contract, version, kind, and id"}}),
        ),
    ] {
        let args = Arguments::raw(name, raw).unwrap();
        let result = execute(
            &runtime,
            name,
            &args,
            &TenantId::new("a").unwrap(),
            &Cancellation::default(),
        )
        .await
        .unwrap();
        assert_eq!(result, expected, "{raw}");
    }
    access.0.store(false, Ordering::SeqCst);
    let args = Arguments::raw(
        TOOL_NAMES[2],
        r#"{"source":{"version":1e999},"result_ids":[]}"#,
    )
    .unwrap();
    assert_eq!(
        execute(
            &runtime,
            TOOL_NAMES[2],
            &args,
            &TenantId::new("a").unwrap(),
            &Cancellation::default()
        )
        .await
        .unwrap(),
        json!({"error":{"code":"tool_disabled","message":"retrieval receipts require MCP_GRANT_RETRIEVAL_RECEIPTS=1"}})
    );
    assert_eq!(repo.reads.load(Ordering::SeqCst), 0);
    assert!(repo.rows.lock().unwrap().is_empty());
}
