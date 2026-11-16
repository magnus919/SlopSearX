//! PubChem name-to-CID then compound-details search.
use super::clinicaltrials::*;
use super::source_json::{Number, Value};
use crate::http::AdapterHttp;
use ssx_application::{PortFuture, SearchAdmission, SearchContext, SearchEngine};
use ssx_domain::{AdapterResponse, SearchRequest};
use std::{sync::Arc, time::Instant};
pub struct PubChemAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: BiomedicalConfig,
}
impl PubChemAdapter {
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
        admit_with_context(&self.admission, "pubchem", context).await?;
        let d = network_deadline_for_http(&self.http, &self.config, context)?;
        let quoted = owned_quote(budget, &q.query, true, context).await?;
        let url = owned_join(
            budget,
            &[
                &self.config.base_url,
                "/compound/name/",
                quoted.as_str(),
                "/cids/JSON",
            ],
            "",
            None,
            context,
        )
        .await?;
        let mut session = session(&self.http, d)?;
        let Some(data) = get_observed(
            &mut session,
            request(url.as_str(), &[], false, false)?,
            d,
            true,
            false,
            context,
            observer,
        )
        .await?
        else {
            return ProducedBatch::new(budget, 0, context).map_err(projection_error);
        };
        drop(url);
        drop(quoted);
        let m = map();
        let l = list();
        let raw_ids = field(field(&data, "IdentifierList", &m)?, "CID", &l)?;
        if !truthy(raw_ids) {
            return ProducedBatch::new(budget, 0, context).map_err(projection_error);
        }
        let mut ids = OwnedVec::new(budget).map_err(owned_error)?;
        match raw_ids {
            Value::Array(values) => {
                let end = slice_end(&self.config.max_results, values.len())?;
                ids.try_reserve_exact(end).map_err(owned_error)?;
                for value in &values[..end] {
                    row_checkpoint(context).await?;
                    ids.try_push(owned_stringify(budget, value, context).await?)
                        .map_err(owned_error)?;
                }
            }
            Value::String(text) => {
                let end = slice_end(&self.config.max_results, text.len())?;
                ids.try_reserve_exact(end).map_err(owned_error)?;
                for point in &text.codepoints()[..end] {
                    row_checkpoint(context).await?;
                    let c = char::from_u32(*point).ok_or_else(|| {
                        error("source stringification contains non-scalar Unicode")
                    })?;
                    let mut buf = [0; 4];
                    ids.try_push(
                        owned_join(budget, &[c.encode_utf8(&mut buf)], "", None, context).await?,
                    )
                    .map_err(owned_error)?;
                }
            }
            _ => return Err(error("CID collection does not support slicing")),
        }
        let mut parts = OwnedVec::with_capacity(budget, ids.len()).map_err(owned_error)?;
        for (index, id) in ids.as_slice().iter().enumerate() {
            if index % 1024 == 0 {
                if index > 0 {
                    observe(observer, Phase::CidParts);
                }
                row_checkpoint(context).await?;
            }
            parts.try_push(id.as_str()).map_err(owned_error)?;
        }
        observe(observer, Phase::CidPartsComplete);
        let joined = owned_join(budget, parts.as_slice(), ",", None, context).await?;
        let url = owned_join(
            budget,
            &[
                &self.config.base_url,
                "/compound/cid/",
                joined.as_str(),
                "/JSON",
            ],
            "",
            None,
            context,
        )
        .await?;
        let d = network_deadline_for_http(&self.http, &self.config, context)?;
        session.set_deadline(tokio::time::Instant::from_std(d));
        let req = request(url.as_str(), &[], false, false)?;
        drop(url);
        drop(joined);
        drop(parts);
        drop(ids);
        let detail = get_observed(&mut session, req, d, false, false, context, observer)
            .await?
            .unwrap();
        let compounds = result_rows(
            field(&detail, "PC_Compounds", &l)?,
            &self.config.max_results,
        )?;
        let mut batch =
            ProducedBatch::new(budget, compounds.len(), context).map_err(projection_error)?;
        for (idx, compound) in compounds.iter().enumerate() {
            row_checkpoint(context).await?;
            let e = empty();
            let cid = owned_stringify(
                budget,
                field(field(field(compound, "id", &m)?, "id", &m)?, "cid", &e)?,
                context,
            )
            .await?;
            let props = field(compound, "props", &l)?;
            let props = match props {
                Value::Array(v) => v.as_slice(),
                Value::String(v) if v.is_empty() => &[],
                Value::Object(v) if v.is_empty() => &[],
                _ => return Err(error("source property element does not support get")),
            };
            let (mut name, mut formula, mut mass) = (
                OwnedString::new(budget).map_err(owned_error)?,
                OwnedString::new(budget).map_err(owned_error)?,
                OwnedString::new(budget).map_err(owned_error)?,
            );
            for (iteration, prop) in props.iter().enumerate() {
                if iteration >= 1024 {
                    observe(observer, Phase::Inner);
                }
                row_checkpoint(context).await?;
                let urn = field(prop, "urn", &m)?;
                let label = field(urn, "label", &e)?;
                let pname = field(urn, "name", &e)?;
                let value = field(prop, "value", &m)?;
                if label.as_str() == Some("IUPAC Name") && pname.as_str() == Some("Preferred") {
                    let v = field(value, "sval", &e)?;
                    if truthy(v) {
                        name = owned_join(budget, &[borrowed_text(v)?], "", None, context).await?;
                    }
                } else if label.as_str() == Some("Molecular Formula") {
                    let v = field(value, "sval", &e)?;
                    if truthy(v) {
                        formula = owned_stringify(budget, v, context).await?;
                    }
                } else if label.as_str() == Some("Molecular Weight") {
                    let v = field(value, "fval", &Value::Null)?;
                    if truthy(v) {
                        let n = match v {
                            Value::Bool(v) => {
                                if *v {
                                    1.
                                } else {
                                    0.
                                }
                            }
                            Value::Number(Number::Float(n)) => *n,
                            Value::Number(n) => n
                                .as_f64()
                                .ok_or_else(|| error("molecular weight overflow"))?,
                            _ => return Err(error("molecular weight does not support .2f")),
                        };
                        if !n.is_finite() {
                            mass = owned_join(
                                budget,
                                &[if n.is_nan() {
                                    "nan"
                                } else if n > 0. {
                                    "inf"
                                } else {
                                    "-inf"
                                }],
                                "",
                                None,
                                context,
                            )
                            .await?;
                        } else {
                            mass = fixed_format(budget, 1024, format_args!("{n:.2}"))?;
                        }
                    }
                }
            }
            if name.is_empty() {
                name =
                    owned_join(budget, &["Compound CID ", cid.as_str()], "", None, context).await?;
            }
            let formula_part = if formula.is_empty() {
                OwnedString::new(budget).map_err(owned_error)?
            } else {
                owned_join(budget, &["Formula: ", formula.as_str()], "", None, context).await?
            };
            let mass_part = if mass.is_empty() {
                OwnedString::new(budget).map_err(owned_error)?
            } else {
                owned_join(budget, &["MW: ", mass.as_str()], "", None, context).await?
            };
            let content = if formula_part.is_empty() && mass_part.is_empty() {
                owned_join(budget, &["Chemical compound"], "", None, context).await?
            } else if formula_part.is_empty() {
                owned_join(budget, &[mass_part.as_str()], "", Some(500), context).await?
            } else if mass_part.is_empty() {
                owned_join(budget, &[formula_part.as_str()], "", Some(500), context).await?
            } else {
                owned_join(
                    budget,
                    &[formula_part.as_str(), mass_part.as_str()],
                    " | ",
                    Some(500),
                    context,
                )
                .await?
            };
            let url = owned_join(
                budget,
                &["https://pubchem.ncbi.nlm.nih.gov/compound/", cid.as_str()],
                "",
                None,
                context,
            )
            .await?;
            let draft = Draft {
                url,
                title: name,
                content,
                published_date: None,
                position: idx as i64 + 1,
                score: 1.,
            };
            batch
                .push(draft.skeleton("pubchem"), None)
                .await
                .map_err(projection_error)?;
        }
        Ok(batch)
    }
}
impl SearchEngine for PubChemAdapter {
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
