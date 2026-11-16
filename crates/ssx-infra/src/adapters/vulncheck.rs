//! VulnCheck source CVE routing and single-record enrichment.
use super::infrastructure_security::{self as s, *};
use super::{source_json::Value, source_payload::ProducedBatch};
use ssx_owned_budget::{Budget, OwnedString};
use std::time::Instant;
s::adapter!(VulnCheckAdapter, Kind::VulnCheck);
pub(super) async fn cve(
    query: &str,
    d: Option<Instant>,
    b: &Budget,
) -> Result<Option<OwnedString>> {
    for (i, (start, _)) in query.char_indices().enumerate() {
        if i % 128 == 0 {
            checkpoint(d).await?;
        }
        let Some(prefix) = query.as_bytes().get(start..start + 4) else {
            continue;
        };
        if !prefix.eq_ignore_ascii_case(b"CVE-") {
            continue;
        }
        let mut tail = query[start + 4..].char_indices();
        let mut year = true;
        for _ in 0..4 {
            year &= tail.next().is_some_and(|(_, c)| ssx_json::decimal_digit(c));
        }
        if !year || !tail.next().is_some_and(|(_, c)| c == '-') {
            continue;
        }
        let mut digits = 0;
        let mut end = query.len();
        for (at, c) in tail {
            if !ssx_json::decimal_digit(c) {
                end = start + 4 + at;
                break;
            }
            if digits % 1024 == 0 {
                checkpoint(d).await?;
            }
            digits += 1;
        }
        if digits < 4 {
            continue;
        }
        let mut out = OwnedString::new(b).map_err(resource)?;
        out.try_reserve_exact(end - start).map_err(resource)?;
        out.try_push_str("CVE-").map_err(resource)?;
        for (at, c) in query[start + 4..end].chars().enumerate() {
            if at % 1024 == 0 {
                checkpoint(d).await?;
            }
            out.try_push(c).map_err(resource)?;
        }
        return Ok(Some(out));
    }
    Ok(None)
}
pub(super) async fn parse(
    v: &Value,
    cve: &str,
    batch: &mut ProducedBatch<'_>,
    b: &Budget,
    d: Instant,
) -> Result<()> {
    let map = Value::Object(Default::default());
    let list = Value::Array(Vec::new());
    let empty = Value::String("".into());
    let data = field(v, "data", &map)?;
    if !truth(data) {
        return Ok(());
    }
    let exploit = field(data, "exploit_state", &empty)?;
    let date = field(data, "date_added", &empty)?;
    let date = if truth(date) {
        date
    } else {
        field(data, "vulncheck_date_added", &empty)?
    };
    let vendor = field(data, "vendor_data", &list)?;
    let exploit_urls = field(data, "exploit_urls", &list)?;
    let cpe = field(data, "cpe", &list)?;
    let mut p = parts(b, 5).await?;
    if truth(exploit) {
        let value = stringify(b, exploit, d).await?;
        add(
            &mut p,
            join(b, &["Exploit State: ", value.as_str()], "", d).await?,
        )
        .await?;
    }
    if truth(date) {
        let value = slice_repr(b, date, 10, d).await?;
        add(&mut p, join(b, &["Added: ", value.as_str()], "", d).await?).await?;
    }
    if truth(vendor) {
        let mut vendors = std::collections::BTreeSet::new();
        match vendor {
            Value::String(_) => {}
            _ => {
                for v in slice(vendor, &ssx_config::ConfigScalar::Integer(5))? {
                    if v.as_object().is_some() {
                        vendors.insert(text(field(v, "vendor", &empty)?)?);
                    }
                }
            }
        }
        if !vendors.is_empty() {
            let values = vendors.iter().copied().collect::<Vec<_>>();
            let value = join(b, &values, ", ", d).await?;
            add(
                &mut p,
                join(b, &["Affected: ", value.as_str()], "", d).await?,
            )
            .await?;
        }
    }
    for (value, prefix, suffix) in [
        (exploit_urls, "Exploits: ", " available"),
        (cpe, "CPEs: ", ""),
    ] {
        if truth(value) {
            let len = match value {
                Value::Array(v) => v.len(),
                Value::Object(v) => v.len(),
                Value::String(v) => v.len(),
                _ => {
                    return Err(error(format!(
                        "object of type '{}' has no len()",
                        typ(value)
                    )));
                }
            };
            add(
                &mut p,
                join(b, &[prefix, &len.to_string(), suffix], "", d).await?,
            )
            .await?;
        }
    }
    let content = content(b, &p, "VulnCheck result", d).await?;
    let url = join(
        b,
        &["https://api.vulncheck.com/v3/community/cve/", cve],
        "",
        d,
    )
    .await?;
    push(
        batch,
        Kind::VulnCheck,
        Row {
            url: url.as_str(),
            title: cve,
            content: content.as_str(),
            score: 0.,
            position: 0,
            date: None,
        },
    )
    .await?;
    Ok(())
}
