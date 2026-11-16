//! Engine-local cooperative email-date compatibility for Semantic Scholar.
//! Mirrors application/retry_after.rs and pinned Python email.utils; consolidate
//! after guarded shared adoption. Existing shared callers remain unchanged.
use super::scholarly_float::{Error, string_float_scalar};
use ssx_owned_budget::Budget;
use std::time::Instant;
async fn tick(deadline: Instant) -> Result<(), Error> {
    tokio::task::yield_now().await;
    if Instant::now() >= deadline {
        Err(Error::Deadline)
    } else {
        Ok(())
    }
}
// Exact static subset of accepted f11/unicode.json digit_extra_ranges.
const DIGIT_EXTRA: &[(u32, u32)] = &[
    (178, 179),
    (185, 185),
    (4969, 4977),
    (6618, 6618),
    (8304, 8304),
    (8308, 8313),
    (8320, 8329),
    (9312, 9320),
    (9332, 9340),
    (9352, 9360),
    (9450, 9450),
    (9461, 9469),
    (9471, 9471),
    (10102, 10110),
    (10112, 10120),
    (10122, 10130),
    (68160, 68163),
    (69216, 69224),
    (69714, 69722),
    (127232, 127242),
];
fn digit(ch: char) -> bool {
    ssx_domain::storage::python_decimal_digit(ch).is_some()
        || DIGIT_EXTRA
            .iter()
            .any(|&(a, b)| (a..=b).contains(&(ch as u32)))
}
async fn trim(raw: &str, deadline: Instant) -> Result<&str, Error> {
    let (mut a, mut b) = (0, raw.len());
    for (i, (at, c)) in raw.char_indices().enumerate() {
        if i % 1024 == 0 {
            tick(deadline).await?;
        }
        if !ssx_json::python_whitespace(c) {
            a = at;
            break;
        }
        a = at + c.len_utf8();
    }
    for (i, (at, c)) in raw.char_indices().rev().enumerate() {
        if i % 1024 == 0 {
            tick(deadline).await?;
        }
        if !ssx_json::python_whitespace(c) {
            b = at + c.len_utf8();
            break;
        }
        b = at;
    }
    Ok(if a >= b { "" } else { &raw[a..b] })
}
#[derive(Clone, Copy)]
enum Int {
    Invalid,
    Small(i64),
    Large,
}
async fn integer(raw: &str, deadline: Instant) -> Result<Int, Error> {
    let raw = trim(raw, deadline).await?;
    let (negative, raw) = if let Some(s) = raw.strip_prefix('-') {
        (true, s)
    } else {
        (false, raw.strip_prefix('+').unwrap_or(raw))
    };
    let (mut count, mut val, mut large, mut prev) = (0usize, 0i64, false, false);
    for (i, ch) in raw.chars().enumerate() {
        if i % 1024 == 0 {
            tick(deadline).await?;
        }
        if ch == '_' {
            if !prev {
                return Ok(Int::Invalid);
            }
            prev = false;
            continue;
        }
        let Some(d) = ssx_domain::storage::python_decimal_digit(ch) else {
            return Ok(Int::Invalid);
        };
        count += 1;
        if count > 4300 {
            return Ok(Int::Invalid);
        }
        prev = true;
        if let Some(next) = val
            .checked_mul(10)
            .and_then(|v| v.checked_add(i64::from(d)))
        {
            val = next;
        } else {
            large = true;
        }
    }
    Ok(if count == 0 || !prev {
        Int::Invalid
    } else if large {
        Int::Large
    } else {
        Int::Small(if negative { -val } else { val })
    })
}
async fn small(raw: &str, deadline: Instant) -> Result<Option<i64>, Error> {
    Ok(match integer(raw, deadline).await? {
        Int::Small(v) => Some(v),
        _ => None,
    })
}
// Fixed descriptor slots: source only interprets first five tokens, but total
// count controls obsolete three/four-token rewrites; scan all input cooperatively.
async fn tokens(raw: &str, deadline: Instant) -> Result<([&str; 8], usize), Error> {
    let (mut out, mut n, mut start) = ([""; 8], 0usize, None);
    for (i, (at, ch)) in raw.char_indices().enumerate() {
        if i % 1024 == 0 {
            tick(deadline).await?;
        }
        if ssx_json::python_whitespace(ch) {
            if let Some(a) = start.take() {
                if n < 8 {
                    out[n] = &raw[a..at];
                }
                n = n.checked_add(1).ok_or(Error::Resource)?;
            }
        } else if start.is_none() {
            start = Some(at);
        }
    }
    if let Some(a) = start {
        if n < 8 {
            out[n] = &raw[a..];
        }
        n = n.checked_add(1).ok_or(Error::Resource)?;
    }
    Ok((out, n))
}
async fn find(
    raw: &str,
    needle: char,
    last: bool,
    deadline: Instant,
) -> Result<Option<usize>, Error> {
    let mut found = None;
    for (i, (at, ch)) in raw.char_indices().enumerate() {
        if i % 1024 == 0 {
            tick(deadline).await?;
        }
        if ch == needle {
            found = Some(at);
            if !last {
                break;
            }
        }
    }
    Ok(found)
}
fn month(raw: &str) -> Option<i64> {
    const MONTHS: [&str; 24] = [
        "jan",
        "feb",
        "mar",
        "apr",
        "may",
        "jun",
        "jul",
        "aug",
        "sep",
        "oct",
        "nov",
        "dec",
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ];
    MONTHS
        .iter()
        .position(|m| raw.eq_ignore_ascii_case(m))
        .map(|i| (i % 12 + 1) as i64)
}
fn civil(year: i64, month: i64, day: i64) -> Option<i64> {
    if !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
        return None;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max = match month {
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if day < 1 || day > max {
        return None;
    }
    let y = year - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    Some(era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468)
}
async fn zone(raw: &str, deadline: Instant) -> Result<Option<i64>, Error> {
    // Only ASCII names of <=3 chars can match. Full casefold may expand a scalar;
    // fixed stack storage keeps normalization charged by no heap allocation.
    let (mut bytes, mut n, mut overflow) = ([0u8; 3], 0usize, false);
    for (i, ch) in raw.chars().enumerate() {
        if i % 1024 == 0 {
            tick(deadline).await?;
        }
        let mut original = [0; 4];
        let s = ssx_json::casefold_char(ch).unwrap_or_else(|| ch.encode_utf8(&mut original));
        for b in s.bytes() {
            if n < 3 {
                bytes[n] = b.to_ascii_uppercase();
            } else {
                overflow = true;
            }
            n = n.saturating_add(1);
        }
    }
    let named = if overflow {
        None
    } else {
        match &bytes[..n] {
            b"UT" | b"UTC" | b"GMT" | b"Z" => Some(0),
            b"AST" => Some(-400),
            b"ADT" => Some(-300),
            b"EST" => Some(-500),
            b"EDT" => Some(-400),
            b"CST" => Some(-600),
            b"CDT" => Some(-500),
            b"MST" => Some(-700),
            b"MDT" => Some(-600),
            b"PST" => Some(-800),
            b"PDT" => Some(-700),
            _ => None,
        }
    };
    if named.is_some() {
        return Ok(named);
    }
    Ok(match integer(raw, deadline).await? {
        Int::Small(v) => Some(v),
        Int::Invalid => Some(0),
        Int::Large => None,
    })
}
async fn date(raw: &str, deadline: Instant) -> Result<Option<f64>, Error> {
    let (mut data, mut n) = tokens(raw, deadline).await?;
    if n == 0 {
        return Ok(None);
    }
    if data[0].ends_with(',')
        || ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
            .iter()
            .any(|s| data[0].eq_ignore_ascii_case(s))
    {
        data.rotate_left(1);
        n -= 1;
    } else if let Some(i) = find(data[0], ',', true, deadline).await? {
        data[0] = &data[0][i + 1..];
    }
    if n == 3 {
        let raw = data[0];
        let (mut pieces, mut cnt, mut start) = ([""; 3], 0usize, 0usize);
        for (i, (at, ch)) in raw.char_indices().enumerate() {
            if i % 1024 == 0 {
                tick(deadline).await?;
            }
            if ch == '-' {
                if cnt < 3 {
                    pieces[cnt] = &raw[start..at];
                }
                cnt += 1;
                start = at + 1;
            }
        }
        if cnt < 3 {
            pieces[cnt] = &raw[start..];
        }
        cnt += 1;
        if cnt == 3 {
            data[4] = data[2];
            data[3] = data[1];
            data[..3].copy_from_slice(&pieces);
            n = 5;
        }
    }
    if n == 4 {
        let cell = data[3];
        let i = if let Some(i) = find(cell, '+', false, deadline).await? {
            Some(i)
        } else {
            find(cell, '-', false, deadline).await?
        };
        if let Some(i) = i.filter(|i| *i > 0) {
            data[3] = &cell[..i];
            data[4] = &cell[i..];
        } else {
            data[4] = "";
        }
        n = 5;
    }
    if n < 5 {
        return Ok(None);
    }
    let (mut dd, mut yy, mut tm, mut tz) = (data[0], data[2], data[3], data[4]);
    let month = if let Some(m) = month(data[1]) {
        m
    } else {
        dd = data[1];
        let Some(m) = month(data[0]) else {
            return Ok(None);
        };
        m
    };
    dd = dd.strip_suffix(',').unwrap_or(dd);
    if find(yy, ':', false, deadline).await?.is_some_and(|i| i > 0) {
        std::mem::swap(&mut yy, &mut tm);
    }
    yy = yy.strip_suffix(',').unwrap_or(yy);
    if !yy.chars().next().is_some_and(digit) {
        std::mem::swap(&mut yy, &mut tz);
    }
    tm = tm.strip_suffix(',').unwrap_or(tm);
    let sep = if find(tm, ':', false, deadline).await?.is_some() {
        ':'
    } else {
        '.'
    };
    let (mut pieces, mut cnt, mut start) = ([""; 3], 0usize, 0usize);
    for (i, (at, ch)) in tm.char_indices().enumerate() {
        if i % 1024 == 0 {
            tick(deadline).await?;
        }
        if ch == sep {
            if cnt < 3 {
                pieces[cnt] = &tm[start..at];
            }
            cnt += 1;
            start = at + 1;
        }
    }
    if cnt < 3 {
        pieces[cnt] = &tm[start..];
    }
    cnt += 1;
    if cnt != 2 && cnt != 3 {
        return Ok(None);
    }
    let Some(h) = small(pieces[0], deadline).await? else {
        return Ok(None);
    };
    let Some(min) = small(pieces[1], deadline).await? else {
        return Ok(None);
    };
    let sec = if cnt == 3 {
        let Some(s) = small(pieces[2], deadline).await? else {
            return Ok(None);
        };
        s
    } else {
        0
    };
    if !(0..24).contains(&h) || !(0..60).contains(&min) || !(0..60).contains(&sec) {
        return Ok(None);
    }
    let Some(mut year) = small(yy, deadline).await? else {
        return Ok(None);
    };
    if year < 100 {
        year += if year > 68 { 1900 } else { 2000 };
    }
    let Some(day) = small(dd, deadline).await? else {
        return Ok(None);
    };
    let Some(days) = civil(year, month, day) else {
        return Ok(None);
    };
    let Some(offset) = zone(tz, deadline).await? else {
        return Ok(None);
    };
    let Some(abs) = offset.checked_abs() else {
        return Ok(None);
    };
    let Some(seconds) = (abs / 100)
        .checked_mul(3600)
        .and_then(|s| s.checked_add(abs % 100 * 60))
    else {
        return Ok(None);
    };
    if seconds >= 86400 {
        return Ok(None);
    }
    Ok(Some(
        (days * 86400 + h * 3600 + min * 60 + sec - seconds * offset.signum()) as f64,
    ))
}
pub async fn retry_after(
    raw: Option<&str>,
    now: f64,
    budget: &Budget,
    deadline: Instant,
) -> Result<f64, Error> {
    let Some(raw) = raw.filter(|r| !r.is_empty()) else {
        return Ok(30.0);
    };
    let raw = trim(raw, deadline).await?;
    if let Some(seconds) = string_float_scalar(raw, budget, deadline)
        .await?
        .filter(|s| s.is_finite() && *s >= 0.0)
    {
        return Ok(seconds.clamp(1.0, 300.0));
    }
    let Some(at) = date(raw, deadline).await? else {
        return Ok(30.0);
    };
    let remaining = at - now;
    Ok(if remaining.is_finite() && remaining > 0.0 {
        remaining.clamp(1.0, 300.0)
    } else {
        30.0
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
        time::Duration,
    };
    #[tokio::test]
    async fn exact_pinned_original_retry_after_controls() {
        let data: ssx_json::Value = ssx_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a06/source-retry-after-bounded.json"
        ))
        .unwrap();
        let budget = Budget::new(1024 * 1024).unwrap();
        for row in data["records"].as_array().unwrap() {
            let actual = retry_after(
                row["raw"].as_str(),
                row["now"].as_f64().unwrap(),
                &budget,
                Instant::now() + Duration::from_secs(5),
            )
            .await
            .unwrap();
            assert_eq!(
                format!("{:016x}", actual.to_bits()),
                row["bits"].as_str().unwrap(),
                "{}",
                row["id"].as_str().unwrap()
            );
            assert_eq!(budget.snapshot().unwrap().used, 0);
            crate::adapters::a06_case::completed(
                "ssx-infra/lib/adapters::semantic_retry_after::tests::exact_pinned_original_retry_after_controls",
                "docs/rust-rewrite/a06/source-retry-after-bounded.json",
                row["id"].as_str().unwrap(),
                "default",
                "bounded_helper",
            );
        }
        let table: ssx_json::Value = ssx_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/f11/unicode.json"
        ))
        .unwrap();
        let expected: Vec<(u32, u32)> = table["digit_extra_ranges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| (r[0].as_u64().unwrap() as u32, r[1].as_u64().unwrap() as u32))
            .collect();
        assert_eq!(DIGIT_EXTRA, expected);
    }
    #[tokio::test]
    async fn raw_header_work_is_cooperative_and_scratch_is_root_owned() {
        let raw = format!("1{}", "0".repeat(262144));
        let small = Budget::new(1024).unwrap();
        assert_eq!(
            retry_after(
                Some(&raw),
                0.,
                &small,
                Instant::now() + Duration::from_secs(5)
            )
            .await,
            Err(Error::Resource)
        );
        assert_eq!(small.snapshot().unwrap().used, 0);
        for expire in [false, true] {
            let budget = Budget::new(8 * 1024 * 1024).unwrap();
            let deadline = Instant::now() + Duration::from_secs(if expire { 2 } else { 30 });
            let mut future = Box::pin(retry_after(Some(&raw), 0., &budget, deadline));
            let mut cx = Context::from_waker(Waker::noop());
            let mut seen = 0;
            for _ in 0..100000 {
                assert!(matches!(future.as_mut().poll(&mut cx), Poll::Pending));
                if budget.snapshot().unwrap().used > 0 {
                    seen += 1;
                    if seen == 16 {
                        break;
                    }
                }
            }
            assert_eq!(seen, 16);
            if expire {
                tokio::time::sleep_until(deadline.into()).await;
                assert_eq!(future.await, Err(Error::Deadline));
            } else {
                drop(future);
            }
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
        let raw = format!("14 Nov 2023 22:14:05 {}", "z".repeat(262144));
        let budget = Budget::new(8 * 1024 * 1024).unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut future = Box::pin(date(&raw, deadline));
        let mut cx = Context::from_waker(Waker::noop());
        for _ in 0..256 {
            assert!(matches!(future.as_mut().poll(&mut cx), Poll::Pending));
        }
        drop(future);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
}
