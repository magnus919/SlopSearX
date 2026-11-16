//! TMDB capitalize using accepted source Unicode16 tables, independent of Rust Unicode data.
use super::clinicaltrials as c;
use std::{collections::BTreeMap, sync::OnceLock, time::Instant};
#[derive(serde::Deserialize)]
struct Table {
    upper: BTreeMap<u32, String>,
    title: BTreeMap<u32, String>,
    lower: BTreeMap<u32, String>,
    cased: Vec<(u32, u32)>,
    case_ignorable: Vec<(u32, u32)>,
}
fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a08/case16-data.json"
        ))
        .expect("frozen FDA Unicode16 table")
    })
}
fn property(ch: char, ranges: &[(u32, u32)]) -> bool {
    let p = ch as u32;
    let n = ranges.partition_point(|(_, end)| *end < p);
    ranges.get(n).is_some_and(|(start, _)| *start <= p)
}
async fn transform(
    text: &str,
    kind: u8,
    budget: &c::Budget,
    deadline: Option<Instant>,
) -> c::Result<c::OwnedString> {
    let t = table();
    let mut output = c::OwnedString::new(budget).map_err(c::owned_error)?;
    output
        .try_reserve_exact(text.len().checked_mul(3).ok_or_else(c::resource)?)
        .map_err(c::owned_error)?;
    let mut previous_cased = false;
    let mut previous_title_cased = false;
    for (n, (offset, ch)) in text.char_indices().enumerate() {
        if n % 1024 == 0 {
            c::row_checkpoint(deadline).await?
        }
        let upper = kind == 0;
        let title = kind == 1 && !previous_title_cased || kind == 2 && n == 0;
        let mapped = if upper {
            t.upper.get(&(ch as u32))
        } else if title {
            t.title.get(&(ch as u32))
        } else {
            t.lower.get(&(ch as u32))
        };
        if !upper && !title && ch == 'Σ' {
            let mut following = false;
            if previous_cased {
                for (i, next) in text[offset + ch.len_utf8()..].chars().enumerate() {
                    if i % 1024 == 0 {
                        c::row_checkpoint(deadline).await?
                    }
                    if !property(next, &t.case_ignorable) {
                        following = property(next, &t.cased);
                        break;
                    }
                }
            }
            output
                .try_push_str(if previous_cased && !following {
                    "ς"
                } else {
                    "σ"
                })
                .map_err(c::owned_error)?
        } else if let Some(s) = mapped {
            output.try_push_str(s).map_err(c::owned_error)?
        } else {
            output.try_push(ch).map_err(c::owned_error)?
        }
        previous_title_cased = property(ch, &t.cased);
        if !property(ch, &t.case_ignorable) {
            previous_cased = property(ch, &t.cased)
        }
    }
    Ok(output)
}
pub(super) async fn capitalize(
    text: &str,
    budget: &c::Budget,
    deadline: Option<Instant>,
) -> c::Result<c::OwnedString> {
    transform(text, 2, budget, deadline).await
}
pub(super) async fn lower(
    text: &str,
    budget: &c::Budget,
    deadline: Option<Instant>,
) -> c::Result<c::OwnedString> {
    transform(text, 3, budget, deadline).await
}
