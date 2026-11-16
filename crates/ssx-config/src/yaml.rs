//! PyYAML SafeLoader scalar resolution over grammar-aware parser events.
//! Parsing budgets remain those of the pinned YAML reader; alias copies are bounded too.
use super::ConfigError;
use serde_json::Number;
type Map<K, V> = indexmap::IndexMap<K, V>;
#[derive(Clone)]
enum Value {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Value>),
    Object(Map<String, Value>),
}
impl Value {
    fn into_json(self) -> serde_json::Value {
        match self {
            Self::Null => serde_json::Value::Null,
            Self::Bool(v) => serde_json::Value::Bool(v),
            Self::Number(v) => serde_json::Value::Number(v),
            Self::String(v) => serde_json::Value::String(v),
            Self::Array(v) => {
                serde_json::Value::Array(v.into_iter().map(Self::into_json).collect())
            }
            Self::Object(v) => {
                serde_json::Value::Object(v.into_iter().map(|(k, v)| (k, v.into_json())).collect())
            }
        }
    }
    fn child(&self, key: &str) -> Option<&Self> {
        if let Self::Object(m) = self {
            m.get(key)
        } else {
            None
        }
    }
}
use serde_saphyr::{
    Budget,
    granit_parser::{Event, Parser, ScalarStyle, Tag},
};
use std::collections::{BTreeMap, BTreeSet};

type Result<T> = std::result::Result<T, ConfigError>;
fn invalid() -> ConfigError {
    ConfigError("invalid config YAML".into())
}
#[derive(Clone)]
struct Node {
    value: Value,
    nodes: usize,
    bytes: usize,
    merge: bool,
}
impl Node {
    fn scalar(value: Value, bytes: usize, merge: bool) -> Self {
        Self {
            value,
            nodes: 1,
            bytes,
            merge,
        }
    }
}
enum Contents {
    Sequence(Vec<Node>),
    Mapping(Vec<(Node, Node)>, Option<Node>),
}
struct Frame {
    anchor: usize,
    contents: Contents,
}
fn charge_copy(node: &Node, copied_nodes: &mut usize, copied_bytes: &mut usize) -> Result<()> {
    *copied_nodes = copied_nodes.checked_add(node.nodes).ok_or_else(invalid)?;
    *copied_bytes = copied_bytes.checked_add(node.bytes).ok_or_else(invalid)?;
    if *copied_nodes > 1_000_000 || *copied_bytes > 64 * 1024 * 1024 {
        return Err(invalid());
    }
    Ok(())
}
fn nonspecific(tag: &Tag) -> bool {
    tag.handle().is_empty() && tag.suffix() == "!"
}
fn attach(node: Node, stack: &mut [Frame], root: &mut Option<Node>) -> Result<()> {
    match stack.last_mut() {
        Some(Frame {
            contents: Contents::Sequence(items),
            ..
        }) => items.push(node),
        Some(Frame {
            contents: Contents::Mapping(items, pending),
            ..
        }) => {
            if let Some(key) = pending.take() {
                items.push((key, node));
            } else {
                *pending = Some(node);
            }
        }
        None => {
            if root.replace(node).is_some() {
                return Err(invalid());
            }
        }
    }
    Ok(())
}
fn merge_into(target: &mut Map<String, Value>, value: Value) -> Result<()> {
    match value {
        Value::Object(map) => target.extend(map),
        Value::Array(items) => {
            for item in items.into_iter().rev() {
                let Value::Object(map) = item else {
                    return Err(invalid());
                };
                target.extend(map);
            }
        }
        _ => return Err(invalid()),
    }
    Ok(())
}
#[cfg(test)]
pub(super) fn parse(raw: &str) -> Result<serde_json::Value> {
    parse_with_topic_order(raw).map(|(v, _)| v)
}
pub(super) fn parse_with_topic_order(raw: &str) -> Result<(serde_json::Value, Vec<String>)> {
    // This public preflight retains the pinned reader's event/depth/scalar/anchor budgets.
    if serde_saphyr::budget::parse_yaml(raw, Budget::default()).map_err(|_| invalid())? {
        return Err(invalid());
    }
    // The source rejects duplicate anchor names; parser event IDs alone hide them.
    let mut names = BTreeSet::new();
    for token in
        serde_saphyr::granit_parser::Scanner::new(serde_saphyr::granit_parser::StrInput::new(raw))
    {
        let token = token.map_err(|_| invalid())?;
        if let serde_saphyr::granit_parser::TokenType::Anchor(name) = token.token_type()
            && !names.insert(name.to_string())
        {
            return Err(invalid());
        }
    }
    let mut stack: Vec<Frame> = Vec::new();
    let mut root = None;
    let mut anchors: BTreeMap<usize, Node> = BTreeMap::new();
    let mut documents = 0;
    let mut replayed_nodes = 0usize;
    let mut copied_bytes = 0usize;
    for item in Parser::new_from_str(raw) {
        let (event, _) = item.map_err(|_| invalid())?;
        let sequence = matches!(&event, Event::SequenceStart(..));
        match event {
            Event::StreamStart | Event::StreamEnd | Event::DocumentEnd => {}
            Event::DocumentStart(..) => {
                documents += 1;
                if documents > 1 {
                    return Err(invalid());
                }
            }
            Event::Scalar(text, style, anchor, tag) => {
                let implicit = tag.is_none() && style == ScalarStyle::Plain
                    || tag.as_deref().is_some_and(nonspecific);
                let tag_name = tag
                    .as_deref()
                    .and_then(|t| t.suffix_in_namespace("tag:yaml.org,2002:"));
                let merge = tag_name.as_deref() == Some("merge") || implicit && text == "<<";
                let special_key =
                    merge || tag_name.as_deref() == Some("value") || implicit && text == "=";
                if special_key
                    && !matches!(
                        stack.last(),
                        Some(Frame {
                            contents: Contents::Mapping(_, None),
                            ..
                        })
                    )
                {
                    return Err(invalid());
                }
                let value = scalar(&text, style, tag.as_deref())?;
                let node = Node::scalar(value, text.len(), merge);
                if anchor != 0 {
                    charge_copy(&node, &mut replayed_nodes, &mut copied_bytes)?;
                    anchors.insert(anchor, node.clone());
                }
                attach(node, &mut stack, &mut root)?;
            }
            Event::SequenceStart(_, anchor, tag) | Event::MappingStart(_, anchor, tag) => {
                if let Some(tag) = tag.as_deref()
                    && !nonspecific(tag)
                    && !tag.is_yaml_core_schema_tag(if sequence { "seq" } else { "map" })
                {
                    return Err(invalid());
                }
                stack.push(Frame {
                    anchor,
                    contents: if sequence {
                        Contents::Sequence(Vec::new())
                    } else {
                        Contents::Mapping(Vec::new(), None)
                    },
                });
            }
            Event::SequenceEnd | Event::MappingEnd => {
                let frame = stack.pop().ok_or_else(invalid)?;
                let mut nodes = 1usize;
                let mut bytes = 0usize;
                let value = match frame.contents {
                    Contents::Sequence(items) => {
                        if !matches!(event, Event::SequenceEnd) {
                            return Err(invalid());
                        }
                        let mut result = Vec::new();
                        for item in items {
                            nodes = nodes.checked_add(item.nodes).ok_or_else(invalid)?;
                            bytes = bytes.checked_add(item.bytes).ok_or_else(invalid)?;
                            result.push(item.value);
                        }
                        Value::Array(result)
                    }
                    Contents::Mapping(items, pending) => {
                        if !matches!(event, Event::MappingEnd) || pending.is_some() {
                            return Err(invalid());
                        }
                        let mut result = Map::new();
                        let mut explicit = Vec::new();
                        for (key, value) in items {
                            nodes = nodes
                                .checked_add(key.nodes)
                                .and_then(|n| n.checked_add(value.nodes))
                                .ok_or_else(invalid)?;
                            bytes = bytes
                                .checked_add(key.bytes)
                                .and_then(|n| n.checked_add(value.bytes))
                                .ok_or_else(invalid)?;
                            if key.merge {
                                merge_into(&mut result, value.value)?;
                            } else {
                                let Value::String(key) = key.value else {
                                    return Err(invalid());
                                };
                                explicit.push((key, value.value));
                            }
                        }
                        result.extend(explicit);
                        Value::Object(result)
                    }
                };
                let node = Node {
                    value,
                    nodes,
                    bytes,
                    merge: false,
                };
                if frame.anchor != 0 {
                    charge_copy(&node, &mut replayed_nodes, &mut copied_bytes)?;
                    anchors.insert(frame.anchor, node.clone());
                }
                attach(node, &mut stack, &mut root)?;
            }
            Event::Alias(anchor) => {
                // Anchors become visible only after construction: recursive aliases fail closed.
                let node = anchors.get(&anchor).ok_or_else(invalid)?;
                charge_copy(node, &mut replayed_nodes, &mut copied_bytes)?;
                attach(node.clone(), &mut stack, &mut root)?;
            }
            _ => return Err(invalid()),
        }
    }
    if !stack.is_empty() {
        return Err(invalid());
    }
    let root = root.map_or(Value::Null, |n| n.value);
    let mut order = Vec::new();
    if let Some(Value::Object(topics)) = root.child("routing").and_then(|v| v.child("topics")) {
        for key in topics.keys() {
            let charge = Node::scalar(Value::Null, key.len(), false);
            charge_copy(&charge, &mut replayed_nodes, &mut copied_bytes)?;
            order.push(key.clone());
        }
    }
    Ok((root.into_json(), order))
}
fn boolean(text: &str) -> Option<bool> {
    match text {
        "yes" | "Yes" | "YES" | "true" | "True" | "TRUE" | "on" | "On" | "ON" => Some(true),
        "no" | "No" | "NO" | "false" | "False" | "FALSE" | "off" | "Off" | "OFF" => Some(false),
        _ => None,
    }
}
fn null(text: &str) -> bool {
    matches!(text, "" | "~" | "null" | "Null" | "NULL")
}
fn unsigned(text: &str) -> &str {
    text.strip_prefix(['+', '-']).unwrap_or(text)
}
fn digits(text: &str, radix: u32) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c == '_' || c.is_ascii() && c.is_digit(radix))
}
fn sexagesimal(text: &str, zero_first: bool) -> bool {
    let mut parts = text.split(':');
    let first = parts.next().unwrap_or("");
    if !first.starts_with(|c: char| {
        if zero_first {
            c.is_ascii_digit()
        } else {
            ('1'..='9').contains(&c)
        }
    }) || !digits(first, 10)
    {
        return false;
    }
    let tail: Vec<_> = parts.collect();
    !tail.is_empty()
        && tail.iter().all(|s| {
            (1..=2).contains(&s.len())
                && s.bytes().all(|b| b.is_ascii_digit())
                && s.parse::<u8>().is_ok_and(|n| n < 60)
        })
}
fn implicit_int(text: &str) -> bool {
    let text = unsigned(text);
    if let Some(s) = text.strip_prefix("0b") {
        return digits(s, 2);
    }
    if let Some(s) = text.strip_prefix("0x") {
        return digits(s, 16);
    }
    if let Some(s) = text.strip_prefix('0') {
        return s.is_empty() || digits(s, 8);
    }
    text.starts_with(|c: char| ('1'..='9').contains(&c)) && digits(text, 10)
        || sexagesimal(text, false)
}
fn implicit_float(text: &str) -> bool {
    if matches!(unsigned(text), ".inf" | ".Inf" | ".INF")
        || matches!(text, ".nan" | ".NaN" | ".NAN")
    {
        return true;
    }
    let (mantissa, exponent) = text
        .split_once(['e', 'E'])
        .map_or((text, None), |(a, b)| (a, Some(b)));
    if let Some(exp) = exponent
        && (!exp.starts_with(['+', '-']) || !digits(&exp[1..], 10) || exp[1..].contains('_'))
    {
        return false;
    }
    let Some((left, right)) = mantissa.split_once('.') else {
        return false;
    };
    if !right.bytes().all(|b| b.is_ascii_digit() || b == b'_') {
        return false;
    }
    if left.is_empty() {
        return !right.is_empty() && right.as_bytes()[0].is_ascii_digit();
    }
    let left = unsigned(left);
    if left.contains(':') {
        return exponent.is_none() && sexagesimal(left, true);
    }
    left.starts_with(|c: char| c.is_ascii_digit()) && digits(left, 10)
}
fn integer(text: &str) -> Result<Value> {
    let text = text.replace('_', "");
    let negative = text.starts_with('-');
    let body = unsigned(&text);
    let value = if body.contains(':') {
        body.split(':')
            .try_fold(0i128, |total, part| {
                total.checked_mul(60)?.checked_add(
                    ssx_domain::storage::normalize_python_number_text(part)
                        .ok()?
                        .parse::<i128>()
                        .ok()?,
                )
            })
            .ok_or_else(invalid)?
    } else {
        let (body, radix) = if let Some(s) = body.strip_prefix("0b") {
            (s, 2)
        } else if let Some(s) = body.strip_prefix("0x") {
            (s, 16)
        } else if body.starts_with('0') && body.len() > 1 {
            (
                body.strip_prefix("0o")
                    .or_else(|| body.strip_prefix("0O"))
                    .unwrap_or(body),
                8,
            )
        } else {
            (body, 10)
        };
        // Python explicit int constructors accept Unicode decimal digits.
        let normalized =
            ssx_domain::storage::normalize_python_number_text(body).map_err(|_| invalid())?;
        i128::from_str_radix(&normalized, radix).map_err(|_| invalid())?
    };
    let signed = if negative { -value } else { value };
    let number = if signed < 0 {
        Number::from(i64::try_from(signed).map_err(|_| invalid())?)
    } else {
        Number::from(u64::try_from(signed).map_err(|_| invalid())?)
    };
    Ok(Value::Number(number))
}
fn float(text: &str) -> Result<Value> {
    let text = text.replace('_', "");
    let value = if unsigned(&text).contains(':') {
        let value = unsigned(&text)
            .split(':')
            .try_fold(0.0, |total, part| {
                ssx_domain::storage::normalize_python_number_text(part)
                    .map_err(|_| invalid())?
                    .parse::<f64>()
                    .map(|n| total * 60.0 + n)
                    .map_err(|_| invalid())
            })
            .map_err(|_| invalid())?;
        if text.starts_with('-') { -value } else { value }
    } else {
        ssx_domain::storage::normalize_python_number_text(&text)
            .map_err(|_| invalid())?
            .parse::<f64>()
            .map_err(|_| invalid())?
    };
    Number::from_f64(value)
        .map(Value::Number)
        .ok_or_else(invalid)
}
fn scalar(text: &str, style: ScalarStyle, tag: Option<&Tag>) -> Result<Value> {
    if let Some(tag) = tag {
        return match tag.suffix_in_namespace("tag:yaml.org,2002:").as_deref() {
            Some("str" | "value" | "merge") => Ok(Value::String(text.into())),
            Some("null") => Ok(Value::Null),
            Some("bool") => boolean(&text.to_ascii_lowercase())
                .map(Value::Bool)
                .ok_or_else(invalid),
            Some("int") => integer(text),
            Some("float") => float(text),
            _ if nonspecific(tag) => scalar(text, ScalarStyle::Plain, None),
            _ => Err(invalid()),
        };
    }
    if style != ScalarStyle::Plain {
        return Ok(Value::String(text.into()));
    }
    if null(text) {
        return Ok(Value::Null);
    }
    if let Some(value) = boolean(text) {
        return Ok(Value::Bool(value));
    }
    if implicit_int(text) {
        return integer(text);
    }
    if implicit_float(text) {
        return float(text);
    }
    // Only exact PyYAML timestamp-resolver matches are outside this JSON profile.
    if timestamp(text) {
        return Err(invalid());
    }
    Ok(Value::String(text.into()))
}

fn timestamp(text: &str) -> bool {
    struct Cursor<'a> {
        bytes: &'a [u8],
        at: usize,
    }
    impl Cursor<'_> {
        fn take(&mut self, byte: u8) -> bool {
            if self.bytes.get(self.at) == Some(&byte) {
                self.at += 1;
                true
            } else {
                false
            }
        }
        fn digits(&mut self, min: usize, max: usize) -> usize {
            let start = self.at;
            while self.at - start < max && self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
                self.at += 1;
            }
            if self.at - start < min {
                0
            } else {
                self.at - start
            }
        }
        fn spaces(&mut self) -> usize {
            let start = self.at;
            while self
                .bytes
                .get(self.at)
                .is_some_and(|b| matches!(b, b' ' | b'\t'))
            {
                self.at += 1;
            }
            self.at - start
        }
        fn end(&self) -> bool {
            self.at == self.bytes.len()
        }
    }
    let mut c = Cursor {
        bytes: text.as_bytes(),
        at: 0,
    };
    if c.digits(4, 4) != 4 || !c.take(b'-') {
        return false;
    }
    let month = c.digits(1, 2);
    if month == 0 || !c.take(b'-') {
        return false;
    }
    let day = c.digits(1, 2);
    if day == 0 {
        return false;
    }
    if c.end() {
        return month == 2 && day == 2;
    }
    if !(c.take(b'T') || c.take(b't') || c.spaces() > 0) {
        return false;
    }
    if c.digits(1, 2) == 0
        || !c.take(b':')
        || c.digits(2, 2) != 2
        || !c.take(b':')
        || c.digits(2, 2) != 2
    {
        return false;
    }
    if c.take(b'.') {
        c.digits(0, usize::MAX);
    }
    let before_zone_space = c.at;
    c.spaces();
    if c.end() {
        return before_zone_space == c.at;
    }
    if c.take(b'Z') {
        return c.end();
    }
    if !(c.take(b'+') || c.take(b'-')) || c.digits(1, 2) == 0 {
        return false;
    }
    if c.take(b':') && c.digits(2, 2) != 2 {
        return false;
    }
    c.end()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn captured_python_yaml_resolution_and_composition() {
        let corpus: serde_json::Value =
            serde_json::from_str(include_str!("../../xtask/fixtures/f06/yaml.json")).unwrap();
        for case in corpus["cases"].as_array().unwrap() {
            let result = parse(case["yaml"].as_str().unwrap());
            if case["expected"]["native_error"] == true {
                assert!(result.is_err(), "{}", case["yaml"]);
            } else {
                assert_eq!(
                    result.unwrap_or_else(|e| panic!("{}: {e}", case["yaml"])),
                    case["expected"]["value"],
                    "{}",
                    case["yaml"]
                );
            }
        }
    }
    #[test]
    fn native_copy_budget_is_charged_before_clone_and_depth_is_bounded() {
        let node = Node::scalar(Value::Null, 64 * 1024 * 1024, false);
        let mut nodes = 0;
        let mut bytes = 0;
        charge_copy(&node, &mut nodes, &mut bytes).unwrap();
        assert!(charge_copy(&node, &mut nodes, &mut bytes).is_err());
        let node = Node {
            value: Value::Null,
            nodes: usize::MAX,
            bytes: 0,
            merge: false,
        };
        assert!(charge_copy(&node, &mut nodes, &mut bytes).is_err());
        let deep = format!("{}0{}", "[".repeat(65), "]".repeat(65));
        assert!(parse(&deep).is_err());
    }
}
