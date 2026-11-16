//! Consumer-owned browser ports. No machine OAuth or backend transport types.
use crate::PortFuture;
use ssx_domain::browser_identity::*;
use std::collections::BTreeSet;
pub trait BrowserDirectory: Send + Sync {
    fn subject<'a>(
        &'a self,
        issuer: &'a str,
        subject: &'a str,
    ) -> PortFuture<'a, Result<Option<BrowserPrincipal>, BrowserError>>;
    fn principal<'a>(
        &'a self,
        id: &'a str,
    ) -> PortFuture<'a, Result<Option<BrowserPrincipal>, BrowserError>>;
}
pub trait BrowserIdentity: Send + Sync {
    fn begin<'a>(
        &'a self,
        return_to: &'a str,
    ) -> PortFuture<'a, Result<BrowserLogin, BrowserError>>;
    fn complete<'a>(
        &'a self,
        query: &'a str,
        transaction: &'a BrowserSecret,
        old: Option<&'a BrowserSecret>,
    ) -> PortFuture<'a, Result<BrowserAuthenticated, BrowserError>>;
    fn resolve<'a>(
        &'a self,
        handle: &'a BrowserSecret,
        actions: &'a BTreeSet<String>,
    ) -> PortFuture<'a, Result<BrowserSession, BrowserError>>;
    fn tenants<'a>(
        &'a self,
        session: &'a BrowserSession,
    ) -> PortFuture<'a, Result<Vec<BrowserMembership>, BrowserError>>;
    fn rotate<'a>(
        &'a self,
        handle: &'a BrowserSecret,
        session: &'a BrowserSession,
        tenant: &'a str,
    ) -> PortFuture<'a, Result<BrowserAuthenticated, BrowserError>>;
    fn revoke<'a>(&'a self, handle: &'a BrowserSecret) -> PortFuture<'a, Result<(), BrowserError>>;
    fn claim_mutation<'a>(
        &'a self,
        context: &'a BrowserContext,
        action: &'a str,
        idempotency: &'a str,
        digest: &'a str,
    ) -> PortFuture<'a, Result<BrowserAdmission, BrowserError>>;
    fn finish_mutation<'a>(
        &'a self,
        context: &'a BrowserContext,
        action: &'a str,
        idempotency: &'a str,
        digest: &'a str,
        claim: &'a BrowserSecret,
    ) -> PortFuture<'a, Result<(), BrowserError>>;
    fn csrf(&self, expected: &BrowserSecret, submitted: &str) -> bool;
    fn rate<'a>(
        &'a self,
        bucket: BrowserRate,
        identities: &'a [&'a str],
    ) -> PortFuture<'a, Result<bool, BrowserError>>;
    fn audit<'a>(
        &'a self,
        request_id: &'a str,
        action: BrowserAuditAction,
        outcome: BrowserAuditOutcome,
        reason: BrowserAuditReason,
        context: Option<&'a BrowserContext>,
        object: Option<(&'a str, &'a str)>,
    ) -> PortFuture<'a, Result<(), BrowserError>>;
}
/// W10 implements actual workflow operations. This seam never supplies tenant grants.
pub trait BrowserWorkflow: Send + Sync {
    fn read<'a>(
        &'a self,
        context: &'a BrowserContext,
        kind: Option<&'a str>,
        id: Option<&'a str>,
    ) -> PortFuture<'a, Result<String, BrowserError>>;
    fn mutate<'a>(
        &'a self,
        context: &'a BrowserContext,
        command: BrowserMutation<'a>,
    ) -> PortFuture<'a, Result<(), BrowserError>>;
}

pub struct BrowserMutation<'a> {
    pub kind: &'a str,
    pub id: &'a str,
    pub action: &'a str,
    pub revision: u64,
    pub idempotency: &'a str,
    pub fields: &'a [(String, String)],
}

impl std::fmt::Debug for BrowserMutation<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BrowserMutation([REDACTED])")
    }
}
