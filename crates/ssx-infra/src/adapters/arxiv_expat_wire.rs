//! Closed versioned backend payload inside the common u64-length frame.
//! Counts/capacities are checked before reserving; no recursive value decoder.
use ssx_source_xml::Entry;
pub(super) const MEMORY: usize = 64 * 1024 * 1024;
pub(super) const OUTPUT: usize = MEMORY + 13;
const MAGIC: &[u8; 4] = b"AX01";
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Error {
    Invalid,
    Allocation,
    Budget,
    Deadline,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Reply {
    Entries(Vec<Entry>),
    Syntax,
    Resource,
    Defect,
}
fn plus(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_add(b).ok_or(Error::Budget)
}
fn times(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_mul(b).ok_or(Error::Budget)
}
fn string_size(s: &str) -> Result<usize, Error> {
    plus(8, s.len())
}
pub(super) fn encode(reply: &Reply) -> Result<Vec<u8>, Error> {
    let mut size = 5usize;
    if let Reply::Entries(entries) = reply {
        size = plus(size, 8)?;
        for e in entries {
            size = plus(size, 16)?; // 8presence tags + author count
            for f in e.fields.iter().flatten() {
                size = plus(size, string_size(f)?)?;
            }
            for a in &e.authors {
                size = plus(size, string_size(a)?)?;
            }
        }
    }
    if size > OUTPUT {
        return Err(Error::Budget);
    }
    let mut out = Vec::new();
    out.try_reserve_exact(size).map_err(|_| Error::Allocation)?;
    out.extend_from_slice(MAGIC);
    out.push(match reply {
        Reply::Entries(_) => 0,
        Reply::Syntax => 1,
        Reply::Resource => 2,
        Reply::Defect => 3,
    });
    if let Reply::Entries(entries) = reply {
        out.extend_from_slice(&(entries.len() as u64).to_be_bytes());
        for e in entries {
            for f in &e.fields {
                out.push(u8::from(f.is_some()));
                if let Some(s) = f {
                    put_string(&mut out, s)
                }
            }
            out.extend_from_slice(&(e.authors.len() as u64).to_be_bytes());
            for a in &e.authors {
                put_string(&mut out, a)
            }
        }
    }
    if out.len() != size {
        return Err(Error::Invalid);
    }
    Ok(out)
}
fn put_string(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u64).to_be_bytes());
    out.extend_from_slice(s.as_bytes());
}
struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
    charged: usize,
    deadline: tokio::time::Instant,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = plus(self.offset, n)?;
        let b = self.bytes.get(self.offset..end).ok_or(Error::Invalid)?;
        self.offset = end;
        Ok(b)
    }
    fn count(&mut self) -> Result<usize, Error> {
        let bytes = self.take(8)?;
        usize::try_from(u64::from_be_bytes(
            bytes.try_into().map_err(|_| Error::Invalid)?,
        ))
        .map_err(|_| Error::Budget)
    }
    fn charge(&mut self, n: usize) -> Result<(), Error> {
        self.charged = plus(self.charged, n)?;
        if self.charged > MEMORY {
            return Err(Error::Budget);
        }
        Ok(())
    }
    async fn checkpoint(&self) -> Result<(), Error> {
        tokio::task::yield_now().await;
        if tokio::time::Instant::now() >= self.deadline {
            Err(Error::Deadline)
        } else {
            Ok(())
        }
    }
    async fn string(&mut self) -> Result<String, Error> {
        let n = self.count()?;
        let raw = self.take(n)?;
        self.charge(n)?;
        let mut s = String::new();
        s.try_reserve_exact(n).map_err(|_| Error::Allocation)?;
        self.charge(s.capacity() - n)?;
        let mut offset = 0;
        while offset < raw.len() {
            let end = offset.saturating_add(16384).min(raw.len());
            let (valid, end) = match std::str::from_utf8(&raw[offset..end]) {
                Ok(text) => (text, end),
                Err(error) if error.error_len().is_none() && end < raw.len() => {
                    let end = offset + error.valid_up_to();
                    (
                        std::str::from_utf8(&raw[offset..end]).map_err(|_| Error::Invalid)?,
                        end,
                    )
                }
                Err(_) => return Err(Error::Invalid),
            };
            if end == offset {
                return Err(Error::Invalid);
            }
            s.push_str(valid);
            offset = end;
            self.checkpoint().await?;
        }
        Ok(s)
    }
}
pub(super) async fn decode(bytes: &[u8], deadline: tokio::time::Instant) -> Result<Reply, Error> {
    if bytes.len() > OUTPUT {
        return Err(Error::Budget);
    }
    let mut r = Reader {
        bytes,
        offset: 0,
        charged: 0,
        deadline,
    };
    r.checkpoint().await?;
    if r.take(4)? != MAGIC {
        return Err(Error::Invalid);
    }
    let reply = match r.take(1)?[0] {
        0 => {
            let count = r.count()?;
            r.charge(times(count, std::mem::size_of::<Entry>())?)?;
            // Every entry needs at least8presence tags+8author count bytes.
            if times(count, 16)? > r.bytes.len() - r.offset {
                return Err(Error::Invalid);
            }
            let mut entries = Vec::new();
            entries
                .try_reserve_exact(count)
                .map_err(|_| Error::Allocation)?;
            r.charge(times(
                entries.capacity() - count,
                std::mem::size_of::<Entry>(),
            )?)?;
            for index in 0..count {
                if index % 128 == 0 {
                    r.checkpoint().await?
                }
                let mut e = Entry::default();
                for f in &mut e.fields {
                    match r.take(1)?[0] {
                        0 => {}
                        1 => *f = Some(r.string().await?),
                        _ => return Err(Error::Invalid),
                    }
                }
                let authors = r.count()?;
                r.charge(times(authors, std::mem::size_of::<String>())?)?;
                if times(authors, 8)? > r.bytes.len() - r.offset {
                    return Err(Error::Invalid);
                }
                e.authors
                    .try_reserve_exact(authors)
                    .map_err(|_| Error::Allocation)?;
                r.charge(times(
                    e.authors.capacity() - authors,
                    std::mem::size_of::<String>(),
                )?)?;
                for index in 0..authors {
                    if index % 128 == 0 {
                        r.checkpoint().await?
                    }
                    e.authors.push(r.string().await?)
                }
                entries.push(e);
            }
            Reply::Entries(entries)
        }
        1 => Reply::Syntax,
        2 => Reply::Resource,
        3 => Reply::Defect,
        _ => return Err(Error::Invalid),
    };
    if r.offset != bytes.len() {
        return Err(Error::Invalid);
    }
    Ok(reply)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn deadline() -> tokio::time::Instant {
        tokio::time::Instant::now() + std::time::Duration::from_secs(30)
    }
    #[tokio::test(flavor = "current_thread")]
    async fn near_budget_decode_yields_to_cancellation_and_preserves_multibyte_edges() {
        let mut e = Entry::default();
        e.fields[0] = Some("雪".repeat((MEMORY - 4096) / 3));
        let bytes = encode(&Reply::Entries(vec![e])).unwrap();
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(1),
            decode(&bytes, deadline()),
        )
        .await;
        assert!(
            result.is_err(),
            "large decode monopolized the runtime until completion"
        );
        let mut e = Entry::default();
        e.fields[0] = Some(format!("{}雪", "a".repeat(16383)));
        let expected = Reply::Entries(vec![e]);
        assert_eq!(
            decode(&encode(&expected).unwrap(), deadline())
                .await
                .unwrap(),
            expected
        );
        assert_eq!(
            decode(b"AX01\x01", tokio::time::Instant::now()).await,
            Err(Error::Deadline)
        );
    }
    #[tokio::test]
    async fn versioned_payload_round_trip_and_exact_count_bound() {
        let mut e = Entry::default();
        e.fields[0] = Some("A\0雪".into());
        e.fields[1] = Some(String::new());
        e.authors = vec!["X".into(), String::new()];
        for r in [
            Reply::Entries(vec![e]),
            Reply::Entries(vec![]),
            Reply::Syntax,
            Reply::Resource,
            Reply::Defect,
        ] {
            let b = encode(&r).unwrap();
            assert_eq!(decode(&b, deadline()).await.unwrap(), r);
            for i in 0..b.len() {
                assert!(decode(&b[..i], deadline()).await.is_err(), "prefix {i}")
            }
            let mut trailing = b;
            trailing.push(0);
            assert_eq!(decode(&trailing, deadline()).await, Err(Error::Invalid));
        }
    }
    #[tokio::test]
    async fn refuses_unknown_tags_versions_counts_lengths_utf8_before_copy() {
        for b in [b"AX02\x01".as_slice(), b"AX01\x04".as_slice()] {
            assert_eq!(decode(b, deadline()).await, Err(Error::Invalid))
        }
        let mut b = b"AX01\0".to_vec();
        b.extend_from_slice(&u64::MAX.to_be_bytes());
        assert_eq!(decode(&b, deadline()).await, Err(Error::Budget));
        let mut b = b"AX01\0".to_vec();
        b.extend_from_slice(&1u64.to_be_bytes());
        b.push(1);
        b.extend_from_slice(&u64::MAX.to_be_bytes());
        b.extend_from_slice(&[0; 15]);
        assert!(decode(&b, deadline()).await.is_err());
        let mut e = Entry::default();
        e.fields[0] = Some("A".into());
        let mut b = encode(&Reply::Entries(vec![e])).unwrap();
        b[22] = 0xff;
        assert_eq!(decode(&b, deadline()).await, Err(Error::Invalid));
        let mut b = b"AX01\0".to_vec();
        b.extend_from_slice(&1u64.to_be_bytes());
        b.extend_from_slice(&[0; 8]);
        b.extend_from_slice(&u64::MAX.to_be_bytes());
        assert_eq!(decode(&b, deadline()).await, Err(Error::Budget));
        let mut b = b"AX01\0".to_vec();
        b.extend_from_slice(&1u64.to_be_bytes());
        b.extend_from_slice(&[2; 16]);
        assert_eq!(decode(&b, deadline()).await, Err(Error::Invalid));
    }
}
