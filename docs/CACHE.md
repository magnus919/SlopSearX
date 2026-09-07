# Search cache behavior

Search keys use the `search:v2:` namespace and hash an unambiguous JSON tuple
of query and scope inputs. Answer keys use `answer:v2:`. Queries arrive after
transport decoding: literal plus signs, percent escapes, case, punctuation and
interior whitespace remain significant. Only boundary whitespace is stripped.
Older entries expire normally and are never reused under the new semantics.

A failed initial Valkey connection is retried lazily on cache access, at most
once per five seconds per runtime, with one-second connection and socket timeouts.
Concurrent connection attempts are serialized. Failed clients are closed, and
shutdown prevents further reconnection. Once connected, the Valkey client's
connection pool handles later transport reconnections.

Identical concurrent misses within a runtime share one engine fan-out. HTTP
request contexts and MCP searches retain runtime-owned transient coordination;
this is not a distributed lock or a persistent result store. Valkey remains the
shared response cache. Every caller still passes its policy and client rate-limit
checks, receives a separate query ID and mutable response view, and records its
own fresh-search audit event. Include fields and result limits never truncate
the shared canonical response. `prefer_fresh` requests do not join existing work.

Cancelling one caller leaves other waiters running; cancelling the last waiter
cancels and drains the dispatch. Completed flights are removed. No work is shared
across different resolved adapter sets or result-affecting request inputs.

The deterministic concurrency regression sends 20 simultaneous identical
requests with different views and observes one adapter call, separate IDs and
independent results. This demonstrates dispatch reduction under overlap; it is
not a production latency or throughput benchmark.
