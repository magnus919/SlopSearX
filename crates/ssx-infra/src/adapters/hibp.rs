//! HIBP source adapter; shared policy remains the caller's acquisition gate.
use super::{
    clinicaltrials as c,
    source_json::{Number, Text, Value},
    threat_intel as t,
};
use ssx_application::{PortFuture, SearchAdmission, SearchContext, SearchEngine};
use ssx_domain::{AdapterResponse, EngineStatus, SearchRequest};
use ssx_registry::{EngineFactory, Registry, RegistryError};
use std::{sync::Arc, time::Instant};

pub const ENDPOINT: &str = "https://haveibeenpwned.com/api/v3";
pub type HibpConfig = t::ThreatIntelConfig;
pub struct HibpAdapter {
    http: Arc<crate::http::AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: HibpConfig,
}
pub fn register_hibp(
    registry: &mut Registry,
    http: Arc<crate::http::AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: HibpConfig,
) -> Result<(), RegistryError> {
    registry.register(
        "hibp",
        Arc::new(move || {
            Box::new(HibpAdapter::new(
                http.clone(),
                admission.clone(),
                config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )
}
impl HibpAdapter {
    pub fn new(
        http: Arc<crate::http::AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: HibpConfig,
    ) -> Self {
        Self {
            http,
            admission,
            config,
        }
    }
    async fn run(&self, q: &SearchRequest, context: Option<Instant>) -> AdapterResponse {
        let budget = t::Budget::new(c::ROW_BYTES).expect("existing positive native quota");
        let early = async {
            t::admit(&self.admission, "hibp", context).await?;
            if self.config.key().is_empty() {
                return Err(t::error(
                    "HIBP API key not configured (set ENGINE_HIBP_API_KEY)",
                ));
            }
            // Source account extraction precedes its timer. Its native work is
            // nevertheless caller-context cancellable and operation-charged.
            account(&q.query, &budget, context).await
        }
        .await;
        let account = match early {
            Ok(value) => value,
            Err(e) => {
                return AdapterResponse {
                    status: e.status,
                    error_message: e.message,
                    ..Default::default()
                };
            }
        };
        let start = Instant::now();
        t::finish(
            self.execute(account.as_str(), &budget, context).await,
            start,
        )
        .await
    }
    async fn execute<'b>(
        &self,
        account: &str,
        budget: &'b t::Budget,
        context: Option<Instant>,
    ) -> t::Result<c::ProducedBatch<'b>> {
        let deadline = self.config.network(context)?;
        let url = t::join(
            budget,
            &[&self.config.base_url, "/breachedaccount/", account],
            "",
            Some(deadline),
        )
        .await?;
        let mut req = crate::http::Request::get(url.as_str())
            .map_err(|_| t::error("invalid HIBP request URL"))?;
        let key = self.config.key();
        // HTTPX string header values use ASCII; Basic's UTF8 rule is different.
        if !key.is_ascii() {
            return Err(t::error("invalid HIBP credential configuration"));
        }
        let _header_charge = budget.reserve(key.len()).map_err(t::resource)?;
        let mut secret = reqwest::header::HeaderValue::from_str(key)
            .map_err(|_| t::error("invalid HIBP credential configuration"))?;
        secret.set_sensitive(true);
        req.headers.insert("hibp-api-key", secret);
        req.headers.insert(
            "user-agent",
            "SlopSearX/0.1.0".parse().expect("fixed header"),
        );
        req.headers.insert(
            "accept",
            "application/vnd.haveibeenpwned.v3+json"
                .parse()
                .expect("fixed header"),
        );
        let mut session = c::session(&self.http, deadline)?;
        let response = session.execute(req).await.map_err(|e| {
            if e == crate::http::Error::Deadline {
                t::classified(EngineStatus::Timeout, None)
            } else {
                t::error("HIBP request failed")
            }
        })?;
        match response.status {
            429 => return Err(t::classified(EngineStatus::RateLimited, None)),
            403 => return Err(t::classified(EngineStatus::Blocked, None)),
            404 => {
                return c::ProducedBatch::new(budget, 0, Some(deadline))
                    .map_err(c::projection_error)
                    .map_err(Into::into);
            }
            s if !(200..300).contains(&s) => {
                return Err(t::error(format!("HIBP returned HTTP {s}")));
            }
            _ => {}
        }
        let body = c::parse_body_observed(&response.body, Some(deadline), None).await?;
        project(&body, budget, Some(deadline)).await
    }
}
impl SearchEngine for HibpAdapter {
    fn search<'a>(&'a self, q: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.run(q, None))
    }
    fn search_with_context<'a>(
        &'a self,
        q: &'a SearchRequest,
        context: SearchContext,
    ) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.run(q, Some(context.deadline)))
    }
}
fn local(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"._%+-".contains(&byte)
}
fn domain(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b".-".contains(&byte)
}
async fn account(
    query: &str,
    budget: &t::Budget,
    deadline: Option<Instant>,
) -> t::Result<t::OwnedString> {
    let bytes = query.as_bytes();
    let mut previous_word = false;
    let mut work = 0usize;
    for (start, ch) in query.char_indices() {
        if work.is_multiple_of(1024) {
            t::checkpoint(deadline).await?;
        }
        work += 1;
        let current_word = ssx_json::python_text::word(ch);
        let boundary = previous_word != current_word;
        previous_word = current_word;
        if !ch.is_ascii() || !local(ch as u8) || !boundary {
            continue;
        }
        let mut at = start;
        while at < bytes.len() && local(bytes[at]) {
            if work.is_multiple_of(1024) {
                t::checkpoint(deadline).await?;
            }
            work += 1;
            at += 1;
        }
        if bytes.get(at) != Some(&b'@') {
            continue;
        }
        let first = at + 1;
        at = first;
        while at < bytes.len() && domain(bytes[at]) {
            if work.is_multiple_of(1024) {
                t::checkpoint(deadline).await?;
            }
            work += 1;
            at += 1;
        }
        // Greedy domain+, then its rightmost viable dot/TLD. TLD backtracking
        // cannot create a word boundary inside an ASCII alphabetic run.
        for dot in (first..at).rev() {
            if work.is_multiple_of(1024) {
                t::checkpoint(deadline).await?;
            }
            work += 1;
            if bytes[dot] != b'.' || dot == first {
                continue;
            }
            let mut end = dot + 1;
            while end < bytes.len() && bytes[end].is_ascii_alphabetic() {
                if work.is_multiple_of(1024) {
                    t::checkpoint(deadline).await?;
                }
                work += 1;
                end += 1;
            }
            if end - dot <= 2 {
                continue;
            }
            if query[end..]
                .chars()
                .next()
                .is_some_and(ssx_json::python_text::word)
            {
                continue;
            }
            return t::owned(budget, &query[start..end], deadline).await;
        }
    }
    let stripped = t::strip(query, deadline).await?;
    let mut output = t::owned(budget, "", deadline).await?;
    for (i, step) in ssx_json::python_text::lowercase_steps(stripped).enumerate() {
        if i % 1024 == 0 {
            t::checkpoint(deadline).await?;
        }
        if let ssx_json::python_text::LowerStep::Scalar(ch) = step {
            output.try_push(ch).map_err(t::resource)?;
        }
    }
    Ok(output)
}
fn field<'a>(v: &'a Value, key: &str, default: &'a Value) -> t::Result<&'a Value> {
    v.as_object()
        .map(|m| m.get(key).unwrap_or(default))
        .ok_or_else(|| t::error("HIBP response row does not support field access"))
}
enum Prefix<'a> {
    Text(&'a Text, usize),
    Array(&'a [Value]),
}
impl Prefix<'_> {
    fn truthy(&self) -> bool {
        match self {
            Self::Text(v, n) => !v.is_empty() && *n > 0,
            Self::Array(v) => !v.is_empty(),
        }
    }
    fn is_text(&self) -> bool {
        matches!(self, Self::Text(..))
    }
}
fn prefix(v: &Value, n: usize) -> t::Result<Prefix<'_>> {
    match v {
        Value::String(v) => Ok(Prefix::Text(v, n)),
        Value::Array(v) => Ok(Prefix::Array(&v[..v.len().min(n)])),
        _ => Err(t::error("HIBP source value does not support slicing")),
    }
}
fn source_len(v: &Value) -> t::Result<usize> {
    match v {
        Value::String(v) => Ok(v.len()),
        Value::Array(v) => Ok(v.len()),
        Value::Object(v) => Ok(v.len()),
        _ => Err(t::error("HIBP source value has no length")),
    }
}
fn count_format_valid(v: &Value) -> t::Result<()> {
    if matches!(v, Value::Bool(_) | Value::Number(_)) {
        Ok(())
    } else {
        Err(t::error(
            "HIBP account count does not support numeric grouping",
        ))
    }
}
fn count_float(v: &Value) -> t::Result<f64> {
    match v {
        Value::Bool(v) => Ok(if *v { 1. } else { 0. }),
        Value::Number(v) => v
            .as_f64()
            .ok_or_else(|| t::error("HIBP account count cannot become a float")),
        _ => Err(t::error("HIBP account count cannot become a float")),
    }
}
async fn grouped(
    v: &Value,
    budget: &t::Budget,
    deadline: Option<Instant>,
) -> t::Result<t::OwnedString> {
    if let Value::Number(Number::Float(v)) = v
        && !v.is_finite()
    {
        return t::owned(
            budget,
            if v.is_nan() {
                "nan"
            } else if v.is_sign_negative() {
                "-inf"
            } else {
                "inf"
            },
            deadline,
        )
        .await;
    }
    let typed = match v {
        Value::Bool(v) => ssx_json::Value::Bool(*v),
        Value::Number(v) => ssx_json::Value::Number(
            v.project()
                .map_err(|_| t::error("HIBP numeric formatting refused"))?,
        ),
        _ => return Err(t::error("HIBP numeric formatting refused")),
    };
    let formatted = super::crates_io::python_grouped(&typed)
        .map_err(|_| t::error("HIBP numeric formatting refused"))?;
    t::owned(budget, &formatted, deadline).await
}
async fn classes(
    v: &Prefix<'_>,
    budget: &t::Budget,
    deadline: Option<Instant>,
) -> t::Result<t::OwnedString> {
    let mut parts = t::OwnedVec::new(budget).map_err(t::resource)?;
    match v {
        Prefix::Array(values) => {
            for value in *values {
                parts
                    .try_push(t::owned(budget, t::text(value)?, deadline).await?)
                    .map_err(t::resource)?;
            }
        }
        Prefix::Text(text, n) => {
            for (i, point) in text.codepoints().iter().take(*n).enumerate() {
                if i % 1024 == 0 {
                    t::checkpoint(deadline).await?;
                }
                let ch = char::from_u32(*point).ok_or_else(|| {
                    t::error(
                        "source text contains non-scalar Unicode; requires reviewed projection",
                    )
                })?;
                let mut buf = [0; 4];
                parts
                    .try_push(t::owned(budget, ch.encode_utf8(&mut buf), deadline).await?)
                    .map_err(t::resource)?;
            }
        }
    }
    let mut refs = t::OwnedVec::with_capacity(budget, parts.len()).map_err(t::resource)?;
    for p in parts.as_slice() {
        refs.try_push(p.as_str()).map_err(t::resource)?;
    }
    t::join(budget, refs.as_slice(), ", ", deadline).await
}
async fn project<'b>(
    body: &Value,
    budget: &'b t::Budget,
    deadline: Option<Instant>,
) -> t::Result<c::ProducedBatch<'b>> {
    let rows = body.as_array().map_or(&[][..], Vec::as_slice);
    let mut batch =
        c::ProducedBatch::new(budget, rows.len(), deadline).map_err(c::projection_error)?;
    let empty = t::empty();
    let zero = Value::integer(0);
    let empty_array = Value::Array(Vec::new());
    for (i, row) in rows.iter().enumerate() {
        t::checkpoint(deadline).await?;
        let name = field(row, "Name", &empty)?;
        let domain = field(row, "Domain", &empty)?;
        let date = field(row, "BreachDate", &empty)?;
        let count = field(row, "PwnCount", &zero)?;
        let data = field(row, "DataClasses", &empty_array)?;
        let description = prefix(field(row, "Description", &empty)?, 300)?;
        // Validate actual source operations in order before typed rendering.
        if t::truthy(count) {
            count_format_valid(count)?;
        }
        let selected_classes = if t::truthy(data) {
            let selected = prefix(data, 5)?;
            if let Prefix::Array(values) = &selected
                && values.iter().any(|v| !matches!(v, Value::String(_)))
            {
                return Err(t::error("HIBP data class join requires source strings"));
            }
            Some(selected)
        } else {
            None
        };
        if description.truthy() && !description.is_text() {
            return Err(t::error("HIBP content join requires source strings"));
        }
        let score = if t::truthy(count) {
            count_float(count)? / 1_000_000.
        } else {
            0.
        };
        let published = source_len(date)? >= 10;
        let mut parts = t::OwnedVec::with_capacity(budget, 4).map_err(t::resource)?;
        if let Prefix::Text(text, n) = description {
            let value = c::owned_prefix(budget, text, n, deadline).await?;
            if !value.is_empty() {
                parts.try_push(value).map_err(t::resource)?;
            }
        }
        if t::truthy(domain) {
            let text = t::stringify(budget, domain, deadline).await?;
            parts
                .try_push(t::join(budget, &["Domain: ", text.as_str()], "", deadline).await?)
                .map_err(t::resource)?;
        }
        if t::truthy(count) {
            let text = grouped(count, budget, deadline).await?;
            parts
                .try_push(t::join(budget, &["Accounts: ", text.as_str()], "", deadline).await?)
                .map_err(t::resource)?;
        }
        if let Some(data) = &selected_classes {
            let text = classes(data, budget, deadline).await?;
            parts
                .try_push(t::join(budget, &["Data: ", text.as_str()], "", deadline).await?)
                .map_err(t::resource)?;
        }
        let mut refs = t::OwnedVec::with_capacity(budget, parts.len()).map_err(t::resource)?;
        for p in parts.as_slice() {
            refs.try_push(p.as_str()).map_err(t::resource)?;
        }
        let content = t::join(budget, refs.as_slice(), " | ", deadline).await?;
        let name_text = t::stringify(budget, name, deadline).await?;
        let url = t::join(
            budget,
            &["https://haveibeenpwned.com/breaches/", name_text.as_str()],
            "",
            deadline,
        )
        .await?;
        let title = t::text(name)?;
        let published_date = if published {
            Some(t::text(date)?)
        } else {
            None
        };
        t::push(
            &mut batch,
            "hibp",
            t::Fields {
                url: url.as_str(),
                title,
                content: content.as_str(),
                score,
                position: i + 1,
                date: published_date,
            },
        )
        .await?;
    }
    Ok(batch)
}

#[cfg(test)]
mod tests {
    use super::super::source_payload::{BATCH_PHASE, BatchPhase};
    use super::*;
    use std::time::Duration;
    #[tokio::test]
    async fn projection_drop_releases_backing_after_an_actual_stored_row() {
        let body = super::super::source_json::from_slice(
            br#"[{"Name":"one","Description":"first"},{"Name":"two","Description":"second"}]"#,
            None,
        )
        .await
        .unwrap();
        let budget = t::Budget::new(1024 * 1024).unwrap();
        let entered = Arc::new(tokio::sync::Notify::new());
        let signal = entered.clone();
        let observer: Arc<dyn Fn(BatchPhase) -> bool + Send + Sync> = Arc::new(move |phase| {
            if matches!(phase, BatchPhase::Stored(1)) {
                signal.notify_one();
                true
            } else {
                false
            }
        });
        BATCH_PHASE.scope(observer,async{let mut call=Box::pin(project(&body,&budget,None));tokio::time::timeout(Duration::from_secs(1),async{tokio::select!{_ =entered.notified()=>{},_ =&mut call=>panic!("must suspend after actual first stored row")}}).await.unwrap();assert!(budget.snapshot().unwrap().used>0);drop(call);assert_eq!(budget.snapshot().unwrap().used,0);}).await;
        println!("A13_REMAINING_UNIT_COMPLETE hibp-projection-stored-drop");
    }
    #[tokio::test]
    async fn late_failure_and_tiny_quota_leave_no_retained_partial_rows() {
        for (id, raw, limit) in [
            (
                "late-failure",
                br#"[{"Name":"one","Description":"first"},{"Name":"two","Description":false}]"#
                    .as_slice(),
                1024 * 1024,
            ),
            (
                "atomic-quota",
                br#"[{"Name":"one","Description":"first"},{"Name":"two","Description":"second"}]"#
                    .as_slice(),
                64,
            ),
        ] {
            let body = super::super::source_json::from_slice(raw, None)
                .await
                .unwrap();
            let budget = t::Budget::new(limit).unwrap();
            let Err(error) = project(&body, &budget, None).await else {
                panic!("late or capacity failure must refuse partial batch")
            };
            assert_eq!(error.status, EngineStatus::Error);
            assert_eq!(budget.snapshot().unwrap().used, 0);
            println!("A13_REMAINING_UNIT_COMPLETE hibp-{id}");
        }
    }
}
