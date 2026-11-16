//! Fallible/cooperative Python repr over the accepted source value tree.
//! Input DOM and cold immutable Unicode tables are separately owned; all event,
//! destination and finite-float temporary backing is charged to the caller root.
use super::source_json::{Number, Text, Value};
use ssx_owned_budget::{Budget, OwnedString, OwnedVec};
use std::time::Instant;
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Resource,
    Deadline,
    Unicode,
}
fn resource(_: ssx_owned_budget::Error) -> Error {
    Error::Resource
}
async fn checkpoint(deadline: Option<Instant>) -> Result<(), Error> {
    if deadline.is_some_and(|d| Instant::now() >= d) {
        return Err(Error::Deadline);
    }
    tokio::task::yield_now().await;
    if deadline.is_some_and(|d| Instant::now() >= d) {
        return Err(Error::Deadline);
    }
    Ok(())
}
fn append(out: &mut OwnedString, text: &str) -> Result<(), Error> {
    let wanted = out.len().checked_add(text.len()).ok_or(Error::Resource)?;
    if wanted > out.capacity() {
        let capacity = out
            .capacity()
            .checked_mul(2)
            .and_then(|n| n.checked_add(8))
            .ok_or(Error::Resource)?
            .max(wanted);
        out.try_reserve_exact(capacity - out.len())
            .map_err(resource)?;
    }
    out.try_push_str(text).map_err(resource)
}
async fn scalar(out: &mut OwnedString, text: &str, deadline: Option<Instant>) -> Result<(), Error> {
    let mut remaining = text;
    while !remaining.is_empty() {
        checkpoint(deadline).await?;
        let end = remaining
            .char_indices()
            .nth(1024)
            .map_or(remaining.len(), |(i, _)| i);
        append(out, &remaining[..end])?;
        remaining = &remaining[end..];
    }
    Ok(())
}
async fn text(out: &mut OwnedString, text: &Text, deadline: Option<Instant>) -> Result<(), Error> {
    let (mut single, mut double) = (false, false);
    for (i, p) in text.codepoints().iter().enumerate() {
        if i % 1024 == 0 {
            checkpoint(deadline).await?;
        }
        single |= *p == 39;
        double |= *p == 34;
    }
    let quote = if single && !double { '"' } else { '\'' };
    append(out, if quote == '"' { "\"" } else { "'" })?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (i, p) in text.codepoints().iter().enumerate() {
        if i % 1024 == 0 {
            checkpoint(deadline).await?;
        }
        if (0xd800..=0xdfff).contains(p) {
            let mut escaped = [b'\\', b'u', 0, 0, 0, 0];
            for digit in 0..4 {
                escaped[digit + 2] = HEX[((p >> ((3 - digit) * 4)) & 15) as usize];
            }
            append(out, std::str::from_utf8(&escaped).expect("ASCII hex"))?;
        } else {
            let ch = char::from_u32(*p).ok_or(Error::Unicode)?;
            let token = ssx_domain::storage::python_repr_character(
                ch,
                if quote == '"' {
                    ssx_domain::storage::PythonReprQuote::Double
                } else {
                    ssx_domain::storage::PythonReprQuote::Single
                },
            );
            append(out, token.as_str())?;
        }
    }
    append(out, if quote == '"' { "\"" } else { "'" })
}
enum Event<'a> {
    Value(&'a Value),
    Text(&'a Text),
    Literal(&'static str),
}
/// Python repr, including quoted top-level strings. Adapter str(value) callers
/// must keep their existing raw top-string source handling and call this for
/// nonstrings; this function never sanitizes surrogates or finite-normalizes data.
pub async fn repr_budgeted(
    value: &Value,
    budget: &Budget,
    deadline: Option<Instant>,
) -> Result<OwnedString, Error> {
    let mut events = OwnedVec::new(budget).map_err(resource)?;
    events.try_push(Event::Value(value)).map_err(resource)?;
    let mut out = OwnedString::new(budget).map_err(resource)?;
    while let Some(event) = events.pop() {
        checkpoint(deadline).await?;
        match event {
            Event::Literal(value) => append(&mut out, value)?,
            Event::Text(value) => text(&mut out, value, deadline).await?,
            Event::Value(value) => match value {
                Value::Null => append(&mut out, "None")?,
                Value::Bool(v) => append(&mut out, if *v { "True" } else { "False" })?,
                Value::String(v) => text(&mut out, v, deadline).await?,
                Value::Number(number @ Number::Integer(_)) => {
                    scalar(
                        &mut out,
                        number.integer_text().ok_or(Error::Unicode)?,
                        deadline,
                    )
                    .await?
                }
                Value::Number(Number::Float(v)) => {
                    if !v.is_finite() {
                        append(
                            &mut out,
                            if v.is_nan() {
                                "nan"
                            } else if *v > 0. {
                                "inf"
                            } else {
                                "-inf"
                            },
                        )?;
                    } else {
                        let charge = budget.reserve(25).map_err(resource)?;
                        let formatted =
                            ssx_json::Number::from_f64(*v).map_err(|_| Error::Resource)?;
                        append(&mut out, formatted.as_str())?;
                        drop(formatted);
                        drop(charge);
                    }
                }
                Value::Array(values) => {
                    append(&mut out, "[")?;
                    events.try_push(Event::Literal("]")).map_err(resource)?;
                    for (i, value) in values.iter().enumerate().rev() {
                        if i % 1024 == 0 {
                            checkpoint(deadline).await?;
                        }
                        events.try_push(Event::Value(value)).map_err(resource)?;
                        if i > 0 {
                            events.try_push(Event::Literal(", ")).map_err(resource)?;
                        }
                    }
                }
                Value::Object(values) => {
                    append(&mut out, "{")?;
                    events.try_push(Event::Literal("}")).map_err(resource)?;
                    let mut entries =
                        OwnedVec::with_capacity(budget, values.len()).map_err(resource)?;
                    for (i, entry) in values.iter().enumerate() {
                        if i % 1024 == 0 {
                            checkpoint(deadline).await?;
                        }
                        entries.try_push(entry).map_err(resource)?;
                    }
                    for (i, (key, value)) in entries.as_slice().iter().enumerate().rev() {
                        if i % 1024 == 0 {
                            checkpoint(deadline).await?;
                        }
                        events.try_push(Event::Value(value)).map_err(resource)?;
                        events.try_push(Event::Literal(": ")).map_err(resource)?;
                        events.try_push(Event::Text(key)).map_err(resource)?;
                        if i > 0 {
                            events.try_push(Event::Literal(", ")).map_err(resource)?;
                        }
                    }
                }
            },
        }
    }
    checkpoint(deadline).await?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::{Future, poll_fn},
        pin::Pin,
        task::Poll,
        time::Duration,
    };
    fn tagged(value: &ssx_json::Value) -> Pin<Box<dyn Future<Output = Value> + '_>> {
        Box::pin(async move {
            match value["type"].as_str().unwrap() {
                "null" => Value::Null,
                "bool" => Value::Bool(value["value"].as_bool().unwrap()),
                "int" | "float" => super::super::source_json::from_slice(
                    match value["value"].as_str().unwrap() {
                        "nan" => b"NaN",
                        "inf" => b"Infinity",
                        "-inf" => b"-Infinity",
                        other => other.as_bytes(),
                    },
                    None,
                )
                .await
                .unwrap(),
                "str" => Value::String(Text::from_points(
                    value["codepoints"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_u64().unwrap() as u32)
                        .collect(),
                )),
                "nested_list" => {
                    let mut out = tagged(&value["leaf"]).await;
                    for _ in 0..value["depth"].as_u64().unwrap() {
                        out = Value::Array(vec![out]);
                    }
                    out
                }
                "list" => {
                    let mut out = Vec::new();
                    for item in value["items"].as_array().unwrap() {
                        out.push(tagged(item).await);
                    }
                    Value::Array(out)
                }
                "dict" => {
                    let mut out = super::super::source_json::Map::new();
                    for pair in value["items"].as_array().unwrap() {
                        let Value::String(key) = tagged(&pair[0]).await else {
                            panic!("key")
                        };
                        out.insert(key, tagged(&pair[1]).await);
                    }
                    Value::Object(out)
                }
                _ => panic!("unknown tag"),
            }
        })
    }
    #[tokio::test]
    async fn actual_python_35_controls_preserve_quotes_floats_bigints_raw_codepoints_and_depth() {
        let fixture: ssx_json::Value = ssx_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a07/source-repr-controls.json"
        ))
        .unwrap();
        let mut seen = std::collections::BTreeSet::new();
        for case in fixture["cases"].as_array().unwrap() {
            assert!(seen.insert(case["id"].as_str().unwrap()));
            let input = tagged(&case["input"]).await;
            let budget = Budget::new(32 * 1024 * 1024).unwrap();
            let out = repr_budgeted(&input, &budget, None).await.unwrap();
            assert_eq!(
                out.as_str(),
                case["repr"].as_str().unwrap(),
                "{}",
                case["id"]
            );
            assert_eq!(budget.snapshot().unwrap().used, out.charged_bytes());
            drop(out);
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
        assert_eq!(seen.len(), 35);
    }
    #[tokio::test]
    async fn quota_failure_is_fallible_terminal_and_releases_stack_and_output() {
        let value = Value::Array(vec![Value::String(Text::from(
            "\u{200b}".repeat(16384).as_str(),
        ))]);
        let budget = Budget::new(1024).unwrap();
        assert!(matches!(
            repr_budgeted(&value, &budget, None).await,
            Err(Error::Resource)
        ));
        let snapshot = budget.snapshot().unwrap();
        assert_eq!(snapshot.used, 0);
        assert!(snapshot.peak <= snapshot.limit);
    }
    async fn until_live<F: Future>(mut future: Pin<&mut F>, budget: &Budget) {
        poll_fn(|cx| match future.as_mut().poll(cx) {
            Poll::Ready(_) => panic!("completed before live repr observation"),
            Poll::Pending => {
                if budget.snapshot().unwrap().used > 65536 {
                    Poll::Ready(())
                } else {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            }
        })
        .await;
    }
    #[tokio::test]
    async fn cancellation_after_real_output_growth_drops_every_charge() {
        let value = Value::Array(vec![Value::String(Text::from(
            "\u{200b}".repeat(262144).as_str(),
        ))]);
        let budget = Budget::new(32 * 1024 * 1024).unwrap();
        let mut future = Box::pin(repr_budgeted(&value, &budget, None));
        until_live(future.as_mut(), &budget).await;
        let live = budget.snapshot().unwrap().used;
        let start = Instant::now();
        drop(future);
        let drop_us = start.elapsed().as_micros();
        assert_eq!(budget.snapshot().unwrap().used, 0);
        eprintln!("A07_REPR_CANCEL live_bytes={live} drop_us={drop_us}");
    }
    #[tokio::test]
    async fn absolute_deadline_interrupts_large_repr_after_output_started() {
        let value = Value::Array(vec![Value::String(Text::from(
            "\u{200b}".repeat(262144).as_str(),
        ))]);
        let budget = Budget::new(32 * 1024 * 1024).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut future = Box::pin(repr_budgeted(&value, &budget, Some(deadline)));
        until_live(future.as_mut(), &budget).await;
        std::thread::sleep(
            deadline.saturating_duration_since(Instant::now()) + Duration::from_millis(1),
        );
        let start = Instant::now();
        assert!(matches!(future.await, Err(Error::Deadline)));
        eprintln!(
            "A07_REPR_DEADLINE return_us={}",
            start.elapsed().as_micros()
        );
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[tokio::test]
    async fn large_repr_uses_real_storage_envelope_instead_of_arbitrary_point_multiplier() {
        for points in [30000usize, 100000, 300000] {
            let value = Value::Array(vec![Value::String(Text::from(
                "\u{200b}".repeat(points).as_str(),
            ))]);
            let budget = Budget::new(32 * 1024 * 1024).unwrap();
            let start = Instant::now();
            let out = repr_budgeted(&value, &budget, None).await.unwrap();
            assert_eq!(out.len(), points * 6 + 4);
            let snapshot = budget.snapshot().unwrap();
            assert_eq!(snapshot.used, out.charged_bytes());
            assert!(snapshot.peak <= snapshot.limit);
            eprintln!(
                "A07_REPR_COOPERATIVE points={points} elapsed_us={} output={} capacity={} peak={}",
                start.elapsed().as_micros(),
                out.len(),
                out.capacity(),
                snapshot.peak
            );
            drop(out);
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
    }
    fn flat(object: bool) -> Value {
        if !object {
            return Value::Array((0..16384).map(|_| Value::Null).collect());
        }
        let mut map = super::super::source_json::Map::new();
        for i in 0..4096 {
            map.insert(Text::from(format!("k{i:05}").as_str()), Value::Null);
        }
        Value::Object(map)
    }
    #[tokio::test]
    async fn flat_array_and_object_event_backing_peaks_are_charged() {
        for object in [false, true] {
            let value = flat(object);
            let budget = Budget::new(32 * 1024 * 1024).unwrap();
            let out = repr_budgeted(&value, &budget, None).await.unwrap();
            assert_eq!(out.as_str(), value.repr());
            let snapshot = budget.snapshot().unwrap();
            assert_eq!(snapshot.used, out.charged_bytes());
            assert!(snapshot.peak > out.charged_bytes());
            eprintln!(
                "A07_REPR_FLAT object={object} output={} capacity={} peak={}",
                out.len(),
                out.capacity(),
                snapshot.peak
            );
            drop(out);
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
    }
    #[tokio::test]
    async fn flat_container_quota_and_in_progress_event_cancellation_release_every_owner() {
        for object in [false, true] {
            let value = flat(object);
            let small = Budget::new(16384).unwrap();
            assert!(matches!(
                repr_budgeted(&value, &small, None).await,
                Err(Error::Resource)
            ));
            assert_eq!(small.snapshot().unwrap().used, 0);
            assert!(small.snapshot().unwrap().peak <= 16384);
            let budget = Budget::new(32 * 1024 * 1024).unwrap();
            let mut future = Box::pin(repr_budgeted(&value, &budget, None));
            poll_fn(|cx| match future.as_mut().poll(cx) {
                Poll::Ready(_) => panic!("flat repr completed before live event evidence"),
                Poll::Pending => {
                    if budget.snapshot().unwrap().used > 262144 {
                        Poll::Ready(())
                    } else {
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    }
                }
            })
            .await;
            let live = budget.snapshot().unwrap().used;
            let start = Instant::now();
            drop(future);
            eprintln!(
                "A07_REPR_FLAT_CANCEL object={object} live={live} drop_us={}",
                start.elapsed().as_micros()
            );
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
    }
}
