//! Source formatter derivation. No caller-precomputed publication timestamps.
use ssx_domain::SearchResult;
use ssx_wire::HttpResultMetadata;

pub fn metadata(result: &SearchResult) -> HttpResultMetadata {
    HttpResultMetadata {
        pubdate: iso_to_epoch(result.published_date.as_deref()),
        parsed_url: parsed_url(&result.url),
    }
}

pub fn parsed_url(input: &str) -> Option<Vec<String>> {
    if input.is_empty() {
        return None;
    }
    let parts = ssx_application::parsed_url(input)?;
    if parts[0].is_empty() || parts[1].is_empty() {
        return None;
    }
    Some(parts)
}

fn decimal(input: &str) -> Option<i64> {
    if input.is_empty() {
        return None;
    }
    input.chars().try_fold(0i64, |n, c| {
        n.checked_mul(10)?
            .checked_add(i64::from(ssx_json::decimal_value(c)?))
    })
}
fn component(input: &str, kind: u8) -> Option<i64> {
    let chars: Vec<char> = input.chars().collect();
    let digit = |c| ssx_json::decimal_value(c).is_some();
    let valid = match (kind, chars.as_slice()) {
        (b'm', [c]) => matches!(c, '1'..='9'),
        (b'm', [a, b]) => {
            (*a == '0' && matches!(b, '1'..='9')) || (*a == '1' && matches!(b, '0'..='2'))
        }
        (b'd', [' ', b]) => matches!(b, '1'..='9'),
        (b'd', [a, b]) => {
            (matches!(a, '1'..='2') && digit(*b))
                || (*a == '0' && matches!(b, '1'..='9'))
                || (*a == '3' && matches!(b, '0'..='1'))
        }
        (b'd', [c]) => matches!(c, '1'..='9'),
        (b'h', [' ', b]) => digit(*b),
        (b'h', [a, b]) => {
            (matches!(a, '0'..='1') && digit(*b)) || (*a == '2' && matches!(b, '0'..='3'))
        }
        (b'M' | b's', [a, b]) => matches!(a, '0'..='5') && digit(*b),
        (_, [c]) => digit(*c),
        _ => false,
    };
    valid
        .then(|| decimal(input.trim_start_matches(' ')))
        .flatten()
}
fn days(year: i64, month: i64, day: i64) -> Option<i64> {
    if !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
        return None;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let maximum = match month {
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
    if !(1..=maximum).contains(&day) {
        return None;
    }
    let y = year - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = month + if month > 2 { -3 } else { 9 };
    Some(era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + (153 * mp + 2) / 5 + day - 1 - 719468)
}

/// Python 3.14 strptime's three original grammars, including its mixed Unicode
/// decimal and ASCII range branches. Datetime rejects leap seconds after parse.
pub fn iso_to_epoch(input: Option<&str>) -> Option<i64> {
    let input = input?;
    let (date, time) = match input.find(['T', 't']) {
        Some(index) => (&input[..index], Some(&input[index + 1..])),
        None => (input, None),
    };
    let mut pieces = date.split('-');
    let year = pieces.next()?;
    if year.chars().count() != 4 {
        return None;
    }
    let year = decimal(year)?;
    let month = component(pieces.next()?, b'm')?;
    let day = component(pieces.next()?, b'd')?;
    if pieces.next().is_some() {
        return None;
    }
    let base = days(year, month, day)? * 86400;
    let Some(time) = time else {
        return Some(base);
    };
    let (clock, offset) = if let Some(clock) = time.strip_suffix(['Z', 'z']) {
        (clock, 0i64)
    } else {
        let index = time.find(['+', '-'])?;
        let (clock, zone) = time.split_at(index);
        let sign = if zone.starts_with('-') { -1 } else { 1 };
        let zone = &zone[1..];
        // strptime rejects inconsistent colon placement.
        let colon = zone.chars().nth(2) == Some(':');
        let normalized = if colon {
            let chars: Vec<char> = zone.chars().collect();
            if chars.len() < 5
                || (chars.len() > 5 && (chars.len() < 8 || chars.get(5) != Some(&':')))
                || chars
                    .iter()
                    .enumerate()
                    .any(|(index, ch)| *ch == ':' && index != 2 && index != 5)
            {
                return None;
            }
            zone.replace(':', "")
        } else {
            if zone.contains(':') {
                return None;
            }
            zone.to_owned()
        };
        let (whole, fraction) = normalized
            .split_once('.')
            .map_or((normalized.as_str(), None), |(w, f)| (w, Some(f)));
        let digits: Vec<char> = whole.chars().collect();
        if digits.len() != 4 && digits.len() != 6 {
            return None;
        }
        if !matches!(digits[2], '0'..='5') || (digits.len() == 6 && !matches!(digits[4], '0'..='5'))
        {
            return None;
        }
        let hours = decimal(&digits[0..2].iter().collect::<String>())?;
        let minutes = decimal(&digits[2..4].iter().collect::<String>())?;
        let seconds = if digits.len() == 6 {
            decimal(&digits[4..6].iter().collect::<String>())?
        } else {
            0
        };
        let micros = if let Some(fraction) = fraction {
            // Python %z fractional seconds use Unicode decimal digits.
            if digits.len() != 6
                || fraction.is_empty()
                || fraction.chars().count() > 6
                || !fraction
                    .chars()
                    .all(|c| ssx_json::decimal_value(c).is_some())
            {
                return None;
            }
            decimal(fraction)? * 10i64.pow(6 - fraction.chars().count() as u32)
        } else {
            0
        };
        let offset = (hours * 3600 + minutes * 60 + seconds) * 1_000_000 + micros;
        if offset >= 86400 * 1_000_000 {
            return None;
        }
        (clock, sign * offset)
    };
    let mut parts = clock.split(':');
    let hour = component(parts.next()?, b'h')?;
    let minute = component(parts.next()?, b'M')?;
    let second = component(parts.next()?, b's')?;
    if parts.next().is_some() || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    // Python timestamp rounds its rational microseconds to a float, then int
    // truncates toward zero, including fractional
    // offsets around the Unix epoch. All representable microseconds fit i64.
    let micros = (base + hour * 3600 + minute * 60 + second) * 1_000_000 - offset;
    let magnitude = micros.unsigned_abs();
    let decimal = format!(
        "{}{}.{:06}",
        if micros < 0 { "-" } else { "" },
        magnitude / 1_000_000,
        magnitude % 1_000_000
    );
    Some(decimal.parse::<f64>().ok()? as i64)
}
