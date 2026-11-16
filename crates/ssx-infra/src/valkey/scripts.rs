//! Source-exact Lua assets. Return decoding is workflow-specific and fail closed.
use super::Failure;
use redis::Value;
use ssx_json::Value as Json;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    ResearchIdempotentCreate,
    ResearchLeaseRelease,
    ResearchLeaseRenew,
    ResearchLeaseSave,
    ResearchReadyRefresh,
    ResearchReadyTake,
    ResearchRecentRecord,
    SavedAckEvent,
    SavedCas,
    SavedClaim,
    SavedCommit,
    SavedCreate,
    SavedDelete,
    SavedPublishExpiry,
    SavedReadEvents,
    SavedRecentRecord,
    SavedRelease,
    SavedRenew,
    StagedAdmit,
    StagedClaim,
    StagedInterrupt,
    StagedInterruptAcknowledged,
    StagedOwnedSave,
    StagedRenew,
    StagedRetry,
}
impl Script {
    pub fn source(self) -> &'static str {
        match self {
            Self::ResearchIdempotentCreate => {
                include_str!("scripts/research_idempotent_create_script.lua")
            }
            Self::ResearchLeaseRelease => include_str!("scripts/research_lease_release_script.lua"),
            Self::ResearchLeaseRenew => include_str!("scripts/research_lease_renew_script.lua"),
            Self::ResearchLeaseSave => include_str!("scripts/research_lease_save_script.lua"),
            Self::ResearchReadyRefresh => include_str!("scripts/research_ready_refresh_script.lua"),
            Self::ResearchReadyTake => include_str!("scripts/research_ready_take_script.lua"),
            Self::ResearchRecentRecord => include_str!("scripts/research_recent_record_script.lua"),
            Self::SavedAckEvent => include_str!("scripts/saved_ack_event.lua"),
            Self::SavedCas => include_str!("scripts/saved_cas.lua"),
            Self::SavedClaim => include_str!("scripts/saved_claim.lua"),
            Self::SavedCommit => include_str!("scripts/saved_commit.lua"),
            Self::SavedCreate => include_str!("scripts/saved_create.lua"),
            Self::SavedDelete => include_str!("scripts/saved_delete.lua"),
            Self::SavedPublishExpiry => include_str!("scripts/saved_publish_expiry.lua"),
            Self::SavedReadEvents => include_str!("scripts/saved_read_events.lua"),
            Self::SavedRecentRecord => include_str!("scripts/saved_recent_record.lua"),
            Self::SavedRelease => include_str!("scripts/saved_release.lua"),
            Self::SavedRenew => include_str!("scripts/saved_renew.lua"),
            Self::StagedAdmit => include_str!("scripts/staged_admit_script.lua"),
            Self::StagedClaim => include_str!("scripts/staged_claim_script.lua"),
            Self::StagedInterrupt => include_str!("scripts/staged_interrupt_script.lua"),
            Self::StagedInterruptAcknowledged => {
                include_str!("scripts/staged_interrupt_acknowledged.lua")
            }
            Self::StagedOwnedSave => include_str!("scripts/staged_owned_save_script.lua"),
            Self::StagedRenew => include_str!("scripts/staged_renew_script.lua"),
            Self::StagedRetry => include_str!("scripts/staged_retry_script.lua"),
        }
    }
    pub fn key_count(self) -> usize {
        match self {
            Self::ResearchIdempotentCreate => 2,
            Self::ResearchLeaseRelease => 1,
            Self::ResearchLeaseRenew => 1,
            Self::ResearchLeaseSave => 2,
            Self::ResearchReadyRefresh => 4,
            Self::ResearchReadyTake => 2,
            Self::ResearchRecentRecord => 1,
            Self::SavedAckEvent => 3,
            Self::SavedCas => 5,
            Self::SavedClaim => 2,
            Self::SavedCommit => 7,
            Self::SavedCreate => 5,
            Self::SavedDelete => 8,
            Self::SavedPublishExpiry => 3,
            Self::SavedReadEvents => 1,
            Self::SavedRecentRecord => 1,
            Self::SavedRelease => 1,
            Self::SavedRenew => 1,
            Self::StagedAdmit => 5,
            Self::StagedClaim => 4,
            Self::StagedInterrupt | Self::StagedInterruptAcknowledged => 3,
            Self::StagedOwnedSave => 4,
            Self::StagedRenew => 2,
            Self::StagedRetry => 2,
        }
    }
    pub fn arg_count(self) -> usize {
        match self {
            Self::ResearchIdempotentCreate => 4,
            Self::ResearchLeaseRelease => 1,
            Self::ResearchLeaseRenew => 2,
            Self::ResearchLeaseSave => 3,
            Self::ResearchReadyRefresh => 2,
            Self::ResearchReadyTake => 1,
            Self::ResearchRecentRecord => 4,
            Self::SavedAckEvent => 7,
            Self::SavedCas => 11,
            Self::SavedClaim => 4,
            Self::SavedCommit => 20,
            Self::SavedCreate => 10,
            Self::SavedDelete => 4,
            Self::SavedPublishExpiry => 5,
            Self::SavedReadEvents => 3,
            Self::SavedRecentRecord => 4,
            Self::SavedRelease => 1,
            Self::SavedRenew => 2,
            Self::StagedAdmit => 8,
            Self::StagedClaim => 6,
            Self::StagedInterrupt | Self::StagedInterruptAcknowledged => 2,
            Self::StagedOwnedSave => 5,
            Self::StagedRenew => 4,
            Self::StagedRetry => 3,
        }
    }
}
#[derive(Debug, PartialEq)]
pub enum Reply {
    StagedInterrupted(Option<Json>),
    Flag(bool),
    ResearchCreated {
        created: bool,
        job_id: String,
    },
    ResearchIdempotencyMalformed,
    Ready(Option<(String, String)>),
    SavedCreate(SavedCreate),
    SavedCas(SavedCas),
    SavedClaim(Option<Json>),
    SavedCommit {
        status: SavedCommit,
        cursor: String,
    },
    SavedAck(SavedAck),
    SavedExpiry(SavedExpiry),
    SavedEvents {
        first_cursor: String,
        entries: Vec<(String, Vec<(String, String)>)>,
    },
    Staged {
        status: Staged,
        record: Option<Json>,
    },
}
#[derive(Debug, PartialEq, Eq)]
pub enum SavedCreate {
    Created,
    Exists,
    Quota,
    RepairUnavailable,
}
#[derive(Debug, PartialEq, Eq)]
pub enum SavedCas {
    Applied,
    Missing,
    Capacity,
    Revision(u64),
}
#[derive(Debug, PartialEq, Eq)]
pub enum SavedCommit {
    Applied,
    Missing,
    OwnershipLost,
    Conflict,
    Capacity,
}
#[derive(Debug, PartialEq, Eq)]
pub enum SavedAck {
    Advanced,
    Unchanged,
    Capacity,
    Future,
    Empty,
    Expired,
}
#[derive(Debug, PartialEq, Eq)]
pub enum SavedExpiry {
    Published,
    Unchanged,
    Capacity,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Staged {
    Created,
    Replay,
    Conflict,
    Unavailable,
    Quota,
    Missing,
    NotQueued,
    Deadline,
    Busy,
    Budget,
    Claimed,
    Expired,
    None,
    Queued,
}
fn integer(v: Value) -> Result<i64, Failure> {
    if let Value::Int(n) = v {
        Ok(n)
    } else {
        Err(Failure::Malformed)
    }
}
fn string(v: Value) -> Result<String, Failure> {
    if let Value::BulkString(b) = v {
        String::from_utf8(b).map_err(|_| Failure::Malformed)
    } else {
        Err(Failure::Malformed)
    }
}
fn pair(v: Value) -> Result<(Value, Value), Failure> {
    if let Value::Array(mut a) = v
        && a.len() == 2
    {
        let b = a.pop().unwrap();
        return Ok((a.pop().unwrap(), b));
    }
    Err(Failure::Malformed)
}
fn object(s: &str) -> Result<Json, Failure> {
    let v: Json = ssx_json::from_str(s).map_err(|_| Failure::Malformed)?;
    if v.is_object() {
        Ok(v)
    } else {
        Err(Failure::Malformed)
    }
}
fn cursor(s: &str) -> bool {
    s.split_once('-').is_some_and(|(a, b)| {
        !a.is_empty() && !b.is_empty() && a.parse::<u64>().is_ok() && b.parse::<u64>().is_ok()
    })
}
impl Script {
    pub fn decode(self, value: Value) -> Result<Reply, Failure> {
        use Script::*;
        Ok(match self {
            ResearchIdempotentCreate => {
                let (a, b) = pair(value)?;
                let id = string(b)?;
                match integer(a)? {
                    -1 if id.is_empty() => Reply::ResearchIdempotencyMalformed,
                    n @ (0 | 1) if !id.is_empty() => Reply::ResearchCreated {
                        created: n == 1,
                        job_id: id,
                    },
                    _ => return Err(Failure::Malformed),
                }
            }
            ResearchReadyTake => {
                if value == Value::Array(vec![]) {
                    Reply::Ready(None)
                } else {
                    let (a, b) = pair(value)?;
                    Reply::Ready(Some((string(a)?, string(b)?)))
                }
            }
            SavedCreate => Reply::SavedCreate(match integer(value)? {
                1 => self::SavedCreate::Created,
                0 => self::SavedCreate::Exists,
                -1 => self::SavedCreate::Quota,
                -2 => self::SavedCreate::RepairUnavailable,
                _ => return Err(Failure::Malformed),
            }),
            SavedCas | SavedDelete => Reply::SavedCas(match integer(value)? {
                0 => self::SavedCas::Applied,
                -1 => self::SavedCas::Missing,
                -3 if self == SavedCas => self::SavedCas::Capacity,
                n if n > 0 => self::SavedCas::Revision(n as u64),
                _ => return Err(Failure::Malformed),
            }),
            SavedClaim => match value {
                Value::Int(0) => Reply::SavedClaim(None),
                v => Reply::SavedClaim(Some(object(&string(v)?)?)),
            },
            SavedCommit => {
                let (a, b) = pair(value)?;
                let c = string(b)?;
                let status = match integer(a)? {
                    1 => self::SavedCommit::Applied,
                    -1 => self::SavedCommit::Missing,
                    -2 => self::SavedCommit::OwnershipLost,
                    -3 => self::SavedCommit::Conflict,
                    -4 => self::SavedCommit::Capacity,
                    _ => return Err(Failure::Malformed),
                };
                if (!c.is_empty() && !cursor(&c))
                    || (status != self::SavedCommit::Applied && !c.is_empty())
                {
                    return Err(Failure::Malformed);
                }
                Reply::SavedCommit { status, cursor: c }
            }
            SavedAckEvent => Reply::SavedAck(match integer(value)? {
                1 => SavedAck::Advanced,
                0 => SavedAck::Unchanged,
                -1 => SavedAck::Capacity,
                -2 => SavedAck::Future,
                -3 => SavedAck::Empty,
                -4 => SavedAck::Expired,
                _ => return Err(Failure::Malformed),
            }),
            SavedPublishExpiry => Reply::SavedExpiry(match integer(value)? {
                1 => SavedExpiry::Published,
                0 => SavedExpiry::Unchanged,
                -1 => SavedExpiry::Capacity,
                _ => return Err(Failure::Malformed),
            }),
            SavedReadEvents => {
                let (a, b) = pair(value)?;
                let first = string(a)?;
                if !first.is_empty() && !cursor(&first) {
                    return Err(Failure::Malformed);
                }
                let Value::Array(entries) = b else {
                    return Err(Failure::Malformed);
                };
                let mut result = Vec::new();
                for entry in entries {
                    let (a, b) = pair(entry)?;
                    let id = string(a)?;
                    if !cursor(&id) {
                        return Err(Failure::Malformed);
                    }
                    let Value::Array(fields) = b else {
                        return Err(Failure::Malformed);
                    };
                    if fields.len() % 2 != 0 {
                        return Err(Failure::Malformed);
                    }
                    let mut pairs = Vec::new();
                    let mut fields = fields.into_iter();
                    while let Some(k) = fields.next() {
                        pairs.push((string(k)?, string(fields.next().unwrap())?));
                    }
                    result.push((id, pairs));
                }
                Reply::SavedEvents {
                    first_cursor: first,
                    entries: result,
                }
            }
            StagedInterruptAcknowledged => {
                let (flag, raw) = pair(value)?;
                let raw = string(raw)?;
                match integer(flag)? {
                    0 if raw.is_empty() => Reply::StagedInterrupted(None),
                    1 if !raw.is_empty() => Reply::StagedInterrupted(Some(object(&raw)?)),
                    _ => return Err(Failure::Malformed),
                }
            }
            StagedAdmit | StagedClaim | StagedRetry => {
                let (a, b) = pair(value)?;
                let code = string(a)?;
                let status = match (self, code.as_str()) {
                    (StagedAdmit, "created") => Staged::Created,
                    (StagedAdmit | StagedRetry, "replay") => Staged::Replay,
                    (StagedAdmit, "conflict") => Staged::Conflict,
                    (StagedAdmit, "unavailable") => Staged::Unavailable,
                    (StagedAdmit, "quota") => Staged::Quota,
                    (StagedClaim | StagedRetry, "missing") => Staged::Missing,
                    (StagedClaim, "not_queued") => Staged::NotQueued,
                    (StagedClaim | StagedRetry, "deadline") => Staged::Deadline,
                    (StagedClaim | StagedRetry, "busy") => Staged::Busy,
                    (StagedClaim | StagedRetry, "budget") => Staged::Budget,
                    (StagedClaim, "claimed") => Staged::Claimed,
                    (StagedRetry, "expired") => Staged::Expired,
                    (StagedRetry, "none") => Staged::None,
                    (StagedRetry, "queued") => Staged::Queued,
                    _ => return Err(Failure::Malformed),
                };
                let raw = string(b)?;
                let record = if raw.is_empty() {
                    None
                } else {
                    Some(object(&raw)?)
                };
                let required = matches!(
                    status,
                    Staged::Created
                        | Staged::Replay
                        | Staged::Claimed
                        | Staged::Queued
                        | Staged::None
                ) || (self == StagedClaim
                    && matches!(status, Staged::Deadline | Staged::Budget))
                    || (self == StagedRetry
                        && matches!(status, Staged::Busy | Staged::Deadline | Staged::Budget));
                if required != record.is_some() {
                    return Err(Failure::Malformed);
                }
                Reply::Staged { status, record }
            }
            _ => Reply::Flag(match integer(value)? {
                0 => false,
                1 => true,
                _ => return Err(Failure::Malformed),
            }),
        })
    }
}
