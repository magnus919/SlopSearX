//! NCBI ESearch -> ESummary under one aggregate deadline, including source parsing.
use super::clinicaltrials::*;
use super::source_json::{Text, Value};
use crate::http::AdapterHttp;
use ssx_application::{PortFuture, SearchAdmission, SearchContext, SearchEngine};
use ssx_domain::{AdapterResponse, SearchRequest};
use std::{sync::Arc, time::Instant};
pub struct PubMedAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: BiomedicalConfig,
}
impl PubMedAdapter {
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
        admit_with_context(&self.admission, "pubmed", context).await?;
        let d = network_deadline(&self.config, true, context)?;
        let deadline = Some(d);
        let mut session = session(&self.http, d)?;
        let max = scalar_text(&self.config.max_results);
        let req = request(
            &format!("{}/esearch.fcgi", self.config.base_url),
            &[
                ("db", "pubmed"),
                ("term", &q.query),
                ("retmax", &max),
                ("retmode", "json"),
                ("sort", "relevance"),
            ],
            true,
            false,
        )?;
        let search_data = get_observed(&mut session, req, d, false, false, deadline, observer)
            .await?
            .unwrap();
        let search = search_data
            .as_object()
            .ok_or_else(|| error("ESearch payload must be an object"))?
            .get("esearchresult")
            .and_then(Value::as_object)
            .ok_or_else(|| error("ESearch payload is missing esearchresult"))?;
        let raw_ids = search
            .get("idlist")
            .and_then(Value::as_array)
            .ok_or_else(|| error("ESearch payload is missing idlist"))?;
        let mut ids = OwnedVec::with_capacity(budget, raw_ids.len()).map_err(owned_error)?;
        let mut id_parts = OwnedVec::with_capacity(budget, raw_ids.len()).map_err(owned_error)?;
        for value in raw_ids {
            row_checkpoint(deadline).await?;
            let id = value
                .as_text()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| error("ESearch idlist contains an invalid PMID"))?;
            let scalar = id.as_str().ok_or_else(|| {
                error("source text contains non-scalar Unicode; requires reviewed projection")
            })?;
            ids.try_push(id).map_err(owned_error)?;
            id_parts.try_push(scalar).map_err(owned_error)?;
        }
        if ids.is_empty() {
            return ProducedBatch::new(budget, 0, deadline).map_err(projection_error);
        }
        let joined_ids = owned_join(budget, id_parts.as_slice(), ",", None, deadline).await?;
        let req = request(
            &format!("{}/esummary.fcgi", self.config.base_url),
            &[
                ("db", "pubmed"),
                ("id", joined_ids.as_str()),
                ("retmode", "json"),
            ],
            true,
            false,
        )?;
        drop(joined_ids);
        drop(id_parts);
        let data = get_observed(&mut session, req, d, false, false, deadline, observer)
            .await?
            .unwrap();
        let summary = data
            .as_object()
            .ok_or_else(|| error("ESummary payload must be an object"))?
            .get("result")
            .and_then(Value::as_object)
            .ok_or_else(|| error("ESummary payload is missing result"))?;
        struct Pending<'a> {
            row: Draft,
            id: &'a Text,
            doi: &'a Value,
            pmcid: &'a Value,
            types: &'a Value,
            journal: &'a Value,
            authors: OwnedVec<&'a Text>,
        }
        let mut rows = OwnedVec::with_capacity(budget, ids.len()).map_err(owned_error)?;
        let null = &Value::Null;
        let l = list();
        for (idx, id) in ids.as_slice().iter().enumerate() {
            row_checkpoint(deadline).await?;
            let id_str = id.as_str().expect("validated PMID");
            let Some(article) = summary.get(id_str) else {
                continue;
            };
            if article.as_object().is_none() {
                return Err(id_error(
                    budget,
                    "ESummary entry for PMID ",
                    id_str,
                    " must be an object",
                    deadline,
                )
                .await?);
            }
            let title = optional_borrowed_text(article, "title")?;
            let source = optional_borrowed_text(article, "source")?;
            let date = optional_borrowed_text(article, "pubdate")?;
            let authors_value = field(article, "authors", &l)?;
            let Some(authors) = authors_value.as_array() else {
                return Err(id_error(
                    budget,
                    "ESummary authors for PMID ",
                    id_str,
                    " must be a list",
                    deadline,
                )
                .await?);
            };
            let mut names = OwnedVec::with_capacity(budget, authors.len()).map_err(owned_error)?;
            for (iteration, author) in authors.iter().enumerate() {
                if iteration >= 1024 {
                    observe(observer, Phase::Inner);
                }
                row_checkpoint(deadline).await?;
                if author.as_object().is_none() {
                    return Err(id_error(
                        budget,
                        "ESummary author for PMID ",
                        id_str,
                        " must be an object",
                        deadline,
                    )
                    .await?);
                }
                let name = field(author, "name", null)?;
                if name.is_null() {
                    continue;
                }
                let name = name
                    .as_text()
                    .ok_or_else(|| error("ESummary field name must be text"))?;
                if !name.is_empty() {
                    names.try_push(name).map_err(owned_error)?;
                }
            }
            let mut display =
                OwnedVec::with_capacity(budget, names.len().min(3)).map_err(owned_error)?;
            for name in names.as_slice().iter().take(3) {
                display
                    .try_push(name.as_str().ok_or_else(|| {
                        error(
                            "source text contains non-scalar Unicode; requires reviewed projection",
                        )
                    })?)
                    .map_err(owned_error)?;
            }
            let display_names =
                owned_join(budget, display.as_slice(), ", ", None, deadline).await?;
            let content = if names.is_empty() {
                owned_join(budget, &[source], "", Some(500), deadline).await?
            } else {
                owned_join(
                    budget,
                    &[source, display_names.as_str()],
                    " — ",
                    Some(500),
                    deadline,
                )
                .await?
            };
            let url = owned_join(
                budget,
                &["https://pubmed.ncbi.nlm.nih.gov/", id_str, "/"],
                "",
                None,
                deadline,
            )
            .await?;
            let title = owned_join(budget, &[title], "", None, deadline).await?;
            let published_date = if date.is_empty() {
                None
            } else {
                Some(owned_join(budget, &[date], "", None, deadline).await?)
            };
            let (mut doi, mut pmcid) = (null, null);
            if let Some(article_ids) = field(article, "articleids", &l)?.as_array() {
                for item in article_ids {
                    row_checkpoint(deadline).await?;
                    if let Some(item) = item.as_object() {
                        let key = item.get("idtype");
                        if key.is_some_and(|k| matches!(k, Value::Array(_) | Value::Object(_))) {
                            return Err(error("unhashable article ID type"));
                        }
                        if key.and_then(Value::as_str) == Some("doi") {
                            doi = item.get("value").unwrap_or(null);
                        }
                        if key.and_then(Value::as_str) == Some("pmc") {
                            pmcid = item.get("value").unwrap_or(null);
                        }
                    }
                }
            }
            let types = field(article, "pubtype", &l)?;
            let types = if types.is_array() { types } else { null };
            let journal = if source.is_empty() {
                null
            } else {
                field(article, "source", null)?
            };
            rows.try_push(Pending {
                row: Draft {
                    url,
                    title,
                    content,
                    published_date,
                    position: idx as i64 + 1,
                    score: 1.,
                },
                id,
                doi,
                pmcid,
                types,
                journal,
                authors: names,
            })
            .map_err(owned_error)?;
        }
        let mut batch =
            ProducedBatch::new(budget, rows.len(), deadline).map_err(projection_error)?;
        for pending in rows.as_slice() {
            observe(observer, Phase::Payload);
            let authors = if pending.authors.is_empty() {
                FieldRef::Value(null)
            } else {
                FieldRef::SelectedTexts(pending.authors.as_slice())
            };
            let fields = [
                Field {
                    name: "publication_id",
                    value: FieldRef::Text(pending.id),
                },
                Field {
                    name: "pmid",
                    value: FieldRef::Text(pending.id),
                },
                Field {
                    name: "doi",
                    value: FieldRef::Value(pending.doi),
                },
                Field {
                    name: "pmcid",
                    value: FieldRef::Value(pending.pmcid),
                },
                Field {
                    name: "publication_types",
                    value: FieldRef::Value(pending.types),
                },
                Field {
                    name: "journal",
                    value: FieldRef::Value(pending.journal),
                },
                Field {
                    name: "authors",
                    value: authors,
                },
            ];
            let payload = publication_budgeted("pubmed", &fields, budget, deadline)
                .await
                .map_err(projection_error)?;
            batch
                .push(pending.row.skeleton("pubmed"), Some(payload))
                .await
                .map_err(projection_error)?;
        }
        Ok(batch)
    }
}
fn optional_borrowed_text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    let null = &Value::Null;
    let field = field(value, key, null)?;
    if field.is_null() {
        Ok("")
    } else {
        if field.as_text().is_none() {
            return Err(error(format!("ESummary field {key} must be text")));
        }
        borrowed_text(field)
    }
}
impl SearchEngine for PubMedAdapter {
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
