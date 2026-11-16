//! Source Unicode 16 properties absent from the shared F05 text API.
use std::sync::OnceLock;
fn table() -> &'static ssx_json::Value {
    static TABLE: OnceLock<ssx_json::Value> = OnceLock::new();
    TABLE.get_or_init(|| {
        ssx_json::from_str(include_str!("../../../docs/rust-rewrite/f11/unicode.json"))
            .expect("reviewed Unicode 16 properties")
    })
}
fn in_ranges(c: char, key: &str) -> bool {
    table()[key]
        .as_array()
        .expect("range list")
        .iter()
        .any(|r| {
            r[0].as_u64().expect("range start") <= c as u64
                && c as u64 <= r[1].as_u64().expect("range end")
        })
}
/// Character predicate used by Python str.isdigit: decimal digits and Unicode16 digit-extra characters.
/// Whole-string callers must additionally reject the empty string.
pub fn digit(c: char) -> bool {
    ssx_json::decimal_digit(c) || in_ranges(c, "digit_extra_ranges")
}
pub(crate) fn word(c: char) -> bool {
    in_ranges(c, "word_ranges")
}
pub(crate) fn ignorecase(c: char) -> char {
    match c {
        'İ' | 'ı' => 'i',
        'ſ' => 's',
        'K' => 'k',
        c => c.to_ascii_lowercase(),
    }
}
