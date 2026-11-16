//! MITRE ATT&CK direct ID lookup and source-order HTML extraction.
use super::{clinicaltrials as c, vulnerability as v};
use crate::{
    html::{ParserPool, row_wire::Layout},
    http::AdapterHttp,
};
use ssx_application::{PortFuture, SearchAdmission, SearchContext, SearchEngine};
use ssx_domain::{AdapterResponse, SearchRequest};
use std::{sync::Arc, time::Instant};
pub const ENDPOINT: &str = "https://attack.mitre.org";
#[derive(Clone, Debug)]
pub struct MitreAttackConfig {
    pub settings: v::Settings,
    pub allocation_budget_bytes: usize,
}
impl Default for MitreAttackConfig {
    fn default() -> Self {
        Self {
            settings: v::defaults(ENDPOINT),
            allocation_budget_bytes: crate::html_budget::MAX_SEARCH_BYTES,
        }
    }
}
pub struct MitreAttackAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: MitreAttackConfig,
    parser: Arc<ParserPool>,
}
impl MitreAttackAdapter {
    pub fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: MitreAttackConfig,
    ) -> Self {
        Self::with_parser(http, admission, config, crate::html::shared_pool())
    }
    pub fn with_parser(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: MitreAttackConfig,
        parser: Arc<ParserPool>,
    ) -> Self {
        Self {
            http,
            admission,
            config,
            parser,
        }
    }
    async fn run(&self, q: &SearchRequest, context: Option<Instant>) -> AdapterResponse {
        let start = Instant::now();
        let result = async {
            c::admit_with_context(&self.admission, "mitreattack", context).await?;
            let operation = v::html_operation(
                &self.config.settings,
                self.config.allocation_budget_bytes,
                q,
                context,
            )
            .await?;
            let context = Some(operation.deadline.into_std());
            let b = &operation.budget.0;
            let Some((raw, layout, path)) = lookup(&q.query, context).await? else {
                return v::Batch::new(b, 0, context)?
                    .rows
                    .into_adapter_response(super::source_payload::Outcome {
                        status: ssx_domain::EngineStatus::Ok,
                        error_message: None,
                        latency_ms: start.elapsed().as_secs_f64() * 1000.,
                        synthetic: false,
                    })
                    .await
                    .map_err(|e| v::error(format!("MITRE projection: {e:?}")));
            };
            let id = v::upper(raw, b, context).await?;
            let url = v::join(
                b,
                &[
                    self.config.settings.base_url.as_str(),
                    "/",
                    path,
                    "/",
                    id.as_str(),
                    "/",
                ],
                "",
                context,
            )
            .await?;
            let mut req = c::request(url.as_str(), &[], false, false)?;
            v::browser_headers(&mut req, false);
            let response = v::response(&self.http, req, operation.deadline.into_std(), true, false)
                .await?
                .expect("MITRE no404empty branch");
            let decoded = super::duckduckgo::decode(&self.parser, response, operation.clone())
                .await
                .map_err(v::html_error)?;
            let batch = if decoded.text.codepoints().is_empty() {
                v::Batch::new(b, 0, context)?
            } else {
                v::project_html(
                    &self.parser,
                    decoded.text.codepoints(),
                    (layout, super::RowLimit::SliceAll, id.as_str()),
                    operation.clone(),
                    b,
                    context,
                )
                .await?
            };
            // Reply/tree charges survive normalization until this explicit public handoff.
            let response = super::catalog_media::finish(Ok(batch), start).await;
            drop(decoded);
            Ok(response)
        }
        .await;
        match result {
            Ok(response) => response,
            Err(error) => super::catalog_media::finish(Err(error), start).await,
        }
    }
}
impl SearchEngine for MitreAttackAdapter {
    fn search<'a>(&'a self, q: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.run(q, None))
    }
    fn search_with_context<'a>(
        &'a self,
        q: &'a SearchRequest,
        context: SearchContext,
    ) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.run(q, Some(context.deadline)))
    }
}
/// Python re.IGNORECASE also accepts long s for the software ASCII S pattern.
pub(super) async fn lookup(
    query: &str,
    context: Option<Instant>,
) -> v::Result<Option<(&str, Layout, &'static str)>> {
    for (prefix, layout, path) in [
        ('T', Layout::MitreTechnique, "techniques"),
        ('G', Layout::MitreGroup, "groups"),
        ('S', Layout::MitreSoftware, "software"),
    ] {
        for (n, (at, ch)) in query.char_indices().enumerate() {
            if n % 1024 == 0 {
                c::row_checkpoint(context).await?;
            }
            if !ch.eq_ignore_ascii_case(&prefix) && !(prefix == 'S' && ch == 'ſ') {
                continue;
            }
            let mut end = at + ch.len_utf8();
            let mut valid = true;
            for _ in 0..4 {
                if let Some(ch) = query[end..]
                    .chars()
                    .next()
                    .filter(|c| ssx_json::decimal_digit(*c))
                {
                    end += ch.len_utf8();
                } else {
                    valid = false;
                    break;
                }
            }
            if !valid {
                continue;
            }
            if prefix == 'T' && query[end..].starts_with('.') {
                let mut sub = end + 1;
                for _ in 0..3 {
                    if let Some(ch) = query[sub..]
                        .chars()
                        .next()
                        .filter(|c| ssx_json::decimal_digit(*c))
                    {
                        sub += ch.len_utf8();
                    } else {
                        valid = false;
                        break;
                    }
                }
                if valid {
                    end = sub;
                }
            }
            return Ok(Some((&query[at..end], layout, path)));
        }
    }
    Ok(None)
}
