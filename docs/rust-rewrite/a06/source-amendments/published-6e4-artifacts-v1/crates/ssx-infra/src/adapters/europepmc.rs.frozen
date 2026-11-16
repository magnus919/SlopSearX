//! Opt-in Europe PMC lite records. Provider attribution is part of the result contract.
use super::clinicaltrials::*;
use super::source_json::Value;
use crate::http::AdapterHttp;
use ssx_application::{PortFuture, SearchAdmission, SearchContext, SearchEngine};
use ssx_domain::{AdapterResponse, SearchRequest};
use std::{sync::Arc, time::Instant};
pub struct EuropePMCAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: BiomedicalConfig,
}
impl EuropePMCAdapter {
    pub fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: BiomedicalConfig,
    ) -> Self {
        Self {
            http,
            admission,
            config,
        }
    }
    async fn run(&self, q: &SearchRequest, context: Option<Instant>) -> AdapterResponse {
        let start = Instant::now();
        let budget = Budget::new(ROW_BYTES).expect("valid biomedical quota");
        finish_batch(self.run_with_budget(q, context, &budget).await, start).await
    }
    pub(super) async fn run_with_budget<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        budget: &'b Budget,
    ) -> Result<ProducedBatch<'b>> {
        self.run_observed(q, context, budget, None).await
    }
    pub(super) async fn run_observed<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        budget: &'b Budget,
        observer: Observer<'_>,
    ) -> Result<ProducedBatch<'b>> {
        use super::source_payload::{Field, FieldRef, publication_budgeted};
        admit_with_context(&self.admission, "europepmc", context).await?;
        let limit = int_value(&self.config.max_results)?.clamp(1, 1000) as usize;
        let d = network_deadline_for_http(&self.http, &self.config, context)
            .map_err(Failure::empty_timeout_diagnostic)?;
        let req = request(
            &format!("{}/search", self.config.base_url.trim_end_matches('/')),
            &[
                ("query", &q.query),
                ("format", "json"),
                ("resultType", "lite"),
                ("pageSize", &limit.to_string()),
            ],
            false,
            false,
        )?;
        let mut session = session(&self.http, d)?;
        let data = get_observed(&mut session, req, d, false, true, context, observer)
            .await?
            .unwrap();
        let records = data
            .as_object()
            .and_then(|d| d.get("resultList"))
            .and_then(Value::as_object)
            .ok_or_else(|| error("Europe PMC response missing resultList"))?;
        let records = records
            .get("result")
            .and_then(Value::as_array)
            .ok_or_else(|| error("Europe PMC results must be a list"))?;
        struct Pending<'a> {
            row: Draft,
            fields: [Field<'a>; 8],
        }
        let mut rows =
            OwnedVec::with_capacity(budget, records.len().min(limit)).map_err(owned_error)?;
        for record in records.iter().take(limit) {
            row_checkpoint(context).await?;
            let record =
                object(record).map_err(|_| error("Europe PMC record must be an object"))?;
            let (Some(source), Some(id), Some(title)) = (
                record.get("source").and_then(Value::as_text),
                record.get("id").and_then(Value::as_text),
                record.get("title").and_then(Value::as_text),
            ) else {
                continue;
            };
            if source.is_empty() || id.is_empty() || title.is_empty() {
                continue;
            }
            let source_str = source.as_str().ok_or_else(|| {
                error("source text contains non-scalar Unicode; requires reviewed projection")
            })?;
            let id_str = id.as_str().ok_or_else(|| {
                error("source text contains non-scalar Unicode; requires reviewed projection")
            })?;
            let title_str = title.as_str().ok_or_else(|| {
                error("source text contains non-scalar Unicode; requires reviewed projection")
            })?;
            let quoted_source = owned_quote(budget, source_str, false, context).await?;
            let quoted_id = owned_quote(budget, id_str, false, context).await?;
            let url = owned_join(
                budget,
                &[
                    "https://europepmc.org/article/",
                    quoted_source.as_str(),
                    "/",
                    quoted_id.as_str(),
                ],
                "",
                None,
                context,
            )
            .await?;
            let title = owned_join(budget, &[title_str], "", None, context).await?;
            let mut pieces = OwnedVec::with_capacity(budget, 4).map_err(owned_error)?;
            pieces
                .try_push("Source: Europe PMC / EMBL-EBI (https://europepmc.org)")
                .map_err(owned_error)?;
            if source_str == "PPR" {
                pieces
                    .try_push("Preprint (not peer reviewed)")
                    .map_err(owned_error)?;
            }
            let journal = if let Some(text) = record.get("journalTitle").and_then(Value::as_text) {
                owned_prefix(budget, text, 150, context).await?
            } else {
                OwnedString::new(budget).map_err(owned_error)?
            };
            let authors = if let Some(text) = record.get("authorString").and_then(Value::as_text) {
                owned_prefix(budget, text, 200, context).await?
            } else {
                OwnedString::new(budget).map_err(owned_error)?
            };
            if !journal.is_empty() {
                pieces.try_push(journal.as_str()).map_err(owned_error)?;
            }
            if !authors.is_empty() {
                pieces.try_push(authors.as_str()).map_err(owned_error)?;
            }
            let content = owned_join(budget, pieces.as_slice(), " — ", None, context).await?;
            let date = record
                .get("firstPublicationDate")
                .and_then(Value::as_str)
                .and_then(iso_date);
            let published_date = match date {
                Some(date) => Some(fixed_format(
                    budget,
                    10,
                    format_args!("{}", date.format("%Y-%m-%d")),
                )?),
                None => None,
            };
            let null = &Value::Null;
            let pmid = if source_str == "MED" && med_id_digits(id_str, context, observer).await? {
                FieldRef::Text(id)
            } else {
                FieldRef::Value(null)
            };
            let fields = [
                Field {
                    name: "publication_id",
                    value: FieldRef::Text(id),
                },
                Field {
                    name: "source",
                    value: FieldRef::Text(source),
                },
                Field {
                    name: "pmid",
                    value: pmid,
                },
                Field {
                    name: "pmcid",
                    value: FieldRef::Value(record.get("pmcid").unwrap_or(null)),
                },
                Field {
                    name: "doi",
                    value: FieldRef::Value(record.get("doi").unwrap_or(null)),
                },
                Field {
                    name: "journal",
                    value: FieldRef::Value(record.get("journalTitle").unwrap_or(null)),
                },
                Field {
                    name: "is_open_access",
                    value: FieldRef::Value(record.get("isOpenAccess").unwrap_or(null)),
                },
                Field {
                    name: "in_epmc",
                    value: FieldRef::Value(record.get("inEPMC").unwrap_or(null)),
                },
            ];
            let score = 1. / (rows.len() + 1) as f64;
            rows.try_push(Pending {
                row: Draft {
                    url,
                    title,
                    content,
                    published_date,
                    position: 0,
                    score,
                },
                fields,
            })
            .map_err(owned_error)?;
        }
        let mut batch =
            ProducedBatch::new(budget, rows.len(), context).map_err(projection_error)?;
        for pending in rows.as_slice() {
            observe(observer, Phase::Payload);
            let payload = publication_budgeted("europepmc", &pending.fields, budget, context)
                .await
                .map_err(projection_error)?;
            batch
                .push(pending.row.skeleton("europepmc"), Some(payload))
                .await
                .map_err(projection_error)?;
        }
        Ok(batch)
    }
}
impl SearchEngine for EuropePMCAdapter {
    fn search_with_context<'a>(
        &'a self,
        q: &'a SearchRequest,
        context: SearchContext,
    ) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move { self.run(q, Some(context.deadline)).await })
    }

    fn search<'a>(&'a self, q: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move { self.run(q, None).await })
    }
}
/// urllib.parse.quote: slash is safe only for PubChem's name path.
fn iso_date(value: &str) -> Option<chrono::NaiveDate> {
    let b = value.as_bytes();
    let digits = |bytes: &[u8]| bytes.iter().all(u8::is_ascii_digit);
    let year = b.len() >= 4 && digits(&b[..4]);
    let calendar = year
        && ((b.len() == 10
            && b[4] == b'-'
            && b[7] == b'-'
            && digits(&b[5..7])
            && digits(&b[8..10]))
            || (b.len() == 8 && digits(b)));
    let week = year
        && (((b.len() == 8 || b.len() == 10)
            && b[4] == b'-'
            && b[5] == b'W'
            && digits(&b[6..8])
            && (b.len() == 8 || (b[8] == b'-' && b[9].is_ascii_digit())))
            || ((b.len() == 7 || b.len() == 8)
                && b[4] == b'W'
                && digits(&b[5..7])
                && (b.len() == 7 || b[7].is_ascii_digit())));
    if !calendar && !week {
        return None;
    }
    let parsed = if value.len() == 10 && !value.contains('W') {
        chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
    } else if value.len() == 8 && !value.contains('W') {
        chrono::NaiveDate::parse_from_str(value, "%Y%m%d").ok()
    } else {
        chrono::NaiveDate::parse_from_str(value, "%G-W%V-%u")
            .or_else(|_| chrono::NaiveDate::parse_from_str(value, "%GW%V%u"))
            .or_else(|_| parse_week_default(value, "-1", "%G-W%V-%u"))
            .or_else(|_| parse_week_default(value, "1", "%GW%V%u"))
            .ok()
    };
    parsed.filter(|d| (1..=9999).contains(&chrono::Datelike::year(d)))
}

fn parse_week_default(
    value: &str,
    suffix: &str,
    format: &str,
) -> std::result::Result<chrono::NaiveDate, chrono::ParseError> {
    let mut bytes = [0; 12];
    let length = value.len() + suffix.len();
    bytes[..value.len()].copy_from_slice(value.as_bytes());
    bytes[value.len()..length].copy_from_slice(suffix.as_bytes());
    chrono::NaiveDate::parse_from_str(
        std::str::from_utf8(&bytes[..length]).expect("ISO ASCII grammar"),
        format,
    )
}

async fn med_id_digits(id: &str, context: Option<Instant>, observer: Observer<'_>) -> Result<bool> {
    if id.is_empty() {
        return Ok(false);
    }
    for (index, point) in id.chars().enumerate() {
        if index % 1024 == 0 {
            if index > 0 {
                observe(observer, Phase::MedDigits);
            }
            row_checkpoint(context).await?;
        }
        if !ssx_application::python_is_digit(point) {
            return Ok(false);
        }
    }
    observe(observer, Phase::MedDigitsComplete);
    Ok(true)
}
