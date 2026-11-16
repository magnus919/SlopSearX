//! Bounded staged planning and deterministic completion. Persisted plans are data,
//! never dispatch authority; every stage is reauthorized by the composition root.
use crate::policy::{PolicyService, ScopeProposal, ValidatedScope};
use ssx_domain::{
    DomainError, SearchRequest, SearchResponse, StagedSearchId, TenantId, canonical::plan_digest,
    staged::Record,
};
use ssx_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub code: &'static str,
    pub field: String,
    pub message: String,
    pub details: Box<Value>,
}
impl Error {
    pub fn new(code: &'static str, field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code,
            field: field.into(),
            message: message.into(),
            details: Box::new(json!({})),
        }
    }
    pub fn value(&self) -> Value {
        {
            let mut error = json!({"code":self.code,"message":self.message});
            if !self.field.is_empty() {
                error["field"] = json!(self.field);
            }
            if let Some(m) = self.details.as_object() {
                for (k, v) in m {
                    error[k.as_str()] = v.clone();
                }
            }
            json!({"error":error})
        }
    }
}
impl From<DomainError> for Error {
    fn from(e: DomainError) -> Self {
        Self::new("invalid_input", "", e.to_string())
    }
}
#[derive(Debug, Clone)]
pub struct Limits {
    pub granted: bool,
    pub deadline_ms: u64,
    pub engine_calls: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            granted: false,
            deadline_ms: 30000,
            engine_calls: 64,
        }
    }
}
#[derive(Debug, Clone)]
pub struct Input {
    pub query: String,
    pub objectives: Value,
    pub initial_scope: Value,
    pub fallback_scope: Option<Value>,
    pub allow_scope_expansion: bool,
    pub filters: Option<Value>,
}
pub fn disabled() -> Error {
    let mut error = Error::new("tool_disabled", "", "staged search is disabled");
    error.details = Box::new(json!({"grant":"MCP_GRANT_STAGED_SEARCH"}));
    error
}
fn invalid(field: &'static str, message: &str) -> Error {
    Error::new("invalid_input", field, message)
}
pub fn filters(input: Option<&Value>) -> Result<Value, Error> {
    let mut out =
        json!({"language":"en","time_range":null,"safesearch":"off","freshness":"no_preference"});
    if let Some(v) = input {
        let m = v
            .as_object()
            .ok_or_else(|| invalid("filters", "filters contains unsupported fields"))?;
        for (k, v) in m {
            if !["language", "time_range", "safesearch", "freshness"].contains(&k.as_str()) {
                return Err(invalid("filters", "filters contains unsupported fields"));
            }
            out[k.as_str()] = v.clone();
        }
    }
    if !matches!(
        out["safesearch"].as_str(),
        Some("off" | "moderate" | "strict")
    ) {
        return Err(invalid("filters.safesearch", "invalid safesearch value"));
    }
    if !matches!(
        out["freshness"].as_str(),
        Some("no_preference" | "prefer_fresh" | "prefer_cache")
    ) {
        return Err(invalid("filters.freshness", "invalid freshness value"));
    }
    if !out["time_range"].is_null()
        && !matches!(
            out["time_range"].as_str(),
            Some("day" | "week" | "month" | "year")
        )
    {
        return Err(invalid("filters.time_range", "invalid time_range value"));
    }
    if out["language"].as_str().is_none() {
        return Err(invalid("filters.language", "language must be a string"));
    }
    Ok(out)
}
fn request(query: &str, f: &Value) -> SearchRequest {
    SearchRequest {
        query: query.into(),
        language: f["language"].as_str().unwrap().into(),
        time_range: f["time_range"].as_str().map(str::to_owned),
        safesearch: match f["safesearch"].as_str() {
            Some("moderate") => 1,
            Some("strict") => 2,
            _ => 0,
        },
        freshness: f["freshness"].as_str().unwrap().into(),
        generate_suggestions: false,
        ..Default::default()
    }
}
fn proposal(query: &str, requested: &Value, f: &Value) -> Result<ScopeProposal, Error> {
    let m = requested
        .as_object()
        .filter(|m| m.len() == 1)
        .ok_or_else(|| {
            invalid(
                "scope",
                "scope must contain exactly one of engines or intent",
            )
        })?;
    let mut p = ScopeProposal::generic(request(query, f));
    if let Some(v) = m.get("engines") {
        let a = v
            .as_array()
            .filter(|a| !a.is_empty() && a.len() <= 64)
            .ok_or_else(|| {
                invalid(
                    "scope.engines",
                    "engines must be a nonempty list of at most 64 names",
                )
            })?;
        let names = a
            .iter()
            .map(|v| {
                v.as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .ok_or_else(|| {
                        invalid(
                            "scope.engines",
                            "engines must be a nonempty list of at most 64 names",
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if names.iter().collect::<BTreeSet<_>>().len() != names.len() {
            return Err(invalid(
                "scope.engines",
                "duplicate engine names are not allowed",
            ));
        }
        p.request.engines = Some(names);
    } else if let Some(intent) = m
        .get("intent")
        .and_then(Value::as_str)
        .filter(|s| !matches!(*s, "media" | "images" | "videos"))
    {
        let table: Value =
            ssx_json::from_str(include_str!("../../../docs/rust-rewrite/f09/profiles.json"))
                .expect("static profiles");
        if table["intents"][intent].is_null() {
            return Err(invalid(
                "scope.intent",
                "intent must be a supported text-search intent",
            ));
        }
        p.intent = intent.into();
    } else {
        return Err(invalid(
            "scope.intent",
            "intent must be a supported text-search intent",
        ));
    }
    Ok(p)
}
fn resolve_scope(
    policy: &PolicyService,
    query: &str,
    requested: &Value,
    f: &Value,
) -> Result<ssx_domain::ScopeDecision, Error> {
    let mut p = proposal(query, requested, f)?;
    if let Some(names) = &p.request.engines {
        let unknown = names
            .iter()
            .filter(|n| policy.catalog.get(n).is_none())
            .collect::<Vec<_>>();
        let inactive = names
            .iter()
            .filter(|n| policy.catalog.get(n).is_some_and(|e| !e.enabled))
            .collect::<Vec<_>>();
        if !unknown.is_empty() || !inactive.is_empty() {
            let problems = unknown
                .iter()
                .map(|n| format!("{n} (unknown)"))
                .chain(inactive.iter().map(|n| format!("{n} (inactive)")))
                .collect::<Vec<_>>();
            let mut error = Error::new(
                "invalid_scope",
                "engines",
                format!("unknown or inactive engines: {}", problems.join(", ")),
            );
            let valid = policy
                .catalog
                .engines()
                .iter()
                .filter(|e| e.enabled)
                .map(|e| e.name.clone())
                .collect::<BTreeSet<_>>();
            error.details = Box::new(json!({"valid_alternatives":valid}));
            return Err(error);
        }
    } else {
        let table: Value =
            ssx_json::from_str(include_str!("../../../docs/rust-rewrite/f09/profiles.json"))
                .expect("static profiles");
        let profile = &table["intents"][p.intent.as_str()];
        if matches!(p.intent.as_str(), "jobs" | "science" | "security")
            && !policy.policy.grants.contains(&p.intent)
        {
            let grant = format!("MCP_GRANT_{}", p.intent.to_ascii_uppercase());
            let mut error = Error::new(
                "tool_disabled",
                "intent",
                format!(
                    "intent '{}' requires the {} grant ({grant}=1)",
                    p.intent, p.intent
                ),
            );
            error.details = Box::new(json!({"grant":grant}));
            return Err(error);
        }
        let names = profile["engines"].as_array().unwrap();
        if !names.is_empty() {
            p.request.engines = Some(
                names
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|n| policy.catalog.get(n).is_some_and(|e| e.enabled))
                    .map(str::to_owned)
                    .collect(),
            );
        } else {
            let cats = profile["categories"].as_array().unwrap();
            if !cats.is_empty() {
                p.request.categories = Some(
                    cats.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect(),
                );
            }
        }
    }
    let scope = policy.resolver.resolve(&p.request, &policy.catalog);
    if scope.selected_engines.is_empty() {
        return Err(invalid("scope", "scope resolves to no active engines"));
    }
    Ok(scope)
}
fn scope_value(requested: &Value, authority: &ValidatedScope) -> Value {
    let d = &authority.preview().scope;
    json!({"requested":requested,"selected_engines":d.selected_engines,"excluded_engines":d.excluded_engines,"routing":{"rule":d.routing_rule,"matched_topic":d.matched_topic,"fallback":d.routing_fallback,"budget_applied":d.routing_budget_applied}})
}
fn authorize(
    policy: &PolicyService,
    query: &str,
    requested: &Value,
    f: &Value,
) -> Result<ValidatedScope, Error> {
    let p = proposal(query, requested, f)?;
    let token = policy.authorize(p).map_err(|e| {
        if e.field == "safesearch" {
            Error::new(
                "policy_rejected",
                "filters.safesearch",
                "strict SafeSearch cannot be enforced by the full staged scope",
            )
        } else {
            let mut error = Error::new("policy_rejected", e.field, e.message);
            error.details = e.details;
            error
        }
    })?;
    if token.preview().scope.selected_engines.is_empty() {
        return Err(invalid("scope", "scope resolves to no active engines"));
    }
    Ok(token)
}
#[derive(Debug)]
pub struct Plan {
    pub value: Value,
    pub scopes: Vec<ValidatedScope>,
}
impl Plan {
    pub fn build(policy: &PolicyService, limits: &Limits, input: &Input) -> Result<Self, Error> {
        if !limits.granted {
            return Err(disabled());
        }
        let query = input.query.trim_matches(ssx_json::python_whitespace);
        if query.is_empty() {
            return Err(invalid("query", "query is required"));
        }
        if input.query.chars().count() > policy.policy.max_query_length {
            let mut e = invalid(
                "query",
                &format!(
                    "query exceeds the maximum length of {} characters",
                    policy.policy.max_query_length
                ),
            );
            e.details = Box::new(json!({"max_length":policy.policy.max_query_length}));
            return Err(e);
        }
        let o = input
            .objectives
            .as_object()
            .filter(|m| {
                m.len() == 2 && m.contains_key("deadline_ms") && m.contains_key("max_engine_calls")
            })
            .ok_or_else(|| {
                invalid(
                    "objectives",
                    "objectives requires exactly deadline_ms and max_engine_calls",
                )
            })?;
        let deadline = o["deadline_ms"]
            .as_u64()
            .filter(|n| (1..=30000).contains(n))
            .ok_or_else(|| {
                invalid(
                    "objectives.deadline_ms",
                    "deadline_ms must be an integer from 1 to 30000",
                )
            })?;
        let calls = o["max_engine_calls"]
            .as_u64()
            .filter(|n| (1..=64).contains(n))
            .ok_or_else(|| {
                invalid(
                    "objectives.max_engine_calls",
                    "max_engine_calls must be an integer from 1 to 64",
                )
            })?;
        if input.fallback_scope.as_ref().is_some_and(python_truthy) != input.allow_scope_expansion {
            return Err(invalid(
                "fallback_scope",
                "fallback_scope and allow_scope_expansion=true must be supplied together",
            ));
        }
        let f = filters(input.filters.as_ref())?;
        let mut resolved = vec![resolve_scope(policy, query, &input.initial_scope, &f)?];
        if let Some(fallback) = &input.fallback_scope {
            resolved.push(resolve_scope(policy, query, fallback, &f)?);
        }
        let selected = resolved
            .iter()
            .flat_map(|s| s.selected_engines.iter())
            .collect::<Vec<_>>();
        if selected.iter().collect::<BTreeSet<_>>().len() != selected.len() {
            return Err(Error::new(
                "objective_conflict",
                "fallback_scope",
                "initial and fallback engine scopes must be disjoint",
            ));
        }
        let effective = json!({"deadline_ms":deadline.min(limits.deadline_ms),"max_engine_calls":calls.min(limits.engine_calls)});
        if effective["deadline_ms"].as_u64() == Some(0)
            || selected.len() as u64 > calls.min(limits.engine_calls)
        {
            return Err(Error::new(
                "objective_conflict",
                "objectives.max_engine_calls",
                "resolved scopes exceed max_engine_calls",
            ));
        }
        let names = selected.into_iter().cloned().collect::<Vec<_>>();
        policy.engine_rejection(&names, "engines").map_err(|e| {
            let mut error = Error::new("policy_rejected", e.field, e.message);
            error.details = e.details;
            error
        })?;
        let mut scopes = vec![authorize(policy, query, &input.initial_scope, &f)?];
        if let Some(fallback) = &input.fallback_scope {
            scopes.push(authorize(policy, query, fallback, &f)?);
        }
        let initial = scope_value(&input.initial_scope, &scopes[0]);
        let fallback = input
            .fallback_scope
            .as_ref()
            .map(|f| scope_value(f, &scopes[1]));
        let identity = json!({"version":1,"query":query,"objectives":{"requested":input.objectives,"effective":effective},"initial_scope":initial,"fallback_scope":fallback,"allow_scope_expansion":input.allow_scope_expansion,"filters":f});
        let digest = plan_digest(&identity)?;
        Ok(Self {
            value: json!({"identity":identity,"digest":digest,"query":query,"filters":f,"initial_scope":initial,"fallback_scope":fallback,"allow_scope_expansion":input.allow_scope_expansion,"fallback_when":"clean_empty","requested_objectives":input.objectives,"effective_objectives":effective}),
            scopes,
        })
    }
    pub fn record(
        &self,
        tenant: &TenantId,
        id: &StagedSearchId,
        now: f64,
    ) -> Result<Record, Error> {
        let p = &self.value;
        let stages=self.scopes.iter().enumerate().map(|(i,s)|json!({"index":i,"scope":if i==0{&p["initial_scope"]}else{&p["fallback_scope"]},"state":"pending","skipped_reason":null,"attempts":[],"enforcement":Value::Object(s.preview().enforcement.iter().map(|(k,v)|(k.clone(),v.to_value())).collect())})).collect::<Vec<_>>();
        Ok(Record::from_value(
            json!({"operation_id":id.as_str(),"tenant":tenant.as_str(),"version":1,"plan_digest":p["digest"],"accepted_at":now,"execution_deadline_at":now+p["effective_objectives"]["deadline_ms"].as_u64().unwrap() as f64/1000.,"expires_at":now+3600.,"state":"queued","stop_reason":null,"plan":{"query":p["query"],"filters":p["filters"],"initial_scope":p["initial_scope"],"fallback_scope":p["fallback_scope"],"allow_scope_expansion":p["allow_scope_expansion"],"fallback_when":"clean_empty"},"objectives":{"requested":p["requested_objectives"],"effective":p["effective_objectives"],"unmet":[]},"budget":{"unit":"adapter_search_invocations","limit":p["effective_objectives"]["max_engine_calls"],"reserved":0,"observed":0},"stages":stages,"retry_keys":[],"selected_result_attempt_id":null,"next_stage":0}),
        )?)
    }
}
/// Validate every captured stage, including unused fallback, under current policy.
pub fn revalidate(policy: &PolicyService, limits: &Limits, record: &Record) -> Result<(), Error> {
    if !limits.granted {
        return Err(disabled());
    }
    let v = record.value();
    let effective = &v["objectives"]["effective"];
    if effective["deadline_ms"].as_u64().unwrap() > limits.deadline_ms
        || effective["max_engine_calls"].as_u64().unwrap() > limits.engine_calls
    {
        return Err(Error::new(
            "policy_rejected",
            "",
            "current staged-search operator limits are narrower",
        ));
    }
    let mut engines = Vec::new();
    for s in v["stages"].as_array().unwrap() {
        let requested = &s["scope"]["requested"];
        if let Some(intent) = requested["intent"].as_str() {
            if matches!(intent, "jobs" | "security" | "science")
                && !policy.policy.grants.contains(intent)
            {
                return Err(Error::new(
                    "policy_rejected",
                    "",
                    format!("current policy disables the captured {intent} intent"),
                ));
            }
            let current = resolve_scope(
                policy,
                v["plan"]["query"].as_str().unwrap(),
                requested,
                &v["plan"]["filters"],
            );
            if current.is_err()
                || current.is_ok_and(|scope| {
                    json!(scope.selected_engines) != s["scope"]["selected_engines"]
                })
            {
                return Err(Error::new(
                    "policy_rejected",
                    "",
                    "current routing no longer authorizes the captured scope",
                ));
            }
        }
        engines.extend(
            s["scope"]["selected_engines"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string()),
        );
    }
    policy
        .retained_engine_rejection(&engines, "engines")
        .map_err(|e| {
            let mut error = Error::new("policy_rejected", e.field, e.message);
            error.details = e.details;
            error
        })?;

    Ok(())
}
// JSON values at the Python replay boundary retain Python scalar equality.
// Fresh planning still validates strictly typed integer objectives.
pub fn python_equal(left: &Value, right: &Value) -> bool {
    fn numeric(value: &Value) -> Option<f64> {
        match value {
            Value::Bool(v) => Some(if *v { 1.0 } else { 0.0 }),
            Value::Number(v) => v.as_f64(),
            _ => None,
        }
    }
    match (left, right) {
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(k, v)| b.get(k).is_some_and(|other| python_equal(v, other)))
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| python_equal(a, b))
        }
        (Value::Number(_) | Value::Bool(_), Value::Number(_) | Value::Bool(_)) => {
            // Equal integer representations stay exact; float/bool comparisons
            // operate on admitted objective values bounded to 1..30000/1..64.
            if left == right {
                true
            } else {
                numeric(left)
                    .zip(numeric(right))
                    .is_some_and(|(a, b)| a == b)
            }
        }
        _ => left == right,
    }
}
fn python_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) if !v.is_f64() => v.as_str().trim_start_matches('-') != "0",
        Value::Number(v) => v.as_f64().is_some_and(|v| v != 0.0),
        Value::String(v) => !v.is_empty(),
        Value::Array(v) => !v.is_empty(),
        Value::Object(v) => !v.is_empty(),
    }
}
pub fn replay_equivalent(record: &Record, input: &Input) -> bool {
    let v = record.value();
    let p = &v["plan"];
    let mut normalized =
        json!({"language":"en","time_range":null,"safesearch":"off","freshness":"no_preference"});
    if let Some(fields) = input.filters.as_ref().and_then(Value::as_object) {
        for (key, value) in fields {
            normalized[key.as_str()] = value.clone();
        }
    }
    p["query"] == json!(input.query.trim_matches(ssx_json::python_whitespace))
        && python_equal(&v["objectives"]["requested"], &input.objectives)
        && python_equal(&p["initial_scope"]["requested"], &input.initial_scope)
        && python_equal(
            &p["fallback_scope"]["requested"],
            input.fallback_scope.as_ref().unwrap_or(&Value::Null),
        )
        && p["allow_scope_expansion"] == json!(input.allow_scope_expansion)
        && python_equal(&p["filters"], &normalized)
}

pub fn dispatch(record: &Record, remaining_ms: u64) -> SearchRequest {
    let v = record.value();
    let i = v["next_stage"].as_u64().unwrap() as usize;
    let a = v["stages"][i]["attempts"]
        .as_array()
        .unwrap()
        .last()
        .unwrap();
    let mut r = request(v["plan"]["query"].as_str().unwrap(), &v["plan"]["filters"]);
    r.engines = Some(
        v["stages"][i]["scope"]["selected_engines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().into())
            .collect(),
    );
    r.interactive_timeout_ms = Some(remaining_ms.max(1));
    r.client_identifier = Some(format!("mcp:{}", record.tenant()));
    r.execution_isolation_key = Some(format!(
        "{}:{}",
        record.id(),
        a["attempt_id"].as_str().unwrap()
    ));
    r
}
/// A denied claim may close its own attempt without authorizing provider work.
/// Preserve the original rejection envelope separately from the attempt error.
pub fn policy_rejected(
    record: &Record,
    rejection: &Value,
    now: f64,
) -> Result<Record, DomainError> {
    let next = complete(
        record,
        None,
        None,
        now,
        Some((
            "policy_rejected",
            rejection["message"]
                .as_str()
                .ok_or(DomainError("invalid rejection message"))?,
        )),
    )?;
    let mut value = next.value().clone();
    value["error"] = rejection.clone();
    let next = Record::from_value(value)?;
    record.validate_completion(&next)?;
    Ok(next)
}
/// Complete only the acknowledged owned snapshot; no reload can substitute history.
pub fn complete(
    record: &Record,
    response: Option<&SearchResponse>,
    cursor: Option<&str>,
    now: f64,
    failure: Option<(&str, &str)>,
) -> Result<Record, DomainError> {
    let mut v = record.value().clone();
    let i = v["next_stage"].as_u64().unwrap() as usize;
    let has_fallback = i == 0 && v["stages"].as_array().unwrap().len() > 1;
    let mut attempt = v["stages"][i]["attempts"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone();
    let mut unmet = Vec::new();
    let (state, reason);
    if let Some((code, message)) = failure {
        attempt["state"] = json!("failed");
        attempt["error"] = json!({"code":code,"message":message});
        if code == "execution_failed" {
            v["budget"]["observed"] = Value::Null;
            if has_fallback {
                unmet.push(json!({"objective":"fallback","reason":"initial_stage_failed"}));
            }
        } else {
            attempt["observed_engine_calls"] = json!(0);
            if code == "policy_rejected" {
                v["error"] = attempt["error"].clone();
                unmet.push(json!({"objective":"fallback","reason":"policy_rejected"}));
            } else {
                attempt["deadline_exceeded"] = json!(true);
                unmet.push(json!({"objective":"deadline_ms","reason":"deadline_expired"}));
            }
        }
        state = "failed";
        reason = if code == "deadline_exceeded" {
            "deadline_expired"
        } else {
            code
        };
    } else {
        let r = response.ok_or(DomainError("missing staged response"))?;
        let observed = if r.cached {
            0
        } else {
            r.dispatched_engine_count
        };
        if observed < 0 {
            return Err(DomainError("invalid dispatched count"));
        }
        if let Some(n) = v["budget"]["observed"].as_u64() {
            v["budget"]["observed"] = json!(n + observed as u64);
        }
        attempt["state"] = json!("done");
        attempt["observed_engine_calls"] = json!(observed);
        attempt["cached"] = json!(r.cached);
        attempt["query_id"] = json!(r.query_id);
        attempt["cursor"] = json!(cursor);
        attempt["result_count"] = json!(r.results.len());
        attempt["engine_outcomes"] = json!(r.engine_outcomes);
        attempt["deadline_exceeded"] = json!(r.deadline_exceeded);
        if cursor.is_some() {
            v["selected_result_attempt_id"] = attempt["attempt_id"].clone();
        }
        let clean = r.results.is_empty()
            && !r.cached
            && !r.partial
            && !r.deadline_exceeded
            && r.engine_outcomes.len()
                == v["stages"][i]["scope"]["selected_engines"]
                    .as_array()
                    .unwrap()
                    .len()
            && r.engine_outcomes.iter().all(|o| o.status == "ok");
        let expired = now >= v["execution_deadline_at"].as_f64().unwrap();
        if expired {
            attempt["deadline_exceeded"] = json!(true);
            unmet.push(json!({"objective":"deadline_ms","reason":"deadline_expired"}));
        }
        if expired
            || r.deadline_exceeded
            || r.partial
            || r.engine_outcomes.iter().any(|o| o.status != "ok")
        {
            state = "failed";
            reason = if expired || r.deadline_exceeded {
                "deadline_expired"
            } else {
                "execution_failed"
            };
            attempt["state"] = json!("failed");
            if has_fallback {
                unmet.push(json!({"objective":"fallback","reason":"initial_stage_failed"}));
            }
        } else if !r.results.is_empty() {
            state = "completed";
            reason = if i == 0 {
                "initial_nonempty"
            } else {
                "fallback_nonempty"
            };
            for s in v["stages"].as_array_mut().unwrap().iter_mut().skip(i + 1) {
                s["state"] = json!("skipped");
                s["skipped_reason"] = json!("initial_nonempty");
            }
        } else if clean && has_fallback {
            state = "queued";
            reason = "";
            v["next_stage"] = json!(i + 1);
        } else if clean {
            state = "completed";
            reason = "clean_empty";
        } else {
            state = "failed";
            reason = "execution_failed";
            attempt["state"] = json!("failed");
            if has_fallback {
                unmet.push(json!({"objective":"fallback","reason":"initial_stage_not_clean"}));
            }
        }
    }
    attempt["finished_at"] = json!(now);
    let current = &mut v["stages"].as_array_mut().unwrap()[i];
    current["state"] = attempt["state"].clone();
    *current["attempts"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap() = attempt;
    v["state"] = json!(state);
    v["stop_reason"] = if reason.is_empty() {
        Value::Null
    } else {
        json!(reason)
    };
    v["objectives"]["unmet"]
        .as_array_mut()
        .unwrap()
        .extend(unmet);
    let next = Record::from_value(v)?;
    record.validate_completion(&next)?;
    Ok(next)
}
