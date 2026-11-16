//! Fault injection only: deliberately diverge WireState from the real pump's
//! Overlay to simulate a cancelled generation's already-buffered SDK result.
//! This is transport-order evidence, never SearchSurface ownership evidence.
use rmcp::{ErrorData, RoleServer, ServerHandler, model::*, service::RequestContext};
use std::sync::{Arc, Mutex};
#[derive(Clone)]
pub struct CancelWire {
    wire: Arc<Mutex<ssx_mcp::opaque::WireState>>,
    pub ready: Arc<tokio::sync::Semaphore>,
    release: Arc<tokio::sync::Notify>,
    returned: Arc<tokio::sync::Semaphore>,
}
impl CancelWire {
    pub fn new(
        wire: Arc<Mutex<ssx_mcp::opaque::WireState>>,
        ready: Arc<tokio::sync::Semaphore>,
    ) -> Self {
        Self {
            wire,
            ready,
            release: Default::default(),
            returned: Arc::new(tokio::sync::Semaphore::new(0)),
        }
    }
    pub fn catalogs() -> Arc<ssx_mcp::opaque::Catalogs> {
        Arc::new(
            ["ssx/native-tools", "ssx/native-raw-arguments"]
                .into_iter()
                .map(|k| {
                    (
                        k.into(),
                        serde_json::value::RawValue::from_string(
                            "[\"native_cancel_probe\"]".into(),
                        )
                        .unwrap(),
                    )
                })
                .collect(),
        )
    }
}
impl ServerHandler for CancelWire {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }
    async fn call_tool(
        &self,
        _: CallToolRequestParams,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let original = self
            .wire
            .lock()
            .unwrap()
            .original_request(&ctx.id)
            .unwrap()
            .unwrap();
        let original: serde_json::Value = serde_json::from_str(&original).unwrap();
        let scenario = original["params"]["arguments"]["scenario"]
            .as_str()
            .unwrap();
        let text = if scenario.starts_with("old-") {
            {
                let mut wire = self.wire.lock().unwrap();
                wire.request(&serde_json::json!({"method":"notifications/cancelled","params":{"requestId":original["id"]}}).to_string()).unwrap();
                wire.native_handler_complete(&ctx.id).unwrap();
                // Deliberately omit Overlay abandonment. The only safe path
                // for this buffered malformed SDK result is the pump's early
                // stale-generation filter, before restoration sees its marker.
                assert_eq!(wire.drain_cancelled_native().unwrap().len(), 1);
            }
            self.ready.add_permits(1);
            eprintln!("T06_STALE_GENERATION_READY");
            if scenario == "old-after" {
                self.release.notified().await;
                self.returned.add_permits(1);
            }
            "MALFORMED".to_owned()
        } else {
            if scenario == "fresh-after" {
                self.release.notify_one();
                self.returned.acquire().await.unwrap().forget();
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            self.wire
                .lock()
                .unwrap()
                .native_handler_complete(&ctx.id)
                .unwrap();
            serde_json::json!({"generation":scenario,"literal":"snap-business-text"}).to_string()
        };
        let result:CallToolResult=serde_json::from_value(serde_json::json!({"content":[{"type":"text","text":text}],"structuredContent":{},"isError":false,"_meta":{"ssx/native-structured-text":true}})).unwrap();
        Ok(result.into())
    }
}
