//! Unicode 16 authority validation for CPython-compatible URL joins.

#[path = "python_url_join_data.rs"]
mod url_join_data;

/// Whether this scalar's Unicode 16 NFKC decomposition introduces a URL
/// authority delimiter (/ ? # @ :). Apply this to scalars in the authority
/// extracted before joining, as in CPython urlsplit.
pub fn url_join_nfkc_introduces_delimiter(ch: char) -> bool {
    url_join_data::HAZARDS.binary_search(&u32::from(ch)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    const UNICODE_DATA: &str = include_str!("../provenance/idna/inputs/UnicodeData-16.0.0.txt");
    const DELIMITERS: [u32; 5] = [
        b'/' as u32,
        b'?' as u32,
        b'#' as u32,
        b'@' as u32,
        b':' as u32,
    ];

    fn expands_to_delimiter(point: u32, mappings: &BTreeMap<u32, Vec<u32>>) -> bool {
        DELIMITERS.contains(&point)
            || mappings.get(&point).is_some_and(|parts| {
                parts
                    .iter()
                    .any(|part| expands_to_delimiter(*part, mappings))
            })
    }

    #[test]
    fn url_join_delimiter_guard_matches_unicode16_decomposition_closure() {
        let mut mappings = BTreeMap::new();
        for row in UNICODE_DATA.lines() {
            let fields: Vec<_> = row.split(';').collect();
            let point = u32::from_str_radix(fields[0], 16).unwrap();
            let encoded = fields[5];
            let parts: Vec<_> = encoded
                .split_whitespace()
                .filter(|part| !part.starts_with('<'))
                .map(|part| u32::from_str_radix(part, 16).unwrap())
                .collect();
            if !parts.is_empty() {
                if !encoded.starts_with('<') {
                    // No canonical composition creates or consumes a delimiter.
                    // Algorithmic Hangul composition only uses non-ASCII scalars.
                    assert!(!DELIMITERS.contains(&point));
                    assert!(parts.iter().all(|part| !DELIMITERS.contains(part)));
                }
                mappings.insert(point, parts);
            }
        }
        let expected: Vec<_> = mappings
            .keys()
            .copied()
            .filter(|point| expands_to_delimiter(*point, &mappings))
            .collect();
        assert_eq!(expected.len(), 19);
        assert_eq!(url_join_data::HAZARDS, expected);
        for point in expected {
            assert!(url_join_nfkc_introduces_delimiter(
                char::from_u32(point).unwrap()
            ));
        }
        assert!(!url_join_nfkc_introduces_delimiter('/'));
        assert!(!url_join_nfkc_introduces_delimiter('é'));
    }
}
