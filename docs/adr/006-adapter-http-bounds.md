# ADR 006: Bounded adapter HTTP sessions

- Status: Proposed for F08 review; acceptance is the merged F08 PR
- Scope: outbound adapter transport only, not protected identity/gateway clients
- Date: 2026-10-15
- Depends on: accepted F05 and ADR 003/005

## Decision

Preserve adapter-lifetime pools and search-lifetime identity with native reqwest
clients that never own cookies or default auth. Explicitly implement 20 active
connection permits, ten globally retained idle connections, five-second expiry
and eight retained route pools; reqwest's per-host idle setting alone cannot
establish the source's global bound. Use no automatic retries. Provider endpoint
fallback stays in its adapter and retains the aggregate request deadline.

Introduce finite defaults: 8 MiB encoded response, 32 MiB decoded response and
intermediate encoding layer, 2 MiB outbound body, 16 total dispatches per session,
ten redirects per execute call. These are intentional new safety bounds; Python
had no corresponding shared body/request caps. An over-limit exchange returns a
typed limit error rather than partial success or silently truncated content.
Provider batches may choose explicit finite overrides after replay review.
Only gzip and deflate are advertised; raw deflate fallback and stacked known
encodings are supported. Unsupported encoding fails explicitly. Decoding is
bounded and moved off async workers. Four per-adapter permits remain owned by
actual blocking workers through completion, so cancelled waiters cannot create
unbounded abandoned work. Admission closes on shutdown; decoders check stop,
cancellation and deadline every 64 KiB read. Async shutdown_and_wait joins
registered worker completion; synchronous Drop only signals closure. These resource limits do not prove latency or
throughput improvement.

Keep redirects off by default. When explicitly enabled, follow 301/302/303/307/
308 with source method/body rules; strip authorization, explicit cookie,
proxy-authorization and Host on an origin change. This includes an HTTP-to-HTTPS
origin change on the same host, a stricter credential rule than HTTPX's secure
upgrade exception and consistent with ADR 005. Cookie domain/path/secure rules
remain enforced by a session-local jar. Reject non-HTTP(S) destinations.

ADR 005 configured custom roots remain additive. An explicit adapter
SSL_CERT_FILE/SSL_CERT_DIR environment trust store or trust_roots_only option
uses replacement roots to preserve HTTPX semantics. The adapter API preserves
explicit verify=false and client identities as source compatibility settings;
protected identity, OIDC and gateway code must use ADR 005's verified settings
and must not inherit an adapter override. HTTP/2 is opt-in. The existing reqwest
0.13.5 pin is unchanged; its cookies feature adds cookie_store 0.22.1 in the
lockfile. Add exactly pinned flate2 1.1.9 for independently bounded decoding;
transparent reqwest decompression would hide the encoded-byte bound.

## Evidence and limits

F08 source-bound native socket tests prove the named pool, identity, environment,
TLS, deadline, cancellation, encoding, redirect and replay cases. They generate native in-memory
local TLS certificates using exactly pinned test-only rcgen 0.14.10/time 0.3.55
and use no providers. O02/O04 still own platform root/system-proxy
qualification; A01–A14 own actual parsers and provider request sequences. The
new limits and stricter redirects require review before downstream adoption;
this decision does not itself assert complete provider/portal parity.

## Shared native User-Agent amendment (candidate, parent review required)

Outbound AdapterHttp sessions start with the static versioned identity
`SlopSearX/<ssx-infra package version> (meta search engine; agent-native)`.
An explicit request value overrides an explicit session value, which overrides
this default. Each new session constructs only this static default; it never
copies authorization, cookies, or tenant headers from another session or pool.
Redirects retain the selected User-Agent under existing redirect rules.

This intentionally changes the default wire identity from pinned Python HTTPX's
`python-httpx/0.28.1`. It does not claim request-header parity or improved provider
compatibility. Adapters with explicit User-Agent values retain those values.
Default headers remain inside the outstanding F08/G02 backing ownership envelope;
a static identity does not qualify opaque HeaderMap/socket/cookie allocations.
Protected identity/gateway clients do not inherit AdapterHttp policy.
