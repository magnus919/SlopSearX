//! Local reranker parser: duplicate keys, constants and nonfinite floats fail closed.
//! Strings retain Python's surrogate-pass code points, including in unused fields.
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Value {
    Null,
    Bool,
    Number(String, Option<f64>),
    String(Vec<u32>),
    Array,
    Object(BTreeMap<Vec<u32>, usize>),
}
pub(super) struct Document {
    nodes: Vec<Value>,
    root: usize,
}
#[derive(Clone, Copy)]
pub(super) struct Ref<'a> {
    document: &'a Document,
    index: usize,
}
impl Document {
    pub(super) fn root(&self) -> Ref<'_> {
        Ref {
            document: self,
            index: self.root,
        }
    }
}
impl Ref<'_> {
    fn value(&self) -> &Value {
        &self.document.nodes[self.index]
    }
    pub(super) fn get(&self, key: &str) -> Option<Self> {
        match self.value() {
            Value::Object(m) => Some(Self {
                document: self.document,
                index: *m.get(&key.chars().map(u32::from).collect::<Vec<_>>())?,
            }),
            _ => None,
        }
    }
    #[cfg(test)]
    pub(super) fn string(&self) -> Option<String> {
        match self.value() {
            Value::String(s) => s.iter().map(|&c| char::from_u32(c)).collect(),
            _ => None,
        }
    }
    pub(super) fn score(&self) -> Option<f64> {
        match self.value() {
            Value::Number(_, number) => number.filter(|n| (0.0..=9.0).contains(n)),
            _ => None,
        }
    }
    pub(super) fn unsigned_integer(&self) -> Option<&str> {
        match self.value() {
            Value::Number(s, _)
                if !s.contains(['.', 'e', 'E']) && (!s.starts_with('-') || s == "-0") =>
            {
                Some(if s == "-0" { "0" } else { s })
            }
            _ => None,
        }
    }
    pub(super) fn is_string(&self, expected: &str) -> bool {
        match self.value() {
            Value::String(s) => s.iter().copied().eq(expected.chars().map(u32::from)),
            _ => false,
        }
    }
    pub(super) fn has_keys(&self, expected: &BTreeSet<String>) -> bool {
        match self.value() {
            Value::Object(map) => {
                map.len() == expected.len()
                    && map
                        .keys()
                        .zip(expected)
                        .all(|(key, name)| key.iter().copied().eq(name.chars().map(u32::from)))
            }
            _ => false,
        }
    }
}
async fn decode(raw: &[u8], budget: &mut Budget) -> Option<Vec<u32>> {
    let (bytes, width, little) = if let Some(b) = raw.strip_prefix(&[0, 0, 0xfe, 0xff]) {
        (b, 4, false)
    } else if let Some(b) = raw.strip_prefix(&[0xff, 0xfe, 0, 0]) {
        (b, 4, true)
    } else if let Some(b) = raw.strip_prefix(&[0xfe, 0xff]) {
        (b, 2, false)
    } else if let Some(b) = raw.strip_prefix(&[0xff, 0xfe]) {
        (b, 2, true)
    } else if let Some(b) = raw.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        (b, 1, false)
    } else if raw.len() >= 4 && raw[..3] == [0, 0, 0] {
        (raw, 4, false)
    } else if raw.len() >= 4 && raw[1..4] == [0, 0, 0] {
        (raw, 4, true)
    } else if raw.len() >= 2 && raw[0] == 0 {
        (raw, 2, false)
    } else if raw.len() >= 2 && raw[1] == 0 {
        (raw, 2, true)
    } else {
        (raw, 1, false)
    };
    let mut out = Vec::new();
    if width == 4 {
        if bytes.len() % 4 != 0 {
            return None;
        }
        for a in bytes.as_chunks::<4>().0 {
            if budget.count() {
                budget.checkpoint().await?;
            }
            let a = *a;
            let c = if little {
                u32::from_le_bytes(a)
            } else {
                u32::from_be_bytes(a)
            };
            if c > 0x10ffff {
                return None;
            }
            out.push(c);
        }
    } else if width == 2 {
        if bytes.len() % 2 != 0 {
            return None;
        }
        let mut units = Vec::with_capacity(bytes.len() / 2);
        for b in bytes.as_chunks::<2>().0 {
            if budget.count() {
                budget.checkpoint().await?;
            }
            units.push(if little {
                u16::from_le_bytes(*b)
            } else {
                u16::from_be_bytes(*b)
            });
        }
        let mut i = 0;
        while i < units.len() {
            if budget.count() {
                budget.checkpoint().await?;
            }
            let c = u32::from(units[i]);
            i += 1;
            if (0xd800..=0xdbff).contains(&c)
                && units.get(i).is_some_and(|c| (0xdc00..=0xdfff).contains(c))
            {
                out.push(0x10000 + ((c - 0xd800) << 10) + (u32::from(units[i]) - 0xdc00));
                i += 1;
            } else {
                out.push(c);
            }
        }
    } else {
        let mut i = 0;
        while i < bytes.len() {
            if budget.count() {
                budget.checkpoint().await?;
            }
            let b = bytes[i];
            i += 1;
            let (n, mut c, min) = match b {
                0..=0x7f => (0, u32::from(b), 0),
                0xc2..=0xdf => (1, u32::from(b & 31), 0x80),
                0xe0..=0xef => (2, u32::from(b & 15), 0x800),
                0xf0..=0xf4 => (3, u32::from(b & 7), 0x10000),
                _ => return None,
            };
            for _ in 0..n {
                let b = *bytes.get(i)?;
                i += 1;
                if b & 0xc0 != 0x80 {
                    return None;
                }
                c = (c << 6) | u32::from(b & 63);
            }
            if c < min || c > 0x10ffff {
                return None;
            }
            out.push(c);
        }
    }
    Some(out)
}
// All nesting lives in Vec frames and arena indices. Success, parse error and
// document destruction cannot recurse, even for a near-2MiB chain of containers.
pub(super) async fn parse(bytes: &[u8], deadline: tokio::time::Instant) -> Option<Document> {
    parse_with_progress(bytes, deadline, None).await
}
async fn parse_with_progress(
    bytes: &[u8],
    deadline: tokio::time::Instant,
    progress: Option<&std::sync::atomic::AtomicUsize>,
) -> Option<Document> {
    if bytes.len() > super::MAX_RESPONSE_BYTES {
        return None;
    }
    let mut budget = Budget { deadline, steps: 0 };
    if budget.count() {
        budget.checkpoint().await?;
    }
    let mut p = Parser {
        chars: decode(bytes, &mut budget).await?,
        at: 0,
        budget,
    };
    let mut nodes = Vec::new();
    let mut frames: Vec<Frame> = Vec::new();
    let mut root = None;
    loop {
        if p.budget.count() {
            p.budget.checkpoint().await?;
        }
        p.space().await?;
        let mut close = false;
        if let Some(frame) = frames.last_mut() {
            match frame {
                Frame::Object {
                    node,
                    state,
                    pending,
                } => {
                    if *state <= 1 {
                        if *state == 0 && p.eat(b'}') {
                            close = true;
                        } else {
                            let key = p.string().await?;
                            p.space().await?;
                            if !p.eat(b':') {
                                return None;
                            }
                            let Value::Object(map) = &mut nodes[*node] else {
                                return None;
                            };
                            if map.insert(key.clone(), usize::MAX).is_some() {
                                return None;
                            }
                            *pending = Some(key);
                            *state = 2;
                        }
                    } else if *state == 3 {
                        if p.eat(b'}') {
                            close = true;
                        } else if p.eat(b',') {
                            *state = 1;
                            continue;
                        } else {
                            return None;
                        }
                    }
                }
                Frame::Array { state } => {
                    if *state == 0 && p.eat(b']') {
                        close = true;
                    } else if *state == 2 {
                        if p.eat(b']') {
                            close = true;
                        } else if p.eat(b',') {
                            *state = 1;
                            continue;
                        } else {
                            return None;
                        }
                    }
                }
            }
        } else if root.is_some() {
            break;
        }
        if close {
            frames.pop();
            continue;
        }
        p.space().await?;
        let (value, open) = match p.peek()? {
            34 => (Value::String(p.string().await?), 0),
            110 => {
                p.word("null")?;
                (Value::Null, 0)
            }
            116 => {
                p.word("true")?;
                (Value::Bool, 0)
            }
            102 => {
                p.word("false")?;
                (Value::Bool, 0)
            }
            123 => {
                p.at += 1;
                (Value::Object(BTreeMap::new()), 1)
            }
            91 => {
                p.at += 1;
                (Value::Array, 2)
            }
            _ => (p.number().await?, 0),
        };
        let index = nodes.len();
        nodes.push(value);
        if let Some(progress) = progress {
            progress.store(nodes.len(), std::sync::atomic::Ordering::SeqCst);
        }
        match frames.last_mut() {
            Some(Frame::Object {
                node,
                state,
                pending,
            }) => {
                let Value::Object(map) = &mut nodes[*node] else {
                    return None;
                };
                *map.get_mut(&pending.take()?)? = index;
                *state = 3;
            }
            Some(Frame::Array { state }) => *state = 2,
            None => root = Some(index),
        }
        match open {
            1 => frames.push(Frame::Object {
                node: index,
                state: 0,
                pending: None,
            }),
            2 => frames.push(Frame::Array { state: 0 }),
            _ => (),
        }
    }
    p.space().await?;
    if tokio::time::Instant::now() >= p.budget.deadline {
        return None;
    }
    (p.at == p.chars.len()).then_some(Document { nodes, root: root? })
}
enum Frame {
    Object {
        node: usize,
        state: u8,
        pending: Option<Vec<u32>>,
    },
    Array {
        state: u8,
    },
}
// One checkpoint per 1024 lexical/structural operations, including strings,
// whitespace, numbers and encoding conversion. Dropping the owned future joins
// all parser work; no detached blocking task can outlive cancellation.
struct Budget {
    deadline: tokio::time::Instant,
    steps: usize,
}
impl Budget {
    fn count(&mut self) -> bool {
        let checkpoint = self.steps & 1023 == 0;
        self.steps += 1;
        checkpoint
    }
    async fn checkpoint(&self) -> Option<()> {
        if tokio::time::Instant::now() >= self.deadline {
            return None;
        }
        tokio::task::yield_now().await;
        if tokio::time::Instant::now() >= self.deadline {
            return None;
        }
        Some(())
    }
}
async fn finite_float(raw: &str, budget: &mut Budget) -> Option<f64> {
    let mut normalizer = ssx_json::FloatNormalizer::new(raw).ok()?;
    while !normalizer.advance(1024).ok()? {
        budget.checkpoint().await?;
    }
    if tokio::time::Instant::now() >= budget.deadline {
        return None;
    }
    let value = normalizer.finish().ok().filter(|v| v.is_finite());
    if tokio::time::Instant::now() >= budget.deadline {
        return None;
    }
    value
}

struct Parser {
    budget: Budget,
    chars: Vec<u32>,
    at: usize,
}
impl Parser {
    fn peek(&self) -> Option<u32> {
        self.chars.get(self.at).copied()
    }
    fn take(&mut self) -> Option<u32> {
        let c = self.peek()?;
        self.at += 1;
        Some(c)
    }
    fn is(&self, c: u8) -> bool {
        self.peek() == Some(u32::from(c))
    }
    fn eat(&mut self, c: u8) -> bool {
        if self.is(c) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    async fn space(&mut self) -> Option<()> {
        while self.peek().is_some_and(|c| matches!(c, 9 | 10 | 13 | 32)) {
            if self.budget.count() {
                self.budget.checkpoint().await?;
            }
            self.at += 1;
        }
        Some(())
    }
    fn word(&mut self, word: &str) -> Option<()> {
        for c in word.bytes() {
            if !self.eat(c) {
                return None;
            }
        }
        Some(())
    }
    fn hex(&mut self) -> Option<u32> {
        let mut n = 0;
        for _ in 0..4 {
            n = n * 16 + char::from_u32(self.take()?)?.to_digit(16)?;
        }
        Some(n)
    }
    async fn string(&mut self) -> Option<Vec<u32>> {
        if !self.eat(b'"') {
            return None;
        }
        let mut out = Vec::new();
        loop {
            if self.budget.count() {
                self.budget.checkpoint().await?;
            }
            let c = self.take()?;
            match c {
                34 => break,
                0..=31 => return None,
                92 => {
                    let e = self.take()?;
                    out.push(match e {
                        34 | 47 | 92 => e,
                        98 => 8,
                        102 => 12,
                        110 => 10,
                        114 => 13,
                        116 => 9,
                        117 => {
                            let high = self.hex()?;
                            if (0xd800..=0xdbff).contains(&high) {
                                let saved = self.at;
                                if self.eat(b'\\') && self.eat(b'u') {
                                    let low = self.hex()?;
                                    if (0xdc00..=0xdfff).contains(&low) {
                                        0x10000 + ((high - 0xd800) << 10) + low - 0xdc00
                                    } else {
                                        self.at = saved;
                                        high
                                    }
                                } else {
                                    self.at = saved;
                                    high
                                }
                            } else {
                                high
                            }
                        }
                        _ => return None,
                    });
                }
                _ => out.push(c),
            }
        }
        Some(out)
    }
    async fn number(&mut self) -> Option<Value> {
        let start = self.at;
        self.eat(b'-');
        if !self.eat(b'0') {
            if !self.peek().is_some_and(|c| (49..=57).contains(&c)) {
                return None;
            }
            while self.peek().is_some_and(|c| (48..=57).contains(&c)) {
                if self.budget.count() {
                    self.budget.checkpoint().await?;
                }
                self.at += 1;
            }
        }
        let digits = self.at - start - usize::from(self.chars[start] == 45);
        let mut float = false;
        if self.eat(b'.') {
            float = true;
            let begin = self.at;
            while self.peek().is_some_and(|c| (48..=57).contains(&c)) {
                if self.budget.count() {
                    self.budget.checkpoint().await?;
                }
                self.at += 1;
            }
            if begin == self.at {
                return None;
            }
        }
        if self.eat(b'e') || self.eat(b'E') {
            float = true;
            if !self.eat(b'+') {
                self.eat(b'-');
            }
            let begin = self.at;
            while self.peek().is_some_and(|c| (48..=57).contains(&c)) {
                if self.budget.count() {
                    self.budget.checkpoint().await?;
                }
                self.at += 1;
            }
            if begin == self.at {
                return None;
            }
        }
        if !float && digits > 4300 {
            return None;
        }
        if self.budget.count() {
            self.budget.checkpoint().await?;
        }
        let mut raw = String::with_capacity(self.at - start);
        for &c in &self.chars[start..self.at] {
            if self.budget.count() {
                self.budget.checkpoint().await?;
            }
            raw.push(char::from_u32(c)?);
        }
        // The standard correctly-rounded conversion is bounded by 2MiB; check
        // the absolute budget immediately before and after its synchronous scan.
        if tokio::time::Instant::now() >= self.budget.deadline {
            return None;
        }
        let number = if float {
            Some(finite_float(&raw, &mut self.budget).await?)
        } else {
            raw.parse::<f64>().ok().filter(|n| n.is_finite())
        };
        if tokio::time::Instant::now() >= self.budget.deadline {
            return None;
        }
        if float && number.is_none() {
            return None;
        }
        Some(Value::Number(raw, number))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(bytes: &[u8]) -> Option<Document> {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(super::parse(
                bytes,
                tokio::time::Instant::now() + std::time::Duration::from_secs(10),
            ))
    }
    #[test]
    fn strict_json_preserves_python_encodings_numbers_and_surrogates() {
        let raw = r#"{"model":"jev-1.13.0","unused":"\ud800","n":123456789012345678901234567890,"score":-0.0}"#;
        for bom in [false, true] {
            for little in [false, true] {
                let mut utf16 = if bom {
                    if little {
                        vec![255, 254]
                    } else {
                        vec![254, 255]
                    }
                } else {
                    vec![]
                };
                let mut utf32 = if bom {
                    if little {
                        vec![255, 254, 0, 0]
                    } else {
                        vec![0, 0, 254, 255]
                    }
                } else {
                    vec![]
                };
                for c in raw.encode_utf16() {
                    utf16.extend_from_slice(&if little {
                        c.to_le_bytes()
                    } else {
                        c.to_be_bytes()
                    });
                }
                for c in raw.chars().map(u32::from) {
                    utf32.extend_from_slice(&if little {
                        c.to_le_bytes()
                    } else {
                        c.to_be_bytes()
                    });
                }
                for bytes in [utf16, utf32] {
                    let doc = parse(&bytes).unwrap();
                    assert_eq!(
                        doc.root().get("model").unwrap().string().as_deref(),
                        Some("jev-1.13.0")
                    );
                    assert_eq!(
                        doc.root().get("n").unwrap().unsigned_integer(),
                        Some("123456789012345678901234567890")
                    );
                    assert_eq!(doc.root().get("score").unwrap().score(), Some(-0.0));
                    assert!(doc.root().get("unused").unwrap().string().is_none());
                }
            }
        }
        let raw_surrogate = b"{\"unused\":\"\xed\xa0\x80\"}";
        assert!(parse(raw_surrogate).is_some());
        for raw in [
            r#"{"a":1,"\u0061":2}"#,
            r#"{"unused":{"x":0,"x":1}}"#,
            r#"{"unused":[NaN]}"#,
            r#"{"unused":1e400}"#,
            r#"{"unused":Infinity}"#,
            r#"[1,]"#,
            r#"{"x":0,}"#,
            r#"{"\ud800\udc00":1,"𐀀":2}"#,
        ] {
            assert!(parse(raw.as_bytes()).is_none(), "{raw}");
        }
        assert!(parse(format!("{{\"n\":{}}}", "9".repeat(4300)).as_bytes()).is_some());
        assert!(parse(format!("{{\"n\":{}}}", "9".repeat(4301)).as_bytes()).is_none());
    }
    #[test]
    fn strict_json_deep_success_error_and_destruction_use_flat_storage() {
        std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(|| {
                for (open, close, depth) in [("[", "]", 110_000), ("{\"x\":", "}", 20_000)] {
                    let raw = format!("{}0{}", open.repeat(depth), close.repeat(depth));
                    let document = parse(raw.as_bytes())
                        .expect("source-success depth must not consume native stack");
                    drop(document);
                    assert!(
                        parse(
                            format!(
                                "{}{{\"x\":0,\"x\":1}}{}",
                                open.repeat(depth),
                                close.repeat(depth)
                            )
                            .as_bytes()
                        )
                        .is_none()
                    );
                    assert!(parse(&raw.as_bytes()[..raw.len() - 1]).is_none());
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }
    #[tokio::test]
    async fn deep_parser_yields_to_current_thread_heartbeat_and_deadline() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let heartbeat = Arc::new(AtomicUsize::new(0));
        let observed = heartbeat.clone();
        let task = tokio::spawn(async move {
            loop {
                tokio::task::yield_now().await;
                observed.fetch_add(1, Ordering::SeqCst);
            }
        });
        tokio::task::yield_now().await;
        let raw = format!("{}0{}", "[".repeat(900_000), "]".repeat(900_000));
        let before = heartbeat.load(Ordering::SeqCst);
        let start = std::time::Instant::now();
        let document = super::parse(
            raw.as_bytes(),
            tokio::time::Instant::now() + std::time::Duration::from_secs(10),
        )
        .await
        .unwrap();
        let progress = heartbeat.load(Ordering::SeqCst) - before;
        eprintln!(
            "deep parser elapsed_ms={} heartbeat_ticks={progress}",
            start.elapsed().as_millis()
        );
        drop(document);
        task.abort();
        let _ = task.await;
        assert!(
            progress > 1,
            "near-limit provider parsing must yield while its own work is active"
        );
    }
    #[tokio::test]
    async fn cancelled_large_arena_is_dropped_and_joined_without_detached_work() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let progress = Arc::new(AtomicUsize::new(0));
        let observer = progress.clone();
        let raw = format!("{}0{}", "[".repeat(900_000), "]".repeat(900_000));
        let task = tokio::spawn(async move {
            super::parse_with_progress(
                raw.as_bytes(),
                tokio::time::Instant::now() + std::time::Duration::from_secs(10),
                Some(&observer),
            )
            .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while progress.load(Ordering::SeqCst) < 400_000 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let allocated = progress.load(Ordering::SeqCst);
        let started = std::time::Instant::now();
        task.abort();
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), task)
                .await
                .unwrap()
                .err()
                .unwrap()
                .is_cancelled()
        );
        let settled = progress.load(Ordering::SeqCst);
        tokio::task::yield_now().await;
        assert_eq!(progress.load(Ordering::SeqCst), settled);
        eprintln!(
            "cancelled arena nodes={allocated} joined_cleanup_ms={}",
            started.elapsed().as_millis()
        );
        let raw = format!("{}0{}", "[".repeat(900_000), "]".repeat(900_000));
        let started = std::time::Instant::now();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(5);
        assert!(super::parse(raw.as_bytes(), deadline).await.is_none());
        eprintln!(
            "deep parser 5ms deadline returned_ms={}",
            started.elapsed().as_millis()
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
        assert!(
            super::parse(
                b"{}",
                tokio::time::Instant::now() - std::time::Duration::from_millis(1)
            )
            .await
            .is_none()
        );
    }
    #[tokio::test]
    async fn near_limit_finite_float_keeps_heartbeat_and_never_accepts_after_deadline() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let number = format!("0.{}1e+1990000", "0".repeat(1_990_000));
        let raw = format!("{{\"score\":{number}}}");
        let heartbeat = Arc::new(AtomicUsize::new(0));
        let observed = heartbeat.clone();
        let task = tokio::spawn(async move {
            loop {
                tokio::task::yield_now().await;
                observed.fetch_add(1, Ordering::SeqCst);
            }
        });
        tokio::task::yield_now().await;
        let before = heartbeat.load(Ordering::SeqCst);
        let started = std::time::Instant::now();
        let doc = super::parse(
            raw.as_bytes(),
            tokio::time::Instant::now() + std::time::Duration::from_secs(10),
        )
        .await
        .unwrap();
        assert_eq!(doc.root().get("score").unwrap().score(), Some(0.1));
        let progress = heartbeat.load(Ordering::SeqCst) - before;
        assert!(progress > 1);
        let conversion = std::time::Instant::now();
        let mut budget = Budget {
            deadline: tokio::time::Instant::now() + std::time::Duration::from_secs(10),
            steps: 0,
        };
        assert_eq!(finite_float(&number, &mut budget).await.unwrap(), 0.1);
        eprintln!(
            "long finite float elapsed_ms={} heartbeat_ticks={progress} normalized_conversion_ms={}",
            started.elapsed().as_millis(),
            conversion.elapsed().as_millis()
        );
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(5);
        let started = std::time::Instant::now();
        assert!(super::parse(raw.as_bytes(), deadline).await.is_none());
        eprintln!(
            "long finite float 5ms deadline returned_ms={}",
            started.elapsed().as_millis()
        );
        task.abort();
        let _ = task.await;
    }
    #[tokio::test]
    async fn long_significand_conversion_checks_deadline_without_reparsing_scores() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let number = format!("0.{}", "1234567890".repeat(199_000));
        let mut normalizer = ssx_json::FloatNormalizer::new(&number).unwrap();
        while !normalizer.advance(1024).unwrap() {
            tokio::task::yield_now().await;
        }
        let conversion = std::time::Instant::now();
        let converted = normalizer.finish().unwrap();
        eprintln!(
            "near2MiB significant digits final_std_conversion_ms={}",
            conversion.elapsed().as_millis()
        );
        assert_eq!(converted, 0.12345678901234568);
        let heartbeat = Arc::new(AtomicUsize::new(0));
        let observed = heartbeat.clone();
        let task = tokio::spawn(async move {
            loop {
                tokio::task::yield_now().await;
                observed.fetch_add(1, Ordering::SeqCst);
            }
        });
        let raw = format!("{{\"score\":{number}}}");
        let before = heartbeat.load(Ordering::SeqCst);
        let doc = super::parse(
            raw.as_bytes(),
            tokio::time::Instant::now() + std::time::Duration::from_secs(10),
        )
        .await
        .unwrap();
        assert_eq!(doc.root().get("score").unwrap().score(), Some(converted));
        assert!(heartbeat.load(Ordering::SeqCst) > before + 1);
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(5);
        assert!(super::parse(raw.as_bytes(), deadline).await.is_none());
        task.abort();
        let _ = task.await;
    }
}
