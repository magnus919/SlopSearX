//! Serialize credential-changing operations while allowing nonblocking control traffic.

use super::{
    callback::LoopbackCallback,
    protocol::{OAuthProtocol, RegisteredClient, valid_bearer},
    storage::{self, Section, TokenDocument},
};
use crate::{Result, opaque::GatewayAuthorizer};
use futures_util::future::BoxFuture;
use oauth2::{Scope, TokenResponse};
use reqwest::{StatusCode, header::HeaderValue};
use rmcp::transport::auth::{OAuthTokenResponse, WWWAuthenticateParams};
use serde::de::DeserializeOwned;
use serde_json::value::{RawValue, to_raw_value};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::Stdio,
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio::{
    process::Command,
    sync::{Mutex, OwnedMutexGuard},
    task::spawn_blocking,
};
use url::Url;

/// The existing Python gateway allows five minutes to complete a browser sign-in.
pub const AUTHORIZATION_TIMEOUT: Duration = Duration::from_secs(300);

pub struct GatewayOAuth {
    remote: Url,
    path: PathBuf,
    callback_port: u16,
    no_browser: bool,
    document: Arc<Mutex<TokenDocument>>,
    cached: Arc<RwLock<Option<Bearer>>>,
}

struct Bearer {
    value: String,
    scopes: Vec<String>,
}

impl GatewayOAuth {
    pub async fn open(
        remote: &str,
        path: Option<PathBuf>,
        callback_port: u16,
        no_browser: bool,
    ) -> Result<Self> {
        let url = Url::parse(remote).map_err(|_| "Invalid OAuth remote URL")?;
        let path = match path {
            Some(path) => path,
            None => storage::default_path(remote)?,
        };
        let path_for_read = path.clone();
        let document = spawn_blocking(move || storage::load(&path_for_read)).await??;
        // Validate both sections before forwarding an imported bearer.
        let _: Option<RegisteredClient> = section(&document, Section::ClientInfo)?;
        let tokens: Option<OAuthTokenResponse> = section(&document, Section::Tokens)?;
        let cached = tokens.as_ref().map(Bearer::from_tokens).transpose()?;
        Ok(Self {
            remote: url,
            path,
            callback_port,
            no_browser,
            document: Arc::new(Mutex::new(document)),
            cached: Arc::new(RwLock::new(cached)),
        })
    }

    fn challenge<'a>(
        &self,
        challenges: &'a [HeaderValue],
        scope_upgrade: bool,
    ) -> Option<(&'a str, WWWAuthenticateParams)> {
        challenges
            .iter()
            .filter_map(|value| value.to_str().ok())
            .find_map(|value| {
                let scheme = value.split_ascii_whitespace().next()?;
                if !scheme.eq_ignore_ascii_case("Bearer") {
                    return None;
                }
                let parameters = WWWAuthenticateParams::parse(value, &self.remote);
                (!scope_upgrade || parameters.is_insufficient_scope())
                    .then_some((value, parameters))
            })
    }

    async fn authorize_request(
        &self,
        status: StatusCode,
        sent_bearer: Option<&str>,
        challenges: &[HeaderValue],
    ) -> Result<Option<String>> {
        let scope_upgrade = status == StatusCode::FORBIDDEN;
        let challenge = self.challenge(challenges, scope_upgrade);
        let challenged = challenge
            .as_ref()
            .and_then(|(_, parameters)| parameters.scope.as_deref());

        let document = self.document.clone().lock_owned().await;
        if let Some(cached) = self.cached.read().unwrap().as_ref()
            && Some(cached.value.as_str()) != sent_bearer
            && (!scope_upgrade || scopes_cover(&cached.scopes, challenged))
        {
            return Ok(Some(cached.value.clone()));
        }

        let client: Option<RegisteredClient> = section(&document, Section::ClientInfo)?;
        let tokens: Option<OAuthTokenResponse> = section(&document, Section::Tokens)?;
        let granted = tokens
            .as_ref()
            .map(Bearer::from_tokens)
            .transpose()?
            .map_or_else(Vec::new, |bearer| bearer.scopes);
        let mut protocol =
            OAuthProtocol::discover(&self.remote, challenge.as_ref().map(|(header, _)| *header))
                .await?;
        if let Some(client) = &client
            && client
                .issuer
                .as_deref()
                .is_some_and(|issuer| Some(issuer) != protocol.issuer())
        {
            return Err("Saved OAuth registration belongs to a different issuer".into());
        }

        if !scope_upgrade
            && let (Some(client), Some(tokens)) = (&client, tokens)
            && let Some(redirect) = client.redirect_uris.first()
        {
            protocol.configure_client(client, redirect)?;
            if let Some(tokens) = protocol.refresh(client, tokens, granted.clone()).await? {
                return self
                    .persist(document, client, true, &protocol, tokens)
                    .await
                    .map(Some);
            }
        }

        let scopes = if scope_upgrade || !granted.is_empty() {
            protocol.select_scopes(challenged, &granted)
        } else {
            protocol.select_initial_scopes().await
        };
        let callback = LoopbackCallback::bind(self.callback_port)
            .await
            .map_err(|_| "Could not bind OAuth browser callback")?;
        let redirect = callback.redirect_uri();
        let (client, retain_registration) = if let Some(client) =
            client.filter(|client| client.redirect_uris.contains(&redirect))
        {
            protocol.configure_client(&client, &redirect)?;
            (client, true)
        } else {
            (protocol.register(&redirect, &scopes).await?, false)
        };
        let browser = protocol.begin(&scopes).await?;
        eprintln!(
            "Authorize the SlopSearX gateway by visiting:\n  {}\nWaiting for the redirect to {redirect}.",
            browser.url
        );
        let (code, issuer) =
            wait_for_browser(callback, &browser.state, &browser.url, self.no_browser)
                .await?
                .into_parts();
        let tokens = protocol
            .exchange(&code, &browser.state, issuer.as_deref())
            .await?;
        self.persist(document, &client, retain_registration, &protocol, tokens)
            .await
            .map(Some)
    }

    async fn persist(
        &self,
        mut document: OwnedMutexGuard<TokenDocument>,
        client: &RegisteredClient,
        retain_registration: bool,
        protocol: &OAuthProtocol,
        mut tokens: OAuthTokenResponse,
    ) -> Result<String> {
        if tokens.scopes().is_none() {
            let scopes: Vec<_> = protocol
                .granted_scopes()
                .await
                .into_iter()
                .map(Scope::new)
                .collect();
            if !scopes.is_empty() {
                tokens.set_scopes(Some(scopes));
            }
        }
        let bearer = Bearer::from_tokens(&tokens)?;
        let mut updated = document.clone();
        let mut registration: BTreeMap<String, Box<RawValue>> = if retain_registration {
            serde_json::from_str(
                document
                    .section(Section::ClientInfo)
                    .ok_or("Saved OAuth client is missing")?
                    .get(),
            )
            .map_err(|_| "Invalid OAuth registration document")?
        } else {
            serde_json::from_str(to_raw_value(client)?.get())?
        };
        if let Some(issuer) = protocol.issuer() {
            registration.insert("issuer".into(), to_raw_value(issuer)?);
        }
        updated.set_section(Section::ClientInfo, to_raw_value(&registration)?);
        updated.set_section(Section::Tokens, to_raw_value(&tokens)?);

        let path = self.path.clone();
        let cache = self.cached.clone();
        // The owned gate moves with the disk write. If the caller cancels,
        // another request still cannot observe a partly published credential.
        spawn_blocking(move || {
            storage::save(&path, &updated)?;
            *document = updated;
            let value = bearer.value.clone();
            *cache.write().unwrap() = Some(bearer);
            Ok(value)
        })
        .await?
    }
}

impl GatewayAuthorizer for GatewayOAuth {
    fn cached_bearer(&self) -> Option<String> {
        self.cached
            .read()
            .unwrap()
            .as_ref()
            .map(|bearer| bearer.value.clone())
    }

    fn supports_scope_upgrade(&self, challenges: &[HeaderValue]) -> bool {
        self.challenge(challenges, true).is_some()
    }

    fn authorize<'a>(
        &'a self,
        status: StatusCode,
        sent_bearer: Option<&'a str>,
        challenges: &'a [HeaderValue],
    ) -> BoxFuture<'a, Result<Option<String>>> {
        Box::pin(self.authorize_request(status, sent_bearer, challenges))
    }
}

impl Bearer {
    fn from_tokens(tokens: &OAuthTokenResponse) -> Result<Self> {
        Ok(Self {
            value: valid_bearer(tokens)
                .ok_or("OAuth token file has no valid bearer")?
                .into(),
            scopes: tokens
                .scopes()
                .iter()
                .flat_map(|scopes| scopes.iter())
                .map(|scope| scope.as_str().to_string())
                .collect(),
        })
    }
}

fn section<T: DeserializeOwned>(document: &TokenDocument, key: Section) -> Result<Option<T>> {
    let Some(raw) = document.section(key) else {
        return Ok(None);
    };
    let value = raw.get().trim();
    if value
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .is_some_and(|value| value.trim().is_empty())
    {
        return Ok(None);
    }
    serde_json::from_str(value).map_err(|_| "Invalid OAuth credential section".into())
}

fn scopes_cover(granted: &[String], challenge: Option<&str>) -> bool {
    challenge.is_some_and(|challenge| {
        let mut required = challenge.split_ascii_whitespace().peekable();
        required.peek().is_some() && required.all(|scope| granted.iter().any(|item| item == scope))
    })
}

async fn wait_for_browser(
    callback: LoopbackCallback,
    state: &str,
    url: &str,
    no_browser: bool,
) -> Result<super::callback::AuthorizationCode> {
    let callback = callback.wait(state, AUTHORIZATION_TIMEOUT);
    if no_browser {
        return Ok(callback.await?);
    }
    let opener = if cfg!(target_os = "macos") {
        "/usr/bin/open"
    } else {
        "xdg-open"
    };
    let Ok(mut child) = Command::new(opener)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
    else {
        // The terminal has already received the authorization URL.
        return Ok(callback.await?);
    };
    tokio::pin!(callback);
    tokio::select! {
        result = &mut callback => {
            let _ = child.kill().await;
            Ok(result?)
        }
        _ = child.wait() => Ok(callback.await?),
    }
}
