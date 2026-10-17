//! Native HTML helper protocol, ownership and source-facing tree operations.
//! Every C allocation stays in the child. Only owned Rust values cross IPC.
use crate::process_pool::{ProcessError, ProcessPool};
use serde::{Deserialize, Serialize};
use ssx_source_codecs::Text;
use std::{path::PathBuf, sync::OnceLock};
use tokio::time::Instant;

/// Operation-facing errors preserve execution/resource origin without matching
/// diagnostic strings. Legacy public parser APIs retain String compatibility.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum OperationFailure {
    Deadline,
    Resource,
    Cancelled,
    Shutdown,
    Invariant(&'static str),
    Codec(&'static str),
    Selector(SelectorFailure),
    Parse(&'static str),
    Failure(String),
}
impl std::fmt::Display for OperationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Deadline => f.write_str("HTML parser deadline exceeded"),
            Self::Resource => f.write_str(crate::html_budget::RESOURCE_ERROR),
            Self::Cancelled => f.write_str("HTML parser cancelled"),
            Self::Shutdown => f.write_str("HTML parser shutdown"),
            Self::Invariant(message) | Self::Codec(message) | Self::Parse(message) => {
                f.write_str(message)
            }
            Self::Failure(message) => f.write_str(message),
            Self::Selector(error) => std::fmt::Display::fmt(error, f),
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SelectorFailure {
    NullCharacter,
    RuntimeCodec,
    OverflowCodec,
    IntegerLimit(usize),
}
impl std::fmt::Display for SelectorFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NullCharacter => f.write_str("embedded null character"),
            Self::RuntimeCodec => f.write_str("internal codec error"),
            Self::OverflowCodec => f.write_str("Python int too large to convert to C ssize_t"),
            Self::IntegerLimit(digits) => write!(
                f,
                "Exceeds the limit (4300 digits) for integer string conversion: value has {digits} digits; use sys.set_int_max_str_digits() to increase the limit"
            ),
        }
    }
}
impl From<String> for OperationFailure {
    fn from(message: String) -> Self {
        Self::Failure(message)
    }
}
impl From<&str> for OperationFailure {
    fn from(message: &str) -> Self {
        Self::Failure(message.into())
    }
}
impl From<ProcessError> for OperationFailure {
    fn from(error: ProcessError) -> Self {
        match error {
            ProcessError::Deadline => Self::Deadline,
            ProcessError::Allocation => Self::Resource,
            ProcessError::CleanupUnverified => {
                Self::Invariant("HTML helper process failure: CleanupUnverified")
            }
            ProcessError::Supervisor => Self::Invariant("HTML helper process failure: Supervisor"),
            ProcessError::OutputBound => Self::Resource,
            ProcessError::Cancelled => Self::Cancelled,
            ProcessError::Shutdown => Self::Shutdown,
            other => Self::Failure(process_error(other)),
        }
    }
}
// F08 admits 32MiB decoded bytes. Every applicable source codec's scalar output
// requires at most four UTF8 bytes per input byte (family proof remains a gate).
const MAX_POINTS: usize = 32 * 1024 * 1024;
const PROTOCOL_MAGIC: &[u8; 4] = b"A05H";
const PROTOCOL_VERSION: u8 = 1;
// A conservative structural envelope, not an eager allocation: each input byte
// can contribute a node and bounded JSON/schema overhead. Full memory/CPU
// qualification and exact per-codec expansion proofs remain required.
fn output_bound(input: usize) -> Result<usize, String> {
    input
        .checked_mul(256)
        .and_then(|n| n.checked_add(16384))
        .ok_or_else(|| "HTML helper structural bound overflow".into())
}
#[derive(Serialize, Deserialize)]
enum WorkerOutput {
    Tree(Html),
    Decoded(Vec<u32>, DetectorFlags),
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DetectorFlags {
    pub duckduckgo_challenge: bool,
    pub duckduckgo_legitimate_empty: bool,
    pub google_challenge: bool,
}
pub struct DecodedText {
    pub text: Text,
    pub flags: DetectorFlags,
}
#[derive(Serialize, Deserialize, PartialEq)]
enum WorkerStage {
    Decode,
    Parse,
}
#[derive(Serialize, Deserialize)]
struct WorkerFailure {
    stage: WorkerStage,
    message: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Html {
    nodes: Vec<Node>,
    // Declaration order releases every owned tree allocation before its charge.
    #[serde(skip)]
    charge: Option<TreeCharge>,
}
struct TreeCharge(ssx_owned_budget::Charge);
impl std::fmt::Debug for TreeCharge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("TreeCharge").field(&self.0.bytes()).finish()
    }
}
#[derive(Debug, Serialize, Deserialize)]
struct Node {
    name: String,
    #[serde(with = "attribute_map")]
    attrs: Vec<(String, String)>,
    text: String,
    parent: Option<usize>,
    children: Vec<usize>,
    subtree_end: usize,
}
#[derive(Clone, Copy)]
pub struct ElementRef<'a> {
    tree: &'a Html,
    index: usize,
}
pub struct Selector<'css>(&'css str);
impl<'css> Selector<'css> {
    pub fn parse(css: &'css str) -> Result<Self, &'static str> {
        if css.is_empty() {
            Err("empty selector")
        } else {
            Ok(Self(css))
        }
    }
}
impl Html {
    /// Borrow each element in document order without copying the parsed tree.
    /// A10 composes source XPath predicates from these existing native nodes.
    pub(crate) fn elements(&self) -> impl Iterator<Item = ElementRef<'_>> {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(move |(index, node)| {
                (!node.name.is_empty()).then_some(ElementRef { tree: self, index })
            })
    }
    pub fn select<'a>(
        &'a self,
        selector: &'a Selector<'_>,
    ) -> impl Iterator<Item = ElementRef<'a>> {
        self.nodes.iter().enumerate().filter_map(move |(index, _)| {
            let n = ElementRef { tree: self, index };
            n.matches(selector.0).then_some(n)
        })
    }
    fn validate(&self) -> bool {
        if self.nodes.is_empty() {
            return false;
        }
        let mut incoming = vec![0u8; self.nodes.len()];
        for (i, n) in self.nodes.iter().enumerate() {
            if i == 0 && n.parent.is_some() {
                return false;
            }
            if n.subtree_end <= i || n.subtree_end > self.nodes.len() {
                return false;
            }
            let mut expected = i + 1;
            for &child in &n.children {
                if child != expected
                    || child >= n.subtree_end
                    || self.nodes[child].parent != Some(i)
                {
                    return false;
                }
                incoming[child] = incoming[child].saturating_add(1);
                expected = self.nodes[child].subtree_end;
            }
            if expected != n.subtree_end {
                return false;
            }
        }
        self.nodes[0].subtree_end == self.nodes.len()
            && incoming[0] == 0
            && incoming[1..].iter().all(|n| *n == 1)
    }
}
impl<'a> ElementRef<'a> {
    pub fn wrap(node: Self) -> Option<Self> {
        (!node.value().name().is_empty()).then_some(node)
    }
    pub fn value(self) -> Self {
        self
    }
    pub fn name(self) -> &'a str {
        &self.tree.nodes[self.index].name
    }
    pub fn attr(self, key: &str) -> Option<&'a str> {
        self.tree.nodes[self.index]
            .attrs
            .iter()
            .rev()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }
    pub fn text(self) -> impl Iterator<Item = &'a str> {
        self.descendants(true).filter_map(move |n| {
            let v = &self.tree.nodes[n.index];
            (v.name.is_empty()).then_some(v.text.as_str())
        })
    }
    pub fn ancestors(self) -> impl Iterator<Item = Self> {
        std::iter::successors(Some(self), |n| {
            n.tree.nodes[n.index].parent.map(|index| Self {
                tree: n.tree,
                index,
            })
        })
    }
    /// Direct children include text nodes, preserving XPath child-axis shape.
    pub(crate) fn children(self) -> impl Iterator<Item = Self> {
        self.tree.nodes[self.index]
            .children
            .iter()
            .map(move |&index| Self {
                tree: self.tree,
                index,
            })
    }
    /// Borrow source attribute names and values; no selector normalization.
    pub(crate) fn attributes(self) -> impl Iterator<Item = (&'a str, &'a str)> {
        self.tree.nodes[self.index]
            .attrs
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }
    pub fn next_siblings(self) -> impl Iterator<Item = Self> {
        let list = self.tree.nodes[self.index]
            .parent
            .map(|parent| self.tree.nodes[parent].children.as_slice())
            .unwrap_or(&[]);
        let start = list
            .iter()
            .position(|index| *index == self.index)
            .map_or(list.len(), |index| index + 1);
        list[start..].iter().map(move |index| Self {
            tree: self.tree,
            index: *index,
        })
    }
    pub fn descendants(self, inclusive: bool) -> impl Iterator<Item = Self> {
        let start = self.index + usize::from(!inclusive);
        (start..self.tree.nodes[self.index].subtree_end).map(move |index| Self {
            tree: self.tree,
            index,
        })
    }
    pub fn select_context(self, css: &str) -> impl Iterator<Item = Self> {
        self.descendants(true).filter(move |node| node.matches(css))
    }
    fn matches(self, css: &str) -> bool {
        if self.name().is_empty() {
            return false;
        }
        css.split(',').any(|part| {
            let part = part.trim();
            let (base, attribute) = part
                .split_once('[')
                .map_or((part, None), |(a, b)| (a, Some(b.trim_end_matches(']'))));
            let (tag, class) = base
                .split_once('.')
                .map_or((base, None), |(a, b)| (a, Some(b)));
            if !tag.is_empty() && tag != self.name() {
                return false;
            }
            if let Some(class) = class
                && !self
                    .attr("class")
                    .unwrap_or("")
                    .split([' ', '\t', '\n', '\r'])
                    .any(|v| v == class)
            {
                return false;
            }
            if let Some(attr) = attribute {
                for (operator, mode) in [("^=", 0), ("*=", 1), ("=", 2)] {
                    if let Some((key, value)) = attr.split_once(operator) {
                        let value = value.trim_matches(['\'', '"']);
                        return self.attr(key).is_some_and(|actual| match mode {
                            0 => actual.starts_with(value),
                            1 => actual.contains(value),
                            _ => actual == value,
                        });
                    }
                }
                return self.attr(attr).is_some();
            }
            true
        })
    }
}
/// Pool admission is configurable. A request owns its existing deadline while
/// waiting; cancellation drops the waiter without starting a child.
#[derive(Clone)]
pub struct ParserPool {
    process: ProcessPool,
}
impl ParserPool {
    pub fn new(helper: PathBuf, capacity: std::num::NonZeroUsize) -> Result<Self, String> {
        ProcessPool::new(helper, capacity)
            .map(|process| Self { process })
            .map_err(process_error)
    }
    pub fn available_slots(&self) -> usize {
        self.process.available_slots()
    }
    /// Must be awaited before runtime teardown; verified cleanup errors propagate.
    pub async fn shutdown(&self) -> Result<(), String> {
        self.process.shutdown().await.map_err(process_error)
    }
    pub(crate) async fn parse_rows_with_operation(
        &self,
        points: &[u32],
        layout: row_wire::Layout,
        limit: crate::adapters::RowLimit,
        operation: std::sync::Arc<crate::html_budget::Operation>,
    ) -> Result<rows_parent::OwnedRows, OperationFailure> {
        rows_parent::parse(&self.process, points, layout, limit, operation).await
    }
    pub(crate) async fn project_rows_with_operation(
        &self,
        points: &[u32],
        layout: row_wire::Layout,
        limit: crate::adapters::RowLimit,
        query: &str,
        operation: std::sync::Arc<crate::html_budget::Operation>,
    ) -> Result<ProducedHtmlBatch, OperationFailure> {
        let rows = self
            .parse_rows_with_operation(points, layout, limit, operation)
            .await?;
        rows_projection::project(rows, layout, query).await
    }
    pub async fn parse(&self, raw: &str, deadline: Instant) -> Result<Html, String> {
        self.parse_text(&Text::from(raw), deadline).await
    }
    pub async fn parse_text(&self, raw: &Text, deadline: Instant) -> Result<Html, String> {
        self.parse_text_with_operation(raw.codepoints(), deadline, None)
            .await
            .map_err(|error| error.to_string())
    }
    pub(crate) async fn parse_text_with_operation(
        &self,
        raw: &[u32],
        deadline: Instant,
        operation: Option<std::sync::Arc<crate::html_budget::Operation>>,
    ) -> Result<Html, OperationFailure> {
        if raw.len() > MAX_POINTS {
            return Err("HTML decoded input exceeds codec expansion bound".into());
        }
        let count = raw.len();
        let size = count
            .checked_mul(4)
            .ok_or("HTML codepoint frame overflow")?;
        let mut input = Vec::new();
        input
            .try_reserve_exact(size)
            .map_err(|_| "HTML codepoint frame allocation failed")?;
        for point in raw {
            input.extend_from_slice(&point.to_be_bytes());
        }
        match self
            .exchange(0, &[], input, count, deadline, operation)
            .await?
        {
            WorkerOutput::Tree(tree) if tree.validate() => Ok(tree),
            _ => Err("HTML helper invalid tree response".into()),
        }
    }
    pub async fn decode(
        &self,
        bytes: &[u8],
        label: &str,
        deadline: Instant,
    ) -> Result<Text, String> {
        self.decode_detect(bytes, label, deadline)
            .await
            .map(|decoded| decoded.text)
    }
    pub async fn decode_detect(
        &self,
        bytes: &[u8],
        label: &str,
        deadline: Instant,
    ) -> Result<DecodedText, String> {
        self.decode_detect_with_operation(bytes, label, deadline, None)
            .await
            .map_err(|error| error.to_string())
    }
    pub(crate) async fn decode_detect_with_operation(
        &self,
        bytes: &[u8],
        label: &str,
        deadline: Instant,
        operation: Option<std::sync::Arc<crate::html_budget::Operation>>,
    ) -> Result<DecodedText, OperationFailure> {
        if bytes.len() > MAX_POINTS {
            return Err("HTTP text body exceeds transport bound".into());
        }
        let canonical = ssx_source_codecs::canonical(label).unwrap_or("utf-8");
        match self
            .exchange(
                1,
                canonical.as_bytes(),
                bytes.to_vec(),
                bytes.len(),
                deadline,
                operation,
            )
            .await?
        {
            WorkerOutput::Decoded(points, flags) if points.len() <= bytes.len() => {
                Text::from_codepoints(points)
                    .map(|text| DecodedText { text, flags })
                    .map_err(OperationFailure::from)
            }
            _ => Err("HTML helper invalid decoder response".into()),
        }
    }
    #[cfg(test)]
    pub(crate) async fn decode_bounded_with_operation(
        &self,
        bytes: &[u8],
        label: &str,
        operation: std::sync::Arc<crate::html_budget::Operation>,
    ) -> Result<BoundedDecodedText, OperationFailure> {
        bounded_decode::decode(&self.process, bytes, label, operation).await
    }
    pub(crate) async fn decode_http_with_operation(
        &self,
        bytes: &[u8],
        headers: &reqwest::header::HeaderMap,
        operation: std::sync::Arc<crate::html_budget::Operation>,
    ) -> Result<BoundedDecodedText, OperationFailure> {
        bounded_decode::decode_headers(&self.process, bytes, headers, operation).await
    }
    async fn exchange(
        &self,
        operation: u8,
        codec: &[u8],
        input: Vec<u8>,
        count: usize,
        deadline: Instant,
        lease: Option<std::sync::Arc<crate::html_budget::Operation>>,
    ) -> Result<WorkerOutput, OperationFailure> {
        let mut header = PROTOCOL_MAGIC.to_vec();
        header.push(PROTOCOL_VERSION);
        header.extend([
            operation,
            u8::try_from(codec.len()).map_err(|_| "invalid canonical codec length")?,
        ]);
        header.extend(codec);
        header.extend((count as u64).to_be_bytes());
        let response = self
            .process
            .exchange_with_lease(deadline, header, input, output_bound(count)?, lease)
            .await
            .map_err(OperationFailure::from)?;
        let response: Result<WorkerOutput, WorkerFailure> =
            serde_json::from_slice(&response).map_err(|e| e.to_string())?;
        response.map_err(|failure| {
            if failure.stage
                == if operation == 1 {
                    WorkerStage::Decode
                } else {
                    WorkerStage::Parse
                }
            {
                OperationFailure::Failure(failure.message)
            } else {
                OperationFailure::from("HTML helper invalid error stage")
            }
        })
    }
}
fn process_error(error: ProcessError) -> String {
    match error {
        ProcessError::Deadline => "HTML parser deadline exceeded".into(),
        ProcessError::AdmissionClosed => "HTML parser admission closed".into(),
        ProcessError::Shutdown => "HTML parser shutdown".into(),
        ProcessError::Cancelled => "HTML parser cancelled".into(),
        ProcessError::OutputBound => "HTML helper output exceeds structural bound".into(),
        other => format!("HTML helper process failure: {other}"),
    }
}
static POOL: OnceLock<std::sync::Arc<ParserPool>> = OnceLock::new();
/// Explicit shutdown must complete before the backend runtime is torn down.
pub async fn shutdown() -> Result<(), String> {
    if let Some(pool) = POOL.get() {
        pool.shutdown().await
    } else {
        Ok(())
    }
}
fn default_pool() -> &'static std::sync::Arc<ParserPool> {
    POOL.get_or_init(|| {
        let exe = std::env::current_exe().expect("native executable path");
        let parent = exe.parent().expect("executable directory");
        let parent = if parent.file_name().is_some_and(|n| n == "deps") {
            parent.parent().unwrap()
        } else {
            parent
        };
        std::sync::Arc::new(
            ParserPool::new(
                parent.join("ssx-html-parser"),
                std::num::NonZeroUsize::new(4).unwrap(),
            )
            .expect("bounded native helper pool capacity"),
        )
    })
}
pub async fn parse(raw: &Text, deadline: Instant) -> Result<Html, String> {
    default_pool().parse_text(raw, deadline).await
}
pub async fn decode(bytes: &[u8], label: &str, deadline: Instant) -> Result<Text, String> {
    default_pool().decode(bytes, label, deadline).await
}
pub async fn decode_detect(
    bytes: &[u8],
    label: &str,
    deadline: Instant,
) -> Result<DecodedText, String> {
    default_pool().decode_detect(bytes, label, deadline).await
}
pub(crate) fn shared_pool() -> std::sync::Arc<ParserPool> {
    default_pool().clone()
}
/// Dedicated helper entry. No sockets, credentials, arbitrary file names or
/// external-resource-capable API is exposed by its protocol.
pub fn helper_main() -> Result<(), String> {
    use std::io::{Read, Write};
    let mut input = std::io::stdin().lock();
    let mut prefix = [0; 5];
    input.read_exact(&mut prefix).map_err(|e| e.to_string())?;
    if &prefix[..4] != PROTOCOL_MAGIC {
        return Err("invalid HTML helper protocol version".into());
    }
    if prefix[4] == http_headers::VERSION {
        return bounded_decode::worker_headers(&mut input, &mut std::io::stdout().lock());
    }
    if prefix[4] == rows_worker::VERSION {
        return rows_worker::worker(&mut input, &mut std::io::stdout().lock());
    }
    if prefix[4] == 2 {
        return bounded_decode::worker(&mut input, &mut std::io::stdout().lock());
    }
    if prefix[4] != PROTOCOL_VERSION {
        return Err("invalid HTML helper protocol version".into());
    }
    let mut mode = [0; 2];
    input.read_exact(&mut mode).map_err(|e| e.to_string())?;
    if mode[0] > 1 {
        return Err("invalid helper operation".into());
    }
    let mut label = vec![0; usize::from(mode[1])];
    input.read_exact(&mut label).map_err(|e| e.to_string())?;
    let label = std::str::from_utf8(&label).map_err(|_| "invalid helper codec")?;
    if (mode[0] == 0 && !label.is_empty())
        || (mode[0] == 1 && ssx_source_codecs::canonical(label) != Some(label))
    {
        return Err("invalid canonical helper codec".into());
    }
    let mut header = [0; 8];
    input.read_exact(&mut header).map_err(|e| e.to_string())?;
    let count =
        usize::try_from(u64::from_be_bytes(header)).map_err(|_| "HTML helper count overflow")?;
    if count > MAX_POINTS {
        return Err("oversize HTML helper input".into());
    }
    let length = if mode[0] == 0 {
        count.checked_mul(4).ok_or("HTML helper frame overflow")?
    } else {
        count
    };
    let mut raw = Vec::new();
    input
        .by_ref()
        .take(length as u64)
        .read_to_end(&mut raw)
        .map_err(|e| e.to_string())?;
    let mut trailing = [0; 1];
    if input.read(&mut trailing).map_err(|e| e.to_string())? != 0 {
        return Err("HTML helper trailing input".into());
    }
    let response: Result<WorkerOutput, String> = if raw.len() != length {
        Err("HTML helper truncated input".into())
    } else if mode[0] == 1 {
        decode_native(&raw, label).map(|text| {
            let flags = crate::adapters::html_detector_flags(&text);
            WorkerOutput::Decoded(text.codepoints().to_vec(), flags)
        })
    } else {
        let points = raw
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_be_bytes(*b))
            .collect();
        Text::from_codepoints(points)
            .map_err(str::to_owned)
            .and_then(|raw| parse_native(&raw))
            .map(WorkerOutput::Tree)
    };
    let response = response.map_err(|message| WorkerFailure {
        stage: if mode[0] == 1 {
            WorkerStage::Decode
        } else {
            WorkerStage::Parse
        },
        message,
    });
    let response = serde_json::to_vec(&response).map_err(|e| e.to_string())?;
    if response.len() > output_bound(count)? {
        return Err("HTML helper structural bound".into());
    }
    let mut output = std::io::stdout().lock();
    output
        .write_all(&(response.len() as u64).to_be_bytes())
        .map_err(|e| e.to_string())?;
    output.write_all(&response).map_err(|e| e.to_string())?;
    output.flush().map_err(|e| e.to_string())
}
fn parse_native(raw: &Text) -> Result<Html, String> {
    parse_native_points(raw.codepoints())
}
fn parse_native_points(points: &[u32]) -> Result<Html, String> {
    // Exact source declaration predicate is qualified separately; no arbitrary
    // PI/doctype stripping is permitted.
    if encoding_declaration(points) {
        return Err("Unicode strings with encoding declaration are not supported. Please use bytes input or XML fragments without declaration.".into());
    }
    let (doc, _) = libxml::parser::Parser::parse_untrusted_html_codepoints(points)
        .map_err(|_| "Document is empty")?;
    let root = doc.get_root_element().ok_or("Document is empty")?;
    let mut tree = Html {
        nodes: vec![],
        charge: None,
    };
    let mut stack = vec![(root, None)];
    while let Some((node, parent)) = stack.pop() {
        let element = node.is_element_node();
        let text = matches!(
            node.get_type(),
            Some(libxml::tree::NodeType::TextNode | libxml::tree::NodeType::CDataSectionNode)
        );
        if !element && !text {
            continue;
        }
        let index = tree.nodes.len();
        tree.nodes.push(Node {
            name: if element {
                node.get_name()
            } else {
                String::new()
            },
            attrs: if element {
                node.get_properties().into_iter().collect()
            } else {
                Vec::new()
            },
            text: if text {
                node.get_content()
            } else {
                String::new()
            },
            parent,
            children: vec![],
            subtree_end: index + 1,
        });
        if let Some(parent) = parent {
            tree.nodes[parent].children.push(index);
        }
        stack.extend(
            node.get_child_nodes()
                .into_iter()
                .rev()
                .map(|child| (child, Some(index))),
        );
    }
    for index in (0..tree.nodes.len()).rev() {
        if let Some(parent) = tree.nodes[index].parent {
            tree.nodes[parent].subtree_end = tree.nodes[parent]
                .subtree_end
                .max(tree.nodes[index].subtree_end);
        }
    }
    Ok(tree)
}

fn source_space(p: u32) -> bool {
    matches!(p,9..=13|28..=32|0x85|0xa0|0x1680|0x2000..=0x200a|0x2028|0x2029|0x202f|0x205f|0x3000)
}
// Exact source regex membership over codepoints, including non-scalar values.
fn encoding_declaration(points: &[u32]) -> bool {
    let prefix = [60, 63, 120, 109, 108];
    if !points.starts_with(&prefix) {
        return false;
    }
    let end = points.iter().position(|p| *p == 62).unwrap_or(points.len());
    for i in 6..end {
        if !source_space(points[i]) {
            continue;
        }
        let mut j = i;
        while j < points.len() && source_space(points[j]) {
            j += 1;
        }
        let name = [101, 110, 99, 111, 100, 105, 110, 103];
        if !points[j..].starts_with(&name) {
            continue;
        }
        j += name.len();
        while j < points.len() && source_space(points[j]) {
            j += 1;
        }
        if points.get(j) != Some(&61) {
            continue;
        }
        j += 1;
        while j < points.len() && source_space(points[j]) {
            j += 1;
        }
        if !matches!(points.get(j), Some(34 | 39)) {
            continue;
        }
        j += 1;
        if points[j..].iter().any(|p| matches!(p, 34 | 39)) {
            return true;
        }
    }
    false
}

fn decode_native(bytes: &[u8], label: &str) -> Result<Text, String> {
    ssx_source_codecs::decode_http(bytes, label).map_err(|error| error.to_string())
}

#[path = "html_bounded_decode.rs"]
mod bounded_decode;
#[path = "html_decode_wire.rs"]
mod decode_wire;
pub(crate) use bounded_decode::Decoded as BoundedDecodedText;

#[path = "html_wire.rs"]
pub(crate) mod row_wire;
#[path = "html_rows_worker.rs"]
mod rows_worker;

#[path = "html_rows_borrowed.rs"]
pub(crate) mod rows_borrowed;

#[path = "html_rows_parent.rs"]
mod rows_parent;

#[path = "html_rows_projection.rs"]
mod rows_projection;
pub(crate) use rows_projection::Batch as ProducedHtmlBatch;

#[path = "html_http_headers.rs"]
mod http_headers;

// Preserve the legacy tree IPC map shape. Only the isolated legacy deserializer
// constructs this map; the actual row-producing child uses charged ordered pairs.
mod attribute_map {
    use serde::ser::SerializeMap;
    use serde::{Deserialize, Deserializer, Serializer};
    pub(super) fn serialize<S: Serializer>(
        attrs: &[(String, String)],
        s: S,
    ) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(attrs.len()))?;
        for (key, value) in attrs {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Vec<(String, String)>, D::Error> {
        Ok(
            std::collections::BTreeMap::<String, String>::deserialize(d)?
                .into_iter()
                .collect(),
        )
    }
}
#[path = "html_charged_flatten.rs"]
mod charged_flatten;

#[path = "html_child_rows.rs"]
pub(crate) mod child_rows;
