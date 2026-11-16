//! Fixed-bucket, bounded-series Prometheus 0.0.4 registry. No query labels.
use ssx_application::health::OutcomeOrigin;
use ssx_domain::{AdapterResponse, EngineStatus};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
const LATENCY: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1., 2.5, 5., 10., 30., 60.,
];
const RESULTS: &[f64] = &[0., 1., 5., 10., 25., 50., 100., 250., 500.];
#[derive(Clone, Copy)]
pub enum Kind {
    Counter,
    Gauge,
    Histogram,
}
pub struct Definition {
    pub name: &'static str,
    pub help: &'static str,
    pub kind: Kind,
    pub labels: &'static [&'static str],
}
#[derive(Default)]
struct Series {
    value: f64,
    count: u64,
    buckets: Vec<u64>,
}
/// Borrowed finite profile. The native cap is explicit; Python accepts longer
/// profiles, while the actual fixed registry uses only 13 and 9 boundaries.
#[derive(Clone, Copy)]
struct BucketBounds<'a>(&'a [f64]);
impl<'a> BucketBounds<'a> {
    fn new(values: &'a [f64]) -> Result<Self, &'static str> {
        if values.len() > 64 {
            return Err("native histogram bucket capacity");
        }
        if values.iter().any(|v| !v.is_finite() || *v < 0.)
            || values.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err("Histogram buckets must be finite, nonnegative, and strictly increasing");
        }
        Ok(Self(values))
    }
}
impl Series {
    fn observe(&mut self, bounds: BucketBounds<'_>, value: f64) -> Result<(), &'static str> {
        if !value.is_finite() || value < 0. {
            return Ok(());
        }
        if self.count == 0 && self.buckets.is_empty() {
            self.buckets = vec![0; bounds.0.len()]
        }
        if self.buckets.len() != bounds.0.len() {
            return Err("histogram profile changed");
        }
        self.count = self.count.saturating_add(1);
        self.value += value;
        for (index, boundary) in bounds.0.iter().enumerate() {
            if value <= *boundary {
                self.buckets[index] = self.buckets[index].saturating_add(1)
            }
        }
        Ok(())
    }
}
fn render_header(out: &mut String, name: &str, help: &str, kind: Kind) {
    out.push_str(&format!(
        "# HELP {name} {help}\n# TYPE {name} {}\n",
        match kind {
            Kind::Counter => "counter",
            Kind::Gauge => "gauge",
            Kind::Histogram => "histogram",
        }
    ));
}
fn render_histogram(
    out: &mut String,
    name: &str,
    labels: &str,
    bounds: BucketBounds<'_>,
    state: &Series,
) {
    let prefix = if labels.is_empty() {
        String::new()
    } else {
        format!("{labels},")
    };
    for (boundary, count) in bounds.0.iter().zip(&state.buckets) {
        out.push_str(&format!(
            "{name}_bucket{{{prefix}le=\"{}\"}} {count}\n",
            format_value(*boundary)
        ));
    }
    out.push_str(&format!("{name}_bucket{{{prefix}le=\"+Inf\"}} {}\n{name}_sum{{{labels}}} {}\n{name}_count{{{labels}}} {}\n",state.count,format_value(state.value),state.count));
}

pub struct Metrics {
    enabled: bool,
    internal_events: [AtomicU64; 3],
    engines: BTreeSet<String>,
    categories: BTreeSet<String>,
    series: Mutex<BTreeMap<(usize, Vec<String>), Series>>,
    oldest: Mutex<BTreeMap<String, (std::time::Instant, f64)>>,
}
impl Metrics {
    pub fn new(
        enabled: bool,
        engines: impl IntoIterator<Item = String>,
        categories: impl IntoIterator<Item = String>,
    ) -> Result<Self, &'static str> {
        // Bound input work before collecting, including duplicate catalog entries.
        fn catalog(
            values: impl IntoIterator<Item = String>,
        ) -> Result<BTreeSet<String>, &'static str> {
            let mut catalog = BTreeSet::new();
            for (index, value) in values.into_iter().enumerate() {
                if index >= 256 || value.len() > 128 {
                    return Err("unbounded catalog");
                }
                catalog.insert(value);
            }
            Ok(catalog)
        }
        let engines = catalog(engines)?;
        let categories = catalog(categories)?;
        BucketBounds::new(LATENCY)?;
        BucketBounds::new(RESULTS)?;
        Ok(Self {
            enabled,
            internal_events: std::array::from_fn(|_| AtomicU64::new(0)),
            engines,
            categories,
            series: Mutex::new(BTreeMap::new()),
            oldest: Mutex::new(BTreeMap::new()),
        })
    }
    pub fn update(
        &self,
        name: &str,
        labels: &[(&str, &str)],
        value: f64,
    ) -> Result<(), &'static str> {
        let (index, definition) = DEFINITIONS
            .iter()
            .enumerate()
            .find(|(_, d)| d.name == name)
            .ok_or("unknown metric")?;
        if !value.is_finite()
            || (matches!(definition.kind, Kind::Counter | Kind::Histogram) && value < 0.)
        {
            return Ok(());
        }
        if labels.len() != definition.labels.len() {
            return Err("invalid label set");
        }
        let mut values = Vec::new();
        for label in definition.labels {
            let v = labels
                .iter()
                .find(|(key, _)| key == label)
                .ok_or("missing label")?
                .1;
            if !self.allowed(name, label, v) {
                return Err("invalid label value");
            }
            values.push(v.to_owned());
        }
        if !self.enabled {
            return Ok(());
        }
        let mut all = self.series.lock().unwrap();
        let key = (index, values);
        // Defensive ceiling independent of traffic, even for catalog cross-products.
        if all.len() >= 16384 && !all.contains_key(&key) {
            return Err("metric series capacity");
        }
        let state = all.entry(key).or_default();
        match definition.kind {
            Kind::Counter => state.value += value,
            Kind::Gauge => state.value = value.max(0.),
            Kind::Histogram => state.observe(bounds(definition), value)?,
        }
        Ok(())
    }
    fn allowed(&self, name: &str, key: &str, value: &str) -> bool {
        if key == "outcome" {
            return if name.contains("http_search") {
                ["success", "rejected", "failure"].contains(&value)
            } else if name.contains("event_reads") {
                ["delivered", "empty", "gap", "redacted", "rejected"].contains(&value)
            } else if name.contains("acknowledgements") {
                ["advanced", "idempotent", "rejected"].contains(&value)
            } else {
                [
                    "succeeded",
                    "partial",
                    "failed",
                    "interrupted",
                    "cancelled",
                    "expired",
                ]
                .contains(&value)
            };
        }
        if key == "type" {
            return if name == "slopsearx_cache_hit_total" {
                ["hit", "miss", "negative"].contains(&value)
            } else {
                ["timeout", "circuit_open", "rate_limited", "internal"].contains(&value)
            };
        }

        match key {
            "engine" => self.engines.contains(value),
            "category" => self.categories.contains(value) || value == "general" || value == "other",
            "format" => ["json", "yaml", "html", "csv", "rss", "other"].contains(&value),
            "workflow" => [
                "research",
                "dependency_dossier",
                "staged_search",
                "saved_search",
                "retrieval_receipt",
            ]
            .contains(&value),
            "mode" => ["durable_leased", "durable", "immediate"].contains(&value),
            "reason" => [
                "policy",
                "budget",
                "capacity",
                "idempotency",
                "lease_lost",
                "execution",
                "deadline",
                "manual",
            ]
            .contains(&value),
            "artifact" => ["job", "operation", "definition", "report", "receipt"].contains(&value),
            "outcome" => [
                "success",
                "rejected",
                "failure",
                "succeeded",
                "partial",
                "failed",
                "interrupted",
                "cancelled",
                "expired",
                "delivered",
                "empty",
                "gap",
                "redacted",
                "advanced",
                "idempotent",
            ]
            .contains(&value),
            "event_type" => [
                "report_created",
                "run_incomparable",
                "run_failed",
                "definition_paused",
                "definition_expired",
            ]
            .contains(&value),
            "resource" => ["stream", "consumer"].contains(&value),
            "result" => ["hit", "miss"].contains(&value),
            "status" => [
                "ok",
                "error",
                "timeout",
                "blocked",
                "rate_limited",
                "unavailable",
            ]
            .contains(&value),
            "type" => [
                "timeout",
                "circuit_open",
                "rate_limited",
                "internal",
                "hit",
                "miss",
                "negative",
            ]
            .contains(&value),
            "tool" => TOOL_NAMES.contains(&value),
            "code" => [
                "all_engines_failed",
                "budget_exceeded",
                "cursor_expired",
                "deadline_exceeded",
                "exception",
                "expired_handle",
                "idempotency_conflict",
                "input_conflict",
                "internal_error",
                "invalid_cursor",
                "invalid_input",
                "invalid_intent",
                "invalid_job_id",
                "invalid_job_state",
                "invalid_operation_id",
                "invalid_request",
                "invalid_result_id",
                "invalid_scope",
                "invalid_search_id",
                "job_busy",
                "media_coverage_gap",
                "no_retryable_work",
                "objective_conflict",
                "operation_quota_exceeded",
                "policy_denied",
                "policy_rejected",
                "rate_limited",
                "resource_limit",
                "revision_conflict",
                "safesearch_unenforced",
                "source_expired",
                "source_incomparable",
                "source_not_found",
                "source_partial",
                "source_policy_denied",
                "source_truncated",
                "source_unavailable",
                "store_unavailable",
                "tool_disabled",
                "unavailable",
                "unknown",
                "unsupported_source_contract",
                "unsupported_transition",
            ]
            .contains(&value),
            _ => false,
        }
    }
    /// F14 uses these fixed event names for execution diagnostics, separately
    /// from adapter outcomes and passive upstream health. Unknown names add no series.
    pub fn internal_event(&self, name: &'static str) {
        if !self.enabled {
            return;
        }
        // These are the exact closed callbacks emitted by W05's accepted shared
        // bridge. Counter increments carry no age or tenant/subject/event data.
        let saved = match name {
            "saved_event_published_report_created" => Some((
                "slopsearx_saved_search_event_publications_total",
                "event_type",
                "report_created",
            )),
            "saved_event_published_run_incomparable" => Some((
                "slopsearx_saved_search_event_publications_total",
                "event_type",
                "run_incomparable",
            )),
            "saved_event_published_run_failed" => Some((
                "slopsearx_saved_search_event_publications_total",
                "event_type",
                "run_failed",
            )),
            "saved_event_published_definition_paused" => Some((
                "slopsearx_saved_search_event_publications_total",
                "event_type",
                "definition_paused",
            )),
            "saved_event_published_definition_expired" => Some((
                "slopsearx_saved_search_event_publications_total",
                "event_type",
                "definition_expired",
            )),
            "saved_event_read_delivered" => Some((
                "slopsearx_saved_search_event_reads_total",
                "outcome",
                "delivered",
            )),
            "saved_event_read_empty" => Some((
                "slopsearx_saved_search_event_reads_total",
                "outcome",
                "empty",
            )),
            "saved_event_read_gap" => {
                Some(("slopsearx_saved_search_event_reads_total", "outcome", "gap"))
            }
            "saved_event_read_redacted" => Some((
                "slopsearx_saved_search_event_reads_total",
                "outcome",
                "redacted",
            )),
            "saved_event_read_rejected" => Some((
                "slopsearx_saved_search_event_reads_total",
                "outcome",
                "rejected",
            )),
            "saved_event_ack_advanced" => Some((
                "slopsearx_saved_search_event_acknowledgements_total",
                "outcome",
                "advanced",
            )),
            "saved_event_ack_idempotent" => Some((
                "slopsearx_saved_search_event_acknowledgements_total",
                "outcome",
                "idempotent",
            )),
            "saved_event_ack_rejected" => Some((
                "slopsearx_saved_search_event_acknowledgements_total",
                "outcome",
                "rejected",
            )),
            "saved_event_capacity_stream" => Some((
                "slopsearx_saved_search_event_capacity_rejections_total",
                "resource",
                "stream",
            )),
            "saved_event_capacity_consumer" => Some((
                "slopsearx_saved_search_event_capacity_rejections_total",
                "resource",
                "consumer",
            )),
            _ => None,
        };
        if let Some((metric, label, value)) = saved {
            let _ = self.update(metric, &[(label, value)], 1.);
            if name == "saved_event_read_gap" {
                let _ = self.update("slopsearx_saved_search_event_retention_gaps_total", &[], 1.);
            }
            return;
        }
        let index = match name {
            "search_engine_internal_failure" => 0,
            "search_engine_observed" => 1,
            "search_engine_synthetic" => 2,
            _ => return,
        };
        let _ =
            self.internal_events[index].try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                Some(value.saturating_add(1))
            });
    }
    pub fn accepts_engine(&self, engine: &str) -> bool {
        self.engines.contains(engine)
    }
    pub fn render(&self) -> String {
        let mut out = String::new();
        if !self.enabled {
            return out;
        }
        for (workflow, (started, age)) in self.oldest.lock().unwrap().iter() {
            let _ = self.update(
                "slopsearx_workflow_oldest_claimable_age_seconds",
                &[("workflow", workflow)],
                age + started.elapsed().as_secs_f64(),
            );
        }
        let all = self.series.lock().unwrap();
        for (index, d) in DEFINITIONS.iter().enumerate() {
            render_header(&mut out, d.name, d.help, d.kind);
            let mut ordered = all
                .iter()
                .filter(|((i, _), _)| *i == index)
                .map(|((_, values), state)| {
                    let labels = d
                        .labels
                        .iter()
                        .zip(values)
                        .map(|(k, v)| format!("{k}=\"{}\"", escape(v)))
                        .collect::<Vec<_>>()
                        .join(",");
                    (labels, state)
                })
                .collect::<Vec<_>>();
            ordered.sort_by(|a, b| a.0.cmp(&b.0));
            for (labels, s) in ordered {
                if matches!(d.kind, Kind::Histogram) {
                    render_histogram(&mut out, d.name, &labels, bounds(d), s);
                } else {
                    out += &format!("{}{{{}}} {}\n", d.name, labels, format_value(s.value));
                }
            }
        }
        let counts = self
            .internal_events
            .each_ref()
            .map(|count| count.load(Ordering::Relaxed));
        if counts.iter().any(|count| *count > 0) {
            out += "# HELP slopsearx_search_engine_events_total Native search execution observations; internal failures are not upstream failures\n# TYPE slopsearx_search_engine_events_total counter\n";
            for (event, count) in ["internal_failure", "observed", "synthetic"]
                .into_iter()
                .zip(counts)
            {
                out +=
                    &format!("slopsearx_search_engine_events_total{{event=\"{event}\"}} {count}\n");
            }
        }
        out
    }
    pub fn engine_outcome(&self, engine: &str, response: &AdapterResponse, origin: OutcomeOrigin) {
        if origin == OutcomeOrigin::CallerCancelled {
            return;
        }
        let _ = self.update("slopsearx_engine_queries_total", &[("engine", engine)], 1.);
        let _ = self.update(
            "slopsearx_engine_errors_total",
            &[("engine", engine)],
            if response.status == EngineStatus::Ok {
                0.
            } else {
                1.
            },
        );
        let status = match response.status {
            EngineStatus::Ok => 0.,
            EngineStatus::Timeout | EngineStatus::RateLimited => 1.,
            _ => 2.,
        };
        let _ = self.update("slopsearx_engine_status", &[("engine", engine)], status);
        {
            let ms = &response.latency_ms;
            let _ = self.update(
                "slopsearx_engine_latency_seconds",
                &[("engine", engine)],
                ms.get() / 1000.,
            );
        }
    }
}
// Metrics::new validates both immutable profiles before any series can exist.
fn bounds(d: &Definition) -> BucketBounds<'static> {
    BucketBounds(if d.name == "slopsearx_workflow_admitted_results" {
        RESULTS
    } else {
        LATENCY
    })
}
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('"', "\\\"")
}
pub const DEFINITIONS: &[Definition] = &[
    Definition {
        name: "slopsearx_engine_queries_total",
        help: "Total queries dispatched per engine",
        kind: Kind::Counter,
        labels: &["engine"],
    },
    Definition {
        name: "slopsearx_engine_errors_total",
        help: "Failed engine outcomes, excluding successful empty results",
        kind: Kind::Counter,
        labels: &["engine"],
    },
    Definition {
        name: "slopsearx_engine_latency_seconds",
        help: "Query latency per engine in seconds",
        kind: Kind::Histogram,
        labels: &["engine"],
    },
    Definition {
        name: "slopsearx_engine_status",
        help: "Engine status (0=ok, 1=degraded, 2=down)",
        kind: Kind::Gauge,
        labels: &["engine"],
    },
    Definition {
        name: "slopsearx_cache_hit_total",
        help: "Cache hit/miss counters",
        kind: Kind::Counter,
        labels: &["type"],
    },
    Definition {
        name: "slopsearx_server_requests_total",
        help: "Total search requests handled",
        kind: Kind::Counter,
        labels: &[],
    },
    Definition {
        name: "slopsearx_http_search_completed_total",
        help: "Completed HTTP searches by outcome (success, rejected, failure)",
        kind: Kind::Counter,
        labels: &["outcome"],
    },
    Definition {
        name: "slopsearx_http_search_duration_seconds",
        help: "HTTP search application duration including response rendering, by outcome",
        kind: Kind::Histogram,
        labels: &["outcome"],
    },
    Definition {
        name: "slopsearx_server_requests_by_category_total",
        help: "Search requests per category",
        kind: Kind::Counter,
        labels: &["category"],
    },
    Definition {
        name: "slopsearx_server_requests_by_format_total",
        help: "Search requests per output format",
        kind: Kind::Counter,
        labels: &["format"],
    },
    Definition {
        name: "slopsearx_server_errors_total",
        help: "Server errors by type (timeout, circuit_open, rate_limited, internal)",
        kind: Kind::Counter,
        labels: &["type"],
    },
    Definition {
        name: "slopsearx_mcp_tool_calls_total",
        help: "MCP tool invocations per tool",
        kind: Kind::Counter,
        labels: &["tool"],
    },
    Definition {
        name: "slopsearx_mcp_tool_errors_total",
        help: "MCP tool errors per tool and error code",
        kind: Kind::Counter,
        labels: &["code", "tool"],
    },
    Definition {
        name: "slopsearx_mcp_tool_latency_seconds",
        help: "MCP tool latency per tool in seconds",
        kind: Kind::Histogram,
        labels: &["tool"],
    },
    Definition {
        name: "slopsearx_workflow_accepted_total",
        help: "Accepted durable workflow operations",
        kind: Kind::Counter,
        labels: &["mode", "workflow"],
    },
    Definition {
        name: "slopsearx_workflow_terminal_total",
        help: "Durable workflow terminal outcomes",
        kind: Kind::Counter,
        labels: &["outcome", "workflow"],
    },
    Definition {
        name: "slopsearx_workflow_retries_total",
        help: "Durable workflow retry attempts",
        kind: Kind::Counter,
        labels: &["reason", "workflow"],
    },
    Definition {
        name: "slopsearx_workflow_lease_recoveries_total",
        help: "Expired workflow leases recovered by a worker",
        kind: Kind::Counter,
        labels: &["workflow"],
    },
    Definition {
        name: "slopsearx_workflow_rejections_total",
        help: "Rejected workflow operations by reason class",
        kind: Kind::Counter,
        labels: &["reason", "workflow"],
    },
    Definition {
        name: "slopsearx_workflow_expired_total",
        help: "Observed durable workflow artifact expirations",
        kind: Kind::Counter,
        labels: &["artifact", "workflow"],
    },
    Definition {
        name: "slopsearx_workflow_queued",
        help: "Queued workflow operations visible to this replica",
        kind: Kind::Gauge,
        labels: &["workflow"],
    },
    Definition {
        name: "slopsearx_workflow_running",
        help: "Running workflow operations visible to this replica",
        kind: Kind::Gauge,
        labels: &["workflow"],
    },
    Definition {
        name: "slopsearx_workflow_active_leases",
        help: "Active workflow leases held by this replica",
        kind: Kind::Gauge,
        labels: &["workflow"],
    },
    Definition {
        name: "slopsearx_workflow_oldest_claimable_age_seconds",
        help: "Age of the oldest claimable workflow operation",
        kind: Kind::Gauge,
        labels: &["workflow"],
    },
    Definition {
        name: "slopsearx_workflow_store_items",
        help: "Bounded workflow store or index cardinality",
        kind: Kind::Gauge,
        labels: &["workflow"],
    },
    Definition {
        name: "slopsearx_workflow_queue_wait_seconds",
        help: "Workflow queue wait before claim",
        kind: Kind::Histogram,
        labels: &["workflow"],
    },
    Definition {
        name: "slopsearx_workflow_execution_seconds",
        help: "Workflow execution duration",
        kind: Kind::Histogram,
        labels: &["outcome", "workflow"],
    },
    Definition {
        name: "slopsearx_workflow_report_generation_seconds",
        help: "Saved-search report generation duration",
        kind: Kind::Histogram,
        labels: &["workflow"],
    },
    Definition {
        name: "slopsearx_workflow_admitted_results",
        help: "Results admitted to one durable workflow artifact",
        kind: Kind::Histogram,
        labels: &["workflow"],
    },
    Definition {
        name: "slopsearx_saved_search_event_publications_total",
        help: "Saved-search outbox events published by event type",
        kind: Kind::Counter,
        labels: &["event_type"],
    },
    Definition {
        name: "slopsearx_saved_search_event_reads_total",
        help: "Saved-search outbox read operations by closed outcome",
        kind: Kind::Counter,
        labels: &["outcome"],
    },
    Definition {
        name: "slopsearx_saved_search_event_acknowledgements_total",
        help: "Saved-search outbox acknowledgement operations by closed outcome",
        kind: Kind::Counter,
        labels: &["outcome"],
    },
    Definition {
        name: "slopsearx_saved_search_event_retention_gaps_total",
        help: "Saved-search outbox reads that observed a retention gap",
        kind: Kind::Counter,
        labels: &[],
    },
    Definition {
        name: "slopsearx_saved_search_event_capacity_rejections_total",
        help: "Saved-search outbox operations rejected by bounded resource",
        kind: Kind::Counter,
        labels: &["resource"],
    },
    Definition {
        name: "slopsearx_saved_search_event_backlog_age_seconds",
        help: "Age of the oldest event returned by the most recent local consumer read",
        kind: Kind::Gauge,
        labels: &[],
    },
];
const TOOL_NAMES: &[&str] = &[
    "slopsearx_ack_saved_search_events",
    "slopsearx_cancel_job",
    "slopsearx_create_saved_search",
    "slopsearx_delete_saved_search",
    "slopsearx_explain_search_scope",
    "slopsearx_export_research_manifest",
    "slopsearx_extend_research",
    "slopsearx_get_artifact_lineage",
    "slopsearx_get_dependency_dossier",
    "slopsearx_get_job",
    "slopsearx_get_saved_search",
    "slopsearx_get_service_status",
    "slopsearx_get_staged_search",
    "slopsearx_list_capabilities",
    "slopsearx_pause_saved_search",
    "slopsearx_plan_query_variants",
    "slopsearx_plan_research",
    "slopsearx_plan_research_followup",
    "slopsearx_preview_staged_search",
    "slopsearx_read_entities",
    "slopsearx_read_result",
    "slopsearx_read_results",
    "slopsearx_read_retrieval_receipts",
    "slopsearx_read_saved_search_events",
    "slopsearx_read_saved_search_reports",
    "slopsearx_retry_research",
    "slopsearx_retry_staged_search",
    "slopsearx_search",
    "slopsearx_search_jobs",
    "slopsearx_search_science",
    "slopsearx_search_security",
    "slopsearx_search_staged",
    "slopsearx_search_targeted",
    "slopsearx_start_dependency_dossier",
    "slopsearx_start_research",
    "slopsearx_submit_retrieval_receipt",
    "slopsearx_update_research",
    "slopsearx_update_saved_search",
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_buckets_counts_and_invalid_values() {
        let m = Metrics::new(true, vec!["fixture".into()], Vec::new()).unwrap();
        for _ in 0..100_000 {
            m.update(
                "slopsearx_engine_latency_seconds",
                &[("engine", "fixture")],
                0.02,
            )
            .unwrap();
        }
        for v in [f64::NAN, f64::INFINITY, -1.] {
            m.update(
                "slopsearx_engine_latency_seconds",
                &[("engine", "fixture")],
                v,
            )
            .unwrap();
        }
        let all = m.series.lock().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all.values().next().unwrap().buckets.len(), 13);
        drop(all);
        let rendered = m.render();
        assert!(rendered.contains(
            "slopsearx_engine_latency_seconds_bucket{engine=\"fixture\",le=\"0.025\"} 100000"
        ));
        assert!(rendered.contains("_count{engine=\"fixture\"} 100000"));
    }
    #[test]
    fn traffic_does_not_invent_series() {
        let m = Metrics::new(true, vec!["fixture".into()], vec![]).unwrap();
        for i in 0..10000 {
            assert!(
                m.update(
                    "slopsearx_engine_queries_total",
                    &[("engine", &format!("secret{i}"))],
                    1.
                )
                .is_err()
            );
        }
        assert!(m.series.lock().unwrap().is_empty());
        assert!(!m.render().contains("secret"));
    }
    #[test]
    fn all_metric_definitions_have_unique_identity() {
        assert_eq!(DEFINITIONS.len(), 35);
        let names = DEFINITIONS.iter().map(|d| d.name).collect::<BTreeSet<_>>();
        assert_eq!(names.len(), DEFINITIONS.len());
        let m = Metrics::new(false, vec![], vec![]).unwrap();
        m.update("slopsearx_server_requests_total", &[], 1.)
            .unwrap();
        assert!(m.render().is_empty());
    }
    #[test]
    fn empty_success_is_not_error_and_caller_cancelled_is_excluded() {
        let m = Metrics::new(true, vec!["fixture".into()], vec![]).unwrap();
        let response = AdapterResponse::default();
        m.engine_outcome("fixture", &response, OutcomeOrigin::CallerCancelled);
        assert!(!m.render().contains("engine=\"fixture\""));
        m.engine_outcome("fixture", &response, OutcomeOrigin::AdapterReturned);
        let output = m.render();
        assert!(output.contains("slopsearx_engine_queries_total{engine=\"fixture\"} 1"));
        assert!(output.contains("slopsearx_engine_errors_total{engine=\"fixture\"} 0"));
    }
    #[test]
    fn closed_workflow_and_event_vocabularies() {
        let m = Metrics::new(true, vec![], vec![]).unwrap();
        for kind in [
            "research",
            "dependency_dossier",
            "staged_search",
            "saved_search",
            "retrieval_receipt",
        ] {
            m.update(
                "slopsearx_workflow_accepted_total",
                &[("workflow", kind), ("mode", "durable_leased")],
                1.,
            )
            .unwrap();
            m.update(
                "slopsearx_workflow_terminal_total",
                &[("workflow", kind), ("outcome", "interrupted")],
                1.,
            )
            .unwrap();
        }
        for outcome in ["delivered", "empty", "gap", "redacted", "rejected"] {
            m.update(
                "slopsearx_saved_search_event_reads_total",
                &[("outcome", outcome)],
                1.,
            )
            .unwrap();
        }
        assert!(
            m.update(
                "slopsearx_workflow_terminal_total",
                &[("workflow", "tenant-secret"), ("outcome", "failed")],
                1.
            )
            .is_err()
        );
    }
}
#[cfg(test)]
mod source_replay {
    use super::*;
    #[test]
    fn actual_source_metric_catalog_and_workflow_render() {
        let source: ssx_json::Value =
            ssx_json::from_str(include_str!("../../../xtask/fixtures/o01/source.json")).unwrap();
        let metrics = source["metrics"].as_array().unwrap();
        assert_eq!(metrics.len(), DEFINITIONS.len());
        for metric in metrics {
            let d = DEFINITIONS
                .iter()
                .find(|d| d.name == metric["name"].as_str().unwrap())
                .unwrap();
            assert_eq!(d.help, metric["help"].as_str().unwrap());
            assert_eq!(
                match d.kind {
                    Kind::Counter => "counter",
                    Kind::Gauge => "gauge",
                    Kind::Histogram => "histogram",
                },
                metric["kind"].as_str().unwrap()
            );
        }
        let m = Metrics::new(true, vec![], vec![]).unwrap();
        for workflow in [
            "research",
            "dependency_dossier",
            "staged_search",
            "saved_search",
            "retrieval_receipt",
        ] {
            for (name, labels, value) in [
                (
                    "slopsearx_workflow_accepted_total",
                    vec![("workflow", workflow), ("mode", "durable_leased")],
                    1.,
                ),
                (
                    "slopsearx_workflow_terminal_total",
                    vec![("workflow", workflow), ("outcome", "interrupted")],
                    1.,
                ),
                (
                    "slopsearx_workflow_execution_seconds",
                    vec![("workflow", workflow), ("outcome", "interrupted")],
                    0.5,
                ),
                (
                    "slopsearx_workflow_rejections_total",
                    vec![("workflow", workflow), ("reason", "policy")],
                    1.,
                ),
            ] {
                m.update(name, &labels, value).unwrap();
            }
        }
        for event in [
            "definition_expired",
            "definition_paused",
            "report_created",
            "run_failed",
            "run_incomparable",
        ] {
            m.update(
                "slopsearx_saved_search_event_publications_total",
                &[("event_type", event)],
                1.,
            )
            .unwrap();
        }
        for outcome in ["delivered", "empty", "gap", "redacted", "rejected"] {
            m.update(
                "slopsearx_saved_search_event_reads_total",
                &[("outcome", outcome)],
                1.,
            )
            .unwrap();
        }
        m.update("slopsearx_saved_search_event_retention_gaps_total", &[], 1.)
            .unwrap();
        m.update("slopsearx_saved_search_event_backlog_age_seconds", &[], 3.)
            .unwrap();
        for outcome in ["advanced", "idempotent", "rejected"] {
            m.update(
                "slopsearx_saved_search_event_acknowledgements_total",
                &[("outcome", outcome)],
                1.,
            )
            .unwrap();
        }
        for resource in ["consumer", "stream"] {
            m.update(
                "slopsearx_saved_search_event_capacity_rejections_total",
                &[("resource", resource)],
                1.,
            )
            .unwrap();
        }
        assert_eq!(m.render(), source["rendered"].as_str().unwrap());
    }
}

fn format_value(value: f64) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value == f64::INFINITY {
        return "+Inf".into();
    }
    if value == f64::NEG_INFINITY {
        return "-Inf".into();
    }
    if value == value.trunc() {
        return if value == 0.0 {
            "0".into()
        } else {
            format!("{value:.0}")
        };
    }
    let scientific = format!("{value:.5e}");
    let (mantissa, exponent) = scientific.split_once('e').unwrap();
    let exponent: i32 = exponent.parse().unwrap();
    if !(-4..6).contains(&exponent) {
        return format!(
            "{}e{}{:02}",
            mantissa.trim_end_matches('0').trim_end_matches('.'),
            if exponent < 0 { "-" } else { "+" },
            exponent.abs()
        );
    }
    let decimals = (5 - exponent).max(0) as usize;
    let fixed = format!("{value:.decimals$}");
    if fixed.contains('.') {
        fixed.trim_end_matches('0').trim_end_matches('.').into()
    } else {
        fixed
    }
}
#[cfg(test)]
mod wire_controls {
    use super::*;
    #[test]
    fn actual_source_numbers_and_escaped_series_order() {
        let source: ssx_json::Value =
            ssx_json::from_str(include_str!("../../../xtask/fixtures/o01/source.json")).unwrap();
        for case in source["cases"].as_array().unwrap() {
            if case["kind"] == "format_value" {
                let value = case["input"].as_str().unwrap().parse::<f64>().unwrap();
                assert_eq!(format_value(value), case["render"].as_str().unwrap());
            } else if case["kind"] == "ordering" {
                let values = ["\"", "A", "\\", "B", "\n"].map(str::to_owned);
                let metrics = Metrics::new(true, values.to_vec(), values.to_vec()).unwrap();
                let (name, label) = match case["metric_kind"].as_str().unwrap() {
                    "counter" => ("slopsearx_server_requests_by_category_total", "category"),
                    "gauge" => ("slopsearx_engine_status", "engine"),
                    "histogram" => ("slopsearx_engine_latency_seconds", "engine"),
                    _ => panic!("unknown source metric"),
                };
                for value in &values {
                    metrics.update(name, &[(label, value)], 1. / 3.).unwrap();
                }
                let rendered = metrics.render();
                let section = rendered.split(&format!("# HELP {name} ")).nth(1).unwrap();
                let body = section
                    .split_once('\n')
                    .unwrap()
                    .1
                    .split("# HELP ")
                    .next()
                    .unwrap();
                let actual = format!(
                    "# HELP ordered ordered\n{}",
                    body.replace(name, "ordered")
                        .replace("engine=", "category=")
                );
                assert_eq!(actual, case["render"].as_str().unwrap());
            }
        }
    }
    #[test]
    fn accumulated_histogram_overflow_uses_prometheus_spelling() {
        let m = Metrics::new(true, vec!["fixture".into()], vec![]).unwrap();
        for _ in 0..2 {
            m.update(
                "slopsearx_engine_latency_seconds",
                &[("engine", "fixture")],
                f64::MAX,
            )
            .unwrap();
        }
        assert!(m.render().contains("_sum{engine=\"fixture\"} +Inf"));
    }
}

impl Metrics {
    pub fn workflow_accepted(&self, workflow: &str, mode: &str) -> Result<(), &'static str> {
        self.update(
            "slopsearx_workflow_accepted_total",
            &[("workflow", workflow), ("mode", mode)],
            1.,
        )
    }
    pub fn workflow_terminal(
        &self,
        workflow: &str,
        outcome: &str,
        duration: Option<f64>,
    ) -> Result<(), &'static str> {
        let labels = [("workflow", workflow), ("outcome", outcome)];
        self.update("slopsearx_workflow_terminal_total", &labels, 1.)?;
        if let Some(duration) = duration {
            self.update("slopsearx_workflow_execution_seconds", &labels, duration)?;
        }
        Ok(())
    }
    pub fn workflow_reconcile(
        &self,
        workflow: &str,
        queued: u64,
        running: u64,
        leases: u64,
        oldest_age: f64,
    ) -> Result<(), &'static str> {
        if !self.allowed("", "workflow", workflow) {
            return Err("invalid workflow");
        }
        let mut oldest = self.oldest.lock().unwrap();
        if self.enabled && queued > 0 && oldest_age.is_finite() {
            oldest.insert(
                workflow.into(),
                (std::time::Instant::now(), oldest_age.max(0.)),
            );
        } else {
            oldest.remove(workflow);
        }
        for (name, value) in [
            ("slopsearx_workflow_queued", queued as f64),
            ("slopsearx_workflow_running", running as f64),
            ("slopsearx_workflow_active_leases", leases as f64),
            (
                "slopsearx_workflow_oldest_claimable_age_seconds",
                oldest_age.max(0.),
            ),
        ] {
            self.update(name, &[("workflow", workflow)], value)?;
        }
        Ok(())
    }
    pub fn saved_event_read(
        &self,
        outcome: &str,
        oldest_age: Option<f64>,
    ) -> Result<(), &'static str> {
        self.update(
            "slopsearx_saved_search_event_reads_total",
            &[("outcome", outcome)],
            1.,
        )?;
        if outcome == "gap" {
            self.update("slopsearx_saved_search_event_retention_gaps_total", &[], 1.)?;
        }
        self.saved_event_age(["empty", "gap"].contains(&outcome), oldest_age)
    }
    // Kept separate from counters: the shared W05 bridge already counts reads.
    pub(super) fn saved_event_age(
        &self,
        reset_absent: bool,
        oldest_age: Option<f64>,
    ) -> Result<(), &'static str> {
        if let Some(age) = oldest_age {
            self.update(
                "slopsearx_saved_search_event_backlog_age_seconds",
                &[],
                age.max(0.),
            )?;
        } else if reset_absent {
            self.update("slopsearx_saved_search_event_backlog_age_seconds", &[], 0.)?;
        }
        Ok(())
    }
    pub fn workflow_availability(
        availability: &BTreeMap<&str, bool>,
    ) -> BTreeMap<&'static str, bool> {
        [
            "research",
            "dependency_dossier",
            "staged_search",
            "saved_search",
            "retrieval_receipt",
        ]
        .into_iter()
        .map(|workflow| {
            (
                workflow,
                availability.get(workflow).copied().unwrap_or(false),
            )
        })
        .collect()
    }
}
impl Metrics {
    pub fn workflow_transition(
        &self,
        workflow: &str,
        previous: Option<&str>,
        current: &str,
    ) -> Result<(), &'static str> {
        if !self.allowed("", "workflow", workflow) {
            return Err("invalid workflow");
        }
        if !self.enabled {
            return Ok(());
        }
        let mut oldest = self.oldest.lock().unwrap();
        let mut series = self.series.lock().unwrap();
        let mut change = |name: &str, amount: f64| {
            let index = DEFINITIONS.iter().position(|d| d.name == name).unwrap();
            let state = series.entry((index, vec![workflow.into()])).or_default();
            state.value = (state.value + amount).max(0.);
            state.value
        };
        for (state, name) in [
            ("queued", "slopsearx_workflow_queued"),
            ("running", "slopsearx_workflow_running"),
        ] {
            if previous == Some(state) && current != state {
                let value = change(name, -1.);
                if state == "queued" && value == 0. {
                    oldest.remove(workflow);
                    change(
                        "slopsearx_workflow_oldest_claimable_age_seconds",
                        f64::NEG_INFINITY,
                    );
                }
            }
            if current == state && previous != Some(state) {
                let value = change(name, 1.);
                if state == "queued" && value == 1. {
                    oldest.insert(workflow.into(), (std::time::Instant::now(), 0.));
                }
            }
        }
        if previous == Some("running") && current != "running" {
            change("slopsearx_workflow_active_leases", -1.);
        }
        if current == "running" && previous != Some("running") {
            change("slopsearx_workflow_active_leases", 1.);
        }
        Ok(())
    }
    pub fn workflow_health_summary(availability: &BTreeMap<&str, bool>) -> ssx_json::Value {
        let mut result = ssx_json::Map::new();
        for (workflow, available) in Self::workflow_availability(availability) {
            result.insert(workflow.into(),ssx_json::json!({"available":available,"status":if available{"available"}else{"unavailable"}}));
        }
        ssx_json::Value::Object(result)
    }
}
#[cfg(test)]
mod workflow_helpers {
    use super::*;
    #[test]
    fn workflow_transitions_recovery_and_event_reads_close_stale_gauges() {
        let metrics = Metrics::new(true, vec![], vec![]).unwrap();
        metrics
            .workflow_transition("research", None, "queued")
            .unwrap();
        metrics
            .workflow_transition("research", Some("queued"), "running")
            .unwrap();
        let rendered = metrics.render();
        assert!(rendered.contains("slopsearx_workflow_queued{workflow=\"research\"} 0"));
        assert!(rendered.contains("slopsearx_workflow_running{workflow=\"research\"} 1"));
        assert!(rendered.contains("slopsearx_workflow_active_leases{workflow=\"research\"} 1"));
        metrics
            .workflow_transition("research", Some("running"), "interrupted")
            .unwrap();
        metrics
            .workflow_terminal("research", "interrupted", Some(0.5))
            .unwrap();
        metrics.workflow_reconcile("research", 0, 0, 0, 0.).unwrap();
        assert!(
            metrics.render().contains(
                "slopsearx_workflow_oldest_claimable_age_seconds{workflow=\"research\"} 0"
            )
        );
        metrics.saved_event_read("delivered", Some(3600.)).unwrap();
        for outcome in ["empty", "gap"] {
            metrics.saved_event_read(outcome, None).unwrap();
            assert!(
                metrics
                    .render()
                    .contains("slopsearx_saved_search_event_backlog_age_seconds{} 0")
            );
        }
        assert!(
            metrics
                .workflow_transition("private-tenant", None, "queued")
                .is_err()
        );
        let health = Metrics::workflow_health_summary(&BTreeMap::from([("research", true)]));
        assert_eq!(health["research"]["status"], "available");
        assert_eq!(health["saved_search"]["available"], false);
        assert!(!health.to_string().contains("queued"));
    }
}

#[cfg(test)]
mod catalog_admission {
    use super::*;
    #[test]
    fn catalog_stops_before_unbounded_duplicate_input() {
        let mut observed = 0;
        let values = std::iter::repeat_with(|| {
            observed += 1;
            assert!(
                observed <= 257,
                "catalog consumed past fixed refusal boundary"
            );
            "fixture".to_owned()
        });
        assert!(Metrics::new(true, values, Vec::new()).is_err());
        assert_eq!(observed, 257);
        assert!(Metrics::new(true, vec!["x".repeat(129)], Vec::new()).is_err());
        assert!(Metrics::new(true, vec!["fixture".into(); 256], Vec::new()).is_ok());
    }
}

#[cfg(test)]
mod explicit_state_controls {
    use super::*;
    #[test]
    fn same_length_duplicate_or_missing_labels_refuse_without_retaining_series() {
        for enabled in [false, true] {
            let metrics = Metrics::new(enabled, vec![], vec![]).unwrap();
            for labels in [
                [("mode", "immediate"), ("mode", "durable")],
                [("mode", "immediate"), ("unexpected", "saved_search")],
            ] {
                assert!(
                    metrics
                        .update("slopsearx_workflow_accepted_total", &labels, 1.)
                        .is_err()
                );
                assert!(metrics.series.lock().unwrap().is_empty());
            }
        }
    }
    #[test]
    fn original_gap_some_age_and_zero_queued_positive_age_are_preserved() {
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../../xtask/fixtures/o01/metric-state-source.json"
        ))
        .unwrap();
        let metrics = Metrics::new(true, vec![], vec![]).unwrap();
        let cases = source["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 4);
        for case in &cases[..3] {
            metrics
                .saved_event_read(case["outcome"].as_str().unwrap(), case["age"].as_f64())
                .unwrap();
            let output = metrics.render();
            let age = output
                .lines()
                .find(|l| l.starts_with("slopsearx_saved_search_event_backlog_age_seconds{} "))
                .unwrap()
                .split_whitespace()
                .last()
                .unwrap()
                .parse::<f64>()
                .unwrap();
            let gaps = output
                .lines()
                .find(|l| l.starts_with("slopsearx_saved_search_event_retention_gaps_total{} "))
                .unwrap()
                .split_whitespace()
                .last()
                .unwrap()
                .parse::<f64>()
                .unwrap();
            assert_eq!(age, case["backlog"].as_f64().unwrap());
            assert_eq!(gaps, case["retention_gaps"].as_f64().unwrap());
        }
        metrics
            .workflow_reconcile("saved_search", 1, 1, 1, 42.)
            .unwrap();
        metrics
            .workflow_reconcile("saved_search", 0, 0, 0, 7.)
            .unwrap();
        assert!(!metrics.oldest.lock().unwrap().contains_key("saved_search"));
        for _ in 0..2 {
            let output = metrics.render();
            assert!(output.contains(
                "slopsearx_workflow_oldest_claimable_age_seconds{workflow=\"saved_search\"} 7\n"
            ));
        }
        assert_eq!(cases[3]["oldest"], 7.);
        assert_eq!(cases[3]["refresh_state"], false);
    }
}

#[cfg(test)]
mod bucket_source_controls {
    use super::*;
    #[test]
    fn actual_custom_and_invalid_source_profiles_use_the_production_helpers() {
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../../xtask/fixtures/o01/source-supplement.json"
        ))
        .unwrap();
        let mut completed = 0;
        for case in source["cases"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["id"].as_str().unwrap().starts_with("histogram.buckets."))
        {
            let values = case["input"]["buckets"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().parse::<f64>().unwrap())
                .collect::<Vec<_>>();
            let bounds = BucketBounds::new(&values);
            if case["output"]["error"].is_string() {
                assert_eq!(
                    bounds.err().unwrap(),
                    case["output"]["message"].as_str().unwrap()
                );
            } else {
                let bounds = bounds.unwrap();
                let mut state = Series::default();
                for value in [0., 0.1, 0.2, 0.5, 6.] {
                    state.observe(bounds, value).unwrap();
                }
                let mut output = String::new();
                render_header(&mut output, "custom", "custom histogram", Kind::Histogram);
                render_histogram(&mut output, "custom", "engine=\"fixture\"", bounds, &state);
                assert_eq!(output, case["output"]["render"].as_str().unwrap());
            }
            println!(
                "O01_CASE_COMPLETE:{}",
                ssx_json::json!({"id":case["id"].as_str().unwrap(),"consumer":"actual_custom_and_invalid_source_profiles_use_the_production_helpers","class":"source-bounded-helper"})
            );
            completed += 1;
        }
        assert_eq!(completed, 5);
    }
    #[test]
    fn invalid_observations_retain_no_buckets_and_native_profile_cap_is_explicit() {
        let mut state = Series::default();
        let bounds = BucketBounds::new(LATENCY).unwrap();
        for value in [f64::NAN, f64::INFINITY, -1.] {
            state.observe(bounds, value).unwrap();
        }
        assert_eq!(state.buckets.capacity(), 0);
        assert_eq!(state.count, 0);
        assert_eq!(state.value, 0.);
        let values = (0..65).map(f64::from).collect::<Vec<_>>();
        assert!(BucketBounds::new(&values[..64]).is_ok());
        assert_eq!(
            BucketBounds::new(&values).err(),
            Some("native histogram bucket capacity")
        );
        assert!(BucketBounds::new(&[]).is_ok());
        let mut header = String::new();
        render_header(&mut header, "test", "Test", Kind::Histogram);
        assert_eq!(header, "# HELP test Test\n# TYPE test histogram\n");
    }
    #[test]
    fn empty_bounds_render_and_changed_lengths_refuse_without_mutation() {
        let empty = BucketBounds::new(&[]).unwrap();
        let one = BucketBounds::new(&[1.]).unwrap();
        let two = BucketBounds::new(&[1., 2.]).unwrap();
        let mut empty_state = Series::default();
        empty_state.observe(empty, 0.).unwrap();
        empty_state.observe(empty, 1.).unwrap();
        assert_eq!(empty_state.buckets.capacity(), 0);
        let mut output = String::new();
        render_header(&mut output, "empty", "Empty", Kind::Histogram);
        render_histogram(&mut output, "empty", "", empty, &empty_state);
        assert_eq!(
            output,
            "# HELP empty Empty\n# TYPE empty histogram\nempty_bucket{le=\"+Inf\"} 2\nempty_sum{} 1\nempty_count{} 2\n"
        );
        for (initial, changed) in [(empty, one), (one, empty), (one, two)] {
            let mut state = Series::default();
            state.observe(initial, 1.).unwrap();
            let before = (
                state.count,
                state.value,
                state.buckets.clone(),
                state.buckets.capacity(),
            );
            assert_eq!(state.observe(changed, 2.), Err("histogram profile changed"));
            assert_eq!(
                (
                    state.count,
                    state.value,
                    state.buckets.clone(),
                    state.buckets.capacity()
                ),
                before
            );
        }
    }
}

#[cfg(test)]
#[path = "original_metrics_tests.rs"]
mod original_metrics_tests;
