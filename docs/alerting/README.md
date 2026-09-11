# Prometheus monitoring

`/metrics` serves Prometheus text format 0.0.4. Latency metrics now expose
cumulative `_bucket{le="..."}`, `_sum`, and `_count` series instead of the old
malformed client-side quantiles. Update dashboards using `quantile` labels to:

```promql
histogram_quantile(0.95, rate(slopsearx_engine_latency_seconds_bucket[5m]))
```

Each engine/tool label set retains 13 bucket counters, one total, and one count,
regardless of request volume. Quantiles are estimates within bucket boundaries;
5 seconds is an explicit boundary for the latency alert. To aggregate replicas,
sum bucket rates by the labels you want to retain, including `le`.

`slopsearx_engine_errors_total` counts all non-OK, non-empty outcomes (including
blocked, timeout, and rate-limited). Its denominator is
`slopsearx_engine_queries_total`; both are updated at the same outcome boundary.
These are service dispatch outcomes, not a count of physical upstream attempts;
cached responses do not dispatch engines. Successful empty results are not errors.

Metrics remain process-local; scrape each replica/process rather than treating
one endpoint as a shared cluster total. User-supplied format labels are limited
to `json`, `yaml`, or `other`. Categories are limited to those declared by active
engines plus `other`, counted once per request; unknown input cannot accumulate
unbounded metric label sets. Labels are escaped without changing known names.

Validate syntax and firing behavior with the official Prometheus tool:

```sh
promtool check rules docs/alerting/rules.yml
promtool test rules docs/alerting/rules.test.yml
```

CI runs these checks with Prometheus 3.2.1. Tests cover sustained errors,
healthy traffic, zero traffic, counter resets, sustained slow responses, and
healthy latency. Python tests also parse the actual emitted exposition using
`prometheus-client`, a development-only dependency.

## Durable workflow signals

Research, dependency-dossier, staged-search, saved-search, and retrieval-receipt
workflows expose a shared closed-label metric family. Labels are limited to
documented workflow kinds, lifecycle outcomes, execution modes, rejection
classes, and artifact classes. Tenant names, queries, engine lists, object IDs,
URLs, credentials, and exception text never become labels.

Terminal outcomes are `succeeded`, `partial`, `failed`, `interrupted`,
`cancelled`, and `expired`. Lease recovery records `interrupted` when an
attempt is durably classified that way; failure alerts therefore exclude
worker-loss interruptions and rely on the separate lease-recovery signal.

Counters cover accepted work, terminal outcomes, retries, lease recovery,
rejections, and observed artifact expiry. Gauges cover queued/running work,
active leases, oldest claimable age, and bounded index cardinality. Histograms
cover queue wait, execution, saved-report generation, and admitted result
counts. Metrics are process-local, as are the existing engine metrics; sum
counters across replicas and use `max` for oldest-age alerts. Collection only
renders in-memory observations and never scans or mutates Valkey.

The tenant-facing service-status response contains only `available` and a
closed store state (`available` or `unavailable`) for each workflow. It never
reads the process-global activity gauges, avoiding a cross-tenant workload
signal, and intentionally omits counts and tenant identifiers.

The workflow rules are actionable as follows:

- `WorkflowQueueStuck`: inspect worker logs, store connectivity, and the oldest
  claimable age. Restore workers before retrying individual operations.
- `WorkflowLeaseRecoveryBurst`: inspect restarts, event-loop stalls, and lease
  renewal failures. A recovery is safe because writes remain token-fenced.
- `WorkflowTerminalFailures`: inspect terminal reason counters and structured
  workflow responses. Policy and capacity rejections have their own counters
  and should be fixed at configuration or capacity boundaries.

Deploy the metrics before enabling these rules. Verify scrape size and series
cardinality under expected load. Rollback consists of removing the collectors
and rules; no workflow records or Valkey keys need migration.

### Saved-search event outbox

The `slopsearx_saved_search_event_*` series cover publications by closed event
type, reads and acknowledgements by closed outcome, retention gaps, capacity
rejections by `stream` or `consumer`, and the age of the oldest event returned
by the most recent local consumer read. They never label a tenant, consumer,
query, search, run, or event. A capacity increase requires sustained event-rate
evidence; acknowledgement cannot extend retention, so a retention-gap increase
usually means the consumer must reconcile and resume from current state.
