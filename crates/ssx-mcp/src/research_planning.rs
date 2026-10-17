//! W03 real planning/admission bindings, composed with the accepted T06 surface.
use crate::search_surface::SearchSurface;
use rmcp::{ErrorData, RoleServer, ServerHandler, model::*, service::RequestContext};
use ssx_domain::TenantId;
use ssx_infra::research_planning::{AdaptiveResearch, job_summary};
use ssx_json::Value;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, watch};

pub const TOOL_NAMES: [&str; 9] = [
    "slopsearx_plan_research",
    "slopsearx_plan_query_variants",
    "slopsearx_plan_research_followup",
    "slopsearx_start_research",
    "slopsearx_get_job",
    "slopsearx_extend_research",
    "slopsearx_update_research",
    "slopsearx_retry_research",
    "slopsearx_cancel_job",
];
#[derive(Clone)]
pub struct PlanningRegistry {
    planning: AdaptiveResearch,
    search: SearchSurface,
    stop: watch::Sender<bool>,
    tasks: Arc<Mutex<tokio::task::JoinSet<()>>>,
    closed: Arc<RwLock<bool>>,
    connection_tenant: Option<TenantId>,
    wire: Option<Arc<std::sync::Mutex<crate::opaque::WireState>>>,
}
impl PlanningRegistry {
    pub fn new(planning: AdaptiveResearch, search: SearchSurface) -> Self {
        let (stop, _) = watch::channel(false);
        Self {
            planning,
            search,
            stop,
            tasks: Default::default(),
            closed: Default::default(),
            connection_tenant: None,
            wire: None,
        }
    }
    pub fn bind_connection(
        mut self,
        wire: Arc<std::sync::Mutex<crate::opaque::WireState>>,
        client: Option<crate::oauth::AuthenticatedClient>,
    ) -> Self {
        self.connection_tenant = client.as_ref().map(|c| c.0.clone());
        self.search = self.search.bind_connection(wire.clone(), client);
        self.wire = Some(wire);
        self
    }
    pub async fn close(&self) {
        self.stop.send_replace(true);
        let mut closed = self.closed.write().await;
        *closed = true;
        drop(closed);
        let mut tasks = self.tasks.lock().await;
        while tasks.join_next().await.is_some() {}
    }
    pub fn descriptors() -> serde_json::Value {
        let mut catalog = SearchSurface::descriptors();
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/rust-rewrite/w03/source-observation-001.json"
        ))
        .expect("source-bound W03 descriptors");
        let tools = catalog["tools"].as_array_mut().expect("T06 tools");
        tools.extend(
            source["tools"]
                .as_array()
                .expect("W03 tools")
                .iter()
                .cloned(),
        );
        catalog
    }
    pub fn catalogs() -> Arc<crate::opaque::Catalogs> {
        let mut catalog = SearchSurface::catalogs().as_ref().clone();
        let tools = Self::descriptors()["tools"].clone();
        let names: Vec<_> = crate::search_surface::TOOL_NAMES
            .into_iter()
            .chain(TOOL_NAMES)
            .collect();
        for (key, value) in [
            ("tools/list", serde_json::json!({"tools":tools})),
            ("ssx/native-tools", serde_json::json!(names)),
            ("ssx/native-raw-arguments", serde_json::json!(names)),
        ] {
            catalog.insert(
                key.into(),
                serde_json::value::RawValue::from_string(
                    serde_json::to_string(&value).expect("catalog serialization"),
                )
                .expect("raw catalog"),
            );
        }
        Arc::new(catalog)
    }
    /// Trusted composition supplies tenant; no wire argument selects authority.
    pub async fn execute(
        &self,
        name: &str,
        args: Value,
        tenant: TenantId,
    ) -> Result<Value, ErrorData> {
        self.execute_with_stop(name, args, tenant, self.stop.subscribe())
            .await
    }
    async fn execute_with_stop(
        &self,
        name: &str,
        args: Value,
        tenant: TenantId,
        stop: watch::Receiver<bool>,
    ) -> Result<Value, ErrorData> {
        let admission = self.closed.read().await;
        if *admission || *self.stop.borrow() {
            return Err(ErrorData::internal_error("planning registry closed", None));
        }
        validate(name, &args)?;
        let result = match name {
            "slopsearx_plan_research" => self.planning.preview_initial(&args, false).await,
            "slopsearx_plan_query_variants" => self.planning.preview_initial(&args, true).await,
            "slopsearx_plan_research_followup" => {
                self.planning.preview_followup(tenant, &args).await
            }
            "slopsearx_start_research" => match self.planning.admit(tenant.clone(), &args).await {
                Ok((job,created)) => {
                    let mut output = job_summary(&job);
                    output["note"]=ssx_json::json!(if created{"job queued; cancellation stops owned work and preserves completed evidence"}else{"returned existing job for idempotency_key"});
                    if !created{return Ok(output);}
                    let service = self.planning.clone();
                    let stop = self.stop.subscribe();
                    let mut tasks = self.tasks.lock().await;
                    while tasks.try_join_next().is_some() {}
                    tasks.spawn(async move {
                        let _ = service.run(tenant, &job.job_id, stop).await;
                    });
                    Ok(output)
                }
                Err(e) => Err(e),
            },
            "slopsearx_get_job" => self.planning.get_job(tenant,args["job_id"].as_str().unwrap()).await.map(|j|{let mut value=job_summary(&j);value["created_at"]=ssx_json::json!(ssx_infra::research_planning::iso_timestamp(j.created_at.get()));value["note"]=ssx_json::json!("completed queries are immutable; their cursors remain readable");value}),
            "slopsearx_extend_research" => {
                let before=self.planning.load(tenant.clone(),args["job_id"].as_str().unwrap()).await;
                let replay=before.as_ref().is_ok_and(|j|args["continuation_key"].as_str().is_some_and(|key|j.queries.iter().any(|q|q.continuation_key.as_deref()==Some(key))));
                match self.planning.extend(tenant,&args,stop.clone()).await {
                    Ok(job) if !replay && job.state=="cancelled"=>Ok(ssx_json::json!({"job_id":job.job_id,"state":"cancelled","note":"the job had a pending cancellation request; the follow-up query was cancelled, not executed"})),
                    Ok(job) if !replay && job.state=="expired"=>Ok(ssx_json::json!({"job_id":job.job_id,"state":"expired","note":"job deadline had already passed; the follow-up query was not appended or executed"})),
                    Ok(job)=>{let mut value=job_summary(&job);value["note"]=ssx_json::json!(if replay{"returned the current status of the previously accepted continuation"}else{"follow-up query appended and executed; prior completed evidence was preserved"});Ok(value)},
                    Err(e) if e.code=="job_busy"=>Ok(ssx_json::json!({"job_id":args["job_id"],"state":"running","note":"job is still running under a live worker; follow-up query was not appended or executed"})),
                    Err(e)=>Err(e),
                }
            },
            "slopsearx_update_research" => self
                .planning
                .update(tenant, &args)
                .await
                .map(|j| job_summary(&j)),
            "slopsearx_retry_research"=>self.planning.retry_public(tenant,args["job_id"].as_str().unwrap(),stop).await,
            "slopsearx_cancel_job"=>self.planning.cancel(tenant,args["job_id"].as_str().unwrap()).await.map(|j|ssx_json::json!({"job_id":j.job_id,"state":j.state,"note":if ["succeeded","partial","failed","expired"].contains(&j.state.as_str()){"job already finished; completed evidence remains readable"}else{"cancellation requested; completed evidence is preserved and owned work is stopped"}})),
            _ => return Err(ErrorData::invalid_params("unknown planning tool", None)),
        };
        Ok(result.unwrap_or_else(|e| e.value()))
    }
}
fn validate(name: &str, args: &Value) -> Result<(), ErrorData> {
    let tools = PlanningRegistry::descriptors();
    let descriptor = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"].as_str() == Some(name))
        .ok_or_else(|| ErrorData::invalid_params("unknown planning tool", None))?;
    let object = args
        .as_object()
        .ok_or_else(|| ErrorData::invalid_params("arguments must be an object", None))?;
    let properties = descriptor["inputSchema"]["properties"].as_object().unwrap();
    if let Some(required) = descriptor["inputSchema"]["required"].as_array() {
        for key in required {
            if !object.contains_key(key.as_str().unwrap()) {
                return Err(ErrorData::invalid_params(
                    "missing required tool argument",
                    None,
                ));
            }
        }
    }
    for (key, v) in object {
        let Some(schema) = properties.get(key.as_str()) else {
            if descriptor["inputSchema"]["additionalProperties"].as_bool() == Some(false) {
                return Err(ErrorData::invalid_params(
                    format!("undeclared tool argument: {key}"),
                    None,
                ));
            }
            continue;
        };
        if !schema_accepts(schema, v) {
            return Err(ErrorData::invalid_params(
                format!("invalid argument type: {key}"),
                None,
            ));
        }
    }
    Ok(())
}
fn schema_accepts(schema: &serde_json::Value, value: &Value) -> bool {
    if let Some(variants) = schema["anyOf"].as_array() {
        return variants.iter().any(|s| schema_accepts(s, value));
    }
    let kind = match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
        Value::Number(n) => {
            if n.is_f64() {
                "number"
            } else {
                "integer"
            }
        }
    };
    if let Some(t) = schema["type"].as_str()
        && t != kind
        && !(t == "number" && kind == "integer")
    {
        return false;
    }
    if let Some(items) = schema.get("items")
        && let Some(array) = value.as_array()
        && array.iter().any(|v| !schema_accepts(items, v))
    {
        return false;
    }
    if let Some(additional) = schema.get("additionalProperties").filter(|s| s.is_object())
        && let Some(object) = value.as_object()
        && object.values().any(|v| !schema_accepts(additional, v))
    {
        return false;
    }
    true
}
fn parsed<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> Result<T, ErrorData> {
    serde_json::from_value(value).map_err(|e| ErrorData::internal_error(e.to_string(), None))
}
impl ServerHandler for PlanningRegistry {
    fn get_info(&self) -> ServerConfig {
        self.search.get_info()
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        parsed(serde_json::json!({"tools":Self::descriptors()["tools"]}))
    }
    async fn list_resources(
        &self,
        r: Option<PaginatedRequestParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        self.search.list_resources(r, ctx).await
    }
    async fn list_resource_templates(
        &self,
        r: Option<PaginatedRequestParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        self.search.list_resource_templates(r, ctx).await
    }
    async fn list_prompts(
        &self,
        r: Option<PaginatedRequestParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        self.search.list_prompts(r, ctx).await
    }
    async fn read_resource(
        &self,
        r: ReadResourceRequestParams,
        ctx: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        self.search.read_resource(r, ctx).await
    }
    async fn get_prompt(
        &self,
        r: GetPromptRequestParams,
        ctx: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, ErrorData> {
        self.search.get_prompt(r, ctx).await
    }
    async fn call_tool(
        &self,
        r: CallToolRequestParams,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        if !TOOL_NAMES.contains(&r.name.as_ref()) {
            return self.search.call_tool(r, ctx).await;
        }
        let parts = ctx.extensions.get::<axum::http::request::Parts>();
        let tenant = parts
            .and_then(|p| p.extensions.get::<crate::oauth::AuthenticatedClient>())
            .map(|c| c.0.clone())
            .or_else(|| self.connection_tenant.clone())
            .unwrap_or_else(|| TenantId::new("default").unwrap());
        let raw = parts
            .and_then(|p| p.extensions.get::<crate::opaque::OriginalRequest>())
            .map(|r| r.json().to_owned())
            .or_else(|| {
                self.wire
                    .as_ref()
                    .and_then(|w| w.lock().unwrap().original_request(&ctx.id).ok().flatten())
            });
        let args: Value = if let Some(raw) = raw {
            let request: Value = ssx_json::from_str(&raw)
                .map_err(|e| ErrorData::invalid_params(e.to_string(), None))?;
            request["params"]["arguments"].clone()
        } else {
            ssx_json::to_value(r.arguments.unwrap_or_default())
                .map_err(|e| ErrorData::invalid_params(e.to_string(), None))?
        };
        let (cancel, stop) = watch::channel(false);
        let mut global = self.stop.subscribe();
        let task = self.execute_with_stop(&r.name, args, tenant, stop);
        tokio::pin!(task);
        let result = tokio::select! {result=&mut task=>result,_=ctx.ct.cancelled()=>{cancel.send_replace(true);let _=task.await;Err(ErrorData::internal_error("cancelled",None))},_=global.changed()=>{cancel.send_replace(true);let _=task.await;Err(ErrorData::internal_error("registry closing",None))}};
        if let Some(w) = &self.wire {
            w.lock()
                .unwrap()
                .native_handler_complete(&ctx.id)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        }
        let result = match result {
            Ok(value) => value,
            Err(e) => return parsed::<CallToolResult>(
                serde_json::json!({"content":[{"type":"text","text":e.message}],"isError":true}),
            )
            .map(Into::into),
        };
        let text = ssx_json::to_string(&result)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        parsed::<CallToolResult>(serde_json::json!({"content":[{"type":"text","text":text}],"structuredContent":{},"isError":false,"_meta":{"ssx/native-structured-text":true}})).map(Into::into)
    }
}
