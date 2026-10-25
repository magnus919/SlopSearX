//! Pinned Python idna3.18 defaults: IDNA17 data with CPython16 properties.
//! This module neither maps UTS46 input nor changes the existing IDNA2003 codec.
use super::idna2008_data as data;

pub const PROFILE: &str = "python-idna-3.18/IDNA17.0/CPython16.0";

/// Source exception family, with a more specific ordered validation stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    EmptyDomain,
    EmptyLabel,
    DomainLength,
    LabelLength,
    NotNfc,
    Hyphen,
    InitialCombiner,
    InvalidCodepoint,
    InvalidContext,
    UnknownAdjacentCodepoint,
    Bidi,
    InvalidAlabel,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Error {
    pub kind: ErrorKind,
    /// Zero-based label index; zero for a whole-domain error.
    pub label: usize,
    /// One-based codepoint position, when the source stage identifies one.
    pub position: Option<usize>,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{PROFILE}: {:?} in label {}", self.kind, self.label)
    }
}
impl std::error::Error for Error {}
fn error(kind: ErrorKind) -> Error {
    Error {
        kind,
        label: 0,
        position: None,
    }
}
fn at(kind: ErrorKind, position: usize) -> Error {
    Error {
        kind,
        label: 0,
        position: Some(position + 1),
    }
}
fn contains(ranges: &[data::Range], cp: u32) -> bool {
    let i = ranges.partition_point(|range| range.end <= cp);
    ranges.get(i).is_some_and(|range| range.start <= cp)
}
fn property<T: Copy>(ranges: &[data::PropertyRange<T>], cp: u32, default: T) -> T {
    let i = ranges.partition_point(|range| range.end <= cp);
    ranges
        .get(i)
        .filter(|range| range.start <= cp)
        .map_or(default, |range| range.value)
}
fn ccc(cp: u32) -> u8 {
    property(data::CCC, cp, 0)
}
fn bidi(cp: u32) -> &'static str {
    property(data::BIDI, cp, "")
}
fn decompose(cp: u32, out: &mut Vec<u32>) {
    if (0xac00..0xd7a4).contains(&cp) {
        let syllable = cp - 0xac00;
        out.push(0x1100 + syllable / 588);
        out.push(0x1161 + (syllable % 588) / 28);
        let tail = syllable % 28;
        if tail != 0 {
            out.push(0x11a7 + tail);
        }
    } else if let Ok(i) = data::DECOMPOSITIONS.binary_search_by_key(&cp, |d| d.point) {
        let d = &data::DECOMPOSITIONS[i];
        for &point in &data::DECOMPOSITION_POINTS[d.offset..d.offset + d.len] {
            decompose(point, out);
        }
    } else {
        out.push(cp);
    }
}
fn compose(first: u32, second: u32) -> Option<u32> {
    if (0x1100..0x1113).contains(&first) && (0x1161..0x1176).contains(&second) {
        return Some(0xac00 + (first - 0x1100) * 588 + (second - 0x1161) * 28);
    }
    if (0xac00..0xd7a4).contains(&first)
        && (first - 0xac00) % 28 == 0
        && (0x11a8..0x11c3).contains(&second)
    {
        return Some(first + second - 0x11a7);
    }
    data::COMPOSITIONS
        .binary_search_by_key(&(first, second), |c| (c.first, c.second))
        .ok()
        .map(|i| data::COMPOSITIONS[i].result)
}
fn is_nfc(points: &[u32]) -> bool {
    let mut decomposed = Vec::new();
    for &point in points {
        decompose(point, &mut decomposed);
    }
    // Stable canonical ordering, including leading non-starters.
    for i in 1..decomposed.len() {
        let class = ccc(decomposed[i]);
        if class == 0 {
            continue;
        }
        let mut j = i;
        while j > 0 && ccc(decomposed[j - 1]) > class {
            decomposed.swap(j - 1, j);
            j -= 1;
        }
    }
    let mut normalized = Vec::with_capacity(decomposed.len());
    let mut starter: Option<usize> = None;
    let mut last_class = 0;
    for point in decomposed {
        let class = ccc(point);
        if let Some(i) = starter
            && (last_class == 0 || last_class < class)
            && let Some(composed) = compose(normalized[i], point)
        {
            normalized[i] = composed;
            continue;
        }
        if class == 0 {
            starter = Some(normalized.len());
        }
        normalized.push(point);
        last_class = class;
    }
    normalized == points
}
fn joining(cp: u32) -> &'static str {
    if contains(data::JOINING_T, cp) {
        "T"
    } else if contains(data::JOINING_L, cp) {
        "L"
    } else if contains(data::JOINING_R, cp) {
        "R"
    } else if contains(data::JOINING_D, cp) {
        "D"
    } else if contains(data::JOINING_C, cp) {
        "C"
    } else {
        ""
    }
}
fn context_j(points: &[u32], i: usize) -> Result<bool, Error> {
    if i > 0 {
        let previous = points[i - 1];
        if ccc(previous) == 9 {
            return Ok(true);
        }
        if ccc(previous) == 0 && !contains(data::NAMED, previous) {
            return Err(at(ErrorKind::UnknownAdjacentCodepoint, i));
        }
    }
    if points[i] == 0x200d {
        return Ok(false);
    }
    let left = points[..i]
        .iter()
        .rev()
        .copied()
        .find(|&cp| joining(cp) != "T");
    if !left.is_some_and(|cp| matches!(joining(cp), "L" | "D")) {
        return Ok(false);
    }
    let right = points[i + 1..]
        .iter()
        .copied()
        .find(|&cp| joining(cp) != "T");
    Ok(right.is_some_and(|cp| matches!(joining(cp), "R" | "D")))
}
fn context_o(points: &[u32], i: usize) -> bool {
    match points[i] {
        0xb7 => i > 0 && points.get(i + 1) == Some(&0x6c) && points[i - 1] == 0x6c,
        0x375 => points
            .get(i + 1)
            .is_some_and(|&cp| contains(data::SCRIPT_GREEK, cp)),
        0x5f3 | 0x5f4 => i > 0 && contains(data::SCRIPT_HEBREW, points[i - 1]),
        0x30fb => points.iter().any(|&cp| {
            cp != 0x30fb
                && (contains(data::SCRIPT_HIRAGANA, cp)
                    || contains(data::SCRIPT_KATAKANA, cp)
                    || contains(data::SCRIPT_HAN, cp))
        }),
        0x660..=0x669 => !points.iter().any(|cp| (0x6f0..=0x6f9).contains(cp)),
        0x6f0..=0x6f9 => !points.iter().any(|cp| (0x660..=0x669).contains(cp)),
        _ => false,
    }
}
fn check_bidi(points: &[u32]) -> Result<(), Error> {
    let mut has_rtl = false;
    for (i, &cp) in points.iter().enumerate() {
        let direction = bidi(cp);
        if direction.is_empty() {
            return Err(at(ErrorKind::Bidi, i));
        }
        has_rtl |= matches!(direction, "R" | "AL" | "AN");
    }
    if !has_rtl {
        return Ok(());
    }
    let rtl = match bidi(points[0]) {
        "R" | "AL" => true,
        "L" => false,
        _ => return Err(at(ErrorKind::Bidi, 0)),
    };
    let mut valid_ending = false;
    let mut number = None;
    for (i, &cp) in points.iter().enumerate() {
        let direction = bidi(cp);
        let allowed = if rtl {
            matches!(
                direction,
                "R" | "AL" | "AN" | "EN" | "ES" | "CS" | "ET" | "ON" | "BN" | "NSM"
            )
        } else {
            matches!(
                direction,
                "L" | "EN" | "ES" | "CS" | "ET" | "ON" | "BN" | "NSM"
            )
        };
        if !allowed {
            return Err(at(ErrorKind::Bidi, i));
        }
        let ending = if rtl {
            matches!(direction, "R" | "AL" | "EN" | "AN")
        } else {
            matches!(direction, "L" | "EN")
        };
        if ending {
            valid_ending = true;
        } else if direction != "NSM" {
            valid_ending = false;
        }
        if rtl && matches!(direction, "AN" | "EN") {
            if number.is_some_and(|value| value != direction) {
                return Err(at(ErrorKind::Bidi, i));
            }
            number = Some(direction);
        }
    }
    if valid_ending {
        Ok(())
    } else {
        Err(error(ErrorKind::Bidi))
    }
}
fn check_label(label: &str) -> Result<(), Error> {
    let points: Vec<u32> = label.chars().map(u32::from).collect();
    check_points(&points)
}
fn check_points(points: &[u32]) -> Result<(), Error> {
    if points.len() > 1024 {
        return Err(error(ErrorKind::LabelLength));
    }
    if points.is_empty() {
        return Err(error(ErrorKind::EmptyLabel));
    }
    if points.len() > 254 {
        return Err(error(ErrorKind::LabelLength));
    }
    if !is_nfc(points) {
        return Err(error(ErrorKind::NotNfc));
    }
    if (points.get(2) == Some(&0x2d) && points.get(3) == Some(&0x2d))
        || points.first() == Some(&0x2d)
        || points.last() == Some(&0x2d)
    {
        return Err(error(ErrorKind::Hyphen));
    }
    if contains(data::MARKS, points[0]) {
        return Err(error(ErrorKind::InitialCombiner));
    }
    for (i, &point) in points.iter().enumerate() {
        if contains(data::CLASS_PVALID, point) {
            continue;
        }
        if contains(data::CLASS_CONTEXTJ, point) {
            if !context_j(points, i)? {
                return Err(at(ErrorKind::InvalidContext, i));
            }
        } else if contains(data::CLASS_CONTEXTO, point) {
            if !context_o(points, i) {
                return Err(at(ErrorKind::InvalidContext, i));
            }
        } else {
            return Err(at(ErrorKind::InvalidCodepoint, i));
        }
    }
    check_bidi(points)
}
fn ulabel(label: &str) -> Result<String, Error> {
    if label.chars().count() > 1024 {
        return Err(error(ErrorKind::LabelLength));
    }
    if !label.is_ascii() {
        check_label(label)?;
        return Ok(label.to_owned());
    }
    let label = label.to_ascii_lowercase();
    if let Some(payload) = label.strip_prefix("xn--") {
        if payload.is_empty() || payload.ends_with('-') {
            return Err(error(ErrorKind::InvalidAlabel));
        }
        if let Some(decoded) = ::idna::punycode::decode_to_string(payload) {
            check_label(&decoded)?;
            Ok(decoded)
        } else {
            // CPython punycode admits surrogate points before IDNA rejects them.
            // Reuse the existing exact primitive only on library-error paths.
            let decoded = crate::punycode::decode_strict(payload.as_bytes())
                .map_err(|_| error(ErrorKind::InvalidAlabel))?;
            check_points(decoded.codepoints())?;
            decoded
                .into_codepoints()
                .into_iter()
                .map(char::from_u32)
                .collect::<Option<String>>()
                .ok_or(error(ErrorKind::InvalidAlabel))
        }
    } else {
        check_label(&label)?;
        Ok(label)
    }
}
fn alabel(label: &str) -> Result<String, Error> {
    if label.chars().count() > 1024 {
        return Err(error(ErrorKind::LabelLength));
    }
    let encoded = if label.is_ascii() {
        ulabel(label)?;
        label.to_owned()
    } else {
        check_label(label)?;
        format!(
            "xn--{}",
            ::idna::punycode::encode_str(label).ok_or(error(ErrorKind::InvalidAlabel))?
        )
    };
    if encoded.len() > 63 {
        return Err(error(ErrorKind::LabelLength));
    }
    Ok(encoded)
}
fn source_lower(text: &str) -> String {
    super::python_text::lowercase_steps(text)
        .filter_map(|step| match step {
            super::python_text::LowerStep::Scalar(ch) => Some(ch),
            super::python_text::LowerStep::Progress => None,
        })
        .collect()
}
fn domain(domain: &str, display: bool) -> Result<String, Error> {
    let length = domain.chars().count();
    if length > 1024 || length > 254 {
        return Err(error(ErrorKind::DomainLength));
    }
    let mut labels: Vec<&str> = domain
        .split(['.', '\u{3002}', '\u{ff0e}', '\u{ff61}'])
        .collect();
    if domain.is_empty() {
        return Err(error(ErrorKind::EmptyDomain));
    }
    let trailing = labels.last() == Some(&"");
    if trailing {
        labels.pop();
    }
    let mut result = Vec::with_capacity(labels.len() + usize::from(trailing));
    for (i, label) in labels.into_iter().enumerate() {
        let converted = if display {
            match ulabel(label) {
                Err(_)
                    if label
                        .get(..4)
                        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("xn--")) =>
                {
                    Ok(source_lower(label))
                }
                other => other,
            }
        } else {
            alabel(label)
        };
        let converted = converted.map_err(|mut e| {
            e.label = i;
            e
        })?;
        if converted.is_empty() {
            return Err(Error {
                label: i,
                ..error(ErrorKind::EmptyLabel)
            });
        }
        result.push(converted);
    }
    if trailing {
        result.push(String::new());
    }
    let result = result.join(".");
    if !display && result.len() > if trailing { 254 } else { 253 } {
        return Err(error(ErrorKind::DomainLength));
    }
    Ok(result)
}
/// Strict source-default IDNA encoding. No UTS46 mapping or implicit lowercase.
pub fn encode(domain: &str) -> Result<String, Error> {
    self::domain(domain, false)
}
/// Source display decode: recover only invalid ACE-prefixed labels.
pub fn decode_display(domain: &str) -> Result<String, Error> {
    self::domain(domain, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_ascii_case_and_nontransitional_sharp_s() {
        assert_eq!(encode("EXAMPLE.com").unwrap(), "EXAMPLE.com");
        assert_eq!(encode("faß.de").unwrap(), "xn--fa-hia.de");
        assert_eq!(encode("bücher.example.").unwrap(), "xn--bcher-kva.example.");
        assert_eq!(
            decode_display("XN--BCHER-KVA.example").unwrap(),
            "bücher.example"
        );
    }
    #[test]
    fn source_dots_nfc_and_ordered_failures() {
        assert_eq!(encode("a\u{3002}b\u{ff0e}c\u{ff61}").unwrap(), "a.b.c.");
        assert_eq!(encode("e\u{301}").unwrap_err().kind, ErrorKind::NotNfc);
        assert_eq!(
            encode("\u{301}a").unwrap_err().kind,
            ErrorKind::InitialCombiner
        );
        assert_eq!(encode("-a").unwrap_err().kind, ErrorKind::Hyphen);
        assert_eq!(encode("a_b").unwrap_err().kind, ErrorKind::InvalidCodepoint);
        assert_eq!(encode("a..b").unwrap_err().kind, ErrorKind::EmptyLabel);
    }
    #[test]
    fn source_display_recovers_only_ace_and_uses_pinned_lower() {
        assert_eq!(decode_display("XN--.example").unwrap(), "xn--.example");
        assert_eq!(decode_display("xn--AΣ.example").unwrap(), "xn--aς.example");
        assert_eq!(
            decode_display("xn--.bad_label").unwrap_err().kind,
            ErrorKind::InvalidCodepoint
        );
        // RFC3492 digits for U+D800: 55168 -> i,b,9,b (static derivation).
        let error = encode("xn--ib9b").unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidCodepoint);
        assert_eq!(error.position, Some(1));
        assert_eq!(decode_display("xn--ib9b").unwrap(), "xn--ib9b");
    }
    #[test]
    fn source_context_bidi_and_hybrid_version() {
        assert!(encode("l·l").is_ok());
        assert_eq!(encode("a·a").unwrap_err().kind, ErrorKind::InvalidContext);
        assert_eq!(
            encode("a\u{200d}b").unwrap_err().kind,
            ErrorKind::InvalidContext
        );
        assert!(contains(data::CLASS_PVALID, 0x088f));
        assert_eq!(bidi(0x088f), "");
        assert!(is_nfc(&[0x088f]));
        assert_eq!(encode("\u{088f}").unwrap_err().kind, ErrorKind::Bidi);
        // NFC/hyphen checks still precede unknown Unicode16 directionality.
        assert_eq!(encode("-\u{088f}").unwrap_err().kind, ErrorKind::Hyphen);
    }
    #[test]
    fn source_hangul_and_composition_blocking() {
        assert!(is_nfc(&[0xac00]));
        assert!(!is_nfc(&[0x1100, 0x1161]));
        assert!(!is_nfc(&[0x65, 0x301]));
        assert!(is_nfc(&[0xe9]));
        assert_eq!(compose(0x65, 0x301), Some(0xe9));
        // A blocked composition must not erase the intervening equal CCC mark.
        assert!(is_nfc(&[0x71, 0x301, 0x300]));
    }
    #[test]
    fn source_limits_are_codepoints_then_wire_bytes() {
        assert!(encode(&"a".repeat(63)).is_ok());
        assert_eq!(
            encode(&"a".repeat(64)).unwrap_err().kind,
            ErrorKind::LabelLength
        );
        assert_eq!(
            encode(&"a".repeat(255)).unwrap_err().kind,
            ErrorKind::DomainLength
        );
        assert_eq!(encode("").unwrap_err().kind, ErrorKind::EmptyDomain);
        assert_eq!(encode(".").unwrap_err().kind, ErrorKind::EmptyLabel);
    }
}
