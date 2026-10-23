//! Native search surfaces and shared MCP transports. T01 Registry remains a qualification fixture.
pub mod gateway;
pub mod gateway_client;
pub mod harness;
pub mod http_client;
pub mod legacy;
pub mod oauth;
pub mod opaque;
pub mod output_limits;
pub mod projection;
pub mod search_surface;
use rmcp::{ErrorData, RoleServer, ServerHandler, model::*, service::RequestContext};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;
/// Injectable registry, shared by stdio, streamable HTTP, legacy SSE and proxy.
#[derive(Clone)]
pub struct Registry {
    pub descriptors: Arc<Value>,
    pub calls: Arc<AtomicUsize>,
    pub in_flight: Arc<AtomicUsize>,
    pub discovery: Discovery,
    pub initialize_delay: std::time::Duration,
    pub response_delay: std::time::Duration,
    pub opaque: Arc<opaque::Catalogs>,
}
#[derive(Clone, Copy, Default)]
pub enum Discovery {
    #[default]
    Normal,
    RepeatedCursor,
    Duplicate,
    TooManyPages,
    TooManyItems,
    Delayed,
}
struct InFlight(Arc<AtomicUsize>);
impl Drop for InFlight {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
impl Registry {
    pub fn synthetic() -> Self {
        let captured: Value = serde_json::from_str(include_str!("../fixtures/python-sse.json"))
            .expect("captured fixture");
        Self {
            descriptors: Arc::new(
                json!({"tools":captured["observations"]["tools"],"resources":captured["observations"]["resources"],"prompts":captured["observations"]["prompts"]}),
            ),
            calls: Arc::new(AtomicUsize::new(0)),
            in_flight: Default::default(),
            discovery: Discovery::Normal,
            initialize_delay: std::time::Duration::ZERO,
            response_delay: std::time::Duration::ZERO,
            opaque: Default::default(),
        }
    }
    pub fn large_catalog() -> Self {
        let root: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
            serde_json::from_str(include_str!("../fixtures/python-sse.json"))
                .expect("source fixture");
        let observations: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
            serde_json::from_str(root["observations"].get()).expect("observations");
        let raw = format!("{{\"tools\":{}}}", observations["large_tools"].get());
        let mut registry = Self::synthetic();
        let mut descriptors = registry.descriptors.as_ref().clone();
        descriptors["tools"] =
            serde_json::from_str(observations["large_tools"].get()).expect("tool descriptors");
        registry.descriptors = Arc::new(descriptors);
        registry.opaque = Arc::new(
            [(
                String::from("tools/list"),
                serde_json::value::RawValue::from_string(raw).expect("raw catalog"),
            )]
            .into_iter()
            .collect(),
        );
        registry
    }
    async fn wait_response(
        &self,
        context: &RequestContext<RoleServer>,
    ) -> std::result::Result<(), ErrorData> {
        if !self.response_delay.is_zero() {
            self.in_flight.fetch_add(1, Ordering::SeqCst);
            let _flight = InFlight(self.in_flight.clone());
            tokio::select! { _=tokio::time::sleep(self.response_delay)=>{}, _=context.ct.cancelled()=>return Err(ErrorData::internal_error("cancelled",None)) }
        }
        Ok(())
    }
    fn parse<T: serde::de::DeserializeOwned>(v: Value) -> std::result::Result<T, ErrorData> {
        serde_json::from_value(v).map_err(|e| ErrorData::internal_error(e.to_string(), None))
    }
}
impl ServerHandler for Registry {
    async fn initialize(
        &self,
        r: InitializeRequestParams,
        c: RequestContext<RoleServer>,
    ) -> std::result::Result<InitializeResult, ErrorData> {
        tokio::time::sleep(self.initialize_delay).await;
        c.peer.set_peer_info(r.clone());
        self.negotiate_initialize(&r)
    }
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
        r: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, ErrorData> {
        let cursor = r.and_then(|p| p.cursor);
        let value = match self.discovery {
            Discovery::Normal => json!({"tools":self.descriptors["tools"]}),
            Discovery::RepeatedCursor => json!({"tools":[],"nextCursor":"same"}),
            Discovery::Duplicate => {
                json!({"tools":self.descriptors["tools"],"nextCursor":if cursor.is_none(){"first"}else{"second"}})
            }
            Discovery::TooManyPages => {
                json!({"tools":[],"nextCursor":cursor.and_then(|s|s.parse::<u32>().ok()).unwrap_or(0).saturating_add(1).to_string()})
            }
            Discovery::TooManyItems => {
                let template = &self.descriptors["tools"][0];
                let tools: Vec<_> = (0..4097)
                    .map(|i| {
                        let mut t = template.clone();
                        t["name"] = json!(format!("tool{i}"));
                        t
                    })
                    .collect();
                json!({"tools":tools})
            }
            Discovery::Delayed => {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                json!({"tools":[]})
            }
        };
        Self::parse(value)
    }
    async fn list_resources(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> std::result::Result<ListResourcesResult, ErrorData> {
        Self::parse(json!({"resources":self.descriptors["resources"]}))
    }
    async fn list_prompts(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> std::result::Result<ListPromptsResult, ErrorData> {
        Self::parse(json!({"prompts":self.descriptors["prompts"]}))
    }
    async fn list_resource_templates(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> std::result::Result<ListResourceTemplatesResult, ErrorData> {
        Self::parse(
            json!({"resourceTemplates":self.descriptors.get("resourceTemplates").cloned().unwrap_or_else(||json!([]))}),
        )
    }
    async fn read_resource(
        &self,
        r: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<ReadResourceResponse, ErrorData> {
        self.wait_response(&context).await?;
        if r.uri != "t01://text" {
            return Err(ErrorData::invalid_params("unknown resource", None));
        }
        Self::parse::<ReadResourceResult>(
            json!({"contents":[{"uri":r.uri,"mimeType":"text/plain","text":"fixture text"}]}),
        )
        .map(Into::into)
    }
    async fn get_prompt(
        &self,
        r: GetPromptRequestParams,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<GetPromptResponse, ErrorData> {
        self.wait_response(&context).await?;
        if r.name != "t01_prompt" {
            return Err(ErrorData::invalid_params("unknown prompt", None));
        }
        Self::parse::<GetPromptResult>(json!({"messages":[{"role":"user","content":{"type":"text","text":format!("topic: {}",r.arguments.and_then(|a|a.get("topic").and_then(Value::as_str).map(str::to_owned)).unwrap_or_default())}}]})).map(Into::into)
    }
    async fn call_tool(
        &self,
        r: CallToolRequestParams,
        ctx: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResponse, ErrorData> {
        if r.name != "t01_echo" && r.name != "t01_large" {
            return Err(ErrorData::invalid_params("unknown tool", None));
        }
        let args = r.arguments.unwrap_or_default();
        let payload = args
            .get("payload")
            .ok_or_else(|| ErrorData::invalid_params("payload required", None))?;
        if payload
            .get("items")
            .and_then(Value::as_array)
            .is_none_or(|a| a.iter().any(|v| !(v.is_i64() || v.is_u64())))
        {
            return Err(ErrorData::invalid_params(
                "payload.items must be integers",
                None,
            ));
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        let _flight = InFlight(self.in_flight.clone());
        if args.get("note") == Some(&json!("wait")) {
            ctx.ct.cancelled().await;
            return Err(ErrorData::internal_error("cancelled", None));
        }
        if args.get("note") == Some(&json!("error")) {
            return Self::parse::<CallToolResult>(
                json!({"content":[{"type":"text","text":"synthetic failure"}],"isError":true}),
            )
            .map(Into::into);
        }
        let note = args.get("note").unwrap_or(&Value::Null);
        let text = format!("{{\"payload\":{payload},\"note\":{note}}}");
        let value = json!({"payload":payload,"note":note});
        Self::parse::<CallToolResult>(json!({"content":[{"type":"text","text":text}],"structuredContent":value,"isError":false})).map(Into::into)
    }
}
/// Complete structural equality; absence/null, order, nested schemas and metadata stay exact.
pub fn compare_descriptor(expected: &Value, actual: &Value) -> Result<()> {
    if expected == actual {
        Ok(())
    } else {
        Err("complete descriptor mismatch".into())
    }
}

pub mod saved_events;

pub mod research_planning;
