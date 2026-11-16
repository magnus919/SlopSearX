//! Raw authority identity for source arXiv redirect policy. Wire URL parsing
//! remains F08-owned; URL normalization equivalence is not claimed here.
use super::{Failure, append};
use crate::adapters::hostname_lower::lower_hostname;
use ssx_owned_budget::{Budget, OwnedString};
use ssx_source_codecs::BoundedTextError;
use std::time::Instant;
async fn check(deadline: Instant) -> Result<(), Failure> {
    tokio::task::yield_now().await;
    if Instant::now() >= deadline {
        Err(Failure::Deadline)
    } else {
        Ok(())
    }
}
async fn find(text: &str, chars: &[u8], deadline: Instant) -> Result<Option<usize>, Failure> {
    for (i, byte) in text.bytes().enumerate() {
        if i.is_multiple_of(1024) {
            check(deadline).await?;
        }
        if chars.contains(&byte) {
            return Ok(Some(i));
        }
    }
    Ok(None)
}
struct Parts<'a> {
    scheme: &'a str,
    host: &'a str,
    port: Option<&'a str>,
    authority_end: usize,
}
async fn parts(text: &str, deadline: Instant) -> Result<Parts<'_>, Failure> {
    let colon = find(text, b":/?#", deadline)
        .await?
        .filter(|i| text.as_bytes()[*i] == b':');
    let Some(colon) = colon else {
        return Err(invalid());
    };
    let scheme = &text[..colon];
    if !text[colon + 1..].starts_with("//") {
        return Ok(Parts {
            scheme,
            host: "",
            port: None,
            authority_end: colon + 1,
        });
    }
    let start = colon + 3;
    let end = find(&text[start..], b"/?#", deadline)
        .await?
        .map_or(text.len(), |i| start + i);
    let mut host_start = start;
    for (i, byte) in text[start..end].bytes().enumerate() {
        if i.is_multiple_of(1024) {
            check(deadline).await?;
        }
        if byte == b'@' {
            host_start = start + i + 1;
        }
    }
    let hostport = &text[host_start..end];
    let (host, port) = if hostport.starts_with('[') {
        let close = find(hostport, b"]", deadline).await?.ok_or_else(invalid)?;
        let tail = &hostport[close + 1..];
        (&hostport[1..close], tail.strip_prefix(':'))
    } else if let Some(colon) = find(hostport, b":", deadline).await? {
        (&hostport[..colon], Some(&hostport[colon + 1..]))
    } else {
        (hostport, None)
    };
    Ok(Parts {
        scheme,
        host,
        port: port.filter(|p| !p.is_empty()),
        authority_end: end,
    })
}
async fn port(
    raw: Option<&str>,
    budget: &Budget,
    deadline: Instant,
) -> Result<Option<u16>, Failure> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let mut number = 0u32;
    let mut valid = true;
    for (i, byte) in raw.bytes().enumerate() {
        if i.is_multiple_of(1024) {
            check(deadline).await?;
        }
        if !byte.is_ascii_digit() {
            valid = false;
        }
        number = number
            .saturating_mul(10)
            .saturating_add(u32::from(byte.saturating_sub(b'0')));
    }
    if valid && number <= u16::MAX as u32 {
        return Ok(Some(number as u16));
    }
    let mut message = OwnedString::new(budget).map_err(|_| Failure::Resource)?;
    append(
        &mut message,
        "Invalid URL in location header: Invalid port: '",
        deadline,
    )
    .await?;
    append(&mut message, raw, deadline).await?;
    append(&mut message, "'.", deadline).await?;
    Err(Failure::Source(message))
}
fn lower_error(error: BoundedTextError) -> Failure {
    match error {
        BoundedTextError::Deadline => Failure::Deadline,
        _ => Failure::Resource,
    }
}
/// Preserve explicit default ports, raw Unicode/IPv6 spelling, and source
/// diagnostics before the wire URL library canonicalizes those distinctions.
pub(super) async fn target(
    base: &str,
    location: &str,
    budget: &Budget,
    deadline: Instant,
) -> Result<url::Url, Failure> {
    let base_parts = parts(base, deadline).await?;
    let first = find(location, b":/?#", deadline).await?;
    let mut absolute = first.is_some_and(|i| location.as_bytes()[i] == b':');
    if let Some(colon) = first.filter(|_| absolute) {
        for (i, byte) in location[..colon].bytes().enumerate() {
            if i.is_multiple_of(1024) {
                check(deadline).await?;
            }
            if !(if i == 0 {
                byte.is_ascii_alphabetic()
            } else {
                byte.is_ascii_alphanumeric() || b"+-.".contains(&byte)
            }) {
                absolute = false;
            }
        }
        absolute &= colon != 0;
    }
    let mut raw = OwnedString::new(budget).map_err(|_| Failure::Resource)?;
    if absolute {
        append(&mut raw, location, deadline).await?;
    } else if location.starts_with("//") {
        append(&mut raw, base_parts.scheme, deadline).await?;
        append(&mut raw, ":", deadline).await?;
        append(&mut raw, location, deadline).await?;
    } else {
        let joined = url::Url::parse(base)
            .and_then(|url| url.join(location))
            .map_err(|_| invalid())?;
        // Relative references inherit the configured raw authority, including
        // an explicit default port, while the wire library resolves its path.
        let joined_parts = parts(joined.as_str(), deadline).await?;
        append(&mut raw, &base[..base_parts.authority_end], deadline).await?;
        append(
            &mut raw,
            &joined.as_str()[joined_parts.authority_end..],
            deadline,
        )
        .await?;
    }
    let selected = parts(raw.as_str(), deadline).await?;
    // HTTPX validates Location before source adapter policy checks.
    let selected_port = port(selected.port, budget, deadline).await?;
    let configured_port = port(base_parts.port, budget, deadline).await?;
    let selected_host = lower_hostname(selected.host, budget, deadline)
        .await
        .map_err(lower_error)?;
    let configured_host = lower_hostname(base_parts.host, budget, deadline)
        .await
        .map_err(lower_error)?;
    if !selected.scheme.eq_ignore_ascii_case("https")
        || selected_host.as_str() != configured_host.as_str()
        || selected_port != configured_port
    {
        let mut message = OwnedString::new(budget).map_err(|_| Failure::Resource)?;
        append(
            &mut message,
            "redirect rejected: expected HTTPS on the configured arXiv host, got ",
            deadline,
        )
        .await?;
        append(&mut message, raw.as_str(), deadline).await?;
        return Err(Failure::Source(message));
    }
    let wire = url::Url::parse(raw.as_str()).map_err(|_| invalid())?;
    let configured = url::Url::parse(base).map_err(|_| invalid())?;
    // A different path/authority interpretation must never dispatch elsewhere.
    // This fail-closed transport guard does not claim urllib/url parity.
    if wire.scheme() != "https" || wire.host_str() != configured.host_str() {
        return Err(invalid());
    }
    Ok(wire)
}
fn invalid() -> Failure {
    Failure::Transport(crate::http::Error::InvalidResponse)
}
