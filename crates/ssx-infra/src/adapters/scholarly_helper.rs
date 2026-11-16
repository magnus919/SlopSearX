//! Same-process selector/body/Atom path. Parent mirrors this allowance and owns
//! absolute deadline/kill/reap; no second worker or independent codec quota.
use super::{
    arxiv_expat_wire::{self as atom_wire, MEMORY, Reply},
    http_charset::{self, SelectorError},
    scholarly_frame::{self, OperationView},
    scholarly_reply::{self, SourceClass, Stage, View},
};
use ssx_owned_budget::{Budget, Charge, OwnedString};
use ssx_source_codecs::{BoundedDecodeError, BoundedTextError};
use std::{
    fmt::{self, Write as _},
    io::{self, Read, Write},
};
const MAX_BODY: usize = 32 * 1024 * 1024;
const MAX_TEXT: usize = 128 * 1024 * 1024;
#[derive(Debug)]
enum Error {
    Resource(Stage),
    Frame(scholarly_frame::Error),
    Selector(SelectorError),
    Body(BoundedDecodeError),
    Surrogate {
        start: usize,
        end: usize,
        point: u32,
    },
    Defect(Stage),
}
struct Atom {
    reply: Reply,
    _charge: Charge,
}
fn scalar_text(points: &[u32], budget: &Budget) -> Result<OwnedString, Error> {
    if points.len() > MAX_BODY {
        return Err(Error::Resource(Stage::Utf8));
    }
    let mut bytes = 0usize;
    for (index, &point) in points.iter().enumerate() {
        let Some(c) = char::from_u32(point) else {
            if !(0xd800..=0xdfff).contains(&point) {
                return Err(Error::Defect(Stage::Utf8));
            }
            let end = index
                + points[index..]
                    .iter()
                    .take_while(|&&p| (0xd800..=0xdfff).contains(&p))
                    .count();
            return Err(Error::Surrogate {
                start: index,
                end,
                point,
            });
        };
        bytes = bytes
            .checked_add(c.len_utf8())
            .filter(|n| *n <= MAX_TEXT)
            .ok_or(Error::Resource(Stage::Utf8))?;
    }
    let mut text = OwnedString::new(budget).map_err(|_| Error::Resource(Stage::Utf8))?;
    text.try_reserve_exact(bytes)
        .map_err(|_| Error::Resource(Stage::Utf8))?;
    for &point in points {
        text.try_push(char::from_u32(point).ok_or(Error::Defect(Stage::Utf8))?)
            .map_err(|_| Error::Resource(Stage::Utf8))?;
    }
    Ok(text)
}
fn process(reader: &mut impl Read, budget: &Budget) -> Result<Atom, Error> {
    let input = scholarly_frame::read_owned(reader, budget).map_err(Error::Frame)?;
    let view = input.view().map_err(Error::Frame)?;
    if !matches!(view.operation, OperationView::Atom) {
        return Err(Error::Defect(Stage::Frame));
    }
    if view.body.len() > MAX_BODY {
        return Err(Error::Resource(Stage::Body));
    }
    // View construction validates every immutable header boundary before this
    // iterator is borrowed. A second pair vector would duplicate descriptor cost.
    let headers = view.headers().map(|header| {
        let header = header.expect("validated immutable scholarly frame");
        (header.name, header.value)
    });
    let codec =
        http_charset::select_http_encoding_bounded(headers, view.body.is_empty(), budget, &|| {
            Ok(())
        })
        .map_err(Error::Selector)?;
    let decoded = ssx_source_codecs::decode_http_bounded(view.body, codec, budget, &|| Ok(()))
        .map_err(Error::Body)?;
    let text = scalar_text(decoded.codepoints(), budget)?;
    drop(decoded);
    // The unchanged native+retained quota is reserved from the SAME allowance.
    // Its C/Rust allocator ledger covers their overlap before parse returns.
    let mut charge = budget
        .reserve(MEMORY)
        .map_err(|_| Error::Resource(Stage::Xml))?;
    let reply = match ssx_source_xml::parse(text.as_str(), MEMORY) {
        Ok(parsed) => {
            if parsed.retained_capacity > MEMORY || parsed.tracked_peak > MEMORY {
                return Err(Error::Defect(Stage::Xml));
            }
            charge
                .reduce_to(parsed.retained_capacity)
                .map_err(|_| Error::Defect(Stage::Xml))?;
            Reply::Entries(parsed.entries)
        }
        Err(ssx_source_xml::Error::Syntax) => {
            charge.reduce_to(0).map_err(|_| Error::Defect(Stage::Xml))?;
            Reply::Syntax
        }
        Err(ssx_source_xml::Error::NativeMemory | ssx_source_xml::Error::OutputMemory) => {
            return Err(Error::Resource(Stage::Xml));
        }
        Err(_) => return Err(Error::Defect(Stage::Xml)),
    };
    drop(text);
    drop(input);
    Ok(Atom {
        reply,
        _charge: charge,
    })
}
/// Fixed512-byte stack diagnostic covers fixed source messages and at most
/// three usize decimal offsets; no diagnostic allocation can obscure Resource.
struct Diagnostic {
    bytes: [u8; 512],
    length: usize,
}
impl Diagnostic {
    fn new() -> Self {
        Self {
            bytes: [0; 512],
            length: 0,
        }
    }
    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.length]).expect("formatted UTF8")
    }
}
impl fmt::Write for Diagnostic {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self
            .length
            .checked_add(text.len())
            .filter(|&n| n <= self.bytes.len())
            .ok_or(fmt::Error)?;
        self.bytes[self.length..end].copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}
fn output_failure(stage: Stage, error: BoundedTextError) -> View<'static> {
    match error {
        BoundedTextError::Resource(_) => View::Resource(stage),
        BoundedTextError::Deadline => View::Deadline(stage),
        _ => View::Defect(stage),
    }
}
fn failure<'a>(error: &Error, message: &'a mut Diagnostic) -> View<'a> {
    match error {
        Error::Resource(stage) => View::Resource(*stage),
        Error::Defect(stage) => View::Defect(*stage),
        Error::Frame(scholarly_frame::Error::Resource) => View::Resource(Stage::Frame),
        Error::Frame(scholarly_frame::Error::Deadline) => View::Deadline(Stage::Frame),
        Error::Frame(_) => View::Defect(Stage::Frame),
        Error::Selector(error) => {
            if let Some(output) = error.output_error() {
                return output_failure(Stage::Selector, output);
            }
            let class = match error.source_exception() {
                Some("ValueError") => SourceClass::ValueError,
                Some("RuntimeError") => SourceClass::RuntimeError,
                Some("OverflowError") => SourceClass::OverflowError,
                _ => return View::Defect(Stage::Selector),
            };
            if write!(message, "{error}").is_err() {
                return View::Defect(Stage::Selector);
            }
            View::Source {
                stage: Stage::Selector,
                class,
                message: message.as_str(),
            }
        }
        Error::Body(BoundedDecodeError::Output(error)) => output_failure(Stage::Body, *error),
        Error::Body(BoundedDecodeError::RegistryInvariant) => View::Defect(Stage::Body),
        Error::Body(BoundedDecodeError::Codec(text)) => View::Source {
            stage: Stage::Body,
            class: SourceClass::Codec,
            message: text,
        },
        Error::Surrogate { start, end, point } => {
            let result = if *end == start + 1 {
                write!(
                    message,
                    "'utf-8' codec can't encode character '\\u{point:04x}' in position {start}: surrogates not allowed"
                )
            } else {
                write!(
                    message,
                    "'utf-8' codec can't encode characters in position {start}-{}: surrogates not allowed",
                    end - 1
                )
            };
            if result.is_err() {
                return View::Defect(Stage::Utf8);
            }
            View::Source {
                stage: Stage::Utf8,
                class: SourceClass::UnicodeEncodeError,
                message: message.as_str(),
            }
        }
    }
}
fn finish_atom(writer: &mut impl Write, atom: Atom, budget: &Budget) -> io::Result<()> {
    let output_charge = match budget.reserve(atom_wire::OUTPUT) {
        Ok(charge) => charge,
        Err(_) => return scholarly_reply::failure(writer, View::Resource(Stage::Reply)),
    };
    let output = match atom_wire::encode(&atom.reply) {
        Ok(output) => output,
        Err(_) => return scholarly_reply::failure(writer, View::Resource(Stage::Reply)),
    };
    drop(atom);
    let result = scholarly_reply::atom(writer, &output);
    drop(output);
    drop(output_charge);
    result
}
/// Entire frame validation precedes selection. Static scratch drains only a
/// finite parent-owned frame on early refusal, avoiding BrokenPipe precedence.
/// The parent supervisor remains authoritative if an input pipe never closes.
/// Returned requested-capacity statistics are observational; not an RSS claim.
pub(super) fn worker(
    reader: &mut impl Read,
    writer: &mut impl Write,
    allowance: usize,
) -> io::Result<ssx_owned_budget::Snapshot> {
    let budget =
        Budget::new(allowance).map_err(|_| io::Error::other("invalid scholarly allowance"))?;
    let result = process(reader, &budget);
    match result {
        Ok(atom) => finish_atom(writer, atom, &budget)?,
        Err(error) => {
            let mut scratch = [0; 4096];
            let mut drained = 0usize;
            loop {
                let count = reader.read(&mut scratch)?;
                if count == 0 {
                    break;
                }
                drained = drained
                    .checked_add(count)
                    .ok_or_else(|| io::Error::other("scholarly drain bound"))?;
            }
            let mut diagnostic = Diagnostic::new();
            scholarly_reply::failure(writer, failure(&error, &mut diagnostic))?;
        }
    }
    budget.snapshot().map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    const FEED:&[u8]=b"<feed xmlns='http://www.w3.org/2005/Atom'><entry><id>https://arxiv.org/abs/1</id><title>Paper</title></entry></feed>";
    async fn input(headers: &[(&[u8], &[u8])], body: &[u8]) -> scholarly_frame::Frame {
        let budget = Budget::new(1024 * 1024).unwrap();
        scholarly_frame::encode(
            scholarly_frame::Operation::Atom,
            headers
                .iter()
                .map(|(name, value)| scholarly_frame::Header { name, value }),
            body,
            &budget,
            Instant::now() + Duration::from_secs(30),
        )
        .await
        .unwrap()
    }
    fn payload(output: &[u8]) -> &[u8] {
        let n = u64::from_be_bytes(output[..8].try_into().unwrap()) as usize;
        assert_eq!(n, output.len() - 8);
        &output[8..]
    }
    #[tokio::test]
    async fn same_budget_selects_decodes_parses_and_serializes_with_overlap() {
        let frame = input(
            &[(b"content-type", b"text/plain; charset*=us-ascii''utf-8")],
            FEED,
        )
        .await;
        let mut output = Vec::new();
        let stats = worker(
            &mut io::Cursor::new(frame.as_ref()),
            &mut output,
            scholarly_frame::CANDIDATE_CHILD_BYTES,
        )
        .unwrap();
        assert_eq!(stats.used, 0);
        assert!(stats.peak >= MEMORY);
        assert!(stats.peak <= stats.limit);
        let View::Atom(bytes) = scholarly_reply::view(payload(&output)).unwrap() else {
            panic!("not Atom")
        };
        let Reply::Entries(entries) =
            atom_wire::decode(bytes, tokio::time::Instant::now() + Duration::from_secs(30))
                .await
                .unwrap()
        else {
            panic!("not entries")
        };
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].fields[0].as_deref(), Some("Paper"));
        // Native/retained storage remains owned while reply storage is reserved.
        let budget = Budget::new(scholarly_frame::CANDIDATE_CHILD_BYTES).unwrap();
        let atom = process(&mut io::Cursor::new(frame.as_ref()), &budget).unwrap();
        let retained = atom._charge.bytes();
        assert!(retained > 0);
        assert_eq!(budget.snapshot().unwrap().used, retained);
        let output_charge = budget.reserve(atom_wire::OUTPUT).unwrap();
        let encoded = atom_wire::encode(&atom.reply).unwrap();
        assert!(encoded.len() <= output_charge.bytes());
        assert_eq!(
            budget.snapshot().unwrap().used,
            retained + atom_wire::OUTPUT
        );
        drop(encoded);
        drop(output_charge);
        drop(atom);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[tokio::test]
    async fn closed_stage_errors_and_complete_frame_validation_precedence() {
        for (header, body, stage, class, message) in [
            (
                b"text/plain; charset=\0".as_slice(),
                FEED,
                Stage::Selector,
                SourceClass::ValueError,
                "embedded null character",
            ),
            (
                b"text/plain; charset=base64".as_slice(),
                FEED,
                Stage::Body,
                SourceClass::Codec,
                "",
            ),
            (
                b"text/plain; charset=unicode_escape".as_slice(),
                b"\\ud800".as_slice(),
                Stage::Utf8,
                SourceClass::UnicodeEncodeError,
                "'utf-8' codec can't encode character '\\ud800' in position 0: surrogates not allowed",
            ),
            (
                b"text/plain; charset=unicode_escape".as_slice(),
                b"a\\ud800\\udfff".as_slice(),
                Stage::Utf8,
                SourceClass::UnicodeEncodeError,
                "'utf-8' codec can't encode characters in position 1-2: surrogates not allowed",
            ),
        ] {
            let frame = input(&[(b"content-type", header)], body).await;
            let mut output = Vec::new();
            let stats = worker(
                &mut io::Cursor::new(frame.as_ref()),
                &mut output,
                scholarly_frame::CANDIDATE_CHILD_BYTES,
            )
            .unwrap();
            assert_eq!(stats.used, 0);
            assert_eq!(
                scholarly_reply::view(payload(&output)).unwrap(),
                View::Source {
                    stage,
                    class,
                    message
                }
            );
        }
        let frame = input(&[(b"content-type", b"text/plain; charset=\0")], FEED).await;
        let mut invalid = frame.as_ref().to_vec();
        invalid.push(1);
        let mut output = Vec::new();
        worker(
            &mut io::Cursor::new(invalid),
            &mut output,
            scholarly_frame::CANDIDATE_CHILD_BYTES,
        )
        .unwrap();
        assert_eq!(
            scholarly_reply::view(payload(&output)).unwrap(),
            View::Defect(Stage::Frame)
        );
        let mut output = Vec::new();
        let stats = worker(&mut io::Cursor::new(frame.as_ref()), &mut output, 1).unwrap();
        assert_eq!(stats.used, 0);
        assert_eq!(
            scholarly_reply::view(payload(&output)).unwrap(),
            View::Resource(Stage::Frame)
        );
    }
    #[tokio::test]
    async fn empty_body_bypasses_selector_and_xml_syntax_is_source_empty() {
        for body in [b"".as_slice(), b"<notxml".as_slice()] {
            let frame = input(
                &[(
                    b"content-type",
                    if body.is_empty() {
                        b"charset=\0"
                    } else {
                        b"charset=utf8"
                    },
                )],
                body,
            )
            .await;
            let mut output = Vec::new();
            let stats = worker(
                &mut io::Cursor::new(frame.as_ref()),
                &mut output,
                scholarly_frame::CANDIDATE_CHILD_BYTES,
            )
            .unwrap();
            assert_eq!(stats.used, 0);
            let View::Atom(bytes) = scholarly_reply::view(payload(&output)).unwrap() else {
                panic!("not Atom")
            };
            assert_eq!(
                atom_wire::decode(bytes, tokio::time::Instant::now() + Duration::from_secs(30))
                    .await
                    .unwrap(),
                Reply::Syntax
            );
        }
    }
    #[tokio::test]
    async fn actual_source_unicode_boundaries_are_exact_without_surrogate_sanitation() {
        use base64::Engine as _;
        let corpus: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a06/source-atom-decode.json"
        ))
        .unwrap();
        for row in corpus["records"].as_array().unwrap() {
            let body = base64::engine::general_purpose::STANDARD
                .decode(row["responses"][0]["body_b64"].as_str().unwrap())
                .unwrap();
            let frame = input(
                &[(b"content-type", b"text/plain; charset=unicode_escape")],
                &body,
            )
            .await;
            let mut output = Vec::new();
            let stats = worker(
                &mut io::Cursor::new(frame.as_ref()),
                &mut output,
                scholarly_frame::CANDIDATE_CHILD_BYTES,
            )
            .unwrap();
            assert_eq!(stats.used, 0);
            let view = scholarly_reply::view(payload(&output)).unwrap();
            if row["output"]["status"] == "error" {
                assert_eq!(
                    view,
                    View::Source {
                        stage: Stage::Utf8,
                        class: SourceClass::UnicodeEncodeError,
                        message: row["output"]["error_message"].as_str().unwrap()
                    },
                    "{}",
                    row["id"]
                );
            } else {
                let View::Atom(bytes) = view else {
                    panic!("not Atom")
                };
                assert_eq!(
                    atom_wire::decode(bytes, tokio::time::Instant::now() + Duration::from_secs(30))
                        .await
                        .unwrap(),
                    Reply::Syntax
                );
            }
        }
    }
}
