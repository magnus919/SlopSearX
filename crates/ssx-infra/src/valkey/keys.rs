//! Key bytes deliberately preserve the Python tenant encoding contracts.
use sha2::{Digest, Sha256};
use ssx_domain::TenantId;
pub fn research(kind: &str, tenant: &TenantId, id: &str) -> Option<String> {
    let prefix = match kind {
        "job" => "mcp:job",
        "idem" => "mcp:idem",
        "lease" => "mcp:joblease",
        "cancel" => "mcp:jobcancel",
        _ => return None,
    };
    Some(format!("{prefix}:{tenant}:{id}"))
}
/// Additive W01 key: disjoint from legacy mcp:idem and unambiguous for colons.
pub fn research_idempotency_v2(tenant: &TenantId, id: &str) -> String {
    format!("mcp:idem-v2:{}:{tenant}:{id}", tenant.as_str().len())
}
pub fn snapshot(tenant: &TenantId, id: &str) -> String {
    format!("mcp:snapshot:{tenant}:{id}")
}
pub fn saved(kind: &str, tenant: &TenantId, id: Option<&str>) -> Option<String> {
    if ![
        "definition",
        "definition-count",
        "definitions",
        "recent",
        "due",
        "lease",
        "deleted",
        "report",
        "reports",
        "baseline",
        "events",
        "event-ack",
        "event-consumers",
        "event-expired",
    ]
    .contains(&kind)
    {
        return None;
    }
    Some(match id {
        Some(id) => format!("mcp:saved:v1:{kind}:{tenant}:{id}"),
        None => format!("mcp:saved:v1:{kind}:{tenant}"),
    })
}
pub fn staged(kind: &str, tenant: &TenantId, id: Option<&str>) -> Option<String> {
    if !["record", "index", "idem", "lease", "tenant"].contains(&kind) {
        return None;
    }
    let hash = digest(tenant.as_str());
    let prefix = format!("mcp:staged:v1:{kind}:{}", &hash[..24]);
    Some(match id {
        Some(id) if kind == "idem" => format!("{prefix}:{}", digest(id)),
        Some(id) => format!("{prefix}:{id}"),
        None => prefix,
    })
}
pub fn literal_scan_prefix(prefix: &str) -> String {
    let mut out = String::new();
    for c in prefix.chars() {
        if "\\*?[]".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('*');
    out
}

fn digest(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn research_recent(tenant: &TenantId) -> String {
    format!("mcp:recent:v1:{tenant}")
}
pub fn research_ready(tenant: &TenantId) -> String {
    format!("mcp:ready:v1:tenant:{tenant}")
}
pub const RESEARCH_READY_TENANTS: &str = "mcp:ready:v1:tenants";
pub const RESEARCH_READY_SEQUENCE: &str = "mcp:ready:v1:turn";
pub const SAVED_TENANTS: &str = "mcp:saved:v1:tenants";
pub const STAGED_READY: &str = "mcp:staged:v1:ready";
pub const STAGED_RUNNING: &str = "mcp:staged:v1:running";
