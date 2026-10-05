# Production-path timing audit

Source inspected at `8544c90032281d32c1a0a2f6790fd84730b5c525`. This is a read-only code audit with no provider calls, measurements or quality analysis.

`slopsearx/service.py:1037-1060` groups scholarly results, performs ordinary ranking, checks selected-engine and result provenance for sensitive sources, then invokes reranking. Ordinary noninteractive reranking receives its own one-second allowance after dispatch; interactive requests receive the lesser of one second and the remaining dispatch budget. This does not authorize a longer budget for the two-request EXP-073 candidate.

`SearchService._rerank_results` projects the first 40 candidates before its `asyncio.wait_for` call. The provider coroutine runs within that outer wait. Service membership validation and reordering happen after the await; the remaining tail is appended unchanged. This is the shipped ordinary path, not a caller-purpose complete-pool selector.

`slopsearx/rerank.py:124-242` performs request validation and serialization, then enters its internal `asyncio.timeout` with the provider semaphore. It creates an `httpx.AsyncClient` for each invocation, streams a bounded response and closes the client. JSON decoding, usage handling, answer validation and ordering remain **inside** the timeout context, though outside the client context. Cancellation is cooperative; lexical scope alone does not prove a strict interruption of synchronous CPU work. There is no subprocess and no shared HTTP client.

EXP-073 launches two separate wrapper processes. Its single combined clock includes pending call receipts, both subprocess lifecycles and exchanges, response validation, exact fusion, and final file/directory synchronization. The endpoint is measured after durability. Its failed capacity check cannot be reinterpreted as an isolated provider-latency result or as failure of the shipped single-call path.

The next zero-call diagnosis must separate wrapper startup/local exchange/validation/durability from simulated network time, using identical frozen request bodies. It may justify a separately registered deployable-path comparison; it cannot subtract overhead from EXP-073, establish real provider latency or ranking uplift, or satisfy untouched confirmation. The original inputs, full-pool objective, shared deadline, resource bounds, exact membership and fallback protections remain requirements for any comparable candidate.
