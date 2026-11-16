//! Expat2.7.4 XML_GE accounting for the ElementTree UTF8 Unicode-input path.
//! See xmlparse.c accountingDiffTolerated, doContent, appendAttributeValue.
use super::arxiv::{Failure, checkpoint};
use quick_xml::{Reader, events::Event};
use std::{collections::BTreeMap, time::Instant};
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Direct,
    Indirect,
    None,
}
#[derive(Default)]
pub(super) struct Accounting {
    pub direct: u64,
    pub indirect: u64,
}
impl Accounting {
    pub fn add(&mut self, mode: Mode, bytes: usize, enforce: bool) -> Result<(), Failure> {
        let target = match mode {
            Mode::None => return Ok(()),
            Mode::Direct => &mut self.direct,
            Mode::Indirect => &mut self.indirect,
        };
        *target = target
            .checked_add(u64::try_from(bytes).map_err(|_| Failure::Syntax)?)
            .ok_or(Failure::Syntax)?;
        let total = self
            .direct
            .checked_add(self.indirect)
            .ok_or(Failure::Syntax)?;
        let factor = if self.direct != 0 {
            total as f32 / self.direct as f32
        } else {
            self.indirect.checked_add(22).ok_or(Failure::Syntax)? as f32 / 22f32
        };
        if enforce && total >= 8_388_608 && factor > 100f32 {
            return Err(Failure::Syntax);
        }
        if self.indirect > super::arxiv::MAX_INPUT_BYTES as u64 {
            return Err(Failure::Limit("entity work byte (134217728)"));
        }
        Ok(())
    }
}
fn normalized(value: &str) -> bool {
    value.contains(['&', '\t', '\r', '\n'])
        || value.starts_with(' ')
        || value.ends_with(' ')
        || value.contains("  ")
}
#[derive(Clone, Copy)]
pub(super) enum Context {
    Content,
    Attribute,
}
struct Frame<'a> {
    text: std::borrow::Cow<'a, str>,
    pos: usize,
    mode: Mode,
    context: Context,
    entity: Option<String>,
    names: Vec<String>,
}
impl<'a> Frame<'a> {
    fn new(
        text: impl Into<std::borrow::Cow<'a, str>>,
        mode: Mode,
        context: Context,
        entity: Option<String>,
    ) -> Self {
        Self {
            text: text.into(),
            pos: 0,
            mode,
            context,
            entity,
            names: Vec::new(),
        }
    }
}
/// Iterative entity stack, including attribute reprocessing and predefined
/// one-byte expansions. Direct bytes are charged when consumed, not by body size.
pub(super) async fn fragment(
    text: &str,
    entities: &BTreeMap<String, Option<String>>,
    mode: Mode,
    context: Context,
    account: &mut Accounting,
    deadline: Option<Instant>,
) -> Result<(), Failure> {
    let mut stack = vec![Frame::new(text, mode, context, None)];
    let mut work = 0usize;
    while !stack.is_empty() {
        if work.is_multiple_of(128) {
            checkpoint(deadline).await?;
        }
        work += 1;
        let f = stack.last_mut().unwrap();
        if f.pos == f.text.len() {
            if !f.names.is_empty() {
                return Err(Failure::Syntax);
            }
            stack.pop();
            continue;
        }
        let tail = &f.text[f.pos..];
        let (reference, attrs) = match f.context {
            Context::Attribute => {
                if tail.starts_with('&') {
                    let end = tail.find(';').ok_or(Failure::Syntax)?;
                    account.add(f.mode, end + 1, true)?;
                    f.pos += end + 1;
                    (Some(tail[1..end].to_owned()), Vec::new())
                } else {
                    let end = tail
                        .find(['&', '\r', '\n', '\t', ' '])
                        .filter(|i| *i > 0)
                        .unwrap_or_else(|| tail.chars().next().unwrap().len_utf8());
                    let end = if tail.starts_with("\r\n") { 2 } else { end };
                    account.add(f.mode, end, true)?;
                    f.pos += end;
                    (None, Vec::new())
                }
            }
            Context::Content => {
                if tail.starts_with('\u{feff}') {
                    account.add(f.mode, 3, true)?;
                    f.pos += 3;
                    continue;
                }
                let mut reader = Reader::from_str(tail);
                reader.config_mut().allow_unmatched_ends = true;
                let event = reader.read_event().map_err(|_| Failure::Syntax)?;
                let len = reader.buffer_position() as usize;
                match &event {
                    Event::Text(_) => text_tokens(&tail[..len], f.mode, account, deadline).await?,
                    Event::CData(_) => {
                        account.add(f.mode, 9, true)?;
                        text_tokens(&tail[9..len - 3], f.mode, account, deadline).await?;
                        account.add(f.mode, 3, true)?;
                    }
                    Event::Eof => {}
                    _ => account.add(f.mode, len, true)?,
                }
                f.pos += len;
                match event {
                    Event::GeneralRef(r) => (Some(r.as_ref().to_owned()), Vec::new()),
                    Event::Start(s) | Event::Empty(s) => {
                        let empty = tail[..len].ends_with("/>");
                        if !empty {
                            f.names.push(s.name().as_ref().to_owned());
                        }
                        let mut attrs = Vec::new();
                        for a in s.attributes() {
                            let a = a.map_err(|_| Failure::Syntax)?;
                            if normalized(&a.value) {
                                attrs.push(a.value.into_owned());
                            }
                        }
                        (None, attrs)
                    }
                    Event::End(s) => {
                        if f.names.pop().as_deref() != Some(s.name().as_ref()) {
                            return Err(Failure::Syntax);
                        }
                        (None, Vec::new())
                    }
                    Event::Eof => return Err(Failure::Syntax),
                    Event::Decl(_) | Event::DocType(_) => return Err(Failure::Syntax),
                    _ => (None, Vec::new()),
                }
            }
        };
        let mode = stack.last().unwrap().mode;
        for attr in attrs.into_iter().rev() {
            stack.push(Frame::new(attr, mode, Context::Attribute, None));
        }
        if let Some(name) = reference {
            if matches!(name.as_str(), "amp" | "lt" | "gt" | "apos" | "quot") {
                account.add(Mode::Indirect, 1, false)?;
            } else if !name.starts_with('#') {
                let value = entities
                    .get(&name)
                    .and_then(Option::as_deref)
                    .ok_or(Failure::Syntax)?;
                if stack
                    .iter()
                    .any(|f| f.entity.as_deref() == Some(name.as_str()))
                {
                    return Err(Failure::Syntax);
                }
                let context = stack.last().unwrap().context;
                stack.push(Frame::new(value, Mode::Indirect, context, Some(name)));
            }
        }
    }
    Ok(())
}
/// Charge an ordinary source token and its normalization work. This function is
/// also used for top-level markup tokens whose element depth belongs to caller.
pub(super) async fn start_attributes<'a>(
    start: &quick_xml::events::BytesStart<'a>,
    entities: &'a BTreeMap<String, Option<String>>,
    mode: Mode,
    account: &mut Accounting,
    deadline: Option<Instant>,
) -> Result<(), Failure> {
    for attr in start.attributes() {
        let attr = attr.map_err(|_| Failure::Syntax)?;
        if normalized(&attr.value) {
            fragment(
                attr.value.as_ref(),
                entities,
                mode,
                Context::Attribute,
                account,
                deadline,
            )
            .await?;
        }
    }
    Ok(())
}

pub(super) async fn text_tokens(
    text: &str,
    mode: Mode,
    account: &mut Accounting,
    deadline: Option<Instant>,
) -> Result<(), Failure> {
    let mut at = 0;
    let mut pos = 0;
    let mut work = 0usize;
    while pos < text.len() {
        if work.is_multiple_of(4096) {
            checkpoint(deadline).await?;
        }
        work += 1;
        let c = text[pos..].chars().next().unwrap();
        if c == '\r' || c == '\n' {
            if pos > at {
                account.add(mode, pos - at, true)?;
            }
            let n = if text[pos..].starts_with("\r\n") {
                2
            } else {
                1
            };
            account.add(mode, n, true)?;
            pos += n;
            at = pos;
        } else {
            pos += c.len_utf8();
        }
    }
    if pos > at {
        account.add(mode, pos - at, true)?;
    }
    Ok(())
}
