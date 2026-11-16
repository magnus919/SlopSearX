//! Staged v1 durable records. JSON extras are preserved; only declared arrays normalize.
use crate::{DomainError, StagedSearchId, TenantId, canonical::plan_digest};
use ssx_json::{Value, json};

#[derive(Debug, Clone, PartialEq)]
pub struct Record(Value);
fn invalid() -> DomainError {
    DomainError("invalid staged record")
}
fn array(v: &mut Value) {
    if v.as_object().is_some_and(|m| m.is_empty()) {
        *v = json!([]);
    }
}
fn enforcement(v: &mut Value) {
    if let Some(m) = v.as_object_mut() {
        for entry in m.values_mut() {
            if let Some(m) = entry.as_object_mut()
                && let Some(v) = m.get_mut("enforced_by")
            {
                array(v);
            }
        }
    }
}
fn scope(v: &mut Value) {
    if let Some(m) = v.as_object_mut()
        && let Some(v) = m.get_mut("excluded_engines")
    {
        array(v);
    }
}
fn normalize(v: &mut Value) {
    if let Some(m) = v.as_object_mut()
        && let Some(v) = m.get_mut("retry_keys")
    {
        array(v);
    }
    array(&mut v["objectives"]["unmet"]);
    scope(&mut v["plan"]["initial_scope"]);
    if !v["plan"]["fallback_scope"].is_null() {
        scope(&mut v["plan"]["fallback_scope"]);
    }
    if let Some(stages) = v["stages"].as_array_mut() {
        for stage in stages {
            scope(&mut stage["scope"]);
            enforcement(&mut stage["enforcement"]);
            array(&mut stage["attempts"]);
            if let Some(attempts) = stage["attempts"].as_array_mut() {
                for attempt in attempts {
                    array(&mut attempt["engine_outcomes"]);
                    enforcement(&mut attempt["enforcement"]);
                }
            }
        }
    }
}
fn count(v: &Value) -> Result<u64, DomainError> {
    v.as_u64().ok_or_else(invalid)
}
fn time(v: &Value) -> Result<f64, DomainError> {
    v.as_f64().filter(|n| n.is_finite()).ok_or_else(invalid)
}
fn text(v: &Value) -> Result<&str, DomainError> {
    v.as_str().filter(|s| !s.is_empty()).ok_or_else(invalid)
}
/// Identity uses the exact accepted Python ASCII compact JSON profile from F05.
pub fn identity(v: &Value) -> Value {
    json!({"version":v["version"],"query":v["plan"]["query"],
        "objectives":{"requested":v["objectives"]["requested"],"effective":v["objectives"]["effective"]},
        "initial_scope":v["plan"]["initial_scope"],"fallback_scope":v["plan"]["fallback_scope"],
        "allow_scope_expansion":v["plan"]["allow_scope_expansion"],"filters":v["plan"]["filters"]})
}
impl Record {
    pub fn decode(bytes: &[u8]) -> Result<Self, DomainError> {
        let value: Value = ssx_json::from_slice(bytes)?;
        Self::from_value(value)
    }
    pub fn from_value(mut v: Value) -> Result<Self, DomainError> {
        // Never index malformed scalar containers with IndexMut (which can panic).
        if !v.is_object()
            || !v["plan"].is_object()
            || !v["objectives"].is_object()
            || !v["budget"].is_object()
            || !v["stages"].is_array()
        {
            return Err(invalid());
        }
        if v["stages"].as_array().unwrap().iter().any(|s| {
            !s.is_object()
                || !s["scope"].is_object()
                || !(s["attempts"].is_array()
                    || s["attempts"].as_object().is_some_and(|m| m.is_empty()))
        }) {
            return Err(invalid());
        }
        for stage in v["stages"].as_array().unwrap() {
            if stage["attempts"]
                .as_array()
                .is_some_and(|a| a.iter().any(|v| !v.is_object()))
            {
                return Err(invalid());
            }
        }
        normalize(&mut v);
        TenantId::new(text(&v["tenant"])?)?;
        StagedSearchId::new(text(&v["operation_id"])?)?;
        if v["version"] != json!(1)
            || !matches!(
                text(&v["state"])?,
                "queued" | "running" | "failed" | "interrupted" | "completed"
            )
            || v["plan"]["fallback_when"] != json!("clean_empty")
            || !v["plan"]["filters"].is_object()
            || v["plan"]["allow_scope_expansion"].as_bool().is_none()
        {
            return Err(invalid());
        }
        text(&v["plan"]["query"])?;
        let accepted = time(&v["accepted_at"])?;
        if time(&v["expires_at"])? <= accepted || time(&v["execution_deadline_at"])? < accepted {
            return Err(invalid());
        }
        let limit = count(&v["budget"]["limit"])?;
        let reserved = count(&v["budget"]["reserved"])?;
        if limit == 0
            || limit > 64
            || reserved > limit
            || v["budget"]["unit"] != json!("adapter_search_invocations")
        {
            return Err(invalid());
        }
        for kind in ["requested", "effective"] {
            if !(1..=30000).contains(&count(&v["objectives"][kind]["deadline_ms"])?)
                || !(1..=64).contains(&count(&v["objectives"][kind]["max_engine_calls"])?)
            {
                return Err(invalid());
            }
        }
        if count(&v["objectives"]["effective"]["max_engine_calls"])? != limit {
            return Err(invalid());
        }
        let stages = v["stages"].as_array().ok_or_else(invalid)?;
        if !(1..=2).contains(&stages.len())
            || count(&v["next_stage"])? as usize >= stages.len()
            || (stages.len() == 1) != v["plan"]["fallback_scope"].is_null()
            || v["plan"]["allow_scope_expansion"] != json!(stages.len() == 2)
        {
            return Err(invalid());
        }
        let mut engines = std::collections::BTreeSet::new();
        let mut attempt_ids = std::collections::BTreeSet::new();
        let mut running = 0usize;
        let current_index = count(&v["next_stage"])? as usize;
        let mut sum = 0u64;
        let mut observed = Some(0u64);
        for (index, stage) in stages.iter().enumerate() {
            if count(&stage["index"])? != index as u64
                || !stage["enforcement"].is_object()
                || !matches!(
                    text(&stage["state"])?,
                    "pending" | "running" | "failed" | "interrupted" | "done" | "skipped"
                )
            {
                return Err(invalid());
            }
            let expected = if index == 0 {
                &v["plan"]["initial_scope"]
            } else {
                &v["plan"]["fallback_scope"]
            };
            if &stage["scope"] != expected || !stage["scope"]["excluded_engines"].is_array() {
                return Err(invalid());
            }
            for entry in stage["enforcement"].as_object().unwrap().values() {
                if !entry.is_object() || !entry["enforced_by"].is_array() {
                    return Err(invalid());
                }
            }
            let selected = stage["scope"]["selected_engines"]
                .as_array()
                .ok_or_else(invalid)?;
            if selected.is_empty() {
                return Err(invalid());
            }
            for engine in selected {
                if !engines.insert(text(engine)?) {
                    return Err(invalid());
                }
            }
            for attempt in stage["attempts"].as_array().ok_or_else(invalid)? {
                if !attempt.is_object()
                    || !attempt["engine_outcomes"].is_array()
                    || !attempt["enforcement"].is_object()
                    || !matches!(
                        text(&attempt["state"])?,
                        "running" | "done" | "failed" | "interrupted"
                    )
                {
                    return Err(invalid());
                }
                if !attempt_ids.insert(text(&attempt["attempt_id"])?) {
                    return Err(invalid());
                }
                if attempt["state"] == json!("running") {
                    running += 1;
                    if index != current_index
                        || v["state"] != json!("running")
                        || stage["state"] != json!("running")
                        || !attempt["finished_at"].is_null()
                    {
                        return Err(invalid());
                    }
                } else if attempt["finished_at"].is_null() {
                    return Err(invalid());
                }
                if attempt["enforcement"] != stage["enforcement"] {
                    return Err(invalid());
                }
                time(&attempt["started_at"])?;
                let cost = count(&attempt["reserved_engine_calls"])?;
                if cost != selected.len() as u64 {
                    return Err(invalid());
                }
                sum = sum.checked_add(cost).ok_or_else(invalid)?;
                if attempt["observed_engine_calls"].is_null() {
                    if attempt["state"] != json!("running") {
                        observed = None;
                    }
                } else {
                    let n = count(&attempt["observed_engine_calls"])?;
                    if n > cost {
                        return Err(invalid());
                    }
                    observed = observed.and_then(|total| total.checked_add(n));
                }
            }
        }
        let current = &stages[current_index];
        if (v["state"] == json!("running")
            && (running != 1
                || current["attempts"]
                    .as_array()
                    .unwrap()
                    .last()
                    .map(|a| &a["state"])
                    != Some(&json!("running"))))
            || (v["state"] == json!("completed") && current["state"] != json!("done"))
            || (v["state"] == json!("interrupted") && current["state"] != json!("interrupted"))
            || (current["state"] == json!("done")
                && current["attempts"]
                    .as_array()
                    .unwrap()
                    .last()
                    .map(|a| &a["state"])
                    != Some(&json!("done")))
        {
            return Err(invalid());
        }
        if !v["selected_result_attempt_id"].is_null() {
            let selected = text(&v["selected_result_attempt_id"])?;
            if !stages
                .iter()
                .flat_map(|s| s["attempts"].as_array().unwrap())
                .any(|a| {
                    a["attempt_id"].as_str() == Some(selected)
                        && a["cursor"].as_str().is_some_and(|s| !s.is_empty())
                })
            {
                return Err(invalid());
            }
        }
        if sum != reserved || engines.len() as u64 > limit {
            return Err(invalid());
        }
        if !v["budget"]["observed"].is_null() && Some(count(&v["budget"]["observed"])?) != observed
        {
            return Err(invalid());
        }
        if !v["retry_keys"]
            .as_array()
            .is_some_and(|a| a.iter().all(|v| v.as_str().is_some()))
            || !v["objectives"]["unmet"].is_array()
        {
            return Err(invalid());
        }
        if plan_digest(&identity(&v))? != text(&v["plan_digest"])? {
            return Err(DomainError("staged plan digest mismatch"));
        }
        Ok(Self(v))
    }
    pub fn value(&self) -> &Value {
        &self.0
    }
    pub fn tenant(&self) -> TenantId {
        TenantId::new(self.0["tenant"].as_str().unwrap()).unwrap()
    }
    pub fn id(&self) -> StagedSearchId {
        StagedSearchId::new(self.0["operation_id"].as_str().unwrap()).unwrap()
    }
    pub fn state(&self) -> &str {
        self.0["state"].as_str().unwrap()
    }
    pub fn expires_at(&self) -> f64 {
        self.0["expires_at"].as_f64().unwrap()
    }
    pub fn encode(&self) -> Result<Vec<u8>, DomainError> {
        Ok(ssx_json::to_vec(&self.0)?)
    }
    /// Fenced completion may alter only current attempt/result state. All earlier
    /// attempts, scopes, identity, timestamps and charged reservations stay immutable.
    pub fn validate_completion(&self, next: &Record) -> Result<(), DomainError> {
        if self.0["budget"]["observed"].is_null() && !next.0["budget"]["observed"].is_null() {
            return Err(invalid());
        }
        let index = count(&self.0["next_stage"])? as usize;
        let next_index = count(&next.0["next_stage"])? as usize;
        let stages = next.0["stages"].as_array().ok_or_else(invalid)?;
        let stage = stages.get(index).ok_or_else(invalid)?;
        let attempt = stage["attempts"]
            .as_array()
            .and_then(|a| a.last())
            .ok_or_else(invalid)?;
        if self.state() != "running"
            || !matches!(next.state(), "completed" | "failed" | "queued")
            || attempt["finished_at"].is_null()
            || !matches!(text(&attempt["state"])?, "done" | "failed")
            || stage["state"] != attempt["state"]
            || (next.state() == "failed" && attempt["state"] != json!("failed"))
            || (next.state() != "failed" && attempt["state"] != json!("done"))
            || (next.state() == "queued" && next_index != index + 1)
            || (next.state() != "queued" && next_index != index)
        {
            return Err(invalid());
        }
        if !next.0["owner_token"].is_null() && next.0["owner_token"] != self.0["owner_token"] {
            return Err(invalid());
        }
        if !self.0["selected_result_attempt_id"].is_null()
            && next.0["selected_result_attempt_id"].is_null()
        {
            return Err(invalid());
        }
        if let Some(selected) = next.0["selected_result_attempt_id"].as_str() {
            let old = self.0["selected_result_attempt_id"].as_str();
            if Some(selected) != old && selected != text(&attempt["attempt_id"])? {
                return Err(invalid());
            }
        }
        // Completion must conserve the aggregate derivable from every attempt.
        // A legacy read may contain an unknown aggregate, but a new completion
        // cannot erase a known total or invent observations for an unknown one.
        let observed = if self.0["budget"]["observed"].is_null() {
            // Legacy unknown accounting remains conservative across completion.
            None
        } else {
            stages
                .iter()
                .flat_map(|stage| stage["attempts"].as_array().unwrap())
                .try_fold(0u64, |total, attempt| {
                    total.checked_add(attempt["observed_engine_calls"].as_u64()?)
                })
        };
        if next.0["budget"]["observed"].as_u64() != observed {
            return Err(invalid());
        }
        let mut before = self.0.clone();
        let mut after = next.0.clone();
        // Only unused pending stages may become skipped after initial success.
        for (i, following) in stages.iter().enumerate().skip(index + 1) {
            if following["state"] != self.0["stages"][i]["state"]
                || following["skipped_reason"] != self.0["stages"][i]["skipped_reason"]
            {
                if next.state() != "completed"
                    || self.0["stages"][i]["state"] != json!("pending")
                    || following["state"] != json!("skipped")
                    || following["skipped_reason"] != json!("initial_nonempty")
                    || !following["attempts"]
                        .as_array()
                        .is_some_and(|a| a.is_empty())
                {
                    return Err(invalid());
                }
                for value in [&mut before, &mut after] {
                    let stage = &mut value["stages"].as_array_mut().ok_or_else(invalid)?[i];
                    stage["state"] = Value::Null;
                    stage["skipped_reason"] = Value::Null;
                }
            }
        }
        for value in [&mut before, &mut after] {
            for key in [
                "state",
                "stop_reason",
                "owner_token",
                "next_stage",
                "selected_result_attempt_id",
                "error",
            ] {
                value.as_object_mut().ok_or_else(invalid)?.shift_remove(key);
            }
            value["objectives"]["unmet"] = json!([]);
            value["budget"]["observed"] = Value::Null;
            let stage = &mut value["stages"].as_array_mut().ok_or_else(invalid)?[index];
            stage["state"] = Value::Null;
            let last = stage["attempts"]
                .as_array_mut()
                .and_then(|a| a.last_mut())
                .ok_or_else(invalid)?;
            for key in [
                "state",
                "finished_at",
                "observed_engine_calls",
                "cached",
                "query_id",
                "cursor",
                "result_count",
                "engine_outcomes",
                "error",
                "deadline_exceeded",
            ] {
                last.as_object_mut().ok_or_else(invalid)?.shift_remove(key);
            }
        }
        if before != after {
            return Err(DomainError("immutable staged history changed"));
        }
        Ok(())
    }
}
