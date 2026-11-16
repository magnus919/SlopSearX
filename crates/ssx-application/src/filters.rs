//! One truthful filter report resolver; parameter consumption is never enforcement.
use ssx_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnforcementStatus {
    Enforced,
    PartiallyEnforced,
    Unsupported,
    Rejected,
}
impl EnforcementStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enforced => "enforced",
            Self::PartiallyEnforced => "partially_enforced",
            Self::Unsupported => "unsupported",
            Self::Rejected => "rejected",
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct EnforcementEntry {
    pub requested: Value,
    pub status: EnforcementStatus,
    pub reason: String,
    pub enforced_by: BTreeSet<String>,
}
impl EnforcementEntry {
    pub fn to_value(&self) -> Value {
        json!({"requested": self.requested, "status": self.status.as_str(), "reason": self.reason, "enforced_by": self.enforced_by})
    }
    pub fn rejected(requested: Value, reason: impl Into<String>) -> Self {
        Self {
            requested,
            status: EnforcementStatus::Rejected,
            reason: reason.into(),
            enforced_by: BTreeSet::new(),
        }
    }
}
pub type FilterDeclarations = BTreeMap<String, BTreeMap<String, String>>;
/// Only audited upstream/local declarations are recognized. Local enforcement
/// exists only for the implemented, value-dependent relative-time postfilter.
pub fn resolve_filter_enforcement(
    selected: &[String],
    filter: &str,
    requested: Value,
    declarations: &FilterDeclarations,
) -> EnforcementEntry {
    let mut upstream = Vec::new();
    let mut local = Vec::new();
    for name in selected {
        match declarations
            .get(name)
            .and_then(|d| d.get(filter))
            .map(String::as_str)
        {
            Some("upstream") => upstream.push(name.as_str()),
            Some("local")
                if filter == "time_range"
                    && requested
                        .as_str()
                        .is_none_or(|s| time_range_days(s).is_some()) =>
            {
                local.push(name.as_str())
            }
            _ => {}
        }
    }
    let count = upstream.len() + local.len();
    let status = if count > 0 && count == selected.len() {
        EnforcementStatus::Enforced
    } else if count > 0 {
        EnforcementStatus::PartiallyEnforced
    } else {
        EnforcementStatus::Unsupported
    };
    let reason = match status {
        EnforcementStatus::Enforced if local.is_empty() => {
            format!("{filter} is enforced upstream by every selected adapter")
        }
        EnforcementStatus::Enforced if upstream.is_empty() => {
            format!("{filter} is enforced locally by the service for every selected adapter")
        }
        EnforcementStatus::Enforced => {
            format!("{filter} is enforced across every selected adapter (upstream and local)")
        }
        EnforcementStatus::PartiallyEnforced => {
            let mut parts = Vec::new();
            if !upstream.is_empty() {
                parts.push(format!("upstream by {}", upstream.join(", ")));
            }
            if !local.is_empty() {
                parts.push(format!("locally by {}", local.join(", ")));
            }
            format!(
                "{filter} is enforced only for a subset of selected adapters ({})",
                parts.join("; ")
            )
        }
        _ => format!("no selected adapter enforces the {filter} filter"),
    };
    let enforced_by = upstream
        .into_iter()
        .map(|n| format!("upstream:{n}"))
        .chain(local.into_iter().map(|n| format!("local:{n}")))
        .collect();
    EnforcementEntry {
        requested,
        status,
        reason,
        enforced_by,
    }
}
pub fn time_range_days(value: &str) -> Option<i64> {
    match value {
        "day" => Some(1),
        "week" => Some(7),
        "month" => Some(30),
        "year" => Some(365),
        _ => None,
    }
}
/// Strict publication bounds use exactly canonical YYYY-MM-DD, inclusive.
pub fn publication_day(value: &str) -> Option<i64> {
    let b = value.as_bytes();
    if b.len() != 10
        || b[4] != b'-'
        || b[7] != b'-'
        || !b
            .iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    {
        return None;
    }
    crate::calendar::days_from_civil(
        value[..4].parse().ok()?,
        value[5..7].parse().ok()?,
        value[8..].parse().ok()?,
    )
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateFilterError {
    pub field: &'static str,
    pub message: String,
}
/// Allocation-free diagnostic for callers that own their error destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorrowedDateFilterError {
    pub field: &'static str,
    pub message: &'static str,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicationBounds {
    start: Option<i64>,
    end: Option<i64>,
}
impl PublicationBounds {
    pub fn new(start: Option<&str>, end: Option<&str>) -> Result<Self, DateFilterError> {
        Self::new_borrowed(start, end).map_err(|error| DateFilterError {
            field: error.field,
            message: error.message.to_owned(),
        })
    }
    /// Same canonical inclusive validator, without allocating a diagnostic.
    /// Both inputs use the existing fixed ten-byte publication_day parser;
    /// malformed lower bounds retain precedence over malformed upper bounds.
    pub fn new_borrowed(
        start: Option<&str>,
        end: Option<&str>,
    ) -> Result<Self, BorrowedDateFilterError> {
        let parse = |value: Option<&str>, field, message| {
            value
                .map(|s| publication_day(s).ok_or(BorrowedDateFilterError { field, message }))
                .transpose()
        };
        let start = parse(
            start,
            "date_from",
            "date_from must be a valid YYYY-MM-DD calendar date",
        )?;
        let end = parse(
            end,
            "date_to",
            "date_to must be a valid YYYY-MM-DD calendar date",
        )?;
        if start.zip(end).is_some_and(|(a, b)| a > b) {
            return Err(BorrowedDateFilterError {
                field: "date_from",
                message: "date_from must be on or before date_to",
            });
        }
        Ok(Self { start, end })
    }
    pub fn contains(self, value: Option<&str>) -> bool {
        value
            .and_then(publication_day)
            .is_some_and(|d| self.start.is_none_or(|a| d >= a) && self.end.is_none_or(|b| d <= b))
    }
}
/// Source relative-date postfilter examines the first ten Unicode characters.
/// Compact/basic and ISO-week spellings are additionally accepted by Python's
/// date parser; this primitive is completed by the source replay tests.
pub fn within_time_range(value: Option<&str>, range: &str, today: i64) -> Option<bool> {
    let days = time_range_days(range)?;
    let text: String = value?.chars().take(10).collect();
    let start = today.checked_sub(days)?;
    source_date(&text).map(|day| (start..=today).contains(&day))
}

fn source_date(s: &str) -> Option<i64> {
    if let Some(d) = publication_day(s) {
        return Some(d);
    }
    if !s.is_ascii() {
        return None;
    }
    let b = s.as_bytes();
    if b.len() < 4 || !b[..4].iter().all(u8::is_ascii_digit) {
        return None;
    }
    if b.len() == 8 && b.iter().all(u8::is_ascii_digit) {
        return crate::calendar::days_from_civil(
            s[..4].parse().ok()?,
            s[4..6].parse().ok()?,
            s[6..].parse().ok()?,
        );
    }
    let (year, week, day) =
        if matches!(b.len(), 8 | 10) && b.get(4) == Some(&b'-') && b.get(5) == Some(&b'W') {
            if !b[6..8].iter().all(u8::is_ascii_digit) {
                return None;
            }
            if b.len() == 10 && b[8] != b'-' {
                return None;
            }
            (
                s[..4].parse().ok()?,
                s[6..8].parse::<i64>().ok()?,
                if b.len() == 10 {
                    s[9..].parse::<i64>().ok()?
                } else {
                    1
                },
            )
        } else if matches!(b.len(), 7 | 8) && b.get(4) == Some(&b'W') {
            if !b[5..7].iter().all(u8::is_ascii_digit) {
                return None;
            }
            (
                s[..4].parse().ok()?,
                s[5..7].parse::<i64>().ok()?,
                if b.len() == 8 {
                    s[7..].parse::<i64>().ok()?
                } else {
                    1
                },
            )
        } else {
            return None;
        };
    if !(1..=7).contains(&day) || !(1..=53).contains(&week) {
        return None;
    }
    let jan4 = crate::calendar::days_from_civil(year, 1, 4)?;
    let first = jan4 - (jan4 + 3).rem_euclid(7);
    let dec28 = crate::calendar::days_from_civil(year, 12, 28)?;
    if week > (dec28 - first) / 7 + 1 {
        return None;
    }
    let result = first + (week - 1) * 7 + day - 1;
    if result < crate::calendar::days_from_civil(1, 1, 1)?
        || result > crate::calendar::days_from_civil(9999, 12, 31)?
    {
        return None;
    }
    Some(result)
}

/// Implemented local postfilter: unknown ranges pass through; recognized ranges
/// keep only proven fresh publication dates, never dates inferred from other fields.
pub fn filter_results_by_time_range(
    results: &[ssx_domain::SearchResult],
    range: &str,
    today: i64,
) -> Vec<ssx_domain::SearchResult> {
    if time_range_days(range).is_none() {
        return results.to_vec();
    }
    results
        .iter()
        .filter(|r| within_time_range(r.published_date.as_deref(), range, today) == Some(true))
        .cloned()
        .collect()
}

#[cfg(test)]
mod borrowed_bounds_controls {
    use super::*;
    #[test]
    fn canonical_inclusive_date_bounds_and_diagnostic_precedence() {
        let valid = [
            (None, None),
            (Some("0001-01-01"), None),
            (None, Some("9999-12-31")),
            (Some("2024-02-29"), Some("2024-02-29")),
            (Some("2020-02-29"), Some("2024-03-01")),
        ];
        for (lower, upper) in valid {
            let actual = PublicationBounds::new_borrowed(lower, upper).unwrap();
            assert_eq!(PublicationBounds::new(lower, upper).unwrap(), actual);
            if let Some(lower) = lower {
                assert!(actual.contains(Some(lower)));
            }
            if let Some(upper) = upper {
                assert!(actual.contains(Some(upper)));
            }
            assert!(!actual.contains(None));
            assert!(!actual.contains(Some("2024-02-30")));
            assert!(!actual.contains(Some("20240229")));
        }
        let equal =
            PublicationBounds::new_borrowed(Some("2024-02-29"), Some("2024-02-29")).unwrap();
        assert!(!equal.contains(Some("2024-02-28")));
        assert!(!equal.contains(Some("2024-03-01")));
        for invalid in [
            "",
            "2023-02-29",
            "1900-02-29",
            "0000-01-01",
            "10000-01-01",
            "2024-2-29",
            "2024-02-9",
            "2024-13-01",
            "2024-01-00",
            "2024-04-31",
            "２０２４-02-29",
            "2024-02-29x",
            " 2024-02-29",
        ] {
            for (lower, upper, field) in [
                (Some(invalid), None, "date_from"),
                (None, Some(invalid), "date_to"),
                (Some(invalid), Some(invalid), "date_from"),
            ] {
                let e = PublicationBounds::new_borrowed(lower, upper).unwrap_err();
                assert_eq!(e.field, field);
                assert_eq!(
                    e.message,
                    if field == "date_from" {
                        "date_from must be a valid YYYY-MM-DD calendar date"
                    } else {
                        "date_to must be a valid YYYY-MM-DD calendar date"
                    }
                );
                let old = PublicationBounds::new(lower, upper).unwrap_err();
                assert_eq!(old.field, e.field);
                assert_eq!(old.message, e.message);
            }
        }
        let e =
            PublicationBounds::new_borrowed(Some("2024-03-01"), Some("2024-02-29")).unwrap_err();
        assert_eq!(
            e,
            BorrowedDateFilterError {
                field: "date_from",
                message: "date_from must be on or before date_to"
            }
        );
        let old = PublicationBounds::new(Some("2024-03-01"), Some("2024-02-29")).unwrap_err();
        assert_eq!(old.field, e.field);
        assert_eq!(old.message, e.message);
        let huge = "x".repeat(1024 * 1024);
        let e = PublicationBounds::new_borrowed(Some(&huge), Some("bad")).unwrap_err();
        assert_eq!(e.field, "date_from");
    }
}
