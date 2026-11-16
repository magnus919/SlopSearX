//! Pure receipt observation normalization. Every URL/reference remains inert.
//! Captured timestamps retain the original Python 3.14 ISO input spelling;
//! validation does not convert them to UTC or claim the observation is verified.
use sha2::{Digest, Sha256};
use ssx_domain::canonical::{JsonProfile, canonical_json};
use ssx_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidInput(pub String);
fn bounded(
    value: Option<&str>,
    field: &str,
    cap: usize,
    required: bool,
) -> Result<Option<String>, InvalidInput> {
    match value {
        None if required => Err(InvalidInput(format!("{field} is required"))),
        None => Ok(None),
        Some(value) if !value.is_empty() && value.chars().count() <= cap => Ok(Some(value.into())),
        _ => Err(InvalidInput(format!("{field} must be 1-{cap} characters"))),
    }
}
fn digits(chars: &[char]) -> Option<u32> {
    if chars.is_empty() || !chars.iter().all(char::is_ascii_digit) {
        return None;
    }
    chars.iter().try_fold(0u32, |n, c| {
        n.checked_mul(10)?.checked_add(*c as u32 - '0' as u32)
    })
}
fn leap(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}
fn ordinal(year: u32, month: u32, day: u32) -> Option<u32> {
    if !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
        return None;
    }
    let months = [
        31,
        if leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if day == 0 || day > months[month as usize - 1] {
        return None;
    }
    let y = year - 1;
    Some(
        365 * y + y / 4 - y / 100
            + y / 400
            + months[..month as usize - 1].iter().sum::<u32>()
            + day,
    )
}
fn date(chars: &[char]) -> Option<u32> {
    if ![7, 8, 10].contains(&chars.len()) {
        return None;
    }
    let y = digits(&chars[..4])?;
    if !(1..=9999).contains(&y) {
        return None;
    }
    let separated = chars[4] == '-';
    let mut pos = 4 + usize::from(separated);
    if chars.get(pos) == Some(&'W') {
        pos += 1;
        let week = digits(chars.get(pos..pos + 2)?)?;
        pos += 2;
        let day = if chars.len() > pos {
            if (chars.get(pos) == Some(&'-')) != separated {
                return None;
            }
            pos += usize::from(separated);
            digits(chars.get(pos..pos + 1)?)?
        } else {
            1
        };
        let jan = ordinal(y, 1, 1)?;
        let jan_weekday = (jan - 1) % 7;
        let max = if jan_weekday == 3 || (jan_weekday == 2 && leap(y)) {
            53
        } else {
            52
        };
        if week == 0 || week > max || !(1..=7).contains(&day) {
            return None;
        }
        let monday = jan as i64 - jan_weekday as i64 + if jan_weekday > 3 { 7 } else { 0 };
        let result = monday + (week as i64 - 1) * 7 + day as i64 - 1;
        (1..=3_652_059).contains(&result).then_some(result as u32)
    } else {
        let month = digits(chars.get(pos..pos + 2)?)?;
        pos += 2;
        if (chars.get(pos) == Some(&'-')) != separated {
            return None;
        }
        pos += usize::from(separated);
        let day = digits(chars.get(pos..pos + 2)?)?;
        ordinal(y, month, day)
    }
}
fn separator(c: &[char]) -> Option<usize> {
    if c.len() < 7 {
        return None;
    }
    if c.len() == 7 {
        return Some(7);
    }
    if c[4] == '-' {
        if c[5] != 'W' {
            return Some(10);
        }
        if c.len() > 8 && c[8] == '-' {
            if c.len() == 9 {
                return None;
            }
            if c.len() > 10 && c[10].is_ascii_digit() {
                Some(8)
            } else {
                Some(10)
            }
        } else {
            Some(8)
        }
    } else if c[4] == 'W' {
        let mut i = 7;
        while i < c.len() && c[i].is_ascii_digit() {
            i += 1;
        }
        Some(if i < 9 {
            i
        } else if i.is_multiple_of(2) {
            7
        } else {
            8
        })
    } else {
        Some(8)
    }
}
fn time_components(c: &[char]) -> Option<[u32; 4]> {
    let mut parts = [0; 4];
    let mut pos = 0;
    let mut separated = false;
    for (i, part) in parts[..3].iter_mut().enumerate() {
        *part = digits(c.get(pos..pos + 2)?)?;
        pos += 2;
        let next = c.get(pos);
        if i == 0 {
            separated = next == Some(&':');
        }
        if next.is_none() || i == 2 {
            break;
        }
        if separated && next != Some(&':') {
            return None;
        }
        pos += usize::from(separated);
    }
    if pos < c.len() {
        if !matches!(c[pos], '.' | ',') {
            return None;
        }
        pos += 1;
        let fraction = c.get(pos..)?;
        if fraction.is_empty() || !fraction.iter().all(char::is_ascii_digit) {
            return None;
        }
        let width = fraction.len().min(6);
        parts[3] = digits(&fraction[..width])? * 10u32.pow((6 - width) as u32);
    }
    Some(parts)
}
fn timestamp_grammar(value: &str) -> Option<bool> {
    let replaced = value.replace('Z', "+00:00");
    let c: Vec<_> = replaced.chars().collect();
    let sep = separator(&c)?;
    let day = date(c.get(..sep)?)?;
    let time = c.get((sep + 1).min(c.len())..)?;
    if time.is_empty() {
        return Some(false);
    }
    let tz = time
        .iter()
        .position(|c| *c == '-')
        .or_else(|| time.iter().position(|c| *c == '+'));
    let main = time_components(&time[..tz.unwrap_or(time.len())])?;
    if main[1] > 59 || main[2] > 59 || main[0] > 24 {
        return None;
    }
    if main[0] == 24 && (main[1..].iter().any(|n| *n != 0) || day == 3_652_059) {
        return None;
    }
    if let Some(tz) = tz {
        let offset = &time[tz + 1..];
        if [0, 1, 3].contains(&offset.len()) {
            return None;
        }
        let off = time_components(offset)?;
        let micros = (((off[0] as u64 * 60 + off[1] as u64) * 60 + off[2] as u64) * 1_000_000)
            + off[3] as u64;
        if micros >= 86_400_000_000 {
            return None;
        }
    }
    Some(tz.is_some())
}
pub fn captured_at(value: Option<&str>, required: bool) -> Result<Option<String>, InvalidInput> {
    let value = bounded(value, "captured_at", 64, required)?;
    if let Some(value) = value.as_deref() {
        match timestamp_grammar(value) {
            None => return Err(InvalidInput("captured_at must be RFC3339".into())),
            Some(false) => return Err(InvalidInput("captured_at must include a timezone".into())),
            Some(true) => {}
        }
    }
    Ok(value)
}
#[derive(Debug, Clone, Default)]
pub struct ObservationInput {
    pub retriever: String,
    pub idempotency_key: String,
    pub status: String,
    pub captured_at: Option<String>,
    pub final_url: Option<String>,
    pub content_sha256: Option<String>,
    pub capture_ref: Option<String>,
    /// Wire decoding must first enforce list[dict[str,str]]; the pure original
    /// helper additionally treats falsey empty object as its empty-list default.
    pub passage_refs: Option<Value>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
}
#[derive(Debug, Clone)]
pub struct Observation {
    pub body: Value,
    pub digest: String,
    pub idempotency_key: String,
}
impl ObservationInput {
    pub fn normalize(self) -> Result<Observation, InvalidInput> {
        let retriever = bounded(Some(&self.retriever), "retriever", 128, true)?.unwrap();
        bounded(Some(&self.idempotency_key), "idempotency_key", 128, true)?;
        let success = self.status == "succeeded";
        if !success && self.status != "failed" {
            return Err(InvalidInput("status must be succeeded or failed".into()));
        }
        let captured = captured_at(self.captured_at.as_deref(), success)?;
        let final_url = bounded(self.final_url.as_deref(), "final_url", 2048, false)?;
        let capture = bounded(self.capture_ref.as_deref(), "capture_ref", 2048, success)?;
        if self
            .content_sha256
            .as_ref()
            .is_some_and(|s| s.len() != 64 || !s.bytes().all(|c| c.is_ascii_hexdigit()))
        {
            return Err(InvalidInput(
                "content_sha256 must be 64 hexadecimal characters".into(),
            ));
        }
        if success && (self.failure_code.is_some() || self.failure_message.is_some()) {
            return Err(InvalidInput(
                "successful receipts reject failure fields".into(),
            ));
        }
        if !success
            && !self.failure_code.as_ref().is_some_and(|s| {
                !s.is_empty()
                    && s.len() <= 64
                    && s.bytes()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'-'))
            })
        {
            return Err(InvalidInput(
                "failed receipts require a valid failure_code".into(),
            ));
        }
        let message = bounded(
            self.failure_message.as_deref(),
            "failure_message",
            1024,
            false,
        )?;
        let empty = Vec::new();
        let passages = match self.passage_refs.as_ref() {
            None | Some(Value::Null) => &empty,
            Some(Value::Object(o)) if o.is_empty() => &empty,
            Some(Value::Array(a)) if a.len() <= 32 => a,
            _ => {
                return Err(InvalidInput(
                    "passage_refs must contain at most 32 entries".into(),
                ));
            }
        };
        let mut normalized = Vec::with_capacity(passages.len());
        for passage in passages {
            let object = passage
                .as_object()
                .filter(|o| o.keys().all(|k| matches!(k.as_str(), "ref" | "label")))
                .ok_or_else(|| {
                    InvalidInput("each passage reference accepts only ref and label".into())
                })?;
            let get = |field: &str, cap: usize, required| {
                if object
                    .get(field)
                    .is_some_and(|v| !v.is_null() && !v.is_string())
                {
                    return Err(InvalidInput(format!(
                        "passage_refs.{field} must be 1-{cap} characters"
                    )));
                }
                bounded(
                    object.get(field).and_then(Value::as_str),
                    &format!("passage_refs.{field}"),
                    cap,
                    required,
                )
            };
            let mut item = json!({"ref":get("ref",2048,true)?.unwrap()});
            if let Some(label) = get("label", 256, false)? {
                item["label"] = Value::String(label);
            }
            normalized.push(item);
        }
        let hash = self
            .content_sha256
            .map(|value| json!({"algorithm":"sha256","value":value.to_ascii_lowercase()}));
        let failure = (!success).then(|| json!({"code":self.failure_code,"message":message}));
        let mut body = json!({"retriever":retriever,"observation":{"status":self.status,"captured_at":captured,"final_url":final_url,"content_hash":hash,"capture_ref":capture,"failure":failure}});
        body["observation"]["passage_refs"] = Value::Array(normalized);
        let encoded = canonical_json(&body, JsonProfile::Utf8Compact)
            .map_err(|_| InvalidInput("invalid bounded observation".into()))?;
        if encoded.len() > super::MAX_RECEIPT_BYTES {
            return Err(InvalidInput("encoded receipt exceeds 32768 bytes".into()));
        }
        let digest = Sha256::digest(encoded)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Ok(Observation {
            body,
            digest,
            idempotency_key: self.idempotency_key,
        })
    }
}
