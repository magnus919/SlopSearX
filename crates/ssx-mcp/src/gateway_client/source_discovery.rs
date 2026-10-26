//! Raw SDK discovery planning, deliberately before HTTPX transport URL parsing.
//! Candidate provenance is explicit; raw observations grant no authority.
use crate::Result;
use reqwest::source_pool::{
    source_raw_lowercase, source_raw_urljoin, source_raw_urlparse, source_raw_urlsplit,
    source_raw_urlunsplit,
};
use rmcp::transport::auth::source_metadata::{
    ModelHttpUrl, SourceEndpoint, SourceMetadataKind, SourceMetadataUrlsRequest,
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiscoveryKind {
    Prm,
    Asm,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CandidateOrigin {
    Advertised,
    Derived,
}
#[derive(Clone)]
pub(crate) struct DiscoveryCandidate {
    pub(crate) text: Arc<str>,
    pub(crate) origin: CandidateOrigin,
}
impl std::fmt::Debug for DiscoveryCandidate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiscoveryCandidate")
            .field("origin", &self.origin)
            .finish_non_exhaustive()
    }
}
fn candidate(text: String, origin: CandidateOrigin) -> DiscoveryCandidate {
    DiscoveryCandidate {
        text: text.into(),
        origin,
    }
}
/// Exact case-sensitive re.search for resource_metadata=(?:"([^"]+)"|([^\s,]+)).
/// No scheme/WWW-Authenticate parser, trimming, or header normalization occurs.
pub(crate) fn resource_metadata_field(raw: &str) -> Option<&str> {
    let needle = "resource_metadata=";
    let mut offset = 0;
    while let Some(index) = raw[offset..].find(needle) {
        let start = offset + index + needle.len();
        let tail = &raw[start..];
        if let Some(quoted) = tail.strip_prefix('"') {
            if let Some(end) = quoted.find('"') {
                if end > 0 {
                    return Some(&quoted[..end]);
                }
            }
        }
        let end = tail
            .find(|c: char| c == ',' || ssx_json::python_whitespace(c))
            .unwrap_or(tail.len());
        if end > 0 {
            return Some(&tail[..end]);
        }
        offset = start;
    }
    None
}
fn raw_origin(raw: &str) -> Result<(String, String)> {
    let parts = source_raw_urlparse(raw)?;
    Ok((
        format!("{}://{}", parts.scheme(), parts.netloc()),
        parts.path().into(),
    ))
}
/// Ordered raw utils.py plan. Model server0 must be supplied from actual model
/// validation; raw base and advertised text are never normalized through it.
pub(crate) fn metadata_urls(
    kind: DiscoveryKind,
    raw_base: &str,
    raw_challenge: Option<&str>,
    selected_server: Option<&ModelHttpUrl>,
) -> Result<Vec<DiscoveryCandidate>> {
    let mut urls = Vec::new();
    match kind {
        DiscoveryKind::Prm => {
            if let Some(text) = raw_challenge.and_then(resource_metadata_field) {
                if !text.is_empty() {
                    urls.push(candidate(text.into(), CandidateOrigin::Advertised));
                }
            }
            let (base, path) = raw_origin(raw_base)?;
            if !path.is_empty() && path != "/" {
                urls.push(candidate(
                    source_raw_urljoin(
                        &base,
                        &format!("/.well-known/oauth-protected-resource{path}"),
                    )?,
                    CandidateOrigin::Derived,
                ));
            }
            urls.push(candidate(
                source_raw_urljoin(&base, "/.well-known/oauth-protected-resource")?,
                CandidateOrigin::Derived,
            ));
        }
        DiscoveryKind::Asm => {
            let Some(server) = selected_server else {
                let (base, _) = raw_origin(raw_base)?;
                return Ok(vec![candidate(
                    format!("{base}/.well-known/oauth-authorization-server"),
                    CandidateOrigin::Derived,
                )]);
            };
            let (base, path) = raw_origin(server.as_str())?;
            if !path.is_empty() && path != "/" {
                let path = path.trim_end_matches('/');
                for reference in [
                    format!("/.well-known/oauth-authorization-server{path}"),
                    format!("/.well-known/openid-configuration{path}"),
                    format!("{path}/.well-known/openid-configuration"),
                ] {
                    urls.push(candidate(
                        source_raw_urljoin(&base, &reference)?,
                        CandidateOrigin::Derived,
                    ));
                }
            } else {
                for reference in [
                    "/.well-known/oauth-authorization-server",
                    "/.well-known/openid-configuration",
                ] {
                    urls.push(candidate(
                        source_raw_urljoin(&base, reference)?,
                        CandidateOrigin::Derived,
                    ));
                }
            }
        }
    }
    Ok(urls)
}
/// Selected server0 is already a model URL. Otherwise raw urlparse produces the
/// origin INPUT and the same genuine model URL validator performs serialization.
pub(crate) fn expected_issuer(
    raw_base: &str,
    selected_server: Option<&ModelHttpUrl>,
) -> Result<Arc<str>> {
    if let Some(server) = selected_server {
        return Ok(server.endpoint().text().clone());
    }
    let (origin, _) = raw_origin(raw_base)?;
    let model = ModelHttpUrl::parse(&origin)?;
    Ok(model.endpoint().text().clone())
}

/// Concrete project implementation for the SDK's typed discovery callback.
/// Each raw candidate retains exact provenance; no parse/sort/dedup is added.
pub(crate) fn source_metadata_urls(
    request: SourceMetadataUrlsRequest,
) -> Result<Vec<SourceEndpoint>> {
    let kind = match request.kind {
        SourceMetadataKind::Prm => DiscoveryKind::Prm,
        SourceMetadataKind::Asm => DiscoveryKind::Asm,
    };
    Ok(metadata_urls(
        kind,
        &request.base,
        request.raw_challenge.as_deref(),
        request.selected_server.as_ref(),
    )?
    .into_iter()
    .map(|row| match row.origin {
        CandidateOrigin::Advertised => SourceEndpoint::advertised(row.text),
        CandidateOrigin::Derived => SourceEndpoint::derived(row.text),
    })
    .collect())
}
/// Concrete project implementation for the SDK's separate expected-issuer callback.
pub(crate) fn source_expected_issuer(
    base: Arc<str>,
    selected_server: Option<ModelHttpUrl>,
) -> Result<Arc<str>> {
    expected_issuer(&base, selected_server.as_ref())
}

/// Raw RFC8707 source helper: lower scheme/entire netloc with pinned Python
/// casing, remove fragment, retain nonempty query through public urlunsplit.
pub(crate) fn source_resource_url(raw_base: &str) -> Result<String> {
    let parts = source_raw_urlsplit(raw_base)?;
    Ok(source_raw_urlunsplit(
        &source_raw_lowercase(parts.scheme()),
        &source_raw_lowercase(parts.netloc()),
        parts.path(),
        parts.query(),
        None,
    ))
}
/// Raw urlparse hierarchy policy; no default-port, userinfo or IDNA equivalence.
pub(crate) fn source_resource_allowed(requested: &str, configured: &str) -> Result<bool> {
    let requested = source_raw_urlparse(requested)?;
    let configured = source_raw_urlparse(configured)?;
    if source_raw_lowercase(requested.scheme()) != source_raw_lowercase(configured.scheme())
        || source_raw_lowercase(requested.netloc()) != source_raw_lowercase(configured.netloc())
    {
        return Ok(false);
    }
    let slash = |p: &str| {
        if p.ends_with('/') {
            p.to_owned()
        } else {
            format!("{p}/")
        }
    };
    Ok(slash(requested.path()).starts_with(&slash(configured.path())))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn texts(rows: Vec<DiscoveryCandidate>) -> Vec<String> {
        rows.into_iter().map(|r| r.text.to_string()).collect()
    }
    #[test]
    fn challenge_search_is_case_sensitive_unanchored_and_python_whitespace_exact() {
        assert_eq!(
            resource_metadata_field("Bearer xresource_metadata=RAW"),
            Some("RAW")
        );
        assert_eq!(resource_metadata_field("Resource_Metadata=wrong"), None);
        assert_eq!(resource_metadata_field("resource_metadata =wrong"), None);
        assert_eq!(
            resource_metadata_field("resource_metadata=first\u{1c}next"),
            Some("first")
        );
        assert_eq!(
            resource_metadata_field("resource_metadata=first\u{2003}next"),
            Some("first")
        );
        assert_eq!(
            resource_metadata_field("resource_metadata=\"two words,raw\""),
            Some("two words,raw")
        );
        assert_eq!(
            resource_metadata_field("resource_metadata=\"\""),
            Some("\"\"")
        );
        assert_eq!(
            resource_metadata_field("resource_metadata=\"unterminated,next"),
            Some("\"unterminated")
        );
        assert_eq!(
            resource_metadata_field("resource_metadata=,resource_metadata=second"),
            Some("second")
        );
    }
    #[test]
    fn prm_preserves_advertised_first_raw_case_userinfo_params_and_duplicates() {
        let raw = " \0HTTP://User:Pass@EXAMPLE.test:80/a;b/c;params?q#fragment";
        let rows = metadata_urls(
            DiscoveryKind::Prm,
            raw,
            Some("Bearer resource_metadata=HTTP://RAW/"),
            None,
        )
        .unwrap();
        assert_eq!(rows[0].origin, CandidateOrigin::Advertised);
        assert_eq!(
            texts(rows),
            vec![
                "HTTP://RAW/",
                "http://User:Pass@EXAMPLE.test:80/.well-known/oauth-protected-resource/a;b/c",
                "http://User:Pass@EXAMPLE.test:80/.well-known/oauth-protected-resource"
            ]
        );
        let rows = metadata_urls(
            DiscoveryKind::Prm,
            "http://EXAMPLE/a",
            Some("resource_metadata=http://EXAMPLE/.well-known/oauth-protected-resource/a"),
            None,
        )
        .unwrap();
        assert_eq!(rows[0].text, rows[1].text);
        assert_ne!(rows[0].origin, rows[1].origin);
    }
    #[test]
    fn asm_has_exact_path_three_root_two_and_raw_legacy_one_candidates() {
        let model = ModelHttpUrl::parse("HTTPS://EXAMPLE.test/issuer///").unwrap();
        assert_eq!(
            texts(metadata_urls(DiscoveryKind::Asm, "unused", None, Some(&model)).unwrap()),
            vec![
                "https://example.test/.well-known/oauth-authorization-server/issuer",
                "https://example.test/.well-known/openid-configuration/issuer",
                "https://example.test/issuer/.well-known/openid-configuration"
            ]
        );
        let model = ModelHttpUrl::parse("https://EXAMPLE.test").unwrap();
        assert_eq!(
            texts(metadata_urls(DiscoveryKind::Asm, "unused", None, Some(&model)).unwrap()),
            vec![
                "https://example.test/.well-known/oauth-authorization-server",
                "https://example.test/.well-known/openid-configuration"
            ]
        );
        assert_eq!(
            texts(
                metadata_urls(
                    DiscoveryKind::Asm,
                    "HTTP://User@EXAMPLE.test:80/a;params",
                    None,
                    None
                )
                .unwrap()
            ),
            vec!["http://User@EXAMPLE.test:80/.well-known/oauth-authorization-server"]
        );
    }
    #[test]
    fn unknown_scheme_raw_plan_and_model_issuer_boundaries_remain_distinct() {
        assert_eq!(
            texts(metadata_urls(DiscoveryKind::Prm, "custom://Host/a;b", None, None).unwrap()),
            vec![
                "/.well-known/oauth-protected-resource/a;b",
                "/.well-known/oauth-protected-resource"
            ]
        );
        assert!(expected_issuer("custom://Host/a", None).is_err());
        assert_eq!(
            &*expected_issuer("HTTP://EXAMPLE.test:80/path", None).unwrap(),
            "http://example.test"
        );
        let model = ModelHttpUrl::parse("https://AUTH.test/issuer?x#f").unwrap();
        assert_eq!(
            &*expected_issuer("ignored", Some(&model)).unwrap(),
            model.as_str()
        );
        assert!(
            Arc::ptr_eq(
                &expected_issuer("ignored", Some(&model)).unwrap(),
                model.endpoint().text()
            ),
            "selected server0 retains the actual model serialization allocation"
        );
    }
}

#[cfg(test)]
mod resource_tests {
    use super::*;
    #[test]
    fn resource_url_uses_raw_split_python_lower_and_public_empty_query_collapse() {
        assert_eq!(
            source_resource_url("HTTP://User:Pass@HOST:80/api;keep?#fragment").unwrap(),
            "http://user:pass@host:80/api;keep"
        );
        assert_eq!(
            source_resource_url("http://BÜCHER.example/a?x#f").unwrap(),
            "http://bücher.example/a?x"
        );
        assert_eq!(source_resource_url("http://ΟΣ/a").unwrap(), "http://ος/a");
    }
    #[test]
    fn resource_policy_preserves_ports_userinfo_unicode_params_and_path_boundaries() {
        assert!(!source_resource_allowed("http://Host:80/api/x", "http://Host/api").unwrap());
        assert!(
            source_resource_allowed("HTTP://USER:PASS@HOST/api/x", "http://user:pass@host/api/")
                .unwrap()
        );
        assert!(!source_resource_allowed("http://host/api", "http://user@host/api").unwrap());
        assert!(source_resource_allowed("http://BÜCHER/api/x", "http://bücher/api").unwrap());
        assert!(
            !source_resource_allowed("http://xn--bcher-kva/api/x", "http://bücher/api").unwrap()
        );
        assert!(
            source_resource_allowed("http://host/api;requested", "http://host/api;configured")
                .unwrap()
        );
        assert!(!source_resource_allowed("http://host/api123", "http://host/api").unwrap());
        assert!(source_resource_allowed("http://host/api/x?q#f", "http://host/api/").unwrap());
    }
}

#[cfg(test)]
mod callback_tests {
    use super::*;
    use rmcp::transport::auth::source_metadata::SourceEndpointOrigin;
    #[test]
    fn typed_callback_preserves_duplicate_raw_text_and_construction_provenance() {
        let rows = source_metadata_urls(SourceMetadataUrlsRequest {
            kind: SourceMetadataKind::Prm,
            base: Arc::from("http://Case/a"),
            raw_challenge: Some(Arc::from(
                "resource_metadata=http://Case/.well-known/oauth-protected-resource/a",
            )),
            selected_server: None,
        })
        .unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].text(), rows[1].text());
        assert_eq!(rows[0].origin(), SourceEndpointOrigin::Advertised);
        assert_eq!(rows[1].origin(), SourceEndpointOrigin::Derived);
    }
}
