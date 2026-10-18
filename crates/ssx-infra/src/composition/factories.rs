use super::*;
fn numeric(config: &Config, name: &str) -> Result<Option<(u64, usize)>> {
    let Some(e) = config.engines.get(name).filter(|e| e.enabled) else {
        return Ok(None);
    };
    let ms = e
        .timeout_ms
        .effective_i64()
        .map_err(|_| Error::new("config", "adapter_timeout"))?;
    let count = e
        .max_results
        .effective_i64()
        .map_err(|_| Error::new("config", "adapter_max_results"))?;
    Ok(Some((
        u64::try_from(ms).map_err(|_| Error::new("config", "adapter_timeout"))?,
        usize::try_from(count).map_err(|_| Error::new("config", "adapter_max_results"))?,
    )))
}
fn base(config: &Config, name: &str, default: &str) -> String {
    config
        .engines
        .get(name)
        .map(|e| e.base_url.as_str())
        .filter(|s| !s.chars().all(ssx_json::python_whitespace))
        .unwrap_or(default)
        .to_owned()
}
fn key(config: &Config, name: &str) -> String {
    config
        .engines
        .get(name)
        .filter(|e| e.enabled)
        .and_then(|e| e.api_key.expose())
        .unwrap_or("")
        .into()
}
pub(super) fn register(
    registry: &mut Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: &Config,
    settings: &SearchSettings,
) -> Result<()> {
    macro_rules! mapped {
        ($settings:expr,$name:expr) => {{
            let value = &mut $settings;
            if let Some((ms, count)) = numeric(config, $name)? {
                value.timeout_ms = ms;
                value.max_results = count;
                value.base_url = base(config, $name, &value.base_url);
            }
        }};
    }
    macro_rules! attach {
        ($fn:ident,$settings:expr) => {
            adapters::$fn(registry, http.clone(), admission.clone(), $settings)
                .map_err(|_| Error::new("registry", stringify!($fn)))?
        };
    }
    let mut pilots = adapters::PilotConfig::default();
    mapped!(pilots.npm, "npm");
    mapped!(pilots.crates, "crates");
    attach!(register_pilots, pilots);
    let mut broad = adapters::BroadApiConfig::default();
    mapped!(broad.brave, "brave");
    mapped!(broad.wikipedia, "wikipedia");
    mapped!(broad.reddit, "reddit");
    broad.brave.api_key = key(config, "brave");
    broad.brave.category_routing = settings.brave_category_routing;
    broad.reddit.api_key = key(config, "reddit");
    if let Some(e) = config.engines.get("reddit") {
        broad.reddit.access_token = e
            .extra_env_settings
            .get("access_token")
            .cloned()
            .unwrap_or_default();
    }
    if let Some(e) = config.engines.get("wikipedia") {
        broad.wikipedia.user_agent = e.user_agent.clone().unwrap_or_default();
    }
    attach!(register_broad_api, broad);
    let mut packages = adapters::PackageConfig::default();
    mapped!(packages.pypi, "pypi");
    mapped!(packages.rubygems, "rubygems");
    mapped!(packages.dockerhub, "dockerhub");
    attach!(register_packages, packages);
    let mut community = adapters::CommunityConfig::default();
    for (name, value) in [
        ("github", &mut community.github.0),
        ("huggingface", &mut community.huggingface.0),
        ("stackexchange", &mut community.stackexchange.0),
        ("hackernews", &mut community.hackernews.0),
    ] {
        if let Some(entry) = config.engines.get(name).filter(|e| e.enabled) {
            let secret = key(config, name);
            let secret = if secret.is_empty() {
                None
            } else {
                Some(
                    ssx_domain::oauth::Secret::new(secret)
                        .map_err(|_| Error::new("config", "community_credential"))?,
                )
            };
            let max_text = serde_json::to_string(&entry.max_results)
                .map_err(|_| Error::new("config", "community_max_results"))?;
            let max_results = ssx_json::from_str(&max_text)
                .map_err(|_| Error::new("config", "community_max_results"))?;
            *value = adapters::CommunitySettings::resolve(
                base(config, name, &value.base_url),
                entry
                    .timeout_ms
                    .effective_f64()
                    .map_err(|_| Error::new("config", "community_timeout"))?,
                &max_results,
                secret,
            )
            .map_err(|_| Error::new("config", "community_settings"))?;
        }
    }
    attach!(register_community, community);
    attach!(register_jobs, adapters::JobsConfig::from_resolved(config));
    attach!(
        register_biomedical,
        adapters::BiomedicalSettings::from_resolved(config)
    );
    attach!(
        register_regulatory,
        adapters::RegulatorySettings::from_resolved(config)
    );
    attach!(
        register_catalog_media,
        adapters::CatalogMediaSettings::from_resolved(config)
    );
    attach!(
        register_threat_intelligence,
        adapters::ThreatIntelSettings::from_resolved(config)
    );
    // HTML/other families are deliberately unavailable until their lifecycle/config
    // mappings are implemented. resolve() refuses any enabled missing factory.
    Ok(())
}

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;
