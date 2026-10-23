//! Preserve Python's OS string domain until source proxy precedence and bypass.
use super::{
    platform_api::{self as api, RawMap},
    source_text as st,
};
use std::collections::BTreeMap;
pub(super) fn resolve(
    environment: &[(st::Text, st::Text)],
    discovery: impl FnOnce() -> crate::Result<BTreeMap<String, String>>,
) -> crate::Result<BTreeMap<String, String>> {
    let suffix = st::text("_proxy");
    let mut map = RawMap::new();
    for (key, value) in environment {
        let lowered = st::lower(key);
        if let Some(name) = lowered.strip_suffix(&suffix[..]) {
            if !value.is_empty() {
                map.insert(name.to_vec(), value.clone());
            }
        }
    }
    if environment
        .iter()
        .any(|(k, _)| *k == st::text("REQUEST_METHOD"))
    {
        map.remove(&st::text("http"));
    }
    for (key, value) in environment {
        if key.ends_with(&suffix) {
            let name = st::lower(&key[..key.len() - suffix.len()]);
            if value.is_empty() {
                map.remove(&name);
            } else {
                map.insert(name, value.clone());
            }
        }
    }
    if map.is_empty() {
        discovery()
    } else {
        api::httpx_map(map)
    }
}
fn resolve_os(
    environment: Vec<(st::Text, st::Text)>,
    windows: bool,
    discovery: impl FnOnce() -> crate::Result<BTreeMap<String, String>>,
) -> crate::Result<BTreeMap<String, String>> {
    if !windows {
        return resolve(&environment, discovery);
    }
    let mut normalized: Vec<(st::Text, st::Text)> = Vec::new();
    for (key, value) in environment {
        let key = st::upper(&key);
        if let Some(entry) = normalized.iter_mut().find(|(k, _)| *k == key) {
            entry.1 = value;
        } else {
            normalized.push((key, value));
        }
    }
    resolve(&normalized, discovery)
}
fn os_text(value: &std::ffi::OsStr) -> st::Text {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        st::surrogateescape(value.as_bytes())
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        st::utf16(&value.encode_wide().collect::<Vec<_>>())
    }
    #[cfg(not(any(unix, windows)))]
    {
        st::text(&value.to_string_lossy())
    }
}
pub(super) fn current(
    discovery: impl FnOnce() -> crate::Result<BTreeMap<String, String>>,
) -> crate::Result<BTreeMap<String, String>> {
    let environment = std::env::vars_os()
        .map(|(k, v)| (os_text(&k), os_text(&v)))
        .collect::<Vec<_>>();
    resolve_os(environment, cfg!(windows), discovery)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opaque_environment_key_suppresses_os_without_lossy_collision() {
        let mut key = vec![0xdcff];
        key.extend(st::text("_proxy"));
        let map = resolve(&[(key, vec![0xd800])], || {
            panic!("opaque source key must suppress OS fallback")
        })
        .unwrap();
        assert!(!map.is_empty());
        assert!(!map.contains_key("http"));
    }
    #[test]
    fn raw_environment_lowercase_removal_and_cgi_are_preserved() {
        let key = [vec![0xdcff], st::text("_proxy")].concat();
        let map = resolve(&[(key.clone(), st::text("x")), (key, vec![])], || {
            Ok(BTreeMap::from([("http".into(), "os".into())]))
        })
        .unwrap();
        assert_eq!(map["http"], "os");
        let map = resolve(
            &[
                (st::text("HTTP_PROXY"), vec![0xd800]),
                (st::text("REQUEST_METHOD"), st::text("GET")),
            ],
            || Ok(BTreeMap::new()),
        )
        .unwrap();
        assert!(map.is_empty());
    }
    #[test]
    fn ignored_surrogate_values_and_global_no_proxy_do_not_raise() {
        let map = resolve(&[(st::text("FTP_PROXY"), vec![0xd800])], || {
            panic!("source map is not empty")
        })
        .unwrap();
        assert!(map.contains_key("ftp"));
        let map = resolve(
            &[
                (st::text("HTTP_PROXY"), vec![0xd800]),
                (st::text("NO_PROXY"), st::text("ignored, *")),
            ],
            || panic!("source map is not empty"),
        )
        .unwrap();
        assert_eq!(map["no"], "*");
        assert!(!map.contains_key("http"));
    }
    #[test]
    fn consumed_surrogate_proxy_is_rejected_only_after_source_precedence() {
        assert!(
            resolve(&[(st::text("HTTP_PROXY"), vec![0xd800])], || panic!(
                "nonempty source map"
            ))
            .is_err()
        );
        let map = resolve(
            &[
                (st::text("HTTP_PROXY"), vec![0xd800]),
                (st::text("http_proxy"), st::text("http://local")),
            ],
            || panic!("nonempty source map"),
        )
        .unwrap();
        assert_eq!(map["http"], "http://local");
    }
    #[test]
    fn windows_os_environ_uppercase_precedes_proxy_precedence() {
        let env = vec![
            (st::text("http_proxy"), st::text("x")),
            (st::text("REQUEST_METHOD"), st::text("GET")),
        ];
        let map = resolve_os(env, true, || {
            Ok(BTreeMap::from([("http".into(), "os".into())]))
        })
        .unwrap();
        assert_eq!(map["http"], "os");
        let env = vec![
            (st::text("ß_PROXY"), st::text("x")),
            (st::text("SS_PROXY"), vec![]),
        ];
        assert!(resolve_os(env, true, || Ok(BTreeMap::new()))
            .unwrap()
            .is_empty());
    }
}
