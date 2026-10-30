//! CLI-private immutable source request target; never an unchecked URI API.
use std::{fmt, sync::Arc};

/// Raw HTTP1 request target validated like the frozen source's request line.
/// It preserves encoded dot segments, literal proxy fragments and arbitrarily
/// long admitted queries. Routing, Host, method and version remain separate.
#[derive(Clone)]
pub struct SourceRequestTarget(Arc<str>);

/// Invalid request target. The error never retains or exposes caller bytes.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct InvalidSourceRequestTarget;
impl fmt::Display for InvalidSourceRequestTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid source HTTP request target")
    }
}
impl std::error::Error for InvalidSourceRequestTarget {}
impl fmt::Debug for SourceRequestTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceRequestTarget").field("bytes", &self.0.len()).finish()
    }
}
impl SourceRequestTarget {
    /// Construct from owned ASCII graphic bytes. C0, space, DEL, non-ASCII and
    /// empty input fail before serialization, preventing request-line injection.
    ///
    /// # Errors
    /// Returns an error if the input is empty or contains a non-graphic ASCII byte.
    pub fn new(value: String) -> Result<Self, InvalidSourceRequestTarget> {
        if value.is_empty() || !value.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
            return Err(InvalidSourceRequestTarget);
        }
        Ok(Self(Arc::from(value)))
    }
    /// Borrow immutable validated bytes. No unchecked or mutable constructor exists.
    pub fn as_str(&self) -> &str { &self.0 }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_raw_target_rejects_controls_space_del_nonascii_and_empty() {
        assert!(SourceRequestTarget::new(String::new()).is_err());
        for point in (0..=0x20).chain(0x7f..=0xff) {
            assert!(SourceRequestTarget::new(format!("/prefix{}suffix",char::from_u32(point).unwrap())).is_err());
        }
        assert!(SourceRequestTarget::new("/a\r\nInjected: header".into()).is_err());
    }
    #[test]
    fn source_raw_target_retains_long_path_query_and_fragment_octets() {
        for value in [format!("/{}?end", "a".repeat(66000)),format!("/?q={}","%CE%BB".repeat(11000)),format!("http://Host:00080/#{}", "x".repeat(66000)),"/a/%2e%2e/final".into(),"http://Host#fragment-without-added-slash".into()] {
            let target=SourceRequestTarget::new(value.clone()).unwrap();
            assert_eq!(target.as_str(),value);
            assert!(!format!("{target:?}").contains("fragment-without-added-slash"));
        }
    }
}
