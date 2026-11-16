//! TMDB multi-search: Bearer only, source page1, complete media-item payload.
use super::{
    catalog_media as m, clinicaltrials as c,
    source_json::Value,
    source_payload::{self, Field, FieldRef},
};
use ssx_domain::SearchRequest;
use std::time::Instant;
pub const ENDPOINT: &str = "https://api.themoviedb.org/3/search/multi";
#[derive(Clone, Debug)]
pub struct TmdbConfig {
    pub settings: m::Settings,
    pub api_key: ssx_config::Secret,
}
impl Default for TmdbConfig {
    fn default() -> Self {
        Self {
            settings: m::Settings::defaults(ENDPOINT),
            api_key: ssx_config::Secret::new(None),
        }
    }
}
m::adapter!(TmdbAdapter, TmdbConfig, "tmdb", TmdbConfig::default());
impl TmdbAdapter {
    async fn execute<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        b: &'b c::Budget,
    ) -> m::Result<m::Batch<'b>> {
        c::admit_with_context(&self.admission, "tmdb", context).await?;
        let key = self
            .config
            .api_key
            .expose()
            .unwrap_or("")
            .trim_matches(ssx_json::python_whitespace);
        if key.is_empty() {
            return Err(m::error(
                "TMDB API key not configured. Set ENGINE_TMDB_API_KEY env var.",
            ));
        }
        let deadline = c::network_deadline(&self.config.settings, false, context)?;
        let mut req = c::request(
            &self.config.settings.base_url,
            &[("query", &q.query), ("page", "1")],
            false,
            false,
        )?;
        // Invalid headers never disclose the credential in diagnostic text.
        let auth = format!("Bearer {key}")
            .parse()
            .map_err(|_| m::error("TMDB authorization header invalid"))?;
        req.headers.insert("authorization", auth);
        let data = m::fetch(&self.http, req, deadline, context, false, false, "TMDB").await?;
        let list = c::list();
        let empty = c::empty();
        let null = Value::Null;
        let rows = c::result_rows(
            c::field(&data, "results", &list)?,
            &self.config.settings.max_results,
        )?;
        let mut batch = m::Batch::new(b, rows.len(), context)?;
        for (idx, item) in rows.iter().enumerate() {
            c::row_checkpoint(context).await?;
            let kind = c::field(item, "media_type", &empty)?;
            let kind_str = if c::truthy(kind) { m::text(kind)? } else { "" };
            let (title, date, label) = match kind_str {
                "movie" => (
                    c::field(item, "title", &empty)?,
                    c::field(item, "release_date", &empty)?,
                    m::owned(b, "Movie", context).await?,
                ),
                "tv" => (
                    c::field(item, "name", &empty)?,
                    c::field(item, "first_air_date", &empty)?,
                    m::owned(b, "TV", context).await?,
                ),
                _ => {
                    let name = c::field(item, "name", &empty)?;
                    let title = if c::truthy(name) {
                        name
                    } else {
                        c::field(item, "title", &empty)?
                    };
                    // Python capitalize: titlecase first codepoint, lowercase rest; Unicode table is source-pinned below.
                    let label = if kind_str.is_empty() {
                        m::owned(b, "Content", context).await?
                    } else {
                        capitalize(b, kind_str, context).await?
                    };
                    (title, &empty, label)
                }
            };
            let title_str = m::text(title)?;
            let overview = c::field(item, "overview", &empty)?;
            let overview = if c::truthy(overview) {
                overview
            } else {
                &empty
            };
            let overview_text = overview
                .as_text()
                .ok_or_else(|| m::error("TMDB overview must support text slicing"))?;
            let overview_display = c::owned_prefix(b, overview_text, 200, context).await?;
            let date_str = if c::truthy(date) {
                Some(m::text(date)?)
            } else {
                None
            };
            let date_prefix = if let Some(s) = date.as_text() {
                c::owned_prefix(b, s, 4, context).await?
            } else {
                m::owned(b, "", context).await?
            };
            let prefix = if date_str.is_some() {
                m::join(
                    b,
                    &["[", label.as_str(), "] (", date_prefix.as_str(), ") "],
                    None,
                    context,
                )
                .await?
            } else {
                m::join(b, &["[", label.as_str(), "] "], None, context).await?
            };
            let content = m::join(
                b,
                &[prefix.as_str(), overview_display.as_str()],
                Some(500),
                context,
            )
            .await?;
            let id = c::owned_stringify(b, c::field(item, "id", &empty)?, context).await?;
            let url = if c::truthy(kind) {
                m::join(
                    b,
                    &["https://www.themoviedb.org/", kind_str, "/", id.as_str()],
                    None,
                    context,
                )
                .await?
            } else {
                m::owned(b, "", context).await?
            };
            let vote = item.get("vote_average").unwrap_or(&null);
            let vote_f = if matches!(vote, Value::Null) {
                None
            } else {
                Some(m::float(vote)?)
            };
            let score = if c::truthy(vote) { m::float(vote)? } else { 0. };
            let vote_value = vote_f
                .map(|v| Value::Number(super::source_json::Number::Float(v)))
                .unwrap_or(Value::Null);
            let fields = [
                ("media_type", if c::truthy(kind) { kind } else { &null }),
                ("title", if c::truthy(title) { title } else { &null }),
                ("release_date", if c::truthy(date) { date } else { &null }),
                (
                    "overview",
                    if c::truthy(overview) { overview } else { &null },
                ),
                ("vote_average", &vote_value),
            ];
            let fields = fields.map(|(name, value)| Field {
                name,
                value: FieldRef::Value(value),
            });
            let payload = source_payload::envelope_budgeted(
                "media",
                "media_item",
                "tmdb",
                &fields,
                b,
                context,
            )
            .await
            .map_err(c::projection_error)?;
            let thumbnail = if let Some(poster) = item.get("poster_path").filter(|v| c::truthy(v)) {
                let text = c::owned_stringify(b, poster, context).await?;
                Some(
                    m::join(
                        b,
                        &["https://image.tmdb.org/t/p/w200", text.as_str()],
                        None,
                        context,
                    )
                    .await?,
                )
            } else {
                None
            };
            batch
                .rows
                .push_with_thumbnail(
                    c::Skeleton {
                        url: url.as_str(),
                        title: title_str,
                        content: content.as_str(),
                        engine: "tmdb",
                        category: "general",
                        score,
                        position: idx as i64 + 1,
                        tier: 1,
                        published_date: date_str,
                    },
                    Some(payload),
                    thumbnail.as_ref().map(c::OwnedString::as_str),
                )
                .await
                .map_err(c::projection_error)?;
        }
        Ok(batch)
    }
}
async fn capitalize(b: &c::Budget, s: &str, context: Option<Instant>) -> m::Result<c::OwnedString> {
    Ok(super::tmdb_case::capitalize(s, b, context).await?)
}

#[cfg(test)]
fn set_test_key(config: &mut TmdbConfig) {
    config.api_key = ssx_config::Secret::new(Some("synthetic-tmdb-key".into()));
}
