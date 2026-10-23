//! Concrete source-profile HTTP owner. Requires the reviewed local Reqwest fork.
//! Normal requests have no request-wide timeout: P/C/W use C; plaintext reads R300.
//! The final session DELETE has a separate SDK-derived grace in RawGateway.
use crate::{
    Result,
    opaque::{GatewayChunkFuture, GatewayHttpResponse},
};
use reqwest::source_pool::{SourceCookieJar, SourceCookieStore, SourceCookieView};
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;
pub struct Network {
    client: reqwest::Client,
    pool: reqwest::source_pool::SourcePoolControl,
    connect: Duration,
    oauth: bool,
    stop: CancellationToken,
    cookies: Arc<dyn SourceCookieStore>,
    #[cfg(unix)]
    resolver: Arc<super::resolver::GatewayResolver>,
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
        #[cfg(not(unix))]
        {
            let _ = oauth;
            Err("gateway DNS owner requires Unix".into())
        }
        #[cfg(unix)]
        {
            let connect = Duration::from_secs(if oauth { 30 } else { 60 });
            let resolver = super::resolver::GatewayResolver::new(
                super::resolver::install::current_helper()?,
                connect,
            );
            // Only this gateway client opts in. Existing Reqwest clients and
            // SourcePool route cloning retain their unchanged defaults.
            let client = reqwest::source_pool::gateway_builder_from_environment(connect)?
                .dns_resolver(resolver.clone())
                .build()?;
            let pool = client
                .source_http1_pool_control()
                .ok_or("gateway source pool absent")?;
            Ok(Arc::new(Self {
                client,
                pool,
                connect,
                oauth,
                stop: CancellationToken::new(),
                cookies: Arc::new(SourceCookieJar::default()),
                resolver,
            }))
        }
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
    pub(crate) fn prepare_cookies(
        &self,
        url: &str,
        headers: &mut reqwest::header::HeaderMap,
    ) -> Result<()> {
        if self.cookies.has_cookies() {
            let view = SourceCookieView::parse(url, headers)?;
            if let Some(value) = self.cookies.cookies(&view)? {
                headers.insert(reqwest::header::COOKIE, value);
            }
        }
        Ok(())
    }
    /// HTTPX builds the next cookie-bearing request before the MCP follow gate,
    /// including an unfollowed redirect or a reached redirect budget.
    pub(crate) fn prepare_redirect(
        &self,
        method: &reqwest::Method,
        current: &url::Url,
        response: &reqwest::Response,
        headers: &mut reqwest::header::HeaderMap,
    ) -> Result<Option<url::Url>> {
        let location = response.headers().get(reqwest::header::LOCATION);
        let Some(candidate) = super::redirect::location_url(current, response.status(), location)
        else {
            return Ok(None);
        };
        headers.remove(reqwest::header::COOKIE);
        self.prepare_cookies(candidate.as_str(), headers)?;
        Ok(super::redirect::next_url(
            method,
            current,
            response.status(),
            location,
        ))
    }
    pub async fn send_once(
        &self,
        method: reqwest::Method,
        url: String,
        mut headers: reqwest::header::HeaderMap,
        body: Vec<u8>,
    ) -> Result<reqwest::Response> {
        self.prepare_cookies(&url, &mut headers)?;
        self.send_prepared(method, url, headers, body).await
    }
    pub(crate) async fn send_prepared(
        &self,
        method: reqwest::Method,
        url: String,
        mut headers: reqwest::header::HeaderMap,
        body: Vec<u8>,
    ) -> Result<reqwest::Response> {
        let view = SourceCookieView::parse(&url, &headers)?;
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
        let response = tokio::select! {biased;_=self.stop.cancelled()=>return Err("gateway HTTP owner closed".into()),v=operation=>v?};
        if response.headers().contains_key(reqwest::header::SET_COOKIE)
            || response.headers().contains_key("set-cookie2")
        {
            self.cookies.extract(&view, response.headers())?;
        }
        Ok(response)
    }
    /// MCP stream_within_origin builds an initial request from the client jar.
    pub async fn send(
        &self,
        method: reqwest::Method,
        url: String,
        headers: reqwest::header::HeaderMap,
        body: Vec<u8>,
    ) -> Result<reqwest::Response> {
        self.send_loop(method, url, headers, body, 20, true).await
    }
    /// Source OAuth yields already-built HTTPX Request objects. Client.send
    /// extracts response cookies but does not merge client defaults into those
    /// initial requests. Redirect construction does use the shared jar.
    pub(crate) async fn send_built_request(
        &self,
        method: reqwest::Method,
        url: String,
        headers: reqwest::header::HeaderMap,
        body: Vec<u8>,
        max_redirects: usize,
    ) -> Result<reqwest::Response> {
        self.send_loop(method, url, headers, body, max_redirects, false)
            .await
    }
    async fn send_loop(
        &self,
        method: reqwest::Method,
        url: String,
        headers: reqwest::header::HeaderMap,
        body: Vec<u8>,
        max_redirects: usize,
        merge_initial_cookies: bool,
    ) -> Result<reqwest::Response> {
        let mut headers = headers;
        if merge_initial_cookies {
            self.prepare_cookies(&url, &mut headers)?;
        }
        let mut url = url::Url::parse(&url)?;
        let mut followed = 0;
        loop {
            let mut response = self
                .send_prepared(
                    method.clone(),
                    url.to_string(),
                    headers.clone(),
                    body.clone(),
                )
                .await?;
            let next = self.prepare_redirect(&method, &url, &response, &mut headers)?;
            if followed == max_redirects || next.is_none() {
                return Ok(response);
            }
            while self.chunk(&mut response).await?.is_some() {}
            url = next.unwrap();
            followed += 1;
        }
    }
    pub async fn shutdown(&self) -> Result<()> {
        self.stop.cancel();
        self.pool.close().await;
        // DELETE/callback/browser cleanup is finished before the network owner
        // closes DNS admission. Then join every actual registered helper child.
        #[cfg(unix)]
        self.resolver.close().await?;
        Ok(())
    }
}

#[cfg(all(test, unix))]
#[path = "network_cookie_tests.rs"]
mod cookie_wire_tests;
