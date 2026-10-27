//! Gateway-only client settings. Never reads server engine configuration.
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, PartialEq, Eq)]
pub struct Settings {
    pub remote_url: String,
    pub token: Option<String>,
    pub oauth: bool,
    // Keep argparse's integer contract until the loopback bind boundary.
    pub callback_port: String,
    pub no_browser: bool,
    pub token_file: Option<PathBuf>,
}
impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settings")
            .field("remote_url", &"<redacted>")
            .field("token", &self.token.as_ref().map(|_| "<redacted>"))
            .field("oauth", &self.oauth)
            .field("callback_port", &self.callback_port)
            .field("no_browser", &self.no_browser)
            .field(
                "token_file",
                &self.token_file.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsError {
    MissingRemote,
    MissingValue,
    InvalidCallbackPort,
    ConflictingAuthentication,
    UnknownArgument,
}
/// Python argparse invokes int before callback binding; preserve arbitrary
/// decimal magnitude here and validate socket range only at the bind boundary.
fn callback_integer(value: &str) -> Result<String, SettingsError> {
    let normalized = ssx_domain::storage::normalize_python_number_text(value)
        .map_err(|_| SettingsError::InvalidCallbackPort)?;
    let (negative, digits) = normalized.strip_prefix('-').map_or_else(
        || (false, normalized.strip_prefix('+').unwrap_or(&normalized)),
        |digits| (true, digits),
    );
    if digits.is_empty() || digits.len() > 4300 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(SettingsError::InvalidCallbackPort);
    }
    let significant = digits.trim_start_matches('0');
    if significant.is_empty() {
        return Ok("0".into());
    }
    Ok(if negative {
        format!("-{significant}")
    } else {
        significant.into()
    })
}
fn flag(value: Option<&String>) -> bool {
    value.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes"
        )
    })
}
impl Settings {
    /// Environment is supplied by the executable; fixtures never mutate process env.
    pub fn parse(args: &[String], env: &BTreeMap<String, String>) -> Result<Self, SettingsError> {
        let mut remote = env.get("MCP_REMOTE_URL").cloned().unwrap_or_default();
        let mut token = env.get("MCP_REMOTE_TOKEN").cloned().unwrap_or_default();
        let mut oauth = flag(env.get("MCP_REMOTE_OAUTH"));
        let mut callback_port = callback_integer(
            env.get("MCP_REMOTE_OAUTH_CALLBACK_PORT")
                .map(String::as_str)
                .unwrap_or("8765"),
        )?;
        let mut no_browser = flag(env.get("MCP_REMOTE_OAUTH_NO_BROWSER"));
        let mut token_file = env
            .get("MCP_REMOTE_TOKEN_FILE")
            .cloned()
            .unwrap_or_default();
        let mut i = 0;
        while i < args.len() {
            let (name, inline) = args[i]
                .split_once('=')
                .map_or((args[i].as_str(), None), |(k, v)| (k, Some(v)));
            match name {
                "--oauth" | "--oauth-no-browser" if inline.is_none() => {
                    if name == "--oauth" {
                        oauth = true;
                    } else {
                        no_browser = true;
                    }
                }
                "--remote"
                | "--token"
                | "--oauth-callback-port"
                | "--oauth-token-file"
                | "--transport" => {
                    let value = if let Some(value) = inline {
                        value.to_owned()
                    } else {
                        i += 1;
                        args.get(i)
                            .filter(|v| !v.starts_with("--"))
                            .ok_or(SettingsError::MissingValue)?
                            .clone()
                    };
                    match name {
                        "--remote" => remote = value,
                        "--token" => token = value,
                        "--oauth-callback-port" => callback_port = callback_integer(&value)?,
                        "--oauth-token-file" => token_file = value,
                        // Remote mode always uses stdio, as in the original CLI.
                        "--transport" => {}
                        _ => unreachable!(),
                    }
                }
                _ => return Err(SettingsError::UnknownArgument),
            }
            i += 1;
        }
        if remote.is_empty() {
            return Err(SettingsError::MissingRemote);
        }
        let token = (!token.trim().is_empty()).then(|| token.trim().to_owned());
        if oauth && token.is_some() {
            return Err(SettingsError::ConflictingAuthentication);
        }
        Ok(Self {
            remote_url: remote.trim().to_owned(),
            token,
            oauth,
            callback_port,
            no_browser,
            token_file: (!token_file.is_empty()).then(|| PathBuf::from(token_file)),
        })
    }
}
pub fn default_token_path(server_url: &str, xdg: Option<&str>, home: &std::path::Path) -> PathBuf {
    let hash: String = Sha256::digest(server_url.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let base = xdg
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    base.join("slopsearx/oauth")
        .join(format!("{}.json", &hash[..16]))
}
/// Explicit storage and XDG paths do not require an unrelated home variable.
/// Windows mirrors native home selection: USERPROFILE, then HOMEDRIVE/HOMEPATH;
/// HOME is a Unix input and cannot override a configured Windows profile.
pub(super) fn resolved_token_path(
    settings: &Settings,
    env: &BTreeMap<String, std::ffi::OsString>,
) -> Option<PathBuf> {
    if let Some(path) = &settings.token_file {
        return Some(path.clone());
    }
    if let Some(xdg) = env.get("XDG_CONFIG_HOME").and_then(|s| s.to_str()) {
        return Some(default_token_path(
            &settings.remote_url,
            Some(xdg),
            std::path::Path::new(""),
        ));
    }
    #[cfg(windows)]
    let home = windows_home(env)?;
    #[cfg(not(windows))]
    let home = PathBuf::from(env.get("HOME")?);
    Some(default_token_path(&settings.remote_url, None, &home))
}
#[cfg(any(windows, test))]
fn windows_home(env: &BTreeMap<String, std::ffi::OsString>) -> Option<PathBuf> {
    if let Some(profile) = env.get("USERPROFILE").filter(|s| !s.is_empty()) {
        return Some(profile.into());
    }
    let mut drive = env.get("HOMEDRIVE").filter(|s| !s.is_empty())?.clone();
    drive.push(env.get("HOMEPATH").filter(|s| !s.is_empty())?);
    Some(drive.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| (*s).into()).collect()
    }
    #[test]
    fn cli_overrides_environment_without_reading_engine_keys() {
        let env = [
            ("MCP_REMOTE_URL".into(), "http://env/mcp".into()),
            ("MCP_REMOTE_TOKEN".into(), "env-secret".into()),
            ("ENGINE_BRAVE_API_KEY".into(), "engine-secret".into()),
        ]
        .into();
        let parsed = Settings::parse(
            &args(&["--remote", " http://cli/mcp ", "--token= client-secret "]),
            &env,
        )
        .unwrap();
        assert_eq!(parsed.remote_url, "http://cli/mcp");
        assert_eq!(parsed.token.as_deref(), Some("client-secret"));
        assert!(!format!("{parsed:?}").contains("secret"));
        assert_eq!(parsed.callback_port, "8765");
    }
    #[test]
    fn boolean_environment_and_mutual_exclusion() {
        for raw in ["true", " 1 ", "YeS"] {
            let env = [
                ("MCP_REMOTE_URL".into(), "remote".into()),
                ("MCP_REMOTE_OAUTH".into(), raw.into()),
            ]
            .into();
            assert!(Settings::parse(&[], &env).unwrap().oauth);
            assert_eq!(
                Settings::parse(&args(&["--token", "synthetic"]), &env),
                Err(SettingsError::ConflictingAuthentication)
            );
        }
        for raw in ["false", "on", "0", ""] {
            let env = [
                ("MCP_REMOTE_URL".into(), "remote".into()),
                ("MCP_REMOTE_OAUTH".into(), raw.into()),
            ]
            .into();
            assert!(!Settings::parse(&[], &env).unwrap().oauth);
        }
    }
    #[test]
    fn blank_token_override_and_raw_token_path() {
        let env = [
            ("MCP_REMOTE_URL".into(), "remote".into()),
            ("MCP_REMOTE_TOKEN".into(), "secret".into()),
            ("MCP_REMOTE_OAUTH".into(), "true".into()),
        ]
        .into();
        let parsed = Settings::parse(
            &args(&[
                "--token= ",
                "--oauth-token-file= file with spaces ",
                "--transport=http",
            ]),
            &env,
        )
        .unwrap();
        assert!(parsed.token.is_none());
        assert_eq!(parsed.token_file, Some(PathBuf::from(" file with spaces ")));
    }
    #[test]
    fn source_integer_forms_preserve_unicode_separators_magnitude_and_eager_env_failure() {
        for (raw, expected) in [
            ("8765", "8765"),
            ("  +8765 ", "8765"),
            ("-1", "-1"),
            ("0", "0"),
            ("65_535", "65535"),
            ("٨٧٦٥", "8765"),
            ("９９９９", "9999"),
            ("9223372036854775808", "9223372036854775808"),
            ("-000", "0"),
        ] {
            assert_eq!(callback_integer(raw).unwrap(), expected);
        }
        for raw in ["", "1.0", "_1", "1_", "1__0"] {
            assert_eq!(
                callback_integer(raw),
                Err(SettingsError::InvalidCallbackPort)
            );
        }
        assert!(callback_integer(&"1".repeat(4300)).is_ok());
        assert_eq!(
            callback_integer(&"0".repeat(4301)),
            Err(SettingsError::InvalidCallbackPort)
        );
        let env = [
            ("MCP_REMOTE_URL".into(), "remote".into()),
            ("MCP_REMOTE_OAUTH_CALLBACK_PORT".into(), "bad".into()),
        ]
        .into();
        assert_eq!(
            Settings::parse(&args(&["--oauth-callback-port", "8765"]), &env),
            Err(SettingsError::InvalidCallbackPort)
        );
    }
    #[test]
    fn default_path_hashes_exact_url_and_preserves_empty_xdg() {
        assert_eq!(
            default_token_path("abc", None, std::path::Path::new("/home/u")),
            PathBuf::from("/home/u/.config/slopsearx/oauth/ba7816bf8f01cfea.json")
        );
        assert_eq!(
            default_token_path("abc", Some(""), std::path::Path::new("/unused")),
            PathBuf::from("slopsearx/oauth/ba7816bf8f01cfea.json")
        );
        assert_ne!(
            default_token_path("abc", None, std::path::Path::new("/h")),
            default_token_path("abc ", None, std::path::Path::new("/h"))
        );
    }
}

#[cfg(test)]
mod platform_path_tests {
    use super::*;
    fn env(rows: &[(&str, &str)]) -> BTreeMap<String, std::ffi::OsString> {
        rows.iter()
            .map(|(k, v)| (k.to_string(), (*v).into()))
            .collect()
    }
    #[test]
    fn token_path_explicit_and_xdg_inputs_do_not_require_home() {
        let mut settings = Settings::parse(
            &[
                "--remote".into(),
                "https://fixture.invalid/mcp".into(),
                "--oauth".into(),
            ],
            &BTreeMap::new(),
        )
        .unwrap();
        settings.token_file = Some("chosen.json".into());
        assert_eq!(
            resolved_token_path(&settings, &env(&[])),
            Some("chosen.json".into())
        );
        settings.token_file = None;
        assert!(
            resolved_token_path(&settings, &env(&[("XDG_CONFIG_HOME", "")]))
                .unwrap()
                .starts_with("slopsearx/oauth")
        );
        assert!(
            resolved_token_path(
                &settings,
                &env(&[("XDG_CONFIG_HOME", "chosen"), ("HOME", "ignored")])
            )
            .unwrap()
            .starts_with("chosen/slopsearx/oauth")
        );
        assert!(resolved_token_path(&settings, &env(&[])).is_none());
    }
    #[test]
    fn windows_native_home_precedence_empty_missing_and_conflicting_inputs() {
        assert_eq!(
            windows_home(&env(&[
                ("HOME", "ignored"),
                ("USERPROFILE", r"C:\Users\profile"),
                ("HOMEDRIVE", "D:"),
                ("HOMEPATH", r"\fallback")
            ])),
            Some(r"C:\Users\profile".into())
        );
        assert_eq!(
            windows_home(&env(&[
                ("USERPROFILE", ""),
                ("HOMEDRIVE", "D:"),
                ("HOMEPATH", r"\fallback")
            ])),
            Some(r"D:\fallback".into())
        );
        for rows in [
            vec![],
            vec![("HOME", "ignored")],
            vec![("USERPROFILE", "")],
            vec![("HOMEDRIVE", "C:")],
            vec![("HOMEDRIVE", ""), ("HOMEPATH", r"\fallback")],
        ] {
            assert!(windows_home(&env(&rows)).is_none());
        }
    }
}
