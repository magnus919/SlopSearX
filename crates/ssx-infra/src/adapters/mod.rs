//! Native provider adapters currently ported from the registry-search pilot.
pub mod arxiv;
mod arxiv_accounting;
mod arxiv_xml;
mod brave;
mod community;
mod crates_io;
mod github;
mod hackernews;
mod huggingface;
mod jobs;
mod npm;
mod reddit;
pub mod source_json;
mod stackexchange;
mod wikipedia;

use crate::http::AdapterHttp;
use ssx_application::rate::RateLimiter;
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission};
use ssx_domain::rate::{EngineKey, RateReason};
use ssx_registry::{EngineFactory, Registry, RegistryError};
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Instant;

pub use crates_io::CratesConfig;
pub use jobs::{AshbyConfig, GreenhouseConfig, JobsConfig, LeverConfig};
pub use npm::NpmConfig;

pub use brave::BraveConfig;
pub use reddit::RedditConfig;
pub use wikipedia::WikipediaConfig;

#[derive(Clone, Debug, Default)]
pub struct BroadApiConfig {
    pub brave: BraveConfig,
    pub wikipedia: WikipediaConfig,
    pub reddit: RedditConfig,
}

/// Register the three broad API adapters owned by A04. Kept separate from
/// `register_pilots` so A01's accepted factory contract remains unchanged.
pub fn register_broad_api(
    registry: &mut Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: BroadApiConfig,
) -> Result<(), RegistryError> {
    let engine_http = http.clone();
    let engine_admission = admission.clone();
    let engine_config = config.brave;
    registry.register(
        "brave",
        Arc::new(move || {
            Box::new(brave::BraveAdapter::new(
                engine_http.clone(),
                engine_admission.clone(),
                engine_config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    let engine_http = http.clone();
    let engine_admission = admission.clone();
    let engine_config = config.wikipedia;
    registry.register(
        "wikipedia",
        Arc::new(move || {
            Box::new(wikipedia::WikipediaAdapter::new(
                engine_http.clone(),
                engine_admission.clone(),
                engine_config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    let engine_http = http;
    let engine_admission = admission;
    let engine_config = config.reddit;
    registry.register(
        "reddit",
        Arc::new(move || {
            Box::new(reddit::RedditAdapter::new(
                engine_http.clone(),
                engine_admission.clone(),
                engine_config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    Ok(())
}

/// Resolved source-compatible settings for the A01 pilot adapters.
#[derive(Clone, Debug, Default)]
pub struct PilotConfig {
    pub npm: NpmConfig,
    pub crates: CratesConfig,
}

/// Attach the two implemented F06 descriptors to their executable factories.
/// The caller owns the shared transport lifetime; adapters never close it.
pub fn register_pilots(
    registry: &mut Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: PilotConfig,
) -> Result<(), RegistryError> {
    let npm_http = http.clone();
    let npm_config = config.npm;
    let npm_admission = admission.clone();
    registry.register(
        "npm",
        Arc::new(move || {
            Box::new(npm::NpmAdapter::new(
                npm_http.clone(),
                npm_admission.clone(),
                npm_config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    let crates_http = http;
    let crates_config = config.crates;
    let crates_admission = admission;
    registry.register(
        "crates",
        Arc::new(move || {
            Box::new(crates_io::CratesAdapter::new(
                crates_http.clone(),
                crates_admission.clone(),
                crates_config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    Ok(())
}

/// Register the three company-scoped ATS providers owned by A14.
pub fn register_jobs(
    registry: &mut Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: JobsConfig,
) -> Result<(), RegistryError> {
    let engine_http = http.clone();
    let engine_admission = admission.clone();
    let engine_config = config.ashby;
    registry.register(
        "ashby",
        Arc::new(move || {
            Box::new(jobs::AshbyAdapter::ashby(
                engine_http.clone(),
                engine_admission.clone(),
                engine_config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    let engine_http = http.clone();
    let engine_admission = admission.clone();
    let engine_config = config.greenhouse;
    registry.register(
        "greenhouse",
        Arc::new(move || {
            Box::new(jobs::GreenhouseAdapter::greenhouse(
                engine_http.clone(),
                engine_admission.clone(),
                engine_config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    let engine_http = http;
    let engine_admission = admission;
    let engine_config = config.lever;
    registry.register(
        "lever",
        Arc::new(move || {
            Box::new(jobs::LeverAdapter::lever(
                engine_http.clone(),
                engine_admission.clone(),
                engine_config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    Ok(())
}

/// Explicit no-quota mode for tests and deployments with no configured limiter.
#[derive(Default)]
pub struct AllowAllAdmission;
impl SearchAdmission for AllowAllAdmission {
    fn admit<'a>(&'a self, _: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async { AdmissionDecision::Allowed })
    }
}

/// Adapter-facing wrapper around the accepted F10 policy; quota behavior stays
/// in `RateLimiter`, while this bridge translates its classified decision.
pub struct F10Admission {
    limiter: Arc<tokio::sync::Mutex<RateLimiter>>,
}
impl F10Admission {
    /// The composition root retains this handle for lifecycle, reactivation and
    /// cooldown operations; the adapter bridge handles quota admission and
    /// optional upstream cooldown feedback through the same limiter.
    pub fn new(limiter: Arc<tokio::sync::Mutex<RateLimiter>>) -> Self {
        Self { limiter }
    }
}
impl SearchAdmission for F10Admission {
    fn admit<'a>(&'a self, engine: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async move {
            let Ok(key) = EngineKey::new(engine) else {
                return AdmissionDecision::Unavailable;
            };
            let mut limiter = self.limiter.lock().await;
            let now = f10_monotonic_seconds();
            match limiter.acquire(&key, 1, now).await {
                Ok(decision) if decision.allowed => AdmissionDecision::Allowed,
                Ok(decision) if decision.reason == RateReason::QuotaUnavailable => {
                    AdmissionDecision::Unavailable
                }
                Ok(_) => AdmissionDecision::Denied,
                Err(_) => AdmissionDecision::Unavailable,
            }
        })
    }
    fn set_upstream_cooldown<'a>(
        &'a self,
        engine: &'static str,
        seconds: f64,
    ) -> PortFuture<'a, ()> {
        Box::pin(async move {
            let Ok(key) = EngineKey::new(engine) else {
                return;
            };
            let mut limiter = self.limiter.lock().await;
            // Use the exact same process-local epoch as admission. Source
            // adapters ignore optional cooldown publication failures.
            let _ = limiter
                .set_upstream_cooldown(&key, seconds, f10_monotonic_seconds())
                .await;
        })
    }
}

/// Shared process-local monotonic epoch for all `RateLimiter` operations used
/// with `F10Admission`. Composition roots must pass this value to limiter
/// cooldown APIs; a second `Instant` origin would invalidate stored deadlines.
pub fn f10_monotonic_seconds() -> f64 {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    ORIGIN.get_or_init(Instant::now).elapsed().as_secs_f64()
}

pub use community::CommunitySettings;
pub use github::GitHubConfig;
pub use hackernews::HackerNewsConfig;
pub use huggingface::HuggingFaceConfig;
pub use stackexchange::StackExchangeConfig;

/// A03 settings projected by the composition root; no configuration loader or
/// credential discovery is hidden in an adapter.
#[derive(Clone, Debug, Default)]
pub struct CommunityConfig {
    pub github: GitHubConfig,
    pub huggingface: HuggingFaceConfig,
    pub stackexchange: StackExchangeConfig,
    pub hackernews: HackerNewsConfig,
}
/// Attach A03 factories while retaining caller ownership of transport/admission.
pub fn register_community(
    registry: &mut Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: CommunityConfig,
) -> Result<(), RegistryError> {
    let engine_http = http.clone();
    let engine_admission = admission.clone();
    let engine_config = config.github;
    registry.register(
        "github",
        Arc::new(move || {
            Box::new(github::GitHubAdapter::new(
                engine_http.clone(),
                engine_admission.clone(),
                engine_config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    let engine_http = http.clone();
    let engine_admission = admission.clone();
    let engine_config = config.huggingface;
    registry.register(
        "huggingface",
        Arc::new(move || {
            Box::new(huggingface::HuggingFaceAdapter::new(
                engine_http.clone(),
                engine_admission.clone(),
                engine_config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    let engine_http = http.clone();
    let engine_admission = admission.clone();
    let engine_config = config.stackexchange;
    registry.register(
        "stackexchange",
        Arc::new(move || {
            Box::new(stackexchange::StackExchangeAdapter::new(
                engine_http.clone(),
                engine_admission.clone(),
                engine_config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    let engine_http = http.clone();
    let engine_admission = admission.clone();
    let engine_config = config.hackernews;
    registry.register(
        "hackernews",
        Arc::new(move || {
            Box::new(hackernews::HackerNewsAdapter::new(
                engine_http.clone(),
                engine_admission.clone(),
                engine_config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    Ok(())
}

mod dockerhub;
mod pypi;
mod rubygems;
pub use dockerhub::DockerHubConfig;
pub use pypi::PyPIConfig;
pub use rubygems::RubyGemsConfig;

/// Source-compatible package adapter settings. Caller retains transport/admission ownership.
#[derive(Clone, Debug, Default)]
pub struct PackageConfig {
    pub pypi: PyPIConfig,
    pub rubygems: RubyGemsConfig,
    pub dockerhub: DockerHubConfig,
}

/// Register executable named-engine factories without changing catalog metadata.
pub fn register_packages(
    registry: &mut Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: PackageConfig,
) -> Result<(), RegistryError> {
    let h = http.clone();
    let a = admission.clone();
    let c = config.pypi;
    registry.register(
        "pypi",
        Arc::new(move || {
            Box::new(pypi::PyPIAdapter::new(h.clone(), a.clone(), c.clone()))
                as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    let h = http.clone();
    let a = admission.clone();
    let c = config.rubygems;
    registry.register(
        "rubygems",
        Arc::new(move || {
            Box::new(rubygems::RubyGemsAdapter::new(
                h.clone(),
                a.clone(),
                c.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    let h = http.clone();
    let a = admission.clone();
    let c = config.dockerhub;
    registry.register(
        "dockerhub",
        Arc::new(move || {
            Box::new(dockerhub::DockerHubAdapter::new(
                h.clone(),
                a.clone(),
                c.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    Ok(())
}

/// Expat candidate, not a registered adapter.
pub mod arxiv_expat;

mod arxiv_expat_wire;

pub mod scholarly_wire;
mod scholarly_word;
// Python semantics shared only by the A04 adapters. Keep wire encoding separate
// from URL path handling and apply whitespace conversion only at source calls.
fn broad_query(params: &[(&str, &str)]) -> String {
    fn append(value: &str, out: &mut String) {
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        for b in value.bytes() {
            if b.is_ascii_alphanumeric() || b"_.-~".contains(&b) {
                out.push(char::from(b));
            } else if b == b' ' {
                out.push('+');
            } else {
                out.push('%');
                out.push(char::from(HEX[(b >> 4) as usize]));
                out.push(char::from(HEX[(b & 15) as usize]));
            }
        }
    }
    let mut out = String::new();
    for (index, (key, value)) in params.iter().enumerate() {
        if index != 0 {
            out.push('&');
        }
        append(key, &mut out);
        out.push('=');
        append(value, &mut out);
    }
    out
}
fn broad_whitespace(ch: char) -> bool {
    ch.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&ch)
}
fn broad_truthy(value: Option<&source_json::Value>) -> bool {
    match value {
        None | Some(source_json::Value::Null) => false,
        Some(source_json::Value::Bool(v)) => *v,
        Some(source_json::Value::Number(n)) => {
            if n.is_f64() {
                n.as_f64().is_some_and(|n| n != 0.0)
            } else {
                n.integer_text() != Some("0")
            }
        }
        Some(source_json::Value::String(v)) => !v.is_empty(),
        Some(source_json::Value::Array(v)) => !v.is_empty(),
        Some(source_json::Value::Object(v)) => !v.is_empty(),
    }
}
fn broad_type(value: &source_json::Value) -> &'static str {
    match value {
        source_json::Value::Null => "NoneType",
        source_json::Value::Bool(_) => "bool",
        source_json::Value::Number(n) => {
            if n.is_f64() {
                "float"
            } else {
                "int"
            }
        }
        source_json::Value::String(_) => "str",
        source_json::Value::Array(_) => "list",
        source_json::Value::Object(_) => "dict",
    }
}

fn broad_field_error(
    value: Option<&source_json::Value>,
    optional: bool,
    engine: &str,
    field: &str,
) -> Option<String> {
    match value {
        None => None,
        Some(source_json::Value::String(text)) => text
            .as_str()
            .is_none()
            .then(|| format!("{engine} result {field} must contain only Unicode scalar values")),
        Some(source_json::Value::Null) if optional => None,
        _ => Some(format!(
            "{engine} result {field} must be a string{}",
            if optional { " or null" } else { "" }
        )),
    }
}
fn broad_record_refusal(refusal: &mut Option<String>, error: Option<String>) {
    if refusal.is_none() {
        *refusal = error;
    }
}

fn broad_scalar_output(
    text: &source_json::Text,
    engine: &str,
    field: &str,
    refusal: &mut Option<String>,
) -> String {
    match text.scalar() {
        Some(s) => s,
        None => {
            broad_record_refusal(
                refusal,
                Some(format!(
                    "{engine} result {field} must contain only Unicode scalar values"
                )),
            );
            String::new()
        }
    }
}

pub mod semantic_scholar;

pub mod scholarly_order;

pub mod scholarly_sort;

pub mod scholarly_config;

pub mod source_payload;

pub mod scholarly_frame;

/// Shared charged HTTPX/email charset selection inside supervised helpers.
pub mod http_charset;

mod scholarly_helper;
pub mod scholarly_reply;

mod arxiv_projection;

pub mod arxiv_runtime;

pub mod http_headers;

pub mod hostname_lower;

pub mod source_repr;

pub mod scholarly_float;

pub mod semantic_projection;

pub mod semantic_retry_after;

pub mod semantic_runtime;

pub mod openalex_projection;

#[cfg(test)]
#[path = "../../tests/support/a06_case.rs"]
pub(crate) mod a06_case;
mod duckduckgo;
mod google;
mod html_timeout;
pub mod openalex_runtime;
pub mod scholarly_runtime;
pub use duckduckgo::{DuckDuckGoConfig, ScrapeConfig};
pub use google::GoogleConfig;

/// Executable HTML factories; F06 catalog eligibility remains independent.
#[derive(Clone, Debug, Default)]
pub struct HtmlConfig {
    pub duckduckgo: DuckDuckGoConfig,
    pub google: GoogleConfig,
}
pub fn register_html(
    registry: &mut Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: HtmlConfig,
) -> Result<(), RegistryError> {
    register_html_with_parser(
        registry,
        http,
        admission,
        config,
        crate::html::shared_pool(),
    )
}

/// Register factories sharing a caller-owned parser lifecycle. Retain `parser`
/// and await its shutdown before tearing down the async runtime. Dropping an
/// engine or search future does not replace that explicit cleanup boundary.
pub fn register_html_with_parser(
    registry: &mut Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: HtmlConfig,
    parser: Arc<crate::html::ParserPool>,
) -> Result<(), RegistryError> {
    let h = http.clone();
    let a = admission.clone();
    let c = config.duckduckgo;
    let ddg_parser = parser.clone();
    registry.register(
        "duckduckgo",
        Arc::new(move || {
            Box::new(duckduckgo::DuckDuckGoAdapter::with_parser(
                h.clone(),
                a.clone(),
                c.clone(),
                ddg_parser.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    let c = config.google;
    registry.register(
        "google",
        Arc::new(move || {
            Box::new(google::GoogleAdapter::with_parser(
                http.clone(),
                admission.clone(),
                c.clone(),
                parser.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    Ok(())
}

pub(crate) fn html_detector_flags(raw: &ssx_source_codecs::Text) -> crate::html::DetectorFlags {
    crate::html::DetectorFlags {
        duckduckgo_challenge: duckduckgo::challenge(raw),
        duckduckgo_legitimate_empty: duckduckgo::legitimate_empty(raw),
        google_challenge: google::challenge(raw),
    }
}

/// Same source predicates over charge-owned decoded points. The static maximum
/// is derived from the actual closed pattern lists, not a configurable cap.
pub(crate) fn html_detector_flags_points(
    points: &[u32],
    checkpoint: &dyn Fn() -> Result<(), ssx_source_codecs::BoundedTextError>,
) -> Result<crate::html::DetectorFlags, ssx_source_codecs::BoundedTextError> {
    const fn maximum(patterns: &[&str], mut value: usize) -> usize {
        let mut at = 0;
        while at < patterns.len() {
            if patterns[at].len() > value {
                value = patterns[at].len();
            }
            at += 1;
        }
        value
    }
    const SIZE: usize = maximum(
        google::GOOGLE_CHALLENGE,
        maximum(duckduckgo::DDG_EMPTY, maximum(duckduckgo::DDG_CHALLENGE, 0)),
    );
    let mut scratch = [0usize; SIZE];
    let mut any = |patterns: &[&str]| -> Result<bool, ssx_source_codecs::BoundedTextError> {
        for pattern in patterns {
            if ssx_source_codecs::contains_ascii_lower_with_scratch(
                points,
                pattern,
                &mut scratch[..pattern.len()],
                checkpoint,
            )? {
                return Ok(true);
            }
        }
        Ok(false)
    };
    Ok(crate::html::DetectorFlags {
        duckduckgo_challenge: any(duckduckgo::DDG_CHALLENGE)?,
        duckduckgo_legitimate_empty: any(duckduckgo::DDG_EMPTY)?,
        google_challenge: any(google::GOOGLE_CHALLENGE)?,
    })
}

/// Closed helper-facing config. Invalid types raise only upon a selected row,
/// preserving source valid-empty behavior without copying arbitrary config.
#[derive(Clone, Copy)]
pub(crate) enum RowLimit {
    Number(f64),
    Unlimited,
    Invalid(u8),
    /// A10 slices selected rows before filtering and retains exact signed integers.
    Slice(i64),
    SliceAll,
    InvalidSlice,
}
impl RowLimit {
    fn from_value(value: &ssx_json::Value) -> Self {
        match value {
            ssx_json::Value::Bool(value) => Self::Number(f64::from(u8::from(*value))),
            ssx_json::Value::Number(number) => {
                number.as_f64().map(Self::Number).unwrap_or(Self::Unlimited)
            }
            ssx_json::Value::Null => Self::Invalid(0),
            ssx_json::Value::String(_) => Self::Invalid(1),
            ssx_json::Value::Array(_) => Self::Invalid(2),
            ssx_json::Value::Object(_) => Self::Invalid(3),
        }
    }
    fn reached(self, count: usize) -> Result<bool, &'static str> {
        match self {
            Self::Number(number) => Ok(count as f64 >= number),
            Self::Unlimited => Ok(false),
            Self::Invalid(0) => Err("'>=' not supported between instances of 'int' and 'NoneType'"),
            Self::Invalid(1) => Err("'>=' not supported between instances of 'int' and 'str'"),
            Self::Invalid(2) => Err("'>=' not supported between instances of 'int' and 'list'"),
            Self::Invalid(3) => Err("'>=' not supported between instances of 'int' and 'dict'"),
            Self::Invalid(_) => Err("invalid HTML row-limit tag"),
            Self::Slice(_) | Self::SliceAll | Self::InvalidSlice => {
                Err("A10 slice on A05 row layout")
            }
        }
    }
}

#[cfg(test)]
mod html_helper_tests;

// Child-only row extraction uses no caller query bytes. Image fallback is
// carried as a wire flag and applied by the trusted parent after validation.
pub(crate) fn html_extract_rows(
    doc: &crate::html::Html,
    layout: crate::html::row_wire::Layout,
    limit: RowLimit,
    budget: &ssx_owned_budget::Budget,
) -> Result<ssx_owned_budget::OwnedVec<crate::html::child_rows::Row>, crate::html::OperationFailure>
{
    use crate::html::row_wire::Layout;
    match layout {
        Layout::Google => google::extract_tree(doc, limit, budget),
        other if other.is_a10() => Err(crate::html::OperationFailure::Invariant(
            "A10 layout requires typed child rows",
        )),
        other => duckduckgo::extract_tree(
            doc,
            "",
            limit,
            other == Layout::DdgImage,
            other == Layout::DdgLite,
            budget,
        ),
    }
}

// A07 native biomedical implementations; selection remains resolved policy.
#[cfg(test)]
mod a07_matrix_replay;
pub mod clinicaltrials;
pub mod europepmc;
pub mod pubchem;
pub mod pubmed;
pub mod uniprot;
/// Source-compatible settings for the five biomedical factories. Availability
/// does not grant selection: callers retain the resolved enabled policy.
#[derive(Clone, Debug)]
pub struct BiomedicalSettings {
    pub clinicaltrials: clinicaltrials::BiomedicalConfig,
    pub europepmc: clinicaltrials::BiomedicalConfig,
    pub pubmed: clinicaltrials::BiomedicalConfig,
    pub pubchem: clinicaltrials::BiomedicalConfig,
    pub uniprot: clinicaltrials::BiomedicalConfig,
}
impl BiomedicalSettings {
    /// Apply the source service's base-url normalization before adapter defaults.
    /// Other scalars retain F06's exact resolved variants.
    pub fn from_resolved(config: &ssx_config::Config) -> Self {
        fn resolve(
            config: &ssx_config::Config,
            name: &str,
            endpoint: &str,
        ) -> clinicaltrials::BiomedicalConfig {
            let Some(entry) = config.engines.get(name) else {
                return clinicaltrials::BiomedicalConfig::defaults(endpoint);
            };
            let mut settings = clinicaltrials::BiomedicalConfig::from_resolved(entry);
            if settings.base_url.chars().all(ssx_json::python_whitespace) {
                settings.base_url = endpoint.into();
            }
            settings
        }
        Self {
            clinicaltrials: resolve(
                config,
                "clinicaltrials",
                "https://clinicaltrials.gov/api/v2/studies",
            ),
            europepmc: resolve(
                config,
                "europepmc",
                "https://www.ebi.ac.uk/europepmc/webservices/rest",
            ),
            pubmed: resolve(
                config,
                "pubmed",
                "https://eutils.ncbi.nlm.nih.gov/entrez/eutils",
            ),
            pubchem: resolve(
                config,
                "pubchem",
                "https://pubchem.ncbi.nlm.nih.gov/rest/pug",
            ),
            uniprot: resolve(
                config,
                "uniprot",
                "https://rest.uniprot.org/uniprotkb/search",
            ),
        }
    }
}
/// Attach actual factories without changing historical F06 catalog declarations
/// or the caller's enabled/active policy. Shared handles belong to the caller.
pub fn register_biomedical(
    registry: &mut Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: BiomedicalSettings,
) -> Result<(), RegistryError> {
    macro_rules! attach {
        ($module:ident, $adapter:ident) => {{
            let http = http.clone();
            let admission = admission.clone();
            let settings = config.$module;
            registry.register(
                stringify!($module),
                Arc::new(move || {
                    Box::new($module::$adapter::new(
                        http.clone(),
                        admission.clone(),
                        settings.clone(),
                    )) as Box<dyn ssx_registry::Engine>
                }) as EngineFactory,
            )?;
        }};
    }
    attach!(clinicaltrials, ClinicalTrialsAdapter);
    attach!(europepmc, EuropePMCAdapter);
    attach!(pubmed, PubMedAdapter);
    attach!(pubchem, PubChemAdapter);
    attach!(uniprot, UniProtAdapter);
    Ok(())
}

// A08 regulatory, financial and legal sources; independent factory attachment.
pub mod edgar;
mod fda_case;
pub mod fred;
pub mod openfda;
pub mod oyez;
mod regulatory;
/// Resolved A08 settings. The composition root snapshots credentials; adapters
/// never reread process environment or change shared selection policy.
#[derive(Clone, Debug)]
pub struct RegulatorySettings {
    pub edgar: edgar::EdgarConfig,
    pub fred: fred::FredConfig,
    pub openfda: openfda::OpenFdaConfig,
    pub oyez: oyez::OyezConfig,
}
impl RegulatorySettings {
    pub fn from_resolved(config: &ssx_config::Config) -> Self {
        fn resolve(
            config: &ssx_config::Config,
            name: &str,
            endpoint: &str,
        ) -> clinicaltrials::BiomedicalConfig {
            let mut settings = config
                .engines
                .get(name)
                .map(clinicaltrials::BiomedicalConfig::from_resolved)
                .unwrap_or_else(|| clinicaltrials::BiomedicalConfig::defaults(endpoint));
            if settings.base_url.chars().all(ssx_json::python_whitespace) {
                settings.base_url = endpoint.into();
            }
            settings
        }
        Self {
            edgar: resolve(config, "edgar", edgar::ENDPOINT),
            fred: fred::FredConfig {
                settings: resolve(config, "fred", fred::ENDPOINT),
                api_key: config
                    .engines
                    .get("fred")
                    .map(|c| c.api_key.clone())
                    .unwrap_or_default(),
            },
            openfda: resolve(config, "openfda", openfda::ENDPOINT),
            oyez: oyez::OyezConfig {
                settings: resolve(config, "oyez", ""),
                search_url: None,
            },
        }
    }
}
pub fn register_regulatory(
    registry: &mut Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: RegulatorySettings,
) -> Result<(), RegistryError> {
    macro_rules! attach {
        ($module:ident, $adapter:ident) => {{
            let http = http.clone();
            let admission = admission.clone();
            let settings = config.$module;
            registry.register(
                stringify!($module),
                Arc::new(move || {
                    Box::new($module::$adapter::new(
                        http.clone(),
                        admission.clone(),
                        settings.clone(),
                    )) as Box<dyn ssx_registry::Engine>
                }) as EngineFactory,
            )?;
        }};
    }
    attach!(edgar, EdgarAdapter);
    attach!(fred, FredAdapter);
    attach!(openfda, OpenFdaAdapter);
    attach!(oyez, OyezAdapter);
    Ok(())
}

// A09 engine-local implementations; availability remains separate from enabled policy.
mod catalog_media;
pub mod internetarchive;
pub mod musicbrainz;
pub mod nominatim;
pub mod openlibrary;
pub mod tmdb;
#[derive(Clone, Debug)]
pub struct CatalogMediaSettings {
    pub openlibrary: openlibrary::OpenLibraryConfig,
    pub musicbrainz: musicbrainz::MusicBrainzConfig,
    pub tmdb: tmdb::TmdbConfig,
    pub nominatim: nominatim::NominatimConfig,
    pub internetarchive: internetarchive::InternetArchiveConfig,
}
impl CatalogMediaSettings {
    pub fn from_resolved(config: &ssx_config::Config) -> Self {
        fn resolve(config: &ssx_config::Config, name: &str, base: &str) -> catalog_media::Settings {
            let mut c = config
                .engines
                .get(name)
                .map(catalog_media::Settings::from_resolved)
                .unwrap_or_else(|| catalog_media::Settings::defaults(base));
            if c.base_url.chars().all(ssx_json::python_whitespace) {
                c.base_url = base.into();
            }
            c
        }
        Self {
            openlibrary: resolve(config, "openlibrary", openlibrary::ENDPOINT),
            musicbrainz: resolve(config, "musicbrainz", musicbrainz::ENDPOINT),
            tmdb: tmdb::TmdbConfig {
                settings: resolve(config, "tmdb", tmdb::ENDPOINT),
                api_key: config
                    .engines
                    .get("tmdb")
                    .map(|c| c.api_key.clone())
                    .unwrap_or_default(),
            },
            nominatim: resolve(config, "nominatim", nominatim::ENDPOINT),
            internetarchive: internetarchive::InternetArchiveConfig {
                settings: resolve(config, "internetarchive", internetarchive::ENDPOINT),
                cdx_url: None,
                availability_url: None,
            },
        }
    }
}
pub fn register_catalog_media(
    registry: &mut Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: CatalogMediaSettings,
) -> Result<(), RegistryError> {
    macro_rules! attach {
        ($module:ident,$adapter:ident) => {{
            let h = http.clone();
            let a = admission.clone();
            let c = config.$module;
            registry.register(
                stringify!($module),
                Arc::new(move || {
                    Box::new($module::$adapter::new(h.clone(), a.clone(), c.clone()))
                        as Box<dyn ssx_registry::Engine>
                }) as EngineFactory,
            )?;
        }};
    }
    attach!(openlibrary, OpenLibraryAdapter);
    attach!(musicbrainz, MusicBrainzAdapter);
    attach!(tmdb, TmdbAdapter);
    attach!(nominatim, NominatimAdapter);
    attach!(internetarchive, InternetArchiveAdapter);
    Ok(())
}

mod tmdb_case;

// A10 named vulnerability and exploit-knowledge adapters; metadata stays F06-owned.
pub mod cve;
pub mod epss;
pub mod exploitdb;
pub mod mitreattack;
pub mod nvd;
mod vulnerability;
pub use vulnerability::{
    VulnerabilitySettings, register_vulnerability, register_vulnerability_with_parser,
};

pub(crate) mod vulnerability_html;
pub(crate) mod vulnerability_wire;

// A11 engine-local IP/URL intelligence; catalog and admission remain shared.
pub mod abuseipdb;
pub mod greynoise;
pub mod otx;
mod threat_intel;
mod threat_ioc;
pub mod urlhaus;
pub use threat_intel::{ThreatIntelConfig, ThreatIntelSettings, register_threat_intelligence};

// A12 engine-local infrastructure security; shared policy/transport remain caller-owned.
pub mod censys;
pub mod crtsh;
mod infrastructure_security;
pub mod shodan;
pub mod virustotal;
pub mod vulncheck;
pub use infrastructure_security::{
    Kind as InfrastructureSecurityKind, SecurityConfig, SecuritySettings,
    register_infrastructure_security,
};
// A13 first slice: a real sensitive factory, mounted explicitly by its owner.
pub mod dehashed;

// A13 remaining threat intelligence; construction is explicit and policy-owned.
pub mod hibp;
pub mod intelx;
