//! Gregorian civil arithmetic for source UTC timestamp codecs.
use ssx_domain::DomainError;
pub(crate) fn days_from_civil(year: i64, month: i64, day: i64) -> Option<i64> {
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
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    y += i64::from(m <= 2);
    (y, m, d)
}
pub fn iso_timestamp(seconds: f64) -> Result<String, DomainError> {
    if !seconds.is_finite() || !(-62_135_596_800.0..253_402_300_800.0).contains(&seconds) {
        return Err(DomainError(
            "health timestamp outside source datetime range",
        ));
    }
    let mut whole = seconds.floor() as i64;
    let mut micros = ((seconds - whole as f64) * 1_000_000.0).round_ties_even() as i64;
    if micros == 1_000_000 {
        whole += 1;
        micros = 0;
    }
    if !(-62_135_596_800..253_402_300_800).contains(&whole) {
        return Err(DomainError(
            "rounded health timestamp outside source datetime range",
        ));
    }
    let (y, m, d) = civil_from_days(whole.div_euclid(86400));
    if !(1..=9999).contains(&y) {
        return Err(DomainError(
            "health timestamp year outside source datetime range",
        ));
    }
    let rem = whole.rem_euclid(86400);
    let base = format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    );
    Ok(if micros == 0 {
        format!("{base}+00:00")
    } else {
        format!("{base}.{micros:06}+00:00")
    })
}
