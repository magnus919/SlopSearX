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

/// A strict readonly header, with validated original result count. This is not
/// a SearchSnapshot with an empty/fabricated result vector.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotSummary {
    id: SnapshotId,
    query: String,
    query_id: QueryId,
    scope: crate::ScopeDecision,
    tenant: TenantId,
    created_at: WallClockTimestamp,
    expires_at: Option<WallClockExpiry>,
    ranking_explanation: String,
    lineage: Vec<Value>,
    lineage_available: bool,
    total: usize,
}
impl SnapshotSummary {
    pub fn from_snapshot(snapshot: &SearchSnapshot) -> Self {
        Self {
            id: snapshot.id().clone(),
            query: snapshot.query().into(),
            query_id: snapshot.query_id().clone(),
            scope: snapshot.scope().clone(),
            tenant: snapshot.tenant().clone(),
            created_at: snapshot.created_at(),
            expires_at: snapshot.expires_at(),
            ranking_explanation: snapshot.ranking_explanation().into(),
            lineage: snapshot.lineage().to_vec(),
            lineage_available: snapshot.lineage_available(),
            total: snapshot.total(),
        }
    }
    pub fn id(&self) -> &SnapshotId {
        &self.id
    }
    pub fn query(&self) -> &str {
        &self.query
    }
    pub fn query_id(&self) -> &QueryId {
        &self.query_id
    }
    pub fn scope(&self) -> &crate::ScopeDecision {
        &self.scope
    }
    pub fn tenant(&self) -> &TenantId {
        &self.tenant
    }
    pub fn created_at(&self) -> WallClockTimestamp {
        self.created_at
    }
    pub fn expires_at(&self) -> Option<WallClockExpiry> {
        self.expires_at
    }
    pub fn ranking_explanation(&self) -> &str {
        &self.ranking_explanation
    }
    pub fn lineage(&self) -> &[Value] {
        &self.lineage
    }
    pub fn lineage_available(&self) -> bool {
        self.lineage_available
    }
    pub fn total(&self) -> usize {
        self.total
    }
    fn into_snapshot(
        self,
        results: Vec<crate::SearchResult>,
    ) -> Result<SearchSnapshot, StorageError> {
        if results.len() != self.total {
            return Err(StorageError::InvalidResponse);
        }
        SearchSnapshot::new(
            self.id,
            self.query,
            self.query_id,
            results,
            self.scope,
            self.tenant,
            self.created_at,
            self.expires_at,
        )
        .map(|s| {
            s.with_lineage(
                self.ranking_explanation,
                self.lineage,
                self.lineage_available,
            )
        })
        .map_err(|_| StorageError::InvalidResponse)
    }
}
type Object = ssx_json::Map<String, Value>;
type SnapshotParts<'a> = (&'a Object, Option<&'a Object>, &'a [Value]);
fn parts(value: &Value) -> Result<SnapshotParts<'_>, StorageError> {
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
    Ok((object, scope, raw_results))
}
fn header(
    object: &ssx_json::Map<String, Value>,
    scope: Option<&ssx_json::Map<String, Value>>,
    result_count: usize,
) -> Result<SnapshotSummary, StorageError> {
    if object.get("total").is_some_and(Value::is_null) {
        return Err(StorageError::InvalidNumber("total"));
    }
    let total = parse_i64(object.get("total"), 0, "total")?;
    if usize::try_from(total).ok() != Some(result_count) {
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
    let id = SnapshotId::new(source_string(object.get("snapshot_id"), "")).map_err(map_error)?;
    let query = source_string(object.get("query"), "");
    let query_id = QueryId::new(source_string(object.get("query_id"), "")).map_err(map_error)?;
    let tenant = TenantId::new(source_string(object.get("tenant"), "")).map_err(map_error)?;
    let created_at = WallClockTimestamp::new(created).map_err(map_error)?;
    let expires_at = expiry
        .map(WallClockExpiry::new)
        .transpose()
        .map_err(map_error)?;
    crate::identity::validate_snapshot_expiry(created_at, expires_at).map_err(map_error)?;
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
    Ok(SnapshotSummary {
        id,
        query,
        query_id,
        scope,
        tenant,
        created_at,
        expires_at,
        total: result_count,
        ranking_explanation: source_string(
            object.get("ranking_explanation"),
            "tier_then_cross_engine_presence",
        ),
        lineage,
        lineage_available: object.contains_key("lineage"),
    })
}
/// Read the Python legacy projection while retaining accepted F05 invariants.
pub fn decode(value: Value) -> Result<SearchSnapshot, StorageError> {
    let (object, scope, raw_results) = parts(&value)?;
    let results = raw_results
        .iter()
        .cloned()
        .map(decode_result)
        .collect::<Result<Vec<_>, _>>()?;
    header(object, scope, results.len())?.into_snapshot(results)
}
/// First pass validates EVERY row through the existing F05 decoder, then the
/// identical header grammar/order. Each typed result drops before the next row.
/// No callback runs here, including on expired/misdirected/corrupt records.
pub fn decode_rows(value: Value) -> Result<DecodedSnapshotRows, StorageError> {
    let (object, scope, raw_results) = parts(&value)?;
    for row in raw_results {
        drop(decode_result_ref(row)?);
    }
    let summary = header(object, scope, raw_results.len())?;
    Ok(DecodedSnapshotRows { value, summary })
}
/// Owns one parsed bounded record, not Vec<SearchResult>. The second CPU pass is
/// explicit. Callers must apply their authority/expiry/policy fence before visit.
/// The input tree cannot mutate between the two validation passes.
pub struct DecodedSnapshotRows {
    value: Value,
    summary: SnapshotSummary,
}
impl DecodedSnapshotRows {
    pub fn summary(&self) -> &SnapshotSummary {
        &self.summary
    }
    pub fn visit_results(
        &self,
        visitor: &mut dyn FnMut(usize, &crate::SearchResult),
    ) -> Result<(), StorageError> {
        let (_, _, rows) = parts(&self.value)?;
        for (index, row) in rows.iter().enumerate() {
            let decoded = decode_result_ref(row)?;
            visitor(index, &decoded);
        }
        Ok(())
    }
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
