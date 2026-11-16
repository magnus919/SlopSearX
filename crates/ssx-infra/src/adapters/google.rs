//! Google HTML adapter. Provider HTML and source order are preserved, including
//! source's permissive `http` prefix and position gaps after skipped nodes.
use super::duckduckgo::{
    ScrapeConfig, ScrapeRuntime, elapsed, headers, http_error, python_form, select,
    select_with_context,
};
use crate::http::{AdapterHttp, Cancellation, Error as HttpError, Options, Request};
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchEngine};
use ssx_domain::{AdapterResponse, EngineStatus, SearchRequest};
use std::{sync::Arc, time::Instant};

#[derive(Clone, Debug)]
pub struct GoogleConfig {
    pub base_url: String,
    pub timeout_ms: ssx_config::ConfigScalar,
    pub max_results: ssx_json::Value,
    pub allocation_budget_bytes: usize,
    pub scrape: ScrapeConfig,
}
impl Default for GoogleConfig {
    fn default() -> Self {
        Self {
            base_url: "https://www.google.com/search".into(),
            timeout_ms: 10_000.into(),
            max_results: ssx_json::json!(10),
            allocation_budget_bytes: crate::html_budget::MAX_SEARCH_BYTES,
            scrape: ScrapeConfig::default(),
        }
    }
}
pub(super) struct GoogleAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: GoogleConfig,
    runtime: ScrapeRuntime,
    parser: Arc<crate::html::ParserPool>,
}
impl GoogleAdapter {
    pub(super) fn with_parser(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: GoogleConfig,
        parser: Arc<crate::html::ParserPool>,
    ) -> Self {
        let runtime = ScrapeRuntime::new(&config.scrape);
        Self {
            http,
            admission,
            config,
            runtime,
            parser,
        }
    }
    #[cfg(test)]
    pub(super) fn test_feedback(&self) -> Vec<bool> {
        self.runtime.test_events()
    }
    async fn run(
        &self,
        input: &SearchRequest,
        context: Option<ssx_application::SearchContext>,
    ) -> AdapterResponse {
        let started = Instant::now();
        let response = |status, results, error_message| AdapterResponse {
            status,
            results,
            error_message,
            latency_ms: elapsed(started),
            ..AdapterResponse::default()
        };
        match self.admission.admit("google").await {
            AdmissionDecision::Allowed => {}
            AdmissionDecision::Denied => return response(EngineStatus::RateLimited, vec![], None),
            AdmissionDecision::Unavailable => {
                return response(
                    EngineStatus::Error,
                    vec![],
                    Some("rate policy unavailable".into()),
                );
            }
        }
        let deadline = match super::html_timeout::deadline(&self.config.timeout_ms, context) {
            Ok(deadline) => deadline,
            Err(error) => {
                return response(EngineStatus::Error, vec![], Some(error.message().into()));
            }
        };
        let operation = match crate::html_budget::Operation::start(
            self.config.allocation_budget_bytes,
            input.query.capacity(),
            deadline,
        )
        .await
        {
            Ok(operation) => operation,
            Err(crate::html_budget::OperationError::Deadline) => {
                return response(EngineStatus::Timeout, vec![], None);
            }
            Err(error) => {
                return response(EngineStatus::Error, vec![], Some(error.message().into()));
            }
        };
        let proxy = self.runtime.select();
        let mut outbound = match Request::get(&self.config.base_url) {
            Ok(r) => r,
            Err(_) => {
                return response(
                    EngineStatus::Error,
                    vec![],
                    Some("invalid Google base URL".into()),
                );
            }
        };
        let num = ssx_domain::storage::python_value_string(&self.config.max_results);
        let size = match super::duckduckgo::python_form_size(&[
            ("q", &input.query),
            ("hl", "en"),
            ("num", &num),
        ]) {
            Some(size) => size,
            None => {
                return response(
                    EngineStatus::Error,
                    vec![],
                    Some(crate::html_budget::RESOURCE_ERROR.into()),
                );
            }
        };
        let _request_charge = match operation.budget.charge(size) {
            Ok(charge) => charge,
            Err(error) => return response(EngineStatus::Error, vec![], Some(error.into())),
        };
        let encoded = match python_form(&[("q", &input.query), ("hl", "en"), ("num", &num)]) {
            Ok(encoded) => encoded,
            Err(error) => return response(EngineStatus::Error, vec![], Some(error)),
        };
        outbound.url.set_query(Some(&encoded));
        outbound.headers = headers();
        let options = Options {
            proxy: proxy.clone(),
            follow_redirects: true,
            ..Options::default()
        };
        let deadline = operation.deadline;
        let mut session = match self
            .http
            .session(options, deadline, Cancellation::default())
        {
            Ok(s) => s,
            Err(_) => {
                return response(
                    EngineStatus::Error,
                    vec![],
                    Some("HTTP session unavailable".into()),
                );
            }
        };
        let resp = match session.execute(outbound).await {
            Ok(r) => r,
            Err(HttpError::Deadline) => return response(EngineStatus::Timeout, vec![], None),
            Err(e) => return response(EngineStatus::Error, vec![], Some(format!("{e:?}"))),
        };
        if [429, 403, 503].contains(&resp.status) {
            self.runtime.feedback(proxy.as_deref(), false);
            return response(
                if resp.status == 429 {
                    EngineStatus::RateLimited
                } else {
                    EngineStatus::Blocked
                },
                vec![],
                None,
            );
        }
        if !(200..300).contains(&resp.status) {
            return response(EngineStatus::Error, vec![], Some(http_error(&resp)));
        }
        let body = match super::duckduckgo::decode(&self.parser, resp, operation.clone()).await {
            Ok(body) => body,
            Err(error) => {
                let (status, message) = helper_failure(error);
                return response(status, vec![], message);
            }
        };
        if body.flags.google_challenge {
            self.runtime.feedback(proxy.as_deref(), false);
            return response(EngineStatus::Blocked, vec![], None);
        }
        self.runtime.feedback(proxy.as_deref(), true);
        match parse(
            &self.parser,
            &body.text,
            &self.config.max_results,
            operation.clone(),
        )
        .await
        {
            Ok(rows) => rows.into_adapter_response(elapsed(started)),
            Err(e) => {
                let (status, message) = helper_failure(e);
                response(status, vec![], message)
            }
        }
    }
}
fn helper_failure(error: crate::html::OperationFailure) -> (EngineStatus, Option<String>) {
    match error {
        crate::html::OperationFailure::Deadline => (EngineStatus::Timeout, None),
        error => (EngineStatus::Error, Some(error.to_string())),
    }
}
impl SearchEngine for GoogleAdapter {
    fn search<'a>(&'a self, r: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.run(r, None))
    }
    fn search_with_context<'a>(
        &'a self,
        r: &'a SearchRequest,
        context: ssx_application::SearchContext,
    ) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.run(r, Some(context)))
    }
}
pub(super) const GOOGLE_CHALLENGE: &[&str] = &[
    "challenge",
    "recaptcha",
    "cf-browser-verification",
    "unusual traffic",
    "verify you're not a robot",
    "g-recaptcha",
];
pub(super) fn challenge(raw: &ssx_source_codecs::Text) -> bool {
    GOOGLE_CHALLENGE.iter().any(|s| raw.contains_ascii_lower(s))
}
async fn parse(
    parser: &crate::html::ParserPool,
    raw: &ssx_source_codecs::BudgetText,
    max: &ssx_json::Value,
    operation: Arc<crate::html_budget::Operation>,
) -> Result<crate::html::ProducedHtmlBatch, crate::html::OperationFailure> {
    parser
        .project_rows_with_operation(
            raw.codepoints(),
            crate::html::row_wire::Layout::Google,
            super::RowLimit::from_value(max),
            "",
            operation,
        )
        .await
}
pub(super) fn extract_tree(
    doc: &crate::html::Html,
    max: super::RowLimit,
    budget: &ssx_owned_budget::Budget,
) -> Result<ssx_owned_budget::OwnedVec<crate::html::child_rows::Row>, crate::html::OperationFailure>
{
    use crate::html::{
        OperationFailure as Failure,
        child_rows::{Row, optional_text, owned},
    };
    let mut rows = ssx_owned_budget::OwnedVec::new(budget).map_err(|_| Failure::Resource)?;
    // Keys borrow the affine tree. The sorted charged index owns no cloned URL
    // strings or private hash-table backing; each insertion keeps old+new peak.
    let mut seen =
        ssx_owned_budget::OwnedVec::<&str>::new(budget).map_err(|_| Failure::Resource)?;
    for (i, node) in doc.select(&select("div.g")).enumerate() {
        if max.reached(rows.len()).map_err(Failure::Parse)? {
            break;
        }
        let Some(link) = select_with_context(node, "a[href^='http']").next() else {
            continue;
        };
        let url = link.value().attr("href").unwrap_or("");
        if url.is_empty() {
            continue;
        }
        let index = match seen.as_slice().binary_search(&url) {
            Ok(_) => continue,
            Err(index) => index,
        };
        seen.try_push(url).map_err(|_| Failure::Resource)?;
        seen.as_mut_slice()[index..].rotate_right(1);
        rows.try_push(Row {
            url: owned(budget, url)?,
            title: optional_text(budget, select_with_context(node, "h3").next())?,
            content: optional_text(
                budget,
                select_with_context(node, "span.aCOpRe, div.VwiC3b, span.st").next(),
            )?,
            position: i64::try_from(i + 1).map_err(|_| Failure::Resource)?,
            img_src: None,
            media: None,
        })
        .map_err(|_| Failure::Resource)?;
    }
    Ok(rows)
}
