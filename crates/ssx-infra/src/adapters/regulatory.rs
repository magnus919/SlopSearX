//! A08-local provider request classification. Transport and codecs remain shared.
use super::{clinicaltrials as c, source_json::Value};
use crate::http::{AdapterHttp, Request};
use std::time::Instant;
pub(super) async fn fetch(
    http: &AdapterHttp,
    request: Request,
    deadline: Instant,
    empty404: bool,
    context: Option<Instant>,
) -> c::Result<Option<Value>> {
    let mut session = c::session(http, deadline)?;
    c::get_observed(
        &mut session,
        request,
        deadline,
        empty404,
        false,
        context,
        None,
    )
    .await
}
pub(super) fn raw_score(v: &Value) -> c::Result<f64> {
    match v {
        Value::Number(n) => n
            .as_f64()
            .ok_or_else(|| c::error("source score is not representable")),
        Value::Bool(b) => Ok(if *b { 1. } else { 0. }),
        _ => Err(c::error("source score must be numeric")),
    }
}
pub(super) fn float(value: &Value) -> c::Result<f64> {
    match value {
        Value::String(s) => ssx_domain::storage::normalize_python_number_text(
            s.as_str().ok_or_else(|| c::error("non-scalar number"))?,
        )
        .map_err(|_| c::error("invalid source float"))?
        .parse()
        .map_err(|_| c::error("invalid source float")),
        v => raw_score(v),
    }
}

// urllib.parse's URL parsing view: C0/space prefix stripping precedes removal
// of the three URL-ignored ASCII characters. Consumers decide whether to retain
// the original supplied URL or return a canonical provider identity.
pub(super) fn url_view(raw: &str) -> String {
    raw.trim_start_matches(|c| c <= '\u{20}')
        .chars()
        .filter(|c| !matches!(c, '\r' | '\n' | '\t'))
        .collect()
}
// urlparse separates params at the first semicolon in the FINAL path segment.
// Semicolons in preceding segments remain part of the path.
pub(super) fn path_without_params(path: &str) -> &str {
    let start = path.rfind('/').map_or(0, |i| i + 1);
    path[start..].find(';').map_or(path, |i| &path[..start + i])
}
