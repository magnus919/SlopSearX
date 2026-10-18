use super::*;
use serde_json::Value;
pub(super) struct WorkerPolicy {
    pub search: SearchPolicy,
    pub required: BTreeSet<String>,
    pub snapshot_ttl: u64,
    pub lease: NonZeroU64,
    pub poll: Duration,
    pub workers: NonZeroU64,
    pub ceilings: ResearchCeilings,
}
fn strip(s: &str) -> &str {
    s.trim_matches(ssx_json::python_whitespace)
}
fn boolean(s: &str) -> Option<bool> {
    match strip(s).to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}
fn integer(v: &Value) -> Option<u64> {
    if v == &Value::Bool(true) {
        Some(1)
    } else {
        v.as_u64().filter(|v| *v > 0)
    }
}
fn number(
    section: &Value,
    env: &BTreeMap<String, String>,
    field: &str,
    key: &str,
    default: u64,
) -> Result<u64> {
    let mut value = section.get(field).and_then(integer).unwrap_or(default);
    if let Some(raw) = env.get(key).map(|s| strip(s)).filter(|s| !s.is_empty())
        && let Ok(normalized) = ssx_domain::storage::normalize_python_number_text(raw)
    {
        let digits = normalized.strip_prefix('+').unwrap_or(&normalized);
        if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
            let parsed = digits
                .parse::<u64>()
                .map_err(|_| Error::new("config", key))?;
            if parsed > 0 {
                value = parsed;
            }
        }
    }
    Ok(value)
}
impl WorkerPolicy {
    pub(super) fn resolve(section: &Value, env: &BTreeMap<String, String>) -> Result<Self> {
        let mut search = SearchPolicy::default();
        for (name, key) in [
            ("jobs", "MCP_GRANT_JOBS"),
            ("security", "MCP_GRANT_SECURITY"),
            ("science", "MCP_GRANT_SCIENCE"),
            ("research", "MCP_GRANT_RESEARCH"),
            ("dependency_dossier", "MCP_GRANT_DEPENDENCY_DOSSIER"),
            ("staged_search", "MCP_GRANT_STAGED_SEARCH"),
            ("retrieval_receipts", "MCP_GRANT_RETRIEVAL_RECEIPTS"),
            ("saved_searches", "MCP_GRANT_SAVED_SEARCHES"),
            ("saved_search_events", "MCP_GRANT_SAVED_SEARCH_EVENTS"),
        ] {
            let enabled = env.get(key).and_then(|v| boolean(v)).unwrap_or_else(|| {
                section
                    .get("enabled_tools")
                    .and_then(|v| v.get(name))
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            });
            if enabled {
                search.grants.insert(name.into());
            }
        }
        search.sensitive_allowed = env
            .get("MCP_TARGETED_SENSITIVE_ALLOWED")
            .and_then(|v| boolean(v))
            .unwrap_or_else(|| {
                section
                    .get("targeted_sensitive_allowed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            });
        if let Some(values) = section.get("sensitive_engines").and_then(Value::as_array) {
            search.sensitive_engines = values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
        }
        if let Some(raw) = env.get("MCP_SENSITIVE_ENGINES").filter(|s| !s.is_empty()) {
            search.sensitive_engines = raw
                .split(',')
                .map(strip)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
        }
        let mut required: BTreeSet<String> = [
            "abuseipdb",
            "brave",
            "censys",
            "dehashed",
            "fred",
            "hibp",
            "intelx",
            "otx",
            "shodan",
            "tmdb",
            "urlhaus",
            "virustotal",
            "vulncheck",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        if let Some(values) = section
            .get("required_key_engines")
            .and_then(Value::as_array)
        {
            required = values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
        }
        search.max_query_length = usize::try_from(number(
            section,
            env,
            "max_query_length",
            "MCP_MAX_QUERY_LENGTH",
            500,
        )?)
        .map_err(|_| Error::new("config", "max_query_length"))?;
        let snapshot_ttl = number(
            section,
            env,
            "snapshot_ttl_seconds",
            "MCP_SNAPSHOT_TTL_SECONDS",
            3600,
        )?;
        SnapshotTtl::new(snapshot_ttl).map_err(|_| Error::new("config", "snapshot_ttl_seconds"))?;
        let lease = NonZeroU64::new(number(
            section,
            env,
            "job_lease_ttl_seconds",
            "MCP_JOB_LEASE_TTL_SECONDS",
            60,
        )?)
        .ok_or_else(|| Error::new("config", "lease"))?;
        let workers = NonZeroU64::new(number(
            section,
            env,
            "job_max_concurrent_jobs",
            "MCP_JOB_MAX_CONCURRENT_JOBS",
            1,
        )?)
        .ok_or_else(|| Error::new("config", "workers"))?;
        if workers.get() > 128 || lease.get() > i64::MAX as u64 / 1000 / 2 {
            return Err(Error::new("config", "native_worker_bounds"));
        }
        let mut poll = section
            .get("job_poll_interval_seconds")
            .and_then(|v| {
                if v == &Value::Bool(true) {
                    Some(1.0)
                } else {
                    v.as_f64()
                }
            })
            .filter(|n| *n > 0.0)
            .unwrap_or(1.0);
        if let Some(raw) = env.get("MCP_JOB_POLL_INTERVAL_SECONDS")
            && let Ok(n) = strip(raw).parse::<f64>()
            && n > 0.0
        {
            poll = n;
        }
        let poll = Duration::try_from_secs_f64(poll)
            .map_err(|_| Error::new("config", "job_poll_interval_seconds"))?;
        if poll.is_zero() || Instant::now().checked_add(poll).is_none() {
            return Err(Error::new("config", "job_poll_interval_seconds"));
        }
        let ceilings = ResearchCeilings {
            queries: number(section, env, "job_max_queries", "MCP_JOB_MAX_QUERIES", 20)?,
            engines_per_query: number(
                section,
                env,
                "job_max_engines_per_query",
                "MCP_JOB_MAX_ENGINES_PER_QUERY",
                10,
            )?,
            results: number(section, env, "job_max_results", "MCP_JOB_MAX_RESULTS", 500)?,
        };
        Ok(Self {
            search,
            required,
            snapshot_ttl,
            lease,
            poll,
            workers,
            ceilings,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_policy_environment_overrides_file_grants_and_timing() {
        let section = serde_json::json!({"enabled_tools":{"research":false},"job_lease_ttl_seconds":8,"job_poll_interval_seconds":0.5,"sensitive_engines":["custom"]});
        let env = BTreeMap::from([
            ("MCP_GRANT_RESEARCH".into(), " yes ".into()),
            ("MCP_JOB_LEASE_TTL_SECONDS".into(), "12".into()),
        ]);
        let p = WorkerPolicy::resolve(&section, &env).unwrap();
        assert!(p.search.grants.contains("research"));
        assert_eq!(p.lease.get(), 12);
        assert_eq!(p.poll, Duration::from_millis(500));
        assert!(p.search.sensitive_engines.contains("custom"));
        assert!(!p.search.sensitive_allowed);
    }
    #[test]
    fn worker_policy_malformed_override_preserves_file_value() {
        let p = WorkerPolicy::resolve(
            &serde_json::json!({"job_lease_ttl_seconds":8}),
            &BTreeMap::from([("MCP_JOB_LEASE_TTL_SECONDS".into(), "wrong".into())]),
        )
        .unwrap();
        assert_eq!(p.lease.get(), 8);
    }
    #[test]
    fn worker_policy_unrepresentable_bounds_reject_before_io() {
        assert!(
            WorkerPolicy::resolve(
                &Value::Null,
                &BTreeMap::from([("MCP_JOB_MAX_CONCURRENT_JOBS".into(), "129".into())])
            )
            .is_err()
        );
    }
}

/// Native operator projection for the already accepted O01 options.
/// These names are new explicit native controls, not claimed Python aliases.
pub(super) struct ObservabilitySettings {
    monitoring: bool,
    audit: bool,
    privacy: crate::observability::QueryPrivacy,
}
impl ObservabilitySettings {
    pub(super) fn resolve(section: &Value, env: &BTreeMap<String, String>) -> Result<Self> {
        use crate::observability::QueryPrivacy;
        let mut value = Self {
            monitoring: true,
            audit: true,
            privacy: QueryPrivacy::Redacted,
        };
        if !section.is_null() {
            let fields = section
                .as_object()
                .ok_or_else(|| Error::new("config", "observability"))?;
            for (key, item) in fields {
                match key.as_str() {
                    "monitoring" => {
                        value.monitoring = item
                            .as_bool()
                            .ok_or_else(|| Error::new("config", "observability.monitoring"))?
                    }
                    "audit" => {
                        value.audit = item
                            .as_bool()
                            .ok_or_else(|| Error::new("config", "observability.audit"))?
                    }
                    "privacy" => {
                        value.privacy = match item.as_str() {
                            Some("redacted") => QueryPrivacy::Redacted,
                            Some("retain_raw") => QueryPrivacy::RetainRaw,
                            _ => return Err(Error::new("config", "observability.privacy")),
                        }
                    }
                    _ => return Err(Error::new("unsupported", "observability.option")),
                }
            }
        }
        for (key, target) in [
            ("SSX_MONITORING_ENABLED", &mut value.monitoring),
            ("SSX_AUDIT_ENABLED", &mut value.audit),
        ] {
            if let Some(raw) = env.get(key) {
                *target = boolean(raw).ok_or_else(|| Error::new("config", key))?;
            }
        }
        if let Some(raw) = env.get("SSX_AUDIT_PRIVACY") {
            value.privacy = match strip(raw) {
                "redacted" => QueryPrivacy::Redacted,
                "retain_raw" => QueryPrivacy::RetainRaw,
                _ => return Err(Error::new("config", "SSX_AUDIT_PRIVACY")),
            };
        }
        Ok(value)
    }
    pub(super) fn options(&self) -> Options {
        Options {
            monitoring: self.monitoring,
            audit: self.audit,
            privacy: self.privacy,
            ..Options::default()
        }
    }
}
#[cfg(test)]
mod observability_tests {
    use super::*;
    #[test]
    fn observability_operator_controls_reach_accepted_options() {
        let settings = ObservabilitySettings::resolve(
            &serde_json::json!({"monitoring":false,"audit":false,"privacy":"retain_raw"}),
            &BTreeMap::from([("SSX_AUDIT_ENABLED".into(), "yes".into())]),
        )
        .unwrap();
        let options = settings.options();
        assert!(!options.monitoring);
        assert!(options.audit);
        assert!(matches!(
            options.privacy,
            crate::observability::QueryPrivacy::RetainRaw
        ));
        let defaults = ObservabilitySettings::resolve(&Value::Null, &BTreeMap::new())
            .unwrap()
            .options();
        assert!(defaults.monitoring);
        assert!(defaults.audit);
        assert!(matches!(
            defaults.privacy,
            crate::observability::QueryPrivacy::Redacted
        ));
    }
    #[test]
    fn observability_unknown_or_malformed_controls_fail_preflight() {
        assert!(
            ObservabilitySettings::resolve(
                &serde_json::json!({"privacy":"unknown"}),
                &BTreeMap::new()
            )
            .is_err()
        );
        assert!(
            ObservabilitySettings::resolve(
                &serde_json::json!({"sentry_dsn":"fixture-secret"}),
                &BTreeMap::new()
            )
            .is_err()
        );
        assert!(
            ObservabilitySettings::resolve(
                &Value::Null,
                &BTreeMap::from([("SSX_MONITORING_ENABLED".into(), "maybe".into())])
            )
            .is_err()
        );
    }
}
