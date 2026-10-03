# SlopSearX service-level objectives

Status: draft, 2026-10-03. Service owner: Magnus. No numerical objectives or
production error-budget policies are approved by this document. No production
baseline was available during the one-off code audit. Targets remain unset.

## User journeys and proposed indicators

| Journey | Eligible event | Good event | Baseline / target |
| --- | --- | --- | --- |
| HTTP search response availability | Search attempt reaching the application, excluding expected input/policy rejection | Search response completes without service or capacity failure | Collection proposed; target unset |
| HTTP search responsiveness | Same eligible population | Successful response generated within a selected deadline | Deadline and target unset; histogram buckets constrain exact thresholds |
| MCP search response availability and responsiveness | Authorized, valid search-tool attempt | Contract-valid successful completion within a selected deadline | Audit required: tool metrics are not yet a complete journey SLI |
| Durable research completion | Accepted operation, with explicit cancellation policy | Retrievable contract-valid terminal artifact within deadline, including queue wait | Cohort measurement and deadlines required |
| Evidence integrity and task quality | Fixed representative tasks with independent scoring | Correct provenance/policy and sufficient relevant evidence to finish the task | Benchmark required; passing availability is not task success |

Proposed reporting window: 28 days rolling, evaluated daily. Show sample count,
coverage and missing-data status. Zero observations do not mean 100% success.
Choose thresholds from observed distributions and user tolerances before enabling
budget-based prioritization. Do not copy generic availability targets.

## HTTP instrumentation boundary

`slopsearx_http_search_completed_total{outcome}` observes `/search` and search
requests to `/`, for GET and POST, including early validation returns. A successful
GET landing page without a query is excluded. Health, metrics and other routes
are excluded. Labels are the closed vocabulary `success`, `rejected`, `failure`:

- success: returned status 200–399. In the present search path this includes
  partial results and legitimate empty results. It does not certify relevance.
- rejected: returned status 400–499 except 429. Track separately; do not count
  as good events. Audit rejection rates for service-caused or surprising rejections.
- failure: status 429 or 500+, or unhandled exception/cancellation. Capacity
  rejection consumes budget. Total upstream failure is 503 and consumes budget.

`slopsearx_http_search_duration_seconds{outcome}` observes the same attempt from
entry into the endpoint wrapper through response generation, including validation,
cache/dispatch and rendering. It stops before network transmission or browser
rendering. Cancellation is counted as failure conservatively; application telemetry
cannot distinguish intentional abandonment from a network/service fault.

For a populated interval, the proposed application response-availability SLI is:

```promql
sum(increase(slopsearx_http_search_completed_total{outcome="success"}[28d]))
/
sum(increase(slopsearx_http_search_completed_total{outcome=~"success|failure"}[28d]))
```

Use per-instance `increase` before aggregation; counters are replica-local and
reset on restart. A latency SLI uses successful observations at a chosen finite
histogram bucket divided by all eligible attempts (success plus failure), so fast
failures cannot improve it. Do not average replica percentiles. Cache-hit and
uncached cohorts are not yet separated by these metrics.

## Gaps before approving production SLOs

1. External/client evidence covering DNS, TLS, proxy failure, response transfer,
   and application downtime; reconcile with application observations.
2. Representative traffic, sample counts, scrape coverage, cache mix and query
   families. Collect without raw queries, tenant IDs or secrets as metric labels.
3. MCP application-result classification and async accepted-job cohorts. Do not
   divide workflow terminal counts by acceptance counts from mismatched cohorts.
4. Independently scored task benchmark; result count is not useful evidence.
5. Owner-reviewed targets, deadlines and budget policy. Define urgent actionable
   burn alerts separately from daily improvement scheduling; one daily run is not
   an incident response mechanism.

Once approved, the daily improvement runner should prioritize consequential
budget-consuming failure classes, or task-quality gaps when reliability is healthy.
A candidate needs a reproduced failure, bounded scope, frozen practical acceptance
criteria, normal checks/review, delivery tracking and post-release observation.
Supported replay is not proof of improved production SLOs. No automated production
mitigation, rollout, rollback, alert routing or schedule change is authorized here.
