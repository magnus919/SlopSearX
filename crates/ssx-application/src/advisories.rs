//! Read-boundary facts and explicitly attributed operator reports. Never evidence.
use ssx_domain::{ScopeDecision, SearchRequest};
use ssx_json::{Value, json};
use std::collections::BTreeSet;
#[derive(Debug, Clone)]
pub struct Engine {
    pub name: String,
    pub display_name: String,
    pub enabled: bool,
    pub credentials_required: bool,
    pub credentials_configured: bool,
    pub sensitive: bool,
    pub categories: Vec<String>,
    pub media_types: Vec<String>,
}
/// Secret-free facts projected by trusted native composition, not caller grants.
pub struct Context {
    pub engines: Vec<Engine>,
    pub hidden: BTreeSet<String>,
    pub automatic_names: BTreeSet<String>,
    pub routing_configured: bool,
    pub reranking_configured: bool,
}
pub fn search(
    request: &SearchRequest,
    scope: &ScopeDecision,
    context: &Context,
    result_count: usize,
) -> Vec<Value> {
    let mut relevant: BTreeSet<String> =
        if let Some(names) = request.engines.as_ref().filter(|n| !n.is_empty()) {
            names.iter().cloned().collect()
        } else if let Some(categories) = request.categories.as_ref().filter(|n| !n.is_empty()) {
            context
                .engines
                .iter()
                .filter(|e| e.categories.iter().any(|c| categories.contains(c)))
                .map(|e| e.name.clone())
                .collect()
        } else if let Some(media) = &request.media_type {
            context
                .engines
                .iter()
                .filter(|e| e.media_types.contains(media))
                .map(|e| e.name.clone())
                .collect()
        } else {
            context.automatic_names.clone()
        };
    if let Some(media) = &request.media_type {
        relevant.retain(|n| {
            context
                .engines
                .iter()
                .any(|e| &e.name == n && e.media_types.contains(media))
        });
    }
    let mut sources = Vec::new();
    for name in relevant {
        let Some(engine) = context.engines.iter().find(|e| e.name == name) else {
            continue;
        };
        if engine.sensitive || context.hidden.contains(&name) {
            continue;
        }
        let (reason, action, message) = if !engine.enabled {
            (
                "engine_disabled",
                "review_engine_configuration",
                format!(
                    "{} is not enabled for this platform. An operator can review its configuration.",
                    engine.display_name
                ),
            )
        } else if engine.credentials_required && !engine.credentials_configured {
            (
                "credentials_missing",
                "configure_credentials",
                format!(
                    "Required credentials for {} are not configured. Configuration belongs to an operator.",
                    engine.display_name
                ),
            )
        } else {
            continue;
        };
        sources.push(json!({"code":"relevant_source_unavailable","engine":name,"reason":reason,"message":message,"action":{"actor":"operator","kind":action},"expected_quality_gain":"unmeasured"}));
        if sources.len() == 2 {
            break;
        }
    }
    let names: BTreeSet<_> = scope
        .selected_engines
        .iter()
        .chain(request.engines.iter().flatten())
        .collect();
    let sensitive = names.iter().any(|n| {
        context.hidden.contains(*n) || context.engines.iter().any(|e| &e.name == *n && e.sensitive)
    });
    let mut notes = Vec::new();
    if result_count > 5 && !context.reranking_configured && !sensitive {
        notes.push(json!({"code":"optional_reranking_unavailable","capability":"jev_reranking","reason":"not_available","message":"Consider configuring Jev reranking for this result set of more than five results. The operator reports substantial production improvements in placing promising results first; benefit for this query has not been measured.","action":{"actor":"operator","kind":"review_optional_reranking"},"quality_evidence":"operator_reported_production","expected_quality_gain":"unmeasured"}));
    }
    if request.engines.as_ref().is_none_or(Vec::is_empty)
        && request.categories.as_ref().is_none_or(Vec::is_empty)
        && request.media_type.is_none()
        && !context.routing_configured
        && scope.jev_added_engines.is_empty()
    {
        notes.push(json!({"code":"optional_routing_unavailable","capability":"jev_specialist_routing","reason":"not_available","message":"Optional Jev specialist routing was unavailable for this request. The operator reports excellent production results from Jev specialist query planning; benefit for this query has not been measured.","action":{"actor":"operator","kind":"review_optional_routing"},"quality_evidence":"operator_reported_production","expected_quality_gain":"unmeasured"}));
    }
    notes.extend(sources);
    notes.truncate(3);
    notes
}
