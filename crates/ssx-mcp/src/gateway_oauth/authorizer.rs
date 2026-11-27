//! Serialize credential-changing operations while allowing nonblocking control traffic.

use super::{
    callback::LoopbackCallback,
    protocol::{self, OAuthProtocol, RegisteredClient, valid_bearer},
    storage::{self, Section, TokenDocument, Transaction},
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
use tokio::{process::Command, task::spawn_blocking};
use url::Url;

/// The existing Python gateway allows five minutes to complete a browser sign-in.
pub const AUTHORIZATION_TIMEOUT: Duration = Duration::from_secs(300);

pub struct GatewayOAuth {
    remote: Url,
    path: PathBuf,
    callback_port: u16,
    no_browser: bool,
    roots: Vec<reqwest::Certificate>,
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
        Self::open_with_roots(remote, path, callback_port, no_browser, Vec::new()).await
    }

    pub async fn open_with_roots(
        remote: &str,
        path: Option<PathBuf>,
        callback_port: u16,
        no_browser: bool,
        roots: Vec<reqwest::Certificate>,
    ) -> Result<Self> {
        let url = Url::parse(remote).map_err(|_| "Invalid OAuth remote URL")?;
        protocol::check_remote(&url)?;
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
            roots,
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

        let transaction = loop {
            let path = self.path.clone();
            if let Some(transaction) = spawn_blocking(move || storage::try_begin(&path)).await?? {
                break transaction;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        };
        let document = transaction.document();
        let client: Option<RegisteredClient> = section(document, Section::ClientInfo)?;
        let tokens: Option<OAuthTokenResponse> = section(document, Section::Tokens)?;
        let bearer = tokens.as_ref().map(Bearer::from_tokens).transpose()?;
        if let Some(bearer) = bearer.as_ref()
            && Some(bearer.value.as_str()) != sent_bearer
            && (!scope_upgrade || scopes_cover(&bearer.scopes, challenged))
        {
            let value = bearer.value.clone();
            *self.cached.write().unwrap() = Some(Bearer {
                value: value.clone(),
                scopes: bearer.scopes.clone(),
            });
            return Ok(Some(value));
        }
        let granted = bearer.map_or_else(Vec::new, |bearer| bearer.scopes);
        let mut protocol = OAuthProtocol::discover(
            &self.remote,
            challenge.as_ref().map(|(header, _)| *header),
            &self.roots,
        )
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
                    .persist(transaction, client, true, &protocol, tokens)
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
        self.persist(transaction, &client, retain_registration, &protocol, tokens)
            .await
            .map(Some)
    }

    async fn persist(
        &self,
        transaction: Transaction,
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
        let document = transaction.document();
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

        let cache = self.cached.clone();
        // Keep the file lease through publication, even if the caller cancels.
        spawn_blocking(move || {
            transaction.save(&updated)?;
            let value = bearer.value.clone();
            *cache.write().unwrap() = Some(bearer);
            drop(transaction);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_a_cleartext_remote_before_opening_credentials() {
        let result =
            GatewayOAuth::open("http://resource.test/mcp", Some(PathBuf::new()), 0, true).await;
        assert!(matches!(
            result,
            Err(error) if error.downcast_ref() == Some(&protocol::ProtocolError::Endpoint)
        ));
    }
}
