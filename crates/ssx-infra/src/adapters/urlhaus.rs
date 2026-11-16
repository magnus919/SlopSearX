//! URLhaus exact URL/MD5/host routing and all-record validation before slicing10.
use super::{clinicaltrials as c, source_json::Value, threat_intel as t, threat_ioc};
use ssx_domain::{EngineStatus, SearchRequest};
use std::time::Instant;
pub const ENDPOINT: &str = "https://urlhaus-api.abuse.ch";
pub type URLhausConfig = t::ThreatIntelConfig;
t::adapter!(URLhausAdapter);
impl URLhausAdapter {
    async fn execute<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        b: &'b t::Budget,
    ) -> t::Result<c::ProducedBatch<'b>> {
        // Source URLhaus uniquely refuses absent credentials before rate admission.
        if self.config.key().is_empty() {
            return Err(t::classified(
                EngineStatus::Unavailable,
                Some("URLhaus requires ENGINE_URLHAUS_API_KEY"),
            ));
        }
        t::admit(&self.admission, "urlhaus", context).await?;
        let deadline = self.config.network(context)?;
        let normalized = t::strip(&q.query, context).await?;
        if normalized.is_empty() {
            return Err(t::error("URLhaus query is empty"));
        }
        let matched_url = threat_ioc::url(&q.query, context).await?;
        let ip = threat_ioc::ip(&q.query, true, context).await?;
        let mut lower = t::OwnedString::new(b).map_err(t::resource)?;
        let (kind, key, value) = if let Some(url) = matched_url {
            ("url", "url", url)
        } else if threat_ioc::md5(normalized) {
            // Only this source branch lowercases: exactly32 ASCII hex bytes.
            for byte in normalized.bytes() {
                lower
                    .try_push(byte.to_ascii_lowercase() as char)
                    .map_err(t::resource)?;
            }
            ("payload", "md5_hash", lower.as_str())
        } else if let Some(ip) = ip {
            ("host", "host", ip)
        } else {
            let word = t::first_word(normalized, context).await?;
            if !threat_ioc::hostname(word, context).await? {
                return Err(t::error(
                    "URLhaus query is not a supported URL, host, or MD5 hash",
                ));
            }
            ("host", "host", word)
        };
        let endpoint = t::join(b, &[&self.config.base_url, "/v1/", kind, "/"], "", context).await?;
        let mut req = t::get(endpoint.as_str(), &[], true)?;
        req.method = reqwest::Method::POST;
        req.body = super::broad_query(&[(key, value)]).into_bytes();
        req.headers.insert(
            "content-type",
            "application/x-www-form-urlencoded"
                .parse()
                .expect("static header"),
        );
        t::credential(&mut req, "auth-key", self.config.key())?;
        let data = t::fetch(&self.http, req, deadline, context, "URLhaus", true)
            .await?
            .expect("only GreyNoise empty404");
        #[cfg(test)]
        t::projection_entered();
        if data.as_object().is_none() {
            return Err(t::error("URLhaus response must be a JSON object"));
        }
        let status = data
            .get("query_status")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| t::error("URLhaus response is missing query_status"))?;
        let mut batch = c::ProducedBatch::new(b, 10, context).map_err(c::projection_error)?;
        if status == "no_results" {
            return Ok(batch);
        }
        if [
            "invalid_host",
            "invalid_url",
            "invalid_md5",
            "invalid_sha256",
        ]
        .contains(&status)
        {
            return Err(t::error(format!(
                "URLhaus rejected invalid input ({status}) for {kind} query"
            )));
        }
        if status != "ok" {
            return Err(t::error(format!(
                "URLhaus returned unexpected query_status: {status}"
            )));
        }
        if kind == "url" {
            if data.get("id").is_none() && data.get("url_id").is_none() {
                return Err(t::error("URLhaus URL response is missing its identifier"));
            }
            self.single(&mut batch, &data, context, b).await?;
        } else {
            let rows = data
                .get("urls")
                .and_then(Value::as_array)
                .filter(|v| !v.is_empty())
                .ok_or_else(|| t::error("URLhaus response is missing a non-empty urls list"))?;
            for row in rows {
                t::checkpoint(context).await?;
                if row.as_object().is_none() {
                    return Err(t::error("URLhaus response contains a malformed URL record"));
                }
            }
            for row in rows.iter().take(10) {
                t::checkpoint(context).await?;
                self.single(&mut batch, row, context, b).await?
            }
        }
        Ok(batch)
    }
    async fn single<'b>(
        &self,
        batch: &mut c::ProducedBatch<'b>,
        item: &Value,
        d: Option<Instant>,
        b: &'b t::Budget,
    ) -> t::Result<()> {
        let empty = t::empty();
        let list = c::list();
        let url = t::field(item, "url", &empty)?;
        let threat = t::field(item, "threat", &empty)?;
        let tags = t::field(item, "tags", &list)?;
        let kind = t::field(item, "file_type", &empty)?;
        let first = t::field(item, "firstseen", &empty)?;
        let last = t::field(item, "lastseen", &empty)?;
        let status = t::field(item, "url_status", &empty)?;
        let host = t::field(item, "host", &empty)?;
        let threat_value = t::stringify(b, threat, d).await?;
        let threat_text = t::join(b, &["Threat: ", threat_value.as_str()], "", d).await?;
        let tag_value = if t::truthy(tags) {
            t::tags(b, tags, d).await?
        } else {
            t::owned(b, "", d).await?
        };
        let tag_text = t::join(b, &["Tags: ", tag_value.as_str()], "", d).await?;
        let kind_value = t::stringify(b, kind, d).await?;
        let kind_text = t::join(b, &["Type: ", kind_value.as_str()], "", d).await?;
        let status_value = t::stringify(b, status, d).await?;
        let status_text = t::join(b, &["Status: ", status_value.as_str()], "", d).await?;
        let first_slice = if t::truthy(first) {
            t::slice_format(b, first, 10, d).await?
        } else {
            t::owned(b, "", d).await?
        };
        let first_text = t::join(b, &["First: ", first_slice.as_str()], "", d).await?;
        let last_slice = if t::truthy(last) {
            t::slice_format(b, last, 10, d).await?
        } else {
            t::owned(b, "", d).await?
        };
        let last_text = t::join(b, &["Last: ", last_slice.as_str()], "", d).await?;
        let mut parts = Vec::new();
        for (present, text) in [
            (t::truthy(threat), threat_text.as_str()),
            (t::truthy(tags), tag_text.as_str()),
            (t::truthy(kind), kind_text.as_str()),
            (t::truthy(status), status_text.as_str()),
            (t::truthy(first), first_text.as_str()),
            (t::truthy(last), last_text.as_str()),
        ] {
            if present {
                parts.push(text)
            }
        }
        let content = t::join(b, &parts, " | ", d).await?;
        let title = if !t::truthy(url) {
            "malware URL"
        } else if t::truthy(host) {
            t::text(host)?
        } else {
            t::text(url)?
                .split('/')
                .nth(2)
                .ok_or_else(|| t::error("list index out of range"))?
        };
        let id = item
            .get("url_id")
            .or_else(|| item.get("id"))
            .unwrap_or(&empty);
        let id_value = t::stringify(b, id, d).await?;
        let fallback = t::join(
            b,
            &["https://urlhaus.abuse.ch/url/", id_value.as_str()],
            "",
            d,
        )
        .await?;
        let result_url = if t::truthy(url) {
            t::text(url)?
        } else {
            fallback.as_str()
        };
        if t::truthy(first) && !matches!(first, Value::String(_)) {
            return Err(t::error(
                "source publication date requires reviewed typed projection",
            ));
        }
        t::push(
            batch,
            "urlhaus",
            t::Fields {
                url: result_url,
                title,
                content: content.as_str(),
                score: 0.,
                position: 1,
                date: if t::truthy(first) {
                    Some(first_slice.as_str())
                } else {
                    None
                },
            },
        )
        .await
    }
}
