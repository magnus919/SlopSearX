//! Checked v2 scholarly helper frame; no factory or charset-selector activation.
use super::scholarly_config::Scalar;
use ssx_owned_budget::{Budget, OwnedVec};
use std::{io::Read, time::Instant};

const MAGIC: &[u8; 4] = b"SSXS";
const VERSION: u8 = 2;
pub const HEADER_BYTES: usize = 14;
/// Candidate child backing-storage allowance, distinct from the unchanged
/// native Expat+retained64MiB subquota. Parent must reserve it through reap.
pub const CANDIDATE_CHILD_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Resource,
    Deadline,
    Magic,
    Version,
    Operation,
    Truncated,
    Overflow,
    Boolean,
    Scalar,
    Utf8,
    Date,
    Trailing,
    Io(std::io::ErrorKind),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header<'a> {
    pub name: &'a [u8],
    pub value: &'a [u8],
}
#[derive(Clone, Copy)]
pub enum Operation<'a> {
    Atom,
    SemanticScholar,
    OpenAlex {
        max_results: &'a Scalar,
        date_from: Option<&'a str>,
        date_to: Option<&'a str>,
    },
}
#[derive(Clone, Copy, Debug)]
pub enum ScalarView<'a> {
    Boolean(bool),
    Integer(i64),
    Float(f64),
    Text(&'a str),
}
#[derive(Clone, Copy, Debug)]
pub enum OperationView<'a> {
    Atom,
    SemanticScholar,
    OpenAlex {
        max_results: ScalarView<'a>,
        date_from: Option<&'a str>,
        date_to: Option<&'a str>,
    },
}
/// The complete14byte header and payload share one charge-owning allocation.
/// Pass this as ProcessPool payload with an empty common header Vec.
pub struct Frame(OwnedVec<u8>);
impl AsRef<[u8]> for Frame {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}
impl Frame {
    pub fn charged_bytes(&self) -> usize {
        self.0.charged_bytes()
    }
}
fn add(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_add(b).ok_or(Error::Overflow)
}
fn date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(i, b)| i != 4 && i != 7 && !b.is_ascii_digit())
    {
        return false;
    }
    let year: i32 = value[..4].parse().unwrap_or(0);
    let month: u32 = value[5..7].parse().unwrap_or(0);
    let day: u32 = value[8..].parse().unwrap_or(0);
    year != 0 && chrono::NaiveDate::from_ymd_opt(year, month, day).is_some()
}
fn dates(from: Option<&str>, to: Option<&str>) -> Result<(), Error> {
    if from.is_some_and(|v| !date(v))
        || to.is_some_and(|v| !date(v))
        || matches!((from,to), (Some(a),Some(b)) if a>b)
    {
        Err(Error::Date)
    } else {
        Ok(())
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
async fn append(out: &mut OwnedVec<u8>, bytes: &[u8], deadline: Instant) -> Result<(), Error> {
    for chunk in bytes.chunks(16 * 1024) {
        checkpoint(deadline).await?;
        out.try_extend_from_slice(chunk)
            .map_err(|_| Error::Resource)?;
    }
    Ok(())
}
fn push(out: &mut OwnedVec<u8>, byte: u8) -> Result<(), Error> {
    out.try_push(byte).map_err(|_| Error::Resource)
}
fn integer(out: &mut OwnedVec<u8>, n: usize) -> Result<(), Error> {
    let n = u64::try_from(n).map_err(|_| Error::Overflow)?;
    out.try_extend_from_slice(&n.to_be_bytes())
        .map_err(|_| Error::Resource)
}
/// Checked preflight precedes the single frame allocation. Copying and header
/// counting yield under the original absolute deadline; no timeout is renewed.
pub async fn encode<'a, I>(
    operation: Operation<'a>,
    headers: I,
    body: &[u8],
    budget: &Budget,
    deadline: Instant,
) -> Result<Frame, Error>
where
    I: Iterator<Item = Header<'a>> + Clone,
{
    checkpoint(deadline).await?;
    let mut count = 0u32;
    let mut payload = add(4, add(8, body.len())?)?;
    for header in headers.clone() {
        if count.is_multiple_of(128) {
            checkpoint(deadline).await?;
        }
        count = count.checked_add(1).ok_or(Error::Overflow)?;
        payload = add(
            payload,
            add(16, add(header.name.len(), header.value.len())?)?,
        )?;
    }
    let tag = match operation {
        Operation::Atom => 0,
        Operation::SemanticScholar => 1,
        Operation::OpenAlex {
            max_results,
            date_from,
            date_to,
        } => {
            dates(date_from, date_to)?;
            let scalar = match max_results {
                Scalar::Boolean(_) => 2,
                Scalar::Integer(_) | Scalar::Float(_) => 9,
                Scalar::Text(s) => add(9, s.len())?,
            };
            payload = add(payload, scalar)?;
            payload = add(
                payload,
                1 + 10 * (usize::from(date_from.is_some()) + usize::from(date_to.is_some())),
            )?;
            2
        }
    };
    let size = add(HEADER_BYTES, payload)?;
    let mut out = OwnedVec::with_capacity(budget, size).map_err(|_| Error::Resource)?;
    out.try_extend_from_slice(MAGIC)
        .map_err(|_| Error::Resource)?;
    push(&mut out, VERSION)?;
    push(&mut out, tag)?;
    integer(&mut out, payload)?;
    out.try_extend_from_slice(&count.to_be_bytes())
        .map_err(|_| Error::Resource)?;
    for (index, header) in headers.enumerate() {
        if index % 128 == 0 {
            checkpoint(deadline).await?;
        }
        integer(&mut out, header.name.len())?;
        append(&mut out, header.name, deadline).await?;
        integer(&mut out, header.value.len())?;
        append(&mut out, header.value, deadline).await?;
    }
    integer(&mut out, body.len())?;
    append(&mut out, body, deadline).await?;
    if let Operation::OpenAlex {
        max_results,
        date_from,
        date_to,
    } = operation
    {
        match max_results {
            Scalar::Boolean(v) => {
                push(&mut out, 0)?;
                push(&mut out, u8::from(*v))?;
            }
            Scalar::Integer(v) => {
                push(&mut out, 1)?;
                out.try_extend_from_slice(&v.to_be_bytes())
                    .map_err(|_| Error::Resource)?;
            }
            Scalar::Float(v) => {
                push(&mut out, 2)?;
                out.try_extend_from_slice(&v.to_bits().to_be_bytes())
                    .map_err(|_| Error::Resource)?;
            }
            Scalar::Text(v) => {
                push(&mut out, 3)?;
                integer(&mut out, v.len())?;
                append(&mut out, v.as_bytes(), deadline).await?;
            }
        }
        push(
            &mut out,
            u8::from(date_from.is_some()) | (u8::from(date_to.is_some()) << 1),
        )?;
        for value in [date_from, date_to].into_iter().flatten() {
            append(&mut out, value.as_bytes(), deadline).await?;
        }
    }
    if out.len() != size {
        return Err(Error::Overflow);
    }
    checkpoint(deadline).await?;
    Ok(Frame(out))
}
#[derive(Clone)]
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = add(self.at, n)?;
        let value = self.bytes.get(self.at..end).ok_or(Error::Truncated)?;
        self.at = end;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }
    fn length(&mut self) -> Result<usize, Error> {
        usize::try_from(u64::from_be_bytes(self.take(8)?.try_into().unwrap()))
            .map_err(|_| Error::Overflow)
    }
    fn header(&mut self) -> Result<Header<'a>, Error> {
        let n = self.length()?;
        let name = self.take(n)?;
        let n = self.length()?;
        let value = self.take(n)?;
        Ok(Header { name, value })
    }
}
#[derive(Clone)]
pub struct Headers<'a> {
    cursor: Cursor<'a>,
    count: u32,
}
impl<'a> Iterator for Headers<'a> {
    type Item = Result<Header<'a>, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.count == 0 {
            return None;
        }
        self.count -= 1;
        Some(self.cursor.header())
    }
}
pub struct View<'a> {
    pub operation: OperationView<'a>,
    pub body: &'a [u8],
    headers: &'a [u8],
    count: u32,
}
impl<'a> View<'a> {
    pub fn headers(&self) -> Headers<'a> {
        Headers {
            cursor: Cursor {
                bytes: self.headers,
                at: 0,
            },
            count: self.count,
        }
    }
}
fn fixed_header(header: &[u8]) -> Result<(u8, usize), Error> {
    if header.len() != HEADER_BYTES {
        return Err(Error::Truncated);
    }
    if &header[..4] != MAGIC {
        return Err(Error::Magic);
    }
    if header[4] != VERSION {
        return Err(Error::Version);
    }
    let tag = header[5];
    if tag > 2 {
        return Err(Error::Operation);
    }
    let length = usize::try_from(u64::from_be_bytes(header[6..].try_into().unwrap()))
        .map_err(|_| Error::Overflow)?;
    Ok((tag, length))
}
fn parts<'a>(header: &[u8], payload: &'a [u8]) -> Result<View<'a>, Error> {
    let (tag, length) = fixed_header(header)?;
    if payload.len() < length {
        return Err(Error::Truncated);
    }
    if payload.len() > length {
        return Err(Error::Trailing);
    }
    let mut cursor = Cursor {
        bytes: payload,
        at: 0,
    };
    let count = u32::from_be_bytes(cursor.take(4)?.try_into().unwrap());
    if usize::try_from(count).map_err(|_| Error::Overflow)? > payload.len().saturating_sub(4) / 16 {
        return Err(Error::Truncated);
    }
    let start = cursor.at;
    for _ in 0..count {
        cursor.header()?;
    }
    let headers = &payload[start..cursor.at];
    let length = cursor.length()?;
    let body = cursor.take(length)?;
    let operation = match tag {
        0 => OperationView::Atom,
        1 => OperationView::SemanticScholar,
        2 => {
            let max_results = match cursor.byte()? {
                0 => match cursor.byte()? {
                    0 => ScalarView::Boolean(false),
                    1 => ScalarView::Boolean(true),
                    _ => return Err(Error::Boolean),
                },
                1 => ScalarView::Integer(i64::from_be_bytes(cursor.take(8)?.try_into().unwrap())),
                2 => ScalarView::Float(f64::from_bits(u64::from_be_bytes(
                    cursor.take(8)?.try_into().unwrap(),
                ))),
                3 => {
                    let n = cursor.length()?;
                    ScalarView::Text(std::str::from_utf8(cursor.take(n)?).map_err(|_| Error::Utf8)?)
                }
                _ => return Err(Error::Scalar),
            };
            let flags = cursor.byte()?;
            if flags > 3 {
                return Err(Error::Date);
            }
            let date_from = if flags & 1 != 0 {
                Some(std::str::from_utf8(cursor.take(10)?).map_err(|_| Error::Date)?)
            } else {
                None
            };
            let date_to = if flags & 2 != 0 {
                Some(std::str::from_utf8(cursor.take(10)?).map_err(|_| Error::Date)?)
            } else {
                None
            };
            dates(date_from, date_to)?;
            OperationView::OpenAlex {
                max_results,
                date_from,
                date_to,
            }
        }
        _ => unreachable!("validated operation"),
    };
    if cursor.at != payload.len() {
        return Err(Error::Trailing);
    }
    Ok(View {
        operation,
        body,
        headers,
        count,
    })
}
pub fn decode(bytes: &[u8]) -> Result<View<'_>, Error> {
    if bytes.len() < HEADER_BYTES {
        return Err(Error::Truncated);
    }
    parts(&bytes[..HEADER_BYTES], &bytes[HEADER_BYTES..])
}
/// Child input is charged before allocation/read. No reconstructed header-pair
/// vector or concatenated header/body copy is allocated by the borrowed view.
pub struct OwnedInput {
    header: [u8; HEADER_BYTES],
    payload: OwnedVec<u8>,
}
impl OwnedInput {
    pub fn view(&self) -> Result<View<'_>, Error> {
        parts(&self.header, self.payload.as_slice())
    }
}
pub fn read_owned(reader: &mut impl Read, budget: &Budget) -> Result<OwnedInput, Error> {
    let mut header = [0; HEADER_BYTES];
    reader.read_exact(&mut header).map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            Error::Truncated
        } else {
            Error::Io(e.kind())
        }
    })?;
    let (_, length) = fixed_header(&header)?;
    let mut payload = OwnedVec::with_capacity(budget, length).map_err(|_| Error::Resource)?;
    let zeros = [0; 64 * 1024];
    while payload.len() < length {
        let n = (length - payload.len()).min(zeros.len());
        payload
            .try_extend_from_slice(&zeros[..n])
            .map_err(|_| Error::Resource)?;
    }
    reader.read_exact(payload.as_mut_slice()).map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            Error::Truncated
        } else {
            Error::Io(e.kind())
        }
    })?;
    let mut extra = [0];
    if reader.read(&mut extra).map_err(|e| Error::Io(e.kind()))? != 0 {
        return Err(Error::Trailing);
    }
    let input = OwnedInput { header, payload };
    input.view()?;
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Cursor as Input, sync::Arc, time::Duration};
    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(10)
    }
    #[tokio::test]
    async fn frame_preserves_raw_headers_body_scalar_bits_and_dates() {
        let headers = [
            Header {
                name: b"content-type",
                value: b"text/xml",
            },
            Header {
                name: b"content-type",
                value: b"\xff; charset=latin1",
            },
            Header {
                name: b"x",
                value: b"",
            },
        ];
        let budget = Budget::new(1024 * 1024).unwrap();
        for scalar in [
            Scalar::Boolean(false),
            Scalar::Integer(i64::MIN),
            Scalar::Float(-0.0),
            Scalar::Float(f64::from_bits(0x7ff8000000000042)),
            Scalar::Text(Arc::from("é")),
        ] {
            let frame = encode(
                Operation::OpenAlex {
                    max_results: &scalar,
                    date_from: Some("2024-02-29"),
                    date_to: Some("2024-02-29"),
                },
                headers.into_iter(),
                b"\x00\xff",
                &budget,
                deadline(),
            )
            .await
            .unwrap();
            assert_eq!(budget.snapshot().unwrap().used, frame.charged_bytes());
            let view = decode(frame.as_ref()).unwrap();
            assert_eq!(view.body, b"\x00\xff");
            assert_eq!(
                view.headers().collect::<Result<Vec<_>, _>>().unwrap(),
                headers
            );
            let OperationView::OpenAlex {
                max_results,
                date_from,
                date_to,
            } = view.operation
            else {
                panic!("OpenAlex operation")
            };
            assert_eq!(date_from, Some("2024-02-29"));
            assert_eq!(date_to, date_from);
            match (&scalar, max_results) {
                (Scalar::Boolean(a), ScalarView::Boolean(b)) => assert_eq!(*a, b),
                (Scalar::Integer(a), ScalarView::Integer(b)) => assert_eq!(*a, b),
                (Scalar::Float(a), ScalarView::Float(b)) => assert_eq!(a.to_bits(), b.to_bits()),
                (Scalar::Text(a), ScalarView::Text(b)) => assert_eq!(a.as_ref(), b),
                _ => panic!("scalar type changed"),
            }
            drop(frame);
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
        for op in [Operation::Atom, Operation::SemanticScholar] {
            let frame = encode(op, std::iter::empty(), b"x", &budget, deadline())
                .await
                .unwrap();
            assert_eq!(&frame.as_ref()[..5], b"SSXS\x02");
            assert_eq!(frame.as_ref().len(), HEADER_BYTES + 4 + 8 + 1);
            assert_eq!(decode(frame.as_ref()).unwrap().body, b"x");
            drop(frame);
        }
    }
    #[tokio::test]
    async fn frame_decoder_refuses_truncation_tags_dates_and_trailing_without_copy() {
        let budget = Budget::new(1024 * 1024).unwrap();
        let scalar = Scalar::Boolean(true);
        let frame = encode(
            Operation::OpenAlex {
                max_results: &scalar,
                date_from: None,
                date_to: None,
            },
            std::iter::empty(),
            b"x",
            &budget,
            deadline(),
        )
        .await
        .unwrap();
        for n in 0..frame.as_ref().len() {
            assert!(decode(&frame.as_ref()[..n]).is_err(), "prefix{n}");
        }
        for (at, byte, expected) in [
            (0, b'X', Error::Magic),
            (4, 255, Error::Version),
            (5, 3, Error::Operation),
            (28, 2, Error::Boolean),
            (27, 255, Error::Scalar),
            (29, 4, Error::Date),
        ] {
            let mut bad = frame.as_ref().to_vec();
            bad[at] = byte;
            assert!(
                matches!(decode(&bad),Err(e) if e==expected),
                "byte{at} expected{expected:?}"
            );
        }
        let mut bad = frame.as_ref().to_vec();
        bad.push(0);
        assert!(matches!(decode(&bad), Err(Error::Trailing)));
        assert!(matches!(
            encode(
                Operation::OpenAlex {
                    max_results: &scalar,
                    date_from: Some("2023-02-29"),
                    date_to: None
                },
                std::iter::empty(),
                b"",
                &budget,
                deadline()
            )
            .await,
            Err(Error::Date)
        ));
        assert!(matches!(
            encode(
                Operation::OpenAlex {
                    max_results: &scalar,
                    date_from: Some("2024-03-01"),
                    date_to: Some("2024-02-29")
                },
                std::iter::empty(),
                b"",
                &budget,
                deadline()
            )
            .await,
            Err(Error::Date)
        ));
    }
    #[tokio::test]
    async fn owned_child_read_charges_before_allocation_and_drops_on_failure() {
        let parent = Budget::new(1024 * 1024).unwrap();
        let frame = encode(
            Operation::Atom,
            std::iter::empty(),
            b"body",
            &parent,
            deadline(),
        )
        .await
        .unwrap();
        let child = Budget::new(1024).unwrap();
        let input = read_owned(&mut Input::new(frame.as_ref()), &child).unwrap();
        assert_eq!(input.view().unwrap().body, b"body");
        assert_eq!(
            child.snapshot().unwrap().used,
            frame.as_ref().len() - HEADER_BYTES
        );
        drop(input);
        assert_eq!(child.snapshot().unwrap().used, 0);
        let tiny = Budget::new(1).unwrap();
        assert!(matches!(
            read_owned(&mut Input::new(frame.as_ref()), &tiny),
            Err(Error::Resource)
        ));
        assert_eq!(tiny.snapshot().unwrap().used, 0);
        assert!(matches!(
            read_owned(
                &mut Input::new(&frame.as_ref()[..frame.as_ref().len() - 1]),
                &child
            ),
            Err(Error::Truncated)
        ));
        assert_eq!(child.snapshot().unwrap().used, 0);
        let mut trailing = frame.as_ref().to_vec();
        trailing.push(0);
        assert!(matches!(
            read_owned(&mut Input::new(trailing), &child),
            Err(Error::Trailing)
        ));
        assert_eq!(child.snapshot().unwrap().used, 0);
    }
    #[tokio::test]
    async fn frame_cancelled_during_copy_restores_charge() {
        let budget = Budget::new(4 * 1024 * 1024).unwrap();
        let body = vec![b'x'; 1024 * 1024];
        {
            let job = encode(
                Operation::Atom,
                std::iter::empty(),
                &body,
                &budget,
                deadline(),
            );
            tokio::pin!(job);
            tokio::select! {
                result=&mut job=>panic!("copy completed before observation: {}",result.is_ok()),
                _=async {loop{if budget.snapshot().unwrap().used>=body.len(){break;}tokio::task::yield_now().await;}}=>{}
            }
        }
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
}
