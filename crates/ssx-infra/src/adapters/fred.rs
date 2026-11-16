//! Federal Reserve series search with provider-mandated query credentials.
use super::{
    clinicaltrials as c,
    source_json::Value,
    source_payload::{self, Field, FieldRef},
};
use crate::http::AdapterHttp;
use ssx_application::{PortFuture, SearchAdmission, SearchContext, SearchEngine};
use ssx_config::Secret;
use ssx_domain::{AdapterResponse, SearchRequest};
use std::{sync::Arc, time::Instant};
pub const ENDPOINT: &str = "https://api.stlouisfed.org/fred/series/search";
#[derive(Clone, Debug)]
pub struct FredConfig {
    pub settings: c::BiomedicalConfig,
    pub api_key: Secret,
}
impl Default for FredConfig {
    fn default() -> Self {
        Self {
            settings: c::BiomedicalConfig::defaults(ENDPOINT),
            api_key: Secret::new(None),
        }
    }
}
pub struct FredAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: FredConfig,
}
impl FredAdapter {
    pub fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: FredConfig,
    ) -> Self {
        Self {
            http,
            admission,
            config,
        }
    }
    async fn run(&self, q: &SearchRequest, context: Option<Instant>) -> AdapterResponse {
        let start = Instant::now();
        let budget = c::Budget::new(c::ROW_BYTES).expect("valid A08 quota");
        let mut response = c::finish_batch(self.execute(q, context, &budget).await, start).await;
        if response.error_message.as_deref()
            == Some("FRED API key not configured. Set ENGINE_FRED_API_KEY env var.")
        {
            response.latency_ms = Default::default();
        }
        response
    }
    async fn execute<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        budget: &'b c::Budget,
    ) -> c::Result<c::ProducedBatch<'b>> {
        c::admit_with_context(&self.admission, "fred", context).await?;
        let key = self
            .config
            .api_key
            .expose()
            .unwrap_or("")
            .trim_matches(ssx_json::python_whitespace);
        if key.is_empty() {
            return Err(c::error(
                "FRED API key not configured. Set ENGINE_FRED_API_KEY env var.",
            ));
        }
        let deadline = c::network_deadline(&self.config.settings, false, context)?;
        let max = c::scalar_text(&self.config.settings.max_results);
        let req = c::request(
            &self.config.settings.base_url,
            &[
                ("api_key", key),
                ("file_type", "json"),
                ("search_text", &q.query),
                ("limit", &max),
                ("order_by", "popularity"),
                ("sort_order", "desc"),
            ],
            false,
            false,
        )?;
        let data = super::regulatory::fetch(&self.http, req, deadline, false, context)
            .await?
            .unwrap();
        let list = c::list();
        let empty = c::empty();
        let null = Value::Null;
        let rows = c::result_rows(
            c::field(&data, "seriess", &list)?,
            &self.config.settings.max_results,
        )?;
        let mut batch =
            c::ProducedBatch::new(budget, rows.len(), context).map_err(c::projection_error)?;
        for (idx, row) in rows.iter().enumerate() {
            c::row_checkpoint(context).await?;
            let id = c::field(row, "id", &empty)?;
            let title = c::field(row, "title", &empty)?;
            let date = c::field(row, "observation_start", &empty)?;
            let units = c::field(row, "units", &empty)?;
            let freq = c::field(row, "frequency", &empty)?;
            let season = c::field(row, "seasonal_adjustment", &empty)?;
            let notes = c::field(row, "notes", &empty)?;
            let notes = if c::truthy(notes) { notes } else { &empty };
            let notes_display = c::owned_prefix(
                budget,
                notes
                    .as_text()
                    .ok_or_else(|| c::error("FRED notes must support text slicing"))?,
                200,
                context,
            )
            .await?;
            let mut parts = Vec::new();
            for v in [freq, units, season] {
                if c::truthy(v) {
                    parts.push(c::borrowed_text(v)?)
                }
            }
            if !notes_display.as_str().is_empty() {
                parts.push(notes_display.as_str())
            }
            if parts.is_empty() {
                parts.push("Economic data series")
            }
            let content = c::owned_join(budget, &parts, " — ", Some(500), context).await?;
            let id_text = c::owned_stringify(budget, id, context).await?;
            let title_text = c::owned_stringify(budget, title, context).await?;
            let url = c::owned_join(
                budget,
                &["https://fred.stlouisfed.org/series/", id_text.as_str()],
                "",
                None,
                context,
            )
            .await?;
            let title = c::owned_join(
                budget,
                &[title_text.as_str(), " (", id_text.as_str(), ")"],
                "",
                None,
                context,
            )
            .await?;
            let fields = [
                ("series_id", id),
                ("title", c::field(row, "title", &empty)?),
                ("units", units),
                ("frequency", freq),
                ("seasonal_adjustment", season),
                ("observation_start", date),
                ("notes", notes),
            ]
            .map(|(name, value)| Field {
                name,
                value: FieldRef::Value(if c::truthy(value) { value } else { &null }),
            });
            let payload = source_payload::envelope_budgeted(
                "financial",
                "economic_series",
                "fred",
                &fields,
                budget,
                context,
            )
            .await
            .map_err(c::projection_error)?;
            let score = match row.get("popularity").filter(|v| c::truthy(v)) {
                None => 0.,
                Some(v) => super::regulatory::float(v)?,
            };
            batch
                .push(
                    c::Skeleton {
                        url: url.as_str(),
                        title: title.as_str(),
                        content: content.as_str(),
                        engine: "fred",
                        category: "general",
                        score,
                        position: idx as i64 + 1,
                        tier: 1,
                        published_date: if c::truthy(date) {
                            Some(c::borrowed_text(date)?)
                        } else {
                            None
                        },
                    },
                    Some(payload),
                )
                .await
                .map_err(c::projection_error)?;
        }
        Ok(batch)
    }
}
impl SearchEngine for FredAdapter {
    fn search<'a>(&'a self, q: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.run(q, None))
    }
    fn search_with_context<'a>(
        &'a self,
        q: &'a SearchRequest,
        c: SearchContext,
    ) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.run(q, Some(c.deadline)))
    }
}
