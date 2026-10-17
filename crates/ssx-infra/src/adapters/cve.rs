//! CVE Program: one authoritative record, CNA/ADP fallback and all-entry CVSS priority.
use super::{
    clinicaltrials as c,
    source_json::Value,
    source_payload::{self, Field, FieldRef},
    vulnerability as v,
};
use ssx_domain::SearchRequest;
use std::time::Instant;
pub const ENDPOINT: &str = "https://cveawg.mitre.org/api";
pub type CveConfig = v::Settings;
v::json_adapter!(CveAdapter, CveConfig);
impl CveAdapter {
    async fn execute<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        b: &'b c::Budget,
    ) -> v::Result<v::Batch<'b>> {
        c::admit_with_context(&self.admission, "cve", context).await?;
        let matches = v::cves(&q.query, 1, context, b).await?;
        let Some(raw) = matches.as_slice().first() else {
            return v::Batch::new(b, 0, context);
        };
        let id = v::canonical_cve(b, raw, context).await?;
        let url = v::join(
            b,
            &[self.config.base_url.as_str(), "/cve/", id.as_str()],
            "",
            context,
        )
        .await?;
        let req = c::request(url.as_str(), &[], false, false)?;
        let data = v::json(
            &self.http,
            req,
            c::network_deadline_for_http(&self.http, &self.config, context)?,
            context,
            true,
        )
        .await?;
        match data {
            Some(data) => parse_record(&data, id.as_str(), b, context).await,
            None => v::Batch::new(b, 0, context),
        }
    }
}
pub struct Cvss<'v> {
    pub version: &'static str,
    pub key: &'static str,
    pub score: Option<&'v Value>,
    pub severity: Option<&'v Value>,
    pub vector: Option<&'v Value>,
    pub raw: &'v Value,
}
impl<'v> Cvss<'v> {
    pub(super) fn fields(
        &self,
        b: &c::Budget,
    ) -> v::Result<c::OwnedVec<(&'static str, &'v Value)>> {
        let mut fields = c::OwnedVec::with_capacity(b, 4).map_err(c::owned_error)?;
        if let Some(value) = self.score.filter(|v| !v.is_null()) {
            fields.try_push(("score", value)).map_err(c::owned_error)?;
        }
        if let Some(value) = self.severity.filter(|v| c::truthy(v)) {
            fields
                .try_push(("severity", value))
                .map_err(c::owned_error)?;
        }
        if let Some(value) = self.vector.filter(|v| c::truthy(v)) {
            fields.try_push(("vector", value)).map_err(c::owned_error)?;
        }
        fields
            .try_push(("version", v::version_value(self.version)))
            .map_err(c::owned_error)?;
        Ok(fields)
    }
}
pub(super) async fn best_cvss<'v>(
    container: &'v Value,
    context: Option<Instant>,
) -> v::Result<Option<Cvss<'v>>> {
    if !c::truthy(container) {
        return Ok(None);
    }
    let Some(metrics) = v::object(container)?.get("metrics") else {
        return Ok(None);
    };
    let mut best = None;
    let mut priority = usize::MAX;
    for entry in v::iterable(metrics, true)? {
        c::row_checkpoint(context).await?;
        if entry.as_object().is_none() {
            continue;
        }
        for (rank, (key, version)) in [
            ("cvssV4_0", "4.0"),
            ("cvssV3_1", "3.1"),
            ("cvssV3_0", "3.0"),
            ("cvssV2_0", "2.0"),
        ]
        .iter()
        .enumerate()
        {
            let Some(raw) = entry.get(key).filter(|v| v.as_object().is_some()) else {
                continue;
            };
            let score = raw.get("baseScore");
            let severity = raw.get("baseSeverity");
            let vector = raw.get("vectorString");
            if score.is_none_or(Value::is_null)
                && severity.is_none_or(|v| !c::truthy(v))
                && vector.is_none_or(|v| !c::truthy(v))
            {
                continue;
            }
            if rank < priority {
                priority = rank;
                best = Some(Cvss {
                    version,
                    key,
                    score,
                    severity,
                    vector,
                    raw,
                });
            }
        }
    }
    Ok(best)
}
pub(super) async fn description(
    container: &Value,
    b: &c::Budget,
    context: Option<Instant>,
) -> v::Result<c::OwnedString> {
    let list = c::list();
    let descriptions = v::field(container, "descriptions", &list)?;
    let descriptions = if c::truthy(descriptions) {
        descriptions
    } else {
        &list
    };
    let rows = v::iterable(descriptions, true)?;
    let empty = c::empty();
    for row in rows {
        c::row_checkpoint(context).await?;
        if row.as_object().is_some() && row.get("lang").and_then(Value::as_str) == Some("en") {
            return v::stringify(b, v::field(row, "value", &empty)?, context).await;
        }
    }
    for row in rows {
        c::row_checkpoint(context).await?;
        if row.as_object().is_some() {
            return v::stringify(b, v::field(row, "value", &empty)?, context).await;
        }
    }
    v::join(b, &[], "", context).await
}
pub(super) async fn metrics_text(
    container: &Value,
    b: &c::Budget,
    context: Option<Instant>,
) -> v::Result<c::OwnedString> {
    let Some(best) = best_cvss(container, context).await? else {
        return v::join(b, &[], "", context).await;
    };
    format_metrics(&best, b, context).await
}
async fn format_metrics(
    best: &Cvss<'_>,
    b: &c::Budget,
    context: Option<Instant>,
) -> v::Result<c::OwnedString> {
    let score = if let Some(score) = best
        .score
        .filter(|v| !v.is_null() && v.as_str() != Some(""))
    {
        let text = v::stringify(b, score, context).await?;
        Some(v::join(b, &["CVSS ", text.as_str()], "", context).await?)
    } else {
        None
    };
    let vector = best
        .vector
        .filter(|v| c::truthy(v))
        .map(v::text)
        .transpose()?;
    match (score, vector) {
        (Some(score), Some(vector)) => v::join(b, &[score.as_str(), vector], " | ", context).await,
        (Some(score), None) => Ok(score),
        (None, Some(vector)) => v::join(b, &[vector], "", context).await,
        _ => v::join(b, &[], "", context).await,
    }
}
pub(super) async fn parse_record<'b>(
    data: &Value,
    id: &str,
    b: &'b c::Budget,
    context: Option<Instant>,
) -> v::Result<v::Batch<'b>> {
    let map = c::map();
    let list = c::list();
    let null = Value::Null;
    let containers = v::field(data, "containers", &map)?;
    let cna = v::field(containers, "cna", &map)?;
    let mut desc = if c::truthy(cna) {
        description(cna, b, context).await?
    } else {
        v::join(b, &[], "", context).await?
    };
    if desc.as_str().is_empty() {
        let adp = v::field(containers, "adp", &list)?;
        let adp = if c::truthy(adp) { adp } else { &list };
        for container in v::iterable(adp, false)? {
            c::row_checkpoint(context).await?;
            desc = description(container, b, context).await?;
            if !desc.as_str().is_empty() {
                break;
            }
        }
    }
    let refs = if c::truthy(cna) {
        v::field(cna, "references", &list)?
    } else {
        &list
    };
    let refs = v::references(refs, b, true, context).await?;
    // Keep the same all-entry selection for display and payload.
    let selected = best_cvss(cna, context).await?;
    let metrics = metrics_text(cna, b, context).await?;
    let desc_prefix = c::owned_join(b, &[desc.as_str()], "", Some(500), context).await?;
    let ref_text = if refs.is_empty() {
        None
    } else {
        Some(v::refs_text(b, refs.as_slice(), context).await?)
    };
    let mut content = c::OwnedVec::with_capacity(b, 3).map_err(c::owned_error)?;
    content
        .try_push(if desc.as_str().is_empty() {
            "(No description available)"
        } else {
            desc_prefix.as_str()
        })
        .map_err(c::owned_error)?;
    if !metrics.as_str().is_empty() {
        content.try_push(metrics.as_str()).map_err(c::owned_error)?;
    }
    if let Some(refs) = &ref_text {
        content.try_push(refs.as_str()).map_err(c::owned_error)?;
    }
    let content = v::join(b, content.as_slice(), " | ", context).await?;
    let mut date = if c::truthy(cna) {
        cna.get("datePublic").unwrap_or(&null)
    } else {
        &null
    };
    if !c::truthy(date) && v::field(data, "cveId", &null)?.as_str() == Some(id) {
        let published = v::field(data, "datePublished", &null)?;
        date = if c::truthy(published) {
            published
        } else {
            v::field(data, "dateUpdated", &null)?
        };
    }
    let date = match date {
        Value::Null => None,
        Value::String(_) => Some(v::prefix(b, date, 10, context).await?),
        _ => {
            return Err(v::error(
                "source published_date requires reviewed string projection",
            ));
        }
    };
    let cvss = selected.as_ref().map(|s| s.fields(b)).transpose()?;
    let fields = [
        Field {
            name: "cve_id",
            value: if id.is_empty() {
                FieldRef::Value(&null)
            } else {
                FieldRef::Scalar(id)
            },
        },
        Field {
            name: "description",
            value: if desc.as_str().is_empty() {
                FieldRef::Value(&null)
            } else {
                FieldRef::Scalar(desc.as_str())
            },
        },
        Field {
            name: "cvss",
            value: cvss.as_ref().map_or(FieldRef::Value(&null), |cvss| {
                FieldRef::SelectedObject(cvss.as_slice())
            }),
        },
        Field {
            name: "references",
            value: if refs.is_empty() {
                FieldRef::Value(&null)
            } else {
                FieldRef::SelectedValues(refs.as_slice())
            },
        },
    ];
    let payload =
        source_payload::envelope_budgeted("security", "vulnerability", "cve", &fields, b, context)
            .await
            .map_err(c::projection_error)?;
    let url = v::join(b, &["https://nvd.nist.gov/vuln/detail/", id], "", context).await?;
    let mut batch = v::Batch::new(b, 1, context)?;
    batch
        .rows
        .push(
            c::Skeleton {
                url: url.as_str(),
                title: id,
                content: content.as_str(),
                engine: "cve",
                category: "general",
                score: 0.,
                position: 1,
                tier: 1,
                published_date: date.as_ref().map(c::OwnedString::as_str),
            },
            Some(payload),
        )
        .await
        .map_err(c::projection_error)?;
    Ok(batch)
}
