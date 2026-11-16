//! Caller-directed adaptive admission over the accepted research store and worker.
//! Previews never write; only the caller supplies questions, variants and stopping decisions.
use crate::{
    research_runner::{ResearchRunner, RunnerConfig, RunnerError},
    search::{SearchClock, SearchService, SystemSearchClock},
    valkey::{
        TokenSource,
        research::{Error as StoreError, JobRead, ResearchStore},
    },
};
use sha2::{Digest, Sha256};
use ssx_application::{
    policy::{ScopeProposal, ValidatedScope},
    snapshot::{SnapshotResultLookup, SnapshotService},
};
use ssx_domain::{
    research::{ResearchCeilings, ResearchJob, ResearchQuery},
    *,
};
use ssx_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use tokio::sync::watch;

#[derive(Debug, Clone, PartialEq)]
pub struct PlanningError {
    pub code: &'static str,
    pub message: String,
    context: Box<PlanningErrorContext>,
}
#[derive(Debug, Clone, PartialEq, Default)]
struct PlanningErrorContext {
    details: ssx_json::Map<String, Value>,
    outer: ssx_json::Map<String, Value>,
}
impl PlanningError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            context: Box::default(),
        }
    }
    pub fn with_field(self, field: &str) -> Self {
        self.with_detail("field", json!(field))
    }
    fn with_detail(mut self, key: &str, value: Value) -> Self {
        self.context.details.insert(key.into(), value);
        self
    }
    fn plain(mut self) -> Self {
        self.context.details.clear();
        self.context.outer.clear();
        self
    }
    fn from_policy(error: ssx_application::policy::PolicyError) -> Self {
        // Presentation only: authority already denied at the sole shared gate.
        let mut envelope = error.to_value();
        let mut details = envelope["error"].as_object().cloned().unwrap_or_default();
        details.shift_remove("code");
        details.shift_remove("message");
        envelope.as_object_mut().unwrap().shift_remove("error");
        Self {
            code: error.code,
            message: error.message,
            context: Box::new(PlanningErrorContext {
                details,
                outer: envelope.as_object().cloned().unwrap_or_default(),
            }),
        }
    }
    pub fn value(&self) -> Value {
        let mut error = json!({"code":self.code,"message":self.message});
        for (key, value) in &self.context.details {
            error[key.as_str()] = value.clone();
        }
        let mut value = json!({"error":error});
        for (key, extra) in &self.context.outer {
            value[key.as_str()] = extra.clone();
        }
        value
    }
}
type Result<T> = std::result::Result<T, PlanningError>;
fn invalid(message: impl Into<String>) -> PlanningError {
    PlanningError::new("invalid_input", message)
}
fn bad(code: &'static str, message: &str) -> PlanningError {
    PlanningError::new(code, message)
}
fn domain(_: DomainError) -> PlanningError {
    invalid("invalid research record or budget")
}
fn policy(error: crate::search::SearchError) -> PlanningError {
    match error {
        crate::search::SearchError::Policy(e) => PlanningError::from_policy(e),
        _ => bad("store_unavailable", "search authority is unavailable"),
    }
}
fn query_policy(error: crate::search::SearchError, intent: &str, direct: bool) -> PlanningError {
    let mut e = policy(error);
    if e.code == "invalid_input"
        && e.context.details.get("field").and_then(Value::as_str) == Some("intent")
    {
        if direct {
            let profiles: Value =
                ssx_json::from_str(include_str!("../../../docs/rust-rewrite/f09/profiles.json"))
                    .expect("accepted profiles");
            let mut alternatives: Vec<_> = profiles["intents"]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect();
            alternatives.sort();
            e = e.with_detail("valid_alternatives", json!(alternatives));
        } else {
            return invalid("unknown research intent");
        }
    }
    if e.code == "invalid_intent" && !direct {
        return bad(
            "invalid_intent",
            "research subqueries do not support media searches",
        );
    }
    if e.code == "tool_disabled" && e.message == format!("{intent} requires the {intent} grant") {
        if direct {
            let env = format!("MCP_GRANT_{}", intent.to_ascii_uppercase());
            return PlanningError::new(
                "tool_disabled",
                format!("intent '{intent}' requires the {intent} grant ({env}=1)"),
            )
            .with_field("intent")
            .with_detail("grant", json!(env));
        }
        return PlanningError::new(
            "tool_disabled",
            format!("intent {intent} requires the {intent} grant"),
        );
    }
    if direct { e } else { e.plain() }
}
fn workflow_error(error: crate::search::SearchError) -> PlanningError {
    let mut e = policy(error);
    if e.code == "tool_disabled"
        && let Some(start) = e.message.find("MCP_GRANT_")
    {
        let env: String = e.message[start..]
            .chars()
            .take_while(|c| c.is_ascii_uppercase() || *c == '_')
            .collect();
        return PlanningError::new(
            "tool_disabled",
            format!("dependency dossier requires {env}"),
        )
        .with_detail("grant", json!(env));
    }
    e.code = "policy_rejected";
    e
}
fn workflow_budget(job: &ResearchJob) -> Result<()> {
    if job.workflow.get("kind").and_then(Value::as_str) != Some("dependency_dossier") {
        return Ok(());
    }
    let raw_budget = job.workflow.get("budget").unwrap_or(&Value::Null);
    let present = match raw_budget {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::String(v) => !v.is_empty(),
        Value::Array(v) => !v.is_empty(),
        Value::Object(v) => !v.is_empty(),
        Value::Number(v) => v.as_f64() != Some(0.0),
    };
    if present && !raw_budget.is_object() {
        return Err(invalid("invalid dependency dossier budget"));
    }
    let budget = if present { raw_budget } else { &Value::Null };
    // These values need integer ordering only. Preserve Python's arbitrary-width
    // integers rather than inventing an i64/i128 budget cap at this read boundary.
    let integer = |key: &str, default: usize| -> Result<(bool, String)> {
        let raw = match budget.get(key) {
            None => default.to_string(),
            Some(Value::Bool(v)) => if *v { "1" } else { "0" }.into(),
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) if !n.is_f64() => n.as_str().into(),
            Some(Value::Number(n)) => n
                .as_f64()
                .filter(|n| n.is_finite())
                .map(|n| format!("{:.0}", n.trunc()))
                .ok_or_else(|| invalid("invalid dependency dossier budget"))?,
            _ => return Err(invalid("invalid dependency dossier budget")),
        };
        let mut text = trim(&raw);
        let negative = text.starts_with('-');
        if text.starts_with(['-', '+']) {
            text = &text[1..];
        }
        let mut digits = String::new();
        let mut previous_digit = false;
        for ch in text.chars() {
            if ch == '_' && previous_digit {
                previous_digit = false;
                continue;
            }
            let n = ssx_json::decimal_value(ch)
                .ok_or_else(|| invalid("invalid dependency dossier budget"))?;
            digits.push(char::from(b'0' + n));
            previous_digit = true;
        }
        if !previous_digit {
            return Err(invalid("invalid dependency dossier budget"));
        }
        let digits = digits.trim_start_matches('0');
        Ok((
            negative && !digits.is_empty(),
            if digits.is_empty() {
                "0".into()
            } else {
                digits.into()
            },
        ))
    };
    let at_least = |left: (bool, String), right: (bool, String)| {
        if left.0 != right.0 {
            return !left.0;
        }
        let order = left
            .1
            .len()
            .cmp(&right.1.len())
            .then_with(|| left.1.cmp(&right.1));
        if left.0 { order.is_le() } else { order.is_ge() }
    };
    if at_least(
        integer("used_adapter_calls", 0)?,
        integer("max_adapter_calls", job.queries.len())?,
    ) {
        return Err(bad(
            "budget_exceeded",
            "dependency dossier adapter call budget is exhausted",
        ));
    }
    if at_least(integer("captured_results", 0)?, integer("max_results", 0)?) {
        return Err(bad(
            "budget_exceeded",
            "dependency dossier result budget is exhausted",
        ));
    }
    Ok(())
}
fn evidence_ids(value: Option<&Value>, direct: bool) -> Result<Vec<String>> {
    let Some(value) = value.filter(|v| !(direct && v.is_null())) else {
        return Ok(vec![]);
    };
    let list = value
        .as_array()
        .filter(|v| v.len() <= 10)
        .ok_or_else(|| invalid("evidence_result_ids must be a list of at most 10 result IDs"))?;
    list.iter()
        .map(|v| {
            v.as_str()
                .filter(|s| !s.is_empty() && s.chars().count() <= 160)
                .map(str::to_owned)
                .ok_or_else(|| invalid("evidence_result_ids must contain nonempty bounded strings"))
        })
        .collect()
}
fn store_error(error: StoreError) -> PlanningError {
    match error {
        StoreError::Invalid(DomainError(code)) if code.starts_with("w03:") => {
            let mut fields = code[4..].splitn(2, ':');
            let code = fields.next().unwrap_or("invalid_input");
            PlanningError::new(code, fields.next().unwrap_or("adaptive mutation rejected"))
        }
        StoreError::Invalid(DomainError("metadata-only completion exceeds deadline")) => {
            bad("deadline_exceeded", "job deadline has passed")
        }
        StoreError::Invalid(e) => domain(e),
        _ => bad("store_unavailable", "research job store is unavailable"),
    }
}
fn fenced_into(slot: &mut Option<PlanningError>, error: PlanningError) -> DomainError {
    // The callback-local owned error survives the store's finite static boundary.
    // No allocation is leaked, no caller text enters DomainError, and the error
    // is returned only when this exact claim callback refuses the mutation.
    *slot = Some(error);
    DomainError("w03:invalid_input:adaptive mutation rejected")
}
fn now() -> WallClockTimestamp {
    SystemSearchClock.capture().0
}
fn random() -> String {
    let mut bytes = [0; 32];
    aws_lc_rs::rand::fill(&mut bytes).expect("OS random identity");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
struct Tokens;
impl TokenSource<JobId> for Tokens {
    fn next_token(&mut self) -> LeaseToken<JobId> {
        LeaseToken::new(random()).expect("random lease")
    }
}
fn attempt() -> AttemptId {
    AttemptId::new(random()).expect("random attempt")
}
pub fn query_identity(query: &str, engines: &[String]) -> (String, BTreeSet<String>) {
    (
        ssx_json::casefold(
            &query
                .split(ssx_json::python_whitespace)
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" "),
        ),
        engines.iter().cloned().collect(),
    )
}
fn word(c: char) -> bool {
    static TABLE: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        ssx_json::from_str(include_str!("../../../docs/rust-rewrite/f11/unicode.json"))
            .expect("Unicode16 source table")
    })["word_ranges"]
        .as_array()
        .expect("word ranges")
        .iter()
        .any(|r| r[0].as_u64().unwrap() <= c as u64 && c as u64 <= r[1].as_u64().unwrap())
}
fn trim(s: &str) -> &str {
    s.trim_matches(ssx_json::python_whitespace)
}
/// Python's exact leftmost, nonoverlapping IGNORECASE pattern, using pinned
/// Unicode16 decimal/word predicates rather than ASCII or Rust Unicode drift.
pub fn identifiers(input: &str) -> Vec<String> {
    let chars: Vec<char> = input.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let boundary = i == 0 || word(chars[i - 1]) != word(chars[i]);
        let rest = &chars[i..];
        let mut end = None;
        if boundary
            && rest.len() >= 4
            && rest[0].eq_ignore_ascii_case(&'c')
            && rest[1].eq_ignore_ascii_case(&'v')
            && rest[2].eq_ignore_ascii_case(&'e')
            && rest[3] == '-'
        {
            let mut j = i + 4;
            if (0..4).all(|n| {
                chars
                    .get(j + n)
                    .is_some_and(|c| ssx_json::decimal_digit(*c))
            }) {
                j += 4;
                if chars.get(j) == Some(&'-') {
                    j += 1;
                    let start = j;
                    while chars.get(j).is_some_and(|c| ssx_json::decimal_digit(*c)) {
                        j += 1;
                    }
                    if j - start >= 4 && (j == chars.len() || !word(chars[j])) {
                        end = Some(j);
                    }
                }
            }
        }
        if end.is_none() && boundary && rest.starts_with(&['1', '0', '.']) {
            let mut j = i + 3;
            let start = j;
            while j - start < 9 && chars.get(j).is_some_and(|c| ssx_json::decimal_digit(*c)) {
                j += 1;
            }
            if j - start >= 4 && chars.get(j) == Some(&'/') {
                j += 1;
                let start = j;
                while chars
                    .get(j)
                    .is_some_and(|c| !ssx_json::python_whitespace(*c) && !['<', '>'].contains(c))
                {
                    j += 1;
                }
                if j > start {
                    end = Some(j);
                }
            }
        }
        if end.is_none() && boundary {
            let mut j = i;
            if (0..4).all(|n| {
                chars
                    .get(j + n)
                    .is_some_and(|c| ssx_json::decimal_digit(*c))
            }) {
                j += 4;
                if chars.get(j) == Some(&'.') {
                    j += 1;
                    let start = j;
                    while j - start < 5 && chars.get(j).is_some_and(|c| ssx_json::decimal_digit(*c))
                    {
                        j += 1;
                    }
                    if j - start >= 4 {
                        if chars.get(j).is_some_and(|c| c.eq_ignore_ascii_case(&'v'))
                            && chars
                                .get(j + 1)
                                .is_some_and(|c| ssx_json::decimal_digit(*c))
                        {
                            j += 2;
                            while chars.get(j).is_some_and(|c| ssx_json::decimal_digit(*c)) {
                                j += 1;
                            }
                        }
                        if j == chars.len() || !word(chars[j]) {
                            end = Some(j);
                        }
                    }
                }
            }
        }
        if end.is_none() && chars[i] == '@' {
            let allowed = |c: char| word(c) || ['.', '-'].contains(&c);
            let mut j = i + 1;
            let start = j;
            while chars.get(j).is_some_and(|c| allowed(*c)) {
                j += 1;
            }
            if j > start && chars.get(j) == Some(&'/') {
                j += 1;
                let start = j;
                while chars.get(j).is_some_and(|c| allowed(*c)) {
                    j += 1;
                }
                if j > start {
                    end = Some(j);
                }
            }
        }
        if let Some(j) = end {
            let text: String = chars[i..j].iter().collect();
            out.push(
                text.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', '"', ']', '}'])
                    .into(),
            );
            i = j;
        } else {
            i += 1;
        }
    }
    out
}
pub fn validate_variant(question: &str, variant: &str) -> Result<()> {
    let tokens: BTreeSet<_> = identifiers(variant)
        .into_iter()
        .map(|s| ssx_json::casefold(&s))
        .collect();
    if identifiers(question)
        .iter()
        .any(|s| !tokens.contains(&ssx_json::casefold(s)))
    {
        return Err(invalid(
            "terminology variants must preserve every CVE, DOI, arXiv or scoped-package identifier",
        ));
    }
    Ok(())
}
pub fn validate_metadata(query: &ResearchQuery) -> Result<()> {
    if query.planning_method.as_deref().is_some_and(|s| {
        ![
            "original",
            "decomposition",
            "terminology_expansion",
            "evidence_followup",
        ]
        .contains(&s)
    }) {
        return Err(invalid("unknown planning_method"));
    }
    let ids = &query.evidence_result_ids;
    if ids.len() > 10 {
        return Err(invalid(
            "evidence_result_ids must be a list of at most 10 result IDs",
        ));
    }
    if ids.iter().any(|s| s.is_empty() || s.chars().count() > 160) {
        return Err(invalid(
            "evidence_result_ids must contain nonempty bounded strings",
        ));
    }
    if ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
        return Err(invalid("duplicate evidence_result_ids"));
    }
    if !ids.is_empty() && (query.parent_attempt_id.is_none() || query.rationale.is_none()) {
        return Err(invalid(
            "evidence references require a parent_attempt_id and rationale",
        ));
    }
    if query.planning_method.as_deref() == Some("evidence_followup") && ids.is_empty() {
        return Err(invalid(
            "evidence_followup requires admitted result references",
        ));
    }
    Ok(())
}
pub fn metadata(value: Option<&Value>, name: &str, limit: usize) -> Result<Option<String>> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .filter(|s| !trim(s).is_empty() && s.chars().count() <= limit)
            .map(|s| Some(trim(s).to_owned()))
            .ok_or_else(|| {
                invalid(format!(
                    "{name} must be a nonempty string of at most {limit} characters"
                ))
            }),
    }
}
fn sha(value: &Value, sorted: bool) -> String {
    fn sort(v: &Value) -> Value {
        match v {
            Value::Object(m) => Value::Object(
                m.iter()
                    .map(|(k, v)| (k.clone(), sort(v)))
                    .collect::<BTreeMap<_, _>>()
                    .into_iter()
                    .collect(),
            ),
            Value::Array(a) => Value::Array(a.iter().map(sort).collect()),
            v => v.clone(),
        }
    }
    let value = if sorted { sort(value) } else { value.clone() };
    // Source json.dumps defaults: ASCII escapes and one space after separators.
    let raw = ssx_json::to_string(&value).expect("JSON value");
    let mut spaced = String::new();
    let mut quoted = false;
    let mut escape = false;
    for c in raw.chars() {
        if c as u32 >= 127 {
            for unit in c.encode_utf16(&mut [0; 2]).iter() {
                spaced.push_str(&format!("\\u{unit:04x}"));
            }
        } else {
            spaced.push(c);
        }
        if quoted {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                quoted = false;
            }
        } else if c == '"' {
            quoted = true;
        } else if c == ',' || c == ':' {
            spaced.push(' ');
        }
    }
    Sha256::digest(spaced.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn continuation_digest(q: &ResearchQuery) -> String {
    let mut engines = q.engines.clone();
    engines.sort();
    let mut value = json!([
        q.query,
        q.intent,
        engines,
        q.subquestion_id,
        q.rationale,
        q.parent_attempt_id
    ]);
    if q.planning_method.is_some() || !q.evidence_result_ids.is_empty() {
        let mut ids = q.evidence_result_ids.clone();
        ids.sort();
        value
            .as_array_mut()
            .unwrap()
            .push(json!({"planning_method":q.planning_method,"evidence_result_ids":ids}));
    }
    sha(&value, false)
}
pub fn entry(q: &ResearchQuery) -> Value {
    let mut value = json!({"query":q.query,"intent":q.intent,"engines":q.engines,"planning_method":q.planning_method});
    for (name, field) in [
        ("subquestion_id", &q.subquestion_id),
        ("rationale", &q.rationale),
        ("parent_attempt_id", &q.parent_attempt_id),
    ] {
        if let Some(s) = field {
            value[name] = json!(s);
        }
    }
    if !q.evidence_result_ids.is_empty() {
        value["evidence_result_ids"] = json!(q.evidence_result_ids);
    }
    value
}
/// Captured evidence contains opaque authorities and expiry, never caller flags.
struct Evidence {
    authorities: Vec<ValidatedScope>,
    earliest_expiry: Option<f64>,
    records: Vec<Value>,
}
#[derive(Clone)]
pub struct AdaptiveResearch {
    jobs: ResearchStore,
    search: SearchService,
    snapshots: SnapshotService,
    runner: ResearchRunner,
    config: RunnerConfig,
    max_query_length: usize,
    default_deadline_seconds: u64,
}
impl AdaptiveResearch {
    pub fn new(
        jobs: ResearchStore,
        search: SearchService,
        snapshots: SnapshotService,
        config: RunnerConfig,
        max_query_length: usize,
    ) -> std::result::Result<Self, RunnerError> {
        let runner = ResearchRunner::new(
            jobs.clone(),
            search.clone(),
            snapshots.clone(),
            config.clone(),
        )?;
        Ok(Self {
            jobs,
            search,
            snapshots,
            runner,
            config,
            max_query_length,
            default_deadline_seconds: 600,
        })
    }
    pub fn with_default_deadline_seconds(mut self, seconds: u64) -> Result<Self> {
        if seconds == 0 {
            return Err(invalid("default deadline must be positive"));
        }
        self.default_deadline_seconds = seconds;
        Ok(self)
    }
    pub fn store(&self) -> &ResearchStore {
        &self.jobs
    }
    pub fn ceilings(&self) -> ResearchCeilings {
        self.config.ceilings
    }
    async fn grant(&self, operation: &str) -> Result<()> {
        self.search
            .research_template_scope("web")
            .map(|_| ())
            .map_err(|e| {
                let mut error = policy(e).plain();
                if error.code == "tool_disabled" {
                    error.message = match operation {
                        "preview" => "query planning requires MCP_GRANT_RESEARCH=1".into(),
                        "update" => "research grant is required".into(),
                        name => {
                            format!("{name} requires the research grant (MCP_GRANT_RESEARCH=1)")
                        }
                    };
                }
                error
            })
    }
    async fn read_job(
        &self,
        tenant: TenantId,
        id: &str,
        context: &str,
        workflow: bool,
    ) -> Result<ResearchJob> {
        let missing = || {
            let e = bad("invalid_job_id", "unknown job id");
            if context == "preview" {
                e
            } else {
                e.with_field("job_id")
            }
        };
        let id = JobId::new(id).map_err(|_| missing())?;
        let job = match self.jobs.for_tenant(tenant).load(&id).await {
            JobRead::Found(job) => *job,
            JobRead::Missing => return Err(missing()),
            _ => {
                return Err(match context {
                    "preview" => bad("store_unavailable", "research job store is unavailable"),
                    "update" => bad("store_unavailable", "job store is unavailable"),
                    _ => bad(
                        "store_unavailable",
                        "job store is unavailable; research jobs are not persisted",
                    )
                    .with_field("job_id"),
                });
            }
        };
        if workflow {
            self.search
                .validate_research_workflow(&job)
                .map_err(workflow_error)?;
        }
        Ok(job)
    }
    pub async fn get_job(&self, tenant: TenantId, id: &str) -> Result<ResearchJob> {
        self.read_job(tenant, id, "public", true).await
    }
    pub async fn load(&self, tenant: TenantId, id: &str) -> Result<ResearchJob> {
        self.grant("preview").await?;
        let job = self.read_job(tenant, id, "preview", true).await?;
        self.retained_scope(&job)?;
        Ok(job)
    }
    fn retained_scope(&self, job: &ResearchJob) -> Result<()> {
        // Only retained engine scope is disclosed here. The clone is never
        // persisted; evidence() checks the selected parent's original flag.
        let engines: BTreeSet<_> = job
            .queries
            .iter()
            .flat_map(|q| q.engines.iter().cloned())
            .collect();
        if engines.is_empty() {
            return Ok(());
        }
        let scope_only = ResearchQuery {
            engines: engines.into_iter().collect(),
            requires_intent_grant: false,
            query: job.question.clone(),
            intent: "web".into(),
            ..Default::default()
        };
        self.search
            .prepare_research_plan(&ScopeProposal::from_research_query(&scope_only))
            .map_err(policy)?;
        Ok(())
    }
    pub async fn prepare_query(&self, entry: &Value, max_engines: u64) -> Result<ResearchQuery> {
        self.prepare_query_kind(entry, max_engines, false, None)
            .await
    }
    async fn prepare_query_kind(
        &self,
        entry: &Value,
        max_engines: u64,
        direct: bool,
        continuation: Option<&Value>,
    ) -> Result<ResearchQuery> {
        let object = entry
            .as_object()
            .ok_or_else(|| invalid("invalid research plan entry fields"))?;
        if object.keys().any(|k| {
            ![
                "query",
                "intent",
                "engines",
                "subquestion_id",
                "rationale",
                "parent_attempt_id",
                "planning_method",
                "evidence_result_ids",
            ]
            .contains(&k.as_str())
        }) {
            return Err(invalid("invalid research plan entry fields"));
        }
        let text = entry["query"]
            .as_str()
            .ok_or_else(|| invalid("query must be a nonempty bounded string"))?;
        if direct {
            self.validate_text(text, "query")?;
        } else if trim(text).is_empty() || text.chars().count() > self.max_query_length {
            return Err(invalid("query must be a nonempty bounded string"));
        }
        let intent = object
            .get("intent")
            .map_or(Some("web"), Value::as_str)
            .ok_or_else(|| invalid("unknown research intent"))?;
        let mut proposal = ScopeProposal::generic(SearchRequest {
            query: trim(text).into(),
            ..Default::default()
        });
        proposal.surface = ssx_application::policy::SearchSurface::Research;
        proposal.intent = intent.into();
        if let Some(value) = object.get("engines").filter(|v| !v.is_null()) {
            let names = value.as_array().filter(|v| !v.is_empty()).and_then(|v| {
                v.iter()
                    .map(|s| s.as_str().map(str::to_owned))
                    .collect::<Option<Vec<_>>>()
            });
            let Some(names) = names else {
                if let Err(error) = self.search.prepare_research_plan(&proposal) {
                    let e = query_policy(error, intent, direct);
                    if e.context.details.get("field").and_then(Value::as_str) == Some("intent")
                        || e.code == "invalid_intent"
                        || e.message == "unknown research intent"
                        || e.message == format!("intent {intent} requires the {intent} grant")
                    {
                        return Err(e);
                    }
                }
                return Err(if direct {
                    invalid("engines must be a nonempty list when supplied").with_field("engines")
                } else {
                    invalid("engines must be a nonempty list of names")
                });
            };
            proposal.request.engines = Some(names);
        }
        let authority = self
            .search
            .prepare_research_plan(&proposal)
            .map_err(|e| query_policy(e, intent, direct))?;
        let requested = proposal
            .request
            .engines
            .unwrap_or_else(|| authority.preview().scope.selected_engines.clone());
        let mut seen = BTreeSet::new();
        let engines = if direct {
            requested
                .into_iter()
                .take(usize::try_from(max_engines).unwrap_or(usize::MAX))
                .filter(|n| seen.insert(n.clone()))
                .collect()
        } else {
            requested
                .into_iter()
                .filter(|n| seen.insert(n.clone()))
                .take(usize::try_from(max_engines).unwrap_or(usize::MAX))
                .collect()
        };
        let mut query = ResearchQuery {
            query: trim(text).into(),
            intent: intent.into(),
            engines,
            subquestion_id: metadata(object.get("subquestion_id"), "subquestion_id", 128)?,
            rationale: metadata(object.get("rationale"), "rationale", self.max_query_length)?,
            parent_attempt_id: metadata(object.get("parent_attempt_id"), "parent_attempt_id", 128)?,
            continuation_key: if direct {
                metadata(continuation, "continuation_key", 128)?
            } else {
                None
            },
            ..Default::default()
        };
        query.planning_method = match object.get("planning_method") {
            None | Some(Value::Null) => None,
            Some(v) => Some(
                v.as_str()
                    .ok_or_else(|| invalid("unknown planning_method"))?
                    .to_owned(),
            ),
        };
        if query.planning_method.as_deref().is_some_and(|m| {
            ![
                "original",
                "decomposition",
                "terminology_expansion",
                "evidence_followup",
            ]
            .contains(&m)
        }) {
            return Err(invalid("unknown planning_method"));
        }
        query.evidence_result_ids = evidence_ids(object.get("evidence_result_ids"), direct)?;
        validate_metadata(&query)?;
        if query.engines.is_empty() {
            return Err(bad(
                "tool_disabled",
                "research query has no permitted engines",
            ));
        }
        Ok(query)
    }
    fn validate_text(&self, text: &str, field: &str) -> Result<()> {
        if trim(text).is_empty() {
            return Err(invalid(format!("{field} is required")).with_field(field));
        }
        if text.chars().count() > self.max_query_length {
            let mut e = invalid(format!(
                "{field} exceeds the maximum length of {} characters",
                self.max_query_length
            ))
            .with_field(field);
            if field == "query" {
                e = e.with_detail("max_length", json!(self.max_query_length));
            }
            return Err(e);
        }
        Ok(())
    }
    fn limits(&self, args: &Value, start: bool) -> Result<BTreeMap<String, u64>> {
        let c = self.config.ceilings;
        let mut values = BTreeMap::new();
        let mut fields = [
            ("queries", "max_queries", c.queries),
            (
                "engines_per_query",
                "max_engines_per_query",
                c.engines_per_query,
            ),
            ("attempts", "max_attempts", c.queries),
            (
                "engine_attempts",
                "max_engine_attempts",
                c.queries
                    .checked_mul(c.engines_per_query)
                    .ok_or_else(|| invalid("research budget overflow"))?,
            ),
            ("results", "max_results", c.results),
        ];
        // The original start function validates attempts before engines; the
        // preview helper validates engines before attempts.
        if start {
            fields.swap(1, 2);
        }
        for (key, field, ceiling) in fields {
            let n = match args.get(field) {
                None | Some(Value::Null) => ceiling,
                Some(v) => v
                    .as_u64()
                    .filter(|n| *n > 0)
                    .ok_or_else(|| invalid(format!("{field} must be a positive integer")))?
                    .min(ceiling),
            };
            values.insert(key.into(), n);
        }
        let cap = values["attempts"].saturating_mul(values["engines_per_query"]);
        values
            .entry("engine_attempts".into())
            .and_modify(|n| *n = (*n).min(cap));
        Ok(values)
    }
    fn question(&self, args: &Value) -> Result<String> {
        args["question"]
            .as_str()
            .filter(|s| !trim(s).is_empty() && s.chars().count() <= self.max_query_length)
            .map(|s| trim(s).to_owned())
            .ok_or_else(|| invalid("question must be a nonempty bounded string"))
    }
    pub async fn preview_initial(&self, args: &Value, variants: bool) -> Result<Value> {
        self.grant("preview").await?;
        let limits = self.limits(args, false)?;
        let question = args["question"].as_str().unwrap_or_default().to_owned();
        let list = args[if variants { "variants" } else { "subquestions" }]
            .as_array()
            .filter(|a| !a.is_empty() && (a.len() as u64) < limits["queries"])
            .ok_or_else(|| {
                invalid(if variants {
                    "variants must leave a query slot for the original"
                } else {
                    "subquestions must leave a query slot for the original"
                })
            })?;
        let intent = args.get("intent").cloned().unwrap_or(json!("web"));
        let engines = args.get("engines").cloned().unwrap_or(Value::Null);
        let mut entries = vec![
            json!({"query":question,"intent":intent,"engines":engines,"planning_method":"original"}),
        ];
        let mut declared = vec![];
        let mut seen = BTreeSet::new();
        for item in list {
            if variants {
                entries.push(json!({"query":item,"intent":intent,"engines":engines,"planning_method":"terminology_expansion","rationale":"Caller terminology proposal; generated wording is not evidence"}));
            } else {
                let o = item
                    .as_object()
                    .ok_or_else(|| invalid("invalid subquestion fields"))?;
                if o.keys().any(|k| {
                    !["id", "question", "query", "intent", "engines"].contains(&k.as_str())
                }) {
                    return Err(invalid("invalid subquestion fields"));
                }
                let id = metadata(o.get("id"), "subquestion id", 128)?
                    .ok_or_else(|| invalid("subquestion ids must be nonempty and unique"))?;
                let text = metadata(
                    o.get("question"),
                    "subquestion question",
                    self.max_query_length,
                )?
                .ok_or_else(|| invalid("subquestion ids must be nonempty and unique"))?;
                if !seen.insert(id.clone()) {
                    return Err(invalid("subquestion ids must be nonempty and unique"));
                }
                declared.push(json!({"id":id,"question":text}));
                entries.push(json!({"query":o.get("query").cloned().unwrap_or(json!(text)),"intent":o.get("intent").unwrap_or(&intent),"engines":o.get("engines").unwrap_or(&engines),"subquestion_id":id,"rationale":"Caller-decomposed evidence need","planning_method":"decomposition"}));
            }
        }
        let question = self.question(args)?;
        let queries = self
            .prepare_initial(&question, &entries, &declared, &limits, true)
            .await?;
        let arguments = json!({"question":question,"initial_plan":queries.iter().map(entry).collect::<Vec<_>>(),"subquestions":declared,"max_queries":limits["queries"],"max_attempts":limits["attempts"],"max_engines_per_query":limits["engines_per_query"],"max_engine_attempts":limits["engine_attempts"],"max_results":limits["results"]});
        Ok(
            json!({"contract":"slopsearx.research.plan","version":1,"execution":{"tool":"slopsearx_start_research","arguments":arguments},"plan_digest":sha(&arguments,true),"budget":{"limits":limits,"planned_queries":queries.len(),"planned_engine_attempts":queries.iter().map(|q|q.engines.len()).sum::<usize>()},"dispatched":false,"note":"Caller-proposed reading-lead searches; execution revalidates policy and budgets. No evidence sufficiency is certified."}),
        )
    }
    async fn prepare_initial(
        &self,
        question: &str,
        entries: &[Value],
        declared: &[Value],
        limits: &BTreeMap<String, u64>,
        preview: bool,
    ) -> Result<Vec<ResearchQuery>> {
        if entries.is_empty() || entries.len() as u64 > limits["queries"] {
            return Err(invalid("initial_plan must fit the positive query budget"));
        }
        if preview && entries.len() as u64 > limits["attempts"] {
            return Err(bad(
                "job_budget_exceeded",
                "plan exceeds query or attempt budget; no entries dropped",
            ));
        }
        let mut queries = Vec::new();
        for e in entries {
            queries.push(self.prepare_query(e, limits["engines_per_query"]).await?);
        }
        if preview
            && queries.iter().map(|q| q.engines.len() as u64).sum::<u64>()
                > limits["engine_attempts"]
        {
            return Err(bad(
                "job_budget_exceeded",
                "plan exceeds engine-attempt budget; no entries dropped",
            ));
        }
        let tagged = preview || queries.iter().any(|q| q.planning_method.is_some());
        let mut seen = BTreeSet::new();
        if tagged
            && queries
                .iter()
                .any(|q| !seen.insert(query_identity(&q.query, &q.engines)))
        {
            return Err(bad(
                "duplicate_query",
                "plan repeats the same query and source scope",
            ));
        }
        for (i, query) in queries.iter_mut().enumerate() {
            query.index = i as u64;
            if query.planning_method.as_deref() == Some("terminology_expansion") {
                validate_variant(question, &query.query)?;
            }
            if !preview && query.parent_attempt_id.is_some() {
                return Err(invalid("initial queries cannot reference parent attempts"));
            }
            if !preview
                && query
                    .subquestion_id
                    .as_ref()
                    .is_some_and(|id| !declared.iter().any(|s| s["id"].as_str() == Some(id)))
            {
                return Err(invalid("unknown subquestion_id"));
            }
        }
        if !preview
            && (queries.len() as u64 > limits["attempts"]
                || queries.iter().map(|q| q.engines.len() as u64).sum::<u64>()
                    > limits["engine_attempts"])
        {
            return Err(bad(
                "job_budget_exceeded",
                "initial plan exceeds execution budget",
            ));
        }
        Ok(queries)
    }
    /// Port of source's four deterministic templates. No query is generated
    /// from evidence, and no autonomous sufficiency/stopping policy is added.
    pub fn plan_defaults(
        &self,
        question: &str,
        strategy: &str,
        max_queries: u64,
        max_engines: u64,
    ) -> Result<(Vec<ResearchQuery>, Vec<String>)> {
        let specs: Vec<(&str, String, Option<&str>)> = match strategy {
            "triangulate" => vec![
                ("web", question.into(), None),
                ("science", question.into(), None),
                ("reference", question.into(), None),
            ],
            "broad" => vec![
                ("web", question.into(), None),
                ("news", question.into(), None),
                ("reference", question.into(), None),
                ("social", question.into(), None),
            ],
            "fresh" => vec![
                ("web", question.into(), Some("month")),
                ("news", question.into(), Some("day")),
            ],
            "counterevidence" => vec![
                ("reference", question.into(), None),
                ("science", format!("{question} limitations"), None),
                ("reference", format!("{question} criticism"), None),
                ("science", format!("{question} counterexample"), None),
            ],
            _ => return Err(invalid("unknown research strategy")),
        };
        let mut queries = Vec::new();
        let mut warnings = Vec::new();
        for (intent, text, time_range) in specs
            .into_iter()
            .take(usize::try_from(max_queries).unwrap_or(usize::MAX))
        {
            let (mut engines, notes) = self
                .search
                .research_template_scope(intent)
                .map_err(policy)?;
            warnings.extend(notes);
            engines.truncate(usize::try_from(max_engines).unwrap_or(usize::MAX));
            queries.push(ResearchQuery {
                index: queries.len() as u64,
                query: text,
                intent: intent.into(),
                engines,
                time_range: time_range.map(str::to_owned),
                requires_intent_grant: false,
                ..Default::default()
            });
        }
        Ok((queries, warnings))
    }
    pub async fn start(&self, tenant: TenantId, args: &Value) -> Result<ResearchJob> {
        self.admit(tenant, args).await.map(|(job, _)| job)
    }
    /// The creation bit is authoritative; idempotent start never schedules a second worker.
    pub async fn admit(&self, tenant: TenantId, args: &Value) -> Result<(ResearchJob, bool)> {
        self.grant("slopsearx_start_research").await?;
        let raw_question = args["question"]
            .as_str()
            .ok_or_else(|| invalid("question is required").with_field("question"))?;
        self.validate_text(raw_question, "question")?;
        let question = trim(raw_question).to_owned();
        if args.get("strategy").is_some_and(|v| {
            v.as_str()
                .is_none_or(|s| !["triangulate", "broad", "fresh", "counterevidence"].contains(&s))
        }) {
            return Err(invalid(format!(
                "unknown strategy '{}'",
                args["strategy"].as_str().unwrap_or_default()
            ))
            .with_field("strategy")
            .with_detail(
                "valid_alternatives",
                json!(["triangulate", "broad", "fresh", "counterevidence"]),
            ));
        }
        if args.get("source").is_some_and(|v| !v.is_null()) {
            return Err(invalid(
                "artifact-composed research admission belongs to W09; source was not consumed",
            ));
        }
        // Unlike bounded continuation metadata, source start keys are opaque
        // caller strings: preserve whitespace, empty values and long keys.
        let idempotency_key = match args.get("idempotency_key") {
            None | Some(Value::Null) => None,
            Some(v) => Some(
                v.as_str()
                    .ok_or_else(|| invalid("idempotency_key must be a string"))?
                    .to_owned(),
            ),
        };
        if let Some(key) = idempotency_key.as_ref().filter(|key| !key.is_empty()) {
            match self
                .jobs
                .for_tenant(tenant.clone())
                .find_by_idempotency(key)
                .await
            {
                JobRead::Found(existing) => {
                    if existing
                        .workflow
                        .get("composition")
                        .and_then(|v| v.get("request_digest"))
                        .is_some_and(|v| !v.is_null())
                    {
                        return Err(bad(
                            "idempotency_conflict",
                            "idempotency key refers to a different composed request",
                        ));
                    }
                    return Ok((*existing, false));
                }
                JobRead::Missing => {}
                _ => {
                    return Err(bad(
                        "store_unavailable",
                        "research job store is unavailable",
                    ));
                }
            }
        }
        let limits = self.limits(args, true)?;
        let declared = match args.get("subquestions") {
            None | Some(Value::Null) => vec![],
            Some(v) => v
                .as_array()
                .cloned()
                .ok_or_else(|| invalid("subquestions exceeds query budget"))?,
        };
        if declared.len() as u64 > limits["queries"] {
            return Err(invalid("subquestions exceeds query budget"));
        }
        let mut subquestions = ssx_json::Map::new();
        for item in &declared {
            let o = item
                .as_object()
                .ok_or_else(|| invalid("subquestion requires exactly id and question"))?;
            if o.len() != 2 || !o.contains_key("id") || !o.contains_key("question") {
                return Err(invalid("subquestion requires exactly id and question"));
            }
            let id = metadata(o.get("id"), "subquestion id", 128)?
                .ok_or_else(|| invalid("duplicate or missing subquestion"))?;
            let text = metadata(
                o.get("question"),
                "subquestion question",
                self.max_query_length,
            )?
            .ok_or_else(|| invalid("duplicate or missing subquestion"))?;
            if subquestions
                .insert(
                    id,
                    BTreeMap::from([
                        ("question".into(), text),
                        ("state".into(), "unresolved".into()),
                    ]),
                )
                .is_some()
            {
                return Err(invalid("duplicate or missing subquestion"));
            }
        }
        let timestamp = now().seconds();
        let deadline = resolve_deadline_at(
            args.get("deadline"),
            timestamp,
            self.default_deadline_seconds,
        )?;
        let strategy = args
            .get("strategy")
            .and_then(Value::as_str)
            .unwrap_or("triangulate");
        let (queries, warnings) = match args.get("initial_plan") {
            None | Some(Value::Null) => self.plan_defaults(
                &question,
                strategy,
                limits["queries"],
                limits["engines_per_query"],
            )?,
            Some(value) => {
                let entries = value
                    .as_array()
                    .ok_or_else(|| invalid("initial_plan must fit the positive query budget"))?;
                (
                    self.prepare_initial(&question, entries, &declared, &limits, false)
                        .await?,
                    Vec::new(),
                )
            }
        };
        for query in &queries {
            if !query.engines.is_empty() {
                self.search
                    .prepare_research_plan(&ScopeProposal::from_research_query(query))
                    .map_err(|e| policy(e).plain())?;
            }
        }
        let job = ResearchJob {
            job_id: format!("job-{}", random()),
            tenant: tenant.to_string(),
            question,
            strategy: args
                .get("strategy")
                .and_then(Value::as_str)
                .unwrap_or("triangulate")
                .into(),
            queries,
            warnings,
            subquestions,
            budget_limits: limits,
            budget_used: BTreeMap::from([
                ("attempts".into(), 0),
                ("engine_attempts".into(), 0),
                ("results".into(), 0),
            ]),
            created_at: FiniteF64::new(timestamp).map_err(domain)?,
            deadline: FiniteF64::new(deadline).map_err(domain)?,
            idempotency_key,
            ..Default::default()
        };
        self.jobs
            .for_tenant(tenant)
            .create(&job)
            .await
            .map_err(store_error)
    }
    /// Run the existing fixed-size durable polling pool. The composition owner
    /// must supervise exactly one pool per registry; the durable ready index,
    /// claims and current policy remain authoritative across all tenants.
    pub async fn run_workers(&self, stop: watch::Receiver<bool>) {
        self.runner.run(stop).await;
    }
    pub async fn run(
        &self,
        tenant: TenantId,
        id: &str,
        stop: watch::Receiver<bool>,
    ) -> Result<ResearchJob> {
        let id = JobId::new(id).map_err(domain)?;
        let scoped = self.jobs.for_tenant(tenant);
        let claim = scoped
            .claim_prepared(
                &id,
                &self.config.owner,
                self.config.lease_seconds,
                &mut Tokens,
                now(),
                attempt,
                false,
                |_| Ok(()),
            )
            .await
            .map_err(store_error)?
            .ok_or_else(|| bad("job_busy", "job is owned by a live worker"))?;
        self.runner
            .execute_claim(claim, stop)
            .await
            .map_err(runner_error)
    }
    fn associations(&self, job: &ResearchJob, q: &mut ResearchQuery, replay: bool) -> Result<()> {
        validate_metadata(q)?;
        if q.subquestion_id
            .as_ref()
            .is_some_and(|id| !job.subquestions.contains_key(id))
        {
            return Err(invalid("unknown subquestion_id"));
        }
        let parent = job
            .queries
            .iter()
            .flat_map(|q| q.attempts.iter().map(move |a| (q, a)))
            .find(|(_, a)| {
                Some(&a.attempt_id) == q.parent_attempt_id.as_ref() && a.state != "running"
            });
        if q.parent_attempt_id.is_some() && parent.is_none() {
            return Err(invalid(
                "parent_attempt_id must identify a terminal attempt in this job",
            ));
        }
        if !q.evidence_result_ids.is_empty() {
            let (p, a) =
                parent.ok_or_else(|| invalid("evidence parent must be a terminal attempt"))?;
            if !["done", "failed", "cancelled", "expired", "interrupted"]
                .contains(&a.state.as_str())
            {
                return Err(invalid("evidence parent must be a terminal attempt"));
            }
            if q.evidence_result_ids
                .iter()
                .any(|id| !a.admitted_result_ids.contains(id))
            {
                return Err(bad(
                    "invalid_result_id",
                    "evidence must be admitted results of the named parent",
                ));
            }
            // Source association order: admitted IDs, then the actual parent's
            // current dispatch policy (including its own captured evidence),
            // before resolved/duplicate/variant/budget checks. A bare parent
            // reference with no evidence IDs inherits no parent authority.
            self.search
                .prepare_research_plan(&ScopeProposal::from_research_query(p))
                .map_err(|error| PlanningError::new("policy_rejected", policy(error).message))?;
            q.evidence_engines = p.engines.clone();
            q.evidence_intent = p.requires_intent_grant.then(|| p.intent.clone());
        }
        if q.planning_method.is_some() && !replay {
            if q.planning_method.as_deref() == Some("evidence_followup")
                && q.subquestion_id
                    .as_ref()
                    .is_some_and(|id| job.subquestions[id]["state"] == "resolved")
            {
                return Err(bad(
                    "invalid_job_state",
                    "subquestion is already caller-resolved",
                ));
            }
            if job.queries.iter().any(|prior| {
                query_identity(&prior.query, &prior.engines) == query_identity(&q.query, &q.engines)
            }) {
                return Err(bad(
                    "duplicate_query",
                    "query already searched or planned in this scope",
                ));
            }
        }
        if q.planning_method.as_deref() == Some("terminology_expansion") {
            validate_variant(&job.question, &q.query)?;
        }
        Ok(())
    }
    async fn evidence(
        &self,
        tenant: TenantId,
        job: &ResearchJob,
        q: &ResearchQuery,
        read_snapshots: bool,
    ) -> Result<Evidence> {
        let mut authorities = Vec::new();
        let mut records = Vec::new();
        let mut expiry: Option<f64> = None;
        if !q.evidence_result_ids.is_empty() {
            for prior in &job.queries {
                if prior
                    .attempts
                    .iter()
                    .any(|a| Some(&a.attempt_id) == q.parent_attempt_id.as_ref())
                {
                    authorities.push(
                        self.search
                            .prepare_research_plan(&ScopeProposal::from_research_query(prior))
                            .map_err(|e| {
                                PlanningError::new("policy_rejected", policy(e).message)
                            })?,
                    );
                }
            }
        }
        authorities.push(
            self.search
                .prepare_research_plan(&ScopeProposal::from_research_query(q))
                .map_err(policy)?,
        );
        if read_snapshots {
            for result in &q.evidence_result_ids {
                let id = parse_result_id(result)?;
                let captured = match self
                    .snapshots
                    .for_tenant(tenant.clone())
                    .read_result(&id)
                    .await
                {
                    SnapshotResultLookup::Found(v) => v,
                    SnapshotResultLookup::Expired { expires_at } => {
                        return Err(bad("expired_handle", "snapshot has expired")
                            .with_field("result_id")
                            .with_detail("handle", json!(result))
                            .with_detail(
                                "expires_at",
                                json!(iso_timestamp(expires_at.timestamp().seconds())),
                            ));
                    }
                    SnapshotResultLookup::Unavailable => {
                        return Err(bad("store_unavailable", "snapshot store is unavailable")
                            .with_field("result_id"));
                    }
                    SnapshotResultLookup::Missing => {
                        return Err(bad("invalid_cursor", "unknown or expired snapshot")
                            .with_field("result_id"));
                    }
                    SnapshotResultLookup::InvalidIndex => {
                        return Err(bad("invalid_result_id", "result index out of range")
                            .with_field("result_id"));
                    }
                };
                if let Some(e) = captured.snapshot().expires_at() {
                    expiry = Some(expiry.map_or(e.timestamp().seconds(), |old| {
                        old.min(e.timestamp().seconds())
                    }));
                }
                let r = captured.result();
                // The source serializer always falls back to the primary engine,
                // even an empty name. Its shared gate runs before subset checking.
                let mut sources = r.engines.clone();
                if sources.is_empty() {
                    sources.insert(r.engine.clone());
                }
                let scope_only = ResearchQuery {
                    query: q.query.clone(),
                    intent: "web".into(),
                    engines: sources.iter().cloned().collect(),
                    requires_intent_grant: false,
                    ..Default::default()
                };
                authorities.push(
                    self.search
                        .prepare_research_plan(&ScopeProposal::from_research_query(&scope_only))
                        .map_err(policy)?,
                );
                if sources
                    .iter()
                    .any(|name| !q.evidence_engines.contains(name))
                {
                    return Err(bad(
                        "invalid_result_id",
                        "captured evidence does not match its parent source scope",
                    ));
                }
                records.push(json!({"result_id":result,"title":r.title,"citation":json!({"label":r.title,"url":r.url})}));
            }
        }
        Ok(Evidence {
            authorities,
            earliest_expiry: expiry,
            records,
        })
    }
    fn remaining(job: &ResearchJob, query: &ResearchQuery, preview: bool) -> Result<()> {
        if preview {
            let rem = job.budget_summary().remaining;
            if [
                ("queries", 1),
                ("attempts", 1),
                ("engine_attempts", query.engines.len() as u64),
                ("results", 1),
            ]
            .iter()
            .any(|(k, n)| rem.get(*k).copied().unwrap_or(0) < *n)
            {
                return Err(bad(
                    "job_budget_exceeded",
                    "job has insufficient remaining budget",
                ));
            }
        } else {
            if job.queries.len() as u64 >= job.budget_limits["queries"] {
                return Err(bad("job_budget_exceeded", "job query budget is exhausted"));
            }
            for (key, amount) in [
                ("attempts", 1),
                ("engine_attempts", query.engines.len() as u64),
                ("results", 1),
            ] {
                if job.budget_used[key].saturating_add(amount) > job.budget_limits[key] {
                    return Err(PlanningError::new(
                        "job_budget_exceeded",
                        format!("job {key} budget is exhausted"),
                    ));
                }
            }
            if query.engines.len() as u64 > job.budget_limits["engines_per_query"] {
                return Err(bad(
                    "job_budget_exceeded",
                    "follow-up exceeds per-query engine budget",
                ));
            }
        }
        Ok(())
    }
    fn open(job: &ResearchJob, preview: bool) -> Result<()> {
        if preview {
            if ["queued", "running", "cancelled", "expired"].contains(&job.state.as_str())
                || job.caller_completed
            {
                return Err(bad(
                    "invalid_job_state",
                    "job must be idle and open to continuations",
                ));
            }
            if now().seconds() >= job.deadline.get() {
                return Err(bad("deadline_exceeded", "job deadline has passed"));
            }
        } else {
            if now().seconds() >= job.deadline.get() {
                return Err(bad(
                    "deadline_exceeded",
                    "job deadline has passed; cannot extend",
                )
                .with_field("query"));
            }
            if ["cancelled", "expired"].contains(&job.state.as_str()) || job.caller_completed {
                return Err(PlanningError::new(
                    "invalid_job_state",
                    format!("job is in terminal state '{}'; cannot extend", job.state),
                )
                .with_field("query")
                .with_detail("job_id", json!(job.job_id))
                .with_detail("state", json!(job.state)));
            }
        }
        Ok(())
    }
    async fn followup(
        &self,
        tenant: TenantId,
        args: &Value,
        preview: bool,
    ) -> Result<(ResearchJob, ResearchQuery, Evidence)> {
        let id = args["job_id"]
            .as_str()
            .ok_or_else(|| invalid("job_id is required"))?;
        self.grant(if preview {
            "preview"
        } else {
            "slopsearx_extend_research"
        })
        .await?;
        let mut job = self
            .read_job(
                tenant.clone(),
                id,
                if preview { "preview" } else { "public" },
                !preview,
            )
            .await?;
        if preview {
            self.retained_scope(&job)?;
        }
        if !job.workflow.is_empty() {
            if preview {
                self.search
                    .validate_research_workflow(&job)
                    .map_err(workflow_error)?;
            }
            let e = invalid("workflow research jobs cannot be extended");
            return Err(if preview { e } else { e.with_field("job_id") });
        }
        if preview {
            Self::open(&job, true)?;
        }
        job.initialize_budget(self.config.ceilings)
            .map_err(domain)?;
        let mut input = args.clone();
        input
            .as_object_mut()
            .ok_or_else(|| invalid("invalid followup"))?
            .shift_remove("job_id");
        input
            .as_object_mut()
            .unwrap()
            .shift_remove("continuation_key");
        if preview {
            input["planning_method"] = json!("evidence_followup");
        }
        if !preview && input.get("evidence_result_ids").is_some_and(Value::is_null) {
            input
                .as_object_mut()
                .unwrap()
                .shift_remove("evidence_result_ids");
        }
        let mut q = self
            .prepare_query_kind(
                &input,
                if preview {
                    job.budget_limits["engines_per_query"]
                } else {
                    self.config.ceilings.engines_per_query
                },
                !preview,
                args.get("continuation_key"),
            )
            .await?;
        q.continuation_key = metadata(args.get("continuation_key"), "continuation_key", 128)?;
        q.continuation_digest = Some(continuation_digest(&q));
        let replay = !preview
            && q.continuation_key.as_ref().is_some_and(|key| {
                job.queries
                    .iter()
                    .any(|prior| prior.continuation_key.as_ref() == Some(key))
            });
        if replay {
            let prior = job
                .queries
                .iter()
                .find(|p| p.continuation_key == q.continuation_key)
                .unwrap();
            if prior.continuation_digest != q.continuation_digest {
                return Err(bad(
                    "idempotency_conflict",
                    "continuation_key was used for another request",
                ));
            }
        } else {
            Self::open(&job, preview)?;
        }
        if !replay || !q.evidence_result_ids.is_empty() {
            self.associations(&job, &mut q, replay).map_err(|e| {
                if !preview && !replay {
                    e.with_field("query")
                } else {
                    e
                }
            })?;
        }
        if !replay && preview {
            Self::remaining(&job, &q, true)?;
        }
        let evidence = self
            .evidence(tenant, &job, &q, !replay)
            .await
            .map_err(|e| {
                if !preview && !replay && e.code == "policy_rejected" {
                    e.with_field("query")
                } else {
                    e
                }
            })?;
        Ok((job, q, evidence))
    }
    pub async fn preview_followup(&self, tenant: TenantId, args: &Value) -> Result<Value> {
        let (job, q, e) = self.followup(tenant, args, true).await?;
        let mut arguments = entry(&q);
        arguments["job_id"] = args["job_id"].clone();
        Ok(
            json!({"contract":"slopsearx.research.followup_plan","version":1,"execution":{"tool":"slopsearx_extend_research","arguments":arguments},"evidence":e.records,"budget":job.budget_summary(),"dispatched":false,"note":"Evidence links attribute the caller's proposal; snippets are untrusted leads, not verified facts."}),
        )
    }
    pub async fn extend(
        &self,
        tenant: TenantId,
        args: &Value,
        stop: watch::Receiver<bool>,
    ) -> Result<ResearchJob> {
        let (job, mut q, evidence) = self.followup(tenant.clone(), args, false).await?;
        if q.continuation_key.is_some()
            && job
                .queries
                .iter()
                .any(|prior| prior.continuation_key == q.continuation_key)
        {
            return Ok(job);
        }
        let id = JobId::new(&job.job_id).map_err(domain)?;
        let scoped = self.jobs.for_tenant(tenant.clone());
        let mut appended = false;
        let mut rejected = None;
        let outcome = scoped
            .claim_prepared(
                &id,
                &self.config.owner,
                self.config.lease_seconds,
                &mut Tokens,
                now(),
                attempt,
                false,
                |current| {
                    current.initialize_budget(self.config.ceilings)?;
                    if current.caller_completed {
                        return Err(fenced_into(
                            &mut rejected,
                            bad("invalid_job_state", "caller already completed this job"),
                        ));
                    }
                    Self::open(current, false).map_err(|e| fenced_into(&mut rejected, e))?;
                    if let Some(prior) = current.queries.iter().find(|prior| {
                        q.continuation_key.is_some() && prior.continuation_key == q.continuation_key
                    }) {
                        return Err(fenced_into(
                            &mut rejected,
                            bad(
                                if prior.continuation_digest == q.continuation_digest {
                                    "continuation_replayed"
                                } else {
                                    "idempotency_conflict"
                                },
                                if prior.continuation_digest == q.continuation_digest {
                                    "continuation already accepted"
                                } else {
                                    "continuation_key was used for another request"
                                },
                            ),
                        ));
                    }
                    self.associations(current, &mut q, false)
                        .map_err(|e| fenced_into(&mut rejected, e))?;
                    Self::remaining(current, &q, false)
                        .map_err(|e| fenced_into(&mut rejected, e))?;
                    if evidence
                        .earliest_expiry
                        .is_some_and(|expiry| now().seconds() >= expiry)
                    {
                        return Err(fenced_into(
                            &mut rejected,
                            bad("expired_handle", "snapshot has expired"),
                        ));
                    }
                    for authority in &evidence.authorities {
                        self.search
                            .revalidate_research_scope(authority)
                            .map_err(|_| {
                                fenced_into(
                                    &mut rejected,
                                    bad(
                                        "policy_rejected",
                                        "current evidence policy rejected the plan",
                                    ),
                                )
                            })?;
                    }
                    q.index = current.queries.len() as u64;
                    current.queries.push(q);
                    appended = true;
                    Ok(())
                },
            )
            .await;
        let claim = match outcome {
            Ok(Some(c)) => c,
            Ok(None) => return Err(bad("job_busy", "job is owned by a live worker")),
            Err(e) => {
                let error = rejected.take().unwrap_or_else(|| store_error(e));
                if error.code == "continuation_replayed" {
                    return self.load(tenant, id.as_str()).await;
                }
                return Err(error.with_field("query"));
            }
        };
        let settled = self
            .runner
            .execute_claim(claim, stop)
            .await
            .map_err(runner_error)?;
        if !appended && settled.caller_completed {
            return Err(
                bad("invalid_job_state", "caller already completed this job").with_field("query"),
            );
        }
        Ok(settled)
    }
    pub async fn retry(
        &self,
        tenant: TenantId,
        id: &str,
        stop: watch::Receiver<bool>,
    ) -> Result<ResearchJob> {
        self.grant("slopsearx_retry_research").await?;
        let job = self.get_job(tenant.clone(), id).await?;
        workflow_budget(&job)?;
        if job.caller_completed
            || ["cancelled", "expired"].contains(&job.state.as_str())
            || !job.queries.iter().any(ResearchQuery::retryable)
        {
            return Ok(job);
        }
        self.runner
            .retry(tenant, &JobId::new(id).map_err(domain)?, stop)
            .await
            .map_err(runner_error)
    }
    pub async fn retry_public(
        &self,
        tenant: TenantId,
        id: &str,
        stop: watch::Receiver<bool>,
    ) -> Result<Value> {
        self.grant("slopsearx_retry_research").await?;
        let before = self.get_job(tenant.clone(), id).await?;
        workflow_budget(&before)?;
        if before.caller_completed || ["cancelled", "expired"].contains(&before.state.as_str()) {
            return Ok(
                json!({"job_id":before.job_id,"state":before.state,"note":"job already reached a terminal state; it was not retried"}),
            );
        }
        if !before.queries.iter().any(ResearchQuery::retryable) {
            return Ok(
                json!({"error":{"code":"no_retryable_work","message":"job has no failed or empty subqueries to retry","job_id":before.job_id,"state":before.state,"field":"job_id"}}),
            );
        }
        let job = match self
            .runner
            .retry(tenant, &JobId::new(id).map_err(domain)?, stop)
            .await
        {
            Ok(job) => job,
            Err(RunnerError::LiveOwner) => {
                return Ok(
                    json!({"job_id":id,"state":"running","note":"job is still running under a live worker; retry was not started to avoid concurrent execution"}),
                );
            }
            Err(e) => return Err(runner_error(e)),
        };
        if job.state == "cancelled" {
            return Ok(
                json!({"job_id":id,"state":"cancelled","note":"the job had a pending cancellation request; the retry was cancelled, not executed"}),
            );
        }
        let mut value = job_summary(&job);
        if job.state == "expired" {
            value["note"] = json!(
                "job deadline had already passed; retry finalized the job to expired and re-executed no subqueries"
            );
        } else {
            let retried: Vec<_> = job
                .queries
                .iter()
                .filter(|q| {
                    q.attempts.len()
                        > before
                            .queries
                            .iter()
                            .find(|old| old.index == q.index)
                            .map_or(0, |old| old.attempts.len())
                })
                .map(|q| q.index)
                .collect();
            value["retried"] = json!(retried);
            value["note"] = json!(
                "retry accounting recorded; inspect retried indices and stop_reason for executed work"
            );
        }
        Ok(value)
    }
    pub async fn cancel(&self, tenant: TenantId, id: &str) -> Result<ResearchJob> {
        // Cancelling retained work needs no new search grant or dispatch authority.
        let _ = self.read_job(tenant.clone(), id, "public", false).await?;
        self.jobs
            .for_tenant(tenant.clone())
            .request_cancel(
                &JobId::new(id).map_err(domain)?,
                now(),
                &mut Tokens,
                attempt,
            )
            .await
            .map_err(store_error)?;
        self.read_job(tenant, id, "public", false).await
    }
    pub async fn update(&self, tenant: TenantId, args: &Value) -> Result<ResearchJob> {
        let id = args["job_id"]
            .as_str()
            .ok_or_else(|| invalid("job_id is required"))?;
        self.grant("update").await?;
        let job = self.read_job(tenant.clone(), id, "update", false).await?;
        let states: BTreeMap<String, String> = args["subquestion_states"]
            .as_object()
            .ok_or_else(|| invalid("subquestion states must be resolved or unresolved"))?
            .iter()
            .map(|(k, v)| {
                v.as_str()
                    .filter(|s| ["resolved", "unresolved"].contains(s))
                    .map(|s| (k.clone(), s.into()))
                    .ok_or_else(|| invalid("subquestion states must be resolved or unresolved"))
            })
            .collect::<Result<_>>()?;
        let complete = match args.get("complete") {
            None => false,
            Some(v) => v
                .as_bool()
                .ok_or_else(|| invalid("complete must be a boolean"))?,
        };
        let rationale = metadata(args.get("rationale"), "rationale", self.max_query_length)?;
        let matches = |j: &ResearchJob| {
            j.completion_rationale == rationale
                && j.completion_subquestion_states.as_ref().map_or_else(
                    || {
                        !states.is_empty()
                            && states.iter().all(|(id, s)| {
                                j.subquestions.get(id).and_then(|v| v.get("state")) == Some(s)
                            })
                    },
                    |old| old == &states,
                )
        };
        if job.caller_completed && complete {
            if states.keys().any(|k| !job.subquestions.contains_key(k)) {
                return Err(invalid("unknown subquestion id"));
            }
            if matches(&job) {
                return Ok(job);
            }
            return Err(bad(
                "idempotency_conflict",
                "completion request conflicts with the persisted completion",
            ));
        }
        if ["cancelled", "expired"].contains(&job.state.as_str()) || job.caller_completed {
            return Err(bad("invalid_job_state", "job is already terminal"));
        }
        let scoped = self.jobs.for_tenant(tenant);
        let id = JobId::new(id).map_err(domain)?;
        let mut rejected = None;
        let claim = scoped
            .claim_prepared(
                &id,
                &self.config.owner,
                self.config.lease_seconds,
                &mut Tokens,
                now(),
                attempt,
                complete,
                |current| {
                    if states.keys().any(|k| !current.subquestions.contains_key(k)) {
                        return Err(fenced_into(
                            &mut rejected,
                            invalid("unknown subquestion id"),
                        ));
                    }
                    if current.caller_completed {
                        if complete && matches(current) {
                            return Ok(());
                        }
                        return Err(fenced_into(
                            &mut rejected,
                            bad(
                                "idempotency_conflict",
                                "completion request conflicts with the persisted completion",
                            ),
                        ));
                    }
                    current.initialize_budget(self.config.ceilings)?;
                    for (id, state) in &states {
                        current
                            .subquestions
                            .get_mut(id)
                            .unwrap()
                            .insert("state".into(), state.clone());
                    }
                    if complete {
                        current.caller_completed = true;
                        current.completion_rationale = rationale.clone();
                        current.completion_subquestion_states = Some(states.clone());
                        if ![
                            "result_budget_exhausted",
                            "query_budget_exhausted",
                            "attempt_budget_exhausted",
                            "engine_budget_exhausted",
                        ]
                        .contains(&current.stop_reason.as_deref().unwrap_or(""))
                        {
                            current.stop_reason = Some("caller_completed".into());
                            for q in &mut current.queries {
                                if ["pending", "running"].contains(&q.state.as_str()) {
                                    q.state = "cancelled".into();
                                }
                            }
                            current.state = "succeeded".into();
                        }
                    }
                    Ok(())
                },
            )
            .await
            .map_err(|e| rejected.take().unwrap_or_else(|| store_error(e)))?
            .ok_or_else(|| {
                bad(
                    "job_busy",
                    "job is owned by a live worker; retry the progress update later",
                )
            })?;
        let output = claim.job;
        scoped
            .release(&id, &claim.token)
            .await
            .map_err(store_error)?;
        Ok(output)
    }
}
fn runner_error(e: RunnerError) -> PlanningError {
    match e {
        RunnerError::Store(e) => store_error(e),
        RunnerError::Policy(e) => policy(e),
        RunnerError::LiveOwner => bad("job_busy", "job is owned by a live worker"),
        _ => bad("execution_failed", "research execution did not complete"),
    }
}

fn parse_result_id(value: &str) -> Result<ResultId> {
    let Some((snapshot, index)) = value.rsplit_once(':') else {
        return Err(bad(
            "invalid_result_id",
            "result_id must be a server-issued snapshot result ID",
        )
        .with_field("result_id"));
    };
    let malformed = || bad("invalid_result_id", "malformed result_id").with_field("result_id");
    let mut digits = trim(index);
    let negative = digits.starts_with('-');
    if digits.starts_with(['-', '+']) {
        digits = &digits[1..];
    }
    if digits.is_empty() {
        return Err(malformed());
    }
    let mut integer = 0u64;
    let mut previous_digit = false;
    let mut nonzero = false;
    for ch in digits.chars() {
        if ch == '_' && previous_digit {
            previous_digit = false;
            continue;
        }
        let Some(digit) = ssx_json::decimal_value(ch) else {
            return Err(malformed());
        };
        integer = integer.saturating_mul(10).saturating_add(u64::from(digit));
        nonzero |= digit != 0;
        previous_digit = true;
    }
    if !previous_digit {
        return Err(malformed());
    }
    if negative && nonzero {
        return Err(bad("invalid_result_id", "result index out of range").with_field("result_id"));
    }
    let snapshot = SnapshotId::new(snapshot).map_err(|_| {
        bad("invalid_cursor", "unknown or expired snapshot").with_field("result_id")
    })?;
    Ok(ResultId::new(snapshot, integer))
}

pub fn job_summary(job: &ResearchJob) -> Value {
    fn artifact(kind: &str, id: &str) -> Value {
        json!({"contract":"slopsearx.artifact_ref","version":1,"kind":kind,"id":id})
    }
    fn snapshot(id: &Option<String>) -> Value {
        id.as_ref().map_or(Value::Null, |s| artifact("snapshot", s))
    }
    fn coverage(values: &[ssx_domain::research::EngineCoverage]) -> Value {
        let c = ssx_domain::research::summarize_coverage(values).expect("validated coverage");
        json!({"attempted":c.attempted,"successful":c.successful,"empty":c.empty,"failed":c.failed,"unavailable":c.unavailable,"not_selected":c.not_selected})
    }
    let (completed, total) = job.progress();
    let all: Vec<_> = job
        .queries
        .iter()
        .flat_map(|q| q.engine_coverage.clone())
        .collect();
    let queries:Vec<_>=job.queries.iter().map(|q|{
        let attempts:Vec<_>=q.attempts.iter().map(|a|{
            let mut value=ssx_json::to_value(a).expect("attempt serialization");
            value["artifact"]=if a.attempt_id.is_empty(){Value::Null}else{artifact("research_attempt",&ssx_domain::canonical::composite_artifact_id(&[&job.job_id,&a.attempt_id]).expect("valid attempt identity"))};value["snapshot_artifact"]=snapshot(&a.cursor);value
        }).collect();
        json!({"index":q.index,"query":q.query,"intent":q.intent,"engines":q.engines,"state":q.state,"result_count":q.result_count,"query_id":q.query_id,"cursor":q.cursor,"snapshot_artifact":snapshot(&q.cursor),"error":q.error,"subquestion_id":q.subquestion_id,"rationale":q.rationale,"parent_attempt_id":q.parent_attempt_id,"planning_method":q.planning_method,"evidence_result_ids":q.evidence_result_ids,"continuation_key":q.continuation_key,"attempts":attempts,"engine_coverage":q.engine_coverage,"coverage":coverage(&q.engine_coverage),"enforcement":q.enforcement})
    }).collect();
    let deadline = iso_timestamp(job.deadline.get());
    json!({"job_id":job.job_id,"artifact":artifact(if job.workflow.get("kind").and_then(Value::as_str)==Some("dependency_dossier"){"dependency_dossier"}else{"research_job"},&job.job_id),"state":job.state,"question":job.question,"strategy":job.strategy,"progress":{"completed":completed,"total":total},"deadline":deadline,"idempotency_key":job.idempotency_key,"queries":queries,"coverage":coverage(&all),"warnings":job.warnings,"subquestions":job.subquestions,"unresolved_subquestions":job.subquestions.iter().filter(|(_,s)|s.get("state").map(String::as_str)==Some("unresolved")).map(|(id,_)|id).collect::<Vec<_>>(),"caller_completed":job.caller_completed,"completion_rationale":job.completion_rationale,"stop_reason":job.stop_reason,"budgets":job.budget_summary()})
}

pub fn iso_timestamp(seconds: f64) -> Option<String> {
    let micros = (seconds * 1e6).round();
    if !micros.is_finite() || micros < i64::MIN as f64 || micros > i64::MAX as f64 {
        return None;
    }
    chrono::DateTime::from_timestamp_micros(micros as i64).map(|v| {
        v.to_rfc3339_opts(
            if v.timestamp_subsec_micros() == 0 {
                chrono::SecondsFormat::Secs
            } else {
                chrono::SecondsFormat::Micros
            },
            false,
        )
    })
}

/// CPython ISO datetime input accepted by research admission. Numeric fields are
/// ASCII; the date/time separator may be one Unicode character. Date/week/basic
/// forms, local naive times, fractional offsets and microsecond truncation remain
/// explicit, instead of accepting only RFC3339 and silently replacing valid input.
pub fn parse_iso_deadline(input: &str) -> Option<f64> {
    let normalized = input.replace('Z', "+00:00");
    let chars: Vec<_> = normalized.chars().collect();
    let n = chars.len();
    if n < 7 {
        return None;
    }
    let separator = if n == 7 {
        7
    } else if chars.get(4) == Some(&'-') {
        if chars.get(5) == Some(&'W') {
            if n > 8 && chars.get(8) == Some(&'-') {
                if n == 9 {
                    return None;
                }
                if n > 10 && chars[10].is_ascii_digit() {
                    8
                } else {
                    10
                }
            } else {
                8
            }
        } else {
            10
        }
    } else if chars.get(4) == Some(&'W') {
        let mut index = 7;
        while index < n && chars[index].is_ascii_digit() {
            index += 1;
        }
        if index < 9 {
            index
        } else if index % 2 == 0 {
            7
        } else {
            8
        }
    } else {
        8
    };
    if separator > n || n == separator + 1 || ![7, 8, 10].contains(&separator) {
        return None;
    }
    let digits = |start: usize, count: usize| -> Option<u32> {
        let slice = chars.get(start..start + count)?;
        if slice.iter().any(|c| !c.is_ascii_digit()) {
            return None;
        }
        slice.iter().try_fold(0u32, |v, c| {
            v.checked_mul(10)?.checked_add(*c as u32 - '0' as u32)
        })
    };
    let year = digits(0, 4)?;
    if !(1..=9999).contains(&year) {
        return None;
    }
    let has_sep = chars.get(4) == Some(&'-');
    let mut pos = 4 + usize::from(has_sep);
    let date = if chars.get(pos) == Some(&'W') {
        pos += 1;
        let week = digits(pos, 2)?;
        pos += 2;
        let day = if separator > pos {
            if (chars.get(pos) == Some(&'-')) != has_sep {
                return None;
            }
            pos += usize::from(has_sep);
            digits(pos, 1)?
        } else {
            1
        };
        let weekday = match day {
            1 => chrono::Weekday::Mon,
            2 => chrono::Weekday::Tue,
            3 => chrono::Weekday::Wed,
            4 => chrono::Weekday::Thu,
            5 => chrono::Weekday::Fri,
            6 => chrono::Weekday::Sat,
            7 => chrono::Weekday::Sun,
            _ => return None,
        };
        chrono::NaiveDate::from_isoywd_opt(year as i32, week, weekday)?
    } else {
        let month = digits(pos, 2)?;
        pos += 2;
        if (chars.get(pos) == Some(&'-')) != has_sep {
            return None;
        }
        pos += usize::from(has_sep);
        let day = digits(pos, 2)?;
        chrono::NaiveDate::from_ymd_opt(year as i32, month, day)?
    };
    let remaining = if n == separator {
        String::new()
    } else {
        chars.get(separator + 1..)?.iter().collect::<String>()
    };
    fn time(value: &str) -> Option<[u32; 4]> {
        let bytes = value.as_bytes();
        let mut out = [0; 4];
        let mut pos = 0;
        let mut colon = false;
        for (component, slot) in out.iter_mut().enumerate().take(3) {
            let pair = bytes.get(pos..pos + 2)?;
            if !pair.iter().all(u8::is_ascii_digit) {
                return None;
            }
            *slot = ((pair[0] - b'0') as u32) * 10 + (pair[1] - b'0') as u32;
            pos += 2;
            if component == 0 {
                colon = bytes.get(pos) == Some(&b':');
            }
            if component < 2 && matches!(bytes.get(pos), Some(b'.' | b',')) {
                return None;
            }
            if pos == bytes.len() || component == 2 {
                break;
            }
            if colon {
                if bytes.get(pos) != Some(&b':') {
                    return None;
                }
                pos += 1;
            }
        }
        if pos < bytes.len() {
            if !matches!(bytes[pos], b'.' | b',') {
                return None;
            }
            pos += 1;
            let fraction = bytes.get(pos..)?;
            if fraction.is_empty() || !fraction.iter().all(u8::is_ascii_digit) {
                return None;
            }
            let take = fraction.len().min(6);
            let mut micros = 0;
            for c in &fraction[..take] {
                micros = micros * 10 + (c - b'0') as u32;
            }
            out[3] = micros * 10u32.pow((6 - take) as u32);
        }
        Some(out)
    }
    let (components, offset) = if remaining.is_empty() {
        ([0; 4], None)
    } else {
        let tz = remaining.find('-').or_else(|| remaining.find('+'));
        let (main, offset) = if let Some(i) = tz {
            let zone = &remaining[i + 1..];
            if [0, 1, 3].contains(&zone.len()) {
                return None;
            }
            let z = time(zone)?;
            let total =
                (z[0] as f64) * 3600.0 + (z[1] as f64) * 60.0 + z[2] as f64 + (z[3] as f64) / 1e6;
            if total >= 86400.0 {
                return None;
            }
            (
                &remaining[..i],
                Some(if remaining.as_bytes()[i] == b'-' {
                    -total
                } else {
                    total
                }),
            )
        } else {
            (remaining.as_str(), None)
        };
        (time(main)?, offset)
    };
    let mut day = date;
    let mut hour = components[0];
    if hour == 24 && components[1..].iter().all(|n| *n == 0) {
        day = day.succ_opt()?;
        hour = 0;
    }
    let naive = day.and_hms_micro_opt(hour, components[1], components[2], components[3])?;
    Some(if let Some(offset) = offset {
        naive.and_utc().timestamp_micros() as f64 / 1e6 - offset
    } else {
        naive_local_timestamp(naive)?
    })
}
/// CPython datetime._mktime fold=0: solve t=local(u), probe24h for a
/// repeated local time, and choose max(u1,u2) for a skipped local time.
/// Parsing a local DST gap must not become an invalid ISO timestamp.
fn naive_local_timestamp(naive: chrono::NaiveDateTime) -> Option<f64> {
    let local = |seconds: i64| -> Option<i64> {
        let value = chrono::DateTime::from_timestamp(seconds, 0)?
            .with_timezone(&chrono::Local)
            .naive_local();
        (1..=9999)
            .contains(&chrono::Datelike::year(&value))
            .then_some(value.and_utc().timestamp())
    };
    let utc = naive.and_utc();
    let t = utc.timestamp();
    let fraction = utc.timestamp_subsec_micros() as f64 / 1e6;
    let a = local(t)?.checked_sub(t)?;
    let u1 = t.checked_sub(a)?;
    let t1 = local(u1)?;
    let b = if t1 == t {
        let probe = u1.checked_sub(86_400)?;
        let b = local(probe)?.checked_sub(probe)?;
        if a == b {
            return Some(u1 as f64 + fraction);
        }
        b
    } else {
        t1.checked_sub(u1)?
    };
    let u2 = t.checked_sub(b)?;
    let t2 = local(u2)?;
    let seconds = if t2 == t {
        u2
    } else if t1 == t {
        u1
    } else {
        u1.max(u2)
    };
    Some(seconds as f64 + fraction)
}
pub fn resolve_deadline_at(value: Option<&Value>, now: f64, default_seconds: u64) -> Result<f64> {
    let requested = match value {
        None | Some(Value::Null) => now + default_seconds as f64,
        Some(Value::String(text)) if text.is_empty() => now + default_seconds as f64,
        Some(Value::String(text)) => parse_iso_deadline(text).ok_or_else(|| {
            invalid("deadline must be an ISO 8601 timestamp").with_field("deadline")
        })?,
        _ => return Err(invalid("deadline must be an ISO 8601 timestamp").with_field("deadline")),
    };
    Ok(requested.clamp(now + 60.0, now + 86_400.0))
}
