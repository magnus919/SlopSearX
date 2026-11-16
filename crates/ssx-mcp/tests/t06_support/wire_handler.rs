//! Qualification-only trusted handler for the native response envelope seam.
use rmcp::{ErrorData, RoleServer, ServerHandler, model::*, service::RequestContext};
#[derive(Clone)]
pub struct WireHandler {
    pub payload: String,
}
impl WireHandler {
    pub fn new(kind: &str) -> Self {
        let body = match kind {
            "quotes" => "\\\"".repeat(1_638_500),
            "controls" => "\u{0001}".repeat(1_092_000),
            _ => "x".repeat(6_554_000),
        };
        let quoted = serde_json::to_string(&body).unwrap();
        let nested = format!("{}0{}", "[".repeat(100), "]".repeat(100));
        Self {
            payload: format!(
                "{{\"a\":18446744073709551617,\"b\":18446744073709551618,\"null\":null,\"reserved\":{{\"$ssx_number\":\"18446744073709551619\"}},\"deep\":{nested},\"body\":{quoted}}}"
            ),
        }
    }
    pub fn catalogs() -> std::sync::Arc<ssx_mcp::opaque::Catalogs> {
        std::sync::Arc::new(
            [(
                "ssx/native-tools".into(),
                serde_json::value::RawValue::from_string("[\"native_wire_probe\"]".into()).unwrap(),
            )]
            .into_iter()
            .collect(),
        )
    }
}
impl ServerHandler for WireHandler {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }
    async fn call_tool(
        &self,
        _: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let result:CallToolResult=serde_json::from_value(serde_json::json!({"content":[{"type":"text","text":self.payload}],"structuredContent":{},"isError":false,"_meta":{"ssx/native-structured-text":true,"literal":{"number":1844}}})).unwrap();
        Ok(result.into())
    }
}
