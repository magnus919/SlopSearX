//! NVD preserves first-entry-per-version display/payload selection and header-only auth.
use super::{
    clinicaltrials as c,
    cve::Cvss,
    source_json::Value,
    source_payload::{self, Field, FieldRef},
    vulnerability as v,
};
use ssx_domain::SearchRequest;
use std::time::Instant;
pub const ENDPOINT: &str = "https://services.nvd.nist.gov/rest/json/cves/2.0";
#[derive(Clone, Debug)]
pub struct NvdConfig {
    pub settings: v::Settings,
    pub api_key: ssx_config::Secret,
    /// Direct/raw override; never populated by F06 YAML/env.
    pub raw_max_results: Option<Value>,
}
impl Default for NvdConfig {
    fn default() -> Self {
        Self {
            settings: v::defaults(ENDPOINT),
            api_key: Default::default(),
            raw_max_results: None,
        }
    }
}
v::json_adapter!(NvdAdapter, NvdConfig);
impl NvdAdapter {
    async fn execute<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        b: &'b c::Budget,
    ) -> v::Result<v::Batch<'b>> {
        c::admit_with_context(&self.admission, "nvd", context).await?;
        let matches = v::cves(&q.query, 100, context, b).await?;
        let mut ids = c::OwnedVec::with_capacity(b, matches.len()).map_err(c::owned_error)?;
        for raw in matches.as_slice() {
            ids.try_push(v::canonical_cve(b, raw, context).await?)
                .map_err(c::owned_error)?;
        }
        let mut id_parts = c::OwnedVec::with_capacity(b, ids.len()).map_err(c::owned_error)?;
        for id in ids.as_slice() {
            id_parts.try_push(id.as_str()).map_err(c::owned_error)?;
        }
        let joined = v::join(b, id_parts.as_slice(), ",", context).await?;
        let limit = v::Max::new(&self.config.settings, &self.config.raw_max_results);
        let max = limit.nvd_wire(b, context).await?;
        let query = if matches.is_empty() {
            ("keywordSearch", q.query.as_str())
        } else {
            ("cveIds", joined.as_str())
        };
        let mut req = c::request(
            &self.config.settings.base_url,
            &[("resultsPerPage", max.as_str()), query],
            false,
            false,
        )?;
        if let Some(raw) = self.config.api_key.expose() {
            let key = v::trim(raw);
            if !key.is_empty() {
                req.headers.insert(
                    "apiKey",
                    key.parse()
                        .map_err(|_| v::error("NVD API key header invalid"))?,
                );
            }
        }
        let data = v::json(
            &self.http,
            req,
            c::network_deadline_for_http(&self.http, &self.config.settings, context)?,
            context,
            false,
        )
        .await?
        .expect("NVD has no404empty branch");
        let list = c::list();
        let rows = v::field(&data, "vulnerabilities", &list)?;
        if self.config.raw_max_results.is_none() {
            parse_vulnerabilities(rows, &self.config.settings.max_results, b, context).await
        } else {
            parse_with_limit(rows, limit, b, context).await
        }
    }
}
const VERSIONS: [(&str, &str); 4] = [
    ("cvssMetricV40", "4.0"),
    ("cvssMetricV31", "3.1"),
    ("cvssMetricV30", "3.0"),
    ("cvssMetricV2", "2.0"),
];
fn entry(value: &Value) -> v::Result<Option<&Value>> {
    if !c::truthy(value) {
        return Ok(None);
    }
    match value {
        Value::Array(v) => Ok(v.first().filter(|v| v.as_object().is_some())),
        Value::String(_) => Ok(None),
        Value::Object(_) => Err(v::error("0")),
        _ => Err(v::error(format!(
            "'{}' object is not subscriptable",
            v::kind(value)
        ))),
    }
}
fn cvss_data(entry: &Value) -> v::Result<&Value> {
    let raw = entry.get("cvssData");
    let data = raw.filter(|v| c::truthy(v)).unwrap_or(entry);
    v::object(data)?;
    Ok(data)
}
pub(super) async fn format_cvss(
    metrics: &Value,
    b: &c::Budget,
    context: Option<Instant>,
) -> v::Result<c::OwnedString> {
    v::object(metrics)?;
    for (key, _) in VERSIONS {
        c::row_checkpoint(context).await?;
        let Some(entry) = metrics.get(key).map(entry).transpose()?.flatten() else {
            continue;
        };
        let data = cvss_data(entry)?;
        let score = if let Some(score) = data
            .get("baseScore")
            .filter(|v| !v.is_null() && v.as_str() != Some(""))
        {
            let score = v::stringify(b, score, context).await?;
            Some(v::join(b, &["CVSS ", score.as_str()], "", context).await?)
        } else {
            None
        };
        let severity = if let Some(severity) = data.get("baseSeverity").filter(|v| c::truthy(v)) {
            let severity = v::stringify(b, severity, context).await?;
            Some(v::join(b, &["(", severity.as_str(), ")"], "", context).await?)
        } else {
            None
        };
        let vector = data
            .get("vectorString")
            .filter(|v| c::truthy(v))
            .map(v::text)
            .transpose()?;
        let mut parts = c::OwnedVec::with_capacity(b, 3).map_err(c::owned_error)?;
        if let Some(score) = &score {
            parts.try_push(score.as_str()).map_err(c::owned_error)?;
        }
        if let Some(severity) = &severity {
            parts.try_push(severity.as_str()).map_err(c::owned_error)?;
        }
        if let Some(vector) = vector {
            parts.try_push(vector).map_err(c::owned_error)?;
        }
        if !parts.is_empty() {
            return v::join(b, parts.as_slice(), " ", context).await;
        }
    }
    v::join(b, &[], "", context).await
}
pub(super) async fn cvss_payload<'v>(
    metrics: &'v Value,
    context: Option<Instant>,
) -> v::Result<Option<Cvss<'v>>> {
    v::object(metrics)?;
    for (key, version) in VERSIONS {
        c::row_checkpoint(context).await?;
        let Some(entry) = metrics.get(key).map(entry).transpose()?.flatten() else {
            continue;
        };
        let data = cvss_data(entry)?;
        let score = data.get("baseScore");
        let severity = data
            .get("baseSeverity")
            .filter(|v| c::truthy(v))
            .or_else(|| entry.get("baseSeverity"));
        let vector = data.get("vectorString");
        if score.is_some_and(|v| !v.is_null())
            || severity.is_some_and(c::truthy)
            || vector.is_some_and(c::truthy)
        {
            return Ok(Some(Cvss {
                version,
                key,
                score,
                severity,
                vector,
                raw: data,
            }));
        }
    }
    Ok(None)
}
struct Prepared<'v> {
    id: &'v Value,
    content: c::OwnedString,
    description: &'v Value,
    cvss: Option<Cvss<'v>>,
    cwes: c::OwnedVec<&'v Value>,
    references: c::OwnedVec<&'v Value>,
    date: Option<c::OwnedString>,
    url: c::OwnedString,
}
pub(super) async fn parse_vulnerabilities<'b>(
    rows: &Value,
    max_results: &ssx_config::ConfigScalar,
    b: &'b c::Budget,
    context: Option<Instant>,
) -> v::Result<v::Batch<'b>> {
    parse_with_limit(rows, v::Max::Resolved(max_results), b, context).await
}
async fn parse_with_limit<'b>(
    rows: &Value,
    limit: v::Max<'_>,
    b: &'b c::Budget,
    context: Option<Instant>,
) -> v::Result<v::Batch<'b>> {
    let rows = limit.rows(rows)?;
    let list = c::list();
    let map = c::map();
    let empty = c::empty();
    let null = Value::Null;
    let mut prepared = c::OwnedVec::with_capacity(b, rows.len()).map_err(c::owned_error)?;
    for row in rows {
        c::row_checkpoint(context).await?;
        let cve = v::field(row, "cve", &map)?;
        let id = v::field(cve, "id", &empty)?;
        let descriptions = v::field(cve, "descriptions", &list)?;
        let desc_rows = v::iterable(descriptions, false)?;
        let mut description = &empty;
        for desc in desc_rows {
            c::row_checkpoint(context).await?;
            if v::field(desc, "lang", &null)?.as_str() == Some("en") {
                description = v::field(desc, "value", &empty)?;
                break;
            }
        }
        if !c::truthy(description) {
            description = if let Some(first) = desc_rows.first() {
                v::field(first, "value", &empty)?
            } else {
                &empty
            };
        }
        let desc_prefix = v::prefix(b, description, 500, context).await?;
        let metrics = v::field(cve, "metrics", &map)?;
        let cvss_text = format_cvss(metrics, b, context).await?;
        let weaknesses = v::field(cve, "weaknesses", &list)?;
        let mut cwes = c::OwnedVec::with_capacity(b, v::iterable(weaknesses, false)?.len())
            .map_err(c::owned_error)?;
        for weakness in v::iterable(weaknesses, false)? {
            c::row_checkpoint(context).await?;
            let descriptions = v::field(weakness, "description", &list)?;
            for desc in v::iterable(descriptions, false)? {
                c::row_checkpoint(context).await?;
                let value = v::field(desc, "value", &empty)?;
                if c::truthy(value)
                    && !v::source_contains(cwes.as_slice(), value, b, context).await?
                {
                    if cwes.len() == cwes.capacity() {
                        cwes.try_reserve_exact(1).map_err(c::owned_error)?;
                    }
                    cwes.try_push(desc.get("value").expect("truthy source CWE"))
                        .map_err(c::owned_error)?;
                }
            }
        }
        let mut cwe_parts = c::OwnedVec::with_capacity(b, cwes.len()).map_err(c::owned_error)?;
        for cwe in cwes.as_slice() {
            cwe_parts.try_push(v::text(cwe)?).map_err(c::owned_error)?;
        }
        let cwe_text = v::join(b, cwe_parts.as_slice(), " | ", context).await?;
        let refs = v::field(cve, "references", &list)?;
        let refs = v::references(refs, b, false, context).await?;
        let ref_text = if refs.is_empty() {
            None
        } else {
            Some(v::refs_text(b, refs.as_slice(), context).await?)
        };
        let mut parts = c::OwnedVec::with_capacity(b, 4).map_err(c::owned_error)?;
        parts
            .try_push(desc_prefix.as_str())
            .map_err(c::owned_error)?;
        if !cvss_text.as_str().is_empty() {
            parts.try_push(cvss_text.as_str()).map_err(c::owned_error)?;
        }
        if !cwe_text.as_str().is_empty() {
            parts.try_push(cwe_text.as_str()).map_err(c::owned_error)?;
        }
        if let Some(ref_text) = &ref_text {
            parts.try_push(ref_text.as_str()).map_err(c::owned_error)?;
        }
        let content = v::join(b, parts.as_slice(), " | ", context).await?;
        let published = v::field(cve, "published", &null)?;
        let date = if c::truthy(published) {
            let raw = v::text(published)?;
            Some(v::join(b, &[raw.split_once('T').map_or(raw, |v| v.0)], "", context).await?)
        } else {
            None
        };
        let cvss = cvss_payload(metrics, context).await?;
        let id_text = v::stringify(b, id, context).await?;
        let url = v::join(
            b,
            &["https://nvd.nist.gov/vuln/detail/", id_text.as_str()],
            "",
            context,
        )
        .await?;
        prepared
            .try_push(Prepared {
                id,
                content,
                description,
                cvss,
                cwes,
                references: refs,
                date,
                url,
            })
            .map_err(c::owned_error)?;
    }
    // Public representability is checked only after every source row operation,
    // so an earlier non-string title cannot mask a later source parser error.
    let mut batch = v::Batch::new(b, prepared.len(), context)?;
    for (index, row) in prepared.as_slice().iter().enumerate() {
        c::row_checkpoint(context).await?;
        let cvss = row.cvss.as_ref().map(|s| s.fields(b)).transpose()?;
        let fields = [
            Field {
                name: "cve_id",
                value: FieldRef::Value(if c::truthy(row.id) { row.id } else { &null }),
            },
            Field {
                name: "description",
                value: FieldRef::Value(if c::truthy(row.description) {
                    row.description
                } else {
                    &null
                }),
            },
            Field {
                name: "cvss",
                value: cvss.as_ref().map_or(FieldRef::Value(&null), |s| {
                    FieldRef::SelectedObject(s.as_slice())
                }),
            },
            Field {
                name: "cwe_ids",
                value: if row.cwes.is_empty() {
                    FieldRef::Value(&null)
                } else {
                    FieldRef::SelectedValues(row.cwes.as_slice())
                },
            },
            Field {
                name: "references",
                value: if row.references.is_empty() {
                    FieldRef::Value(&null)
                } else {
                    FieldRef::SelectedValues(row.references.as_slice())
                },
            },
        ];
        let title = v::text(row.id)?;
        let payload = source_payload::envelope_budgeted(
            "security",
            "vulnerability",
            "nvd",
            &fields,
            b,
            context,
        )
        .await
        .map_err(c::projection_error)?;
        batch
            .rows
            .push(
                c::Skeleton {
                    url: row.url.as_str(),
                    title,
                    content: row.content.as_str(),
                    engine: "nvd",
                    category: "general",
                    score: 0.,
                    position: index as i64 + 1,
                    tier: 1,
                    published_date: row.date.as_ref().map(c::OwnedString::as_str),
                },
                Some(payload),
            )
            .await
            .map_err(c::projection_error)?;
    }
    Ok(batch)
}
