//! AbuseIPDB source reputation lookup, including all report category traversal.
use super::{
    clinicaltrials as c,
    source_json::{Text, Value},
    threat_intel as t, threat_ioc,
};
use ssx_domain::SearchRequest;
use std::{cmp::Ordering, time::Instant};
pub const ENDPOINT: &str = "https://api.abuseipdb.com/api/v2";
pub type AbuseIPDBConfig = t::ThreatIntelConfig;
t::adapter!(AbuseIPDBAdapter);
enum Category<'a> {
    Text(&'a Text),
    Point(u32),
    Formatted(t::OwnedString),
}
enum Points<'a> {
    Raw(std::slice::Iter<'a, u32>),
    Scalar(std::str::Chars<'a>),
    One(std::option::IntoIter<u32>),
}
impl Iterator for Points<'_> {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        match self {
            Self::Raw(i) => i.next().copied(),
            Self::Scalar(i) => i.next().map(u32::from),
            Self::One(i) => i.next(),
        }
    }
}
impl Category<'_> {
    fn points(&self) -> Points<'_> {
        match self {
            Self::Text(t) => Points::Raw(t.codepoints().iter()),
            Self::Point(p) => Points::One(Some(*p).into_iter()),
            Self::Formatted(s) => Points::Scalar(s.as_str().chars()),
        }
    }
}
async fn compare(a: &Category<'_>, b: &Category<'_>, d: Option<Instant>) -> t::Result<Ordering> {
    let (mut a, mut b) = (a.points(), b.points());
    let mut n = 0;
    loop {
        if n % 1024 == 0 {
            t::checkpoint(d).await?
        }
        n += 1;
        match (a.next(), b.next()) {
            (None, None) => return Ok(Ordering::Equal),
            (x, y) => {
                let order = x.cmp(&y);
                if order != Ordering::Equal {
                    return Ok(order);
                }
            }
        }
    }
}
async fn keep<'a>(
    top: &mut t::OwnedVec<Category<'a>>,
    value: Category<'a>,
    d: Option<Instant>,
) -> t::Result<()> {
    let mut at = top.len();
    for (i, old) in top.as_slice().iter().enumerate() {
        match compare(&value, old, d).await? {
            Ordering::Equal => return Ok(()),
            Ordering::Less => {
                at = i;
                break;
            }
            Ordering::Greater => {}
        }
    }
    if at >= 5 {
        return Ok(());
    }
    if top.len() == 5 {
        top.pop();
    }
    top.try_push(value).map_err(t::resource)?;
    let last = top.len() - 1;
    for index in (at..last).rev() {
        top.as_mut_slice().swap(index, index + 1)
    }
    Ok(())
}
async fn categories(b: &t::Budget, data: &Value, d: Option<Instant>) -> t::Result<t::OwnedString> {
    let mut top = t::OwnedVec::with_capacity(b, 5).map_err(t::resource)?;
    if let Some(Value::Array(reports)) = data.get("reports") {
        for report in reports {
            t::checkpoint(d).await?;
            let Some(raw) = report.as_object().and_then(|m| m.get("categories")) else {
                continue;
            };
            match raw {
                Value::Array(values) => {
                    for value in values {
                        t::checkpoint(d).await?;
                        let category = match value {
                            Value::String(text) => Category::Text(text),
                            _ => Category::Formatted(t::stringify(b, value, d).await?),
                        };
                        keep(&mut top, category, d).await?
                    }
                }
                Value::String(text) => {
                    for point in text.codepoints() {
                        t::checkpoint(d).await?;
                        keep(&mut top, Category::Point(*point), d).await?
                    }
                }
                Value::Object(map) => {
                    for (key, _) in map.iter() {
                        t::checkpoint(d).await?;
                        keep(&mut top, Category::Text(key), d).await?
                    }
                }
                _ => return Err(t::error("source report categories are not iterable")),
            }
        }
    }
    let mut strings = t::OwnedVec::with_capacity(b, top.len()).map_err(t::resource)?;
    for category in top.as_slice() {
        let value = match category {
            Category::Text(text) => c::owned_prefix(b, text, text.len(), d).await?,
            Category::Point(p) => {
                let ch = char::from_u32(*p)
                    .ok_or_else(|| t::error("source category contains non-scalar Unicode"))?;
                let mut out = t::OwnedString::new(b).map_err(t::resource)?;
                out.try_push(ch).map_err(t::resource)?;
                out
            }
            Category::Formatted(s) => t::owned(b, s.as_str(), d).await?,
        };
        strings.try_push(value).map_err(t::resource)?
    }
    let mut refs = t::OwnedVec::with_capacity(b, strings.len()).map_err(t::resource)?;
    for s in strings.as_slice() {
        refs.try_push(s.as_str()).map_err(t::resource)?
    }
    t::join(b, refs.as_slice(), ", ", d).await
}
impl AbuseIPDBAdapter {
    async fn execute<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        b: &'b t::Budget,
    ) -> t::Result<c::ProducedBatch<'b>> {
        t::admit(&self.admission, "abuseipdb", context).await?;
        if self.config.key().is_empty() {
            return Err(t::error(
                "AbuseIPDB API key not configured (set ENGINE_ABUSEIPDB_API_KEY)",
            ));
        }
        let ip = if let Some(ip) = threat_ioc::ip(&q.query, false, context).await? {
            ip
        } else {
            t::strip(&q.query, context).await?
        };
        let deadline = self.config.network(context)?;
        let url = t::join(b, &[&self.config.base_url, "/check"], "", context).await?;
        let mut req = t::get(
            url.as_str(),
            &[("ipAddress", ip), ("maxAgeInDays", "90"), ("verbose", "")],
            true,
        )?;
        t::credential(&mut req, "key", self.config.key())?;
        let body = t::fetch(&self.http, req, deadline, context, "AbuseIPDB", false)
            .await?
            .expect("only GreyNoise permits empty404");
        #[cfg(test)]
        t::projection_entered();
        let map = c::map();
        let data = t::field(&body, "data", &map)?;
        let mut rows = c::ProducedBatch::new(b, 1, context).map_err(c::projection_error)?;
        if !t::truthy(data) {
            return Ok(rows);
        }
        let empty = t::empty();
        let zero = c::parse_body_observed(b"0", context, None).await?;
        let confidence = t::field(data, "abuseConfidenceScore", &zero)?;
        let total = t::field(data, "totalReports", &zero)?;
        let last = t::field(data, "lastReportedAt", &empty)?;
        let confidence_text = t::stringify(b, confidence, context).await?;
        let first = t::join(
            b,
            &["Abuse Confidence: ", confidence_text.as_str(), "%"],
            "",
            context,
        )
        .await?;
        let total_text = t::stringify(b, total, context).await?;
        let report = t::join(b, &["Reports: ", total_text.as_str()], "", context).await?;
        let category = categories(b, data, context).await?;
        let category_text = t::join(b, &["Categories: ", category.as_str()], "", context).await?;
        let last_slice = if t::truthy(last) {
            t::slice_format(b, last, 10, context).await?
        } else {
            t::owned(b, "", context).await?
        };
        let last_text = t::join(b, &["Last reported: ", last_slice.as_str()], "", context).await?;
        let mut parts = vec![first.as_str()];
        if t::truthy(total) {
            parts.push(report.as_str())
        }
        if !category.is_empty() {
            parts.push(category_text.as_str())
        }
        for key in ["countryCode", "isp", "domain", "usageType"] {
            let value = t::field(data, key, &empty)?;
            if t::truthy(value) {
                parts.push(t::text(value)?)
            }
        }
        if t::truthy(last) {
            parts.push(last_text.as_str())
        }
        let content = t::join(b, &parts, " | ", context).await?;
        let url = t::join(b, &["https://www.abuseipdb.com/check/", ip], "", context).await?;
        let score = super::regulatory::float(confidence)? / 100.;
        t::push(
            &mut rows,
            "abuseipdb",
            t::Fields {
                url: url.as_str(),
                title: ip,
                content: content.as_str(),
                score,
                position: 1,
                date: None,
            },
        )
        .await?;
        Ok(rows)
    }
}
