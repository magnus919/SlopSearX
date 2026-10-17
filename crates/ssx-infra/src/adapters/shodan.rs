//! Native Shodan device matching; bearer auth lives in the A12 request owner.
use super::infrastructure_security::{self as s, *};
use super::{source_json::Value, source_payload::ProducedBatch};
use ssx_owned_budget::Budget;
use std::time::Instant;
s::adapter!(ShodanAdapter, Kind::Shodan);
pub(super) async fn parse(
    v: &Value,
    limit: &ssx_config::ConfigScalar,
    batch: &mut ProducedBatch<'_>,
    b: &Budget,
    d: Instant,
) -> Result<()> {
    let empty = Value::String("".into());
    let list = Value::Array(Vec::new());
    let zero = Value::Bool(false);
    for (i, m) in slice(field(v, "matches", &list)?, limit)?
        .iter()
        .enumerate()
    {
        checkpoint(Some(d)).await?;
        let ip = field(m, "ip_str", &empty)?;
        let port = field(m, "port", &empty)?;
        let org = field(m, "org", &empty)?;
        let hostnames = field(m, "hostnames", &list)?;
        // The source first slices hostnames, then joins; a mapping cannot be sliced.
        if hostnames.as_object().is_some() {
            return Err(error("unhashable type: 'slice'"));
        }
        let hostnames = joined_values(b, hostnames, 3, d).await?;
        let product = field(m, "product", &empty)?;
        let version = field(m, "version", &empty)?;
        let transport = field(m, "transport", &empty)?;
        let cvss = field(m, "cvss", &zero)?;
        let vulns = field(m, "vulns", &list)?;
        let ip_format = stringify(b, ip, d).await?;
        let port_format = stringify(b, port, d).await?;
        let title = if truth(port) {
            join(b, &[ip_format.as_str(), port_format.as_str()], ":", d).await?
        } else {
            owned(b, text(ip)?, d).await?
        };
        let mut p = parts(b, 6).await?;
        if truth(product) {
            let part = if truth(version) {
                let product = stringify(b, product, d).await?;
                let version = stringify(b, version, d).await?;
                join(b, &[product.as_str(), version.as_str()], " ", d).await?
            } else {
                owned(b, text(product)?, d).await?
            };
            add(&mut p, part).await?;
        }
        for value in [transport, org] {
            if truth(value) {
                add(&mut p, owned(b, text(value)?, d).await?).await?;
            }
        }
        if !hostnames.is_empty() {
            add(&mut p, hostnames).await?;
        }
        if truth(cvss) {
            let cvss = stringify(b, cvss, d).await?;
            add(&mut p, join(b, &["CVSS ", cvss.as_str()], "", d).await?).await?;
        }
        if truth(vulns) {
            let vulns = joined_values(b, vulns, 5, d).await?;
            add(&mut p, join(b, &["CVEs: ", vulns.as_str()], "", d).await?).await?;
        }
        let content = content(b, &p, "Device match", d).await?;
        let url = join(
            b,
            &["https://www.shodan.io/host/", ip_format.as_str()],
            "",
            d,
        )
        .await?;
        push(
            batch,
            Kind::Shodan,
            Row {
                url: if truth(ip) { url.as_str() } else { "" },
                title: title.as_str(),
                content: content.as_str(),
                score: if truth(cvss) { float(cvss)? } else { 0. },
                position: i,
                date: None,
            },
        )
        .await?;
    }
    Ok(())
}
