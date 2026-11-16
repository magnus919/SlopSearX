//! Pure saved-definition validation. Prepared values are not dispatch authority.
use crate::policy::{PolicyService, ScopeProposal};
use ssx_domain::{
    DomainError, FiniteF64, SearchRequest, canonical::plan_digest, saved::SavedDefinition,
};
use ssx_json::Value;
#[cfg(test)]
use ssx_json::json;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedLimits {
    pub definitions: u64,
    pub engines: usize,
    pub rows: u64,
    pub reports: u64,
    pub default_reports: u64,
    pub retention_seconds: u64,
    pub default_retention_seconds: u64,
    pub minimum_interval: u64,
    pub maximum_interval: u64,
}
impl Default for SavedLimits {
    fn default() -> Self {
        Self {
            definitions: 20,
            engines: 5,
            rows: 100,
            reports: 100,
            default_reports: 20,
            retention_seconds: 2_592_000,
            default_retention_seconds: 604_800,
            minimum_interval: 60,
            maximum_interval: 86_400,
        }
    }
}
/// Operator/composition input. No credential values are carried or logged.
pub struct SavedPolicy<'a> {
    pub search: &'a PolicyService,
    pub saved_granted: bool,
    pub auth_configured: &'a BTreeMap<String, bool>,
    pub limits: &'a SavedLimits,
}
impl SavedPolicy<'_> {
    pub fn fingerprint(&self, engines: &[String]) -> Result<String, DomainError> {
        let mut names = engines.to_vec();
        names.sort();
        let mut capabilities = Vec::new();
        for name in names {
            let cap = self.search.catalog.get(&name);
            let configured = self
                .auth_configured
                .get(&name)
                .ok_or(DomainError("incomplete saved auth configuration"))?;
            let mut categories = cap.map(|c| c.categories.clone()).unwrap_or_default();
            categories.sort();
            capabilities.push(object([
                ("name", Value::String(name)),
                ("active", Value::Bool(cap.is_some_and(|c| c.active))),
                ("enabled", Value::Bool(cap.is_some_and(|c| c.enabled))),
                ("auth_configured", Value::Bool(cap.is_some() && *configured)),
                (
                    "categories",
                    Value::Array(categories.into_iter().map(Value::String).collect()),
                ),
            ]));
        }
        plan_digest(&object([
            ("saved_searches", Value::Bool(self.saved_granted)),
            (
                "targeted_sensitive_allowed",
                Value::Bool(self.search.policy.sensitive_allowed),
            ),
            (
                "sensitive_engines",
                Value::Array(
                    self.search
                        .policy
                        .sensitive_engines
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect(),
                ),
            ),
            ("capabilities", Value::Array(capabilities)),
        ]))
    }
    pub fn validate_scope(&self, query: &str, engines: &[String]) -> Result<(), DomainError> {
        if !self.saved_granted {
            return Err(DomainError("saved-search grant is disabled"));
        }
        let proposal = ScopeProposal::generic(SearchRequest {
            query: query.into(),
            engines: Some(engines.to_vec()),
            ..Default::default()
        });
        self.search
            .preview(&proposal)
            .map_err(|_| DomainError("saved-search scope rejected"))?;
        self.fingerprint(engines)?;
        Ok(())
    }
    pub fn validate_definition(&self, definition: &SavedDefinition) -> Result<(), DomainError> {
        let l = self.limits;
        if [
            definition.interval_seconds,
            definition.retention_seconds,
            definition.max_results,
            definition.max_reports,
            definition.revision,
        ]
        .iter()
        .any(|n| *n > i64::MAX as u64)
            || l.definitions == 0
            || l.engines == 0
            || l.minimum_interval == 0
            || l.minimum_interval > l.maximum_interval
            || definition.interval_seconds < l.minimum_interval
            || definition.interval_seconds > l.maximum_interval
            || !(1..=l.retention_seconds).contains(&definition.retention_seconds)
            || !(1..=l.rows).contains(&definition.max_results)
            || !(1..=l.reports).contains(&definition.max_reports)
            || definition.engines.is_empty()
            || definition.engines.len() > l.engines
            || definition.engines.iter().collect::<BTreeSet<_>>().len() != definition.engines.len()
            || definition.query.chars().count() > self.search.policy.max_query_length
            || strip(&definition.query).is_empty()
            || definition.revision == 0
            || definition.created_at.get() > definition.expires_at.get()
            || definition.expires_at.get() - definition.created_at.get()
                > l.retention_seconds as f64
        {
            return Err(DomainError("invalid saved definition limits"));
        }
        self.validate_scope(&definition.query, &definition.engines)
    }
}
fn object<const N: usize>(fields: [(&'static str, Value); N]) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}
fn strip(text: &str) -> &str {
    text.trim_matches(ssx_json::python_whitespace)
}
fn strict_u64(value: &Value, min: u64, max: u64) -> Result<u64, DomainError> {
    let n = value
        .as_u64()
        .ok_or(DomainError("saved value must be an integer"))?;
    if !(min..=max).contains(&n) {
        return Err(DomainError("saved value outside limits"));
    }
    Ok(n)
}
fn selected(value: &Value, max: usize) -> Result<Vec<String>, DomainError> {
    let values = value
        .as_array()
        .ok_or(DomainError("saved engines must be a list"))?;
    if values.is_empty() || values.len() > max {
        return Err(DomainError("saved engine count outside limits"));
    }
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for value in values {
        let name = value.as_str().ok_or(DomainError("invalid saved engine"))?;
        if seen.insert(name) {
            out.push(name.into());
        }
    }
    Ok(out)
}
#[derive(Clone)]
pub struct PreparedDefinition {
    definition: SavedDefinition,
    quota: u64,
    maximum_lifetime: u64,
}
impl PreparedDefinition {
    pub fn definition(&self) -> &SavedDefinition {
        &self.definition
    }
    pub fn quota(&self) -> u64 {
        self.quota
    }
    pub fn maximum_lifetime(&self) -> u64 {
        self.maximum_lifetime
    }
}
/// Raw JSON retains bool-vs-integer distinctions from the tool contract.
pub fn prepare_create(
    policy: &SavedPolicy<'_>,
    tenant: &str,
    id: &str,
    input: &Value,
    now: f64,
) -> Result<PreparedDefinition, DomainError> {
    if !policy.saved_granted {
        return Err(DomainError("saved-search grant is disabled"));
    }
    ssx_domain::saved::validate_saved_id(id)?;
    let fields = input
        .as_object()
        .ok_or(DomainError("invalid saved input"))?;
    let query = fields
        .get("query")
        .and_then(Value::as_str)
        .ok_or(DomainError("saved query required"))?;
    if query.chars().count() > policy.search.policy.max_query_length || strip(query).is_empty() {
        return Err(DomainError("invalid saved query"));
    }
    let engines = selected(
        fields
            .get("engines")
            .ok_or(DomainError("saved engines required"))?,
        policy.limits.engines,
    )?;
    policy.validate_scope(strip(query), &engines)?;
    let l = policy.limits;
    let get = |name, default, min, max| {
        fields
            .get(name)
            .filter(|v| !v.is_null())
            .map(|v| strict_u64(v, min, max))
            .unwrap_or(Ok(default))
    };
    let interval = get(
        "interval_seconds",
        0,
        l.minimum_interval,
        l.maximum_interval,
    )?;
    let retention = get(
        "retention_seconds",
        l.default_retention_seconds,
        1,
        l.retention_seconds,
    )?;
    let lifetime = get("expires_in_seconds", retention, 1, l.retention_seconds)?;
    let immediate = fields
        .get("start_immediately")
        .map(|v| {
            v.as_bool()
                .ok_or(DomainError("saved start flag must be boolean"))
        })
        .transpose()?
        .unwrap_or(false);
    let definition = SavedDefinition {
        search_id: id.into(),
        tenant: tenant.into(),
        query: strip(query).into(),
        engines,
        interval_seconds: interval,
        retention_seconds: retention.min(lifetime),
        max_results: get("max_results", l.rows, 1, l.rows)?,
        max_reports: get("max_reports", l.default_reports, 1, l.reports)?,
        created_at: FiniteF64::new(now)?,
        expires_at: FiniteF64::new(now + lifetime as f64)?,
        next_due: FiniteF64::new(if immediate {
            now
        } else {
            now + interval as f64
        })?,
        revision: 1,
        paused: false,
        policy_fingerprint: String::new(),
        last_slot: -1,
        latest_run_id: None,
        baseline: None,
        lease_token: None,
        lease_expires_at: FiniteF64::default(),
        deleted: false,
    };
    let mut definition = definition;
    policy.validate_definition(&definition)?;
    definition.policy_fingerprint = policy.fingerprint(&definition.engines)?;
    Ok(PreparedDefinition {
        definition,
        quota: l.definitions,
        maximum_lifetime: l.retention_seconds,
    })
}

/// Apply only source-supported editable fields; identity/expiry/ownership are immutable.
pub fn prepare_update(
    policy: &SavedPolicy<'_>,
    current: &SavedDefinition,
    input: &Value,
    now: f64,
) -> Result<SavedDefinition, DomainError> {
    if !policy.saved_granted {
        return Err(DomainError("saved-search grant is disabled"));
    }
    let fields = input
        .as_object()
        .ok_or(DomainError("invalid saved update"))?;
    if fields.keys().any(|name| {
        ![
            "query",
            "engines",
            "interval_seconds",
            "retention_seconds",
            "max_results",
            "max_reports",
        ]
        .contains(&name.as_str())
    }) {
        return Err(DomainError("unsupported saved update field"));
    }
    let mut next = current.clone();
    let old_window = current.window()?;
    let present = |name| fields.get(name).filter(|value| !value.is_null());
    if let Some(query) = present("query") {
        let query = query.as_str().ok_or(DomainError("invalid saved query"))?;
        if query.chars().count() > policy.search.policy.max_query_length || strip(query).is_empty()
        {
            return Err(DomainError("invalid saved query"));
        }
        next.query = strip(query).into();
    }
    if let Some(engines) = present("engines") {
        next.engines = selected(engines, policy.limits.engines)?;
    }
    // Source validates only supplied query text. Engine authority is always fresh,
    // including when an unchanged historical query exceeds a lowered limit.
    if present("query").is_some() {
        policy.validate_scope(&next.query, &next.engines)?;
    } else {
        policy
            .search
            .engine_rejection(&next.engines, "engines")
            .map_err(|_| DomainError("saved-search scope rejected"))?;
        policy.fingerprint(&next.engines)?;
    }
    if let Some(v) = present("interval_seconds") {
        next.interval_seconds = strict_u64(
            v,
            policy.limits.minimum_interval,
            policy.limits.maximum_interval,
        )?;
    }
    if let Some(v) = present("retention_seconds") {
        next.retention_seconds = strict_u64(v, 1, policy.limits.retention_seconds)?
            .min((current.expires_at.get() - now).trunc().max(1.0) as u64);
    }
    if let Some(v) = present("max_results") {
        next.max_results = strict_u64(v, 1, policy.limits.rows)?;
    }
    if let Some(v) = present("max_reports") {
        next.max_reports = strict_u64(v, 1, policy.limits.reports)?;
    }
    next.policy_fingerprint = policy.fingerprint(&next.engines)?;
    if next.window()? != old_window || present("interval_seconds").is_some() {
        reset_schedule(&mut next, now)?;
    }
    finish_revision(next)
}
pub fn prepare_pause(
    policy: &SavedPolicy<'_>,
    current: &SavedDefinition,
    paused: bool,
    now: f64,
) -> Result<SavedDefinition, DomainError> {
    if !policy.saved_granted {
        return Err(DomainError("saved-search grant is disabled"));
    }
    let mut next = current.clone();
    next.paused = paused;
    reset_schedule(&mut next, now)?;
    next.policy_fingerprint = policy.fingerprint(&next.engines)?;
    finish_revision(next)
}
fn reset_schedule(definition: &mut SavedDefinition, now: f64) -> Result<(), DomainError> {
    definition.baseline = None;
    definition.latest_run_id = None;
    definition.last_slot = -1;
    definition.created_at = FiniteF64::new(now)?;
    definition.next_due = FiniteF64::new(now + definition.interval_seconds as f64)?;
    Ok(())
}
fn finish_revision(mut definition: SavedDefinition) -> Result<SavedDefinition, DomainError> {
    definition.revision = definition
        .revision
        .checked_add(1)
        .filter(|revision| *revision <= i64::MAX as u64)
        .ok_or(DomainError("saved revision overflow"))?;
    definition.lease_token = None;
    definition.lease_expires_at = FiniteF64::default();
    // Source CAS clears the persisted baseline even when the summary preserves it.
    definition.baseline = None;
    Ok(definition)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        policy::SearchPolicy,
        scope::{CatalogSnapshot, EngineCapability, ScopeResolver},
    };
    fn service() -> PolicyService {
        PolicyService {
            policy: SearchPolicy::default(),
            resolver: ScopeResolver::default(),
            catalog: CatalogSnapshot::new(
                [
                    ("brave", vec!["news", "general"]),
                    ("arxiv", vec!["science"]),
                    ("hibp", vec!["security"]),
                ]
                .into_iter()
                .map(|(name, categories)| EngineCapability {
                    name: name.into(),
                    enabled: true,
                    active: true,
                    available: false,
                    categories: categories.into_iter().map(str::to_owned).collect(),
                    media_types: vec![],
                    auth_ready: true,
                    circuit_open: false,
                    cost_class: "free".into(),
                    health_status: "ok".into(),
                    health_stale: false,
                    enforced_filters: BTreeMap::new(),
                })
                .collect(),
            )
            .unwrap(),
        }
    }
    #[test]
    fn configured_auth_fingerprint_is_source_exact_and_health_independent() {
        let mut service = service();
        let limits = SavedLimits::default();
        let mut auth = BTreeMap::from([
            ("brave".into(), false),
            ("arxiv".into(), false),
            ("hibp".into(), true),
        ]);
        let corpus: Value =
            ssx_json::from_str(include_str!("../../../docs/rust-rewrite/w00/source.json")).unwrap();
        for row in corpus["policy_observations"].as_array().unwrap() {
            match row["name"].as_str().unwrap() {
                "configured_key" => {
                    auth.insert("brave".into(), true);
                }
                "inactive" => {
                    let mut caps = service.catalog.engines().to_vec();
                    caps.iter_mut().find(|c| c.name == "arxiv").unwrap().active = false;
                    service.catalog = CatalogSnapshot::new(caps).unwrap();
                }
                _ => {}
            }
            let p = SavedPolicy {
                search: &service,
                saved_granted: true,
                auth_configured: &auth,
                limits: &limits,
            };
            assert_eq!(
                p.fingerprint(&["brave".into(), "arxiv".into()]).unwrap(),
                row["fingerprint"].as_str().unwrap()
            );
        }
    }
    #[test]
    fn strict_inputs_and_sensitive_scope_reject_before_preparation() {
        let service = service();
        let limits = SavedLimits::default();
        let auth = BTreeMap::from([
            ("brave".into(), false),
            ("arxiv".into(), false),
            ("hibp".into(), true),
        ]);
        let p = SavedPolicy {
            search: &service,
            saved_granted: true,
            auth_configured: &auth,
            limits: &limits,
        };
        let corpus: Value =
            ssx_json::from_str(include_str!("../../../docs/rust-rewrite/w00/source.json")).unwrap();
        for row in corpus["strict_interval_observations"].as_array().unwrap() {
            let input = json!({"query":"q","engines":["brave"],"interval_seconds":row["input"]});
            assert_eq!(
                prepare_create(&p, "tenant", "saved-a", &input, 1000.0).is_ok(),
                row["outcome"].as_object().unwrap().contains_key("ok")
            );
        }
        let mut input = json!({"query":"\u{1c} q \u{1f}","engines":["brave","brave"],"interval_seconds":60,"expires_in_seconds":100});
        let prepared = prepare_create(&p, "tenant", "saved-a", &input, 1000.0).unwrap();
        assert_eq!(prepared.definition().query, "q");
        assert_eq!(prepared.definition().engines, ["brave"]);
        assert_eq!(prepared.definition().retention_seconds, 100);
        input["engines"] = json!(["brave", "hibp"]);
        assert!(prepare_create(&p, "tenant", "saved-a", &input, 1000.0).is_err());
        let mut allowed = service.clone();
        allowed.policy.sensitive_allowed = true;
        let permitted = SavedPolicy {
            search: &allowed,
            ..p
        };
        assert!(prepare_create(&permitted, "tenant", "saved-a", &input, 1000.0).is_ok());
        let empty_auth = BTreeMap::new();
        let incomplete = SavedPolicy {
            auth_configured: &empty_auth,
            ..p
        };
        assert!(
            prepare_create(
                &incomplete,
                "tenant",
                "saved-a",
                &json!({"query":"q","engines":["brave"],"interval_seconds":60}),
                1000.0
            )
            .is_err()
        );
        input["engines"] = json!(["brave", "brave", "brave", "brave", "brave", "brave"]);
        assert!(prepare_create(&p, "tenant", "saved-a", &input, 1000.0).is_err());
        let denied = SavedPolicy {
            saved_granted: false,
            ..p
        };
        assert!(
            prepare_create(
                &denied,
                "tenant",
                "saved-a",
                &json!({"query":"q","engines":["brave"],"interval_seconds":60}),
                1000.0
            )
            .is_err()
        );
    }
}
#[cfg(test)]
mod fingerprint_boundary_tests {
    use super::*;
    use crate::{
        policy::SearchPolicy,
        scope::{CatalogSnapshot, EngineCapability, ScopeResolver},
    };
    #[test]
    fn large_policy_metadata_identity_does_not_use_persistence_macro_bounds() {
        let catalog = CatalogSnapshot::new(vec![EngineCapability {
            name: "brave".into(),
            enabled: true,
            active: true,
            available: false,
            categories: vec!["x".repeat(ssx_json::MAX_JSON_BYTES + 1)],
            media_types: vec![],
            auth_ready: true,
            circuit_open: false,
            cost_class: "free".into(),
            health_status: "ok".into(),
            health_stale: false,
            enforced_filters: BTreeMap::new(),
        }])
        .unwrap();
        let service = PolicyService {
            policy: SearchPolicy::default(),
            catalog,
            resolver: ScopeResolver::default(),
        };
        let limits = SavedLimits::default();
        let auth = BTreeMap::from([("brave".into(), false)]);
        let p = SavedPolicy {
            search: &service,
            saved_granted: true,
            auth_configured: &auth,
            limits: &limits,
        };
        assert!(std::panic::catch_unwind(|| p.fingerprint(&["brave".into()])).is_ok());
    }
}

/// Source facts only: ranking and transient metadata never enter a baseline.
pub fn observe(
    definition: &SavedDefinition,
    response: &ssx_domain::SearchResponse,
    observed_at: f64,
) -> Result<Value, DomainError> {
    use ssx_domain::{canonical::saved_identity, saved::IDENTITY_VERSION};
    let mut reasons = BTreeSet::new();
    let coverage = response
        .engine_outcomes
        .iter()
        .map(|o| (o.engine.clone(), Value::String(o.status.clone())))
        .collect::<ssx_json::Map<_, _>>();
    if response.cached {
        reasons.insert("cached_observation");
    }
    if response.partial || response.deadline_exceeded || response.all_unresponsive {
        reasons.insert("incomplete_response");
    }
    if response
        .scope
        .selected_engines
        .iter()
        .collect::<BTreeSet<_>>()
        != definition.engines.iter().collect::<BTreeSet<_>>()
    {
        reasons.insert("scope_changed");
    }
    if definition
        .engines
        .iter()
        .any(|e| coverage.get(e).and_then(Value::as_str) != Some("ok"))
    {
        reasons.insert("incomplete_engine_coverage");
    }
    if response.results.len() as u64 > definition.max_results {
        reasons.insert("observation_truncated");
    }
    let mut records = ssx_json::Map::new();
    for result in response
        .results
        .iter()
        .take(usize::try_from(definition.max_results).unwrap_or(usize::MAX))
    {
        if result.url.is_empty()
            || [&result.url, &result.title, &result.content]
                .into_iter()
                .any(|s| s.len() > 8192)
            || result
                .published_date
                .as_ref()
                .is_some_and(|s| s.len() > 8192)
        {
            reasons.insert("unrepresentable_source_record");
            continue;
        }
        let identity = saved_identity(&[
            Value::String(IDENTITY_VERSION.into()),
            Value::String(crate::normalise_url(&result.url)),
        ])?;
        let engines = if result.engines.is_empty() {
            vec![result.engine.clone()]
        } else {
            result.engines.iter().cloned().collect()
        };
        let record = ssx_json::json!({"url":result.url,"engine":result.engine,"engines":engines,"fields":{"title":result.title,"content":result.content,"published_date":result.published_date}});
        if records.get(&identity).is_some_and(|old| *old != record) {
            reasons.insert("ambiguous_url_identity");
        }
        records.insert(identity, record);
    }
    Ok(
        ssx_json::json!({"window":definition.window()?,"observed_at":observed_at,"query_id":response.query_id,"cached":response.cached,"coverage":coverage,"discovered_count":response.results.len(),"records":records,"incomparable_reasons":reasons.into_iter().collect::<Vec<_>>()}),
    )
}

/// Compare a bounded search window; absence is never asserted to be deletion.
pub fn compare_observations(
    definition: &SavedDefinition,
    observation: Value,
    run_id: &str,
    scheduled_at: f64,
    started_at: f64,
    finished_at: f64,
    missed_slots: i64,
) -> Result<(Value, Option<Value>), DomainError> {
    use ssx_domain::canonical::{JsonProfile, canonical_json, saved_identity};
    let invalid = || DomainError("malformed saved observation");
    if [scheduled_at, started_at, finished_at]
        .iter()
        .any(|n| !n.is_finite())
    {
        return Err(invalid());
    }
    let mut reasons = observation["incomparable_reasons"]
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .map(|v| v.as_str().map(str::to_owned).ok_or_else(invalid))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut baseline = definition.baseline.as_ref();
    let mut baseline_reason = "first_observation";
    if let Some(b) = baseline
        && b["expires_at"].as_f64().ok_or_else(invalid)? <= finished_at
    {
        baseline = None;
        baseline_reason = "baseline_expired";
    }
    if let Some(b) = baseline
        && b["window"] != observation["window"]
    {
        baseline = None;
        baseline_reason = "comparison_window_changed";
    }
    let current = observation["records"].as_object().ok_or_else(invalid)?;
    let empty = ssx_json::Map::new();
    let prior = match baseline {
        Some(b) => b["records"].as_object().ok_or_else(invalid)?,
        None => &empty,
    };
    if baseline.is_some()
        && current.iter().any(|(id, row)| {
            prior.get(id).is_some_and(|old| {
                row["engine"] != old["engine"] || row["engines"] != old["engines"]
            })
        })
    {
        reasons.insert("source_provenance_changed".into());
    }
    let mut events = Vec::new();
    if reasons.is_empty() && baseline.is_some() {
        for id in current.keys().chain(prior.keys()).collect::<BTreeSet<_>>() {
            let before = prior.get(id);
            let after = current.get(id);
            let kind = match (before, after) {
                (None, _) => "added",
                (_, None) => "not_observed_in_latest_run",
                (Some(b), Some(a)) if b["fields"] != a["fields"] => "source_field_changed",
                _ => continue,
            };
            events.push(ssx_json::json!({"event_id":saved_identity(&[Value::String(run_id.into()),Value::String(id.clone()),Value::String(kind.into())])?,"kind":kind,"identity":id,"before":before,"after":after}));
        }
    }
    let expires_at = definition
        .expires_at
        .get()
        .min(finished_at + definition.retention_seconds as f64);
    let mut report = object([
        ("run_id", Value::String(run_id.into())),
        ("search_id", Value::String(definition.search_id.clone())),
        ("revision", ssx_json::to_value(definition.revision)?),
        ("scheduled_at", ssx_json::to_value(scheduled_at)?),
        ("started_at", ssx_json::to_value(started_at)?),
        ("finished_at", ssx_json::to_value(finished_at)?),
        ("missed_slots", ssx_json::to_value(missed_slots)?),
        (
            "status",
            Value::String(
                if !reasons.is_empty() {
                    "incomparable"
                } else if baseline.is_some() {
                    "compared"
                } else {
                    "baseline_initialized"
                }
                .into(),
            ),
        ),
        ("incomparable_reasons", ssx_json::to_value(&reasons)?),
        (
            "baseline_reason",
            if baseline.is_none() {
                Value::String(baseline_reason.into())
            } else {
                Value::Null
            },
        ),
        (
            "baseline_observed_at",
            baseline.map_or(Value::Null, |b| b["observed_at"].clone()),
        ),
        ("observation", observation),
        ("events", Value::Array(events)),
        ("expires_at", ssx_json::to_value(expires_at)?),
    ]);
    let oversized = ssx_json::to_vec(&report)?.len() > ssx_json::MAX_JSON_BYTES
        || canonical_json(&report, JsonProfile::AsciiSpaced)?.len() > 4 * 1024 * 1024;
    if oversized {
        reasons.insert("report_size_exceeded".into());
        report["status"] = Value::String("incomparable".into());
        report["incomparable_reasons"] = ssx_json::to_value(reasons)?;
        report["events"] = Value::Array(vec![]);
        report["observation"]["records"] = Value::Object(ssx_json::Map::new());
        return Ok((report, None));
    }
    let replacement = if reasons.is_empty() {
        let mut value = report["observation"].clone();
        value["expires_at"] = ssx_json::to_value(expires_at)?;
        Some(value)
    } else {
        None
    };
    Ok((report, replacement))
}
