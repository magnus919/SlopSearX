//! Python Unicode16 hostname lower, preserving the suffix after the first '%'.
//! This models urllib hostname identity only; it is not a URL parser.
use ssx_owned_budget::{Budget, OwnedString};
use ssx_source_codecs::BoundedTextError;
use std::time::Instant;
#[path = "hostname_lower_data.rs"]
mod data;
const BLOCK: usize = 1024;
fn property(point: u32, ranges: &[(u32, u32)]) -> bool {
    let index = ranges.partition_point(|(_, end)| *end < point);
    ranges.get(index).is_some_and(|(start, _)| *start <= point)
}
async fn checkpoint(deadline: Instant) -> Result<(), BoundedTextError> {
    if Instant::now() >= deadline {
        return Err(BoundedTextError::Deadline);
    }
    tokio::task::yield_now().await;
    if Instant::now() >= deadline {
        return Err(BoundedTextError::Deadline);
    }
    Ok(())
}
async fn visit(
    text: &str,
    deadline: Instant,
    mut emit: impl FnMut(&str) -> Result<(), BoundedTextError>,
) -> Result<(), BoundedTextError> {
    let mut previous_cased = false;
    let mut work = 0usize;
    for (offset, char) in text.char_indices() {
        if work.is_multiple_of(BLOCK) {
            checkpoint(deadline).await?;
        }
        work += 1;
        if char == 'Σ' {
            let mut following_cased = false;
            if previous_cased {
                for next in text[offset + char.len_utf8()..].chars() {
                    if work.is_multiple_of(BLOCK) {
                        checkpoint(deadline).await?;
                    }
                    work += 1;
                    if !property(next as u32, data::CASE_IGNORABLE) {
                        following_cased = property(next as u32, data::CASED);
                        break;
                    }
                }
            }
            emit(if previous_cased && !following_cased {
                "ς"
            } else {
                "σ"
            })?;
        } else if let Ok(index) =
            data::LOWER.binary_search_by_key(&(char as u32), |(point, _)| *point)
        {
            emit(data::LOWER[index].1)?;
        } else {
            let mut bytes = [0; 4];
            emit(char.encode_utf8(&mut bytes))?;
        }
        if !property(char as u32, data::CASE_IGNORABLE) {
            previous_cased = property(char as u32, data::CASED);
        }
    }
    checkpoint(deadline).await
}
/// Two passes compute exact output bytes before allocation. Neither pass uses
/// Rust's current Unicode version. All output shares the caller's root Budget.
pub async fn lower_hostname(
    text: &str,
    budget: &Budget,
    deadline: Instant,
) -> Result<OwnedString, BoundedTextError> {
    let mut percent = text.len();
    for (work, (at, char)) in text.char_indices().enumerate() {
        if work.is_multiple_of(BLOCK) {
            checkpoint(deadline).await?;
        }
        if char == '%' {
            percent = at;
            break;
        }
    }
    let (prefix, suffix) = text.split_at(percent);
    let mut size = suffix.len();
    visit(prefix, deadline, |part| {
        size = size
            .checked_add(part.len())
            .ok_or(ssx_owned_budget::Error::Overflow)?;
        Ok(())
    })
    .await?;
    let mut output = OwnedString::new(budget)?;
    output.try_reserve_exact(size)?;
    visit(prefix, deadline, |part| {
        output.try_push_str(part)?;
        Ok(())
    })
    .await?;
    let mut at = 0;
    while at < suffix.len() {
        checkpoint(deadline).await?;
        let mut end = at.saturating_add(BLOCK).min(suffix.len());
        while !suffix.is_char_boundary(end) {
            end -= 1;
        }
        output.try_push_str(&suffix[at..end])?;
        at = end;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn complete_pinned_mapping_and_context_controls() {
        let data: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a06/source-hostname-lower.json"
        ))
        .unwrap();
        let budget = Budget::new(1024 * 1024).unwrap();
        let deadline = Instant::now() + std::time::Duration::from_secs(30);
        for row in data["records"].as_array().unwrap() {
            let input = row[0].as_str().unwrap();
            let expected = row[1].as_str().unwrap();
            let output = lower_hostname(input, &budget, deadline).await.unwrap();
            assert_eq!(output.as_str(), expected, "{input:?}");
            drop(output);
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
    }
    #[tokio::test]
    async fn refused_output_and_expired_deadline_release() {
        let budget = Budget::new(1).unwrap();
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        assert!(lower_hostname("İΣ", &budget, deadline).await.is_err());
        assert_eq!(budget.snapshot().unwrap().used, 0);
        assert!(matches!(
            lower_hostname("A", &budget, Instant::now()).await,
            Err(BoundedTextError::Deadline)
        ));
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[tokio::test]
    async fn charged_copy_repeated_pending_then_drop_or_real_deadline() {
        use std::{
            future::Future,
            task::{Context, Poll, Waker},
            time::Duration,
        };
        for expire in [false, true] {
            // Long CaseIgnorable lookahead and preserved Unicode zone both need
            // scheduling points while the charged output allocation stays live.
            let text = format!("AΣ{}%{}", "\u{301}".repeat(65536), "ZONEΣ".repeat(16384));
            let budget = Budget::new(1024 * 1024).unwrap();
            let deadline = Instant::now() + Duration::from_secs(if expire { 2 } else { 30 });
            let mut future = Box::pin(lower_hostname(&text, &budget, deadline));
            let mut context = Context::from_waker(Waker::noop());
            let mut observed = 0;
            for _ in 0..100000 {
                assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
                if budget.snapshot().unwrap().used > 0 {
                    observed += 1;
                    if observed == 16 {
                        break;
                    }
                }
            }
            assert_eq!(observed, 16);
            if expire {
                tokio::time::sleep_until(deadline.into()).await;
                assert!(matches!(future.await, Err(BoundedTextError::Deadline)));
            } else {
                drop(future);
            }
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
    }
}
