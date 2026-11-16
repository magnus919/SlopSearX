//! Native staged admission and transitions. No engine/provider execution.
use super::{
    Failure, Read, Valkey, keys, retained_ttl,
    scripts::{Reply, Script, Staged},
};
use ssx_application::policy::{PolicyService, SearchSurface, ValidatedScope};
use ssx_domain::{
    LeaseToken, StagedSearchId, TenantId, WallClockExpiry, WallClockTimestamp, staged::Record,
};
use ssx_json::{Value, json};
const READY: &str = keys::STAGED_READY;
const RUNNING: &str = keys::STAGED_RUNNING;
const LEASE_SECONDS: u64 = 60;
fn key(kind: &str, tenant: &TenantId, id: Option<&str>) -> String {
    keys::staged(kind, tenant, id).unwrap()
}
fn ttl(record: &Record, now: WallClockTimestamp) -> Result<String, Failure> {
    Ok(retained_ttl(
        WallClockExpiry::new(record.expires_at()).map_err(|_| Failure::InvalidInput)?,
        now,
    )?
    .to_string())
}
fn decode(value: Value) -> Result<Record, Failure> {
    Record::from_value(value).map_err(|_| Failure::Malformed)
}
fn read_failure<T>(read: Read<T>) -> Failure {
    match read {
        Read::Closed => Failure::Closed,
        Read::Unavailable => Failure::Unavailable,
        _ => Failure::Malformed,
    }
}
#[derive(Debug, PartialEq)]
pub enum RecordRead {
    Found(Record),
    Missing,
    Expired { expires_at: f64 },
}
#[derive(Debug)]
pub struct Transition {
    pub status: Staged,
    pub record: Option<Record>,
}
/// Acknowledged claim result. Terminal transitions are returned from the atomic
/// claim response; a reload must never be used to infer a transition event.
pub enum ClaimOutcome {
    Claimed(OwnedStage),
    Transition(Transition),
}
/// An ownership observation, not permanent dispatch authorization. W06 must
/// revalidate policy and renew immediately before execution and fence its commit.
pub struct OwnedStage {
    raw: Vec<u8>,
    record: Record,
    token: LeaseToken<StagedSearchId>,
}
impl OwnedStage {
    pub fn record(&self) -> &Record {
        &self.record
    }
    pub fn token(&self) -> &LeaseToken<StagedSearchId> {
        &self.token
    }
}
/// Validated read snapshot used for compare-before-claim. A stale snapshot makes
/// no reservation; callers may begin a fresh observation, never dispatch it.
pub struct PreparedClaim {
    record: Record,
    raw: Vec<u8>,
    token: LeaseToken<StagedSearchId>,
    attempt_id: String,
}
impl PreparedClaim {
    pub fn record(&self) -> &Record {
        &self.record
    }
}
const CLAIM_GUARD: &str = include_str!("staged_scripts/claim_guard.lua");
const SAVE_GUARD: &str = include_str!("staged_scripts/save_guard.lua");
pub struct StagedStore<'a> {
    store: &'a Valkey,
}
impl<'a> StagedStore<'a> {
    pub fn new(store: &'a Valkey) -> Self {
        Self { store }
    }
    async fn transition(
        &self,
        script: Script,
        keys: &[&str],
        args: &[String],
    ) -> Result<Transition, Failure> {
        match self.store.run(script, keys, args).await? {
            Reply::Staged { status, record } => Ok(Transition {
                status,
                record: record.map(decode).transpose()?,
            }),
            _ => Err(Failure::Malformed),
        }
    }
    async fn flag(&self, script: Script, keys: &[&str], args: &[String]) -> Result<bool, Failure> {
        match self.store.run(script, keys, args).await? {
            Reply::Flag(v) => Ok(v),
            _ => Err(Failure::Malformed),
        }
    }
    pub async fn read(
        &self,
        tenant: &TenantId,
        id: &StagedSearchId,
        now: WallClockTimestamp,
    ) -> Result<RecordRead, Failure> {
        match self
            .store
            .get_object(&key("record", tenant, Some(id.as_str())))
            .await
        {
            Read::Found(v) => {
                let record = decode(v)?;
                if record.tenant() != *tenant || record.id() != *id {
                    return Err(Failure::Malformed);
                }
                if record.expires_at() <= now.seconds() {
                    Ok(RecordRead::Expired {
                        expires_at: record.expires_at(),
                    })
                } else {
                    Ok(RecordRead::Found(record))
                }
            }
            Read::Missing => Ok(RecordRead::Missing),
            other => Err(read_failure(other)),
        }
    }
    pub async fn find_by_idempotency(
        &self,
        tenant: &TenantId,
        idem: &str,
        now: WallClockTimestamp,
    ) -> Result<RecordRead, Failure> {
        let mapping = match self
            .store
            .get_object(&key("idem", tenant, Some(idem)))
            .await
        {
            Read::Missing => return Ok(RecordRead::Missing),
            Read::Found(v) => v,
            other => return Err(read_failure(other)),
        };
        let expected = format!("{}:", key("record", tenant, None));
        let id = mapping["record_key"]
            .as_str()
            .and_then(|s| s.strip_prefix(&expected))
            .ok_or(Failure::Malformed)?;
        self.read(
            tenant,
            &StagedSearchId::new(id).map_err(|_| Failure::Malformed)?,
            now,
        )
        .await
    }
    /// Fresh F09 scopes are required for every captured stage. This validates
    /// storage eligibility; W06 additionally owns staged tool grant/operator limits
    /// and fresh policy revalidation at dispatch (records are never authority).
    pub async fn admit(
        &self,
        policy: &PolicyService,
        record: &Record,
        idem: &str,
        scopes: &[&ValidatedScope],
        now: WallClockTimestamp,
    ) -> Result<Transition, Failure> {
        let stages = record.value()["stages"].as_array().unwrap();
        if record.state() != "queued"
            || record.value()["budget"]["reserved"] != json!(0)
            || record.expires_at() <= now.seconds()
            || idem.is_empty()
            || idem.chars().count() > 128
            || scopes.len() != stages.len()
        {
            return Err(Failure::InvalidInput);
        }
        for (stage, scope) in stages.iter().zip(scopes) {
            let bound = policy
                .with_revalidated(scope, |command| {
                    let request = command.request();
                    let filters = &record.value()["plan"]["filters"];
                    let safesearch = match request.safesearch {
                        0 => "off",
                        1 => "moderate",
                        2 => "strict",
                        _ => return false,
                    };
                    !matches!(
                        command.origin(),
                        SearchSurface::Http | SearchSurface::Worker | SearchSurface::Preview
                    ) && record.value()["plan"]["query"] == json!(request.query)
                        && filters["language"] == json!(request.language)
                        && filters["time_range"] == json!(request.time_range)
                        && filters["safesearch"] == json!(safesearch)
                        && filters["freshness"] == json!(request.freshness)
                        && request.date_from.is_none()
                        && request.date_to.is_none()
                        && request.media_type.is_none()
                })
                .map_err(|_| Failure::InvalidInput)?;
            if !bound {
                return Err(Failure::InvalidInput);
            }
            let preview = scope.preview();
            if stage["scope"]["selected_engines"] != json!(preview.scope.selected_engines)
                || stage["enforcement"]
                    != Value::Object(
                        preview
                            .enforcement
                            .iter()
                            .map(|(k, v)| (k.clone(), v.to_value()))
                            .collect(),
                    )
            {
                return Err(Failure::InvalidInput);
            }
        }
        let tenant = record.tenant();
        let id = record.id();
        let rk = key("record", &tenant, Some(id.as_str()));
        let ik = key("idem", &tenant, Some(idem));
        let index = key("index", &tenant, None);
        let marker = key("tenant", &tenant, None);
        let pattern = format!("{}:*", key("record", &tenant, None));
        let raw = String::from_utf8(record.encode().map_err(|_| Failure::InvalidInput)?)
            .map_err(|_| Failure::InvalidInput)?;
        let result = self
            .transition(
                Script::StagedAdmit,
                &[&rk, &ik, &index, READY, &marker],
                &[
                    record.value()["plan_digest"].as_str().unwrap().into(),
                    now.seconds().to_string(),
                    "32".into(),
                    ttl(record, now)?,
                    raw,
                    record.expires_at().to_string(),
                    pattern,
                    id.to_string(),
                ],
            )
            .await?;
        if result.record.as_ref().is_some_and(|r| r.tenant() != tenant) {
            return Err(Failure::Malformed);
        }
        Ok(result)
    }
    pub async fn prepare_claim(
        &self,
        tenant: &TenantId,
        id: &StagedSearchId,
        token: LeaseToken<StagedSearchId>,
        attempt_id: &str,
        now: WallClockTimestamp,
    ) -> Result<Option<PreparedClaim>, Failure> {
        ssx_domain::AttemptId::new(attempt_id).map_err(|_| Failure::InvalidInput)?;
        let raw = match self
            .store
            .get(&key("record", tenant, Some(id.as_str())))
            .await
        {
            Read::Missing => return Ok(None),
            Read::Found(raw) => raw,
            other => return Err(read_failure(other)),
        };
        let record = Record::decode(&raw).map_err(|_| Failure::Malformed)?;
        if record.tenant() != *tenant || record.id() != *id {
            return Err(Failure::Malformed);
        }
        if record.expires_at() <= now.seconds() || record.state() != "queued" {
            return Ok(None);
        }
        if record.value()["stages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| {
                s["attempts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|a| a["attempt_id"].as_str() == Some(attempt_id))
            })
        {
            return Err(Failure::InvalidInput);
        }
        Ok(Some(PreparedClaim {
            record,
            raw,
            token,
            attempt_id: attempt_id.into(),
        }))
    }
    pub async fn claim_prepared_outcome(
        &self,
        prepared: PreparedClaim,
        now: WallClockTimestamp,
    ) -> Result<ClaimOutcome, Failure> {
        let PreparedClaim {
            record,
            raw,
            token,
            attempt_id,
        } = prepared;
        let rk = key("record", &record.tenant(), Some(record.id().as_str()));
        let lk = key("lease", &record.tenant(), Some(record.id().as_str()));
        let script = format!("{CLAIM_GUARD}{}", Script::StagedClaim.source());
        let mut cmd = redis::cmd("EVAL");
        cmd.arg(script)
            .arg(4)
            .arg(&[&rk, &lk, READY, RUNNING])
            .arg(&[
                now.seconds().to_string(),
                token.as_str().into(),
                ttl(&record, now)?,
                LEASE_SECONDS.to_string(),
                attempt_id,
                now.seconds().to_string(),
            ]);
        cmd.arg(raw);
        let raw_reply = self.store.execute(cmd, true).await?;
        let (_status, stored): (String, Vec<u8>) =
            redis::from_redis_value(raw_reply.clone()).map_err(|_| Failure::Malformed)?;
        match Script::StagedClaim.decode(raw_reply)? {
            Reply::Staged {
                status: Staged::Claimed,
                record: Some(value),
            } => Ok(ClaimOutcome::Claimed(OwnedStage {
                record: decode(value)?,
                raw: stored,
                token,
            })),
            Reply::Staged { status, record } => Ok(ClaimOutcome::Transition(Transition {
                status,
                record: record.map(decode).transpose()?,
            })),
            _ => Err(Failure::Malformed),
        }
    }
    /// Compatibility wrapper. Runners needing terminal acknowledgment events use
    /// `claim_prepared_outcome`; this retains the accepted W11 API.
    pub async fn claim_prepared(
        &self,
        prepared: PreparedClaim,
        now: WallClockTimestamp,
    ) -> Result<Option<OwnedStage>, Failure> {
        Ok(match self.claim_prepared_outcome(prepared, now).await? {
            ClaimOutcome::Claimed(owned) => Some(owned),
            ClaimOutcome::Transition(_) => None,
        })
    }
    pub async fn claim(
        &self,
        tenant: &TenantId,
        id: &StagedSearchId,
        token: LeaseToken<StagedSearchId>,
        attempt_id: &str,
        now: WallClockTimestamp,
    ) -> Result<Option<OwnedStage>, Failure> {
        let Some(prepared) = self
            .prepare_claim(tenant, id, token, attempt_id, now)
            .await?
        else {
            return Ok(None);
        };
        self.claim_prepared(prepared, now).await
    }
    pub async fn renew(
        &self,
        owned: &OwnedStage,
        now: WallClockTimestamp,
    ) -> Result<bool, Failure> {
        let r = &owned.record;
        let lk = key("lease", &r.tenant(), Some(r.id().as_str()));
        let rk = key("record", &r.tenant(), Some(r.id().as_str()));
        self.flag(
            Script::StagedRenew,
            &[&lk, RUNNING],
            &[
                owned.token.as_str().into(),
                LEASE_SECONDS.to_string(),
                (now.seconds() + LEASE_SECONDS as f64).to_string(),
                rk,
            ],
        )
        .await
    }
    pub async fn save_owned(
        &self,
        owned: &OwnedStage,
        next: &Record,
        now: WallClockTimestamp,
    ) -> Result<bool, Failure> {
        owned
            .record
            .validate_completion(next)
            .map_err(|_| Failure::InvalidInput)?;
        if next.expires_at() <= now.seconds() {
            return Err(Failure::InvalidInput);
        }
        let rk = key("record", &next.tenant(), Some(next.id().as_str()));
        let lk = key("lease", &next.tenant(), Some(next.id().as_str()));
        let mut cmd = redis::cmd("EVAL");
        cmd.arg(format!("{SAVE_GUARD}{}", Script::StagedOwnedSave.source()))
            .arg(4)
            .arg(&[&rk, &lk, READY, RUNNING])
            .arg(&[
                owned.token.as_str().into(),
                ttl(next, now)?,
                String::from_utf8(next.encode().map_err(|_| Failure::InvalidInput)?).unwrap(),
                next.state().into(),
                now.seconds().to_string(),
            ]);
        cmd.arg(&owned.raw);
        match Script::StagedOwnedSave.decode(self.store.execute(cmd, true).await?)? {
            Reply::Flag(v) => Ok(v),
            _ => Err(Failure::Malformed),
        }
    }
    pub async fn request_retry(
        &self,
        tenant: &TenantId,
        id: &StagedSearchId,
        retry_key: &str,
        now: WallClockTimestamp,
    ) -> Result<Transition, Failure> {
        if retry_key.is_empty() || retry_key.chars().count() > 128 {
            return Err(Failure::InvalidInput);
        }
        let record = match self.read(tenant, id, now).await? {
            RecordRead::Found(v) => v,
            RecordRead::Missing => {
                return Ok(Transition {
                    status: Staged::Missing,
                    record: None,
                });
            }
            RecordRead::Expired { .. } => {
                return Ok(Transition {
                    status: Staged::Expired,
                    record: None,
                });
            }
        };
        self.transition(
            Script::StagedRetry,
            &[&key("record", tenant, Some(id.as_str())), READY],
            &[
                now.seconds().to_string(),
                retry_key.into(),
                ttl(&record, now)?,
            ],
        )
        .await
    }
    async fn command<T: redis::FromRedisValue>(
        &self,
        cmd: redis::Cmd,
        write: bool,
    ) -> Result<T, Failure> {
        redis::from_redis_value(self.store.execute(cmd, write).await?)
            .map_err(|_| Failure::Malformed)
    }
    async fn candidates(
        &self,
        index: &str,
        now: WallClockTimestamp,
        limit: usize,
    ) -> Result<Vec<String>, Failure> {
        let mut cmd = redis::cmd("ZRANGEBYSCORE");
        cmd.arg(index)
            .arg("-inf")
            .arg(now.seconds())
            .arg("LIMIT")
            .arg(0)
            .arg(limit);
        self.command(cmd, false).await
    }
    async fn remove_hint(&self, index: &str, rk: &str) -> Result<(), Failure> {
        let mut cmd = redis::cmd("ZREM");
        cmd.arg(index).arg(rk);
        let _: i64 = self.command(cmd, true).await?;
        Ok(())
    }
    async fn record_key(&self, rk: &str) -> Result<Option<Record>, Failure> {
        match self.store.get_object(rk).await {
            Read::Missing => Ok(None),
            Read::Found(v) => {
                let r = decode(v)?;
                if key("record", &r.tenant(), Some(r.id().as_str())) != rk {
                    return Err(Failure::Malformed);
                }
                Ok(Some(r))
            }
            other => Err(read_failure(other)),
        }
    }
    pub async fn claim_next(
        &self,
        now: WallClockTimestamp,
    ) -> Result<Option<(TenantId, StagedSearchId)>, Failure> {
        for rk in self.candidates(READY, now, 16).await? {
            match self.record_key(&rk).await? {
                Some(r) if r.expires_at() > now.seconds() && r.state() == "queued" => {
                    return Ok(Some((r.tenant(), r.id())));
                }
                _ => self.remove_hint(READY, &rk).await?,
            }
        }
        Ok(None)
    }
    async fn interrupt(
        &self,
        rk: &str,
        r: &Record,
        now: WallClockTimestamp,
    ) -> Result<bool, Failure> {
        self.flag(
            Script::StagedInterrupt,
            &[
                rk,
                &key("lease", &r.tenant(), Some(r.id().as_str())),
                RUNNING,
            ],
            &[now.seconds().to_string(), ttl(r, now)?],
        )
        .await
    }
    /// Unknown attempts remain fully charged. No automatic retry/provider request.
    pub async fn recover_orphans(&self, now: WallClockTimestamp) -> Result<usize, Failure> {
        let mut count = 0;
        for rk in self.candidates(RUNNING, now, 32).await? {
            if let Some(r) = self.record_key(&rk).await? {
                count += usize::from(self.interrupt(&rk, &r, now).await?);
            } else {
                self.remove_hint(RUNNING, &rk).await?;
            }
        }
        Ok(count)
    }
    /// Caller persists cursor in its worker loop; hints never authorize execution.
    /// Races only create stale hints; claims/interruption recheck record and lease.
    pub async fn reconcile_indexes(
        &self,
        cursor: u64,
        now: WallClockTimestamp,
    ) -> Result<u64, Failure> {
        let mut cmd = redis::cmd("SCAN");
        cmd.arg(cursor)
            .arg("MATCH")
            .arg("mcp:staged:v1:record:*")
            .arg("COUNT")
            .arg(128);
        let (next, keys): (u64, Vec<String>) = self.command(cmd, false).await?;
        for rk in keys {
            let Some(r) = self.record_key(&rk).await? else {
                continue;
            };
            if r.expires_at() <= now.seconds() {
                continue;
            }
            let (index, score) = if r.state() == "queued" {
                (READY, now.seconds())
            } else if r.state() == "running" {
                let mut cmd = redis::cmd("TTL");
                cmd.arg(key("lease", &r.tenant(), Some(r.id().as_str())));
                let remaining: i64 = self.command(cmd, false).await?;
                if remaining <= 0 {
                    self.interrupt(&rk, &r, now).await?;
                    continue;
                }
                (RUNNING, now.seconds() + remaining as f64)
            } else {
                continue;
            };
            let mut cmd = redis::cmd("ZADD");
            cmd.arg(index);
            if index == READY {
                cmd.arg("NX");
            }
            cmd.arg(score).arg(&rk);
            let _: i64 = self.command(cmd, true).await?;
        }
        Ok(next)
    }
    /// New acknowledged interruption protocol, leaving legacy Lua/reply intact.
    async fn interrupt_acknowledged(
        &self,
        rk: &str,
        r: &Record,
        now: WallClockTimestamp,
    ) -> Result<Option<Record>, Failure> {
        match self
            .store
            .run(
                Script::StagedInterruptAcknowledged,
                &[
                    rk,
                    &key("lease", &r.tenant(), Some(r.id().as_str())),
                    RUNNING,
                ],
                &[now.seconds().to_string(), ttl(r, now)?],
            )
            .await?
        {
            Reply::StagedInterrupted(value) => {
                let record = value.map(decode).transpose()?;
                if record.as_ref().is_some_and(|v| {
                    v.tenant() != r.tenant() || v.id() != r.id() || v.state() != "interrupted"
                }) {
                    return Err(Failure::Malformed);
                }
                Ok(record)
            }
            _ => Err(Failure::Malformed),
        }
    }
    /// Only successful atomic acknowledgments are returned. An unknown write
    /// produces an error, never a speculative event or a retry of that write.
    pub async fn recover_orphans_acknowledged(
        &self,
        now: WallClockTimestamp,
        mut acknowledged: impl FnMut(Record),
    ) -> Result<usize, Failure> {
        let mut count = 0;
        for rk in self.candidates(RUNNING, now, 32).await? {
            if let Some(r) = self.record_key(&rk).await? {
                if let Some(interrupted) = self.interrupt_acknowledged(&rk, &r, now).await? {
                    acknowledged(interrupted);
                    count += 1;
                }
            } else {
                self.remove_hint(RUNNING, &rk).await?;
            }
        }
        Ok(count)
    }
    pub async fn reconcile_indexes_acknowledged(
        &self,
        cursor: u64,
        now: WallClockTimestamp,
        mut acknowledged: impl FnMut(Record),
    ) -> Result<u64, Failure> {
        let mut cmd = redis::cmd("SCAN");
        cmd.arg(cursor)
            .arg("MATCH")
            .arg("mcp:staged:v1:record:*")
            .arg("COUNT")
            .arg(128);
        let (next, keys): (u64, Vec<String>) = self.command(cmd, false).await?;
        for rk in keys {
            let Some(r) = self.record_key(&rk).await? else {
                continue;
            };
            if r.expires_at() <= now.seconds() {
                continue;
            }
            let (index, score) = if r.state() == "queued" {
                (READY, now.seconds())
            } else if r.state() == "running" {
                let mut cmd = redis::cmd("TTL");
                cmd.arg(key("lease", &r.tenant(), Some(r.id().as_str())));
                let remaining: i64 = self.command(cmd, false).await?;
                if remaining <= 0 {
                    if let Some(interrupted) = self.interrupt_acknowledged(&rk, &r, now).await? {
                        acknowledged(interrupted);
                    }
                    continue;
                }
                (RUNNING, now.seconds() + remaining as f64)
            } else {
                continue;
            };
            let mut cmd = redis::cmd("ZADD");
            cmd.arg(index);
            if index == READY {
                cmd.arg("NX");
            }
            cmd.arg(score).arg(&rk);
            let _: i64 = self.command(cmd, true).await?;
        }
        Ok(next)
    }
    pub async fn list_recent(
        &self,
        tenant: &TenantId,
        before: Option<(f64, String)>,
        limit: usize,
        now: WallClockTimestamp,
    ) -> Result<Vec<Record>, Failure> {
        if !(1..=50).contains(&limit) {
            return Err(Failure::InvalidInput);
        }
        let mut cmd = redis::cmd("ZREVRANGE");
        cmd.arg(key("index", tenant, None)).arg(0).arg(31);
        let ids: Vec<String> = self.command(cmd, false).await?;
        let mut rows = Vec::new();
        let prefix = format!("{}:", key("record", tenant, None));
        for member in ids {
            if member.starts_with("idem:") {
                continue;
            }
            let id = member.strip_prefix(&prefix).unwrap_or(&member);
            if id.starts_with("mcp:staged:v1:record:") {
                continue;
            }
            let id = StagedSearchId::new(id).map_err(|_| Failure::Malformed)?;
            if let RecordRead::Found(r) = self.read(tenant, &id, now).await? {
                rows.push(r);
            }
        }
        rows.sort_by(|a, b| {
            b.value()["accepted_at"]
                .as_f64()
                .unwrap()
                .total_cmp(&a.value()["accepted_at"].as_f64().unwrap())
                .then_with(|| b.id().cmp(&a.id()))
        });
        if let Some((at, id)) = before {
            rows.retain(|r| {
                (
                    r.value()["accepted_at"].as_f64().unwrap(),
                    r.id().to_string(),
                ) < (at, id.clone())
            });
        }
        rows.truncate(limit);
        Ok(rows)
    }
}
