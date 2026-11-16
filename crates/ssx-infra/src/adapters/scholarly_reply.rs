//! Closed scholarly helper envelope inside ProcessPool's u64-length frame.
use std::io::{self, Write};
pub const PREFIX: usize = 6;
pub const MAX_BYTES: usize = super::arxiv_expat_wire::OUTPUT + PREFIX;
const MAGIC: &[u8; 4] = b"SA01";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Stage {
    Frame = 0,
    Selector = 1,
    Body = 2,
    Utf8 = 3,
    Xml = 4,
    Reply = 5,
}
impl Stage {
    fn parse(n: u8) -> Option<Self> {
        Some(match n {
            0 => Self::Frame,
            1 => Self::Selector,
            2 => Self::Body,
            3 => Self::Utf8,
            4 => Self::Xml,
            5 => Self::Reply,
            _ => return None,
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SourceClass {
    ValueError = 1,
    RuntimeError = 2,
    OverflowError = 3,
    UnicodeEncodeError = 4,
    Codec = 5,
}
impl SourceClass {
    fn parse(n: u8) -> Option<Self> {
        Some(match n {
            1 => Self::ValueError,
            2 => Self::RuntimeError,
            3 => Self::OverflowError,
            4 => Self::UnicodeEncodeError,
            5 => Self::Codec,
            _ => return None,
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View<'a> {
    Atom(&'a [u8]),
    Resource(Stage),
    Deadline(Stage),
    Source {
        stage: Stage,
        class: SourceClass,
        message: &'a str,
    },
    Defect(Stage),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error;
pub fn view(bytes: &[u8]) -> Result<View<'_>, Error> {
    if bytes.len() < PREFIX || bytes.len() > MAX_BYTES || &bytes[..4] != MAGIC {
        return Err(Error);
    }
    let stage = Stage::parse(bytes[5]).ok_or(Error)?;
    Ok(match bytes[4] {
        0 if stage == Stage::Xml => View::Atom(&bytes[PREFIX..]),
        1 if bytes.len() == PREFIX => View::Resource(stage),
        2 if bytes.len() == PREFIX => View::Deadline(stage),
        3 if bytes.len() > PREFIX && bytes.len() <= PREFIX + 1 + 512 => {
            let class = SourceClass::parse(bytes[PREFIX]).ok_or(Error)?;
            if !matches!(
                (stage, class),
                (
                    Stage::Selector,
                    SourceClass::ValueError
                        | SourceClass::RuntimeError
                        | SourceClass::OverflowError
                ) | (Stage::Body, SourceClass::Codec)
                    | (Stage::Utf8, SourceClass::UnicodeEncodeError)
            ) {
                return Err(Error);
            }
            View::Source {
                stage,
                class,
                message: std::str::from_utf8(&bytes[PREFIX + 1..]).map_err(|_| Error)?,
            }
        }
        4 if bytes.len() == PREFIX => View::Defect(stage),
        _ => return Err(Error),
    })
}
fn begin(writer: &mut impl Write, tag: u8, stage: Stage, length: usize) -> io::Result<()> {
    let length = length
        .checked_add(PREFIX)
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(|| io::Error::other("scholarly output overflow"))?;
    writer.write_all(&length.to_be_bytes())?;
    writer.write_all(MAGIC)?;
    writer.write_all(&[tag, stage as u8])
}
pub(super) fn atom(writer: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    if bytes.len() > super::arxiv_expat_wire::OUTPUT {
        return Err(io::Error::other("scholarly output bound"));
    }
    begin(writer, 0, Stage::Xml, bytes.len())?;
    writer.write_all(bytes)?;
    writer.flush()
}
pub(super) fn failure(writer: &mut impl Write, value: View<'_>) -> io::Result<()> {
    match value {
        View::Resource(stage) => begin(writer, 1, stage, 0)?,
        View::Deadline(stage) => begin(writer, 2, stage, 0)?,
        View::Defect(stage) => begin(writer, 4, stage, 0)?,
        View::Source {
            stage,
            class,
            message,
        } => {
            if message.len() > 512 {
                return Err(io::Error::other("scholarly diagnostic bound"));
            }
            begin(writer, 3, stage, message.len() + 1)?;
            writer.write_all(&[class as u8])?;
            writer.write_all(message.as_bytes())?;
        }
        View::Atom(_) => return Err(io::Error::other("invalid scholarly failure")),
    }
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn payload(bytes: &[u8]) -> &[u8] {
        let n = u64::from_be_bytes(bytes[..8].try_into().unwrap()) as usize;
        assert_eq!(n, bytes.len() - 8);
        &bytes[8..]
    }
    #[test]
    fn typed_closed_stages_round_trip_and_reject_wrong_stage_class_or_payload() {
        for value in [
            View::Resource(Stage::Frame),
            View::Deadline(Stage::Selector),
            View::Defect(Stage::Reply),
            View::Source {
                stage: Stage::Selector,
                class: SourceClass::OverflowError,
                message: "overflow",
            },
            View::Source {
                stage: Stage::Body,
                class: SourceClass::Codec,
                message: "",
            },
            View::Source {
                stage: Stage::Utf8,
                class: SourceClass::UnicodeEncodeError,
                message: "surrogates not allowed",
            },
        ] {
            let mut out = Vec::new();
            failure(&mut out, value).unwrap();
            assert_eq!(view(payload(&out)).unwrap(), value);
        }
        for bytes in [
            b"SA01\x01\x00x".as_slice(),
            b"SA01\x02\x06".as_slice(),
            b"SA01\x03\x01\x05wrong".as_slice(),
            b"SA01\x03\x02\x01wrong".as_slice(),
            b"SA01\x03\x03\x04\xff".as_slice(),
            b"SA01\x04\x05x".as_slice(),
            b"SA01\x00\x01".as_slice(),
            b"SA01\xff\x00".as_slice(),
        ] {
            assert!(view(bytes).is_err());
        }
        let mut oversized = b"SA01\x03\x01\x01".to_vec();
        oversized.extend_from_slice(&[b'x'; 513]);
        assert!(view(&oversized).is_err());
    }
}
