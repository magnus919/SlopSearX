//! Concrete source-profile HTTP owner. Requires the reviewed local Reqwest fork.
//! No request-wide timeout: P/C/W use C; each actual plaintext read uses R300.
use crate::{
    Result,
    opaque::{GatewayChunkFuture, GatewayHttpResponse},
};
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;
pub struct Network {
    client: reqwest::Client,
    pool: reqwest::source_pool::SourcePoolControl,
    connect: Duration,
    oauth: bool,
    stop: CancellationToken,
}
pub struct Response {
    response: reqwest::Response,
    stop: CancellationToken,
}
impl Response {
    pub fn new(response: reqwest::Response, stop: CancellationToken) -> Self {
        Self { response, stop }
    }
}
impl GatewayHttpResponse for Response {
    fn status(&self) -> reqwest::StatusCode {
        self.response.status()
    }
    fn headers(&self) -> &reqwest::header::HeaderMap {
        self.response.headers()
    }
    fn content_length(&self) -> Option<u64> {
        self.response.content_length()
    }
    fn chunk(&mut self) -> GatewayChunkFuture<'_> {
        Box::pin(async move { self.chunk_owned().await })
    }
}
impl Response {
    async fn chunk_owned(&mut self) -> Result<Option<axum::body::Bytes>> {
        tokio::select! {biased;_=self.stop.cancelled()=>Err("gateway HTTP owner closed".into()),value=self.response.chunk()=>Ok(value?)}
    }
}
impl Network {
    pub fn create(oauth: bool) -> Result<Arc<Self>> {
        let connect = Duration::from_secs(if oauth { 30 } else { 60 });
        let client = reqwest::source_pool::gateway_builder_from_environment(connect)?.build()?;
        let pool = client
            .source_http1_pool_control()
            .ok_or("gateway source pool absent")?;
        Ok(Arc::new(Self {
            client,
            pool,
            connect,
            oauth,
            stop: CancellationToken::new(),
        }))
    }
    pub fn cancellation(&self) -> CancellationToken {
        self.stop.clone()
    }
    pub async fn chunk(
        &self,
        response: &mut reqwest::Response,
    ) -> Result<Option<axum::body::Bytes>> {
        tokio::select! {biased;_=self.stop.cancelled()=>Err("gateway HTTP owner closed".into()),value=response.chunk()=>Ok(value?)}
    }
    pub async fn send_once(
        &self,
        method: reqwest::Method,
        url: String,
        mut headers: reqwest::header::HeaderMap,
        body: Vec<u8>,
    ) -> Result<reqwest::Response> {
        let url = super::redirect::wire_url(&url, self.oauth, &mut headers)?;
        // The source fork selects the trusted mount on every actual attempt.
        // Source owner shutdown cancels dispatch, then joins all actual drivers.
        let operation = self
            .client
            .request(method, url)
            .headers(headers)
            .body(body)
            .source_http1_timeouts(self.connect, Duration::from_secs(300))
            .send();
        tokio::select! {biased;_=self.stop.cancelled()=>Err("gateway HTTP owner closed".into()),v=operation=>Ok(v?)}
    }
    /// MCP stream_within_origin uses HTTPX's independent default20-hop cap.
    pub async fn send(
        &self,
        method: reqwest::Method,
        url: String,
        headers: reqwest::header::HeaderMap,
        body: Vec<u8>,
    ) -> Result<reqwest::Response> {
        let mut url = url::Url::parse(&url)?;
        let mut followed = 0;
        loop {
            let mut response = self
                .send_once(
                    method.clone(),
                    url.to_string(),
                    headers.clone(),
                    body.clone(),
                )
                .await?;
            let next = super::redirect::next_url(
                &method,
                &url,
                response.status(),
                response.headers().get(reqwest::header::LOCATION),
            );
            if followed == 20 || next.is_none() {
                return Ok(response);
            }
            while self.chunk(&mut response).await?.is_some() {}
            url = next.unwrap();
            followed += 1;
        }
    }
    pub async fn shutdown(&self) {
        self.stop.cancel();
        self.pool.close().await;
    }
}
