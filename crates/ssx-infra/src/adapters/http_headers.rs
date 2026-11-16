//! One source HTTPX named-header reader with bounded sync/async drivers.
//! Inputs stay borrowed; global UTF8/Latin1 selection and ordered duplicate join
//! are shared by helper charset selection and parent redirect handling.
use ssx_owned_budget::{Budget, OwnedString};
use ssx_source_codecs::BoundedTextError;
use std::time::Instant;
const BLOCK: usize = 4096;
#[derive(Debug, PartialEq, Eq)]
pub enum HeaderError {
    Output(BoundedTextError),
    Invariant,
}
impl From<ssx_owned_budget::Error> for HeaderError {
    fn from(error: ssx_owned_budget::Error) -> Self {
        Self::Output(error.into())
    }
}
impl From<BoundedTextError> for HeaderError {
    fn from(error: BoundedTextError) -> Self {
        Self::Output(error)
    }
}
impl std::fmt::Display for HeaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for HeaderError {}
#[derive(Clone, Copy)]
enum Stage {
    Validate,
    Size,
    Decode,
    Done,
}
/// Validate at most BLOCK+3 bytes. Extend an incomplete boundary by exactly its
/// remaining scalar bytes; including a new scalar prefix would reject valid UTF8.
fn utf8_chunk(bytes: &[u8], at: usize) -> Result<(&str, usize), ()> {
    let end = at.saturating_add(BLOCK).min(bytes.len());
    match std::str::from_utf8(&bytes[at..end]) {
        Ok(text) => Ok((text, end)),
        Err(error) if error.error_len().is_none() => {
            let start = at + error.valid_up_to();
            let width = match bytes.get(start).copied() {
                Some(0xc2..=0xdf) => 2,
                Some(0xe0..=0xef) => 3,
                Some(0xf0..=0xf4) => 4,
                _ => return Err(()),
            };
            let end = start
                .checked_add(width)
                .filter(|end| *end <= bytes.len())
                .ok_or(())?;
            std::str::from_utf8(&bytes[at..end])
                .map(|text| (text, end))
                .map_err(|_| ())
        }
        Err(_) => Err(()),
    }
}
struct Reader<'a, I> {
    all: I,
    scan: I,
    name: &'static [u8],
    budget: Budget,
    stage: Stage,
    current: Option<(&'a [u8], &'a [u8])>,
    value_part: bool,
    at: usize,
    utf8: bool,
    size: usize,
    count: usize,
    copied: usize,
    output: Option<OwnedString>,
}
impl<'a, I> Reader<'a, I>
where
    I: Iterator<Item = (&'a [u8], &'a [u8])> + Clone,
{
    fn new(headers: I, name: &'static [u8], budget: &Budget) -> Self {
        Self {
            scan: headers.clone(),
            all: headers,
            name,
            budget: budget.clone(),
            stage: Stage::Validate,
            current: None,
            value_part: false,
            at: 0,
            utf8: true,
            size: 0,
            count: 0,
            copied: 0,
            output: None,
        }
    }
    fn add_size(&mut self, size: usize) -> Result<(), HeaderError> {
        self.size = self
            .size
            .checked_add(size)
            .ok_or(ssx_owned_budget::Error::Overflow)?;
        Ok(())
    }
    /// One bounded chunk or one field advance per call, including empty fields.
    fn step(&mut self) -> Result<bool, HeaderError> {
        match self.stage {
            Stage::Validate => {
                let Some((name, value)) = self.current else {
                    if let Some(header) = self.scan.next() {
                        self.current = Some(header);
                        self.value_part = false;
                        self.at = 0;
                    } else {
                        self.scan = self.all.clone();
                        self.stage = Stage::Size;
                    }
                    return Ok(false);
                };
                let bytes = if self.value_part { value } else { name };
                let end = match utf8_chunk(bytes, self.at) {
                    Ok((_, end)) => end,
                    Err(()) => {
                        self.utf8 = false;
                        bytes.len()
                    }
                };
                self.at = end;
                if end == bytes.len() {
                    if self.value_part {
                        self.current = None;
                    } else {
                        self.value_part = true;
                        self.at = 0;
                    }
                }
            }
            Stage::Size => {
                let Some((_, value)) = self.current else {
                    if let Some(header) = self.scan.next() {
                        // Target names are static ASCII and short. The standard
                        // length check rejects arbitrary long names before scan.
                        if header.0.eq_ignore_ascii_case(self.name) {
                            self.add_size(header.1.len())?;
                            if self.count != 0 {
                                self.add_size(2)?;
                            }
                            self.count = self
                                .count
                                .checked_add(1)
                                .ok_or(ssx_owned_budget::Error::Overflow)?;
                            if !self.utf8 {
                                self.current = Some(header);
                                self.at = 0;
                            }
                        }
                    } else {
                        // Preserve the original selector's empty-owner path.
                        // Zero-capacity backing carries no invented overhead.
                        let mut out = OwnedString::new(&self.budget)?;
                        out.try_reserve_exact(self.size)?;
                        self.output = Some(out);
                        self.scan = self.all.clone();
                        self.stage = Stage::Decode;
                    }
                    return Ok(false);
                };
                let end = self.at.saturating_add(BLOCK).min(value.len());
                let extra = value[self.at..end]
                    .iter()
                    .filter(|byte| **byte >= 128)
                    .count();
                self.add_size(extra)?;
                self.at = end;
                if end == value.len() {
                    self.current = None;
                }
            }
            Stage::Decode => {
                let Some((_, value)) = self.current else {
                    if let Some(header) = self.scan.next() {
                        if header.0.eq_ignore_ascii_case(self.name) {
                            if self.copied != 0 {
                                self.output
                                    .as_mut()
                                    .ok_or(HeaderError::Invariant)?
                                    .try_push_str(", ")?;
                            }
                            self.copied = self
                                .copied
                                .checked_add(1)
                                .ok_or(ssx_owned_budget::Error::Overflow)?;
                            self.current = Some(header);
                            self.at = 0;
                        }
                    } else {
                        self.stage = Stage::Done;
                        return Ok(true);
                    }
                    return Ok(false);
                };
                let output = self.output.as_mut().ok_or(HeaderError::Invariant)?;
                let end = if self.utf8 {
                    let (text, end) =
                        utf8_chunk(value, self.at).map_err(|_| HeaderError::Invariant)?;
                    output.try_push_str(text)?;
                    end
                } else {
                    let end = self.at.saturating_add(BLOCK).min(value.len());
                    for &byte in &value[self.at..end] {
                        output.try_push(char::from(byte))?;
                    }
                    end
                };
                self.at = end;
                if end == value.len() {
                    self.current = None;
                }
            }
            Stage::Done => return Ok(true),
        }
        Ok(false)
    }
    fn finish(mut self) -> Result<Option<OwnedString>, HeaderError> {
        if !matches!(self.stage, Stage::Done) || self.copied != self.count {
            return Err(HeaderError::Invariant);
        }
        let output = self.output.take().ok_or(HeaderError::Invariant)?;
        Ok((self.count != 0).then_some(output))
    }
}
/// Driver for the already supervised helper; checkpoint errors stay terminal.
pub fn get_bounded<'a, I>(
    headers: I,
    name: &'static [u8],
    budget: &Budget,
    check: &dyn Fn() -> Result<(), BoundedTextError>,
) -> Result<Option<OwnedString>, HeaderError>
where
    I: Iterator<Item = (&'a [u8], &'a [u8])> + Clone,
{
    let mut reader = Reader::new(headers, name, budget);
    loop {
        check()?;
        if reader.step()? {
            return reader.finish();
        }
    }
}
/// Parent driver yields between bounded chunks, using the same absolute caller
/// deadline and Budget. Dropping this future releases any reserved output.
pub async fn get_cooperative<'a, I>(
    headers: I,
    name: &'static [u8],
    budget: &Budget,
    deadline: Instant,
) -> Result<Option<OwnedString>, HeaderError>
where
    I: Iterator<Item = (&'a [u8], &'a [u8])> + Clone,
{
    let mut reader = Reader::new(headers, name, budget);
    loop {
        if Instant::now() >= deadline {
            return Err(BoundedTextError::Deadline.into());
        }
        tokio::task::yield_now().await;
        if Instant::now() >= deadline {
            return Err(BoundedTextError::Deadline.into());
        }
        if reader.step()? {
            return reader.finish();
        }
    }
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
    async fn both_drivers_match_all_frozen_httpx_named_header_values() {
        let source: ssx_json::Value = ssx_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a06/source-charset.json"
        ))
        .unwrap();
        let mut count = 0;
        for row in source["records"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["engine"].as_str() == Some("arxiv"))
        {
            let pairs = row["responses"][0]["headers_b64"]
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
                .collect::<Vec<_>>();
            let input = || {
                pairs
                    .iter()
                    .map(|(name, value)| (name.as_slice(), value.as_slice()))
            };
            let budget = Budget::new(1024 * 1024).unwrap();
            let expected = row["selector"]["content_type"].as_str();
            let actual = get_bounded(input(), b"content-type", &budget, &|| Ok(())).unwrap();
            assert_eq!(
                actual.as_ref().map(OwnedString::as_str),
                expected,
                "{}",
                row["id"]
            );
            drop(actual);
            assert_eq!(budget.snapshot().unwrap().used, 0);
            let actual = get_cooperative(
                input(),
                b"content-type",
                &budget,
                Instant::now() + Duration::from_secs(5),
            )
            .await
            .unwrap();
            assert_eq!(
                actual.as_ref().map(OwnedString::as_str),
                expected,
                "{}",
                row["id"]
            );
            drop(actual);
            assert_eq!(budget.snapshot().unwrap().used, 0);
            count += 1;
        }
        assert_eq!(count, 45);
    }
    #[tokio::test]
    async fn global_decode_duplicate_empty_and_utf8_chunk_boundaries_are_exact() {
        for width in 2..=4 {
            for offset in BLOCK - 4..=BLOCK + 2 {
                let ch = match width {
                    2 => 'é',
                    3 => '☃',
                    _ => '𐀀',
                };
                let text = format!("{}{}end", "a".repeat(offset), ch);
                let pairs = [(b"location".as_slice(), text.as_bytes())];
                let budget = Budget::new(65536).unwrap();
                let value = get_cooperative(
                    pairs.into_iter(),
                    b"location",
                    &budget,
                    Instant::now() + Duration::from_secs(5),
                )
                .await
                .unwrap()
                .unwrap();
                assert_eq!(value.as_str(), text);
                drop(value);
                assert_eq!(budget.snapshot().unwrap().used, 0);
            }
        }
        let pairs = [
            (b"LoCaTiOn".as_slice(), "é".as_bytes()),
            (b"location".as_slice(), b"".as_slice()),
            (b"other".as_slice(), b"\xff".as_slice()),
            (b"location".as_slice(), b"z".as_slice()),
        ];
        let budget = Budget::new(128).unwrap();
        let value = get_bounded(pairs.into_iter(), b"location", &budget, &|| Ok(()))
            .unwrap()
            .unwrap();
        assert_eq!(value.as_str(), "Ã©, , z");
        drop(value);
        assert_eq!(budget.snapshot().unwrap().used, 0);
        let empty = [(b"location".as_slice(), b"".as_slice())];
        assert!(
            get_bounded(empty.into_iter(), b"location", &budget, &|| Ok(()))
                .unwrap()
                .unwrap()
                .is_empty()
        );
        let scarce = Budget::new(1).unwrap();
        assert!(
            get_bounded(std::iter::empty(), b"location", &scarce, &|| Ok(()))
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            get_bounded(empty.into_iter(), b"location", &scarce, &|| Err(
                BoundedTextError::Deadline
            )),
            Err(HeaderError::Output(BoundedTextError::Deadline))
        ));
        assert_eq!(
            super::super::http_charset::select_http_encoding_bounded(
                empty.into_iter(),
                true,
                &scarce,
                &|| Err(BoundedTextError::Deadline)
            ),
            Ok("utf-8")
        );
    }
    #[test]
    fn output_is_precharged_and_failures_release_exact_capacity() {
        let value = [0xff; 4097];
        let pairs = [(b"location".as_slice(), value.as_slice())];
        let successful = Budget::new(65536).unwrap();
        let output = get_bounded(pairs.into_iter(), b"location", &successful, &|| Ok(()))
            .unwrap()
            .unwrap();
        let peak = successful.snapshot().unwrap().peak;
        assert_eq!(output.len(), value.len() * 2);
        assert_eq!(output.capacity(), output.len());
        drop(output);
        assert_eq!(successful.snapshot().unwrap().used, 0);
        for limit in [1, 24, 25, peak - 1] {
            let budget = Budget::new(limit).unwrap();
            assert!(matches!(
                get_bounded(pairs.into_iter(), b"location", &budget, &|| Ok(())),
                Err(HeaderError::Output(BoundedTextError::Resource(_)))
            ));
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
        let exact = Budget::new(peak).unwrap();
        let output = get_bounded(pairs.into_iter(), b"location", &exact, &|| Ok(()))
            .unwrap()
            .unwrap();
        drop(output);
        assert_eq!(exact.snapshot().unwrap().used, 0);
    }
    #[tokio::test]
    async fn charged_empty_duplicate_copy_has_repeated_pending_then_cancellation_or_real_deadline()
    {
        for expire in [false, true] {
            let pairs = vec![(b"location".as_slice(), b"".as_slice()); 2048];
            let budget = Budget::new(65536).unwrap();
            let deadline = Instant::now() + Duration::from_secs(if expire { 2 } else { 30 });
            let mut future = Box::pin(get_cooperative(
                pairs.iter().copied(),
                b"location",
                &budget,
                deadline,
            ));
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
            assert!(budget.snapshot().unwrap().used > 0);
            if expire {
                tokio::time::sleep_until(deadline.into()).await;
                assert!(matches!(
                    future.await,
                    Err(HeaderError::Output(BoundedTextError::Deadline))
                ));
            } else {
                drop(future)
            }
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
    }
}
