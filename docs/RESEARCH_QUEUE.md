# Durable research ready index

Valkey workers discover work through per-tenant sorted sets under `mcp:ready:v1`.
A global sorted set rotates tenants; each candidate reservation moves the tenant
to the back of the rotation. Up to 16 tenants are inspected per poll, so a busy
tenant cannot monopolize the queue and an idle poll has bounded index work.
Tenant names containing colons remain intact. Scoped claims inspect only their
own tenant's index.

The index is derived state. Existing job records, cancellation flags, and the
unique lease token remain authoritative. Saving a job refreshes its index entry
with Lua that reads the current persisted record and actual lease TTL. Terminal
or missing records are removed; queued and running records become visible when
no live lease exists. Renewal delays visibility; release makes unfinished work
visible again. A stale worker cannot save, renew, or release another owner's
lease. Direct retry/extend continue through the existing claim lifecycle.

Candidate reservation is non-destructive with a five-second visibility delay.
A worker dying between reservation and lease acquisition therefore leaves
recoverable work. A worker dying after acquiring its lease is recoverable after
lease expiry. Saving the record and refreshing the index are separate operations;
a crash in that gap is repaired by reconciliation, including for legacy records.

Reconciliation is coordinated by a shared ten-second throttle. Each interval
processes one SCAN page with COUNT 128 and stores its cursor after refreshing the
page. COUNT is a Valkey hint, not a strict returned-key bound. A crash repeats a
page safely; a full sweep can take multiple intervals and grows with database
size. Normal indexed jobs do not wait for the sweep. No recurring worker poll
loads every retained completed job. Startup stale-job cleanup and explicit legacy
store/test seams retain their existing scans. Index corruption or deletion is
repairable by a sweep; queue discovery requires Valkey availability.

## Evidence

Run against an explicitly designated disposable/test Valkey instance:

```sh
SLOPSEARX_TEST_VALKEY_URL=redis://localhost:6379/15 \
  python -m pytest tests/test_research_queue_integration.py --no-cov -q -s
```

Tests use random prefixes and delete only their own keys. They cover competing
workers, lease renewal, orphan recovery, stale-token fencing, terminal cleanup,
tenant rotation, cancellation, save/index gaps, and reservation crashes.

The operation-count experiment retains 1,000 completed jobs. After reconciliation
is throttled, 20 idle indexed polls issue 20 SET and 20 EVAL client commands and
zero job-record reads. The legacy scan path on the same history reads 1,000 job
records plus 1,000 cancellation keys in one idle poll, in addition to SCAN calls.
Lua commands execute inside Valkey and are not counted as separate client calls;
an empty ready index exits immediately. These are controlled operation counts,
not a latency or throughput claim under production load. Reconciliation work is
explicitly outside the normal-poll measurement and remains proportional to the
keyspace over a complete sweep.
