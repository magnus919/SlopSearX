//! Native SEC EDGAR source operations. Public links retain provider byte identity.
use super::{
    clinicaltrials as c,
    source_json::{Text, Value},
};
use crate::http::AdapterHttp;
use ssx_application::{PortFuture, SearchAdmission, SearchContext, SearchEngine};
use ssx_domain::{AdapterResponse, SearchRequest};
use std::{sync::Arc, time::Instant};
pub const ENDPOINT: &str = "https://efts.sec.gov/LATEST/search-index";
pub type EdgarConfig = c::BiomedicalConfig;
pub struct EdgarAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: EdgarConfig,
}
impl EdgarAdapter {
    pub fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: EdgarConfig,
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
        let result = self.execute(q, context, &budget, &mut diagnostic).await;
        match result {
            Ok(batch) => match batch
                .into_adapter_response(super::source_payload::Outcome {
                    status: ssx_domain::EngineStatus::Ok,
                    error_message: diagnostic.as_deref(),
                    latency_ms: start.elapsed().as_secs_f64() * 1000.,
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
        c::admit_with_context(&self.admission, "edgar", context).await?;
        let deadline = c::network_deadline(&self.config, false, context)?;
        let max = c::scalar_text(&self.config.max_results);
        let mut req = c::request(
            &self.config.base_url,
            &[("q", &q.query), ("limit", &max), ("offset", "0")],
            false,
            false,
        )?;
        req.headers.insert(
            "user-agent",
            "SlopSearX/0.1.0 (meta search engine; research)"
                .parse()
                .unwrap(),
        );
        req.headers.insert(
            "accept",
            "application/json, text/plain, */*".parse().unwrap(),
        );
        let data = super::regulatory::fetch(&self.http, req, deadline, false, context)
            .await?
            .unwrap();
        let map = c::map();
        let list = c::list();
        let hits = c::field(c::field(&data, "hits", &map)?, "hits", &list)?;
        let rows = c::result_rows(hits, &self.config.max_results)?;
        let mut batch =
            c::ProducedBatch::new(budget, rows.len(), context).map_err(c::projection_error)?;
        let mut unresolved = 0;
        for hit in rows {
            c::row_checkpoint(context).await?;
            let Some(src) = hit
                .as_object()
                .and_then(|h| h.get("_source").or(Some(&map)))
                .filter(|s| s.as_object().is_some())
            else {
                unresolved += 1;
                continue;
            };
            let empty = c::empty();
            let first;
            let default_names = Value::Array(vec![Value::Null]);
            let names = c::field(src, "display_names", &default_names)?;
            let name = if !c::truthy(names) {
                c::field(src, "entity_name", &empty)?
            } else {
                match names {
                    Value::Array(a) if a.is_empty() => c::field(src, "entity_name", &empty)?,
                    Value::Array(a) => &a[0],
                    Value::String(t) if t.is_empty() => c::field(src, "entity_name", &empty)?,
                    Value::String(t) => {
                        first = Value::String(Text::from_points(vec![t.codepoints()[0]]));
                        &first
                    }
                    _ => return Err(c::error("EDGAR display_names is not subscriptable")),
                }
            };
            let form = c::field(src, "form_type", &empty)?;
            let desc = c::field(src, "description", &empty)?;
            let date = c::field(src, "filed_at", &empty)?;
            let Some(url) = filing_url(hit, src) else {
                unresolved += 1;
                continue;
            };
            let form_text = if c::truthy(form) {
                Some(c::borrowed_text(form)?)
            } else {
                None
            };
            let name = if form_text.is_some() {
                c::owned_stringify(budget, name, context).await?
            } else {
                c::owned_join(budget, &[c::borrowed_text(name)?], "", None, context).await?
            };
            let title = if let Some(form) = form_text {
                c::owned_join(budget, &[name.as_str(), form], " — ", None, context).await?
            } else {
                name
            };
            let mut parts = Vec::new();
            if let Some(form) = form_text {
                parts.push(form)
            }
            if c::truthy(desc) {
                parts.push(c::borrowed_text(desc)?)
            }
            if parts.is_empty() {
                parts.push("SEC filing")
            }
            let content = c::owned_join(budget, &parts, " — ", Some(500), context).await?;
            let date = if c::truthy(date) {
                Some(
                    c::owned_prefix(
                        budget,
                        date.as_text()
                            .ok_or_else(|| c::error("EDGAR filed_at is not sliceable text"))?,
                        10,
                        context,
                    )
                    .await?,
                )
            } else {
                None
            };
            let score = hit
                .get("_score")
                .filter(|v| c::truthy(v))
                .map_or(Ok(0.), |v| match v {
                    Value::Number(_) => super::regulatory::raw_score(v),
                    _ => Err(c::error("EDGAR score must be numeric")),
                })?;
            batch
                .push(
                    c::Skeleton {
                        url: &url,
                        title: title.as_str(),
                        content: content.as_str(),
                        engine: "edgar",
                        category: "general",
                        score,
                        position: batch.results().len() as i64 + 1,
                        tier: 1,
                        published_date: date.as_ref().map(|s| s.as_str()),
                    },
                    None,
                )
                .await
                .map_err(c::projection_error)?;
        }
        if unresolved > 0 {
            *diagnostic = Some(format!(
                "{unresolved} SEC result(s) omitted because no validated SEC URL or filing identity was available"
            ));
        }
        Ok(batch)
    }
}
impl SearchEngine for EdgarAdapter {
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
fn strip(s: &str) -> &str {
    s.trim_matches(ssx_json::python_whitespace)
}
fn text(value: Option<&Value>) -> Option<&str> {
    value.and_then(Value::as_str)
}
fn ascii_segment(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
}
fn digits(s: &str, n: std::ops::RangeInclusive<usize>) -> bool {
    n.contains(&s.chars().count()) && s.chars().all(ssx_json::decimal_digit)
}
fn cik(value: &Value) -> Option<String> {
    let raw = match value {
        Value::String(t) => strip(t.as_str()?),
        Value::Number(n) if !n.is_f64() => n.integer_text()?,
        _ => return None,
    };
    if !digits(raw, 1..=10) {
        return None;
    }
    let number = raw.chars().try_fold(0u64, |n, c| {
        Some(n * 10 + u64::from(ssx_json::decimal_value(c)?))
    })?;
    (number != 0).then(|| number.to_string())
}
fn archive(value: &Value) -> Option<String> {
    let candidate = strip(value.as_str()?);
    let view = super::regulatory::url_view(candidate);
    let (scheme, rest) = view.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let (authority, path) = rest.split_once('/')?;
    let (host, port) = authority
        .split_once(':')
        .map_or((authority, None), |(h, p)| (h, Some(p)));
    if !host.eq_ignore_ascii_case("www.sec.gov")
        || authority.contains('@')
        || port.is_some_and(|p| {
            !p.is_empty() && (!p.bytes().all(|b| b.is_ascii_digit()) || p.parse::<u16>() != Ok(443))
        })
    {
        return None;
    }
    let (path, fragment) = path.split_once('#').map_or((path, ""), |(p, f)| (p, f));
    let (path, query) = path.split_once('?').map_or((path, ""), |(p, q)| (p, q));
    if !fragment.is_empty() || !query.is_empty() {
        return None;
    }
    let path = super::regulatory::path_without_params(path);
    let suffix = path.strip_prefix("Archives/edgar/data/")?;
    let parts: Vec<_> = suffix.split('/').collect();
    if parts.len() < 3
        || cik(&Value::String(Text::from(parts[0]))).is_none()
        || !digits(parts[1], 18..=18)
        || !parts.iter().all(|p| ascii_segment(p))
    {
        return None;
    }
    Some(candidate.into())
}
fn filing_url(hit: &Value, src: &Value) -> Option<String> {
    for record in [src, hit] {
        for field in ["filing_url", "file_url", "url", "link", "href"] {
            if let Some(url) = record.get(field).and_then(archive) {
                return Some(url);
            }
        }
    }
    let cik = ["cik", "ciks"].into_iter().find_map(|key| {
        src.get(key)
            .and_then(|v| match v {
                Value::Array(a) => a.first(),
                v => Some(v),
            })
            .and_then(cik)
    });
    let accession = ["accession_no", "adsh"].into_iter().find_map(|key| {
        let s = strip(text(src.get(key))?);
        let p: Vec<_> = s.split('-').collect();
        (p.len() == 3 && digits(p[0], 10..=10) && digits(p[1], 2..=2) && digits(p[2], 6..=6))
            .then_some(s)
    });
    match (cik, accession) {
        (Some(cik), Some(a)) => {
            let doc = [
                "primary_doc",
                "primaryDocument",
                "primary_document",
                "file_name",
            ]
            .into_iter()
            .find_map(|k| {
                let s = strip(text(src.get(k))?);
                ascii_segment(s).then_some(s)
            });
            Some(format!(
                "https://www.sec.gov/Archives/edgar/data/{cik}/{}/{}",
                a.replace('-', ""),
                doc.map(str::to_owned)
                    .unwrap_or_else(|| format!("{a}-index.html"))
            ))
        }
        (Some(cik), None)
            if !["form_type", "filed_at"]
                .iter()
                .any(|k| text(src.get(k)).is_some_and(|s| !strip(s).is_empty())) =>
        {
            Some(format!(
                "https://www.sec.gov/cgi-bin/browse-edgar?action=getcompany&CIK={cik}"
            ))
        }
        _ => None,
    }
}
