//! Source wall-time conversion and exact platform local-year primitive.
//! No worker task, timezone parser/cache, UTC fallback or normalized URL.
use std::time::{SystemTime, UNIX_EPOCH};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SourceCookieClockError;
impl std::fmt::Display for SourceCookieClockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("source cookie wall/local-time observation failed")
    }
}
impl std::error::Error for SourceCookieClockError {}
pub(super) fn seconds_double() -> Result<f64, SourceCookieClockError> {
    let ns = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(value) => i128::try_from(value.as_nanos()).map_err(|_| SourceCookieClockError)?,
        Err(error) => {
            -i128::try_from(error.duration().as_nanos()).map_err(|_| SourceCookieClockError)?
        }
    };
    let ns = i64::try_from(ns).map_err(|_| SourceCookieClockError)?;
    Ok(seconds_double_from_ns(ns))
}
fn seconds_double_from_ns(ns: i64) -> f64 {
    // PyTime_AsSecondsDouble divides integral seconds before conversion, and
    // otherwise converts the signed nanoseconds to double before division.
    if ns % 1_000_000_000 == 0 {
        (ns / 1_000_000_000) as f64
    } else {
        ns as f64 / 1e9
    }
}
pub(super) fn local_year() -> Result<i32, SourceCookieClockError> {
    local_year_at(seconds_double()?.floor() as i64)
}
pub(super) fn local_year_at(seconds: i64) -> Result<i32, SourceCookieClockError> {
    let native_seconds = seconds as libc::time_t;
    if i128::from(native_seconds) != i128::from(seconds) {
        return Err(SourceCookieClockError);
    }
    let seconds = native_seconds;
    let mut result = std::mem::MaybeUninit::<libc::tm>::uninit();
    #[cfg(unix)]
    {
        // SAFETY: libc provides the platform time_t/tm layout. Input lives for
        // the call; writable output has that layout and is assumed initialized
        // only when localtime_r returns its non-null result pointer. No pointer
        // or tm_zone member escapes, and libc owns TZ/zonefile semantics.
        if unsafe { libc::localtime_r(&seconds, result.as_mut_ptr()) }.is_null() {
            return Err(SourceCookieClockError);
        }
    }
    #[cfg(windows)]
    {
        // SAFETY: matching platform CRT ABI/layout from libc. localtime_s writes
        // tm only on status0; the error status never triggers assume_init.
        if unsafe { libc::localtime_s(result.as_mut_ptr(), &seconds) } != 0 {
            return Err(SourceCookieClockError);
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        return Err(SourceCookieClockError);
    }
    #[cfg(any(unix, windows))]
    {
        // SAFETY: initialized only after the checked platform call above.
        let result = unsafe { result.assume_init() };
        result
            .tm_year
            .checked_add(1900)
            .ok_or(SourceCookieClockError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_wall_seconds_keep_integral_and_negative_nanosecond_branches() {
        assert_eq!(seconds_double_from_ns(1_000_000_000), 1.0);
        assert_eq!(seconds_double_from_ns(-1_000_000_000), -1.0);
        assert_eq!(seconds_double_from_ns(-1), -1.0 / 1e9);
        assert_eq!(seconds_double_from_ns(i64::MAX), i64::MAX as f64 / 1e9);
        assert_eq!(seconds_double_from_ns(i64::MIN), i64::MIN as f64 / 1e9);
    }
    #[test]
    fn source_local_year_uses_checked_platform_result_without_utc_fallback() {
        // Mid-year avoids assuming the caller's timezone at the year boundary.
        // Exact TZ/New-Year witnesses are separate process-isolated controls.
        assert_eq!(local_year_at(1_751_328_000).unwrap(), 2025);
        assert!(local_year_at(i64::MAX).is_err());
    }
}
