//! Bounded POST adapter for rmcp's SDK client. The SDK 3.5.1 reqwest backend
//! buffers JSON/error bodies without a size limit; GET/resume and DELETE retain
//! the SDK implementation. Status/auth/session behavior follows that backend.
use axum::http::{HeaderName, HeaderValue, StatusCode};
use futures_util::StreamExt;
use rmcp::{
    model::{ClientJsonRpcMessage, ClientRequest, ServerJsonRpcMessage},
    transport::{common::client_side_sse::BoxedSseResponse, streamable_http_client::*},
};
use std::{collections::HashMap, sync::Arc};
pub const MAX_RPC_BYTES: usize = 4 * 1024 * 1024;
#[derive(Clone)]
pub struct BoundedClient(pub reqwest::Client);
fn invalid(message: impl Into<String>) -> StreamableHttpError<reqwest::Error> {
    StreamableHttpError::Io(std::io::Error::other(message.into()))
}
fn scope(header: &str) -> Option<String> {
    let pos = header.to_ascii_lowercase().find("scope=")?;
    let value = &header[pos + 6..];
    if let Some(value) = value.strip_prefix('"') {
        Some(value.split('"').next()?.to_owned())
    } else {
        Some(
            value
                .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
                .next()?
                .to_owned(),
        )
    }
}
impl StreamableHttpClient for BoundedClient {
    type Error = reqwest::Error;
    async fn get_stream(
        &self,
        uri: Arc<str>,
        session: Option<Arc<str>>,
        last: Option<String>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<BoxedSseResponse, StreamableHttpError<Self::Error>> {
        self.get_stream_with_max_sse_event_size(uri, session, last, auth, headers, MAX_RPC_BYTES)
            .await
    }
    async fn get_stream_with_max_sse_event_size(
        &self,
        uri: Arc<str>,
        session: Option<Arc<str>>,
        last: Option<String>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
        max: usize,
    ) -> Result<BoxedSseResponse, StreamableHttpError<Self::Error>> {
        self.0
            .get_stream_with_max_sse_event_size(
                uri,
                session,
                last,
                auth,
                headers,
                max.min(MAX_RPC_BYTES),
            )
            .await
    }
    async fn delete_session(
        &self,
        uri: Arc<str>,
        session: Arc<str>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<(), StreamableHttpError<Self::Error>> {
        self.0.delete_session(uri, session, auth, headers).await
    }
    async fn post_message(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session: Option<Arc<str>>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        self.post_message_with_max_sse_event_size(
            uri,
            message,
            session,
            auth,
            headers,
            MAX_RPC_BYTES,
        )
        .await
    }
    async fn post_message_with_max_sse_event_size(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session: Option<Arc<str>>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
        max: usize,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        let mut request = self
            .0
            .post(uri.as_ref())
            .header("accept", "application/json, text/event-stream");
        if let Some(auth) = auth {
            request = request.bearer_auth(auth);
        }
        // Match SDK reserved-header policy; negotiated protocol headers are allowed.
        for (name, value) in headers {
            if ["accept", "mcp-session-id", "last-event-id"].contains(&name.as_str()) {
                return Err(StreamableHttpError::ReservedHeaderConflict(
                    name.to_string(),
                ));
            }
            request = request.header(name, value);
        }
        let attached = session.is_some();
        if let Some(session) = session {
            request = request.header("mcp-session-id", session.as_ref());
        }
        let mut response = request
            .json(&message)
            .send()
            .await
            .map_err(StreamableHttpError::Client)?;
        let status = response.status();
        if let Some(header) = response.headers().get("www-authenticate") {
            let header = header
                .to_str()
                .map_err(|_| invalid("invalid www-authenticate header"))?
                .to_owned();
            if status == StatusCode::UNAUTHORIZED {
                return Err(StreamableHttpError::AuthRequired(AuthRequiredError::new(
                    header,
                )));
            }
            if status == StatusCode::FORBIDDEN {
                let required = scope(&header);
                return Err(StreamableHttpError::InsufficientScope(
                    InsufficientScopeError::new(header, required),
                ));
            }
        }
        if status == StatusCode::ACCEPTED || status == StatusCode::NO_CONTENT {
            return Ok(StreamableHttpPostResponse::Accepted);
        }
        if status == StatusCode::NOT_FOUND && attached {
            return Err(StreamableHttpError::SessionExpired);
        }
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let session = response
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let notification = !matches!(message, ClientJsonRpcMessage::Request(_));
        if status.is_success() && notification && response.content_length() == Some(0) {
            return Ok(StreamableHttpPostResponse::Accepted);
        }
        if response
            .content_length()
            .is_some_and(|n| n > MAX_RPC_BYTES as u64)
        {
            return Err(invalid("MCP response Content-Length exceeds 4 MiB"));
        }
        if status.is_success()
            && content_type
                .as_deref()
                .is_some_and(|v| v.starts_with("text/event-stream"))
        {
            let limit = max.min(MAX_RPC_BYTES);
            let bytes = futures_util::stream::try_unfold(
                (response, 0usize),
                move |(mut response, total)| async move {
                    let Some(chunk) = response.chunk().await.map_err(std::io::Error::other)? else {
                        return Ok(None);
                    };
                    let total = total.saturating_add(chunk.len());
                    if total > limit {
                        return Err(std::io::Error::other("MCP POST SSE response exceeds 4 MiB"));
                    }
                    Ok(Some((chunk, (response, total))))
                },
            );
            return Ok(StreamableHttpPostResponse::Sse(
                sse_stream::SseStream::from_bytes_stream(bytes).boxed(),
                session,
            ));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(StreamableHttpError::Client)?
        {
            if body.len().saturating_add(chunk.len()) > MAX_RPC_BYTES {
                return Err(invalid("MCP response exceeds 4 MiB"));
            }
            body.extend_from_slice(&chunk);
        }
        if !status.is_success() {
            // Preserve the SDK's modern-discover fallback/correlation behavior.
            if !attached
                && status.is_client_error()
                && ![StatusCode::UNAUTHORIZED, StatusCode::FORBIDDEN].contains(&status)
                && let ClientJsonRpcMessage::Request(request) = &message
                && matches!(request.request, ClientRequest::DiscoverRequest(_))
            {
                let error = match serde_json::from_slice::<ServerJsonRpcMessage>(&body) {
                    Ok(ServerJsonRpcMessage::Error(error)) => error.error,
                    _ => rmcp::ErrorData::invalid_request(
                        format!("server/discover rejected with HTTP {status}"),
                        None,
                    ),
                };
                return Ok(StreamableHttpPostResponse::Json(
                    ServerJsonRpcMessage::error(error, Some(request.id.clone())),
                    None,
                ));
            }
            if content_type
                .as_deref()
                .is_some_and(|v| v.starts_with("application/json"))
                && let Ok(error @ ServerJsonRpcMessage::Error(_)) = serde_json::from_slice(&body)
            {
                return Ok(StreamableHttpPostResponse::Json(error, session));
            }
            return Err(invalid(format!("remote MCP HTTP status {status}")));
        }
        if !content_type
            .as_deref()
            .is_some_and(|v| v.starts_with("application/json"))
        {
            return Err(StreamableHttpError::UnexpectedContentType(content_type));
        }
        match serde_json::from_slice(&body) {
            Ok(message) => Ok(StreamableHttpPostResponse::Json(message, session)),
            Err(_) if notification => Ok(StreamableHttpPostResponse::Accepted),
            Err(error) => Err(invalid(format!("invalid MCP JSON response: {error}"))),
        }
    }
}
