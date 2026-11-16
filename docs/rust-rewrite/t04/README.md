# T04: authoritative machine OAuth state

Native state service for issue #899, based on accepted F07 Valkey and F06
configuration contracts at main `1f3f5bc85083f4fd24bd9a2b07f161626e6bfef5`.
`ssx_domain::oauth` owns records, redacted credentials and OAuth-only settings;
`ssx_application::oauth::OAuthState` is the consumer-owned port;
`ssx_infra::valkey::oauth::OAuthStore` implements shared authority. There are no
HTTP endpoints, discovery routes, OIDC relying-party sessions, cookies,
membership or tenant decisions here. T05 owns protocol/endpoint validation.

## Configuration and protocol boundary

Composition supplies captured F06 values and an explicit authentication
composition choice: OAuth enabled, issuer, TTLs, an explicitly requested static
wrapper, Valkey handle and authority identifier. No module reads ambient URLs or
credentials. `OAuthMode::configured` enumerates disabled, static bearer, OAuth and
explicitly combined OAuth-plus-static-bearer helper compositions; its static flag
is not an instruction to layer credentials automatically from raw config presence.

The pinned source distinguishes entrypoint selection from helper composition.
[`server.py:437-453`](../../../slopsearx/mcp/server.py#L437-L453) selects OAuth alone
when OAuth is enabled, passing an empty static token even if one is configured;
otherwise it selects static bearer when configured.
[`security.py:64-79`](../../../slopsearx/mcp/security.py#L64-L79) can deliberately
wrap an OAuth server with an outer static gate. `Both` represents that explicit
helper use, not CLI precedence. T05 must preserve the CLI selection contract.

`OAuthStore::load_token` validates a stored OAuth grant; it does not authorize a
transport request. T05 enforces the selected transport middleware. Static bearer
and OAuth tokens are independent credentials: revoking an OAuth pair does not
revoke the operator static token.
The source network SSE entrypoint at `server.py:463-467` bypasses auth assembly;
T05 closes that bypass under issue #900's explicit all-network-transport scope,
while retaining trusted stdio and auto-approval without a new consent policy.
These source anchors refer to accepted baseline `1f3f5bc8`; they do not change
T04 runtime behavior or activate an endpoint. OAuth requires
an HTTP(S) issuer and derives the `/mcp` resource URL. It strips raw trailing
slashes before independently normalizing issuer/resource URLs, retaining slashes
introduced by dot-segment normalization. User-info, query and fragment components
are explicitly rejected as a typed configuration hardening. F09 owns general MCP policy;
this is an OAuth-specific projection, not a second policy parser. Invalid TTLs
(zero or beyond one year), oversized/control-bearing identities and malformed
bounded records reject before a mutating command.

Registered clients retain the complete pinned MCP model's metadata, including
nullable/empty redirect lists and auth methods, JWKS, issuer and software fields.
The required client ID and optional secret are separate from bounded lossless
metadata; metadata cannot shadow either. T05 constructs this record after SDK-
equivalent model validation and reconstructs its public registration response.
T04 storage does not turn an unusable redirect list into a valid authorization:
code creation requires a registered matching redirect. Registration uses
create-only collision semantics; retries or a different registration under an
existing ID cannot overwrite it. Reconcile an unknown result through lookup;
never blindly register/mint again after an ambiguous write.

T05 authenticates clients and validates PKCE, redirect requirements, scopes,
resource audience and protocol errors. After loading a grant, it passes the exact
validated binding to redemption/rotation. State rereads that binding and fences
it atomically; it does not accept a detached grant as authority. Optional resource,
subject and redirect explicitness are retained exactly, and refresh scopes may
only narrow (an empty request retains the previous scope list). The resulting
grant is revalidated against the 128-scope storage bound before minting or
mutation; an oversized request leaves the original pair byte-for-byte intact.

## Storage and atomicity

All time and TTL authorization decisions use Valkey `TIME`; caller/process clocks
cannot affect them. Expiries are epoch milliseconds, with `PXAT` retention at
that horizon. Client/code/access/refresh defaults remain 30 days/600 seconds/
3600 seconds/30 days. Namespace:
`mcp:oauth:v1:{sha256(authority)}:<kind>:<sha256(identity)>`.
The authority identifier is a deployment machine-OAuth authority, not tenant
membership. All keys for one authority share a cluster slot.

Internal bounded JSON records have schema 1, a kind, expiry and typed data.
Access/refresh records store reciprocal digest IDs and counterpart expiries,
not raw tokens. Native opaque credentials use the already-pinned rustls secure
random source. Raw credentials have redacted Debug and no public Serialize
implementation; only the private client codec stores a client secret. Metadata
and bindings also redact Debug, and errors carry static classifications only.

The transition Lua checks exact expected bytes, all key types, missing/new-key
collisions and authoritative expiry before any writes. Code redemption atomically
consumes the code and publishes both tokens. Refresh rotation consumes the old
pair and publishes the replacement pair. Revocation deletes the pair atomically.
Concurrent losers deny; there is no retry, local fallback or new credential
issuance on unavailable/closed/malformed/unknown-completion outcomes.

Reciprocal links and bindings are verified before mutation. A missing counterpart
is allowed only after its recorded expiry: refresh naturally outlives access,
and reversed configured lifetimes work too. An extant malformed or mismatched
counterpart cannot delete another pair. Revoke/rotate races linearize to one
winner; revocation of a stale old token does not revoke a newly rotated pair.
Missing and retained-but-expired records are distinct denied outcomes; expired
keys already evicted by Valkey are missing. The read is a snapshot, not a promise
of continued validity; mutation validates its expected bytes and expiry again.

F07's unknown write completion remains explicit. A response loss may leave a
committed pair whose opaque values the caller did not receive. Do not retry the
same transition into fresh credentials: deny/reconcile or start a fresh user-
authorized flow. Cancellation can likewise leave a committed command; no claim
of exactly-once network response delivery is made.

## Source parity and intentional corrections

The isolated `capture.py` reference reads exact accepted source bytes and
captures four complete SDK client records, nine OAuth settings cases, and provider flow projections under
Python 3.14.7, MCP 2.3.0 and FastMCP 4.0.11. Native gates never invoke it.
It is migration evidence only and must retire under G08. Source code/token
binding, default TTLs, empty-scope rotation, linked revocation, registration
metadata and token lifetimes are replayed in native tests.

Reviewed corrections to the source's multi-step/degrading implementation:

- Durable authority has no process-memory fallback. No memory development mode
  is implemented; production cannot silently issue isolated credentials.
- Single-use consumption/rotation/revocation is atomic and uses server time.
- Source `_issue_tokens` drops the code's resource (`resource=None` for both
  token kinds). Native tokens preserve it through issuance and refresh. The
  fixture retains the source null; native replay permits only this explicit
  resource correction and opaque-random/time substitutions.
- Registration collisions cannot replace existing records. Reciprocal malformed
  links fail closed rather than deleting a token named by untrusted state.
- Secrets are hashed in keys and opaque pair records use digest links; schema
  and namespaces are versioned. TTLs are positive and bounded, JSON records
  bounded at 64 KiB, scope lists at 128, metadata keys at 64 and strings at
  4096 bytes. Expiry is strict at the server millisecond boundary.

## Migration and downstream handoff

Legacy Python `mcp:oauth:<kind>:<raw-secret>` records are not silently adopted.
No runtime consumer switches to this namespace in T04. Before T05/G07 cutover,
choose and explicitly approve a drain/reissue window: stop legacy code/token
issuance across all replicas, drain or expire outstanding codes, revoke/drain
existing tokens and re-register/re-authorize clients against the new authority.
Do not mix old and new writers or pretend old credentials remain valid. Preserve
legacy state through the agreed rollback horizon; rollback of new credentials
requires reauthorization against the legacy issuer. G04/G07 own rehearsal,
operator authorization, downtime/reauthorization communication and rollback.
No production migration, flush, deployment or provider use is authorized here.

Portal impact: this state-only module is not wired into public HTTP/MCP or browser
sessions. T05 must qualify browser-visible authorization errors and transport
behavior when wiring it. Existing Python/portal CI remains unchanged.

## Qualification

`cargo xtask check T04` checks immutable source/corpus/exporter/script anchors,
the exact native test inventory, private real-Valkey integration and domain
settings/compile-fail secret serialization tests. `.github/workflows/rust.yml`
runs it in both existing Linux architecture jobs and uploads `T04.json` with the
accepted evidence. The gate is state-service qualification only, not OAuth
endpoint or full protocol certification.

### Create-only absence correction

The native transition now distinguishes a missing Valkey key from an extant
empty-string record. Empty expected bytes designate absence only; malformed empty
records remain untouched and cause a conflict. A private-Valkey regression covers
actual client registration, successful creation after genuine deletion, ordinary
record collisions and a later-key collision preventing all writes in a multi-key
transition. This corrects the native CAS implementation without changing the
source observation corpus or the public storage error vocabulary.
