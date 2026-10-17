//! Native Censys hosts/search projection, statically derived from engines/censys.py.
use super::infrastructure_security::{self as s, *};
use super::{source_json::Value, source_payload::ProducedBatch};
use ssx_owned_budget::Budget;
use std::time::Instant;
s::adapter!(CensysAdapter, Kind::Censys);
pub(super) async fn parse(
    v: &Value,
    batch: &mut ProducedBatch<'_>,
    b: &Budget,
    d: Instant,
) -> Result<()> {
    let empty = Value::String("".into());
    let map = Value::Object(Default::default());
    let list = Value::Array(Vec::new());
    let result = field(v, "result", &map)?;
    let hits = if result.as_object().is_some() {
        field(result, "hits", &list)?
    } else {
        &list
    };
    for (i, hit) in iterable(hits)?.iter().enumerate() {
        checkpoint(Some(d)).await?;
        let ip = field(hit, "ip", &empty)?;
        let location = field(hit, "location", &map)?;
        let location = if truth(location) { location } else { &map };
        let services = field(hit, "services", &list)?;
        let services = if truth(services) { services } else { &list };
        let asn = field(location, "asn", &empty)?;
        let country = field(location, "country", &empty)?;
        let city = field(location, "city", &empty)?;
        let mut entries = parts(b, 5).await?;
        if truth(services) {
            for row in slice(services, &ssx_config::ConfigScalar::Integer(5))? {
                let name = field(row, "service_name", &empty)?;
                if truth(name) {
                    let port = field(row, "port", &empty)?;
                    let name = stringify(b, name, d).await?;
                    let port = stringify(b, port, d).await?;
                    add(
                        &mut entries,
                        join(b, &[name.as_str(), port.as_str()], ":", d).await?,
                    )
                    .await?;
                }
            }
        }
        let refs = entries
            .as_slice()
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>();
        let service = join(b, &refs, ", ", d).await?;
        let mut content_parts = parts(b, 4).await?;
        if !service.is_empty() {
            add(&mut content_parts, service).await?;
        }
        if truth(asn) {
            add(&mut content_parts, owned(b, text(asn)?, d).await?).await?;
        }
        if truth(city) && truth(country) {
            let city = stringify(b, city, d).await?;
            let country = stringify(b, country, d).await?;
            add(
                &mut content_parts,
                join(b, &[city.as_str(), country.as_str()], ", ", d).await?,
            )
            .await?;
        } else if truth(country) {
            add(&mut content_parts, owned(b, text(country)?, d).await?).await?;
        }
        let content = content(b, &content_parts, "Host result", d).await?;
        let formatted = stringify(b, ip, d).await?;
        let url = join(
            b,
            &["https://search.censys.io/hosts/", formatted.as_str()],
            "",
            d,
        )
        .await?;
        push(
            batch,
            Kind::Censys,
            Row {
                url: if truth(ip) { url.as_str() } else { "" },
                title: text(ip)?,
                content: content.as_str(),
                score: 0.,
                position: i,
                date: None,
            },
        )
        .await?;
    }
    Ok(())
}
