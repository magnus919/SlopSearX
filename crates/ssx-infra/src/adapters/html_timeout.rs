//! Engine-local raw scalar projection. This intentionally does not use F06's
//! numeric-text coercion: Python evaluates timeout_ms / 1000.0 directly.
use ssx_application::SearchContext;
use ssx_config::ConfigScalar;
use std::time::Duration;
use tokio::time::Instant;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum TimerError {
    Text,
    Invalid,
    FiniteRequired,
}
impl TimerError {
    pub(super) fn message(&self) -> &'static str {
        match self {
            Self::Text => "unsupported operand type(s) for /: 'str' and 'float'",
            Self::Invalid => "HTML invalid execution timer",
            Self::FiniteRequired => "HTML finite execution deadline required",
        }
    }
}

pub(super) fn deadline(
    value: &ConfigScalar,
    context: Option<SearchContext>,
) -> Result<Instant, TimerError> {
    let milliseconds = match value {
        ConfigScalar::Boolean(value) => f64::from(u8::from(*value)),
        ConfigScalar::Integer(value) => *value as f64,
        ConfigScalar::Float(value) => *value,
        ConfigScalar::Text(_) => return Err(TimerError::Text),
    };
    let seconds = milliseconds / 1000.0;
    if seconds.is_nan() {
        return Err(TimerError::Invalid);
    }
    let now = Instant::now();
    let inherited = context.map(|value| Instant::from_std(value.deadline));
    if seconds <= 0.0 {
        return Ok(inherited.map_or(now, |value| value.min(now)));
    }
    let configured = Duration::try_from_secs_f64(seconds)
        .ok()
        .and_then(|duration| now.checked_add(duration));
    match (configured, inherited) {
        (Some(configured), Some(inherited)) => Ok(configured.min(inherited)),
        (Some(configured), None) => Ok(configured),
        (None, Some(inherited)) => Ok(inherited),
        (None, None) => Err(TimerError::FiniteRequired),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(start_paused = true)]
    async fn scalar_arithmetic_and_absolute_context_are_preserved_without_coercion() {
        let now = Instant::now();
        for (value, seconds) in [
            (ConfigScalar::Integer(1000), 1.0),
            (ConfigScalar::Float(12.5), 0.0125),
            (ConfigScalar::Boolean(true), 0.001),
            (ConfigScalar::Boolean(false), 0.0),
            (ConfigScalar::Integer(-1), 0.0),
            (ConfigScalar::Float(f64::NEG_INFINITY), 0.0),
        ] {
            assert_eq!(
                deadline(&value, None).unwrap(),
                now + Duration::from_secs_f64(seconds)
            );
        }
        assert_eq!(
            deadline(&ConfigScalar::Text("1000".into()), None),
            Err(TimerError::Text)
        );
        assert_eq!(
            deadline(&ConfigScalar::Float(f64::NAN), None),
            Err(TimerError::Invalid)
        );
        let limit = now + Duration::from_millis(40);
        let context = SearchContext {
            deadline: limit.into_std(),
        };
        for value in [f64::INFINITY, 1e300, 1000.0] {
            assert_eq!(
                deadline(&ConfigScalar::Float(value), Some(context)).unwrap(),
                limit
            );
        }
        for value in [f64::INFINITY, 1e300] {
            assert_eq!(
                deadline(&ConfigScalar::Float(value), None),
                Err(TimerError::FiniteRequired)
            );
        }
        tokio::time::advance(Duration::from_millis(50)).await;
        assert_eq!(
            deadline(&ConfigScalar::Integer(1000), Some(context)).unwrap(),
            limit
        );
    }
}
