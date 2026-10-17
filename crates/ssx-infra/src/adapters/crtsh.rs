//! CRT.sh preserves challenge/HTML/JSON/row distinctions without body diagnostics.
use super::infrastructure_security::{self as s, *};
use super::{
    http_charset, http_headers,
    source_json::{self, Value},
    source_payload::ProducedBatch,
};
use ssx_owned_budget::Budget;
use std::time::Instant;
s::adapter!(CrtShAdapter, Kind::CrtSh);
const MARKERS: [(&str, &str); 9] = [
    ("access_denied", "access denied"),
    ("captcha", "captcha"),
    ("cf_challenge", "cf-chl-"),
    ("challenge", "challenge"),
    ("cloudflare", "cloudflare"),
    ("hcaptcha", "hcaptcha"),
    ("recaptcha", "recaptcha"),
    ("unusual_traffic", "unusual traffic"),
    ("verify_human", "verify you are human"),
];
fn byte_whitespace(byte: u8) -> bool {
    matches!(byte, b'\t' | b'\n' | b'\x0b' | b'\x0c' | b'\r' | b' ')
}
fn html_prefix(body: &[u8]) -> bool {
    let prefix = &body[..body.len().min(256)];
    let at = prefix
        .iter()
        .position(|b| !byte_whitespace(*b))
        .unwrap_or(prefix.len());
    let prefix = &prefix[at..];
    if prefix.len() >= 5 && prefix[..5].eq_ignore_ascii_case(b"<html") {
        return prefix
            .get(5)
            .is_none_or(|b| !b.is_ascii_alphanumeric() && *b != b'_');
    }
    if prefix.len() >= 9 && prefix[..9].eq_ignore_ascii_case(b"<!doctype") {
        let tail = &prefix[9..];
        let spaces = tail.iter().take_while(|b| byte_whitespace(**b)).count();
        return spaces > 0
            && tail
                .get(spaces..spaces + 4)
                .is_some_and(|s| s.eq_ignore_ascii_case(b"html"));
    }
    false
}
pub(super) async fn decode(r: &crate::http::Response, d: Instant, b: &Budget) -> Result<Value> {
    let mut fields =
        ssx_owned_budget::OwnedVec::with_capacity(b, r.headers.len()).map_err(resource)?;
    for (name, value) in &r.headers {
        checkpoint(Some(d)).await?;
        fields
            .try_push((name.as_str().as_bytes(), value.as_bytes()))
            .map_err(resource)?;
    }
    let headers = || fields.as_slice().iter().copied();
    let content = http_headers::get_cooperative(headers(), b"content-type", b, d)
        .await
        .map_err(|e| {
            if matches!(
                e,
                http_headers::HeaderError::Output(ssx_source_codecs::BoundedTextError::Deadline)
            ) {
                timeout()
            } else {
                error("CRT.sh response header cannot be represented")
            }
        })?;
    let lower = content
        .as_ref()
        .map(|s| s.as_str().to_lowercase())
        .unwrap_or_default();
    let mime = lower
        .split(';')
        .next()
        .unwrap_or("")
        .trim_matches(ssx_json::python_whitespace);
    let context = format!(
        "HTTP {}, content-type {}",
        r.status,
        if mime.is_empty() { "unknown" } else { mime }
    );
    if lower.contains("html") || html_prefix(&r.body) {
        let check = || {
            if Instant::now() >= d {
                Err(ssx_source_codecs::BoundedTextError::Deadline)
            } else {
                Ok(())
            }
        };
        let encoding =
            http_charset::select_http_encoding_bounded(headers(), r.body.is_empty(), b, &check)
                .map_err(|e| {
                    if matches!(
                        e.output_error(),
                        Some(ssx_source_codecs::BoundedTextError::Deadline)
                    ) {
                        timeout()
                    } else {
                        error("CRT.sh response encoding is invalid")
                    }
                })?;
        let decoded = ssx_source_codecs::decode_http_bounded(&r.body, encoding, b, &check)
            .map_err(|e| {
                if matches!(
                    e,
                    ssx_source_codecs::BoundedDecodeError::Output(
                        ssx_source_codecs::BoundedTextError::Deadline
                    )
                ) {
                    timeout()
                } else {
                    error("CRT.sh response text cannot be decoded within native bounds")
                }
            })?;
        let points = &decoded.codepoints()[..decoded.codepoints().len().min(65536)];
        let mut found = Vec::new();
        for (name, marker) in MARKERS {
            let mut matched = false;
            for (at, window) in points.windows(marker.len()).enumerate() {
                if at % 1024 == 0 {
                    checkpoint(Some(d)).await?;
                }
                if window
                    .iter()
                    .zip(marker.bytes())
                    .all(|(p, b)| *p <= 127 && (*p as u8).to_ascii_lowercase() == b)
                {
                    matched = true;
                    break;
                }
            }
            if matched {
                found.push(name);
            }
        }
        if found.is_empty() {
            return Err(error(format!(
                "CRT.sh returned non-JSON HTML ({context}; no challenge markers)"
            )));
        }
        return Err(Failure {
            status: ssx_domain::EngineStatus::Blocked,
            message: Some(format!(
                "CRT.sh returned an upstream challenge/block page ({context}; markers={})",
                found.join(",")
            )),
        });
    }
    let value = source_json::from_slice(&r.body, Some(d))
        .await
        .map_err(|e| {
            if e == source_json::Error::Deadline {
                timeout()
            } else {
                error(e.message(
                    "CRT.sh",
                    &format!("CRT.sh returned malformed JSON ({context})"),
                ))
            }
        })?;
    let Some(rows) = value.as_array() else {
        return Err(error(format!(
            "CRT.sh returned an unexpected JSON shape; expected a list ({context})"
        )));
    };
    for row in rows {
        checkpoint(Some(d)).await?;
        if row.as_object().is_none() {
            return Err(error(format!(
                "CRT.sh returned a malformed certificate row ({context})"
            )));
        }
    }
    Ok(value)
}
pub(super) async fn parse(
    v: &Value,
    limit: &ssx_config::ConfigScalar,
    batch: &mut ProducedBatch<'_>,
    b: &Budget,
    d: Instant,
) -> Result<()> {
    let empty = Value::String("".into());
    for (i, cert) in slice(v, limit)?.iter().enumerate() {
        checkpoint(Some(d)).await?;
        let name = field(cert, "common_name", &empty)?;
        let issuer = field(cert, "issuer_name", &empty)?;
        let sans = field(cert, "name_value", &empty)?;
        let before = field(cert, "not_before", &empty)?;
        let after = field(cert, "not_after", &empty)?;
        let serial = field(cert, "serial_number", &empty)?;
        let id = field(cert, "id", &empty)?;
        let mut p = parts(b, 4).await?;
        if truth(issuer) {
            let issuer = stringify(b, issuer, d).await?;
            add(
                &mut p,
                join(b, &["Issuer: ", issuer.as_str()], "", d).await?,
            )
            .await?;
        }
        if truth(sans) {
            let sans = first_line_points(sans, d).await?;
            if !sans.is_empty() {
                let mut content = owned(b, "SANs: ", d).await?;
                append_points(&mut content, sans, d).await?;
                add(&mut p, content).await?;
            }
        }
        let before_text = if truth(before) {
            Some(prefix(b, before, 10, d).await?)
        } else {
            None
        };
        if let Some(before) = &before_text {
            add(&mut p, join(b, &["Valid: ", before.as_str()], "", d).await?).await?;
        }
        if truth(after) {
            let after = slice_repr(b, after, 10, d).await?;
            add(
                &mut p,
                join(b, &["Expires: ", after.as_str()], "", d).await?,
            )
            .await?;
        }
        let fallback = if !p.is_empty() {
            owned(b, "", d).await?
        } else if truth(serial) {
            let serial = slice_repr(b, serial, 20, d).await?;
            join(b, &["Serial: ", serial.as_str()], "", d).await?
        } else {
            owned(b, "Certificate result", d).await?
        };
        let content = content(b, &p, fallback.as_str(), d).await?;
        let id = stringify(b, id, d).await?;
        let name_fmt = stringify(b, name, d).await?;
        let has_id = cert.get("id").is_some_and(truth);
        let url = if has_id {
            join(b, &["https://crt.sh/?id=", id.as_str()], "", d).await?
        } else {
            join(b, &["https://crt.sh/?q=", name_fmt.as_str()], "", d).await?
        };
        let title = if truth(name) {
            owned(b, text(name)?, d).await?
        } else {
            join(b, &["Certificate #", id.as_str()], "", d).await?
        };
        push(
            batch,
            Kind::CrtSh,
            Row {
                url: url.as_str(),
                title: title.as_str(),
                content: content.as_str(),
                score: 0.,
                position: i,
                date: before_text.as_ref().map(|v| v.as_str()),
            },
        )
        .await?;
    }
    Ok(())
}
