//! Executable composition. Every async owner exists before cancellable startup.
use super::diagnostics::{Diagnostics, Stage};
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
    diagnostics: Arc<Diagnostics>,
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
            // Observe numeric status before the opaque protocol boundary maps it
            // into a generic error. No response text or headers enter diagnostics.
            if response.status().is_client_error() || response.status().is_server_error() {
                self.diagnostics.record_http_status(response.status());
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
    let diagnostics = Arc::new(Diagnostics::from_env());
    let io = Arc::new(IoOwner::with_diagnostics(diagnostics.clone()));
    let browser = Arc::new(BrowserOwner::default());
    let auth_cancel = CancellationToken::new();
    let callback: Arc<tokio::sync::Mutex<Option<LoopbackCallback>>> = Default::default();
    // The one file owner exists before cancellable CA startup. Every error path
    // drains its original worker before returning; no detached filesystem work.
    let network = match async {
        let end = std::time::Instant::now() + Duration::from_secs(30);
        let roots = super::ca_bundle::load(
            std::env::var_os("MCP_REMOTE_CA_BUNDLE").map(std::path::PathBuf::from),
            &io,
            end,
            &cancel,
        )
        .await?;
        Network::create(settings.oauth, roots, &io, end, &cancel).await
    }
    .await
    {
        Ok(network) => network,
        Err(error) => {
            diagnostics.record(Stage::NetworkStartup, error.as_ref());
            io.close();
            let drained = io
                .shutdown()
                .await
                .map_err(|_| "gateway file cleanup failed".into());
            let result = Err::<(), _>(error).and(drained);
            let _ = diagnostics.emit(&mut std::io::stderr().lock());
            return result;
        }
    };

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
                .await
                .inspect_err(|error| diagnostics.record(Stage::OAuthStartup, error.as_ref()))?,
            )
        } else {
            None
        };
        let http = Arc::new(Http {
            network: network.clone(),
            oauth,
            auth_cancel: auth_cancel.clone(),
            last_access_token: Default::default(),
            diagnostics: diagnostics.clone(),
        });
        startup = Some(
            RawGatewayStart::new_source(settings.remote_url.clone(), settings.token.clone(), http)
                .inspect_err(|error| {
                    diagnostics.record(Stage::RemoteConstruction, error.as_ref())
                })?,
        );
        let start = startup.as_mut().ok_or("gateway startup owner absent")?;
        start
            .initialize((!settings.oauth).then_some(Duration::from_secs(60)))
            .await
            .inspect_err(|error| diagnostics.record(Stage::RemoteInitialization, error.as_ref()))?;
        start
            .discover_tools(Duration::from_secs(60))
            .await
            .inspect_err(|error| diagnostics.record(Stage::RemoteDiscovery, error.as_ref()))?;
        start
            .install_source_projection(super::source_projection::catalogs()?)
            .inspect_err(|error| diagnostics.record(Stage::Projection, error.as_ref()))?;
        start.ready()
    };
    let prepared = tokio::select! {biased;_=cancel.cancelled()=>Ok(None),v=serving=>v.map(Some)};
    let result = match prepared {
        Ok(Some(gateway)) => gateway
            .stdio_owned(cancel.clone())
            .await
            .inspect_err(|error| diagnostics.record(Stage::Stdio, error.as_ref())),
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
    let callback_result: crate::Result<()> = if let Some(cb) = callback.lock().await.as_mut() {
        cb.shutdown()
            .await
            .map_err(|_| "gateway callback cleanup failed".into())
    } else {
        Ok(())
    };
    if let Err(error) = &remote {
        diagnostics.record(Stage::RemoteCleanup, error.as_ref());
    }
    if let Err(error) = &callback_result {
        diagnostics.record(Stage::CallbackCleanup, error.as_ref());
    }
    let browser_result = browser
        .shutdown()
        .await
        .inspect_err(|error| diagnostics.record(Stage::BrowserCleanup, error.as_ref()));
    let network_result = network
        .shutdown()
        .await
        .inspect_err(|error| diagnostics.record(Stage::NetworkCleanup, error.as_ref()));
    io.close();
    let files = io
        .shutdown()
        .await
        .map_err(|_| -> crate::Error { "gateway file cleanup failed".into() })
        .inspect_err(|error| diagnostics.record(Stage::FileCleanup, error.as_ref()));
    let result = result
        .and(remote)
        .and(callback_result)
        .and(browser_result)
        .and(network_result)
        .and(files);
    let _ = diagnostics.emit(&mut std::io::stderr().lock());
    result
}
