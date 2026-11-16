# F15 parity and finite boundaries

The independently recaptured 150 cases bind 13 original source files. The initial 124 source objects remain exact; additions cover Python-supported encodings, unused deep structures and 14 decimal/IEEE conversion observations. A separate 12-case actual source capture binds early header admission and body-read counts. Source failure observations remain in the corpus even where safe native behavior differs.

| Boundary | Native behavior and evidence |
| --- | --- |
| Provider authority | Only an exact, unique permutation of known IDs can change order. Output cannot supply result text, provenance, metadata or URLs. Original complete records are compared in host replay. |
| Candidate privacy | First 40 candidates; bounded title256/URL512/snippet1200/query4096 UTF-8 bytes; URLs omit credentials, query and fragment. Provider usage contains aggregate counts only. |
| Missing/error provider | Original fallback remains; actual malformed response retries on the next search because no canonical cache entry is written. Provider absence retains configured deterministic ranking. |
| Native resource bounds | Additional 8MiB encoded wire ceiling and flat cooperative parser are deliberate finite native protections. The decoded limit and queue-inclusive one-second provider deadline match source. |
| Recursion | Pinned CPython accepted 110,000 unused array levels and failed at 120,000 in this environment. Flat native parsing safely accepts the deeper case; that source failure is retained as a named delta, not a portable Python recursion contract. |
| Unicode | Strict relevant member/ID boundaries require native Unicode strings; unused source surrogate strings remain represented without coercion into public candidate IDs. URL and text projection preserve source observations, including IPv6 zone spelling and Python integer port limit. |
| Duplicate/nonfinite JSON | Duplicate decoded keys at every depth and nonfinite numbers reject the whole decision. No partial answer salvage or new IDs. |
| Decimal conversion | Shared `FloatNormalizer` validates strict JSON numeric grammar in bounded steps and retains all significant digits before IEEE conversion. `Number` and F15 reject nonfinite values; SourceJSON keeps its pre-existing nonfinite representation policy. Source-success long decimals previously misparsed by std conversion now match pinned Python bits. |
| Numeric scheduling | F15 and SourceJSON scan/normalize cooperatively and check deadlines before/after final conversion. Flat cancellation cleanup is joined; no detached parser work. Scheduling probes are correctness observations, not throughput benchmarks. |
| Consumer scope | This fix covers raw JSON numbers in `ssx-json::Number`, SourceJSON float tokens and F15 scores. `ssx-domain::storage::parse_value_f64` string coercion and A06 Semantic Scholar quoted Python-float lexical normalization remain separately owned. Do not claim all Python float coercions are fixed. |
| State | No cache/snapshot schema or durable state rewrite. Concrete native host/provider identity intentionally creates a cold cache namespace; old ephemeral keys expire under their existing TTL. Read-boundary advisories never enter stored canonical/snapshot objects. |
| Surface successors | T02 HTTP, T03 portal and T06 MCP own their native public formatting/layout/escaping and live catalog wiring. F15 qualifies the shared native data/caller-view contract; historical source surface assertions do not establish completed successor transports. |

All source captures are developer-only with synthetic private data and mock transports. Real native socket tests use loopback; Valkey tests use a disposable private Unix socket. No live provider, production state, scientific/relevance comparison or deployment was exercised.
