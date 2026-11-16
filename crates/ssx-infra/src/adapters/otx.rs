//! OTX source precedence IPv4, hash, CVE, then subscribed pulse search.
use super::{clinicaltrials as c, source_json::Value, threat_intel as t, threat_ioc};
use ssx_domain::SearchRequest;
use std::time::Instant;
pub const ENDPOINT: &str = "https://otx.alienvault.com/api/v1";
pub type OTXConfig = t::ThreatIntelConfig;
t::adapter!(OTXAdapter);
impl OTXAdapter {
    async fn execute<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        b: &'b t::Budget,
    ) -> t::Result<c::ProducedBatch<'b>> {
        t::admit(&self.admission, "otx", context).await?;
        if self.config.key().is_empty() {
            return Err(t::error(
                "OTX API key not configured (set ENGINE_OTX_API_KEY)",
            ));
        }
        let deadline = self.config.network(context)?;
        let mut upper = t::owned(b, "", context).await?;
        let (kind, indicator) = if let Some(ip) = threat_ioc::ip(&q.query, false, context).await? {
            ("IPv4", ip)
        } else if let Some(hash) = threat_ioc::hash(&q.query, context).await? {
            ("file", hash)
        } else if let Some(cve) = threat_ioc::cve(&q.query, context).await? {
            // The matched prefix is ASCII c/v/e and the remainder consists of digits/hyphens.
            upper.try_reserve_exact(cve.len()).map_err(t::resource)?;
            for (n, ch) in cve.chars().enumerate() {
                if n % 1024 == 0 {
                    t::checkpoint(context).await?
                }
                upper
                    .try_push(ch.to_ascii_uppercase())
                    .map_err(t::resource)?
            }
            ("CVE", upper.as_str())
        } else {
            ("pulses", "")
        };
        let limit = c::scalar_text(&self.config.max_results);
        let url = if kind == "pulses" {
            t::join(
                b,
                &[&self.config.base_url, "/pulses/subscribed"],
                "",
                context,
            )
            .await?
        } else {
            t::join(
                b,
                &[
                    &self.config.base_url,
                    "/indicators/",
                    kind,
                    "/",
                    indicator,
                    "/general",
                ],
                "",
                context,
            )
            .await?
        };
        let pulse_params = [("q", q.query.as_str()), ("limit", limit.as_str())];
        let mut req = t::get(
            url.as_str(),
            if kind == "pulses" { &pulse_params } else { &[] },
            true,
        )?;
        t::credential(&mut req, "x-otx-api-key", self.config.key())?;
        let data = t::fetch(&self.http, req, deadline, context, "OTX", false)
            .await?
            .expect("only GreyNoise empty404");
        #[cfg(test)]
        t::projection_entered();
        if kind == "pulses" {
            self.pulses(&data, context, b).await
        } else {
            self.indicator(&data, indicator, context, b).await
        }
    }
    async fn indicator<'b>(
        &self,
        data: &Value,
        indicator: &str,
        d: Option<Instant>,
        b: &'b t::Budget,
    ) -> t::Result<c::ProducedBatch<'b>> {
        let empty = t::empty();
        let zero = c::parse_body_observed(b"0", d, None).await?;
        let base = t::field(data, "base_score", &zero)?;
        let pulse = data
            .get("pulse_info")
            .and_then(Value::as_object)
            .and_then(|v| v.get("count"))
            .unwrap_or(&zero);
        let tags = t::field(data, "tags", &empty)?;
        let country = t::field(data, "country_code", &empty)?;
        let asn = t::field(data, "asn", &empty)?;
        let reputation = t::field(data, "reputation", &zero)?;
        let base_value = t::stringify(b, base, d).await?;
        let base_text = t::join(b, &["Score: ", base_value.as_str()], "", d).await?;
        let pulse_value = t::stringify(b, pulse, d).await?;
        let pulse_text = t::join(b, &["Pulses: ", pulse_value.as_str()], "", d).await?;
        let tag_value = if t::truthy(tags) {
            t::tags(b, tags, d).await?
        } else {
            t::owned(b, "", d).await?
        };
        let tag_text = t::join(b, &["Tags: ", tag_value.as_str()], "", d).await?;
        let rep_value = t::stringify(b, reputation, d).await?;
        let rep_text = t::join(b, &["Reputation: ", rep_value.as_str()], "", d).await?;
        let mut parts = Vec::new();
        if t::truthy(base) {
            parts.push(base_text.as_str())
        }
        if t::truthy(pulse) {
            parts.push(pulse_text.as_str())
        }
        if t::truthy(tags) {
            parts.push(tag_text.as_str())
        }
        if t::truthy(reputation) {
            parts.push(rep_text.as_str())
        }
        if t::truthy(country) {
            parts.push(t::text(country)?)
        }
        if t::truthy(asn) {
            parts.push(t::text(asn)?)
        }
        if parts.is_empty() {
            parts.push("OTX indicator result")
        }
        let content = t::join(b, &parts, " | ", d).await?;
        let url = t::join(
            b,
            &["https://otx.alienvault.com/indicator/ip/", indicator],
            "",
            d,
        )
        .await?;
        let score = if t::truthy(base) {
            super::regulatory::float(base)?
        } else {
            0.
        };
        let mut batch = c::ProducedBatch::new(b, 1, d).map_err(c::projection_error)?;
        t::push(
            &mut batch,
            "otx",
            t::Fields {
                url: url.as_str(),
                title: indicator,
                content: content.as_str(),
                score,
                position: 1,
                date: None,
            },
        )
        .await?;
        Ok(batch)
    }
    async fn pulses<'b>(
        &self,
        data: &Value,
        d: Option<Instant>,
        b: &'b t::Budget,
    ) -> t::Result<c::ProducedBatch<'b>> {
        let list = c::list();
        let empty = t::empty();
        let zero = c::parse_body_observed(b"0", d, None).await?;
        let raw = t::field(data, "results", &list)?;
        let rows = c::result_rows(raw, &self.config.max_results)?;
        let mut batch = c::ProducedBatch::new(b, rows.len(), d).map_err(c::projection_error)?;
        for (index, pulse) in rows.iter().enumerate() {
            t::checkpoint(d).await?;
            let name = t::text(t::field(pulse, "name", &empty)?)?;
            let desc = t::field(pulse, "description", &empty)?;
            let description = match desc {
                Value::Array(v) if v.is_empty() => t::owned(b, "", d).await?,
                _ => t::prefix(b, desc, 300, d).await?,
            };
            let tags = t::field(pulse, "tags", &list)?;
            let created = t::field(pulse, "created", &empty)?;
            let count = t::field(pulse, "indicator_count", &zero)?;
            let id = t::field(pulse, "id", &empty)?;
            let tag_value = if t::truthy(tags) {
                t::tags(b, tags, d).await?
            } else {
                t::owned(b, "", d).await?
            };
            let tag_text = t::join(b, &["Tags: ", tag_value.as_str()], "", d).await?;
            let count_value = t::stringify(b, count, d).await?;
            let count_text = t::join(b, &["IOCs: ", count_value.as_str()], "", d).await?;
            let mut parts = Vec::new();
            if !description.is_empty() {
                parts.push(description.as_str())
            }
            if t::truthy(tags) {
                parts.push(tag_text.as_str())
            }
            if t::truthy(count) {
                parts.push(count_text.as_str())
            }
            let content = t::join(b, &parts, " | ", d).await?;
            let id_value = t::stringify(b, id, d).await?;
            let url = if t::truthy(id) {
                t::join(
                    b,
                    &["https://otx.alienvault.com/pulse/", id_value.as_str()],
                    "",
                    d,
                )
                .await?
            } else {
                t::owned(b, "", d).await?
            };
            let date = if !t::truthy(created) {
                None
            } else {
                match created {
                    Value::String(text) if text.len() >= 10 => {
                        Some(t::prefix(b, created, 10, d).await?)
                    }
                    Value::String(_) => None,
                    Value::Array(rows) if rows.len() < 10 => None,
                    Value::Object(map) if map.len() < 10 => None,
                    _ => {
                        return Err(t::error(
                            "source publication date requires reviewed typed projection",
                        ));
                    }
                }
            };
            t::push(
                &mut batch,
                "otx",
                t::Fields {
                    url: url.as_str(),
                    title: name,
                    content: content.as_str(),
                    score: 0.,
                    position: index + 1,
                    date: date.as_ref().map(|s| s.as_str()),
                },
            )
            .await?;
        }
        Ok(batch)
    }
}
