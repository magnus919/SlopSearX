//! Source-compatible native HTTP text codec families. No interpreter/backend.
mod alias_only;
mod aliases;
mod bounded;
mod bounded_http;
pub use bounded::{
    BoundedStrictAsciiOutcome, BudgetAscii, BudgetText, BudgetTextBuilder,
    Error as BoundedTextError,
};
pub use bounded_http::{BoundedDecodeError, decode_http_bounded};
mod doublebyte;
mod gb18030_ranges;
mod hz;
mod idna;
mod idna_data;
mod iso2022;
mod label_alnum;
mod singlebyte;
mod strict_cjk_errors;
mod strict_parameter;
mod strict_validity;
pub use strict_parameter::{ParameterError, ascii as strict_parameter_ascii};
/// Resolve a normalized registry label with pinned Unicode16 classification.
/// Use lookup_http_encoding for source validation and typed NUL errors.
pub fn canonical(label: &str) -> Option<&'static str> {
    let normalized = normalize_label(label);
    if let Some(name) = alias_only::canonical(&normalized) {
        return Some(name);
    }
    if normalized.contains('.') {
        return alias_only::canonical(&normalized.replace('.', "_"));
    }
    aliases::canonical(&normalized)
}

/// Source codec lookup distinction used by the HTTPX charset selector.
/// Whole-response empty-body precedence belongs to the caller, before lookup.
pub fn lookup_http_encoding(label: &str) -> Result<Option<&'static str>, ParameterError> {
    if label.contains('\0') {
        Err(ParameterError::ValueError("embedded null character"))
    } else {
        Ok(canonical(label))
    }
}

fn normalize_label(label: &str) -> String {
    let mut normalized = String::new();
    let mut punctuation = false;
    for c in label.chars() {
        if label_alnum::is_alnum(c) || c == '.' {
            if punctuation && !normalized.is_empty() {
                normalized.push('_');
            }
            if c.is_ascii() {
                normalized.push(c.to_ascii_lowercase());
            }
            punctuation = false;
        } else {
            punctuation = true;
        }
    }
    normalized
}
/// Decode a registered single-byte table; other families use the full decoder.
pub fn decode_single_byte(bytes: &[u8], label: &str) -> Option<String> {
    let table = singlebyte::table(canonical(label)?)?;
    Some(
        bytes
            .iter()
            .map(|b| char::from_u32(table[usize::from(*b)]).expect("generated mapping scalar"))
            .collect(),
    )
}

fn decode_single_byte_points(bytes: &[u8], canonical: &str) -> Option<Text> {
    let table = singlebyte::table(canonical)?;
    let mut points = Vec::with_capacity(bytes.len());
    points.extend(bytes.iter().map(|b| table[usize::from(*b)]));
    Some(Text::from_codepoints(points).expect("generated mapping codepoints"))
}

mod punycode;
mod raw_unicode_escape;
mod text;
mod unicode;
mod unicode_escape;
mod unicode_name_ranges;
mod unicode_names;
mod utf7;
pub use text::{Text, contains_ascii_lower_with_scratch};
/// Native source codec registry. None denotes an internal registry omission,
/// never a runtime fallback to another backend.
/// Empty bodies short-circuit before registry rejection, matching HTTPX.
pub fn decode_implemented(bytes: &[u8], label: &str) -> Option<Result<Text, String>> {
    if bytes.is_empty() {
        return Some(Ok(Text::from("")));
    }
    let name = canonical(label).unwrap_or("utf-8");
    if let Some(text) = decode_single_byte_points(bytes, name) {
        return Some(Ok(text));
    }
    if let Some(text) = doublebyte::decode(bytes, name) {
        return Some(Ok(text));
    }
    if let Some(result) = iso2022::decode(bytes, name) {
        return Some(result);
    }
    if let Some(result) = unicode::decode(bytes, name) {
        return Some(result);
    }
    match name {
        "hz" => Some(Ok(hz::decode(bytes))),
        "unicode-escape" => unicode_escape::decode(bytes).map(Ok),
        "punycode" => Some(Ok(punycode::decode(bytes))),
        "raw-unicode-escape" => Some(Ok(raw_unicode_escape::decode(bytes))),
        "ascii" => Some(Ok(Text::from_codepoints(
            bytes
                .iter()
                .map(|b| if b.is_ascii() { u32::from(*b) } else { 0xfffd })
                .collect(),
        )
        .expect("ASCII replacement codepoints"))),
        "charmap" | "iso8859-1" => Some(Ok(Text::from_codepoints(
            bytes.iter().copied().map(u32::from).collect(),
        )
        .expect("Latin1 codepoints"))),
        "base64" | "bz2" | "hex" | "quopri" | "uu" | "zlib" => Some(Err(String::new())),
        "rot-13" => Some(Err(
            "descriptor 'translate' for 'str' objects doesn't apply to a 'bytes' object".into(),
        )),
        "idna" => Some(Err("Unsupported error handling: replace".into())),
        "undefined" => Some(Err("undefined encoding".into())),
        _ => None,
    }
}

/// HTTPX codec failures remain distinct from a broken native registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    Codec(String),
    RegistryInvariant,
}
impl std::fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Codec(message) => formatter.write_str(message),
            Self::RegistryInvariant => {
                formatter.write_str("native HTTP text decoder registry invariant")
            }
        }
    }
}
impl std::error::Error for DecodeError {}
/// Run only inside a supervised native worker for request-owned input.
/// Empty body precedes label rejection; an unknown label selects UTF8 exactly
/// as the source HTTPX text decoder. No alternate runtime backend is invoked.
pub fn decode_http(body: &[u8], label: &str) -> Result<Text, DecodeError> {
    decode_implemented(body, label)
        .ok_or(DecodeError::RegistryInvariant)?
        .map_err(DecodeError::Codec)
}

/// Result of strict IDNA parameter decoding. Unicode failures tell the email
/// selector to retain its original parameter text; they are not body errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrictAsciiOutcome {
    Ascii(String),
    NonAscii,
    UnicodeFailure,
}
/// Narrow RFC2231 IDNA capability. Run inside the supervised decoder worker;
/// validates all labels and the exact source case-insensitive roundtrip.
pub fn strict_idna_parameter(bytes: &[u8]) -> Result<StrictAsciiOutcome, ParameterError> {
    match idna::ascii_outcome(bytes) {
        Ok(true) => Ok(StrictAsciiOutcome::Ascii(
            std::str::from_utf8(bytes)
                .expect("strict IDNA ASCII outcome")
                .to_owned(),
        )),
        Ok(false) => Ok(StrictAsciiOutcome::NonAscii),
        Err(punycode::StrictError::Unicode { .. }) => Ok(StrictAsciiOutcome::UnicodeFailure),
        Err(punycode::StrictError::Overflow) => Err(ParameterError::OverflowError(
            "Python int too large to convert to C ssize_t",
        )),
    }
}

mod bounded_parameter;
pub use bounded_parameter::{
    BoundedParameterError, lookup_http_encoding_bounded, strict_parameter_ascii_bounded,
};

/// Opt-in strict Python IDNA2008 profile; existing IDNA2003 remains unchanged.
#[cfg(feature = "source-idna2008")]
pub mod idna2008;
#[cfg(feature = "source-idna2008")]
mod idna2008_data;
/// Pinned Python text operations for the opt-in source URL consumer.
#[cfg(feature = "source-idna2008")]
pub mod python_text;
