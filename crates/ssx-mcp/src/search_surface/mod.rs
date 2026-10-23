//! Native T06 search/discovery surface. Engines and shared stores are injected,
//! while protocol dispatch and network authorization remain the same runtime.
pub(crate) mod entities;
mod resources;
mod results;
mod runtime_discovery;
pub(crate) mod validation;
pub use runtime_discovery::{RuntimeDiagnostics, RuntimeDiagnosticsState, RuntimeDiscovery};

use rmcp::{ErrorData, RoleServer, ServerHandler, model::*, service::RequestContext};
use ssx_application::{policy::*, snapshot::*};
use ssx_domain::{SearchRequest, SearchResponse, SnapshotId, TenantId};
use ssx_infra::search::{SearchCancellation, SearchError, SearchService};
use ssx_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};

fn source_contract() -> &'static Value {
    static SOURCE: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    SOURCE.get_or_init(|| {
        ssx_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/t06/source.json"
        ))
        .expect("frozen source contract")
    })
}
pub const TOOL_NAMES: [&str; 11] = [
    "slopsearx_search",
    "slopsearx_search_targeted",
    "slopsearx_search_jobs",
    "slopsearx_search_security",
    "slopsearx_search_science",
    "slopsearx_list_capabilities",
    "slopsearx_explain_search_scope",
    "slopsearx_get_service_status",
    "slopsearx_read_results",
    "slopsearx_read_result",
    "slopsearx_read_entities",
];
/// Trusted runtime observation port. Implementations project the effective
/// registry/config and passive health; they never probe providers or reveal keys.
pub trait DiscoveryState: Send + Sync {
    fn capabilities(&self) -> Vec<Value>;
    fn diagnostics(&self) -> Value;
}
#[derive(Clone)]
pub struct SearchState {
    pub service: SearchService,
    pub snapshots: SnapshotService,
    pub discovery: Arc<dyn DiscoveryState>,
    pub max_results: u64,
}
impl SearchState {
    pub fn new(
        service: SearchService,
        snapshots: SnapshotService,
        discovery: Arc<dyn DiscoveryState>,
        max_results: u64,
    ) -> Result<Self, &'static str> {
        if max_results == 0 {
            return Err("MCP_MAX_RESULTS must be positive");
        }
        Ok(Self {
            service,
            snapshots,
            discovery,
            max_results,
        })
    }
}
#[derive(Clone)]
pub struct SearchSurface {
    state: Arc<SearchState>,
    evidence: Option<Arc<crate::evidence::EvidenceRuntime>>,
    wire: Option<Arc<std::sync::Mutex<crate::opaque::WireState>>>,
    connection_tenant: Option<TenantId>,
}
impl SearchSurface {
    pub fn new(state: SearchState) -> Self {
        Self {
            state: Arc::new(state),
            evidence: None,
            wire: None,
            connection_tenant: None,
        }
    }
    /// Attach the existing borrowed evidence graph before sharing the surface.
    /// Its four tools are advertised by the matching runtime catalog.
    pub fn with_evidence(
        mut self,
        evidence: Arc<crate::evidence::EvidenceRuntime>,
    ) -> Result<Self, &'static str> {
        if Arc::strong_count(&self.state) != 1 || self.wire.is_some() {
            return Err("surface already shared or bound");
        }
        self.evidence = Some(evidence);
        Ok(self)
    }
    pub fn evidence(&self) -> Option<&crate::evidence::EvidenceRuntime> {
        self.evidence.as_deref()
    }
    /// Trusted per-connection state supplied by the qualified transport factory.
    pub fn bind_connection(
        mut self,
        wire: Arc<std::sync::Mutex<crate::opaque::WireState>>,
        client: Option<crate::oauth::AuthenticatedClient>,
    ) -> Self {
        self.wire = Some(wire);
        self.connection_tenant = client.map(|c| c.0);
        self
    }
    pub fn state(&self) -> &SearchState {
        &self.state
    }
    pub fn descriptors() -> serde_json::Value {
        serde_json::from_str::<serde_json::Value>(include_str!(
            "../../../../docs/rust-rewrite/t06/source.json"
        ))
        .expect("frozen source descriptors")["catalog"]
            .clone()
    }
    /// Complete captured descriptors are retained by the accepted opaque
    /// transport overlay; SDK decoding is independently compared in tests.
    pub fn catalogs() -> Arc<crate::opaque::Catalogs> {
        let source: BTreeMap<String, Box<serde_json::value::RawValue>> = serde_json::from_str(
            include_str!("../../../../docs/rust-rewrite/t06/source.json"),
        )
        .expect("source capture");
        let catalog: BTreeMap<String, Box<serde_json::value::RawValue>> =
            serde_json::from_str(source["catalog"].get()).expect("catalog");
        let mut catalogs: crate::opaque::Catalogs = [
            ("tools/list", "tools"),
            ("resources/list", "resources"),
            ("resources/templates/list", "resourceTemplates"),
            ("prompts/list", "prompts"),
        ]
        .into_iter()
        .map(|(method, key)| {
            (
                method.into(),
                serde_json::value::RawValue::from_string(format!(
                    "{{{key:?}:{}}}",
                    catalog[key].get()
                ))
                .expect("catalog result"),
            )
        })
        .collect();
        let names = serde_json::to_string(&TOOL_NAMES).expect("native names");
        catalogs.insert(
            "ssx/native-tools".into(),
            serde_json::value::RawValue::from_string(names.clone()).expect("names"),
        );
        catalogs.insert(
            "ssx/native-raw-arguments".into(),
            serde_json::value::RawValue::from_string(names).expect("names"),
        );
        let names: Vec<_> = Self::descriptors()["prompts"]
            .as_array()
            .expect("prompts")
            .iter()
            .map(|v| v["name"].as_str().expect("prompt name").to_owned())
            .collect();
        catalogs.insert(
            "ssx/native-raw-prompts".into(),
            serde_json::value::RawValue::from_string(
                serde_json::to_string(
                    &serde_json::json!({"names":names,"validate_unknown_names":true,"validate_invalid_params":true}),
                )
                .expect("prompt profile"),
            )
            .expect("names"),
        );
        Arc::new(catalogs)
    }
    /// An unattached T06 fixture retains its frozen catalog. The same native
    /// factory uses this instance catalog when it attaches the evidence owner.
    pub fn runtime_descriptors(&self) -> serde_json::Value {
        let mut descriptors = Self::descriptors();
        if self.evidence.is_some() {
            descriptors["tools"]
                .as_array_mut()
                .expect("tools")
                .extend(crate::evidence::tools::descriptors().iter().cloned());
        }
        descriptors
    }
    pub fn runtime_catalogs(&self) -> Arc<crate::opaque::Catalogs> {
        let mut catalogs = Self::catalogs().as_ref().clone();
        if self.evidence.is_some() {
            let names: Vec<_> = TOOL_NAMES
                .into_iter()
                .chain(crate::evidence::tools::TOOL_NAMES)
                .collect();
            for (key, value) in [
                (
                    "tools/list",
                    serde_json::json!({"tools":self.runtime_descriptors()["tools"]}),
                ),
                ("ssx/native-tools", serde_json::json!(names)),
                ("ssx/native-raw-arguments", serde_json::json!(names)),
            ] {
                catalogs.insert(
                    key.into(),
                    serde_json::value::RawValue::from_string(
                        serde_json::to_string(&value).expect("evidence catalog"),
                    )
                    .expect("evidence catalog"),
                );
            }
        }
        Arc::new(catalogs)
    }
    pub async fn execute(
        &self,
        name: &str,
        arguments: serde_json::Map<String, serde_json::Value>,
        tenant: TenantId,
        cancellation: SearchCancellation,
    ) -> Result<Value, ErrorData> {
        let arguments = ssx_json::to_value(arguments)
            .map_err(|e| ErrorData::invalid_params(e.to_string(), None))?
            .as_object()
            .cloned()
            .ok_or_else(|| ErrorData::invalid_params("arguments must be an object", None))?;
        self.execute_lossless(name, arguments, tenant, cancellation)
            .await
    }
    pub async fn execute_lossless(
        &self,
        name: &str,
        arguments: ssx_json::Map<String, Value>,
        tenant: TenantId,
        cancellation: SearchCancellation,
    ) -> Result<Value, ErrorData> {
        self.execute_arguments(
            name,
            validation::Arguments::new(name, arguments),
            tenant,
            cancellation,
        )
        .await
    }
    async fn execute_arguments(
        &self,
        name: &str,
        args: Result<validation::Arguments, ErrorData>,
        tenant: TenantId,
        cancellation: SearchCancellation,
    ) -> Result<Value, ErrorData> {
        let args = args?;
        match name {
            "slopsearx_list_capabilities" => Ok(self.capabilities(&args)),
            "slopsearx_get_service_status" => {
                let details = self.state.discovery.diagnostics();
                let mut v = json!({"status":"ok"});
                for (k, value) in details.as_object().expect("diagnostics object") {
                    v[k.as_str()] = value.clone();
                }
                Ok(v)
            }
            "slopsearx_read_results" | "slopsearx_read_entities" | "slopsearx_read_result" => {
                self.read(name, &args, tenant).await
            }
            _ if crate::evidence::tools::TOOL_NAMES.contains(&name) => {
                let Some(evidence) = &self.evidence else {
                    return Err(ErrorData::invalid_params(
                        format!("Unknown tool: {name}"),
                        None,
                    ));
                };
                let cancel = ssx_application::receipts::Cancellation::default();
                let task = crate::evidence::tools::execute(
                    evidence,
                    name,
                    args.values(),
                    &tenant,
                    &cancel,
                );
                tokio::pin!(task);
                tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => {
                        cancel.cancel();
                        let _ = task.await;
                        Err(ErrorData::internal_error("cancelled", None))
                    }
                    value = &mut task => value,
                }
            }
            _ if TOOL_NAMES.contains(&name) => self.search(name, &args, tenant, cancellation).await,
            _ => Err(ErrorData::invalid_params(
                format!("Unknown tool: {name}"),
                None,
            )),
        }
    }
    fn capabilities(&self, args: &validation::Arguments) -> Value {
        let family = args.string_opt("family");
        let category = args.string_opt("category");
        let disabled = args.boolean("include_disabled", false);
        let entries: Vec<_> = self
            .state
            .discovery
            .capabilities()
            .into_iter()
            .filter(|v| {
                (disabled || v["enabled"].as_bool() == Some(true))
                    && [family, category].into_iter().flatten().all(|s| {
                        v["categories"]
                            .as_array()
                            .is_some_and(|a| a.iter().any(|x| x.as_str() == Some(s)))
                    })
            })
            .map(|mut v| {
                if !args.boolean("include_auth_requirements", true) {
                    v.as_object_mut()
                        .expect("capability object")
                        .shift_remove("auth");
                }
                v
            })
            .collect();
        json!({"engines":entries,"count":entries.len(),"filter":{"family":family,"category":category,"include_disabled":disabled}})
    }
    async fn search(
        &self,
        name: &str,
        args: &validation::Arguments,
        tenant: TenantId,
        cancellation: SearchCancellation,
    ) -> Result<Value, ErrorData> {
        let mut request = SearchRequest {
            query: args.string("query", "").into(),
            client_identifier: Some(format!("mcp:{}", tenant.as_str())),
            include: ["results", "engine_status"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            ..Default::default()
        };
        let mut proposal = ScopeProposal::generic(request.clone());
        proposal.intent = args.string("intent", "auto").into();
        proposal.surface = match name {
            "slopsearx_search_targeted" => SearchSurfaceKind::Targeted,
            "slopsearx_search_jobs" => SearchSurfaceKind::Jobs,
            "slopsearx_search_security" => SearchSurfaceKind::Security,
            "slopsearx_search_science" => SearchSurfaceKind::Science,
            "slopsearx_explain_search_scope" => SearchSurfaceKind::Preview,
            _ => SearchSurfaceKind::Generic,
        };
        request.categories = args.strings("categories");
        request.engines = args.strings("engines");
        request.media_type = args.string_opt("media_type").map(str::to_owned);
        request.language = args.string("language", "en").into();
        request.time_range = args.string_opt("time_range").map(str::to_owned);
        request.freshness = args.string("freshness", "no_preference").into();
        request.date_from = args.string_opt("date_from").map(str::to_owned);
        request.date_to = args.string_opt("date_to").map(str::to_owned);
        if let Some(ms) = args.integer("interactive_timeout_ms") {
            if !(1..=30000).contains(&ms) {
                return Ok(error(
                    "invalid_input",
                    "interactive_timeout_ms must be an integer between 1 and 30000",
                    Some("interactive_timeout_ms"),
                ));
            }
            request.interactive_timeout_ms = Some(ms as u64);
        }
        match args.string("safesearch", "off") {
            "off" => request.safesearch = 0,
            "moderate" => request.safesearch = 1,
            "strict" => request.safesearch = 2,
            _ => {
                return Ok(error(
                    "invalid_input",
                    "safesearch must be off, moderate, or strict",
                    Some("safesearch"),
                ));
            }
        }
        if name == "slopsearx_search"
            && let Some(include) = args.strings("include")
        {
            request.include = include.into_iter().collect();
        }
        request.max_results = None; // Capture full immutable evidence before presentation.
        match proposal.surface {
            SearchSurfaceKind::Jobs => {
                let company = args
                    .string("company", "")
                    .trim_matches(ssx_json::python_whitespace);
                if company.is_empty() {
                    return Ok(error(
                        "invalid_input",
                        "company is required",
                        Some("company"),
                    ));
                }
                let title = args.strings("keywords").unwrap_or_default().join(" ");
                let title = title.trim_matches(ssx_json::python_whitespace);
                request.query = format!(
                    "{} at {company}",
                    if title.is_empty() { "jobs" } else { title }
                );
                request.engines = args.strings("sources");
                proposal.job_location = args.string_opt("location").map(str::to_owned);
                proposal.job_employment_type =
                    args.string_opt("employment_type").map(str::to_owned);
            }
            SearchSurfaceKind::Security => {
                proposal.specialist_types = args.strings("evidence_types").unwrap_or_default()
            }
            SearchSurfaceKind::Science => {
                proposal.specialist_types = args.strings("source_types").unwrap_or_default()
            }
            _ => {}
        }
        proposal.request = request.clone();
        if name == "slopsearx_explain_search_scope" {
            let preview = match self
                .state
                .service
                .preview_scope_with_cancellation(&proposal, cancellation.clone())
                .await
            {
                Ok(v) => v,
                Err(e) => return Ok(search_error(e)),
            };
            let common = scope(&preview.scope, &proposal.intent);
            let mut v = json!({"selected_engines":common["selected_engines"],"excluded_engines":common["excluded_engines"],"routing":common["routing"],"routing_reason":common["routing_reason"],"routing_rule":preview.scope.routing_rule,"matched_topic":preview.scope.matched_topic,"requested_intent":proposal.intent,"media_type":request.media_type,"warnings":warnings(&proposal,&preview)});
            if let Some(jev) = common.get("jev_routing") {
                v["jev_routing"] = jev.clone();
            }
            return Ok(v);
        }
        let authority = match self
            .state
            .service
            .prepare_scope_with_cancellation(&proposal, cancellation.clone())
            .await
        {
            Ok(v) => v,
            Err(e) => return Ok(scope_error(e, &request)),
        };
        let preview = authority.preview().clone();
        let capture = match self
            .state
            .service
            .capture_prepared(authority, cancellation.clone())
            .await
        {
            Ok(v) => v,
            Err(e) => return Ok(search_error(e)),
        };
        let total = capture.full_result_count();
        let view = match capture.present_with_advisories() {
            Ok(v) => v,
            Err(e) => return Ok(search_error(e)),
        };
        // `present` has no presentation bound and preserves include filtering;
        // canonical snapshot creation must use the complete canonical capture.
        // Persist full canonical fields despite include/max_results presentation
        // choices; this is one admitted execution and one snapshot write.
        let captured = match self
            .state
            .service
            .snapshot_read_capture(
                capture,
                &self.state.snapshots.for_tenant(tenant),
                &[],
                cancellation,
            )
            .await
        {
            Ok(v) => v,
            Err(e) => return Ok(search_error(e)),
        };
        let snapshot = captured.snapshot;
        let mut response = view.response;
        let mut notes = caller_warnings(&proposal, &preview);
        let cursor = snapshot.ok().map(|v| v.to_string());
        if cursor.is_none() {
            notes.push("snapshot store unavailable — pagination cursor not created".into());
        }
        // Match the source envelope: caller notes, optional snapshot failure,
        // then the actual service scope, including post-dispatch warnings.
        notes.extend(response.scope.warnings.clone());
        response
            .results
            .truncate(args.bound("max_results", self.state.max_results) as usize);
        let mut enforcement: Value = ssx_json::to_value(
            [
                "language",
                "time_range",
                "safesearch",
                "location",
                "employment_type",
                "date_from",
                "date_to",
            ]
            .into_iter()
            .filter_map(|k| preview.enforcement.get(k).map(|v| (k, v.to_value())))
            .collect::<ssx_json::Map<_, _>>(),
        )
        .expect("enforcement");
        if !enforcement.is_object() {
            enforcement = json!({});
        }
        let wire_intent = match proposal.surface {
            SearchSurfaceKind::Targeted => "explicit engine",
            SearchSurfaceKind::Jobs => "jobs",
            SearchSurfaceKind::Security => "security",
            SearchSurfaceKind::Science => "science",
            _ => &proposal.intent,
        };
        let mut out = envelope(
            &response,
            wire_intent,
            notes,
            cursor.as_deref(),
            total,
            enforcement,
            request.include.contains("payload"),
        );
        if !view.advisories.is_empty() {
            out["meta"]["advisories"] = json!(view.advisories);
        }
        if let Some(date) = response.date_filter {
            out["meta"]["date_filter"] = ssx_json::to_value(&date).expect("date proof");
        }
        Ok(out)
    }
    async fn read(
        &self,
        name: &str,
        args: &validation::Arguments,
        tenant: TenantId,
    ) -> Result<Value, ErrorData> {
        let result = name == "slopsearx_read_result";
        let field = if result { "result_id" } else { "cursor" };
        let handle = args.string(field, "");
        let mut index = None;
        let cursor = if result {
            let Some((id, i)) = handle.rsplit_once(':') else {
                return Ok(error(
                    "invalid_result_id",
                    "result_id must be a server-issued snapshot result ID",
                    Some(field),
                ));
            };
            match result_index(i) {
                Ok(i) => {
                    index = i;
                    id
                }
                Err(message) => return Ok(error("invalid_result_id", message, Some(field))),
            }
        } else {
            handle
        };
        if !result && cursor.trim_matches(ssx_json::python_whitespace).is_empty() {
            return Ok(error("invalid_input", "cursor is required", Some(field)));
        }
        let page = args.integer("page").unwrap_or(1);
        if page < 1 {
            return Ok(error("invalid_input", "page must be >= 1", Some("page")));
        }
        let version = args.integer("version").unwrap_or(1);
        if name == "slopsearx_read_entities" && ![1, 2].contains(&version) {
            return Ok(error(
                "invalid_input",
                "version must be one of (1, 2)",
                Some("version"),
            ));
        }
        let snapshot = match SnapshotId::new(cursor.to_owned()) {
            Ok(id) => match self.state.snapshots.for_tenant(tenant).read(&id).await {
                SnapshotLookup::Found(s) => s,
                SnapshotLookup::Unavailable => {
                    return Ok(error(
                        "store_unavailable",
                        "snapshot store is unavailable",
                        Some(field),
                    ));
                }
                SnapshotLookup::Expired { expires_at } => {
                    let mut v = error("expired_handle", "snapshot has expired", Some(field));
                    v["error"]["handle"] = json!(handle);
                    v["error"]["expires_at"] = json!(
                        ssx_application::iso_timestamp(expires_at.timestamp().seconds()).ok()
                    );
                    return Ok(v);
                }
                SnapshotLookup::Missing => {
                    return Ok(error(
                        "invalid_cursor",
                        if result {
                            "unknown or expired snapshot"
                        } else {
                            "unknown cursor"
                        },
                        Some(field),
                    ));
                }
            },
            Err(_) => {
                return Ok(error(
                    "invalid_cursor",
                    if result {
                        "unknown or expired snapshot"
                    } else {
                        "unknown cursor"
                    },
                    Some(field),
                ));
            }
        };
        if result {
            return Ok(match index.and_then(|i| snapshot.results().get(i)) {
                Some(r) => results::record(r, &snapshot, handle),
                None => error(
                    "invalid_result_id",
                    "result index out of range",
                    Some(field),
                ),
            });
        }
        let size = args.bound("max_results", self.state.max_results) as usize;
        let start = usize::try_from(page - 1)
            .unwrap_or(usize::MAX)
            .saturating_mul(size);
        if name == "slopsearx_read_entities" {
            return Ok(entities::read(&snapshot, page, start, size, version as u8));
        }
        let cards: Vec<_> = snapshot
            .results()
            .iter()
            .enumerate()
            .skip(start)
            .take(size)
            .map(|(i, r)| results::card(r, Some(&format!("{cursor}:{i}")), false))
            .collect();
        Ok(
            json!({"query":snapshot.query(),"cursor":cursor,"artifact":artifact("snapshot",cursor),"page":page,"results":cards,"meta":{"total":snapshot.total(),"ranking":snapshot.ranking_explanation(),"has_more":start.saturating_add(size)<snapshot.total(),"query_id":snapshot.query_id().as_str()}}),
        )
    }
}
use ssx_application::policy::SearchSurface as SearchSurfaceKind;
fn result_index(raw: &str) -> Result<Option<usize>, &'static str> {
    let raw = raw.trim_matches(ssx_json::python_whitespace);
    let negative = raw.starts_with('-');
    let digits = raw.strip_prefix(['+', '-']).unwrap_or(raw);
    let mut digits = digits.chars().peekable();
    let mut previous = false;
    let mut count = 0;
    let mut nonzero = false;
    let mut value = Some(0usize);
    while let Some(ch) = digits.next() {
        if ch == '_' {
            if !previous
                || digits
                    .peek()
                    .copied()
                    .and_then(ssx_json::decimal_value)
                    .is_none()
            {
                return Err("malformed result_id");
            }
            previous = false;
            continue;
        }
        let digit = ssx_json::decimal_value(ch).ok_or("malformed result_id")?;
        count += 1;
        nonzero |= digit != 0;
        value = value
            .and_then(|n| n.checked_mul(10))
            .and_then(|n| n.checked_add(digit as usize));
        previous = true;
    }
    if !previous || count > 4300 {
        return Err("malformed result_id");
    }
    if negative && nonzero {
        return Err("result index out of range");
    }
    Ok(value)
}
fn scope_error(e: SearchError, r: &SearchRequest) -> Value {
    let dates = matches!(&e,SearchError::Policy(p) if p.message=="selected adapter does not enforce publication dates");
    let mut v = search_error(e);
    if dates {
        v["enforcement"] = json!({"engines":{"requested":r.engines,"status":"rejected","reason":"selected adapter does not enforce publication dates","enforced_by":[]}});
    }
    v
}
fn search_error(error: SearchError) -> Value {
    match error {
        SearchError::Policy(e) => {
            let mut v = e.to_value();
            if e.code == "invalid_input" {
                let alternatives = match e.field.as_str() {
                    "intent" => Some(json!([
                        "web",
                        "news",
                        "science",
                        "reference",
                        "code",
                        "social",
                        "historical",
                        "jobs",
                        "security",
                        "medical",
                        "finance",
                        "packages",
                        "media",
                        "images",
                        "videos",
                        "legal",
                        "geography"
                    ])),
                    "media_type" => Some(json!(["image", "video"])),
                    "freshness" => Some(json!(["prefer_cache", "prefer_fresh", "no_preference"])),
                    "evidence_types" => Some(json!([
                        "exploit",
                        "exposure",
                        "malware",
                        "reputation",
                        "threat_intel",
                        "vulnerability"
                    ])),
                    "source_types" => Some(json!([
                        "biomedical",
                        "chemistry",
                        "datasets",
                        "general_reference",
                        "papers",
                        "scholarly_index"
                    ])),
                    _ => None,
                };
                if let Some(a) = alternatives {
                    v["error"]["valid_alternatives"] = a;
                }
            }
            v
        }
        SearchError::ClientLimited => self::error(
            "rate_limited",
            "too many requests; please retry later",
            None,
        ),
        _ => self::error("service_unavailable", "search service is unavailable", None),
    }
}

pub(super) fn error(code: &str, message: &str, field: Option<&str>) -> Value {
    let mut v = json!({"error":{"code":code,"message":message}});
    if let Some(f) = field {
        v["error"]["field"] = json!(f);
    }
    v
}
pub(super) fn artifact(kind: &str, id: &str) -> Value {
    json!({"contract":"slopsearx.artifact_ref","version":1,"kind":kind,"id":id})
}
fn scope(s: &ssx_domain::ScopeDecision, intent: &str) -> Value {
    let excluded: Vec<_> = s
        .excluded_engines
        .iter()
        .map(|e| json!({"engine":e.engine,"reason":e.reason,"stage":e.stage}))
        .collect();
    let routing = if s.routing_rule == "explicit engine" {
        json!({"applied":false})
    } else {
        json!({"applied":true,"fallback":s.routing_fallback,"budget_applied":s.routing_budget_applied,"tradeoffs":s.routing_tradeoffs.iter().map(|t|json!({"kind":t.kind,"detail":t.detail})).collect::<Vec<_>>()})
    };
    let mut v = json!({"requested_intent":intent,"resolved_categories":s.resolved_categories,"selected_engines":s.selected_engines,"routing_reason":s.routing_rule,"excluded_engines":excluded,"routing":routing});
    if !s.jev_added_engines.is_empty() {
        v["jev_routing"] = json!({"added_engines":s.jev_added_engines,"scores":s.jev_scores});
    }
    v
}
fn caller_warnings(p: &ScopeProposal, preview: &ScopePreview) -> Vec<String> {
    let mut v = Vec::new();
    let profiles: Value = ssx_json::from_str(include_str!(
        "../../../../docs/rust-rewrite/f09/profiles.json"
    ))
    .expect("profiles");
    if p.surface == SearchSurfaceKind::Jobs {
        v.push("current ATS adapters return title, URL, location, salary/department where available; they provide no full job descriptions and no cross-ATS global search".into());
        for (key, value) in [
            ("location", &p.job_location),
            ("employment_type", &p.job_employment_type),
        ] {
            if let Some(s) = value.as_ref().filter(|s| !s.is_empty()) {
                v.push(format!("{key} '{s}' is not consumed by current adapters"));
            }
        }
    } else if p.surface == SearchSurfaceKind::Security {
        v.push("results are search findings, not a complete security assessment; absence from the selected engines does not mean absence of a vulnerability or exposure".into());
        v.push(format!(
            "resolved evidence_types: {}",
            if p.specialist_types.is_empty() {
                "vulnerability".into()
            } else {
                p.specialist_types.join(", ")
            }
        ));
    } else if p.surface == SearchSurfaceKind::Science {
        v.push("source provenance and engine coverage are reported, but peer-review status, study quality, and citation completeness are not inferred from search results".into());
        v.push(format!(
            "resolved source_types: {}",
            if p.specialist_types.is_empty() {
                "papers".into()
            } else {
                p.specialist_types.join(", ")
            }
        ));
    } else {
        if p.intent != "auto"
            && p.request.engines.as_ref().is_none_or(Vec::is_empty)
            && p.request.categories.as_ref().is_none_or(Vec::is_empty)
        {
            let profile = &profiles["intents"][p.intent.as_str()];
            let detail = if profile["engines"].as_array().is_some_and(|v| !v.is_empty()) {
                String::from("explicit engines")
            } else if let Some(media) = profile["media_types"]
                .as_array()
                .and_then(|v| v.first())
                .and_then(Value::as_str)
            {
                format!("media type '{media}'")
            } else {
                String::from("categories")
            };
            v.push(format!("intent profile '{}' selected {detail}", p.intent));
        }
        if p.surface != SearchSurfaceKind::Preview {
            for key in ["language", "time_range"] {
                if let Some(e) = preview.enforcement.get(key)
                    && e.status == ssx_application::filters::EnforcementStatus::Unsupported
                {
                    v.push(format!(
                        "{key} '{}' is not enforced by selected adapters",
                        e.requested.as_str().unwrap_or_default()
                    ));
                }
            }
            if p.request.safesearch == 1
                && preview.enforcement.get("safesearch").is_none_or(|e| {
                    e.status != ssx_application::filters::EnforcementStatus::Enforced
                })
            {
                v.push(
                    "moderate safesearch is requested but may not be enforced by every adapter"
                        .into(),
                );
            }
        }
    }
    for key in ["date_from", "date_to"] {
        if let Some(e) = preview.enforcement.get(key)
            && e.status != ssx_application::filters::EnforcementStatus::Enforced
        {
            v.push(e.reason.clone());
        }
    }
    v
}
fn warnings(p: &ScopeProposal, preview: &ScopePreview) -> Vec<String> {
    let mut v = caller_warnings(p, preview);
    v.extend(preview.scope.warnings.clone());
    v
}
fn envelope(
    r: &SearchResponse,
    intent: &str,
    warnings: Vec<String>,
    cursor: Option<&str>,
    total: usize,
    enforcement: Value,
    include_payload: bool,
) -> Value {
    let scope = scope(&r.scope, intent);
    if r.all_unresponsive {
        let mut v = error(
            "all_engines_failed",
            if r.deadline_exceeded {
                "interactive deadline reached before any engine responded"
            } else {
                "every selected engine failed to respond"
            },
            None,
        );
        v["error"]["deadline_exceeded"] = json!(r.deadline_exceeded);
        v["error"]["query_id"] = json!(r.query_id);
        v["error"]["scope"] = scope;
        v["error"]["engine_outcomes"]=json!(r.engine_outcomes.iter().map(|o|json!({"engine":o.engine,"status":o.status,"result_count":o.result_count,"message":o.message})).collect::<Vec<_>>());
        v["error"]["retry_guidance"] = json!(if r.deadline_exceeded {
            "increase or omit interactive_timeout_ms and retry"
        } else {
            "check the engine outcomes, adjust scope, and retry"
        });
        return v;
    }
    json!({"query":r.query,"results":r.results.iter().enumerate().map(|(i,v)|results::card(v,cursor.map(|c|format!("{c}:{i}")).as_deref(),include_payload)).collect::<Vec<_>>(),"scope":scope,"answers":r.answers,"corrections":r.corrections,"infoboxes":r.infoboxes,"empty_engines":r.empty_engines.iter().map(|e|json!({"engine":e.first(),"reason":e.get(1)})).collect::<Vec<_>>(),"enforcement":enforcement,"engine_outcomes":r.engine_outcomes.iter().map(|o|json!({"engine":o.engine,"status":o.status,"result_count":o.result_count,"latency_ms":o.latency_ms,"message":o.message})).collect::<Vec<_>>(),"meta":{"query_id":r.query_id,"cached":r.cached,"cached_error":r.cached_error,"response_time_ms":r.response_time_ms,"partial":r.partial,"deadline_exceeded":r.deadline_exceeded,"ranking":r.ranking_explanation,"cursor":cursor,"artifact":cursor.map(|s|artifact("snapshot",s)),"suggestions":r.suggestions,"total":total,"has_more":total>r.results.len()},"warnings":warnings})
}
fn original_arguments(raw: &str, name: &str) -> Result<validation::Arguments, ErrorData> {
    type RawMap = BTreeMap<String, Box<serde_json::value::RawValue>>;
    let invalid = |e: serde_json::Error| ErrorData::invalid_params(e.to_string(), None);
    let fields: RawMap = serde_json::from_str(raw).map_err(invalid)?;
    let params: RawMap = fields
        .get("params")
        .map(|p| serde_json::from_str(p.get()))
        .transpose()
        .map_err(invalid)?
        .unwrap_or_default();
    let original_name: Option<String> = params
        .get("name")
        .map(|n| serde_json::from_str(n.get()))
        .transpose()
        .map_err(invalid)?;
    if original_name.as_deref() != Some(name) {
        return Err(ErrorData::internal_error(
            "original request/tool identity mismatch",
            None,
        ));
    }
    validation::Arguments::raw(
        name,
        params.get("arguments").map(|a| a.get()).unwrap_or("{}"),
    )
}
fn original_prompt_arguments(
    raw: &str,
    name: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, ErrorData> {
    type RawMap = BTreeMap<String, Box<serde_json::value::RawValue>>;
    let invalid =
        || ErrorData::invalid_params("Invalid request parameters", Some(serde_json::json!("")));
    let fields: RawMap = serde_json::from_str(raw).map_err(|_| invalid())?;
    let params: RawMap = fields
        .get("params")
        .map(|p| serde_json::from_str(p.get()))
        .transpose()
        .map_err(|_| invalid())?
        .unwrap_or_default();
    let original_name = params
        .get("name")
        .and_then(|n| serde_json::from_str::<String>(n.get()).ok())
        .ok_or_else(invalid)?;
    if original_name != name {
        return Err(ErrorData::internal_error(
            "original request/prompt identity mismatch",
            None,
        ));
    }
    let Some(arguments) = params.get("arguments").filter(|p| p.get() != "null") else {
        return Ok(Default::default());
    };
    let arguments: RawMap = serde_json::from_str(arguments.get()).map_err(|_| invalid())?;
    arguments
        .into_iter()
        .map(|(key, raw)| {
            serde_json::from_str::<String>(raw.get())
                .map(|s| (key, serde_json::Value::String(s)))
                .map_err(|_| invalid())
        })
        .collect()
}
fn parsed<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> Result<T, ErrorData> {
    serde_json::from_value(v).map_err(|e| ErrorData::internal_error(e.to_string(), None))
}
impl ServerHandler for SearchSurface {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        parsed(serde_json::json!({"tools":self.runtime_descriptors()["tools"]}))
    }
    async fn list_resources(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        parsed(serde_json::json!({"resources":Self::descriptors()["resources"]}))
    }
    async fn list_resource_templates(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        parsed(serde_json::json!({"resourceTemplates":Self::descriptors()["resourceTemplates"]}))
    }
    async fn list_prompts(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        parsed(serde_json::json!({"prompts":Self::descriptors()["prompts"]}))
    }
    async fn call_tool(
        &self,
        r: CallToolRequestParams,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        // Completion is published only after this inner future has returned:
        // cancellation first signals and awaits every owned search operation.
        let completion = self.wire.clone().map(|wire| (wire, ctx.id.clone()));
        let response = async {
        let parts = ctx.extensions.get::<axum::http::request::Parts>();
        let tenant = parts
            .and_then(|parts| parts.extensions.get::<crate::oauth::AuthenticatedClient>())
            .map(|c| c.0.clone())
            .or_else(|| self.connection_tenant.clone())
            .unwrap_or_else(|| TenantId::new("default").expect("default tenant"));
        let raw = parts
            .and_then(|parts| parts.extensions.get::<crate::opaque::OriginalRequest>())
            .map(|r| r.json().to_owned())
            .or_else(|| {
                self.wire.as_ref().and_then(|wire| {
                    wire.lock()
                        .unwrap()
                        .original_request(&ctx.id)
                        .ok()
                        .flatten()
                })
            });
        let arguments = if let Some(raw) = raw {
            original_arguments(&raw, &r.name)
        } else {
            ssx_json::to_value(r.arguments.unwrap_or_default())
                .map_err(|e| ErrorData::invalid_params(e.to_string(), None))
                .and_then(|v| {
                    validation::Arguments::new(&r.name, v.as_object().cloned().unwrap_or_default())
                })
        };
        let cancel = SearchCancellation::default();
        let task = self.execute_arguments(&r.name, arguments, tenant, cancel.clone());
        tokio::pin!(task);
        let v = tokio::select! {value=&mut task=>match value {Ok(v)=>v,Err(e)=>return parsed::<CallToolResult>(serde_json::json!({"content":[{"type":"text","text":e.message}],"isError":true})).map(Into::into)},_=ctx.ct.cancelled()=>{cancel.cancel();let _=task.await;return Err(ErrorData::internal_error("cancelled",None));}};
        let text =
            ssx_json::to_string(&v).map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        parsed::<CallToolResult>(serde_json::json!({"content":[{"type":"text","text":text}],"structuredContent":{},"isError":false,"_meta":{"ssx/native-structured-text":true}})).map(Into::into)
        }.await;
        if let Some((wire, id)) = completion {
            wire.lock()
                .unwrap()
                .native_handler_complete(&id)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        }
        response
    }

    async fn read_resource(
        &self,
        r: ReadResourceRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        let text = resources::read(&r.uri, self.state.discovery.as_ref())?;
        parsed::<ReadResourceResult>(
            serde_json::json!({"contents":[{"uri":r.uri,"mimeType":"text/plain","text":text}]}),
        )
        .map(Into::into)
    }
    async fn get_prompt(
        &self,
        r: GetPromptRequestParams,
        ctx: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, ErrorData> {
        let completion = self.wire.clone().map(|wire| (wire, ctx.id.clone()));
        let response = (|| {
            let raw = ctx
                .extensions
                .get::<axum::http::request::Parts>()
                .and_then(|parts| parts.extensions.get::<crate::opaque::OriginalRequest>())
                .map(|r| r.json().to_owned())
                .or_else(|| {
                    self.wire.as_ref().and_then(|wire| {
                        wire.lock()
                            .unwrap()
                            .original_request(&ctx.id)
                            .ok()
                            .flatten()
                    })
                });
            let arguments = if let Some(raw) = raw {
                original_prompt_arguments(&raw, &r.name)?
            } else {
                r.arguments.unwrap_or_default()
            };
            resources::prompt(&r.name, arguments).map(Into::into)
        })();
        // Rendering owns no asynchronous background work. Publish completion
        // after all raw validation/rendering returns, including error paths.
        if let Some((wire, id)) = completion {
            wire.lock()
                .unwrap()
                .native_handler_complete(&id)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        }
        response
    }
}

#[cfg(test)]
mod warning_tests {
    use super::*;

    #[test]
    fn specialist_previews_preserve_scope_warning_order_and_multiplicity() {
        for surface in [
            SearchSurfaceKind::Jobs,
            SearchSurfaceKind::Security,
            SearchSurfaceKind::Science,
        ] {
            let mut proposal = ScopeProposal::generic(SearchRequest::default());
            proposal.surface = surface;
            let mut preview = ScopePreview {
                scope: Default::default(),
                enforcement: Default::default(),
            };
            // Repetition is source behavior, not a set to be deduplicated.
            preview.scope.warnings = vec!["same".into(), "same".into(), "last".into()];
            let caller = caller_warnings(&proposal, &preview);
            let mut expected = caller.clone();
            expected.extend(preview.scope.warnings.clone());
            assert_eq!(warnings(&proposal, &preview), expected);
            assert!(!caller.is_empty());
            assert!(!caller.iter().any(|warning| warning == "same"));
        }
    }
}
