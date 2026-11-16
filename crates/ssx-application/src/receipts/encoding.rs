//! W07 output-container encoding only. Retained fields already crossed bounded
//! typed store parsers; no input parser or shared F05 canonical profile changes.
use super::{Cancellation, MAX_MANIFEST_BYTES};
use ssx_json::{JsonProfile, Value, canonical_json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Error {
    TooLarge,
    Malformed,
    Cancelled,
}
// A stored receipt is nested by manifest/items/item/receipts (four levels).
// Eight levels also cover additive graph/read/artifact containers. This is an
// output traversal bound, never an increase to the input/store parser's100.
const OUTPUT_DEPTH: usize = ssx_json::MAX_JSON_DEPTH + 8;
struct Encoder<'a> {
    bytes: Vec<u8>,
    cancel: &'a Cancellation,
}
impl Encoder<'_> {
    fn append(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if self.cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if bytes.len() > MAX_MANIFEST_BYTES.saturating_sub(self.bytes.len()) {
            return Err(Error::TooLarge);
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn text_chunk(&mut self, value: &str) -> Result<(), Error> {
        // A UTF8 chunk is at most4099bytes. F05 owns exact Unicode/control
        // escaping. Strip only its enclosing quotes when joining chunks.
        let encoded = canonical_json(&Value::String(value.into()), JsonProfile::Utf8Compact)
            .map_err(|_| Error::Malformed)?;
        self.append(&encoded[1..encoded.len() - 1])
    }
    fn text(&mut self, value: &str) -> Result<(), Error> {
        self.append(b"\"")?;
        let mut start = 0;
        for (at, _) in value.char_indices() {
            if at - start >= 4096 {
                self.text_chunk(&value[start..at])?;
                start = at;
            }
        }
        self.text_chunk(&value[start..])?;
        self.append(b"\"")
    }
    fn object(&mut self, object: &ssx_json::Map<String, Value>, depth: usize) -> Result<(), Error> {
        self.append(b"{")?;
        // Borrow values/keys: no full personalized/canonical clone.
        let mut entries = object.iter().collect::<Vec<_>>();
        entries.sort_unstable_by(|a, b| a.0.cmp(b.0));
        for (index, (key, value)) in entries.into_iter().enumerate() {
            if index > 0 {
                self.append(b",")?;
            }
            self.text(key)?;
            self.append(b":")?;
            self.value(value, depth + 1)?;
        }
        self.append(b"}")
    }
    fn value(&mut self, value: &Value, depth: usize) -> Result<(), Error> {
        if depth > OUTPUT_DEPTH {
            return Err(Error::Malformed);
        }
        if self.cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        match value {
            Value::Object(object) => self.object(object, depth),
            Value::Array(array) => {
                self.append(b"[")?;
                for (index, value) in array.iter().enumerate() {
                    if index > 0 {
                        self.append(b",")?;
                    }
                    self.value(value, depth + 1)?;
                }
                self.append(b"]")
            }
            Value::String(value) => self.text(value),
            Value::Number(number) if !number.is_f64() => self.append(number.as_str().as_bytes()),
            // F05 owns finite float spelling (including negative zero). A long
            // admitted float token may shrink, so raw length cannot reject it.
            Value::Number(_) | Value::Bool(_) | Value::Null => {
                let scalar = canonical_json(value, JsonProfile::Utf8Compact)
                    .map_err(|_| Error::Malformed)?;
                self.append(&scalar)
            }
        }
    }
}
#[derive(Debug)]
pub(crate) struct Prefix {
    bytes: Vec<u8>,
    fault: Option<Error>,
}
impl Prefix {
    pub(crate) fn new() -> Self {
        Self {
            bytes: Vec::with_capacity(MAX_MANIFEST_BYTES),
            fault: None,
        }
    }
    fn update(
        &mut self,
        cancel: &Cancellation,
        apply: impl FnOnce(&mut Encoder<'_>) -> Result<(), Error>,
    ) {
        // The original service checks cancellation after every readonly port
        // before it begins canonical encoding. A latched semantic error cannot
        // hide a later cancellation while those same reads still execute.
        if cancel.is_cancelled() {
            self.fault = Some(Error::Cancelled);
            return;
        }
        if self.fault.is_some() {
            return;
        }
        let mut encoder = Encoder {
            bytes: std::mem::take(&mut self.bytes),
            cancel,
        };
        self.fault = apply(&mut encoder).err();
        self.bytes = encoder.bytes;
    }
    pub(crate) fn value(&mut self, value: &Value, depth: usize, cancel: &Cancellation) {
        self.update(cancel, |encoder| encoder.value(value, depth));
    }
    pub(crate) fn raw(&mut self, bytes: &[u8], cancel: &Cancellation) {
        // Only already-encoded private canonical pieces or fixed syntax reach
        // this method. Caller/wire text never crosses this internal boundary.
        self.update(cancel, |encoder| encoder.append(bytes));
    }
    pub(crate) fn text(&mut self, value: &str, cancel: &Cancellation) {
        self.update(cancel, |encoder| encoder.text(value));
    }
    pub(crate) fn absorb(&mut self, other: &Self, cancel: &Cancellation) {
        if other.fault == Some(Error::Cancelled) {
            self.fault = Some(Error::Cancelled);
            return;
        }
        self.raw(&other.bytes, cancel);
        if self.fault.is_none() {
            self.fault = other.fault;
        }
    }
    pub(crate) fn fault(&self) -> Option<Error> {
        self.fault
    }
    pub(crate) fn complete(&self) -> Result<&[u8], Error> {
        self.fault.map_or(Ok(self.bytes.as_slice()), Err)
    }
    /// Seal exactly one fixed syntax byte after completed readonly walks.
    /// Service selection/clock decisions precede its next cancellation check.
    /// No caller bytes, loop, allocation growth or cancellation authority here.
    pub(crate) fn close_completed_array(&mut self) {
        if self.fault.is_none() {
            if self.bytes.len() == MAX_MANIFEST_BYTES {
                self.fault = Some(Error::TooLarge);
            } else {
                self.bytes.push(b']');
            }
        }
    }
    pub(crate) fn requested_bytes(&self) -> usize {
        self.bytes.capacity()
    }
    pub(crate) fn node(&mut self, node: &crate::lineage::Node, cancel: &Cancellation) {
        self.update(cancel, |encoder| {
            // These are the canonical Node keys, independently checked against
            // encode(Node::to_value(), depth4) in closed component tests. The
            // potentially large details map remains borrowed throughout.
            encoder.append(b"{\"artifact\":")?;
            encoder.value(&node.artifact.to_value(), 5)?;
            encoder.append(b",\"created_at\":")?;
            encoder.value(
                &ssx_json::json!(node.created_at.map(|time| time.seconds())),
                5,
            )?;
            if let Some(details) = node.details.as_ref().filter(|details| !details.is_empty()) {
                encoder.append(b",\"details\":")?;
                encoder.object(details, 5)?;
            }
            encoder.append(b",\"expires_at\":")?;
            encoder.value(
                &ssx_json::json!(node.expires_at.map(|time| time.seconds())),
                5,
            )?;
            encoder.append(b",\"status\":")?;
            encoder.text(node.status.as_str())?;
            encoder.append(b"}")
        });
    }
}

/// Canonical sorted compact UTF8 output, capped at exactly the manifest limit.
/// Preimage oversize implies final oversize because only artifact/lineage fields
/// are added later; all selected readonly source operations have already run.
pub(super) fn encode(value: &Value, cancel: &Cancellation) -> Result<Vec<u8>, Error> {
    let mut encoder = Encoder {
        bytes: Vec::with_capacity(MAX_MANIFEST_BYTES),
        cancel,
    };
    encoder.value(value, 0)?;
    Ok(encoder.bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_output_matches_shared_canonical_scalars_and_sorted_containers() {
        for raw in [
            r#"{"z":null,"a":[true,false,"雪😀\\\"\n\u0000\u007f"],"$serde_json::private::Number":"literal"}"#,
            r#"[18446744073709551617,-18446744073709551617,-0.0,1e-5,1e16,1.25]"#,
        ] {
            let value: Value = ssx_json::from_str(raw).unwrap();
            assert_eq!(
                encode(&value, &Cancellation::default()).unwrap(),
                canonical_json(&value, JsonProfile::Utf8Compact).unwrap()
            );
        }
        let value = Value::String("雪😀\n\\\"\u{0}".repeat(3000));
        assert_eq!(
            encode(&value, &Cancellation::default()).unwrap(),
            canonical_json(&value, JsonProfile::Utf8Compact).unwrap()
        );
    }
    #[test]
    fn exact_output_budget_deep_store_wrappers_and_cancellation() {
        let cancel = Cancellation::default();
        let exact = Value::String("x".repeat(MAX_MANIFEST_BYTES - 2));
        assert_eq!(encode(&exact, &cancel).unwrap().len(), MAX_MANIFEST_BYTES);
        let over = Value::String("x".repeat(MAX_MANIFEST_BYTES - 1));
        assert_eq!(encode(&over, &cancel), Err(Error::TooLarge));
        assert_eq!(
            encode(
                &Value::String("\u{0}".repeat(MAX_MANIFEST_BYTES / 6 + 1)),
                &cancel
            ),
            Err(Error::TooLarge)
        );
        let mut deep = Value::Null;
        for _ in 0..ssx_json::MAX_JSON_DEPTH {
            deep = Value::Array(vec![deep]);
        }
        // Same bounded retained Value plus the four manifest receipt wrappers.
        for _ in 0..4 {
            deep = Value::Array(vec![deep]);
        }
        assert!(encode(&deep, &cancel).is_ok());
        for _ in 0..5 {
            deep = Value::Array(vec![deep]);
        }
        assert_eq!(encode(&deep, &cancel), Err(Error::Malformed));
        cancel.cancel();
        assert_eq!(encode(&Value::Null, &cancel), Err(Error::Cancelled));
    }
    #[test]
    fn borrowed_node_prefix_matches_existing_encoder_at_exact_manifest_depth() {
        use crate::lineage::{Node, Status};
        use ssx_domain::{ArtifactKind, ArtifactRef};
        let cancel = Cancellation::default();
        for depth in [0, 98, 101, 103] {
            let mut body = ssx_json::from_str::<Value>(r#"{"z":null,"a":[18446744073709551617,1.000e0,-0.0,"雪😀\n"],"$serde_json::private::Number":"literal"}"#).unwrap();
            for _ in 0..depth {
                body = Value::Array(vec![body]);
            }
            let mut node = Node::missing(
                ArtifactRef::new(ArtifactKind::Snapshot, "snapshot").unwrap(),
                Status::Live,
            );
            let mut details = ssx_json::Map::new();
            details.insert("body".into(), body);
            node.details = Some(details);
            let mut expected = Encoder {
                bytes: Vec::with_capacity(MAX_MANIFEST_BYTES),
                cancel: &cancel,
            };
            let expected_error = expected.value(&node.to_value(), 4).err();
            assert_eq!(
                expected_error,
                if depth >= 101 {
                    Some(Error::Malformed)
                } else {
                    None
                }
            );
            let mut actual = Prefix::new();
            actual.node(&node, &cancel);
            assert_eq!(actual.fault(), expected_error, "depth={depth}");
            assert_eq!(actual.bytes, expected.bytes, "depth={depth}");
        }
    }
    #[test]
    fn prefix_fault_composition_preserves_first_error_and_cancellation() {
        let cancel = Cancellation::default();
        let mut deep = Value::Null;
        for _ in 0..110 {
            deep = Value::Array(vec![deep]);
        }
        let mut node = crate::lineage::Node::missing(
            ssx_domain::ArtifactRef::new(ssx_domain::ArtifactKind::Snapshot, "snapshot").unwrap(),
            crate::lineage::Status::Live,
        );
        let mut details = ssx_json::Map::new();
        details.insert("deep".into(), deep);
        node.details = Some(details);
        let mut malformed = Prefix::new();
        malformed.node(&node, &cancel);
        assert_eq!(malformed.fault(), Some(Error::Malformed));
        let mut too_large = Prefix::new();
        too_large.raw(&vec![b'x'; MAX_MANIFEST_BYTES], &cancel);
        too_large.raw(b"x", &cancel);
        assert_eq!(too_large.fault(), Some(Error::TooLarge));
        let mut items_first = Prefix::new();
        items_first.absorb(&malformed, &cancel);
        items_first.absorb(&too_large, &cancel);
        assert_eq!(items_first.fault(), Some(Error::Malformed));
        let mut edges_first = Prefix::new();
        edges_first.absorb(&too_large, &cancel);
        edges_first.absorb(&malformed, &cancel);
        assert_eq!(edges_first.fault(), Some(Error::TooLarge));
        cancel.cancel();
        edges_first.raw(b"ignored", &cancel);
        assert_eq!(edges_first.fault(), Some(Error::Cancelled));
        assert_eq!(edges_first.requested_bytes(), MAX_MANIFEST_BYTES);
    }
}
