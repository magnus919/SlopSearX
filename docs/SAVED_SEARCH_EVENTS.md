# Saved-search event outbox

The saved-search event outbox lets an MCP client consume saved-search changes
incrementally. It is a tenant-scoped Valkey stream. SlopSearX does not accept a
callback URL and never sends an event to an external service. A client reads
and acknowledges events through MCP.

Enable saved searches with `MCP_GRANT_SAVED_SEARCHES=1`. Enable publication and
the two event tools separately with `MCP_GRANT_SAVED_SEARCH_EVENTS=1`. Deploy
the reader contract before enabling the event grant. Disabling that grant
stops new publications and reads; saved-search reports continue to run and
remain available through the existing report tool.

## Version 1 contract

Each `slopsearx.saved_search_event` version 1 record contains:

- an opaque `event_id` for consumer deduplication and a tenant-monotonic
  Valkey stream `cursor`;
- `event_type`: `report_created`, `run_incomparable`, `run_failed`,
  `definition_paused`, or `definition_expired`;
- the saved-search and run artifact references, plus their compatibility IDs;
- `occurred_at`, closed report summary counts, `coverage_state`, and a closed
  `reason_code`.

Events contain no result body, query, URL, credential, callback, or secret.
Ordering is guaranteed within one tenant stream. Delivery is at least once, so
consumers must deduplicate by `event_id`.

## Read and acknowledge

Call `slopsearx_read_saved_search_events` with a caller-selected `consumer_id`,
an optional cursor, and a batch `limit` from 1 through 100. If no cursor is
provided, the reader starts after that consumer's last acknowledgement. Reads
do not change acknowledgement state. A restart can therefore repeat the read
and process the same event safely.

After durable downstream processing, call
`slopsearx_ack_saved_search_events(consumer_id, cursor)`. Acknowledgement is
idempotent and monotonic. An older or repeated cursor leaves the current fence
unchanged. A cursor ahead of the tenant stream is rejected. Consumer IDs are
bounded safe identifiers and their state is fenced by the authenticated
tenant.

The stream is bounded by `saved_event_capacity` and
`saved_event_retention_seconds`. A slow consumer never extends retention. When
a requested cursor precedes the first retained item, the read returns
`gap.detected=true`, `reason=retention_expired`, and the first available cursor.
The consumer must reconcile from saved-search state and reports before moving
its acknowledgement forward. Acknowledging an expired cursor is rejected. If
the entire stream expired, the same gap has a null first-available cursor.

New report publication is part of the existing lease/revision Lua commit. With
events enabled, the report and event are accepted together or neither is
written. A full stream rejects the commit as capacity pressure; the run can be
retried after retention frees space. Pause transitions use the same atomic
state/event rule. Expiry publication is idempotent while the definition's
bounded storage-margin record remains.

Current policy is checked again at read time. If the saved-search grant or a
sensitive-engine grant was revoked, SlopSearX returns the event identity and a
redacted state explanation. Summary and coverage detail are withheld. Retained
events do not preserve access that current policy denies.

`/metrics` exposes publication, read, acknowledgement, retention-gap, capacity
rejection, and backlog-age series under the
`slopsearx_saved_search_event_*` prefix. Labels use only closed event,
operation-outcome, and resource vocabularies. Tenant, consumer, query, search,
run, and event identities are never metric labels.

## Configuration

```yaml
mcp:
  enabled_tools:
    saved_searches: false
    saved_search_events: false
  saved_event_capacity: 1000
  saved_event_retention_seconds: 604800
  saved_event_max_consumers: 100
```

Environment equivalents are `MCP_GRANT_SAVED_SEARCH_EVENTS`,
`MCP_SAVED_EVENT_CAPACITY`, `MCP_SAVED_EVENT_RETENTION_SECONDS`, and
`MCP_SAVED_EVENT_MAX_CONSUMERS`.

Rollback by disabling `MCP_GRANT_SAVED_SEARCH_EVENTS`. Existing bounded stream
and acknowledgement keys expire normally. No SearXNG HTTP route, parameter, or
response field depends on the grant.
