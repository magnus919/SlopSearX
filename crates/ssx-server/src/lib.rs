//! Pure composition projections shared by HTTP and worker executable roots.
//! Business policy remains in ssx-application; this module performs no I/O.
use ssx_application::{
    health::{HealthState, build_engine_health},
    policy::default_topics,
    scope::{CatalogSnapshot, EngineCapability, RoutingBudget, ScopeResolver, Topic},
};
use ssx_config::{Config, capability_configured};
use ssx_domain::WallClockTimestamp;
use ssx_registry::{AuthClass, Registry};
use std::collections::{BTreeMap, BTreeSet};

/// `active` records actual runtime membership independently from enabled config
/// and registered executable factories. Absent observations remain unknown.
pub fn project_catalog(
    config: &Config,
    registry: &Registry,
    active: &BTreeSet<String>,
    health: &BTreeMap<String, HealthState>,
    now: WallClockTimestamp,
    stale_after: f64,
) -> Result<CatalogSnapshot, String> {
    if active.iter().any(|n| registry.descriptor(n).is_none()) {
        return Err("active engine has no audited descriptor".into());
    }
    let mut engines = Vec::new();
    for d in registry.descriptors() {
        let (enabled, auth, key, categories) = capability_configured(d, config);
        let observation = build_engine_health(health.get(&d.name), None, now, stale_after)
            .map_err(|e| e.to_string())?;
        engines.push(EngineCapability {
            name: d.name.clone(),
            enabled,
            active: active.contains(&d.name),
            available: registry.is_available(&d.name),
            categories,
            media_types: d.supported_media_types.clone(),
            auth_ready: auth != AuthClass::Required || key,
            circuit_open: observation["circuit_open"].as_bool().unwrap_or(false),
            cost_class: d.cost_class.clone().unwrap_or_default(),
            health_status: observation["status"].as_str().unwrap_or("unknown").into(),
            health_stale: observation["stale"].as_bool().unwrap_or(false),
            enforced_filters: d.enforced_filters.clone(),
        });
    }
    CatalogSnapshot::new(engines).map_err(str::to_owned)
}
fn strings(v: &serde_json::Value) -> Result<Vec<String>, String> {
    v.as_array()
        .ok_or("topic list must be an array")?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or("topic list entries must be text".into())
        })
        .collect()
}
fn topic_name(value: Option<&serde_json::Value>, default: &str) -> Result<String, String> {
    match value {
        None => Ok(default.into()),
        Some(serde_json::Value::Null) => Ok(String::new()),
        Some(serde_json::Value::String(s)) => Ok(s.clone()),
        _ => Err("topic name must be text or null".into()),
    }
}
/// A missing router is represented by ScopeResolver::default().topics=None.
/// A configured disabled router is Some(empty), retaining tier-1 fallback.
pub fn project_resolver(
    config: &Config,
    env: &BTreeMap<String, String>,
) -> Result<ScopeResolver, String> {
    let topics = if !config.routing.enabled {
        Vec::new()
    } else {
        match &config.routing.topics {
            None => default_topics(),
            Some(serde_json::Value::Array(items)) => items
                .iter()
                .filter(|v| v.is_object())
                .map(|v| {
                    Ok(Topic {
                        name: topic_name(v.get("name"), "")?,
                        keywords: v
                            .get("keywords")
                            .map(strings)
                            .transpose()?
                            .unwrap_or_default(),
                        engines: v
                            .get("engines")
                            .map(strings)
                            .transpose()?
                            .unwrap_or_default(),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
            Some(serde_json::Value::Object(items)) => {
                if items.len() != config.routing.named_topic_order.len()
                    || config
                        .routing
                        .named_topic_order
                        .iter()
                        .collect::<BTreeSet<_>>()
                        .len()
                        != items.len()
                    || config
                        .routing
                        .named_topic_order
                        .iter()
                        .any(|n| !items.contains_key(n))
                {
                    return Err("named topics require original insertion order; use ordered list topics for programmatic config".into());
                }
                let mut topics = Vec::new();
                for name in &config.routing.named_topic_order {
                    let t = &items[name];
                    if t.is_object() && t.get("keywords").is_some() && t.get("engines").is_some() {
                        topics.push(Topic {
                            name: topic_name(t.get("name"), name)?,
                            keywords: strings(&t["keywords"])?,
                            engines: strings(&t["engines"])?,
                        });
                    }
                }
                topics
            }
            Some(_) => return Err("routing topics must be a mapping or sequence".into()),
        }
    };
    let b = config.routing.budget.as_ref();
    let positive = |key: &str, env_key: &str| -> usize {
        let raw = env
            .get(env_key)
            .map(|s| {
                s.trim_matches(|c: char| c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}'))
            })
            .filter(|s| !s.is_empty());
        let value = b.and_then(|v| v.get(key));
        if let Some(s) = raw.or_else(|| value.and_then(serde_json::Value::as_str)) {
            return ssx_domain::storage::normalize_python_number_text(s)
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0);
        }
        value
            .and_then(|v| {
                if v.is_boolean() {
                    None
                } else {
                    v.as_u64()
                        .and_then(|n| usize::try_from(n).ok())
                        .or_else(|| {
                            v.as_f64()
                                .filter(|f| *f > 0.0 && *f < (usize::MAX as f64))
                                .map(|f| f.trunc() as usize)
                        })
                }
            })
            .unwrap_or(0)
    };
    let max = env
        .get("ROUTING_MAX_COST_CLASS")
        .map(|s| s.trim_matches(|c: char| c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}')))
        .filter(|s| !s.is_empty())
        .or_else(|| b.and_then(|v| v["max_cost_class"].as_str()))
        .filter(|s| matches!(*s, "free" | "freemium" | "paid"))
        .unwrap_or("paid");
    Ok(ScopeResolver {
        topics: Some(topics),
        budget: Some(RoutingBudget {
            max_engines: positive("max_engines_per_intent", "ROUTING_MAX_ENGINES_PER_INTENT"),
            max_cost_class: max.into(),
            coverage_target: positive("coverage_target", "ROUTING_COVERAGE_TARGET"),
        }),
        ..Default::default()
    })
}
