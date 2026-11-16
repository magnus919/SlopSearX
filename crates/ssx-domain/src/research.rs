//! Durable research evidence and conservative reservation accounting. No I/O or runtime execution.
use crate::{DomainError, FiniteF64, WallClockTimestamp};
use serde::{Deserialize, Serialize};
use ssx_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineCoverage {
    pub engine: String,
    pub bucket: String,
    pub status: Option<String>,
    pub result_count: u64,
    pub failure_class: Option<String>,
}
impl Default for EngineCoverage {
    fn default() -> Self {
        Self {
            engine: Default::default(),
            bucket: "not-selected".into(),
            status: Default::default(),
            result_count: Default::default(),
            failure_class: Default::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ResearchQueryAttempt {
    pub cursor: Option<String>,
    pub query_id: Option<String>,
    pub result_count: u64,
    pub error: Option<String>,
    pub state: String,
    pub attempted_at: FiniteF64,
    pub engine_coverage: Vec<EngineCoverage>,
    pub attempt_id: String,
    pub finished_at: Option<FiniteF64>,
    pub enforcement: BTreeMap<String, Value>,
    pub admitted_result_ids: Vec<String>,
    pub new_lead_ids: Vec<String>,
}
impl Default for ResearchQueryAttempt {
    fn default() -> Self {
        Self {
            cursor: Default::default(),
            query_id: Default::default(),
            result_count: Default::default(),
            error: Default::default(),
            state: "done".into(),
            attempted_at: Default::default(),
            engine_coverage: Default::default(),
            attempt_id: "".into(),
            finished_at: Default::default(),
            enforcement: Default::default(),
            admitted_result_ids: Default::default(),
            new_lead_ids: Default::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ResearchQuery {
    pub index: u64,
    pub query: String,
    pub intent: String,
    pub engines: Vec<String>,
    pub time_range: Option<String>,
    pub state: String,
    pub query_id: Option<String>,
    pub result_count: u64,
    pub cursor: Option<String>,
    pub error: Option<String>,
    pub engine_coverage: Vec<EngineCoverage>,
    pub attempts: Vec<ResearchQueryAttempt>,
    pub enforcement: BTreeMap<String, Value>,
    pub subquestion_id: Option<String>,
    pub rationale: Option<String>,
    pub parent_attempt_id: Option<String>,
    pub continuation_key: Option<String>,
    pub continuation_digest: Option<String>,
    pub requires_intent_grant: bool,
    pub planning_method: Option<String>,
    pub evidence_result_ids: Vec<String>,
    pub evidence_engines: Vec<String>,
    pub evidence_intent: Option<String>,
}
impl Default for ResearchQuery {
    fn default() -> Self {
        Self {
            index: Default::default(),
            query: Default::default(),
            intent: Default::default(),
            engines: Default::default(),
            time_range: Default::default(),
            state: "pending".into(),
            query_id: Default::default(),
            result_count: Default::default(),
            cursor: Default::default(),
            error: Default::default(),
            engine_coverage: Default::default(),
            attempts: Default::default(),
            enforcement: Default::default(),
            subquestion_id: Default::default(),
            rationale: Default::default(),
            parent_attempt_id: Default::default(),
            continuation_key: Default::default(),
            continuation_digest: Default::default(),
            requires_intent_grant: true,
            planning_method: Default::default(),
            evidence_result_ids: Default::default(),
            evidence_engines: Default::default(),
            evidence_intent: Default::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ResearchJob {
    pub job_id: String,
    pub question: String,
    pub strategy: String,
    pub state: String,
    pub queries: Vec<ResearchQuery>,
    pub warnings: Vec<String>,
    pub created_at: FiniteF64,
    pub deadline: FiniteF64,
    pub tenant: String,
    pub idempotency_key: Option<String>,
    pub workflow: BTreeMap<String, Value>,
    pub cancel_requested: bool,
    pub owner_id: Option<String>,
    pub lease_token: Option<String>,
    pub lease_expires_at: FiniteF64,
    pub subquestions: ssx_json::Map<String, BTreeMap<String, String>>,
    pub budget_limits: BTreeMap<String, u64>,
    pub budget_used: BTreeMap<String, u64>,
    pub seen_lead_ids: Vec<String>,
    pub caller_completed: bool,
    pub completion_rationale: Option<String>,
    pub completion_subquestion_states: Option<BTreeMap<String, String>>,
    pub stop_reason: Option<String>,
}
impl Default for ResearchJob {
    fn default() -> Self {
        Self {
            job_id: Default::default(),
            question: Default::default(),
            strategy: Default::default(),
            state: "queued".into(),
            queries: Default::default(),
            warnings: Default::default(),
            created_at: Default::default(),
            deadline: Default::default(),
            tenant: "default".into(),
            idempotency_key: Default::default(),
            workflow: Default::default(),
            cancel_requested: false,
            owner_id: Default::default(),
            lease_token: Default::default(),
            lease_expires_at: Default::default(),
            subquestions: Default::default(),
            budget_limits: Default::default(),
            budget_used: Default::default(),
            seen_lead_ids: Default::default(),
            caller_completed: false,
            completion_rationale: Default::default(),
            completion_subquestion_states: Default::default(),
            stop_reason: Default::default(),
        }
    }
}

/// Policy inputs are passed by the shared policy layer; this module cannot dispatch.
#[derive(Debug, Clone, Copy)]
pub struct ResearchCeilings {
    pub queries: u64,
    pub engines_per_query: u64,
    pub results: u64,
}

/// Settlement accepts source-normalized lead identities from the application layer.
/// This type checks representation, not URL normalization. The application owns that proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeadId(String);
impl LeadId {
    pub fn new(value: String) -> Result<Self, DomainError> {
        let digest = value
            .strip_prefix("lead-v1-")
            .ok_or(DomainError("invalid lead identity"))?;
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(DomainError("invalid lead identity"));
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BudgetSummary {
    pub limits: BTreeMap<String, u64>,
    pub used: BTreeMap<String, u64>,
    pub remaining: BTreeMap<String, u64>,
    pub accounting: &'static str,
}
const UNKNOWN_DISPATCH: &str =
    "Previous dispatch outcome is unknown after worker recovery; reservation retained";
const USED: [&str; 3] = ["attempts", "engine_attempts", "results"];

impl ResearchQuery {
    pub fn retryable(&self) -> bool {
        self.state == "failed"
            || (self.state == "done"
                && self.result_count == 0
                && self.error.as_deref().is_none_or(str::is_empty))
    }
}
impl ResearchJob {
    /// Source-compatible payload reader. Empty/null collection paths are normalized only
    /// at schema-declared paths, including Lua cjson's empty-table encoding.
    pub fn from_payload(mut value: Value) -> Result<Self, DomainError> {
        normalize_record(&mut value)?;
        let job: Self =
            ssx_json::from_value(value).map_err(|_| DomainError("malformed research record"))?;
        job.validate()?;
        Ok(job)
    }
    pub fn to_payload(&self) -> Result<Value, DomainError> {
        self.validate()?;
        ssx_json::to_value(self).map_err(Into::into)
    }
    pub fn validate(&self) -> Result<(), DomainError> {
        crate::JobId::new(self.job_id.clone())?;
        crate::TenantId::new(self.tenant.clone())?;
        if ![
            "queued",
            "running",
            "partial",
            "succeeded",
            "failed",
            "cancelled",
            "expired",
        ]
        .contains(&self.state.as_str())
        {
            return Err(DomainError("invalid research state"));
        }
        if self.queries.iter().any(|q| {
            !["pending", "running", "done", "failed", "cancelled"].contains(&q.state.as_str())
                || q.attempts.iter().any(|a| {
                    ![
                        "pending",
                        "running",
                        "done",
                        "failed",
                        "cancelled",
                        "expired",
                        "interrupted",
                    ]
                    .contains(&a.state.as_str())
                })
        }) {
            return Err(DomainError("invalid research query or attempt state"));
        }
        let mut ids = std::collections::BTreeSet::new();
        for query in &self.queries {
            for attempt in &query.attempts {
                if !attempt.attempt_id.is_empty() && !ids.insert(&attempt.attempt_id) {
                    return Err(DomainError("duplicate research attempt identity"));
                }
            }
        }
        Ok(())
    }
    pub fn progress(&self) -> (usize, usize) {
        (
            self.queries
                .iter()
                .filter(|q| ["done", "failed", "cancelled"].contains(&q.state.as_str()))
                .count(),
            self.queries.len(),
        )
    }
    /// Materialize implicit legacy completed evidence before retry resets or
    /// accounting initialization. The zero timestamp/empty ID mean unknown legacy
    /// identity; current snapshot/provenance is retained rather than invented.
    pub fn normalize_legacy_evidence(&mut self) {
        let explicit = self
            .queries
            .iter()
            .map(|q| q.attempts.len() as u64)
            .sum::<u64>();
        let mut accounted = self
            .budget_used
            .get("attempts")
            .copied()
            .unwrap_or(0)
            .saturating_sub(explicit);
        for q in &mut self.queries {
            if q.attempts.is_empty()
                && ["done", "failed"].contains(&q.state.as_str())
                && (self.budget_limits.is_empty() || accounted > 0)
            {
                q.attempts.push(ResearchQueryAttempt {
                    cursor: q.cursor.clone(),
                    query_id: q.query_id.clone(),
                    result_count: q.result_count,
                    error: q.error.clone(),
                    state: q.state.clone(),
                    engine_coverage: q.engine_coverage.clone(),
                    enforcement: q.enforcement.clone(),
                    ..Default::default()
                });
                accounted = accounted.saturating_sub(1);
            }
        }
    }
    pub fn reset_retryable_queries(&mut self) {
        if ["cancelled", "expired"].contains(&self.state.as_str()) {
            return;
        }
        if self.queries.iter().any(ResearchQuery::retryable) {
            self.state = "running".into();
            for q in &mut self.queries {
                if q.retryable() {
                    q.state = "pending".into();
                }
            }
        }
    }
    /// Only lowers persisted limits. Legacy work is counted even without explicit IDs.
    pub fn initialize_budget(&mut self, policy: ResearchCeilings) -> Result<(), DomainError> {
        let mut next = self.clone();
        next.initialize_budget_inner(policy)?;
        *self = next;
        Ok(())
    }
    fn initialize_budget_inner(&mut self, p: ResearchCeilings) -> Result<(), DomainError> {
        let legacy = self.budget_limits.is_empty();
        let engines = p
            .queries
            .checked_mul(p.engines_per_query)
            .ok_or(DomainError("research budget overflow"))?;
        for (key, ceiling) in [
            ("queries", p.queries),
            ("attempts", p.queries),
            ("engines_per_query", p.engines_per_query),
            ("engine_attempts", engines),
            ("results", p.results),
        ] {
            self.budget_limits
                .entry(key.into())
                .and_modify(|v| *v = (*v).min(ceiling))
                .or_insert(ceiling);
        }
        if legacy {
            let mut attempts = 0u64;
            let mut engines = 0u64;
            let mut results = 0u64;
            for q in &self.queries {
                let count = (q.attempts.len() as u64).max(u64::from(
                    ["done", "failed", "running"].contains(&q.state.as_str()),
                ));
                attempts = checked_add(attempts, count)?;
                engines = checked_add(
                    engines,
                    count
                        .checked_mul(q.engines.len() as u64)
                        .ok_or(DomainError("research budget overflow"))?,
                )?;
                results = checked_add(
                    results,
                    if q.attempts.is_empty() {
                        q.result_count
                    } else {
                        q.attempts
                            .iter()
                            .try_fold(0, |sum, a| checked_add(sum, a.result_count))?
                    },
                )?;
            }
            self.budget_used = [
                ("attempts".into(), attempts),
                ("engine_attempts".into(), engines),
                ("results".into(), results),
            ]
            .into();
        }
        for k in USED {
            self.budget_used.entry(k.into()).or_insert(0);
        }
        Ok(())
    }
    pub fn budget_summary(&self) -> BudgetSummary {
        let mut used = self.budget_used.clone();
        used.insert("queries".into(), self.queries.len() as u64);
        BudgetSummary {
            limits: self.budget_limits.clone(),
            remaining: self
                .budget_limits
                .iter()
                .filter(|(k, _)| k.as_str() != "engines_per_query")
                .map(|(k, v)| (k.clone(), v.saturating_sub(*used.get(k).unwrap_or(&0))))
                .collect(),
            used,
            accounting: "Attempts and engine slots are reserved before dispatch; admitted leads each use one slot.",
        }
    }
    /// Caller must persist the reservation under its lease before dispatch. This API
    /// performs no external work and never refunds a reservation on recovery.
    pub fn reserve_attempt(
        &mut self,
        index: usize,
        id: crate::AttemptId,
        now: WallClockTimestamp,
    ) -> Result<bool, DomainError> {
        self.validate_accounting()?;
        if self
            .queries
            .iter()
            .flat_map(|q| &q.attempts)
            .any(|a| a.attempt_id == id.as_str())
        {
            return Err(DomainError("attempt identity already used"));
        }
        let q = self
            .queries
            .get_mut(index)
            .ok_or(DomainError("unknown research query"))?;
        if q.state != "pending" || q.attempts.last().is_some_and(|a| a.state == "running") {
            return Err(DomainError(
                "reservation requires a pending query without an active attempt",
            ));
        }
        for (key, amount, reason) in [
            ("attempts", 1, "attempt_budget_exhausted"),
            (
                "engine_attempts",
                q.engines.len() as u64,
                "engine_budget_exhausted",
            ),
            ("results", 1, "result_budget_exhausted"),
        ] {
            if self.budget_used[key] > self.budget_limits[key]
                || amount > self.budget_limits[key].saturating_sub(self.budget_used[key])
            {
                self.stop_reason = Some(reason.into());
                q.state = "failed".into();
                q.error = Some(reason.into());
                return Ok(false);
            }
        }
        if q.engines.len() as u64 > self.budget_limits["engines_per_query"] {
            self.stop_reason = Some("engine_budget_exhausted".into());
            q.state = "failed".into();
            q.error = self.stop_reason.clone();
            return Ok(false);
        }
        q.query_id = None;
        q.cursor = None;
        q.error = None;
        q.result_count = 0;
        q.engine_coverage.clear();
        q.enforcement.clear();
        q.state = "running".into();
        q.attempts.push(ResearchQueryAttempt {
            attempt_id: id.as_str().into(),
            state: "running".into(),
            attempted_at: FiniteF64::new(now.seconds())?,
            ..Default::default()
        });
        *self.budget_used.get_mut("attempts").expect("validated") += 1;
        *self
            .budget_used
            .get_mut("engine_attempts")
            .expect("validated") += q.engines.len() as u64;
        Ok(true)
    }
    /// Snapshot current evidence once. Every admitted result is charged, including
    /// duplicates; only new_lead_ids is deduplicated. None means no response.
    /// expected_attempt must be captured when reserving, never reloaded from the
    /// current query by a completion callback. Orphan recovery may replace it.
    pub fn finish_attempt(
        &mut self,
        index: usize,
        expected_attempt: &crate::AttemptId,
        leads: Option<&[LeadId]>,
        now: WallClockTimestamp,
    ) -> Result<(), DomainError> {
        self.validate_accounting()?;
        let q = self
            .queries
            .get_mut(index)
            .ok_or(DomainError("unknown research query"))?;
        let a = q
            .attempts
            .last_mut()
            .ok_or(DomainError("no running research attempt"))?;
        if a.attempt_id != expected_attempt.as_str() {
            return Err(DomainError(
                "research attempt identity changed before settlement",
            ));
        }
        if a.state != "running" {
            return Err(DomainError("only a running attempt may be finalized"));
        }
        if q.state == "running" || !["done", "failed", "cancelled"].contains(&q.state.as_str()) {
            return Err(DomainError("settlement requires a terminal query outcome"));
        }
        a.finished_at = Some(FiniteF64::new(now.seconds())?);
        a.state = q.state.clone();
        a.cursor = q.cursor.clone();
        a.query_id = q.query_id.clone();
        a.result_count = q.result_count;
        a.error = q.error.clone();
        a.engine_coverage = q.engine_coverage.clone();
        a.enforcement = q.enforcement.clone();
        if let (Some(leads), Some(cursor)) =
            (leads, q.cursor.as_ref().filter(|cursor| !cursor.is_empty()))
        {
            let available =
                self.budget_limits["results"].saturating_sub(self.budget_used["results"]);
            if leads.len() as u64 > available {
                self.stop_reason = Some("result_budget_exhausted".into());
            }
            let admitted = leads
                .len()
                .min(usize::try_from(available).unwrap_or(usize::MAX));
            for (index, lead) in leads.iter().take(admitted).enumerate() {
                a.admitted_result_ids.push(format!("{cursor}:{index}"));
                if !self.seen_lead_ids.contains(&lead.0) {
                    self.seen_lead_ids.push(lead.0.clone());
                    a.new_lead_ids.push(lead.0.clone());
                }
            }
            *self.budget_used.get_mut("results").expect("validated") += admitted as u64;
        }
        Ok(())
    }
    pub fn recover_orphan_attempts(
        &mut self,
        now: WallClockTimestamp,
        mut next_id: impl FnMut() -> crate::AttemptId,
    ) -> Result<(), DomainError> {
        for q in &mut self.queries {
            if q.state == "running"
                && self.budget_limits.is_empty()
                && q.attempts.last().is_none_or(|a| a.state != "running")
            {
                q.attempts.push(ResearchQueryAttempt {
                    attempt_id: next_id().as_str().into(),
                    state: "running".into(),
                    attempted_at: FiniteF64::new(now.seconds())?,
                    ..Default::default()
                });
            }
            interrupt(q, now);
        }
        Ok(())
    }
    fn validate_evidence_charges(&self) -> Result<(), DomainError> {
        if self.budget_limits.is_empty() {
            return Ok(());
        }
        let mut attempts = 0u64;
        let mut engines = 0u64;
        let mut results = 0u64;
        for query in &self.queries {
            attempts = checked_add(attempts, query.attempts.len() as u64)?;
            engines = checked_add(
                engines,
                (query.attempts.len() as u64)
                    .checked_mul(query.engines.len() as u64)
                    .ok_or(DomainError("research budget overflow"))?,
            )?;
            for attempt in &query.attempts {
                results = checked_add(results, attempt.admitted_result_ids.len() as u64)?;
            }
        }
        for (key, evidence) in [
            ("attempts", attempts),
            ("engine_attempts", engines),
            ("results", results),
        ] {
            if self.budget_used.get(key).copied().unwrap_or(0) < evidence {
                return Err(DomainError("research evidence lacks conserved charges"));
            }
        }
        Ok(())
    }
    fn validate_accounting(&self) -> Result<(), DomainError> {
        if USED
            .iter()
            .any(|k| !self.budget_used.contains_key(*k) || !self.budget_limits.contains_key(*k))
            || !self.budget_limits.contains_key("engines_per_query")
        {
            return Err(DomainError("research budget not initialized"));
        }
        Ok(())
    }
    /// Compare an authoritative previous record before a fenced write. Evidence
    /// remains immutable independently of mutable owner/lease fields.
    pub fn validate_transition(&self, next: &Self) -> Result<(), DomainError> {
        self.validate()?;
        next.validate()?;
        if self.job_id != next.job_id
            || self.tenant != next.tenant
            || self.idempotency_key != next.idempotency_key
        {
            return Err(DomainError("research authority changed"));
        }
        if ["cancelled", "expired"].contains(&self.state.as_str()) && next.state != self.state {
            return Err(DomainError("terminal research state cannot resurrect"));
        }
        if self.caller_completed && !next.caller_completed {
            return Err(DomainError("caller completion cannot be revoked"));
        }
        if self.cancel_requested && !next.cancel_requested {
            return Err(DomainError("research cancellation cannot be revoked"));
        }
        if self.question != next.question
            || self.strategy != next.strategy
            || self.created_at != next.created_at
            || self.deadline != next.deadline
        {
            return Err(DomainError("research origin or deadline changed"));
        }
        for (key, old) in &self.budget_limits {
            if next.budget_limits.get(key).is_none_or(|v| v > old) {
                return Err(DomainError("research ceiling increased"));
            }
        }
        for (key, old) in &self.budget_used {
            if next.budget_used.get(key).is_none_or(|v| v < old) {
                return Err(DomainError("research reservation refunded"));
            }
        }
        if !next.seen_lead_ids.starts_with(&self.seen_lead_ids) {
            return Err(DomainError("research lead history changed"));
        }
        if next.queries.len() < self.queries.len() {
            return Err(DomainError("research query history removed"));
        }
        if next.queries.len() > self.queries.len()
            && next
                .budget_limits
                .get("queries")
                .is_none_or(|cap| next.queries.len() as u64 > *cap)
        {
            return Err(DomainError("research added queries exceed ceiling"));
        }
        let mut known_leads: std::collections::BTreeSet<&str> =
            self.seen_lead_ids.iter().map(String::as_str).collect();
        let mut added_leads = std::collections::BTreeSet::new();
        let mut added_attempts = 0u64;
        let mut added_engines = 0u64;
        let mut added_results = 0u64;
        let mut legacy_attempts = 0u64;
        let mut legacy_engines = 0u64;
        let mut legacy_results = 0u64;
        for (index, new) in next.queries.iter().enumerate() {
            let old = self.queries.get(index);
            let prior = old.map_or(0, |q| q.attempts.len());
            let completed_legacy = old.is_some_and(|q| {
                q.attempts.is_empty() && ["done", "failed"].contains(&q.state.as_str())
            });
            let materialized_legacy = old.zip(new.attempts.first()).is_some_and(|(q, a)| {
                completed_legacy
                    && a.attempt_id.is_empty()
                    && a.attempted_at.get() == 0.0
                    && a.finished_at.is_none()
                    && a.state == q.state
                    && a.cursor == q.cursor
                    && a.query_id == q.query_id
                    && a.result_count == q.result_count
                    && a.error == q.error
                    && a.engine_coverage == q.engine_coverage
                    && a.enforcement == q.enforcement
                    && a.admitted_result_ids.is_empty()
                    && a.new_lead_ids.is_empty()
            });
            if completed_legacy
                && !materialized_legacy
                && old.is_some_and(|q| {
                    q.state != new.state
                        || q.cursor != new.cursor
                        || q.query_id != new.query_id
                        || q.result_count != new.result_count
                        || q.error != new.error
                        || q.engine_coverage != new.engine_coverage
                        || q.enforcement != new.enforcement
                        || !new.attempts.is_empty()
                        || self.budget_limits.is_empty() && !next.budget_limits.is_empty()
                })
            {
                // Failed budget admission without any reserved work has no
                // implicit provider attempt. Pre-ledger records do.
                let charged = self.budget_limits.is_empty()
                    || self.budget_used.get("attempts").copied().unwrap_or(0)
                        > self
                            .queries
                            .iter()
                            .map(|q| q.attempts.len() as u64)
                            .sum::<u64>();
                if charged {
                    return Err(DomainError(
                        "legacy completed evidence must be materialized before mutation",
                    ));
                }
            }
            let mut count = new.attempts.len().saturating_sub(prior) as u64;
            // Source pre-ledger orphan recovery synthesizes one interrupted
            // historical attempt, not a new dispatch reservation.
            let recovered_legacy = self.budget_limits.is_empty()
                && old.is_some_and(|q| {
                    q.state == "running" && q.attempts.last().is_none_or(|a| a.state != "running")
                })
                && new.attempts.get(prior).is_some_and(|a| {
                    a.state == "interrupted"
                        && a.error.as_deref() == Some(UNKNOWN_DISPATCH)
                        && a.admitted_result_ids.is_empty()
                        && a.new_lead_ids.is_empty()
                        && a.result_count == 0
                        && a.finished_at.is_some()
                });
            if recovered_legacy || materialized_legacy {
                count = count.saturating_sub(1);
            }
            for (i, attempt) in new.attempts.iter().enumerate().skip(prior) {
                if !(materialized_legacy && i == 0) {
                    crate::AttemptId::new(attempt.attempt_id.clone())?;
                }
            }

            if let Some(old) = old {
                let consumed = (old.attempts.len() as u64).max(u64::from(
                    ["done", "failed", "running"].contains(&old.state.as_str()),
                ));
                legacy_attempts = checked_add(legacy_attempts, consumed)?;
                legacy_engines = checked_add(
                    legacy_engines,
                    consumed
                        .checked_mul(old.engines.len() as u64)
                        .ok_or(DomainError("research budget overflow"))?,
                )?;
                let results = if old.attempts.is_empty() {
                    if recovered_legacy {
                        0
                    } else {
                        old.result_count
                    }
                } else {
                    old.attempts
                        .iter()
                        .try_fold(0, |sum, a| checked_add(sum, a.result_count))?
                };
                legacy_results = checked_add(legacy_results, results)?;
            }
            if count > 0
                && next
                    .budget_limits
                    .get("engines_per_query")
                    .is_none_or(|cap| new.engines.len() as u64 > *cap)
            {
                return Err(DomainError("research added engine scope exceeds ceiling"));
            }
            added_attempts = checked_add(added_attempts, count)?;
            added_engines = checked_add(
                added_engines,
                count
                    .checked_mul(new.engines.len() as u64)
                    .ok_or(DomainError("research budget overflow"))?,
            )?;
            for (i, attempt) in new.attempts.iter().enumerate() {
                let before = old.and_then(|q| q.attempts.get(i));
                if let Some(before) = before
                    && (!attempt
                        .admitted_result_ids
                        .starts_with(&before.admitted_result_ids)
                        || !attempt.new_lead_ids.starts_with(&before.new_lead_ids))
                {
                    return Err(DomainError("research admitted history changed"));
                }
                let prior_admissions = before.map_or(0, |a| a.admitted_result_ids.len());
                let prior_leads = before.map_or(0, |a| a.new_lead_ids.len());
                let admissions = attempt
                    .admitted_result_ids
                    .len()
                    .saturating_sub(prior_admissions);
                let leads = attempt.new_lead_ids.len().saturating_sub(prior_leads);
                if leads > admissions {
                    return Err(DomainError("new research leads lack result admissions"));
                }
                if admissions > 0 {
                    if attempt.state == "running" {
                        return Err(DomainError(
                            "running research evidence cannot admit results",
                        ));
                    }
                    let cursor = attempt
                        .cursor
                        .as_ref()
                        .filter(|cursor| !cursor.is_empty())
                        .ok_or(DomainError("research admissions require snapshot cursor"))?;
                    for (i, result) in attempt
                        .admitted_result_ids
                        .iter()
                        .enumerate()
                        .skip(prior_admissions)
                    {
                        if *result != format!("{cursor}:{i}") {
                            return Err(DomainError("research result admission identity changed"));
                        }
                    }
                }
                for lead in attempt.new_lead_ids.iter().skip(prior_leads) {
                    LeadId::new(lead.clone())?;
                    if !known_leads.insert(lead.as_str()) {
                        return Err(DomainError("research new lead was already seen"));
                    }
                    added_leads.insert(lead.as_str());
                }
                added_results = checked_add(added_results, admissions as u64)?;
            }
        }
        if added_attempts > 0 && (self.cancel_requested || next.cancel_requested) {
            return Err(DomainError(
                "cancelled research cannot reserve new attempts",
            ));
        }
        let seen_suffix = &next.seen_lead_ids[self.seen_lead_ids.len()..];
        let suffix: std::collections::BTreeSet<&str> =
            seen_suffix.iter().map(String::as_str).collect();
        if suffix.len() != seen_suffix.len() || suffix != added_leads {
            return Err(DomainError(
                "research seen history lacks matching admitted lead evidence",
            ));
        }
        if !self.budget_limits.is_empty() {
            next.validate_accounting()?;
            for (key, delta) in [
                ("attempts", added_attempts),
                ("engine_attempts", added_engines),
                ("results", added_results),
            ] {
                let old = self.budget_used.get(key).copied().unwrap_or(0);
                let new = next.budget_used[key];
                if new < checked_add(old, delta)? {
                    return Err(DomainError("research work was not charged"));
                }
                if new > old && new > next.budget_limits[key] {
                    return Err(DomainError("research new work exceeds ceiling"));
                }
            }
        }
        if self.budget_limits.is_empty()
            && (!next.budget_limits.is_empty() || added_attempts > 0 || added_results > 0)
        {
            next.validate_accounting()?;
            for (key, consumed, delta) in [
                ("attempts", legacy_attempts, added_attempts),
                ("engine_attempts", legacy_engines, added_engines),
                ("results", legacy_results, added_results),
            ] {
                if next.budget_used[key] < checked_add(consumed, delta)? {
                    return Err(DomainError("research legacy work was not charged"));
                }
                if delta > 0 && next.budget_used[key] > next.budget_limits[key] {
                    return Err(DomainError("research new work exceeds ceiling"));
                }
            }
        }
        next.validate_evidence_charges()?;
        for (old, new) in self.queries.iter().zip(&next.queries) {
            if old.index != new.index
                || old.query != new.query
                || old.intent != new.intent
                || old.engines != new.engines
                || old.time_range != new.time_range
                || old.subquestion_id != new.subquestion_id
                || old.rationale != new.rationale
                || old.parent_attempt_id != new.parent_attempt_id
                || old.continuation_key != new.continuation_key
                || old.continuation_digest != new.continuation_digest
                || old.requires_intent_grant != new.requires_intent_grant
                || old.planning_method != new.planning_method
                || old.evidence_result_ids != new.evidence_result_ids
                || old.evidence_engines != new.evidence_engines
                || old.evidence_intent != new.evidence_intent
            {
                return Err(DomainError("research query identity changed"));
            }
            if new.attempts.len() < old.attempts.len() {
                return Err(DomainError("research attempt history removed"));
            }
            for (before, after) in old.attempts.iter().zip(&new.attempts) {
                if before.state != "running" && before != after {
                    return Err(DomainError("completed research evidence changed"));
                }
                if before.state == "running"
                    && (before.attempt_id != after.attempt_id
                        || before.attempted_at != after.attempted_at)
                {
                    return Err(DomainError("running research identity changed"));
                }
            }
        }
        Ok(())
    }
}
fn checked_add(a: u64, b: u64) -> Result<u64, DomainError> {
    a.checked_add(b)
        .ok_or(DomainError("research budget overflow"))
}
fn interrupt(q: &mut ResearchQuery, now: WallClockTimestamp) {
    if let Some(a) = q.attempts.last_mut().filter(|a| a.state == "running") {
        a.state = "interrupted".into();
        a.finished_at = Some(FiniteF64::new(now.seconds()).expect("finite wall time"));
        a.error = Some(UNKNOWN_DISPATCH.into());
    }
}
fn normalize_record(v: &mut Value) -> Result<(), DomainError> {
    let o = v
        .as_object_mut()
        .ok_or(DomainError("research record must be object"))?;
    numeric_fields(o, &[], &["created_at", "deadline", "lease_expires_at"])?;
    normalize_paths(
        o,
        &["queries", "warnings", "seen_lead_ids"],
        &["workflow", "subquestions", "budget_limits", "budget_used"],
    )?;
    if let Some(Value::Array(queries)) = o.get_mut("queries") {
        for q in queries {
            let q = q
                .as_object_mut()
                .ok_or(DomainError("research query must be object"))?;
            numeric_fields(q, &["index", "result_count"], &[])?;
            coverage_fields(q)?;
            normalize_paths(
                q,
                &[
                    "engines",
                    "engine_coverage",
                    "attempts",
                    "evidence_result_ids",
                    "evidence_engines",
                ],
                &["enforcement"],
            )?;
            if let Some(Value::Array(attempts)) = q.get_mut("attempts") {
                for a in attempts {
                    let a = a
                        .as_object_mut()
                        .ok_or(DomainError("research attempt must be object"))?;
                    numeric_fields(a, &["result_count"], &["attempted_at"])?;
                    coverage_fields(a)?;
                    normalize_paths(
                        a,
                        &["engine_coverage", "admitted_result_ids", "new_lead_ids"],
                        &["enforcement"],
                    )?;
                }
            }
        }
    }
    Ok(())
}
fn normalize_paths(
    o: &mut ssx_json::Map<String, Value>,
    arrays: &[&str],
    objects: &[&str],
) -> Result<(), DomainError> {
    for key in arrays {
        if let Some(v) = o.get_mut(*key) {
            if v.is_null() || v.as_object().is_some_and(|m| m.is_empty()) {
                *v = Value::Array(vec![]);
            }
            if !v.is_array() {
                return Err(DomainError("malformed research array"));
            }
        }
    }
    for key in objects {
        if let Some(v) = o.get_mut(*key) {
            if v.is_null() || v.as_array().is_some_and(Vec::is_empty) {
                *v = ssx_json::json!({});
            }
            if !v.is_object() {
                return Err(DomainError("malformed research object"));
            }
        }
    }
    Ok(())
}

fn numeric_fields(
    o: &mut ssx_json::Map<String, Value>,
    integers: &[&str],
    floats: &[&str],
) -> Result<(), DomainError> {
    for key in integers {
        if let Some(v) = o.get_mut(*key) {
            let number = match v {
                Value::String(s) => s.trim().parse::<u64>().ok(),
                Value::Bool(b) => Some(u64::from(*b)),
                Value::Number(n) => n.as_u64().or_else(|| {
                    n.as_f64()
                        .filter(|v| *v >= 0.0 && *v < u64::MAX as f64)
                        .map(|v| v.trunc() as u64)
                }),
                _ => None,
            }
            .ok_or(DomainError("invalid research count"))?;
            *v = ssx_json::to_value(number)?;
        }
    }
    for key in floats {
        if let Some(v) = o.get_mut(*key) {
            let n = match v {
                Value::String(s) => s.trim().parse::<f64>().ok(),
                Value::Bool(b) => Some(f64::from(u8::from(*b))),
                Value::Number(n) => n.as_f64(),
                _ => None,
            }
            .ok_or(DomainError("invalid research time"))?;
            *v = ssx_json::to_value(FiniteF64::new(n)?)?;
        }
    }
    Ok(())
}
fn coverage_fields(o: &mut ssx_json::Map<String, Value>) -> Result<(), DomainError> {
    if let Some(Value::Array(entries)) = o.get_mut("engine_coverage") {
        for entry in entries {
            numeric_fields(
                entry
                    .as_object_mut()
                    .ok_or(DomainError("invalid research coverage"))?,
                &["result_count"],
                &[],
            )?;
        }
    }
    Ok(())
}
/// Disjoint source coverage classification. Credential absence is a derived
/// auth_required token and takes precedence over a dispatched engine status.
pub fn classify_coverage(
    engine: String,
    dispatched: bool,
    status: Option<&str>,
    result_count: u64,
    credential_missing: bool,
) -> EngineCoverage {
    if !dispatched {
        return EngineCoverage {
            engine,
            ..Default::default()
        };
    }
    if credential_missing {
        return EngineCoverage {
            engine,
            bucket: "unavailable".into(),
            status: status.map(Into::into),
            result_count,
            failure_class: Some("auth_required".into()),
        };
    }
    let status = status.filter(|s| !s.is_empty()).unwrap_or("error");
    let bucket = if status == "ok" {
        if result_count > 0 {
            "successful"
        } else {
            "empty"
        }
    } else if status == "unavailable" {
        "unavailable"
    } else {
        "failed"
    };
    EngineCoverage {
        engine,
        bucket: bucket.into(),
        status: Some(status.into()),
        result_count,
        failure_class: Some(status.into()),
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CoverageSummary {
    pub attempted: u64,
    pub successful: u64,
    pub empty: u64,
    pub failed: u64,
    pub unavailable: u64,
    pub not_selected: u64,
}
pub fn summarize_coverage(coverage: &[EngineCoverage]) -> Result<CoverageSummary, DomainError> {
    let mut summary = CoverageSummary::default();
    for entry in coverage {
        match entry.bucket.as_str() {
            "successful" => summary.successful += 1,
            "empty" => summary.empty += 1,
            "failed" => summary.failed += 1,
            "unavailable" => summary.unavailable += 1,
            "not-selected" => summary.not_selected += 1,
            _ => return Err(DomainError("unknown coverage bucket")),
        }
    }
    summary.attempted = coverage.len() as u64 - summary.not_selected;
    Ok(summary)
}
