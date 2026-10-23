//! Additive source JSON diagnostic formatter. Existing parser/callers unchanged.
use super::{Encoding, Error as ParseError, allocation::Accounting, checkpoint, decode, encoding};
use ssx_owned_budget::{Budget, OwnedString};
use std::time::Instant;
#[derive(Debug, PartialEq, Eq)]
pub enum DiagnosticError {
    Resource(ssx_owned_budget::Error),
    Deadline,
    Invariant,
}
impl From<ParseError> for DiagnosticError {
    fn from(error: ParseError) -> Self {
        match error {
            ParseError::Resource(error) => Self::Resource(error),
            ParseError::Deadline => Self::Deadline,
            _ => Self::Invariant,
        }
    }
}
type Error = DiagnosticError;
fn bad() -> Error {
    Error::Invariant
}
async fn append(out: &mut OwnedString, text: &str, deadline: Option<Instant>) -> Result<(), Error> {
    out.try_reserve_exact(text.len()).map_err(Error::Resource)?;
    for (i, ch) in text.chars().enumerate() {
        if i.is_multiple_of(1024) {
            checkpoint(deadline).await?;
        }
        out.try_push(ch).map_err(Error::Resource)?;
    }
    Ok(())
}
async fn number(
    out: &mut OwnedString,
    value: usize,
    budget: &Budget,
    deadline: Option<Instant>,
) -> Result<(), Error> {
    // Fixed usize formatting scratch, separately retained through its copy.
    let _charge = budget.reserve(128).map_err(Error::Resource)?;
    let token = value.to_string();
    if token.capacity() > 128 {
        return Err(bad());
    }
    append(out, &token, deadline).await
}
struct EncodingFault {
    codec: &'static str,
    start: usize,
    end: usize,
    byte: u8,
    reason: &'static str,
}
fn fault(input: &[u8], position: usize) -> Result<EncodingFault, Error> {
    let (encoding, offset) = encoding(input);
    let byte = *input.get(position).ok_or_else(bad)?;
    let mut start = position;
    let (codec, mut end, reason) = match encoding {
        Encoding::Utf8 => {
            let width = match byte {
                0xc2..=0xdf => 2,
                0xe0..=0xef => 3,
                0xf0..=0xf4 => 4,
                _ => 1,
            };
            let (end, reason) = if width == 1 {
                (start + 1, "invalid start byte")
            } else {
                let mut result = None;
                for index in 1..width {
                    let Some(next) = input.get(start + index).copied() else {
                        result = Some((input.len(), "unexpected end of data"));
                        break;
                    };
                    let invalid_second = index == 1
                        && ((byte == 0xe0 && next < 0xa0)
                            || (byte == 0xf0 && next < 0x90)
                            || (byte == 0xf4 && next > 0x8f));
                    if next & 0xc0 != 0x80 || invalid_second {
                        result = Some((start + index, "invalid continuation byte"));
                        break;
                    }
                }
                result.ok_or_else(bad)?
            };
            ("utf-8", end, reason)
        }
        Encoding::Utf16(little) => {
            // surrogatepass accepts every complete u16, even unpaired surrogates;
            // only an odd trailing byte fails, independent of an earlier pair.
            if input.len().is_multiple_of(2) {
                return Err(bad());
            }
            start = input.len() - 1;
            (
                if little { "utf-16-le" } else { "utf-16-be" },
                input.len(),
                "truncated data",
            )
        }
        Encoding::Utf32(little) => {
            let end = start.checked_add(4).ok_or_else(bad)?.min(input.len());
            (
                if little { "utf-32-le" } else { "utf-32-be" },
                end,
                if end - start < 4 {
                    "truncated data"
                } else {
                    "code point not in range(0x110000)"
                },
            )
        }
    };
    let byte = *input.get(start).ok_or_else(bad)?;
    if matches!(encoding, Encoding::Utf8) && offset == 3 {
        start = start.checked_sub(3).ok_or_else(bad)?;
        end = end.checked_sub(3).ok_or_else(bad)?;
    }
    Ok(EncodingFault {
        codec,
        start,
        end,
        byte,
        reason,
    })
}
// Preserve old parser cursor fields while recovering CPython diagnostic
// locations for string scans that advanced before reporting their failure.
async fn syntax_position(
    message: &str,
    position: usize,
    decoded: &[u32],
    deadline: Option<Instant>,
) -> Result<usize, Error> {
    if position > decoded.len() {
        return Err(bad());
    }
    match message {
        "Invalid control character at" => return position.checked_sub(1).ok_or_else(bad),
        "Invalid \\escape" => return position.checked_sub(2).ok_or_else(bad),
        "Expecting value" if position > 0 && decoded[position - 1] == 45 => return Ok(position - 1),
        "Unterminated string starting at" | "Invalid \\uXXXX escape" => {}
        _ => return Ok(position),
    }
    let (mut quoted, mut escaped, mut opening, mut unicode) = (false, false, None, None);
    for (i, point) in decoded[..position].iter().enumerate() {
        if i.is_multiple_of(1024) {
            checkpoint(deadline).await?;
        }
        if quoted {
            if escaped {
                if *point == 117 {
                    unicode = Some(i);
                }
                escaped = false;
            } else if *point == 92 {
                escaped = true;
            } else if *point == 34 {
                quoted = false;
                opening = None;
                unicode = None;
            }
        } else if *point == 34 {
            quoted = true;
            opening = Some(i);
            unicode = None;
        }
    }
    if message == "Unterminated string starting at" {
        opening.ok_or_else(bad)
    } else {
        unicode.ok_or_else(bad)
    }
}
/// Same-root fallible diagnostic. Re-decoded syntax scratch is retained/charged
/// through line/column construction. Resource/deadline never becomes wording.
pub async fn message_budgeted(
    error: &ParseError,
    input: &[u8],
    engine: &'static str,
    budget: &Budget,
    deadline: Option<Instant>,
) -> Result<OwnedString, Error> {
    match error {
        ParseError::Deadline => return Err(Error::Deadline),
        ParseError::Resource(error) => return Err(Error::Resource(*error)),
        _ => {}
    }
    checkpoint(deadline).await?;
    let mut out = OwnedString::new(budget).map_err(Error::Resource)?;
    out.try_reserve_exact(512).map_err(Error::Resource)?;
    match error {
        ParseError::Syntax { position, message } => {
            let mut accounting = Accounting::new(Some(budget)).map_err(Error::Resource)?;
            let decoded = decode(input, deadline, &mut accounting).await?;
            let position = syntax_position(message, *position, &decoded, deadline).await?;
            let (mut line, mut column) = (1usize, 1usize);
            for (i, point) in decoded[..position].iter().enumerate() {
                if i.is_multiple_of(1024) {
                    checkpoint(deadline).await?;
                }
                if *point == 10 {
                    line = line.checked_add(1).ok_or_else(bad)?;
                    column = 1;
                } else {
                    column = column.checked_add(1).ok_or_else(bad)?;
                }
            }
            append(&mut out, message, deadline).await?;
            append(&mut out, ": line ", deadline).await?;
            number(&mut out, line, budget, deadline).await?;
            append(&mut out, " column ", deadline).await?;
            number(&mut out, column, budget, deadline).await?;
            append(&mut out, " (char ", deadline).await?;
            number(&mut out, position, budget, deadline).await?;
            append(&mut out, ")", deadline).await?;
        }
        ParseError::Encoding { position } => {
            let fault = fault(input, *position)?;
            append(&mut out, "'", deadline).await?;
            append(&mut out, fault.codec, deadline).await?;
            if fault.end == fault.start + 1 {
                append(&mut out, "' codec can't decode byte 0x", deadline).await?;
                const HEX: &[u8; 16] = b"0123456789abcdef";
                let hex = [
                    HEX[(fault.byte >> 4) as usize],
                    HEX[(fault.byte & 15) as usize],
                ];
                append(
                    &mut out,
                    std::str::from_utf8(&hex).map_err(|_| bad())?,
                    deadline,
                )
                .await?;
                append(&mut out, " in position ", deadline).await?;
                number(&mut out, fault.start, budget, deadline).await?;
            } else {
                append(
                    &mut out,
                    "' codec can't decode bytes in position ",
                    deadline,
                )
                .await?;
                number(&mut out, fault.start, budget, deadline).await?;
                append(&mut out, "-", deadline).await?;
                number(
                    &mut out,
                    fault.end.checked_sub(1).ok_or_else(bad)?,
                    budget,
                    deadline,
                )
                .await?;
            }
            append(&mut out, ": ", deadline).await?;
            append(&mut out, fault.reason, deadline).await?;
        }
        ParseError::IntegerDigits(digits) => {
            append(
                &mut out,
                "Exceeds the limit (4300 digits) for integer string conversion: value has ",
                deadline,
            )
            .await?;
            number(&mut out, *digits, budget, deadline).await?;
            append(
                &mut out,
                " digits; use sys.set_int_max_str_digits() to increase the limit",
                deadline,
            )
            .await?;
        }
        ParseError::ByteLimit | ParseError::DepthLimit | ParseError::TextLimit => {
            append(&mut out, engine, deadline).await?;
            append(
                &mut out,
                match error {
                    ParseError::ByteLimit => " response exceeds JSON byte limit (4194304 bytes)",
                    ParseError::DepthLimit => " response exceeds JSON depth limit (100 levels)",
                    _ => " response exceeds JSON decoded-text limit (4194304 code points)",
                },
                deadline,
            )
            .await?;
        }
        ParseError::Deadline | ParseError::Resource(_) => {
            unreachable!("returned before output allocation")
        }
    }
    checkpoint(deadline).await?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
        time::Duration,
    };
    #[tokio::test]
    async fn exact_original_full_adapter_malformed_utf8_utf16_utf32_diagnostics() {
        let data: ssx_json::Value = ssx_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a06/source-json-diagnostic.json"
        ))
        .unwrap();
        let mut ids = std::collections::BTreeSet::new();
        for row in data["records"].as_array().unwrap() {
            assert!(ids.insert(row["id"].as_str().unwrap()));
            let body = base64::engine::general_purpose::STANDARD
                .decode(row["responses"][0]["body_b64"].as_str().unwrap())
                .unwrap();
            let budget = Budget::new(1024 * 1024).unwrap();
            let input = budget.reserve(body.capacity()).unwrap();
            let deadline = Some(Instant::now() + Duration::from_secs(30));
            let Err(error) = super::super::from_slice_budgeted(&body, deadline, &budget).await
            else {
                panic!("expected source parse failure")
            };
            let message = message_budgeted(&error, &body, "Semantic Scholar", &budget, deadline)
                .await
                .unwrap();
            assert_eq!(
                message.as_str(),
                row["output"]["error_message"].as_str().unwrap(),
                "{}",
                row["id"]
            );
            drop(message);
            drop(body);
            drop(input);
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
        assert_eq!(ids.len(), 35);
    }
    #[tokio::test]
    async fn formatting_keeps_original_operational_class_and_charged_scratch_drops() {
        let budget = Budget::new(1).unwrap();
        let expired = Some(Instant::now());
        assert!(matches!(
            message_budgeted(
                &ParseError::Resource(ssx_owned_budget::Error::Exhausted),
                b"",
                "Semantic Scholar",
                &budget,
                expired
            )
            .await,
            Err(Error::Resource(ssx_owned_budget::Error::Exhausted))
        ));
        assert!(matches!(
            message_budgeted(
                &ParseError::Deadline,
                b"",
                "Semantic Scholar",
                &budget,
                None
            )
            .await,
            Err(Error::Deadline)
        ));
        assert!(matches!(
            message_budgeted(
                &ParseError::Syntax {
                    position: 0,
                    message: "Expecting value"
                },
                b"",
                "Semantic Scholar",
                &budget,
                None
            )
            .await,
            Err(Error::Resource(_))
        ));
        assert_eq!(budget.snapshot().unwrap().used, 0);
        for expire in [false, true] {
            let body = format!("{}]", " ".repeat(512 * 1024)).into_bytes();
            let budget = Budget::new(32 * 1024 * 1024).unwrap();
            let input = budget.reserve(body.capacity()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(if expire { 2 } else { 30 });
            let Err(error) =
                super::super::from_slice_budgeted(&body, Some(deadline), &budget).await
            else {
                panic!("expected source parse failure")
            };
            let baseline = budget.snapshot().unwrap().used;
            let mut future = Box::pin(message_budgeted(
                &error,
                &body,
                "Semantic Scholar",
                &budget,
                Some(deadline),
            ));
            let mut context = Context::from_waker(Waker::noop());
            let mut observed = 0;
            for _ in 0..100000 {
                assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
                if budget.snapshot().unwrap().used >= baseline + 1024 * 1024 {
                    observed += 1;
                    if observed == 16 {
                        break;
                    }
                }
            }
            assert_eq!(observed, 16);
            if expire {
                tokio::time::sleep_until(deadline.into()).await;
                assert!(matches!(future.await, Err(Error::Deadline)));
            } else {
                drop(future);
            }
            assert_eq!(budget.snapshot().unwrap().used, baseline);
            drop(body);
            drop(input);
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
    }
}
