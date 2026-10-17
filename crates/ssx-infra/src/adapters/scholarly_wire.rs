//! Engine-local request construction only; no factories or decoder fallback.
use super::scholarly_word::WORD;
use url::Url;
fn whitespace(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}
fn word(c: char) -> bool {
    let c = u32::from(c);
    WORD.binary_search_by(|&(start, end)| {
        if end < c {
            std::cmp::Ordering::Less
        } else if start > c {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    })
    .is_ok()
}
/// Python urllib.quote safe='/' versus HTTPX query parameter quote_plus.
fn quote(value: &str, plus: bool) -> String {
    let mut output = String::new();
    const HEX: &[u8] = b"0123456789ABCDEF";
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) || (!plus && byte == b'/') {
            output.push(char::from(byte));
        } else if plus && byte == b' ' {
            output.push('+');
        } else {
            output.push('%');
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 15)]));
        }
    }
    output
}
pub fn arxiv_query(query: &str) -> String {
    let stripped = query.trim_matches(whitespace);
    let mut count = 0;
    let mut plain = true;
    let mut boolean = false;
    for term in stripped.split(whitespace).filter(|t| !t.is_empty()) {
        count += 1;
        plain &= term.chars().all(|c| c == '-' || word(c));
        boolean |= ["AND", "OR", "ANDNOT"]
            .iter()
            .any(|operator| term.eq_ignore_ascii_case(operator));
    }
    if count >= 2 && plain && !boolean {
        format!("all:\"{stripped}\"")
    } else {
        format!("all:{query}")
    }
}
fn parameters(base: &str, pairs: &[(&str, &str)]) -> Result<String, url::ParseError> {
    let mut url = Url::parse(base)?;
    let query = pairs
        .iter()
        .map(|(k, v)| format!("{}={}", quote(k, true), quote(v, true)))
        .collect::<Vec<_>>()
        .join("&");
    url.set_query(Some(&query));
    Ok(url.into())
}
pub fn arxiv_url(base: &str, query: &str, limit: usize) -> Result<String, url::ParseError> {
    arxiv_url_text(base, query, &limit.to_string())
}
fn arxiv_url_text(base: &str, query: &str, limit: &str) -> Result<String, url::ParseError> {
    parameters(
        base,
        &[
            ("search_query", &arxiv_query(query)),
            ("start", "0"),
            ("max_results", limit),
            ("sortBy", "relevance"),
            ("sortOrder", "descending"),
        ],
    )
}
pub fn semantic_scholar_url(
    base: &str,
    query: &str,
    limit: usize,
) -> Result<String, url::ParseError> {
    semantic_scholar_url_text(base, query, &limit.to_string())
}
fn semantic_scholar_url_text(
    base: &str,
    query: &str,
    limit: &str,
) -> Result<String, url::ParseError> {
    parameters(
        base,
        &[
            ("query", query),
            ("limit", limit),
            (
                "fields",
                "title,url,abstract,citationCount,publicationDate,externalIds,authors",
            ),
        ],
    )
}
/// Raw scalar variants preserve F06 type until request rendering. Existing usize
/// helpers remain compatibility conveniences for their accepted integer controls.
pub fn arxiv_url_scalar(
    base: &str,
    query: &str,
    limit: &super::scholarly_config::Scalar,
) -> Result<String, url::ParseError> {
    arxiv_url_text(base, query, &limit.query_text())
}
pub fn semantic_scholar_url_scalar(
    base: &str,
    query: &str,
    limit: &super::scholarly_config::Scalar,
) -> Result<String, url::ParseError> {
    semantic_scholar_url_text(base, query, &limit.query_text())
}
pub fn openalex_url_scalar(
    base: &str,
    query: &str,
    limit: &super::scholarly_config::Scalar,
    lower: Option<&str>,
    upper: Option<&str>,
) -> String {
    openalex_url_text(base, query, &limit.python_text(), lower, upper)
}
/// Actual OpenAlex validates dates/time_range before quota admission. The
/// enclosing service's relative-window/proof policy remains authoritative.
pub fn openalex_preflight(
    request: &ssx_domain::SearchRequest,
) -> Result<ssx_application::filters::PublicationBounds, String> {
    let bounds = ssx_application::filters::PublicationBounds::new(
        request.date_from.as_deref(),
        request.date_to.as_deref(),
    )
    .map_err(|error| error.message)?;
    if request.time_range.as_deref().is_some_and(|range| {
        !range.is_empty() && ssx_application::filters::time_range_days(range).is_none()
    }) {
        return Err("time_range must be day, week, month, or year".into());
    }
    Ok(bounds)
}
/// Caller validates inclusive date bounds before admission. This function only
/// preserves the source query order/quote mode and never invents an anchor.
pub fn openalex_url(
    base: &str,
    query: &str,
    limit: usize,
    lower: Option<&str>,
    upper: Option<&str>,
) -> String {
    openalex_url_text(base, query, &limit.to_string(), lower, upper)
}
fn openalex_url_text(
    base: &str,
    query: &str,
    limit: &str,
    lower: Option<&str>,
    upper: Option<&str>,
) -> String {
    let mut url = format!(
        "{base}/works?search={}&per_page={limit}",
        quote(query, false)
    );
    let mut filters = Vec::new();
    if let Some(date) = lower {
        filters.push(format!("from_publication_date:{date}"));
    }
    if let Some(date) = upper {
        filters.push(format!("to_publication_date:{date}"));
    }
    if !filters.is_empty() {
        url.push_str("&filter=");
        url.push_str(&quote(&filters.join(","), false));
    }
    url
}
/// Source-owned arXiv query scanning/encoding shares the admitted operation
/// budget. URL/header/socket storage beyond this returned query remains the
/// explicitly open F08/G02 transport ownership boundary.
pub async fn arxiv_parameters_budgeted(
    budget: &ssx_owned_budget::Budget,
    query: &str,
    limit: &super::scholarly_config::Scalar,
    deadline: std::time::Instant,
) -> Result<ssx_owned_budget::OwnedString, super::source_payload::BudgetError> {
    arxiv_parameters_for_clock(
        budget,
        query,
        limit,
        deadline,
        &crate::http::DeadlineClock::default(),
    )
    .await
}
pub(crate) async fn arxiv_parameters_for_clock(
    budget: &ssx_owned_budget::Budget,
    query: &str,
    limit: &super::scholarly_config::Scalar,
    deadline: std::time::Instant,
    clock: &crate::http::DeadlineClock,
) -> Result<ssx_owned_budget::OwnedString, super::source_payload::BudgetError> {
    use super::source_payload::BudgetError;
    use ssx_owned_budget::OwnedString;
    async fn check(
        deadline: std::time::Instant,
        clock: &crate::http::DeadlineClock,
    ) -> Result<(), BudgetError> {
        tokio::task::yield_now().await;
        if clock.expired(deadline) {
            Err(BudgetError::Deadline)
        } else {
            Ok(())
        }
    }
    async fn encode(
        out: &mut OwnedString,
        text: &str,
        deadline: std::time::Instant,
        clock: &crate::http::DeadlineClock,
    ) -> Result<(), BudgetError> {
        const HEX: &[u8] = b"0123456789ABCDEF";
        for (index, byte) in text.bytes().enumerate() {
            if index % 4096 == 0 {
                check(deadline, clock).await?;
            }
            // OwnedString's exact-capacity append must not turn this bounded
            // byte loop into a fresh old-buffer copy for every encoded byte.
            let wanted = out.len().checked_add(3).ok_or(BudgetError::Resource)?;
            if wanted > out.capacity() {
                let capacity = out
                    .capacity()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(8))
                    .ok_or(BudgetError::Resource)?
                    .max(wanted);
                out.try_reserve_exact(capacity - out.len())
                    .map_err(|_| BudgetError::Resource)?;
            }
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                out.try_push(char::from(byte))
                    .map_err(|_| BudgetError::Resource)?;
            } else if byte == b' ' {
                out.try_push('+').map_err(|_| BudgetError::Resource)?;
            } else {
                for c in [
                    '%',
                    char::from(HEX[usize::from(byte >> 4)]),
                    char::from(HEX[usize::from(byte & 15)]),
                ] {
                    out.try_push(c).map_err(|_| BudgetError::Resource)?;
                }
            }
        }
        Ok(())
    }
    check(deadline, clock).await?;
    let (mut first, mut last) = (None, 0);
    for (index, (at, c)) in query.char_indices().enumerate() {
        if index % 1024 == 0 {
            check(deadline, clock).await?;
        }
        if !whitespace(c) {
            first.get_or_insert(at);
            last = at + c.len_utf8();
        }
    }
    let stripped = &query[first.unwrap_or(last)..last];
    let (mut terms, mut plain, mut boolean, mut term_start) = (0, true, false, None);
    for (index, (at, c)) in stripped
        .char_indices()
        .chain(std::iter::once((stripped.len(), ' ')))
        .enumerate()
    {
        if index % 1024 == 0 {
            check(deadline, clock).await?;
        }
        if whitespace(c) {
            if let Some(start) = term_start.take() {
                terms += 1;
                let term = &stripped[start..at];
                // Matching candidates have at most six ASCII bytes; reject
                // long terms before eq_ignore_ascii_case can scan them.
                if term.len() <= 6 {
                    boolean |= ["AND", "OR", "ANDNOT"]
                        .iter()
                        .any(|op| term.eq_ignore_ascii_case(op));
                }
            }
        } else {
            term_start.get_or_insert(at);
            plain &= c == '-' || word(c);
        }
    }
    let quoted = terms >= 2 && plain && !boolean;
    let mut out = OwnedString::new(budget).map_err(|_| BudgetError::Resource)?;
    out.try_push_str("search_query=all%3A")
        .map_err(|_| BudgetError::Resource)?;
    if quoted {
        out.try_push_str("%22").map_err(|_| BudgetError::Resource)?;
    }
    encode(
        &mut out,
        if quoted { stripped } else { query },
        deadline,
        clock,
    )
    .await?;
    if quoted {
        out.try_push_str("%22").map_err(|_| BudgetError::Resource)?;
    }
    out.try_push_str("&start=0&max_results=")
        .map_err(|_| BudgetError::Resource)?;
    // Numeric source formatting is finite (i64/f64); text is borrowed. Keep the
    // reservation through encoding so its tiny numeric temporary overlaps out.
    let _numeric = budget.reserve(256).map_err(|_| BudgetError::Resource)?;
    let limit = limit.query_text();
    encode(&mut out, &limit, deadline, clock).await?;
    out.try_push_str("&sortBy=relevance&sortOrder=descending")
        .map_err(|_| BudgetError::Resource)?;
    check(deadline, clock).await?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn historical_first_requests_and_openalex_preflight_errors_remain_source_exact() {
        let mut count = 0;
        let mut invalid = 0;
        for raw in [
            include_str!("../../../../docs/rust-rewrite/a06/source-initial.json"),
            include_str!("../../../../docs/rust-rewrite/a06/source-regressions.json"),
            include_str!("../../../../docs/rust-rewrite/a06/source-shared.json"),
        ] {
            let corpus: ssx_json::Value = ssx_json::from_str(raw).unwrap();
            for row in corpus["records"].as_array().unwrap() {
                let engine = row["engine"].as_str().unwrap();
                let query = row["query"].as_str().unwrap();
                let cfg = &row["config"];
                let limit = cfg["max_results"]
                    .as_u64()
                    .unwrap_or(if engine == "openalex" { 10 } else { 5 })
                    as usize;
                let request = ssx_domain::SearchRequest {
                    date_from: row["params"]["date_from"].as_str().map(str::to_owned),
                    date_to: row["params"]["date_to"].as_str().map(str::to_owned),
                    time_range: row["params"]["time_range"].as_str().map(str::to_owned),
                    ..Default::default()
                };
                if engine == "openalex"
                    && let Err(message) = openalex_preflight(&request)
                {
                    assert_eq!(message, row["output"]["error_message"].as_str().unwrap());
                    assert!(row["requests"].as_array().unwrap().is_empty());
                    invalid += 1;
                    continue;
                }
                let Some(expected) = row["requests"].as_array().unwrap().first() else {
                    continue;
                };
                let actual = match engine {
                    "arxiv" => arxiv_url(
                        cfg["base_url"]
                            .as_str()
                            .unwrap_or("https://export.arxiv.org/api/query"),
                        query,
                        limit,
                    )
                    .unwrap(),
                    "semanticscholar" => semantic_scholar_url(
                        cfg["base_url"]
                            .as_str()
                            .unwrap_or("https://api.semanticscholar.org/graph/v1/paper/search"),
                        query,
                        limit,
                    )
                    .unwrap(),
                    "openalex" => openalex_url(
                        cfg["base_url"]
                            .as_str()
                            .unwrap_or("https://api.openalex.org"),
                        query,
                        limit,
                        request.date_from.as_deref(),
                        request.date_to.as_deref(),
                    ),
                    _ => unreachable!(),
                };
                assert_eq!(actual, expected["url"].as_str().unwrap(), "{}", row["id"]);
                count += 1;
            }
        }
        assert_eq!(invalid, 9);
        assert_eq!(count, 106);
    }
    #[test]
    fn exhaustive_unicode_word_ranges_and_actual_source_request_urls() {
        let fixture: ssx_json::Value = ssx_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a06/source-wire.json"
        ))
        .unwrap();
        let ranges = fixture["word_predicate"]["ranges"].as_array().unwrap();
        assert_eq!(ranges.len(), WORD.len());
        for (actual, expected) in WORD.iter().zip(ranges) {
            assert_eq!(
                (actual.0 as u64, actual.1 as u64),
                (expected[0].as_u64().unwrap(), expected[1].as_u64().unwrap())
            );
        }
        // Every Unicode scalar is checked against the captured exhaustive oracle,
        // including combining marks that Rust Alphabetic/regex \\w may classify differently.
        let mut range = 0;
        for point in 0..=0x10ffff {
            while range < WORD.len() && WORD[range].1 < point {
                range += 1;
            }
            if let Some(c) = char::from_u32(point) {
                assert_eq!(word(c), range < WORD.len() && WORD[range].0 <= point);
            }
        }
        let mut count = 0;
        for row in fixture["records"].as_array().unwrap().iter().take(63) {
            let query = row["query"].as_str().unwrap();
            let actual = match row["engine"].as_str().unwrap() {
                "arxiv" => arxiv_url("https://export.arxiv.org/api/query", query, 5).unwrap(),
                "semanticscholar" => semantic_scholar_url(
                    "https://api.semanticscholar.org/graph/v1/paper/search",
                    query,
                    5,
                )
                .unwrap(),
                "openalex" => openalex_url("https://api.openalex.org", query, 10, None, None),
                _ => unreachable!(),
            };
            assert_eq!(
                actual,
                row["requests"][0]["url"].as_str().unwrap(),
                "{}",
                row["id"]
            );
            count += 1;
        }
        assert_eq!(count, 63);
    }
}

#[cfg(test)]
mod bounded_arxiv_growth_controls {
    use super::*;
    use crate::adapters::{scholarly_config::Scalar, source_payload::BudgetError};
    use ssx_owned_budget::Budget;
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
        time::{Duration, Instant},
    };
    #[tokio::test]
    async fn large_complete_query_matches_full_original_request_bytes() {
        let query = "z".repeat(1024 * 1024);
        let budget = Budget::new(8 * 1024 * 1024).unwrap();
        let actual = arxiv_parameters_budgeted(
            &budget,
            &query,
            &Scalar::Integer(5),
            Instant::now() + Duration::from_secs(10),
        )
        .await
        .unwrap();
        let source = arxiv_url_scalar(
            "https://export.arxiv.org/api/query",
            &query,
            &Scalar::Integer(5),
        )
        .unwrap();
        let source = url::Url::parse(&source).unwrap();
        assert_eq!(actual.as_bytes(), source.query().unwrap().as_bytes());
        assert_eq!(
            actual.len(),
            query.len()
                + "search_query=all%3A&start=0&max_results=5&sortBy=relevance&sortOrder=descending"
                    .len()
        );
        assert!(budget.snapshot().unwrap().peak > actual.charged_bytes());
        drop(actual);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[tokio::test]
    async fn large_encoding_quota_drop_and_real_deadline_keep_one_root() {
        let query = "z".repeat(1024 * 1024);
        let tiny = Budget::new(1024).unwrap();
        assert!(matches!(
            arxiv_parameters_budgeted(
                &tiny,
                &query,
                &Scalar::Integer(5),
                Instant::now() + Duration::from_secs(30)
            )
            .await,
            Err(BudgetError::Resource)
        ));
        assert_eq!(tiny.snapshot().unwrap().used, 0);
        for expire in [false, true] {
            let budget = Budget::new(8 * 1024 * 1024).unwrap();
            let deadline = Instant::now() + Duration::from_secs(if expire { 2 } else { 30 });
            let limit = Scalar::Integer(5);
            let mut future = Box::pin(arxiv_parameters_budgeted(&budget, &query, &limit, deadline));
            let mut context = Context::from_waker(Waker::noop());
            let mut observed = 0;
            for _ in 0..100000 {
                assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
                if budget.snapshot().unwrap().used >= 1024 * 1024 {
                    observed += 1;
                    if observed == 16 {
                        break;
                    }
                }
            }
            assert_eq!(observed, 16);
            if expire {
                tokio::time::sleep_until(deadline.into()).await;
                assert!(matches!(future.await, Err(BudgetError::Deadline)));
            } else {
                drop(future);
            }
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
    }
}
