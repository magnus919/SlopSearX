//! Internet Archive catalog and authoritative CDX with one bounded closest fallback.
use super::{catalog_media as m, clinicaltrials as c, source_json::Value};
use ssx_domain::{EngineStatus, SearchRequest};
use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};
pub const ENDPOINT: &str = "https://archive.org";
pub const CDX: &str = "https://web.archive.org/cdx/search/cdx";
pub const AVAILABILITY: &str = "https://archive.org/wayback/available";
#[derive(Clone, Debug)]
pub struct InternetArchiveConfig {
    pub settings: m::Settings,
    /// Private-loopback qualification injection; never read from YAML/env.
    pub cdx_url: Option<String>,
    pub availability_url: Option<String>,
}
impl Default for InternetArchiveConfig {
    fn default() -> Self {
        let mut settings = m::Settings::defaults(ENDPOINT);
        settings.timeout_ms = ssx_config::ConfigScalar::Integer(10000);
        Self {
            settings,
            cdx_url: None,
            availability_url: None,
        }
    }
}
m::adapter!(
    InternetArchiveAdapter,
    InternetArchiveConfig,
    "internetarchive",
    InternetArchiveConfig::default()
);
fn word(ch: char) -> bool {
    static RANGES: OnceLock<Vec<(u32, u32)>> = OnceLock::new();
    let ranges = RANGES.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a09/word16.json"
        ))
        .expect("frozen Unicode16 Python word ranges")
    });
    let point = ch as u32;
    let n = ranges.partition_point(|(_, end)| *end < point);
    ranges.get(n).is_some_and(|(start, _)| *start <= point)
}
pub fn is_domain_query(query: &str) -> bool {
    let query = query.trim_matches(ssx_json::python_whitespace);
    let Some((head, suffix)) = query.rsplit_once('.') else {
        return false;
    };
    !head.is_empty()
        && head.chars().all(|c| word(c) || matches!(c, '.' | '-'))
        && suffix.chars().count() >= 2
        && suffix
            .chars()
            .all(|c| c.is_ascii_alphabetic() || matches!(c, 'İ' | 'ı' | 'ſ' | 'K'))
}
async fn domain_query_with_context(query: &str, context: Option<Instant>) -> m::Result<bool> {
    let query = query.trim_matches(ssx_json::python_whitespace);
    let Some((head, suffix)) = query.rsplit_once('.') else {
        return Ok(false);
    };
    if head.is_empty() {
        return Ok(false);
    }
    for (i, ch) in head.chars().enumerate() {
        if i % 1024 == 0 {
            c::row_checkpoint(context).await?;
        }
        if !(word(ch) || matches!(ch, '.' | '-')) {
            return Ok(false);
        }
    }
    let mut length = 0;
    for ch in suffix.chars() {
        if length % 1024 == 0 {
            c::row_checkpoint(context).await?;
        }
        length += 1;
        if !(ch.is_ascii_alphabetic() || matches!(ch, 'İ' | 'ı' | 'ſ' | 'K')) {
            return Ok(false);
        }
    }
    Ok(length >= 2)
}
impl InternetArchiveAdapter {
    async fn execute<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        b: &'b c::Budget,
    ) -> m::Result<m::Batch<'b>> {
        c::admit_with_context(&self.admission, "internetarchive", context).await?;
        if domain_query_with_context(&q.query, context).await? {
            self.wayback(q, context, b).await
        } else {
            self.catalog(q, context, b).await
        }
    }
    async fn catalog<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        b: &'b c::Budget,
    ) -> m::Result<m::Batch<'b>> {
        let deadline =
            c::network_deadline(&self.config.settings, false, context).map_err(|error| {
                let mut failure = m::Failure::from(error);
                // Only configured nonpositive timers fail here; caller-context expiry
                // is checked by admission/fetch and retains Timeout classification.
                if failure.status == EngineStatus::Timeout {
                    failure.status = EngineStatus::Error;
                    failure.message = Some(String::new());
                }
                failure
            })?;
        let quoted = c::owned_quote(b, &q.query, true, context).await?;
        let max = match &self.config.settings.max_results {
            ssx_config::ConfigScalar::Boolean(v) => if *v { "True" } else { "False" }.into(),
            v => c::scalar_text(v),
        };
        let url = m::join(
            b,
            &[
                &self.config.settings.base_url,
                "/advancedsearch.php?q=",
                quoted.as_str(),
                "&output=json&rows=",
                &max,
            ],
            None,
            context,
        )
        .await?;
        let req = c::request(url.as_str(), &[], false, false)?;
        let data = m::fetch(
            &self.http,
            req,
            deadline,
            context,
            false,
            true,
            "Internet Archive",
        )
        .await?;
        let map = c::map();
        let list = c::list();
        let empty = c::empty();
        let response = c::field(&data, "response", &map)?;
        let rows = c::result_rows(
            c::field(response, "docs", &list)?,
            &self.config.settings.max_results,
        )?;
        let mut batch = m::Batch::new(b, rows.len(), context)?;
        for doc in rows {
            c::row_checkpoint(context).await?;
            let id = c::field(doc, "identifier", &empty)?;
            let id_text = c::owned_stringify(b, id, context).await?;
            let url = if c::truthy(id) {
                m::join(
                    b,
                    &["https://archive.org/details/", id_text.as_str()],
                    None,
                    context,
                )
                .await?
            } else {
                m::owned(b, "", context).await?
            };
            let title = m::text(c::field(doc, "title", &empty)?)?;
            let description = c::field(doc, "description", &empty)?;
            let description = if c::truthy(description) {
                description
            } else {
                &empty
            };
            let content = c::owned_prefix(
                b,
                description
                    .as_text()
                    .ok_or_else(|| m::error("Internet Archive description must be text"))?,
                500,
                context,
            )
            .await?;
            let score = if let Some(v) = doc.get("downloads") {
                m::float(v)? / 10000.
            } else {
                0.
            };
            let date = doc
                .get("date")
                .filter(|v| !matches!(v, Value::Null))
                .map(m::text)
                .transpose()?;
            batch
                .rows
                .push(
                    c::Skeleton {
                        url: url.as_str(),
                        title,
                        content: content.as_str(),
                        engine: "internetarchive",
                        category: "general",
                        score,
                        position: 0,
                        tier: 1,
                        published_date: date,
                    },
                    None,
                )
                .await
                .map_err(c::projection_error)?;
        }
        Ok(batch)
    }
    async fn wayback<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        b: &'b c::Budget,
    ) -> m::Result<m::Batch<'b>> {
        let millis = c::numeric(&self.config.settings.timeout_ms, false)?;
        let seconds = (millis / 1000.).max(0.001);
        let duration = Duration::try_from_secs_f64(seconds)
            .map_err(|_| m::error("invalid archive timeout"))?;
        let now = Instant::now();
        let configured = now
            .checked_add(duration)
            .ok_or_else(|| m::error("invalid archive deadline"))?;
        let deadline = context.map_or(configured, |v| v.min(configured));
        let cdx_deadline = now
            .checked_add(Duration::from_secs_f64((seconds / 2.).max(0.001)))
            .ok_or_else(|| m::error("invalid CDX deadline"))?
            .min(deadline);
        let domain = q.query.trim_matches(ssx_json::python_whitespace);
        let quoted = c::owned_quote(b, domain, true, context).await?;
        let max = match &self.config.settings.max_results {
            ssx_config::ConfigScalar::Boolean(v) => if *v { "True" } else { "False" }.into(),
            v => c::scalar_text(v),
        };
        let url = m::join(
            b,
            &[
                self.config.cdx_url.as_deref().unwrap_or(CDX),
                "?url=",
                quoted.as_str(),
                "&output=json&limit=",
                &max,
                "&filter=statuscode:200&collapse=timestamp:6",
            ],
            None,
            context,
        )
        .await?;
        let req = c::request(url.as_str(), &[], false, false)?;
        let attempted = m::fetch(
            &self.http,
            req,
            cdx_deadline,
            Some(deadline),
            true,
            false,
            "CDX",
        )
        .await;
        let cdx_failure = match attempted {
            Ok(raw) => match self.cdx(&raw, domain, b, Some(deadline)).await {
                Ok(batch) => return Ok(batch),
                Err(e) => e,
            },
            Err(e) => e,
        };
        if Instant::now() >= deadline {
            return Err(m::failure(
                EngineStatus::Timeout,
                "CDX failed and the aggregate Wayback deadline expired before fallback",
            ));
        }
        let url = m::join(
            b,
            &[
                self.config
                    .availability_url
                    .as_deref()
                    .unwrap_or(AVAILABILITY),
                "?url=",
                quoted.as_str(),
            ],
            None,
            Some(deadline),
        )
        .await?;
        let req = c::request(url.as_str(), &[], false, false)?;
        let data = m::fetch(
            &self.http,
            req,
            deadline,
            Some(deadline),
            true,
            false,
            "Availability API",
        )
        .await?;
        let snapshots = data
            .get("archived_snapshots")
            .filter(|v| v.as_object().is_some())
            .ok_or_else(|| m::error("Availability API returned malformed JSON"))?;
        let closest = snapshots.get("closest").filter(|v| v.as_object().is_some());
        let mut batch = m::Batch::new(b, 1, Some(deadline))?;
        let Some(closest) = closest.filter(|v| v.get("available") == Some(&Value::Bool(true)))
        else {
            batch.diagnostic = Some("No archived snapshot available".into());
            return Ok(batch);
        };
        let null = Value::Null;
        let status =
            c::owned_stringify(b, closest.get("status").unwrap_or(&null), Some(deadline)).await?;
        if status.as_str() != "200" {
            return Err(m::error(
                "Availability API returned an unusable snapshot status",
            ));
        }
        self.snapshot(&mut batch, domain, closest, b, Some(deadline))
            .await?;
        let status = match cdx_failure.status {
            EngineStatus::Timeout => "timeout",
            EngineStatus::RateLimited => "rate_limited",
            EngineStatus::Blocked => "blocked",
            _ => "error",
        };
        batch.diagnostic = Some(format!(
            "CDX unavailable ({status}); returned one closest snapshot from the Availability API, not a CDX history"
        ));
        Ok(batch)
    }
    async fn cdx<'b>(
        &self,
        raw: &Value,
        domain: &str,
        b: &'b c::Budget,
        context: Option<Instant>,
    ) -> m::Result<m::Batch<'b>> {
        let rows = raw
            .as_array()
            .filter(|rows| rows.is_empty() || rows[0].as_array().is_some())
            .ok_or_else(|| m::error("CDX returned malformed JSON"))?;
        let mut batch = m::Batch::new(b, rows.len().saturating_sub(1), context)?;
        for row in rows.iter().skip(1) {
            c::row_checkpoint(context).await?;
            let Some(row) = row.as_array().filter(|v| v.len() >= 3) else {
                continue;
            };
            let timestamp = c::owned_stringify(b, &row[1], context).await?;
            let ts = timestamp.as_str();
            let original = c::owned_stringify(b, &row[2], context).await?;
            if ts.chars().count() != 14
                || !ts.chars().all(ssx_json::decimal_digit)
                || original.is_empty()
            {
                continue;
            }
            let empty = c::empty();
            let mime = c::owned_stringify(b, row.get(3).unwrap_or(&empty), context).await?;
            let date = date(b, ts, context).await?;
            let url = m::join(
                b,
                &["https://web.archive.org/web/", ts, "/", original.as_str()],
                None,
                context,
            )
            .await?;
            let title = m::join(
                b,
                &[domain, " — ", date.as_str(), " (", mime.as_str(), ")"],
                None,
                context,
            )
            .await?;
            let content = m::join(
                b,
                &[
                    "Archived snapshot of ",
                    original.as_str(),
                    " from ",
                    date.as_str(),
                ],
                None,
                context,
            )
            .await?;
            batch
                .rows
                .push(
                    c::Skeleton {
                        url: url.as_str(),
                        title: title.as_str(),
                        content: content.as_str(),
                        engine: "internetarchive",
                        category: "general",
                        score: 0.5,
                        position: 0,
                        tier: 1,
                        published_date: Some(date.as_str()),
                    },
                    None,
                )
                .await
                .map_err(c::projection_error)?;
        }
        Ok(batch)
    }
    async fn snapshot(
        &self,
        batch: &mut m::Batch<'_>,
        domain: &str,
        closest: &Value,
        b: &c::Budget,
        context: Option<Instant>,
    ) -> m::Result<()> {
        let reject = || m::error("Availability API returned an untrusted snapshot URL");
        let empty = c::empty();
        let ts = c::owned_stringify(b, closest.get("timestamp").unwrap_or(&empty), context).await?;
        if ts.as_str().chars().count() != 14 || !ts.as_str().chars().all(ssx_json::decimal_digit) {
            return Err(reject());
        }
        let raw = closest
            .get("url")
            .and_then(|v| v.as_text())
            .and_then(|v| v.as_str())
            .ok_or_else(reject)?;
        let mut view = c::OwnedString::new(b).map_err(c::owned_error)?;
        view.try_reserve_exact(raw.len()).map_err(c::owned_error)?;
        for (i, ch) in raw
            .trim_start_matches(|c| c <= '\u{20}')
            .chars()
            .enumerate()
        {
            if i % 1024 == 0 {
                c::row_checkpoint(context).await?;
            }
            if !matches!(ch, '\r' | '\n' | '\t') {
                view.try_push(ch).map_err(c::owned_error)?;
            }
        }
        let outer = split(view.as_str()).ok_or_else(reject)?;
        if !(outer.scheme.eq_ignore_ascii_case("http")
            || outer.scheme.eq_ignore_ascii_case("https"))
            || !outer.host.eq_ignore_ascii_case("web.archive.org")
            || outer.port
            || outer.userinfo
            || !outer.query.is_empty()
            || !outer.fragment.is_empty()
        {
            return Err(reject());
        }
        let path = super::regulatory::path_without_params(outer.path);
        let tail = path.strip_prefix("/web/").ok_or_else(reject)?;
        let (stamp, original) = tail.split_once('/').ok_or_else(reject)?;
        if stamp != ts.as_str()
            || !(original.starts_with("http://") || original.starts_with("https://"))
        {
            return Err(reject());
        }
        let original_parts = split(original).ok_or_else(reject)?;
        let requested = super::tmdb_case::lower(domain, b, context).await?;
        let requested = requested.as_str().trim_end_matches('.');
        let original_host = super::tmdb_case::lower(original_parts.host, b, context).await?;
        if !(original_parts.scheme.eq_ignore_ascii_case("http")
            || original_parts.scheme.eq_ignore_ascii_case("https"))
            || original_host.as_str() != requested
        {
            return Err(reject());
        }
        let normalized_path = if outer.path.ends_with(';') && path.len() + 1 == outer.path.len() {
            path
        } else {
            outer.path
        };
        let url = m::join(
            b,
            &["https://", outer.netloc, normalized_path],
            None,
            context,
        )
        .await?;
        let date = date(b, ts.as_str(), context).await?;
        let title = m::join(
            b,
            &[domain, " — closest archived snapshot (", date.as_str(), ")"],
            None,
            context,
        )
        .await?;
        let original_scheme = super::tmdb_case::lower(original_parts.scheme, b, context).await?;
        let normalized_original = m::join(
            b,
            &[
                original_scheme.as_str(),
                "://",
                original.split_once("://").unwrap().1,
            ],
            None,
            context,
        )
        .await?;
        let content = m::join(
            b,
            &[
                "One closest archived snapshot of ",
                normalized_original.as_str(),
                " from ",
                date.as_str(),
                "; the Availability API does not provide a full CDX history.",
            ],
            None,
            context,
        )
        .await?;
        batch
            .rows
            .push(
                c::Skeleton {
                    url: url.as_str(),
                    title: title.as_str(),
                    content: content.as_str(),
                    engine: "internetarchive",
                    category: "general",
                    score: 0.5,
                    position: 0,
                    tier: 1,
                    published_date: Some(date.as_str()),
                },
                None,
            )
            .await
            .map_err(c::projection_error)?;
        Ok(())
    }
}
async fn date(b: &c::Budget, s: &str, context: Option<Instant>) -> m::Result<c::OwnedString> {
    // 14 Unicode decimal codepoints, matching source slicing rather than bytes.
    let mut out = c::OwnedString::new(b).map_err(c::owned_error)?;
    out.try_reserve_exact(s.len() + 2).map_err(c::owned_error)?;
    for (i, ch) in s.chars().take(8).enumerate() {
        if i == 4 || i == 6 {
            out.try_push('-').map_err(c::owned_error)?;
        }
        out.try_push(ch).map_err(c::owned_error)?;
    }
    c::row_checkpoint(context).await?;
    Ok(out)
}
struct Parts<'a> {
    scheme: &'a str,
    netloc: &'a str,
    host: &'a str,
    port: bool,
    userinfo: bool,
    path: &'a str,
    query: &'a str,
    fragment: &'a str,
}
fn split(raw: &str) -> Option<Parts<'_>> {
    let (scheme, tail) = raw.split_once("://")?;
    let end = tail.find(['/', '?', '#']).unwrap_or(tail.len());
    let netloc = &tail[..end];
    if ssx_domain::storage::python_url_netloc_error(netloc).is_some() {
        return None;
    }
    let host_port = netloc.rsplit_once('@').map_or(netloc, |(_, v)| v);
    let (host, port) = host_port
        .rsplit_once(':')
        .map_or((host_port, false), |(h, p)| (h, !p.is_empty()));
    let rest = &tail[end..];
    let (rest, fragment) = rest.split_once('#').unwrap_or((rest, ""));
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    Some(Parts {
        scheme,
        netloc,
        host,
        port,
        userinfo: netloc.contains('@'),
        path,
        query,
        fragment,
    })
}

#[cfg(test)]
fn set_test_key(config: &mut InternetArchiveConfig) {
    let _ = config;
}
