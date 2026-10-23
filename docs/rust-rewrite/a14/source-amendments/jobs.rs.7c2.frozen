//! Public ATS job-board adapters and their shared company-targeting utility.
use super::{
    source_json::{self, Map, Text, Value},
    source_payload,
};
use crate::http::Error as HttpError;
use crate::http::{AdapterHttp, Bounds, Cancellation, Options, Request, Response};
use ssx_application::{
    AdmissionDecision, PortFuture, SearchAdmission, SearchContext, SearchEngine,
};
use ssx_config::ConfigScalar;
use ssx_domain::{AdapterResponse, EngineStatus, SearchRequest, SearchResult};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use url::Url;
const USER_AGENT: &str = "SlopSearX/0.1.0 (meta search engine; agent-native)";
const ASHBY_QUERY: &str = "\nquery ApiJobBoardWithTeams {\n  jobBoard {\n    jobPostings {\n      id\n      title\n      locationName\n      salaryRange\n      departmentName\n      updatedAt\n    }\n  }\n}\n";
macro_rules! provider_config {
    ($name:ident,$base:literal,$kind:ident) => {
        #[derive(Clone, Debug)]
        pub struct $name {
            pub base_url: String,
            pub timeout_ms: ConfigScalar,
            pub max_results: ConfigScalar,
        }
        impl Default for $name {
            fn default() -> Self {
                Self {
                    base_url: $base.into(),
                    timeout_ms: 5000.into(),
                    max_results: 10.into(),
                }
            }
        }
        impl $name {
            pub fn from_resolved(entry: &ssx_config::EngineConfig) -> Self {
                Self {
                    base_url: entry.base_url.clone(),
                    timeout_ms: entry.timeout_ms.clone(),
                    max_results: entry.max_results.clone(),
                }
            }
        }
        impl JobProvider for $name {
            fn timeout_ms(&self) -> &ConfigScalar {
                &self.timeout_ms
            }
            fn max_results(&self) -> &ConfigScalar {
                &self.max_results
            }
            fn base_url(&self) -> &str {
                &self.base_url
            }
            fn kind(&self) -> Kind {
                Kind::$kind
            }
        }
    };
}
provider_config!(AshbyConfig, "https://jobs.ashbyhq.com", Ashby);
provider_config!(
    GreenhouseConfig,
    "https://boards-api.greenhouse.io/v1/boards",
    Greenhouse
);
provider_config!(LeverConfig, "https://api.lever.co/v0/postings", Lever);
#[derive(Clone, Debug, Default)]
pub struct JobsConfig {
    pub ashby: AshbyConfig,
    pub greenhouse: GreenhouseConfig,
    pub lever: LeverConfig,
}
impl JobsConfig {
    pub fn from_resolved(config: &ssx_config::Config) -> Self {
        Self {
            ashby: config
                .engines
                .get("ashby")
                .map(AshbyConfig::from_resolved)
                .unwrap_or_default(),
            greenhouse: config
                .engines
                .get("greenhouse")
                .map(GreenhouseConfig::from_resolved)
                .unwrap_or_default(),
            lever: config
                .engines
                .get("lever")
                .map(LeverConfig::from_resolved)
                .unwrap_or_default(),
        }
    }
}
/// Match the source jobs utility: use the final `at`, `for`, or `@` trigger,
/// keep the original company spelling for display, and ASCII-slugify for URLs.
pub fn extract_company(query: &str) -> (Option<String>, Option<String>) {
    let lowered: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    let original: Vec<char> = query.chars().collect();
    let mut best: Option<(usize, usize)> = None;
    for trigger in [" at ", " for ", " @ "] {
        let needle: Vec<char> = trigger.chars().collect();
        if needle.len() > lowered.len() {
            continue;
        }
        if let Some(index) = lowered
            .windows(needle.len())
            .rposition(|window| window == needle)
            && best.is_none_or(|(current, _)| index > current)
        {
            best = Some((index, needle.len()));
        }
    }
    let Some((index, length)) = best else {
        return (None, None);
    };
    let company: String = original
        .iter()
        .skip(index + length)
        .collect::<String>()
        .trim_matches(ssx_json::python_whitespace)
        .to_owned();
    if company.is_empty() {
        return (None, None);
    }
    let mut slug = String::new();
    let mut dash = false;
    for ch in company.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            dash = false;
        } else if !dash {
            slug.push('-');
            dash = true;
        }
    }
    let slug = slug.trim_matches('-').to_owned();
    if slug.is_empty() {
        (None, None)
    } else {
        (Some(slug), Some(company))
    }
}

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Ashby,
    Greenhouse,
    Lever,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Ashby => "ashby",
            Self::Greenhouse => "greenhouse",
            Self::Lever => "lever",
        }
    }
}
pub(super) trait JobProvider: Send + Sync {
    fn timeout_ms(&self) -> &ConfigScalar;
    fn max_results(&self) -> &ConfigScalar;
    fn base_url(&self) -> &str;
    fn kind(&self) -> Kind;
}
pub(super) struct JobsAdapter<C> {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: C,
    name: &'static str,
}
pub(super) type AshbyAdapter = JobsAdapter<AshbyConfig>;
pub(super) type GreenhouseAdapter = JobsAdapter<GreenhouseConfig>;
pub(super) type LeverAdapter = JobsAdapter<LeverConfig>;
macro_rules! constructor {
    ($config:ty,$name:ident,$label:literal) => {
        impl JobsAdapter<$config> {
            pub(super) fn $name(
                http: Arc<AdapterHttp>,
                admission: Arc<dyn SearchAdmission>,
                config: $config,
            ) -> Self {
                Self {
                    http,
                    admission,
                    config,
                    name: $label,
                }
            }
        }
    };
}
constructor!(AshbyConfig, ashby, "ashby");
constructor!(GreenhouseConfig, greenhouse, "greenhouse");
constructor!(LeverConfig, lever, "lever");
impl<C: JobProvider> SearchEngine for JobsAdapter<C> {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.search_inner(request, None))
    }
    fn search_with_context<'a>(
        &'a self,
        request: &'a SearchRequest,
        context: SearchContext,
    ) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.search_inner(request, Some(context.deadline)))
    }
}
#[derive(Debug)]
pub(super) enum ParseFailure {
    Json,
    Shape(&'static str),
    Typed(&'static str),
    Timeout,
}
fn check(deadline: Option<Instant>) -> Result<(), ParseFailure> {
    if deadline.is_some_and(|d| Instant::now() >= d) {
        Err(ParseFailure::Timeout)
    } else {
        Ok(())
    }
}
fn slice_end(limit: &ConfigScalar, len: usize) -> Result<usize, ParseFailure> {
    // F06 effective_i64 also accepts float/text; Python's slice protocol does not.
    if !matches!(limit, ConfigScalar::Boolean(_) | ConfigScalar::Integer(_)) {
        return Err(ParseFailure::Shape("slice indices must be integers"));
    }
    let n = limit
        .effective_i64()
        .map_err(|_| ParseFailure::Shape("slice indices must be integers"))?;
    Ok(if n < 0 {
        len.saturating_sub(n.unsigned_abs().min(usize::MAX as u64) as usize)
    } else {
        len.min(n as usize)
    })
}
fn rows<'a>(value: &'a Value, limit: &ConfigScalar) -> Result<&'a [Value], ParseFailure> {
    match value {
        Value::Array(a) => Ok(&a[..slice_end(limit, a.len())?]),
        Value::String(s) if slice_end(limit, s.len())? == 0 => Ok(&[]),
        Value::String(_) => Err(ParseFailure::Shape("posting must support .get")),
        _ => Err(ParseFailure::Shape("job collection is not sliceable")),
    }
}
fn iterable(value: Option<&Value>) -> Result<&[Value], ParseFailure> {
    match value {
        None => Ok(&[]),
        Some(Value::Array(a)) => Ok(a),
        Some(Value::Object(m)) if m.is_empty() => Ok(&[]),
        Some(Value::String(t)) if t.is_empty() => Ok(&[]),
        Some(Value::Object(_) | Value::String(_)) => {
            Err(ParseFailure::Shape("iterated member must support .get"))
        }
        _ => Err(ParseFailure::Shape("field is not iterable")),
    }
}
fn source_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_none_or(|n| n != 0.),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}
fn text_default(value: Option<&Value>) -> Value {
    value
        .cloned()
        .unwrap_or_else(|| Value::String(Text::from("")))
}
fn text_or(value: Option<&Value>) -> Value {
    text_default(value.filter(|v| source_truthy(v)))
}
fn as_source_text(value: &Value, field: &'static str) -> Result<Text, ParseFailure> {
    value.as_text().cloned().ok_or(ParseFailure::Shape(field))
}
fn join(
    parts: &[Value],
    separator: &str,
    cap: Option<usize>,
    field: &'static str,
) -> Result<Text, ParseFailure> {
    let mut points = Vec::new();
    for (i, v) in parts.iter().enumerate() {
        let t = as_source_text(v, field)?;
        if i > 0 {
            points.extend(separator.chars().map(u32::from));
        }
        points.extend_from_slice(t.codepoints());
    }
    if let Some(cap) = cap {
        points.truncate(cap);
    }
    Ok(Text::from_points(points))
}
fn interpolated(prefix: &str, id: &Value) -> Text {
    let mut points = prefix.chars().map(u32::from).collect::<Vec<_>>();
    points.extend_from_slice(id.python_string().codepoints());
    Text::from_points(points)
}
struct SourceJob {
    url: Value,
    title: Value,
    content: Text,
    published: Option<Value>,
    payload: Option<Value>,
}
async fn parse_provider(
    kind: Kind,
    response: Response,
    company: &str,
    slug: &str,
    limit: &ConfigScalar,
    deadline: Option<Instant>,
) -> Result<Vec<SearchResult>, ParseFailure> {
    if response.status == 404 {
        return Ok(vec![]);
    }
    let data = source_json::from_slice(&response.body, deadline)
        .await
        .map_err(|e| {
            if e == source_json::Error::Deadline {
                ParseFailure::Timeout
            } else {
                ParseFailure::Json
            }
        })?;
    let empty_list = Value::Array(vec![]);
    let empty_map = Value::Object(Map::new());
    let collection = match kind {
        Kind::Ashby => {
            let root = data
                .as_object()
                .ok_or(ParseFailure::Shape("ashby response must be an object"))?;
            let data = root
                .get("data")
                .filter(|v| source_truthy(v))
                .unwrap_or(&empty_map);
            let data = data
                .as_object()
                .ok_or(ParseFailure::Shape("ashby data must be an object"))?;
            let board = data
                .get("jobBoard")
                .filter(|v| source_truthy(v))
                .unwrap_or(&empty_map);
            let board = board
                .as_object()
                .ok_or(ParseFailure::Shape("ashby jobBoard must be an object"))?;
            board
                .get("jobPostings")
                .filter(|v| source_truthy(v))
                .unwrap_or(&empty_list)
        }
        Kind::Greenhouse => data
            .as_object()
            .ok_or(ParseFailure::Shape("greenhouse response must be an object"))?
            .get("jobs")
            .unwrap_or(&empty_list),
        Kind::Lever => {
            if !data.is_array() {
                return Ok(vec![]);
            }
            &data
        }
    };
    let selected = rows(collection, limit)?;
    let mut completed = Vec::new();
    for value in selected {
        check(deadline)?;
        tokio::task::yield_now().await;
        let row = value
            .as_object()
            .ok_or(ParseFailure::Shape("posting must be an object"))?;
        let mut parts = vec![Value::String(company.into())];
        let job = match kind {
            Kind::Ashby => {
                let id = text_or(row.get("id"));
                let title = text_or(row.get("title"));
                let location = text_or(row.get("locationName"));
                let salary = text_or(row.get("salaryRange"));
                let department = text_or(row.get("departmentName"));
                let published = row.get("updatedAt").cloned();
                let url = Value::String(interpolated(
                    &format!("https://jobs.ashbyhq.com/{slug}/"),
                    &id,
                ));
                for v in [location, department, salary] {
                    if source_truthy(&v) {
                        parts.push(v)
                    }
                }
                SourceJob {
                    url,
                    title,
                    content: join(
                        &parts,
                        " | ",
                        Some(500),
                        "ashby content part must be a string",
                    )?,
                    published,
                    payload: None,
                }
            }
            Kind::Greenhouse => {
                let title = text_default(row.get("title"));
                let id = text_default(row.get("id"));
                let url = row.get("absolute_url").cloned().unwrap_or_else(|| {
                    Value::String(interpolated(
                        &format!("https://boards.greenhouse.io/{slug}/jobs/"),
                        &id,
                    ))
                });
                let published = row.get("updated_at").cloned();
                let mut locations = vec![];
                for office in iterable(row.get("offices"))? {
                    check(deadline)?;
                    let office = office
                        .as_object()
                        .ok_or(ParseFailure::Shape("greenhouse office must be an object"))?;
                    locations.push(text_default(office.get("name")));
                }
                let location = Value::String(join(
                    &locations,
                    ", ",
                    None,
                    "greenhouse office name must be a string",
                )?);
                let mut salary = Value::String(Text::from(""));
                for meta in iterable(row.get("metadata"))? {
                    check(deadline)?;
                    let meta = meta.as_object().ok_or(ParseFailure::Shape(
                        "greenhouse metadata item must be an object",
                    ))?;
                    let name = text_default(meta.get("name"));
                    let name = as_source_text(&name, "greenhouse metadata name must be a string")?;
                    if name.as_str().is_some_and(|s| s.to_lowercase() == "salary") {
                        salary = text_default(meta.get("value"));
                        break;
                    }
                }
                for v in [&location, &salary] {
                    if source_truthy(v) {
                        parts.push(v.clone())
                    }
                }
                // Python build_payload happens before the content join; retain source
                // values here, and apply only the F05 projection after source completion.
                let mut payload = Map::new();
                for (k, v) in [
                    ("company", Value::String(company.into())),
                    ("title", title.clone()),
                    ("location", location),
                    ("salary", salary),
                    ("job_id", id),
                ] {
                    if source_truthy(&v) {
                        payload.insert(k.into(), v)
                    }
                }
                SourceJob {
                    url,
                    title,
                    content: join(
                        &parts,
                        " | ",
                        Some(500),
                        "greenhouse content part must be a string",
                    )?,
                    published,
                    payload: Some(Value::Object(payload)),
                }
            }
            Kind::Lever => {
                let title = text_default(row.get("text"));
                let id = text_default(row.get("id"));
                let url = row.get("hostedUrl").cloned().unwrap_or_else(|| {
                    Value::String(interpolated(&format!("https://jobs.lever.co/{slug}/"), &id))
                });
                let categories = row
                    .get("categories")
                    .filter(|v| source_truthy(v))
                    .unwrap_or(&empty_map)
                    .as_object()
                    .ok_or(ParseFailure::Shape("lever categories must be an object"))?;
                let location = text_default(categories.get("location"));
                let commitment = text_default(categories.get("commitment"));
                let team = text_default(categories.get("team"));
                let published = match row.get("createdAt").filter(|v| source_truthy(v)) {
                    None => None,
                    Some(v) => {
                        let ms = match v {
                            Value::Bool(b) => f64::from(u8::from(*b)),
                            Value::Number(n) => n.as_f64().ok_or(ParseFailure::Shape(
                                "lever createdAt is outside datetime range",
                            ))?,
                            _ => {
                                return Err(ParseFailure::Shape("lever createdAt must be numeric"));
                            }
                        };
                        Some(Value::String(
                            epoch_ms_iso_f64(ms)
                                .ok_or(ParseFailure::Shape(
                                    "lever createdAt is outside datetime range",
                                ))?
                                .into(),
                        ))
                    }
                };
                for v in [location, commitment, team] {
                    if source_truthy(&v) {
                        parts.push(v)
                    }
                }
                SourceJob {
                    url,
                    title,
                    content: join(
                        &parts,
                        " | ",
                        Some(500),
                        "lever content part must be a string",
                    )?,
                    published,
                    payload: None,
                }
            }
        };
        completed.push(job);
    }
    // No projection before this point: genuine source failures in any later row
    // discard the whole batch and take precedence over typed output refusal.
    let budget = ssx_owned_budget::Budget::new(32 * 1024 * 1024)
        .map_err(|_| ParseFailure::Shape("job output budget unavailable"))?;
    let mut output = source_payload::ProducedBatch::new(&budget, completed.len(), deadline)
        .map_err(projection_failure)?;
    let mut pending = None;
    for (i, row) in completed.into_iter().enumerate() {
        check(deadline)?;
        tokio::task::yield_now().await;
        if let Err(e) = project_job(kind, &row, i, &budget, &mut output, deadline).await {
            if matches!(e, ParseFailure::Timeout) {
                return Err(e);
            }
            if pending.is_none() {
                pending = Some(e)
            }
        }
    }
    check(deadline)?;
    if let Some(e) = pending {
        return Err(e);
    }
    Ok(output
        .into_adapter_response(source_payload::Outcome {
            status: EngineStatus::Ok,
            error_message: None,
            latency_ms: 0.,
            synthetic: false,
        })
        .await
        .map_err(projection_failure)?
        .results)
}
fn typed_text<'a>(value: &'a Value, field: &'static str) -> Result<&'a str, ParseFailure> {
    value.as_str().ok_or(ParseFailure::Typed(field))
}
fn projection_failure(error: source_payload::BudgetError) -> ParseFailure {
    match error {
        source_payload::BudgetError::Deadline => ParseFailure::Timeout,
        source_payload::BudgetError::Projection(_) => ParseFailure::Typed("greenhouse payload"),
        _ => ParseFailure::Shape("job output projection resource limit"),
    }
}
async fn project_job(
    kind: Kind,
    row: &SourceJob,
    index: usize,
    budget: &ssx_owned_budget::Budget,
    output: &mut source_payload::ProducedBatch<'_>,
    deadline: Option<Instant>,
) -> Result<(), ParseFailure> {
    let url = typed_text(
        &row.url,
        match kind {
            Kind::Ashby => "ashby url",
            Kind::Greenhouse => "greenhouse absolute_url",
            Kind::Lever => "lever hostedUrl",
        },
    )?;
    let title = typed_text(
        &row.title,
        match kind {
            Kind::Ashby => "ashby title",
            Kind::Greenhouse => "greenhouse title",
            Kind::Lever => "lever text",
        },
    )?;
    let published_date = match &row.published {
        None | Some(Value::Null) => None,
        Some(v) => Some(typed_text(
            v,
            match kind {
                Kind::Ashby => "ashby updatedAt",
                Kind::Greenhouse => "greenhouse updated_at",
                Kind::Lever => "lever createdAt",
            },
        )?),
    };
    let payload = if let Some(source) = &row.payload {
        let fields = source
            .as_object()
            .expect("source payload map")
            .iter()
            .map(|(name, value)| source_payload::Field {
                name: name.as_str().expect("static field"),
                value: source_payload::FieldRef::Value(value),
            })
            .collect::<Vec<_>>();
        Some(
            source_payload::envelope_budgeted(
                "jobs",
                "job",
                "greenhouse",
                &fields,
                budget,
                deadline,
            )
            .await
            .map_err(projection_failure)?,
        )
    } else {
        None
    };
    output
        .push(
            source_payload::Skeleton {
                url,
                title,
                content: row
                    .content
                    .as_str()
                    .ok_or(ParseFailure::Typed("job content"))?,
                engine: kind.name(),
                category: "jobs",
                score: 0.,
                position: (index + 1) as i64,
                tier: 2,
                published_date,
            },
            payload,
        )
        .await
        .map_err(projection_failure)
}
impl<C: JobProvider> JobsAdapter<C> {
    async fn search_inner(
        &self,
        request: &SearchRequest,
        context: Option<Instant>,
    ) -> AdapterResponse {
        let started = Instant::now();
        let result = self.execute(request, context).await;
        match result {
            Ok(results) => AdapterResponse {
                results,
                status: EngineStatus::Ok,
                latency_ms: elapsed(started),
                ..Default::default()
            },
            Err((status, message)) => failed(status, &message, started),
        }
    }
    async fn execute(
        &self,
        request: &SearchRequest,
        context: Option<Instant>,
    ) -> Result<Vec<SearchResult>, (EngineStatus, String)> {
        let failure = |message: &str| (EngineStatus::Error, message.to_owned());
        let admit = self.admission.admit(self.name);
        let admitted = if let Some(d) = context {
            tokio::time::timeout_at(tokio::time::Instant::from_std(d), admit)
                .await
                .map_err(|_| (EngineStatus::Timeout, "job search deadline exceeded".into()))?
        } else {
            admit.await
        };
        match admitted {
            AdmissionDecision::Allowed => {}
            AdmissionDecision::Denied => {
                return Err((
                    EngineStatus::RateLimited,
                    "job provider request denied by rate policy".into(),
                ));
            }
            AdmissionDecision::Unavailable => {
                return Err(failure("job provider rate policy unavailable"));
            }
        }
        let (Some(slug), Some(display)) = extract_company(&request.query) else {
            return Err((
                EngineStatus::Unavailable,
                "company-scoped search requires a company name".into(),
            ));
        };
        let kind = self.config.kind();
        let path = match kind {
            Kind::Ashby => format!("{}/{slug}/api/non-user-graphql", self.config.base_url()),
            Kind::Greenhouse => format!("{}/{slug}/jobs", self.config.base_url()),
            Kind::Lever => format!("{}/{slug}", self.config.base_url()),
        };
        let mut url = Url::parse(&path).map_err(|_| failure("invalid job provider base URL"))?;
        match kind {
            Kind::Greenhouse => {
                url.query_pairs_mut().append_pair("content", "true");
            }
            Kind::Lever => {
                url.query_pairs_mut().append_pair("mode", "json");
            }
            _ => {}
        }
        let mut outbound =
            Request::get(url.as_str()).map_err(|_| failure("invalid job provider request URL"))?;
        outbound
            .headers
            .insert("user-agent", USER_AGENT.parse().expect("static header"));
        if matches!(kind, Kind::Ashby) {
            outbound.method = reqwest::Method::POST;
            outbound.headers.insert(
                "content-type",
                "application/json".parse().expect("static header"),
            );
            outbound.body=serde_json::to_vec(&serde_json::json!({"operationName":"ApiJobBoardWithTeams","variables":{},"query":ASHBY_QUERY})).expect("static JSON");
        }
        // Preserve source arithmetic kind: numeric strings do not support /.
        let ms = match self.config.timeout_ms() {
            ConfigScalar::Boolean(b) => f64::from(u8::from(*b)),
            ConfigScalar::Integer(i) => *i as f64,
            ConfigScalar::Float(f) => *f,
            ConfigScalar::Text(_) => return Err(failure("timeout_ms does not support arithmetic")),
        };
        if ms <= 0. {
            return Err((
                EngineStatus::Timeout,
                "job provider request timed out".into(),
            ));
        }
        // Same accepted A09 semantics as clinicaltrials::network_deadline:
        // numeric overflow/unbounded timeout can use a finite caller context;
        // a source string still fails division, and NaN is invalid arithmetic.
        if ms.is_nan() {
            return Err(failure("invalid timeout_ms"));
        }
        let configured = Duration::try_from_secs_f64(ms / 1000.)
            .ok()
            .and_then(|duration| Instant::now().checked_add(duration));
        let deadline = match (configured, context) {
            (Some(configured), Some(limit)) => configured.min(limit),
            (Some(configured), None) => configured,
            (None, Some(limit)) => limit,
            (None, None) => return Err(failure("timeout requires a finite lifecycle deadline")),
        };
        let mut session = self
            .http
            .session(
                Options {
                    bounds: Bounds {
                        requests: 1,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                tokio::time::Instant::from_std(deadline),
                Cancellation::default(),
            )
            .map_err(|_| failure("job provider HTTP session unavailable"))?;
        let response = session.execute(outbound).await.map_err(|e| {
            if matches!(e, HttpError::Deadline) {
                (
                    EngineStatus::Timeout,
                    "job provider request timed out".into(),
                )
            } else {
                failure("job provider request failed")
            }
        })?;
        if response.status == 429 {
            return Err((
                EngineStatus::RateLimited,
                "job provider rate limited".into(),
            ));
        }
        if !(200..300).contains(&response.status) && response.status != 404 {
            return Err(failure("job provider returned an unsuccessful status"));
        }
        // Source timeout is HTTP-only; caller context covers parse/projection too.
        parse_provider(
            kind,
            response,
            &display,
            &slug,
            self.config.max_results(),
            context,
        )
        .await
        .map_err(|e| match e {
            ParseFailure::Json => failure("job provider returned invalid JSON"),
            ParseFailure::Shape(message) => failure(message),
            ParseFailure::Typed(field) => (
                EngineStatus::Error,
                format!("provider field {field} cannot be represented by the typed Rust result"),
            ),
            ParseFailure::Timeout => (EngineStatus::Timeout, "job search deadline exceeded".into()),
        })
    }
}
// CPython 3.14.7: Lever divides milliseconds by 1000 FIRST. datetime then
// calls _PyTime_ObjectToTimeval(HALF_EVEN): modf(seconds), round fractional
// seconds * 1e6 with ties-to-even, and normalize carry/negative fractions.
#[cfg(test)]
fn epoch_ms_iso(ms: i64) -> Option<String> {
    epoch_ms_iso_f64(ms as f64)
}
fn epoch_ms_iso_f64(ms: f64) -> Option<String> {
    let seconds = ms / 1000.;
    if !seconds.is_finite() {
        return None;
    }
    let mut integral = seconds.trunc();
    let mut micros = (seconds.fract() * 1_000_000.).round_ties_even();
    if micros >= 1_000_000. {
        micros -= 1_000_000.;
        integral += 1.;
    } else if micros < 0. {
        micros += 1_000_000.;
        integral -= 1.;
    }
    if !(-62_135_596_800.0..253_402_300_800.0).contains(&integral) {
        return None;
    }
    epoch_micros_iso(
        (integral as i64)
            .checked_mul(1_000_000)?
            .checked_add(micros as i64)?,
    )
}
fn epoch_micros_iso(micros: i64) -> Option<String> {
    let secs = micros.div_euclid(1_000_000);
    let micros = micros.rem_euclid(1_000_000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    if !(1..=9999).contains(&year) {
        return None;
    }
    if micros == 0 {
        Some(format!(
            "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}+00:00",
            rem / 3600,
            (rem % 3600) / 60,
            rem % 60
        ))
    } else {
        Some(format!(
            "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{micros:06}+00:00",
            rem / 3600,
            (rem % 3600) / 60,
            rem % 60
        ))
    }
}
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 }.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    y += i64::from(m <= 2);
    (y, m, d)
}
fn failed(status: EngineStatus, message: &str, started: Instant) -> AdapterResponse {
    AdapterResponse {
        status,
        error_message: Some(message.into()),
        latency_ms: elapsed(started),
        ..AdapterResponse::default()
    }
}
fn elapsed(started: Instant) -> ssx_domain::FiniteF64 {
    ssx_domain::FiniteF64::new(started.elapsed().as_secs_f64() * 1000.0).unwrap_or_default()
}

#[cfg(test)]
fn as_text(v: &ssx_json::Value) -> Option<&str> {
    v.as_str()
}
#[cfg(test)]
fn as_i64(v: &ssx_json::Value) -> Option<i64> {
    match v {
        ssx_json::Value::Number(n) => n.as_i64(),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::AdapterHttp;
    use std::sync::Mutex;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn source_company_cases_are_preserved() {
        let _case = CompletedCase("legacy.source_company_cases_are_preserved");
        for (query, expected) in [
            (
                "Senior AI Engineer at Anthropic",
                Some(("anthropic", "Anthropic")),
            ),
            (
                "looking for engineer for Stripe",
                Some(("stripe", "Stripe")),
            ),
            ("PM role @ Notion", Some(("notion", "Notion"))),
            ("Engineer at Red Hat", Some(("red-hat", "Red Hat"))),
            ("python developer jobs", None),
            ("hiring at", None),
            (
                "dev at Hello! World Inc.",
                Some(("hello-world-inc", "Hello! World Inc.")),
            ),
            ("Engineer at !!!", None),
            ("site:jobs.lever.co at Stripe", Some(("stripe", "Stripe"))),
            ("HIRING AT MICROSOFT", Some(("microsoft", "MICROSOFT"))),
            ("Engineer at Café Labs", Some(("caf-labs", "Café Labs"))),
            ("Engineer at Straße", Some(("stra-e", "Straße"))),
            ("Engineer at A/B & C", Some(("a-b-c", "A/B & C"))),
            ("Engineer at \u{1c}Acme\u{1f}", Some(("acme", "Acme"))),
            ("ΟΣ at Acme", Some(("acme", "Acme"))),
            ("ΟΣA at Acme", Some(("acme", "Acme"))),
            // Python lower() expands U+0130; the source then slices the original
            // string with that lowered-string index, producing the suffix "cme".
            ("İ at Acme", Some(("cme", "cme"))),
        ] {
            let got = extract_company(query);
            assert_eq!(
                got,
                expected
                    .map(|(a, b)| (Some(a.into()), Some(b.into())))
                    .unwrap_or((None, None)),
                "{query}"
            );
        }
        assert_unicode_lower_projection_matches_u16();
    }

    fn assert_unicode_lower_projection_matches_u16() {
        use std::collections::HashMap;

        const UNICODE_DATA: &str = include_str!(
            "../../../../crates/ssx-source-codecs/provenance/idna/inputs/UnicodeData-16.0.0.txt"
        );
        const SPECIAL_CASING: &str = include_str!(
            "../../../../crates/ssx-source-codecs/provenance/idna/inputs/SpecialCasing-16.0.0.txt"
        );
        let mut mappings: HashMap<u32, Vec<u32>> = HashMap::new();
        for line in UNICODE_DATA.lines() {
            let fields: Vec<_> = line.split(';').collect();
            if let Some(lower) = fields.get(13).filter(|value| !value.is_empty()) {
                let scalar = u32::from_str_radix(fields[0], 16).unwrap();
                mappings.insert(
                    scalar,
                    lower
                        .split_whitespace()
                        .map(|value| u32::from_str_radix(value, 16).unwrap())
                        .collect(),
                );
            }
        }
        for line in SPECIAL_CASING.lines() {
            let fields: Vec<_> = line.split('#').next().unwrap().split(';').collect();
            if fields.len() >= 5 && fields[4].trim().is_empty() {
                let scalar = u32::from_str_radix(fields[0].trim(), 16).unwrap();
                mappings.insert(
                    scalar,
                    fields[1]
                        .split_whitespace()
                        .map(|value| u32::from_str_radix(value, 16).unwrap())
                        .collect(),
                );
            }
        }
        let conditional: std::collections::BTreeSet<_> = SPECIAL_CASING
            .lines()
            .filter_map(|line| {
                let fields: Vec<_> = line.split('#').next()?.split(';').collect();
                (fields.len() >= 5 && !fields[4].trim().is_empty())
                    .then(|| fields[4].trim().to_owned())
            })
            .collect();
        assert_eq!(
            conditional,
            [
                "Final_Sigma",
                "az",
                "az After_I",
                "az Not_Before_Dot",
                "lt",
                "lt After_Soft_Dotted",
                "lt More_Above",
                "tr",
                "tr After_I",
                "tr Not_Before_Dot",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect()
        );

        let projection = |value: &str| {
            value
                .chars()
                .map(|ch| ch.is_ascii().then_some(ch))
                .collect::<Vec<_>>()
        };
        let mut scalar_count = 0usize;
        for codepoint in 0..=0x10ffff {
            let Some(ch) = char::from_u32(codepoint) else {
                continue;
            };
            scalar_count += 1;
            let expected: String = mappings
                .get(&codepoint)
                .map(|points| {
                    points
                        .iter()
                        .map(|point| char::from_u32(*point).unwrap())
                        .collect()
                })
                .unwrap_or_else(|| ch.to_string());
            let actual: String = ch.to_lowercase().collect();
            assert_eq!(
                actual.chars().count(),
                expected.chars().count(),
                "U+{codepoint:04X} scalar count"
            );
            assert_eq!(
                projection(&actual),
                projection(&expected),
                "U+{codepoint:04X} ASCII positions"
            );
        }
        assert_eq!(scalar_count, 1_112_064);

        // Python's default Final_Sigma rule changes only which non-ASCII sigma
        // scalar is emitted. It cannot alter trigger positions or the slug's
        // ASCII projection; these contexts pin both sides of that rule.
        assert_eq!(projection("ΟΣ"), projection("οσ"));
        assert_eq!(projection("ΟΣ"), projection("ος"));
        assert_eq!("Σ".to_lowercase().chars().count(), 1);
    }
    #[test]
    fn lever_epoch_milliseconds_match_utc_iso() {
        let _case = CompletedCase("legacy.lever_epoch_milliseconds_match_utc_iso");
        assert_eq!(
            epoch_ms_iso(1_720_000_000_000).as_deref(),
            Some("2024-07-03T09:46:40+00:00")
        );
        assert_eq!(
            epoch_ms_iso(1_720_000_000_001).as_deref(),
            Some("2024-07-03T09:46:40.001000+00:00")
        );
        assert_eq!(
            epoch_ms_iso_f64(1_720_000_000_000.5).as_deref(),
            Some("2024-07-03T09:46:40.000500+00:00")
        );
        assert_eq!(
            epoch_ms_iso_f64(1.0).as_deref(),
            Some("1970-01-01T00:00:00.001000+00:00")
        );
        assert!(epoch_ms_iso_f64(1e30).is_none());
    }

    async fn loopback(
        body: impl AsRef<[u8]>,
        status: u16,
    ) -> (String, Arc<Mutex<Vec<String>>>, tokio::task::JoinHandle<()>) {
        let body = body.as_ref().to_vec();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let seen = captured.clone();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0u8; 2048];
            loop {
                let n = socket.read(&mut chunk).await.unwrap();
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..n]);
                if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&bytes[..end + 4]).to_string();
                    let length = head
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|v| v.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        seen.lock()
                            .unwrap()
                            .push(String::from_utf8_lossy(&bytes).into_owned());
                        break;
                    }
                }
            }
            let reason = if status == 200 {
                "OK"
            } else if status == 404 {
                "Not Found"
            } else if status == 429 {
                "Too Many Requests"
            } else {
                "Error"
            };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.write_all(&body).await;
        });
        (format!("http://{address}"), captured, task)
    }
    fn request(query: &str) -> SearchRequest {
        SearchRequest {
            query: query.into(),
            page: 4,
            // Deliberately conflicts with provider config to prove that the
            // source adapter's configured max_results, not request filters,
            // sets upstream and local result bounds.
            max_results: Some(99),
            language: "fr".into(),
            safesearch: 2,
            time_range: Some("month".into()),
            ..Default::default()
        }
    }
    async fn run<C: JobProvider>(adapter: &JobsAdapter<C>, query: &str) -> AdapterResponse {
        audit_loopback_proxies();
        adapter.search(&request(query)).await
    }

    #[tokio::test]
    async fn ashby_uses_graphql_post_and_normalizes_job_postings() {
        let _case = CompletedCase("legacy.ashby_uses_graphql_post_and_normalizes_job_postings");
        let (base,seen,server)=loopback(r#"{"data":{"jobBoard":{"jobPostings":[{"id":7,"title":"Senior AI Engineer","locationName":"Remote","salaryRange":"$180k","departmentName":"Research","updatedAt":"2026-06-01"},{"id":"p-2","title":"ignored"},{"id":"p-3","title":"over configured limit"}]}}}"#,200).await;
        let engine = JobsAdapter::ashby(
            Arc::new(AdapterHttp::default()),
            Arc::new(super::super::AllowAllAdmission),
            AshbyConfig {
                base_url: base,
                max_results: 2.into(),
                ..Default::default()
            },
        );
        let result = run(&engine, "Engineer at Anthropic").await;
        server.await.unwrap();
        assert_eq!(result.status, EngineStatus::Ok);
        assert_eq!(result.results.len(), 2);
        let job = &result.results[0];
        assert_eq!(job.url, "https://jobs.ashbyhq.com/anthropic/7");
        assert_eq!(job.title, "Senior AI Engineer");
        assert_eq!(job.content, "Anthropic | Remote | Research | $180k");
        assert_eq!(job.published_date.as_deref(), Some("2026-06-01"));
        assert_eq!(job.category, "jobs");
        assert_eq!(job.tier, 2);
        assert_eq!(result.results[1].title, "ignored");
        assert_eq!(result.results[1].position, 2);
        assert_eq!(seen.lock().unwrap().len(), 1);
        let request = seen.lock().unwrap()[0].clone();
        assert!(request.starts_with("POST /anthropic/api/non-user-graphql HTTP/1.1"));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("user-agent: slopsearx/0.1.0 (meta search engine; agent-native)")
        );
        assert!(
            request
                .to_ascii_lowercase()
                .contains("content-type: application/json")
        );
        let body = request.split_once("\r\n\r\n").unwrap().1;
        let body: ssx_json::Value = ssx_json::from_str(body).unwrap();
        assert_eq!(
            body.get("operationName").and_then(as_text),
            Some("ApiJobBoardWithTeams")
        );
        assert!(
            body.get("variables")
                .is_some_and(|v| v.as_object().is_some_and(|v| v.is_empty()))
        );
        assert!(body.get("query").and_then(as_text) == Some(ASHBY_QUERY));
        assert!(!request.to_ascii_lowercase().contains("authorization:"));
        assert!(!request.contains("Engineer"));
        assert!(!request.to_ascii_lowercase().contains("page="));
        assert!(!request.to_ascii_lowercase().contains("safesearch"));
        assert!(!request.to_ascii_lowercase().contains("language"));
    }

    #[tokio::test]
    async fn greenhouse_builds_content_query_and_job_payload_with_absent_id_preserved() {
        let _case = CompletedCase(
            "legacy.greenhouse_builds_content_query_and_job_payload_with_absent_id_preserved",
        );
        let (base,seen,server)=loopback(r#"{"jobs":[{"id":123,"title":"Engineer","absolute_url":"https://boards.greenhouse.io/acme/jobs/123","offices":[{"name":"NYC"},{"name":"Remote"}],"metadata":[{"name":"Other","value":"x"},{"name":"Salary","value":"$200k"}],"updated_at":"2026-07-01"},{"title":"No ID","offices":[],"metadata":[]},{"id":999,"title":"over configured limit"}] }"#,200).await;
        let engine = JobsAdapter::greenhouse(
            Arc::new(AdapterHttp::default()),
            Arc::new(super::super::AllowAllAdmission),
            GreenhouseConfig {
                base_url: format!("{base}/v1/boards"),
                max_results: 2.into(),
                ..Default::default()
            },
        );
        let result = run(&engine, "Designer at Acme Inc.").await;
        server.await.unwrap();
        assert_eq!(result.status, EngineStatus::Ok);
        assert_eq!(result.results.len(), 2);
        let first = &result.results[0];
        assert_eq!(first.category, "jobs");
        assert_eq!(first.tier, 2);
        assert_eq!(first.title, "Engineer");
        assert_eq!(first.url, "https://boards.greenhouse.io/acme/jobs/123");
        assert_eq!(first.content, "Acme Inc. | NYC, Remote | $200k");
        assert_eq!(first.published_date.as_deref(), Some("2026-07-01"));
        let data = first.payload.as_ref().unwrap().get("data").unwrap();
        assert_eq!(data.get("company").and_then(as_text), Some("Acme Inc."));
        assert_eq!(data.get("title").and_then(as_text), Some("Engineer"));
        assert_eq!(data.get("location").and_then(as_text), Some("NYC, Remote"));
        let second_data = result.results[1]
            .payload
            .as_ref()
            .unwrap()
            .get("data")
            .unwrap();
        assert_eq!(second_data.get("title").and_then(as_text), Some("No ID"));
        assert!(second_data.get("salary").is_none());
        assert_eq!(data.get("job_id").and_then(as_i64), Some(123));
        assert_eq!(data.get("salary").and_then(as_text), Some("$200k"));
        let payload = first.payload.as_ref().unwrap();
        assert_eq!(payload.get("domain").and_then(as_text), Some("jobs"));
        assert_eq!(payload.get("type").and_then(as_text), Some("job"));
        assert_eq!(
            payload
                .get("provenance")
                .and_then(|v| v.get("engine"))
                .and_then(as_text),
            Some("greenhouse")
        );
        assert_eq!(seen.lock().unwrap().len(), 1);
        assert!(
            result.results[1]
                .payload
                .as_ref()
                .unwrap()
                .get("data")
                .unwrap()
                .get("job_id")
                .is_none()
        );
        assert_eq!(
            result.results[1].url,
            "https://boards.greenhouse.io/acme-inc/jobs/"
        );
        let request = seen.lock().unwrap()[0].clone();
        assert!(
            request.starts_with("GET /v1/boards/acme-inc/jobs?content=true HTTP/1.1"),
            "{request}"
        );
        assert!(!request.to_ascii_lowercase().contains("authorization:"));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("user-agent: slopsearx/0.1.0 (meta search engine; agent-native)")
        );
        assert!(!request.contains("page="));
        assert!(!request.contains("q="));
        assert!(!request.to_ascii_lowercase().contains("safesearch"));
    }

    #[tokio::test]
    async fn lever_sends_mode_json_and_normalizes_categories_epoch_and_limit() {
        let _case =
            CompletedCase("legacy.lever_sends_mode_json_and_normalizes_categories_epoch_and_limit");
        let (base,seen,server)=loopback(r#"[{"id":17,"text":"Software Engineer","categories":{"location":"SF","commitment":"Full-time","team":"Eng"},"createdAt":1720000000000},{"id":"a2","text":"Product Manager","categories":{"location":"Remote","commitment":"Full-time","team":"Product"},"hostedUrl":"https://jobs.lever.co/acme/a2","createdAt":1719900000000},{"id":"a3","text":"over configured limit"}]"#,200).await;
        let engine = JobsAdapter::lever(
            Arc::new(AdapterHttp::default()),
            Arc::new(super::super::AllowAllAdmission),
            LeverConfig {
                base_url: format!("{base}/v0/postings"),
                max_results: 2.into(),
                ..Default::default()
            },
        );
        let result = run(&engine, "Engineer at Acme").await;
        server.await.unwrap();
        assert_eq!(result.status, EngineStatus::Ok);
        assert_eq!(result.results.len(), 2);
        let job = &result.results[0];
        assert_eq!(job.url, "https://jobs.lever.co/acme/17");
        assert_eq!(job.content, "Acme | SF | Full-time | Eng");
        assert_eq!(
            job.published_date.as_deref(),
            Some("2024-07-03T09:46:40+00:00")
        );
        assert_eq!(job.title, "Software Engineer");
        assert_eq!(job.category, "jobs");
        assert_eq!(job.tier, 2);
        assert_eq!(result.results[1].url, "https://jobs.lever.co/acme/a2");
        assert_eq!(result.results[1].title, "Product Manager");
        assert_eq!(result.results[1].position, 2);
        assert_eq!(seen.lock().unwrap().len(), 1);
        assert!(
            seen.lock().unwrap()[0].starts_with("GET /v0/postings/acme?mode=json HTTP/1.1"),
            "{:?}",
            seen.lock().unwrap()[0]
        );
        let wire = seen.lock().unwrap()[0].clone();
        assert!(
            wire.to_ascii_lowercase()
                .contains("user-agent: slopsearx/0.1.0 (meta search engine; agent-native)")
        );
        assert!(!wire.contains("page="));
        assert!(!wire.contains("q="));
        assert!(!wire.to_ascii_lowercase().contains("language"));
        assert!(!wire.to_ascii_lowercase().contains("authorization:"));
    }

    #[tokio::test]
    async fn company_scope_rejects_before_dispatch_and_404_is_successful_empty() {
        let _case = CompletedCase(
            "legacy.company_scope_rejects_before_dispatch_and_404_is_successful_empty",
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let engine = JobsAdapter::ashby(
            Arc::new(AdapterHttp::default()),
            Arc::new(super::super::AllowAllAdmission),
            AshbyConfig {
                base_url: base,
                ..Default::default()
            },
        );
        let result = run(&engine, "python developer jobs").await;
        assert_eq!(result.status, EngineStatus::Unavailable);
        assert!(result.results.is_empty());
        assert_eq!(
            result.error_message.as_deref(),
            Some("company-scoped search requires a company name")
        );
        let greenhouse = JobsAdapter::greenhouse(
            Arc::new(AdapterHttp::default()),
            Arc::new(super::super::AllowAllAdmission),
            GreenhouseConfig {
                base_url: format!("http://{}/v1/boards", listener.local_addr().unwrap()),
                ..Default::default()
            },
        );
        let result = run(&greenhouse, "python developer jobs").await;
        assert_eq!(result.status, EngineStatus::Unavailable);
        assert!(result.results.is_empty());
        let lever = JobsAdapter::lever(
            Arc::new(AdapterHttp::default()),
            Arc::new(super::super::AllowAllAdmission),
            LeverConfig {
                base_url: format!("http://{}/v0/postings", listener.local_addr().unwrap()),
                ..Default::default()
            },
        );
        let result = run(&lever, "python developer jobs").await;
        assert_eq!(result.status, EngineStatus::Unavailable);
        assert!(result.results.is_empty());
        drop(listener);
        let (base, _, server) = loopback("", 404).await;
        let engine = JobsAdapter::lever(
            Arc::new(AdapterHttp::default()),
            Arc::new(super::super::AllowAllAdmission),
            LeverConfig {
                base_url: base,
                ..Default::default()
            },
        );
        let result = run(&engine, "Engineer at MissingCo").await;
        server.await.unwrap();
        assert_eq!(result.status, EngineStatus::Ok);
        assert!(result.results.is_empty());

        let engine = JobsAdapter::ashby(
            Arc::new(AdapterHttp::default()),
            Arc::new(super::super::AllowAllAdmission),
            AshbyConfig {
                base_url: "not a URL".into(),
                ..Default::default()
            },
        );
        let result = run(&engine, "Engineer at Acme").await;
        assert_eq!(result.status, EngineStatus::Error);
        assert_eq!(
            result.error_message.as_deref(),
            Some("invalid job provider base URL")
        );
    }

    #[tokio::test]
    async fn malformed_payloads_and_http_failures_are_classified() {
        let _case = CompletedCase("legacy.malformed_payloads_and_http_failures_are_classified");
        for (body, status, expected) in [
            (r#"{"jobs":[]}"#, 200, EngineStatus::Ok),
            ("{", 200, EngineStatus::Error),
            (r#"{"jobs":{}}"#, 200, EngineStatus::Error),
            ("[]", 200, EngineStatus::Error),
            (r#"{"jobs":null}"#, 200, EngineStatus::Error),
            (r#"{"jobs":[{"offices":null}]}"#, 200, EngineStatus::Error),
            (r#"{"jobs":[{"metadata":null}]}"#, 200, EngineStatus::Error),
            (r#"{"jobs":[{"offices":[null]}]}"#, 200, EngineStatus::Error),
            (
                r#"{"jobs":[{"metadata":[null]}]}"#,
                200,
                EngineStatus::Error,
            ),
            (
                r#"{"jobs":[{"metadata":[{"name":null}]}]}"#,
                200,
                EngineStatus::Error,
            ),
            (
                r#"{"jobs":[{"metadata":[{"name":"salary","value":1}]}]}"#,
                200,
                EngineStatus::Error,
            ),
            (
                r#"{"jobs":[{"id":0,"title":"Zero ID","offices":[],"metadata":[]}] }"#,
                200,
                EngineStatus::Ok,
            ),
            (
                r#"{"jobs":[{"absolute_url":null}]}"#,
                200,
                EngineStatus::Error,
            ),
            (r#"{"jobs":[{"title":null}]}"#, 200, EngineStatus::Error),
            (r#"{"jobs":[{"updated_at":1}]}"#, 200, EngineStatus::Error),
            ("", 404, EngineStatus::Ok),
            ("{}", 429, EngineStatus::RateLimited),
            ("{}", 500, EngineStatus::Error),
        ] {
            let (base, _, server) = loopback(body, status).await;
            let engine = JobsAdapter::greenhouse(
                Arc::new(AdapterHttp::default()),
                Arc::new(super::super::AllowAllAdmission),
                GreenhouseConfig {
                    base_url: base,
                    ..Default::default()
                },
            );
            let result = run(&engine, "Engineer at Acme").await;
            let _ = server.await;
            assert_eq!(result.status, expected);
            if !body.contains("Zero ID") {
                assert!(result.results.is_empty());
            }
            if body == r#"{"jobs":[{"absolute_url":null}]}"# {
                assert_eq!(
                    result.error_message.as_deref(),
                    Some(
                        "provider field greenhouse absolute_url cannot be represented by the typed Rust result"
                    )
                );
            }
            if body == r#"{"jobs":[{"title":null}]}"# {
                assert_eq!(
                    result.error_message.as_deref(),
                    Some(
                        "provider field greenhouse title cannot be represented by the typed Rust result"
                    )
                );
            }
            if body.contains("Zero ID") {
                assert_eq!(result.results.len(), 1);
                assert_eq!(
                    result.results[0].url,
                    "https://boards.greenhouse.io/acme/jobs/0"
                );
                assert!(
                    result.results[0]
                        .payload
                        .as_ref()
                        .unwrap()
                        .get("data")
                        .unwrap()
                        .get("job_id")
                        .is_none()
                );
            }
        }
        for (body, expected_status, expected) in [
            ("{\"jobs\":[]}", EngineStatus::Ok, 0),
            (
                "{\"data\":{\"jobBoard\":{\"jobPostings\":[]}}}",
                EngineStatus::Ok,
                0,
            ),
            (
                "{\"data\":{\"jobBoard\":{\"jobPostings\":[{\"id\":null,\"title\":null,\"locationName\":null,\"salaryRange\":null,\"departmentName\":null,\"updatedAt\":null}]}}}",
                EngineStatus::Ok,
                1,
            ),
            ("[]", EngineStatus::Error, 0),
            ("{\"data\":false}", EngineStatus::Ok, 0),
            ("{\"data\":0}", EngineStatus::Ok, 0),
            ("{\"data\":[]}", EngineStatus::Ok, 0),
            ("{\"data\":\"\"}", EngineStatus::Ok, 0),
            ("{\"data\":{}}", EngineStatus::Ok, 0),
            ("{\"data\":1}", EngineStatus::Error, 0),
            ("{\"data\":{\"jobBoard\":{}}}", EngineStatus::Ok, 0),
            ("{\"data\":{\"jobBoard\":[]}}", EngineStatus::Ok, 0),
            ("{\"data\":{\"jobBoard\":false}}", EngineStatus::Ok, 0),
            (
                "{\"data\":{\"jobBoard\":{\"jobPostings\":false}}}",
                EngineStatus::Ok,
                0,
            ),
            (
                "{\"data\":{\"jobBoard\":{\"jobPostings\":0}}}",
                EngineStatus::Ok,
                0,
            ),
            (
                "{\"data\":{\"jobBoard\":{\"jobPostings\":{}}}}",
                EngineStatus::Ok,
                0,
            ),
            ("{\"data\":\"truthy\"}", EngineStatus::Error, 0),
            (
                "{\"data\":{\"jobBoard\":{\"jobPostings\":\"truthy\"}}}",
                EngineStatus::Error,
                0,
            ),
            (
                "{\"data\":{\"jobBoard\":{\"jobPostings\":[{\"locationName\":1}]}}}",
                EngineStatus::Error,
                0,
            ),
        ] {
            let (base, _, server) = loopback(body, 200).await;
            let engine = JobsAdapter::ashby(
                Arc::new(AdapterHttp::default()),
                Arc::new(super::super::AllowAllAdmission),
                AshbyConfig {
                    base_url: base,
                    ..Default::default()
                },
            );
            let result = run(&engine, "Engineer at Acme").await;
            let _ = server.await;
            assert_eq!(result.status, expected_status);
            assert_eq!(result.results.len(), expected);
            if body == "{\"data\":{\"jobBoard\":{\"jobPostings\":[{\"locationName\":1}]}}}" {
                assert_eq!(
                    result.error_message.as_deref(),
                    Some("ashby content part must be a string")
                );
            }
            if expected == 1 {
                let job = &result.results[0];
                assert_eq!(job.url, "https://jobs.ashbyhq.com/acme/");
                assert!(job.title.is_empty());
                assert_eq!(job.content, "Acme");
                assert!(job.published_date.is_none());
            }
        }
        for (status, expected) in [
            (404, EngineStatus::Ok),
            (429, EngineStatus::RateLimited),
            (500, EngineStatus::Error),
        ] {
            let (base, _, server) = loopback("", status).await;
            let engine = JobsAdapter::ashby(
                Arc::new(AdapterHttp::default()),
                Arc::new(super::super::AllowAllAdmission),
                AshbyConfig {
                    base_url: base,
                    ..Default::default()
                },
            );
            let result = run(&engine, "Engineer at Acme").await;
            let _ = server.await;
            assert_eq!(result.status, expected);
            assert!(result.results.is_empty());
        }

        for (body, expected_status, expected_count) in [
            ("[]", EngineStatus::Ok, 0),
            (r#"{"error":"not found"}"#, EngineStatus::Ok, 0),
            (r#"[{"categories":false}]"#, EngineStatus::Ok, 1),
            (r#"[{"categories":[]}]"#, EngineStatus::Ok, 1),
            (r#"[{"categories":{}}]"#, EngineStatus::Ok, 1),
            (r#"[{"categories":""}]"#, EngineStatus::Ok, 1),
            (r#"[{"categories":null}]"#, EngineStatus::Ok, 1),
            (r#"[{"categories":0}]"#, EngineStatus::Ok, 1),
            (r#"[{"categories":"truthy"}]"#, EngineStatus::Error, 0),
            (r#"[{"categories":[1]}]"#, EngineStatus::Error, 0),
            (r#"[{"categories":{"location":1}}]"#, EngineStatus::Error, 0),
            (r#"[{"hostedUrl":null}]"#, EngineStatus::Error, 0),
            (r#"[{"createdAt":"not-a-number"}]"#, EngineStatus::Error, 0),
            (r#"[{"createdAt":1e30}]"#, EngineStatus::Error, 0),
            (r#"[{"createdAt":-0.0}]"#, EngineStatus::Ok, 1),
            (r#"[{"createdAt":-0.5}]"#, EngineStatus::Ok, 1),
            (
                r#"[{"id":"without-date","text":"Engineer","categories":{"location":"Remote"},"hostedUrl":"https://jobs.lever.co/acme/without-date"}]"#,
                EngineStatus::Ok,
                1,
            ),
        ] {
            let (base, _, server) = loopback(body, 200).await;
            let engine = JobsAdapter::lever(
                Arc::new(AdapterHttp::default()),
                Arc::new(super::super::AllowAllAdmission),
                LeverConfig {
                    base_url: format!("{base}/v0/postings"),
                    ..Default::default()
                },
            );
            let result = run(&engine, "Engineer at Acme").await;
            let _ = server.await;
            assert_eq!(result.status, expected_status);
            assert_eq!(result.results.len(), expected_count);
            if body == r#"[{"hostedUrl":null}]"# {
                assert_eq!(
                    result.error_message.as_deref(),
                    Some(
                        "provider field lever hostedUrl cannot be represented by the typed Rust result"
                    )
                );
            }
            if expected_count == 1 {
                if body.contains("without-date") || body.contains("-0.0") {
                    assert!(result.results[0].published_date.is_none());
                }
                if body.contains("-0.5") {
                    assert_eq!(
                        result.results[0].published_date.as_deref(),
                        Some("1969-12-31T23:59:59.999500+00:00")
                    );
                }
            }
        }
        for (status, expected) in [
            (404, EngineStatus::Ok),
            (429, EngineStatus::RateLimited),
            (500, EngineStatus::Error),
        ] {
            let (base, _, server) = loopback("", status).await;
            let engine = JobsAdapter::lever(
                Arc::new(AdapterHttp::default()),
                Arc::new(super::super::AllowAllAdmission),
                LeverConfig {
                    base_url: format!("{base}/v0/postings"),
                    ..Default::default()
                },
            );
            let result = run(&engine, "Engineer at Acme").await;
            let _ = server.await;
            assert_eq!(result.status, expected);
            assert!(result.results.is_empty());
        }
        for (body, expected_date) in [
            (
                r#"[{"id":"date","createdAt":true}]"#,
                "1970-01-01T00:00:00.001000+00:00",
            ),
            (
                r#"[{"id":"date","createdAt":1720000000000.5}]"#,
                "2024-07-03T09:46:40.000500+00:00",
            ),
        ] {
            let (base, _, server) = loopback(body, 200).await;
            let engine = JobsAdapter::lever(
                Arc::new(AdapterHttp::default()),
                Arc::new(super::super::AllowAllAdmission),
                LeverConfig {
                    base_url: format!("{base}/v0/postings"),
                    ..Default::default()
                },
            );
            let result = run(&engine, "Engineer at Acme").await;
            let _ = server.await;
            assert_eq!(result.status, EngineStatus::Ok);
            assert_eq!(result.results.len(), 1);
            assert_eq!(
                result.results[0].published_date.as_deref(),
                Some(expected_date)
            );
        }
    }

    #[tokio::test]
    async fn provider_timeout_and_dropped_search_cancel_the_loopback_exchange() {
        let _case = CompletedCase(
            "legacy.provider_timeout_and_dropped_search_cancel_the_loopback_exchange",
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let delayed_server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0u8; 1024];
            loop {
                let n = socket.read(&mut chunk).await.unwrap();
                if n == 0 {
                    return;
                }
                bytes.extend_from_slice(&chunk[..n]);
                if bytes.windows(4).any(|v| v == b"\r\n\r\n") {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
            let body = "{\"jobs\":[]}";
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
        });
        let engine = JobsAdapter::greenhouse(
            Arc::new(AdapterHttp::default()),
            Arc::new(super::super::AllowAllAdmission),
            GreenhouseConfig {
                base_url: format!("http://{address}/v1/boards"),
                timeout_ms: 20.into(),
                ..Default::default()
            },
        );
        let result = run(&engine, "Engineer at TimeoutCo").await;
        assert_eq!(result.status, EngineStatus::Timeout);
        assert!(result.results.is_empty());
        delayed_server.await.unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
        let peer = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0u8; 1024];
            loop {
                let n = socket.read(&mut chunk).await.unwrap();
                if n == 0 {
                    return false;
                }
                bytes.extend_from_slice(&chunk[..n]);
                if bytes.windows(4).any(|v| v == b"\r\n\r\n") {
                    break;
                }
            }
            let _ = accepted_tx.send(());
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    if socket.read(&mut chunk).await.unwrap() == 0 {
                        return true;
                    }
                }
            })
            .await
            .unwrap_or(false)
        });
        let engine = JobsAdapter::lever(
            Arc::new(AdapterHttp::default()),
            Arc::new(super::super::AllowAllAdmission),
            LeverConfig {
                base_url: format!("http://{address}/v0/postings"),
                timeout_ms: 5_000.into(),
                ..Default::default()
            },
        );
        let req = request("Engineer at CancelCo");
        let search = tokio::spawn(async move { engine.search(&req).await });
        accepted_rx.await.unwrap();
        search.abort();
        let _ = search.await;
        assert!(
            peer.await.unwrap(),
            "dropping search must close its active HTTP exchange"
        );
    }
    struct CompletedCase(&'static str);
    impl Drop for CompletedCase {
        fn drop(&mut self) {
            if !std::thread::panicking() {
                mark(self.0)
            }
        }
    }
    fn mark(id: impl AsRef<str>) {
        println!("A14_CASE {}", id.as_ref());
    }
    fn provider_name(kind: Kind) -> &'static str {
        kind.name()
    }
    const PROVIDERS: [Kind; 3] = [Kind::Ashby, Kind::Greenhouse, Kind::Lever];
    fn wrap(kind: Kind, rows: &str) -> String {
        match kind {
            Kind::Ashby => format!(r#"{{"data":{{"jobBoard":{{"jobPostings":{rows}}}}}}}"#),
            Kind::Greenhouse => format!(r#"{{"jobs":{rows}}}"#),
            Kind::Lever => rows.into(),
        }
    }
    fn title_rows(kind: Kind) -> &'static str {
        match kind {
            Kind::Lever => r#"[{"text":"first"},{"text":"second"},{"text":"third"}]"#,
            _ => r#"[{"title":"first"},{"title":"second"},{"title":"third"}]"#,
        }
    }
    fn resolved_factory(
        kind: Kind,
        base: &str,
        timeout: ConfigScalar,
        limit: ConfigScalar,
        http: Arc<AdapterHttp>,
    ) -> Box<dyn SearchEngine> {
        use std::collections::BTreeMap;
        audit_loopback_proxies();
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        // No unresolved default/provider URL can reach a test factory.
        let url = Url::parse(base).unwrap();
        assert_eq!(url.scheme(), "http");
        assert!(matches!(url.host(),Some(url::Host::Ipv4(ip))if ip.is_loopback()));
        assert!(url.port().is_some());
        for name in [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
        ] {
            assert!(
                std::env::var(name).map_or(true, |v| v.is_empty()),
                "proxy enabled: {name}"
            );
        }
        let path = std::env::temp_dir().join(format!(
            "ssx-a14-config-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let input = serde_json::json!({"engines":{kind.name():{"enabled":true,"base_url":base,"timeout_ms":match timeout {ConfigScalar::Float(f) if !f.is_finite()=>ConfigScalar::Float(1.5),_=>timeout.clone()},"max_results":limit,"categories":["jobs"]}}});
        std::fs::write(&path, serde_json::to_vec(&input).unwrap()).unwrap();
        let mut env = BTreeMap::new();
        if let ConfigScalar::Float(f) = timeout
            && !f.is_finite()
        {
            env.insert(
                format!("ENGINE_{}_TIMEOUT_MS", kind.name().to_uppercase()),
                if f.is_nan() {
                    "nan"
                } else if f > 0. {
                    "inf"
                } else {
                    "-inf"
                }
                .to_owned(),
            );
        }
        let config = ssx_config::load_config_with(Some(&path), &env).unwrap();
        std::fs::remove_file(path).unwrap();
        let entry = &config.engines[kind.name()];
        assert_eq!(entry.timeout_ms, timeout);
        assert_eq!(entry.max_results, limit);
        assert_eq!(entry.categories, Some(vec!["jobs".into()]));
        let configs = JobsConfig::from_resolved(&config);
        let (actual_timeout, actual_limit) = match kind {
            Kind::Ashby => (
                configs.ashby.timeout_ms.clone(),
                configs.ashby.max_results.clone(),
            ),
            Kind::Greenhouse => (
                configs.greenhouse.timeout_ms.clone(),
                configs.greenhouse.max_results.clone(),
            ),
            Kind::Lever => (
                configs.lever.timeout_ms.clone(),
                configs.lever.max_results.clone(),
            ),
        };
        assert_eq!(actual_timeout, timeout);
        assert_eq!(actual_limit, limit);
        let mut registry = ssx_registry::Registry::builtins().unwrap();
        super::super::register_jobs(
            &mut registry,
            http,
            Arc::new(super::super::AllowAllAdmission),
            configs,
        )
        .unwrap();
        for name in ["ashby", "greenhouse", "lever"] {
            assert!(registry.is_available(name));
            let d = registry.descriptor(name).unwrap();
            assert_eq!(d.categories, ["jobs"]);
            assert!(!d.categories.iter().any(|s| s == "general"));
        }
        registry.instantiate(kind.name()).unwrap()
    }
    async fn fixture(
        kind: Kind,
        body: impl AsRef<[u8]>,
        status: u16,
        limit: ConfigScalar,
    ) -> AdapterResponse {
        let (base, seen, server) = loopback(body, status).await;
        let http = Arc::new(AdapterHttp::default());
        let engine = resolved_factory(kind, &base, 5000.into(), limit, http.clone());
        let result = engine.search(&request("Engineer at Acme")).await;
        server.await.unwrap();
        assert_eq!(seen.lock().unwrap().len(), 1);
        http.shutdown_and_wait().await;
        result
    }
    #[tokio::test]
    async fn resolved_f06_factories_preserve_slice_kind_and_signed_limits() {
        for kind in PROVIDERS {
            for (name, limit, count, error) in [
                ("positive", ConfigScalar::Integer(2), 2, false),
                ("negative", ConfigScalar::Integer(-1), 2, false),
                ("zero", ConfigScalar::Integer(0), 0, false),
                ("false", ConfigScalar::Boolean(false), 0, false),
                ("true", ConfigScalar::Boolean(true), 1, false),
                ("signed_min", ConfigScalar::Integer(i64::MIN), 0, false),
                ("signed_max", ConfigScalar::Integer(i64::MAX), 3, false),
                ("integral_float", ConfigScalar::Float(1.), 0, true),
                ("fractional_float", ConfigScalar::Float(1.5), 0, true),
                ("text", ConfigScalar::Text("1".into()), 0, true),
            ] {
                let result = fixture(kind, wrap(kind, title_rows(kind)), 200, limit).await;
                assert_eq!(
                    result.status,
                    if error {
                        EngineStatus::Error
                    } else {
                        EngineStatus::Ok
                    },
                    "{} {name}: {:?}",
                    kind.name(),
                    result.error_message
                );
                assert_eq!(result.results.len(), count);
                if error {
                    assert_eq!(
                        result.error_message.as_deref(),
                        Some("slice indices must be integers")
                    )
                } else {
                    for (i, row) in result.results.iter().enumerate() {
                        assert_eq!(row.position, i as i64 + 1);
                        assert_eq!(row.category, "jobs");
                        assert_eq!(row.tier, 2);
                        assert_eq!(row.title, ["first", "second", "third"][i]);
                    }
                }
                mark(format!("{}.config.{name}", provider_name(kind)));
            }
        }
    }
    #[tokio::test]
    async fn source_response_bytes_decode_before_only_retained_output_projection() {
        for kind in PROVIDERS {
            let row = if matches!(kind, Kind::Lever) {
                r#"[{"text":"Engineer","unused":[NaN,Infinity,-Infinity,1e999,"\ud800"]}]"#
            } else {
                r#"[{"title":"Engineer","unused":[NaN,Infinity,-Infinity,1e999,"\ud800"]}]"#
            };
            let json = wrap(kind, row);
            let le16 = json
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>();
            let be16 = json
                .encode_utf16()
                .flat_map(u16::to_be_bytes)
                .collect::<Vec<_>>();
            let le32 = json
                .chars()
                .flat_map(|c| (c as u32).to_le_bytes())
                .collect::<Vec<_>>();
            let be32 = json
                .chars()
                .flat_map(|c| (c as u32).to_be_bytes())
                .collect::<Vec<_>>();
            for (name, bytes) in [
                ("utf8", json.as_bytes().to_vec()),
                (
                    "utf8_bom",
                    [&[0xef, 0xbb, 0xbf][..], json.as_bytes()].concat(),
                ),
                ("utf16le", le16.clone()),
                ("utf16be", be16.clone()),
                ("utf16le_bom", [&[0xff, 0xfe][..], &le16].concat()),
                ("utf16be_bom", [&[0xfe, 0xff][..], &be16].concat()),
                ("utf32le", le32.clone()),
                ("utf32be", be32.clone()),
                ("utf32le_bom", [&[0xff, 0xfe, 0, 0][..], &le32].concat()),
                ("utf32be_bom", [&[0, 0, 0xfe, 0xff][..], &be32].concat()),
            ] {
                let result = fixture(kind, bytes, 200, 10.into()).await;
                assert_eq!(
                    result.status,
                    EngineStatus::Ok,
                    "{} {name}: {:?}",
                    kind.name(),
                    result.error_message
                );
                assert_eq!(result.results.len(), 1);
                let row = &result.results[0];
                assert_eq!(row.title, "Engineer");
                assert_eq!(row.content, "Acme");
                assert_eq!(row.category, "jobs");
                assert_eq!(row.tier, 2);
                mark(format!("{}.decode.{name}", kind.name()));
            }
        }
    }
    #[tokio::test]
    async fn source_iterables_slice_before_row_access_and_atomic_failure_precedence() {
        for (kind, name, body, limit, status, count, diagnostic) in [
            (
                Kind::Greenhouse,
                "jobs_empty_string",
                r#"{"jobs":""}"#,
                10,
                EngineStatus::Ok,
                0,
                None,
            ),
            (
                Kind::Greenhouse,
                "empty_offices_map",
                r#"{"jobs":[{"offices":{}}]}"#,
                10,
                EngineStatus::Ok,
                1,
                None,
            ),
            (
                Kind::Greenhouse,
                "empty_offices_string",
                r#"{"jobs":[{"offices":""}]}"#,
                10,
                EngineStatus::Ok,
                1,
                None,
            ),
            (
                Kind::Greenhouse,
                "empty_metadata_map",
                r#"{"jobs":[{"metadata":{}}]}"#,
                10,
                EngineStatus::Ok,
                1,
                None,
            ),
            (
                Kind::Greenhouse,
                "empty_metadata_string",
                r#"{"jobs":[{"metadata":""}]}"#,
                10,
                EngineStatus::Ok,
                1,
                None,
            ),
            (
                Kind::Greenhouse,
                "truthy_string_slice_zero",
                r#"{"jobs":"x"}"#,
                0,
                EngineStatus::Ok,
                0,
                None,
            ),
            (
                Kind::Ashby,
                "truthy_string_slice_zero",
                r#"{"data":{"jobBoard":{"jobPostings":"x"}}}"#,
                0,
                EngineStatus::Ok,
                0,
                None,
            ),
            (
                Kind::Greenhouse,
                "map_nonsliceable_zero",
                r#"{"jobs":{}}"#,
                0,
                EngineStatus::Error,
                0,
                Some("job collection is not sliceable"),
            ),
            (
                Kind::Ashby,
                "truthy_map_nonsliceable_zero",
                r#"{"data":{"jobBoard":{"jobPostings":{"x":1}}}}"#,
                0,
                EngineStatus::Error,
                0,
                Some("job collection is not sliceable"),
            ),
            (
                Kind::Greenhouse,
                "null_title_source_success_f05_refusal",
                r#"{"jobs":[{"title":null}]}"#,
                10,
                EngineStatus::Error,
                0,
                Some(
                    "provider field greenhouse title cannot be represented by the typed Rust result",
                ),
            ),
            (
                Kind::Greenhouse,
                "null_url_source_success_f05_refusal",
                r#"{"jobs":[{"absolute_url":null}]}"#,
                10,
                EngineStatus::Error,
                0,
                Some(
                    "provider field greenhouse absolute_url cannot be represented by the typed Rust result",
                ),
            ),
            (
                Kind::Greenhouse,
                "pending_title_then_same_row_source_error",
                r#"{"jobs":[{"title":null,"offices":null}]}"#,
                10,
                EngineStatus::Error,
                0,
                Some("field is not iterable"),
            ),
            (
                Kind::Greenhouse,
                "pending_title_then_later_source_error",
                r#"{"jobs":[{"title":null},{"offices":null}]}"#,
                10,
                EngineStatus::Error,
                0,
                Some("field is not iterable"),
            ),
            (
                Kind::Greenhouse,
                "valid_then_later_source_error",
                r#"{"jobs":[{"title":"first"},{"offices":null}]}"#,
                10,
                EngineStatus::Error,
                0,
                Some("field is not iterable"),
            ),
            (
                Kind::Ashby,
                "pending_title_then_later_source_error",
                r#"{"data":{"jobBoard":{"jobPostings":[{"title":1},{"locationName":1}]}}}"#,
                10,
                EngineStatus::Error,
                0,
                Some("ashby content part must be a string"),
            ),
            (
                Kind::Ashby,
                "valid_then_later_source_error",
                r#"{"data":{"jobBoard":{"jobPostings":[{"title":"first"},null]}}}"#,
                10,
                EngineStatus::Error,
                0,
                Some("posting must be an object"),
            ),
            (
                Kind::Lever,
                "pending_url_then_later_source_error",
                r#"[{"hostedUrl":null},{"createdAt":"bad"}]"#,
                10,
                EngineStatus::Error,
                0,
                Some("lever createdAt must be numeric"),
            ),
            (
                Kind::Lever,
                "valid_then_later_source_error",
                r#"[{"text":"first"},{"categories":true}]"#,
                10,
                EngineStatus::Error,
                0,
                Some("lever categories must be an object"),
            ),
            (
                Kind::Greenhouse,
                "valid_then_later_typed_refusal",
                r#"{"jobs":[{"title":"first"},{"title":null}]}"#,
                10,
                EngineStatus::Error,
                0,
                Some(
                    "provider field greenhouse title cannot be represented by the typed Rust result",
                ),
            ),
            (
                Kind::Ashby,
                "valid_then_later_typed_refusal",
                r#"{"data":{"jobBoard":{"jobPostings":[{"title":"first"},{"title":1}]}}}"#,
                10,
                EngineStatus::Error,
                0,
                Some("provider field ashby title cannot be represented by the typed Rust result"),
            ),
            (
                Kind::Lever,
                "valid_then_later_typed_refusal",
                r#"[{"text":"first"},{"text":null}]"#,
                10,
                EngineStatus::Error,
                0,
                Some("provider field lever text cannot be represented by the typed Rust result"),
            ),
        ] {
            let result = fixture(kind, body, 200, limit.into()).await;
            assert_eq!(result.status, status, "{name}");
            assert_eq!(result.results.len(), count, "{name}");
            assert_eq!(result.error_message.as_deref(), diagnostic, "{name}");
            mark(format!("{}.source.{name}", kind.name()));
        }
        for (name, body, expected) in [
            (
                "surrogate_payload_id",
                r#"{"jobs":[{"id":"\ud800","absolute_url":"https://jobs.test/id"}]}"#,
                Some("�"),
            ),
            (
                "unused_surrogate",
                r#"{"jobs":[{"unused":"\ud800"}]}"#,
                None,
            ),
        ] {
            let result = fixture(Kind::Greenhouse, body, 200, 10.into()).await;
            assert_eq!(result.status, EngineStatus::Ok);
            assert_eq!(result.results.len(), 1);
            if let Some(id) = expected {
                assert_eq!(
                    result.results[0]
                        .payload
                        .as_ref()
                        .unwrap()
                        .get("data")
                        .unwrap()
                        .get("job_id")
                        .and_then(as_text),
                    Some(id)
                );
            }
            mark(format!("greenhouse.payload.{name}"));
        }
    }
    #[tokio::test]
    async fn configured_paths_and_source_graphql_document_are_exact() {
        for kind in PROVIDERS {
            let body = wrap(kind, "[]");
            let (base, seen, server) = loopback(body, 200).await;
            let http = Arc::new(AdapterHttp::default());
            let engine = resolved_factory(
                kind,
                &format!("{base}/prefix/"),
                5000.into(),
                10.into(),
                http.clone(),
            );
            let result = engine.search(&request("Engineer at Acme")).await;
            server.await.unwrap();
            assert_eq!(result.status, EngineStatus::Ok);
            let captured = seen.lock().unwrap()[0].clone();
            let expected = match kind {
                Kind::Ashby => "POST /prefix//acme/api/non-user-graphql HTTP/1.1",
                Kind::Greenhouse => "GET /prefix//acme/jobs?content=true HTTP/1.1",
                Kind::Lever => "GET /prefix//acme?mode=json HTTP/1.1",
            };
            assert!(captured.starts_with(expected), "{captured}");
            if matches!(kind, Kind::Ashby) {
                let json: serde_json::Value =
                    serde_json::from_str(captured.split_once("\r\n\r\n").unwrap().1).unwrap();
                let source = include_str!("../../../../engines/ashby.py");
                let query = source
                    .split_once("_ASHBY_QUERY = \"\"\"")
                    .unwrap()
                    .1
                    .split_once("\"\"\"")
                    .unwrap()
                    .0;
                assert_eq!(
                    json,
                    serde_json::json!({"operationName":"ApiJobBoardWithTeams","variables":{},"query":query})
                );
                assert_eq!(query, ASHBY_QUERY);
            }
            http.shutdown_and_wait().await;
            mark(format!(
                "{}.request.trailing_slash_and_document",
                kind.name()
            ));
        }
    }
    #[tokio::test]
    async fn lever_binary64_quotient_then_cpython_half_even_dates() {
        // Expected algorithm derived from the pinned CPython C sources; these
        // controls are native observations, never claimed Python oracle captures.
        for (name, number, expected) in [
            ("positive_half_micro", "0.0005", "1970-01-01T00:00:00+00:00"),
            (
                "negative_half_micro",
                "-0.0005",
                "1970-01-01T00:00:00+00:00",
            ),
            (
                "positive_odd_half",
                "0.0015",
                "1970-01-01T00:00:00.000002+00:00",
            ),
            (
                "negative_odd_half",
                "-0.0015",
                "1969-12-31T23:59:59.999998+00:00",
            ),
            (
                "negative_fraction",
                "-0.5",
                "1969-12-31T23:59:59.999500+00:00",
            ),
            (
                "large_fraction_lost",
                "1720000000000.0005",
                "2024-07-03T09:46:40+00:00",
            ),
            (
                "large_fraction",
                "1720000000000.5",
                "2024-07-03T09:46:40.000500+00:00",
            ),
            (
                "integer_millisecond_rounding",
                "1720000000001",
                "2024-07-03T09:46:40.001000+00:00",
            ),
            (
                "upper_year",
                "253402300799999",
                "9999-12-31T23:59:59.998993+00:00",
            ),
            ("lower_year", "-62135596800000", "0001-01-01T00:00:00+00:00"),
            ("fraction_carry", "999.9995", "1970-01-01T00:00:01+00:00"),
            ("boolean", "true", "1970-01-01T00:00:00.001000+00:00"),
        ] {
            let body = format!(r#"[{{"text":"Engineer","createdAt":{number}}}]"#);
            let result = fixture(Kind::Lever, body, 200, 10.into()).await;
            assert_eq!(
                result.status,
                EngineStatus::Ok,
                "{name}: {:?}",
                result.error_message
            );
            assert_eq!(result.results.len(), 1);
            assert_eq!(
                result.results[0].published_date.as_deref(),
                Some(expected),
                "{name}"
            );
            mark(format!("lever.date.{name}"));
        }
        for (name, number) in [("zero", "0"), ("negative_zero", "-0.0"), ("false", "false")] {
            let result = fixture(
                Kind::Lever,
                format!(r#"[{{"createdAt":{number}}}]"#),
                200,
                10.into(),
            )
            .await;
            assert_eq!(result.status, EngineStatus::Ok);
            assert_eq!(result.results.len(), 1);
            assert!(result.results[0].published_date.is_none());
            mark(format!("lever.date.{name}"));
        }
        for (name, number) in [
            ("above_range", "253402300800000"),
            ("below_range", "-62135596800001"),
            (
                "integer_huge",
                "99999999999999999999999999999999999999999999999999999999999999999999999999999999999",
            ),
            ("nonfinite", "NaN"),
            ("overflow_float", "1e999"),
        ] {
            let result = fixture(
                Kind::Lever,
                format!(r#"[{{"text":"earlier"}},{{"createdAt":{number}}}]"#),
                200,
                10.into(),
            )
            .await;
            assert_eq!(result.status, EngineStatus::Error);
            assert!(result.results.is_empty());
            mark(format!("lever.date.{name}"));
        }
    }
    #[tokio::test]
    async fn every_provider_timeout_drop_and_owner_shutdown_close_and_join_exchange() {
        for kind in PROVIDERS {
            for action in ["timeout", "drop", "shutdown"] {
                let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
                let base = format!("http://{}", listener.local_addr().unwrap());
                let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
                let peer = tokio::spawn(async move {
                    let (mut socket, _) = listener.accept().await.unwrap();
                    let mut bytes = Vec::new();
                    let mut chunk = [0u8; 2048];
                    loop {
                        let n = socket.read(&mut chunk).await.unwrap();
                        if n == 0 {
                            return false;
                        }
                        bytes.extend_from_slice(&chunk[..n]);
                        if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                            let head = String::from_utf8_lossy(&bytes[..end]);
                            let len = head
                                .lines()
                                .find_map(|s| {
                                    s.to_ascii_lowercase()
                                        .strip_prefix("content-length:")
                                        .and_then(|v| v.trim().parse::<usize>().ok())
                                })
                                .unwrap_or(0);
                            if bytes.len() >= end + 4 + len {
                                break;
                            }
                        }
                    }
                    let _ = accepted_tx.send(());
                    tokio::time::timeout(Duration::from_secs(2), async {
                        loop {
                            if socket.read(&mut chunk).await.unwrap() == 0 {
                                return true;
                            }
                        }
                    })
                    .await
                    .unwrap_or(false)
                });
                let http = Arc::new(AdapterHttp::default());
                let timeout = if action == "timeout" {
                    ConfigScalar::Float(40.5)
                } else {
                    5000.into()
                };
                let engine = resolved_factory(kind, &base, timeout, 10.into(), http.clone());
                let task =
                    tokio::spawn(async move { engine.search(&request("Engineer at Acme")).await });
                tokio::time::timeout(Duration::from_secs(2), accepted_rx)
                    .await
                    .unwrap()
                    .unwrap();
                match action {
                    "drop" => {
                        task.abort();
                        assert!(task.await.unwrap_err().is_cancelled());
                    }
                    "shutdown" => {
                        http.shutdown_and_wait().await;
                        let result = task.await.unwrap();
                        assert_eq!(result.status, EngineStatus::Error);
                        assert!(result.results.is_empty());
                    }
                    _ => {
                        let result = task.await.unwrap();
                        assert_eq!(result.status, EngineStatus::Timeout);
                        assert!(result.results.is_empty());
                    }
                }
                assert!(
                    peer.await.unwrap(),
                    "{} {action}: peer must observe EOF",
                    kind.name()
                );
                http.shutdown_and_wait().await;
                assert_eq!(http.retained_pools(), 0);
                mark(format!("{}.lifecycle.{action}", kind.name()));
            }
        }
    }
    #[tokio::test]
    async fn configuration_errors_keep_source_http_and_slice_order() {
        for kind in PROVIDERS {
            for (name, status, body, expected) in [
                ("invalid_slice_after_404", 404, "", EngineStatus::Ok),
                (
                    "invalid_slice_after_429",
                    429,
                    "",
                    EngineStatus::RateLimited,
                ),
                ("invalid_slice_after_500", 500, "", EngineStatus::Error),
                (
                    "invalid_slice_after_json_error",
                    200,
                    "{",
                    EngineStatus::Error,
                ),
            ] {
                let result = fixture(kind, body, status, ConfigScalar::Float(1.)).await;
                assert_eq!(result.status, expected);
                assert!(result.results.is_empty());
                assert_ne!(
                    result.error_message.as_deref(),
                    Some("slice indices must be integers")
                );
                mark(format!("{}.ordering.{name}", kind.name()));
            }
            let body = wrap(kind, "[]");
            let (base, seen, server) = loopback(body, 200).await;
            let http = Arc::new(AdapterHttp::default());
            let engine = resolved_factory(
                kind,
                &base,
                ConfigScalar::Float(5000.5),
                10.into(),
                http.clone(),
            );
            let result = engine.search(&request("Engineer at Acme")).await;
            server.await.unwrap();
            assert_eq!(result.status, EngineStatus::Ok);
            assert_eq!(seen.lock().unwrap().len(), 1);
            http.shutdown_and_wait().await;
            mark(format!("{}.config.fractional_timeout", kind.name()));
            for (name, timeout, expected) in [
                (
                    "timeout_text",
                    ConfigScalar::Text("5000".into()),
                    EngineStatus::Error,
                ),
                (
                    "timeout_zero",
                    ConfigScalar::Integer(0),
                    EngineStatus::Timeout,
                ),
                (
                    "timeout_negative",
                    ConfigScalar::Integer(-1),
                    EngineStatus::Timeout,
                ),
                (
                    "timeout_false",
                    ConfigScalar::Boolean(false),
                    EngineStatus::Timeout,
                ),
            ] {
                let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
                let base = format!("http://{}", listener.local_addr().unwrap());
                let http = Arc::new(AdapterHttp::default());
                let engine = resolved_factory(kind, &base, timeout, 10.into(), http.clone());
                let result = engine.search(&request("Engineer at Acme")).await;
                assert_eq!(result.status, expected);
                assert!(result.results.is_empty());
                assert!(
                    tokio::time::timeout(Duration::from_millis(5), listener.accept())
                        .await
                        .is_err()
                );
                http.shutdown_and_wait().await;
                mark(format!("{}.config.{name}", kind.name()));
            }
            // Parse-stage operation deadline is supplied to the actual decoder.
            let response = Response {
                status: 200,
                url: Url::parse("http://127.0.0.1:1/").unwrap(),
                headers: Default::default(),
                body: wrap(kind, "[]").into_bytes(),
            };
            assert!(matches!(
                parse_provider(
                    kind,
                    response,
                    "Acme",
                    "acme",
                    &10.into(),
                    Some(Instant::now())
                )
                .await,
                Err(ParseFailure::Timeout)
            ));
            mark(format!("{}.lifecycle.parse_deadline", kind.name()));
        }
        let result = fixture(Kind::Lever, "{}", 200, ConfigScalar::Float(1.5)).await;
        assert_eq!(result.status, EngineStatus::Ok);
        assert!(result.results.is_empty());
        mark("lever.ordering.nonlist_before_invalid_slice");
    }
    #[tokio::test]
    async fn original_python_inline_fixtures_match_every_asserted_field() {
        let greenhouse = r#"{"jobs":[{"id":123,"title":"Senior AI Engineer","absolute_url":"https://boards.greenhouse.io/anthropic/jobs/123","offices":[{"name":"San Francisco, CA"}],"metadata":[{"name":"Salary","value":"$200k-$280k"}],"updated_at":"2026-07-01T12:00:00Z"},{"id":456,"title":"ML Research Scientist","absolute_url":"https://boards.greenhouse.io/anthropic/jobs/456","offices":[{"name":"New York, NY"}],"metadata":[],"updated_at":"2026-07-02T12:00:00Z"}],"meta":{"total_count":2}}"#;
        let ashby = r#"{"data":{"jobBoard":{"jobPostings":[{"id":"abc-123","title":"AI Engineer","locationName":"Remote","salaryRange":"$180k-$250k","departmentName":"Engineering","updatedAt":"2026-07-01T12:00:00.000Z"},{"id":"def-456","title":"Product Designer","locationName":"New York, NY","salaryRange":"","departmentName":"Design","updatedAt":"2026-06-30T12:00:00.000Z"}]}}}"#;
        let lever = r#"[{"id":"abc123","text":"Software Engineer","categories":{"location":"San Francisco, CA","commitment":"Full-time","team":"Engineering"},"hostedUrl":"https://jobs.lever.co/stripe/abc123","createdAt":1720000000000},{"id":"def456","text":"Product Manager","categories":{"location":"Remote","commitment":"Full-time","team":"Product"},"hostedUrl":"https://jobs.lever.co/stripe/def456","createdAt":1719900000000}]"#;
        for (kind, body, company, titles, urls, contents, dates) in [
            (
                Kind::Greenhouse,
                greenhouse,
                "Anthropic",
                ["Senior AI Engineer", "ML Research Scientist"],
                [
                    "https://boards.greenhouse.io/anthropic/jobs/123",
                    "https://boards.greenhouse.io/anthropic/jobs/456",
                ],
                [
                    "Anthropic | San Francisco, CA | $200k-$280k",
                    "Anthropic | New York, NY",
                ],
                ["2026-07-01T12:00:00Z", "2026-07-02T12:00:00Z"],
            ),
            (
                Kind::Ashby,
                ashby,
                "Temporal",
                ["AI Engineer", "Product Designer"],
                [
                    "https://jobs.ashbyhq.com/temporal/abc-123",
                    "https://jobs.ashbyhq.com/temporal/def-456",
                ],
                [
                    "Temporal | Remote | Engineering | $180k-$250k",
                    "Temporal | New York, NY | Design",
                ],
                ["2026-07-01T12:00:00.000Z", "2026-06-30T12:00:00.000Z"],
            ),
            (
                Kind::Lever,
                lever,
                "Stripe",
                ["Software Engineer", "Product Manager"],
                [
                    "https://jobs.lever.co/stripe/abc123",
                    "https://jobs.lever.co/stripe/def456",
                ],
                [
                    "Stripe | San Francisco, CA | Full-time | Engineering",
                    "Stripe | Remote | Full-time | Product",
                ],
                ["2024-07-03T09:46:40+00:00", "2024-07-02T06:00:00+00:00"],
            ),
        ] {
            let (base, seen, server) = loopback(body, 200).await;
            let http = Arc::new(AdapterHttp::default());
            let engine = resolved_factory(kind, &base, 5000.into(), 10.into(), http.clone());
            let result = engine
                .search(&request(&format!("Engineer at {company}")))
                .await;
            server.await.unwrap();
            assert_eq!(result.status, EngineStatus::Ok);
            assert!(result.error_message.is_none());
            assert_eq!(result.results.len(), 2);
            for (i, row) in result.results.iter().enumerate() {
                assert_eq!(row.url, urls[i]);
                assert_eq!(row.title, titles[i]);
                assert_eq!(row.content, contents[i]);
                assert_eq!(row.published_date.as_deref(), Some(dates[i]));
                assert_eq!(row.category, "jobs");
                assert_eq!(row.tier, 2);
                assert_eq!(row.position, i as i64 + 1);
                assert_eq!(row.engine, kind.name());
            }
            if matches!(kind, Kind::Greenhouse) {
                let p = result.results[0].payload.as_ref().unwrap();
                assert_eq!(p.get("domain").and_then(as_text), Some("jobs"));
                assert_eq!(p.get("type").and_then(as_text), Some("job"));
                assert_eq!(p.get("schema_version").and_then(as_i64), Some(1));
                let d = p.get("data").unwrap();
                for (k, v) in [
                    ("company", "Anthropic"),
                    ("title", "Senior AI Engineer"),
                    ("location", "San Francisco, CA"),
                    ("salary", "$200k-$280k"),
                ] {
                    assert_eq!(d.get(k).and_then(as_text), Some(v));
                }
                assert_eq!(d.get("job_id").and_then(as_i64), Some(123));
                let provenance = p.get("provenance").unwrap();
                assert_eq!(
                    provenance.get("engine").and_then(as_text),
                    Some("greenhouse")
                );
                let fields = provenance
                    .get("adapter_fields")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(fields, ["company", "title", "location", "salary", "job_id"]);
                let d = result.results[1]
                    .payload
                    .as_ref()
                    .unwrap()
                    .get("data")
                    .unwrap();
                assert_eq!(
                    d.get("title").and_then(as_text),
                    Some("ML Research Scientist")
                );
                assert!(d.get("salary").is_none());
                assert_eq!(d.get("job_id").and_then(as_i64), Some(456));
            }
            assert_eq!(seen.lock().unwrap().len(), 1);
            http.shutdown_and_wait().await;
            mark(format!("{}.original.inline_fixture", kind.name()));
        }
    }
    #[tokio::test]
    async fn resolved_extreme_numeric_timeouts_use_the_finite_caller_lifecycle() {
        for kind in PROVIDERS {
            for (name, timeout) in [
                ("infinite_with_context", ConfigScalar::Float(f64::INFINITY)),
                ("large_with_context", ConfigScalar::Float(f64::MAX)),
                ("signed_max_with_context", ConfigScalar::Integer(i64::MAX)),
            ] {
                let (base, seen, server) = loopback(wrap(kind, "[]"), 200).await;
                let http = Arc::new(AdapterHttp::default());
                let engine = resolved_factory(kind, &base, timeout, 10.into(), http.clone());
                let result = engine
                    .search_with_context(
                        &request("Engineer at Acme"),
                        SearchContext {
                            deadline: Instant::now() + Duration::from_secs(2),
                        },
                    )
                    .await;
                server.await.unwrap();
                assert_eq!(result.status, EngineStatus::Ok);
                assert!(result.results.is_empty());
                assert_eq!(seen.lock().unwrap().len(), 1);
                http.shutdown_and_wait().await;
                mark(format!("{}.config.{name}", kind.name()));
            }
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let http = Arc::new(AdapterHttp::default());
            let engine = resolved_factory(
                kind,
                &base,
                ConfigScalar::Float(f64::INFINITY),
                10.into(),
                http.clone(),
            );
            let result = engine.search(&request("Engineer at Acme")).await;
            assert_eq!(result.status, EngineStatus::Error);
            assert_eq!(
                result.error_message.as_deref(),
                Some("timeout requires a finite lifecycle deadline")
            );
            assert!(result.results.is_empty());
            assert!(
                tokio::time::timeout(Duration::from_millis(5), listener.accept())
                    .await
                    .is_err()
            );
            http.shutdown_and_wait().await;
            mark(format!("{}.config.infinite_without_context", kind.name()));
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let peer = tokio::spawn(async move {
                if let Ok(Ok((mut socket, _))) =
                    tokio::time::timeout(Duration::from_millis(100), listener.accept()).await
                {
                    let mut chunk = [0u8; 2048];
                    let _ = tokio::time::timeout(Duration::from_secs(2), async {
                        loop {
                            if socket.read(&mut chunk).await.unwrap_or(0) == 0 {
                                break;
                            }
                        }
                    })
                    .await;
                }
            });
            let http = Arc::new(AdapterHttp::default());
            let engine = resolved_factory(
                kind,
                &base,
                ConfigScalar::Boolean(true),
                10.into(),
                http.clone(),
            );
            let result = engine.search(&request("Engineer at Acme")).await;
            assert_eq!(result.status, EngineStatus::Timeout);
            assert!(result.results.is_empty());
            http.shutdown_and_wait().await;
            peer.await.unwrap();
            mark(format!("{}.config.timeout_true", kind.name()));
        }
    }
    const SYSTEM_PROXY_FLAGS: [&str; 5] = [
        "HTTPEnable",
        "HTTPSEnable",
        "SOCKSEnable",
        "ProxyAutoConfigEnable",
        "ProxyAutoDiscoveryEnable",
    ];
    fn system_proxy_enabled(flags: &[(String, i32)]) -> bool {
        flags.iter().any(|(_, value)| *value != 0)
    }
    fn audit_loopback_proxies() {
        // Every dispatch path, including the legacy run helper, checks the
        // current environment before the shared system-proxy audit.
        for name in [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
        ] {
            assert!(
                std::env::var(name).map_or(true, |value| value.is_empty()),
                "proxy enabled: {name}"
            );
        }
        static SAFE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        SAFE.get_or_init(|| {
            #[cfg(target_os = "macos")]
            {
                let output = std::process::Command::new("/usr/sbin/scutil")
                    .arg("--proxy")
                    .output()
                    .expect("system proxy audit must execute before loopback dispatch");
                assert!(output.status.success(), "system proxy audit failed closed");
                let raw = String::from_utf8(output.stdout).expect("system proxy audit encoding");
                let flags = SYSTEM_PROXY_FLAGS.map(|key| {
                    let values = raw
                        .lines()
                        .filter_map(|line| {
                            let (name, value) = line.trim().split_once(':')?;
                            (name.trim() == key).then(|| {
                                value
                                    .trim()
                                    .parse::<i32>()
                                    .expect("proxy flag must be an integer")
                            })
                        })
                        .collect::<Vec<_>>();
                    assert!(values.len() <= 1, "ambiguous system proxy flag");
                    (key.to_owned(), values.first().copied().unwrap_or(0))
                });
                assert!(
                    !system_proxy_enabled(&flags),
                    "system proxy enabled: loopback execution denied"
                );
                println!(
                    "A14_SYSTEM_PROXY_FLAGS {}",
                    serde_json::to_string(&flags).unwrap()
                );
            }
        });
    }
    #[test]
    fn system_proxy_guard_rejects_every_enabled_system_flag() {
        let disabled = SYSTEM_PROXY_FLAGS.map(|key| (key.to_owned(), 0));
        assert!(!system_proxy_enabled(&disabled));
        mark("guard.system_proxy.disabled");
        for key in SYSTEM_PROXY_FLAGS {
            let enabled = SYSTEM_PROXY_FLAGS.map(|name| (name.to_owned(), i32::from(name == key)));
            assert!(system_proxy_enabled(&enabled));
            mark(format!("guard.system_proxy.{key}"));
        }
    }
}
