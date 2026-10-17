//! Shared fail-closed search authority. All inputs are pure, secret-free snapshots.
use crate::{
    filters::{EnforcementEntry, PublicationBounds, resolve_filter_enforcement, time_range_days},
    scope::{CatalogSnapshot, ScopeResolver, Topic},
};
use ssx_domain::{ScopeDecision, SearchRequest};
use ssx_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchSurface {
    Http,
    Generic,
    Targeted,
    Jobs,
    Security,
    Science,
    Research,
    Preview,
    Worker,
}
#[derive(Debug, Clone)]
pub struct SearchPolicy {
    pub grants: BTreeSet<String>,
    pub sensitive_engines: BTreeSet<String>,
    pub sensitive_allowed: bool,
    pub max_query_length: usize,
}
impl Default for SearchPolicy {
    fn default() -> Self {
        Self {
            grants: BTreeSet::new(),
            sensitive_engines: ["hibp", "dehashed"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            sensitive_allowed: false,
            max_query_length: 500,
        }
    }
}
#[derive(Debug, Clone)]
struct ResearchPolicyContext {
    requires_intent_grant: bool,
    planning_method: Option<String>,
    evidence_result_ids: Vec<String>,
    evidence_engines: Vec<String>,
    evidence_intent: Option<String>,
    parent: Option<String>,
    rationale: Option<String>,
}
#[derive(Debug, Clone)]
pub struct ScopeProposal {
    pub request: SearchRequest,
    pub intent: String,
    /// Trusted transport selection supplied by server composition, never a wire
    /// field. HTTP deliberately has different access policy from MCP. Callers
    /// must not copy this from request or persisted-record data; recovered
    /// research must use `from_research_query`.
    pub surface: SearchSurface,
    /// Security evidence types or science source types, in caller order.
    pub specialist_types: Vec<String>,
    pub job_location: Option<String>,
    pub job_employment_type: Option<String>,
    research: Option<ResearchPolicyContext>,
}
impl ScopeProposal {
    /// Project recovered source policy metadata, not dispatch authority. W02 must
    /// obtain this record under its authoritative tenant/lease/budget boundary.
    pub fn from_research_query(q: &ssx_domain::research::ResearchQuery) -> Self {
        let mut p = Self::generic(SearchRequest {
            query: q.query.clone(),
            engines: Some(q.engines.clone()),
            time_range: q.time_range.clone(),
            ..Default::default()
        });
        p.surface = SearchSurface::Research;
        p.intent = q.intent.clone();
        p.research = Some(ResearchPolicyContext {
            requires_intent_grant: q.requires_intent_grant,
            planning_method: q.planning_method.clone(),
            evidence_result_ids: q.evidence_result_ids.clone(),
            evidence_engines: q.evidence_engines.clone(),
            evidence_intent: q.evidence_intent.clone(),
            parent: q.parent_attempt_id.clone(),
            rationale: q.rationale.clone(),
        });
        p
    }
    pub fn generic(request: SearchRequest) -> Self {
        Self {
            request,
            intent: "auto".into(),
            surface: SearchSurface::Generic,
            specialist_types: Vec::new(),
            job_location: None,
            job_employment_type: None,
            research: None,
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct PolicyError {
    pub code: &'static str,
    pub field: String,
    pub message: String,
    pub details: Box<Value>,
    pub enforcement: BTreeMap<String, EnforcementEntry>,
}
impl PolicyError {
    fn new(code: &'static str, field: &str, message: impl Into<String>) -> Self {
        Self {
            code,
            field: field.into(),
            message: message.into(),
            details: Box::new(json!({})),
            enforcement: BTreeMap::new(),
        }
    }
    pub fn to_value(&self) -> Value {
        let mut error = json!({"code":self.code,"message":self.message});
        if !self.field.is_empty() {
            error["field"] = json!(self.field);
        }
        if let Some(d) = self.details.as_object() {
            for (k, v) in d {
                error[k.as_str()] = v.clone();
            }
        }
        let mut out = json!({"error":error});
        if !self.enforcement.is_empty() {
            out["enforcement"] = Value::Object(
                self.enforcement
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_value()))
                    .collect(),
            );
        }
        out
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct ScopePreview {
    pub scope: ScopeDecision,
    pub enforcement: BTreeMap<String, EnforcementEntry>,
}
/// Dispatch authority can only be constructed by this module. It is deliberately
/// neither Deserialize nor Default, and stores the proposal for fresh validation.
/// ```compile_fail
/// let _: ssx_application::policy::ValidatedScope = Default::default();
/// ```
/// ```compile_fail
/// let _: ssx_application::policy::ValidatedScope = ssx_json::from_str("{}").unwrap();
/// ```
#[derive(Debug)]
pub struct ValidatedScope {
    proposal: ScopeProposal,
    preview: ScopePreview,
    advice: Vec<(String, ssx_domain::FiniteF64)>,
}
impl ValidatedScope {
    pub fn preview(&self) -> &ScopePreview {
        &self.preview
    }
}
/// Read-only resolved command available only inside a fresh authorization callback.
/// Its origin and specialist parameters come from the privately retained proposal.
pub struct ValidatedCommand<'a> {
    request: &'a SearchRequest,
    preview: &'a ScopePreview,
    proposal: &'a ScopeProposal,
}
impl ValidatedCommand<'_> {
    pub fn request(&self) -> &SearchRequest {
        self.request
    }
    pub fn preview(&self) -> &ScopePreview {
        self.preview
    }
    pub fn origin(&self) -> SearchSurface {
        self.proposal.surface
    }
    pub fn specialist_types(&self) -> &[String] {
        &self.proposal.specialist_types
    }
    pub fn job_location(&self) -> Option<&str> {
        self.proposal.job_location.as_deref()
    }
    pub fn job_employment_type(&self) -> Option<&str> {
        self.proposal.job_employment_type.as_deref()
    }
}
#[derive(Debug, Clone)]
pub struct PolicyService {
    pub policy: SearchPolicy,
    pub catalog: CatalogSnapshot,
    pub resolver: ScopeResolver,
}
fn strings(v: &Value) -> Vec<String> {
    v.as_array().map_or_else(Vec::new, |a| {
        a.iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect()
    })
}
fn tables() -> Value {
    ssx_json::from_str(include_str!("../../../docs/rust-rewrite/f09/profiles.json"))
        .expect("checked static policy profile tables")
}
/// Default source topics are captured independently of dynamic operator inputs.
pub fn default_topics() -> Vec<Topic> {
    tables()["topics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| Topic {
            name: t["name"].as_str().unwrap_or_default().into(),
            keywords: strings(&t["keywords"]),
            engines: strings(&t["engines"]),
        })
        .collect()
}
impl PolicyService {
    pub fn engine_rejection(&self, names: &[String], field: &str) -> Result<(), PolicyError> {
        self.engine_rejection_with_activity(names, field, true)
    }
    /// Authorize disclosure of retained workflow evidence under current engine
    /// policy. Runtime activity/availability is not authorization to view history.
    /// This returns no dispatch token: acquisition still requires full authorize.
    pub fn retained_engine_rejection(
        &self,
        names: &[String],
        field: &str,
    ) -> Result<(), PolicyError> {
        self.engine_rejection_with_activity(names, field, false)
    }
    fn engine_rejection_with_activity(
        &self,
        names: &[String],
        field: &str,
        require_active: bool,
    ) -> Result<(), PolicyError> {
        let unknown: Vec<_> = names
            .iter()
            .filter(|n| self.catalog.get(n).is_none())
            .collect();
        let inactive: Vec<_> = names
            .iter()
            .filter(|n| self.catalog.get(n).is_some_and(|e| !e.enabled))
            .collect();
        if !unknown.is_empty() || !inactive.is_empty() {
            let problems = unknown
                .iter()
                .map(|n| format!("{n} (unknown)"))
                .chain(inactive.iter().map(|n| format!("{n} (inactive)")))
                .collect::<Vec<_>>();
            let mut e = PolicyError::new(
                "invalid_scope",
                field,
                format!("unknown or inactive engines: {}", problems.join(", ")),
            );
            let valid: BTreeSet<_> = self
                .catalog
                .engines()
                .iter()
                .filter(|e| e.enabled)
                .map(|e| &e.name)
                .collect();
            e.details = Box::new(json!({"valid_alternatives":valid}));
            return Err(e);
        }
        let sensitive: BTreeSet<_> = names
            .iter()
            .filter(|n| self.policy.sensitive_engines.contains(*n))
            .cloned()
            .collect();
        if !sensitive.is_empty() && !self.policy.sensitive_allowed {
            let mut e = PolicyError::new(
                "tool_disabled",
                field,
                format!(
                    "sensitive engines are unreachable without the sensitive-engine grant (MCP_TARGETED_SENSITIVE_ALLOWED=1): {}",
                    sensitive.iter().cloned().collect::<Vec<_>>().join(", ")
                ),
            );
            e.details =
                Box::new(json!({"engines":sensitive,"grant":"MCP_TARGETED_SENSITIVE_ALLOWED"}));
            return Err(e);
        }
        // Authorization precedes runtime availability: an enabled sensitive
        // engine remains sensitive even when no adapter instance is mounted.
        // Authorized requests still fail closed atomically if any requested
        // adapter is unavailable; never silently drop an explicit engine.
        let unavailable: Vec<_> = names
            .iter()
            .filter(|n| require_active && self.catalog.get(n).is_some_and(|e| !e.active))
            .collect();
        if !unavailable.is_empty() {
            let mut e = PolicyError::new(
                "invalid_scope",
                field,
                format!(
                    "unknown or inactive engines: {}",
                    unavailable
                        .iter()
                        .map(|n| format!("{n} (inactive)"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            );
            let valid: BTreeSet<_> = self
                .catalog
                .engines()
                .iter()
                .filter(|e| e.enabled && e.active)
                .map(|e| &e.name)
                .collect();
            e.details = Box::new(json!({"valid_alternatives":valid}));
            return Err(e);
        }
        Ok(())
    }
    /// Source default-strategy metadata only. This is not dispatch authority;
    /// every resulting query still passes authorize/from_research_query at admission
    /// and again at dispatch. No optional routing provider or store is consulted.
    pub fn research_template_scope(
        &self,
        intent: &str,
    ) -> Result<(Vec<String>, Vec<String>), PolicyError> {
        self.grant("research", "")?;
        let table = tables();
        let profile = &table["intents"][intent];
        if profile.is_null() || !strings(&profile["media_types"]).is_empty() {
            return Err(PolicyError::new(
                "invalid_intent",
                "intent",
                "invalid research template intent",
            ));
        }
        let explicit = strings(&profile["engines"]);
        let mut warnings = Vec::new();
        let mut names = if explicit.is_empty() {
            let categories = strings(&profile["categories"]);
            self.catalog
                .engines()
                .iter()
                .filter(|e| e.enabled && e.categories.iter().any(|c| categories.contains(c)))
                .map(|e| e.name.clone())
                .collect::<Vec<_>>()
        } else {
            let mut missing = explicit
                .iter()
                .filter(|n| self.catalog.get(n).is_none())
                .cloned()
                .collect::<Vec<_>>();
            missing.sort();
            if !missing.is_empty() {
                warnings.push(format!(
                    "intent '{intent}' references unknown engines: {}",
                    missing.join(", ")
                ));
            }
            explicit
                .into_iter()
                .filter(|n| self.catalog.get(n).is_some())
                .collect()
        };
        let mut excluded = names
            .iter()
            .filter(|n| {
                self.policy.sensitive_engines.contains(*n) && !self.policy.sensitive_allowed
            })
            .cloned()
            .collect::<Vec<_>>();
        if !excluded.is_empty() {
            excluded.sort();
            names.retain(|n| !excluded.contains(n));
            warnings.push(format!("sensitive engines excluded by policy (no MCP_TARGETED_SENSITIVE_ALLOWED grant): {}", excluded.join(", ")));
        }
        Ok((names, warnings))
    }
    /// Recovered workflow metadata is never dispatch authority. Dossier grants
    /// and every planned engine traverse the same fail-closed gate atomically.
    pub fn validate_research_workflow(
        &self,
        job: &ssx_domain::research::ResearchJob,
    ) -> Result<(), PolicyError> {
        if job.workflow.get("kind").and_then(Value::as_str) != Some("dependency_dossier") {
            return Ok(());
        }
        for grant in ["dependency_dossier", "research", "security"] {
            self.grant(grant, "")?;
        }
        for query in &job.queries {
            self.authorize(ScopeProposal::from_research_query(query))?;
        }
        Ok(())
    }
    fn grant(&self, grant: &str, field: &str) -> Result<(), PolicyError> {
        if self.policy.grants.contains(grant) {
            return Ok(());
        }
        let env = format!("MCP_GRANT_{}", grant.to_ascii_uppercase());
        let tool = if grant == "research" {
            "slopsearx_start_research".into()
        } else {
            format!("slopsearx_search_{grant}")
        };
        let _ = field;
        Err(PolicyError::new(
            "tool_disabled",
            "",
            format!("{tool} requires the {grant} grant ({env}=1)"),
        ))
    }
    fn research_policy(&self, p: &ScopeProposal) -> Result<(), PolicyError> {
        let reject = |message: &str| PolicyError::new("tool_disabled", "engines", message);
        if !self.policy.grants.contains("research") {
            if p.research.is_none() {
                return self.grant("research", "");
            }
            return Err(reject("research grant is disabled"));
        }
        let table = tables();
        let profile = &table["intents"][p.intent.as_str()];
        if profile.is_null() {
            return Err(PolicyError::new(
                "invalid_input",
                "intent",
                format!("unknown intent '{}'", p.intent),
            ));
        }
        if profile["media_types"]
            .as_array()
            .is_some_and(|types| !types.is_empty())
        {
            return Err(PolicyError::new(
                "invalid_intent",
                "intent",
                format!(
                    "intent '{}' selects a media type; research subqueries do not support media searches",
                    p.intent
                ),
            ));
        }
        if let Some(c) = &p.research {
            if c.planning_method.as_deref().is_some_and(|m| {
                !matches!(
                    m,
                    "original" | "decomposition" | "terminology_expansion" | "evidence_followup"
                )
            }) {
                return Err(reject("unknown planning_method"));
            }
            if c.evidence_result_ids.len() > 10 {
                return Err(reject(
                    "evidence_result_ids must be a list of at most 10 result IDs",
                ));
            }
            if c.evidence_result_ids
                .iter()
                .any(|s| s.is_empty() || s.chars().count() > 160)
            {
                return Err(reject(
                    "evidence_result_ids must contain nonempty bounded strings",
                ));
            }
            if c.evidence_result_ids.iter().collect::<BTreeSet<_>>().len()
                != c.evidence_result_ids.len()
            {
                return Err(reject("duplicate evidence_result_ids"));
            }
            if !c.evidence_result_ids.is_empty()
                && (c.parent.as_deref().is_none_or(str::is_empty)
                    || c.rationale.as_deref().is_none_or(str::is_empty))
            {
                return Err(reject(
                    "evidence references require a parent_attempt_id and rationale",
                ));
            }
            if c.planning_method.as_deref() == Some("evidence_followup")
                && c.evidence_result_ids.is_empty()
            {
                return Err(reject(
                    "evidence_followup requires admitted result references",
                ));
            }
        }
        if matches!(p.intent.as_str(), "jobs" | "security" | "science")
            && p.research.as_ref().is_none_or(|c| c.requires_intent_grant)
            && !self.policy.grants.contains(&p.intent)
        {
            return Err(reject(&format!(
                "{} requires the {} grant",
                p.intent, p.intent
            )));
        }
        if p.research.is_some() && p.request.engines.as_ref().is_none_or(Vec::is_empty) {
            return Err(reject("research query has no permitted engines"));
        }
        if let Some(c) = &p.research
            && !c.evidence_result_ids.is_empty()
        {
            if c.evidence_engines.is_empty() {
                return Err(reject(
                    "evidence references have no valid captured source scope",
                ));
            }
            if let Some(intent) = &c.evidence_intent {
                if tables()["intents"][intent.as_str()].is_null() {
                    return Err(reject(
                        "evidence references have an invalid captured intent",
                    ));
                }
                if matches!(intent.as_str(), "jobs" | "security" | "science")
                    && !self.policy.grants.contains(intent)
                {
                    return Err(reject("captured evidence intent grant has been revoked"));
                }
            }
            self.engine_rejection(&c.evidence_engines, "engines")?;
        }
        Ok(())
    }
    fn resolve_request(&self, proposal: &ScopeProposal) -> Result<SearchRequest, PolicyError> {
        if proposal.research.is_some() && proposal.surface != SearchSurface::Research {
            return Err(PolicyError::new(
                "invalid_scope",
                "surface",
                "captured research authority cannot be retagged",
            ));
        }
        if proposal.surface == SearchSurface::Research {
            self.research_policy(proposal)?;
        }
        if proposal.surface == SearchSurface::Worker {
            return Err(PolicyError::new(
                "invalid_scope",
                "surface",
                "worker must revalidate the original validated authority",
            ));
        }
        if proposal.surface == SearchSurface::Targeted
            && proposal.request.engines.as_ref().is_none_or(Vec::is_empty)
        {
            return Err(PolicyError::new(
                "invalid_input",
                "engines",
                "engines is required and must list at least one engine",
            ));
        }
        if proposal.surface == SearchSurface::Http {
            return Ok(proposal.request.clone());
        }
        let mut r = proposal.request.clone();
        let table = tables();
        if proposal.surface == SearchSurface::Generic
            && proposal.intent != "auto"
            && table["intents"][proposal.intent.as_str()].is_null()
        {
            return Err(PolicyError::new(
                "invalid_input",
                "intent",
                format!("unknown intent '{}'", proposal.intent),
            ));
        }
        if r.interactive_timeout_ms
            .is_some_and(|ms| !(1..=30_000).contains(&ms))
        {
            return Err(PolicyError::new(
                "invalid_input",
                "interactive_timeout_ms",
                "interactive_timeout_ms must be an integer between 1 and 30000",
            ));
        }
        if r.media_type
            .as_deref()
            .is_some_and(|m| !matches!(m, "image" | "video"))
        {
            return Err(PolicyError::new(
                "invalid_input",
                "media_type",
                format!("unknown media_type '{}'", r.media_type.as_deref().unwrap()),
            ));
        }
        if !matches!(
            r.freshness.as_str(),
            "prefer_cache" | "prefer_fresh" | "no_preference"
        ) {
            return Err(PolicyError::new(
                "invalid_input",
                "freshness",
                format!("unknown freshness '{}'", r.freshness),
            ));
        }

        let specialist = match proposal.surface {
            SearchSurface::Jobs => Some("jobs"),
            SearchSurface::Security => Some("security"),
            SearchSurface::Science => Some("science"),
            SearchSurface::Research => Some("research"),
            _ => None,
        };
        if let Some(g) = specialist {
            self.grant(g, "tool")?;
        }
        if matches!(
            proposal.surface,
            SearchSurface::Security | SearchSurface::Science
        ) {
            let (section, default, field) = if proposal.surface == SearchSurface::Security {
                ("security", "vulnerability", "evidence_types")
            } else {
                ("science", "papers", "source_types")
            };
            let types = if proposal.specialist_types.is_empty() {
                vec![default.to_owned()]
            } else {
                proposal.specialist_types.clone()
            };
            let mut names = Vec::new();
            for t in types {
                if table[section][t.as_str()].is_null() {
                    return Err(PolicyError::new(
                        "invalid_input",
                        field,
                        format!(
                            "unknown {} '{t}'",
                            if section == "security" {
                                "evidence_type"
                            } else {
                                "source_type"
                            }
                        ),
                    ));
                }
                for n in strings(&table[section][t.as_str()]) {
                    if !names.contains(&n) {
                        names.push(n);
                    }
                }
            }
            if r.engines.as_ref().is_none_or(Vec::is_empty) {
                r.engines = Some(
                    names
                        .into_iter()
                        .filter(|n| self.catalog.get(n).is_some())
                        .collect(),
                );
            }
        } else if proposal.surface == SearchSurface::Jobs
            && r.engines.as_ref().is_none_or(Vec::is_empty)
        {
            r.engines = Some(
                ["greenhouse", "ashby", "lever"]
                    .into_iter()
                    .filter(|n| self.catalog.get(n).is_some())
                    .map(str::to_owned)
                    .collect(),
            );
        }
        let profile = &table["intents"][proposal.intent.as_str()];
        if r.media_type.is_none() {
            r.media_type = strings(&profile["media_types"]).into_iter().next();
        }
        if r.engines.as_ref().is_some_and(|e| !e.is_empty()) {
            self.engine_rejection(r.engines.as_ref().unwrap(), "engines")?;
        } else if r.categories.as_ref().is_none_or(Vec::is_empty) && proposal.intent != "auto" {
            if profile.is_null() {
                return Err(PolicyError::new(
                    "invalid_input",
                    "intent",
                    format!("unknown intent '{}'", proposal.intent),
                ));
            }
            if matches!(proposal.intent.as_str(), "jobs" | "security" | "science")
                && !self.policy.grants.contains(&proposal.intent)
            {
                let env = format!("MCP_GRANT_{}", proposal.intent.to_ascii_uppercase());
                let mut e = PolicyError::new(
                    "tool_disabled",
                    "intent",
                    format!(
                        "intent '{}' requires the {} grant ({env}=1)",
                        proposal.intent, proposal.intent
                    ),
                );
                e.details = Box::new(json!({"grant":env}));
                return Err(e);
            }
            let names = strings(&profile["engines"]);
            if !names.is_empty() {
                r.engines = Some(
                    names
                        .into_iter()
                        .filter(|n| self.catalog.get(n).is_some())
                        .collect(),
                );
            } else {
                let cats = strings(&profile["categories"]);
                if !cats.is_empty() {
                    r.categories = Some(cats);
                }
            }
        }
        if let Some(names) = &r.engines {
            self.engine_rejection(names, "engines")?;
        }
        Ok(r)
    }
    /// Scope-only inspection performs no dispatch and yields no dispatch token.
    pub fn preview(&self, proposal: &ScopeProposal) -> Result<ScopePreview, PolicyError> {
        self.preview_with_advice(proposal, &[])
    }
    /// Optional advice remains untrusted and traverses the same policy/filter gate.
    pub fn preview_with_advice(
        &self,
        proposal: &ScopeProposal,
        advice: &[(String, ssx_domain::FiniteF64)],
    ) -> Result<ScopePreview, PolicyError> {
        let r = self.resolve_request(proposal)?;
        if r.interactive_timeout_ms
            .is_some_and(|ms| !(1..=30_000).contains(&ms))
        {
            return Err(PolicyError::new(
                "invalid_input",
                "interactive_timeout_ms",
                "interactive_timeout_ms must be an integer between 1 and 30000",
            ));
        }
        if r.query
            .trim_matches(|c: char| c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}'))
            .is_empty()
        {
            return Err(PolicyError::new(
                "invalid_input",
                "query",
                "query is required",
            ));
        }
        if proposal.surface != SearchSurface::Http
            && r.query.chars().count() > self.policy.max_query_length
        {
            return Err(PolicyError::new(
                "invalid_input",
                "query",
                format!(
                    "query exceeds the maximum length of {} characters",
                    self.policy.max_query_length
                ),
            ));
        }
        if r.safesearch > 2 {
            return Err(PolicyError::new(
                "invalid_input",
                "safesearch",
                "safesearch must be off, moderate, or strict",
            ));
        }
        PublicationBounds::new(r.date_from.as_deref(), r.date_to.as_deref()).map_err(|e| {
            let mut error = PolicyError::new("invalid_input", e.field, e.message.clone());
            for (field, value) in [("date_from", &r.date_from), ("date_to", &r.date_to)] {
                if let Some(value) = value {
                    error.enforcement.insert(
                        field.into(),
                        EnforcementEntry::rejected(json!(value), e.message.clone()),
                    );
                }
            }
            error
        })?;
        // Common to HTTP and every MCP/worker surface, including Http's early
        // resolve_request branch. Preserve malformed-date validation precedence.
        let bounded = r.date_from.is_some() || r.date_to.is_some();
        if bounded && r.time_range.is_some() {
            return Err(PolicyError::new(
                "invalid_input",
                "time_range",
                "absolute dates cannot be combined with time_range",
            ));
        }
        if bounded
            && (!proposal
                .request
                .engines
                .as_ref()
                .is_some_and(|names| names.len() == 1 && names[0] == "openalex")
                || r.media_type.is_some())
        {
            return Err(PolicyError::new(
                "invalid_input",
                "engines",
                "absolute dates require explicit engines=['openalex']",
            ));
        }
        let advice = if ScopeResolver::allowed(&proposal.request) {
            advice
        } else {
            &[]
        };
        let mut resolver = self.resolver.clone();
        if !advice.is_empty() {
            resolver
                .sensitive
                .extend(self.policy.sensitive_engines.iter().cloned());
        }
        let scope = resolver.with_specialist_advice(&r, &self.catalog, advice);
        if bounded {
            if scope.selected_engines != ["openalex"] {
                return Err(PolicyError::new(
                    "invalid_input",
                    "engines",
                    "audited date scope is unavailable",
                ));
            }
            for field in ["date_from", "date_to"] {
                if self
                    .catalog
                    .get("openalex")
                    .and_then(|engine| engine.enforced_filters.get(field))
                    .map(String::as_str)
                    != Some("upstream")
                {
                    return Err(PolicyError::new(
                        "invalid_input",
                        "engines",
                        "selected adapter does not enforce publication dates",
                    ));
                }
            }
        }
        if proposal.surface != SearchSurface::Http {
            self.engine_rejection(&scope.selected_engines, "engines")?;
        }
        let declarations = self.catalog.declarations();
        if r.safesearch == 2 {
            let bad: BTreeSet<_> = scope
                .selected_engines
                .iter()
                .filter(|n| {
                    declarations
                        .get(*n)
                        .and_then(|d| d.get("safesearch"))
                        .map(String::as_str)
                        != Some("upstream")
                })
                .cloned()
                .collect();
            if scope.selected_engines.is_empty() || !bad.is_empty() {
                let reason = format!(
                    "strict SafeSearch is not enforced by {}; strict results cannot be guaranteed — use 'off' or 'moderate'",
                    if bad.is_empty() {
                        "any selected engine".into()
                    } else {
                        bad.into_iter().collect::<Vec<_>>().join(", ")
                    }
                );
                let mut e = PolicyError::new("safesearch_unenforced", "safesearch", reason.clone());
                e.details = Box::new(json!({"selected_engines":scope.selected_engines}));
                e.enforcement.insert(
                    "safesearch".into(),
                    EnforcementEntry::rejected(json!("strict"), reason),
                );
                return Err(e);
            }
        }
        if let Some(range) = &r.time_range
            && time_range_days(range).is_none()
            && scope.selected_engines.iter().any(|n| {
                declarations
                    .get(n)
                    .and_then(|d| d.get("time_range"))
                    .is_some_and(|s| matches!(s.as_str(), "upstream" | "local"))
            })
        {
            return Err(PolicyError::new(
                "invalid_input",
                "time_range",
                "time_range must be day, week, month, or year",
            ));
        }
        let mut enforcement: BTreeMap<String, EnforcementEntry> = BTreeMap::new();
        let mut requested = vec![("pagination", json!(r.page))];
        if !r.language.is_empty() {
            requested.push(("language", json!(r.language)));
        }
        if let Some(range) = &r.time_range
            && !range.is_empty()
        {
            requested.push(("time_range", json!(range)));
        }
        if r.safesearch > 0 {
            requested.push((
                "safesearch",
                json!(if r.safesearch == 2 {
                    "strict"
                } else {
                    "moderate"
                }),
            ));
        }
        for (field, value) in [("date_from", &r.date_from), ("date_to", &r.date_to)] {
            if let Some(v) = value {
                requested.push((field, json!(v)));
            }
        }
        for (field, value) in requested {
            enforcement.insert(
                field.into(),
                resolve_filter_enforcement(&scope.selected_engines, field, value, &declarations),
            );
        }
        if matches!(
            proposal.surface,
            SearchSurface::Science | SearchSurface::Security
        ) {
            enforcement.retain(|field, _| {
                matches!(field.as_str(), "date_from" | "date_to" | "pagination")
            });
        }
        if proposal.surface == SearchSurface::Jobs {
            enforcement.retain(|field, _| field == "pagination");
            for (field, value) in [
                ("location", &proposal.job_location),
                ("employment_type", &proposal.job_employment_type),
            ] {
                if let Some(v) = value
                    && !v.is_empty()
                {
                    enforcement.insert(
                        field.into(),
                        EnforcementEntry {
                            requested: json!(v),
                            status: crate::filters::EnforcementStatus::Unsupported,
                            reason: format!("{field} is not consumed by current adapters"),
                            enforced_by: BTreeSet::new(),
                        },
                    );
                }
            }
        }
        Ok(ScopePreview { scope, enforcement })
    }
    pub fn authorize(&self, proposal: ScopeProposal) -> Result<ValidatedScope, PolicyError> {
        self.authorize_with_advice(proposal, Vec::new())
    }
    pub fn authorize_with_advice(
        &self,
        proposal: ScopeProposal,
        advice: Vec<(String, ssx_domain::FiniteF64)>,
    ) -> Result<ValidatedScope, PolicyError> {
        if proposal.surface == SearchSurface::Preview {
            return Err(PolicyError::new(
                "invalid_scope",
                "surface",
                "scope preview cannot authorize dispatch",
            ));
        }
        let preview = self.preview_with_advice(&proposal, &advice)?;
        if preview.scope.selected_engines.is_empty() {
            return Err(PolicyError::new(
                "invalid_scope",
                "engines",
                "no active engine is selected",
            ));
        }
        let unavailable: Vec<_> = preview
            .scope
            .selected_engines
            .iter()
            .filter(|n| {
                self.catalog
                    .get(n)
                    .is_none_or(|e| !e.active || !e.available)
            })
            .cloned()
            .collect();
        if !unavailable.is_empty() {
            let mut e = PolicyError::new(
                "invalid_scope",
                "engines",
                "selected native engine implementation is unavailable",
            );
            e.details = Box::new(json!({"engines":unavailable}));
            return Err(e);
        }
        Ok(ValidatedScope {
            proposal,
            preview,
            advice,
        })
    }
    /// Recheck current policy and expose explanation only. An empty/unavailable
    /// preview is never dispatch authority; consumers still need with_revalidated.
    pub fn revalidate_preview(&self, old: &ValidatedScope) -> Result<ScopePreview, PolicyError> {
        self.preview_with_advice(&old.proposal, &old.advice)
    }
    /// Revalidate a delayed proposal using current grants, catalog, health and
    /// routing. Any changed scope requires fresh caller admission/budgeting.
    /// The callback is the caller's dispatch/commit linearization boundary;
    /// concurrent revocation after it is cooperative cancellation, not prevented.
    pub fn with_revalidated<T>(
        &self,
        old: &ValidatedScope,
        commit: impl FnOnce(&ValidatedCommand<'_>) -> T,
    ) -> Result<T, PolicyError> {
        let current = self.authorize_with_advice(old.proposal.clone(), old.advice.clone())?;
        if current.preview != old.preview {
            return Err(PolicyError::new(
                "invalid_scope",
                "engines",
                "scope changed; fresh admission is required",
            ));
        }
        let request = self.resolve_request(&current.proposal)?;
        Ok(commit(&ValidatedCommand {
            request: &request,
            preview: &current.preview,
            proposal: &current.proposal,
        }))
    }
}
