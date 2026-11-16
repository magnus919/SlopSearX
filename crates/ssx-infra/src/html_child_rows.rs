//! Charge-owning row projection inside the supervised helper. Media sanitizer
//! allocations use the same child root, including shared bounded sanitizer.
use super::{ElementRef, OperationFailure as Failure};
use ssx_owned_budget::{Budget, OwnedString, OwnedVec};

pub(crate) struct Row {
    pub(crate) url: OwnedString,
    pub(crate) title: OwnedString,
    pub(crate) content: OwnedString,
    pub(crate) position: i64,
    pub(crate) img_src: Option<OwnedString>,
    pub(crate) media: Option<ssx_domain::storage::BudgetMediaInfo>,
}
pub(crate) fn owned(budget: &Budget, value: &str) -> Result<OwnedString, Failure> {
    OwnedString::try_from_str(budget, value).map_err(|_| Failure::Resource)
}
pub(crate) fn empty(budget: &Budget) -> Result<OwnedString, Failure> {
    owned(budget, "")
}
pub(crate) fn text(budget: &Budget, node: ElementRef<'_>) -> Result<OwnedString, Failure> {
    let mut total = 0usize;
    let mut start = None;
    let mut end = 0usize;
    for segment in node.text() {
        for (index, ch) in segment.char_indices() {
            if !python_whitespace(ch) {
                start.get_or_insert(total.checked_add(index).ok_or(Failure::Resource)?);
                end = total
                    .checked_add(index)
                    .and_then(|n| n.checked_add(ch.len_utf8()))
                    .ok_or(Failure::Resource)?;
            }
        }
        total = total.checked_add(segment.len()).ok_or(Failure::Resource)?;
    }
    let Some(start) = start else {
        return empty(budget);
    };
    let mut result = empty(budget)?;
    result
        .try_reserve_exact(end - start)
        .map_err(|_| Failure::Resource)?;
    let mut offset = 0usize;
    for segment in node.text() {
        let segment_end = offset.checked_add(segment.len()).ok_or(Failure::Resource)?;
        let left = start.max(offset);
        let right = end.min(segment_end);
        if left < right {
            result
                .try_push_str(&segment[left - offset..right - offset])
                .map_err(|_| Failure::Resource)?;
        }
        offset = segment_end;
        if offset >= end {
            break;
        }
    }
    Ok(result)
}
pub(crate) fn optional_text(
    budget: &Budget,
    node: Option<ElementRef<'_>>,
) -> Result<OwnedString, Failure> {
    node.map_or_else(|| empty(budget), |node| text(budget, node))
}
pub(crate) fn prefixed(budget: &Budget, prefix: &str, value: &str) -> Result<OwnedString, Failure> {
    let size = prefix
        .len()
        .checked_add(value.len())
        .ok_or(Failure::Resource)?;
    let mut out = empty(budget)?;
    out.try_reserve_exact(size).map_err(|_| Failure::Resource)?;
    out.try_push_str(prefix)
        .and_then(|_| out.try_push_str(value))
        .map_err(|_| Failure::Resource)?;
    Ok(out)
}
fn form_bytes(value: &str) -> impl Iterator<Item = u8> + '_ {
    let mut bytes = value.as_bytes();
    std::iter::from_fn(move || {
        let (&first, rest) = bytes.split_first()?;
        bytes = rest;
        if first == b'%' && bytes.len() >= 2 {
            let hex = |c: u8| match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                b'A'..=b'F' => Some(c - b'A' + 10),
                _ => None,
            };
            if let (Some(a), Some(b)) = (hex(bytes[0]), hex(bytes[1])) {
                bytes = &bytes[2..];
                return Some(a * 16 + b);
            }
        }
        Some(if first == b'+' { b' ' } else { first })
    })
}
fn decoded_form(budget: &Budget, value: &str) -> Result<OwnedString, Failure> {
    let mut bytes = OwnedVec::with_capacity(budget, value.len()).map_err(|_| Failure::Resource)?;
    for byte in form_bytes(value) {
        bytes.try_push(byte).map_err(|_| Failure::Resource)?;
    }
    let mut out = empty(budget)?;
    let mut rest = bytes.as_slice();
    loop {
        match std::str::from_utf8(rest) {
            Ok(valid) => {
                out.try_push_str(valid).map_err(|_| Failure::Resource)?;
                break;
            }
            Err(error) => {
                let valid = std::str::from_utf8(&rest[..error.valid_up_to()])
                    .map_err(|_| Failure::Invariant("HTML form UTF8 prefix invariant"))?;
                out.try_push_str(valid)
                    .and_then(|_| out.try_push('\u{fffd}'))
                    .map_err(|_| Failure::Resource)?;
                let Some(invalid) = error.error_len() else {
                    break;
                };
                rest = &rest[error.valid_up_to() + invalid..];
            }
        }
    }
    Ok(out)
}
pub(crate) fn unwrap_redirect(budget: &Budget, raw: &str) -> Result<OwnedString, Failure> {
    if raw.contains("//duckduckgo.com/l/") {
        let query = raw
            .split_once('?')
            .map(|(_, q)| q.split('#').next().unwrap_or(q))
            .unwrap_or("");
        for pair in query.split('&').filter(|p| !p.is_empty()) {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            if !value.is_empty() && form_bytes(key).eq(b"uddg".iter().copied()) {
                let value = decoded_form(budget, value)?;
                if !value.is_empty() {
                    return Ok(value);
                }
            }
        }
    }
    owned(budget, raw)
}

fn python_whitespace(c: char) -> bool {
    matches!(c,'\u{0009}'..='\u{000d}'|'\u{001c}'..='\u{0020}'|'\u{0085}'|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}')
}

// Test-only observation at the exact production bounded sanitizer call boundary.
// No hold, alternate result, quota or production protocol is introduced.
#[cfg(test)]
pub(crate) fn observe_sanitizer_entry(budget: &Budget) {
    use std::io::Write;
    if let Some(path) = std::env::var_os("A05_SANITIZER_ENTRY_FILE") {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
        {
            Ok(mut file) => {
                let used = budget.snapshot().expect("private child ledger").used;
                writeln!(file, "{} {used}", std::process::id()).expect("private phase observation");
                file.flush().expect("private phase flush");
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => panic!("private sanitizer observation failed: {error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn charged_redirect_decoder_matches_previous_full_form_semantics() {
        let budget = Budget::new(4096).unwrap();
        for first in 0..=255u8 {
            for second in 0..=255u8 {
                let raw = format!(
                    "https://duckduckgo.com/l/?%75ddg=%{first:02X}%{second:02X}&uddg=later#ignored"
                );
                let expected = url::form_urlencoded::parse(
                    raw.split_once('?')
                        .unwrap()
                        .1
                        .split('#')
                        .next()
                        .unwrap()
                        .as_bytes(),
                )
                .find(|(k, v)| k == "uddg" && !v.is_empty())
                .unwrap()
                .1
                .into_owned();
                let result = unwrap_redirect(&budget, &raw).unwrap();
                assert_eq!(result.as_str(), expected);
                assert_eq!(budget.snapshot().unwrap().used, result.charged_bytes());
                drop(result);
                assert_eq!(budget.snapshot().unwrap().used, 0);
            }
        }
        for raw in [
            "",
            "https://other/l/?uddg=x",
            "//duckduckgo.com/l/?uddg=&uddg=second",
            "//duckduckgo.com/l/?uddg=%&uddg=second",
            "//duckduckgo.com/l/?uddg=%F0%9F%92",
            "//duckduckgo.com/l/?uddg=a+b%2Bc",
            "//duckduckgo.com/l/?uddg#fragment",
        ] {
            let expected = if raw.contains("//duckduckgo.com/l/") {
                let query = raw
                    .split_once('?')
                    .map(|(_, q)| q.split('#').next().unwrap())
                    .unwrap_or("");
                url::form_urlencoded::parse(query.as_bytes())
                    .find(|(k, v)| k == "uddg" && !v.is_empty())
                    .map(|(_, v)| v.into_owned())
                    .unwrap_or_else(|| raw.into())
            } else {
                raw.into()
            };
            let result = unwrap_redirect(&budget, raw).unwrap();
            assert_eq!(result.as_str(), expected);
            drop(result);
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
    }
    fn tree(parts: &[&str]) -> super::super::Html {
        use super::super::{Html, Node};
        let mut nodes = vec![Node {
            name: "div".into(),
            attrs: vec![],
            text: String::new(),
            parent: None,
            children: (1..=parts.len()).collect(),
            subtree_end: parts.len() + 1,
        }];
        for (index, text) in parts.iter().enumerate() {
            nodes.push(Node {
                name: String::new(),
                attrs: vec![],
                text: (*text).into(),
                parent: Some(0),
                children: vec![],
                subtree_end: index + 2,
            });
        }
        Html {
            nodes,
            charge: None,
        }
    }
    #[test]
    fn charged_text_trims_across_segments_with_python_whitespace_only() {
        let budget = Budget::new(4096).unwrap();
        for parts in [
            vec!["\u{1c} \u{a0}a", " ", "b \u{1f}"],
            vec!["\u{1f}", "", "\u{3000}"],
            vec!["a", "💩", "b"],
            vec!["\u{200b}x\u{200b}"],
        ] {
            let doc = tree(&parts);
            let node = super::super::ElementRef {
                tree: &doc,
                index: 0,
            };
            let result = text(&budget, node).unwrap();
            let expected = parts.concat();
            assert_eq!(result.as_str(), expected.trim_matches(python_whitespace));
            assert_eq!(budget.snapshot().unwrap().used, result.charged_bytes());
            drop(result);
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
        let doc = tree(&["a", "b"]);
        let tiny = Budget::new(1).unwrap();
        assert!(matches!(
            text(
                &tiny,
                super::super::ElementRef {
                    tree: &doc,
                    index: 0
                }
            ),
            Err(Failure::Resource)
        ));
        assert_eq!(tiny.snapshot().unwrap().used, 0);
    }
    #[test]
    fn redirect_resource_failure_is_typed_and_releases_temporary_bytes() {
        let raw = "//duckduckgo.com/l/?uddg=%FF%FF%FF";
        for quota in 1..40 {
            let budget = Budget::new(quota).unwrap();
            match unwrap_redirect(&budget, raw) {
                Ok(value) => {
                    assert_eq!(value.as_str(), "���");
                    drop(value);
                }
                Err(Failure::Resource) => {}
                other => panic!("unexpected redirect result: {}", other.is_ok()),
            }
            assert_eq!(budget.snapshot().unwrap().used, 0, "quota {quota}");
        }
    }
    #[test]
    fn google_row_and_borrowed_key_capacity_sweep_keeps_full_output_or_resource() {
        use crate::{
            adapters::{RowLimit, html_extract_rows},
            html::row_wire::Layout,
        };
        let source = "<div class=g><a href='https://one'><h3>one</h3></a></div><div class=g><a href='https://one'><h3>duplicate</h3></a></div><div class=g><a href='https://two'><h3>two</h3></a></div>";
        // Legacy parse is an uncharged test input constructor. The claim here
        // covers extractor-owned output and borrowed-key index, not that tree.
        let points: Vec<_> = source.chars().map(u32::from).collect();
        let doc = super::super::parse_native_points(&points).unwrap();
        let budget = Budget::new(8192).unwrap();
        let rows = html_extract_rows(&doc, Layout::Google, RowLimit::Unlimited, &budget).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            (
                rows.as_slice()[0].url.as_str(),
                rows.as_slice()[0].title.as_str(),
                rows.as_slice()[0].position
            ),
            ("https://one", "one", 1)
        );
        assert_eq!(
            (
                rows.as_slice()[1].url.as_str(),
                rows.as_slice()[1].title.as_str(),
                rows.as_slice()[1].position
            ),
            ("https://two", "two", 3)
        );
        assert!(
            rows.as_slice()
                .iter()
                .all(|row| row.content.is_empty() && row.img_src.is_none() && row.media.is_none())
        );
        let peak = budget.snapshot().unwrap().peak;
        drop(rows);
        assert_eq!(budget.snapshot().unwrap().used, 0);
        let mut refusals = 0;
        let mut successes = 0;
        for quota in 1..=peak {
            let budget = Budget::new(quota).unwrap();
            match html_extract_rows(&doc, Layout::Google, RowLimit::Unlimited, &budget) {
                Ok(rows) => {
                    assert_eq!(rows.len(), 2);
                    assert_eq!(rows.as_slice()[1].position, 3);
                    drop(rows);
                    successes += 1;
                }
                Err(Failure::Resource) => {
                    refusals += 1;
                }
                Err(error) => panic!("unexpected extraction failure: {error}"),
            }
            assert_eq!(budget.snapshot().unwrap().used, 0, "quota {quota}");
        }
        assert!(refusals > 0 && successes > 0);
        let tiny = Budget::new(1).unwrap();
        assert!(matches!(
            html_extract_rows(&doc, Layout::Google, RowLimit::Invalid(0), &tiny),
            Err(Failure::Parse(
                "'>=' not supported between instances of 'int' and 'NoneType'"
            ))
        ));
        assert_eq!(tiny.snapshot().unwrap().used, 0);
        assert_eq!(
            html_extract_rows(&doc, Layout::Google, RowLimit::Number(0.0), &tiny)
                .unwrap()
                .len(),
            0
        );
        assert_eq!(tiny.snapshot().unwrap().used, 0);
    }
}
