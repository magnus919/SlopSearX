//! UniProt bounded overfetch and stable exact identifier/gene promotion.
use super::clinicaltrials::*;
use super::source_json::Value;
use crate::http::AdapterHttp;
use ssx_application::{PortFuture, SearchAdmission, SearchContext, SearchEngine};
use ssx_config::ConfigScalar;
use ssx_domain::{AdapterResponse, SearchRequest};
use std::{sync::Arc, time::Instant};
pub struct UniProtAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: BiomedicalConfig,
}
impl UniProtAdapter {
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
        admit_with_context(&self.admission, "uniprot", context).await?;
        let candidate = candidate(&self.config.max_results)?;
        let d = network_deadline_for_http(&self.http, &self.config, context)?;
        let plan = query_plan(&q.query);
        let intent = plan.intent;
        let upstream = if let Some(identifier) = plan.expanded {
            owned_join(
                budget,
                &[
                    "(",
                    plan.source,
                    ") OR (",
                    if identifier { "accession" } else { "gene" },
                    ":",
                    plan.source,
                    ")",
                ],
                "",
                None,
                context,
            )
            .await?
        } else {
            owned_join(budget, &[plan.source], "", None, context).await?
        };
        let size = scalar_text(&candidate);
        let req = request(
            &self.config.base_url,
            &[
                ("query", upstream.as_str()),
                ("size", &size),
                ("format", "json"),
            ],
            true,
            true,
        )?;
        let mut session = session(&self.http, d)?;
        let data = get_observed(&mut session, req, d, false, false, context, observer)
            .await?
            .unwrap();
        let l = list();
        let proteins = result_rows(field(&data, "results", &l)?, &candidate)?;
        let mut ranked = OwnedVec::with_capacity(budget, proteins.len()).map_err(owned_error)?;
        for (index, protein) in proteins.iter().enumerate() {
            row_checkpoint(context).await?;
            ranked
                .try_push((
                    rank_owned(protein, intent.as_ref(), budget, context, observer).await?,
                    index,
                    protein,
                ))
                .map_err(owned_error)?;
        }
        // Source ranks are only0/1/2. Evaluate every source rank first, then
        // stably partition under the same ledger instead of an unyielding sort.
        let mut ordered = OwnedVec::with_capacity(budget, ranked.len()).map_err(owned_error)?;
        for wanted in 0..=2 {
            for (index, entry) in ranked.as_slice().iter().enumerate() {
                if index % 1024 == 0 {
                    if index > 0 {
                        observe(observer, Phase::RankPartition);
                    }
                    row_checkpoint(context).await?;
                }
                if entry.0 == wanted {
                    ordered.try_push(*entry).map_err(owned_error)?;
                }
            }
        }
        observe(observer, Phase::RankPartitionComplete);
        drop(ranked);
        let ranked = ordered;
        let end = slice_end(&self.config.max_results, ranked.len())?;
        let mut batch = ProducedBatch::new(budget, end, context).map_err(projection_error)?;
        for (position, (rank, _, protein)) in ranked.as_slice()[..end].iter().enumerate() {
            row_checkpoint(context).await?;
            let e = empty();
            let m = map();
            let accession = field(protein, "primaryAccession", &e)?;
            let id = field(protein, "uniProtkbId", &e)?;
            let description = field(protein, "proteinDescription", &m)?;
            let recommended = field(description, "recommendedName", &m)?;
            let recommended = if truthy(recommended) { recommended } else { &m };
            let full = field(field(recommended, "fullName", &m)?, "value", &e)?;
            let full = if truthy(full) { full } else { &e };
            let organism = field(protein, "organism", &m)?;
            let organism = if truthy(organism) { organism } else { &m };
            let organism = field(organism, "scientificName", &e)?;
            let genes = gene_names_owned(protein, budget, context, observer).await?;
            let mut parts = OwnedVec::with_capacity(budget, 2).map_err(owned_error)?;
            if truthy(organism) {
                parts
                    .try_push(borrowed_text(organism)?)
                    .map_err(owned_error)?;
            }
            let gene_part = if let Some(gene) = genes.as_slice().first().filter(|v| !v.is_empty()) {
                owned_join(budget, &["Gene: ", gene], "", None, context).await?
            } else {
                OwnedString::new(budget).map_err(owned_error)?
            };
            if !gene_part.is_empty() {
                parts.try_push(gene_part.as_str()).map_err(owned_error)?;
            }
            let content = if parts.is_empty() {
                owned_join(budget, &["Protein"], "", None, context).await?
            } else {
                owned_join(budget, parts.as_slice(), " — ", Some(500), context).await?
            };
            let label = if truthy(full) {
                full
            } else if truthy(id) {
                id
            } else {
                accession
            };
            let accession_text = owned_stringify(budget, accession, context).await?;
            let title = if truthy(full) {
                let label = owned_stringify(budget, label, context).await?;
                owned_join(
                    budget,
                    &[accession_text.as_str(), label.as_str()],
                    " — ",
                    None,
                    context,
                )
                .await?
            } else {
                owned_join(budget, &[borrowed_text(label)?], "", None, context).await?
            };
            let url = owned_join(
                budget,
                &[
                    "https://www.uniprot.org/uniprotkb/",
                    accession_text.as_str(),
                    "/entry",
                ],
                "",
                None,
                context,
            )
            .await?;
            let draft = Draft {
                url,
                title,
                content,
                published_date: None,
                position: position as i64 + 1,
                score: 3. - f64::from(*rank),
            };
            batch
                .push(draft.skeleton("uniprot"), None)
                .await
                .map_err(projection_error)?;
        }
        Ok(batch)
    }
}
impl SearchEngine for UniProtAdapter {
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
fn candidate(v: &ConfigScalar) -> Result<ConfigScalar> {
    match v {
        ConfigScalar::Boolean(v) => Ok(ConfigScalar::Integer(if *v { 3 } else { 0 })),
        ConfigScalar::Integer(v) => v
            .checked_mul(2)
            .and_then(|n| v.checked_add(n.min(40)))
            .map(ConfigScalar::Integer)
            .ok_or_else(|| error("candidate size overflow")),
        ConfigScalar::Float(v) => Ok(ConfigScalar::Float(v + (v * 2.).min(40.))),
        _ => Err(error("candidate size arithmetic requires number")),
    }
}
async fn gene_names_owned<'a>(
    protein: &'a Value,
    budget: &Budget,
    deadline: Option<Instant>,
    observer: Observer<'_>,
) -> Result<OwnedVec<&'a str>> {
    let mut names = OwnedVec::new(budget).map_err(owned_error)?;
    let Some(genes) = object(protein)?.get("genes") else {
        return Ok(names);
    };
    if !truthy(genes) || matches!(genes, Value::String(_) | Value::Object(_)) {
        return Ok(names);
    }
    for gene in array(genes)? {
        row_checkpoint(deadline).await?;
        let Some(gene) = gene.as_object() else {
            continue;
        };
        for key in ["geneName", "orderedLocusName", "orfName"] {
            if let Some(v) = gene
                .get(key)
                .and_then(Value::as_object)
                .and_then(|v| v.get("value"))
                .and_then(Value::as_str)
            {
                names.try_push(v).map_err(owned_error)?;
            }
        }
        if let Some(synonyms) = gene.get("synonyms").filter(|v| truthy(v)) {
            if matches!(synonyms, Value::String(_) | Value::Object(_)) {
                continue;
            }
            for (iteration, synonym) in array(synonyms)?.iter().enumerate() {
                if iteration >= 1024 {
                    observe(observer, Phase::Inner);
                }
                row_checkpoint(deadline).await?;
                if let Some(v) = synonym
                    .as_object()
                    .and_then(|v| v.get("value"))
                    .and_then(Value::as_str)
                {
                    names.try_push(v).map_err(owned_error)?;
                }
            }
        }
    }
    Ok(names)
}
async fn casefold_owned(
    value: &str,
    budget: &Budget,
    deadline: Option<Instant>,
) -> Result<OwnedString> {
    let mut length = 0usize;
    for (i, ch) in value.chars().enumerate() {
        if i % 1024 == 0 {
            row_checkpoint(deadline).await?;
        }
        length = length
            .checked_add(ssx_json::casefold_char(ch).map_or(ch.len_utf8(), str::len))
            .ok_or_else(resource)?;
    }
    let mut out = OwnedString::new(budget).map_err(owned_error)?;
    out.try_reserve_exact(length).map_err(owned_error)?;
    for (i, ch) in value.chars().enumerate() {
        if i % 1024 == 0 {
            row_checkpoint(deadline).await?;
        }
        if let Some(folded) = ssx_json::casefold_char(ch) {
            out.try_push_str(folded).map_err(owned_error)?;
        } else {
            out.try_push(ch).map_err(owned_error)?;
        }
    }
    Ok(out)
}
async fn rank_owned(
    protein: &Value,
    intent: Option<&(bool, &str)>,
    budget: &Budget,
    deadline: Option<Instant>,
    observer: Observer<'_>,
) -> Result<u8> {
    let Some((identifier, value)) = intent else {
        return Ok(2);
    };
    let needle = casefold_owned(value, budget, deadline).await?;
    if *identifier {
        let e = empty();
        for key in ["primaryAccession", "uniProtkbId"] {
            let value = owned_stringify(budget, field(protein, key, &e)?, deadline).await?;
            if casefold_owned(value.as_str(), budget, deadline)
                .await?
                .as_str()
                == needle.as_str()
            {
                return Ok(0);
            }
        }
    } else {
        let genes = gene_names_owned(protein, budget, deadline, observer).await?;
        for gene in genes.as_slice() {
            if casefold_owned(gene, budget, deadline).await?.as_str() == needle.as_str() {
                return Ok(1);
            }
        }
    }
    Ok(2)
}
fn ascii_id(c: char) -> bool {
    c.is_ascii_alphanumeric() || "._-".contains(c)
}
fn regex_lower(c: char) -> char {
    match c {
        'İ' | 'ı' => 'i',
        'ſ' => 's',
        'K' => 'k',
        c => c.to_ascii_lowercase(),
    }
}
fn field_value(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|c| regex_lower(c).is_ascii_alphanumeric())
        && chars.all(|c| ascii_id(regex_lower(c)))
}
fn accession(value: &str) -> bool {
    let mut pieces = value.split('-');
    let root = pieces.next().unwrap_or("");
    if let Some(version) = pieces.next()
        && (version.is_empty()
            || !version.chars().all(ssx_json::decimal_digit)
            || pieces.next().is_some())
    {
        return false;
    }
    if !matches!(root.chars().count(), 6 | 10) {
        return false;
    }
    let mut chars = root.chars().map(regex_lower);
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.next().is_some_and(|c| c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_alphanumeric())
}
struct QueryPlan<'a> {
    source: &'a str,
    intent: Option<(bool, &'a str)>,
    expanded: Option<bool>,
}
fn query_plan(query: &str) -> QueryPlan<'_> {
    let stripped = query.trim_matches(ssx_json::python_whitespace);
    if let Some((field, value)) = stripped.split_once(':') {
        let field = field.trim_end_matches(ssx_json::python_whitespace);
        let is_gene = field.chars().map(regex_lower).eq("gene".chars());
        let accepted = is_gene
            || field.chars().map(regex_lower).eq("accession".chars())
            || field.chars().map(regex_lower).eq("id".chars());
        let value = value.trim_start_matches(ssx_json::python_whitespace);
        if accepted && field_value(value) {
            return QueryPlan {
                source: stripped,
                intent: Some((!is_gene, value)),
                expanded: None,
            };
        }
    }
    let length = stripped.chars().take(65).count();
    let mut chars = stripped.chars();
    if !(2..=64).contains(&length)
        || !chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        || !chars.all(ascii_id)
    {
        return QueryPlan {
            source: query,
            intent: None,
            expanded: None,
        };
    }
    let identifier = accession(stripped);
    QueryPlan {
        source: stripped,
        intent: Some((identifier, stripped)),
        expanded: Some(identifier),
    }
}
