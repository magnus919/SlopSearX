# A03 developer and community adapters

Issue [#923](https://github.com/magnus919/SlopSearX/issues/923) implements
GitHub, Hugging Face, Stack Exchange and Hacker News using the accepted A01
`SearchEngine` port, F06 descriptors, F08 HTTP sessions and F10 admission.
The prerequisite is accepted A01 [handoff](https://github.com/magnus919/SlopSearX/issues/917#issuecomment-6212053625).
The source baseline is `35d97ccf8c9b2d8246373b2024cc8d7a457705a3`.

`register_community` attaches four engine-local factories to the existing
registry. It leaves all 51 capability descriptors, categories, failure classes,
Jev routing cards and A01 factories unchanged. No orchestration edit is needed.
The caller retains `Arc<AdapterHttp>` and `Arc<dyn SearchAdmission>` ownership.
Use the accepted `F10Admission` bridge with the shared limiter and its public
`f10_monotonic_seconds()` epoch for composition-set cooldowns. An explicit
`AllowAllAdmission` configuration remains available; adapters do not discover
credentials, environment variables or state URLs themselves.

`CommunityConfig` contains four engine-specific settings wrappers. Defaults
match each source adapter. A composition root can call the pure
`CommunitySettings::resolve` with its resolved URL, timeout, integer value and
optional `ssx_domain::oauth::Secret`. This is a numeric/type boundary, not a
second YAML/env loader. Timeout milliseconds may be fractional or zero and must
be finite, nonnegative and representable by `Duration` and an absolute monotonic
deadline. Max results must be an exact nonnegative integer representable as
`usize`, including zero; integral float lexemes are refused. Direct typed
construction is rechecked for timeout validity before transport use. Secrets
retain the accepted redacted Debug contract and native 4096-byte bound. GitHub
and Stack Exchange strip Python whitespace from keys; Hugging Face intentionally
preserves whitespace keys. None of these adapters requires a key for its public
search modes; GitHub code search alone refuses missing/stripped-empty tokens.

Each search obtains admission before creating a request. Sessions use accepted
F08 TLS/proxy/environment settings and default redirect refusal. Dropping the
search future cancels the live request; adapters never close the shared owner
or retry an unknown request outcome themselves. As in A01, this port has no
remaining-budget context: configured provider timeout is not a claim about the
whole orchestration deadline. F14 owns that integration. F10 configured/observed
health remains separate from the actual search outcome; no synthetic provider
health probe is introduced.

## Source contracts

| Engine | Request and parse contract |
| --- | --- |
| GitHub | Code categories win over issues/PRs; all other categories use repositories. Full endpoint substring checks retain custom-base parser/auth behavior. GET uses `q`, `per_page`, page 1, v3 Accept and engine-owned UA, optional Bearer header. 403 rate-limit text, other 403, 401 and 422 remain distinct. Code fallback URLs and ordered repository/path payload provenance are preserved. |
| Hugging Face | Datasets win over papers, otherwise models. The complete endpoint controls the parser, including custom-base oddities. GET uses search/limit and its UA, optional unstripped Bearer header. Models, datasets and papers retain distinct URLs, content, authors, scores and categories. 429 alone is rate_limited. |
| Stack Exchange | First matching site category wins. Query uses Python quote semantics (slash stays safe; spaces become `%20`) and literal URL assembly, retaining valid custom-base query oddities. Key is only `X-API-Key`; no invented engine UA or query key. 400 is rate_limited, all other failures including timeout are error. Slice precedes row inspection. Unix timestamps accept actual numbers/bools, not numeric strings. |
| Hacker News | GET uses query, hitsPerPage and tags=story with its UA. Missing/falsey story URL falls back to the HN item; comments are a content count, not a comment-search mode. All non-success statuses, including 429, are error. |

GitHub, HF and HN do not locally slice provider rows, even for configured zero;
Stack Exchange does. Request paging, max_results, filters and media flags never
override those source rules. No language, SafeSearch, time-range or date
enforcement is invented. All results remain text; structured GitHub payloads
round-trip through the accepted F05 storage codec in actual replay assertions.

## Evidence and handoff

`cases.json` has 232 actual unmodified Python adapter observations captured
offline with injected MockTransport. The original 218 observations were
independently recaptured byte-exact; fourteen appended cases retain every original
row unchanged. MockTransport request/header records are provenance and are not
live provider-wire certification. Latency is excluded; raw exceptions and
invalid typed result values remain in the source observations. No output is
rewritten to match Rust.

`coverage.json` maps all 45 named-engine source regression functions, including
`tests/test_github.py`, parameter variants, shared adapter/tier tests, F06's HF
alias and accepted capability/intent ownership. The native gate runs eight
integration tests, the fractional-timeout and slice-order unit tests, F06's named alias test and
three gate mutation negatives. Exact named inventories, passed counts and zero
ignored tests are required. Of the 232 source rows, 196 execute through actual
F08 loopback sockets; 28 invalid numeric configs use the resolver, four
cancellations use real pending sockets and four zero deadlines use the explicit
operation boundary. Fractional timeout source mocks are not socket-timing proof;
a unit assertion proves 1.5ms becomes 1500µs exactly.

Private Valkey testing starts a new process/socket/directory, never accepts a
shared store URL and never flushes. All four adapters admit exactly one actual
HTTP request, conservatively charge the denied second quota acquisition and
make no third acquisition under provider cooldown. Cancellation asserts peer
EOF, exactly one request, no retry, and continued caller ownership. Metadata
comparison retains all accepted F06/Jev declarations. `cargo xtask check A03`
executes no Python; the temporary exporter is migration-only evidence retired
under G08. Both native CI architectures run this gate and retain its report.

Portal impact: these factories add backend implementations without changing
browser routes, request/result schemas, capability declarations or enforcement.
Codec replay checks payload/default/media fields used by the portal. Existing
portal gates stay intact; production composition and browser journey activation
remain with G01/T02 and their acceptance gates. This package does not claim
live-provider health, relevance, performance or production adoption.

See [PARITY.md](PARITY.md) for explicit source divergences and downstream limits.

Independent source review corrected four GitHub iterable cases: topic strings
join their characters, topic objects join their insertion-ordered keys, and
label strings/object keys are discarded by the source dict-only label filter.
A fifth actual-source case confirms nonempty Stack Exchange string collections
slice to empty at max_results=0; mappings/scalars remain unsliceable errors.
All previous 227 observation rows remain byte-equivalent as full JSON values.
