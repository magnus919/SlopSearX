//! Python json.loads/json.dumps(indent=2) compatibility at this wire-only client.
//!
//! Serde and ssx-json deliberately reject nonfinite tokens/lone surrogates. This
//! small recursive-descent adapter preserves those values without weakening the
//! shared codecs. Finite floating conversion/formatting reuses ssx-json. Strings
//! store Unicode codepoints (including escaped surrogates), objects preserve first
//! insertion position with last-value-wins duplicate keys, integers never use f64.
use crate::Failure;
use std::collections::HashMap;
pub enum Value {
    Null,
    Bool(bool),
    Number(String),
    String(Vec<u32>),
    Array(Vec<Value>),
    Object(Vec<(Vec<u32>, Value)>),
}
pub fn text(s: &str) -> Vec<u32> {
    s.chars().map(u32::from).collect()
}
pub fn string(s: &str) -> Value {
    Value::String(text(s))
}
impl Value {
    pub fn get(&self, key: &str) -> Result<Option<&Self>, Failure> {
        match self {
            Self::Object(v) => {
                let key = text(key);
                Ok(v.iter().find(|(k, _)| *k == key).map(|(_, v)| v))
            }
            _ => Err(Failure::Shape),
        }
    }
    pub fn object(&self) -> Result<&[(Vec<u32>, Self)], Failure> {
        if let Self::Object(v) = self {
            Ok(v)
        } else {
            Err(Failure::Shape)
        }
    }
    pub fn sequence(&self) -> Result<Vec<Value>, Failure> {
        match self {
            Self::Array(v) => Ok(v.clone()),
            Self::String(v) => Ok(v.iter().map(|c| Self::String(vec![*c])).collect()),
            Self::Object(v) => Ok(v.iter().map(|(k, _)| Self::String(k.clone())).collect()),
            _ => Err(Failure::Shape),
        }
    }
    pub fn sorted_sequence(&self) -> Result<Vec<Self>, Failure> {
        let mut values = self.sequence()?;
        if values.iter().all(|v| matches!(v, Self::String(_))) {
            values.sort_by(|a, b| match (a, b) {
                (Self::String(a), Self::String(b)) => a.cmp(b),
                _ => unreachable!(),
            });
            return Ok(values);
        }
        // Non-string category values follow the source's admitted ordering where
        // it is a total order; unordered NaN has an explicit reviewed disposition.
        fallible_stable_sort_by(&mut values, compare)?;
        Ok(values)
    }
    fn repr(&self) -> Result<String, Failure> {
        enum Part<'a> {
            Value(&'a Value),
            Key(&'a [u32]),
            Text(&'static str),
        }
        fn quote(points: &[u32], out: &mut String) -> Result<(), Failure> {
            let q = if points.contains(&39) && !points.contains(&34) {
                34
            } else {
                39
            };
            out.push(char::from_u32(q).expect("quote"));
            for &cp in points {
                match cp {
                    92 => out.push_str(r"\\"),
                    10 => out.push_str(r"\n"),
                    13 => out.push_str(r"\r"),
                    9 => out.push_str(r"\t"),
                    cp if cp == q => {
                        out.push('\\');
                        out.push(char::from_u32(cp).expect("quote"));
                    }
                    cp if !(32..127).contains(&cp)
                        && crate::decimal::NONPRINTABLE
                            .iter()
                            .any(|&(a, b)| (a..=b).contains(&cp)) =>
                    {
                        out.push_str(&if cp < 256 {
                            format!("\\x{cp:02x}")
                        } else if cp < 65536 {
                            format!("\\u{cp:04x}")
                        } else {
                            format!("\\U{cp:08x}")
                        })
                    }
                    cp => out.push(char::from_u32(cp).ok_or(Failure::Encoding)?),
                }
            }
            out.push(char::from_u32(q).expect("quote"));
            Ok(())
        }
        let mut out = String::new();
        let mut pending = vec![Part::Value(self)];
        while let Some(next) = pending.pop() {
            match next {
                Part::Text(s) => out.push_str(s),
                Part::Key(k) => quote(k, &mut out)?,
                Part::Value(v) => match v {
                    Self::Null => out.push_str("None"),
                    Self::Bool(v) => out.push_str(if *v { "True" } else { "False" }),
                    Self::Number(v) => out.push_str(match v.as_str() {
                        "NaN" => "nan",
                        "Infinity" => "inf",
                        "-Infinity" => "-inf",
                        _ => v,
                    }),
                    Self::String(v) => quote(v, &mut out)?,
                    Self::Array(values) => {
                        out.push('[');
                        pending.push(Part::Text("]"));
                        for (i, v) in values.iter().enumerate().rev() {
                            pending.push(Part::Value(v));
                            if i > 0 {
                                pending.push(Part::Text(", "));
                            }
                        }
                    }
                    Self::Object(values) => {
                        out.push('{');
                        pending.push(Part::Text("}"));
                        for (i, (k, v)) in values.iter().enumerate().rev() {
                            pending.push(Part::Value(v));
                            pending.push(Part::Text(": "));
                            pending.push(Part::Key(k));
                            if i > 0 {
                                pending.push(Part::Text(", "));
                            }
                        }
                    }
                },
            }
        }
        Ok(out)
    }
    pub fn display(&self) -> Result<String, Failure> {
        if let Self::String(v) = self {
            points(v)
        } else {
            self.repr()
        }
    }
}

// Sort indices first: an error leaves the input untouched. Left-run ties retain
// source insertion order; each merge level compares at most n elements. Only
// after every fallible comparison succeeds do we apply the index permutation.
fn fallible_stable_sort_by<T>(
    values: &mut [T],
    mut compare: impl FnMut(&T, &T) -> Result<std::cmp::Ordering, Failure>,
) -> Result<(), Failure> {
    let n = values.len();
    if n < 2 {
        return Ok(());
    }
    let mut order: Vec<usize> = (0..n).collect();
    let mut buffer = vec![0; n];
    let mut width = 1usize;
    while width < n {
        let mut start = 0;
        while start < n {
            let middle = start.saturating_add(width).min(n);
            let end = middle.saturating_add(width).min(n);
            let (mut left, mut right, mut out) = (start, middle, start);
            while left < middle && right < end {
                if compare(&values[order[left]], &values[order[right]])?
                    != std::cmp::Ordering::Greater
                {
                    buffer[out] = order[left];
                    left += 1;
                } else {
                    buffer[out] = order[right];
                    right += 1;
                }
                out += 1;
            }
            buffer[out..out + middle - left].copy_from_slice(&order[left..middle]);
            out += middle - left;
            buffer[out..out + end - right].copy_from_slice(&order[right..end]);
            start = end;
        }
        std::mem::swap(&mut order, &mut buffer);
        width = width.saturating_mul(2);
    }
    // order maps destination -> original; invert it before moving values.
    for (destination, &original) in order.iter().enumerate() {
        buffer[original] = destination;
    }
    for i in 0..n {
        while buffer[i] != i {
            let destination = buffer[i];
            values.swap(i, destination);
            buffer.swap(i, destination);
        }
    }
    Ok(())
}

pub fn points(s: &[u32]) -> Result<String, Failure> {
    s.iter()
        .map(|&c| char::from_u32(c).ok_or(Failure::Encoding))
        .collect()
}
struct Parser<'a> {
    s: &'a str,
    pos: usize,
}
impl Parser<'_> {
    fn ws(&mut self) {
        while self
            .s
            .as_bytes()
            .get(self.pos)
            .is_some_and(|c| matches!(c, b' ' | b'\n' | b'\r' | b'\t'))
        {
            self.pos += 1;
        }
    }
    fn take(&mut self, b: u8) -> bool {
        self.ws();
        if self.s.as_bytes().get(self.pos) == Some(&b) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn quoted(&mut self) -> Result<Vec<u32>, Failure> {
        if !self.take(b'"') {
            return Err(Failure::Json);
        }
        let mut out = Vec::new();
        loop {
            let c = self.s[self.pos..].chars().next().ok_or(Failure::Json)?;
            self.pos += c.len_utf8();
            match c {
                '"' => return Ok(out),
                '\\' => {
                    let b = *self.s.as_bytes().get(self.pos).ok_or(Failure::Json)?;
                    self.pos += 1;
                    out.push(match b {
                        b'"' => 34,
                        b'\\' => 92,
                        b'/' => 47,
                        b'b' => 8,
                        b'f' => 12,
                        b'n' => 10,
                        b'r' => 13,
                        b't' => 9,
                        b'u' => {
                            let end = self.pos.checked_add(4).ok_or(Failure::Json)?;
                            let raw = self.s.get(self.pos..end).ok_or(Failure::Json)?;
                            if !raw.bytes().all(|b| b.is_ascii_hexdigit()) {
                                return Err(Failure::Json);
                            }
                            let mut cp = u32::from_str_radix(raw, 16).map_err(|_| Failure::Json)?;
                            self.pos = end;
                            if (0xd800..=0xdbff).contains(&cp)
                                && self.s[self.pos..].starts_with("\\u")
                                && let Some(low) = self
                                    .s
                                    .get(self.pos + 2..self.pos + 6)
                                    .and_then(|s| u32::from_str_radix(s, 16).ok())
                                    .filter(|n| (0xdc00..=0xdfff).contains(n))
                            {
                                cp = 0x10000 + (cp - 0xd800) * 1024 + low - 0xdc00;
                                self.pos += 6;
                            }
                            cp
                        }
                        _ => return Err(Failure::Json),
                    });
                }
                c if u32::from(c) < 32 => return Err(Failure::Json),
                c => out.push(u32::from(c)),
            }
        }
    }
    fn atom(&mut self) -> Result<Value, Failure> {
        self.ws();
        let b = *self.s.as_bytes().get(self.pos).ok_or(Failure::Json)?;
        if b == b'"' {
            return self.quoted().map(Value::String);
        }
        for (token, v) in [
            ("null", Value::Null),
            ("true", Value::Bool(true)),
            ("false", Value::Bool(false)),
            ("NaN", Value::Number("NaN".into())),
            ("Infinity", Value::Number("Infinity".into())),
            ("-Infinity", Value::Number("-Infinity".into())),
        ] {
            if self.s[self.pos..].starts_with(token) {
                self.pos += token.len();
                return Ok(v);
            }
        }
        let start = self.pos;
        if b == b'-' {
            self.pos += 1;
        }
        match self.s.as_bytes().get(self.pos) {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                while self
                    .s
                    .as_bytes()
                    .get(self.pos)
                    .is_some_and(u8::is_ascii_digit)
                {
                    self.pos += 1
                }
            }
            _ => return Err(Failure::Json),
        }
        let integer_end = self.pos;
        let mut float = false;
        if self.s.as_bytes().get(self.pos) == Some(&b'.') {
            float = true;
            self.pos += 1;
            let at = self.pos;
            while self
                .s
                .as_bytes()
                .get(self.pos)
                .is_some_and(u8::is_ascii_digit)
            {
                self.pos += 1
            }
            if at == self.pos {
                return Err(Failure::Json);
            }
        }
        if self
            .s
            .as_bytes()
            .get(self.pos)
            .is_some_and(|c| matches!(c, b'e' | b'E'))
        {
            float = true;
            self.pos += 1;
            if self
                .s
                .as_bytes()
                .get(self.pos)
                .is_some_and(|c| matches!(c, b'+' | b'-'))
            {
                self.pos += 1
            }
            let at = self.pos;
            while self
                .s
                .as_bytes()
                .get(self.pos)
                .is_some_and(u8::is_ascii_digit)
            {
                self.pos += 1
            }
            if at == self.pos {
                return Err(Failure::Json);
            }
        }
        let raw = &self.s[start..self.pos];
        let number = if float {
            let f = crate::cli_float::parse_json_f64(raw).map_err(|_| Failure::Json)?;
            if f.is_infinite() {
                if f.is_sign_negative() {
                    "-Infinity"
                } else {
                    "Infinity"
                }
                .into()
            } else {
                ssx_json::canonical::python_float(f).map_err(|_| Failure::Json)?
            }
        } else {
            if integer_end - start - usize::from(b == b'-') > 4300 {
                return Err(Failure::Integer);
            }
            if raw == "-0" { "0".into() } else { raw.into() }
        };
        Ok(Value::Number(number))
    }
}
// Parsing and JSON printing use explicit stacks: source admits >1000 levels and
// >4MiB payloads, so the shared bounded codec cannot supply this client contract.
pub fn parse(raw: &str) -> Result<Value, Failure> {
    enum Frame {
        Array(Vec<Value>),
        Object(Vec<(Vec<u32>, Value)>, HashMap<Vec<u32>, usize>, Vec<u32>),
    }
    let mut p = Parser { s: raw, pos: 0 };
    let mut stack = Vec::new();
    let mut completed = None;
    loop {
        let value = if let Some(value) = completed.take() {
            value
        } else {
            p.ws();
            match p.s.as_bytes().get(p.pos) {
                Some(b'[') => {
                    p.pos += 1;
                    if p.take(b']') {
                        Value::Array(Vec::new())
                    } else {
                        stack.push(Frame::Array(Vec::new()));
                        continue;
                    }
                }
                Some(b'{') => {
                    p.pos += 1;
                    if p.take(b'}') {
                        Value::Object(Vec::new())
                    } else {
                        let key = p.quoted()?;
                        if !p.take(b':') {
                            return Err(Failure::Json);
                        }
                        stack.push(Frame::Object(Vec::new(), HashMap::new(), key));
                        continue;
                    }
                }
                _ => p.atom()?,
            }
        };
        match stack.last_mut() {
            Some(Frame::Array(values)) => {
                values.push(value);
                if p.take(b']') {
                    let Some(Frame::Array(values)) = stack.pop() else {
                        unreachable!()
                    };
                    completed = Some(Value::Array(values));
                } else if !p.take(b',') {
                    return Err(Failure::Json);
                }
            }
            Some(Frame::Object(fields, positions, key)) => {
                let key = std::mem::take(key);
                if let Some(&position) = positions.get(&key) {
                    fields[position].1 = value;
                } else {
                    positions.insert(key.clone(), fields.len());
                    fields.push((key, value));
                }
                if p.take(b'}') {
                    let Some(Frame::Object(fields, _, _)) = stack.pop() else {
                        unreachable!()
                    };
                    completed = Some(Value::Object(fields));
                } else {
                    if !p.take(b',') {
                        return Err(Failure::Json);
                    }
                    let next = p.quoted()?;
                    if !p.take(b':') {
                        return Err(Failure::Json);
                    }
                    let Some(Frame::Object(_, _, key)) = stack.last_mut() else {
                        unreachable!()
                    };
                    *key = next;
                }
            }
            None => {
                p.ws();
                return if p.pos == raw.len() {
                    Ok(value)
                } else {
                    Err(Failure::Json)
                };
            }
        }
    }
}
impl Clone for Value {
    fn clone(&self) -> Self {
        enum Work<'a> {
            Value(&'a Value),
            Array(usize),
            Object(Vec<Vec<u32>>),
        }
        let mut work = vec![Work::Value(self)];
        let mut output = Vec::new();
        while let Some(next) = work.pop() {
            match next {
                Work::Value(v) => match v {
                    Self::Null => output.push(Self::Null),
                    Self::Bool(v) => output.push(Self::Bool(*v)),
                    Self::Number(v) => output.push(Self::Number(v.clone())),
                    Self::String(v) => output.push(Self::String(v.clone())),
                    Self::Array(v) => {
                        work.push(Work::Array(v.len()));
                        for v in v.iter().rev() {
                            work.push(Work::Value(v));
                        }
                    }
                    Self::Object(v) => {
                        work.push(Work::Object(v.iter().map(|(k, _)| k.clone()).collect()));
                        for (_, v) in v.iter().rev() {
                            work.push(Work::Value(v));
                        }
                    }
                },
                Work::Array(n) => {
                    let values = output.split_off(output.len() - n);
                    output.push(Self::Array(values));
                }
                Work::Object(keys) => {
                    let values = output.split_off(output.len() - keys.len());
                    output.push(Self::Object(keys.into_iter().zip(values).collect()));
                }
            }
        }
        output.pop().expect("one cloned root")
    }
}
impl Drop for Value {
    fn drop(&mut self) {
        fn drain(value: &mut Value, pending: &mut Vec<Value>) {
            match value {
                Value::Array(values) => pending.append(values),
                Value::Object(fields) => pending.extend(fields.drain(..).map(|(_, v)| v)),
                _ => {}
            }
        }
        let mut pending = Vec::new();
        drain(self, &mut pending);
        while let Some(mut next) = pending.pop() {
            drain(&mut next, &mut pending);
        }
    }
}
fn quoted(v: &[u32], out: &mut String) {
    out.push('"');
    for &cp in v {
        match cp {
            34 => out.push_str("\\\""),
            92 => out.push_str("\\\\"),
            8 => out.push_str("\\b"),
            9 => out.push_str("\\t"),
            10 => out.push_str("\\n"),
            12 => out.push_str("\\f"),
            13 => out.push_str("\\r"),
            32..=126 => out.push(char::from_u32(cp).expect("ASCII")),
            0..=0xffff => out.push_str(&format!("\\u{cp:04x}")),
            _ => {
                let n = cp - 0x10000;
                out.push_str(&format!(
                    "\\u{:04x}\\u{:04x}",
                    0xd800 + (n >> 10),
                    0xdc00 + (n & 1023)
                ))
            }
        }
    }
    out.push('"')
}
fn write(value: &Value, depth: usize, out: &mut String) {
    enum Event<'a> {
        Value(&'a Value, usize),
        Key(&'a [u32]),
        Text(&'static str),
        Indent(usize),
    }
    let mut pending = vec![Event::Value(value, depth)];
    while let Some(next) = pending.pop() {
        match next {
            Event::Text(s) => out.push_str(s),
            Event::Key(k) => quoted(k, out),
            Event::Indent(d) => out.push_str(&"  ".repeat(d)),
            Event::Value(value, d) => match value {
                Value::Null => out.push_str("null"),
                Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
                Value::Number(n) => out.push_str(n),
                Value::String(s) => quoted(s, out),
                Value::Array(values) => {
                    out.push('[');
                    if values.is_empty() {
                        out.push(']');
                        continue;
                    }
                    pending.push(Event::Text("]"));
                    pending.push(Event::Indent(d));
                    pending.push(Event::Text("\n"));
                    for (i, v) in values.iter().enumerate().rev() {
                        pending.push(Event::Value(v, d + 1));
                        pending.push(Event::Indent(d + 1));
                        pending.push(Event::Text("\n"));
                        if i > 0 {
                            pending.push(Event::Text(","));
                        }
                    }
                }
                Value::Object(values) => {
                    out.push('{');
                    if values.is_empty() {
                        out.push('}');
                        continue;
                    }
                    pending.push(Event::Text("}"));
                    pending.push(Event::Indent(d));
                    pending.push(Event::Text("\n"));
                    for (i, (k, v)) in values.iter().enumerate().rev() {
                        pending.push(Event::Value(v, d + 1));
                        pending.push(Event::Text(": "));
                        pending.push(Event::Key(k));
                        pending.push(Event::Indent(d + 1));
                        pending.push(Event::Text("\n"));
                        if i > 0 {
                            pending.push(Event::Text(","));
                        }
                    }
                }
            },
        }
    }
}
pub fn pretty(v: &Value) -> String {
    let mut out = String::new();
    write(v, 0, &mut out);
    out.push('\n');
    out
}

fn compare_integer(a: &str, b: &str) -> std::cmp::Ordering {
    let an = a.starts_with('-');
    let bn = b.starts_with('-');
    if an != bn {
        return if an {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        };
    }
    let a = a.trim_start_matches('-');
    let b = b.trim_start_matches('-');
    let cmp = a.len().cmp(&b.len()).then_with(|| a.cmp(b));
    if an { cmp.reverse() } else { cmp }
}
fn numeric(v: &Value) -> Option<String> {
    match v {
        Value::Bool(b) => Some(if *b { "1" } else { "0" }.into()),
        Value::Number(s) => Some(s.clone()),
        _ => None,
    }
}
fn numeric_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let af = a.contains(['.', 'e', 'E']) || matches!(a, "NaN" | "Infinity" | "-Infinity");
    let bf = b.contains(['.', 'e', 'E']) || matches!(b, "NaN" | "Infinity" | "-Infinity");
    if !af && !bf {
        return compare_integer(a, b);
    }
    if af && bf {
        return a
            .parse::<f64>()
            .unwrap()
            .partial_cmp(&b.parse::<f64>().unwrap())
            .unwrap_or(Ordering::Equal);
    }
    let (integer, float, reverse) = if af { (b, a, true) } else { (a, b, false) };
    let f = float.parse::<f64>().unwrap();
    let cmp = if f.is_nan() {
        Ordering::Equal
    } else if f == f64::INFINITY {
        Ordering::Less
    } else if f == f64::NEG_INFINITY {
        Ordering::Greater
    } else {
        let truncated = format!("{:.0}", f.trunc());
        let truncated = if truncated == "-0" { "0" } else { &truncated };
        let cmp = compare_integer(integer, truncated);
        if cmp == Ordering::Equal {
            if f.fract() > 0.0 {
                Ordering::Less
            } else if f.fract() < 0.0 {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        } else {
            cmp
        }
    };
    if reverse { cmp.reverse() } else { cmp }
}
fn compare(a: &Value, b: &Value) -> Result<std::cmp::Ordering, Failure> {
    enum Work<'a> {
        Pair(&'a Value, &'a Value, bool),
        Length(usize, usize),
    }
    let mut work = vec![Work::Pair(a, b, false)];
    while let Some(next) = work.pop() {
        match next {
            Work::Length(a, b) => {
                let cmp = a.cmp(&b);
                if cmp != std::cmp::Ordering::Equal {
                    return Ok(cmp);
                }
            }
            Work::Pair(a, b, nested) => {
                if nested && !matches!(a, Value::Array(_)) && a == b {
                    continue;
                }
                let cmp = if let (Some(a), Some(b)) = (numeric(a), numeric(b)) {
                    if a == "NaN" || b == "NaN" {
                        return Err(Failure::Shape);
                    }
                    numeric_cmp(&a, &b)
                } else {
                    match (a, b) {
                        (Value::String(a), Value::String(b)) => a.cmp(b),
                        (Value::Array(a), Value::Array(b)) => {
                            work.push(Work::Length(a.len(), b.len()));
                            for (a, b) in a.iter().zip(b).rev() {
                                work.push(Work::Pair(a, b, true));
                            }
                            continue;
                        }
                        _ => return Err(Failure::Shape),
                    }
                };
                if cmp != std::cmp::Ordering::Equal {
                    return Ok(cmp);
                }
            }
        }
    }
    Ok(std::cmp::Ordering::Equal)
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        let mut work = vec![(self, other)];
        while let Some((a, b)) = work.pop() {
            if let (Some(a), Some(b)) = (numeric(a), numeric(b)) {
                if a == "NaN" || b == "NaN" {
                    if a != b {
                        return false;
                    }
                } else if numeric_cmp(&a, &b) != std::cmp::Ordering::Equal {
                    return false;
                }
                continue;
            }
            match (a, b) {
                (Self::Null, Self::Null) => {}
                (Self::String(a), Self::String(b)) if a == b => {}
                (Self::Array(a), Self::Array(b)) if a.len() == b.len() => {
                    work.extend(a.iter().zip(b))
                }
                (Self::Object(a), Self::Object(b)) if a.len() == b.len() => {
                    // Preserve the first matching field even for manually built
                    // duplicate-key Values; parsed objects already have unique keys.
                    let mut fields: HashMap<&[u32], &Value> = HashMap::with_capacity(b.len());
                    for (key, value) in b {
                        fields.entry(key.as_slice()).or_insert(value);
                    }
                    for (key, value) in a {
                        let Some(&other) = fields.get(key.as_slice()) else {
                            return false;
                        };
                        work.push((value, other));
                    }
                }
                _ => return false,
            }
        }
        true
    }
}
impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&pretty(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;

    #[test]
    fn decoded_object_index_preserves_first_position_and_last_value() {
        let parsed = parse(
            r#"{"first":0,"\u0066irst":1,"\ud800":2,"\uD800":3,"😀":4,"\ud83d\ude00":5,"é":6,"\u00e9":7,"e\u0301":8,"nested":{"same":1,"\u0073ame":2},"same":3,"first":10}"#,
        )
        .unwrap();
        assert_eq!(
            pretty(&parsed),
            concat!(
                "{\n  \"first\": 10,\n  \"\\ud800\": 3,\n",
                "  \"\\ud83d\\ude00\": 5,\n  \"\\u00e9\": 7,\n",
                "  \"e\\u0301\": 8,\n  \"nested\": {\n    \"same\": 2\n  },\n",
                "  \"same\": 3\n}\n"
            )
        );
        assert_eq!(parsed.object().unwrap().len(), 7);
        assert_eq!(
            parsed.get("first").unwrap(),
            Some(&Value::Number("10".into()))
        );
        assert_eq!(parsed.get("missing").unwrap(), None);

        let mut raw = String::from("{");
        for i in 0..32_768 {
            if i != 0 {
                raw.push(',');
            }
            write!(&mut raw, "\"k{i:06}\":{i}").unwrap();
        }
        raw.push_str(",\"\\u006b000000\":-1,\"k032767\":-2}");
        let wide = parse(&raw).unwrap();
        let fields = wide.object().unwrap();
        assert_eq!(fields.len(), 32_768);
        for (i, (key, value)) in fields.iter().enumerate() {
            assert_eq!(*key, text(&format!("k{i:06}")));
            let expected = match i {
                0 => "-1".to_owned(),
                32_767 => "-2".to_owned(),
                _ => i.to_string(),
            };
            assert_eq!(*value, Value::Number(expected));
        }
    }

    #[test]
    fn fallible_category_sort_is_stable_bounded_and_preserves_errors() {
        const N: usize = 32_768;
        let bound = N * 15; // ceil(log2(N)) merge levels, at most N comparisons each.
        let mut values: Vec<_> = (0..N).rev().map(|i| Value::Number(i.to_string())).collect();
        let mut comparisons = 0;
        fallible_stable_sort_by(&mut values, |a, b| {
            comparisons += 1;
            assert!(comparisons <= bound, "category comparison bound exceeded");
            compare(a, b)
        })
        .unwrap();
        for (i, value) in values.iter().enumerate() {
            assert_eq!(*value, Value::Number(i.to_string()));
        }
        println!("T08A category comparisons {comparisons} bound {bound}");

        let ties = parse("[true,1.0,1,0]").unwrap().sorted_sequence().unwrap();
        assert_eq!(
            pretty(&Value::Array(ties)),
            "[\n  0,\n  true,\n  1.0,\n  1\n]\n"
        );
        let rotated = parse("[2,0,1,4,3]").unwrap().sorted_sequence().unwrap();
        assert_eq!(
            pretty(&Value::Array(rotated)),
            "[\n  0,\n  1,\n  2,\n  3,\n  4\n]\n"
        );
        let mut unchanged = vec![3, 2, 1];
        let mut calls = 0;
        let result = fallible_stable_sort_by(&mut unchanged, |a, b| {
            calls += 1;
            if calls == 2 {
                Err(Failure::Shape)
            } else {
                Ok(a.cmp(b))
            }
        });
        assert!(matches!(result, Err(Failure::Shape)));
        assert_eq!(unchanged, [3, 2, 1]);
        assert!(matches!(
            parse("[NaN,0]").unwrap().sorted_sequence(),
            Err(Failure::Shape)
        ));
        assert!(matches!(
            parse("[{},0]").unwrap().sorted_sequence(),
            Err(Failure::Shape)
        ));
        assert_eq!(parse("[NaN]").unwrap().sorted_sequence().unwrap().len(), 1);
        assert_eq!(
            compare(&parse("[NaN,1]").unwrap(), &parse("[NaN,2]").unwrap()).unwrap(),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn nested_object_equality_preserves_decoded_keys_and_first_match() {
        let mut forward = Vec::new();
        let mut reverse = Vec::new();
        for i in 0..32_768 {
            forward.push((text(&format!("k{i:06}")), Value::Number(i.to_string())));
        }
        for i in (0..32_768).rev() {
            reverse.push((text(&format!("k{i:06}")), Value::Number(i.to_string())));
        }
        assert_eq!(Value::Object(forward), Value::Object(reverse));
        assert_eq!(
            parse(r#"{"é":NaN,"\ud800":true}"#).unwrap(),
            parse(r#"{"\uD800":1,"\u00e9":NaN}"#).unwrap()
        );
        assert_ne!(
            parse(r#"{"é":NaN}"#).unwrap(),
            parse(r#"{"e\u0301":NaN}"#).unwrap()
        );
        let duplicate = || {
            Value::Object(vec![
                (text("x"), Value::Bool(true)),
                (text("x"), Value::Bool(false)),
            ])
        };
        let first = Value::Object(vec![
            (text("x"), Value::Number("1".into())),
            (text("x"), Value::Number("1".into())),
        ]);
        assert_eq!(first, duplicate());
        let last = Value::Object(vec![
            (text("x"), Value::Bool(false)),
            (text("x"), Value::Bool(false)),
        ]);
        assert_ne!(last, duplicate());
    }
}
