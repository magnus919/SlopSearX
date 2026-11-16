//! GreyNoise source single-IP community/enterprise selection.
use super::{
    clinicaltrials as c,
    source_json::{Text, Value},
    threat_intel as t, threat_ioc,
};
use ssx_domain::SearchRequest;
use std::time::Instant;
pub const ENDPOINT: &str = "https://api.greynoise.io";
pub type GreyNoiseConfig = t::ThreatIntelConfig;
t::adapter!(GreyNoiseAdapter);
impl GreyNoiseAdapter {
    async fn execute<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        b: &'b t::Budget,
    ) -> t::Result<c::ProducedBatch<'b>> {
        t::admit(&self.admission, "greynoise", context).await?;
        let mut rows = c::ProducedBatch::new(b, 1, context).map_err(c::projection_error)?;
        let Some(ip) = threat_ioc::ip(&q.query, false, context).await? else {
            return Ok(rows);
        };
        let deadline = self.config.network(context)?;
        let enterprise = !self.config.key().is_empty();
        let url = t::join(
            b,
            &[
                &self.config.base_url,
                if enterprise {
                    "/v3/enterprise/"
                } else {
                    "/v3/community/"
                },
                ip,
            ],
            "",
            context,
        )
        .await?;
        let mut req = t::get(url.as_str(), &[], enterprise)?;
        if enterprise {
            t::credential(&mut req, "key", self.config.key())?
        }
        let Some(data) = t::fetch(&self.http, req, deadline, context, "GreyNoise", false).await?
        else {
            return Ok(rows);
        };
        #[cfg(test)]
        t::projection_entered();
        if data.as_object().is_none() {
            return Err(t::error(
                "GreyNoise returned malformed JSON; expected an object",
            ));
        }
        if data.get("ip").and_then(Value::as_str) != Some(ip) {
            return Err(t::error(
                "GreyNoise response IP did not match the requested IP",
            ));
        }
        let empty = t::empty();
        let unknown = Value::String(Text::from("unknown"));
        let false_value = Value::Bool(false);
        let classification = t::field(&data, "classification", &unknown)?;
        let noise = t::field(&data, "noise", &false_value)?;
        let riot = t::field(&data, "riot", &false_value)?;
        let bot = t::field(&data, "bot", &false_value)?;
        let tags = t::field(&data, "tags", &empty)?;
        let name = t::field(&data, "name", &empty)?;
        let link = t::field(&data, "link", &empty)?;
        let last = t::field(&data, "last_seen", &empty)?;
        let classification_value = t::stringify(b, classification, context).await?;
        let classification_text = t::join(
            b,
            &["Classification: ", classification_value.as_str()],
            "",
            context,
        )
        .await?;
        let tag_value = if t::truthy(tags) && tags.is_array() {
            t::tags(b, tags, context).await?
        } else {
            t::owned(b, "", context).await?
        };
        let tag_text = t::join(b, &["Tags: ", tag_value.as_str()], "", context).await?;
        let name_value = t::stringify(b, name, context).await?;
        let name_text = t::join(b, &["Name: ", name_value.as_str()], "", context).await?;
        let last_value = if t::truthy(last) {
            t::slice_format(b, last, 10, context).await?
        } else {
            t::owned(b, "", context).await?
        };
        let last_text = t::join(b, &["Last seen: ", last_value.as_str()], "", context).await?;
        let mut parts = vec![if t::truthy(noise) {
            "Noise: background scanner"
        } else {
            "Noise: potentially targeted"
        }];
        if t::truthy(riot) {
            parts.push("RIOT: benign service")
        }
        if classification.as_str() != Some("unknown") {
            parts.push(classification_text.as_str())
        }
        if t::truthy(bot) {
            parts.push("Bot activity detected")
        }
        if t::truthy(tags) && tags.is_array() {
            parts.push(tag_text.as_str())
        }
        if t::truthy(name) {
            parts.push(name_text.as_str())
        }
        if t::truthy(link) {
            parts.push(t::text(link)?)
        }
        if t::truthy(last) {
            parts.push(last_text.as_str())
        }
        let content = t::join(b, &parts, " | ", context).await?;
        let url = t::join(b, &["https://viz.greynoise.io/ip/", ip], "", context).await?;
        let title = t::join(b, &["GreyNoise — ", ip], "", context).await?;
        t::push(
            &mut rows,
            "greynoise",
            t::Fields {
                url: url.as_str(),
                title: title.as_str(),
                content: content.as_str(),
                score: 0.,
                position: 1,
                date: None,
            },
        )
        .await?;
        Ok(rows)
    }
}
