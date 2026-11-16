//! Closed decoder reply. Borrowed full-frame validation precedes projection.
use super::{DetectorFlags, OperationFailure};
use ssx_source_codecs::{BoundedDecodeError, BoundedTextError};
pub(super) const HEADER: usize = 15;
const MAGIC: &[u8; 4] = b"A05D";
const VERSION: u8 = 1;
// The complete closed native replacement-body diagnostic set. Unknown new
// diagnostics fail as an invariant; no available diagnostic is normalized away.
const CODEC_ERRORS: [&str; 7] = [
    "",
    "descriptor 'translate' for 'str' objects doesn't apply to a 'bytes' object",
    "Unsupported error handling: replace",
    "undefined encoding",
    "'utf-16' codec can't decode bytes in position 0-1: Stream does not start with BOM",
    "'utf-32' codec can't decode bytes in position 0-3: Stream does not start with BOM",
    "internal codec error",
];
pub(super) fn bound(input: usize) -> Result<usize, OperationFailure> {
    input
        .checked_mul(4)
        .and_then(|n| n.checked_add(HEADER))
        .ok_or(OperationFailure::Resource)
}
pub(super) struct Decoded<'a> {
    pub(super) count: usize,
    pub(super) bytes: &'a [u8],
    pub(super) flags: DetectorFlags,
}
// Fixed 1024-point chunks bound uninterrupted validation work. Checks remain
// per point; yielding lets caller cancellation poll while all owners stay live.
const POINT_CHUNK: usize = 1024;
#[cfg(test)]
pub(super) async fn validate_async<'a>(
    frame: &'a [u8],
    input: usize,
    checkpoint: &(dyn Fn() -> Result<(), OperationFailure> + Sync),
) -> Result<Decoded<'a>, OperationFailure> {
    validate_http_async(frame, input, 0, checkpoint).await
}
pub(super) async fn validate_http_async<'a>(
    frame: &'a [u8],
    input: usize,
    header_bytes: usize,
    checkpoint: &(dyn Fn() -> Result<(), OperationFailure> + Sync),
) -> Result<Decoded<'a>, OperationFailure> {
    let decoded = validate_header(frame, input, header_bytes, checkpoint)?;
    for chunk in decoded.bytes.chunks(POINT_CHUNK * 4) {
        for point in chunk.as_chunks::<4>().0 {
            checkpoint()?;
            if u32::from_be_bytes(*point) > 0x10ffff {
                return Err(OperationFailure::Invariant(
                    "HTML helper invalid decoder response",
                ));
            }
        }
        tokio::task::yield_now().await;
        checkpoint()?;
    }
    Ok(decoded)
}
#[cfg(test)]
pub(super) fn validate<'a>(
    frame: &'a [u8],
    input: usize,
    checkpoint: &dyn Fn() -> Result<(), OperationFailure>,
) -> Result<Decoded<'a>, OperationFailure> {
    let decoded = validate_header(frame, input, 0, checkpoint)?;
    for point in decoded.bytes.as_chunks::<4>().0 {
        checkpoint()?;
        if u32::from_be_bytes(*point) > 0x10ffff {
            return Err(OperationFailure::Invariant(
                "HTML helper invalid decoder response",
            ));
        }
    }
    checkpoint()?;
    Ok(decoded)
}
fn validate_header<'a>(
    frame: &'a [u8],
    input: usize,
    header_bytes: usize,
    checkpoint: &dyn Fn() -> Result<(), OperationFailure>,
) -> Result<Decoded<'a>, OperationFailure> {
    checkpoint()?;
    let invalid = || OperationFailure::Invariant("HTML helper invalid decoder response");
    if frame.get(..4) != Some(MAGIC) || frame.get(4) != Some(&VERSION) {
        return Err(invalid());
    }
    match frame.get(5) {
        Some(0) => {
            let flags = *frame.get(6).ok_or_else(invalid)?;
            if flags & !7 != 0 {
                return Err(invalid());
            }
            let count = usize::try_from(u64::from_be_bytes(
                frame
                    .get(7..15)
                    .ok_or_else(invalid)?
                    .try_into()
                    .map_err(|_| invalid())?,
            ))
            .map_err(|_| invalid())?;
            if count > input || frame.len() != bound(count)? {
                return Err(invalid());
            }
            let bytes = &frame[HEADER..];
            checkpoint()?;
            Ok(Decoded {
                count,
                bytes,
                flags: DetectorFlags {
                    duckduckgo_challenge: flags & 1 != 0,
                    duckduckgo_legitimate_empty: flags & 2 != 0,
                    google_challenge: flags & 4 != 0,
                },
            })
        }
        Some(5) if header_bytes != 0 => {
            use super::SelectorFailure;
            let failure = match (frame.get(6), frame.len()) {
                (Some(0), 7) => SelectorFailure::NullCharacter,
                (Some(1), 7) => SelectorFailure::RuntimeCodec,
                (Some(2), 7) => SelectorFailure::OverflowCodec,
                (Some(3), 15) => {
                    let digits = usize::try_from(u64::from_be_bytes(
                        frame[7..15].try_into().map_err(|_| invalid())?,
                    ))
                    .map_err(|_| invalid())?;
                    if digits <= 4300 || digits > header_bytes {
                        return Err(invalid());
                    }
                    SelectorFailure::IntegerLimit(digits)
                }
                _ => return Err(invalid()),
            };
            Err(OperationFailure::Selector(failure))
        }
        Some(tag @ (1..=4)) => {
            if frame.len() != 7 {
                return Err(invalid());
            }
            Err(match (*tag, frame[6]) {
                (1, 0) => OperationFailure::Resource,
                (2, code) => OperationFailure::Codec(
                    CODEC_ERRORS.get(usize::from(code)).ok_or_else(invalid)?,
                ),
                (3, 0) => OperationFailure::Invariant("HTML helper decoder invariant"),
                (4, 0) => OperationFailure::Deadline,
                _ => return Err(invalid()),
            })
        }
        _ => Err(invalid()),
    }
}
pub(super) fn write_success(
    writer: &mut impl std::io::Write,
    points: &[u32],
    flags: DetectorFlags,
) -> std::io::Result<()> {
    let length = bound(points.len()).map_err(|error| std::io::Error::other(error.to_string()))?;
    writer.write_all(&(length as u64).to_be_bytes())?;
    writer.write_all(MAGIC)?;
    writer.write_all(&[
        VERSION,
        0,
        u8::from(flags.duckduckgo_challenge)
            | (u8::from(flags.duckduckgo_legitimate_empty) << 1)
            | (u8::from(flags.google_challenge) << 2),
    ])?;
    writer.write_all(&(points.len() as u64).to_be_bytes())?;
    for point in points {
        writer.write_all(&point.to_be_bytes())?;
    }
    writer.flush()
}
pub(super) fn write_failure(
    writer: &mut impl std::io::Write,
    error: BoundedDecodeError,
) -> std::io::Result<()> {
    let (tag, code) = match error {
        BoundedDecodeError::Output(BoundedTextError::Resource(_)) => (1, 0),
        BoundedDecodeError::Output(BoundedTextError::Deadline) => (4, 0),
        BoundedDecodeError::Codec(message) => CODEC_ERRORS
            .iter()
            .position(|candidate| *candidate == message)
            .map_or((3, 0), |index| (2, index as u8)),
        _ => (3, 0),
    };
    writer.write_all(&7u64.to_be_bytes())?;
    writer.write_all(MAGIC)?;
    writer.write_all(&[VERSION, tag, code])?;
    writer.flush()
}
pub(super) fn write_selector_failure(
    writer: &mut impl std::io::Write,
    error: crate::adapters::http_charset::SelectorError,
) -> std::io::Result<()> {
    use crate::adapters::http_charset::SelectorError;
    use ssx_source_codecs::BoundedParameterError;
    if let Some(output) = error.output_error() {
        return write_failure(writer, BoundedDecodeError::Output(output));
    }
    let (code, digits) = match error {
        SelectorError::Parameter(BoundedParameterError::ValueError("embedded null character")) => {
            (0, None)
        }
        SelectorError::Parameter(BoundedParameterError::RuntimeError("internal codec error")) => {
            (1, None)
        }
        SelectorError::Parameter(BoundedParameterError::OverflowError(
            "Python int too large to convert to C ssize_t",
        )) => (2, None),
        SelectorError::IntegerLimit { digits } if digits > 4300 => (3, Some(digits)),
        _ => return write_failure(writer, BoundedDecodeError::RegistryInvariant),
    };
    writer.write_all(&(if digits.is_some() { 15u64 } else { 7 }).to_be_bytes())?;
    writer.write_all(MAGIC)?;
    writer.write_all(&[VERSION, 5, code])?;
    if let Some(digits) = digits {
        writer.write_all(&(digits as u64).to_be_bytes())?;
    }
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn selector_stage_codes_are_closed_and_tied_to_header_envelope() {
        use crate::adapters::http_charset::SelectorError;
        use ssx_source_codecs::BoundedParameterError;
        let errors = [
            (
                SelectorError::Parameter(BoundedParameterError::ValueError(
                    "embedded null character",
                )),
                super::super::SelectorFailure::NullCharacter,
            ),
            (
                SelectorError::Parameter(BoundedParameterError::RuntimeError(
                    "internal codec error",
                )),
                super::super::SelectorFailure::RuntimeCodec,
            ),
            (
                SelectorError::Parameter(BoundedParameterError::OverflowError(
                    "Python int too large to convert to C ssize_t",
                )),
                super::super::SelectorFailure::OverflowCodec,
            ),
            (
                SelectorError::IntegerLimit { digits: 4301 },
                super::super::SelectorFailure::IntegerLimit(4301),
            ),
        ];
        for (error, expected) in errors {
            let mut wire = Vec::new();
            write_selector_failure(&mut wire, error).unwrap();
            let frame = &wire[8..];
            assert!(matches!(
                validate_async(frame, 1, &|| Ok(())).await,
                Err(OperationFailure::Invariant(_))
            ));
            let actual = validate_http_async(frame, 1, 5000, &|| Ok(())).await;
            assert!(matches!(actual, Err(OperationFailure::Selector(ref e)) if *e == expected));
            for n in 0..frame.len() {
                assert!(matches!(
                    validate_http_async(&frame[..n], 1, 5000, &|| Ok(())).await,
                    Err(OperationFailure::Invariant(_))
                ));
            }
            let mut trailing = frame.to_vec();
            trailing.push(0);
            assert!(matches!(
                validate_http_async(&trailing, 1, 5000, &|| Ok(())).await,
                Err(OperationFailure::Invariant(_))
            ));
            let mut unknown = frame.to_vec();
            unknown[6] = 4;
            assert!(matches!(
                validate_http_async(&unknown, 1, 5000, &|| Ok(())).await,
                Err(OperationFailure::Invariant(_))
            ));
            if matches!(expected, super::super::SelectorFailure::IntegerLimit(_)) {
                assert!(matches!(
                    validate_http_async(frame, 1, 4300, &|| Ok(())).await,
                    Err(OperationFailure::Invariant(_))
                ));
                let mut too_small = frame.to_vec();
                too_small[7..].copy_from_slice(&4300u64.to_be_bytes());
                assert!(matches!(
                    validate_http_async(&too_small, 1, 5000, &|| Ok(())).await,
                    Err(OperationFailure::Invariant(_))
                ));
            }
        }
        let mut unknown = Vec::new();
        write_selector_failure(
            &mut unknown,
            SelectorError::Parameter(BoundedParameterError::ValueError(
                "new unreviewed diagnostic",
            )),
        )
        .unwrap();
        assert!(matches!(
            validate_http_async(&unknown[8..], 1, 5000, &|| Ok(())).await,
            Err(OperationFailure::Invariant(_))
        ));
    }
    fn frame() -> Vec<u8> {
        let mut bytes = Vec::new();
        write_success(
            &mut bytes,
            &[0, 0xd800, 0xdc00, 0x10ffff],
            DetectorFlags {
                duckduckgo_challenge: true,
                duckduckgo_legitimate_empty: false,
                google_challenge: true,
            },
        )
        .unwrap();
        let length = u64::from_be_bytes(bytes[..8].try_into().unwrap()) as usize;
        assert_eq!(length, bytes.len() - 8);
        bytes[8..].to_vec()
    }
    #[tokio::test]
    async fn asynchronous_validation_preserves_closed_frame_errors_and_deadline() {
        let valid = frame();
        for length in 0..valid.len() {
            assert!(
                validate_async(&valid[..length], 4, &|| Ok(()))
                    .await
                    .is_err()
            );
        }
        assert_eq!(
            validate_async(&valid, 4, &|| Ok(())).await.unwrap().count,
            4
        );
        let mut invalid = valid.clone();
        invalid.extend([0]);
        assert!(validate_async(&invalid, 4, &|| Ok(())).await.is_err());
        let mut invalid = valid;
        invalid[HEADER..HEADER + 4].copy_from_slice(&0x110000u32.to_be_bytes());
        assert!(validate_async(&invalid, 4, &|| Ok(())).await.is_err());
        assert!(matches!(
            validate_async(&frame(), 4, &|| Err(OperationFailure::Deadline)).await,
            Err(OperationFailure::Deadline)
        ));
    }
    #[test]
    fn complete_frame_preserves_surrogates_and_rejects_every_truncation_and_trailing_byte() {
        let original = frame();
        let decoded = validate(&original, 4, &|| Ok(())).unwrap();
        assert_eq!(decoded.count, 4);
        assert!(decoded.flags.duckduckgo_challenge && decoded.flags.google_challenge);
        assert!(!decoded.flags.duckduckgo_legitimate_empty);
        assert_eq!(
            decoded
                .bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| u32::from_be_bytes(*b))
                .collect::<Vec<_>>(),
            [0, 0xd800, 0xdc00, 0x10ffff]
        );
        for at in 0..original.len() {
            assert!(
                matches!(
                    validate(&original[..at], 4, &|| Ok(())),
                    Err(OperationFailure::Invariant(_))
                ),
                "truncation {at}"
            );
        }
        let mut trailing = original.clone();
        trailing.push(0);
        assert!(matches!(
            validate(&trailing, 4, &|| Ok(())),
            Err(OperationFailure::Invariant(_))
        ));
        assert!(matches!(
            validate(&original, 3, &|| Ok(())),
            Err(OperationFailure::Invariant(_))
        ));
        for (at, byte) in [(0, b'X'), (4, 2), (5, 5), (6, 8)] {
            let mut bad = original.clone();
            bad[at] = byte;
            assert!(matches!(
                validate(&bad, 4, &|| Ok(())),
                Err(OperationFailure::Invariant(_))
            ));
        }
        let mut bad = original.clone();
        bad[7..15].copy_from_slice(&u64::MAX.to_be_bytes());
        assert!(matches!(
            validate(&bad, 4, &|| Ok(())),
            Err(OperationFailure::Invariant(_))
        ));
        let mut bad = original;
        bad[HEADER..HEADER + 4].copy_from_slice(&0x110000u32.to_be_bytes());
        assert!(matches!(
            validate(&bad, 4, &|| Ok(())),
            Err(OperationFailure::Invariant(_))
        ));
    }
    #[test]
    fn closed_source_diagnostics_resource_deadline_and_validation_expiry_stay_distinct() {
        for (index, message) in CODEC_ERRORS.iter().enumerate() {
            let mut frame = Vec::new();
            write_failure(&mut frame, BoundedDecodeError::Codec(message)).unwrap();
            assert_eq!(frame[14], index as u8);
            assert!(
                matches!(validate(&frame[8..], 0, &|| Ok(())), Err(OperationFailure::Codec(value)) if value == *message)
            );
        }
        for (failure, expected) in [
            (
                BoundedDecodeError::Output(BoundedTextError::Resource(
                    ssx_owned_budget::Error::Exhausted,
                )),
                OperationFailure::Resource,
            ),
            (
                BoundedDecodeError::Output(BoundedTextError::Deadline),
                OperationFailure::Deadline,
            ),
            (
                BoundedDecodeError::RegistryInvariant,
                OperationFailure::Invariant("HTML helper decoder invariant"),
            ),
        ] {
            let mut frame = Vec::new();
            write_failure(&mut frame, failure).unwrap();
            assert_eq!(validate(&frame[8..], 0, &|| Ok(())).err(), Some(expected));
        }
        let mut invalid = b"A05D\x01\x02\xff".to_vec();
        assert!(matches!(
            validate(&invalid, 0, &|| Ok(())),
            Err(OperationFailure::Invariant(_))
        ));
        invalid[6] = 0;
        invalid.push(0);
        assert!(matches!(
            validate(&invalid, 0, &|| Ok(())),
            Err(OperationFailure::Invariant(_))
        ));
        let calls = std::cell::Cell::new(0);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() == 3 {
                Err(OperationFailure::Deadline)
            } else {
                Ok(())
            }
        };
        assert!(matches!(
            validate(&frame(), 4, &check),
            Err(OperationFailure::Deadline)
        ));
    }
}
