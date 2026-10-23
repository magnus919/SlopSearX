//! F06-resolved scholarly settings and actual factory attachment.
//! Configuration remains caller-owned startup storage; selection policy is separate.
use super::{
    arxiv_expat::AtomPool,
    arxiv_runtime::{self, ArxivConfig},
    openalex_runtime::{self, OpenAlexConfig},
    semantic_runtime::{self, SemanticConfig},
};
use crate::http::AdapterHttp;
use ssx_application::SearchAdmission;
use ssx_registry::{Registry, RegistryError};
use std::sync::Arc;

#[derive(Clone)]
pub struct ScholarlySettings {
    pub arxiv: ArxivConfig,
    pub openalex: OpenAlexConfig,
    pub semanticscholar: SemanticConfig,
}
fn scalar(value: &ssx_config::ConfigScalar) -> super::scholarly_config::Scalar {
    use super::scholarly_config::Scalar;
    match value {
        ssx_config::ConfigScalar::Boolean(v) => Scalar::Boolean(*v),
        ssx_config::ConfigScalar::Integer(v) => Scalar::Integer(*v),
        ssx_config::ConfigScalar::Float(v) => Scalar::Float(*v),
        ssx_config::ConfigScalar::Text(v) => Scalar::Text(Arc::from(v.as_str())),
    }
}
fn base(value: &str, default: &Arc<str>) -> Arc<str> {
    if value.chars().all(ssx_json::python_whitespace) {
        default.clone()
    } else {
        Arc::from(value)
    }
}
impl ScholarlySettings {
    /// Preserve F06's YAML/environment precedence and raw scalar variants. The
    /// source service omits only blank base URLs before adapter construction.
    /// No enabled/active setting, transport policy or catalog metadata changes.
    pub fn from_resolved(config: &ssx_config::Config) -> Self {
        let mut arxiv = ArxivConfig::default();
        let mut openalex = OpenAlexConfig::default();
        let mut semanticscholar = SemanticConfig::default();
        if let Some(entry) = config.engines.get("arxiv") {
            arxiv.base_url = base(&entry.base_url, &arxiv.base_url);
            arxiv.timeout_ms = scalar(&entry.timeout_ms);
            arxiv.max_results = scalar(&entry.max_results);
        }
        if let Some(entry) = config.engines.get("openalex") {
            openalex.base_url = base(&entry.base_url, &openalex.base_url);
            openalex.timeout_ms = scalar(&entry.timeout_ms);
            openalex.max_results = scalar(&entry.max_results);
        }
        if let Some(entry) = config.engines.get("semanticscholar") {
            semanticscholar.base_url = base(&entry.base_url, &semanticscholar.base_url);
            semanticscholar.timeout_ms = scalar(&entry.timeout_ms);
            semanticscholar.max_results = scalar(&entry.max_results);
            semanticscholar.api_key = entry
                .api_key
                .expose()
                .filter(|v| !v.is_empty())
                .map(Arc::from);
        }
        Self {
            arxiv,
            openalex,
            semanticscholar,
        }
    }
}
/// Attach callable factories while the composition root retains shared HTTP,
/// admission and parser-lifecycle handles. Availability does not enable selection.
pub fn register(
    registry: &mut Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    settings: ScholarlySettings,
    parser: AtomPool,
) -> Result<(), RegistryError> {
    arxiv_runtime::register(
        registry,
        http.clone(),
        admission.clone(),
        settings.arxiv,
        parser.clone(),
    )?;
    openalex_runtime::register(
        registry,
        http.clone(),
        admission.clone(),
        settings.openalex,
        parser.clone(),
    )?;
    semantic_runtime::register(registry, http, admission, settings.semanticscholar, parser)
}
