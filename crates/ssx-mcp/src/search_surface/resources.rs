use super::DiscoveryState;
use rmcp::{ErrorData, model::GetPromptResult};
use ssx_json::Value;
fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Null => "None".into(),
        _ => v.to_string(),
    }
}
fn joined(v: &Value) -> String {
    v.as_array()
        .map(|a| a.iter().map(text).collect::<Vec<_>>().join(", "))
        .unwrap_or_default()
}
fn block(cap: &Value, catalog: bool) -> String {
    let marker = if cap["enabled"].as_bool() == Some(true) {
        ""
    } else {
        " (disabled)"
    };
    let mut lines = vec![format!(
        "{} {}{marker} — {}",
        if catalog { "##" } else { "#" },
        text(&cap["name"]),
        text(&cap["display_name"])
    )];
    let supported = [
        "language",
        "time_range",
        "safesearch",
        "pagination",
        "date_from",
        "date_to",
    ]
    .into_iter()
    .filter(|k| cap["supported_filters"][*k].as_bool() == Some(true))
    .collect::<Vec<_>>()
    .join(", ");
    let media = joined(&cap["supported_media_types"]);
    for (key, value) in [
        ("type", text(&cap["type"])),
        ("categories", joined(&cap["categories"])),
        (
            "auth class",
            format!(
                "{}{}",
                text(&cap["auth"]["class"]),
                if cap["auth"]["configured"].as_bool() == Some(true) {
                    ", credentials configured"
                } else {
                    ""
                }
            ),
        ),
        ("sensitive", text(&cap["sensitive"])),
        (
            "supported filters",
            if supported.is_empty() {
                "none".into()
            } else {
                supported
            },
        ),
        (
            "supported result types",
            joined(&cap["supported_result_types"]),
        ),
        (
            "supported media types",
            if media.is_empty() {
                "none".into()
            } else {
                media
            },
        ),
        ("failure classes", joined(&cap["failure_classes"])),
        (
            "cost class",
            cap["cost_class"].as_str().unwrap_or("unknown").into(),
        ),
        ("last known status", text(&cap["last_known_status"])),
        (
            "last known status at",
            cap["last_known_status_at"]
                .as_str()
                .unwrap_or("never observed")
                .into(),
        ),
        ("stale", text(&cap["last_known_status_stale"])),
        ("circuit open", text(&cap["circuit_open"])),
        (
            "consecutive errors",
            text(&cap["circuit_consecutive_errors"]),
        ),
        ("scope hints", joined(&cap["scope_hints"])),
    ] {
        lines.push(format!("- {key}: {value}"));
    }
    if let Some(caveats) = cap["caveats"].as_array().filter(|a| !a.is_empty()) {
        lines.push(format!(
            "- caveats: {}",
            caveats.iter().map(text).collect::<Vec<_>>().join("; ")
        ));
    }
    lines.join("\n")
}
pub(super) fn read(uri: &str, state: &dyn DiscoveryState) -> Result<String, ErrorData> {
    match uri {
        "slopsearx://capabilities" => {
            let mut lines = vec!["# SlopSearX engine catalog".to_owned(), String::new()];
            for cap in state.capabilities() {
                lines.push(block(&cap, true));
                lines.push(String::new());
            }
            Ok(lines.join("\n"))
        }
        "slopsearx://routing-profiles" => {
            let source: Value = ssx_json::from_str(include_str!(
                "../../../../docs/rust-rewrite/t06/source.json"
            ))
            .expect("source");
            Ok(source["routing_profiles"]
                .as_str()
                .expect("captured static routing profiles")
                .into())
        }
        "slopsearx://health/summary" => Ok(health(&state.diagnostics())),
        _ => {
            if let Some(engine) = uri
                .strip_prefix("slopsearx://capabilities/")
                .filter(|s| !s.contains('/'))
            {
                let caps = state.capabilities();
                Ok(
                    if let Some(cap) = caps.iter().find(|c| c["name"].as_str() == Some(engine)) {
                        block(cap, false)
                    } else {
                        let mut names = caps
                            .iter()
                            .filter_map(|c| c["name"].as_str())
                            .collect::<Vec<_>>();
                        names.sort();
                        format!(
                            "Unknown engine '{engine}'. Valid engines: {}",
                            names.join(", ")
                        )
                    },
                )
            } else {
                Err(ErrorData::invalid_params(
                    format!("Resource not found: '{uri}'"),
                    Some(serde_json::json!({"uri":uri})),
                ))
            }
        }
    }
}
fn health(d: &Value) -> String {
    let mut lines = vec![
        "# SlopSearX service status".to_owned(),
        "- status: ok (liveness)".to_owned(),
    ];
    for (label, v) in [
        ("version", &d["version"]),
        ("contract version", &d["contract_version"]),
        ("valkey connected", &d["valkey"]["connected"]),
        ("valkey fail-closed", &d["valkey"]["fail_closed"]),
        ("cache connected", &d["cache_connected"]),
        ("snapshots available", &d["snapshots_available"]),
        ("job store available", &d["job_store_available"]),
        ("active engines", &d["active_engines"]),
        ("router enabled", &d["router_enabled"]),
    ] {
        lines.push(format!("- {label}: {}", text(v)));
    }
    let grants = joined(&d["grants"]["enabled"]);
    lines.push(format!(
        "- grants enabled: {}",
        if grants.is_empty() { "none" } else { &grants }
    ));
    let p = &d["policy_bounds"];
    lines.push(format!("- policy bounds: max_query_length={}, max_results={}, snapshot_ttl_seconds={}, job_max_queries={}, job_max_engines_per_query={}, job_max_results={}, job_default_deadline_seconds={}",text(&p["max_query_length"]),text(&p["max_results"]),text(&p["snapshot_ttl_seconds"]),text(&p["job_max_queries"]),text(&p["job_max_engines_per_query"]),text(&p["job_max_results"]),text(&p["job_default_deadline_seconds"])));
    let causes = joined(&d["degradation"]["causes"]);
    lines.push(format!(
        "- degradation: {} (causes: {})",
        text(&d["degradation"]["summary"]),
        if causes.is_empty() { "none" } else { &causes }
    ));
    lines.push(format!("- freshness: {}", text(&d["freshness"])));
    let e = &d["engine_health"];
    lines.push(format!("- engine health: ok={}, rate_limited={}, blocked={}, error={}, timeout={}, unavailable={}, unknown={}",text(&e["ok"]),text(&e["rate_limited"]),text(&e["blocked"]),text(&e["error"]),text(&e["timeout"]),text(&e["unavailable"]),text(&e["unknown"])));
    lines.push(format!("- engine health note: {}", text(&e["note"])));
    lines.join("\n")
}
pub(super) fn prompt(
    name: &str,
    args: serde_json::Map<String, serde_json::Value>,
) -> Result<GetPromptResult, ErrorData> {
    if args.values().any(|v| !v.is_string()) {
        return Err(ErrorData::invalid_params(
            "Invalid request parameters",
            Some(serde_json::json!("")),
        ));
    }
    let source: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../docs/rust-rewrite/t06/source.json"
    ))
    .expect("source");
    let entry = &source["prompt_calls"][name];
    let Some(argument) = entry["argument"].as_str() else {
        return Err(ErrorData::invalid_params(
            format!("Unknown prompt: '{name}'"),
            None,
        ));
    };
    let Some(value) = args.get(argument).and_then(serde_json::Value::as_str) else {
        return Err(ErrorData::internal_error(
            format!(
                "Error rendering prompt '{name}': Missing required arguments: {{'{argument}'}}"
            ),
            None,
        ));
    };
    let text = entry["template"]
        .as_str()
        .expect("template")
        .replace("<CALLER_DATA>", value);
    let mut result = entry["result"].clone();
    result["messages"][0]["content"]["text"] = serde_json::Value::String(text);
    serde_json::from_value(result).map_err(|e| ErrorData::internal_error(e.to_string(), None))
}
