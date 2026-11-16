# T05 OAuth endpoints and network authorization

T05 consumes accepted T04's `OAuthState` port and accepted T01 network transports.
The source baseline is `403dfd3720e969fa829f426136bf0f05b0360c56`.
`ssx-mcp::oauth::OAuthEndpoints` supplies discovery, registration, authorization,
code/refresh exchange and revocation. `NetworkAuth::mount` wraps the complete
streamable HTTP router or complete legacy SSE/messages pair, including session
methods and message suffix paths. The composition root applies it with T01's
`ManagedRouter::map`, retaining cleanup ownership. Trusted stdio is unchanged.
Production server startup remains T06's responsibility; these are actual qualified
network modules, not a claim that the unfinished production binary exposes them.

`NetworkAuth::from_cli` preserves source OAuth precedence over a configured static
token. Explicit `Both` represents the source helper's deliberate outer static gate;
it never follows automatically from the presence of two config values. Both gates
see the same Authorization header; it is not an alternative-credential union.
Static bearer comparison is constant-time. Duplicate Authorization credentials
fail closed on every network method. Metadata remains public in OAuth-only mode.
No browser identity, tenant membership, extra consent UI or provider grant policy
is introduced. The source operator subject and existing auto-approval remain.
Portal authorization behavior is covered through actual HTTP redirects, error
bodies, CORS and metadata; T09's separate browser session authority is untouched.

Client metadata preserves all supported known nullable fields; unknown metadata is
ignored as in the SDK. Client IDs use UUIDv4 (122 random bits); client secrets use
32 bytes from accepted AWS-LC secure randomness (256 bits), hex encoded. Credentials
are never logged or included in generic Debug/errors. Registration intentionally
returns its new secret, matching the source protocol. PKCE uses SHA256/base64url.
Client authentication, normalized exact redirect/explicitness, verifier, scopes
and resource binding are checked before T04's exact atomic transition. Malformed
explicit redirect input never becomes an omitted redirect; a rejected exchange
leaves the code usable. Code and refresh replay do not issue another token pair.

The narrow `OAuthState::now_ms` extension delegates to existing Valkey TIME, in
milliseconds; unavailable authority never falls back to a process clock. Secret
metadata expiry uses the source integer-second boundary at authentication read.
That read is not atomic with the subsequent state transition: authentication
linearizes at that read; T04 separately checks client-record TTL and exact grant
state atomically at commit. This module does not claim an impossible zero-race
metadata-expiry guarantee across those operations. Expired durable grants cannot
be consumed. An unknown committed outcome returns 503 without automatic retry or
fresh credential issuance; callers must not interpret 503 as proof of no write.

ADR005 permits HTTP credential development endpoints only on actual loopback
(`localhost`,127/8,::1); other endpoints require HTTPS. Configuration DTOs are
revalidated before header construction. Network proxy policy and production TLS
termination remain the owning composition boundary's responsibility.

## Source and native evidence

`cargo xtask check T05 --output target/rust-evidence/T05.json` runs native tests
against disposable official Valkey8.1.10 and real independent HTTP/rmcp clients.
It binds reviewed corpus/exporter digests, exact accepted source bytes, exact
executed test names and zero ignored tests. Both Linux architectures run this
gate and upload the report; existing legacy and native gates remain enabled.
T01/T04 qualifications independently retain stdio, transport lifecycle and actual
lost-wire-reply/CAS proofs. The endpoint unknown-result test commits real private
Valkey state then injects its classified unknown result at the application port;
it does not pretend to independently drop a wire reply.

The 80-case source corpus contains actual status/body/selected-header observations
from pinned Python3.14.7, MCP2.3.0 and FastMCP4.0.11 using an isolated synthetic
loopback authority. No engine or external provider is called. Only opaque IDs,
secrets, codes/tokens and issuance epoch are replaced. `PYTHONHASHSEED=0` fixes the
SDK's set-derived Allow order; headers are not sorted or rewritten. Multipart
capture normalization descends into tuple-valued httpx fields, avoiding embedded
random source IDs. The original69 observations remain byte-equivalent when the
malformed implicit redirect case was added. First failed native replay and both
parent-discovered defects are retained as red evidence in the handoff.

Temporary developer-only reproduction:

```sh
PYTHONHASHSEED=0 PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/t05/capture.py --output /private/tmp/t05-source.json
```

The exporter is inventoried as backend-coupled tooling, never invoked by native
gates/runtime, and retired under G08. Source captures are reference observations,
not successful production OAuth deployment or provider access evidence.
