//! Transport-neutral staged admission, replay, retry and retained reads. These
//! paths never invoke search; polling runner discovery consumes durable hints.
use crate::{
    staged_runner::{Event, Events, StagedAuthority},
    valkey::{
        Failure, Valkey,
        saved::{Error as SavedError, RecentReport, SavedStore},
        scripts::Staged,
        staged::{RecordRead, StagedStore},
    },
};
use ssx_application::{
    PortFuture,
    policy::PolicyService,
    saved::{SavedLimits, SavedPolicy},
    snapshot::{SnapshotClock, SnapshotLookup, SnapshotService},
    staged::{self, Error, Input, Limits, Plan},
};
use ssx_domain::{
    ArtifactKind, ArtifactRef, ResultId, SavedSearchId, SearchResult, SnapshotId, StagedSearchId,
    TenantId, canonical::parse_composite_artifact_id, staged::Record,
};
use ssx_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
pub trait Sources: Send + Sync {
    fn resolve<'a>(
        &'a self,
        tenant: &'a TenantId,
        source: &'a Value,
    ) -> PortFuture<'a, Result<Value, Error>>;
}
/// Immutable source policy composition; W09 supplies fresh composition when
/// grants/config change, or a live Sources implementation. No hot reload claim.
pub struct SavedReportSources {
    pub store: SavedStore,
    pub search: PolicyService,
    pub granted: bool,
    pub auth_configured: BTreeMap<String, bool>,
    pub limits: SavedLimits,
}
fn report_reasons(report: &Value) -> Vec<String> {
    let mut reasons = report["incomparable_reasons"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| v.to_string())
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    reasons.sort();
    reasons.dedup();
    reasons
}
fn source_artifact(source: &Value) -> Result<ArtifactRef, Error> {
    let rejected = |message| Error::new("unsupported_source_contract", "source", message);
    let object = source
        .as_object()
        .filter(|object| {
            object.len() == 4
                && ["contract", "version", "kind", "id"]
                    .iter()
                    .all(|key| object.contains_key(*key))
        })
        .ok_or_else(|| rejected("artifact must contain exactly contract, version, kind, and id"))?;
    if object["contract"] != json!("slopsearx.artifact_ref")
        || !staged::python_equal(&object["version"], &json!(1))
    {
        return Err(rejected(
            "unsupported artifact reference contract or version",
        ));
    }
    let (Some(kind), Some(id)) = (object["kind"].as_str(), object["id"].as_str()) else {
        return Err(rejected("artifact kind and id must be strings"));
    };
    let kind: ArtifactKind =
        ssx_json::from_value(json!(kind)).map_err(|_| rejected("unsupported artifact kind"))?;
    if id.is_empty() || id.chars().count() > 1024 {
        return Err(rejected("artifact id must contain 1 to 1024 characters"));
    }
    if id.chars().any(|c| c.is_ascii_control()) {
        return Err(rejected("artifact id contains control characters"));
    }
    ArtifactRef::new(kind, id).map_err(Error::from)
}
impl Sources for SavedReportSources {
    fn resolve<'a>(
        &'a self,
        tenant: &'a TenantId,
        source: &'a Value,
    ) -> PortFuture<'a, Result<Value, Error>> {
        Box::pin(async move {
            let artifact = source_artifact(source)?;
            if artifact.kind() != ArtifactKind::SavedReport {
                let kind = artifact.to_value()["kind"].as_str().unwrap().to_string();
                let mut error = Error::new(
                    "unsupported_transition",
                    "",
                    format!("{kind} cannot be used as a source for staged_search"),
                );
                error.details = Box::new(json!({"source_kind":kind,"destination":"staged_search"}));
                return Err(error);
            }
            if !self.granted {
                return Err(Error::new(
                    "source_policy_denied",
                    "",
                    "saved-search access is disabled by current policy",
                ));
            }
            let parts = parse_composite_artifact_id(artifact.id(), 2).map_err(|_| {
                Error::new(
                    "source_not_found",
                    "",
                    "source saved-search report was not found",
                )
            })?;
            let id = SavedSearchId::new(&parts[0]).map_err(|_| {
                Error::new(
                    "source_not_found",
                    "",
                    "source saved-search report was not found",
                )
            })?;
            let policy = SavedPolicy {
                search: &self.search,
                saved_granted: self.granted,
                auth_configured: &self.auth_configured,
                limits: &self.limits,
            };
            let report = self
                .store
                .for_tenant(tenant.clone())
                .recent_report(&id, &parts[1], &policy)
                .await
                .map_err(|e| match e {
                    SavedError::Store(_) => Error::new(
                        "source_unavailable",
                        "",
                        "saved-search store is unavailable",
                    ),
                    SavedError::Invalid(_) | SavedError::Conflict => Error::new(
                        "source_policy_denied",
                        "",
                        "current policy denies the source artifact",
                    ),
                    _ => Error::new(
                        "source_not_found",
                        "",
                        "source saved-search report was not found",
                    ),
                })?;
            match report {
                None => Err(Error::new(
                    "source_not_found",
                    "",
                    "source saved-search report was not found",
                )),
                Some(RecentReport::Truncated(r)) => {
                    let mut error = Error::new(
                        "source_truncated",
                        "",
                        "source saved-search report is truncated",
                    );
                    error.details = Box::new(json!({"reasons":report_reasons(&r)}));
                    Err(error)
                }
                Some(RecentReport::Incomparable(r)) => {
                    let reasons = report_reasons(&r);
                    let mut error = Error::new(
                        "source_incomparable",
                        "",
                        if r["status"].as_str() == Some("incomparable") || !reasons.is_empty() {
                            "source saved-search report is incomparable"
                        } else {
                            "source saved-search report lacks a complete observation window"
                        },
                    );
                    if r["status"].as_str() == Some("incomparable") || !reasons.is_empty() {
                        error.details = Box::new(json!({"reasons":reasons}));
                    }
                    Err(error)
                }
                Some(RecentReport::Found(r)) => Ok(
                    json!({"artifact":artifact.to_value(),"status":"live","result_ids":[],"query":r["observation"]["window"]["query"],"details":{"search_id":parts[0],"run_id":parts[1],"report_status":r["status"],"revision":r["revision"]}}),
                ),
            }
        })
    }
}
#[derive(Clone)]
pub struct Service {
    pub store: Arc<Valkey>,
    pub policy: PolicyService,
    pub limits: Limits,
    pub authority: Arc<dyn StagedAuthority>,
    pub clock: Arc<dyn SnapshotClock>,
    pub snapshots: SnapshotService,
    pub sources: Option<Arc<dyn Sources>>,
    pub events: Arc<dyn Events>,
}
pub struct Submission {
    pub query: Option<String>,
    pub objectives: Value,
    pub initial_scope: Value,
    pub fallback_scope: Option<Value>,
    pub allow_scope_expansion: bool,
    pub filters: Option<Value>,
    pub idempotency_key: String,
    pub source: Option<Value>,
}
fn store_error(_: Failure) -> Error {
    Error::new(
        "store_unavailable",
        "",
        "staged search store is unavailable",
    )
}
fn random_id() -> Result<StagedSearchId, Error> {
    let mut bytes = [0; 16];
    aws_lc_rs::rand::fill(&mut bytes)
        .map_err(|_| Error::new("store_unavailable", "", "identity source unavailable"))?;
    StagedSearchId::new(bytes.iter().map(|b| format!("{b:02x}")).collect::<String>())
        .map_err(Error::from)
}
impl Service {
    pub fn preview(&self, input: &Input) -> Result<Value, Error> {
        let plan = Plan::build(&self.policy, &self.limits, input)?;
        Ok(
            json!({"contract":"slopsearx.staged_search","version":1,"plan":plan.value,"plan_digest":plan.value["digest"],"objectives":{"requested":plan.value["requested_objectives"],"effective":plan.value["effective_objectives"]},"dispatch":false}),
        )
    }
    pub async fn submit(
        &self,
        tenant: &TenantId,
        submission: &Submission,
    ) -> Result<Record, Error> {
        if !self.limits.granted {
            return Err(staged::disabled());
        }
        let key = submission
            .idempotency_key
            .trim_matches(ssx_json::python_whitespace);
        if key.is_empty() || key.chars().count() > 128 {
            return Err(Error::new(
                "invalid_input",
                "idempotency_key",
                "idempotency_key must contain 1 to 128 characters",
            ));
        }
        let source = if let Some(source) = &submission.source {
            if submission.query.is_some() {
                return Err(Error::new(
                    "input_conflict",
                    "query",
                    "query is derived from source and must be omitted",
                ));
            }
            Some(
                self.sources
                    .as_ref()
                    .ok_or_else(|| {
                        Error::new(
                            "source_unavailable",
                            "source",
                            "source resolver is unavailable",
                        )
                    })?
                    .resolve(tenant, source)
                    .await?,
            )
        } else {
            None
        };
        let query = source
            .as_ref()
            .and_then(|s| s["query"].as_str())
            .map(str::to_owned)
            .or_else(|| submission.query.clone())
            .ok_or_else(|| {
                Error::new(
                    "invalid_input",
                    "query",
                    "query is required when source is omitted",
                )
            })?;
        if !submission.objectives.is_object() {
            return Err(Error::new(
                "invalid_input",
                "objectives",
                "objectives is required",
            ));
        }
        if !submission.initial_scope.is_object() {
            return Err(Error::new(
                "invalid_input",
                "initial_scope",
                "initial_scope is required",
            ));
        }
        let input = Input {
            query,
            objectives: submission.objectives.clone(),
            initial_scope: submission.initial_scope.clone(),
            fallback_scope: submission.fallback_scope.clone(),
            allow_scope_expansion: submission.allow_scope_expansion,
            filters: submission.filters.clone(),
        };
        let store = StagedStore::new(&self.store);
        match store
            .find_by_idempotency(tenant, key, self.clock.now())
            .await
            .map_err(|_| {
                Error::new(
                    "store_unavailable",
                    "",
                    "staged search requires connected Valkey",
                )
            })? {
            RecordRead::Found(existing) => {
                if existing.value()["composition"]["source"]["artifact"]
                    != source
                        .as_ref()
                        .map(|v| v["artifact"].clone())
                        .unwrap_or(Value::Null)
                {
                    return Err(Error::new(
                        "idempotency_conflict",
                        "",
                        "idempotency key refers to a different composition source",
                    ));
                }
                if !staged::replay_equivalent(&existing, &input) {
                    return Err(Error::new(
                        "idempotency_conflict",
                        "",
                        "idempotency key refers to a different plan",
                    ));
                }
                self.authority.validate(&existing)?;
                return Ok(existing);
            }
            RecordRead::Expired { expires_at } => {
                let mut error = Error::new("expired_handle", "", "staged operation expired");
                error.details = Box::new(json!({"expires_at":expires_at}));
                return Err(error);
            }
            RecordRead::Missing => (),
        }
        let plan = Plan::build(&self.policy, &self.limits, &input)?;
        let mut record = plan.record(tenant, &random_id()?, self.clock.now().seconds())?;
        if let Some(source) = source {
            let mut v = record.value().clone();
            v["composition"] = json!({"source":source});
            v["lineage"] = json!([{"from":ArtifactRef::new(ArtifactKind::StagedSearch,record.id().as_str())?.to_value(),"relation":"derived_from","to":source["artifact"]}]);
            record = Record::from_value(v)?;
        }
        self.authority.validate(&record)?;
        let scopes = plan.scopes.iter().collect::<Vec<_>>();
        let transition = store
            .admit(&self.policy, &record, key, &scopes, self.clock.now())
            .await
            .map_err(|_| {
                Error::new(
                    "store_unavailable",
                    "",
                    "staged search requires connected Valkey",
                )
            })?;
        match transition.status {
            Staged::Created | Staged::Replay => {
                let stored = transition.record.ok_or_else(|| {
                    Error::new("store_unavailable", "", "missing admission acknowledgment")
                })?;
                self.authority.validate(&stored)?;
                if transition.status == Staged::Created {
                    self.events.acknowledged(Event {
                        from: None,
                        to: "queued".into(),
                        record: stored.clone(),
                    });
                }
                Ok(stored)
            }
            Staged::Quota => Err(Error::new(
                "operation_quota_exceeded",
                "",
                "tenant has 32 retained staged operations",
            )),
            Staged::Conflict => Err(Error::new(
                "idempotency_conflict",
                "",
                "idempotency key refers to a different plan",
            )),
            _ => Err(Error::new(
                "store_unavailable",
                "",
                "staged admission unavailable",
            )),
        }
    }
    pub async fn get(&self, tenant: &TenantId, id: &StagedSearchId) -> Result<Record, Error> {
        if !self.limits.granted {
            return Err(staged::disabled());
        }
        match StagedStore::new(&self.store)
            .read(tenant, id, self.clock.now())
            .await
            .map_err(store_error)?
        {
            RecordRead::Found(record) => {
                self.authority.validate(&record)?;
                Ok(record)
            }
            RecordRead::Expired { expires_at } => {
                let mut error = Error::new("expired_handle", "", "staged operation expired");
                error.details = Box::new(json!({"expires_at":expires_at}));
                Err(error)
            }
            RecordRead::Missing => Err(Error::new(
                "invalid_operation_id",
                "",
                "unknown staged operation",
            )),
        }
    }
    pub async fn retry(
        &self,
        tenant: &TenantId,
        id: &StagedSearchId,
        key: &str,
    ) -> Result<Record, Error> {
        if !self.limits.granted {
            return Err(staged::disabled());
        }
        let key = key.trim_matches(ssx_json::python_whitespace);
        if key.is_empty() || key.chars().count() > 128 {
            return Err(Error::new(
                "invalid_input",
                "retry_key",
                "retry_key must contain 1 to 128 characters",
            ));
        }
        let before = match StagedStore::new(&self.store)
            .read(tenant, id, self.clock.now())
            .await
            .map_err(store_error)?
        {
            RecordRead::Found(record) => {
                self.authority.validate(&record)?;
                Some(record)
            }
            _ => None,
        };
        let transition = StagedStore::new(&self.store)
            .request_retry(tenant, id, key, self.clock.now())
            .await
            .map_err(store_error)?;
        let failure = match transition.status {
            Staged::Budget => Some((
                "budget_exhausted",
                "remaining budget cannot fund the full stage",
            )),
            Staged::Deadline => Some(("deadline_exceeded", "operation deadline has passed")),
            Staged::Busy => Some(("operation_busy", "operation already queued or running")),
            Staged::None => Some(("no_retryable_work", "operation has no retryable stage")),
            Staged::Expired => Some(("expired_handle", "staged operation expired")),
            Staged::Missing => Some(("invalid_operation_id", "unknown staged operation")),
            Staged::Queued | Staged::Replay => None,
            _ => Some(("store_unavailable", "staged retry unavailable")),
        };
        if let Some((code, message)) = failure {
            return Err(Error::new(code, "", message));
        }
        let record = transition
            .record
            .ok_or_else(|| Error::new("store_unavailable", "", "missing retry acknowledgment"))?;
        self.authority.validate(&record)?;
        if transition.status == Staged::Queued {
            self.events.acknowledged(Event {
                from: before.as_ref().and_then(|r| match r.state() {
                    "failed" => Some("failed"),
                    "interrupted" => Some("interrupted"),
                    _ => None,
                }),
                to: "queued".into(),
                record: record.clone(),
            });
        }
        Ok(record)
    }
    /// Projection remains consumer-owned; MCP uses its shared result-card codec.
    /// No provider work is performed, and max_results never changes stored evidence.
    pub async fn render(
        &self,
        record: &Record,
        view: &View,
        project: impl Fn(&SearchResult, &ResultId, bool) -> Value,
    ) -> Result<Value, Error> {
        self.authority.validate(record)?;
        let v = record.value();
        let mut results = vec![];
        let mut meta = json!({"cursor":null,"artifact":null,"query_id":null,"total":0,"returned":0,"has_more":false,"selected_result_attempt_id":v["selected_result_attempt_id"],"result_status":"not_available"});
        let selected = v["selected_result_attempt_id"].as_str();
        let attempt = v["stages"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|s| s["attempts"].as_array().unwrap())
            .find(|a| a["attempt_id"].as_str() == selected);
        if let Some(a) = attempt
            && let Some(cursor) = a["cursor"].as_str()
        {
            let id = SnapshotId::new(cursor)?;
            match self.snapshots.for_tenant(record.tenant()).read(&id).await {
                SnapshotLookup::Found(snapshot) => {
                    self.authority.validate(record)?;
                    if view.include.iter().any(|s| s == "results") {
                        for (i, result) in snapshot
                            .results()
                            .iter()
                            .take(view.limit as usize)
                            .enumerate()
                        {
                            results.push(project(
                                result,
                                &ResultId::new(id.clone(), i as u64),
                                view.include.iter().any(|s| s == "payload"),
                            ));
                        }
                    }
                    meta["cursor"] = json!(cursor);
                    meta["artifact"] = ArtifactRef::new(ArtifactKind::Snapshot, cursor)?.to_value();
                    meta["query_id"] = a["query_id"].clone();
                    meta["total"] = json!(snapshot.total());
                    meta["returned"] = json!(results.len());
                    meta["has_more"] = json!(snapshot.total() > results.len());
                    meta["result_status"] = json!("available");
                }
                SnapshotLookup::Expired { .. } => meta["result_status"] = json!("expired_handle"),
                _ => (),
            }
        }
        self.authority.validate(record)?;
        meta["elapsed_ms"] = json!(
            ((self.clock.now().seconds() - v["accepted_at"].as_f64().unwrap()) * 1000.).max(0.)
                as u64
        );
        let mut budget = v["budget"].clone();
        budget["remaining"] = json!(
            budget["limit"]
                .as_u64()
                .unwrap()
                .saturating_sub(budget["reserved"].as_u64().unwrap())
        );
        let mut out = json!({"contract":"slopsearx.staged_search","version":1,"operation_id":v["operation_id"],"artifact":ArtifactRef::new(ArtifactKind::StagedSearch,record.id().as_str())?.to_value(),"state":v["state"],"stop_reason":v["stop_reason"],"accepted_at":v["accepted_at"],"execution_deadline_at":v["execution_deadline_at"],"expires_at":v["expires_at"],"plan_digest":v["plan_digest"],"plan":v["plan"],"objectives":v["objectives"],"budget":budget,"stages":v["stages"],"results":results,"meta":meta});
        if !v["composition"].is_null() {
            out["source"] = v["composition"]["source"].clone();
            out["lineage"] = v["lineage"].clone();
        }
        Ok(out)
    }
}
#[derive(Debug)]
pub struct View {
    pub include: Vec<String>,
    pub limit: u64,
}
impl View {
    pub fn parse(
        include: Option<&Value>,
        max_results: Option<&Value>,
        operator_max: u64,
    ) -> Result<Self, Error> {
        let mut fields = match include {
            None => vec![],
            Some(v) => v
                .as_array()
                .ok_or_else(|| {
                    Error::new(
                        "invalid_input",
                        "include",
                        "include contains unsupported fields",
                    )
                })?
                .iter()
                .map(|v| {
                    v.as_str().map(str::to_owned).ok_or_else(|| {
                        Error::new(
                            "invalid_input",
                            "include",
                            "include contains unsupported fields",
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        };
        if fields.iter().any(|s| {
            ![
                "results",
                "suggestions",
                "engine_status",
                "diagnostics",
                "payload",
            ]
            .contains(&s.as_str())
        }) {
            return Err(Error::new(
                "invalid_input",
                "include",
                "include contains unsupported fields",
            ));
        }
        if fields.is_empty() {
            fields = vec!["results".into(), "engine_status".into()];
        }
        let limit = match max_results {
            None => operator_max,
            Some(v) => v
                .as_u64()
                .filter(|n| *n > 0)
                .ok_or_else(|| {
                    Error::new(
                        "invalid_input",
                        "max_results",
                        "max_results must be a positive integer",
                    )
                })?
                .min(operator_max),
        };
        Ok(Self {
            include: fields,
            limit,
        })
    }
}
