//! Source-derived engine descriptors and registration without orchestrator edits.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt, sync::Arc};

const CATALOG: &str = include_str!("../../../docs/rust-rewrite/f06/catalog.json");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineDescriptor {
    pub name: String,
    pub display_name: String,
    pub engine_type: String,
    pub categories: Vec<String>,
    pub enabled_by_default: bool,
    pub auth_class: AuthClass,
    pub api_key_env: String,
    pub api_secret_env: Option<String>,
    pub sensitive: bool,
    pub cost_class: Option<String>,
    pub supported_filters: BTreeMap<String, bool>,
    pub enforced_filters: BTreeMap<String, String>,
    pub supported_result_types: Vec<String>,
    pub supported_media_types: Vec<String>,
    pub failure_classes: Vec<String>,
    pub caveats: Vec<String>,
    pub routing: RoutingCard,
    pub defaults: EngineDefaults,
    pub implementation: ImplementationStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthClass {
    None,
    Optional,
    Required,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutingCard {
    pub role: RoutingRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_when: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingRole {
    General,
    BroadSupport,
    Specialist,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineDefaults {
    pub enabled: bool,
    pub base_url: String,
    #[serde(rename = "type")]
    pub engine_type: String,
    pub timeout_ms: i64,
    pub max_results: i64,
    pub rate_limit: Option<serde_json::Number>,
    pub weight: serde_json::Number,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImplementationStatus {
    Unavailable,
    Available,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    Duplicate(String),
    Unknown(String),
    InvalidCatalog(String),
}
impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate(name) => write!(f, "duplicate engine registration: {name}"),
            Self::Unknown(name) => write!(f, "engine descriptor missing: {name}"),
            Self::InvalidCatalog(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for RegistryError {}

/// A factory may capture an immutable, cloneable application context (usually
/// behind `Arc`) that owns shared runtime services and resolved engine-local
/// configuration. Each factory then builds its engine without requiring an
/// orchestrator edit. The executable search port is application-owned; the
/// registry depends on that consumer contract and never on config or infra.
pub use ssx_application::SearchEngine as Engine;
pub type EngineFactory = Arc<dyn Fn() -> Box<dyn Engine> + Send + Sync>;

#[derive(Clone)]
pub struct Registry {
    descriptors: Vec<EngineDescriptor>,
    factories: BTreeMap<String, EngineFactory>,
}
impl Registry {
    /// Frozen Python migration inventory, kept separate from additive native providers.
    pub fn migration_baseline() -> Result<Self, RegistryError> {
        let descriptors: Vec<EngineDescriptor> = serde_json::from_str(CATALOG)
            .map_err(|error| RegistryError::InvalidCatalog(error.to_string()))?;
        Self::from_descriptors(descriptors)
    }

    /// Native-only providers extend the frozen migration catalog without changing its provenance.
    pub fn builtins() -> Result<Self, RegistryError> {
        let mut descriptors = Self::migration_baseline()?.descriptors;
        let native: Vec<EngineDescriptor> = serde_json::from_str(include_str!(
            "../../../docs/rust-rewrite/native-engines/catalog.json"
        ))
        .map_err(|error| RegistryError::InvalidCatalog(error.to_string()))?;
        descriptors.extend(native);
        Self::from_descriptors(descriptors)
    }

    pub fn from_descriptors(
        descriptors: impl IntoIterator<Item = EngineDescriptor>,
    ) -> Result<Self, RegistryError> {
        let mut by_name = BTreeMap::new();
        let mut ordered = Vec::new();
        for descriptor in descriptors {
            if descriptor.name.trim().is_empty() {
                return Err(RegistryError::InvalidCatalog("empty engine name".into()));
            }
            let name = descriptor.name.clone();
            if by_name.insert(name.clone(), ()).is_some() {
                return Err(RegistryError::Duplicate(name));
            }
            ordered.push(descriptor);
        }
        for descriptor in &ordered {
            validate_descriptor(descriptor)?;
        }
        Ok(Self {
            descriptors: ordered,
            factories: BTreeMap::new(),
        })
    }

    /// Attach a factory to an existing engine descriptor. The captured context
    /// belongs to the application/engine wiring layer; factory construction is
    /// engine-local and does not add orchestration or registry dependencies.
    pub fn register(&mut self, name: &str, factory: EngineFactory) -> Result<(), RegistryError> {
        if self.descriptor(name).is_none() {
            return Err(RegistryError::Unknown(name.into()));
        }
        if self.factories.contains_key(name) {
            return Err(RegistryError::Duplicate(name.into()));
        }
        self.factories.insert(name.into(), factory);
        Ok(())
    }

    pub fn descriptors(&self) -> impl Iterator<Item = &EngineDescriptor> {
        self.descriptors.iter()
    }
    pub fn descriptor(&self, name: &str) -> Option<&EngineDescriptor> {
        self.descriptors
            .iter()
            .find(|descriptor| descriptor.name == name)
    }
    pub fn instantiate(&self, name: &str) -> Option<Box<dyn Engine>> {
        self.factories.get(name).map(|factory| factory())
    }
    pub fn is_available(&self, name: &str) -> bool {
        self.factories.contains_key(name)
    }
    pub fn len(&self) -> usize {
        self.descriptors.len()
    }
    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }
}

pub fn validate_descriptor(descriptor: &EngineDescriptor) -> Result<(), RegistryError> {
    if descriptor.name.trim().is_empty() || descriptor.display_name.trim().is_empty() {
        return Err(RegistryError::InvalidCatalog(
            "descriptor identity is empty".into(),
        ));
    }
    if descriptor.routing.role == RoutingRole::Specialist
        && (descriptor
            .routing
            .purpose
            .as_deref()
            .is_none_or(str::is_empty)
            || descriptor
                .routing
                .use_when
                .as_deref()
                .is_none_or(str::is_empty))
    {
        return Err(RegistryError::InvalidCatalog(format!(
            "specialist routing card incomplete: {}",
            descriptor.name
        )));
    }
    for number in
        std::iter::once(&descriptor.defaults.weight).chain(descriptor.defaults.rate_limit.iter())
    {
        if !number.is_f64() && number.as_i64().is_none() {
            return Err(RegistryError::InvalidCatalog(
                "integer default outside signed 64-bit range".into(),
            ));
        }
    }
    if descriptor.implementation == ImplementationStatus::Available && descriptor.name.is_empty() {
        return Err(RegistryError::InvalidCatalog(
            "available adapter has no name".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ssx_domain::{AdapterResponse, SearchRequest};
    struct FixtureEngine;
    impl Engine for FixtureEngine {
        fn search<'a>(
            &'a self,
            _: &'a SearchRequest,
        ) -> ssx_application::PortFuture<'a, AdapterResponse> {
            Box::pin(async { AdapterResponse::default() })
        }
    }

    #[test]
    fn builtins_are_complete_and_unported_engines_are_unavailable() {
        let registry = Registry::migration_baseline().unwrap();
        assert_eq!(registry.len(), 51);
        assert_eq!(
            registry
                .descriptors()
                .filter(|e| e.implementation == ImplementationStatus::Unavailable)
                .count(),
            51
        );
        assert_eq!(
            registry
                .descriptors()
                .filter(|e| e.routing.role == RoutingRole::Specialist)
                .count(),
            44
        );
        assert!(!registry.descriptor("europepmc").unwrap().defaults.enabled);
        assert!(
            registry
                .descriptors()
                .all(|engine| !registry.is_available(&engine.name))
        );
    }

    #[test]
    fn native_additions_preserve_baseline_and_reject_duplicates() {
        let baseline = Registry::migration_baseline().unwrap();
        let runtime = Registry::builtins().unwrap();
        assert_eq!(runtime.len(), baseline.len() + 1);
        for original in baseline.descriptors() {
            assert_eq!(runtime.descriptor(&original.name), Some(original));
        }
        let native = runtime.descriptor("deepseek").unwrap();
        assert!(!native.enabled_by_default);
        assert!(!native.defaults.enabled);
        assert_eq!(native.defaults.max_results, 10);
        assert_eq!(native.api_key_env, "ENGINE_DEEPSEEK_API_KEY");
        assert_eq!(native.auth_class, AuthClass::Required);
        assert_eq!(native.implementation, ImplementationStatus::Available);
        assert_eq!(native.cost_class.as_deref(), Some("paid"));
        assert_eq!(native.categories, ["general"]);
        assert_eq!(native.routing.role, RoutingRole::General);
        assert!(native.enforced_filters.is_empty());
        assert!(
            native
                .supported_filters
                .values()
                .all(|supported| !supported)
        );
        let mut duplicate: Vec<_> = runtime.descriptors().cloned().collect();
        duplicate.push(native.clone());
        assert!(matches!(
            Registry::from_descriptors(duplicate),
            Err(RegistryError::Duplicate(_))
        ));
        let mut invalid = native.clone();
        invalid.routing.role = RoutingRole::Specialist;
        assert!(Registry::from_descriptors([invalid]).is_err());
    }

    #[test]
    fn fixture_adapter_registers_through_the_engine_owned_seam() {
        let builtins = Registry::builtins().unwrap();
        let mut descriptors: Vec<_> = builtins.descriptors().cloned().collect();
        let mut fixture_descriptor = descriptors[0].clone();
        fixture_descriptor.name = "fixture".into();
        fixture_descriptor.display_name = "Fixture adapter".into();
        fixture_descriptor.api_key_env = "ENGINE_FIXTURE_API_KEY".into();
        descriptors.push(fixture_descriptor);
        let mut registry = Registry::from_descriptors(descriptors).unwrap();
        registry
            .register("fixture", Arc::new(|| Box::new(FixtureEngine)))
            .unwrap();
        assert_eq!(registry.len(), builtins.len() + 1);
        assert!(registry.is_available("fixture"));
        assert!(registry.instantiate("fixture").is_some());
        assert!(matches!(
            registry.register("fixture", Arc::new(|| Box::new(FixtureEngine))),
            Err(RegistryError::Duplicate(_))
        ));
        assert!(matches!(
            registry.register("not-in-catalog", Arc::new(|| Box::new(FixtureEngine))),
            Err(RegistryError::Unknown(_))
        ));
    }

    #[test]
    fn filter_consumption_and_enforcement_are_distinct_layered_metadata() {
        let registry = Registry::builtins().unwrap();
        let openalex = registry.descriptor("openalex").unwrap();
        assert_eq!(openalex.supported_filters.get("date_from"), Some(&true));
        assert_eq!(
            openalex
                .enforced_filters
                .get("date_from")
                .map(String::as_str),
            Some("upstream")
        );
        assert_eq!(
            openalex
                .enforced_filters
                .get("time_range")
                .map(String::as_str),
            Some("local")
        );
        let brave = registry.descriptor("brave").unwrap();
        assert_eq!(brave.supported_filters.get("safesearch"), Some(&false));
        assert!(!brave.enforced_filters.contains_key("safesearch"));
    }

    #[test]
    fn duplicate_descriptor_and_invalid_specialist_card_fail_closed() {
        let descriptor = Registry::builtins()
            .unwrap()
            .descriptor("brave")
            .unwrap()
            .clone();
        assert!(matches!(
            Registry::from_descriptors([descriptor.clone(), descriptor]),
            Err(RegistryError::Duplicate(_))
        ));
        let mut invalid = Registry::builtins()
            .unwrap()
            .descriptor("arxiv")
            .unwrap()
            .clone();
        invalid.routing.purpose = None;
        assert!(validate_descriptor(&invalid).is_err());
    }
}
