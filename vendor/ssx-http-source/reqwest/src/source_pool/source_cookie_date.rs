//! CPython3.14 cookiejar.http2time language; no backend interpreter.
use chrono::Datelike;
use regex::Regex;
use std::sync::LazyLock;
static STRICT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\A[SMTWF][a-z][a-z], ([0-9]{2}) ([JFMASOND][a-z][a-z]) ([0-9]{4}) ([0-9]{2}):([0-9]{2}):([0-9]{2}) GMT(?:\n)?\z").expect("literal source grammar")
});
static WEEKDAY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\A(?i-u:(?:Sun|Mon|Tue|Wed|Thu|Fri|Sat)[a-z]*,?)[\x09-\x0d\x20]*")
        .expect("literal source grammar")
});
static LOOSE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\A([0-9]{1,2})(?:[\x09-\x0d\x20]+|[-/])([A-Za-z0-9_]+)(?:[\x09-\x0d\x20]+|[-/])([0-9]+)(?:(?:[\x09-\x0d\x20]+|:)([0-9]{1,2}):([0-9]{2})(?::([0-9]{2}))?)?[\x09-\x0d\x20]*(?:([-+]?[0-9]{2,4}|[A-Za-z]+)[\x09-\x0d\x20]*)?(?:\([A-Za-z0-9_]+\)[\x09-\x0d\x20]*)?\z").expect("literal source grammar")
});
static ZONE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\A([-+])?([0-9]{1,2}):?([0-9]{2})?\z").expect("literal source grammar")
});
const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];
/// A strict syntactic match with unknown month raises in source and aborts the
/// entire Netscape response-header parse batch, unlike an invalid date's None.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SourceCookieDateError;
#[cfg(test)]
fn http2time(text: &str, local_year: i32) -> Result<Option<i64>, SourceCookieDateError> {
    http2time_with_year(text, || Ok(local_year))
}
pub(super) fn http2time_with_year(
    text: &str,
    mut local_year: impl FnMut() -> Result<i32, SourceCookieDateError>,
) -> Result<Option<i64>, SourceCookieDateError> {
    if let Some(c) = STRICT.captures(text) {
        let month = MONTHS
            .iter()
            .position(|m| *m == c[2].to_ascii_lowercase())
            .ok_or(SourceCookieDateError)? as u32
            + 1;
        return Ok(timegm(
            c[3].parse().ok(),
            Some(month),
            c[1].parse().ok(),
            c[4].parse().ok(),
            c[5].parse().ok(),
            c[6].parse().ok(),
        ));
    }
    let text = text.trim_start_matches(super::source_cookies::python_whitespace);
    let text = WEEKDAY.replace(text, "");
    let Some(c) = LOOSE.captures(&text) else {
        return Ok(None);
    };
    if c[3].len() > 4300 {
        return Err(SourceCookieDateError);
    }
    let Some(mut year) = c[3].parse::<i32>().ok().filter(|y| *y <= 9999) else {
        return Ok(None);
    };
    let month = MONTHS
        .iter()
        .position(|m| *m == c[2].to_ascii_lowercase())
        .map(|n| n as u32 + 1)
        .or_else(|| {
            // LOOSE uses ASCII \w+, but CPython int(mon) also accepts decimal
            // underscores. Invalid numeric/4300-digit inputs return None here.
            let text = &c[2];
            super::source_cookies::python_integer(text)
                .then(|| text.chars().filter(|c| *c != '_').collect::<String>())
                .and_then(|text| text.parse().ok())
        });
    let Some(month) = month.filter(|m| (1..=12).contains(m)) else {
        return Ok(None);
    };
    if year < 1000 {
        let local_year = local_year()?;
        let suffix = local_year % 100;
        let distance = suffix - year;
        year += local_year - suffix;
        if distance.abs() > 50 {
            year += if distance > 0 { 100 } else { -100 };
        }
    }
    if year > 9999 {
        return Err(SourceCookieDateError);
    }
    let value = timegm(
        Some(year),
        Some(month),
        c[1].parse().ok(),
        c.get(4).map_or(Some(0), |v| v.as_str().parse().ok()),
        c.get(5).map_or(Some(0), |v| v.as_str().parse().ok()),
        c.get(6).map_or(Some(0), |v| v.as_str().parse().ok()),
    );
    let Some(value) = value else {
        return Ok(None);
    };
    let zone = c.get(7).map_or("UTC", |v| v.as_str()).to_ascii_uppercase();
    let offset = if ["GMT", "UTC", "UT", "Z"].contains(&zone.as_str()) {
        Some(0)
    } else {
        ZONE.captures(&zone).and_then(|z| {
            let hour = z[2].parse::<i64>().ok()?;
            let minute = z
                .get(3)
                .map_or(Some(0), |m| m.as_str().parse::<i64>().ok())?;
            Some(
                (3600 * hour + 60 * minute)
                    * if z.get(1).is_some_and(|s| s.as_str() == "-") {
                        -1
                    } else {
                        1
                    },
            )
        })
    };
    Ok(offset.and_then(|offset| value.checked_sub(offset)))
}
fn timegm(
    year: Option<i32>,
    month: Option<u32>,
    day: Option<u32>,
    hour: Option<u32>,
    minute: Option<u32>,
    second: Option<u32>,
) -> Option<i64> {
    let (year, month, day, hour, minute, second) = (year?, month?, day?, hour?, minute?, second?);
    if year < 1970
        || !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 24
        || minute > 59
        || second > 61
    {
        return None;
    }
    // calendar.timegm validates day1, then adds mday-1 and clock counts;
    // February31, hour24 and leap seconds therefore normalize arithmetically.
    let first = chrono::NaiveDate::from_ymd_opt(year, month, 1)?;
    let days = i64::from(first.num_days_from_ce()) - 719163 + i64::from(day) - 1;
    Some(days * 86400 + i64::from(hour) * 3600 + i64::from(minute) * 60 + i64::from(second))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_cookie_date_language_window_and_calendar_arithmetic() {
        assert_eq!(
            http2time("Thu, 01 Jan 1970 00:00:00 GMT", 2026),
            Ok(Some(0))
        );
        assert_eq!(http2time("01-Jan-70", 2026), Ok(Some(3155760000)));
        assert_eq!(http2time("01-Jan-70", 2020), Ok(Some(3155760000)));
        assert_eq!(
            http2time("31-Feb-2024 24:00:61 UTC", 2026),
            http2time("03-Mar-2024 00:01:01 UTC", 2026)
        );
        assert_eq!(http2time("01 Jan 2024 00:00 EST", 2026), Ok(None));
        assert_eq!(http2time("01 Jan 2024 00:00 AM", 2026), Ok(None));
        assert_eq!(http2time("01 Jan 1969 00:00 GMT", 2026), Ok(None));
        assert_eq!(
            http2time("Mon, 01 Jxx 2024 00:00:00 GMT", 2026),
            Err(SourceCookieDateError)
        );
    }
    #[test]
    fn loose_numeric_month_preserves_python_decimal_underscores() {
        assert_eq!(
            http2time("01-0_1-2030", 2026),
            http2time("01-Jan-2030", 2026)
        );
        assert!(http2time("01-0_1-2030", 2026).unwrap().is_some());
        assert_eq!(http2time("01-0__1-2030", 2026), Ok(None));
        assert_eq!(http2time("01-_1-2030", 2026), Ok(None));
        assert_eq!(http2time("01-1_-2030", 2026), Ok(None));
        assert_eq!(http2time("01-1_3-2030", 2026), Ok(None));
    }
}
