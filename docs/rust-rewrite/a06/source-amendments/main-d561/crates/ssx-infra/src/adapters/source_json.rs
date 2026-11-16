//! Bounded Python `json.loads(bytes)` values for adapter source operations.
//!
//! This boundary deliberately does not change the finite public JSON codec.
use std::{future::Future, pin::Pin, time::Instant};

pub const MAX_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_DEPTH: usize = 100;
pub const MAX_TEXT: usize = 4 * 1024 * 1024;
pub const MAX_INTEGER_DIGITS: usize = 4300;

/// Source text can contain Python's unpaired surrogate code points.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Text(Vec<u32>, Option<String>);
impl Text {
    pub fn codepoints(&self) -> &[u32] {
        &self.0
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn scalar(&self) -> Option<String> {
        self.1.clone()
    }
    pub fn as_str(&self) -> Option<&str> {
        self.1.as_deref()
    }
    pub(crate) fn from_points(points: Vec<u32>) -> Self {
        let scalar = points.iter().map(|c| char::from_u32(*c)).collect();
        Self(points, scalar)
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
impl From<&str> for Text {
    fn from(value: &str) -> Self {
        Self(value.chars().map(u32::from).collect(), Some(value.into()))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(Number),
    String(Text),
    Array(Vec<Value>),
    Object(Map),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Integer(String);

#[derive(Clone, Debug, PartialEq)]
pub enum Number {
    Integer(Integer),
    Float(f64),
}
impl Number {
    pub fn is_f64(&self) -> bool {
        matches!(self, Self::Float(_))
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Integer(s) => s.0.parse::<f64>().ok().filter(|f| f.is_finite()),
            Self::Float(f) => Some(*f),
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Integer(s) => s.0.parse().ok(),
            _ => None,
        }
    }
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Integer(s) => s.0.parse().ok(),
            _ => None,
        }
    }
    pub fn integer_text(&self) -> Option<&str> {
        match self {
            Self::Integer(s) => Some(s.0.as_str()),
            _ => None,
        }
    }
    pub fn project(&self) -> Result<ssx_json::Number, ProjectionError> {
        match self {
            Self::Integer(s) => Ok(ssx_json::Number::new(&s.0).expect("parsed integer")),
            Self::Float(f) => {
                let s = ssx_json::python_float(*f).map_err(|_| ProjectionError::Nonfinite)?;
                Ok(ssx_json::Number::new(&s).expect("finite Python float"))
            }
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Map(Vec<(Text, Value)>);
impl Map {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0
            .iter()
            .find(|(k, _)| k.as_str() == Some(key))
            .map(|(_, v)| v)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&Text, &Value)> {
        self.0.iter().map(|(k, v)| (k, v))
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn insert(&mut self, key: Text, value: Value) {
        if let Some((_, v)) = self.0.iter_mut().find(|(k, _)| k == &key) {
            *v = value;
        } else {
            self.0.push((key, value));
        }
    }
}
impl From<String> for Text {
    fn from(value: String) -> Self {
        Self::from(value.as_str())
    }
}
impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => s.as_str(),
            _ => None,
        }
    }
    pub fn as_text(&self) -> Option<&Text> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_object(&self) -> Option<&Map> {
        match self {
            Self::Object(o) => Some(o),
            _ => None,
        }
    }
    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(n) => n.as_f64(),
            _ => None,
        }
    }
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
    pub fn is_array(&self) -> bool {
        matches!(self, Self::Array(_))
    }
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_object()?.get(key)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Syntax {
        position: usize,
        message: &'static str,
    },
    Encoding {
        position: usize,
    },
    IntegerDigits(usize),
    ByteLimit,
    DepthLimit,
    TextLimit,
    Deadline,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionError {
    Nonfinite,
    Unicode,
}
impl Error {
    pub fn message(&self, engine: &str, malformed: &str) -> String {
        match self {
            Self::ByteLimit => format!("{engine} response exceeds JSON byte limit (4194304 bytes)"),
            Self::DepthLimit => format!("{engine} response exceeds JSON depth limit (100 levels)"),
            Self::TextLimit => {
                format!("{engine} response exceeds JSON decoded-text limit (4194304 code points)")
            }
            Self::IntegerDigits(digits) => format!(
                "Exceeds the limit (4300 digits) for integer string conversion: value has {digits} digits; use sys.set_int_max_str_digits() to increase the limit"
            ),
            Self::Syntax { .. } | Self::Encoding { .. } => malformed.into(),
            Self::Deadline => unreachable!("deadline must retain timeout status"),
        }
    }
}
impl Value {
    /// Projection occurs only after source operations have succeeded.
    pub fn project(&self) -> Result<ssx_json::Value, ProjectionError> {
        Ok(match self {
            Self::Null => ssx_json::Value::Null,
            Self::Bool(b) => ssx_json::Value::Bool(*b),
            Self::Number(n) => ssx_json::Value::Number(n.project()?),
            Self::String(s) => ssx_json::Value::String(s.scalar().ok_or(ProjectionError::Unicode)?),
            Self::Array(a) => {
                ssx_json::Value::Array(a.iter().map(Self::project).collect::<Result<_, _>>()?)
            }
            Self::Object(o) => {
                let mut out = ssx_json::Map::new();
                for (k, v) in o.iter() {
                    out.insert(k.scalar().ok_or(ProjectionError::Unicode)?, v.project()?);
                }
                ssx_json::Value::Object(out)
            }
        })
    }
}

#[derive(Clone, Copy)]
enum Encoding {
    Utf8,
    Utf16(bool),
    Utf32(bool),
}
fn encoding(bytes: &[u8]) -> (Encoding, usize) {
    if bytes.starts_with(&[0, 0, 254, 255]) {
        (Encoding::Utf32(false), 4)
    } else if bytes.starts_with(&[255, 254, 0, 0]) {
        (Encoding::Utf32(true), 4)
    } else if bytes.starts_with(&[254, 255]) {
        (Encoding::Utf16(false), 2)
    } else if bytes.starts_with(&[255, 254]) {
        (Encoding::Utf16(true), 2)
    } else if bytes.starts_with(&[239, 187, 191]) {
        (Encoding::Utf8, 3)
    } else if bytes.len() >= 4 && bytes[0] == 0 {
        (
            if bytes[1] != 0 {
                Encoding::Utf16(false)
            } else {
                Encoding::Utf32(false)
            },
            0,
        )
    } else if bytes.len() >= 4 && bytes[1] == 0 {
        (
            if bytes[2] != 0 || bytes[3] != 0 {
                Encoding::Utf16(true)
            } else {
                Encoding::Utf32(true)
            },
            0,
        )
    } else if bytes.len() == 2 && bytes[0] == 0 {
        (Encoding::Utf16(false), 0)
    } else if bytes.len() == 2 && bytes[1] == 0 {
        (Encoding::Utf16(true), 0)
    } else {
        (Encoding::Utf8, 0)
    }
}
async fn checkpoint(deadline: Option<Instant>) -> Result<(), Error> {
    if deadline.is_some_and(|d| Instant::now() >= d) {
        return Err(Error::Deadline);
    }
    tokio::task::yield_now().await;
    if deadline.is_some_and(|d| Instant::now() >= d) {
        return Err(Error::Deadline);
    }
    Ok(())
}
async fn decode(bytes: &[u8], deadline: Option<Instant>) -> Result<Vec<u32>, Error> {
    let (enc, mut pos) = encoding(bytes);
    let mut out = Vec::new();
    let mut next_yield = pos;
    while pos < bytes.len() {
        if pos >= next_yield {
            checkpoint(deadline).await?;
            next_yield = pos + 4096;
        }
        let start = pos;
        let bad = || Error::Encoding { position: start };
        let point = match enc {
            Encoding::Utf8 => {
                let lead = bytes[pos];
                pos += 1;
                if lead < 128 {
                    u32::from(lead)
                } else {
                    let (count, mut c, min) = match lead {
                        0xc2..=0xdf => (1, u32::from(lead & 31), 0x80),
                        0xe0..=0xef => (2, u32::from(lead & 15), 0x800),
                        0xf0..=0xf4 => (3, u32::from(lead & 7), 0x10000),
                        _ => return Err(bad()),
                    };
                    for _ in 0..count {
                        let b = *bytes.get(pos).ok_or_else(bad)?;
                        if b & 0xc0 != 0x80 {
                            return Err(bad());
                        }
                        c = (c << 6) | u32::from(b & 63);
                        pos += 1;
                    }
                    if c < min || c > 0x10ffff {
                        return Err(bad());
                    }
                    c
                }
            }
            Encoding::Utf16(le) => {
                let pair: [u8; 2] = bytes
                    .get(pos..pos + 2)
                    .ok_or_else(bad)?
                    .try_into()
                    .expect("two bytes");
                let c = if le {
                    u16::from_le_bytes(pair)
                } else {
                    u16::from_be_bytes(pair)
                };
                pos += 2;
                if (0xd800..=0xdbff).contains(&c) && pos + 2 <= bytes.len() {
                    let pair: [u8; 2] = bytes[pos..pos + 2].try_into().expect("two bytes");
                    let d = if le {
                        u16::from_le_bytes(pair)
                    } else {
                        u16::from_be_bytes(pair)
                    };
                    if (0xdc00..=0xdfff).contains(&d) {
                        pos += 2;
                        0x10000 + ((u32::from(c) - 0xd800) << 10) + u32::from(d) - 0xdc00
                    } else {
                        u32::from(c)
                    }
                } else {
                    u32::from(c)
                }
            }
            Encoding::Utf32(le) => {
                let quad: [u8; 4] = bytes
                    .get(pos..pos + 4)
                    .ok_or_else(bad)?
                    .try_into()
                    .expect("four bytes");
                pos += 4;
                let c = if le {
                    u32::from_le_bytes(quad)
                } else {
                    u32::from_be_bytes(quad)
                };
                if c > 0x10ffff {
                    return Err(bad());
                }
                c
            }
        };
        out.push(point);
        if out.len() > MAX_TEXT {
            return Err(Error::TextLimit);
        }
    }
    Ok(out)
}

/// No work survives dropping this future. Deadlines are inherited, never renewed.
pub async fn from_slice(bytes: &[u8], deadline: Option<Instant>) -> Result<Value, Error> {
    if bytes.len() > MAX_BYTES {
        return Err(Error::ByteLimit);
    }
    let text = decode(bytes, deadline).await?;
    let mut parser = Parser {
        text: &text,
        pos: 0,
        work: 0,
        deadline,
    };
    let value = parser.value(0).await?;
    parser.whitespace().await?;
    if parser.pos != text.len() {
        return Err(parser.syntax("Extra data"));
    }
    checkpoint(deadline).await?;
    Ok(value)
}
struct Parser<'a> {
    text: &'a [u32],
    pos: usize,
    work: usize,
    deadline: Option<Instant>,
}
impl Parser<'_> {
    fn syntax(&self, message: &'static str) -> Error {
        Error::Syntax {
            position: self.pos,
            message,
        }
    }
    async fn tick(&mut self) -> Result<(), Error> {
        if self.pos >= self.work {
            checkpoint(self.deadline).await?;
            self.work = self.pos + 4096;
        }
        Ok(())
    }
    async fn whitespace(&mut self) -> Result<(), Error> {
        while self
            .text
            .get(self.pos)
            .is_some_and(|c| matches!(c, 0x20 | 0x09 | 0x0a | 0x0d))
        {
            self.pos += 1;
            self.tick().await?;
        }
        Ok(())
    }
    fn consume(&mut self, token: &str) -> bool {
        if self
            .text
            .get(self.pos..self.pos + token.len())
            .is_some_and(|s| s.iter().copied().eq(token.bytes().map(u32::from)))
        {
            self.pos += token.len();
            true
        } else {
            false
        }
    }
    fn value(
        &mut self,
        depth: usize,
    ) -> Pin<Box<dyn Future<Output = Result<Value, Error>> + Send + '_>> {
        Box::pin(async move {
            self.whitespace().await?;
            self.tick().await?;
            match self.text.get(self.pos).copied() {
                Some(0x22) => Ok(Value::String(self.string().await?)),
                Some(0x5b) => {
                    if depth >= MAX_DEPTH {
                        return Err(Error::DepthLimit);
                    }
                    self.pos += 1;
                    self.whitespace().await?;
                    let mut out = Vec::new();
                    if self.consume("]") {
                        return Ok(Value::Array(out));
                    }
                    loop {
                        out.push(self.value(depth + 1).await?);
                        self.whitespace().await?;
                        if self.consume("]") {
                            return Ok(Value::Array(out));
                        }
                        if !self.consume(",") {
                            return Err(self.syntax("Expecting ',' delimiter"));
                        }
                    }
                }
                Some(0x7b) => {
                    if depth >= MAX_DEPTH {
                        return Err(Error::DepthLimit);
                    }
                    self.pos += 1;
                    self.whitespace().await?;
                    let mut out: Vec<(Text, Value)> = Vec::new();
                    let mut positions: std::collections::BTreeMap<Text, usize> =
                        std::collections::BTreeMap::new();
                    if self.consume("}") {
                        return Ok(Value::Object(Map(out)));
                    }
                    loop {
                        self.whitespace().await?;
                        if self.text.get(self.pos) != Some(&0x22) {
                            return Err(
                                self.syntax("Expecting property name enclosed in double quotes")
                            );
                        }
                        let key = self.string().await?;
                        self.whitespace().await?;
                        if !self.consume(":") {
                            return Err(self.syntax("Expecting ':' delimiter"));
                        }
                        let value = self.value(depth + 1).await?;
                        if let Some(index) = positions.get(&key).copied() {
                            out[index].1 = value;
                        } else {
                            positions.insert(key.clone(), out.len());
                            out.push((key, value));
                        }
                        self.whitespace().await?;
                        if self.consume("}") {
                            return Ok(Value::Object(Map(out)));
                        }
                        if !self.consume(",") {
                            return Err(self.syntax("Expecting ',' delimiter"));
                        }
                    }
                }
                Some(0x6e) if self.consume("null") => Ok(Value::Null),
                Some(0x74) if self.consume("true") => Ok(Value::Bool(true)),
                Some(0x66) if self.consume("false") => Ok(Value::Bool(false)),
                Some(0x4e) if self.consume("NaN") => Ok(Value::Number(Number::Float(f64::NAN))),
                Some(0x49) if self.consume("Infinity") => {
                    Ok(Value::Number(Number::Float(f64::INFINITY)))
                }
                Some(0x2d) if self.consume("-Infinity") => {
                    Ok(Value::Number(Number::Float(f64::NEG_INFINITY)))
                }
                Some(0x2d | 0x30..=0x39) => self.number().await,
                _ => Err(self.syntax("Expecting value")),
            }
        })
    }
    async fn string(&mut self) -> Result<Text, Error> {
        self.pos += 1;
        let mut out = Vec::new();
        loop {
            self.tick().await?;
            let c = *self
                .text
                .get(self.pos)
                .ok_or_else(|| self.syntax("Unterminated string starting at"))?;
            self.pos += 1;
            match c {
                0x22 => return Ok(Text::from_points(out)),
                0..=0x1f => return Err(self.syntax("Invalid control character at")),
                0x5c => {
                    let escape = *self
                        .text
                        .get(self.pos)
                        .ok_or_else(|| self.syntax("Unterminated string starting at"))?;
                    self.pos += 1;
                    let c = match escape {
                        0x22 | 0x5c | 0x2f => escape,
                        0x62 => 8,
                        0x66 => 12,
                        0x6e => 10,
                        0x72 => 13,
                        0x74 => 9,
                        0x75 => {
                            let c = self.hex()?;
                            if (0xd800..=0xdbff).contains(&c)
                                && self.text.get(self.pos..self.pos + 2) == Some(&[0x5c, 0x75])
                            {
                                let save = self.pos;
                                self.pos += 2;
                                match self.hex() {
                                    Ok(d) if (0xdc00..=0xdfff).contains(&d) => {
                                        0x10000 + ((c - 0xd800) << 10) + d - 0xdc00
                                    }
                                    _ => {
                                        self.pos = save;
                                        c
                                    }
                                }
                            } else {
                                c
                            }
                        }
                        _ => return Err(self.syntax("Invalid \\escape")),
                    };
                    out.push(c);
                }
                _ => out.push(c),
            }
        }
    }
    fn hex(&mut self) -> Result<u32, Error> {
        let mut n = 0;
        for _ in 0..4 {
            let c = *self
                .text
                .get(self.pos)
                .ok_or_else(|| self.syntax("Invalid \\uXXXX escape"))?;
            let digit = match c {
                0x30..=0x39 => c - 0x30,
                0x61..=0x66 => c - 0x61 + 10,
                0x41..=0x46 => c - 0x41 + 10,
                _ => return Err(self.syntax("Invalid \\uXXXX escape")),
            };
            n = n * 16 + digit;
            self.pos += 1;
        }
        Ok(n)
    }
    async fn number(&mut self) -> Result<Value, Error> {
        let start = self.pos;
        self.consume("-");
        if !self.consume("0") {
            if !self
                .text
                .get(self.pos)
                .is_some_and(|c| (0x31..=0x39).contains(c))
            {
                return Err(self.syntax("Expecting value"));
            }
            while self
                .text
                .get(self.pos)
                .is_some_and(|c| (0x30..=0x39).contains(c))
            {
                self.pos += 1;
                self.tick().await?;
            }
        }
        let integer_end = self.pos;
        if self.text.get(self.pos) == Some(&0x2e)
            && self
                .text
                .get(self.pos + 1)
                .is_some_and(|c| (0x30..=0x39).contains(c))
        {
            self.pos += 1;
            while self
                .text
                .get(self.pos)
                .is_some_and(|c| (0x30..=0x39).contains(c))
            {
                self.pos += 1;
                self.tick().await?;
            }
        }
        if self
            .text
            .get(self.pos)
            .is_some_and(|c| matches!(c, 0x65 | 0x45))
        {
            let save = self.pos;
            self.pos += 1;
            if self
                .text
                .get(self.pos)
                .is_some_and(|c| matches!(c, 0x2b | 0x2d))
            {
                self.pos += 1;
            }
            let digits = self.pos;
            while self
                .text
                .get(self.pos)
                .is_some_and(|c| (0x30..=0x39).contains(c))
            {
                self.pos += 1;
                self.tick().await?;
            }
            if self.pos == digits {
                self.pos = save;
            }
        }
        let mut s = String::with_capacity(self.pos - start);
        for (index, codepoint) in self.text[start..self.pos].iter().enumerate() {
            if index & 4095 == 0 {
                checkpoint(self.deadline).await?;
            }
            s.push(char::from_u32(*codepoint).expect("ASCII number"));
        }
        if self.pos == integer_end {
            let digits = s.len() - usize::from(s.starts_with('-'));
            if digits > MAX_INTEGER_DIGITS {
                return Err(Error::IntegerDigits(digits));
            }
            Ok(Value::Number(Number::Integer(Integer(if s == "-0" {
                "0".into()
            } else {
                s
            }))))
        } else {
            let mut normalizer = ssx_json::FloatNormalizer::new(&s)
                .map_err(|_| self.syntax("invalid JSON float"))?;
            while !normalizer
                .advance(1024)
                .map_err(|_| self.syntax("invalid JSON float"))?
            {
                checkpoint(self.deadline).await?;
            }
            checkpoint(self.deadline).await?;
            let value = normalizer
                .finish()
                .map_err(|_| self.syntax("invalid JSON float"))?;
            checkpoint(self.deadline).await?;
            Ok(Value::Number(Number::Float(value)))
        }
    }
}

impl Text {
    pub fn take(&self, count: usize) -> Self {
        Self::from_points(self.0.iter().copied().take(count).collect())
    }
    pub fn starts_with(&self, prefix: &Self) -> bool {
        self.0.starts_with(&prefix.0)
    }
    pub fn starts_with_ascii(&self, prefix: &str) -> bool {
        self.0
            .iter()
            .copied()
            .take(prefix.len())
            .eq(prefix.bytes().map(u32::from))
            && self.0.len() >= prefix.len()
    }
    pub fn replace_ascii(&self, from: u32, to: u32) -> Self {
        Self::from_points(
            self.0
                .iter()
                .map(|c| if *c == from { to } else { *c })
                .collect(),
        )
    }
    pub fn concat(parts: &[Self]) -> Self {
        Self::from_points(parts.iter().flat_map(|s| s.0.iter().copied()).collect())
    }
    pub fn collapse_whitespace(&self) -> Self {
        let mut out = Vec::new();
        let mut pending = false;
        for c in &self.0 {
            if char::from_u32(*c).is_some_and(super::broad_whitespace) {
                pending = !out.is_empty();
            } else {
                if pending {
                    out.push(32);
                    pending = false;
                }
                out.push(*c);
            }
        }
        Self::from_points(out)
    }
    pub fn repr(&self) -> String {
        if let Some(s) = self.as_str() {
            let escaped = ssx_domain::storage::python_value_string(&ssx_json::json!([s]));
            return escaped[1..escaped.len() - 1].into();
        }
        let quote = if self.0.contains(&39) && !self.0.contains(&34) {
            '"'
        } else {
            '\''
        };
        let mut out = String::from(quote);
        for code in &self.0 {
            if *code == u32::from(quote) {
                out.push('\\');
                out.push(quote);
            } else if *code == 39 || *code == 34 {
                out.push(char::from_u32(*code).expect("quote"));
            } else if let Some(ch) = char::from_u32(*code) {
                // Delegate scalar printability/escaping to the accepted Unicode16 formatter.
                let one = ssx_json::json!([ch.to_string()]);
                let escaped = ssx_domain::storage::python_value_string(&one);
                out.push_str(&escaped[2..escaped.len() - 2]);
            } else {
                out.push_str(&format!("\\u{code:04x}"));
            }
        }
        out.push(quote);
        out
    }
}
impl Value {
    pub fn python_string(&self) -> Text {
        if let Self::String(s) = self {
            s.clone()
        } else {
            Text::from(self.repr())
        }
    }
    pub fn repr(&self) -> String {
        match self {
            Self::Null => "None".into(),
            Self::Bool(b) => if *b { "True" } else { "False" }.into(),
            Self::Number(Number::Integer(s)) => s.0.clone(),
            Self::Number(Number::Float(f)) => {
                if f.is_nan() {
                    "nan".into()
                } else if *f == f64::INFINITY {
                    "inf".into()
                } else if *f == f64::NEG_INFINITY {
                    "-inf".into()
                } else {
                    ssx_json::python_float(*f).expect("finite float")
                }
            }
            Self::String(s) => s.repr(),
            Self::Array(a) => format!(
                "[{}]",
                a.iter().map(Self::repr).collect::<Vec<_>>().join(", ")
            ),
            Self::Object(o) => format!(
                "{{{}}}",
                o.iter()
                    .map(|(k, v)| format!("{}: {}", k.repr(), v.repr()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
    pub fn integer(value: i64) -> Self {
        Self::Number(Number::Integer(Integer(value.to_string())))
    }
}
impl Text {
    pub fn utf8_error(&self) -> Option<String> {
        let start = self.0.iter().position(|c| (0xd800..=0xdfff).contains(c))?;
        let end = start
            + self.0[start..]
                .iter()
                .take_while(|c| (0xd800..=0xdfff).contains(*c))
                .count();
        Some(if end == start + 1 {
            format!(
                "'utf-8' codec can't encode character '{}' in position {start}: surrogates not allowed",
                format_args!("\\u{:04x}", self.0[start])
            )
        } else {
            format!(
                "'utf-8' codec can't encode characters in position {start}-{}: surrogates not allowed",
                end - 1
            )
        })
    }
}

/// Sanitize source media text before requiring Unicode scalar output.
/// Scalar URLs always use the accepted domain sanitizer. Extended text can
/// become scalar only when every surrogate occurs in a removed secret value.
pub fn scalar_media_url(text: &Text) -> Option<String> {
    if let Some(s) = text.as_str() {
        return ssx_domain::storage::build_media("image", Some(s), None, None, None, None, None)?
            .url;
    }
    let cleaned: Vec<u32> = text
        .0
        .iter()
        .copied()
        .skip_while(|c| *c <= 32)
        .filter(|c| !matches!(c, 9 | 10 | 13))
        .collect();
    let fragment = cleaned
        .iter()
        .position(|c| *c == 35)
        .unwrap_or(cleaned.len());
    let query = cleaned[..fragment].iter().position(|c| *c == 63)?;
    let prefix = Text::from_points(cleaned[..query].to_vec()).scalar()?;
    let suffix = Text::from_points(cleaned[fragment..].to_vec()).scalar()?;
    let mut kept = Vec::new();
    for pair in cleaned[query + 1..fragment].split(|c| *c == 38) {
        let key_end = pair.iter().position(|c| *c == 61).unwrap_or(pair.len());
        let raw_key = Text::from_points(pair[..key_end].to_vec()).scalar()?;
        let key = url::form_urlencoded::parse(raw_key.as_bytes())
            .next()
            .map(|(k, _)| k.into_owned())
            .unwrap_or_default();
        if matches!(
            key.as_str(),
            "api_key" | "key" | "apiKey" | "token" | "access_token"
        ) {
            continue;
        }
        kept.push(Text::from_points(pair.to_vec()).scalar()?);
    }
    // The existing parser has an intentional original-input fallback. Prove
    // parsing succeeded using a real sensitive parameter that must be removed;
    // this temporary URL is never returned or sent to any transport.
    let probe = format!("{prefix}?{}&token={suffix}", kept.join("&"));
    let checked =
        ssx_domain::storage::build_media("image", Some(&probe), None, None, None, None, None)?;
    if checked.url.as_deref() == Some(probe.as_str()) {
        return None;
    }
    checked.url
}
pub fn opaque_error(value: &Value, engine: &str, field: &str) -> Option<String> {
    value.project().err().map(|error| match error {
        ProjectionError::Nonfinite => {
            format!("{engine} {field} must contain only finite JSON numbers")
        }
        ProjectionError::Unicode => {
            format!("{engine} {field} must contain only Unicode scalar strings")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn encodings_and_recursive_float_projection() {
        let input = "{\"x\":[1.234567890123456789,1e3,1e-7,-0.0,1e-9999,99999999999999999999999999999999999999999]}";
        let mut forms = vec![
            input.as_bytes().to_vec(),
            [vec![239, 187, 191], input.as_bytes().to_vec()].concat(),
        ];
        for le in [false, true] {
            let utf16: Vec<u8> = input
                .encode_utf16()
                .flat_map(|c| if le { c.to_le_bytes() } else { c.to_be_bytes() })
                .collect();
            forms.push(utf16.clone());
            forms.push([if le { vec![255, 254] } else { vec![254, 255] }, utf16].concat());
            let utf32: Vec<u8> = input
                .chars()
                .flat_map(|c| {
                    if le {
                        u32::from(c).to_le_bytes()
                    } else {
                        u32::from(c).to_be_bytes()
                    }
                })
                .collect();
            forms.push(utf32.clone());
            forms.push(
                [
                    if le {
                        vec![255, 254, 0, 0]
                    } else {
                        vec![0, 0, 254, 255]
                    },
                    utf32,
                ]
                .concat(),
            );
        }
        let expected = r#"{"x":[1.2345678901234567,1000.0,1e-07,-0.0,0.0,99999999999999999999999999999999999999999]}"#;
        for bytes in forms {
            let v = from_slice(&bytes, None).await.unwrap();
            assert_eq!(
                ssx_json::to_string(&v.project().unwrap()).unwrap(),
                expected
            );
        }
    }
    #[tokio::test]
    async fn source_nonfinite_and_surrogates_are_data() {
        let value = from_slice(br#"[NaN,Infinity,-Infinity,1e309,-1e309,"\ud800","\ud800\udc00",{"type":"float","value":"Infinity"}]"#,None).await.unwrap();
        let Value::Array(a) = value else {
            panic!("array")
        };
        assert!(matches!(a[0],Value::Number(Number::Float(f)) if f.is_nan()));
        assert_eq!(a[1].project(), Err(ProjectionError::Nonfinite));
        assert_eq!(a[5], Value::String(Text::from_points(vec![0xd800])));
        assert_eq!(a[6].project().unwrap(), ssx_json::json!("\u{10000}"));
        assert_eq!(
            a[7].project().unwrap(),
            ssx_json::json!({"type":"float","value":"Infinity"})
        );
        let raw = [b"\"".as_slice(), &[0xed, 0xa0, 0x80], b"\""].concat();
        assert_eq!(from_slice(&raw, None).await.unwrap(), a[5]);
    }
    #[tokio::test]
    async fn strict_grammar_and_byte_failures() {
        for input in [
            b"01".as_slice(),
            b"-01",
            b"1.",
            b"1e",
            b"1e+",
            b"+1",
            b".1",
            b"[1,]",
            b"{\"x\":1,}",
            b"[nan]",
            b"[inf]",
            b"\"\x01\"",
            br#""\uQ000""#,
            br#""\x20""#,
        ] {
            assert!(
                matches!(from_slice(input, None).await, Err(Error::Syntax { .. })),
                "{input:?}"
            );
        }
        for input in [
            &[0xff][..],
            &[0xff, 0xfe, 0x22],
            &[0, 0, 0xfe, 0xff, 0],
            &[0xf4, 0x90, 0x80, 0x80],
            &[0xc0, 0x80],
        ] {
            assert!(
                matches!(from_slice(input, None).await, Err(Error::Encoding { .. })),
                "{input:?}"
            );
        }
        assert_eq!(
            from_slice(b"{\"x\":1,\"y\":2,\"x\":3}", None)
                .await
                .unwrap()
                .project()
                .unwrap(),
            ssx_json::json!({"x":3,"y":2})
        );
    }
    #[tokio::test]
    async fn bounded_depth_bytes_integer_digits_and_deadline() {
        let nested = |depth: usize| format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
        assert!(from_slice(nested(MAX_DEPTH).as_bytes(), None).await.is_ok());
        assert_eq!(
            from_slice(nested(MAX_DEPTH + 1).as_bytes(), None).await,
            Err(Error::DepthLimit)
        );
        assert!(from_slice("9".repeat(4300).as_bytes(), None).await.is_ok());
        assert_eq!(
            from_slice("9".repeat(4301).as_bytes(), None).await,
            Err(Error::IntegerDigits(4301))
        );
        assert!(
            from_slice(format!("-{}", "9".repeat(4300)).as_bytes(), None)
                .await
                .is_ok()
        );
        assert!(
            from_slice(format!("{}e0", "9".repeat(4301)).as_bytes(), None)
                .await
                .is_ok()
        );
        assert!(matches!(
            from_slice(b"00", None).await,
            Err(Error::Syntax { .. })
        ));
        let bytes = vec![b' '; MAX_BYTES];
        assert!(matches!(
            from_slice(&bytes, None).await,
            Err(Error::Syntax { .. })
        ));
        assert_eq!(
            from_slice(&vec![b' '; MAX_BYTES + 1], None).await,
            Err(Error::ByteLimit)
        );
        assert_eq!(
            from_slice(b"null", Some(Instant::now())).await,
            Err(Error::Deadline)
        );
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(1),
            from_slice(&bytes, None),
        )
        .await;
        assert!(result.is_err(), "decoder must yield to cancellation");
    }
}

#[cfg(test)]
mod f15_numeric_tests {
    use super::*;
    #[tokio::test]
    async fn shared_float_normalizer_preserves_long_finite_and_nonfinite_source_policy() {
        use sha2::{Digest, Sha256};
        let fixture: ssx_json::Value = ssx_json::from_slice(include_bytes!(
            "../../../../docs/rust-rewrite/f15/cases.json"
        ))
        .unwrap();
        let mut count = 0;
        for case in fixture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["kind"] == "number")
        {
            let input = &case["input"];
            let raw = if let Some(text) = input["text"].as_str() {
                text.to_owned()
            } else if let Some(zeros) = input["zeros"].as_u64() {
                format!("0.{}1e+{zeros}", "0".repeat(zeros as usize))
            } else {
                format!(
                    "0.{}",
                    input["repeat"]
                        .as_str()
                        .unwrap()
                        .repeat(input["count"].as_u64().unwrap() as usize)
                )
            };
            if let Some(expected) = case["input_sha256"].as_str() {
                assert_eq!(
                    Sha256::digest(raw.as_bytes())
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>(),
                    expected
                );
            }
            let expected = case["output"]["bits"].as_str().unwrap();
            let number = ssx_json::parse_json_f64(&raw).unwrap();
            assert_eq!(
                format!("{:016x}", number.to_bits()),
                expected,
                "{} primitive",
                case["id"]
            );
            let source = from_slice(raw.as_bytes(), None).await.unwrap();
            assert_eq!(
                format!("{:016x}", source.as_f64().unwrap().to_bits()),
                expected,
                "{} SourceJSON",
                case["id"]
            );
            if case["output"]["finite"].as_bool().unwrap() {
                let value = ssx_json::Number::new(&raw).unwrap();
                assert_eq!(
                    format!("{:016x}", value.as_f64().unwrap().to_bits()),
                    expected,
                    "{} publicNumber",
                    case["id"]
                );
            } else {
                assert!(ssx_json::Number::new(&raw).is_err());
                assert!(source.project().is_err());
            }
            count += 1;
        }
        assert_eq!(count, 14);
    }
}
