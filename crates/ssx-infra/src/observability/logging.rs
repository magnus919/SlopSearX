//! Allowlisted structured diagnostics and native monitoring configuration.
//! This module encodes events only; dispatch belongs to the bounded exporter.
use ssx_json::{Value, json};
#[derive(Clone, Copy)]
pub enum Level {
    Debug,
    Info,
    Warning,
    Error,
}
impl Level {
    fn text(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
    fn rank(self) -> u8 {
        match self {
            Self::Debug => 0,
            Self::Info => 1,
            Self::Warning => 2,
            Self::Error => 3,
        }
    }
}
#[derive(Clone, Copy)]
pub enum LogEvent {
    RequestCompleted,
    JobCompleted,
    StoreUnavailable,
    TelemetryDropped,
    InternalFailure,
}
impl LogEvent {
    fn text(self) -> &'static str {
        match self {
            Self::RequestCompleted => "request_completed",
            Self::JobCompleted => "job_completed",
            Self::StoreUnavailable => "store_unavailable",
            Self::TelemetryDropped => "telemetry_dropped",
            Self::InternalFailure => "internal_failure",
        }
    }
}
/// Arbitrary message/error text is excluded because it can contain credentials.
pub struct LogRecord {
    pub event: LogEvent,
    pub level: Level,
    pub request_id: Option<String>,
    pub query_id: Option<String>,
    pub duration_ms: Option<u64>,
}
impl LogRecord {
    pub fn write(
        &self,
        writer: &mut impl std::io::Write,
        minimum: Level,
        json_output: bool,
    ) -> std::io::Result<()> {
        if let Some(line) = self.render(minimum, json_output) {
            writeln!(writer, "{line}")?;
        }
        Ok(())
    }
    pub fn json(&self) -> Value {
        let id = self.request_id.as_deref().filter(|s| valid_request_id(s));
        let query_id = self
            .query_id
            .as_deref()
            .filter(|s| s.len() == 12 && s.bytes().all(|b| b.is_ascii_hexdigit()));
        json!({"timestamp":chrono::Utc::now().to_rfc3339(),"logger":"slopsearx","level":self.level.text(),"event":self.event.text(),"request_id":id,"query_id":query_id,"duration_ms":self.duration_ms})
    }
    pub fn render(&self, minimum: Level, json_output: bool) -> Option<String> {
        if self.level.rank() < minimum.rank() {
            return None;
        }
        let value = self.json();
        Some(if json_output {
            value.to_string()
        } else {
            let mut line = format!(
                "{} {} {}",
                value["timestamp"],
                self.level.text(),
                self.event.text()
            );
            for field in ["request_id", "query_id", "duration_ms"] {
                if !value[field].is_null() {
                    line += &format!(" {field}={}", value[field]);
                }
            }
            line
        })
    }
    pub fn sentry_event(&self) -> Value {
        json!({"platform":"rust","level":self.level.text(),"logger":"slopsearx","message":self.event.text(),"tags":{"request_id":self.json()["request_id"].clone(),"query_id":self.json()["query_id"].clone()}})
    }
}
/// Credentials deliberately have no Debug implementation.
pub struct SentryConfig {
    pub endpoint: url::Url,
    key: String,
    secret: Option<String>,
    pub sample_rate: f64,
}
impl SentryConfig {
    /// Source explicit-nonempty selection precedes the supplied environment value.
    /// Reading environment is left to the configuration owner, never the exporter.
    pub fn resolve(
        explicit: Option<&str>,
        environment: Option<&str>,
        sample_rate: f64,
    ) -> Result<Option<Self>, &'static str> {
        Self::parse(
            explicit.filter(|s| !s.is_empty()).or(environment),
            sample_rate,
        )
    }
    pub fn parse(dsn: Option<&str>, sample_rate: f64) -> Result<Option<Self>, &'static str> {
        let Some(dsn) = dsn.filter(|s| !s.is_empty()) else {
            return Ok(None);
        };
        if dsn.len() > 4096 || !sample_rate.is_finite() || !(0.0..=1.0).contains(&sample_rate) {
            return Err("invalid monitoring configuration");
        }
        let mut endpoint = url::Url::parse(dsn).map_err(|_| "invalid Sentry DSN")?;
        if !["https", "http"].contains(&endpoint.scheme())
            || endpoint.username().is_empty()
            || endpoint.host_str().is_none()
        {
            return Err("invalid Sentry DSN");
        }
        let key = endpoint.username().to_owned();
        let secret = endpoint.password().map(str::to_owned);
        if key.len() > 128 || secret.as_ref().is_some_and(|s| s.len() > 128) {
            return Err("invalid Sentry credentials");
        }
        // Read the unescaped source path before WHATWG percent-encoding so Python
        // Unicode-decimal project normalization remains observable.
        let rest = dsn.split_once("://").ok_or("invalid Sentry DSN")?.1;
        let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let raw_path = rest[authority_end..].split(['?', '#']).next().unwrap_or("");
        let (prefix, project) = raw_path.rsplit_once('/').ok_or("invalid Sentry project")?;
        let normalized = ssx_domain::storage::normalize_python_number_text(project)
            .map_err(|_| "invalid Sentry project")?;
        let (negative, digits) = if let Some(s) = normalized.strip_prefix('-') {
            (true, s)
        } else {
            (false, normalized.strip_prefix('+').unwrap_or(&normalized))
        };
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err("invalid Sentry project");
        }
        let digits = digits.trim_start_matches('0');
        let digits = if digits.is_empty() { "0" } else { digits };
        let sign = if negative && digits != "0" { "-" } else { "" };
        endpoint.set_path(&format!("{prefix}/api/{sign}{digits}/envelope/"));
        endpoint.set_query(None);
        endpoint.set_fragment(None);
        endpoint
            .set_username("")
            .map_err(|_| "invalid Sentry DSN")?;
        endpoint
            .set_password(None)
            .map_err(|_| "invalid Sentry DSN")?;
        let config = Self {
            endpoint,
            key,
            secret,
            sample_rate,
        };
        reqwest::header::HeaderValue::from_str(&config.authentication())
            .map_err(|_| "invalid Sentry credentials")?;
        Ok(Some(config))
    }
    pub fn authentication(&self) -> String {
        let mut value = format!(
            "Sentry sentry_key={}, sentry_version=7, sentry_client=slopsearx.rust/{}",
            self.key,
            env!("CARGO_PKG_VERSION")
        );
        if let Some(secret) = &self.secret {
            value.push_str(", sentry_secret=");
            value.push_str(secret)
        }
        value
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostics_do_not_contain_raw_text() {
        let record = LogRecord {
            event: LogEvent::InternalFailure,
            level: Level::Error,
            request_id: Some("api_key=secret".into()),
            query_id: None,
            duration_ms: Some(3),
        };
        assert!(!record.json().to_string().contains("secret"));
        assert!(!record.sentry_event().to_string().contains("secret"));
        assert!(record.render(Level::Error, true).is_some());
        assert!(
            LogRecord {
                level: Level::Info,
                ..record
            }
            .render(Level::Error, false)
            .is_none()
        );
    }
    #[test]
    fn sentry_disabled_invalid_and_safe_configuration() {
        assert!(SentryConfig::parse(None, 1.).unwrap().is_none());
        assert!(SentryConfig::parse(Some("invalid"), 1.).is_err());
        assert!(SentryConfig::parse(Some("https://secret@example.invalid/42"), f64::NAN).is_err());
        let config = SentryConfig::parse(Some("https://abc123@example.invalid/prefix/42"), 1.)
            .unwrap()
            .unwrap();
        assert_eq!(
            config.endpoint.as_str(),
            "https://example.invalid/prefix/api/42/envelope/"
        );
        assert!(!config.endpoint.as_str().contains("abc123"));
    }
}
fn valid_request_id(value: &str) -> bool {
    (value.len() == 32 && value.bytes().all(|b| b.is_ascii_hexdigit()))
        || (value.len() == 36
            && value.bytes().enumerate().all(|(i, b)| {
                if [8, 13, 18, 23].contains(&i) {
                    b == b'-'
                } else {
                    b.is_ascii_hexdigit()
                }
            }))
}
#[cfg(test)]
mod correlation_tests {
    use super::*;
    #[test]
    fn request_uuid_and_query_correlation_are_separate_validated_fields() {
        let record = LogRecord {
            event: LogEvent::RequestCompleted,
            level: Level::Info,
            request_id: Some("01234567-89ab-4cde-8fab-0123456789ab".into()),
            query_id: Some("0123456789ab".into()),
            duration_ms: None,
        };
        assert_eq!(record.json()["query_id"], "0123456789ab");
        assert_eq!(
            record.json()["request_id"],
            "01234567-89ab-4cde-8fab-0123456789ab"
        );
        let bad = LogRecord {
            query_id: Some("secretquery".into()),
            request_id: Some("0123456789ab".into()),
            ..record
        };
        assert_eq!(bad.json()["query_id"], Value::Null);
        assert_eq!(bad.json()["request_id"], Value::Null);
    }
}
#[cfg(test)]
mod console_writer_tests {
    use super::*;
    #[test]
    fn console_writer_preserves_validated_correlation_and_timing() {
        let record = LogRecord {
            event: LogEvent::RequestCompleted,
            level: Level::Info,
            request_id: Some("01234567-89ab-4cde-8fab-0123456789ab".into()),
            query_id: Some("0123456789ab".into()),
            duration_ms: Some(0),
        };
        let mut output = Vec::new();
        record.write(&mut output, Level::Info, false).unwrap();
        let console = String::from_utf8(output).unwrap();
        assert!(console.ends_with('\n'));
        for expected in [
            "request_completed",
            "request_id=\"01234567-89ab-4cde-8fab-0123456789ab\"",
            "query_id=\"0123456789ab\"",
            "duration_ms=0",
        ] {
            assert!(console.contains(expected), "{console}");
        }
        let rejected = LogRecord {
            request_id: Some("private secret header".into()),
            query_id: Some("raw search query".into()),
            duration_ms: None,
            ..record
        };
        let mut output = Vec::new();
        rejected.write(&mut output, Level::Info, false).unwrap();
        let console = String::from_utf8(output).unwrap();
        assert!(!console.contains("private"));
        assert!(!console.contains("raw search"));
        assert!(!console.contains("request_id="));
        assert!(!console.contains("query_id="));
        assert!(!console.contains("duration_ms="));
    }
}

#[cfg(test)]
mod sentry_source_controls {
    use super::*;
    #[test]
    fn pinned_sdk_dsn_controls_preserve_endpoint_auth_and_explicit_native_identity() {
        let capture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../xtask/fixtures/o01/sentry-protocol-source.json"
        ))
        .unwrap();
        let mut completed = 0;
        for case in capture["cases"].as_array().unwrap() {
            if !case["id"].as_str().unwrap().starts_with("sentry.dsn.") {
                continue;
            }
            let dsn = case["input"].as_str().unwrap();
            let actual = SentryConfig::parse(Some(dsn), 1.);
            if case["output"]["error_type"].is_string() {
                assert!(actual.is_err(), "{}", case["id"]);
            } else {
                let actual = actual.unwrap().unwrap();
                assert_eq!(
                    actual.endpoint.as_str(),
                    case["output"]["endpoint"].as_str().unwrap(),
                    "{}",
                    case["id"]
                );
                let expected = case["output"]["authentication"].as_str().unwrap().replace(
                    "sentry.python/2.71.0",
                    concat!("slopsearx.rust/", env!("CARGO_PKG_VERSION")),
                );
                assert_eq!(actual.authentication(), expected, "{}", case["id"]);
                assert!(actual.endpoint.username().is_empty());
                assert!(actual.endpoint.password().is_none());
            }
            completed += 1;
        }
        assert_eq!(completed, 7);
    }
    #[test]
    fn disabled_fallback_selection_and_native_config_bounds() {
        assert!(SentryConfig::resolve(None, None, 1.).unwrap().is_none());
        let environment = "https://envkey@example.invalid/7";
        assert_eq!(
            SentryConfig::resolve(Some(""), Some(environment), 1.)
                .unwrap()
                .unwrap()
                .endpoint
                .path(),
            "/api/7/envelope/"
        );
        assert_eq!(
            SentryConfig::resolve(
                Some("https://explicit@example.invalid/8"),
                Some(environment),
                1.
            )
            .unwrap()
            .unwrap()
            .endpoint
            .path(),
            "/api/8/envelope/"
        );
        assert!(SentryConfig::parse(Some(&"x".repeat(4097)), 1.).is_err());
        assert!(SentryConfig::parse(Some(environment), f64::INFINITY).is_err());
    }
}

#[cfg(test)]
mod original_configuration_rows {
    use super::*;
    #[test]
    fn five_original_explicit_environment_configuration_cases_select_exact_native_profile() {
        let source: ssx_json::Value = ssx_json::from_str(include_str!(
            "../../../xtask/fixtures/o01/source-supplement.json"
        ))
        .unwrap();
        let mut completed = Vec::new();
        for case in source["cases"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|case| case["id"].as_str().unwrap().starts_with("logging.config."))
        {
            let selected = SentryConfig::resolve(
                case["input"]["explicit"].as_str(),
                case["input"]["environment"].as_str(),
                1.,
            )
            .unwrap();
            let calls = case["output"].as_array().unwrap();
            if calls.is_empty() {
                assert!(selected.is_none());
            } else {
                assert_eq!(calls.len(), 1);
                let call = &calls[0];
                assert_eq!(call["send_default_pii"], false);
                assert_eq!(call["traces_sample_rate"].as_f64(), Some(0.));
                assert_eq!(
                    call["_experiments"]["continuous_profiling_auto_start"],
                    false
                );
                let selected = selected.unwrap();
                assert_eq!(Some(selected.sample_rate), call["sample_rate"].as_f64());
                let (endpoint, key) = match call["dsn"].as_str().unwrap() {
                    "https://publickey@monitor.invalid/12" => {
                        ("https://monitor.invalid/api/12/envelope/", "publickey")
                    }
                    "https://explicitkey@monitor.invalid/13" => {
                        ("https://monitor.invalid/api/13/envelope/", "explicitkey")
                    }
                    other => panic!("unexpected immutable captured DSN {other}"),
                };
                assert_eq!(selected.endpoint.as_str(), endpoint);
                assert_eq!(
                    selected.authentication(),
                    format!(
                        "Sentry sentry_key={key}, sentry_version=7, sentry_client=slopsearx.rust/{}",
                        env!("CARGO_PKG_VERSION")
                    )
                );
            }
            completed.push(case["id"].as_str().unwrap());
        }
        assert_eq!(completed.len(), 5);
        for id in completed {
            println!(
                "O01_CASE_COMPLETE:{}",
                ssx_json::json!({"id":id,"consumer":"five_original_explicit_environment_configuration_cases_select_exact_native_profile","class":"source-configuration-api","difference":"native identity; closed error events without traces/profiling; supplied environment value only, no bootstrap activation"})
            );
        }
    }
}
