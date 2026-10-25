//! Attributed historical evidence access. These ports cannot acquire new data:
//! no engine, HTTP client, workflow runner, lease or scheduler is present.
use super::{
    Cancellation, Failure, Insert, MAX_MANIFEST_RESULTS, ReceiptRepository, Replay, Submission,
    captured_manifest::{Outcome, Walks},
    observation::{Observation, ObservationInput},
};
use crate::{
    PortFuture,
    lineage::{self, Direction, Resolver},
};
use ssx_domain::{
    ArtifactKind, ArtifactRef, TenantId, WallClockTimestamp, canonical::composite_artifact_id,
};
use ssx_json::{Value, json};
use std::{collections::BTreeSet, sync::Arc};

pub const NON_VERIFICATION_NOTE: &str = "SlopSearX did not fetch or verify the linked page";
/// Composition supplies live grant/connectivity observations, never caller data.
pub trait Access: Send + Sync {
    fn enabled(&self) -> bool;
    fn connected(&self) -> bool;
}
pub trait Clock: Send + Sync {
    fn now(&self) -> WallClockTimestamp;
}
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Live(Value),
    InvalidId,
    Unknown,
    Expired(WallClockTimestamp),
    Unavailable,
}
impl Source {
    fn error(&self) -> Option<Value> {
        Some(match self {
            Self::Live(_) => return None,
            Self::InvalidId => error(
                "invalid_result_id",
                "result_id must be a server-issued snapshot result handle",
            ),
            Self::Unknown => error("invalid_result_id", "unknown result_id"),
            Self::Expired(at) => {
                let mut e = error("expired_handle", "originating snapshot has expired");
                e["error"]["expires_at"] = json!(at.seconds());
                e
            }
            Self::Unavailable => error("store_unavailable", "snapshot store is unavailable"),
        })
    }
    fn status(&self) -> &'static str {
        match self {
            Self::Live(_) => "live",
            Self::Expired(_) => "expired",
            Self::Unavailable => "unavailable",
            _ => "unknown",
        }
    }
}
/// Uses F13's tenant-bound immutable snapshot read, then pure projection. It
/// deliberately does not reauthorize historical receipt access with engine
/// policy: the measured original receipt paths have that compatibility profile.
pub trait SourceReader: Send + Sync {
    fn discovery<'a>(
        &'a self,
        tenant: &'a TenantId,
        result_id: &'a str,
        cancel: &'a Cancellation,
    ) -> PortFuture<'a, Result<Source, Failure>>;
}
#[derive(Clone, Debug)]
pub struct ResolvedManifestSource {
    pub artifact: ArtifactRef,
    pub result_ids: Vec<String>,
    pub stored: Value,
}
/// W09 composes workflow admission; this readonly port only resolves an already
/// retained research/staged source for the existing manifest transition.
pub trait ManifestSources: Send + Sync {
    fn resolve<'a>(
        &'a self,
        tenant: &'a TenantId,
        source: &'a ArtifactRef,
        cancel: &'a Cancellation,
    ) -> PortFuture<'a, Result<ResolvedManifestSource, Value>>;
}
pub fn error(code: &str, message: &str) -> Value {
    json!({"error":{"code":code,"message":message}})
}
fn conflict() -> Value {
    error(
        "idempotency_conflict",
        "idempotency key was already used with different content",
    )
}
fn decorate(result_id: &str, receipt: Value) -> Result<Value, Failure> {
    let mut object = receipt.as_object().cloned().ok_or(Failure::Malformed)?;
    let id = object
        .get("receipt_id")
        .and_then(Value::as_str)
        .ok_or(Failure::Malformed)?;
    let composite = composite_artifact_id(&[result_id, id]).map_err(|_| Failure::Malformed)?;
    let reference = ArtifactRef::new(ArtifactKind::RetrievalReceipt, composite)
        .map_err(|_| Failure::Malformed)?;
    let target =
        ArtifactRef::new(ArtifactKind::Result, result_id).map_err(|_| Failure::Malformed)?;
    object.insert("artifact".into(), reference.to_value());
    object.insert(
        "lineage".into(),
        Value::Array(vec![
            json!({"from":reference.to_value(),"relation":"retrieval_of","to":target.to_value()}),
        ]),
    );
    Ok(Value::Object(object))
}
#[derive(Clone)]
pub struct Service {
    repo: Arc<dyn ReceiptRepository>,
    source: Arc<dyn SourceReader>,
    access: Arc<dyn Access>,
    clock: Arc<dyn Clock>,
    lineage: Arc<dyn Resolver>,
    sources: Arc<dyn ManifestSources>,
    metrics: Arc<dyn crate::Metrics>,
}
impl Service {
    pub fn new(
        repo: Arc<dyn ReceiptRepository>,
        source: Arc<dyn SourceReader>,
        access: Arc<dyn Access>,
        clock: Arc<dyn Clock>,
        lineage: Arc<dyn Resolver>,
        sources: Arc<dyn ManifestSources>,
        metrics: Arc<dyn crate::Metrics>,
    ) -> Self {
        Self {
            repo,
            source,
            access,
            clock,
            lineage,
            sources,
            metrics,
        }
    }
    fn enabled(&self) -> Option<Value> {
        if !self.access.enabled() {
            Some(error(
                "tool_disabled",
                "retrieval receipts require MCP_GRANT_RETRIEVAL_RECEIPTS=1",
            ))
        } else if !self.access.connected() {
            Some(error(
                "store_unavailable",
                "retrieval receipts require connected Valkey",
            ))
        } else {
            None
        }
    }
    pub async fn submit(
        &self,
        tenant: &TenantId,
        result_id: &str,
        input: ObservationInput,
        cancel: &Cancellation,
    ) -> Result<Value, Failure> {
        cancel.check()?;
        if let Some(e) = self.enabled() {
            return Ok(e);
        }
        let observation = match input.normalize() {
            Ok(v) => v,
            Err(e) => return Ok(error("invalid_input", &e.0)),
        };
        self.submit_normalized(tenant, result_id, observation, cancel)
            .await
    }
    async fn submit_normalized(
        &self,
        tenant: &TenantId,
        result_id: &str,
        observation: Observation,
        cancel: &Cancellation,
    ) -> Result<Value, Failure> {
        let now = self.clock.now();
        match self
            .repo
            .replay(
                tenant,
                result_id,
                &observation.idempotency_key,
                &observation.digest,
                now,
                cancel,
            )
            .await?
        {
            Replay::Conflict => {
                self.metrics
                    .workflow_rejected("retrieval_receipt", "idempotency");
                return Ok(conflict());
            }
            Replay::Replayed(value) => {
                return Ok(json_with_receipt("replayed", decorate(result_id, value)?));
            }
            Replay::Missing => {}
        }
        cancel.check()?;
        let source = self.source.discovery(tenant, result_id, cancel).await?;
        cancel.check()?;
        if let Some(e) = source.error() {
            return Ok(e);
        }
        let Source::Live(discovery) = source else {
            unreachable!()
        };
        let outcome = if observation.body["observation"]["status"] == "succeeded" {
            "succeeded"
        } else {
            "failed"
        };
        let mut receipt = json!({"contract":super::CONTRACT,"version":1,"result_id":result_id,"attribution":{"retriever":observation.body["retriever"],"authenticated_tenant":tenant.as_str()},"observations_verified":false});
        receipt["observation"] = observation.body["observation"].clone();
        receipt["discovery"] = discovery;
        cancel.check()?;
        match self
            .repo
            .submit(Insert {
                tenant,
                result_id,
                idempotency_key: &observation.idempotency_key,
                digest: &observation.digest,
                receipt: &receipt,
                now,
                cancellation: cancel,
            })
            .await?
        {
            Submission::Created(v) => {
                self.metrics
                    .workflow_accepted("retrieval_receipt", "immediate");
                self.metrics.workflow_terminal("retrieval_receipt", outcome);
                Ok(json_with_receipt("created", decorate(result_id, v)?))
            }
            Submission::Replayed(v) => {
                // Source insert-time replay records events; early retained replay does not.
                self.metrics
                    .workflow_accepted("retrieval_receipt", "immediate");
                self.metrics.workflow_terminal("retrieval_receipt", outcome);
                Ok(json_with_receipt("replayed", decorate(result_id, v)?))
            }
            Submission::Conflict => {
                self.metrics
                    .workflow_rejected("retrieval_receipt", "idempotency");
                Ok(conflict())
            }
            Submission::Capacity => {
                self.metrics
                    .workflow_rejected("retrieval_receipt", "capacity");
                Ok(error(
                    "resource_limit",
                    "receipt limit reached for this result",
                ))
            }
            Submission::Size => Ok(error(
                "invalid_input",
                "encoded receipt exceeds 32768 bytes",
            )),
        }
    }
    pub async fn read(
        &self,
        tenant: &TenantId,
        result_id: &str,
        limit: usize,
        cancel: &Cancellation,
    ) -> Result<Value, Failure> {
        cancel.check()?;
        // Original validates its StrictInt presentation bound before the grant.
        if !(1..=20).contains(&limit) {
            return Ok(error(
                "invalid_input",
                "limit must be an integer between 1 and 20",
            ));
        }
        if let Some(e) = self.enabled() {
            return Ok(e);
        }
        let retained = self
            .repo
            .read(tenant, result_id, self.clock.now(), limit, cancel)
            .await?;
        cancel.check()?;
        let source = self.source.discovery(tenant, result_id, cancel).await?;
        cancel.check()?;
        let discovery = match &source {
            Source::Live(v) => v.clone(),
            _ if retained.receipts.is_empty() => return Ok(source.error().expect("nonlive")),
            _ => retained.receipts[0]
                .get("discovery")
                .cloned()
                .unwrap_or(Value::Null),
        };
        let returned = retained.receipts.len();
        let receipts = retained
            .receipts
            .into_iter()
            .map(|r| decorate(result_id, r))
            .collect::<Result<Vec<_>, _>>()?;
        let mut value = json!({"result_id":result_id,"source_snapshot_status":source.status(),"discovery":null,"receipts":null,"total":retained.total,"returned":returned,"has_more":retained.total>returned,"observations_verified":false});
        value["discovery"] = discovery;
        value["receipts"] = Value::Array(receipts);
        Ok(value)
    }
    pub async fn manifest(
        &self,
        tenant: &TenantId,
        request: ManifestRequest,
        cancel: &Cancellation,
    ) -> Result<Value, Failure> {
        self.manifest_draft(tenant, request.into(), cancel).await
    }
    /// Type validation has already completed. Artifact semantic failures are
    /// deferred so the same availability/conflict/source/bounds ordering applies
    /// to both typed callers and source-compatible MCP dict[Any] inputs.
    pub async fn manifest_draft(
        &self,
        tenant: &TenantId,
        request: ManifestDraft,
        cancel: &Cancellation,
    ) -> Result<Value, Failure> {
        self.manifest_operation(tenant, request, false, cancel)
            .await?
            .value()
    }
    /// Explicit component API only; no MCP/catalog/output profile enables it.
    /// Validation, readonly operations and late canonical/identity decisions use
    /// the exact same sequence as the existing Value-returning method.
    pub async fn capture_manifest_draft(
        &self,
        tenant: &TenantId,
        request: ManifestDraft,
        cancel: &Cancellation,
    ) -> Result<Outcome, Failure> {
        self.manifest_operation(tenant, request, true, cancel).await
    }
    async fn manifest_operation(
        &self,
        tenant: &TenantId,
        request: ManifestDraft,
        captured: bool,
        cancel: &Cancellation,
    ) -> Result<Outcome, Failure> {
        cancel.check()?;
        if let Some(e) = self.enabled() {
            return Ok(Outcome::Value(e));
        }
        if request.source.is_some() && (request.result_ids.is_some() || request.artifacts.is_some())
        {
            return Ok(Outcome::Value(error(
                "input_conflict",
                "source cannot be combined with result_ids or artifacts",
            )));
        }
        let resolved = if let Some(source) = &request.source {
            let source = match source {
                Ok(source) => source,
                Err(message) => {
                    let mut value = error("unsupported_source_contract", message);
                    value["error"]["field"] = Value::String("source".into());
                    return Ok(Outcome::Value(value));
                }
            };
            let outcome = self.sources.resolve(tenant, source, cancel).await;
            cancel.check()?;
            match outcome {
                Ok(v) => Some(v),
                Err(e) => return Ok(Outcome::Value(e)),
            }
        } else {
            None
        };
        let ids = resolved
            .as_ref()
            .map(|s| s.result_ids.clone())
            .unwrap_or_else(|| request.result_ids.unwrap_or_default());
        let roots = request.artifacts.unwrap_or_default();
        if ids.len() > MAX_MANIFEST_RESULTS {
            return Ok(Outcome::Value(error(
                "invalid_input",
                "result_ids must contain at most 25 entries",
            )));
        }
        if ids.iter().any(String::is_empty) {
            return Ok(Outcome::Value(error(
                "invalid_input",
                "every result_id must be a non-empty string",
            )));
        }
        if roots.len() > MAX_MANIFEST_RESULTS {
            return Ok(Outcome::Value(error(
                "invalid_input",
                "artifacts must contain at most 25 entries",
            )));
        }
        if ids.is_empty() && roots.is_empty() {
            return Ok(Outcome::Value(error(
                "invalid_input",
                "provide at least one result_id or artifact",
            )));
        }
        if request.max_depth > 5 {
            return Ok(Outcome::Value(error(
                "invalid_input",
                "max_depth must be an integer from 0 to 5",
            )));
        }
        if !(1..=100).contains(&request.max_nodes) {
            return Ok(Outcome::Value(error(
                "invalid_input",
                "max_nodes must be an integer from 1 to 100",
            )));
        }
        let roots = match roots.into_iter().collect::<Result<Vec<_>, _>>() {
            Ok(roots) => roots,
            Err(message) => return Ok(Outcome::Value(error("invalid_input", &message))),
        };
        let mut seen = BTreeSet::new();
        if roots
            .iter()
            .any(|r| !seen.insert((format!("{:?}", r.kind()), r.id().to_owned())))
        {
            return Ok(Outcome::Value(error(
                "invalid_input",
                "artifacts must not contain duplicates",
            )));
        }
        let mut seen = BTreeSet::new();
        if ids.iter().any(|id| !seen.insert(id.clone())) {
            return Ok(Outcome::Value(error(
                "invalid_input",
                "result_ids must not contain duplicates",
            )));
        }
        let mut walks = Walks::new(captured, roots.len(), ids, seen, cancel)?;
        for root in roots {
            cancel.check()?;
            walks
                .append(
                    self.lineage.as_ref(),
                    tenant,
                    lineage::Request {
                        root,
                        direction: Direction::Both,
                        max_depth: request.max_depth,
                        max_nodes: request.max_nodes,
                    },
                    cancel,
                )
                .await?;
        }
        let (ids, mut assembly) = walks.finish(cancel)?;
        if ids.len() > MAX_MANIFEST_RESULTS {
            return Ok(Outcome::Value(error(
                "resource_limit",
                "lineage selection exceeds 25 results",
            )));
        }
        for id in &ids {
            let mut item = self.read(tenant, id, 20, cancel).await?;
            if item.get("error").is_some() {
                item["result_id"] = Value::String(id.clone());
            }
            assembly.item(item, cancel)?;
        }
        assembly.finish(ids, resolved, self.clock.now(), cancel)
    }
}
fn json_with_receipt(state: &str, receipt: Value) -> Value {
    let mut out = json!({"state":state});
    out["receipt"] = receipt;
    out
}

#[cfg(test)]
mod service_contract_tests {
    use super::*;
    use crate::receipts::Retained;
    use crate::{
        PortFuture,
        lineage::{Node, Resolved, Status},
    };
    use ssx_domain::{ArtifactRef, TenantId, WallClockTimestamp};
    use std::sync::Mutex;

    #[derive(Default)]
    struct Trace(Mutex<Vec<&'static str>>);
    impl Trace {
        fn push(&self, event: &'static str) {
            self.0.lock().unwrap().push(event);
        }
        fn events(&self) -> Vec<&'static str> {
            self.0.lock().unwrap().clone()
        }
    }
    impl crate::Metrics for Trace {
        fn increment(&self, _: &'static str) {
            panic!("receipt telemetry must use bounded workflow events");
        }
        fn workflow_accepted(&self, workflow: &'static str, mode: &'static str) {
            assert_eq!((workflow, mode), ("retrieval_receipt", "immediate"));
            self.push("accepted");
        }
        fn workflow_rejected(&self, workflow: &'static str, reason: &'static str) {
            assert_eq!(workflow, "retrieval_receipt");
            self.push(match reason {
                "idempotency" => "rejected_idempotency",
                "capacity" => "rejected_capacity",
                _ => panic!("unexpected receipt rejection label"),
            });
        }
        fn workflow_terminal(&self, workflow: &'static str, outcome: &'static str) {
            assert_eq!(workflow, "retrieval_receipt");
            self.push(match outcome {
                "succeeded" => "terminal_succeeded",
                "failed" => "terminal_failed",
                _ => panic!("unexpected receipt terminal label"),
            });
        }
    }
    struct TelemetryRepo {
        trace: Arc<Trace>,
        replay: Result<Replay, Failure>,
        insert: Result<Submission, Failure>,
    }
    impl ReceiptRepository for TelemetryRepo {
        fn replay<'a>(
            &'a self,
            _: &'a TenantId,
            _: &'a str,
            _: &'a str,
            _: &'a str,
            _: WallClockTimestamp,
            _: &'a Cancellation,
        ) -> PortFuture<'a, Result<Replay, Failure>> {
            self.trace.push("replay");
            let result = self.replay.clone();
            Box::pin(async move { result })
        }
        fn submit<'a>(&'a self, _: Insert<'a>) -> PortFuture<'a, Result<Submission, Failure>> {
            self.trace.push("submit");
            let result = self.insert.clone();
            Box::pin(async move { result })
        }
        fn read<'a>(
            &'a self,
            _: &'a TenantId,
            _: &'a str,
            _: WallClockTimestamp,
            _: usize,
            _: &'a Cancellation,
        ) -> PortFuture<'a, Result<Retained, Failure>> {
            Box::pin(async { panic!("submission telemetry cannot read receipts") })
        }
    }
    fn telemetry_service(
        trace: Arc<Trace>,
        enabled: bool,
        replay: Result<Replay, Failure>,
        insert: Result<Submission, Failure>,
        source: Source,
    ) -> Service {
        Service::new(
            Arc::new(TelemetryRepo {
                trace: trace.clone(),
                replay,
                insert,
            }),
            Arc::new(SourcePort {
                trace: trace.clone(),
                value: source,
            }),
            Arc::new(Grant(enabled)),
            Arc::new(TestClock),
            Arc::new(NoLineage),
            Arc::new(NoSources),
            trace,
        )
    }
    struct Repo {
        trace: Arc<Trace>,
        replay: Replay,
        retained: Retained,
    }
    impl ReceiptRepository for Repo {
        fn replay<'a>(
            &'a self,
            _: &'a TenantId,
            _: &'a str,
            _: &'a str,
            _: &'a str,
            _: WallClockTimestamp,
            _: &'a Cancellation,
        ) -> PortFuture<'a, Result<Replay, Failure>> {
            self.trace.push("replay");
            let value = self.replay.clone();
            Box::pin(async move { Ok(value) })
        }
        fn submit<'a>(&'a self, insert: Insert<'a>) -> PortFuture<'a, Result<Submission, Failure>> {
            self.trace.push("submit");
            let mut value = insert.receipt.clone();
            value["receipt_id"] = Value::String("receipt-created".into());
            Box::pin(async move { Ok(Submission::Created(value)) })
        }
        fn read<'a>(
            &'a self,
            _: &'a TenantId,
            _: &'a str,
            _: WallClockTimestamp,
            _: usize,
            _: &'a Cancellation,
        ) -> PortFuture<'a, Result<Retained, Failure>> {
            self.trace.push("read");
            let value = self.retained.clone();
            Box::pin(async move { Ok(value) })
        }
    }
    struct SourcePort {
        trace: Arc<Trace>,
        value: Source,
    }
    impl SourceReader for SourcePort {
        fn discovery<'a>(
            &'a self,
            _: &'a TenantId,
            _: &'a str,
            _: &'a Cancellation,
        ) -> PortFuture<'a, Result<Source, Failure>> {
            self.trace.push("source");
            let value = self.value.clone();
            Box::pin(async move { Ok(value) })
        }
    }
    struct Grant(bool);
    impl Access for Grant {
        fn enabled(&self) -> bool {
            self.0
        }
        fn connected(&self) -> bool {
            true
        }
    }
    struct TestClock;
    impl Clock for TestClock {
        fn now(&self) -> WallClockTimestamp {
            WallClockTimestamp::new(100.).unwrap()
        }
    }
    struct NoLineage;
    impl Resolver for NoLineage {
        fn resolve<'a>(
            &'a self,
            _: &'a TenantId,
            artifact: &'a ArtifactRef,
            _: &'a Cancellation,
        ) -> PortFuture<'a, Result<Resolved, lineage::Failure>> {
            let node = Node::missing(artifact.clone(), Status::Missing);
            Box::pin(async move {
                Ok(Resolved {
                    node,
                    edges: vec![],
                })
            })
        }
    }
    struct WalkResolver {
        trace: Arc<Trace>,
    }
    impl Resolver for WalkResolver {
        fn resolve<'a>(
            &'a self,
            _: &'a TenantId,
            artifact: &'a ArtifactRef,
            _: &'a Cancellation,
        ) -> PortFuture<'a, Result<Resolved, lineage::Failure>> {
            self.trace.push("walk");
            let node = Node {
                artifact: artifact.clone(),
                status: Status::Live,
                created_at: None,
                expires_at: None,
                details: None,
            };
            Box::pin(async move {
                Ok(Resolved {
                    node,
                    edges: vec![],
                })
            })
        }
    }
    struct NoSources;
    impl ManifestSources for NoSources {
        fn resolve<'a>(
            &'a self,
            _: &'a TenantId,
            _: &'a ArtifactRef,
            _: &'a Cancellation,
        ) -> PortFuture<'a, Result<ResolvedManifestSource, Value>> {
            Box::pin(async { panic!("source resolution was not expected") })
        }
    }
    struct ManifestPort {
        trace: Arc<Trace>,
        result_ids: Vec<String>,
    }
    impl ManifestSources for ManifestPort {
        fn resolve<'a>(
            &'a self,
            _: &'a TenantId,
            source: &'a ArtifactRef,
            _: &'a Cancellation,
        ) -> PortFuture<'a, Result<ResolvedManifestSource, Value>> {
            self.trace.push("manifest_source");
            let result = ResolvedManifestSource {
                artifact: source.clone(),
                result_ids: self.result_ids.clone(),
                stored: json!({"artifact":source.to_value()}),
            };
            Box::pin(async move { Ok(result) })
        }
    }
    fn tenant() -> TenantId {
        TenantId::new("tenant-a").unwrap()
    }
    fn observation() -> ObservationInput {
        ObservationInput {
            retriever: "agent".into(),
            idempotency_key: "key-1".into(),
            status: "succeeded".into(),
            captured_at: Some("2026-01-02T03:04:05Z".into()),
            final_url: None,
            content_sha256: None,
            capture_ref: Some("capture://one".into()),
            passage_refs: None,
            failure_code: None,
            failure_message: None,
        }
    }
    fn service(
        trace: Arc<Trace>,
        enabled: bool,
        replay: Replay,
        source: Source,
        retained: Retained,
    ) -> Service {
        Service::new(
            Arc::new(Repo {
                trace: trace.clone(),
                replay,
                retained,
            }),
            Arc::new(SourcePort {
                trace,
                value: source,
            }),
            Arc::new(Grant(enabled)),
            Arc::new(TestClock),
            Arc::new(NoLineage),
            Arc::new(NoSources),
            Arc::new(crate::NoopSink),
        )
    }
    fn manifest_service(trace: Arc<Trace>) -> Service {
        Service::new(
            Arc::new(Repo {
                trace: trace.clone(),
                replay: Replay::Missing,
                retained: Retained {
                    receipts: vec![],
                    total: 0,
                },
            }),
            Arc::new(SourcePort {
                trace: trace.clone(),
                value: Source::Live(json!({"query":"captured"})),
            }),
            Arc::new(Grant(true)),
            Arc::new(TestClock),
            Arc::new(NoLineage),
            Arc::new(ManifestPort {
                trace,
                result_ids: vec!["result-1".into()],
            }),
            Arc::new(crate::NoopSink),
        )
    }
    fn manifest_walk_service(trace: Arc<Trace>) -> Service {
        Service::new(
            Arc::new(Repo {
                trace: trace.clone(),
                replay: Replay::Missing,
                retained: Retained {
                    receipts: vec![],
                    total: 0,
                },
            }),
            Arc::new(SourcePort {
                trace: trace.clone(),
                value: Source::Live(json!({"query":"captured"})),
            }),
            Arc::new(Grant(true)),
            Arc::new(TestClock),
            Arc::new(WalkResolver { trace }),
            Arc::new(NoSources),
            Arc::new(crate::NoopSink),
        )
    }
    fn run<F: std::future::Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("test port unexpectedly yielded"),
        }
    }

    #[test]
    fn submit_checks_grant_before_normalization_and_replays_before_expired_source_lookup() {
        let mut invalid = observation();
        invalid.status = "unknown".into();
        let trace = Arc::new(Trace::default());
        let disabled = service(
            trace.clone(),
            false,
            Replay::Missing,
            Source::Unknown,
            Retained {
                receipts: vec![],
                total: 0,
            },
        );
        let value =
            run(disabled.submit(&tenant(), "result-1", invalid, &Cancellation::default())).unwrap();
        assert_eq!(value["error"]["code"], "tool_disabled");
        assert!(trace.events().is_empty());

        let trace = Arc::new(Trace::default());
        let replayed = json!({"receipt_id":"receipt-1","discovery":{"query":"retained"}});
        let service = service(
            trace.clone(),
            true,
            Replay::Replayed(replayed),
            Source::Expired(WallClockTimestamp::new(90.).unwrap()),
            Retained {
                receipts: vec![],
                total: 0,
            },
        );
        let value = run(service.submit(
            &tenant(),
            "result-1",
            observation(),
            &Cancellation::default(),
        ))
        .unwrap();
        assert_eq!(value["state"], "replayed");
        assert_eq!(trace.events(), vec!["replay"]);
    }

    #[test]
    fn receipt_telemetry_preserves_source_branch_order_and_terminal_status() {
        let receipt = json!({"receipt_id":"receipt-trace"});
        for (name, replay, insert, status, wanted, events) in [
            (
                "early replay",
                Replay::Replayed(receipt.clone()),
                Submission::Size,
                "succeeded",
                "replayed",
                vec!["replay"],
            ),
            (
                "early conflict",
                Replay::Conflict,
                Submission::Size,
                "succeeded",
                "idempotency_conflict",
                vec!["replay", "rejected_idempotency"],
            ),
            (
                "insert conflict",
                Replay::Missing,
                Submission::Conflict,
                "succeeded",
                "idempotency_conflict",
                vec!["replay", "source", "submit", "rejected_idempotency"],
            ),
            (
                "capacity",
                Replay::Missing,
                Submission::Capacity,
                "succeeded",
                "resource_limit",
                vec!["replay", "source", "submit", "rejected_capacity"],
            ),
            (
                "created succeeded",
                Replay::Missing,
                Submission::Created(receipt.clone()),
                "succeeded",
                "created",
                vec![
                    "replay",
                    "source",
                    "submit",
                    "accepted",
                    "terminal_succeeded",
                ],
            ),
            (
                "created failed",
                Replay::Missing,
                Submission::Created(receipt.clone()),
                "failed",
                "created",
                vec!["replay", "source", "submit", "accepted", "terminal_failed"],
            ),
            (
                "insert-time replay",
                Replay::Missing,
                Submission::Replayed(receipt),
                "failed",
                "replayed",
                vec!["replay", "source", "submit", "accepted", "terminal_failed"],
            ),
        ] {
            let trace = Arc::new(Trace::default());
            let service = telemetry_service(
                trace.clone(),
                true,
                Ok(replay),
                Ok(insert),
                Source::Live(json!({"query":"captured"})),
            );
            let mut input = observation();
            input.status = status.into();
            if status == "failed" {
                input.failure_code = Some("blocked".into());
            }
            let value = run(service.submit(&tenant(), "result-2", input, &Cancellation::default()))
                .unwrap();
            let result = value
                .get("state")
                .and_then(Value::as_str)
                .or_else(|| value["error"]["code"].as_str());
            assert_eq!(result, Some(wanted), "{name}");
            assert_eq!(trace.events(), events, "{name}");
        }
    }

    #[test]
    fn receipt_telemetry_never_invents_acceptance_for_refusal_or_unknown_completion() {
        for failure in [
            Failure::Unavailable,
            Failure::Closed,
            Failure::Malformed,
            Failure::InvalidInput,
            Failure::CompletionUnknown,
            Failure::ScriptErrorMayHaveWritten,
            Failure::Collision,
            Failure::Cancelled,
        ] {
            let trace = Arc::new(Trace::default());
            let service = telemetry_service(
                trace.clone(),
                true,
                Ok(Replay::Missing),
                Err(failure),
                Source::Live(json!({"query":"captured"})),
            );
            assert_eq!(
                run(service.submit(
                    &tenant(),
                    "result-2",
                    observation(),
                    &Cancellation::default()
                )),
                Err(failure)
            );
            assert_eq!(trace.events(), ["replay", "source", "submit"]);
        }
        let trace = Arc::new(Trace::default());
        let service = telemetry_service(
            trace.clone(),
            true,
            Ok(Replay::Missing),
            Ok(Submission::Size),
            Source::Live(json!({"query":"captured"})),
        );
        let value = run(service.submit(
            &tenant(),
            "result-2",
            observation(),
            &Cancellation::default(),
        ))
        .unwrap();
        assert_eq!(value["error"]["code"], "invalid_input");
        assert_eq!(trace.events(), ["replay", "source", "submit"]);
        for (enabled, invalid, cancelled, source, wanted, events) in [
            (
                false,
                false,
                false,
                Source::Unknown,
                "tool_disabled",
                vec![],
            ),
            (true, true, false, Source::Unknown, "invalid_input", vec![]),
            (true, false, true, Source::Unknown, "cancelled", vec![]),
            (
                true,
                false,
                false,
                Source::Unavailable,
                "store_unavailable",
                vec!["replay", "source"],
            ),
        ] {
            let trace = Arc::new(Trace::default());
            let service = telemetry_service(
                trace.clone(),
                enabled,
                Ok(Replay::Missing),
                Ok(Submission::Size),
                source,
            );
            let mut input = observation();
            if invalid {
                input.status = "invalid".into();
            }
            let cancel = Cancellation::default();
            if cancelled {
                cancel.cancel();
            }
            let result = run(service.submit(&tenant(), "result-2", input, &cancel));
            if cancelled {
                assert_eq!(result, Err(Failure::Cancelled));
            } else {
                assert_eq!(result.unwrap()["error"]["code"], wanted);
            }
            assert_eq!(trace.events(), events);
        }
        let trace = Arc::new(Trace::default());
        let service = telemetry_service(
            trace.clone(),
            true,
            Err(Failure::Unavailable),
            Ok(Submission::Size),
            Source::Unknown,
        );
        assert_eq!(
            run(service.submit(
                &tenant(),
                "result-2",
                observation(),
                &Cancellation::default()
            )),
            Err(Failure::Unavailable)
        );
        assert_eq!(trace.events(), ["replay"]);
    }

    #[test]
    fn submit_normalizes_observation_before_repository_and_never_claims_verification() {
        let trace = Arc::new(Trace::default());
        let service = service(
            trace.clone(),
            true,
            Replay::Missing,
            Source::Live(json!({"query":"q"})),
            Retained {
                receipts: vec![],
                total: 0,
            },
        );
        let mut invalid = observation();
        invalid.status = "unrecognized".into();
        let value =
            run(service.submit(&tenant(), "result-2", invalid, &Cancellation::default())).unwrap();
        assert_eq!(value["error"]["code"], "invalid_input");
        assert!(trace.events().is_empty());

        let value = run(service.submit(
            &tenant(),
            "result-2",
            observation(),
            &Cancellation::default(),
        ))
        .unwrap();
        assert_eq!(value["state"], "created");
        assert_eq!(value["receipt"]["observation"]["status"], "succeeded");
        assert_eq!(
            value["receipt"]["observation"]["captured_at"],
            "2026-01-02T03:04:05Z"
        );
        assert_eq!(value["receipt"]["observations_verified"], false);
        assert_eq!(trace.events(), vec!["replay", "source", "submit"]);
    }

    #[test]
    fn read_rejects_bad_limit_before_grant_or_repository_and_uses_retained_discovery_after_expiry()
    {
        let trace = Arc::new(Trace::default());
        let disabled = service(
            trace.clone(),
            false,
            Replay::Missing,
            Source::Unknown,
            Retained {
                receipts: vec![],
                total: 0,
            },
        );
        let invalid =
            run(disabled.read(&tenant(), "result-1", 0, &Cancellation::default())).unwrap();
        assert_eq!(invalid["error"]["code"], "invalid_input");
        assert!(trace.events().is_empty());

        let trace = Arc::new(Trace::default());
        let saved = json!({"receipt_id":"receipt-1","discovery":{"query":"original"}});
        let enabled = service(
            trace.clone(),
            true,
            Replay::Missing,
            Source::Expired(WallClockTimestamp::new(90.).unwrap()),
            Retained {
                receipts: vec![saved],
                total: 1,
            },
        );
        let value = run(enabled.read(&tenant(), "result-1", 5, &Cancellation::default())).unwrap();
        assert_eq!(value["source_snapshot_status"], "expired");
        assert_eq!(value["discovery"]["query"], "original");
        assert_eq!(trace.events(), vec!["read", "source"]);
    }

    #[test]
    fn manifest_conflict_precedes_ports_and_valid_source_resolution_precedes_item_reads() {
        let source = ArtifactRef::new(ssx_domain::ArtifactKind::ResearchJob, "job-source").unwrap();
        let trace = Arc::new(Trace::default());
        let service = manifest_service(trace.clone());
        let conflict = run(service.manifest_draft(
            &tenant(),
            ManifestDraft {
                source: Some(Ok(source.clone())),
                result_ids: Some(vec![]),
                artifacts: None,
                max_depth: 2,
                max_nodes: 100,
            },
            &Cancellation::default(),
        ))
        .unwrap();
        assert_eq!(conflict["error"]["code"], "input_conflict");
        assert!(trace.events().is_empty());

        let manifest = run(service.manifest_draft(
            &tenant(),
            ManifestDraft {
                source: Some(Ok(source)),
                result_ids: None,
                artifacts: None,
                max_depth: 2,
                max_nodes: 100,
            },
            &Cancellation::default(),
        ))
        .unwrap();
        assert_eq!(manifest["items"][0]["result_id"], "result-1");
        assert_eq!(manifest["source"]["artifact"]["kind"], "research_job");
        assert_eq!(trace.events(), vec!["manifest_source", "read", "source"]);
    }

    #[test]
    fn manifest_artifact_walk_completes_before_the_selected_result_item_is_read() {
        let trace = Arc::new(Trace::default());
        let service = manifest_walk_service(trace.clone());
        let artifact = ArtifactRef::new(ssx_domain::ArtifactKind::Result, "result-1").unwrap();
        let manifest = run(service.manifest_draft(
            &tenant(),
            ManifestDraft {
                source: None,
                result_ids: None,
                artifacts: Some(vec![Ok(artifact)]),
                max_depth: 2,
                max_nodes: 100,
            },
            &Cancellation::default(),
        ))
        .unwrap();
        assert_eq!(manifest["items"][0]["result_id"], "result-1");
        assert_eq!(trace.events(), vec!["walk", "read", "source"]);
    }
}
#[derive(Clone, Debug)]
pub struct ManifestRequest {
    pub result_ids: Option<Vec<String>>,
    pub artifacts: Option<Vec<ArtifactRef>>,
    pub max_depth: usize,
    pub max_nodes: usize,
    pub source: Option<ArtifactRef>,
}
impl Default for ManifestRequest {
    fn default() -> Self {
        Self {
            result_ids: None,
            artifacts: None,
            max_depth: 2,
            max_nodes: 100,
            source: None,
        }
    }
}
impl ManifestRequest {
    pub fn new() -> Self {
        Self::default()
    }
}

/// W07-specific function boundary, not a general argument validator. Errors are
/// the pure artifact parser's messages; caller-supplied JSON error objects are
/// never accepted. Presence (including an empty explicit selection) is retained.
#[derive(Clone, Debug)]
pub struct ManifestDraft {
    pub result_ids: Option<Vec<String>>,
    pub artifacts: Option<Vec<Result<ArtifactRef, String>>>,
    pub max_depth: usize,
    pub max_nodes: usize,
    pub source: Option<Result<ArtifactRef, String>>,
}
impl From<ManifestRequest> for ManifestDraft {
    fn from(request: ManifestRequest) -> Self {
        Self {
            result_ids: request.result_ids,
            artifacts: request
                .artifacts
                .map(|roots| roots.into_iter().map(Ok).collect()),
            max_depth: request.max_depth,
            max_nodes: request.max_nodes,
            source: request.source.map(Ok),
        }
    }
}
impl Default for ManifestDraft {
    fn default() -> Self {
        Self::from(ManifestRequest::default())
    }
}
