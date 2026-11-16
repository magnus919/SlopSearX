//! Payload-only Python `_json_safe` projection for adapter source values.
//!
//! This leaves ordinary Value::project and result title/content handling intact.
//! The legacy semantic wrapper remains synchronous. Additive charged visitors
//! and the narrow batch assembler require caller-owned source/scratch budgets;
//! their operation accounting ends explicitly at final adapter response handoff.
use super::source_json::{ProjectionError, Text, Value};
mod batch;
#[cfg(test)]
pub(crate) use batch::{BATCH_PHASE, BatchPhase};
pub use batch::{Outcome, ProducedBatch, Skeleton};

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Projection(ProjectionError),
    Resource,
}
fn sanitized(text: &Text) -> Result<String, Error> {
    fn point(point: u32) -> Result<char, Error> {
        if (0xd800..=0xdfff).contains(&point) {
            Ok('\u{fffd}')
        } else {
            char::from_u32(point).ok_or(Error::Projection(ProjectionError::Unicode))
        }
    }
    let length = text.codepoints().iter().try_fold(0usize, |n, p| {
        n.checked_add(point(*p)?.len_utf8()).ok_or(Error::Resource)
    })?;
    let mut output = String::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| Error::Resource)?;
    for p in text.codepoints() {
        output.push(point(*p)?);
    }
    Ok(output)
}
/// Project one retained top-level build_payload data field (depth1). Caller
/// drops top-level None values first and keeps original field names/order for
/// provenance. Nested Null remains. Nonfinite numbers are a typed refusal,
/// never silently replaced with Null; callers must finish source operations
/// before applying any pending output-representability refusal.
pub fn project_field(value: &Value) -> Result<ssx_json::Value, Error> {
    project_at(value, 1)
}
fn project_at(value: &Value, depth: usize) -> Result<ssx_json::Value, Error> {
    use ssx_json::Value as Out;
    if depth > 100 {
        return Ok(Out::String("<max depth exceeded>".into()));
    }
    Ok(match value {
        Value::Null => Out::Null,
        Value::Bool(value) => Out::Bool(*value),
        Value::Number(value) => Out::Number(value.project().map_err(Error::Projection)?),
        Value::String(value) => Out::String(sanitized(value)?),
        Value::Array(values) => {
            let mut output = Vec::new();
            output
                .try_reserve_exact(values.len())
                .map_err(|_| Error::Resource)?;
            for value in values {
                output.push(project_at(value, depth + 1)?);
            }
            Out::Array(output)
        }
        Value::Object(values) => {
            let mut output = ssx_json::Map::new();
            output
                .try_reserve_exact(values.len())
                .map_err(|_| Error::Resource)?;
            for (key, value) in values.iter() {
                // Ordered insertion matches Python dict comprehension: a
                // sanitized collision keeps first position and last value.
                output.insert(sanitized(key)?, project_at(value, depth + 1)?);
            }
            Out::Object(output)
        }
    })
}

/// Charged output of one payload field. Storage drops before all conservative
/// backing reservations. Borrowing does not authorize cloning an uncharged tree.
pub struct BudgetPayload {
    value: ssx_json::Value,
    _charge: ssx_owned_budget::Charge,
}
impl BudgetPayload {
    pub fn value(&self) -> &ssx_json::Value {
        &self.value
    }
    pub fn charged_bytes(&self) -> usize {
        self._charge.bytes()
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum BudgetError {
    Projection(ProjectionError),
    Resource,
    Deadline,
    UnsupportedLayout,
}
struct Projection<'a> {
    budget: &'a ssx_owned_budget::Budget,
    charge: ssx_owned_budget::Charge,
    deadline: Option<std::time::Instant>,
}
impl Projection<'_> {
    fn reserve(&mut self, bytes: usize) -> Result<(), BudgetError> {
        let mut charge = self
            .budget
            .reserve(bytes)
            .map_err(|_| BudgetError::Resource)?;
        self.charge
            .merge_from(&mut charge)
            .map_err(|_| BudgetError::Resource)
    }
    async fn checkpoint(&self) -> Result<(), BudgetError> {
        if self
            .deadline
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
        {
            return Err(BudgetError::Deadline);
        }
        tokio::task::yield_now().await;
        if self
            .deadline
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
        {
            return Err(BudgetError::Deadline);
        }
        Ok(())
    }
    async fn string(&mut self, text: &Text) -> Result<String, BudgetError> {
        if text.codepoints().len() > 4 * 1024 * 1024 {
            return Err(BudgetError::Resource);
        }
        let mut bytes = 0usize;
        for (i, &point) in text.codepoints().iter().enumerate() {
            if i % 1024 == 0 {
                self.checkpoint().await?;
            }
            bytes = bytes
                .checked_add(safe_point(point)?.len_utf8())
                .ok_or(BudgetError::Resource)?;
        }
        self.reserve(bytes)?;
        let mut out = String::new();
        out.try_reserve_exact(bytes)
            .map_err(|_| BudgetError::Resource)?;
        if out.capacity() != bytes {
            return Err(BudgetError::Resource);
        }
        for (i, &point) in text.codepoints().iter().enumerate() {
            if i % 1024 == 0 {
                self.checkpoint().await?;
            }
            out.push(safe_point(point)?);
        }
        Ok(out)
    }
}
fn safe_point(point: u32) -> Result<char, BudgetError> {
    if (0xd800..=0xdfff).contains(&point) {
        Ok('\u{fffd}')
    } else {
        char::from_u32(point).ok_or(BudgetError::Projection(ProjectionError::Unicode))
    }
}
/// Requested layouts for NEW empty locked IndexMap2.14.2/hashbrown0.17.1.
/// The supported target prediction is separately qualified by the allocator
/// probe; allocator metadata/usable-size are outside this backing ledger.
fn map_bytes(n: usize) -> Result<usize, BudgetError> {
    use std::mem::{align_of, size_of};
    if size_of::<usize>() != 8
        || align_of::<ssx_json::Value>() != 8
        || size_of::<ssx_json::Value>() != 72
    {
        return Err(BudgetError::UnsupportedLayout);
    }
    let group = if cfg!(target_arch = "aarch64") {
        8usize
    } else if cfg!(target_arch = "x86_64") {
        16
    } else {
        return Err(BudgetError::UnsupportedLayout);
    };
    if n == 0 {
        return Ok(0);
    }
    let buckets = match n {
        1..=3 => 4,
        4..=7 => 8,
        8..=14 => 16,
        _ => n
            .checked_mul(8)
            .and_then(|v| (v / 7).checked_next_power_of_two())
            .ok_or(BudgetError::Resource)?,
    };
    let offset = buckets
        .checked_mul(8)
        .and_then(|v| v.checked_add(group - 1))
        .map(|v| v & !(group - 1))
        .ok_or(BudgetError::Resource)?;
    let bytes = n
        .checked_mul(8 + size_of::<String>() + size_of::<ssx_json::Value>())
        .and_then(|v| v.checked_add(offset))
        .and_then(|v| v.checked_add(buckets))
        .and_then(|v| v.checked_add(group))
        .ok_or(BudgetError::Resource)?;
    if bytes > isize::MAX as usize {
        return Err(BudgetError::Resource);
    }
    Ok(bytes)
}
type Visit<'a> = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<ssx_json::Value, BudgetError>> + Send + 'a>,
>;
fn visit<'a>(
    value: &'a Value,
    depth: usize,
    state: &'a mut Projection<'_>,
) -> Result<Visit<'a>, BudgetError> {
    fn wrap<'a, F>(future: F, budget: &ssx_owned_budget::Budget) -> Result<Visit<'a>, BudgetError>
    where
        F: std::future::Future<Output = Result<ssx_json::Value, BudgetError>> + Send + 'a,
    {
        // The box contains the future and charge together; its storage is
        // accounted before allocation. Process containment handles system OOM.
        struct Charged<F> {
            inner: std::pin::Pin<Box<F>>,
            _charge: ssx_owned_budget::Charge,
        }
        impl<F: std::future::Future> std::future::Future for Charged<F> {
            type Output = F::Output;
            fn poll(
                mut self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<Self::Output> {
                self.inner.as_mut().poll(cx)
            }
        }
        let bytes = std::alloc::Layout::new::<F>()
            .size()
            .checked_add(std::alloc::Layout::new::<Charged<F>>().size())
            .ok_or(BudgetError::Resource)?;
        let charge = budget.reserve(bytes).map_err(|_| BudgetError::Resource)?;
        Ok(Box::pin(Charged {
            inner: Box::pin(future),
            _charge: charge,
        }))
    }
    let budget = state.budget;
    wrap(
        async move {
            use ssx_json::Value as Out;
            state.checkpoint().await?;
            if depth > 100 {
                const SENTINEL: &str = "<max depth exceeded>";
                state.reserve(SENTINEL.len())?;
                let mut text = String::new();
                text.try_reserve_exact(SENTINEL.len())
                    .map_err(|_| BudgetError::Resource)?;
                if text.capacity() != SENTINEL.len() {
                    return Err(BudgetError::Resource);
                }
                text.push_str(SENTINEL);
                return Ok(Out::String(text));
            }
            Ok(match value {
                Value::Null => Out::Null,
                Value::Bool(v) => Out::Bool(*v),
                Value::Number(v) => {
                    if let Some(raw) = v.integer_text() {
                        // Validation owns a RawValue token while constructing
                        // the final Number token; map failure, never expect.
                        state.reserve(raw.len().checked_mul(2).ok_or(BudgetError::Resource)?)?;
                        Out::Number(ssx_json::Number::new(raw).map_err(|_| BudgetError::Resource)?)
                    } else {
                        let super::source_json::Number::Float(value) = v else {
                            unreachable!("source numeric variants");
                        };
                        if !value.is_finite() {
                            return Err(BudgetError::Projection(ProjectionError::Nonfinite));
                        }
                        // Reviewed shared formatter: fixed stack scratch, one
                        // fallible output allocation, capacity <=25 bytes.
                        state.reserve(25)?;
                        Out::Number(
                            ssx_json::Number::from_f64(*value)
                                .map_err(|_| BudgetError::Resource)?,
                        )
                    }
                }
                Value::String(v) => Out::String(state.string(v).await?),
                Value::Array(values) => {
                    let bytes = std::alloc::Layout::array::<Out>(values.len())
                        .map_err(|_| BudgetError::Resource)?
                        .size();
                    state.reserve(bytes)?;
                    let mut out = Vec::new();
                    out.try_reserve_exact(values.len())
                        .map_err(|_| BudgetError::Resource)?;
                    if out.capacity() != values.len() {
                        return Err(BudgetError::Resource);
                    }
                    for v in values {
                        out.push(visit(v, depth + 1, state)?.await?);
                    }
                    Out::Array(out)
                }
                Value::Object(values) => {
                    state.reserve(map_bytes(values.len())?)?;
                    let mut out = ssx_json::Map::new();
                    out.try_reserve_exact(values.len())
                        .map_err(|_| BudgetError::Resource)?;
                    if out.capacity() != values.len() {
                        return Err(BudgetError::Resource);
                    }
                    for (key, v) in values.iter() {
                        let key = state.string(key).await?;
                        let v = visit(v, depth + 1, state)?.await?;
                        state.checkpoint().await?;
                        out.insert(key, v);
                        state.checkpoint().await?;
                    }
                    Out::Object(out)
                }
            })
        },
        budget,
    )
}
/// Charged/cooperative field visitor. Source input ownership, surrounding
/// batch assembly and final response handoff remain the caller's responsibility.
/// Individual bounded map insertions are synchronous between checkpoints;
/// this does not provide process-preemptive deadline enforcement.
pub async fn project_field_budgeted(
    value: &Value,
    budget: &ssx_owned_budget::Budget,
    deadline: std::time::Instant,
) -> Result<BudgetPayload, BudgetError> {
    project_field_budgeted_optional(value, budget, Some(deadline)).await
}
/// None preserves legacy callers without imposing a new parse deadline.
/// Cooperative yields and requested backing limits remain in effect.
pub async fn project_field_budgeted_optional(
    value: &Value,
    budget: &ssx_owned_budget::Budget,
    deadline: Option<std::time::Instant>,
) -> Result<BudgetPayload, BudgetError> {
    let mut state = Projection {
        budget,
        charge: budget.reserve(0).map_err(|_| BudgetError::Resource)?,
        deadline,
    };
    let value = visit(value, 1, &mut state)?.await?;
    state.checkpoint().await?;
    Ok(BudgetPayload {
        value,
        _charge: state.charge,
    })
}
/// Borrowed source descriptors. SelectedTexts preserves unpaired codepoints and
/// author order without constructing or cloning an intermediate source array.
#[derive(Clone, Copy)]
pub enum FieldRef<'a> {
    Value(&'a Value),
    Text(&'a Text),
    SelectedTexts(&'a [&'a Text]),
    /// Arbitrary borrowed author values; source payload sanitation stays shared.
    SelectedValues(&'a [&'a Value]),
    /// Already scalar-only Expat fields; no temporary SourceText allocation.
    Scalar(&'a str),
    /// Borrowed scalar author views; descriptor storage is caller-owned/charged.
    SelectedScalars(&'a [&'a str]),
}
#[derive(Clone, Copy)]
pub struct Field<'a> {
    pub name: &'a str,
    pub value: FieldRef<'a>,
}
impl Projection<'_> {
    async fn scalar(&mut self, text: &str) -> Result<String, BudgetError> {
        self.reserve(text.len())?;
        let mut out = String::new();
        out.try_reserve_exact(text.len())
            .map_err(|_| BudgetError::Resource)?;
        if out.capacity() != text.len() {
            return Err(BudgetError::Resource);
        }
        for (i, ch) in text.chars().enumerate() {
            if i % 1024 == 0 {
                self.checkpoint().await?;
            }
            out.push(ch);
        }
        Ok(out)
    }
    fn map(&mut self, count: usize) -> Result<ssx_json::Map<String, ssx_json::Value>, BudgetError> {
        self.reserve(map_bytes(count)?)?;
        let mut out = ssx_json::Map::new();
        out.try_reserve_exact(count)
            .map_err(|_| BudgetError::Resource)?;
        if out.capacity() != count {
            return Err(BudgetError::Resource);
        }
        Ok(out)
    }
    fn array(&mut self, count: usize) -> Result<Vec<ssx_json::Value>, BudgetError> {
        self.reserve(
            std::alloc::Layout::array::<ssx_json::Value>(count)
                .map_err(|_| BudgetError::Resource)?
                .size(),
        )?;
        let mut out = Vec::new();
        out.try_reserve_exact(count)
            .map_err(|_| BudgetError::Resource)?;
        if out.capacity() != count {
            return Err(BudgetError::Resource);
        }
        Ok(out)
    }
    async fn insert(
        &mut self,
        map: &mut ssx_json::Map<String, ssx_json::Value>,
        key: &str,
        value: ssx_json::Value,
    ) -> Result<(), BudgetError> {
        let key = self.scalar(key).await?;
        self.checkpoint().await?;
        map.insert(key, value);
        self.checkpoint().await
    }
}
/// Build the full science/publication envelope from borrowed source fields.
/// Top-level None drops before sanitation, selected authors retain their raw
/// source Text, and provenance follows original compact-field insertion order.
/// Repeated descriptor names behave like the source dict before compacting:
/// first insertion position, last value (including a last None dropping it).
pub async fn publication_budgeted(
    engine: &str,
    fields: &[Field<'_>],
    budget: &ssx_owned_budget::Budget,
    deadline: Option<std::time::Instant>,
) -> Result<BudgetPayload, BudgetError> {
    envelope_budgeted("science", "publication", engine, fields, budget, deadline).await
}

/// Generic source payload envelope. Domain/type are engine declarations, never
/// inferred from provider data. Identical visitor, provenance and charge owner
/// to the publication API; existing callers retain that API unchanged.
pub async fn envelope_budgeted(
    domain: &str,
    kind: &str,
    engine: &str,
    fields: &[Field<'_>],
    budget: &ssx_owned_budget::Budget,
    deadline: Option<std::time::Instant>,
) -> Result<BudgetPayload, BudgetError> {
    use ssx_json::Value as Out;
    let mut state = Projection {
        budget,
        charge: budget.reserve(0).map_err(|_| BudgetError::Resource)?,
        deadline,
    };
    state.checkpoint().await?;
    // Source engine field lists are short, fixed adapter declarations. The
    // allocation-free duplicate walk is quadratic; checkpoints bound its work.
    let mut data = state.map(fields.len())?;
    let mut names = state.array(fields.len())?;
    for (i, field) in fields.iter().enumerate() {
        state.checkpoint().await?;
        let mut seen = false;
        for (n, old) in fields[..i].iter().enumerate() {
            if n % 128 == 0 {
                state.checkpoint().await?;
            }
            if old.name == field.name {
                seen = true;
                break;
            }
        }
        if seen {
            continue;
        }
        let mut selected = field;
        for (n, next) in fields[i + 1..].iter().enumerate() {
            if n % 128 == 0 {
                state.checkpoint().await?;
            }
            if next.name == field.name {
                selected = next;
            }
        }
        let value = match selected.value {
            FieldRef::Value(Value::Null) => continue,
            FieldRef::Value(value) => visit(value, 1, &mut state)?.await?,
            FieldRef::Text(text) => Out::String(state.string(text).await?),
            FieldRef::Scalar(text) => Out::String(state.scalar(text).await?),
            FieldRef::SelectedValues(values) => {
                let mut output = state.array(values.len())?;
                for (index, value) in values.iter().enumerate() {
                    if index % 128 == 0 {
                        state.checkpoint().await?;
                    }
                    output.push(visit(value, 2, &mut state)?.await?);
                }
                Out::Array(output)
            }
            FieldRef::SelectedScalars(texts) => {
                let mut output = state.array(texts.len())?;
                for (index, text) in texts.iter().enumerate() {
                    if index % 128 == 0 {
                        state.checkpoint().await?;
                    }
                    output.push(Out::String(state.scalar(text).await?));
                }
                Out::Array(output)
            }
            FieldRef::SelectedTexts(texts) => {
                let mut output = state.array(texts.len())?;
                for (index, text) in texts.iter().enumerate() {
                    if index % 128 == 0 {
                        state.checkpoint().await?;
                    }
                    output.push(Out::String(state.string(text).await?));
                }
                Out::Array(output)
            }
        };
        // Engine-owned top-level names are scalar strings. Nested provider keys
        // go through visit and payload-only surrogate sanitation.
        names.push(Out::String(state.scalar(field.name).await?));
        state.insert(&mut data, field.name, value).await?;
    }
    let mut provenance = state.map(4)?;
    let engine = Out::String(state.scalar(engine).await?);
    state.insert(&mut provenance, "engine", engine).await?;
    state
        .insert(&mut provenance, "adapter_fields", Out::Array(names))
        .await?;
    state
        .insert(&mut provenance, "normalized_fields", Out::Array(Vec::new()))
        .await?;
    state
        .insert(&mut provenance, "inferred_fields", Out::Array(Vec::new()))
        .await?;
    let mut out = state.map(5)?;
    let domain = Out::String(state.scalar(domain).await?);
    state.insert(&mut out, "domain", domain).await?;
    let kind = Out::String(state.scalar(kind).await?);
    state.insert(&mut out, "type", kind).await?;
    state.reserve(2)?;
    let version = ssx_json::Number::new("1").map_err(|_| BudgetError::Resource)?;
    state
        .insert(&mut out, "schema_version", Out::Number(version))
        .await?;
    state.insert(&mut out, "data", Out::Object(data)).await?;
    state
        .insert(&mut out, "provenance", Out::Object(provenance))
        .await?;
    state.checkpoint().await?;
    Ok(BudgetPayload {
        value: Out::Object(out),
        _charge: state.charge,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::source_json::{Map, Number};
    fn tagged(value: &ssx_json::Value) -> Value {
        match value["type"].as_str().unwrap() {
            "str" => Value::String(Text::from_points(
                value["codepoints"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| p.as_u64().unwrap() as u32)
                    .collect(),
            )),
            "dict" => {
                let mut map = Map::new();
                for pair in value["items"].as_array().unwrap() {
                    let Value::String(key) = tagged(&pair[0]) else {
                        panic!("source string key")
                    };
                    map.insert(key, tagged(&pair[1]));
                }
                Value::Object(map)
            }
            "list" => Value::Array(
                value["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(tagged)
                    .collect(),
            ),
            "NoneType" => Value::Null,
            "bool" => Value::Bool(value["value"].as_bool().unwrap()),
            "int" => Value::integer(value["value"].as_i64().unwrap()),
            kind => panic!("unexpected frozen tagged input {kind}"),
        }
    }
    #[tokio::test]
    async fn actual_source_payload_controls_preserve_codepoints_order_and_depth() {
        let fixture: ssx_json::Value = ssx_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a06/source-payload-controls.json"
        ))
        .unwrap();
        let mut checked = Vec::new();
        for row in fixture["records"].as_array().unwrap() {
            let root = if let Some(depth) = row["list_depth"].as_u64() {
                let mut value = Value::String(Text::from("leaf"));
                for _ in 0..depth {
                    value = Value::Array(vec![value]);
                }
                let mut root = Map::new();
                root.insert(Text::from("doi"), value);
                Value::Object(root)
            } else {
                tagged(&row["tagged_input"])
            };
            let Value::Object(fields) = root else {
                panic!("source compact data")
            };
            let mut data = ssx_json::Map::new();
            let mut names = Vec::new();
            for (key, value) in fields.iter() {
                if matches!(value, Value::Null) {
                    continue;
                }
                let key = key.scalar().unwrap();
                names.push(ssx_json::Value::String(key.clone()));
                let budget = ssx_owned_budget::Budget::new(1024 * 1024).unwrap();
                let bounded = project_field_budgeted(
                    value,
                    &budget,
                    std::time::Instant::now() + std::time::Duration::from_secs(5),
                )
                .await
                .unwrap();
                assert_eq!(bounded.value(), &project_field(value).unwrap());
                drop(bounded);
                assert_eq!(budget.snapshot().unwrap().used, 0);
                data.insert(key, project_field(value).unwrap());
            }
            assert_eq!(
                ssx_json::to_string(&ssx_json::Value::Object(data)).unwrap(),
                row["expected_data_json"].as_str().unwrap(),
                "{}",
                row["id"]
            );
            assert_eq!(
                ssx_json::Value::Array(names),
                row["expected_adapter_fields"],
                "{} provenance",
                row["id"]
            );
            checked.push(row["id"].as_str().unwrap());
        }
        assert_eq!(
            checked,
            [
                "doi_lone_surrogate",
                "nested_keys_null_false_zero",
                "sanitized_key_collision_order",
                "paired_scalar_vs_separate_surrogates",
                "depth-99",
                "depth-100",
                "depth-101"
            ]
        );
    }
    #[tokio::test]
    async fn budgeted_projection_retains_storage_and_restores_on_refusal() {
        use ssx_owned_budget::Budget;
        use std::time::{Duration, Instant};
        let mut input = Map::new();
        input.insert(
            Text::from_points(vec![120, 0xd800]),
            Value::Array(vec![Value::Null, Value::integer(42)]),
        );
        input.insert(
            Text::from_points(vec![120, 0xdfff]),
            Value::String(Text::from("last")),
        );
        let input = Value::Object(input);
        let budget = Budget::new(1024 * 1024).unwrap();
        let output =
            project_field_budgeted(&input, &budget, Instant::now() + Duration::from_secs(5))
                .await
                .unwrap();
        assert_eq!(output.value(), &project_field(&input).unwrap());
        assert_eq!(budget.snapshot().unwrap().used, output.charged_bytes());
        assert!(output.charged_bytes() > 0);
        drop(output);
        assert_eq!(budget.snapshot().unwrap().used, 0);
        let tiny = Budget::new(1).unwrap();
        assert!(matches!(
            project_field_budgeted(&input, &tiny, Instant::now() + Duration::from_secs(5)).await,
            Err(BudgetError::Resource)
        ));
        assert_eq!(tiny.snapshot().unwrap().used, 0);
        assert!(matches!(
            project_field_budgeted(&input, &budget, Instant::now()).await,
            Err(BudgetError::Deadline)
        ));
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[tokio::test]
    async fn publication_envelope_matches_complete_actual_source_payloads() {
        use ssx_owned_budget::Budget;
        let fixture: ssx_json::Value = ssx_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a06/source-publication-envelope.json"
        ))
        .unwrap();
        let mut checked = Vec::new();
        for record in fixture["cases"].as_array().unwrap().iter().take(2) {
            let engine = record["engine"].as_str().unwrap();
            let wire = record["wire_response_bodies"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()
                .as_str()
                .unwrap();
            let source = super::super::source_json::from_slice(wire.as_bytes(), None)
                .await
                .unwrap();
            let budget = Budget::new(1024 * 1024).unwrap();
            let output = if engine == "pubmed" {
                let item = source.get("result").unwrap().get("123").unwrap();
                let names: Vec<&Text> = item
                    .get("authors")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.get("name").unwrap().as_text().unwrap())
                    .collect();
                let id = Value::String(Text::from("123"));
                let types = Value::Array(Vec::new());
                publication_budgeted(
                    engine,
                    &[
                        Field {
                            name: "publication_id",
                            value: FieldRef::Value(&id),
                        },
                        Field {
                            name: "pmid",
                            value: FieldRef::Value(&id),
                        },
                        Field {
                            name: "publication_types",
                            value: FieldRef::Value(&types),
                        },
                        Field {
                            name: "journal",
                            value: FieldRef::Value(item.get("source").unwrap()),
                        },
                        Field {
                            name: "authors",
                            value: FieldRef::SelectedTexts(&names),
                        },
                    ],
                    &budget,
                    None,
                )
                .await
                .unwrap()
            } else {
                let item = &source
                    .get("resultList")
                    .unwrap()
                    .get("result")
                    .unwrap()
                    .as_array()
                    .unwrap()[0];
                publication_budgeted(
                    engine,
                    &[
                        Field {
                            name: "publication_id",
                            value: FieldRef::Value(item.get("id").unwrap()),
                        },
                        Field {
                            name: "source",
                            value: FieldRef::Value(item.get("source").unwrap()),
                        },
                        Field {
                            name: "pmid",
                            value: FieldRef::Value(item.get("id").unwrap()),
                        },
                        Field {
                            name: "doi",
                            value: FieldRef::Value(item.get("doi").unwrap()),
                        },
                    ],
                    &budget,
                    None,
                )
                .await
                .unwrap()
            };
            assert_eq!(
                output.value(),
                &record["output"]["results"][0]["payload"],
                "{}",
                record["id"]
            );
            assert_eq!(
                ssx_json::to_string(output.value()).unwrap(),
                ssx_json::to_string(&record["output"]["results"][0]["payload"]).unwrap()
            );
            drop(output);
            assert_eq!(budget.snapshot().unwrap().used, 0);
            checked.push(record["id"].as_str().unwrap());
        }
        assert_eq!(
            checked,
            [
                "pm-raw-author-after-display",
                "ep-raw-payload-only-surrogate"
            ]
        );
        let zero = Value::integer(0);
        let null = Value::Null;
        let no = Value::Bool(false);
        let budget = Budget::new(1024 * 1024).unwrap();
        let out = publication_budgeted(
            "test",
            &[
                Field {
                    name: "x",
                    value: FieldRef::Value(&zero),
                },
                Field {
                    name: "y",
                    value: FieldRef::Value(&no),
                },
                Field {
                    name: "x",
                    value: FieldRef::Value(&null),
                },
            ],
            &budget,
            None,
        )
        .await
        .unwrap();
        assert_eq!(
            ssx_json::to_string(&out.value()["data"]).unwrap(),
            "{\"y\":false}"
        );
        assert_eq!(
            ssx_json::to_string(&out.value()["provenance"]["adapter_fields"]).unwrap(),
            "[\"y\"]"
        );
        drop(out);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[tokio::test]
    async fn budgeted_numeric_projection_preserves_tokens_and_bounds() {
        use ssx_owned_budget::Budget;
        use std::time::{Duration, Instant};
        for (value, expected) in [
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (1e-5, "1e-05"),
            (1e-4, "0.0001"),
            (1e15, "1000000000000000.0"),
            (1e16, "1e+16"),
            (f64::from_bits(1), "5e-324"),
            (f64::MAX, "1.7976931348623157e+308"),
        ] {
            let source = Value::Number(Number::Float(value));
            let budget = Budget::new(1024 * 1024).unwrap();
            let output =
                project_field_budgeted(&source, &budget, Instant::now() + Duration::from_secs(5))
                    .await
                    .unwrap();
            let ssx_json::Value::Number(number) = output.value() else {
                panic!("number")
            };
            assert_eq!(number.as_str(), expected);
            assert_eq!(output.charged_bytes(), 25);
            drop(output);
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let budget = Budget::new(1024 * 1024).unwrap();
            assert!(matches!(
                project_field_budgeted(
                    &Value::Number(Number::Float(value)),
                    &budget,
                    Instant::now() + Duration::from_secs(5)
                )
                .await,
                Err(BudgetError::Projection(ProjectionError::Nonfinite))
            ));
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
        let raw = "9".repeat(super::super::source_json::MAX_INTEGER_DIGITS);
        let source = super::super::source_json::from_slice(
            raw.as_bytes(),
            Some(Instant::now() + Duration::from_secs(5)),
        )
        .await
        .unwrap();
        let budget = Budget::new(1024 * 1024).unwrap();
        let output =
            project_field_budgeted(&source, &budget, Instant::now() + Duration::from_secs(5))
                .await
                .unwrap();
        let ssx_json::Value::Number(number) = output.value() else {
            panic!("integer")
        };
        assert_eq!(number.as_str(), raw);
        assert_eq!(output.charged_bytes(), 2 * raw.len());
        drop(output);
        assert_eq!(budget.snapshot().unwrap().used, 0);
        let too_long = "9".repeat(super::super::source_json::MAX_INTEGER_DIGITS + 1);
        assert!(
            super::super::source_json::from_slice(
                too_long.as_bytes(),
                Some(Instant::now() + Duration::from_secs(5))
            )
            .await
            .is_err()
        );
    }
    #[tokio::test]
    async fn maximal_key_hash_and_collision_latency_on_actual_executor() {
        use ssx_owned_budget::Budget;
        use std::{
            cell::Cell,
            time::{Duration, Instant},
        };
        for collision in [false, true] {
            let mut fields = Map::new();
            let key = vec![
                if collision { 0xd800 } else { 0x10ffff };
                super::super::source_json::MAX_TEXT
            ];
            fields.insert(Text::from_points(key), Value::Null);
            let mut key = vec![
                if collision { 0xdfff } else { 0x10ffff };
                super::super::source_json::MAX_TEXT
            ];
            if !collision {
                *key.last_mut().unwrap() = 0x10fffe;
            }
            fields.insert(Text::from_points(key), Value::Bool(true));
            let input = Value::Object(fields);
            let budget = Budget::new(64 * 1024 * 1024).unwrap();
            let previous = Cell::new(Instant::now());
            let gap = Cell::new(Duration::ZERO);
            let monitor = async {
                loop {
                    tokio::task::yield_now().await;
                    let now = Instant::now();
                    gap.set(gap.get().max(now.duration_since(previous.replace(now))));
                }
            };
            let output = tokio::select! {
                result=project_field_budgeted(&input,&budget,Instant::now()+Duration::from_secs(30))=>result.unwrap(),
                _=monitor=>unreachable!(),
            };
            assert_eq!(
                output.value().as_object().unwrap().len(),
                if collision { 1 } else { 2 }
            );
            eprintln!(
                "maximal key codepoints={} projected_bytes_per_key={} collision={} observed_executor_gap_us={}",
                super::super::source_json::MAX_TEXT,
                if collision {
                    3 * super::super::source_json::MAX_TEXT
                } else {
                    4 * super::super::source_json::MAX_TEXT
                },
                collision,
                gap.get().as_micros()
            );
            drop(output);
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
        let over = Value::String(Text::from_points(vec![
            0xd800;
            super::super::source_json::MAX_TEXT
                + 1
        ]));
        let budget = Budget::new(64 * 1024 * 1024).unwrap();
        assert!(matches!(
            project_field_budgeted(&over, &budget, Instant::now() + Duration::from_secs(5)).await,
            Err(BudgetError::Resource)
        ));
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[tokio::test]
    async fn depth_sentinel_reservation_precedes_allocation_and_rolls_back() {
        use ssx_owned_budget::Budget;
        use std::time::{Duration, Instant};
        let value = Value::Null;
        let large = Budget::new(1024 * 1024).unwrap();
        let mut state = Projection {
            budget: &large,
            charge: large.reserve(0).unwrap(),
            deadline: Some(Instant::now() + Duration::from_secs(5)),
        };
        let out = visit(&value, 101, &mut state).unwrap().await.unwrap();
        assert_eq!(out, ssx_json::Value::String("<max depth exceeded>".into()));
        let peak = large.snapshot().unwrap().peak;
        drop(out);
        drop(state);
        assert_eq!(large.snapshot().unwrap().used, 0);
        let scarce = Budget::new(peak - 1).unwrap();
        {
            let mut state = Projection {
                budget: &scarce,
                charge: scarce.reserve(0).unwrap(),
                deadline: Some(Instant::now() + Duration::from_secs(5)),
            };
            assert!(matches!(
                visit(&value, 101, &mut state).unwrap().await,
                Err(BudgetError::Resource)
            ));
        }
        assert_eq!(scarce.snapshot().unwrap().used, 0);
    }
    #[tokio::test]
    async fn budgeted_projection_cancellation_releases_actual_output_allocations() {
        use ssx_owned_budget::Budget;
        use std::time::{Duration, Instant};
        let value = Value::Array(
            (0..100_000)
                .map(|_| Value::String(Text::from("x")))
                .collect(),
        );
        let budget = Budget::new(32 * 1024 * 1024).unwrap();
        {
            let job =
                project_field_budgeted(&value, &budget, Instant::now() + Duration::from_secs(10));
            tokio::pin!(job);
            tokio::select! {
                result=&mut job=>panic!("must observe allocation before completion: {}",result.is_ok()),
                _=async { loop { if budget.snapshot().unwrap().used>100_000*std::mem::size_of::<ssx_json::Value>() { break; } tokio::task::yield_now().await; } }=>{}
            }
        }
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[test]
    fn ordinary_projection_and_nonfinite_refusal_are_unchanged() {
        let raw = Value::String(Text::from_points(vec![0xd800]));
        assert_eq!(raw.project(), Err(ProjectionError::Unicode));
        assert_eq!(
            project_field(&raw),
            Ok(ssx_json::Value::String("\u{fffd}".into()))
        );
        for number in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                project_field(&Value::Number(Number::Float(number))),
                Err(Error::Projection(ProjectionError::Nonfinite))
            );
        }
        assert_eq!(
            project_field(&Value::Number(Number::Float(1.5)))
                .unwrap()
                .as_f64(),
            Some(1.5)
        );
    }
}

#[cfg(test)]
mod scalar_borrow_tests {
    use super::*;
    use std::time::{Duration, Instant};
    #[tokio::test]
    async fn scalar_fields_match_source_text_without_intermediate_source_copies() {
        let budget = ssx_owned_budget::Budget::new(1024 * 1024).unwrap();
        let text = Text::from("A\n🦑");
        let empty = Text::from("");
        let author = Text::from("Ada");
        let texts = [&text, &empty, &author];
        let scalars = ["A\n🦑", "", "Ada"];
        let old = [
            Field {
                name: "title",
                value: FieldRef::Text(&text),
            },
            Field {
                name: "authors",
                value: FieldRef::SelectedTexts(&texts),
            },
            Field {
                name: "empty",
                value: FieldRef::Text(&empty),
            },
        ];
        let new = [
            Field {
                name: "title",
                value: FieldRef::Scalar("A\n🦑"),
            },
            Field {
                name: "authors",
                value: FieldRef::SelectedScalars(&scalars),
            },
            Field {
                name: "empty",
                value: FieldRef::Scalar(""),
            },
        ];
        let a = publication_budgeted("arxiv", &old, &budget, None)
            .await
            .unwrap();
        let b = publication_budgeted("arxiv", &new, &budget, None)
            .await
            .unwrap();
        assert_eq!(a.value(), b.value());
        assert_eq!(a.charged_bytes(), b.charged_bytes());
        drop((a, b));
        assert_eq!(budget.snapshot().unwrap().used, 0);
        let scarce = ssx_owned_budget::Budget::new(1).unwrap();
        assert!(matches!(
            publication_budgeted("arxiv", &new, &scarce, None).await,
            Err(BudgetError::Resource)
        ));
        assert_eq!(scarce.snapshot().unwrap().used, 0);
    }
    #[tokio::test(flavor = "current_thread")]
    async fn scalar_copy_cancellation_releases_observed_live_reservation() {
        let text = "x".repeat(4 * 1024 * 1024);
        let budget = ssx_owned_budget::Budget::new(32 * 1024 * 1024).unwrap();
        let fields = [Field {
            name: "abstract",
            value: FieldRef::Scalar(&text),
        }];
        {
            let job = publication_budgeted(
                "arxiv",
                &fields,
                &budget,
                Some(Instant::now() + Duration::from_secs(30)),
            );
            tokio::pin!(job);
            tokio::select! {
                result=&mut job=>panic!("copy completed before observed allocation: {}",result.is_ok()),
                _=async{loop{if budget.snapshot().unwrap().used>=text.len(){break}tokio::task::yield_now().await;}}=>{}
            }
        }
        assert_eq!(budget.snapshot().unwrap().used, 0);
        assert!(matches!(
            publication_budgeted(
                "arxiv",
                &fields,
                &budget,
                Some(Instant::now() - Duration::from_secs(1))
            )
            .await,
            Err(BudgetError::Deadline)
        ));
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[tokio::test(flavor = "current_thread")]
    async fn both_empty_author_paths_yield_after_array_charge_and_observe_deadline() {
        let empty = Text::from("");
        let scalar_authors = vec![""; 65536];
        let text_authors = vec![&empty; 65536];
        for value in [
            FieldRef::SelectedScalars(&scalar_authors),
            FieldRef::SelectedTexts(&text_authors),
        ] {
            for expire in [false, true] {
                let budget = ssx_owned_budget::Budget::new(32 * 1024 * 1024).unwrap();
                let fields = [Field {
                    name: "authors",
                    value,
                }];
                let array_bytes = 65536 * std::mem::size_of::<ssx_json::Value>();
                let deadline = Instant::now() + Duration::from_secs(2);
                {
                    let job = publication_budgeted("arxiv", &fields, &budget, Some(deadline));
                    tokio::pin!(job);
                    tokio::select! {
                        result=&mut job=>panic!("empty author copy completed before array charge observation: {}",result.is_ok()),
                        _=async{loop{if budget.snapshot().unwrap().used>=array_bytes{break}tokio::task::yield_now().await;}}=>{}
                    }
                    // More Pending boundaries than all following envelope
                    // metadata combined proves we remain inside the array-copy
                    // phase rather than merely observing its retained charge.
                    for poll in 0..256 {
                        std::future::poll_fn(|cx| {
                            assert!(
                                matches!(
                                    std::future::Future::poll(job.as_mut(), cx),
                                    std::task::Poll::Pending
                                ),
                                "array copy completed at repeated poll {poll}"
                            );
                            assert!(budget.snapshot().unwrap().used >= array_bytes);
                            std::task::Poll::Ready(())
                        })
                        .await;
                    }
                    if expire {
                        // Freeze this single-thread test executor only after
                        // observing real array reservation. std::Instant is the
                        // production clock; no Tokio pause or sleep assumption.
                        if let Some(wait) = deadline.checked_duration_since(Instant::now()) {
                            std::thread::sleep(wait + Duration::from_millis(1));
                        }
                        assert!(matches!(job.await, Err(BudgetError::Deadline)));
                    }
                }
                assert_eq!(budget.snapshot().unwrap().used, 0);
            }
        }
    }
}

#[cfg(test)]
mod selected_values_controls {
    use super::*;
    use base64::Engine as _;
    use ssx_owned_budget::{Budget, OwnedVec};
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
        time::{Duration, Instant},
    };
    #[tokio::test]
    async fn original_adapter_post_first3_arbitrary_author_values_preserve_payload() {
        let corpus: ssx_json::Value = ssx_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a06/source-selected-values.json"
        ))
        .unwrap();
        let mut ids = std::collections::BTreeSet::new();
        for row in corpus["records"].as_array().unwrap() {
            assert!(ids.insert(row["id"].as_str().unwrap()));
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(row["responses"][0]["body_b64"].as_str().unwrap())
                .unwrap();
            let budget = Budget::new(1024 * 1024).unwrap();
            let input = budget.reserve(bytes.capacity()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(30);
            let source =
                super::super::source_json::from_slice_budgeted(&bytes, Some(deadline), &budget)
                    .await
                    .unwrap();
            let authors = source.value().get("data").unwrap().as_array().unwrap()[0]
                .get("authors")
                .unwrap()
                .as_array()
                .unwrap();
            let empty = Value::String(Text::from(""));
            let mut names = OwnedVec::with_capacity(&budget, authors.len()).unwrap();
            for author in authors {
                if let Some(author) = author.as_object() {
                    names
                        .try_push(author.get("name").unwrap_or(&empty))
                        .unwrap();
                }
            }
            let fields = [Field {
                name: "authors",
                value: FieldRef::SelectedValues(names.as_slice()),
            }];
            let payload = publication_budgeted("semanticscholar", &fields, &budget, Some(deadline))
                .await
                .unwrap();
            assert_eq!(
                payload.value(),
                &row["output"]["results"][0]["payload"],
                "{}",
                row["id"]
            );
            drop(payload);
            drop(names);
            drop(source);
            drop(input);
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
        assert_eq!(ids.len(), 10);
    }
    #[tokio::test]
    async fn charged_empty_selected_values_yield_then_cancel_or_expire_and_release() {
        let empty = Value::String(Text::from(""));
        for expire in [false, true] {
            let budget = Budget::new(32 * 1024 * 1024).unwrap();
            let mut values = OwnedVec::with_capacity(&budget, 65536).unwrap();
            for _ in 0..65536 {
                values.try_push(&empty).unwrap();
            }
            let descriptors = values.charged_bytes();
            let array = 65536 * std::mem::size_of::<ssx_json::Value>();
            let fields = [Field {
                name: "authors",
                value: FieldRef::SelectedValues(values.as_slice()),
            }];
            let deadline = Instant::now() + Duration::from_secs(if expire { 2 } else { 30 });
            let mut future = Box::pin(publication_budgeted(
                "semanticscholar",
                &fields,
                &budget,
                Some(deadline),
            ));
            let mut context = Context::from_waker(Waker::noop());
            let mut observed = 0;
            for _ in 0..100000 {
                assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
                if budget.snapshot().unwrap().used >= descriptors + array {
                    observed += 1;
                    if observed == 256 {
                        break;
                    }
                }
            }
            assert_eq!(observed, 256);
            if expire {
                tokio::time::sleep_until(deadline.into()).await;
                assert!(matches!(future.await, Err(BudgetError::Deadline)));
            } else {
                drop(future);
            }
            assert_eq!(budget.snapshot().unwrap().used, descriptors);
            drop(values);
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
        let budget = Budget::new(1024 * 1024).unwrap();
        let mut values = OwnedVec::with_capacity(&budget, 65536).unwrap();
        for _ in 0..65536 {
            values.try_push(&empty).unwrap();
        }
        let descriptors = values.charged_bytes();
        let fields = [Field {
            name: "authors",
            value: FieldRef::SelectedValues(values.as_slice()),
        }];
        assert!(matches!(
            publication_budgeted(
                "semanticscholar",
                &fields,
                &budget,
                Some(Instant::now() + Duration::from_secs(30))
            )
            .await,
            Err(BudgetError::Resource)
        ));
        assert_eq!(budget.snapshot().unwrap().used, descriptors);
        drop(values);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
}

#[cfg(test)]
mod generic_envelope_tests {
    use super::*;
    #[tokio::test]
    async fn generic_science_wrapper_keeps_bytes_charges_and_declared_envelopes() {
        let value = Value::String(Text::from("full source field"));
        let fields = [Field {
            name: "notes",
            value: FieldRef::Value(&value),
        }];
        let budget = ssx_owned_budget::Budget::new(1 << 20).unwrap();
        let publication = publication_budgeted("engine", &fields, &budget, None)
            .await
            .unwrap();
        let generic = envelope_budgeted("science", "publication", "engine", &fields, &budget, None)
            .await
            .unwrap();
        assert_eq!(publication.value(), generic.value());
        assert_eq!(publication.charged_bytes(), generic.charged_bytes());
        for (domain, kind) in [
            ("financial", "economic_series"),
            ("biomedical", "drug_label"),
        ] {
            let payload = envelope_budgeted(domain, kind, "engine", &fields, &budget, None)
                .await
                .unwrap();
            assert_eq!(payload.value()["domain"], domain);
            assert_eq!(payload.value()["type"], kind);
            assert_eq!(payload.value()["data"]["notes"], "full source field");
            assert_eq!(
                payload.value()["provenance"]["adapter_fields"],
                ssx_json::json!(["notes"])
            );
        }
    }
}
