//! Source-compatible snapshot payload codec. Legacy reads never grant authority.
use super::*;
use crate::{QueryId, SearchSnapshot, SnapshotId, TenantId, WallClockExpiry, WallClockTimestamp};

/// Preserve the Python snapshot writer's field set and F05 result codecs.
pub fn encode(snapshot: &SearchSnapshot) -> Result<Value, StorageError> {
    encode_with_limits(snapshot, StorageLimits::default())
}

pub fn encode_with_limits(
    snapshot: &SearchSnapshot,
    limits: StorageLimits,
) -> Result<Value, StorageError> {
    let results = snapshot
        .results()
        .iter()
        .map(|result| encode_result_with_limits(result, limits))
        .collect::<Result<Vec<_>, _>>()?;
    ssx_json::to_value(StoredSnapshot {
        snapshot_id: snapshot.id().as_str(),
        query: snapshot.query(),
        query_id: snapshot.query_id().as_str(),
        ranking_explanation: snapshot.ranking_explanation(),
        results,
        scope: StoredScope::from(snapshot.scope()),
        total: snapshot.total(),
        tenant: snapshot.tenant().as_str(),
        created_at: snapshot.created_at().seconds(),
        expires_at: snapshot.expires_at().map(|v| v.timestamp().seconds()),
        lineage: snapshot.lineage(),
    })
    .map_err(|_| StorageError::Serialization)
}
// Field order deliberately preserves the actual source/native durable byte profile.
// Unlike json! dynamic expressions, conversion is fallible for the whole record.
#[derive(Serialize)]
struct StoredSnapshot<'a> {
    snapshot_id: &'a str,
    query: &'a str,
    query_id: &'a str,
    ranking_explanation: &'a str,
    results: Vec<Value>,
    scope: StoredScope,
    total: usize,
    tenant: &'a str,
    created_at: f64,
    expires_at: Option<f64>,
    lineage: &'a [Value],
}

/// Read the Python legacy projection while retaining accepted F05 invariants.
pub fn decode(value: Value) -> Result<SearchSnapshot, StorageError> {
    let object = value
        .as_object()
        .ok_or(StorageError::ExpectedObject("snapshot"))?;
    let scope = match object.get("scope") {
        Some(value) if python_truthy(value) => Some(
            value
                .as_object()
                .ok_or(StorageError::ExpectedObject("snapshot scope"))?,
        ),
        _ => None,
    };
    let raw_results = match object.get("results") {
        Some(value) if python_truthy(value) => value
            .as_array()
            .ok_or(StorageError::ExpectedObject("snapshot results"))?
            .as_slice(),
        _ => &[],
    };
    let results = raw_results
        .iter()
        .cloned()
        .map(decode_result)
        .collect::<Result<Vec<_>, _>>()?;
    if object.get("total").is_some_and(Value::is_null) {
        return Err(StorageError::InvalidNumber("total"));
    }
    let total = parse_i64(object.get("total"), 0, "total")?;
    if usize::try_from(total).ok() != Some(results.len()) {
        return Err(StorageError::InvalidResponse);
    }
    let created = object
        .get("created_at")
        .map(parse_value_f64)
        .transpose()?
        .unwrap_or(0.0);
    let expiry = object
        .get("expires_at")
        .filter(|v| !v.is_null())
        .map(parse_value_f64)
        .transpose()?;
    let map_error = |_| StorageError::InvalidResponse;
    if scope
        .and_then(|s| s.get("jev_scores"))
        .is_some_and(|v| python_truthy(v) && !v.is_object())
    {
        return Err(StorageError::ExpectedObject("snapshot Jev scores"));
    }
    let mut decoded_scope = decode_scope(scope)?;
    if let Some(raw) = scope {
        decoded_scope.selected_engines = legacy_strings(raw.get("selected_engines"))?;
        decoded_scope.resolved_categories = legacy_strings(raw.get("resolved_categories"))?;
        decoded_scope.warnings = legacy_strings(raw.get("warnings"))?;
        decoded_scope.jev_added_engines = legacy_strings(raw.get("jev_added_engines"))?;
    }
    let mut scope = decoded_scope;
    // Python's snapshot reader historically projects only these scope fields.
    scope.excluded_engines.clear();
    scope.routing_fallback = false;
    scope.routing_budget_applied = false;
    scope.routing_tradeoffs.clear();
    let snapshot = SearchSnapshot::new(
        SnapshotId::new(source_string(object.get("snapshot_id"), "")).map_err(map_error)?,
        source_string(object.get("query"), ""),
        QueryId::new(source_string(object.get("query_id"), "")).map_err(map_error)?,
        results,
        scope,
        TenantId::new(source_string(object.get("tenant"), "")).map_err(map_error)?,
        WallClockTimestamp::new(created).map_err(map_error)?,
        expiry
            .map(WallClockExpiry::new)
            .transpose()
            .map_err(map_error)?,
    )
    .map_err(map_error)?;
    if object.get("lineage").is_some_and(|v| {
        python_truthy(v) && !matches!(v, Value::Array(_) | Value::String(_) | Value::Object(_))
    }) {
        return Err(StorageError::InvalidResponse);
    }
    let lineage = array_field(object.get("lineage"))
        .iter()
        .filter(|v| v.is_object())
        .cloned()
        .collect();
    Ok(snapshot.with_lineage(
        source_string(
            object.get("ranking_explanation"),
            "tier_then_cross_engine_presence",
        ),
        lineage,
        object.contains_key("lineage"),
    ))
}

// SnapshotStore explicitly uses list(value or []), unlike the general F05
// response reader. Preserve valid typed outputs of strings/object-key iteration.
fn legacy_strings(value: Option<&Value>) -> Result<Vec<String>, StorageError> {
    match value.filter(|v| python_truthy(v)) {
        None => Ok(Vec::new()),
        Some(Value::String(v)) => Ok(v.chars().map(|c| c.to_string()).collect()),
        Some(Value::Object(v)) => Ok(v.keys().cloned().collect()),
        Some(Value::Array(v)) => v
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_owned)
                    .ok_or(StorageError::InvalidResponse)
            })
            .collect(),
        Some(_) => Err(StorageError::InvalidResponse),
    }
}
