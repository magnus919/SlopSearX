//! Executable composition. Every async owner exists before cancellable startup.
use super::{
    browser::BrowserOwner, callback::LoopbackCallback, io_owner::IoOwner, network::Network,
    oauth::OAuth, settings::Settings,
};
use crate::{
    Result,
    opaque::{
        GatewayAuthMode, GatewayHttp, GatewayHttpRequest, GatewayHttpResponse, GatewayOpenFuture,
        RawGatewayStart,
    },
};
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;
struct Http {
    network: Arc<Network>,
    oauth: Option<Arc<OAuth>>,
    auth_cancel: CancellationToken,
    last_access_token: tokio::sync::Mutex<Option<String>>,
}
impl GatewayHttp for Http {
    fn open(&self, request: GatewayHttpRequest) -> GatewayOpenFuture<'_> {
        Box::pin(async move {
            let mut headers = reqwest::header::HeaderMap::new();
            for (name, value) in request.headers {
                let name = reqwest::header::HeaderName::from_static(name);
                let mut value = reqwest::header::HeaderValue::from_str(&value)?;
                if name == reqwest::header::AUTHORIZATION {
                    value.set_sensitive(true)
                }
                headers.insert(name, value);
            }
            // Cancellation and cleanup use only an already-held bearer. This
            // mutex is independent of OAuth.manager, which the retained original
            // request may hold while suspended in its callback wait. Even an
            // expired token cannot trigger refresh or authorization for controls.
            let may_authorize = request.auth == GatewayAuthMode::Interactive
                && request.method != reqwest::Method::DELETE
                && !self.auth_cancel.is_cancelled();
            if let Some(auth) = &self.oauth {
                let token = if may_authorize {
                    auth.request_protocol(
                        headers
                            .get("mcp-protocol-version")
                            .and_then(|v| v.to_str().ok()),
                    )
                    .await;
                    let token = auth.cached_access_token().await?;
                    *self.last_access_token.lock().await = token.clone();
                    token
                } else {
                    self.last_access_token.lock().await.clone()
                };
                if let Some(token) = token {
                    let mut value =
                        reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))?;
                    value.set_sensitive(true);
                    headers.insert(reqwest::header::AUTHORIZATION, value);
                }
            }
            let body = request.body.unwrap_or_default().into_bytes();
            let mut response = self
                .network
                .send(
                    request.method.clone(),
                    request.url.clone(),
                    headers.clone(),
                    body.clone(),
                )
                .await?;
            let challenge = response
                .headers()
                .get(reqwest::header::WWW_AUTHENTICATE)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            let step_up = response.status() == reqwest::StatusCode::FORBIDDEN
                && OAuth::scope_challenge(challenge.as_deref(), &request.url);
            if may_authorize
                && !self.auth_cancel.is_cancelled()
                && (response.status() == reqwest::StatusCode::UNAUTHORIZED || step_up)
                && let Some(auth) = &self.oauth
            {
                drop(response);
                let token = auth.authorize(challenge.as_deref(), step_up).await?;
                *self.last_access_token.lock().await = Some(token.clone());
                let mut value = reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))?;
                value.set_sensitive(true);
                headers.insert(reqwest::header::AUTHORIZATION, value);
                response = self
                    .network
                    .send(request.method, request.url, headers, body)
                    .await?;
            }
            Ok(Box::new(super::network::Response::new(
                response,
                self.network.cancellation(),
            )) as Box<dyn GatewayHttpResponse>)
        })
    }
}
/// Runtime errors intentionally never format URLs, HTTP/SDK error chains or tokens.
/// The executable prints a fixed classification; private qualification asserts detail.
pub async fn run(settings: Settings, cancel: CancellationToken) -> Result<()> {
    let io = Arc::new(IoOwner::new());
    let browser = Arc::new(BrowserOwner::default());
    let auth_cancel = CancellationToken::new();
    let callback: Arc<tokio::sync::Mutex<Option<LoopbackCallback>>> = Default::default();
    let network = Network::create(settings.oauth)?;
    let mut startup: Option<RawGatewayStart> = None;
    let serving = async {
        let oauth = if settings.oauth {
            Some(
                OAuth::create(
                    &settings,
                    io.clone(),
                    auth_cancel.clone(),
                    network.clone(),
                    callback.clone(),
                    browser.clone(),
                )
                .await?,
            )
        } else {
            None
        };
        let http = Arc::new(Http {
            network: network.clone(),
            oauth,
            auth_cancel: auth_cancel.clone(),
            last_access_token: Default::default(),
        });
        startup = Some(RawGatewayStart::new_source(
            settings.remote_url.clone(),
            settings.token.clone(),
            http,
        )?);
        let start = startup.as_mut().ok_or("gateway startup owner absent")?;
        start
            .initialize((!settings.oauth).then_some(Duration::from_secs(60)))
            .await?;
        start.discover_tools(Duration::from_secs(60)).await?;
        start.install_source_projection(super::source_projection::catalogs()?)?;
        start.ready()
    };
    let prepared = tokio::select! {biased;_=cancel.cancelled()=>Ok(None),v=serving=>v.map(Some)};
    let result = match prepared {
        Ok(Some(gateway)) => gateway.stdio_owned(cancel.clone()).await,
        Ok(None) => Ok(()),
        Err(error) => Err(error),
    };
    // Revoke OAuth admission before remote cleanup. DELETE keeps the HTTP owner
    // alive and sends only the in-memory bearer, without credential-store work.
    // Its bounded grace may fail; every later cleanup still runs before errors
    // are combined, including the actual DNS supervisor/child joins.
    auth_cancel.cancel();
    let remote = if let Some(start) = startup.as_mut() {
        start.close().await
    } else {
        Ok(())
    };
    let callback_result = if let Some(cb) = callback.lock().await.as_mut() {
        cb.shutdown()
            .await
            .map_err(|_| "gateway callback cleanup failed".into())
    } else {
        Ok(())
    };
    let browser_result = browser.shutdown().await;
    let network_result = network.shutdown().await;
    io.close();
    let files = io
        .shutdown()
        .await
        .map_err(|_| "gateway file cleanup failed".into());
    result
        .and(remote)
        .and(callback_result)
        .and(browser_result)
        .and(network_result)
        .and(files)
}
