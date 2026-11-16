//! Native FDA exact-name search, one empty-result fallback and stable local rank.
use super::{
    clinicaltrials as c,
    source_json::{Text, Value},
    source_payload::{self, Field, FieldRef},
};
use crate::http::AdapterHttp;
use ssx_application::{PortFuture, SearchAdmission, SearchContext, SearchEngine};
use ssx_domain::{AdapterResponse, EngineStatus, SearchRequest};
use std::{collections::BTreeSet, sync::Arc, time::Instant};
pub const ENDPOINT: &str = "https://api.fda.gov/drug/label.json";
pub type OpenFdaConfig = c::BiomedicalConfig;
const NAMES: [&str; 3] = ["brand_name", "generic_name", "substance_name"];
pub struct OpenFdaAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: OpenFdaConfig,
}
impl OpenFdaAdapter {
    pub fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: OpenFdaConfig,
    ) -> Self {
        Self {
            http,
            admission,
            config,
        }
    }
    async fn run(&self, q: &SearchRequest, context: Option<Instant>) -> AdapterResponse {
        let start = Instant::now();
        let budget = c::Budget::new(c::ROW_BYTES).expect("valid A08 quota");
        let mut diagnostic = None;
        match self.execute(q, context, &budget, &mut diagnostic).await {
            Ok(batch) => match batch
                .into_adapter_response(source_payload::Outcome {
                    status: EngineStatus::Ok,
                    error_message: diagnostic.as_deref(),
                    latency_ms: if q.query.trim_matches(ssx_json::python_whitespace).is_empty() {
                        0.
                    } else {
                        start.elapsed().as_secs_f64() * 1000.
                    },
                    synthetic: false,
                })
                .await
            {
                Ok(response) => response,
                Err(e) => c::outcome(Err(c::projection_error(e)), start),
            },
            Err(e) => c::outcome(Err(e), start),
        }
    }
    async fn execute<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        budget: &'b c::Budget,
        diagnostic: &mut Option<String>,
    ) -> c::Result<c::ProducedBatch<'b>> {
        c::admit_with_context(&self.admission, "openfda", context).await?;
        let query = q.query.trim_matches(ssx_json::python_whitespace);
        if query.is_empty() {
            return c::ProducedBatch::new(budget, 0, context).map_err(c::projection_error);
        }
        let base = self
            .config
            .base_url
            .trim_matches(ssx_json::python_whitespace);
        let base = if base.is_empty() { ENDPOINT } else { base };
        let max = c::scalar_text(&self.config.max_results);
        let exact_expression = expression(query, true, budget, context).await?;
        let request = c::request(
            base,
            &[("search", exact_expression.as_str()), ("limit", &max)],
            false,
            false,
        )?;
        let mut data = super::regulatory::fetch(
            &self.http,
            request,
            c::network_deadline(&self.config, false, context)?,
            true,
            context,
        )
        .await?;
        let empty = c::list();
        let empty_map = c::map();
        let exact_items = items(data.as_ref(), &empty)?;
        let mut has_object = false;
        for (idx, item) in exact_items.iter().enumerate() {
            if idx % 1024 == 0 {
                c::row_checkpoint(context).await?
            }
            if item.as_object().is_some() {
                has_object = true;
                break;
            }
        }
        if !has_object {
            let broad = expression(query, false, budget, context).await?;
            let request = c::request(
                base,
                &[("search", broad.as_str()), ("limit", &max)],
                false,
                false,
            )?;
            data = super::regulatory::fetch(
                &self.http,
                request,
                c::network_deadline(&self.config, false, context)?,
                true,
                context,
            )
            .await?;
        }
        let items = items(data.as_ref(), &empty)?;
        let mut ranked = c::OwnedVec::with_capacity(budget, items.len()).map_err(c::owned_error)?;
        let mut seen = BTreeSet::new();
        let mut charges = c::OwnedVec::new(budget).map_err(c::owned_error)?;
        let mut omitted = 0;
        let target = ssx_json::casefold(query);
        for item in items {
            c::row_checkpoint(context).await?;
            if item.as_object().is_none() {
                continue;
            }
            if identity(item)?.is_none() {
                omitted += 1;
                continue;
            }
            if let Some(key) = key(item)? {
                if seen.contains(&key) {
                    continue;
                }
                charges
                    .try_push(budget.reserve(128).map_err(c::owned_error)?)
                    .map_err(c::owned_error)?;
                seen.insert(key);
            }
            let mut rank = 0;
            let harmonized = item
                .get("openfda")
                .filter(|v| v.as_object().is_some())
                .unwrap_or(&empty_map);
            for field in NAMES {
                if let Some(value) = harmonized.get(field) {
                    match value {
                        Value::Array(values) => {
                            for value in values {
                                c::row_checkpoint(context).await?;
                                if let Some(s) = first_scalar(value)? {
                                    rank = rank.max(match_rank(&target, s));
                                }
                            }
                        }
                        v => {
                            if let Some(s) = first_scalar(v)? {
                                rank = rank.max(match_rank(&target, s));
                            }
                        }
                    }
                }
            }
            ranked.try_push((rank, item)).map_err(c::owned_error)?;
        }
        let end = c::slice_end(&self.config.max_results, ranked.len())?;
        let mut batch = c::ProducedBatch::new(budget, end, context).map_err(c::projection_error)?;
        for rank in (0..=3).rev() {
            for (item_rank, item) in ranked.as_slice() {
                c::row_checkpoint(context).await?;
                if *item_rank != rank || batch.results().len() >= end {
                    continue;
                }
                let h = item
                    .get("openfda")
                    .filter(|v| v.as_object().is_some())
                    .unwrap_or(&empty_map);
                let brand = first(h.get("brand_name"))?;
                let generic = first(h.get("generic_name"))?;
                let manufacturer = first(h.get("manufacturer_name"))?;
                let substance = first(h.get("substance_name"))?;
                let purpose = first(item.get("purpose"))?;
                let indications = first_text(item.get("indications_and_usage"));
                let indications_charge = budget
                    .reserve(
                        indications.map_or(0, |t| t.len().saturating_mul(8).saturating_add(64)),
                    )
                    .map_err(c::owned_error)?;
                let indications = indications.map(trim_text);
                let display = if let Some(text) = &indications {
                    c::owned_prefix(budget, text, 120, context).await?
                } else {
                    c::owned_join(budget, &[], "", None, context).await?
                };
                let fallback =
                    c::owned_join(budget, &["FDA drug: ", query], "", None, context).await?;
                let title = brand.or(generic).or(substance).unwrap_or(fallback.as_str());
                let active = if let Some(s) = substance {
                    c::owned_join(budget, &["Active: ", s], "", None, context).await?
                } else {
                    c::owned_join(budget, &[], "", None, context).await?
                };
                let mut parts = Vec::new();
                if let Some(v) = manufacturer {
                    parts.push(v)
                }
                if substance.is_some() {
                    parts.push(active.as_str())
                }
                if let Some(v) = purpose {
                    parts.push(v)
                }
                if !display.as_str().is_empty() {
                    parts.push(display.as_str())
                }
                if parts.is_empty() {
                    parts.push("FDA drug labeling information")
                }
                let content = c::owned_join(budget, &parts, " — ", Some(500), context).await?;
                let (id_field, id) =
                    identity(item)?.ok_or_else(|| c::error("FDA identity disappeared"))?;
                let quoted = quote_term(id, budget, context).await?;
                let search = c::owned_join(
                    budget,
                    &[id_field, ".exact:", quoted.as_str()],
                    "",
                    None,
                    context,
                )
                .await?;
                let encoded = quote_plus(search.as_str(), budget, context).await?;
                let url = c::owned_join(
                    budget,
                    &[ENDPOINT, "?search=", encoded.as_str(), "&limit=1"],
                    "",
                    None,
                    context,
                )
                .await?;
                let set = first(item.get("set_id"))?;
                let spl_set = match first(item.get("spl_set_id"))? {
                    Some(v) => Some(v),
                    None => first(h.get("spl_set_id"))?,
                };
                let spl = match first(item.get("spl_id"))? {
                    Some(v) => Some(v),
                    None => first(h.get("spl_id"))?,
                };
                let id = first(item.get("id"))?;
                let null = Value::Null;
                let fields = [
                    ("brand_name", brand),
                    ("generic_name", generic),
                    ("manufacturer", manufacturer),
                    ("substance", substance),
                    ("purpose", purpose),
                    ("set_id", set),
                    ("spl_set_id", spl_set),
                    ("spl_id", spl),
                    ("id", id),
                ]
                .map(|(name, value)| Field {
                    name,
                    value: value.map_or(FieldRef::Value(&null), FieldRef::Scalar),
                });
                let fields = [
                    fields[0],
                    fields[1],
                    fields[2],
                    fields[3],
                    fields[4],
                    Field {
                        name: "indications",
                        value: indications
                            .as_ref()
                            .map_or(FieldRef::Value(&null), FieldRef::Text),
                    },
                    fields[5],
                    fields[6],
                    fields[7],
                    fields[8],
                ];
                let payload = source_payload::envelope_budgeted(
                    "biomedical",
                    "drug_label",
                    "openfda",
                    &fields,
                    budget,
                    context,
                )
                .await
                .map_err(c::projection_error)?;
                batch
                    .push(
                        c::Skeleton {
                            url: url.as_str(),
                            title,
                            content: content.as_str(),
                            engine: "openfda",
                            category: "general",
                            score: [0.25, 0.5, 0.75, 1.][rank],
                            position: batch.results().len() as i64 + 1,
                            tier: 1,
                            published_date: None,
                        },
                        Some(payload),
                    )
                    .await
                    .map_err(c::projection_error)?;
                drop(indications_charge);
            }
        }
        if omitted > 0 {
            *diagnostic = Some(format!(
                "omitted {omitted} openFDA label record(s) without a stable source identifier"
            ));
        }
        Ok(batch)
    }
}
impl SearchEngine for OpenFdaAdapter {
    fn search<'a>(&'a self, q: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.run(q, None))
    }
    fn search_with_context<'a>(
        &'a self,
        q: &'a SearchRequest,
        c: SearchContext,
    ) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.run(q, Some(c.deadline)))
    }
}
fn items<'a>(data: Option<&'a Value>, empty: &'a Value) -> c::Result<&'a [Value]> {
    let Some(data) = data else {
        return c::array(empty);
    };
    if data.as_object().is_none() {
        return Err(c::error("openFDA response must be a JSON object"));
    }
    c::field(data, "results", empty)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| c::error("openFDA results must be a JSON array"))
}
fn first_text(value: Option<&Value>) -> Option<&Text> {
    fn one(v: &Value) -> Option<&Text> {
        v.as_text().filter(|t| {
            t.codepoints()
                .iter()
                .any(|p| !char::from_u32(*p).is_some_and(ssx_json::python_whitespace))
        })
    }
    match value? {
        Value::Array(a) => a.iter().find_map(one),
        v => one(v),
    }
}
fn trim_text(t: &Text) -> Text {
    let p = t.codepoints();
    let whitespace = |p: &u32| char::from_u32(*p).is_some_and(ssx_json::python_whitespace);
    let start = p.iter().position(|p| !whitespace(p)).unwrap_or(p.len());
    let end = p
        .iter()
        .rposition(|p| !whitespace(p))
        .map_or(start, |n| n + 1);
    Text::from_points(p[start..end].to_vec())
}
fn first_scalar(value: &Value) -> c::Result<Option<&str>> {
    if let Some(t) = value.as_text() {
        let s = t
            .as_str()
            .ok_or_else(|| c::error("FDA text contains non-scalar Unicode"))?
            .trim_matches(ssx_json::python_whitespace);
        Ok((!s.is_empty()).then_some(s))
    } else {
        Ok(None)
    }
}
fn first(value: Option<&Value>) -> c::Result<Option<&str>> {
    let Some(value) = value else { return Ok(None) };
    match value {
        Value::Array(a) => {
            for value in a {
                if let Some(s) = first_scalar(value)? {
                    return Ok(Some(s));
                }
            }
            Ok(None)
        }
        v => first_scalar(v),
    }
}
fn identity(item: &Value) -> c::Result<Option<(&'static str, &str)>> {
    if let Some(v) = first(item.get("set_id"))? {
        return Ok(Some(("set_id", v)));
    }
    if let Some(h) = item.get("openfda").filter(|v| v.as_object().is_some()) {
        for (field, path) in [
            ("spl_set_id", "openfda.spl_set_id"),
            ("spl_id", "openfda.spl_id"),
        ] {
            if let Some(v) = first(h.get(field))? {
                return Ok(Some((path, v)));
            }
        }
    }
    Ok(first(item.get("id"))?.map(|v| ("id", v)))
}
fn key(item: &Value) -> c::Result<Option<(&'static str, &str)>> {
    for field in ["set_id", "spl_set_id", "spl_id", "id"] {
        if let Some(v) = first(item.get(field))? {
            return Ok(Some((field, v)));
        }
    }
    identity(item)
}

fn match_rank(target: &str, value: &str) -> usize {
    let value = ssx_json::casefold(value);
    if target == value {
        3
    } else if target.contains(&value) || value.contains(target) {
        2
    } else if target
        .split(ssx_json::python_whitespace)
        .filter(|s| !s.is_empty())
        .all(|t| value.split(ssx_json::python_whitespace).any(|v| v == t))
    {
        1
    } else {
        0
    }
}
async fn quote_term(
    text: &str,
    budget: &c::Budget,
    context: Option<Instant>,
) -> c::Result<c::OwnedString> {
    let mut out = c::OwnedString::new(budget).map_err(c::owned_error)?;
    out.try_reserve_exact(
        text.len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(2))
            .ok_or_else(c::resource)?,
    )
    .map_err(c::owned_error)?;
    out.try_push('"').map_err(c::owned_error)?;
    for (n, ch) in text.chars().enumerate() {
        if n % 1024 == 0 {
            c::row_checkpoint(context).await?
        }
        if matches!(ch, '\\' | '"') {
            out.try_push('\\').map_err(c::owned_error)?
        }
        out.try_push(ch).map_err(c::owned_error)?;
    }
    out.try_push('"').map_err(c::owned_error)?;
    Ok(out)
}
async fn quote_plus(
    text: &str,
    budget: &c::Budget,
    context: Option<Instant>,
) -> c::Result<c::OwnedString> {
    let mut out = c::OwnedString::new(budget).map_err(c::owned_error)?;
    out.try_reserve_exact(text.len().checked_mul(3).ok_or_else(c::resource)?)
        .map_err(c::owned_error)?;
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for (n, b) in text.bytes().enumerate() {
        if n % 1024 == 0 {
            c::row_checkpoint(context).await?
        }
        if b.is_ascii_alphanumeric() || b"_.-~".contains(&b) {
            out.try_push(char::from(b)).map_err(c::owned_error)?
        } else if b == b' ' {
            out.try_push('+').map_err(c::owned_error)?
        } else {
            for b in [b'%', HEX[(b >> 4) as usize], HEX[(b & 15) as usize]] {
                out.try_push(char::from(b)).map_err(c::owned_error)?
            }
        }
    }
    Ok(out)
}
async fn expression(
    query: &str,
    exact: bool,
    budget: &c::Budget,
    context: Option<Instant>,
) -> c::Result<c::OwnedString> {
    let variants = if exact {
        super::fda_case::variants(query, budget, context).await?
    } else {
        c::OwnedVec::new(budget).map_err(c::owned_error)?
    };
    let mut clauses = c::OwnedVec::new(budget).map_err(c::owned_error)?;
    for field in NAMES {
        if exact {
            for variant in variants.as_slice() {
                let quoted = quote_term(variant.as_str(), budget, context).await?;
                clauses
                    .try_push(
                        c::owned_join(
                            budget,
                            &["openfda.", field, ".exact:", quoted.as_str()],
                            "",
                            None,
                            context,
                        )
                        .await?,
                    )
                    .map_err(c::owned_error)?
            }
        } else {
            let quoted = quote_term(query, budget, context).await?;
            clauses
                .try_push(
                    c::owned_join(
                        budget,
                        &["openfda.", field, ":", quoted.as_str()],
                        "",
                        None,
                        context,
                    )
                    .await?,
                )
                .map_err(c::owned_error)?
        }
    }
    let mut refs = c::OwnedVec::with_capacity(budget, clauses.len()).map_err(c::owned_error)?;
    for c in clauses.as_slice() {
        refs.try_push(c.as_str()).map_err(c::owned_error)?
    }
    c::owned_join(budget, refs.as_slice(), " ", None, context).await
}
