//! Source Python float lexical operations over charged cooperative scratch.
use super::source_json::Text;
use ssx_owned_budget::{Budget, OwnedString};
use std::time::Instant;
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Resource,
    Deadline,
}
impl From<ssx_owned_budget::Error> for Error {
    fn from(_: ssx_owned_budget::Error) -> Self {
        Self::Resource
    }
}
async fn checkpoint(deadline: Instant) -> Result<(), Error> {
    tokio::task::yield_now().await;
    if Instant::now() >= deadline {
        Err(Error::Deadline)
    } else {
        Ok(())
    }
}
pub async fn string_float(
    text: &Text,
    budget: &Budget,
    deadline: Instant,
) -> Result<Option<f64>, Error> {
    let Some(raw) = text.as_str() else {
        return Ok(None);
    };
    string_float_scalar(raw, budget, deadline).await
}
/// Borrowed scalar entry point for raw HTTP header values; shares the exact
/// lexical implementation and caller root with the source-Text wrapper.
pub async fn string_float_scalar(
    raw: &str,
    budget: &Budget,
    deadline: Instant,
) -> Result<Option<f64>, Error> {
    let mut normalized = OwnedString::new(budget)?;
    normalized
        .try_reserve_exact(raw.len())
        .map_err(|_| Error::Resource)?;
    let mut previous_digit = false;
    let mut underscore = false;
    for (index, ch) in raw.chars().enumerate() {
        if index % 1024 == 0 {
            checkpoint(deadline).await?;
        }
        // Reuse the accepted Unicode decimal table without scanning a large
        // string synchronously. Whitespace must survive until outer trimming.
        let digit = ssx_domain::storage::python_decimal_digit(ch).map(|digit| b'0' + digit);
        let ch = digit.map_or(ch, char::from);
        if underscore && !ch.is_ascii_digit() {
            return Ok(None);
        }
        if ch == '_' {
            if !previous_digit {
                return Ok(None);
            }
            underscore = true;
            previous_digit = false;
        } else {
            normalized.try_push(ch)?;
            underscore = false;
            previous_digit = ch.is_ascii_digit();
        }
    }
    if underscore {
        return Ok(None);
    }
    // The loop has already visited all characters cooperatively; whitespace
    // runs are bounded separately so trim does not become a second long scan.
    let bytes = normalized.as_bytes();
    let mut first = 0;
    let mut last = bytes.len();
    for (index, (at, ch)) in normalized.as_str().char_indices().enumerate() {
        if index % 1024 == 0 {
            checkpoint(deadline).await?;
        }
        if !float_space(ch) {
            first = at;
            break;
        }
        first = at + ch.len_utf8();
    }
    for (index, (at, ch)) in normalized.as_str().char_indices().rev().enumerate() {
        if index % 1024 == 0 {
            checkpoint(deadline).await?;
        }
        if !float_space(ch) {
            last = at + ch.len_utf8();
            break;
        }
        last = at;
    }
    if first >= last {
        return Ok(None);
    }
    let raw = &normalized.as_str()[first..last];
    let (negative, unsigned) = if let Some(s) = raw.strip_prefix('-') {
        (true, s)
    } else {
        (false, raw.strip_prefix('+').unwrap_or(raw))
    };
    if unsigned.eq_ignore_ascii_case("nan") {
        return Ok(Some(if negative { -f64::NAN } else { f64::NAN }));
    }
    if unsigned.eq_ignore_ascii_case("inf") || unsigned.eq_ignore_ascii_case("infinity") {
        return Ok(Some(if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        }));
    }
    let mut decimal = OwnedString::new(budget)?;
    decimal
        .try_reserve_exact(raw.len().checked_add(2).ok_or(Error::Resource)?)
        .map_err(|_| Error::Resource)?;
    if negative {
        decimal.try_push('-')?;
    }
    let mut integer_end = 0;
    let mut nonzero = None;
    for (index, byte) in unsigned.bytes().enumerate() {
        if index % 1024 == 0 {
            checkpoint(deadline).await?;
        }
        if !byte.is_ascii_digit() {
            break;
        }
        if byte != b'0' && nonzero.is_none() {
            nonzero = Some(index);
        }
        integer_end = index + 1;
    }
    if integer_end == 0 {
        if !unsigned.starts_with('.') || !unsigned.as_bytes().get(1).is_some_and(u8::is_ascii_digit)
        {
            return Ok(None);
        }
        decimal.try_push('0')?;
    } else if let Some(at) = nonzero {
        for chunk in unsigned.as_bytes()[at..integer_end].chunks(1024) {
            checkpoint(deadline).await?;
            decimal.try_push_str(std::str::from_utf8(chunk).expect("ASCII digits"))?;
        }
    } else {
        decimal.try_push('0')?;
    }
    let tail = &unsigned[integer_end..];
    let trailing_dot =
        tail.starts_with('.') && (tail.len() == 1 || matches!(tail.as_bytes()[1], b'e' | b'E'));
    let tail = if trailing_dot {
        decimal.try_push_str(".0")?;
        &tail[1..]
    } else {
        tail
    };
    for chunk in tail.as_bytes().chunks(1024) {
        checkpoint(deadline).await?;
        // Remaining valid grammar is ASCII. Non-ASCII cannot be split into an
        // invalid UTF8 slice and is rejected as a source float lexical error.
        if !chunk.is_ascii() {
            return Ok(None);
        }
        decimal.try_push_str(std::str::from_utf8(chunk).expect("ASCII grammar"))?;
    }
    let capacity = decimal.len().checked_add(8).ok_or(Error::Resource)?;
    // Caller-owned normalizer backing remains charged through parser drop.
    let _scratch = budget.reserve(capacity.checked_add(25).ok_or(Error::Resource)?)?;
    let mut scratch = String::new();
    scratch
        .try_reserve_exact(capacity)
        .map_err(|_| Error::Resource)?;
    if scratch.capacity() != capacity {
        return Err(Error::Resource);
    }
    let mut parser = match ssx_json::FloatNormalizer::with_buffer(decimal.as_str(), scratch) {
        Ok(parser) => parser,
        Err(ssx_json::FloatBufferError::Input(ssx_json::FloatError::ByteLimit)) => {
            return Err(Error::Resource);
        }
        Err(ssx_json::FloatBufferError::Input(_)) => return Ok(None),
        Err(ssx_json::FloatBufferError::Capacity) => return Err(Error::Resource),
    };
    loop {
        checkpoint(deadline).await?;
        match parser.advance(1024) {
            Ok(true) => break,
            Ok(false) => {}
            Err(_) => return Ok(None),
        }
    }
    checkpoint(deadline).await?;
    let value = parser.finish().ok();
    checkpoint(deadline).await?;
    Ok(value)
}
fn float_space(ch: char) -> bool {
    if ch.is_ascii() {
        ch.is_ascii_whitespace() || ch == '\u{b}'
    } else {
        ch.is_whitespace()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
        time::Duration,
    };
    #[tokio::test]
    async fn pinned_original_float_controls_keep_exact_bits() {
        let data: ssx_json::Value = ssx_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a06/source-float-lexical.json"
        ))
        .unwrap();
        let budget = Budget::new(1024 * 1024).unwrap();
        for row in data["records"].as_array().unwrap() {
            let text = Text::from(row["text"].as_str().unwrap());
            let actual = string_float(&text, &budget, Instant::now() + Duration::from_secs(5))
                .await
                .unwrap();
            if let Some(bits) = row["bits"].as_str() {
                assert_eq!(format!("{:016x}", actual.unwrap().to_bits()), bits);
            } else {
                assert_eq!(actual, None);
            }
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
    }
    #[tokio::test]
    async fn charged_lexical_work_yields_then_cancels_or_expires_and_refuses_small_budget() {
        let text = Text::from(format!("{}1", "\u{a0}\u{2000}".repeat(131072)).as_str());
        let small = Budget::new(1024).unwrap();
        assert_eq!(
            string_float(&text, &small, Instant::now() + Duration::from_secs(30)).await,
            Err(Error::Resource)
        );
        assert_eq!(small.snapshot().unwrap().used, 0);
        for expire in [false, true] {
            let budget = Budget::new(8 * 1024 * 1024).unwrap();
            let deadline = Instant::now() + Duration::from_secs(if expire { 2 } else { 30 });
            let mut future = Box::pin(string_float(&text, &budget, deadline));
            let mut context = Context::from_waker(Waker::noop());
            let mut observed = 0;
            for _ in 0..100000 {
                assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
                if budget.snapshot().unwrap().used > 0 {
                    observed += 1;
                    if observed == 16 {
                        break;
                    }
                }
            }
            assert_eq!(observed, 16);
            if expire {
                tokio::time::sleep_until(deadline.into()).await;
                assert_eq!(future.await, Err(Error::Deadline));
            } else {
                drop(future);
            }
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
    }
}
