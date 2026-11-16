//! Owned compact output capture. A fragment is encoded once from the same
//! resolved node and replayed verbatim; no retained store is reread for output.
use super::{Direction, Edge, Failure, Node, Request};
use crate::receipts::Cancellation;
use ssx_domain::{ArtifactRef, LineageRelation};
use ssx_json::{Value, json};

pub mod shared;

pub const REFERENCE_BYTES: usize = 5120;
pub const NODE_BYTES: usize = ssx_json::MAX_JSON_BYTES + REFERENCE_BYTES + 1024;
pub const EDGE_BYTES: usize = 2 * REFERENCE_BYTES + 128;
pub const MAX_EDGES: usize = 5 * 100 * 100;
/// These are expanded-wire bounds, not backing allocation targets.
pub const PAYLOAD_BYTES: usize = 16384 + 100 * NODE_BYTES + MAX_EDGES * EDGE_BYTES;
pub const WIRE_BYTES: usize = 3 * PAYLOAD_BYTES + ssx_json::MAX_JSON_BYTES + 16384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IndexedEdge {
    pub from: u8,
    pub relation: u8,
    pub to: u8,
}
pub fn relation_index(relation: LineageRelation) -> u8 {
    match relation {
        LineageRelation::Contains => 0,
        LineageRelation::DerivedFrom => 1,
        LineageRelation::Selected => 2,
        LineageRelation::Observed => 3,
        LineageRelation::RetrievalOf => 4,
    }
}
pub fn relation_name(relation: u8) -> Option<&'static str> {
    [
        "contains",
        "derived_from",
        "selected",
        "observed",
        "retrieval_of",
    ]
    .get(usize::from(relation))
    .copied()
}
/// No Clone: the transport shares this owner by Arc, not by copying fragments.
#[derive(Debug)]
pub struct CapturedGraph {
    pub direction: Direction,
    pub max_depth: usize,
    pub max_nodes: usize,
    pub references: Vec<Box<str>>,
    pub nodes: Vec<Box<str>>,
    pub edges: Vec<IndexedEdge>,
    pub truncated: bool,
}
impl CapturedGraph {
    /// Requested owned backing, excluding allocator implementation overhead.
    /// All serialized node/ref storage is Box<str> with exact requested length.
    pub fn owned_bytes(&self) -> Option<usize> {
        let mut total = std::mem::size_of::<Self>();
        for (capacity, size) in [
            (self.references.capacity(), std::mem::size_of::<Box<str>>()),
            (self.nodes.capacity(), std::mem::size_of::<Box<str>>()),
            (self.edges.capacity(), std::mem::size_of::<IndexedEdge>()),
        ] {
            total = total.checked_add(capacity.checked_mul(size)?)?;
        }
        for fragment in self.references.iter().chain(&self.nodes) {
            total = total.checked_add(fragment.len())?;
        }
        Some(total)
    }
}

pub(super) trait Build: Sized {
    type Output;
    fn admit(&mut self, reference: &ArtifactRef, index: u8) -> Result<(), Failure>;
    fn node(&mut self, node: Node, cancel: &Cancellation) -> Result<(), Failure>;
    fn edge(&mut self, edge: Edge, index: IndexedEdge);
    fn finish(self, truncated: bool) -> Self::Output;
}
pub(super) struct CompactBuild(CapturedGraph);
impl CompactBuild {
    pub(super) fn new(request: &Request) -> Self {
        Self(CapturedGraph {
            direction: request.direction,
            max_depth: request.max_depth,
            max_nodes: request.max_nodes,
            references: Vec::with_capacity(request.max_nodes),
            nodes: Vec::with_capacity(request.max_nodes),
            edges: Vec::new(),
            truncated: false,
        })
    }
}
impl Build for CompactBuild {
    type Output = CapturedGraph;

    fn admit(&mut self, reference: &ArtifactRef, index: u8) -> Result<(), Failure> {
        if usize::from(index) != self.0.references.len() {
            return Err(Failure::Malformed);
        }
        // Complete encoded ref is <=5120 bytes by accepted F05 identity rules.
        let encoded = ssx_json::to_string(&reference.to_value()).map_err(|_| Failure::Malformed)?;
        if encoded.len() > REFERENCE_BYTES {
            return Err(Failure::Malformed);
        }
        self.0.references.push(encoded.into_boxed_str());
        Ok(())
    }
    fn node(&mut self, node: Node, cancel: &Cancellation) -> Result<(), Failure> {
        let Node {
            artifact,
            status,
            created_at,
            expires_at,
            details,
        } = node;
        let mut value = json!({"artifact":artifact.to_value(),"status":status.as_str(),"created_at":created_at.map(|t|t.seconds()),"expires_at":expires_at.map(|t|t.seconds())});
        if let Some(details) = details.filter(|d| !d.is_empty()) {
            // Move the full projected detail tree. No whole field clone and no
            // macro round trip through the default4MiB parser.
            value["details"] = Value::Object(details);
        }
        self.0.nodes.push(fragment(&value, cancel, NODE_BYTES)?);
        Ok(())
    }
    fn edge(&mut self, _: Edge, index: IndexedEdge) {
        self.0.edges.push(index);
    }
    fn finish(mut self, truncated: bool) -> Self::Output {
        self.0.truncated = truncated;
        self.0
    }
}

/// Match ordinary ssx_json::to_string order/string/number spelling. Unlike the
/// manifest's sorted canonical encoder, objects retain insertion order here.
/// Both passes use <=6-byte scalar scratch; count before allocating the exact
/// fragment once. A retained field's wrapper depth does not alter input bounds.
fn visit(
    value: &Value,
    depth: usize,
    cancel: &Cancellation,
    output: &mut dyn FnMut(&[u8]) -> Result<(), Failure>,
) -> Result<(), Failure> {
    if cancel.is_cancelled() {
        return Err(Failure::Cancelled);
    }
    if depth > ssx_json::MAX_JSON_DEPTH + 8 {
        return Err(Failure::Malformed);
    }
    match value {
        Value::Null => output(b"null"),
        Value::Bool(true) => output(b"true"),
        Value::Bool(false) => output(b"false"),
        Value::Number(number) => output(number.as_str().as_bytes()),
        Value::String(value) => text(value, cancel, output),
        Value::Array(values) => {
            output(b"[")?;
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output(b",")?;
                }
                visit(value, depth + 1, cancel, output)?;
            }
            output(b"]")
        }
        Value::Object(values) => {
            output(b"{")?;
            for (index, (key, value)) in values.iter().enumerate() {
                if index != 0 {
                    output(b",")?;
                }
                text(key, cancel, output)?;
                output(b":")?;
                visit(value, depth + 1, cancel, output)?;
            }
            output(b"}")
        }
    }
}
fn text(
    text: &str,
    cancel: &Cancellation,
    output: &mut dyn FnMut(&[u8]) -> Result<(), Failure>,
) -> Result<(), Failure> {
    output(b"\"")?;
    let hex = b"0123456789abcdef";
    for (offset, ch) in text.char_indices() {
        if offset % 4096 < 4 && cancel.is_cancelled() {
            return Err(Failure::Cancelled);
        }
        match ch {
            '"' => output(b"\\\"")?,
            '\\' => output(b"\\\\")?,
            '\u{8}' => output(b"\\b")?,
            '\u{9}' => output(b"\\t")?,
            '\u{a}' => output(b"\\n")?,
            '\u{c}' => output(b"\\f")?,
            '\u{d}' => output(b"\\r")?,
            ch if (ch as u32) < 32 => {
                let n = ch as u8;
                output(&[
                    b'\\',
                    b'u',
                    b'0',
                    b'0',
                    hex[usize::from(n >> 4)],
                    hex[usize::from(n & 15)],
                ])?;
            }
            ch => {
                let mut bytes = [0; 4];
                output(ch.encode_utf8(&mut bytes).as_bytes())?;
            }
        }
    }
    output(b"\"")
}
pub(crate) fn fragment(
    value: &Value,
    cancel: &Cancellation,
    limit: usize,
) -> Result<Box<str>, Failure> {
    let mut len = 0usize;
    visit(value, 0, cancel, &mut |bytes| {
        len = len.checked_add(bytes.len()).ok_or(Failure::Malformed)?;
        if len > limit {
            return Err(Failure::Malformed);
        }
        Ok(())
    })?;
    let mut bytes = Vec::with_capacity(len);
    visit(value, 0, cancel, &mut |part| {
        bytes.extend_from_slice(part);
        Ok(())
    })?;
    if bytes.len() != len {
        return Err(Failure::Malformed);
    }
    String::from_utf8(bytes)
        .map(String::into_boxed_str)
        .map_err(|_| Failure::Malformed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fragment_is_lossless_ordered_json_including_wrapped_depth_and_literal_reserved_keys() {
        for raw in [
            r#"{"z":null,"a":[18446744073709551617,-18446744073709551617,4.200e-05,-0.0],"$serde_json::private::Number":"literal"}"#,
            r#"{"text":"雪😀\u007f\b\t\n\f\r\u0000\\\"","query_id":false}"#,
        ] {
            let value: Value = ssx_json::from_str(raw).unwrap();
            assert_eq!(
                fragment(&value, &Cancellation::default(), NODE_BYTES)
                    .unwrap()
                    .as_ref(),
                ssx_json::to_string(&value).unwrap()
            );
        }
        let mut value = Value::String((0u8..=127).map(char::from).collect());
        for _ in 0..ssx_json::MAX_JSON_DEPTH {
            value = Value::Array(vec![value]);
        }
        for _ in 0..4 {
            value = Value::Array(vec![value]);
        }
        assert_eq!(
            fragment(&value, &Cancellation::default(), NODE_BYTES)
                .unwrap()
                .as_ref(),
            ssx_json::to_string(&value).unwrap()
        );
    }
    #[test]
    fn fragment_allocates_exact_bytes_and_refuses_over_bound_or_precancel_without_global_parser_changes()
     {
        let value = Value::String("x".repeat(NODE_BYTES - 2));
        let encoded = fragment(&value, &Cancellation::default(), NODE_BYTES).unwrap();
        assert_eq!(encoded.len(), NODE_BYTES);
        assert!(matches!(
            fragment(&value, &Cancellation::default(), NODE_BYTES - 1),
            Err(Failure::Malformed)
        ));
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(matches!(
            fragment(&Value::Null, &cancel, NODE_BYTES),
            Err(Failure::Cancelled)
        ));
        assert!(
            ssx_json::from_str::<Value>(&encoded).is_err(),
            "unchanged global4MiB input parser"
        );
    }
}
