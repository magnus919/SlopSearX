//! Frozen redacted HTTP discovery from the same effective F06 runtime inputs.
use ssx_application::{health::HealthContext, policy::SearchPolicy};
use ssx_config::Config;
use ssx_json::{Value, json};
use ssx_registry::{AuthClass, Registry};
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct ActiveEngine {
    pub name: String,
    pub display_name: String,
    pub engine_type: String,
    pub categories: Vec<String>,
}
pub struct Discovery {
    pub config: Value,
    pub health_contexts: BTreeMap<String, HealthContext>,
}
fn auth_name(auth: AuthClass) -> &'static str {
    match auth {
        AuthClass::None => "none",
        AuthClass::Optional => "optional",
        AuthClass::Required => "required",
        AuthClass::Unknown => "unknown",
    }
}
pub fn build(
    registry: &Registry,
    config: &Config,
    active: &[String],
    policy: &SearchPolicy,
    version: &str,
) -> Result<Discovery, &'static str> {
    let active = active
        .iter()
        .map(|name| {
            let d = registry
                .descriptor(name)
                .ok_or("active engine missing from registry")?;
            Ok(ActiveEngine {
                name: name.clone(),
                display_name: d.display_name.clone(),
                engine_type: d.engine_type.clone(),
                categories: config.engines.get(name).map_or_else(
                    || d.categories.clone(),
                    |e| e.effective_categories(d).to_vec(),
                ),
            })
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    Ok(build_active(
        Some(registry),
        config,
        &active,
        policy,
        version,
    ))
}
/// Catalog failure degrades to source-compatible minimal active engine records.
/// Runtime metadata remains available even when an optional capability entry is absent.
pub fn build_active(
    registry: Option<&Registry>,
    config: &Config,
    active: &[ActiveEngine],
    policy: &SearchPolicy,
    version: &str,
) -> Discovery {
    let mut categories: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut engines = Vec::new();
    let mut health_contexts = BTreeMap::new();
    for runtime in active {
        let name = &runtime.name;
        let engine_categories = &runtime.categories;
        for category in engine_categories {
            let list = categories.entry(category.clone()).or_default();
            if !list.contains(name) {
                list.push(name.clone());
            }
        }
        let mut record = json!({"categories":engine_categories,"enabled":true,"name":name,"shortcut":name,"display_name":if runtime.display_name.is_empty(){name.as_str()}else{&runtime.display_name},"type":runtime.engine_type});
        let Some(descriptor) = registry.and_then(|r| r.descriptor(name)) else {
            engines.push(record);
            health_contexts.insert(
                name.clone(),
                HealthContext {
                    configured: true,
                    auth_class: None,
                    auth_configured: None,
                },
            );
            continue;
        };
        let (enabled, auth, configured, _) = ssx_config::capability_configured(descriptor, config);
        let supported: ssx_json::Map<_, _> = [
            "language",
            "time_range",
            "safesearch",
            "pagination",
            "date_from",
            "date_to",
        ]
        .into_iter()
        .map(|filter| {
            (
                filter.to_owned(),
                json!(
                    descriptor
                        .supported_filters
                        .get(filter)
                        .copied()
                        .unwrap_or(false)
                ),
            )
        })
        .collect();
        let enforced: ssx_json::Map<_, _> = [
            "language",
            "time_range",
            "safesearch",
            "pagination",
            "date_from",
            "date_to",
        ]
        .into_iter()
        .map(|filter| {
            (
                filter.to_owned(),
                json!(descriptor.enforced_filters.get(filter)),
            )
        })
        .collect();
        let mut hints = vec!["explicit"];
        if engine_categories.iter().any(|c| c == "general") {
            hints.push("unscoped");
        }
        let extensions = json!({"sensitive":descriptor.sensitive||policy.sensitive_engines.contains(name),"supported_filters":supported,"enforced_filters":enforced,"supported_result_types":descriptor.supported_result_types,"supported_media_types":descriptor.supported_media_types,"failure_classes":descriptor.failure_classes,"cost_class":descriptor.cost_class.as_ref().filter(|s|!s.is_empty()),"auth":{"class":auth_name(auth),"configured":configured},"scope_hints":hints,"caveats":descriptor.caveats});
        for (k, v) in extensions.as_object().expect("extension object") {
            record[k.as_str()] = v.clone();
        }
        engines.push(record);
        health_contexts.insert(
            name.clone(),
            HealthContext {
                configured: enabled,
                auth_class: Some(auth_name(auth).into()),
                auth_configured: Some(configured),
            },
        );
    }
    let config = json!({"autocomplete":"","categories":categories.keys().collect::<Vec<_>>(),"default_locale":"en","default_theme":"simple","engines":engines,"instance_name":"SlopSearX","locales":{"en":"English"},"plugins":[],"safe_search":0,"version":version,"brand":"SlopSearX","category_engines":categories});
    Discovery {
        config,
        health_contexts,
    }
}
