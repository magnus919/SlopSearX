//! HTTPX Headers + email.message charset selection inside the existing helper.
//! All request-owned scratch uses the caller's budget; no decoder or quota is created.
use ssx_owned_budget::{Budget, OwnedString, OwnedVec};
use ssx_source_codecs::{
    BoundedParameterError, BoundedStrictAsciiOutcome, BoundedTextError,
    lookup_http_encoding_bounded, strict_parameter_ascii_bounded,
};

#[derive(Debug, PartialEq, Eq)]
pub enum SelectorError {
    Output(BoundedTextError),
    Parameter(BoundedParameterError),
    IntegerLimit { digits: usize },
    Invariant,
}
impl SelectorError {
    /// Source exceptions are distinct from operational resource/deadline errors.
    pub fn source_exception(&self) -> Option<&'static str> {
        match self {
            Self::IntegerLimit { .. } | Self::Parameter(BoundedParameterError::ValueError(_)) => {
                Some("ValueError")
            }
            Self::Parameter(BoundedParameterError::RuntimeError(_)) => Some("RuntimeError"),
            Self::Parameter(BoundedParameterError::OverflowError(_)) => Some("OverflowError"),
            _ => None,
        }
    }
    /// Existing helper terminal dispatch can inspect operational failures even
    /// when raised by the strict parameter facade rather than selector scratch.
    pub fn output_error(&self) -> Option<BoundedTextError> {
        match self {
            Self::Output(error) | Self::Parameter(BoundedParameterError::Output(error)) => {
                Some(*error)
            }
            _ => None,
        }
    }
}
impl std::fmt::Display for SelectorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IntegerLimit { digits } => write!(
                f,
                "Exceeds the limit (4300 digits) for integer string conversion: value has {digits} digits; use sys.set_int_max_str_digits() to increase the limit"
            ),
            Self::Parameter(
                BoundedParameterError::ValueError(message)
                | BoundedParameterError::RuntimeError(message)
                | BoundedParameterError::OverflowError(message),
            ) => f.write_str(message),
            _ => write!(f, "{self:?}"),
        }
    }
}
impl std::error::Error for SelectorError {}
impl From<BoundedTextError> for SelectorError {
    fn from(e: BoundedTextError) -> Self {
        Self::Output(e)
    }
}
impl From<ssx_owned_budget::Error> for SelectorError {
    fn from(e: ssx_owned_budget::Error) -> Self {
        Self::Output(e.into())
    }
}
impl From<BoundedParameterError> for SelectorError {
    fn from(e: BoundedParameterError) -> Self {
        Self::Parameter(e)
    }
}
impl From<super::http_headers::HeaderError> for SelectorError {
    fn from(error: super::http_headers::HeaderError) -> Self {
        match error {
            super::http_headers::HeaderError::Output(error) => Self::Output(error),
            super::http_headers::HeaderError::Invariant => Self::Invariant,
        }
    }
}
type Check<'a> = &'a dyn Fn() -> Result<(), BoundedTextError>;
fn space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}
fn trim(s: &str) -> &str {
    s.trim_matches(space)
}
fn push(out: &mut OwnedString, c: char, check: Check<'_>) -> Result<(), SelectorError> {
    check()?;
    out.try_push(c)?;
    Ok(())
}
fn copy(out: &mut OwnedString, s: &str, check: Check<'_>) -> Result<(), SelectorError> {
    for c in s.chars() {
        push(out, c, check)?;
    }
    Ok(())
}
fn unquote(s: &str, budget: &Budget, check: Check<'_>) -> Result<OwnedString, SelectorError> {
    let mut out = OwnedString::new(budget)?;
    out.try_reserve_exact(s.len())?;
    let quote = s.len() > 1 && s.starts_with('"') && s.ends_with('"');
    let angle = s.len() > 1 && s.starts_with('<') && s.ends_with('>');
    let s = if quote || angle {
        &s[1..s.len() - 1]
    } else {
        s
    };
    // Python uses two ordered replacements, not generic backslash escaping.
    let mut stage = OwnedString::new(budget)?;
    stage.try_reserve_exact(s.len())?;
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        check()?;
        if quote && c == '\\' && it.peek() == Some(&'\\') {
            it.next();
        }
        push(&mut stage, c, check)?;
    }
    let mut it = stage.as_str().chars().peekable();
    while let Some(c) = it.next() {
        check()?;
        if quote && c == '\\' && it.peek() == Some(&'"') {
            it.next();
            push(&mut out, '"', check)?;
        } else {
            push(&mut out, c, check)?;
        }
    }
    Ok(out)
}
#[derive(Clone, Copy)]
struct Param<'a> {
    name: &'a str,
    value: &'a str,
    lower: bool,
    order: usize,
    cont: Option<Continuation<'a>>,
}
#[derive(Clone, Copy)]
struct Continuation<'a> {
    base: &'a str,
    number: Option<&'a str>,
    encoded: bool,
}
fn continuation<'a>(
    name: &'a str,
    check: Check<'_>,
) -> Result<Option<Continuation<'a>>, SelectorError> {
    let Some(star) = name.find('*') else {
        return Ok(None);
    };
    let base = &name[..star];
    if base.is_empty() || !base.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        return Ok(None);
    }
    let tail = &name[star + 1..];
    if tail.is_empty() {
        return Ok(Some(Continuation {
            base,
            number: None,
            encoded: true,
        }));
    }
    let number = tail.strip_suffix('*').unwrap_or(tail);
    for b in number.bytes() {
        check()?;
        if !b.is_ascii_digit() {
            return Ok(None);
        }
    }
    if number.is_empty() {
        return Ok(None);
    }
    if number.len() > 4300 {
        return Err(SelectorError::IntegerLimit {
            digits: number.len(),
        });
    }
    Ok(Some(Continuation {
        base,
        number: Some(number),
        encoded: name.ends_with('*'),
    }))
}
fn key_eq(a: &str, alower: bool, b: &str, blower: bool) -> bool {
    a.len() == b.len()
        && a.bytes().zip(b.bytes()).all(|(a, b)| {
            (if alower { a.to_ascii_lowercase() } else { a })
                == (if blower { b.to_ascii_lowercase() } else { b })
        })
}
fn number(n: Option<&str>) -> &str {
    n.unwrap_or("").trim_start_matches('0')
}
fn percent(s: &str, budget: &Budget, check: Check<'_>) -> Result<OwnedString, SelectorError> {
    let mut out = OwnedString::new(budget)?;
    out.try_reserve_exact(s.len())?;
    let mut at = 0;
    while at < s.len() {
        check()?;
        let rest = &s[at..];
        if rest.starts_with('%') && rest.len() >= 3 {
            let a = rest.as_bytes()[1];
            let b = rest.as_bytes()[2];
            let hex = |b: u8| match b {
                b'0'..=b'9' => Some(b - b'0'),
                b'a'..=b'f' => Some(b - b'a' + 10),
                b'A'..=b'F' => Some(b - b'A' + 10),
                _ => None,
            };
            if let (Some(a), Some(b)) = (hex(a), hex(b)) {
                push(&mut out, char::from(a * 16 + b), check)?;
                at += 3;
                continue;
            }
        }
        let c = rest.chars().next().ok_or(SelectorError::Invariant)?;
        push(&mut out, c, check)?;
        at += c.len_utf8();
    }
    Ok(out)
}
fn raw_unicode(s: &str, budget: &Budget, check: Check<'_>) -> Result<OwnedVec<u8>, SelectorError> {
    let mut size = 0usize;
    for c in s.chars() {
        check()?;
        size = size
            .checked_add(if u32::from(c) <= 255 {
                1
            } else if u32::from(c) <= 0xffff {
                6
            } else {
                10
            })
            .ok_or(ssx_owned_budget::Error::Overflow)?;
    }
    let mut out = OwnedVec::with_capacity(budget, size)?;
    const HEX: &[u8] = b"0123456789abcdef";
    for c in s.chars() {
        check()?;
        let n = u32::from(c);
        if n <= 255 {
            out.try_push(n as u8)?;
        } else {
            out.try_extend_from_slice(if n <= 0xffff { b"\\u" } else { b"\\U" })?;
            for shift in (0..if n <= 0xffff { 4 } else { 8 }).rev() {
                out.try_push(HEX[((n >> (shift * 4)) & 15) as usize])?;
            }
        }
    }
    Ok(out)
}
fn finish(label: &str, budget: &Budget, check: Check<'_>) -> Result<&'static str, SelectorError> {
    if !label.is_ascii() {
        return Ok("utf-8");
    }
    let mut lower = OwnedString::new(budget)?;
    lower.try_reserve_exact(label.len())?;
    for c in label.chars() {
        push(&mut lower, c.to_ascii_lowercase(), check)?;
    }
    Ok(lookup_http_encoding_bounded(lower.as_str(), budget, check)?.unwrap_or("utf-8"))
}
/// Borrowed ordered header pairs. Call only after validating the entire helper
/// frame, within its existing admitted operation and supervised process.
pub fn select_http_encoding_bounded<'a, I>(
    headers: I,
    body_empty: bool,
    budget: &Budget,
    check: Check<'_>,
) -> Result<&'static str, SelectorError>
where
    I: Iterator<Item = (&'a [u8], &'a [u8])> + Clone,
{
    if body_empty {
        return Ok("utf-8");
    }
    check()?;
    let content = match super::http_headers::get_bounded(headers, b"content-type", budget, check)? {
        Some(content) => content,
        None => return Ok("utf-8"),
    };
    let text = content.as_str();
    let mut param_count = 1usize;
    for b in text.bytes() {
        check()?;
        if b == b';' {
            param_count = param_count
                .checked_add(1)
                .ok_or(ssx_owned_budget::Error::Overflow)?;
        }
    }
    let mut params = OwnedVec::with_capacity(budget, param_count)?;
    let mut start = 0;
    let mut quotes = 0usize;
    let mut previous = 0u8;
    let mut order = 0;
    for (at, b) in text.bytes().enumerate() {
        check()?;
        if b == b'"' && previous != b'\\' {
            quotes ^= 1;
        }
        // _parseparam accepts a leading semicolon regardless of quote count.
        if b == b';' && (at == start || quotes == 0) {
            add_param(&mut params, &text[start..at], order, check)?;
            order += 1;
            start = at + 1;
            quotes = 0;
        }
        previous = b;
    }
    add_param(&mut params, &text[start..], order, check)?;
    // decode_params handles every continuation integer before get_param.
    for p in params.as_slice() {
        check()?;
        if (p.order == 0 || p.cont.is_none()) && p.name.eq_ignore_ascii_case("charset") {
            let value = unquote(p.value, budget, check)?;
            return finish(value.as_str(), budget, check);
        }
    }
    let selected = params.as_slice().iter().find(|p| {
        p.order != 0
            && p.cont
                .is_some_and(|c| c.base.eq_ignore_ascii_case("charset"))
    });
    let Some(selected) = selected else {
        return Ok("utf-8");
    };
    let selected_base = selected.cont.ok_or(SelectorError::Invariant)?.base;
    let mut segments = OwnedVec::with_capacity(budget, params.len())?;
    let mut has_zero = false;
    for p in params.as_slice() {
        check()?;
        if p.order == 0 {
            continue;
        }
        if let Some(c) = p.cont
            && key_eq(c.base, p.lower, selected_base, selected.lower)
        {
            has_zero |= c.number.is_some_and(|n| number(Some(n)).is_empty());
            segments.try_push(*p)?;
        }
    }
    let mut failure = None;
    segments.as_mut_slice().sort_unstable_by(|a, b| {
        if failure.is_none() {
            failure = check().err();
        }
        let an = number(a.cont.and_then(|c| c.number));
        let bn = number(b.cont.and_then(|c| c.number));
        an.len()
            .cmp(&bn.len())
            .then_with(|| an.cmp(bn))
            .then_with(|| a.order.cmp(&b.order))
    });
    if let Some(error) = failure {
        return Err(error.into());
    }
    check()?;
    let mut join_size = 0usize;
    for p in segments.as_slice() {
        check()?;
        join_size = join_size
            .checked_add(p.value.len())
            .ok_or(ssx_owned_budget::Error::Overflow)?;
    }
    let mut joined = OwnedString::new(budget)?;
    joined.try_reserve_exact(join_size)?;
    let mut extended = false;
    for p in segments.as_slice() {
        check()?;
        let c = p.cont.ok_or(SelectorError::Invariant)?;
        if has_zero && c.number.is_none() {
            continue;
        }
        let value = unquote(p.value, budget, check)?;
        if c.encoded {
            extended = true;
            let decoded = percent(value.as_str(), budget, check)?;
            copy(&mut joined, decoded.as_str(), check)?;
        } else {
            copy(&mut joined, value.as_str(), check)?;
        }
    }
    if !extended {
        return finish(joined.as_str(), budget, check);
    }
    // decode_params quotes the concatenation before splitting charset/language.
    let mut quoted = OwnedString::new(budget)?;
    quoted.try_reserve_exact(
        joined
            .len()
            .checked_mul(2)
            .ok_or(ssx_owned_budget::Error::Overflow)?,
    )?;
    for c in joined.as_str().chars() {
        if c == '\\' || c == '"' {
            push(&mut quoted, '\\', check)?;
        }
        push(&mut quoted, c, check)?;
    }
    let mut parts = quoted.as_str().splitn(3, '\'');
    let first = parts.next().ok_or(SelectorError::Invariant)?;
    let language = parts.next();
    let text = parts.next();
    let (label, text) = if let (Some(_), Some(text)) = (language, text) {
        (if first.is_empty() { "us-ascii" } else { first }, text)
    } else {
        ("us-ascii", quoted.as_str())
    };
    let mut wrapped = OwnedString::new(budget)?;
    wrapped.try_reserve_exact(
        text.len()
            .checked_add(2)
            .ok_or(ssx_owned_budget::Error::Overflow)?,
    )?;
    push(&mut wrapped, '"', check)?;
    copy(&mut wrapped, text, check)?;
    push(&mut wrapped, '"', check)?;
    let text = unquote(wrapped.as_str(), budget, check)?;
    let raw = raw_unicode(text.as_str(), budget, check)?;
    match strict_parameter_ascii_bounded(raw.as_slice(), label, budget, check) {
        Ok(BoundedStrictAsciiOutcome::Ascii(value)) => finish(value.as_str(), budget, check),
        Ok(BoundedStrictAsciiOutcome::NonAscii) => Ok("utf-8"),
        Ok(BoundedStrictAsciiOutcome::UnicodeFailure)
        | Err(BoundedParameterError::LookupFailure) => finish(text.as_str(), budget, check),
        Err(error) => Err(error.into()),
    }
}
fn add_param<'a>(
    params: &mut OwnedVec<Param<'a>>,
    part: &'a str,
    order: usize,
    check: Check<'_>,
) -> Result<(), SelectorError> {
    check()?;
    let part = trim(part);
    let (name, value, lower) = if let Some((name, value)) = part.split_once('=') {
        (trim(name), trim(value), true)
    } else {
        (part, "", false)
    };
    let cont = if order == 0 {
        None
    } else {
        continuation(name, check)?
    };
    params.try_push(Param {
        name,
        value,
        lower,
        order,
        cont,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    #[test]
    fn actual_httpx_charset_corpus() {
        let corpus: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a06/source-charset.json"
        ))
        .unwrap();
        let mut count = 0;
        for row in corpus["records"].as_array().unwrap() {
            let raw: Vec<(Vec<u8>, Vec<u8>)> = row["responses"][0]["headers_b64"]
                .as_array()
                .unwrap()
                .iter()
                .map(|pair| {
                    (
                        base64::engine::general_purpose::STANDARD
                            .decode(pair[0].as_str().unwrap())
                            .unwrap(),
                        base64::engine::general_purpose::STANDARD
                            .decode(pair[1].as_str().unwrap())
                            .unwrap(),
                    )
                })
                .collect();
            let budget = Budget::new(16 * 1024 * 1024).unwrap();
            let result = select_http_encoding_bounded(
                raw.iter().map(|(a, b)| (a.as_slice(), b.as_slice())),
                false,
                &budget,
                &|| Ok(()),
            );
            if row["selector"]["error"].is_object() {
                assert_eq!(
                    result,
                    Err(SelectorError::IntegerLimit { digits: 4301 }),
                    "{}",
                    row["id"]
                );
            } else {
                assert_eq!(
                    result.unwrap(),
                    ssx_source_codecs::canonical(
                        row["selector"]["selected_encoding"].as_str().unwrap()
                    )
                    .unwrap(),
                    "{}",
                    row["id"]
                );
            }
            assert_eq!(budget.snapshot().unwrap().used, 0, "{}", row["id"]);
            count += 1;
        }
        assert_eq!(count, 45);
    }
    #[test]
    fn empty_precedes_headers_checkpoint_and_nul() {
        let budget = Budget::new(1).unwrap();
        assert_eq!(
            select_http_encoding_bounded(
                std::iter::once((
                    b"content-type".as_slice(),
                    b"text/plain; charset=\0".as_slice()
                )),
                true,
                &budget,
                &|| Err(BoundedTextError::Deadline)
            ),
            Ok("utf-8")
        );
        assert_eq!(budget.snapshot().unwrap().peak, 0);
    }
    #[test]
    fn resource_deadline_and_parameter_errors_are_terminal_and_release() {
        let headers = [(
            b"content-type".as_slice(),
            b"text/plain; charset*=utf-8''utf-7".as_slice(),
        )];
        let tiny = Budget::new(1).unwrap();
        assert!(matches!(
            select_http_encoding_bounded(headers.into_iter(), false, &tiny, &|| Ok(())),
            Err(SelectorError::Output(BoundedTextError::Resource(_)))
        ));
        assert_eq!(tiny.snapshot().unwrap().used, 0);
        let budget = Budget::new(1024 * 1024).unwrap();
        let calls = std::cell::Cell::new(0);
        let check = || {
            let n = calls.get() + 1;
            calls.set(n);
            if n > 70 {
                Err(BoundedTextError::Deadline)
            } else {
                Ok(())
            }
        };
        assert_eq!(
            select_http_encoding_bounded(headers.into_iter(), false, &budget, &check),
            Err(SelectorError::Output(BoundedTextError::Deadline))
        );
        assert_eq!(budget.snapshot().unwrap().used, 0);
        let nul = [(
            b"content-type".as_slice(),
            b"text/plain; charset=\0".as_slice(),
        )];
        assert_eq!(
            select_http_encoding_bounded(nul.into_iter(), false, &budget, &|| Ok(())),
            Err(SelectorError::Parameter(BoundedParameterError::ValueError(
                "embedded null character"
            )))
        );
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[test]
    fn actual_source_boundaries_preserve_success_and_exact_errors() {
        let corpus: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a06/source-charset-boundaries.json"
        ))
        .unwrap();
        let rows = corpus["records"].as_array().unwrap();
        assert_eq!(rows.len(), 179);
        for row in rows {
            let raw: Vec<(Vec<u8>, Vec<u8>)> = row["headers_b64"]
                .as_array()
                .unwrap()
                .iter()
                .map(|pair| {
                    (
                        base64::engine::general_purpose::STANDARD
                            .decode(pair[0].as_str().unwrap())
                            .unwrap(),
                        base64::engine::general_purpose::STANDARD
                            .decode(pair[1].as_str().unwrap())
                            .unwrap(),
                    )
                })
                .collect();
            let budget = Budget::new(16 * 1024 * 1024).unwrap();
            let result = select_http_encoding_bounded(
                raw.iter().map(|(a, b)| (a.as_slice(), b.as_slice())),
                row["body_empty"].as_bool().unwrap(),
                &budget,
                &|| Ok(()),
            );
            if row["error"].is_object() {
                let error = result.expect_err(row["id"].as_str().unwrap());
                assert_eq!(
                    error.source_exception(),
                    row["error"]["type"].as_str(),
                    "{}",
                    row["id"]
                );
                assert_eq!(
                    error.to_string(),
                    row["error"]["message"].as_str().unwrap(),
                    "{}",
                    row["id"]
                );
            } else {
                assert_eq!(
                    result.unwrap(),
                    ssx_source_codecs::canonical(row["selected_encoding"].as_str().unwrap())
                        .unwrap(),
                    "{}",
                    row["id"]
                );
            }
            assert_eq!(budget.snapshot().unwrap().used, 0, "{}", row["id"]);
        }
    }
    #[test]
    fn every_checkpoint_and_scarce_budget_propagates_without_source_fallback() {
        for value in [
            b"text/plain; charset*=utf-8''utf-7".as_slice(),
            b"text/plain; charset*=utf-7''utf-7%2B%21".as_slice(),
        ] {
            let headers = [(b"content-type".as_slice(), value)];
            let budget = Budget::new(1024 * 1024).unwrap();
            let retained = budget.reserve(value.len()).unwrap();
            let calls = std::cell::Cell::new(0usize);
            let result = select_http_encoding_bounded(headers.into_iter(), false, &budget, &|| {
                calls.set(calls.get() + 1);
                Ok(())
            });
            assert_eq!(result, Ok("utf-7"));
            let checkpoints = calls.get();
            let peak = budget.snapshot().unwrap().peak;
            assert_eq!(budget.snapshot().unwrap().used, retained.bytes());
            drop(retained);
            for fail_at in 1..=checkpoints {
                let calls = std::cell::Cell::new(0usize);
                let result =
                    select_http_encoding_bounded(headers.into_iter(), false, &budget, &|| {
                        calls.set(calls.get() + 1);
                        if calls.get() == fail_at {
                            Err(BoundedTextError::Deadline)
                        } else {
                            Ok(())
                        }
                    });
                assert_eq!(
                    result.unwrap_err().output_error(),
                    Some(BoundedTextError::Deadline),
                    "checkpoint {fail_at}/{checkpoints}"
                );
                assert_eq!(budget.snapshot().unwrap().used, 0);
            }
            for limit in 1..peak - value.len() {
                let budget = Budget::new(limit).unwrap();
                let result =
                    select_http_encoding_bounded(headers.into_iter(), false, &budget, &|| Ok(()));
                assert!(
                    matches!(
                        result.unwrap_err().output_error(),
                        Some(BoundedTextError::Resource(_))
                    ),
                    "limit {limit}"
                );
                assert_eq!(budget.snapshot().unwrap().used, 0);
            }
        }
    }
}
