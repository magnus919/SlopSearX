//! Distinct token-file CPython JSON and source HTTP Jiter entry boundaries.
use ssx_source_json::{self as source, Number, Text, Value};
use std::time::Instant;
pub const MAX_BYTES: usize = 1024 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonError {
    Malformed,
    Encoding,
    Limit,
    Deadline,
}
pub async fn parse(bytes: &[u8], deadline: Instant) -> Result<Value, JsonError> {
    if bytes.len() > MAX_BYTES {
        return Err(JsonError::Limit);
    }
    // FileTokenStorage calls read_text/loads(str), not loads(bytes): no UTF16/32
    // autodetection, and a UTF8 BOM remains a caught JSON syntax failure.
    std::str::from_utf8(bytes).map_err(|_| JsonError::Encoding)?;
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(JsonError::Malformed);
    }
    source::from_slice(bytes, Some(deadline))
        .await
        .map_err(|e| match e {
            source::Error::Deadline => JsonError::Deadline,
            source::Error::Encoding { .. } => JsonError::Encoding,
            source::Error::Syntax { .. } => JsonError::Malformed,
            _ => JsonError::Limit,
        })
}
struct Writer {
    out: Vec<u8>,
    work: usize,
    deadline: Instant,
}
impl Writer {
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), JsonError> {
        if bytes.len() > MAX_BYTES.saturating_sub(self.out.len()) {
            return Err(JsonError::Limit);
        }
        self.out
            .try_reserve(bytes.len())
            .map_err(|_| JsonError::Limit)?;
        self.out.extend_from_slice(bytes);
        Ok(())
    }
    async fn checkpoint(&mut self) -> Result<(), JsonError> {
        if Instant::now() >= self.deadline {
            return Err(JsonError::Deadline);
        }
        tokio::task::yield_now().await;
        if Instant::now() >= self.deadline {
            return Err(JsonError::Deadline);
        }
        Ok(())
    }
    fn indent(&mut self, depth: usize) -> Result<(), JsonError> {
        if depth > source::MAX_DEPTH {
            return Err(JsonError::Limit);
        }
        self.bytes(&[b' '; source::MAX_DEPTH * 2][..depth * 2])
    }
    fn hex(&mut self, value: u16) -> Result<(), JsonError> {
        let digits = b"0123456789abcdef";
        self.bytes(&[
            b'\\',
            b'u',
            digits[((value >> 12) & 15) as usize],
            digits[((value >> 8) & 15) as usize],
            digits[((value >> 4) & 15) as usize],
            digits[(value & 15) as usize],
        ])
    }
    async fn text(&mut self, text: &Text) -> Result<(), JsonError> {
        self.bytes(b"\"")?;
        for (index, point) in text.codepoints().iter().copied().enumerate() {
            if index % 512 == 0 {
                self.checkpoint().await?;
            }
            match point {
                34 => self.bytes(b"\\\"")?,
                92 => self.bytes(b"\\\\")?,
                8 => self.bytes(b"\\b")?,
                12 => self.bytes(b"\\f")?,
                10 => self.bytes(b"\\n")?,
                13 => self.bytes(b"\\r")?,
                9 => self.bytes(b"\\t")?,
                0x20..=0x7e => self.bytes(&[point as u8])?,
                0..=0xffff => self.hex(point as u16)?,
                0x10000..=0x10ffff => {
                    let point = point - 0x10000;
                    self.hex(0xd800 + (point >> 10) as u16)?;
                    self.hex(0xdc00 + (point & 0x3ff) as u16)?;
                }
                _ => return Err(JsonError::Encoding),
            }
        }
        self.bytes(b"\"")
    }
}
enum Frame<'a> {
    Value(&'a Value, usize),
    Array(std::slice::Iter<'a, Value>, usize, bool),
    Object(
        Box<dyn Iterator<Item = (&'a Text, &'a Value)> + Send + 'a>,
        usize,
        bool,
    ),
}
/// Iterative traversal bounds stack depth and yields for empty-element arrays,
/// scalar text and object members; no subtree cloning or repr interpolation.
enum Start<'a> {
    Value(&'a Value),
    Object(&'a source::Map),
}
pub async fn encode(value: &Value, deadline: Instant) -> Result<Vec<u8>, JsonError> {
    encode_start(Start::Value(value), deadline).await
}
pub async fn encode_map(value: &source::Map, deadline: Instant) -> Result<Vec<u8>, JsonError> {
    encode_start(Start::Object(value), deadline).await
}
async fn encode_start(start: Start<'_>, deadline: Instant) -> Result<Vec<u8>, JsonError> {
    let mut writer = Writer {
        out: Vec::new(),
        work: 0,
        deadline,
    };
    writer.checkpoint().await?;
    let mut stack = Vec::new();
    stack
        .try_reserve(2 * (source::MAX_DEPTH + 2))
        .map_err(|_| JsonError::Limit)?;
    match start {
        Start::Value(value) => stack.push(Frame::Value(value, 0)),
        Start::Object(value) => {
            writer.bytes(b"{")?;
            stack.push(Frame::Object(Box::new(value.iter()), 0, true));
        }
    }
    while let Some(frame) = stack.pop() {
        writer.work += 1;
        if writer.work.is_multiple_of(128) {
            writer.checkpoint().await?;
        }
        match frame {
            Frame::Value(value, depth) => {
                if depth > source::MAX_DEPTH {
                    return Err(JsonError::Limit);
                }
                match value {
                    Value::Null => writer.bytes(b"null")?,
                    Value::Bool(v) => writer.bytes(if *v { b"true" } else { b"false" })?,
                    Value::Number(number @ Number::Integer(_)) => {
                        writer.bytes(number.integer_text().expect("integer").as_bytes())?
                    }
                    Value::Number(Number::Float(v)) => {
                        if v.is_nan() {
                            writer.bytes(b"NaN")?;
                        } else if *v == f64::INFINITY {
                            writer.bytes(b"Infinity")?;
                        } else if *v == f64::NEG_INFINITY {
                            writer.bytes(b"-Infinity")?;
                        } else {
                            writer.bytes(
                                ssx_json::python_float(*v)
                                    .map_err(|_| JsonError::Encoding)?
                                    .as_bytes(),
                            )?;
                        }
                    }
                    Value::String(text) => writer.text(text).await?,
                    Value::Array(values) => {
                        writer.bytes(b"[")?;
                        stack.push(Frame::Array(values.iter(), depth, true));
                    }
                    Value::Object(values) => {
                        writer.bytes(b"{")?;
                        stack.push(Frame::Object(Box::new(values.iter()), depth, true));
                    }
                }
            }
            Frame::Array(mut values, depth, first) => {
                if let Some(value) = values.next() {
                    if !first {
                        writer.bytes(b",")?;
                    }
                    writer.bytes(b"\n")?;
                    writer.indent(depth + 1)?;
                    stack.push(Frame::Array(values, depth, false));
                    stack.push(Frame::Value(value, depth + 1));
                } else {
                    if !first {
                        writer.bytes(b"\n")?;
                        writer.indent(depth)?;
                    }
                    writer.bytes(b"]")?;
                }
            }
            Frame::Object(mut values, depth, first) => {
                if let Some((key, value)) = values.next() {
                    if !first {
                        writer.bytes(b",")?;
                    }
                    writer.bytes(b"\n")?;
                    writer.indent(depth + 1)?;
                    writer.text(key).await?;
                    writer.bytes(b": ")?;
                    stack.push(Frame::Object(values, depth, false));
                    stack.push(Frame::Value(value, depth + 1));
                } else {
                    if !first {
                        writer.bytes(b"\n")?;
                        writer.indent(depth)?;
                    }
                    writer.bytes(b"}")?;
                }
            }
        }
    }
    writer.checkpoint().await?;
    Ok(writer.out)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn deadline() -> Instant {
        Instant::now() + std::time::Duration::from_secs(10)
    }
    #[tokio::test]
    async fn exact_python_ascii_nan_bigint_and_surrogate_spelling() {
        let raw=br#"{"tokens":{"access_token":"synthetic-\ud800"},"unknown":[NaN,Infinity,-Infinity,100000000000000000000000000000000000001,"\u007f\u96ea\ud83d\ude80\n"]}"#;
        let value = parse(raw, deadline()).await.unwrap();
        let expected=b"{\n  \"tokens\": {\n    \"access_token\": \"synthetic-\\ud800\"\n  },\n  \"unknown\": [\n    NaN,\n    Infinity,\n    -Infinity,\n    100000000000000000000000000000000000001,\n    \"\\u007f\\u96ea\\ud83d\\ude80\\n\"\n  ]\n}";
        assert_eq!(encode(&value, deadline()).await.unwrap(), expected);
        assert_eq!(
            encode(&Value::Array(Vec::new()), deadline()).await.unwrap(),
            b"[]"
        );
        assert_eq!(
            encode(&Value::Object(source::Map::new()), deadline())
                .await
                .unwrap(),
            b"{}"
        );
    }
    #[tokio::test(flavor = "current_thread")]
    async fn empty_array_and_long_scalar_work_cooperate_before_completion() {
        let values = [
            Value::Array(vec![Value::Null; 100_000]),
            Value::String(Text::from("X".repeat(100_000))),
        ];
        for value in values {
            let mut encoding = Box::pin(encode(&value, deadline()));
            tokio::select! {biased;_=&mut encoding=>panic!("encoding completed before cooperative probe"),_=async{for _ in 0..32{tokio::task::yield_now().await;}}=>{}}
            drop(encoding);
        }
    }
    #[tokio::test]
    async fn expired_deadline_and_byte_depth_bound_refuse_without_normalizing() {
        assert_eq!(
            encode(
                &Value::Null,
                Instant::now() - std::time::Duration::from_secs(1)
            )
            .await,
            Err(JsonError::Deadline)
        );
        assert_eq!(parse(&[0xff], deadline()).await, Err(JsonError::Encoding));
        assert_eq!(
            parse(b"\xef\xbb\xbf{}", deadline()).await,
            Err(JsonError::Malformed)
        );
        let large = Value::String(Text::from("雪".repeat(200_000)));
        assert_eq!(encode(&large, deadline()).await, Err(JsonError::Limit));
        let mut nested = Value::Null;
        for _ in 0..=source::MAX_DEPTH {
            nested = Value::Array(vec![nested]);
        }
        assert_eq!(encode(&nested, deadline()).await, Err(JsonError::Limit));
    }
    #[tokio::test]
    async fn exact_input_integer_and_depth_resource_boundaries() {
        let end = deadline();
        let integer = vec![b'9'; source::MAX_INTEGER_DIGITS];
        assert!(parse(&integer, end).await.is_ok());
        let excessive_integer = vec![b'9'; source::MAX_INTEGER_DIGITS + 1];
        assert_eq!(parse(&excessive_integer, end).await, Err(JsonError::Limit));
        let mut at_cap = vec![b'x'; MAX_BYTES];
        at_cap[0] = b'"';
        at_cap[MAX_BYTES - 1] = b'"';
        assert!(parse(&at_cap, end).await.is_ok());
        at_cap.insert(1, b'x');
        assert_eq!(parse(&at_cap, end).await, Err(JsonError::Limit));
        let nested = |depth: usize| format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
        assert!(
            parse(nested(source::MAX_DEPTH).as_bytes(), end)
                .await
                .is_ok()
        );
        assert_eq!(
            parse(nested(source::MAX_DEPTH + 1).as_bytes(), end).await,
            Err(JsonError::Limit)
        );
    }
}

/// Source HTTP JSON boundary: Core/Jiter grammar validates the ENTIRE document,
/// including ignored fields, before existing model validators consume SourceIR.
/// File parse remains separate: HTTP rejects surrogate escapes and uses
/// Jiter's own4300-digit/recursion admission. One MiB/deadline are native limits.
#[cfg(feature = "gateway-source-endpoints")]
pub(super) async fn parse_http(bytes: &[u8], deadline: Instant) -> Result<Value, JsonError> {
    use jiter::{JsonValue, PartialMode};
    if bytes.len() > MAX_BYTES {
        return Err(JsonError::Limit);
    }
    if Instant::now() >= deadline {
        return Err(JsonError::Deadline);
    }
    tokio::task::yield_now().await;
    if Instant::now() >= deadline {
        return Err(JsonError::Deadline);
    }
    let parsed = JsonValue::parse_with_config(bytes, true, PartialMode::Off);
    if Instant::now() >= deadline {
        return Err(JsonError::Deadline);
    }
    // Core2.46.4 maps every parser error (including recursion limit) to
    // JsonInvalid/ValidationError. Only OUR body/deadline guards are operational.
    let value = parsed.map_err(|_| JsonError::Malformed)?;
    enum Step<'a> {
        Value(&'a JsonValue<'a>, usize),
        Array(std::slice::Iter<'a, JsonValue<'a>>, Vec<Value>, usize),
        Object(
            std::slice::Iter<'a, (std::borrow::Cow<'a, str>, JsonValue<'a>)>,
            source::Map,
            Option<Text>,
            usize,
        ),
    }
    let mut stack = Vec::new();
    stack.try_reserve(202).map_err(|_| JsonError::Limit)?;
    stack.push(Step::Value(&value, 0));
    let mut complete = None;
    let mut work = 0usize;
    while let Some(step) = stack.pop() {
        if Instant::now() >= deadline {
            return Err(JsonError::Deadline);
        }
        work += 1;
        if work.is_multiple_of(128) {
            tokio::task::yield_now().await;
            if Instant::now() >= deadline {
                return Err(JsonError::Deadline);
            }
        }
        match step {
            Step::Value(value, depth) => {
                // Jiter already admitted its own200-level grammar; file100 is
                // deliberately absent. This guard only bounds our traversal.
                if depth > 200 {
                    return Err(JsonError::Limit);
                }
                complete = Some(match value {
                    JsonValue::Null => Value::Null,
                    JsonValue::Bool(v) => Value::Bool(*v),
                    JsonValue::Int(v) => Value::integer(*v),
                    JsonValue::BigInt(v) => Value::Number(
                        Number::from_integer_text(&v.to_string()).ok_or(JsonError::Malformed)?,
                    ),
                    JsonValue::Float(v) => Value::Number(Number::Float(*v)),
                    JsonValue::Str(v) => Value::String(Text::from(v.as_ref())),
                    JsonValue::Array(values) => {
                        let mut out = Vec::new();
                        out.try_reserve(values.len())
                            .map_err(|_| JsonError::Limit)?;
                        stack.push(Step::Array(values.iter(), out, depth));
                        continue;
                    }
                    JsonValue::Object(fields) => {
                        stack.push(Step::Object(fields.iter(), source::Map::new(), None, depth));
                        continue;
                    }
                });
            }
            Step::Array(mut iter, mut out, depth) => {
                if let Some(value) = complete.take() {
                    out.push(value);
                }
                if let Some(value) = iter.next() {
                    stack.push(Step::Array(iter, out, depth));
                    stack.push(Step::Value(value, depth + 1));
                } else {
                    complete = Some(Value::Array(out));
                }
            }
            Step::Object(mut iter, mut out, pending, depth) => {
                if let Some(key) = pending {
                    out.insert(key, complete.take().ok_or(JsonError::Malformed)?);
                }
                if let Some((key, value)) = iter.next() {
                    // Map insert preserves first key position/final duplicate
                    // value; checkpoints also bound Vec-map insertion batches.
                    stack.push(Step::Object(
                        iter,
                        out,
                        Some(Text::from(key.as_ref())),
                        depth,
                    ));
                    stack.push(Step::Value(value, depth + 1));
                } else {
                    complete = Some(Value::Object(out));
                }
            }
        }
    }
    if Instant::now() >= deadline {
        return Err(JsonError::Deadline);
    }
    complete.ok_or(JsonError::Malformed)
}

#[cfg(all(test, feature = "gateway-source-endpoints"))]
mod source_http_tests {
    use super::*;
    #[tokio::test]
    async fn source_http_ignored_surrogates_fail_before_models_while_file_lane_stays_distinct() {
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        for raw in [
            br#"{"access_token":"ok","token_type":"Bearer","extra":"\ud800"}"#.as_slice(),
            br#"{"client_id":"ok","issuer":{"ignored":"\ud800"}}"#.as_slice(),
        ] {
            assert_eq!(
                parse_http(raw, deadline).await.unwrap_err(),
                JsonError::Malformed
            );
            assert!(
                parse(raw, deadline).await.is_ok(),
                "file JSON permits escaped surrogate text"
            );
        }
        let too_deep = format!(r#"{{"ignored":{}0{}}}"#, "[".repeat(250), "]".repeat(250));
        assert_eq!(
            parse_http(too_deep.as_bytes(), deadline).await.unwrap_err(),
            JsonError::Malformed
        );
        assert_eq!(
            parse_http(b"{}", Instant::now()).await.unwrap_err(),
            JsonError::Deadline
        );
        assert_eq!(
            parse_http(&vec![b' '; MAX_BYTES + 1], deadline)
                .await
                .unwrap_err(),
            JsonError::Limit
        );
    }
    #[tokio::test]
    async fn source_http_ignored_big_nan_and_duplicate_members_use_existing_models() {
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        // Drop after actual conversion has started, not only its initial yield.
        let many = format!(
            "{{{}}}",
            (0..2000)
                .map(|i| format!("\"key{i}\":{i}"))
                .collect::<Vec<_>>()
                .join(",")
        );
        let mut projection = Box::pin(parse_http(many.as_bytes(), deadline));
        for _ in 0..2 {
            assert!(
                std::future::poll_fn(|cx| std::task::Poll::Ready(
                    projection.as_mut().poll(cx).is_pending()
                ))
                .await
            );
        }
        drop(projection);
        // Exact positive-integer boundary in the pinned Jiter decoder.
        assert!(
            parse_http("9".repeat(4300).as_bytes(), deadline)
                .await
                .is_ok()
        );
        assert_eq!(
            parse_http("9".repeat(4301).as_bytes(), deadline).await,
            Err(JsonError::Malformed)
        );
        // Both ignored and known excessive integers fail whole-document
        // Jiter admission before model projection, just as pinned Core does.
        for field in ["expires_in", "extra"] {
            let rejected = format!(
                r#"{{"access_token":"ok","token_type":"Bearer","{field}":{}}}"#,
                "9".repeat(5000)
            );
            assert_eq!(
                parse_http(rejected.as_bytes(), deadline).await,
                Err(JsonError::Malformed)
            );
        }
        let huge = "9".repeat(4000);
        let deep = format!("{}0{}", "[".repeat(150), "]".repeat(150));
        let deep_token = format!(r#"{{"access_token":"ok","ignored":{deep}}}"#);
        let parsed = parse_http(deep_token.as_bytes(), deadline).await.unwrap();
        assert!(
            super::super::token_model::validate(Some(&parsed), deadline)
                .await
                .unwrap()
                .is_some()
        );
        let token = format!(
            r#"{{"access_token":"before","access_token":"after","token_type":"Bearer","extra":[{huge},NaN,Infinity,-Infinity]}}"#
        );
        let parsed = parse_http(token.as_bytes(), deadline).await.unwrap();
        let model = super::super::token_model::validate(Some(&parsed), deadline)
            .await
            .unwrap()
            .unwrap();
        let accepted = parse(model.json(), deadline).await.unwrap();
        assert_eq!(
            accepted.get("access_token").unwrap().as_str(),
            Some("after")
        );
        let client =
            format!(r#"{{"client_id":"before","client_id":"after","extra":[{huge},NaN]}}"#);
        let parsed = parse_http(client.as_bytes(), deadline).await.unwrap();
        let model = super::super::client_model::validate(Some(&parsed), deadline)
            .await
            .unwrap()
            .unwrap();
        let accepted = parse(model.json(), deadline).await.unwrap();
        assert_eq!(accepted.get("client_id").unwrap().as_str(), Some("after"));
        assert!(parse(token.as_bytes(), deadline).await.is_ok());
        assert_eq!(
            parse("9".repeat(5000).as_bytes(), deadline).await,
            Err(JsonError::Limit)
        );
    }
}
