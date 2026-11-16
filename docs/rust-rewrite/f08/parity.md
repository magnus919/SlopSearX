# F08 parity and intentional-difference ledger

Baseline: `7cc420f7ffb61127750e77bf6130e225925f9186`. Evidence is
`cargo xtask check F08` and its source-bound named native coverage manifest.
Review/merge acceptance remains pending.

| Contract | Native evidence / difference |
| --- | --- |
| Pool reuse and search identity | Actual TCP accept counts; cookies persist within a session and auth/header/cookies do not carry to later searches |
| 20/10/5 bounds | Concurrent real sockets establish 20 accepts and ten idle survivors; expiry causes reconnect; shutdown closes sockets |
| Proxy pools | Ten real proxies establish eight reused pools and two ephemeral overflow pools; subprocess observes env proxy and NO_PROXY bypass |
| TLS and redirects | Self-signed untrusted leaf rejected, explicit verification override and CA succeed; real-socket GET/POST/PUT/PATCH/DELETE/HEAD matrix across 301/302/303/307/308, body/framing/Content-Type/cookie rules, and cross-origin credential stripping replayed |
| Deadline/cancel/429 | Actual streaming body deadlines, queued deadline, cancellation and shutdown return typed errors; 429 retains status and no retry occurs |
| Malformed bodies | HTML/XML/JSON bytes replay unchanged; parsers and full adapters remain pending their batches |
| Provider fallback | No automatic transport retry; DDG/PubMed/Internet Archive semantic sequences remain later adapter obligations |
| New body bounds | 8 MiB encoded / 32 MiB decoded per response and intermediate layer, 2 MiB outbound body; typed failure, no truncation; intentional new limits |
| New request bounds | 16 dispatches/session and ten redirects/call, typed failure; explicit finite adapter overrides possible |
| Encodings | gzip + zlib/raw deflate, stacked recognized encodings across repeated and comma-separated headers; reversed and unknown-later stacks rejected; unsupported encoding explicitly fails rather than passing opaque bytes; optional Brotli/Zstd not advertised |
| Redirect tightening | Credentials stripped on every origin change, including same-host secure upgrade; intentional difference from HTTPX upgrade exception |
| Resource strategy | Ten origin-specific idle clients enforce the global idle bound; origin churn may retain fewer useful idle sockets than HTTPX's internal strategy; no performance equivalence claim |
| Platform trust | macOS loopback evidence only; system roots/proxies and mTLS on release platforms remain O02/O04 qualification |
| Portal | Additive native module only; no production wiring/browser contract change; later integration must run the required portal checks |

## Candidate native User-Agent amendment

Shared default identity is versioned `SlopSearX/<ssx-infra package version>
(meta search engine; agent-native)`, with request > session > default precedence.
This is an intentional source/native wire-identity difference from pinned
`python-httpx/0.28.1`; immutable source captures remain unchanged. Native replay
expectations declare it explicitly, while Replay retains complete strict header
comparison. Actual sockets qualify default/overrides, redirects, and continued
cookie/auth isolation. No provider compatibility benefit is claimed. Merged
A07 consumers still require validation after normal main integration or isolated
parent donor validation; this candidate is not activation acceptance.
