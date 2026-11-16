//! MusicBrainz source artist endpoint; no invented release/recording fallback.
use super::{catalog_media as m, clinicaltrials as c};
use ssx_domain::SearchRequest;
use std::time::Instant;
pub const ENDPOINT: &str = "https://musicbrainz.org/ws/2";
pub type MusicBrainzConfig = m::Settings;
m::adapter!(
    MusicBrainzAdapter,
    MusicBrainzConfig,
    "musicbrainz",
    MusicBrainzConfig::defaults(ENDPOINT)
);
impl MusicBrainzAdapter {
    async fn execute<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        b: &'b c::Budget,
    ) -> m::Result<m::Batch<'b>> {
        c::admit_with_context(&self.admission, "musicbrainz", context).await?;
        let deadline = c::network_deadline(&self.config, false, context)?;
        let url = m::join(b, &[&self.config.base_url, "/artist/"], None, context).await?;
        let max = c::scalar_text(&self.config.max_results);
        let req = c::request(
            url.as_str(),
            &[("query", &q.query), ("limit", &max), ("fmt", "json")],
            true,
            true,
        )?;
        let data = m::fetch(
            &self.http,
            req,
            deadline,
            context,
            false,
            false,
            "MusicBrainz",
        )
        .await?;
        let list = c::list();
        let empty = c::empty();
        let map = c::map();
        let rows = c::result_rows(c::field(&data, "artists", &list)?, &self.config.max_results)?;
        let mut batch = m::Batch::new(b, rows.len(), context)?;
        for (idx, artist) in rows.iter().enumerate() {
            c::row_checkpoint(context).await?;
            let title = m::text(c::field(artist, "name", &empty)?)?;
            let id = c::owned_stringify(b, c::field(artist, "id", &empty)?, context).await?;
            let kind = c::field(artist, "type", &empty)?;
            let country = c::field(artist, "country", &empty)?;
            let span = c::field(artist, "life-span", &map)?;
            let span = if c::truthy(span) { span } else { &map };
            let begin = c::field(span, "begin", &empty)?;
            let begin_text = c::owned_stringify(b, begin, context).await?;
            let active = m::join(b, &["Active since ", begin_text.as_str()], None, context).await?;
            let mut parts = Vec::new();
            for value in [kind, country] {
                if c::truthy(value) {
                    parts.push(m::text(value)?);
                }
            }
            if c::truthy(begin) {
                parts.push(active.as_str())
            }
            if parts.is_empty() {
                parts.push("Music artist")
            }
            let content = c::owned_join(b, &parts, " — ", Some(500), context).await?;
            let url = m::join(
                b,
                &["https://musicbrainz.org/artist/", id.as_str()],
                None,
                context,
            )
            .await?;
            batch
                .rows
                .push(
                    c::Skeleton {
                        url: url.as_str(),
                        title,
                        content: content.as_str(),
                        engine: "musicbrainz",
                        category: "general",
                        score: 1.,
                        position: idx as i64 + 1,
                        tier: 1,
                        published_date: None,
                    },
                    None,
                )
                .await
                .map_err(c::projection_error)?;
        }
        Ok(batch)
    }
}

#[cfg(test)]
fn set_test_key(config: &mut MusicBrainzConfig) {
    let _ = config;
}
