//! VirusTotal search records and arbitrary-precision integer detection totals.
use super::infrastructure_security::{self as s, *};
use super::{
    source_json::{Number, Value},
    source_payload::ProducedBatch,
};
use ssx_owned_budget::{Budget, OwnedString, OwnedVec};
use std::time::Instant;
s::adapter!(VirusTotalAdapter, Kind::VirusTotal);
async fn integer_add(b: &Budget, a: &str, rhs: &str, d: Instant) -> Result<OwnedString> {
    let (an, a) = a.strip_prefix('-').map_or((false, a), |s| (true, s));
    let (bn, rhs) = rhs.strip_prefix('-').map_or((false, rhs), |s| (true, s));
    let mut digits = OwnedVec::with_capacity(b, a.len().max(rhs.len()) + 1).map_err(resource)?;
    let negative;
    if an == bn {
        negative = an;
        let mut carry = 0u8;
        let mut ai = a.bytes().rev();
        let mut bi = rhs.bytes().rev();
        loop {
            checkpoint(Some(d)).await?;
            let x = ai.next();
            let y = bi.next();
            if x.is_none() && y.is_none() {
                break;
            }
            let sum = x.map_or(0, |c| c - b'0') + y.map_or(0, |c| c - b'0') + carry;
            digits.try_push(sum % 10).map_err(resource)?;
            carry = sum / 10;
        }
        if carry != 0 {
            digits.try_push(carry).map_err(resource)?;
        }
    } else {
        let cmp = a.len().cmp(&rhs.len()).then_with(|| a.cmp(rhs));
        let (large, small, sign) = if cmp.is_ge() {
            (a, rhs, an)
        } else {
            (rhs, a, bn)
        };
        negative = sign;
        let mut borrow = 0i16;
        let mut small = small.bytes().rev();
        for x in large.bytes().rev() {
            checkpoint(Some(d)).await?;
            let n = i16::from(x - b'0') - i16::from(small.next().map_or(0, |c| c - b'0')) - borrow;
            borrow = i16::from(n < 0);
            digits
                .try_push((n + if n < 0 { 10 } else { 0 }) as u8)
                .map_err(resource)?;
        }
    }
    while digits.len() > 1 && digits.as_slice().last() == Some(&0) {
        digits.pop();
    }
    let mut out = OwnedString::new(b).map_err(resource)?;
    out.try_reserve_exact(digits.len() + 1).map_err(resource)?;
    if negative && digits.as_slice().iter().any(|n| *n != 0) {
        out.try_push('-').map_err(resource)?;
    }
    for n in digits.as_slice().iter().rev() {
        out.try_push(char::from(b'0' + n)).map_err(resource)?;
    }
    Ok(out)
}
async fn total(b: &Budget, stats: &Value, d: Instant) -> Result<Option<OwnedString>> {
    let stats = object(stats)?;
    let mut integer = owned(b, "0", d).await?;
    let mut floating: Option<(f64, f64)> = None;
    for (_, v) in stats.iter() {
        checkpoint(Some(d)).await?;
        let n = match v {
            Value::Bool(v) => {
                if *v {
                    "1"
                } else {
                    "0"
                }
            }
            Value::Number(n @ Number::Integer(_)) => n.integer_text().expect("integer"),
            Value::Number(Number::Float(x)) => {
                let initial = match floating {
                    Some(v) => v,
                    None => {
                        let i = integer
                            .as_str()
                            .parse::<f64>()
                            .map_err(|_| error("int too large to convert to float"))?;
                        if !i.is_finite() {
                            return Err(error("int too large to convert to float"));
                        }
                        (i, 0.)
                    }
                };
                floating = Some(add_float(initial, *x));
                continue;
            }
            _ => {
                return Err(error(format!(
                    "unsupported operand type(s) for +: 'int' and '{}'",
                    typ(v)
                )));
            }
        };
        if let Some(current) = floating {
            let f = n
                .parse::<f64>()
                .map_err(|_| error("int too large to convert to float"))?;
            if !f.is_finite() {
                return Err(error("int too large to convert to float"));
            }
            floating = Some(add_float(current, f));
        } else {
            integer = integer_add(b, integer.as_str(), n, d).await?;
        }
    }
    if let Some((hi, lo)) = floating {
        let sum = if lo != 0. && lo.is_finite() {
            hi + lo
        } else {
            hi
        };
        if sum > 0. {
            Ok(Some(
                stringify(b, &Value::Number(Number::Float(sum)), d).await?,
            ))
        } else {
            Ok(None)
        }
    } else if integer.as_str() != "0" && !integer.as_str().starts_with('-') {
        if integer.len() > 4300 {
            return Err(error(format!(
                "Exceeds the limit (4300 digits) for integer string conversion: value has {} digits; use sys.set_int_max_str_digits() to increase the limit",
                integer.len()
            )));
        }
        Ok(Some(integer))
    } else {
        Ok(None)
    }
}
fn add_float((hi, lo): (f64, f64), x: f64) -> (f64, f64) {
    let t = hi + x;
    let correction = if hi.abs() >= x.abs() {
        (hi - t) + x
    } else {
        (x - t) + hi
    };
    (t, lo + correction)
}
pub(super) async fn parse(
    v: &Value,
    batch: &mut ProducedBatch<'_>,
    b: &Budget,
    d: Instant,
) -> Result<()> {
    let empty = Value::String("".into());
    let map = Value::Object(Default::default());
    let list = Value::Array(Vec::new());
    let zero = super::source_json::from_slice(b"0", Some(d))
        .await
        .map_err(|_| timeout())?;
    for (i, item) in iterable(field(v, "data", &list)?)?.iter().enumerate() {
        checkpoint(Some(d)).await?;
        let attrs = field(item, "attributes", &map)?;
        let id = field(item, "id", &empty)?;
        let stats = field(attrs, "last_analysis_stats", &map)?;
        let malicious = field(stats, "malicious", &zero)?;
        let suspicious = field(stats, "suspicious", &zero)?;
        let sum = if truth(stats) {
            total(b, stats, d).await?
        } else {
            None
        };
        let meaningful = field(attrs, "meaningful_name", &zero)?;
        let description = field(attrs, "type_description", &zero)?;
        let title = if truth(meaningful) {
            owned(b, text(meaningful)?, d).await?
        } else if truth(description) {
            let description = stringify(b, description, d).await?;
            let prefix = slice_repr(b, id, 32, d).await?;
            join(b, &[description.as_str(), ": ", prefix.as_str()], "", d).await?
        } else {
            owned(b, text(id)?, d).await?
        };
        let mut p = parts(b, 4).await?;
        if let Some(sum) = sum {
            let malicious = stringify(b, malicious, d).await?;
            let suspicious = stringify(b, suspicious, d).await?;
            add(
                &mut p,
                join(
                    b,
                    &[
                        "Detection: ",
                        malicious.as_str(),
                        "/",
                        sum.as_str(),
                        " engines (",
                        suspicious.as_str(),
                        " suspicious)",
                    ],
                    "",
                    d,
                )
                .await?,
            )
            .await?;
        } else {
            add(&mut p, owned(b, "Detection: unknown", d).await?).await?;
        }
        if truth(description) {
            let description = stringify(b, description, d).await?;
            add(
                &mut p,
                join(b, &["Type: ", description.as_str()], "", d).await?,
            )
            .await?;
        }
        let reputation = field(attrs, "reputation", &zero)?;
        if truth(reputation) {
            let rep = stringify(b, reputation, d).await?;
            add(
                &mut p,
                join(b, &["Reputation: ", rep.as_str()], "", d).await?,
            )
            .await?;
        }
        let id = stringify(b, id, d).await?;
        let url = join(
            b,
            &["https://virustotal.com/gui/search/", id.as_str()],
            "",
            d,
        )
        .await?;
        add(&mut p, join(b, &["VT link: ", url.as_str()], "", d).await?).await?;
        let content = content(b, &p, "", d).await?;
        push(
            batch,
            Kind::VirusTotal,
            Row {
                url: url.as_str(),
                title: title.as_str(),
                content: content.as_str(),
                score: float(malicious)?,
                position: i,
                date: None,
            },
        )
        .await?;
    }
    Ok(())
}
