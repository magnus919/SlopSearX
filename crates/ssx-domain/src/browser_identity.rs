//! Browser identity is independent of machine OAuth and public search.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt};

/// Opaque credentials never implement serialization or revealing Debug.
/// ```compile_fail
/// use ssx_domain::browser_identity::BrowserSecret;
/// let secret=BrowserSecret::new("synthetic-only".into()).unwrap();
/// let _=serde_json::to_string(&secret);
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct BrowserSecret(String);
impl BrowserSecret {
    pub fn new(value: String) -> Result<Self, BrowserError> {
        if value.is_empty() || value.len() > 4096 {
            return Err(BrowserError::InvalidInput);
        }
        Ok(Self(value))
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for BrowserSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BrowserSecret([REDACTED])")
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserError {
    InvalidInput,
    Denied,
    Missing,
    Expired,
    Revoked,
    Conflict,
    Malformed,
    Unavailable,
    CompletionUnknown,
    Closed,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserMembership {
    pub tenant_id: String,
    pub display_name: String,
    pub grants: BTreeSet<String>,
    pub revision: u64,
    pub enabled: bool,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserPrincipal {
    pub principal_id: String,
    pub issuer: String,
    pub subject: String,
    pub memberships: Vec<BrowserMembership>,
    pub revision: u64,
    pub enabled: bool,
}
impl fmt::Debug for BrowserPrincipal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BrowserPrincipal([REDACTED])")
    }
}
pub const BROWSER_ACTIONS: &[&str] = &[
    "workflow.read",
    "workflow.research.cancel",
    "workflow.research.retry",
    "workflow.saved.control",
    "workflow.saved.ack",
    "workflow.compose",
];
pub fn stable_browser_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.trim() == value
        && !value.chars().any(char::is_control)
}
impl BrowserPrincipal {
    pub fn validate(&self) -> Result<(), BrowserError> {
        let mut tenants = BTreeSet::new();
        if !stable_browser_id(&self.principal_id)
            || !stable_browser_id(&self.subject)
            || self.issuer.len() > 4096
            || self.revision == 0
            || self.memberships.is_empty()
            || self.memberships.len() > 128
        {
            return Err(BrowserError::InvalidInput);
        }
        for m in &self.memberships {
            if !stable_browser_id(&m.tenant_id)
                || !stable_browser_id(&m.display_name)
                || m.revision == 0
                || !tenants.insert(&m.tenant_id)
                || m.grants
                    .iter()
                    .any(|g| !BROWSER_ACTIONS.contains(&g.as_str()))
            {
                return Err(BrowserError::InvalidInput);
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct BrowserContext {
    pub principal_id: String,
    pub tenant_id: String,
    pub tenant_label: String,
    pub grants: BTreeSet<String>,
    pub principal_revision: u64,
    pub membership_revision: u64,
    pub generation: u64,
    pub csrf: BrowserSecret,
}
#[derive(Debug, Clone)]
pub struct BrowserSession {
    pub principal_id: String,
    pub active_tenant: Option<String>,
    pub generation: u64,
    pub principal_revision: u64,
    pub membership_revision: u64,
    pub created_ms: u64,
    pub idle_expires_ms: u64,
    pub absolute_expires_ms: u64,
    pub tenant_csrf: BrowserSecret,
    pub context: Option<BrowserContext>,
}
pub struct BrowserLogin {
    pub location: String,
    pub transaction: BrowserSecret,
}
#[derive(Debug)]
pub struct BrowserAuthenticated {
    pub handle: BrowserSecret,
    pub session: BrowserSession,
    pub return_to: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserAuditAction {
    Login,
    TenantSwitch,
    Logout,
    Read,
    ResearchCancel,
    ResearchRetry,
    SavedControl,
    SavedAck,
    Compose,
}
impl BrowserAuditAction {
    pub fn token(self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::TenantSwitch => "tenant_switch",
            Self::Logout => "logout",
            Self::Read => "workflow.read",
            Self::ResearchCancel => "workflow.research.cancel",
            Self::ResearchRetry => "workflow.research.retry",
            Self::SavedControl => "workflow.saved.control",
            Self::SavedAck => "workflow.saved.ack",
            Self::Compose => "workflow.compose",
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub enum BrowserAuditOutcome {
    Allowed,
    Denied,
    Succeeded,
    Conflict,
    Failed,
}
impl BrowserAuditOutcome {
    pub fn token(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::Denied => "denied",
            Self::Succeeded => "succeeded",
            Self::Conflict => "conflict",
            Self::Failed => "failed",
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub enum BrowserAuditReason {
    Ok,
    AuthenticationRequired,
    AuthorizationDenied,
    CsrfDenied,
    ObjectUnavailable,
    RevisionConflict,
    Duplicate,
    StoreUnavailable,
    CallbackRejected,
}
impl BrowserAuditReason {
    pub fn token(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::AuthenticationRequired => "authentication_required",
            Self::AuthorizationDenied => "authorization_denied",
            Self::CsrfDenied => "csrf_denied",
            Self::ObjectUnavailable => "object_unavailable",
            Self::RevisionConflict => "revision_conflict",
            Self::Duplicate => "duplicate",
            Self::StoreUnavailable => "store_unavailable",
            Self::CallbackRejected => "callback_rejected",
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub enum BrowserRate {
    Login,
    Callback,
    Read,
    Mutation,
    TenantSwitch,
    Logout,
}
impl BrowserRate {
    pub fn settings(self) -> (&'static str, u64, u64) {
        match self {
            Self::Login => ("login", 10, 60),
            Self::Callback => ("callback", 20, 600),
            Self::Read => ("read", 120, 60),
            Self::Mutation => ("mutation", 30, 60),
            Self::TenantSwitch => ("tenant_switch", 30, 60),
            Self::Logout => ("logout", 30, 60),
        }
    }
}
/// Harden source's prefix-only check against encoded backslashes and repeated decoding.
pub fn browser_return_to(value: Option<&str>) -> String {
    let v = value.unwrap_or("");
    if v.len() > 4096
        || !(v == "/workflows" || v.starts_with("/workflows/"))
        || v.chars().any(|c| c.is_control() || c == '\\')
    {
        return "/workflows".into();
    }
    let lower = v.to_ascii_lowercase();
    if ["%25", "%5c", "%2f", "%00", "%0a", "%0d", "%2e"]
        .iter()
        .any(|s| lower.contains(s))
        || v.split('/').any(|s| s == "." || s == "..")
    {
        return "/workflows".into();
    }
    v.into()
}

impl fmt::Debug for BrowserLogin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BrowserLogin([REDACTED])")
    }
}
#[derive(Debug)]
pub enum BrowserAdmission {
    Replay,
    Claimed(BrowserSecret),
}
