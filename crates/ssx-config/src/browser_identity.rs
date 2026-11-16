//! T09 operator-only browser configuration. No ambient file/env access in parsing.
use crate::Secret;
use serde::Deserialize;
use ssx_domain::browser_identity::{
    BROWSER_ACTIONS, BrowserError, BrowserMembership, BrowserPrincipal,
};
use std::collections::{BTreeMap, BTreeSet};
use url::Url;
#[derive(Debug, Clone)]
pub struct BrowserConfig {
    pub external_origin: String,
    pub issuer: String,
    pub client_id: String,
    pub client_secret: Secret,
    pub session_key: Secret,
    pub previous_session_key: Secret,
    pub previous_key_drain_seconds: u64,
    pub membership_path: String,
    pub actions: BTreeSet<String>,
    pub mutations: BTreeSet<String>,
}
impl BrowserConfig {
    pub fn load(env: &BTreeMap<String, String>) -> Result<Option<Self>, BrowserError> {
        let value = |key: &str| env.get(key).map_or("", String::as_str);
        let enabled = value("SLOPSEARX_WORKFLOW_PORTAL_ENABLED")
            .trim_matches(|c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c))
            .to_lowercase();
        if !["1", "true", "yes"].contains(&enabled.as_str()) {
            return Ok(None);
        }
        let origin = value("SLOPSEARX_PORTAL_EXTERNAL_ORIGIN").trim_end_matches('/');
        let u = Url::parse(origin).map_err(|_| BrowserError::InvalidInput)?;
        let issuer = value("SLOPSEARX_PORTAL_OIDC_ISSUER").trim_end_matches('/');
        let i = Url::parse(issuer).map_err(|_| BrowserError::InvalidInput)?;
        let valid = |u: &Url| {
            u.scheme() == "https"
                && u.host_str().is_some()
                && u.username().is_empty()
                && u.password().is_none()
                && u.query().is_none()
                && u.fragment().is_none()
        };
        if !valid(&u)
            || u.path() != "/"
            || !valid(&i)
            || !value("SLOPSEARX_PORTAL_TRUSTED_PROXIES").trim().is_empty()
        {
            return Err(BrowserError::InvalidInput);
        }
        let client_id = value("SLOPSEARX_PORTAL_OIDC_CLIENT_ID");
        let client_secret = value("SLOPSEARX_PORTAL_OIDC_CLIENT_SECRET");
        let key = value("SLOPSEARX_PORTAL_SESSION_KEY");
        let previous = value("SLOPSEARX_PORTAL_PREVIOUS_SESSION_KEY");
        let path = value("SLOPSEARX_PORTAL_MEMBERSHIPS_FILE");
        if client_id.is_empty()
            || client_id.len() > 512
            || client_secret.is_empty()
            || client_secret.len() > 4096
            || !(32..=4096).contains(&key.len())
            || (!previous.is_empty() && !(32..=4096).contains(&previous.len()))
            || path.is_empty()
            || path.len() > 4096
        {
            return Err(BrowserError::InvalidInput);
        }
        let drain = if previous.is_empty() {
            0
        } else {
            let raw = env
                .get("SLOPSEARX_PORTAL_PREVIOUS_KEY_DRAIN_SECONDS")
                .map_or("600", String::as_str);
            let normalized = ssx_domain::storage::normalize_python_number_text(raw)
                .map_err(|_| BrowserError::InvalidInput)?;
            let n = normalized
                .parse::<u64>()
                .map_err(|_| BrowserError::InvalidInput)?;
            if !(60..=600).contains(&n) {
                return Err(BrowserError::InvalidInput);
            }
            n
        };
        let mut actions: BTreeSet<String> = value("SLOPSEARX_WORKFLOW_PORTAL_ACTIONS")
            .split(',')
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(String::from)
            .collect();
        actions.insert("workflow.read".into());
        if actions
            .iter()
            .any(|a| !BROWSER_ACTIONS.contains(&a.as_str()))
        {
            return Err(BrowserError::InvalidInput);
        }
        let mutations = actions
            .iter()
            .filter(|a| a.as_str() != "workflow.read")
            .cloned()
            .collect();
        Ok(Some(Self {
            external_origin: origin.into(),
            issuer: issuer.into(),
            client_id: client_id.into(),
            client_secret: Secret::new(Some(client_secret.into())),
            session_key: Secret::new(Some(key.into())),
            previous_session_key: Secret::new((!previous.is_empty()).then(|| previous.into())),
            previous_key_drain_seconds: drain,
            membership_path: path.into(),
            actions,
            mutations,
        }))
    }
    /// Membership directory is bounded process configuration, not Valkey authority.
    pub fn directory(&self, bytes: &[u8]) -> Result<Vec<BrowserPrincipal>, BrowserError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Root {
            principals: Vec<P>,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct P {
            principal_id: String,
            issuer: String,
            subject: String,
            memberships: Vec<M>,
            #[serde(default = "revision")]
            revision: u64,
            #[serde(default = "enabled")]
            enabled: bool,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct M {
            tenant_id: String,
            display_name: String,
            grants: Vec<String>,
            #[serde(default = "revision")]
            revision: u64,
            #[serde(default = "enabled")]
            enabled: bool,
        }
        fn revision() -> u64 {
            1
        }
        fn enabled() -> bool {
            true
        }
        if bytes.len() > 1_048_576 {
            return Err(BrowserError::InvalidInput);
        }
        let root: Root = serde_json::from_slice(bytes).map_err(|_| BrowserError::InvalidInput)?;
        if root.principals.is_empty() || root.principals.len() > 1024 {
            return Err(BrowserError::InvalidInput);
        }
        let mut ids = BTreeSet::new();
        let mut subjects = BTreeSet::new();
        let mut output = Vec::new();
        for p in root.principals {
            if p.issuer.trim_end_matches('/') != self.issuer
                || !ids.insert(p.principal_id.clone())
                || !subjects.insert(p.subject.clone())
            {
                return Err(BrowserError::InvalidInput);
            }
            let mut memberships = Vec::new();
            for m in p.memberships {
                let grants: BTreeSet<_> = m.grants.iter().cloned().collect();
                if grants.is_empty() || grants.len() != m.grants.len() {
                    return Err(BrowserError::InvalidInput);
                }
                memberships.push(BrowserMembership {
                    tenant_id: m.tenant_id,
                    display_name: m.display_name,
                    grants,
                    revision: m.revision,
                    enabled: m.enabled,
                })
            }
            let p = BrowserPrincipal {
                principal_id: p.principal_id,
                issuer: self.issuer.clone(),
                subject: p.subject,
                memberships,
                revision: p.revision,
                enabled: p.enabled,
            };
            p.validate()?;
            output.push(p)
        }
        Ok(output)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn env() -> BTreeMap<String, String> {
        [
            ("SLOPSEARX_WORKFLOW_PORTAL_ENABLED", "true"),
            ("SLOPSEARX_PORTAL_EXTERNAL_ORIGIN", "https://portal.example"),
            ("SLOPSEARX_PORTAL_OIDC_ISSUER", "https://id.example"),
            ("SLOPSEARX_PORTAL_OIDC_CLIENT_ID", "client"),
            (
                "SLOPSEARX_PORTAL_OIDC_CLIENT_SECRET",
                "synthetic-client-secret",
            ),
            (
                "SLOPSEARX_PORTAL_SESSION_KEY",
                "synthetic-session-key-01234567890123456789",
            ),
            (
                "SLOPSEARX_PORTAL_MEMBERSHIPS_FILE",
                "/operator/members.json",
            ),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect()
    }
    #[test]
    fn operator_settings_disabled_secrets_origin_proxy_and_previous_key_bounds() {
        assert!(BrowserConfig::load(&BTreeMap::new()).unwrap().is_none());
        let good = BrowserConfig::load(&env()).unwrap().unwrap();
        let debug = format!("{good:?}");
        assert!(!debug.contains("synthetic-client-secret"));
        assert!(!debug.contains("synthetic-session-key"));
        for (k, v) in [
            ("SLOPSEARX_PORTAL_EXTERNAL_ORIGIN", "http://portal.example"),
            (
                "SLOPSEARX_PORTAL_EXTERNAL_ORIGIN",
                "https://portal.example/path",
            ),
            ("SLOPSEARX_PORTAL_TRUSTED_PROXIES", "127.0.0.1"),
            ("SLOPSEARX_PORTAL_SESSION_KEY", "short"),
            ("SLOPSEARX_WORKFLOW_PORTAL_ACTIONS", "operator.root"),
        ] {
            let mut e = env();
            e.insert(k.into(), v.into());
            assert!(BrowserConfig::load(&e).is_err(), "{k}");
        }
        let mut e = env();
        e.insert(
            "SLOPSEARX_PORTAL_PREVIOUS_SESSION_KEY".into(),
            "p".repeat(32),
        );
        e.insert(
            "SLOPSEARX_PORTAL_PREVIOUS_KEY_DRAIN_SECONDS".into(),
            "601".into(),
        );
        assert!(BrowserConfig::load(&e).is_err());
    }
    #[test]
    fn previous_key_drain_matches_source_integer_grammar() {
        let corpus: serde_json::Value =
            serde_json::from_str(include_str!("../../xtask/fixtures/t09/source.json")).unwrap();
        let cases = corpus["previous_key_drain"].as_array().unwrap();
        assert_eq!(cases.len(), 15);
        for case in cases {
            let raw = case["raw"].as_str().unwrap();
            let expected = case["source"].as_u64();
            let mut e = env();
            e.insert(
                "SLOPSEARX_PORTAL_PREVIOUS_SESSION_KEY".into(),
                "p".repeat(32),
            );
            e.insert(
                "SLOPSEARX_PORTAL_PREVIOUS_KEY_DRAIN_SECONDS".into(),
                raw.into(),
            );
            let actual = BrowserConfig::load(&e).map(|v| v.unwrap().previous_key_drain_seconds);
            assert_eq!(actual.ok(), expected, "{raw:?}");
        }
    }
    #[test]
    fn directory_retains_defaults_and_rejects_duplicate_unstable_or_coercible_authority() {
        let c = BrowserConfig::load(&env()).unwrap().unwrap();
        let good = json!({"principals":[{"principal_id":"p","issuer":"https://id.example/","subject":"s","memberships":[{"tenant_id":"a","display_name":"A","grants":["workflow.read"]}]}]});
        let bytes = serde_json::to_vec(&good).unwrap();
        let parsed = c.directory(&bytes).unwrap();
        assert_eq!(parsed[0].revision, 1);
        assert!(parsed[0].memberships[0].enabled);
        for value in [json!(true), json!("1"), json!(0), json!(1.5)] {
            let mut bad = good.clone();
            bad["principals"][0]["revision"] = value;
            assert!(c.directory(&serde_json::to_vec(&bad).unwrap()).is_err());
        }
        let duplicate = String::from_utf8(bytes.clone()).unwrap().replace(
            "\"principal_id\":\"p\"",
            "\"principal_id\":\"p\",\"principal_id\":\"q\"",
        );
        assert!(c.directory(duplicate.as_bytes()).is_err());
        let mut bad = good.clone();
        bad["principals"][0]["memberships"][0]["grants"] =
            json!(["workflow.read", "workflow.read"]);
        assert!(c.directory(&serde_json::to_vec(&bad).unwrap()).is_err());
        assert!(c.directory(&vec![b' '; 1048577]).is_err());
    }
    #[test]
    fn exact_source_directory_corpus_with_named_unknown_grant_rejection() {
        let corpus: serde_json::Value =
            serde_json::from_str(include_str!("../../xtask/fixtures/t09/source.json")).unwrap();
        let config = BrowserConfig::load(&env()).unwrap().unwrap();
        let cases = corpus["directories"].as_array().unwrap();
        assert_eq!(cases.len(), 6);
        let mut divergence = 0;
        for c in cases {
            let actual = config.directory(&serde_json::to_vec(&c["input"]).unwrap());
            let expected = if c["id"] == "unknown_grant" {
                assert_eq!(c["source"]["accepted"], true);
                divergence += 1;
                false
            } else {
                c["source"]["accepted"].as_bool().unwrap()
            };
            assert_eq!(actual.is_ok(), expected, "{}", c["id"]);
            if expected {
                assert_eq!(
                    serde_json::json!(actual.unwrap()[0].memberships[0].grants),
                    c["source"]["grants"]
                );
            }
        }
        assert_eq!(divergence, 1);
    }
}
