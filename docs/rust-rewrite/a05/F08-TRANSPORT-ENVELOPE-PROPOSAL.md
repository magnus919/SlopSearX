# Optional F08 operation-owned transport envelope

Status: proposal requiring a shared F08 owner/review; no transport implementation
or aggregate acceptance is implied by the current A05 post-response body charge.
The adapter remains responsible for the existing per-produced-batch envelope;
retained response/caller lifetime and overload qualification remain G02 #937.

## Observed gap and affected ownership

F08 `Response` owns status, Url, HeaderMap and Vec body. The HTTP transport clones
response headers and grows the wire-body Vec before returning it. Compression
work and response/header clones can overlap these buffers. A05 currently reserves
`body.capacity()` only after return. This bounds its later overlap but cannot
retroactively bound F08 allocation peaks. HeaderValue/Bytes public lengths do not
specify retained backing capacity; HeaderMap implementation layout is not an
accounting contract. No estimate based on private layouts is proposed.

The session also retains reqwest's cookie Jar across its search-local bootstrap,
primary/lite requests. Cookie parsing, stored cookie strings, replacement and
expired entries allocate independently of body limits. Pooled connections and
protocol buffers can outlive a search. Existing F08 wire/decoded/request/count/
redirect bounds must remain independently enforced. Accepted body size limits
are not a proof of header/cookie/pool allocation bounds.

The optional path affects AdapterHttp session construction, Session execution,
redirect handling, decompression/bounded_read, cookie storage and Response
handoff. A05 DDG/Google are initial consumers; A06/A07 should consume only after
shared review. Legacy callers retain current signatures/default behavior through
an optional envelope; no blanket all-adapter migration is proposed here.

## Proposed seam and policy decisions

1. Application passes an optional operation-owned transport allocation context
   containing the reviewed neutral Budget and inherited absolute deadline. A
   non-cloneable transport envelope retains owned charges with returned response
   storage; it exposes borrowed header/body access and explicit transfer/drop.
   Do not create a second independent per-session quota. Resource refusal is
   typed, separate from network/status/deadline and never provider health failure.
2. Request serialization, wire collection and decompression use precharged owned
   buffers with checked growth and old+new allocation overlap. Encoded request,
   redirect URL/header copies and decoder scratch are charged before allocation.
   Deadline/cancellation retains live worker/storage charges through verified
   cleanup, using the already reviewed supervisor where blocking work applies.
3. Header backing requires a transport-owned representation/collection seam at
   the actual parser allocation boundary, or a proven finite shared transport
   arena/cap reserved before parsing. Copying HeaderMap afterward is insufficient.
   The owner must identify the concrete Hyper/reqwest public seam and backing
   ownership; if none exists, propose the smallest dependency/private transport
   change with source/version/ABI/security review. Do not infer an exact heap
   bound from count/serialized byte lengths or an incomplete-header threshold.
4. Search-local cookie semantics remain: same-host continuity, Domain-cookie
   subdomain continuity, host-only identical-host restriction, fresh-search and
   unrelated-host isolation. A charged cookie representation/parser must preserve
   source-visible expiry/domain/path/order/replacement behavior. Proposed finite
   cookie/header quota refusal fails the whole adapter as the existing sanitized
   resource Error, rather than silently dropping/truncating cookies or falling
   back. Exact finite policy needs parent review; no arbitrary cookie-count cap is
   adopted by this document. Transient parser allocations must also be covered.
5. Connection buffers cannot silently retain a completed operation's released
   charges. The shared owner must choose explicit handoff into a finite global
   transport budget (accounting overlap during transfer), or operation-local
   connection ownership/drop. Existing pooled transport reuse is observable in
   timing and bootstrap success, so replacing it needs explicit qualification.
   Holding an operation admission permit indefinitely for idle pooled sockets is
   not an acceptable implicit policy. Global transport buffers and runtime/OS
   overhead need separate G02 process/concurrency limits and measured RSS proof.
6. Response envelope remains charged through A05's header/body copy into its
   child frame. Only after transport storage is dropped may those charges end;
   child allowance/frame/reply/projection reservations use the same root. Existing
   source errors preserve status/request/fallback order. A typed transport
   resource failure is terminal with no DDG extra fallback/bootstrap request.

## Required qualification before closure

Actual loopback tests must exercise allocation refusal during request encoding,
large duplicate response headers, body growth/decompression, redirects and cookie
parse/store/replacement; assert zero partial result publication and exact request
counts. Include fresh-search/Domain/host-only/isolation controls, cancellation
while reading/decompressing, retained response overlap, connection reuse/handoff,
and failure cleanup. Source-compatible ordinary inputs retain full response and
request parity. New hardening outcomes need an explicit parity-ledger entry.

A capacity ledger is necessary but not allocator/RSS proof. Qualify GNU amd64,
GNU arm64 and macOS dependency/link closure, child/transport descriptor lifetime,
process/container memory bounds and measured concurrency/overload under G02.
Until that evidence exists, A05 may claim only the explicitly charged phases;
not a complete 256MiB operation peak or process RSS bound.
