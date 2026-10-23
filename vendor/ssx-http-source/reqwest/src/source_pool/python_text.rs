//! Allocation-free Python3.14/Unicode16 text operations.
//!
//! Static tables derive from the attributed, pinned CPython source data under
//! contracts/unicode/python-text-source. No interpreter or host Unicode tables
//! implement these operations. Existing casefold/whitespace profiles are separate.
#[path = "python_text_data.rs"]
mod data;

fn properties(ch: char) -> u8 {
    let cp = ch as u32;
    let at = data::PROPERTIES.partition_point(|row| row.0 <= cp);
    if at == 0 {
        return 0;
    }
    let (_, end, flags) = data::PROPERTIES[at - 1];
    if cp <= end {
        flags
    } else {
        0
    }
}
/// Python Unicode regex word predicate: Unicode16 alphanumeric or underscore.
pub fn word(ch: char) -> bool {
    properties(ch) & 1 != 0
}
fn cased(ch: char) -> bool {
    properties(ch) & 2 != 0
}
fn ignorable(ch: char) -> bool {
    properties(ch) & 4 != 0
}

/// One bounded stream step; lookahead never hides an input-sized scan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LowerStep {
    /// One output Unicode scalar, in source order.
    Scalar(char),
    /// One source lookahead step without output; cooperative callers checkpoint.
    Progress,
}
/// Streaming full lowercase, including the source Final_Sigma context rule.
#[derive(Clone, Debug)]
pub struct Lowercase<'a> {
    input: &'a str,
    chars: std::str::CharIndices<'a>,
    pending: std::str::Chars<'static>,
    scan: Option<std::str::Chars<'a>>,
    preceding_cased: bool,
}
/// Construct a constant-memory, allocation-free Unicode16 lowercase stream.
///
/// Consumers should count both Scalar and Progress steps for cooperative work
/// limits. Output allocation and deadline ownership remain the consumer's job.
pub fn lowercase_steps(input: &str) -> Lowercase<'_> {
    Lowercase {
        input,
        chars: input.char_indices(),
        pending: "".chars(),
        scan: None,
        preceding_cased: false,
    }
}
impl Iterator for Lowercase<'_> {
    type Item = LowerStep;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ch) = self.pending.next() {
            return Some(LowerStep::Scalar(ch));
        }
        if let Some(scan) = self.scan.as_mut() {
            let following = scan.next();
            if following.is_some_and(ignorable) {
                return Some(LowerStep::Progress);
            }
            self.scan = None;
            return Some(LowerStep::Scalar(if following.is_some_and(cased) {
                '\u{3c3}'
            } else {
                '\u{3c2}'
            }));
        }
        let (at, ch) = self.chars.next()?;
        let before = self.preceding_cased;
        if !ignorable(ch) {
            self.preceding_cased = cased(ch);
        }
        if ch == '\u{3a3}' {
            if before {
                self.scan = Some(self.input[at + ch.len_utf8()..].chars());
                return Some(LowerStep::Progress);
            }
            return Some(LowerStep::Scalar('\u{3c3}'));
        }
        if let Ok(at) = data::LOWER.binary_search_by_key(&(ch as u32), |row| row.0) {
            self.pending = data::LOWER[at].1.chars();
            return self.pending.next().map(LowerStep::Scalar);
        }
        Some(LowerStep::Scalar(ch))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const HEADER: &str = include_str!("python-text-source/Objects__unicodetype_db.h.txt");
    fn numeric_array(name: &str) -> Vec<i64> {
        let start = format!("{name}[] = {{");
        let body = HEADER
            .split_once(&start)
            .unwrap()
            .1
            .split_once("};")
            .unwrap()
            .0;
        body.split(|ch: char| !(ch.is_ascii_digit() || ch == '-'))
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().unwrap())
            .collect()
    }
    struct Source {
        records: Vec<[i64; 6]>,
        first: Vec<i64>,
        second: Vec<i64>,
        extended: Vec<i64>,
    }
    impl Source {
        fn new() -> Self {
            let values = numeric_array("_PyUnicode_TypeRecords");
            assert_eq!(values.len() % 6, 0);
            Self {
                records: values
                    .chunks_exact(6)
                    .map(|v| v.try_into().unwrap())
                    .collect(),
                first: numeric_array("index1"),
                second: numeric_array("index2"),
                extended: numeric_array("_PyUnicode_ExtendedCase"),
            }
        }
        fn record(&self, cp: u32) -> [i64; 6] {
            let block = self.first[(cp >> 6) as usize] as usize;
            self.records[self.second[(block << 6) + (cp & 63) as usize] as usize]
        }
        fn lower(&self, cp: u32) -> String {
            let r = self.record(cp);
            let codes: Vec<u32> = if r[5] & 0x4000 != 0 {
                let start = (r[1] & 0xffff) as usize;
                let len = (r[1] >> 24) as usize;
                self.extended[start..start + len]
                    .iter()
                    .map(|v| *v as u32)
                    .collect()
            } else {
                vec![(cp as i64 + r[1]) as u32]
            };
            codes
                .into_iter()
                .map(|cp| char::from_u32(cp).unwrap())
                .collect()
        }
    }
    fn lower(text: &str) -> String {
        lowercase_steps(text)
            .filter_map(|step| match step {
                LowerStep::Scalar(ch) => Some(ch),
                LowerStep::Progress => None,
            })
            .collect()
    }
    #[test]
    fn unicode16_versions_and_static_table_structure_are_closed() {
        assert!(include_str!("python-text-source/Include__patchlevel.h.txt")
            .contains("#define PY_VERSION              \"3.14.7\""));
        assert!(
            include_str!("python-text-source/Modules__unicodedata_db.h.txt")
                .contains("#define UNIDATA_VERSION \"16.0.0\"")
        );
        assert!(HEADER.contains("#define SHIFT 6"));
        assert_eq!(data::PROPERTIES.len(), 1275);
        assert_eq!(data::LOWER.len(), 1460);
        assert!(data::PROPERTIES.windows(2).all(|v| v[0].1 < v[1].0));
        assert!(data::LOWER.windows(2).all(|v| v[0].0 < v[1].0));
    }
    #[test]
    fn every_scalar_matches_frozen_c_word_case_and_full_lower_data() {
        let source = Source::new();
        for cp in 0..0x110000 {
            let Some(ch) = char::from_u32(cp) else {
                continue;
            };
            let r = source.record(cp);
            assert_eq!(
                word(ch),
                r[5] & (1 | 2 | 4 | 0x800) != 0 || ch == '_',
                "word U+{cp:X}"
            );
            assert_eq!(cased(ch), r[5] & 0x2000 != 0, "Cased U+{cp:X}");
            assert_eq!(ignorable(ch), r[5] & 0x1000 != 0, "Case_Ignorable U+{cp:X}");
            assert_eq!(lower(&ch.to_string()), source.lower(cp), "lower U+{cp:X}");
        }
    }
    #[test]
    fn full_lower_expansions_and_final_sigma_use_original_context() {
        for (input, expected) in [
            ("", ""),
            ("İ", "i\u{307}"),
            ("Σ", "σ"),
            ("ΣΣ", "σς"),
            ("AΣ", "aς"),
            ("AΣA", "aσa"),
            ("AΣ\u{345}", "aς\u{345}"),
            ("AΣ'\u{301}", "aς'\u{301}"),
            ("AΣ\u{200d}A", "aσ\u{200d}a"),
            ("\u{1c89}", "\u{1c8a}"),
        ] {
            assert_eq!(lower(input), expected);
        }
    }
    #[test]
    fn word_uses_unicode16_alnum_without_wordbreak_or_casefold_substitution() {
        for ch in ['_', 'A', '0', '²', 'Ⅻ', '界', '\u{1c89}'] {
            assert!(word(ch));
        }
        for ch in [
            '\u{345}',
            '\u{301}',
            '-',
            '.',
            '@',
            '\u{200d}',
            '\u{10ffff}',
        ] {
            assert!(!word(ch));
        }
        assert_eq!(lower("ß"), "ß"); // casefold would expand it.
    }
    #[test]
    fn sigma_lookahead_exposes_bounded_progress_and_keeps_constant_state() {
        let input = format!("AΣ{}A", "\u{301}".repeat(4096));
        let steps: Vec<_> = lowercase_steps(&input).collect();
        assert_eq!(
            steps.iter().filter(|s| **s == LowerStep::Progress).count(),
            4097
        );
        assert_eq!(lower(&input), format!("aσ{}a", "\u{301}".repeat(4096)));
        let mut stopped = lowercase_steps(&input);
        assert_eq!(stopped.next(), Some(LowerStep::Scalar('a')));
        assert_eq!(stopped.next(), Some(LowerStep::Progress));
        drop(stopped);
    }
}
