# Staged search

Staged search gives agents a bounded way to try a precise source scope and
expand to a disjoint fallback scope only when the first stage returns a clean,
fully observed empty result. It is an additive MCP contract; the SearXNG HTTP
routes and response format are unchanged.

Enable it with `MCP_GRANT_STAGED_SEARCH=1`. It also requires connected Valkey.
Operators can lower the hard limits with `MCP_STAGED_MAX_DEADLINE_MS` and
`MCP_STAGED_MAX_ENGINE_CALLS` (defaults: 30,000 ms and 64 adapter calls).

The workflow exposes four tools:

- `slopsearx_preview_staged_search` resolves and validates a plan without
  persistence, suggestion work, or engine dispatch.
- `slopsearx_search_staged` accepts an idempotent operation and queues it for
  asynchronous execution.
- `slopsearx_get_staged_search` reads the operation and its selected immutable
  result snapshot.
- `slopsearx_retry_staged_search` retries only the latest failed stage, under
  the original deadline, scope, and adapter-call budget.

`objectives` must contain `deadline_ms` and `max_engine_calls`. Each scope must
contain exactly one of `engines` or `intent`; an optional fallback requires
`allow_scope_expansion=true`, and its resolved engines must be disjoint from
the initial scope. The fallback condition is fixed to `clean_empty`. Partial,
timed-out, or failed engine coverage never qualifies as a clean empty result.

The budget unit is one adapter `search` invocation. The service reserves the
full stage scope before dispatch and reports reserved and observed calls
separately. This does not claim to count upstream redirects or internal HTTP
requests made by an adapter.

Operations remain logically readable for one hour and are tenant isolated.
Search snapshots have their own retention horizon. If a selected snapshot has
expired, operation provenance remains available while result cards report an
expired handle. Current engine and sensitive-source policy is checked again on
reads and dispatch, so revoked scopes fail closed without revealing results.

Presentation controls (`include` and `max_results`) do not affect the plan
digest, stored snapshot, fallback decision, or adapter budget. Staged searches
disable suggestions explicitly. Their isolation identifier affects only
active in-flight ownership and never changes the completed search cache key.
