//! FIRST EPSS keeps Python float coercion, first-entry semantics and exact thresholds.
use super::{clinicaltrials as c, source_json::Value, vulnerability as v};
use ssx_domain::SearchRequest;
use std::time::Instant;
pub const ENDPOINT: &str = "https://api.first.org/data/v1/epss";
pub type EpssConfig = v::Settings;
v::json_adapter!(EpssAdapter, EpssConfig);
impl EpssAdapter {
    async fn execute<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        b: &'b c::Budget,
    ) -> v::Result<v::Batch<'b>> {
        c::admit_with_context(&self.admission, "epss", context).await?;
        let matches = v::cves(&q.query, 1, context, b).await?;
        let Some(raw) = matches.as_slice().first() else {
            return v::Batch::new(b, 0, context);
        };
        let id = v::canonical_cve(b, raw, context).await?;
        let req = c::request(&self.config.base_url, &[("cve", id.as_str())], false, false)?;
        let data = v::json(
            &self.http,
            req,
            c::network_deadline_for_http(&self.http, &self.config, context)?,
            context,
            false,
        )
        .await?
        .expect("EPSS has no404empty branch");
        let list = c::list();
        let entries = v::field(&data, "data", &list)?;
        parse_epss(entries, id.as_str(), b, context).await
    }
}
pub(super) async fn parse_epss<'b>(
    entries: &Value,
    id: &str,
    b: &'b c::Budget,
    context: Option<Instant>,
) -> v::Result<v::Batch<'b>> {
    if !c::truthy(entries) {
        return v::Batch::new(b, 0, context);
    }
    let first = match entries {
        Value::Array(rows) => rows.first().expect("truthy array"),
        Value::Object(_) => return Err(v::error("0")),
        Value::String(_) => return Err(v::error("'str' object has no attribute 'get'")),
        _ => {
            return Err(v::error(format!(
                "'{}' object is not subscriptable",
                v::kind(entries)
            )));
        }
    };
    let zero = Value::Number(super::source_json::Number::Float(0.));
    let empty = c::empty();
    let score = super::regulatory::float(v::field(first, "epss", &zero)?)?;
    let percentile = super::regulatory::float(v::field(first, "percentile", &zero)?)?;
    let date = v::field(first, "date", &empty)?;
    let level = if score >= 0.9 {
        "Critical"
    } else if score >= 0.7 {
        "High"
    } else if score >= 0.4 {
        "Moderate"
    } else if score >= 0.1 {
        "Low"
    } else {
        "Very Low"
    };
    let date = date_text(date, b, context).await?;
    let score_text = if score.is_nan() {
        v::join(b, &["nan"], "", context).await?
    } else {
        c::fixed_format(b, 340, format_args!("{score:.5}"))?
    };
    let percentile_text = if percentile.is_nan() {
        v::join(b, &["nan"], "", context).await?
    } else {
        c::fixed_format(b, 340, format_args!("{percentile:.2}"))?
    };
    let title = v::join(b, &[id, " — EPSS ", score_text.as_str()], "", context).await?;
    let content = v::join(
        b,
        &[
            "EPSS Score: ",
            score_text.as_str(),
            " (",
            level,
            ") | Percentile: ",
            percentile_text.as_str(),
            " | Date: ",
            date.as_str(),
        ],
        "",
        context,
    )
    .await?;
    let url = v::join(b, &["https://www.first.org/epss/api?cve=", id], "", context).await?;
    let mut batch = v::Batch::new(b, 1, context)?;
    batch
        .rows
        .push(
            c::Skeleton {
                url: url.as_str(),
                title: title.as_str(),
                content: content.as_str(),
                engine: "epss",
                category: "general",
                score: score * 1000.,
                position: 1,
                tier: 1,
                published_date: None,
            },
            None,
        )
        .await
        .map_err(c::projection_error)?;
    Ok(batch)
}

async fn date_text(
    date: &Value,
    b: &c::Budget,
    context: Option<Instant>,
) -> v::Result<c::OwnedString> {
    if !c::truthy(date) {
        return v::join(b, &["N/A"], "", context).await;
    }
    if let Value::Array(values) = date {
        let mut output = c::OwnedString::new(b).map_err(c::owned_error)?;
        output.try_push('[').map_err(c::owned_error)?;
        for (n, value) in values.iter().take(10).enumerate() {
            c::row_checkpoint(context).await?;
            if n != 0 {
                output.try_push_str(", ").map_err(c::owned_error)?;
            }
            let value = super::source_repr::repr_budgeted(value, b, context)
                .await
                .map_err(|e| match e {
                    super::source_repr::Error::Deadline => super::catalog_media::Failure {
                        status: ssx_domain::EngineStatus::Timeout,
                        message: None,
                    },
                    _ => v::error("EPSS date repr refused"),
                })?;
            output
                .try_push_str(value.as_str())
                .map_err(c::owned_error)?;
        }
        output.try_push(']').map_err(c::owned_error)?;
        Ok(output)
    } else {
        v::prefix(b, date, 10, context).await
    }
}
