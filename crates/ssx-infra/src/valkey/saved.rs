//! Tenant-bound saved definitions and guarded schedule claims. No provider execution.
use super::{Failure, Read, TokenSource, Valkey, keys};
pub mod events;
use base64::Engine as _;
use ssx_application::saved::{PreparedDefinition, SavedPolicy, prepare_pause, prepare_update};
use ssx_domain::{
    DomainError, FiniteF64, LeaseToken, SavedSearchId, TenantId,
    saved::{SavedDefinition, validate_saved_id},
};
use std::{collections::BTreeSet, sync::Arc};
#[derive(Default)]
struct TransitionLimits {
    quota: u64,
    max_lifetime: u64,
    revision: Option<u64>,
    baseline: Option<BaselineSnapshot>,
}
struct BaselineSnapshot {
    raw: Option<Vec<u8>>,
    value: Option<ssx_json::Value>,
    expiry: f64,
}
struct TransitionOutput {
    raw: Vec<u8>,
    baseline_live: bool,
}
const TRANSITION: &str = include_str!("saved_scripts/transition.lua");
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Store(Failure),
    Invalid(DomainError),
    Missing,
    Conflict,
    QuotaExceeded,
    RepairIncomplete,
    OutboxCapacity,
    Malformed,
}
impl From<Failure> for Error {
    fn from(e: Failure) -> Self {
        Self::Store(e)
    }
}
impl From<DomainError> for Error {
    fn from(e: DomainError) -> Self {
        Self::Invalid(e)
    }
}
/// Bounded source classification, after current-definition authority and exact
/// snapshot checks. Only Found promises a valid historical observation window.
#[derive(Debug, Clone, PartialEq)]
pub enum RecentReport {
    Found(ssx_json::Value),
    Incomparable(ssx_json::Value),
    Truncated(ssx_json::Value),
}
/// Tokens and raw authority snapshots are deliberately absent from Debug output.
/// ```compile_fail
/// use ssx_infra::valkey::saved::Claim;
/// fn change(claim: &mut Claim) { claim.definition.paused = true; }
/// ```
pub struct Claim {
    definition: SavedDefinition,
    token: LeaseToken<SavedSearchId>,
    raw: Vec<u8>,
}
impl Claim {
    pub fn definition(&self) -> &SavedDefinition {
        &self.definition
    }
}
impl std::fmt::Debug for Claim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SavedClaim")
            .field("definition", &self.definition)
            .finish_non_exhaustive()
    }
}
#[derive(Clone)]
pub struct SavedStore {
    backend: Arc<Valkey>,
    tenant: TenantId,
}
impl SavedStore {
    pub fn new(backend: Arc<Valkey>, tenant: TenantId) -> Self {
        Self { backend, tenant }
    }
    pub fn for_tenant(&self, tenant: TenantId) -> Self {
        Self::new(self.backend.clone(), tenant)
    }
    pub fn tenant(&self) -> &TenantId {
        &self.tenant
    }
    fn key(&self, kind: &str, id: &str) -> String {
        keys::saved(kind, &self.tenant, Some(id)).expect("fixed saved key kind")
    }
    fn index(&self, kind: &str) -> String {
        keys::saved(kind, &self.tenant, None).expect("fixed saved index kind")
    }
    pub async fn now(&self) -> Result<f64, Error> {
        let reply = self.backend.execute(redis::cmd("TIME"), false).await?;
        let (seconds, micros): (u64, u64) =
            redis::from_redis_value(reply).map_err(|_| Error::Malformed)?;
        Ok(seconds as f64 + micros as f64 / 1_000_000.0)
    }
    async fn raw(&self, id: &SavedSearchId) -> Result<(Vec<u8>, SavedDefinition), Error> {
        validate_saved_id(id.as_str())?;
        let raw = match self.backend.get(&self.key("definition", id.as_str())).await {
            Read::Found(v) => v,
            Read::Missing => return Err(Error::Missing),
            Read::Malformed => return Err(Error::Malformed),
            Read::Unavailable => return Err(Error::Store(Failure::Unavailable)),
            Read::Closed => return Err(Error::Store(Failure::Closed)),
        };
        if raw.len() > ssx_json::MAX_JSON_BYTES {
            return Err(Error::RepairIncomplete);
        }
        let value: ssx_json::Value = ssx_json::from_slice(&raw).map_err(|_| Error::Malformed)?;
        let record = SavedDefinition::from_payload(&value).map_err(|_| Error::Malformed)?;
        record
            .validate_binding(self.tenant.as_str(), id.as_str())
            .map_err(|_| Error::Malformed)?;
        Ok((raw, record))
    }
    pub async fn load(&self, id: &SavedSearchId) -> Result<Option<SavedDefinition>, Error> {
        let (_, mut record) = match self.raw(id).await {
            Err(Error::Missing) => return Ok(None),
            other => other?,
        };
        let now = self.now().await?;
        if record.deleted || record.expires_at.get() <= now {
            return Ok(None);
        }
        match self
            .backend
            .get_object(&self.key("baseline", id.as_str()))
            .await
        {
            Read::Found(v) => {
                if ssx_domain::saved::baseline_is_live(&v, now).map_err(|_| Error::Malformed)? {
                    record.baseline = Some(v);
                }
            }
            Read::Missing => {}
            Read::Malformed => return Err(Error::Malformed),
            Read::Unavailable => return Err(Error::Store(Failure::Unavailable)),
            Read::Closed => return Err(Error::Store(Failure::Closed)),
        }
        Ok(Some(record))
    }
    async fn baseline_snapshot(&self, id: &SavedSearchId) -> Result<BaselineSnapshot, Error> {
        let raw = match self.backend.get(&self.key("baseline", id.as_str())).await {
            Read::Found(raw) => Some(raw),
            Read::Missing => None,
            Read::Malformed => return Err(Error::Malformed),
            Read::Unavailable => return Err(Error::Store(Failure::Unavailable)),
            Read::Closed => return Err(Error::Store(Failure::Closed)),
        };
        let value = raw
            .as_ref()
            .map(|raw| {
                if raw.len() > ssx_json::MAX_JSON_BYTES {
                    return Err(Error::RepairIncomplete);
                }
                ssx_json::from_slice(raw).map_err(|_| Error::Malformed)
            })
            .transpose()?;
        let expiry = value
            .as_ref()
            .map(ssx_domain::saved::baseline_expiry)
            .transpose()
            .map_err(|_| Error::Malformed)?
            .unwrap_or(0.0);
        Ok(BaselineSnapshot { raw, value, expiry })
    }
    async fn transition(
        &self,
        mode: &str,
        id: &str,
        raw: &[u8],
        next: Option<&SavedDefinition>,
        token: &str,
        limits: TransitionLimits,
    ) -> Result<Option<TransitionOutput>, Error> {
        validate_saved_id(id)?;
        // Exact Rust validation plus raw CAS is the proof boundary for paths
        // which retain original record bytes. Claim still uses source cjson.
        if matches!(mode, "update" | "delete" | "renew" | "release") {
            let expected = SavedDefinition::from_payload(
                &ssx_json::from_slice(raw).map_err(|_| Error::Malformed)?,
            )
            .map_err(|_| Error::Malformed)?;
            expected.validate_binding(self.tenant.as_str(), id)?;
            if mode == "update" {
                let revision = expected
                    .revision
                    .checked_add(1)
                    .filter(|n| *n <= i64::MAX as u64)
                    .ok_or(Error::Malformed)?;
                if next.is_none_or(|next| next.revision != revision) {
                    return Err(Error::Malformed);
                }
            }
            if mode == "delete" && limits.revision != Some(expected.revision) {
                return Err(Error::Conflict);
            }
        }
        let next_bytes = next
            .map(|d| {
                d.stored_payload()
                    .and_then(|v| ssx_json::to_vec(&v).map_err(Into::into))
            })
            .transpose()?
            .unwrap_or_default();
        if next_bytes.len() > ssx_json::MAX_JSON_BYTES {
            return Err(Error::RepairIncomplete);
        }
        let mut cmd = redis::cmd("EVAL");
        cmd.arg(TRANSITION)
            .arg(10)
            .arg(self.key("definition", id))
            .arg(self.index("definitions"))
            .arg(self.index("due"))
            .arg(keys::SAVED_TENANTS)
            .arg(self.index("definition-count"))
            .arg(self.index("recent"))
            .arg(self.key("lease", id))
            .arg(self.key("deleted", id))
            .arg(self.key("baseline", id))
            .arg(self.key("reports", id))
            .arg(mode)
            .arg(self.tenant.as_str())
            .arg(id)
            .arg(raw)
            .arg(&next_bytes)
            .arg(limits.quota)
            .arg(token)
            .arg(limits.max_lifetime)
            .arg(keys::literal_scan_prefix(&self.key("definition", "")))
            .arg(limits.revision.map(|n| n.to_string()).unwrap_or_default())
            .arg(
                limits
                    .baseline
                    .as_ref()
                    .and_then(|b| b.raw.as_deref())
                    .unwrap_or_default(),
            )
            .arg(
                if limits.baseline.as_ref().is_some_and(|b| b.raw.is_some()) {
                    "exact"
                } else {
                    "missing"
                },
            )
            .arg(limits.baseline.as_ref().map_or(0.0, |b| b.expiry));
        let mut proofs: Vec<(String, Vec<u8>)> = Vec::new();
        for attempt in 0..=1 {
            let mut request = cmd.clone();
            request.arg(attempt);
            for (key, bytes) in &proofs {
                request.arg(key).arg(bytes);
            }
            // A transport failure exits immediately. Only -6 is an explicit
            // pre-write response authorizing this single bounded proof retry.
            let (status, result): (i64, redis::Value) =
                redis::from_redis_value(self.backend.execute(request, true).await?)
                    .map_err(|_| Error::Malformed)?;
            if status == -6 {
                if attempt != 0 || !matches!(mode, "create" | "repair") {
                    return Err(Error::Conflict);
                }
                proofs = self.validate_repair_proofs(result)?;
                continue;
            }
            let bytes: Vec<u8> = redis::from_redis_value(result).map_err(|_| Error::Malformed)?;
            return match status {
                1 | 2 => Ok(Some(TransitionOutput {
                    raw: bytes,
                    baseline_live: status == 2,
                })),
                0 => Ok(None),
                -1 => Err(Error::Missing),
                -2 => Err(Error::Conflict),
                -3 => Err(Error::QuotaExceeded),
                -4 => Err(Error::Malformed),
                -5 => Err(Error::RepairIncomplete),
                _ => Err(Error::Malformed),
            };
        }
        Err(Error::Conflict)
    }
    fn validate_repair_proofs(
        &self,
        result: redis::Value,
    ) -> Result<Vec<(String, Vec<u8>)>, Error> {
        let proofs: Vec<(String, Vec<u8>)> =
            redis::from_redis_value(result).map_err(|_| Error::Malformed)?;
        if proofs.is_empty() || proofs.len() > 4096 {
            return Err(Error::RepairIncomplete);
        }
        let mut total = 0usize;
        let mut keys = BTreeSet::new();
        for (key, bytes) in &proofs {
            if bytes.len() > ssx_json::MAX_JSON_BYTES || bytes.len() > 8 * 1024 * 1024 - total {
                return Err(Error::RepairIncomplete);
            }
            total += bytes.len();
            let value = ssx_json::from_slice(bytes).map_err(|_| Error::Malformed)?;
            let record = SavedDefinition::from_payload(&value).map_err(|_| Error::Malformed)?;
            record
                .validate_binding(self.tenant.as_str(), &record.search_id)
                .map_err(|_| Error::Malformed)?;
            if *key != self.key("definition", &record.search_id) || !keys.insert(key) {
                return Err(Error::Malformed);
            }
        }
        Ok(proofs)
    }
    pub async fn create(&self, prepared: &PreparedDefinition) -> Result<bool, Error> {
        let d = prepared.definition();
        d.validate_binding(self.tenant.as_str(), &d.search_id)?;
        Ok(self
            .transition(
                "create",
                &d.search_id,
                &[],
                Some(d),
                "",
                TransitionLimits {
                    quota: prepared.quota(),
                    max_lifetime: prepared.maximum_lifetime(),
                    revision: None,
                    baseline: None,
                },
            )
            .await?
            .is_some())
    }
    pub async fn update(
        &self,
        id: &SavedSearchId,
        revision: u64,
        changes: &ssx_json::Value,
        policy: &SavedPolicy<'_>,
    ) -> Result<SavedDefinition, Error> {
        let (raw, current) = self.raw(id).await?;
        if current.revision != revision {
            return Err(Error::Conflict);
        }
        let next = prepare_update(policy, &current, changes, self.now().await?)?;
        self.transition(
            "update",
            id.as_str(),
            &raw,
            Some(&next),
            "",
            TransitionLimits::default(),
        )
        .await?;
        Ok(next)
    }
    pub async fn pause(
        &self,
        id: &SavedSearchId,
        revision: u64,
        paused: bool,
        policy: &SavedPolicy<'_>,
    ) -> Result<SavedDefinition, Error> {
        let (raw, current) = self.raw(id).await?;
        if current.revision != revision {
            return Err(Error::Conflict);
        }
        let next = prepare_pause(policy, &current, paused, self.now().await?)?;
        self.transition(
            "update",
            id.as_str(),
            &raw,
            Some(&next),
            "",
            TransitionLimits::default(),
        )
        .await?;
        Ok(next)
    }
    pub async fn delete(&self, id: &SavedSearchId, revision: u64) -> Result<(), Error> {
        let (raw, current) = self.raw(id).await?;
        if current.revision != revision {
            return Err(Error::Conflict);
        }
        self.transition(
            "delete",
            id.as_str(),
            &raw,
            None,
            "",
            TransitionLimits {
                revision: Some(revision),
                ..Default::default()
            },
        )
        .await?;
        Ok(())
    }
    pub fn generate_id() -> Result<SavedSearchId, Error> {
        let mut bytes = [0u8; 18];
        aws_lc_rs::rand::fill(&mut bytes).map_err(|_| Error::Store(Failure::Unavailable))?;
        Ok(SavedSearchId::new(format!(
            "saved-{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
        ))?)
    }
    pub async fn claim_fresh(&self, id: &SavedSearchId) -> Result<Option<Claim>, Error> {
        struct Fresh(LeaseToken<SavedSearchId>);
        impl TokenSource<SavedSearchId> for Fresh {
            fn next_token(&mut self) -> LeaseToken<SavedSearchId> {
                self.0.clone()
            }
        }
        let mut bytes = [0u8; 18];
        aws_lc_rs::rand::fill(&mut bytes).map_err(|_| Error::Store(Failure::Unavailable))?;
        let token =
            LeaseToken::new(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))?;
        self.claim(id, &mut Fresh(token)).await
    }
    pub async fn claim(
        &self,
        id: &SavedSearchId,
        tokens: &mut impl TokenSource<SavedSearchId>,
    ) -> Result<Option<Claim>, Error> {
        let (raw, mut current) = match self.raw(id).await {
            Err(Error::Missing) => return Ok(None),
            v => v?,
        };
        let baseline = self.baseline_snapshot(id).await?;
        let baseline_value = baseline.value.clone();
        let token = tokens.next_token();
        let now = self.now().await?;
        current.lease_token = Some(token.as_str().into());
        current.lease_expires_at = FiniteF64::new(now + 60.0)?;
        let raw = match self
            .transition(
                "claim",
                id.as_str(),
                &raw,
                Some(&current),
                token.as_str(),
                TransitionLimits {
                    baseline: Some(baseline),
                    ..Default::default()
                },
            )
            .await
        {
            Ok(Some(output)) => output,
            Ok(None) | Err(Error::Conflict) | Err(Error::Missing) => return Ok(None),
            Err(error) => return Err(error),
        };
        let baseline_live = raw.baseline_live;
        let raw = raw.raw;
        let mut committed = SavedDefinition::from_payload(
            &ssx_json::from_slice(&raw).map_err(|_| Error::Malformed)?,
        )
        .map_err(|_| Error::Malformed)?;
        committed.validate_binding(self.tenant.as_str(), id.as_str())?;
        if baseline_live {
            committed.baseline = baseline_value;
        }
        Ok(Some(Claim {
            definition: committed,
            token,
            raw,
        }))
    }
    /// Repair is atomic and complete within64pages/4096visited keys; otherwise
    /// no hint/count publication is made and callers receive RepairIncomplete.
    pub async fn repair_indexes(&self) -> Result<usize, Error> {
        let bytes = self
            .transition(
                "repair",
                "saved-repair-placeholder",
                &[],
                None,
                "",
                TransitionLimits::default(),
            )
            .await?
            .ok_or(Error::Malformed)?;
        String::from_utf8(bytes.raw)
            .map_err(|_| Error::Malformed)?
            .parse()
            .map_err(|_| Error::Malformed)
    }
    async fn hint_ids(&self, kind: &str, limit: usize) -> Result<Vec<String>, Error> {
        if !(1..=128).contains(&limit) {
            return Err(Error::Invalid(DomainError(
                "saved list limit outside bounds",
            )));
        }
        let mut cmd = redis::cmd("EVAL");
        cmd.arg(include_str!("saved_scripts/hints.lua"))
            .arg(1)
            .arg(self.index(kind))
            .arg(kind)
            .arg(limit);
        let (status, ids): (i64, Vec<String>) =
            redis::from_redis_value(self.backend.execute(cmd, kind == "definitions").await?)
                .map_err(|_| Error::Malformed)?;
        if status != 1 {
            return Err(Error::Malformed);
        }
        Ok(ids)
    }
    async fn prune(&self, kind: &str, id: &str, raw: Option<&[u8]>) -> Result<(), Error> {
        let mut cmd = redis::cmd("EVAL");
        cmd.arg(include_str!("saved_scripts/prune_hint.lua"))
            .arg(2)
            .arg(self.key("definition", id))
            .arg(self.index(kind))
            .arg(if raw.is_some() { "exact" } else { "missing" })
            .arg(raw.unwrap_or_default())
            .arg(id);
        let status: i64 = redis::from_redis_value(self.backend.execute(cmd, true).await?)
            .map_err(|_| Error::Malformed)?;
        if status < 0 {
            return Err(Error::Malformed);
        }
        Ok(())
    }
    async fn hint_record(&self, kind: &str, name: &str) -> Result<Option<SavedDefinition>, Error> {
        let id = SavedSearchId::new(name)?;
        let (raw, record) = match self.raw(&id).await {
            Err(Error::Missing) => {
                self.prune(kind, name, None).await?;
                return Ok(None);
            }
            v => v?,
        };
        if record.deleted
            || record.expires_at.get() <= self.now().await?
            || (kind == "due" && record.paused)
        {
            self.prune(kind, name, Some(&raw)).await?;
            return Ok(None);
        }
        self.load(&id).await
    }
    pub async fn due_ids(&self, limit: usize) -> Result<Vec<SavedSearchId>, Error> {
        let mut ids = Vec::new();
        for name in self.hint_ids("due", limit).await? {
            if self.hint_record("due", &name).await?.is_some() {
                ids.push(SavedSearchId::new(name)?);
            }
        }
        Ok(ids)
    }
    pub async fn list_definitions(&self, limit: usize) -> Result<Vec<SavedDefinition>, Error> {
        let mut records = Vec::new();
        for name in self.hint_ids("definitions", limit).await? {
            if let Some(record) = self.hint_record("definitions", &name).await? {
                records.push(record);
            }
        }
        Ok(records)
    }
    pub async fn list_recent(
        &self,
        before: Option<(f64, &str)>,
        limit: usize,
    ) -> Result<Vec<SavedDefinition>, Error> {
        if !(1..=200).contains(&limit) || before.is_some_and(|(time, _)| !time.is_finite()) {
            return Err(Error::Invalid(DomainError("invalid saved recent page")));
        }
        let pairs = self.hint_ids("recent", limit.min(128)).await?;
        if pairs.len() % 2 != 0 {
            return Err(Error::Malformed);
        }
        let mut records = Vec::new();
        for pair in pairs.as_chunks::<2>().0 {
            let score: f64 = pair[1].parse().map_err(|_| Error::Malformed)?;
            if !score.is_finite() {
                return Err(Error::Malformed);
            }
            if before.is_some_and(|(time, id)| (score, pair[0].as_str()) >= (time, id)) {
                continue;
            }
            if let Some(record) = self.hint_record("recent", &pair[0]).await?
                && records.len() < limit
            {
                records.push(record)
            }
        }
        Ok(records)
    }
    /// Continuation is explicit and nonauthoritative; it never supplies quota.
    pub async fn scan_definitions(
        &self,
        cursor: u64,
        count: usize,
    ) -> Result<ScanPage<SavedDefinition>, Error> {
        let (cursor, keys) = self
            .scan(
                cursor,
                &keys::literal_scan_prefix(&self.key("definition", "")),
                count,
            )
            .await?;
        let mut values = Vec::new();
        for key in keys {
            let prefix = self.key("definition", "");
            let Some(name) = key.strip_prefix(&prefix) else {
                return Err(Error::Malformed);
            };
            // A longer tenant sharing this prefix is not this tenant's authority.
            if name.contains(':') {
                continue;
            }
            if let Some(value) = self.load(&SavedSearchId::new(name)?).await? {
                values.push(value)
            }
        }
        Ok(ScanPage { cursor, values })
    }
    pub async fn scan_tenants(
        &self,
        cursor: u64,
        count: usize,
    ) -> Result<ScanPage<TenantId>, Error> {
        let mut prune = redis::cmd("EVAL");
        prune
            .arg(include_str!("saved_scripts/prune_tenants.lua"))
            .arg(1)
            .arg(keys::SAVED_TENANTS);
        let status: i64 = redis::from_redis_value(self.backend.execute(prune, true).await?)
            .map_err(|_| Error::Malformed)?;
        if status != 1 {
            return Err(Error::Malformed);
        }
        let (cursor, keys) = self
            .scan(cursor, "mcp:saved:v1:definition:*", count)
            .await?;
        let mut tenants = BTreeSet::new();
        for key in keys {
            let Some((tenant, id)) = key
                .strip_prefix("mcp:saved:v1:definition:")
                .and_then(|rest| rest.rsplit_once(':'))
            else {
                return Err(Error::Malformed);
            };
            let tenant = TenantId::new(tenant)?;
            let id = SavedSearchId::new(id)?;
            let child = self.for_tenant(tenant.clone());
            let (raw, record) = match child.raw(&id).await {
                Err(Error::Missing) => continue,
                v => v?,
            };
            if !record.deleted && record.expires_at.get() > self.now().await? {
                let mut cmd = redis::cmd("EVAL");
                cmd.arg(include_str!("saved_scripts/repair_tenant.lua"))
                    .arg(2)
                    .arg(key)
                    .arg(keys::SAVED_TENANTS)
                    .arg(raw)
                    .arg(record.expires_at.get())
                    .arg(tenant.as_str());
                let status: i64 = redis::from_redis_value(self.backend.execute(cmd, true).await?)
                    .map_err(|_| Error::Malformed)?;
                if status < 0 {
                    return Err(Error::Malformed);
                }
                if status == 1 {
                    tenants.insert(tenant.as_str().to_owned());
                }
            }
        }
        Ok(ScanPage {
            cursor,
            values: tenants
                .into_iter()
                .map(TenantId::new)
                .collect::<Result<_, _>>()?,
        })
    }
    async fn scan(
        &self,
        cursor: u64,
        pattern: &str,
        count: usize,
    ) -> Result<(u64, Vec<String>), Error> {
        if !(1..=128).contains(&count) {
            return Err(Error::Invalid(DomainError("invalid saved scan count")));
        }
        let mut cmd = redis::cmd("SCAN");
        cmd.arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(count);
        let reply: (u64, Vec<String>) =
            redis::from_redis_value(self.backend.execute(cmd, false).await?)
                .map_err(|_| Error::Malformed)?;
        if reply.1.len() > 4096 {
            return Err(Error::RepairIncomplete);
        }
        Ok(reply)
    }
    pub async fn renew(&self, claim: &Claim) -> Result<bool, Error> {
        self.owner_transition("renew", claim).await
    }
    pub async fn release(&self, claim: &Claim) -> Result<bool, Error> {
        self.owner_transition("release", claim).await
    }
    async fn owner_transition(&self, mode: &str, claim: &Claim) -> Result<bool, Error> {
        claim
            .definition
            .validate_binding(self.tenant.as_str(), &claim.definition.search_id)?;
        Ok(self
            .transition(
                mode,
                &claim.definition.search_id,
                &claim.raw,
                None,
                claim.token.as_str(),
                TransitionLimits::default(),
            )
            .await?
            .is_some())
    }
}

#[derive(Debug)]
pub struct ScanPage<T> {
    pub cursor: u64,
    pub values: Vec<T>,
}

impl SavedStore {
    /// The claim is the only source of write authority. Transport failure is an
    /// unknown write outcome; callers must not interpret it as rollback.
    pub async fn commit(
        &self,
        claim: &Claim,
        report: &ssx_json::Value,
        replacement: Option<&ssx_json::Value>,
        next_due: f64,
        policy_fingerprint: &str,
    ) -> Result<bool, Error> {
        Ok(self
            .commit_publication(
                claim,
                report,
                replacement,
                next_due,
                policy_fingerprint,
                None,
            )
            .await?
            .committed)
    }
    async fn commit_publication(
        &self,
        claim: &Claim,
        report: &ssx_json::Value,
        replacement: Option<&ssx_json::Value>,
        next_due: f64,
        policy_fingerprint: &str,
        event: Option<events::Publication<'_>>,
    ) -> Result<events::CommitOutcome, Error> {
        let d = claim.definition();
        d.validate_binding(self.tenant.as_str(), &d.search_id)?;
        if d.paused || d.deleted || d.policy_fingerprint != policy_fingerprint {
            return Ok(events::CommitOutcome::lost());
        }
        let run = report["run_id"].as_str().ok_or(Error::Malformed)?;
        validate_saved_id(run)?;
        let finished = report["finished_at"].as_f64().ok_or(Error::Malformed)?;
        let expires = report["expires_at"].as_f64().ok_or(Error::Malformed)?;
        let slot = d.slot(finished)?;
        // Slot is captured when claiming, so delayed execution may publish an
        // older slot. Its run identity always binds the original next_due.
        let report_slot = report["slot"].as_i64().ok_or(Error::Malformed)?;
        if report["search_id"].as_str() != Some(&d.search_id)
            || report["revision"].as_u64() != Some(d.revision)
            || run != slot.run_id
            || report_slot < 0
            || report_slot > slot.slot
            || expires > d.expires_at.get()
            || !expires.is_finite()
            || expires <= finished
            || next_due
                != d.created_at.get() + (report_slot as f64 + 1.0) * d.interval_seconds as f64
        {
            return Err(Error::Malformed);
        }
        let mut next = d.clone();
        next.latest_run_id = Some(run.into());
        next.last_slot = report_slot;
        next.next_due = FiniteF64::new(next_due)?;
        next.lease_token = None;
        next.lease_expires_at = FiniteF64::default();
        let report_bytes = ssx_json::to_vec(report).map_err(|_| Error::Malformed)?;
        let next_bytes = ssx_json::to_vec(&next.stored_payload()?).map_err(|_| Error::Malformed)?;
        let baseline_expiry = replacement
            .map(ssx_domain::saved::baseline_expiry)
            .transpose()?
            .unwrap_or(0.0);
        if let Some(value) = replacement
            && (value["window"] != d.window()? || baseline_expiry != expires)
        {
            return Err(Error::Malformed);
        }
        let baseline_bytes = replacement
            .map(ssx_json::to_vec)
            .transpose()
            .map_err(|_| Error::Malformed)?
            .unwrap_or_default();
        let mut cmd = redis::cmd("EVAL");
        let event_bytes = event.as_ref().map(|e| e.event.bytes()).transpose()?;
        cmd.arg(if event.is_some() {
            include_str!("saved_scripts/events/commit.lua")
        } else {
            include_str!("saved_scripts/commit.lua")
        })
        .arg(if event.is_some() { 7 } else { 6 })
        .arg(self.key("definition", &d.search_id))
        .arg(self.key("lease", &d.search_id))
        .arg(self.key("report", &format!("{}:{run}", d.search_id)))
        .arg(self.key("reports", &d.search_id))
        .arg(self.index("due"))
        .arg(self.key("baseline", &d.search_id));
        if event.is_some() {
            cmd.arg(self.index("events"));
        }
        cmd.arg(&claim.raw)
            .arg(claim.token.as_str())
            .arg(policy_fingerprint)
            .arg(next_bytes)
            .arg(report_bytes)
            .arg(baseline_bytes)
            .arg(d.expires_at.get())
            .arg(expires)
            .arg(baseline_expiry)
            .arg(finished)
            .arg(run)
            .arg(d.max_reports)
            .arg(self.key("report", &format!("{}:", d.search_id)))
            .arg(next_due)
            .arg(&d.search_id);
        if let Some(event) = event {
            cmd.arg(event_bytes.unwrap_or_default())
                .arg(event.limits.retention_seconds())
                .arg(event.limits.capacity());
            let (status, cursor): (i64, String) =
                redis::from_redis_value(self.backend.execute(cmd, true).await?)
                    .map_err(|_| Error::Malformed)?;
            return match status {
                1 => Ok(events::CommitOutcome {
                    committed: true,
                    cursor: Some(crate::saved_events::codec::Cursor::new(cursor)?),
                }),
                0 => Ok(events::CommitOutcome::lost()),
                -5 => Err(Error::OutboxCapacity),
                _ => Err(Error::Malformed),
            };
        }
        let status: i64 = redis::from_redis_value(self.backend.execute(cmd, true).await?)
            .map_err(|_| Error::Malformed)?;
        match status {
            1 => Ok(events::CommitOutcome {
                committed: true,
                cursor: None,
            }),
            0 => Ok(events::CommitOutcome::lost()),
            _ => Err(Error::Malformed),
        }
    }
    /// Retained evidence is still bound to the currently authorized live scope.
    /// Resolve only membership in the current operator-bounded recent list.
    /// Existing direct report readers retain their original API and behavior.
    pub async fn recent_report(
        &self,
        id: &SavedSearchId,
        run_id: &str,
        policy: &SavedPolicy<'_>,
    ) -> Result<Option<RecentReport>, Error> {
        if !policy.saved_granted {
            return Err(Error::Invalid(DomainError(
                "saved-search grant is disabled",
            )));
        }
        if validate_saved_id(id.as_str()).is_err() {
            return Ok(None);
        }
        if policy.limits.reports == 0 || policy.limits.reports > 100 {
            return Err(Error::Invalid(DomainError(
                "invalid saved report list bound",
            )));
        }
        let initial = match self.raw(id).await {
            Ok((raw, _)) => raw,
            Err(Error::Missing) => return Ok(None),
            Err(error) => return Err(error),
        };
        let Some(d) = self.load(id).await? else {
            return Ok(None);
        };
        policy
            .search
            .retained_engine_rejection(&d.engines, "engines")
            .map_err(|_| Error::Invalid(DomainError("saved-search scope rejected")))?;
        let mut cmd = redis::cmd("ZREVRANGE");
        cmd.arg(self.key("reports", id.as_str()))
            .arg(0)
            .arg(policy.limits.reports - 1);
        let members: Vec<String> = redis::from_redis_value(self.backend.execute(cmd, false).await?)
            .map_err(|_| Error::Malformed)?;
        if !members.iter().any(|r| r == run_id) || validate_saved_id(run_id).is_err() {
            return Ok(None);
        }
        let value = match self
            .backend
            .get_object(&self.key("report", &format!("{}:{run_id}", id.as_str())))
            .await
        {
            Read::Found(value) => value,
            Read::Missing => return Ok(None),
            Read::Malformed => return Err(Error::Malformed),
            Read::Unavailable => return Err(Error::Store(Failure::Unavailable)),
            Read::Closed => return Err(Error::Store(Failure::Closed)),
        };
        if value["search_id"].as_str() != Some(id.as_str())
            || value["run_id"].as_str() != Some(run_id)
        {
            return Err(Error::Malformed);
        }
        if value["expires_at"].as_f64().ok_or(Error::Malformed)? <= self.now().await? {
            return Ok(None);
        }
        if self.raw(id).await?.0 != initial {
            return Err(Error::Conflict);
        }
        policy
            .search
            .retained_engine_rejection(&d.engines, "engines")
            .map_err(|_| Error::Invalid(DomainError("saved-search scope rejected")))?;
        let reasons = value["incomparable_reasons"].as_array();
        if reasons.is_some_and(|a| {
            a.iter().any(|v| {
                matches!(
                    v.as_str(),
                    Some("observation_truncated" | "report_size_exceeded")
                )
            })
        }) {
            return Ok(Some(RecentReport::Truncated(value)));
        }
        if value["status"].as_str() == Some("incomparable")
            || reasons.is_some_and(|a| !a.is_empty())
        {
            return Ok(Some(RecentReport::Incomparable(value)));
        }
        let window = &value["observation"]["window"];
        let Some(_query) = window["query"]
            .as_str()
            .filter(|s| !s.trim_matches(ssx_json::python_whitespace).is_empty())
        else {
            return Ok(Some(RecentReport::Incomparable(value)));
        };
        let Some(engines) = window["engines"].as_array().filter(|a| !a.is_empty()) else {
            return Ok(Some(RecentReport::Incomparable(value)));
        };
        let Some(engines) = engines
            .iter()
            .map(|v| v.as_str().map(str::to_owned))
            .collect::<Option<Vec<_>>>()
        else {
            return Ok(Some(RecentReport::Incomparable(value)));
        };
        policy
            .search
            .retained_engine_rejection(&engines, "engines")
            .map_err(|_| Error::Invalid(DomainError("saved-search scope rejected")))?;
        Ok(Some(RecentReport::Found(value)))
    }
    pub async fn report(
        &self,
        id: &SavedSearchId,
        run_id: &str,
        policy: &SavedPolicy<'_>,
    ) -> Result<Option<ssx_json::Value>, Error> {
        validate_saved_id(run_id)?;
        let initial = match self.raw(id).await {
            Ok((raw, _)) => raw,
            Err(Error::Missing) => return Ok(None),
            Err(error) => return Err(error),
        };
        let Some(d) = self.load(id).await? else {
            return Ok(None);
        };
        policy.validate_scope(&d.query, &d.engines)?;
        if policy.fingerprint(&d.engines)? != d.policy_fingerprint {
            return Err(Error::Conflict);
        }
        match self
            .backend
            .get_object(&self.key("report", &format!("{}:{run_id}", id.as_str())))
            .await
        {
            Read::Found(value) => {
                let window = &value["observation"]["window"];
                let engines = window["engines"]
                    .as_array()
                    .ok_or(Error::Malformed)?
                    .iter()
                    .map(|v| v.as_str().map(str::to_owned).ok_or(Error::Malformed))
                    .collect::<Result<Vec<_>, _>>()?;
                if engines.is_empty() {
                    return Err(Error::Malformed);
                }
                policy
                    .validate_scope(window["query"].as_str().ok_or(Error::Malformed)?, &engines)?;
                if value["search_id"].as_str() != Some(id.as_str())
                    || value["run_id"].as_str() != Some(run_id)
                {
                    return Err(Error::Malformed);
                }
                if value["expires_at"].as_f64().ok_or(Error::Malformed)? <= self.now().await? {
                    return Ok(None);
                }
                // A concurrent pause/update/delete invalidates this read. The
                // second exact snapshot is its authorization linearization point.
                if self.raw(id).await?.0 != initial {
                    return Err(Error::Conflict);
                }
                policy.validate_scope(&d.query, &d.engines)?;
                if policy.fingerprint(&d.engines)? != d.policy_fingerprint {
                    return Err(Error::Conflict);
                }
                Ok(Some(value))
            }
            Read::Missing => Ok(None),
            Read::Malformed => Err(Error::Malformed),
            Read::Unavailable => Err(Error::Store(Failure::Unavailable)),
            Read::Closed => Err(Error::Store(Failure::Closed)),
        }
    }
}
