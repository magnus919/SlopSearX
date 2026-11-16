# F08 adapter HTTP handoff

Issue [#887](https://github.com/magnus919/SlopSearX/issues/887), source baseline
`7cc420f7ffb61127750e77bf6130e225925f9186` after accepted F05 PR #960.
`coverage.json` pins the source blobs and finite native case inventory. These are
synthetic protocol regressions derived from actual source contracts, not
sanitized provider captures or complete adapter parity.

## Stable interface

`ssx_infra::http::AdapterHttp` belongs to one adapter's lifetime. Create one
`Session` per search/tenant with an absolute Tokio monotonic deadline and a
`Cancellation`. Sessions own cookies, default headers and request counters;
requests own method, URL, headers and body. The adapter owns shared clients,
which have no cookie store or default authorization. A session can execute
multiple requests with cookie continuity and one aggregate deadline, including
queueing, headers, body streaming, decoding and redirects. Redirects preserve HTTPX method/body rules, including 302 non-HEAD
method selection and existing GET/HEAD streams; rewritten requests retain
Content-Type, drop body framing and rebuild cookies from the session jar. HTTP status, duplicate
headers and body bytes reach the adapter; 429 remains an ordinary response and
provider-specific status/challenge/parsing classification remains adapter-owned.
No exception or Rust error string containing upstream credentials is synthesized.

Each adapter shares 20 active connection permits and ten idle-retention permits
across every retained route and ephemeral pool. Idle origin-specific clients
have five-second expiry. Each client retains at most one connection
for its one origin. A checkout reserves one idle slot; concurrent overflow
clients are dropped after the request. The first eight route keys (including a
direct key, as in the source dictionary) are retained. Additional explicit
proxies get session-lifetime pools. Explicit TLS/protocol/trust overrides and
environment-proxy sessions also get session-lifetime pools. This avoids an
unbounded retained matrix of TLS, proxy and origin configurations. TLS verifies
by default; HTTP/2 and following redirects default off. Explicit custom roots,
client identity, verification and HTTP/2 are session options, never applied to
an already-built pool.

Environment HTTP_PROXY/HTTPS_PROXY/ALL_PROXY and NO_PROXY resolution is delegated
to the pinned reqwest implementation; a subprocess test observes a real local
proxy and direct bypass. Lowercase variables are also recognized. Explicit
proxy routing takes precedence. SSL_CERT_FILE/SSL_CERT_DIR are loaded when
verification and trust_env are enabled, using replacement roots as HTTPX does;
explicit configured roots are additive per ADR 005 unless trust_roots_only is
explicit. Empty roots-only configuration and invalid CA inputs fail closed,
including after an existing default pool has been used. Platform system proxy and trust-root
behavior still require O02/O04 target qualification; macOS native tests are not
Linux/Windows platform evidence.

`set_transport(Arc<dyn Transport>)` is allowed before first use. It borrows the
caller-owned transport and never closes it. `Replay` checks method, URL, headers
and body against ordered exchanges, rejecting extra or changed requests. Payloads
remain byte-exact, including malformed HTML/XML/JSON; provider parsers are the
later A01–A14 owners. Library-added wire headers (Host, content framing, default
Accept and Accept-Encoding) are tested at sockets rather than fabricated in
exchange templates.

`shutdown()` is idempotent: mark closed, signal cancellation, close all owned
retained and ephemeral pools and reject new sessions/requests. Active callers
must await their request futures after shutdown; this component does not spawn
search tasks or assume ownership of the caller's task supervisor. Dropping a
request future cancels its socket operation and never retries an ambiguous
attempt. Four adapter-owned decode permits are held inside the actual blocking
closures until completion, even if their awaiting request is cancelled. Waiting
for decode admission is cancellable and cannot dispatch a worker after closure.
Decoders check cancellation/deadline/shutdown at every 64 KiB output read.
`shutdown_and_wait().await` closes admission and waits for all registered decoder
workers; deterministic barrier tests prove cancellation retains the budget and
async shutdown waits for actual worker completion. Synchronous `shutdown()` and
Drop signal cancellation and close admission, but do not claim a thread join.
A decoder cannot dispatch another provider request.

`ProxyRotation` preserves round-robin selection, cooldown skipping, tripled
cooldown after three failures, success reset, dynamic endpoint override and
source fail-open selection when all proxies are cooling. Caller-supplied
monotonic time makes these decisions deterministic. Reporting failure never
sends or repeats a request; semantic endpoint fallback belongs to adapters.

## Bounds and intentional differences

See [ADR 006](../../adr/006-adapter-http-bounds.md) and the
[parity/difference ledger](parity.md). Defaults per session: 8 MiB encoded bytes
per response, 32 MiB decoded bytes per response (and intermediate encoding
layer), 2 MiB request body, 16 total dispatches and ten redirects per call.
Adapters can explicitly select other finite bounds, with their fixture review.
Only gzip and zlib/raw deflate are negotiated; repeated Content-Encoding header
values and comma-separated encodings form one ordered stack decoded in reverse. Unsupported encodings fail explicitly. Limits count
actual chunks and decoded bytes rather than trusting Content-Length.

## Verification and delivery

Run with the accepted Rust 1.99 toolchain:

```sh
cargo xtask check F08
cargo build --locked --workspace
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
pre-commit run --all-files
graphify update .
```

The gate checks source freshness, exact named test inventory and executes the
native transport suite. The ignored `env_proxy_child` is a subprocess-only
helper explicitly executed by the environment routing and CA tests, not skipped
acceptance coverage. The TLS test generates a fresh native CA and short-lived
leaf in memory with
exactly pinned test-only rcgen 0.14.10/time 0.3.55. It has no static certificate
expiry, persisted private key or runtime openssl dependency. No provider,
production state,
network fixture capture or performance benefit is claimed.

Portal impact: this additive native infrastructure is not wired to production
SearchService or portal routes and changes no current browser contract. Later
adapter/application integration must carry truthful errors and re-run the portal
gate; these tests do not substitute for that integration evidence.
