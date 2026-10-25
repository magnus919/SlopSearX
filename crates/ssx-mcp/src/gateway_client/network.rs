//! Concrete source-profile HTTP owner. Requires the reviewed local Reqwest fork.
//! Normal requests have no request-wide timeout: P/C/W use C; plaintext reads R300.
//! The final session DELETE has a separate SDK-derived grace in RawGateway.
use crate::{
    Result,
    opaque::{GatewayChunkFuture, GatewayHttpResponse},
};
use reqwest::source_pool::{SourceCookieJar, SourceCookieStore, SourceCookieView, SourceUrl};
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;
pub struct Network {
    client: reqwest::Client,
    pool: reqwest::source_pool::SourcePoolControl,
    connect: Duration,
    oauth: bool,
    stop: CancellationToken,
    cookies: Arc<dyn SourceCookieStore>,
    #[cfg(any(unix, windows))]
    resolver: Arc<super::resolver::GatewayResolver>,
}
pub struct Response {
    response: reqwest::SourceResponse,
    stop: CancellationToken,
}
impl Response {
    pub fn new(response: reqwest::SourceResponse, stop: CancellationToken) -> Self {
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
// HTTPX Origin is distinct from httpcore's truthy-port connection origin.
// Compare IPv6 numerically while retaining the exact scope; no host formatter,
// resolver call, lossy URL roundtrip or early destination port narrowing.
#[derive(PartialEq, Eq)]
enum OriginHost {
    Text(String),
    V6(std::net::Ipv6Addr, Option<String>),
}
fn source_origin(url: &SourceUrl) -> Result<(&str, OriginHost, Option<&str>)> {
    if !url.is_absolute() {
        return Err("source redirect origin requires absolute URL".into());
    }
    let display = url.display_host()?;
    let host = if url.raw_host().contains(':') {
        let ascii = std::str::from_utf8(url.raw_host_bytes()?)?;
        let (address, scope) = ascii
            .split_once('%')
            .map_or((ascii, None), |(a, s)| (a, Some(s.to_owned())));
        OriginHost::V6(address.parse()?, scope)
    } else {
        OriginHost::Text(display)
    };
    let port = url.port().or_else(|| match url.scheme() {
        "ftp" => Some("21"),
        "http" | "ws" => Some("80"),
        "https" | "wss" => Some("443"),
        _ => None,
    });
    Ok((url.scheme(), host, port))
}
// Only initial content-based requests run HTTPX Request._prepare. Redirects
// retain a stream and use _redirect_headers instead, never this helper again.
fn prepare_initial_headers(
    method: &reqwest::Method,
    owner: &SourceUrl,
    headers: &mut reqwest::header::HeaderMap,
    body: &[u8],
) -> Result<()> {
    use reqwest::header::{CONTENT_LENGTH, HOST, HeaderValue, TRANSFER_ENCODING};
    if !body.is_empty()
        && !headers.contains_key(CONTENT_LENGTH)
        && !headers.contains_key(TRANSFER_ENCODING)
    {
        headers.insert(
            CONTENT_LENGTH,
            HeaderValue::from_str(&body.len().to_string())?,
        );
    }
    if !headers.contains_key(HOST) && !owner.display_host()?.is_empty() {
        headers.insert(HOST, HeaderValue::from_bytes(&owner.netloc_bytes()?)?);
    }
    if !headers.contains_key(CONTENT_LENGTH)
        && !headers.contains_key(TRANSFER_ENCODING)
        && matches!(method.as_str(), "POST" | "PUT" | "PATCH" | "QUERY")
    {
        headers.insert(CONTENT_LENGTH, HeaderValue::from_static("0"));
    }
    Ok(())
}
impl Network {
    pub fn create(oauth: bool) -> Result<Arc<Self>> {
        #[cfg(not(any(unix, windows)))]
        {
            let _ = oauth;
            Err("gateway DNS owner is unavailable on this platform".into())
        }
        #[cfg(any(unix, windows))]
        {
            let connect = Duration::from_secs(if oauth { 30 } else { 60 });
            let resolver = super::resolver::GatewayResolver::new(
                super::resolver::install::current_helper()?,
                connect,
            );
            // Only this gateway client opts in. Existing Reqwest clients and
            // SourcePool route cloning retain their unchanged defaults.
            let proxies = reqwest::source_pool::current_environment_proxy_info()?;
            let client = reqwest::source_pool::source_request_builder(connect, &proxies)?
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
        response: &mut reqwest::SourceResponse,
    ) -> Result<Option<axum::body::Bytes>> {
        tokio::select! {biased;_=self.stop.cancelled()=>Err("gateway HTTP owner closed".into()),value=response.chunk()=>Ok(value?)}
    }
    #[cfg(test)]
    pub(crate) fn prepare_cookies(
        &self,
        url: &str,
        headers: &mut reqwest::header::HeaderMap,
    ) -> Result<()> {
        self.prepare_source_cookies(&SourceUrl::parse(url)?, headers)
    }
    fn prepare_source_cookies(
        &self,
        url: &SourceUrl,
        headers: &mut reqwest::header::HeaderMap,
    ) -> Result<()> {
        if self.cookies.has_cookies() {
            // The original cookie identity is the normalized owner before auth.
            let view = SourceCookieView::parse(&url.serialized(), headers)?;
            if let Some(value) = self.cookies.cookies(&view)? {
                headers.insert(reqwest::header::COOKIE, value);
            }
        }
        Ok(())
    }
    /// Prepare cookies before the MCP origin/method and redirect budget gates.
    fn prepare_redirect(
        &self,
        method: &reqwest::Method,
        current: &SourceUrl,
        response: &reqwest::SourceResponse,
        headers: &mut reqwest::header::HeaderMap,
    ) -> Result<Option<Arc<SourceUrl>>> {
        let Some(candidate) =
            super::redirect::source_location_url(current, response.status(), response.headers())?
        else {
            return Ok(None);
        };
        let next_method = super::redirect::source_redirect_method(method, response.status());
        let previous_origin = source_origin(current)?;
        let next_origin = source_origin(&candidate)?;
        if previous_origin != next_origin {
            let upgrade = previous_origin.1 == next_origin.1
                && previous_origin.0 == "http"
                && previous_origin.2 == Some("80")
                && next_origin.0 == "https"
                && next_origin.2 == Some("443");
            if !upgrade {
                headers.remove(reqwest::header::AUTHORIZATION);
            }
            headers.insert(
                reqwest::header::HOST,
                reqwest::header::HeaderValue::from_bytes(&candidate.netloc_bytes()?)?,
            );
        }
        if next_method != *method && next_method == reqwest::Method::GET {
            headers.remove(reqwest::header::CONTENT_LENGTH);
            headers.remove(reqwest::header::TRANSFER_ENCODING);
        }
        headers.remove(reqwest::header::COOKIE);
        self.prepare_source_cookies(&candidate, headers)?;
        if super::redirect::source_follows(method, current, &candidate, response.status())? {
            Ok(Some(Arc::new(candidate)))
        } else {
            Ok(None)
        }
    }
    pub async fn send_once(
        &self,
        method: reqwest::Method,
        url: String,
        mut headers: reqwest::header::HeaderMap,
        body: Vec<u8>,
    ) -> Result<reqwest::SourceResponse> {
        let owner = Arc::new(SourceUrl::parse(&url)?);
        self.prepare_source_cookies(&owner, &mut headers)?;
        prepare_initial_headers(&method, &owner, &mut headers, &body)?;
        self.send_prepared(method, owner, &mut headers, body).await
    }
    async fn send_prepared(
        &self,
        method: reqwest::Method,
        owner: Arc<SourceUrl>,
        headers: &mut reqwest::header::HeaderMap,
        body: Vec<u8>,
    ) -> Result<reqwest::SourceResponse> {
        let view = SourceCookieView::parse(&owner.serialized(), headers)?;
        // Preserve the actual sent Authorization in the request header lineage.
        // Redirect preparation starts from the sent request, not an earlier
        // caller map that predates configured URL Basic authentication.
        if !self.oauth && (!owner.username().is_empty() || !owner.password().is_empty()) {
            use base64::Engine;
            let credentials = format!("{}:{}", owner.username(), owner.password());
            let mut value = reqwest::header::HeaderValue::from_str(&format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes())
            ))?;
            value.set_sensitive(true);
            headers.insert(reqwest::header::AUTHORIZATION, value);
        }
        let operation = self
            .client
            .source_request(method, owner)
            .url_auth(!self.oauth)
            .headers(headers.clone())
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
    ) -> Result<reqwest::SourceResponse> {
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
    ) -> Result<reqwest::SourceResponse> {
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
    ) -> Result<reqwest::SourceResponse> {
        let mut headers = headers;
        let mut url = Arc::new(SourceUrl::parse(&url)?);
        if merge_initial_cookies {
            self.prepare_source_cookies(&url, &mut headers)?;
        }
        prepare_initial_headers(&method, &url, &mut headers, &body)?;
        let mut followed = 0;
        loop {
            let mut response = self
                .send_prepared(method.clone(), url.clone(), &mut headers, body.clone())
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
        #[cfg(any(unix, windows))]
        self.resolver.close().await?;
        Ok(())
    }
}

#[cfg(all(test, unix))]
#[path = "network_cookie_tests.rs"]
mod cookie_wire_tests;
