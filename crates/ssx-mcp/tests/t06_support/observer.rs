//! Qualification-only dispatch observations, published after the native call drains.
use super::fixture;
use rmcp::{ErrorData, RoleServer, ServerHandler, model::*, service::RequestContext};
use ssx_infra::valkey::Valkey;
use ssx_mcp::search_surface::SearchSurface;
use std::{
    collections::BTreeMap,
    num::NonZeroU64,
    sync::{Arc, atomic::Ordering},
};

#[derive(Clone)]
pub struct ObservedSurface {
    pub inner: SearchSurface,
    pub receipts: Option<DispatchReceipts>,
}
#[derive(Clone)]
pub struct DispatchReceipts {
    engines: BTreeMap<String, fixture::FakeEngine>,
    store: Arc<Valkey>,
    published: Arc<tokio::sync::Mutex<BTreeMap<String, usize>>>,
}
impl DispatchReceipts {
    pub fn new(engines: BTreeMap<String, fixture::FakeEngine>, store: Arc<Valkey>) -> Self {
        Self {
            engines,
            store,
            published: Default::default(),
        }
    }
    pub async fn publish(&self) -> Result<(), ErrorData> {
        // Three fixed fixture engines; no response/input-derived keys or tasks.
        // Serialization prevents concurrent post-call observations from regressing.
        let mut published = self.published.lock().await;
        for (name, engine) in &self.engines {
            let count = engine.calls.load(Ordering::SeqCst);
            for dispatch in published.get(name).copied().unwrap_or(0) + 1..=count {
                self.store
                    .put(
                        &format!("t06:qualification:dispatch:{name}:{dispatch}"),
                        b"1",
                        NonZeroU64::new(3600).unwrap(),
                    )
                    .await
                    .map_err(|e| {
                        ErrorData::internal_error(
                            format!("private dispatch observation: {e:?}"),
                            None,
                        )
                    })?;
            }
            published.insert(name.clone(), count);
        }
        Ok(())
    }
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
        let response = self.inner.call_tool(r, c).await;
        if let Some(receipts) = &self.receipts {
            receipts.publish().await?;
        }
        response
    }
}
