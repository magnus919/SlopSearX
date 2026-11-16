use super::{equals, mac, oidc, random};
use crate::valkey::browser_identity::BrowserStore;
use aws_lc_rs::digest;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use ssx_application::{
    PortFuture,
    browser_identity::{BrowserDirectory, BrowserIdentity},
};
use ssx_domain::browser_identity::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};
use tokio::sync::Mutex;
use url::Url;
#[derive(Clone, Debug)]
pub struct BrowserKeys {
    current: BrowserSecret,
    previous: Option<(BrowserSecret, u64)>,
}
impl BrowserKeys {
    /// The previous key has a 60–600 second drain, initialized from Valkey TIME.
    pub fn new(
        current: BrowserSecret,
        previous: Option<(BrowserSecret, u64)>,
    ) -> Result<Self, BrowserError> {
        if current.expose().len() < 32
            || previous
                .as_ref()
                .is_some_and(|(v, drain)| v.expose().len() < 32 || !(60..=600).contains(drain))
        {
            return Err(BrowserError::InvalidInput);
        }
        Ok(Self { current, previous })
    }
    fn candidates(&self, prefix: &str, handle: &BrowserSecret, now: u64) -> Vec<String> {
        let mut keys = vec![format!(
            "{prefix}:{}",
            mac(&self.current, handle.expose().as_bytes())
        )];
        if let Some((old, until)) = &self.previous
            && now < *until
        {
            keys.push(format!("{prefix}:{}", mac(old, handle.expose().as_bytes())))
        }
        keys
    }
}
pub struct StaticBrowserDirectory {
    ids: BTreeMap<String, BrowserPrincipal>,
    subjects: BTreeMap<(String, String), String>,
}
impl StaticBrowserDirectory {
    pub fn new(principals: Vec<BrowserPrincipal>) -> Result<Self, BrowserError> {
        if principals.is_empty() || principals.len() > 1024 {
            return Err(BrowserError::InvalidInput);
        }
        let mut ids = BTreeMap::new();
        let mut subjects = BTreeMap::new();
        for mut p in principals {
            p.validate()?;
            p.issuer = p.issuer.trim_end_matches('/').into();
            if subjects
                .insert(
                    (p.issuer.clone(), p.subject.clone()),
                    p.principal_id.clone(),
                )
                .is_some()
                || ids.insert(p.principal_id.clone(), p).is_some()
            {
                return Err(BrowserError::InvalidInput);
            }
        }
        Ok(Self { ids, subjects })
    }
}
impl BrowserDirectory for StaticBrowserDirectory {
    fn subject<'a>(
        &'a self,
        issuer: &'a str,
        subject: &'a str,
    ) -> PortFuture<'a, Result<Option<BrowserPrincipal>, BrowserError>> {
        Box::pin(async move {
            Ok(self
                .subjects
                .get(&(issuer.trim_end_matches('/').into(), subject.into()))
                .and_then(|id| self.ids.get(id))
                .cloned())
        })
    }
    fn principal<'a>(
        &'a self,
        id: &'a str,
    ) -> PortFuture<'a, Result<Option<BrowserPrincipal>, BrowserError>> {
        Box::pin(async move { Ok(self.ids.get(id).cloned()) })
    }
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Transaction {
    schema: u8,
    state: String,
    nonce: String,
    verifier: String,
    return_to: String,
    created_ms: u64,
    expires_ms: u64,
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Session {
    schema: u8,
    principal_id: String,
    issuer: String,
    active_tenant: Option<String>,
    authenticated_ms: u64,
    created_ms: u64,
    last_seen_ms: u64,
    idle_expires_ms: u64,
    absolute_expires_ms: u64,
    principal_revision: u64,
    membership_revision: u64,
    csrf_secret: String,
    generation: u64,
    refresh_revision: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Admission {
    schema: u8,
    request_digest: String,
    state: String,
    claim_id: String,
    expires_ms: u64,
}
pub struct BrowserAuthorityConfig {
    pub settings: oidc::OidcSettings,
    pub keys: BrowserKeys,
    pub audit_key: BrowserSecret,
    pub roots: Vec<reqwest::Certificate>,
    pub idle_seconds: u64,
    pub absolute_seconds: u64,
}
impl BrowserAuthorityConfig {
    /// Source production defaults; callers may explicitly shorten validated TTLs.
    pub fn new(
        settings: oidc::OidcSettings,
        keys: BrowserKeys,
        audit_key: BrowserSecret,
        roots: Vec<reqwest::Certificate>,
    ) -> Self {
        Self {
            settings,
            keys,
            audit_key,
            roots,
            idle_seconds: 1800,
            absolute_seconds: 28800,
        }
    }
}
pub struct BrowserAuthority {
    store: BrowserStore,
    directory: Arc<dyn BrowserDirectory>,
    settings: oidc::OidcSettings,
    keys: BrowserKeys,
    audit_key: BrowserSecret,
    prefix: String,
    client: reqwest::Client,
    metadata: Mutex<Option<serde_json::Value>>,
    jwks: Mutex<Option<serde_json::Value>>,
    idle_ms: u64,
    absolute_ms: u64,
}
impl BrowserAuthority {
    pub async fn new(
        store: BrowserStore,
        directory: Arc<dyn BrowserDirectory>,
        options: BrowserAuthorityConfig,
    ) -> Result<Self, BrowserError> {
        let BrowserAuthorityConfig {
            settings,
            keys,
            audit_key,
            roots,
            idle_seconds,
            absolute_seconds,
        } = options;
        if !(60..=28_800).contains(&idle_seconds)
            || !(idle_seconds..=28_800).contains(&absolute_seconds)
            || audit_key.expose().len() < 32
        {
            return Err(BrowserError::InvalidInput);
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .tls_certs_merge(roots)
            .build()
            .map_err(|_| BrowserError::InvalidInput)?;
        let mut keys = keys;
        if let Some((_, until)) = &mut keys.previous {
            *until = store
                .now()
                .await?
                .checked_add(*until * 1000)
                .ok_or(BrowserError::InvalidInput)?;
        }
        let authority = format!(
            "{}\0{}\0{}",
            settings.issuer, settings.client_id, settings.callback_url
        );
        let prefix = format!(
            "portal:native:v1:{}",
            super::hex(digest::digest(&digest::SHA256, authority.as_bytes()).as_ref())
        );
        Ok(Self {
            store,
            directory,
            settings,
            keys,
            audit_key,
            prefix,
            client,
            metadata: Mutex::new(None),
            jwks: Mutex::new(None),
            idle_ms: idle_seconds * 1000,
            absolute_ms: absolute_seconds * 1000,
        })
    }
    fn admission_key(
        &self,
        c: &BrowserContext,
        action: &str,
        idem: &str,
    ) -> Result<String, BrowserError> {
        let bytes =
            serde_json::to_vec(&[c.principal_id.as_str(), c.tenant_id.as_str(), action, idem])
                .map_err(|_| BrowserError::InvalidInput)?;
        Ok(format!(
            "{}:{}",
            self.prefix("mutation"),
            mac(&self.audit_key, &bytes)
        ))
    }
    async fn revoke_observed(&self, key: &str, raw: &str, s: &Session) -> Result<(), BrowserError> {
        match self
            .store
            .change("delete", &[key], raw, s, s.absolute_expires_ms)
            .await
        {
            Ok(())
            | Err(BrowserError::Conflict | BrowserError::Missing | BrowserError::Expired) => Ok(()),
            Err(e) => Err(e),
        }
    }
    fn prefix(&self, kind: &str) -> String {
        format!("{}:{kind}", self.prefix)
    }
    async fn metadata(&self) -> Result<serde_json::Value, BrowserError> {
        let mut m = self.metadata.lock().await;
        if let Some(v) = m.as_ref() {
            return Ok(v.clone());
        }
        let v = oidc::discover(&self.client, &self.settings).await?;
        *m = Some(v.clone());
        Ok(v)
    }
    async fn signing_keys(&self, url: &str) -> Result<serde_json::Value, BrowserError> {
        let mut m = self.jwks.lock().await;
        if let Some(v) = m.as_ref() {
            return Ok(v.clone());
        }
        let v = oidc::bounded(
            self.client
                .get(url)
                .send()
                .await
                .map_err(|_| BrowserError::Unavailable)?,
        )
        .await?;
        *m = Some(v.clone());
        Ok(v)
    }
    async fn lookup<T: serde::de::DeserializeOwned>(
        &self,
        kind: &str,
        handle: &BrowserSecret,
        now: u64,
    ) -> Result<(String, T, String), BrowserError> {
        for key in self.keys.candidates(&self.prefix(kind), handle, now) {
            match self.store.read(&key).await {
                Ok((record, raw)) => return Ok((key, record, raw)),
                Err(BrowserError::Missing) => (),
                Err(e) => return Err(e),
            }
        }
        Err(BrowserError::Missing)
    }
    fn session_valid(&self, s: &Session) -> Result<(), BrowserError> {
        if s.schema != 1
            || !stable_browser_id(&s.principal_id)
            || s.issuer != self.settings.issuer
            || s.generation == 0
            || s.principal_revision == 0
            || s.csrf_secret.len() != 43
            || s.created_ms > s.last_seen_ms
            || s.created_ms > s.authenticated_ms
            || s.idle_expires_ms > s.absolute_expires_ms
            || s.absolute_expires_ms.saturating_sub(s.created_ms) > 28_800_000
            || s.active_tenant
                .as_ref()
                .is_some_and(|t| !stable_browser_id(t))
        {
            return Err(BrowserError::Malformed);
        }
        Ok(())
    }
    async fn principal(&self, s: &Session) -> Result<BrowserPrincipal, BrowserError> {
        self.session_valid(s)?;
        let p = self
            .directory
            .principal(&s.principal_id)
            .await?
            .ok_or(BrowserError::Revoked)?;
        p.validate().map_err(|_| BrowserError::Malformed)?;
        if p.principal_id != s.principal_id
            || !p.enabled
            || p.revision != s.principal_revision
            || p.issuer.trim_end_matches('/') != s.issuer
        {
            return Err(BrowserError::Revoked);
        }
        Ok(p)
    }
    fn view(
        &self,
        s: &Session,
        p: &BrowserPrincipal,
        actions: &BTreeSet<String>,
    ) -> Result<BrowserSession, BrowserError> {
        let context = if let Some(tenant) = &s.active_tenant {
            let m = p
                .memberships
                .iter()
                .find(|m| {
                    m.tenant_id == *tenant && m.enabled && m.revision == s.membership_revision
                })
                .ok_or(BrowserError::Revoked)?;
            Some(BrowserContext {
                principal_id: p.principal_id.clone(),
                tenant_id: m.tenant_id.clone(),
                tenant_label: m.display_name.clone(),
                grants: m.grants.intersection(actions).cloned().collect(),
                principal_revision: p.revision,
                membership_revision: m.revision,
                generation: s.generation,
                csrf: BrowserSecret::new(mac(
                    &BrowserSecret::new(s.csrf_secret.clone())?,
                    b"portal-workflow-action-v1",
                ))?,
            })
        } else {
            if s.membership_revision != 0 {
                return Err(BrowserError::Malformed);
            }
            None
        };
        Ok(BrowserSession {
            principal_id: s.principal_id.clone(),
            active_tenant: s.active_tenant.clone(),
            generation: s.generation,
            principal_revision: s.principal_revision,
            membership_revision: s.membership_revision,
            created_ms: s.created_ms,
            idle_expires_ms: s.idle_expires_ms,
            absolute_expires_ms: s.absolute_expires_ms,
            tenant_csrf: BrowserSecret::new(mac(
                &BrowserSecret::new(s.csrf_secret.clone())?,
                b"portal-tenant-choice-v1",
            ))?,
            context,
        })
    }
    async fn create(
        &self,
        p: BrowserPrincipal,
        old: Option<&BrowserSecret>,
        return_to: String,
    ) -> Result<BrowserAuthenticated, BrowserError> {
        let now = self.store.now().await?;
        p.validate()?;
        if !p.enabled {
            return Err(BrowserError::Denied);
        }
        let enabled: Vec<_> = p.memberships.iter().filter(|m| m.enabled).collect();
        if enabled.is_empty() {
            return Err(BrowserError::Denied);
        }
        let active = if enabled.len() == 1 {
            Some(enabled[0])
        } else {
            None
        };
        let s = Session {
            schema: 1,
            principal_id: p.principal_id.clone(),
            issuer: p.issuer.trim_end_matches('/').into(),
            active_tenant: active.map(|m| m.tenant_id.clone()),
            authenticated_ms: now,
            created_ms: now,
            last_seen_ms: now,
            idle_expires_ms: now + self.idle_ms,
            absolute_expires_ms: now + self.absolute_ms,
            principal_revision: p.revision,
            membership_revision: active.map_or(0, |m| m.revision),
            csrf_secret: random()?.expose().into(),
            generation: 1,
            refresh_revision: 0,
        };
        for _ in 0..3 {
            let handle = random()?;
            let new = self
                .keys
                .candidates(&self.prefix("session"), &handle, now)
                .remove(0);
            let oldkeys = old.map(|h| self.keys.candidates(&self.prefix("session"), h, now));
            let result = if let Some(keys) = oldkeys {
                self.store
                    .change(
                        "replace",
                        &[
                            &new,
                            &keys[0],
                            keys.last().ok_or(BrowserError::InvalidInput)?,
                        ],
                        "",
                        &s,
                        s.absolute_expires_ms,
                    )
                    .await
            } else {
                self.store
                    .change("create", &[&new], "", &s, s.absolute_expires_ms)
                    .await
            };
            match result {
                Ok(()) => {
                    return Ok(BrowserAuthenticated {
                        handle,
                        session: self.view(&s, &p, &BTreeSet::new())?,
                        return_to,
                    });
                }
                Err(BrowserError::Conflict) => continue,
                Err(e) => return Err(e),
            }
        }
        Err(BrowserError::Conflict)
    }
}
impl BrowserIdentity for BrowserAuthority {
    fn begin<'a>(
        &'a self,
        return_to: &'a str,
    ) -> PortFuture<'a, Result<BrowserLogin, BrowserError>> {
        Box::pin(async move {
            let m = self.metadata().await?;
            let now = self.store.now().await?;
            let state = random()?;
            let nonce = random()?;
            let verifier = random()?;
            let t = Transaction {
                schema: 1,
                state: state.expose().into(),
                nonce: nonce.expose().into(),
                verifier: verifier.expose().into(),
                return_to: browser_return_to(Some(return_to)),
                created_ms: now,
                expires_ms: now + 600_000,
            };
            for _ in 0..3 {
                let handle = random()?;
                let key = self
                    .keys
                    .candidates(&self.prefix("oidc"), &handle, now)
                    .remove(0);
                match self
                    .store
                    .change("create", &[&key], "", &t, t.expires_ms)
                    .await
                {
                    Ok(()) => {
                        let mut u = Url::parse(
                            m["authorization_endpoint"]
                                .as_str()
                                .ok_or(BrowserError::Malformed)?,
                        )
                        .map_err(|_| BrowserError::Malformed)?;
                        u.query_pairs_mut().extend_pairs([
                            ("response_type", "code"),
                            ("client_id", self.settings.client_id.as_str()),
                            ("redirect_uri", self.settings.callback_url.as_str()),
                            ("scope", "openid"),
                            ("state", state.expose()),
                            ("nonce", nonce.expose()),
                            (
                                "code_challenge",
                                URL_SAFE_NO_PAD
                                    .encode(
                                        digest::digest(
                                            &digest::SHA256,
                                            verifier.expose().as_bytes(),
                                        )
                                        .as_ref(),
                                    )
                                    .as_str(),
                            ),
                            ("code_challenge_method", "S256"),
                        ]);
                        return Ok(BrowserLogin {
                            location: u.into(),
                            transaction: handle,
                        });
                    }
                    Err(BrowserError::Conflict) => (),
                    Err(e) => return Err(e),
                }
            }
            Err(BrowserError::Conflict)
        })
    }
    fn complete<'a>(
        &'a self,
        query: &'a str,
        transaction: &'a BrowserSecret,
        old: Option<&'a BrowserSecret>,
    ) -> PortFuture<'a, Result<BrowserAuthenticated, BrowserError>> {
        Box::pin(async move {
            if query.len() > 8192 {
                return Err(BrowserError::InvalidInput);
            }
            let pairs: Vec<_> = url::form_urlencoded::parse(query.as_bytes()).collect();
            if pairs.len() > 8 {
                return Err(BrowserError::Denied);
            }
            let single = |name: &str| -> Result<String, BrowserError> {
                let v: Vec<_> = pairs.iter().filter(|(k, _)| k == name).collect();
                if v.len() != 1 || v[0].1.is_empty() {
                    return Err(BrowserError::Denied);
                }
                Ok(v[0].1.to_string())
            };
            let code = single("code")?;
            let state = single("state")?;
            let now = self.store.now().await?;
            let (key, t, raw) = self.lookup::<Transaction>("oidc", transaction, now).await?;
            if t.schema != 1
                || t.state.len() != 43
                || t.nonce.len() != 43
                || t.verifier.len() != 43
                || t.expires_ms
                    != t.created_ms
                        .checked_add(600_000)
                        .ok_or(BrowserError::Malformed)?
                || t.created_ms > now
            {
                return Err(BrowserError::Malformed);
            }
            if !equals(&state, &t.state) {
                return Err(BrowserError::Denied);
            }
            self.store
                .change("consume", &[&key], &raw, &t, t.expires_ms)
                .await?;
            let m = self.metadata().await?;
            let form_body = url::form_urlencoded::Serializer::new(String::new())
                .extend_pairs([
                    ("grant_type", "authorization_code"),
                    ("code", code.as_str()),
                    ("redirect_uri", self.settings.callback_url.as_str()),
                    ("code_verifier", t.verifier.as_str()),
                ])
                .finish();
            let result = oidc::bounded(
                self.client
                    .post(
                        m["token_endpoint"]
                            .as_str()
                            .ok_or(BrowserError::Malformed)?,
                    )
                    .basic_auth(
                        &self.settings.client_id,
                        Some(self.settings.client_secret.expose()),
                    )
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(form_body)
                    .header("accept", "application/json")
                    .send()
                    .await
                    .map_err(|_| BrowserError::CompletionUnknown)?,
            )
            .await?;
            let token = result
                .get("id_token")
                .and_then(serde_json::Value::as_str)
                .ok_or(BrowserError::Denied)?;
            let jwks = self
                .signing_keys(m["jwks_uri"].as_str().ok_or(BrowserError::Malformed)?)
                .await?;
            let (issuer, subject) = oidc::verify(
                token,
                &jwks,
                &self.settings,
                &t.nonce,
                self.store.now().await?,
            )?;
            let p = self
                .directory
                .subject(&issuer, &subject)
                .await?
                .ok_or(BrowserError::Denied)?;
            if p.issuer.trim_end_matches('/') != issuer || p.subject != subject {
                return Err(BrowserError::Denied);
            }
            self.create(p, old, browser_return_to(Some(&t.return_to)))
                .await
        })
    }
    fn resolve<'a>(
        &'a self,
        handle: &'a BrowserSecret,
        actions: &'a BTreeSet<String>,
    ) -> PortFuture<'a, Result<BrowserSession, BrowserError>> {
        Box::pin(async move {
            let now = self.store.now().await?;
            let (key, mut s, raw) = self.lookup::<Session>("session", handle, now).await?;
            self.session_valid(&s)?;
            if now >= s.idle_expires_ms.min(s.absolute_expires_ms) {
                return Err(BrowserError::Expired);
            }
            let p = match self.principal(&s).await {
                Ok(p) => p,
                Err(BrowserError::Revoked) => {
                    self.revoke_observed(&key, &raw, &s).await?;
                    return Err(BrowserError::Revoked);
                }
                Err(e) => return Err(e),
            };
            if let Err(e) = self.view(&s, &p, actions) {
                if e == BrowserError::Revoked {
                    self.revoke_observed(&key, &raw, &s).await?;
                }
                return Err(e);
            }
            if s.active_tenant.is_some() {
                s.last_seen_ms = now;
                s.idle_expires_ms = (now + self.idle_ms).min(s.absolute_expires_ms);
                s.refresh_revision = s
                    .refresh_revision
                    .checked_add(1)
                    .ok_or(BrowserError::Malformed)?;
                match self
                    .store
                    .change("refresh", &[&key], &raw, &s, s.absolute_expires_ms)
                    .await
                {
                    Ok(()) => (),
                    Err(BrowserError::Conflict) => {
                        let (latest, _) = self.store.read::<Session>(&key).await?;
                        self.session_valid(&latest)?;
                        if latest.generation != s.generation
                            || latest.principal_id != s.principal_id
                            || latest.active_tenant != s.active_tenant
                            || latest.principal_revision != s.principal_revision
                            || latest.membership_revision != s.membership_revision
                            || now >= latest.idle_expires_ms.min(latest.absolute_expires_ms)
                        {
                            return Err(BrowserError::Revoked);
                        }
                        s = latest
                    }
                    Err(e) => return Err(e),
                }
            }
            self.view(&s, &p, actions)
        })
    }
    fn tenants<'a>(
        &'a self,
        session: &'a BrowserSession,
    ) -> PortFuture<'a, Result<Vec<BrowserMembership>, BrowserError>> {
        Box::pin(async move {
            let p = self
                .directory
                .principal(&session.principal_id)
                .await?
                .ok_or(BrowserError::Revoked)?;
            p.validate()?;
            if p.principal_id != session.principal_id
                || !p.enabled
                || p.revision != session.principal_revision
            {
                return Err(BrowserError::Revoked);
            }
            Ok(p.memberships.into_iter().filter(|m| m.enabled).collect())
        })
    }
    fn rotate<'a>(
        &'a self,
        handle: &'a BrowserSecret,
        expected: &'a BrowserSession,
        tenant: &'a str,
    ) -> PortFuture<'a, Result<BrowserAuthenticated, BrowserError>> {
        Box::pin(async move {
            let now = self.store.now().await?;
            let (key, mut s, raw) = self.lookup::<Session>("session", handle, now).await?;
            let p = self.principal(&s).await?;
            let _ = self.view(&s, &p, &BTreeSet::new())?;
            if s.generation != expected.generation
                || s.active_tenant != expected.active_tenant
                || s.principal_revision != expected.principal_revision
                || s.membership_revision != expected.membership_revision
                || s.created_ms != expected.created_ms
                || s.absolute_expires_ms != expected.absolute_expires_ms
                || s.principal_id != expected.principal_id
                || now >= s.idle_expires_ms.min(s.absolute_expires_ms)
            {
                return Err(BrowserError::Revoked);
            }
            let m = p
                .memberships
                .iter()
                .find(|m| m.enabled && m.tenant_id == tenant)
                .ok_or(BrowserError::Denied)?;
            s.active_tenant = Some(tenant.into());
            s.membership_revision = m.revision;
            s.generation = s.generation.checked_add(1).ok_or(BrowserError::Malformed)?;
            s.refresh_revision = 0;
            s.csrf_secret = random()?.expose().into();
            s.last_seen_ms = now;
            s.idle_expires_ms = (now + self.idle_ms).min(s.absolute_expires_ms);
            let new = random()?;
            let newkey = self
                .keys
                .candidates(&self.prefix("session"), &new, now)
                .remove(0);
            self.store
                .change("rotate", &[&key, &newkey], &raw, &s, s.absolute_expires_ms)
                .await?;
            Ok(BrowserAuthenticated {
                handle: new,
                session: self.view(&s, &p, &BTreeSet::new())?,
                return_to: "/workflows".into(),
            })
        })
    }
    fn revoke<'a>(&'a self, handle: &'a BrowserSecret) -> PortFuture<'a, Result<(), BrowserError>> {
        Box::pin(async move {
            let now = self.store.now().await?;
            let keys = self.keys.candidates(&self.prefix("session"), handle, now);
            self.store
                .revoke(&keys.iter().map(String::as_str).collect::<Vec<_>>())
                .await
        })
    }
    fn claim_mutation<'a>(
        &'a self,
        context: &'a BrowserContext,
        action: &'a str,
        idempotency: &'a str,
        request_digest: &'a str,
    ) -> PortFuture<'a, Result<BrowserAdmission, BrowserError>> {
        Box::pin(async move {
            if !BROWSER_ACTIONS.contains(&action)
                || !(12..=128).contains(&idempotency.chars().count())
                || request_digest.len() != 64
            {
                return Err(BrowserError::InvalidInput);
            }
            let key = self.admission_key(context, action, idempotency)?;
            let now = self.store.now().await?;
            let classify = |r: Admission| -> Result<BrowserAdmission, BrowserError> {
                if r.schema != 1 || r.expires_ms <= now || r.request_digest != request_digest {
                    return Err(BrowserError::Conflict);
                }
                if r.state == "done" {
                    Ok(BrowserAdmission::Replay)
                } else {
                    Err(BrowserError::Conflict)
                }
            };
            match self.store.read::<Admission>(&key).await {
                Ok((r, _)) => return classify(r),
                Err(BrowserError::Missing) => (),
                Err(e) => return Err(e),
            }
            let claim = random()?;
            let r = Admission {
                schema: 1,
                request_digest: request_digest.into(),
                state: "pending".into(),
                claim_id: claim.expose().into(),
                expires_ms: now + 600_000,
            };
            match self
                .store
                .change("create", &[&key], "", &r, r.expires_ms)
                .await
            {
                Ok(()) => Ok(BrowserAdmission::Claimed(claim)),
                Err(BrowserError::Conflict) => {
                    let (r, _) = self.store.read::<Admission>(&key).await?;
                    classify(r)
                }
                Err(e) => Err(e),
            }
        })
    }
    fn finish_mutation<'a>(
        &'a self,
        context: &'a BrowserContext,
        action: &'a str,
        idempotency: &'a str,
        request_digest: &'a str,
        claim: &'a BrowserSecret,
    ) -> PortFuture<'a, Result<(), BrowserError>> {
        Box::pin(async move {
            let key = self.admission_key(context, action, idempotency)?;
            let (mut r, raw) = self.store.read::<Admission>(&key).await?;
            if r.schema != 1
                || r.state != "pending"
                || r.request_digest != request_digest
                || !equals(&r.claim_id, claim.expose())
            {
                return Err(BrowserError::Conflict);
            }
            r.state = "done".into();
            r.expires_ms = self.store.now().await? + 600_000;
            self.store
                .change("refresh", &[&key], &raw, &r, r.expires_ms)
                .await
        })
    }
    fn csrf(&self, expected: &BrowserSecret, submitted: &str) -> bool {
        !submitted.is_empty() && submitted.len() <= 128 && equals(expected.expose(), submitted)
    }
    fn rate<'a>(
        &'a self,
        bucket: BrowserRate,
        identities: &'a [&'a str],
    ) -> PortFuture<'a, Result<bool, BrowserError>> {
        Box::pin(async move {
            if identities.is_empty()
                || identities.len() > 4
                || identities.iter().any(|s| s.len() > 4096)
            {
                return Err(BrowserError::InvalidInput);
            }
            let (name, limit, window) = bucket.settings();
            let key = format!(
                "{}:{name}:{}",
                self.prefix("limit"),
                mac(
                    &self.audit_key,
                    &serde_json::to_vec(identities).map_err(|_| BrowserError::InvalidInput)?
                )
            );
            self.store.rate(&key, limit, window).await
        })
    }
    fn audit<'a>(
        &'a self,
        request_id: &'a str,
        action: BrowserAuditAction,
        outcome: BrowserAuditOutcome,
        reason: BrowserAuditReason,
        context: Option<&'a BrowserContext>,
        object: Option<(&'a str, &'a str)>,
    ) -> PortFuture<'a, Result<(), BrowserError>> {
        Box::pin(async move {
            let hash = |v: &str| mac(&self.audit_key, v.as_bytes())[..24].to_owned();
            let (kind, id) = object.unwrap_or(("none", "none"));
            let kind = if [
                "research",
                "dependency_dossier",
                "saved_search",
                "staged_search",
            ]
            .contains(&kind)
            {
                kind
            } else {
                "none"
            };
            let e = serde_json::json!({"request_id":request_id.chars().take(128).filter(|c|!c.is_control()).collect::<String>(),"action":action.token(),"outcome":outcome.token(),"reason":reason.token(),"principal":hash(context.map_or("none",|c|&c.principal_id)),"tenant":hash(context.map_or("none",|c|&c.tenant_id)),"session_generation":hash(&context.map_or_else(||"none".into(),|c|c.generation.to_string())),"object_kind":kind,"object":hash(id)});
            self.store
                .audit(&self.prefix("audit"), &e.to_string())
                .await
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn previous_key_deadline_is_exclusive_and_new_keys_use_only_current() {
        let secret = |c: &str| BrowserSecret::new(c.repeat(32)).unwrap();
        let mut keys = BrowserKeys::new(secret("n"), Some((secret("o"), 60))).unwrap();
        let settings = oidc::OidcSettings::new(
            "https://id.example".into(),
            "client".into(),
            secret("c"),
            "https://portal.example/auth/callback".into(),
        )
        .unwrap();
        let defaults = BrowserAuthorityConfig::new(settings, keys.clone(), secret("a"), vec![]);
        assert_eq!(
            (defaults.idle_seconds, defaults.absolute_seconds),
            (1800, 28800)
        );
        keys.previous.as_mut().unwrap().1 = 61000;
        let h = BrowserSecret::new("synthetic-handle".into()).unwrap();
        assert_eq!(keys.candidates("session", &h, 60999).len(), 2);
        assert_eq!(keys.candidates("session", &h, 61000).len(), 1);
        let current = keys.candidates("session", &h, 60000).remove(0);
        assert_eq!(
            current,
            format!("session:{}", mac(&keys.current, h.expose().as_bytes()))
        );
    }
}
