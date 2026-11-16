//! Qualification-only causal observations of the real owned handler future.
use rmcp::{ErrorData, RoleServer, ServerHandler, model::*, service::RequestContext};
use ssx_mcp::search_surface::SearchSurface;
use std::{
    future::{Future, poll_fn},
    task::Poll,
};
#[derive(Clone)]
pub struct ObservedSurface {
    pub inner: SearchSurface,
    pub wire: std::sync::Arc<std::sync::Mutex<ssx_mcp::opaque::WireState>>,
}
impl ServerHandler for ObservedSurface {
    fn get_info(&self) -> ServerConfig {
        self.inner.get_info()
    }
    async fn list_tools(
        &self,
        r: Option<PaginatedRequestParams>,
        c: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        self.inner.list_tools(r, c).await
    }
    async fn list_resources(
        &self,
        r: Option<PaginatedRequestParams>,
        c: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        self.inner.list_resources(r, c).await
    }
    async fn list_resource_templates(
        &self,
        r: Option<PaginatedRequestParams>,
        c: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        self.inner.list_resource_templates(r, c).await
    }
    async fn list_prompts(
        &self,
        r: Option<PaginatedRequestParams>,
        c: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        self.inner.list_prompts(r, c).await
    }
    async fn read_resource(
        &self,
        r: ReadResourceRequestParams,
        c: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        self.inner.read_resource(r, c).await
    }
    async fn get_prompt(
        &self,
        r: GetPromptRequestParams,
        c: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, ErrorData> {
        self.inner.get_prompt(r, c).await
    }
    async fn call_tool(
        &self,
        r: CallToolRequestParams,
        c: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let observed = r.name == "slopsearx_read_retrieval_receipts";
        // The qualified opaque owner aliases native SDK IDs. Correlate the
        // observation with its retained ORIGINAL request, not that alias.
        let id = self
            .wire
            .lock()
            .unwrap()
            .original_request(&c.id)
            .ok()
            .flatten()
            .and_then(|raw| ssx_json::from_str::<ssx_json::Value>(&raw).ok())
            .map(|request| request["id"].clone())
            .unwrap_or(ssx_json::Value::Null);
        let cancellation = c.ct.clone();
        let task = self.inner.call_tool(r, c);
        tokio::pin!(task);
        let mut announced = false;
        let result = poll_fn(|cx| {
            let polled = task.as_mut().poll(cx);
            if observed && !announced && matches!(polled, Poll::Pending) {
                // Emitted only AFTER the actual read handler yielded. With the
                // exclusive private Valkey paused, its read cannot have completed.
                eprintln!("{}", serde_json::json!({"w07_read_pending":true,"id":id}));
                announced = true;
            }
            polled
        })
        .await;
        if observed {
            eprintln!(
                "{}",
                serde_json::json!({"w07_read_drained":true,"id":id,"context_cancelled":cancellation.is_cancelled()})
            );
        }
        result
    }
}
