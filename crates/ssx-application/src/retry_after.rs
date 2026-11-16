//! Bounded source Retry-After codec (numeric delay or email/HTTP date).
use crate::{calendar::days_from_civil, rate::bound_provider_cooldown};
/// Python float's Unicode decimal and between-digit underscore input rules.
pub(crate) fn python_float(raw: &str) -> Option<f64> {
    let chars: Vec<char> = raw
        .trim_matches(|c| ssx_json::python_whitespace(c) && !matches!(c, '\u{1c}'..='\u{1f}'))
        .chars()
        .collect();
    let mut out = String::new();
    for (i, ch) in chars.iter().copied().enumerate() {
        if ch == '_' {
            if i == 0
                || i + 1 == chars.len()
                || !ssx_json::decimal_digit(chars[i - 1])
                || !ssx_json::decimal_digit(chars[i + 1])
            {
                return None;
            }
        } else if let Some(value) = ssx_json::decimal_value(ch) {
            out.push(char::from(b'0' + value));
        } else {
            out.push(ch)
        }
    }
    out.parse().ok()
}
pub(crate) fn integer(raw: &str) -> Option<i64> {
    let chars: Vec<char> = raw
        .trim_matches(|c| ssx_json::python_whitespace(c) && !matches!(c, '\u{1c}'..='\u{1f}'))
        .chars()
        .collect();
    let mut out = String::new();
    for (i, ch) in chars.iter().copied().enumerate() {
        if ch == '_' {
            if i == 0
                || i + 1 == chars.len()
                || !ssx_json::decimal_digit(chars[i - 1])
                || !ssx_json::decimal_digit(chars[i + 1])
            {
                return None;
            }
        } else if let Some(v) = ssx_json::decimal_value(ch) {
            out.push(char::from(b'0' + v));
        } else {
            out.push(ch)
        }
    }
    out.parse().ok()
}
pub fn retry_after(value: Option<&str>, now_epoch_seconds: f64) -> f64 {
    let Some(raw) = value.filter(|v| !v.is_empty()) else {
        return 30.0;
    };
    let raw = raw.trim_matches(ssx_json::python_whitespace);
    if let Some(seconds) = python_float(raw).filter(|x| x.is_finite() && *x >= 0.0) {
        return seconds.clamp(1.0, 300.0);
    }
    let Some(at) = email_date(raw) else {
        return 30.0;
    };
    bound_provider_cooldown(at - now_epoch_seconds)
}
/// Source email.utils parsedate tuple semantics, including obsolete dates,
/// naive/unknown zones interpreted as UTC, and its two-digit year pivot.
fn email_date(raw: &str) -> Option<f64> {
    let mut data: Vec<String> = raw
        .split(ssx_json::python_whitespace)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    let first = data.first()?;
    if first.ends_with(',')
        || ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
            .contains(&first.to_ascii_lowercase().as_str())
    {
        data.remove(0);
    } else if let Some(i) = first.rfind(',') {
        data[0] = first[i + 1..].into();
    }
    if data.len() == 3 {
        let parts: Vec<String> = data[0].split('-').map(str::to_owned).collect();
        if parts.len() == 3 {
            data.splice(0..1, parts);
        }
    }
    if data.len() == 4 {
        let cell = data[3].clone();
        let i = cell.find('+').or_else(|| cell.find('-'));
        if let Some(i) = i.filter(|i| *i > 0) {
            data[3] = cell[..i].into();
            data.push(cell[i..].into());
        } else {
            data.push(String::new());
        }
    }
    if data.len() < 5 {
        return None;
    }
    let mut dd = data[0].clone();
    let mut mm = data[1].to_ascii_lowercase();
    let mut yy = data[2].clone();
    let mut tm = data[3].clone();
    let mut tz = data[4].clone();
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
    if !MONTHS.contains(&mm.as_str()) {
        mm = dd.to_ascii_lowercase();
        dd = data[1].to_ascii_lowercase();
    }
    let month = (MONTHS.iter().position(|m| *m == mm)? % 12 + 1) as i64;
    dd = dd.strip_suffix(',').unwrap_or(&dd).into();
    if yy.find(':').is_some_and(|i| i > 0) {
        std::mem::swap(&mut yy, &mut tm);
    }
    yy = yy.strip_suffix(',').unwrap_or(&yy).into();
    if !yy.chars().next().is_some_and(crate::text::digit) {
        std::mem::swap(&mut yy, &mut tz);
    }
    tm = tm.strip_suffix(',').unwrap_or(&tm).into();
    let mut pieces: Vec<&str> = tm.split(':').collect();
    if pieces.len() == 1 && tm.contains('.') {
        pieces = tm.split('.').collect()
    }
    if pieces.len() != 2 && pieces.len() != 3 {
        return None;
    }
    let h = integer(pieces[0])?;
    let min = integer(pieces[1])?;
    let sec = if pieces.len() == 3 {
        integer(pieces[2])?
    } else {
        0
    };
    if !(0..24).contains(&h) || !(0..60).contains(&min) || !(0..60).contains(&sec) {
        return None;
    }
    let mut year = integer(&yy)?;
    if year < 100 {
        year += if year > 68 { 1900 } else { 2000 }
    }
    let days = days_from_civil(year, month, integer(&dd)?)?;
    let offset = match ssx_json::casefold(&tz).to_ascii_uppercase().as_str() {
        "UT" | "UTC" | "GMT" | "Z" => 0,
        "AST" => -400,
        "ADT" | "EST" => -500 + i64::from(tz.eq_ignore_ascii_case("ADT")) * 200,
        "EDT" | "CST" => {
            if tz.eq_ignore_ascii_case("EDT") {
                -400
            } else {
                -600
            }
        }
        "CDT" | "MST" => {
            if tz.eq_ignore_ascii_case("CDT") {
                -500
            } else {
                -700
            }
        }
        "MDT" | "PST" => {
            if tz.eq_ignore_ascii_case("MDT") {
                -600
            } else {
                -800
            }
        }
        "PDT" => -700,
        _ => integer(&tz).unwrap_or(0),
    };
    let absolute = offset.checked_abs()?;
    let seconds = (absolute / 100)
        .checked_mul(3600)?
        .checked_add(absolute % 100 * 60)?;
    if seconds >= 86400 {
        return None;
    }
    Some((days * 86400 + h * 3600 + min * 60 + sec - seconds * offset.signum()) as f64)
}
