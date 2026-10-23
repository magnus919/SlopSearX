//! Source JSON comparisons for OpenAlex `(position, word)` abstract pairs.
//! Sorting policy is separate: standard Rust sort requires a total comparator.
use super::source_json::{Number, Text, Value};
use std::{cmp::Ordering, future::Future, pin::Pin, time::Instant};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Source(String),
    ComparisonType {
        left: &'static str,
        right: &'static str,
    },
    Deadline,
    Resource,
}
#[derive(Clone, Copy)]
pub enum Ref<'a> {
    Value(&'a Value),
    Text(&'a Text),
    Character(u32),
}
impl Ref<'_> {
    fn string_len(self) -> Option<usize> {
        match self {
            Self::Value(Value::String(s)) | Self::Text(s) => Some(s.len()),
            Self::Character(_) => Some(1),
            _ => None,
        }
    }
    fn point(self, at: usize) -> Option<u32> {
        match self {
            Self::Value(Value::String(s)) | Self::Text(s) => s.codepoints().get(at).copied(),
            Self::Character(p) => (at == 0).then_some(p),
            _ => None,
        }
    }
    fn kind(self) -> &'static str {
        match self {
            Self::Value(v) => super::broad_type(v),
            _ => "str",
        }
    }
    fn number(self) -> Option<Numeric<'static>> {
        match self {
            Self::Value(Value::Bool(v)) => Some(Numeric::Integer(if *v { "1" } else { "0" })),
            Self::Value(Value::Number(Number::Float(v))) => Some(Numeric::Float(*v)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy)]
enum Numeric<'a> {
    Integer(&'a str),
    Float(f64),
}
fn numeric(value: Ref<'_>) -> Option<Numeric<'_>> {
    if let Ref::Value(Value::Number(v)) = value
        && let Some(text) = v.integer_text()
    {
        return Some(Numeric::Integer(text));
    }
    value.number()
}
fn integer_parts(raw: &str) -> (bool, &str) {
    let (negative, digits) = raw.strip_prefix('-').map_or((false, raw), |s| (true, s));
    let digits = digits.trim_start_matches('0');
    (negative && !digits.is_empty(), digits)
}
fn integer_cmp(a: &str, b: &str) -> Ordering {
    let (an, a) = integer_parts(a);
    let (bn, b) = integer_parts(b);
    match an.cmp(&bn).reverse() {
        Ordering::Equal => {
            let order = a.len().cmp(&b.len()).then_with(|| a.cmp(b));
            if an { order.reverse() } else { order }
        }
        order => order,
    }
}
fn integer_float_cmp(
    integer: &str,
    float: f64,
    budget: Option<&ssx_owned_budget::Budget>,
) -> Result<Option<Ordering>, Error> {
    if float.is_nan() {
        return Ok(None);
    }
    if float == f64::INFINITY {
        return Ok(Some(Ordering::Less));
    }
    if float == f64::NEG_INFINITY {
        return Ok(Some(Ordering::Greater));
    }
    // Every finite integral f64 is represented exactly by fixed-point formatting.
    // Compare decimal integers without rounding an arbitrary integer to f64.
    let whole = float.trunc();
    // f64::MAX < 10^309: at most309 integral decimal digits, plus one
    // negative sign. No decimal point/exponent at precision zero. Exact backing
    // is precharged; Limited refuses any formatting growth beyond310 bytes.
    const CAPACITY: usize = 310;
    let _charge = budget
        .map(|b| b.reserve(CAPACITY))
        .transpose()
        .map_err(|_| Error::Resource)?;
    let mut text = String::new();
    text.try_reserve_exact(CAPACITY)
        .map_err(|_| Error::Resource)?;
    if text.capacity() != CAPACITY {
        return Err(Error::Resource);
    }
    struct Limited<'a>(&'a mut String);
    impl std::fmt::Write for Limited<'_> {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            if self
                .0
                .len()
                .checked_add(s.len())
                .is_none_or(|n| n > self.0.capacity())
            {
                return Err(std::fmt::Error);
            }
            self.0.push_str(s);
            Ok(())
        }
    }
    use std::fmt::Write as _;
    write!(Limited(&mut text), "{whole:.0}").map_err(|_| Error::Resource)?;
    let order = integer_cmp(integer, &text);
    Ok(Some(if order == Ordering::Equal {
        if float > whole {
            Ordering::Less
        } else if float < whole {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    } else {
        order
    }))
}
fn numeric_cmp(
    a: Numeric<'_>,
    b: Numeric<'_>,
    budget: Option<&ssx_owned_budget::Budget>,
) -> Result<Option<Ordering>, Error> {
    match (a, b) {
        (Numeric::Integer(a), Numeric::Integer(b)) => Ok(Some(integer_cmp(a, b))),
        (Numeric::Float(a), Numeric::Float(b)) => Ok(a.partial_cmp(&b)),
        (Numeric::Integer(a), Numeric::Float(b)) => integer_float_cmp(a, b, budget),
        (Numeric::Float(a), Numeric::Integer(b)) => {
            integer_float_cmp(b, a, budget).map(|order| order.map(Ordering::reverse))
        }
    }
}
async fn tick(deadline: Instant) -> Result<(), Error> {
    tokio::task::yield_now().await;
    if Instant::now() >= deadline {
        Err(Error::Deadline)
    } else {
        Ok(())
    }
}
type Comparison<'a> = Pin<Box<dyn Future<Output = Result<bool, Error>> + Send + 'a>>;
struct ChargedComparison<F> {
    inner: Pin<Box<F>>,
    _charge: ssx_owned_budget::Charge,
}
impl<F: Future> Future for ChargedComparison<F> {
    type Output = F::Output;
    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        self.inner.as_mut().poll(cx)
    }
}
fn boxed<'a, F>(
    future: F,
    budget: Option<&ssx_owned_budget::Budget>,
) -> Result<Comparison<'a>, Error>
where
    F: Future<Output = Result<bool, Error>> + Send + 'a,
{
    if let Some(budget) = budget {
        let bytes = std::alloc::Layout::new::<F>()
            .size()
            .checked_add(std::alloc::Layout::new::<ChargedComparison<F>>().size())
            .ok_or(Error::Resource)?;
        let charge = budget.reserve(bytes).map_err(|_| Error::Resource)?;
        // Match the reviewed parser future boundary: both boxes are charged
        // before allocation; system OOM is supervised process failure. Inner
        // storage drops before its nonClone charge, including cancellation.
        Ok(Box::pin(ChargedComparison {
            inner: Box::pin(future),
            _charge: charge,
        }))
    } else {
        Ok(Box::pin(future))
    }
}

/// Python equality, including numeric cross-types and recursive JSON containers.
pub fn equal<'a>(a: Ref<'a>, b: Ref<'a>, deadline: Instant) -> Comparison<'a> {
    equal_inner(a, b, deadline, None).expect("unbudgeted comparison does not reserve charges")
}
fn equal_inner<'a>(
    a: Ref<'a>,
    b: Ref<'a>,
    deadline: Instant,
    budget: Option<&'a ssx_owned_budget::Budget>,
) -> Result<Comparison<'a>, Error> {
    let future = async move {
        tick(deadline).await?;
        if let (Some(a), Some(b)) = (numeric(a), numeric(b)) {
            // json.loads uses its canonical NaN object for every NaN token.
            // Container/tuple equality invokes RichCompareBool, which tests
            // identity before numeric equality. This is source-byte identity,
            // not a claim that Python float nan == nan is true.
            if matches!((a,b), (Numeric::Float(a),Numeric::Float(b)) if a.is_nan() && b.is_nan()) {
                return Ok(true);
            }
            return Ok(numeric_cmp(a, b, budget)? == Some(Ordering::Equal));
        }
        if let (Some(an), Some(bn)) = (a.string_len(), b.string_len()) {
            if an != bn {
                return Ok(false);
            }
            for at in 0..an {
                if at % 1024 == 0 {
                    tick(deadline).await?;
                }
                if a.point(at) != b.point(at) {
                    return Ok(false);
                }
            }
            return Ok(true);
        }
        match (a, b) {
            (Ref::Value(Value::Null), Ref::Value(Value::Null)) => Ok(true),
            (Ref::Value(Value::Array(a)), Ref::Value(Value::Array(b))) => {
                if a.len() != b.len() {
                    return Ok(false);
                }
                for (a, b) in a.iter().zip(b) {
                    if !equal_inner(Ref::Value(a), Ref::Value(b), deadline, budget)?.await? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (Ref::Value(Value::Object(a)), Ref::Value(Value::Object(b))) => {
                if a.len() != b.len() {
                    return Ok(false);
                }
                for (key, value) in a.iter() {
                    let mut found = false;
                    for (other, value2) in b.iter() {
                        if equal_inner(Ref::Text(key), Ref::Text(other), deadline, budget)?.await? {
                            if !equal_inner(
                                Ref::Value(value),
                                Ref::Value(value2),
                                deadline,
                                budget,
                            )?
                            .await?
                            {
                                return Ok(false);
                            }
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    };
    boxed(future, budget)
}
/// Python less-than; unordered NaN is false, not an invented total ordering.
pub fn less<'a>(a: Ref<'a>, b: Ref<'a>, deadline: Instant) -> Comparison<'a> {
    let inner =
        less_inner(a, b, deadline, None).expect("unbudgeted comparison does not reserve charges");
    Box::pin(async move { inner.await.map_err(legacy_error) })
}
fn less_inner<'a>(
    a: Ref<'a>,
    b: Ref<'a>,
    deadline: Instant,
    budget: Option<&'a ssx_owned_budget::Budget>,
) -> Result<Comparison<'a>, Error> {
    let future = async move {
        tick(deadline).await?;
        if let (Some(a), Some(b)) = (numeric(a), numeric(b)) {
            return Ok(numeric_cmp(a, b, budget)? == Some(Ordering::Less));
        }
        if let (Some(an), Some(bn)) = (a.string_len(), b.string_len()) {
            for at in 0..an.min(bn) {
                if at % 1024 == 0 {
                    tick(deadline).await?;
                }
                match a.point(at).cmp(&b.point(at)) {
                    Ordering::Equal => {}
                    order => return Ok(order == Ordering::Less),
                }
            }
            return Ok(an < bn);
        }
        if let (Ref::Value(Value::Array(a)), Ref::Value(Value::Array(b))) = (a, b) {
            for (a, b) in a.iter().zip(b) {
                if !equal_inner(Ref::Value(a), Ref::Value(b), deadline, budget)?.await? {
                    return less_inner(Ref::Value(a), Ref::Value(b), deadline, budget)?.await;
                }
            }
            return Ok(a.len() < b.len());
        }
        Err(Error::ComparisonType {
            left: a.kind(),
            right: b.kind(),
        })
    };
    boxed(future, budget)
}

#[derive(Clone, Copy)]
pub struct Pair<'a> {
    pub position: Ref<'a>,
    pub word: &'a Text,
}
/// Tuple comparison checks equality first, including decoded NaN identity.
pub async fn pair_less(a: Pair<'_>, b: Pair<'_>, deadline: Instant) -> Result<bool, Error> {
    pair_less_budgeted(a, b, deadline, None)
        .await
        .map_err(legacy_error)
}
pub(super) async fn pair_less_budgeted<'a>(
    a: Pair<'a>,
    b: Pair<'a>,
    deadline: Instant,
    budget: Option<&'a ssx_owned_budget::Budget>,
) -> Result<bool, Error> {
    if equal_inner(a.position, b.position, deadline, budget)?.await? {
        less_inner(Ref::Text(a.word), Ref::Text(b.word), deadline, budget)?.await
    } else {
        less_inner(a.position, b.position, deadline, budget)?.await
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;
    use std::time::Duration;
    #[tokio::test]
    async fn recursive_future_charges_refuse_and_restore_after_observed_cancellation() {
        let flat = Value::Null;
        let tiny = ssx_owned_budget::Budget::new(1).unwrap();
        assert!(matches!(
            equal_inner(
                Ref::Value(&flat),
                Ref::Value(&flat),
                Instant::now() + Duration::from_secs(10),
                Some(&tiny)
            ),
            Err(Error::Resource)
        ));
        assert_eq!(tiny.snapshot().unwrap().used, 0);
        let flat_budget = ssx_owned_budget::Budget::new(1024 * 1024).unwrap();
        assert!(
            equal_inner(
                Ref::Value(&flat),
                Ref::Value(&flat),
                Instant::now() + Duration::from_secs(10),
                Some(&flat_budget)
            )
            .unwrap()
            .await
            .unwrap()
        );
        let single_peak = flat_budget.snapshot().unwrap().peak;
        assert_eq!(flat_budget.snapshot().unwrap().used, 0);
        let budget = ssx_owned_budget::Budget::new(1024 * 1024).unwrap();
        let task_budget = budget.clone();
        let task = tokio::spawn(async move {
            let mut nested = Value::Null;
            for _ in 0..80 {
                nested = Value::Array(vec![nested]);
            }
            equal_inner(
                Ref::Value(&nested),
                Ref::Value(&nested),
                Instant::now() + Duration::from_secs(60),
                Some(&task_budget),
            )
            .unwrap()
            .await
        });
        while budget.snapshot().unwrap().used <= single_peak {
            assert!(
                !task.is_finished(),
                "observe nested future allocation before completion"
            );
            tokio::task::yield_now().await;
        }
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[tokio::test]
    async fn actual_two_pair_source_tuple_comparisons() {
        let fixture: ssx_json::Value = ssx_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a06/source-openalex-order.json"
        ))
        .unwrap();
        let mut checked = Vec::new();
        for row in fixture["records"].as_array().unwrap() {
            let body = base64::engine::general_purpose::STANDARD
                .decode(row["responses"][0]["body_b64"].as_str().unwrap())
                .unwrap();
            let value = super::super::source_json::from_slice(&body, None)
                .await
                .unwrap();
            let work = &value.get("results").unwrap().as_array().unwrap()[0];
            let inverted = work
                .get("abstract_inverted_index")
                .unwrap()
                .as_object()
                .unwrap();
            let mut pairs = Vec::new();
            let mut applicable = true;
            for (word, positions) in inverted.iter() {
                if let Some(positions) = positions.as_array() {
                    for position in positions {
                        pairs.push(Pair {
                            position: Ref::Value(position),
                            word,
                        });
                    }
                } else {
                    applicable = false;
                }
            }
            if !applicable || pairs.len() != 2 {
                continue;
            }
            let actual =
                pair_less(pairs[1], pairs[0], Instant::now() + Duration::from_secs(5)).await;
            if row["output"]["status"].as_str() == Some("error") {
                assert_eq!(
                    actual,
                    Err(Error::Source(
                        row["output"]["error_message"].as_str().unwrap().into()
                    )),
                    "{}",
                    row["id"]
                );
            } else {
                if actual.unwrap() {
                    pairs.swap(0, 1);
                }
                let text = format!(
                    "{} {}",
                    pairs[0].word.as_str().unwrap(),
                    pairs[1].word.as_str().unwrap()
                );
                assert_eq!(
                    text,
                    row["output"]["results"][0]["content"].as_str().unwrap(),
                    "{}",
                    row["id"]
                );
            }
            checked.push(row["id"].as_str().unwrap().to_owned());
        }
        eprintln!(
            "two-pair operation controls={} ids={checked:?}",
            checked.len()
        );
        let expected = [
            "openalex.order.0",
            "openalex.order.1",
            "openalex.order.2",
            "openalex.order.3",
            "openalex.order.4",
            "openalex.order.5",
            "openalex.order.6",
            "openalex.order.9",
            "openalex.order.10",
            "openalex.order.11",
            "openalex.order.12",
            "openalex.order.13",
            "openalex.order.14",
            "openalex.order.15",
            "openalex.order.16",
            "openalex.order.34",
            "openalex.order.35",
            "openalex.order.36",
            "openalex.order.37",
            "openalex.order.38",
            "openalex.order.39",
            "openalex.order.40",
            "openalex.order.41",
            "openalex.order.42",
            "openalex.order.43",
            "openalex.order.44",
            "openalex.order.45",
            "openalex.order.46",
            "openalex.order.47",
            "openalex.order.48",
            "openalex.order.49",
            "openalex.order.50",
            "openalex.order.51",
            "openalex.order.52",
            "openalex.order.53",
            "openalex.order.54",
            "openalex.order.55",
            "openalex.order.56",
            "openalex.order.57",
            "openalex.order.58",
            "openalex.order.59",
            "openalex.order.60",
            "openalex.order.61",
            "openalex.order.62",
            "openalex.order.63",
            "openalex.order.64",
            "openalex.order.65",
            "openalex.order.66",
            "openalex.order.67",
            "openalex.order.68",
            "openalex.order.69",
            "openalex.order.70",
            "openalex.order.71",
            "openalex.order.72",
            "openalex.order.73",
            "openalex.order.74",
            "openalex.order.75",
            "openalex.order.76",
            "openalex.order.77",
            "openalex.order.78",
            "openalex.order.79",
            "openalex.order.80",
            "openalex.order.81",
            "openalex.order.82",
            "openalex.order.83",
            "openalex.order.84",
            "openalex.order.85",
            "openalex.order.86",
            "openalex.order.87",
        ];
        assert_eq!(checked, expected);
    }
}

/// Legacy wrappers retain their original String diagnostic contract.
pub(super) fn legacy_error(error: Error) -> Error {
    match error {
        Error::ComparisonType { left, right } => Error::Source(format!(
            "'<' not supported between instances of '{left}' and '{right}'"
        )),
        other => other,
    }
}

#[cfg(test)]
mod bounded_numeric_controls {
    use super::*;
    #[test]
    fn exact_source_numeric_order_and_fixed_max_scratch() {
        let data: ssx_json::Value = ssx_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a06/source-order-numeric.json"
        ))
        .unwrap();
        let budget = ssx_owned_budget::Budget::new(310).unwrap();
        let mut ids = std::collections::BTreeSet::new();
        for row in data["records"].as_array().unwrap() {
            assert!(ids.insert(row["id"].as_str().unwrap()));
            let float = f64::from_bits(
                u64::from_str_radix(row["float_bits"].as_str().unwrap(), 16).unwrap(),
            );
            let actual =
                integer_float_cmp(row["integer"].as_str().unwrap(), float, Some(&budget)).unwrap();
            let expected = match row["ordering"].as_str() {
                Some("less") => Some(Ordering::Less),
                Some("greater") => Some(Ordering::Greater),
                Some("equal") => Some(Ordering::Equal),
                None => None,
                _ => panic!(),
            };
            assert_eq!(actual, expected, "{}", row["id"]);
            assert_eq!(budget.snapshot().unwrap().used, 0);
            crate::adapters::a06_case::completed(
                "ssx-infra/lib/adapters::scholarly_order::bounded_numeric_controls::exact_source_numeric_order_and_fixed_max_scratch",
                "docs/rust-rewrite/a06/source-order-numeric.json",
                row["id"].as_str().unwrap(),
                "default",
                "bounded_helper",
            );
        }
        assert_eq!(ids.len(), 144);
        assert_eq!(budget.snapshot().unwrap().peak, 310);
        let tiny = ssx_owned_budget::Budget::new(309).unwrap();
        assert_eq!(
            integer_float_cmp("0", -f64::MAX, Some(&tiny)),
            Err(Error::Resource)
        );
        assert_eq!(tiny.snapshot().unwrap().used, 0);
    }
}
