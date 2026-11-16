//! T04 authority. No local fallback, implicit retries or endpoint validation.
use super::{Failure, Read, Valkey};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use ssx_domain::oauth::*;
use std::{collections::BTreeSet, sync::Arc};
const TRANSITION: &str = include_str!("oauth/transition.lua");
const SCHEMA: u8 = 1;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record<T> {
    schema: u8,
    kind: String,
    expires_at: u64,
    data: T,
}
struct Snapshot<T> {
    raw: String,
    record: Record<T>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredClient {
    client_id: String,
    client_secret: Option<String>,
    metadata: serde_json::Map<String, serde_json::Value>,
}
impl From<ClientRegistration> for StoredClient {
    fn from(v: ClientRegistration) -> Self {
        Self {
            client_id: v.client_id,
            client_secret: v.client_secret.map(|s| s.expose().into()),
            metadata: v.metadata,
        }
    }
}
impl StoredClient {
    fn validate(&self) -> bool {
        valid_text(&self.client_id)
            && self.metadata.len() <= 64
            && !self.metadata.contains_key("client_id")
            && !self.metadata.contains_key("client_secret")
            && self
                .client_secret
                .as_ref()
                .is_none_or(|v| !v.is_empty() && v.len() <= 4096)
    }
    fn public(self) -> Result<ClientRegistration, OAuthError> {
        Ok(ClientRegistration {
            client_id: self.client_id,
            client_secret: self
                .client_secret
                .map(Secret::new)
                .transpose()
                .map_err(|_| OAuthError::Malformed)?,
            metadata: self.metadata,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Linked {
    grant: TokenGrant,
    counterpart: String,
    counterpart_expires_at: u64,
}
struct Change {
    key: String,
    before: String,
    after: String,
    expires: u64,
    authorize: u64,
}
impl Change {
    fn retain<T>(key: String, v: &Snapshot<T>, authorize: bool) -> Self {
        Self {
            key,
            before: v.raw.clone(),
            after: v.raw.clone(),
            expires: v.record.expires_at,
            authorize: if authorize { v.record.expires_at } else { 0 },
        }
    }
    fn delete<T>(key: String, v: &Snapshot<T>, authorize: bool) -> Self {
        Self {
            key,
            before: v.raw.clone(),
            after: String::new(),
            expires: 0,
            authorize: if authorize { v.record.expires_at } else { 0 },
        }
    }
    fn absent(key: String) -> Self {
        Self {
            key,
            before: String::new(),
            after: String::new(),
            expires: 0,
            authorize: 0,
        }
    }
}
#[derive(Clone)]
pub struct OAuthStore {
    store: Arc<Valkey>,
    prefix: String,
    ttls: OAuthTtls,
}
fn error(e: Failure) -> OAuthError {
    match e {
        Failure::Closed => OAuthError::Closed,
        Failure::Unavailable => OAuthError::Unavailable,
        Failure::CompletionUnknown | Failure::ScriptErrorMayHaveWritten => {
            OAuthError::CompletionUnknown
        }
        Failure::Malformed => OAuthError::Malformed,
        Failure::InvalidInput => OAuthError::InvalidInput,
    }
}
fn valid_text(v: &str) -> bool {
    !v.is_empty() && v.len() <= 4096 && !v.chars().any(char::is_control)
}
fn validate_grant(v: &TokenGrant) -> Result<(), OAuthError> {
    if !valid_text(&v.client_id)
        || v.scopes.len() > 128
        || v.scopes.iter().any(|s| !valid_text(s))
        || v.resource
            .as_ref()
            .is_some_and(|s| s.len() > 4096 || s.chars().any(char::is_control))
        || v.subject
            .as_ref()
            .is_some_and(|s| s.len() > 4096 || s.chars().any(char::is_control))
    {
        Err(OAuthError::InvalidInput)
    } else {
        Ok(())
    }
}
fn validate_code(v: &CodeGrant) -> Result<(), OAuthError> {
    validate_grant(&TokenGrant::from(v))?;
    if !valid_text(&v.code_challenge) || !valid_text(&v.redirect_uri) {
        return Err(OAuthError::InvalidInput);
    }
    Ok(())
}
fn mint() -> Result<Secret, OAuthError> {
    let mut b = [0u8; 40];
    rustls::crypto::aws_lc_rs::default_provider()
        .secure_random
        .fill(&mut b)
        .map_err(|_| OAuthError::Unavailable)?;
    Secret::new(b.iter().map(|b| format!("{b:02x}")).collect())
        .map_err(|_| OAuthError::InvalidInput)
}
impl OAuthStore {
    /// Scope is a deployment authority identifier, not tenant membership. Sharing it
    /// shares machine OAuth authority; legacy Python keys are deliberately disjoint.
    pub fn new(store: Arc<Valkey>, authority: &str, ttls: OAuthTtls) -> Result<Self, OAuthError> {
        if !valid_text(authority) {
            return Err(OAuthError::InvalidInput);
        };
        Ok(Self {
            store,
            prefix: format!("mcp:oauth:v1:{{{}}}", fingerprint(authority.as_bytes())),
            ttls: ttls.validate().map_err(|_| OAuthError::InvalidInput)?,
        })
    }
    fn key(&self, kind: &str, id: &str) -> String {
        format!("{}:{kind}:{id}", self.prefix)
    }
    fn client_key(&self, id: &str) -> String {
        self.key("client", &fingerprint(id.as_bytes()))
    }
    async fn now(&self) -> Result<u64, OAuthError> {
        Ok(self.store.quota_time().await.map_err(error)?.milliseconds())
    }
    async fn read<T: DeserializeOwned>(
        &self,
        key: &str,
        kind: &str,
    ) -> Result<Snapshot<T>, OAuthError> {
        let raw = match self.store.get(key).await {
            Read::Found(v) => v,
            Read::Missing => return Err(OAuthError::Denied(Denial::Missing)),
            Read::Unavailable => return Err(OAuthError::Unavailable),
            Read::Closed => return Err(OAuthError::Closed),
            Read::Malformed => return Err(OAuthError::Malformed),
        };
        if raw.len() > 65536 {
            return Err(OAuthError::Malformed);
        };
        let raw = String::from_utf8(raw).map_err(|_| OAuthError::Malformed)?;
        let record: Record<T> = serde_json::from_str(&raw).map_err(|_| OAuthError::Malformed)?;
        if record.schema != SCHEMA
            || record.kind != kind
            || record.expires_at > 9_007_199_254_740_991
        {
            return Err(OAuthError::Malformed);
        };
        if record.expires_at <= self.now().await? {
            return Err(OAuthError::Denied(Denial::Expired));
        };
        Ok(Snapshot { raw, record })
    }
    fn create<T: Serialize>(
        &self,
        key: String,
        kind: &str,
        data: T,
        now: u64,
        ttl: u64,
    ) -> Result<Change, OAuthError> {
        let expires = now
            .checked_add(ttl * 1000)
            .ok_or(OAuthError::InvalidInput)?;
        let after = serde_json::to_string(&Record {
            schema: SCHEMA,
            kind: kind.into(),
            expires_at: expires,
            data,
        })
        .map_err(|_| OAuthError::InvalidInput)?;
        if after.len() > 65536 {
            return Err(OAuthError::InvalidInput);
        };
        Ok(Change {
            key,
            before: String::new(),
            after,
            expires,
            authorize: 0,
        })
    }
    async fn commit(&self, changes: Vec<Change>) -> Result<(), OAuthError> {
        let keys: Vec<_> = changes.iter().map(|v| &v.key).collect();
        if keys.iter().collect::<BTreeSet<_>>().len() != keys.len() {
            return Err(OAuthError::InvalidInput);
        };
        let before: Vec<_> = changes.iter().map(|v| &v.before).collect();
        let after: Vec<_> = changes.iter().map(|v| &v.after).collect();
        let expiry: Vec<_> = changes.iter().map(|v| v.expires).collect();
        let auth: Vec<_> = changes.iter().map(|v| v.authorize).collect();
        let mut cmd = redis::cmd("EVAL");
        cmd.arg(TRANSITION).arg(keys.len()).arg(keys);
        for arg in [
            serde_json::to_string(&before),
            serde_json::to_string(&after),
            serde_json::to_string(&expiry),
            serde_json::to_string(&auth),
        ] {
            cmd.arg(arg.map_err(|_| OAuthError::InvalidInput)?);
        }
        let result: i64 =
            redis::from_redis_value(self.store.execute(cmd, true).await.map_err(error)?)
                .map_err(|_| OAuthError::Malformed)?;
        match result {
            1 => Ok(()),
            -1 => Err(OAuthError::Denied(Denial::Conflict)),
            -2 => Err(OAuthError::Denied(Denial::Expired)),
            _ => Err(OAuthError::Malformed),
        }
    }
    async fn client(&self, id: &str) -> Result<Snapshot<StoredClient>, OAuthError> {
        if id.len() > 4096 || id.chars().any(char::is_control) {
            return Err(OAuthError::InvalidInput);
        };
        let v = self
            .read::<StoredClient>(&self.client_key(id), "client")
            .await?;
        if v.record.data.client_id != id || !v.record.data.validate() {
            return Err(OAuthError::Malformed);
        };
        Ok(v)
    }
    pub async fn register_client(&self, client: ClientRegistration) -> Result<(), OAuthError> {
        let stored = StoredClient::from(client);
        if !stored.validate() {
            return Err(OAuthError::InvalidInput);
        }
        let change = self.create(
            self.client_key(&stored.client_id),
            "client",
            &stored,
            self.now().await?,
            self.ttls.client,
        )?;
        self.commit(vec![change]).await
    }

    pub async fn get_client(&self, id: &str) -> Result<ClientRegistration, OAuthError> {
        self.client(id).await?.record.data.public()
    }
    pub async fn create_code(&self, grant: CodeGrant) -> Result<IssuedCode, OAuthError> {
        validate_code(&grant)?;
        let client = self.client(&grant.client_id).await?;
        if !client
            .record
            .data
            .metadata
            .get("redirect_uris")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|uris| {
                uris.iter()
                    .any(|uri| uri.as_str() == Some(grant.redirect_uri.as_str()))
            })
        {
            return Err(OAuthError::Denied(Denial::Binding));
        };
        let code = mint()?;
        let new = self.create(
            self.key("code", &code.fingerprint()),
            "code",
            grant.clone(),
            self.now().await?,
            self.ttls.code,
        )?;
        let expires_at = new.expires;
        self.commit(vec![
            Change::retain(self.client_key(&grant.client_id), &client, true),
            new,
        ])
        .await?;
        Ok(IssuedCode {
            code,
            grant,
            expires_at,
        })
    }
    pub async fn load_code(&self, code: &Secret) -> Result<IssuedCode, OAuthError> {
        let v = self
            .read::<CodeGrant>(&self.key("code", &code.fingerprint()), "code")
            .await?;
        validate_code(&v.record.data).map_err(|_| OAuthError::Malformed)?;
        Ok(IssuedCode {
            code: code.clone(),
            grant: v.record.data,
            expires_at: v.record.expires_at,
        })
    }
    fn pair(&self, grant: TokenGrant, now: u64) -> Result<(TokenPair, Vec<Change>), OAuthError> {
        let access = mint()?;
        let refresh = mint()?;
        let access_expires = now + self.ttls.access * 1000;
        let refresh_expires = now + self.ttls.refresh * 1000;
        let a = self.create(
            self.key("access", &access.fingerprint()),
            "access",
            Linked {
                grant: grant.clone(),
                counterpart: refresh.fingerprint(),
                counterpart_expires_at: refresh_expires,
            },
            now,
            self.ttls.access,
        )?;
        let r = self.create(
            self.key("refresh", &refresh.fingerprint()),
            "refresh",
            Linked {
                grant: grant.clone(),
                counterpart: access.fingerprint(),
                counterpart_expires_at: access_expires,
            },
            now,
            self.ttls.refresh,
        )?;
        Ok((
            TokenPair {
                access_token: access,
                refresh_token: refresh,
                grant,
                expires_in: self.ttls.access,
            },
            vec![a, r],
        ))
    }
    pub async fn redeem_code(
        &self,
        code: &Secret,
        expected: &CodeGrant,
    ) -> Result<TokenPair, OAuthError> {
        let key = self.key("code", &code.fingerprint());
        let current = self.read::<CodeGrant>(&key, "code").await?;
        validate_code(&current.record.data).map_err(|_| OAuthError::Malformed)?;
        if &current.record.data != expected {
            return Err(OAuthError::Denied(Denial::Binding));
        };
        validate_grant(&TokenGrant::from(expected))?;
        let client = self.client(&expected.client_id).await?;
        let (pair, mut changes) = self.pair(TokenGrant::from(expected), self.now().await?)?;
        changes.push(Change::delete(key, &current, true));
        changes.push(Change::retain(
            self.client_key(&expected.client_id),
            &client,
            true,
        ));
        self.commit(changes).await?;
        Ok(pair)
    }
    async fn linked(
        &self,
        kind: TokenKind,
        token: &Secret,
    ) -> Result<(Snapshot<Linked>, Vec<Change>), OAuthError> {
        let (name, other) = match kind {
            TokenKind::Access => ("access", "refresh"),
            TokenKind::Refresh => ("refresh", "access"),
        };
        let id = token.fingerprint();
        let key = self.key(name, &id);
        let current = self.read::<Linked>(&key, name).await?;
        validate_grant(&current.record.data.grant).map_err(|_| OAuthError::Malformed)?;
        let linked = &current.record.data;
        if linked.counterpart.len() != 64
            || !linked.counterpart.bytes().all(|v| v.is_ascii_hexdigit())
        {
            return Err(OAuthError::Malformed);
        };
        let other_key = self.key(other, &linked.counterpart);
        let mut changes = vec![Change::delete(key, &current, true)];
        match self.read::<Linked>(&other_key, other).await {
            Ok(peer) => {
                if peer.record.data.counterpart != id
                    || peer.record.data.grant != linked.grant
                    || peer.record.expires_at != linked.counterpart_expires_at
                    || peer.record.data.counterpart_expires_at != current.record.expires_at
                {
                    return Err(OAuthError::Malformed);
                };
                changes.push(Change::delete(other_key, &peer, false));
            }
            Err(OAuthError::Denied(Denial::Missing | Denial::Expired)) => {
                if linked.counterpart_expires_at > self.now().await? {
                    return Err(OAuthError::Malformed);
                };
                changes.push(Change::absent(other_key));
            }
            Err(e) => return Err(e),
        };
        Ok((current, changes))
    }
    pub async fn load_token(
        &self,
        kind: TokenKind,
        token: &Secret,
    ) -> Result<LoadedGrant, OAuthError> {
        let (record, _) = self.linked(kind, token).await?;
        Ok(LoadedGrant {
            grant: record.record.data.grant,
            expires_at: record.record.expires_at,
        })
    }
    pub async fn rotate_refresh(
        &self,
        token: &Secret,
        expected: &TokenGrant,
        scopes: &[String],
    ) -> Result<TokenPair, OAuthError> {
        let (current, mut changes) = self.linked(TokenKind::Refresh, token).await?;
        if &current.record.data.grant != expected {
            return Err(OAuthError::Denied(Denial::Binding));
        };
        if scopes.iter().any(|s| !expected.scopes.contains(s)) {
            return Err(OAuthError::Denied(Denial::Scope));
        };
        let mut grant = expected.clone();
        if !scopes.is_empty() {
            grant.scopes = scopes.to_vec()
        };
        validate_grant(&grant)?;
        let client = self.client(&grant.client_id).await?;
        let (pair, new) = self.pair(grant, self.now().await?)?;
        changes.extend(new);
        changes.push(Change::retain(
            self.client_key(&expected.client_id),
            &client,
            true,
        ));
        self.commit(changes).await?;
        Ok(pair)
    }
    pub async fn revoke(
        &self,
        kind: TokenKind,
        token: &Secret,
        client_id: &str,
    ) -> Result<(), OAuthError> {
        let (current, changes) = self.linked(kind, token).await?;
        if current.record.data.grant.client_id != client_id {
            return Err(OAuthError::Denied(Denial::Binding));
        };
        self.commit(changes).await
    }
}
impl ssx_application::oauth::OAuthState for OAuthStore {
    fn now_ms(&self) -> ssx_application::PortFuture<'_, Result<u64, OAuthError>> {
        Box::pin(self.now())
    }
    fn register_client(
        &self,
        v: ClientRegistration,
    ) -> ssx_application::PortFuture<'_, Result<(), OAuthError>> {
        Box::pin(self.register_client(v))
    }
    fn get_client<'a>(
        &'a self,
        id: &'a str,
    ) -> ssx_application::PortFuture<'a, Result<ClientRegistration, OAuthError>> {
        Box::pin(self.get_client(id))
    }
    fn create_code(
        &self,
        v: CodeGrant,
    ) -> ssx_application::PortFuture<'_, Result<IssuedCode, OAuthError>> {
        Box::pin(self.create_code(v))
    }
    fn load_code<'a>(
        &'a self,
        v: &'a Secret,
    ) -> ssx_application::PortFuture<'a, Result<IssuedCode, OAuthError>> {
        Box::pin(self.load_code(v))
    }
    fn redeem_code<'a>(
        &'a self,
        v: &'a Secret,
        b: &'a CodeGrant,
    ) -> ssx_application::PortFuture<'a, Result<TokenPair, OAuthError>> {
        Box::pin(self.redeem_code(v, b))
    }
    fn load_token<'a>(
        &'a self,
        k: TokenKind,
        v: &'a Secret,
    ) -> ssx_application::PortFuture<'a, Result<LoadedGrant, OAuthError>> {
        Box::pin(self.load_token(k, v))
    }
    fn rotate_refresh<'a>(
        &'a self,
        v: &'a Secret,
        b: &'a TokenGrant,
        s: &'a [String],
    ) -> ssx_application::PortFuture<'a, Result<TokenPair, OAuthError>> {
        Box::pin(self.rotate_refresh(v, b, s))
    }
    fn revoke<'a>(
        &'a self,
        k: TokenKind,
        v: &'a Secret,
        c: &'a str,
    ) -> ssx_application::PortFuture<'a, Result<(), OAuthError>> {
        Box::pin(self.revoke(k, v, c))
    }
}
