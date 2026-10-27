//! Typed layered configuration with redacted secrets and source-compatible overlays.
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use ssx_domain::storage::normalize_python_number_text;
use ssx_registry::{AuthClass, EngineDescriptor, Registry};
use std::{collections::BTreeMap, fmt, fs, path::Path};

mod yaml;

pub const CONFIG_FILE_PATH: &str = "/etc/slopsearx/config.yaml";

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Secret(Option<String>);
impl Secret {
    pub fn new(value: Option<String>) -> Self {
        Self(value)
    }
    pub fn is_configured(&self) -> bool {
        self.0.as_deref().is_some_and(|value| !value.is_empty())
    }
    pub fn expose(&self) -> Option<&str> {
        self.0.as_deref()
    }
}
impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.is_configured() {
            "<redacted>"
        } else {
            "None"
        })
    }
}
impl Serialize for Secret {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Err(serde::ser::Error::custom(
            "secret values cannot be serialized",
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CacheConfig {
    pub ttl_seconds: i64,
    pub max_result_sets: i64,
    pub revalidate_on_hit: bool,
    #[serde(flatten)]
    pub extension_fields: BTreeMap<String, Value>,
}
impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            ttl_seconds: 300,
            max_result_sets: 10_000,
            revalidate_on_hit: false,
            extension_fields: BTreeMap::new(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RankingConfig {
    pub strategy: String,
    #[serde(flatten)]
    pub extension_fields: BTreeMap<String, Value>,
}
impl Default for RankingConfig {
    fn default() -> Self {
        Self {
            strategy: "presence".into(),
            extension_fields: BTreeMap::new(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoutingConfig {
    pub enabled: bool,
    pub topics: Option<Value>,
    /// Original named-topic insertion order; YAML aliases/merges are resolved first.
    /// Not serialized as configuration or accepted from the YAML overlay.
    #[serde(skip)]
    pub named_topic_order: Vec<String>,
    pub fallback: Option<Value>,
    pub budget: Option<Value>,
}
impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            topics: None,
            named_topic_order: Vec::new(),
            fallback: None,
            budget: None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchConfig {
    pub formats: Vec<String>,
}
impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            formats: vec!["html".into(), "csv".into(), "json".into(), "rss".into()],
        }
    }
}

/// Raw configured scalar, retaining Python's runtime type for environment coercion.
/// Parse effective numeric values explicitly; raw strings are not validated numbers.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ConfigScalar {
    Boolean(bool),
    Integer(i64),
    Float(f64),
    Text(String),
}
impl<'de> Deserialize<'de> for ConfigScalar {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            Value::Bool(value) => Ok(Self::Boolean(value)),
            Value::String(value) => Ok(Self::Text(value)),
            Value::Number(value) if value.is_f64() => value
                .as_f64()
                .map(Self::Float)
                .ok_or_else(|| serde::de::Error::custom("invalid float setting")),
            Value::Number(value) => value.as_i64().map(Self::Integer).ok_or_else(|| {
                serde::de::Error::custom("integer setting outside signed 64-bit range")
            }),
            _ => Err(serde::de::Error::custom("setting must be a scalar")),
        }
    }
}
impl From<i64> for ConfigScalar {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}
impl From<f64> for ConfigScalar {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}
impl From<serde_json::Number> for ConfigScalar {
    fn from(value: serde_json::Number) -> Self {
        value
            .as_i64()
            .map(Self::Integer)
            .unwrap_or_else(|| Self::Float(value.as_f64().expect("JSON numeric default")))
    }
}
impl ConfigScalar {
    /// Validated finite numeric value; bounds such as positivity remain caller-owned.
    pub fn effective_f64(&self) -> Result<f64, ConfigError> {
        let value = match self {
            Self::Boolean(v) => {
                if *v {
                    1.0
                } else {
                    0.0
                }
            }
            Self::Integer(v) => *v as f64,
            Self::Float(v) => *v,
            Self::Text(v) => normalize_python_number_text(v)
                .map_err(|_| ConfigError("invalid numeric setting".into()))?
                .parse()
                .map_err(|_| ConfigError("invalid numeric setting".into()))?,
        };
        if !value.is_finite() {
            return Err(ConfigError(
                "effective numeric setting must be finite".into(),
            ));
        }
        Ok(value)
    }
    /// Exact signed integer value; fractional floats and out-of-range values fail.
    pub fn effective_i64(&self) -> Result<i64, ConfigError> {
        match self {
            Self::Integer(v) => Ok(*v),
            Self::Boolean(v) => Ok(i64::from(*v)),
            Self::Text(v) => normalize_python_number_text(v)
                .map_err(|_| ConfigError("invalid integer setting".into()))?
                .parse()
                .map_err(|_| ConfigError("invalid integer setting".into())),
            Self::Float(v)
                if v.is_finite()
                    && v.fract() == 0.0
                    && (-9223372036854775808.0..9223372036854775808.0).contains(v) =>
            {
                Ok(*v as i64)
            }
            _ => Err(ConfigError(
                "effective setting must be an exact signed integer".into(),
            )),
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EngineConfig {
    pub enabled: bool,
    pub base_url: String,
    #[serde(rename = "type")]
    pub engine_type: String,
    pub timeout_ms: ConfigScalar,
    pub max_results: ConfigScalar,
    pub rate_limit: Option<ConfigScalar>,
    pub weight: ConfigScalar,
    #[serde(skip_serializing, skip_deserializing)]
    pub api_key: Secret,
    #[serde(skip_serializing, skip_deserializing)]
    pub api_secret: Secret,
    pub user_agent: Option<String>,
    pub categories: Option<Vec<String>>,
    pub proxy_pool: Option<String>,
    pub scrape_proxy_url: Option<String>,
    #[serde(skip)]
    pub extra_env_settings: BTreeMap<String, String>,
}
impl fmt::Debug for EngineConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EngineConfig")
            .field("enabled", &self.enabled)
            .field("base_url", &self.base_url)
            .field("type", &self.engine_type)
            .field("timeout_ms", &self.timeout_ms)
            .field("max_results", &self.max_results)
            .field("rate_limit", &self.rate_limit)
            .field("weight", &self.weight)
            .field("api_key", &self.api_key)
            .field("api_secret", &self.api_secret)
            .field("user_agent", &self.user_agent)
            .field("categories", &self.categories)
            .field("proxy_pool", &self.proxy_pool)
            .field("scrape_proxy_url", &self.scrape_proxy_url)
            .finish()
    }
}
impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            base_url: String::new(),
            engine_type: "api".into(),
            timeout_ms: 5000.into(),
            max_results: 10.into(),
            rate_limit: None,
            weight: 1.0.into(),
            api_key: Secret::default(),
            api_secret: Secret::default(),
            user_agent: None,
            categories: None,
            proxy_pool: None,
            scrape_proxy_url: None,
            extra_env_settings: BTreeMap::new(),
        }
    }
}
impl EngineConfig {
    fn from_descriptor(descriptor: &EngineDescriptor) -> Self {
        let defaults = &descriptor.defaults;
        Self {
            enabled: defaults.enabled,
            base_url: defaults.base_url.clone(),
            engine_type: defaults.engine_type.clone(),
            timeout_ms: defaults.timeout_ms.into(),
            max_results: defaults.max_results.into(),
            rate_limit: defaults.rate_limit.clone().map(ConfigScalar::from),
            weight: defaults.weight.clone().into(),
            ..Self::default()
        }
    }
    pub fn effective_categories<'a>(&'a self, descriptor: &'a EngineDescriptor) -> &'a [String] {
        self.categories.as_deref().unwrap_or(&descriptor.categories)
    }
    pub fn auth_class(&self, descriptor: &EngineDescriptor) -> AuthClass {
        if descriptor.auth_class == AuthClass::Required {
            AuthClass::Required
        } else if self.api_key.is_configured() {
            AuthClass::Optional
        } else {
            AuthClass::None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub engines: BTreeMap<String, EngineConfig>,
    pub cache: CacheConfig,
    pub ranking: RankingConfig,
    pub routing: RoutingConfig,
    pub search: SearchConfig,
    pub default_engines: Vec<String>,
    pub log_level: String,
    pub enable_suggestions: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            engines: BTreeMap::new(),
            cache: CacheConfig::default(),
            ranking: RankingConfig::default(),
            routing: RoutingConfig::default(),
            search: SearchConfig::default(),
            default_engines: vec!["brave".into(), "wikipedia".into()],
            log_level: "INFO".into(),
            enable_suggestions: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError(pub String);
impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ConfigError {}

pub fn load_config(path: Option<&Path>) -> Result<Config, ConfigError> {
    let env: Vec<(String, String)> = std::env::vars().collect();
    load_config_entries(path, &env)
}

/// Injectable environment boundary for deterministic tests and embedded callers.
pub fn load_config_with(
    path: Option<&Path>,
    env: &BTreeMap<String, String>,
) -> Result<Config, ConfigError> {
    let entries: Vec<_> = env
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    load_config_entries(path, &entries)
}
fn load_config_entries(
    path: Option<&Path>,
    env: &[(String, String)],
) -> Result<Config, ConfigError> {
    let registry = Registry::builtins().map_err(|error| ConfigError(error.to_string()))?;
    let mut config = Config::default();
    for descriptor in registry.descriptors() {
        // Only the explicit 28-entry Python defaults table creates an
        // EngineEntry. Other built-ins use adapter-local defaults until a
        // custom YAML entry is supplied.
        if !descriptor.defaults.base_url.is_empty() {
            config.engines.insert(
                descriptor.name.clone(),
                EngineConfig::from_descriptor(descriptor),
            );
        }
    }
    let path = path.unwrap_or_else(|| Path::new(CONFIG_FILE_PATH));
    if path.exists() {
        let raw = fs::read_to_string(path)
            .map_err(|error| ConfigError(format!("config file: {error}")))?;
        let (parsed, named_topic_order) = yaml::parse_with_topic_order(&raw)?;
        if !parsed.is_null()
            && parsed != json!(false)
            && parsed != json!(0)
            && parsed != json!("")
            && parsed.as_array().is_none_or(|items| !items.is_empty())
        {
            let root = parsed
                .as_object()
                .ok_or_else(|| ConfigError("config root must be a mapping".into()))?;
            apply_file(&mut config, root)?;
            config.routing.named_topic_order = named_topic_order;
        }
    }
    apply_environment(&mut config, env)?;
    Ok(config)
}

fn apply_file(config: &mut Config, root: &Map<String, Value>) -> Result<(), ConfigError> {
    if let Some(engines) = root.get("engines") {
        let engines = engines
            .as_object()
            .ok_or_else(|| ConfigError("engines must be a mapping".into()))?;
        for (name, raw) in engines {
            let object = raw
                .as_object()
                .ok_or_else(|| ConfigError(format!("engine {name} must be a mapping")))?;
            let mut entry = config.engines.get(name).cloned().unwrap_or_default();
            apply_engine_file(&mut entry, object)
                .map_err(|error| ConfigError(format!("engine {name}: {error}")))?;
            config.engines.insert(name.clone(), entry);
        }
    }
    if let Some(value) = root.get("cache") {
        overlay_typed(&mut config.cache, value, "cache")?;
    }
    if let Some(value) = root.get("ranking") {
        overlay_typed(&mut config.ranking, value, "ranking")?;
    }
    if let Some(value) = root.get("routing") {
        apply_routing(&mut config.routing, value)?;
    }
    if let Some(value) = root.get("search") {
        let object = value
            .as_object()
            .ok_or_else(|| ConfigError("search must be a mapping".into()))?;
        if let Some(formats) = object.get("formats") {
            let formats = if let Some(items) = formats.as_array() {
                items
                    .iter()
                    .map(|item| {
                        item.as_str().map(str::to_owned).ok_or_else(|| {
                            ConfigError("search.formats items must be strings".into())
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?
            } else if let Some(text) = formats.as_str() {
                text.chars()
                    .map(|character| character.to_string())
                    .collect()
            } else {
                return Err(ConfigError("search.formats must be a sequence".into()));
            };
            config.search.formats = formats;
        }
    }
    if let Some(value) = root.get("default_engines") {
        config.default_engines = string_list(value, "default_engines")?;
    }
    if let Some(value) = root.get("log_level") {
        config.log_level = value
            .as_str()
            .ok_or_else(|| ConfigError("log_level must be a string".into()))?
            .to_owned();
    }
    if let Some(value) = root.get("enable_suggestions") {
        config.enable_suggestions = value
            .as_bool()
            .ok_or_else(|| ConfigError("enable_suggestions must be a boolean".into()))?;
    }
    Ok(())
}

fn apply_engine_file(
    entry: &mut EngineConfig,
    object: &Map<String, Value>,
) -> Result<(), ConfigError> {
    let value = serde_json::to_value(&*entry).map_err(|error| ConfigError(error.to_string()))?;
    let mut merged = value
        .as_object()
        .cloned()
        .ok_or_else(|| ConfigError("engine settings must be a mapping".into()))?;
    let mut api_key = entry.api_key.clone();
    let mut api_secret = entry.api_secret.clone();
    for (key, value) in object {
        match key.as_str() {
            "api_key" => api_key = file_secret(value)?,
            "api_secret" => api_secret = file_secret(value)?,
            _ => {
                merged.insert(key.clone(), value.clone());
            }
        }
    }
    let mut next: EngineConfig = serde_json::from_value(Value::Object(merged))
        .map_err(|_| ConfigError("invalid engine settings".into()))?;
    next.api_key = api_key;
    next.api_secret = api_secret;
    *entry = next;
    Ok(())
}

fn file_secret(value: &Value) -> Result<Secret, ConfigError> {
    match value {
        Value::Null => Ok(Secret::default()),
        Value::String(value) => Ok(Secret::new(Some(value.clone()))),
        _ => Err(ConfigError("credential must be a string or null".into())),
    }
}

fn overlay_typed<T: for<'de> Deserialize<'de> + Serialize + Default>(
    target: &mut T,
    value: &Value,
    label: &str,
) -> Result<(), ConfigError> {
    let object = value
        .as_object()
        .ok_or_else(|| ConfigError(format!("{label} must be a mapping")))?;
    let mut merged = serde_json::to_value(&*target)
        .map_err(|error| ConfigError(error.to_string()))?
        .as_object()
        .cloned()
        .ok_or_else(|| ConfigError(format!("{label} defaults must be a mapping")))?;
    for (key, value) in object {
        merged.insert(key.clone(), value.clone());
    }
    *target = serde_json::from_value(Value::Object(merged))
        .map_err(|_| ConfigError(format!("invalid {label} settings")))?;
    Ok(())
}

fn apply_routing(target: &mut RoutingConfig, value: &Value) -> Result<(), ConfigError> {
    let object = value
        .as_object()
        .ok_or_else(|| ConfigError("routing must be a mapping".into()))?;
    let mut known = Map::new();
    for key in ["enabled", "topics", "fallback", "budget"] {
        if let Some(value) = object.get(key) {
            known.insert(key.to_owned(), value.clone());
        }
    }
    overlay_typed(target, &Value::Object(known), "routing")
}

fn apply_environment(config: &mut Config, entries: &[(String, String)]) -> Result<(), ConfigError> {
    let original_env: BTreeMap<_, _> = entries
        .iter()
        .map(|(key, value)| (key.as_str(), value))
        .collect();
    // Resolve aliases before coercing values, just like _load_env_overrides.
    // Runtime reads preserve OS order; deterministic map injection uses map order.
    let mut env = BTreeMap::new();
    for (key, value) in entries {
        if let Some(suffix) = key.strip_prefix("SEARCH_") {
            env.insert(format!("SEARCH_{}", suffix.to_lowercase()), value);
        } else if let Some(suffix) = key.strip_prefix("ENGINE_") {
            env.insert(format!("ENGINE_{}", suffix.to_lowercase()), value);
        }
    }
    if let Some(value) = env.get("SEARCH_cache_ttl_seconds") {
        let ttl = normalize_python_number_text(value)
            .and_then(|text| {
                text.parse::<i64>()
                    .map_err(|_| ssx_domain::storage::StorageError::InvalidNumber("ttl"))
            })
            .map_err(|_| {
                ConfigError("SEARCH_CACHE_TTL_SECONDS must be a positive integer in seconds".into())
            })?;
        if ttl <= 0 {
            return Err(ConfigError(
                "SEARCH_CACHE_TTL_SECONDS must be a positive integer in seconds".into(),
            ));
        }
        config.cache.ttl_seconds = ttl;
    }
    if let Some(value) = env.get("SEARCH_log_level") {
        config.log_level = value.to_uppercase();
    }
    if let Some(value) = env.get("SEARCH_default_engines") {
        config.default_engines = value
            .split(',')
            .map(|part| python_strip(part).to_owned())
            .collect();
    }
    if let Some(value) = env.get("SEARCH_formats") {
        config.search.formats = value
            .split(',')
            .map(|part| python_strip(part).to_lowercase())
            .filter(|part| !part.is_empty())
            .collect();
    }
    for (key, value) in &env {
        if !key.starts_with("ENGINE_") {
            continue;
        }
        let Some((name, setting)) = key
            .strip_prefix("ENGINE_")
            .and_then(|tail| tail.split_once('_'))
        else {
            continue;
        };
        let name = name.to_lowercase();
        let Some(entry) = config.engines.get_mut(&name) else {
            continue;
        };
        let setting = setting.to_lowercase();
        apply_engine_env(entry, &setting, value)?;
    }
    if let Some(value) = original_env.get("HF_TOKEN")
        && !original_env.contains_key("ENGINE_HUGGINGFACE_API_KEY")
        && let Some(entry) = config.engines.get_mut("huggingface")
    {
        entry.api_key = Secret::new(Some(value.to_string()));
    }
    Ok(())
}

fn apply_engine_env(entry: &mut EngineConfig, key: &str, raw: &str) -> Result<(), ConfigError> {
    match key {
        "enabled" => entry.enabled = matches!(raw.to_lowercase().as_str(), "true" | "1" | "yes"),
        "base_url" => entry.base_url = raw.to_owned(),
        "type" => entry.engine_type = raw.to_owned(),
        "timeout_ms" => apply_scalar_env(&mut entry.timeout_ms, raw)?,
        "max_results" => apply_scalar_env(&mut entry.max_results, raw)?,
        "weight" => apply_scalar_env(&mut entry.weight, raw)?,
        "rate_limit" => {
            if let Some(value) = &mut entry.rate_limit {
                apply_scalar_env(value, raw)?;
            } else {
                entry.rate_limit = Some(ConfigScalar::Text(raw.to_owned()));
            }
        }
        "api_key" => entry.api_key = Secret::new(Some(raw.to_owned())),
        "api_secret" => entry.api_secret = Secret::new(Some(raw.to_owned())),
        "user_agent" => entry.user_agent = Some(raw.to_owned()),
        "categories" => {
            entry.categories = Some(if raw.is_empty() {
                vec![]
            } else {
                raw.split(',')
                    .map(python_strip)
                    .filter(|item| !item.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
        }
        "proxy_pool" => entry.proxy_pool = Some(raw.to_owned()),
        "scrape_proxy_url" => entry.scrape_proxy_url = Some(raw.to_owned()),
        other => {
            entry
                .extra_env_settings
                .insert(other.to_owned(), raw.to_owned());
        }
    }
    Ok(())
}

fn apply_scalar_env(target: &mut ConfigScalar, raw: &str) -> Result<(), ConfigError> {
    *target = match target {
        ConfigScalar::Boolean(_) => {
            ConfigScalar::Boolean(matches!(raw.to_lowercase().as_str(), "true" | "1" | "yes"))
        }
        ConfigScalar::Integer(_) => ConfigScalar::Integer(
            normalize_python_number_text(raw)
                .map_err(|_| ConfigError("invalid integer engine setting".into()))?
                .parse()
                .map_err(|_| ConfigError("invalid integer engine setting".into()))?,
        ),
        ConfigScalar::Float(_) => ConfigScalar::Float(
            normalize_python_number_text(raw)
                .map_err(|_| ConfigError("invalid numeric engine setting".into()))?
                .parse()
                .map_err(|_| ConfigError("invalid numeric engine setting".into()))?,
        ),
        ConfigScalar::Text(_) => ConfigScalar::Text(raw.to_owned()),
    };
    Ok(())
}

// Python str.strip includes the ASCII information separators; numeric parsing
// deliberately uses the accepted F05 numeric helper, whose whitespace differs.
fn python_strip(value: &str) -> &str {
    value.trim_matches(|ch: char| ch.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&ch))
}

fn string_list(value: &Value, label: &str) -> Result<Vec<String>, ConfigError> {
    value
        .as_array()
        .ok_or_else(|| ConfigError(format!("{label} must be a sequence")))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| ConfigError(format!("{label} items must be strings")))
        })
        .collect()
}

/// Secret-free capability view. `auth_configured` follows the source catalog's
/// API-key-only rule; Censys' separate secret is deliberately not included.
pub fn capability_configured(
    descriptor: &EngineDescriptor,
    config: &Config,
) -> (bool, AuthClass, bool, Vec<String>) {
    let entry = config.engines.get(&descriptor.name);
    let enabled = entry.is_none_or(|entry| entry.enabled);
    let auth = if descriptor.auth_class == AuthClass::Required {
        AuthClass::Required
    } else {
        entry.map_or(AuthClass::Unknown, |entry| entry.auth_class(descriptor))
    };
    let key_configured = entry.is_some_and(|entry| entry.api_key.is_configured());
    let categories = entry.map_or_else(
        || descriptor.categories.clone(),
        |entry| entry.effective_categories(descriptor).to_vec(),
    );
    (enabled, auth, key_configured, categories)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    static NEXT_TEMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    fn config_yaml(yaml: &str, env: &[(&str, &str)]) -> Config {
        let path = std::env::temp_dir().join(format!(
            "ssx-f06-{}-{}-{}.yaml",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::write(&path, yaml).unwrap();
        let env = env
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect();
        let result = load_config_with(Some(&path), &env);
        let _ = fs::remove_file(path);
        result.unwrap()
    }
    #[test]
    fn missing_empty_and_null_files_use_full_defaults() {
        let empty = config_yaml("", &[]);
        let missing = load_config_with(
            Some(Path::new("/no/such/slopsearx-f06-config")),
            &BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(empty, missing);
        assert_eq!(empty.engines.len(), 29);
        assert!(!empty.engines["deepseek"].enabled);
        assert!(!empty.engines["europepmc"].enabled);
        assert_eq!(
            empty.engines["brave"].max_results.effective_i64().unwrap(),
            20
        );
        assert_eq!(empty.default_engines, ["brave", "wikipedia"]);
        assert_eq!(config_yaml("null", &[]), empty);
    }
    #[test]
    fn file_is_partial_layer_and_supports_custom_entries() {
        let config = config_yaml(
            "engines:\n  brave:\n    max_results: 7\n    categories: [science]\n  fixture:\n    base_url: https://fixture.invalid\n",
            &[],
        );
        assert_eq!(
            config.engines["brave"].max_results.effective_i64().unwrap(),
            7
        );
        assert_eq!(
            config.engines["brave"].base_url,
            "https://api.search.brave.com/res/v1/web/search"
        );
        assert_eq!(
            config.engines["brave"].categories.as_deref().unwrap(),
            ["science"]
        );
        assert_eq!(
            config.engines["fixture"].base_url,
            "https://fixture.invalid"
        );
        assert_eq!(
            config.engines["fixture"]
                .timeout_ms
                .effective_i64()
                .unwrap(),
            5000
        );
        assert!(config.engines["fixture"].enabled);
    }
    #[test]
    fn environment_wins_and_category_replacement_preserves_empty_semantics() {
        let config = config_yaml(
            "engines:\n  brave:\n    max_results: 7\n    enabled: true\n",
            &[
                ("ENGINE_BRAVE_MAX_RESULTS", "3"),
                ("ENGINE_BRAVE_ENABLED", "false"),
                ("ENGINE_BRAVE_CATEGORIES", "science, ,news,,"),
                ("SEARCH_CACHE_TTL_SECONDS", "45"),
                ("SEARCH_LOG_LEVEL", "debug"),
                ("SEARCH_DEFAULT_ENGINES", "brave, wikipedia"),
                ("SEARCH_FORMATS", " JSON, ,HTML "),
            ],
        );
        assert_eq!(
            config.engines["brave"].max_results.effective_i64().unwrap(),
            3
        );
        assert!(!config.engines["brave"].enabled);
        assert_eq!(
            config.engines["brave"].categories.as_deref().unwrap(),
            ["science", "news"]
        );
        assert_eq!(config.cache.ttl_seconds, 45);
        assert_eq!(config.log_level, "DEBUG");
        assert_eq!(config.default_engines, ["brave", "wikipedia"]);
        assert_eq!(config.search.formats, ["json", "html"]);
        assert!(
            config_yaml("", &[("ENGINE_BRAVE_CATEGORIES", "")]).engines["brave"]
                .categories
                .as_deref()
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn numeric_environment_uses_python_grammar_and_distinct_strip_rules() {
        for raw in [" +١_٢ ", "\u{a0}+１２\u{2003}", "\u{10d41}\u{10d42}"] {
            let config = config_yaml("", &[("ENGINE_BRAVE_MAX_RESULTS", raw)]);
            assert_eq!(
                config.engines["brave"].max_results.effective_i64().unwrap(),
                12
            );
        }
        assert_eq!(
            config_yaml("", &[("ENGINE_BRAVE_TIMEOUT_MS", " -٣ ")]).engines["brave"]
                .timeout_ms
                .effective_i64()
                .unwrap(),
            -3
        );
        assert_eq!(
            config_yaml("", &[("SEARCH_CACHE_TTL_SECONDS", " +٤_٥ ")])
                .cache
                .ttl_seconds,
            45
        );
        assert_eq!(
            config_yaml("", &[("ENGINE_BRAVE_WEIGHT", " +١_٢.٥e+١ ")]).engines["brave"]
                .weight
                .effective_f64()
                .unwrap(),
            125.0
        );
        assert_eq!(
            config_yaml("", &[("ENGINE_ARXIV_RATE_LIMIT", " .５ ")]).engines["arxiv"].rate_limit,
            Some(ConfigScalar::Float(0.5))
        );
        for raw in ["true", "TRUE", "1", "yes"] {
            assert!(config_yaml("", &[("ENGINE_BRAVE_ENABLED", raw)]).engines["brave"].enabled);
        }
        for raw in [" true ", "yes\u{1c}", "false"] {
            assert!(!config_yaml("", &[("ENGINE_BRAVE_ENABLED", raw)]).engines["brave"].enabled);
        }
        let config = config_yaml(
            "",
            &[
                (
                    "SEARCH_DEFAULT_ENGINES",
                    "\u{1c}brave\u{1f}, ,wikipedia\u{1d}",
                ),
                ("ENGINE_BRAVE_CATEGORIES", "\u{1c}science\u{1f}, , news"),
                ("SEARCH_FORMATS", "\u{1c}JSON\u{1f}, , HTML"),
            ],
        );
        assert_eq!(config.default_engines, ["brave", "", "wikipedia"]);
        assert_eq!(
            config.engines["brave"].categories.as_ref().unwrap(),
            &["science", "news"]
        );
        assert_eq!(config.search.formats, ["json", "html"]);
        for raw in [
            "_2",
            "2_",
            "1__2",
            "١.٢",
            "\u{1c}2",
            "2e3",
            "0x10",
            "9223372036854775808",
        ] {
            let env = BTreeMap::from([("ENGINE_BRAVE_MAX_RESULTS".into(), raw.into())]);
            assert!(
                load_config_with(Some(Path::new("/no/such/f06-review-config")), &env).is_err(),
                "{raw:?}"
            );
        }
        for raw in ["0", "-1"] {
            let env = BTreeMap::from([("SEARCH_CACHE_TTL_SECONDS".into(), raw.into())]);
            assert!(load_config_with(Some(Path::new("/no/such/f06-review-config")), &env).is_err());
        }
    }
    #[test]
    fn scalar_origin_drives_environment_coercion_and_effective_validation() {
        let defaults = config_yaml("", &[]);
        assert!(matches!(
            defaults.engines["brave"].rate_limit,
            Some(ConfigScalar::Integer(15))
        ));
        assert!(matches!(
            defaults.engines["arxiv"].rate_limit,
            Some(ConfigScalar::Float(_))
        ));
        for raw in ["1.25", ".５"] {
            let env = BTreeMap::from([("ENGINE_BRAVE_RATE_LIMIT".into(), raw.into())]);
            assert!(load_config_with(Some(Path::new("/no/such/f06-origin-config")), &env).is_err());
        }
        let config = config_yaml("", &[("ENGINE_DUCKDUCKGO_RATE_LIMIT", "1.25")]);
        assert_eq!(
            config.engines["duckduckgo"].rate_limit,
            Some(ConfigScalar::Text("1.25".into()))
        );
        assert_eq!(
            config.engines["duckduckgo"]
                .rate_limit
                .as_ref()
                .unwrap()
                .effective_f64()
                .unwrap(),
            1.25
        );
        let invalid = config_yaml("", &[("ENGINE_DUCKDUCKGO_RATE_LIMIT", "not-a-number")]);
        assert!(
            invalid.engines["duckduckgo"]
                .rate_limit
                .as_ref()
                .unwrap()
                .effective_f64()
                .is_err()
        );
        let text = config_yaml(
            "engines:
  brave:
    max_results: file-text
",
            &[("ENGINE_BRAVE_MAX_RESULTS", "next-text")],
        );
        assert_eq!(
            text.engines["brave"].max_results,
            ConfigScalar::Text("next-text".into())
        );
        assert!(text.engines["brave"].max_results.effective_i64().is_err());
        let boolean = config_yaml(
            "engines:
  brave:
    weight: false
",
            &[("ENGINE_BRAVE_WEIGHT", " true ")],
        );
        assert_eq!(
            boolean.engines["brave"].weight,
            ConfigScalar::Boolean(false)
        );
        let float = config_yaml(
            "engines:
  brave:
    max_results: 1.0
",
            &[("ENGINE_BRAVE_MAX_RESULTS", "1.25")],
        );
        assert_eq!(
            float.engines["brave"].max_results,
            ConfigScalar::Float(1.25)
        );
        assert!(float.engines["brave"].max_results.effective_i64().is_err());
        let mut integer = config_yaml(
            "engines:
  brave:
    weight: 1
",
            &[],
        );
        assert!(
            apply_engine_env(integer.engines.get_mut("brave").unwrap(), "weight", "1.25").is_err()
        );
        for yaml in [
            "engines:
  brave:
    weight: [1]
",
            "engines:
  brave:
    max_results: 9223372036854775808
",
        ] {
            assert!(load_yaml_text(yaml).is_err());
        }
        assert!(ConfigScalar::Float(f64::INFINITY).effective_f64().is_err());
        assert!(ConfigScalar::Text("NaN".into()).effective_f64().is_err());
        assert!(
            ConfigScalar::Text("9223372036854775808".into())
                .effective_i64()
                .is_err()
        );
    }
    #[test]
    fn search_suffix_aliases_use_source_case_and_last_value_before_coercion() {
        let config = config_yaml(
            "",
            &[
                ("SEARCH_default_engines", " arxiv , wikipedia "),
                ("SEARCH_Log_Level", "debug"),
                ("SEARCH_cache_ttl_seconds", " +٤_٥ "),
                ("SEARCH_Formats", " JSON, HTML "),
            ],
        );
        assert_eq!(config.default_engines, ["arxiv", "wikipedia"]);
        assert_eq!(config.log_level, "DEBUG");
        assert_eq!(config.cache.ttl_seconds, 45);
        assert_eq!(config.search.formats, ["json", "html"]);
        let ignored = config_yaml(
            "",
            &[("search_FORMATS", "yaml"), ("SEARCH_FORMATſ", "yaml")],
        );
        assert_eq!(ignored.search.formats, SearchConfig::default().formats);
        let mut config = Config::default();
        apply_environment(
            &mut config,
            &[
                ("SEARCH_CACHE_TTL_SECONDS".into(), "invalid".into()),
                ("SEARCH_cache_ttl_seconds".into(), "45".into()),
            ],
        )
        .unwrap();
        assert_eq!(config.cache.ttl_seconds, 45);
        assert!(
            apply_environment(
                &mut config,
                &[
                    ("SEARCH_cache_ttl_seconds".into(), "45".into()),
                    ("SEARCH_CACHE_TTL_SECONDS".into(), "invalid".into())
                ]
            )
            .is_err()
        );
    }
    #[test]
    fn source_captured_scalar_and_search_replay() {
        let corpus: Value =
            serde_json::from_str(include_str!("../../xtask/fixtures/f06/scalars.json")).unwrap();
        assert_eq!(corpus["cases"].as_array().unwrap().len(), 21);
        for case in corpus["cases"].as_array().unwrap() {
            let path = std::env::temp_dir().join(format!(
                "ssx-f06-replay-{}-{}.yaml",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            fs::write(&path, case["yaml"].as_str().unwrap()).unwrap();
            let entries: Vec<_> = case["env"]
                .as_array()
                .unwrap()
                .iter()
                .map(|pair| {
                    (
                        pair[0].as_str().unwrap().to_owned(),
                        pair[1].as_str().unwrap().to_owned(),
                    )
                })
                .collect();
            let result = load_config_entries(Some(&path), &entries);
            fs::remove_file(path).unwrap();
            let observed = match result {
                Err(_) => json!({"error":"invalid"}),
                Ok(config) => {
                    let scalar = |value: Option<&ConfigScalar>| match value {
                        None => json!({"kind":"NoneType","value":null}),
                        Some(value) => {
                            json!({"kind":match value {ConfigScalar::Boolean(_)=>"bool",ConfigScalar::Integer(_)=>"int",ConfigScalar::Float(_)=>"float",ConfigScalar::Text(_)=>"str"},"value":value})
                        }
                    };
                    let mut engines = Map::new();
                    for name in ["brave", "arxiv", "duckduckgo"] {
                        let e = &config.engines[name];
                        engines.insert(name.into(),json!({"enabled":e.enabled,"categories":e.categories,"scalars":{"timeout_ms":scalar(Some(&e.timeout_ms)),"max_results":scalar(Some(&e.max_results)),"weight":scalar(Some(&e.weight)),"rate_limit":scalar(e.rate_limit.as_ref())}}));
                    }
                    json!({"engines":engines,"ttl":config.cache.ttl_seconds,"default_engines":config.default_engines,"formats":config.search.formats,"log_level":config.log_level})
                }
            };
            assert_eq!(observed, case["expected"], "{}", case["name"]);
        }
    }
    #[test]
    fn hf_alias_is_overridden_by_explicit_variable_and_secrets_are_redacted() {
        let alias = config_yaml("", &[("HF_TOKEN", "private-hf-token")]);
        assert_eq!(
            alias.engines["huggingface"].api_key.expose(),
            Some("private-hf-token")
        );
        let explicit = config_yaml(
            "",
            &[
                ("HF_TOKEN", "alias-value"),
                ("ENGINE_HUGGINGFACE_API_KEY", "explicit-value"),
                ("ENGINE_CENSYS_API_KEY", "censys-id"),
                ("ENGINE_CENSYS_API_SECRET", "censys-secret"),
            ],
        );
        assert_eq!(
            explicit.engines["huggingface"].api_key.expose(),
            Some("explicit-value")
        );
        let debug = format!("{:?}", explicit.engines["censys"]);
        assert!(!debug.contains("censys-id") && !debug.contains("censys-secret"));
        let registry = Registry::builtins().unwrap();
        let (enabled, auth, key, _) =
            capability_configured(registry.descriptor("censys").unwrap(), &explicit);
        assert!(enabled && key && auth == AuthClass::Required);
        assert!(explicit.engines["censys"].api_secret.is_configured());
    }
    #[test]
    fn file_credentials_are_accepted_then_overridden_by_environment_without_logging() {
        let file = config_yaml(
            "engines:\n  censys:\n    api_key: file-id\n    api_secret: file-secret\n",
            &[],
        );
        assert_eq!(file.engines["censys"].api_key.expose(), Some("file-id"));
        assert_eq!(
            file.engines["censys"].api_secret.expose(),
            Some("file-secret")
        );
        let env = config_yaml(
            "engines:\n  censys:\n    api_key: file-id\n    api_secret: file-secret\n",
            &[
                ("ENGINE_CENSYS_API_KEY", "env-id"),
                ("ENGINE_CENSYS_API_SECRET", "env-secret"),
            ],
        );
        assert_eq!(env.engines["censys"].api_key.expose(), Some("env-id"));
        assert_eq!(
            env.engines["censys"].api_secret.expose(),
            Some("env-secret")
        );
        let file_debug = format!("{file:?}");
        let file_json = serde_json::to_string(&file).unwrap();
        assert!(!file_debug.contains("file-id") && !file_debug.contains("file-secret"));
        assert!(!file_json.contains("file-id") && !file_json.contains("file-secret"));
        assert!(serde_json::to_string(&file.engines["censys"].api_key).is_err());
        let debug = format!("{env:?}");
        assert!(
            !debug.contains("env-id")
                && !debug.contains("env-secret")
                && !debug.contains("file-secret")
        );
    }
    #[test]
    fn source_captured_yaml_origin_controls_environment_coercion() {
        let corpus: Value =
            serde_json::from_str(include_str!("../../xtask/fixtures/f06/yaml.json")).unwrap();
        for case in corpus["config_cases"].as_array().unwrap() {
            let env: Vec<_> = case["env"]
                .as_array()
                .unwrap()
                .iter()
                .map(|pair| (pair[0].as_str().unwrap(), pair[1].as_str().unwrap()))
                .collect();
            let config = config_yaml(case["yaml"].as_str().unwrap(), &env);
            let engine = &config.engines["brave"];
            for (name, value) in [
                ("weight", &engine.weight),
                ("max_results", &engine.max_results),
            ] {
                let kind = match value {
                    ConfigScalar::Boolean(_) => "bool",
                    ConfigScalar::Integer(_) => "int",
                    ConfigScalar::Float(_) => "float",
                    ConfigScalar::Text(_) => "str",
                };
                assert_eq!(
                    json!({"kind":kind,"value":value}),
                    case["expected"][name],
                    "{}",
                    case["name"]
                );
            }
        }
    }
    #[test]
    fn source_captured_file_credential_replay() {
        let corpus: Value =
            serde_json::from_str(include_str!("../../xtask/fixtures/f06/credentials.json"))
                .unwrap();
        for case in corpus["cases"].as_array().unwrap() {
            let env: Vec<(&str, &str)> = case["env"]
                .as_array()
                .unwrap()
                .iter()
                .map(|pair| (pair[0].as_str().unwrap(), pair[1].as_str().unwrap()))
                .collect();
            let config = config_yaml(case["yaml"].as_str().unwrap(), &env);
            let entry = &config.engines[case["engine"].as_str().unwrap()];
            assert_eq!(
                entry.api_key.expose(),
                case["expected"]["api_key"].as_str(),
                "{}",
                case["name"]
            );
            assert_eq!(
                entry.api_secret.expose(),
                case["expected"]["api_secret"].as_str(),
                "{}",
                case["name"]
            );
            let debug = format!("{config:?}");
            let encoded = serde_json::to_string(&config).unwrap();
            for secret in [entry.api_key.expose(), entry.api_secret.expose()]
                .into_iter()
                .flatten()
                .filter(|s| !s.is_empty())
            {
                assert!(!debug.contains(secret) && !encoded.contains(secret));
            }
        }
    }
    #[test]
    fn malformed_file_credentials_are_redacted_and_failed_overlay_is_atomic() {
        let mut entry = EngineConfig {
            api_key: Secret::new(Some("original-fixture-key".into())),
            api_secret: Secret::new(Some("original-fixture-secret".into())),
            ..Default::default()
        };
        let before = entry.clone();
        let invalid =
            json!({"api_key":"next-fixture-key", "api_secret":{"nested":"never-echo-fixture"}});
        let error = apply_engine_file(&mut entry, invalid.as_object().unwrap()).unwrap_err();
        assert_eq!(entry, before);
        assert!(
            !error.to_string().contains("next-fixture-key")
                && !error.to_string().contains("never-echo-fixture")
        );
        for value in [json!(["never-echo-fixture"]), json!(true), json!(123)] {
            assert!(file_secret(&value).is_err());
        }
        let overlay = json!({"weight":2});
        apply_engine_file(&mut entry, overlay.as_object().unwrap()).unwrap();
        assert_eq!(entry.api_key, before.api_key);
        assert_eq!(entry.api_secret, before.api_secret);
    }
    #[test]
    fn malformed_yaml_errors_never_echo_secret_values() {
        let error = load_yaml_text("api_key: highly-sensitive-fixture\ncache: [\n").unwrap_err();
        assert!(!error.to_string().contains("highly-sensitive-fixture"));
    }
    #[test]
    fn serialized_configuration_and_capability_views_are_secret_free() {
        let config = config_yaml(
            "",
            &[
                ("ENGINE_CENSYS_API_KEY", "fixture-key"),
                ("ENGINE_CENSYS_API_SECRET", "fixture-secret"),
            ],
        );
        let encoded = serde_json::to_string(&config).unwrap();
        assert!(!encoded.contains("fixture-key") && !encoded.contains("fixture-secret"));
        let registry = Registry::builtins().unwrap();
        let (_, _, configured, categories) =
            capability_configured(registry.descriptor("censys").unwrap(), &config);
        let output = serde_json::to_string(&(configured, categories)).unwrap();
        assert!(
            configured && !output.contains("fixture-key") && !output.contains("fixture-secret")
        );
    }
    #[test]
    fn yaml_root_types_and_boolean_env_retain_defined_failure_rules() {
        // Python's `yaml.safe_load(...) or {}` accepts falsey empty values,
        // including an empty list, and rejects truthy non-mapping roots.
        assert!(load_yaml_text("[]").is_ok());
        assert!(load_yaml_text("scalar").is_err());
        assert!(load_yaml_text("3").is_err());
        let env = [("SEARCH_CACHE_TTL_SECONDS".to_owned(), "bad".to_owned())]
            .into_iter()
            .collect();
        assert!(load_config_with(None, &env).is_err());
        let enabled = config_yaml("", &[("ENGINE_BRAVE_ENABLED", "yes")]);
        let disabled = config_yaml("", &[("ENGINE_BRAVE_ENABLED", "on")]);
        assert!(enabled.engines["brave"].enabled);
        assert!(!disabled.engines["brave"].enabled);
    }
    #[test]
    fn extension_fields_are_preserved_where_python_accepts_them() {
        let config = config_yaml(
            "future_setting: ignored\ncache:\n  experimental: 7\nranking:\n  future_mode: true\nrouting:\n  custom: ignored\nsearch:\n  other: ignored\n",
            &[],
        );
        assert_eq!(config.cache.extension_fields["experimental"], json!(7));
        assert_eq!(config.ranking.extension_fields["future_mode"], json!(true));
        assert!(config.routing.enabled);
        assert_eq!(config.search.formats, SearchConfig::default().formats);
    }
    #[test]
    fn unconfigured_adapters_remain_registered_with_unknown_auth_state() {
        let registry = Registry::builtins().unwrap();
        let config = config_yaml("", &[]);
        let descriptor = registry.descriptor("clinicaltrials").unwrap();
        assert!(!config.engines.contains_key("clinicaltrials"));
        let (enabled, auth, key, categories) = capability_configured(descriptor, &config);
        assert!(enabled);
        assert_eq!(auth, AuthClass::Unknown);
        assert!(!key);
        assert_eq!(categories, descriptor.categories);
    }
    #[test]
    fn unconfigured_required_auth_adapters_report_required_without_credentials() {
        let registry = Registry::builtins().unwrap();
        let config = config_yaml("", &[]);
        for name in ["fred", "tmdb", "urlhaus"] {
            let descriptor = registry.descriptor(name).unwrap();
            assert!(!config.engines.contains_key(name));
            let (enabled, auth, configured, _) = capability_configured(descriptor, &config);
            assert!(enabled, "{name}");
            assert_eq!(auth, AuthClass::Required, "{name}");
            assert!(!configured, "{name}");
        }
    }
    fn load_yaml_text(text: &str) -> Result<Config, ConfigError> {
        let path = std::env::temp_dir().join(format!(
            "ssx-f06-bad-{}-{}-{}.yaml",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::write(&path, text).unwrap();
        let result = load_config_with(Some(&path), &BTreeMap::new());
        let _ = fs::remove_file(path);
        result
    }
}

pub mod browser_identity;
