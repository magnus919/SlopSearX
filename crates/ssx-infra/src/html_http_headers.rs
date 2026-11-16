//! Closed borrowed raw-header frame. Validation allocates no pair collection.
use super::OperationFailure;
pub(super) const VERSION: u8 = 4;
pub(super) const OPERATION: u8 = 1;
pub(super) const HEADER: usize = 38;
#[derive(Clone, Copy)]
pub(super) struct Headers<'a> {
    bytes: &'a [u8],
    count: usize,
}
impl<'a> Headers<'a> {
    pub(super) fn validate(bytes: &'a [u8], count: usize) -> Result<Self, OperationFailure> {
        if count > bytes.len() / 16 {
            return Err(invalid());
        }
        let mut remaining = bytes;
        for _ in 0..count {
            let (_, _, rest) = pair(remaining).ok_or_else(invalid)?;
            remaining = rest;
        }
        if !remaining.is_empty() {
            return Err(invalid());
        }
        Ok(Self { bytes, count })
    }
    pub(super) fn iter(self) -> Pairs<'a> {
        Pairs {
            bytes: self.bytes,
            count: self.count,
        }
    }
}
fn invalid() -> OperationFailure {
    OperationFailure::Invariant("HTML invalid raw-header frame")
}
fn pair(bytes: &[u8]) -> Option<(&[u8], &[u8], &[u8])> {
    let name = usize::try_from(u64::from_be_bytes(bytes.get(..8)?.try_into().ok()?)).ok()?;
    let value = usize::try_from(u64::from_be_bytes(bytes.get(8..16)?.try_into().ok()?)).ok()?;
    let end_name = 16usize.checked_add(name)?;
    let end_value = end_name.checked_add(value)?;
    Some((
        bytes.get(16..end_name)?,
        bytes.get(end_name..end_value)?,
        bytes.get(end_value..)?,
    ))
}
#[derive(Clone)]
pub(super) struct Pairs<'a> {
    bytes: &'a [u8],
    count: usize,
}
impl<'a> Iterator for Pairs<'a> {
    type Item = (&'a [u8], &'a [u8]);
    fn next(&mut self) -> Option<Self::Item> {
        if self.count == 0 {
            return None;
        }
        let (name, value, rest) = pair(self.bytes)?;
        self.bytes = rest;
        self.count -= 1;
        Some((name, value))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_order_empty_bytes_and_rejects_every_truncation_and_trailing_byte() {
        let pairs: [(&[u8], &[u8]); 3] = [
            (b"content-type", b"text/plain; charset=cp437"),
            (b"content-type", b""),
            (b"", b"\x00\xff"),
        ];
        let mut bytes = Vec::new();
        for (name, value) in pairs {
            bytes.extend((name.len() as u64).to_be_bytes());
            bytes.extend((value.len() as u64).to_be_bytes());
            bytes.extend(name);
            bytes.extend(value);
        }
        let validated = Headers::validate(&bytes, 3).unwrap();
        assert_eq!(validated.iter().collect::<Vec<_>>(), pairs);
        assert_eq!(validated.iter().clone().collect::<Vec<_>>(), pairs);
        for length in 0..bytes.len() {
            assert!(Headers::validate(&bytes[..length], 3).is_err(), "{length}");
        }
        let mut bad = bytes.clone();
        bad.push(0);
        assert!(Headers::validate(&bad, 3).is_err());
        assert!(Headers::validate(&bytes, 2).is_err());
        assert!(Headers::validate(&bytes, usize::MAX).is_err());
        bad = bytes.clone();
        bad[..8].copy_from_slice(&u64::MAX.to_be_bytes());
        assert!(Headers::validate(&bad, 3).is_err());
        assert!(Headers::validate(&[], 0).unwrap().iter().next().is_none());
    }
}
