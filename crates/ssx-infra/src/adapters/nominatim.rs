//! Nominatim geocoding with source OSM/detail identity and truthiness score.
use super::{catalog_media as m, clinicaltrials as c};
use ssx_domain::SearchRequest;
use std::time::Instant;
pub const ENDPOINT: &str = "https://nominatim.openstreetmap.org/search";
pub type NominatimConfig = m::Settings;
m::adapter!(
    NominatimAdapter,
    NominatimConfig,
    "nominatim",
    NominatimConfig::defaults(ENDPOINT)
);
impl NominatimAdapter {
    async fn execute<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        b: &'b c::Budget,
    ) -> m::Result<m::Batch<'b>> {
        c::admit_with_context(&self.admission, "nominatim", context).await?;
        let deadline = c::network_deadline(&self.config, false, context)?;
        let max = c::scalar_text(&self.config.max_results);
        let req = c::request(
            &self.config.base_url,
            &[
                ("q", &q.query),
                ("format", "json"),
                ("limit", &max),
                ("addressdetails", "1"),
            ],
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
            "Nominatim",
        )
        .await?;
        let empty = c::empty();
        let rows = c::result_rows(&data, &self.config.max_results)?;
        let mut batch = m::Batch::new(b, rows.len(), context)?;
        for (idx, place) in rows.iter().enumerate() {
            c::row_checkpoint(context).await?;
            let display = c::field(place, "display_name", &empty)?;
            let display_text = m::text(display)?;
            let kind = c::field(place, "type", &empty)?;
            let category = c::field(place, "category", &empty)?;
            let kind_text = c::owned_stringify(b, kind, context).await?;
            let category_text = c::owned_stringify(b, category, context).await?;
            let suffix = m::join(
                b,
                &["[", category_text.as_str(), "/", kind_text.as_str(), "]"],
                None,
                context,
            )
            .await?;
            let content = if c::truthy(kind) || c::truthy(category) {
                m::join(
                    b,
                    &[display_text, " — ", suffix.as_str()],
                    Some(500),
                    context,
                )
                .await?
            } else {
                m::join(b, &[display_text], Some(500), context).await?
            };
            let title_prefix = c::owned_prefix(
                b,
                display
                    .as_text()
                    .ok_or_else(|| m::error("Nominatim display name must be text"))?,
                100,
                context,
            )
            .await?;
            let title = if display.as_text().unwrap().len() > 100 {
                m::join(b, &[title_prefix.as_str(), "..."], None, context).await?
            } else {
                title_prefix
            };
            let osm_type = c::field(place, "osm_type", &empty)?;
            let osm_id = c::field(place, "osm_id", &empty)?;
            let url = if c::truthy(osm_type) && c::truthy(osm_id) {
                let kind = c::owned_stringify(b, osm_type, context).await?;
                let id = c::owned_stringify(b, osm_id, context).await?;
                m::join(
                    b,
                    &[
                        "https://www.openstreetmap.org/",
                        kind.as_str(),
                        "/",
                        id.as_str(),
                    ],
                    None,
                    context,
                )
                .await?
            } else {
                let id =
                    c::owned_stringify(b, c::field(place, "place_id", &empty)?, context).await?;
                m::join(
                    b,
                    &[
                        "https://nominatim.openstreetmap.org/details.php?place_id=",
                        id.as_str(),
                    ],
                    None,
                    context,
                )
                .await?
            };
            let importance = place.get("importance").filter(|v| c::truthy(v));
            let score = if let Some(v) = importance {
                m::float(v)?
            } else {
                0.5
            };
            batch
                .rows
                .push(
                    c::Skeleton {
                        url: url.as_str(),
                        title: title.as_str(),
                        content: content.as_str(),
                        engine: "nominatim",
                        category: "general",
                        score,
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
fn set_test_key(config: &mut NominatimConfig) {
    let _ = config;
}
