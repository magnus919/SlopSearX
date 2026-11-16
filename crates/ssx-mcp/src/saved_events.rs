//! W05 real native event bindings. Authenticated composition supplies the typed
//! principal in EventService; this module never derives authority from arguments.
//! Complete production workflow/auth composition remains W09.
use rmcp::{ErrorData, ServerHandler, model::*};
use ssx_application::saved::{SavedPolicy, prepare_create};
use ssx_infra::{
    saved_events::EventService,
    saved_runner::{SavedRunner, StaticSavedAuthority},
    search::SearchService,
    valkey::saved::SavedStore,
};
use ssx_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};
use tokio::sync::{Mutex, RwLock, watch};
const TOOL_NAMES: [&str; 3] = [
    "slopsearx_create_saved_search",
    "slopsearx_read_saved_search_events",
    "slopsearx_ack_saved_search_events",
];
#[derive(Clone)]
pub struct SavedEventRegistry {
    events: EventService,
    runner: SavedRunner,
    authority: Arc<StaticSavedAuthority>,
    stop: watch::Sender<bool>,
    admission: Arc<RwLock<bool>>,
    tasks: Arc<Mutex<tokio::task::JoinSet<()>>>,
}
impl SavedEventRegistry {
    pub fn new(
        events: EventService,
        search: SearchService,
        authority: Arc<StaticSavedAuthority>,
        timeout: Duration,
    ) -> Result<Self, ssx_infra::saved_runner::RunnerError> {
        let runner = SavedRunner::new(events.store().clone(), search, authority.clone(), timeout)?
            .with_events(events.publisher().clone());
        let (stop, _) = watch::channel(false);
        Ok(Self {
            events,
            runner,
            authority,
            stop,
            admission: Default::default(),
            tasks: Default::default(),
        })
    }
    pub async fn close(&self) {
        let mut admission = self.admission.write().await;
        *admission = true;
        self.stop.send_replace(true);
        drop(admission);
        let mut tasks = self.tasks.lock().await;
        while tasks.join_next().await.is_some() {}
    }
    fn descriptors() -> serde_json::Value {
        let source: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/python-sse.json"))
                .expect("captured descriptor fixture");
        let names = TOOL_NAMES;
        serde_json::Value::Array(
            source["observations"]["public_tools"]
                .as_array()
                .expect("captured tools")
                .iter()
                .filter(|v| v["name"].as_str().is_some_and(|n| names.contains(&n)))
                .cloned()
                .collect(),
        )
    }
    fn catalogs() -> crate::Result<Arc<crate::opaque::Catalogs>> {
        use serde_json::value::RawValue;
        let source: BTreeMap<String, Box<RawValue>> =
            serde_json::from_str(include_str!("../fixtures/python-sse.json"))?;
        let observations: BTreeMap<String, Box<RawValue>> =
            serde_json::from_str(source["observations"].get())?;
        let tools: Vec<Box<RawValue>> = serde_json::from_str(observations["public_tools"].get())?;
        let mut selected = Vec::new();
        let mut names = BTreeSet::new();
        for tool in tools {
            let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(tool.get())?;
            let name: String = serde_json::from_str(fields["name"].get())?;
            if TOOL_NAMES.contains(&name.as_str()) {
                if !names.insert(name) {
                    return Err("duplicate saved-event descriptor".into());
                }
                selected.push(tool);
            }
        }
        if names != TOOL_NAMES.into_iter().map(str::to_owned).collect() {
            return Err("missing saved-event descriptor".into());
        }
        let tools = serde_json::to_string(&selected)?;
        Ok(Arc::new(
            [
                (
                    "tools/list".into(),
                    RawValue::from_string(format!("{{\"tools\":{tools}}}"))?,
                ),
                (
                    "ssx/native-tools".into(),
                    RawValue::from_string(serde_json::to_string(&TOOL_NAMES)?)?,
                ),
            ]
            .into_iter()
            .collect(),
        ))
    }
    async fn invoke(&self, name: &str, args: &Value) -> Result<Value, ErrorData> {
        // Retain admission through all awaited store work and task registration.
        // close obtains exclusive admission before signalling and draining.
        let admission = self.admission.read().await;
        if *admission {
            return Err(ErrorData::internal_error(
                "saved event registry is closed",
                None,
            ));
        }
        let invalid = || ErrorData::invalid_params("invalid saved-event tool arguments", None);
        let object = args.as_object().ok_or_else(invalid)?;
        let fields: &[&str] = match name {
            "slopsearx_read_saved_search_events" => &["consumer_id", "cursor", "limit"],
            "slopsearx_ack_saved_search_events" => &["consumer_id", "cursor"],
            "slopsearx_create_saved_search" => &[
                "query",
                "engines",
                "interval_seconds",
                "retention_seconds",
                "expires_in_seconds",
                "max_results",
                "max_reports",
                "start_immediately",
            ],
            _ => return Err(ErrorData::invalid_params("unknown tool", None)),
        };
        if object.keys().any(|k| !fields.contains(&k.as_str())) {
            return Err(invalid());
        }
        if name == "slopsearx_read_saved_search_events" {
            let consumer = args["consumer_id"].as_str().ok_or_else(invalid)?;
            let cursor = match object.get("cursor") {
                Some(Value::Null) | None => None,
                Some(v) => Some(v.as_str().ok_or_else(invalid)?),
            };
            let limit = match object.get("limit") {
                None => 50,
                Some(v) => v
                    .as_u64()
                    .and_then(|v| usize::try_from(v).ok())
                    .ok_or_else(invalid)?,
            };
            return Ok(self.events.read(consumer, cursor, limit).await);
        }
        if name == "slopsearx_ack_saved_search_events" {
            return Ok(self
                .events
                .ack(
                    args["consumer_id"].as_str().ok_or_else(invalid)?,
                    args["cursor"].as_str().ok_or_else(invalid)?,
                )
                .await);
        }
        if !self.authority.saved_granted {
            return Ok(ssx_infra::saved_events::error(
                "tool_disabled",
                "saved searches require the saved-search grant",
            ));
        }
        let policy = SavedPolicy {
            search: &self.authority.search,
            saved_granted: self.authority.saved_granted,
            auth_configured: &self.authority.auth_configured,
            limits: &self.authority.limits,
        };
        let store = self.events.store();
        let now = match store.now().await {
            Ok(v) => v,
            Err(_) => {
                return Ok(ssx_infra::saved_events::error(
                    "store_unavailable",
                    "saved searches require connected Valkey",
                ));
            }
        };
        let id = SavedStore::generate_id()
            .map_err(|_| ErrorData::internal_error("saved identity unavailable", None))?;
        let prepared =
            match prepare_create(&policy, store.tenant().as_str(), id.as_str(), args, now) {
                Ok(v) => v,
                Err(e) => return Ok(ssx_infra::saved_events::error("invalid_input", e.0)),
            };
        if store.create(&prepared).await.is_err() {
            return Ok(ssx_infra::saved_events::error(
                "store_unavailable",
                "saved search could not be created",
            ));
        }
        let d = prepared.definition();
        let output = json!({"search_id":d.search_id,"artifact":{"contract":"slopsearx.artifact_ref","version":1,"kind":"saved_search","id":d.search_id},"revision":d.revision,"query":d.query,"engines":d.engines,"interval_seconds":d.interval_seconds,"retention_seconds":d.retention_seconds,"max_results":d.max_results,"max_reports":d.max_reports,"created_at":d.created_at.get(),"expires_at":d.expires_at.get(),"next_due":d.next_due.get(),"paused":d.paused,"latest_run_id":d.latest_run_id,"comparison":{"window":"fixed_first_page","identity_version":"url-source-window-v1","absence_event":"not_observed_in_latest_run","absence_is_deletion":false}});
        if args["start_immediately"] == Value::Bool(true) {
            let runner = self.runner.clone();
            let stop = self.stop.subscribe();
            let mut tasks = self.tasks.lock().await;
            while tasks.try_join_next().is_some() {}
            tasks.spawn(async move {
                let _ = runner.run_one(id, stop).await;
            });
        }
        Ok(output)
    }
}
impl ServerHandler for SavedEventRegistry {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        serde_json::from_value(serde_json::json!({"tools":Self::descriptors()}))
            .map_err(|_| ErrorData::internal_error("invalid saved event descriptors", None))
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let raw = serde_json::to_vec(&request.arguments.unwrap_or_default())
            .map_err(|_| ErrorData::invalid_params("invalid arguments", None))?;
        if raw.len() > 65536 {
            return Err(ErrorData::invalid_params(
                "saved event arguments exceed bound",
                None,
            ));
        }
        let args = ssx_json::from_slice(&raw)
            .map_err(|_| ErrorData::invalid_params("invalid arguments", None))?;
        let value = self.invoke(request.name.as_ref(), &args).await?;
        let text = ssx_json::to_string(&value).map_err(|_| {
            ErrorData::internal_error("saved event response serialization failed", None)
        })?;
        // Only the trusted transport restores this exact object text. The SDK
        // sees an empty placeholder so historic integers and large batches never
        // pass through its floating-point structured Value representation.
        let response: CallToolResult = serde_json::from_value(serde_json::json!({
            "content": [{"type":"text","text":text}],
            "structuredContent": {},
            "isError": false,
            "_meta": {"ssx/native-structured-text":true}
        }))
        .map_err(|_| ErrorData::internal_error("saved event response envelope failed", None))?;
        Ok(response.into())
    }
}
/// Focused HTTP mount with the caller's actual network authentication and
/// trusted typed principal. W09 owns full production identity composition.
pub fn http_router(
    registry: SavedEventRegistry,
    authentication: crate::oauth::NetworkAuth,
    cancel: tokio_util::sync::CancellationToken,
) -> crate::Result<crate::harness::ManagedRouter> {
    let catalogs = SavedEventRegistry::catalogs()?;
    let output = crate::output_limits::OutputLimits::new(32 * 1024 * 1024)?;
    Ok(
        crate::harness::http_router_for_with_limits(registry, catalogs, cancel, output)
            .map(|router| authentication.mount(router)),
    )
}
