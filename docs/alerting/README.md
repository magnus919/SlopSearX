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
