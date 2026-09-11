# ADR 002: Browser identity and tenant isolation

- **Status:** Proposed for maintainer acceptance
- **Scope:** Protected browser workflow routes tracked by issues #357 and #358
- **Decision authority:** This record becomes Accepted only when the maintainer
  merges the pull request that introduces it. Implementation must not begin
  while the record remains Proposed.
- **Date:** 2026-09-11

## Context

The public portal is a server-rendered, stateless view over `SearchService`.
It has no account, authenticated session, or browser mutation. Durable MCP
workflows, however, are tenant-bound. Their current request identity comes from
an OAuth access token's `client_id`, with a deterministic `tenant_scope()` test
override and a single-tenant `default` fallback. A browser workflow console
needs a human principal, explicit tenant membership, and action authorization;
an OAuth client registration is not a human identity.

SlopSearX's MCP OAuth server is intentionally an auto-approving authorization
server for registered MCP clients. It uses one configured `operator` subject,
stores opaque tokens in Valkey, and maps the MCP tenant to the client ID. The
gateway OAuth client is a native/CLI flow with a loopback callback. Those are
sound boundaries for MCP interoperability, but they do not authenticate a
browser user or establish tenant membership.

The design must preserve these invariants:

- `/` and SearXNG-compatible `/search` stay public and session-independent by
  default. An operator may still protect the entire deployment externally.
- Browser and MCP adapters call the same application services and policy gate.
  The portal never calls MCP over the network or manipulates Valkey directly.
- Shared durable state remains bounded and Valkey-backed.
- An object lookup or action never reveals whether another tenant owns an ID.
- Protected routes can be disabled without disabling public search or changing
  the SearXNG JSON contract.

## Options considered

### A. Reuse the MCP OAuth authorization server and client

This would reuse protocol endpoints and token storage. It was rejected because
the current authorization server proves possession of a registered client, not
a person's identity. Dynamic client registration plus auto-approval would make
client ID a user/tenant surrogate. The fixed `operator` subject, loopback client
flow, and MCP scopes do not provide login, membership, or browser session
semantics. Changing them to do so would couple human identity to MCP protocol
compatibility and increase the impact of either system's changes.

MCP and the portal may share lower-level cryptographic, Valkey, and policy
utilities after their contracts are separated. They do not share bearer tokens,
cookies, audiences, callback routes, or subject-to-tenant mapping.

### B. Trust identity headers from an authenticating reverse proxy

This can be appropriate for a tightly controlled internal deployment, but was
rejected as the primary contract. A missed header strip, an alternate path to
the application, or a broad trusted-proxy range becomes an authentication
bypass. Header formats differ across proxies, multi-tenant membership is often
underspecified, and deterministic behavior becomes deployment-specific.

An implementation may later add this as a separately named provider after the
OIDC contract exists. It must produce the same normalized principal and
authorization context, require an explicit trusted-proxy allowlist, reject
requests from other peers, and cryptographically validate a signed assertion or
use a mutually authenticated hop. Plain `X-User`/`X-Tenant` headers are never an
accepted identity source.

### C. Dedicated OIDC client with a server-side session (chosen)

SlopSearX acts as a confidential OpenID Connect relying party using the
Authorization Code flow with PKCE. The callback validates issuer, audience,
signature, expiry, authorization `state`, PKCE, and `nonce`. It converts the
validated `(issuer, sub)` pair into an internal principal and creates an opaque,
bounded server-side session in Valkey. The browser receives only a random
session handle in a cookie; OIDC tokens are never exposed to browser JavaScript.

This separates external authentication from SlopSearX authorization, supports
standard identity providers, works across replicas, and gives logout,
revocation, tenant switching, CSRF, and test injection one explicit boundary.

## Decision

### Identity and tenant membership

The authoritative external subject key is the exact `(normalized issuer URL,
sub)` pair from a validated ID token. Email, display name, client ID, forwarded
header, and an unverified JWT claim are not identities.

An operator-managed identity directory maps one external subject to one stable
internal `principal_id` and to zero or more tenant memberships. Each membership
contains a stable `tenant_id`, roles/grants, status, and authorization revision.
The directory is server-side configuration or a bounded Valkey-backed service;
it is never supplied by a form or inferred from an object ID. If claim-based
mapping is later supported, the issuer, claim name, accepted values, and tenant
mapping must be allowlisted explicitly. Unknown issuers, subjects, tenants,
claims, or mapping failures deny access.

A subject may belong to multiple tenants. The session carries exactly one
active tenant. After login, a principal with one membership enters it directly;
a principal with several memberships chooses from the server-derived list.
Tenant switching is a CSRF-protected mutation that revalidates membership,
rotates the session identifier, and never accepts a caller-provided tenant that
is absent from that list. No global tenant browser exists. Tenant display names
may be shown only after authentication; stable internal IDs need not be exposed.

Every request rebuilds an authorization context from the session's immutable
principal ID and active tenant, then resolves the current membership revision
and grants server-side. Effective grants are the intersection of current
membership grants and current operator policy; a runtime policy denial always
wins. A cached grant snapshot may be used only when its short operator-set
maximum age has not elapsed and its revision still matches. Store unavailability,
a missing membership, a disabled principal/tenant, or an unknown grant fails
closed. This prevents a valid but stale session from retaining revoked
privileges.

### Session contract

The cookie contains a cryptographically random, opaque handle with at least 256
bits of entropy. Valkey stores only a keyed hash of that handle under a separate
browser-session prefix. The bounded record contains principal ID, active tenant,
OIDC issuer, authentication time, created/last-seen/absolute expiry, membership
revision, CSRF secret, and a session generation. It contains no raw OIDC access,
ID, or refresh token unless a later accepted decision demonstrates a concrete
need and specifies encryption and revocation.

The default cookie name is `__Host-slopsearx_session` and its attributes are
`Secure`, `HttpOnly`, `SameSite=Lax`, `Path=/`, with no `Domain`. Direct
development over loopback HTTP may use a separately named insecure development
cookie only under an explicit development mode that cannot start in a
production configuration. The login transaction cookie/state uses the same
host-only boundary and expires within ten minutes.

Defaults are a 30-minute idle timeout and an eight-hour absolute lifetime.
Operators may shorten them. Longer values require an explicit configuration
change and must remain bounded. Successful authentication, privilege change,
tenant switch, and suspicious reuse rotate the handle atomically and invalidate
the old handle. Activity may advance idle expiry without extending absolute
expiry. Parallel refreshes use compare-and-swap semantics so an old handle
cannot become valid again.

Logout is `POST /auth/logout`. It deletes the server session, expires the cookie,
and redirects only to a validated local target. IdP logout is optional and does
not replace local invalidation. Operator revocation increments the principal or
membership revision and deletes known sessions when indexed; revision checking
provides fail-closed protection if index cleanup is incomplete. Back-channel
logout may be added when the configured IdP supports it. Sessions expire
naturally if it is unavailable.

### CSRF and redirect safety

Every user-initiated state-changing browser request, including logout, tenant
switch, cancel, retry, pause/resume, acknowledgement, and composition, uses POST
(or another non-safe method) and requires all of:

1. a synchronizer token derived from the session CSRF secret and verified in
   constant time, supplied in a form field or `X-CSRF-Token` header;
2. an exact same-origin `Origin` check, falling back to a same-origin `Referer`
   check only for user agents that omit `Origin`;
3. an allowed content type for the route; and
4. action authorization and idempotency/concurrency controls after CSRF passes.

`SameSite=Lax` is defense in depth, not the CSRF control. Outside the OIDC login
protocol, GET/HEAD never mutate, dispatch work, extend artifact retention,
logout, or change tenant. The login start and callback may create a one-time
transaction/session only after their state, nonce, PKCE, issuer, and redirect
checks pass; they cannot perform a workflow action. CSRF failure returns a
generic 403 and is audited without echoing the submitted token.

`return_to` values are local absolute paths under an allowlist (`/workflows`
and its descendants initially). Absolute URLs, authority/scheme-relative URLs,
encoded backslashes, control characters, ambiguous multiple decoding, and paths
outside the allowlist are rejected to `/workflows`. OIDC callback locations are
exactly registered. Authorization state binds the return path, PKCE verifier,
nonce, and initiating browser transaction and is consumed once.

### Route and authorization matrix

| Surface | Identity | Authorization and failure behavior |
| --- | --- | --- |
| `GET /`, public HTML search | None | Existing portal policy; remains public by default. A session does not change search results. |
| `/search` GET/POST and machine formats | None | Existing SearXNG-compatible contract and status/body negotiation; no login redirect. |
| `/healthz` | None | Existing readiness contract; contains no tenant data. |
| `/health`, `/metrics`, deployment/admin surfaces | Operator network policy | Remain operator deployment surfaces; browser session does not grant access. |
| `/mcp` and MCP OAuth endpoints | MCP auth contract | Remain isolated from browser cookies and OIDC sessions. |
| `GET /auth/login`, `GET /auth/callback` | Login transaction | OIDC protocol endpoints, rate-limited; callback failure shows a generic recovery page. |
| `POST /auth/logout` | Browser session + CSRF | Revokes the local session even when IdP logout is unavailable. |
| `GET /workflows`, bounded list/detail/lineage | Browser session | Current tenant membership and action-specific read grant. Unknown, expired, revoked, or cross-tenant object IDs return the same 404 representation. |
| `/workflows/...` mutations | Browser session + CSRF | Current tenant membership, action-specific grant, shared policy gate, idempotency, and concurrency precondition. |
| Future `/operator/...` | Browser session plus explicit operator grant and deployment policy | Separate namespace; no operator authority follows merely from being authenticated or from an IdP group claim. |

For a protected HTML navigation without a valid session, SlopSearX responds
with `303` to `/auth/login` and a validated local return path. An explicit JSON
or other machine representation receives a 401 problem response and
`WWW-Authenticate`; it is never redirected to HTML. An authenticated request
that lacks a route-level grant receives a generic 403. Object-level denial,
including unknown, expired, revoked, malformed-but-plausible, or cross-tenant
IDs, receives the same caller-visible status, body shape, timing class, cache
policy, and reason: generic 404. Logs may retain a private reason code, but the
response cannot distinguish existence.

Actions authorize both the verb and object in the active tenant immediately
before the transition. The shared policy gate remains authoritative for engine
and sensitive-data decisions. UI visibility is convenience only; hiding a
button is never authorization.

### Network and deployment boundary

OIDC is enabled only with a configured external origin using HTTPS, except the
explicit loopback development mode. TLS terminates either in SlopSearX's trusted
server boundary or at a declared reverse proxy. SlopSearX trusts forwarded
scheme, host, and client address only from an operator-configured narrow list of
proxy addresses. The edge must remove incoming forwarding and identity headers,
set fresh forwarding headers, preserve the original Host, and block alternate
direct paths to the application. The application must not infer trust merely
because a request came from a private address.

In direct exposure, forwarding headers are ignored and callback/origin checks
use the configured external origin. In proxy exposure, startup fails when OIDC
is enabled with an invalid external origin, an empty/broad trust configuration,
or an external scheme inconsistent with the TLS topology. Host is allowlisted
before it participates in a redirect or origin calculation.

Separate bounded rate limits apply to unauthenticated login starts by client
network key, callbacks by transaction key and network key, authenticated reads
by principal and tenant, and mutations by principal, tenant, and action. A
failed identity/store/rate-limit dependency fails closed for protected routes;
public search retains its existing availability and rate-limit behavior.

### Audit, redaction, and secrets

Authentication, tenant switching, authorization decisions, session revocation,
and workflow mutations produce structured audit events. Events include time,
request ID, action, outcome, private reason code, object kind, and keyed hashes
of principal, tenant, session generation, and object ID. They exclude cookies,
CSRF values, authorization codes, OIDC tokens, raw `sub`, email, display name,
query text, workflow contents, and engine credentials. Logs and metrics use
closed reason/action vocabularies and bounded cardinality.

Client secret, cookie/session HMAC keys, OIDC trust configuration, and any audit
pseudonymization key come from the operator's secret mechanism. They are never
sent to the browser, committed to configuration examples, placed in URLs, or
logged. Key rotation accepts the immediately previous verification key for a
bounded drain window while issuing only with the current key. Loss of a
verification key invalidates affected login transactions/sessions and requires
sign-in again; it never falls back to unsigned state.

### Deterministic implementation and test seams

The later implementation must define a transport-independent normalized
`PortalPrincipal`/`PortalAuthContext` and inject these boundaries into the
FastAPI application:

- identity provider: starts login and validates callback into an external
  `(issuer, sub)` identity;
- membership resolver: maps the external identity to current principal,
  tenants, revisions, and grants;
- session store: creates, rotates, reads, revokes, and expires tenant-bound
  opaque sessions using an injected clock and token source;
- request identity resolver: converts cookie/session state into the normalized
  authorization context; and
- shared workflow application services/policy: authorize and perform reads or
  actions independently of HTML rendering.

Production factories use OIDC and Valkey. Deterministic fixtures inject fake
providers/resolvers, an in-memory session store, fixed clock/token source, and
two explicitly different tenants. The seam must exercise the real routes,
cookies, CSRF checks, content negotiation, authorization calls, and HTML
formatter; tests must not set a global process user or bypass middleware. The
existing MCP `state_factory` and `tenant_scope()` remain unchanged and cannot be
repurposed as browser authentication.

Required security cases include session fixation/rotation, expired and revoked
sessions, stale membership revision, forged/unsigned OIDC claims, replayed
state/callback, CSRF and Origin failures, confused-deputy action attempts,
spoofed forwarding/identity headers, open redirects, token/cookie redaction,
cross-tenant enumeration, and equal public-search output with and without a
workflow cookie. Browser fixtures cover sign-in recovery, tenant selection,
expiry during a form, forbidden actions, generic missing-object behavior, and
signed-out recovery at narrow and desktop viewports.

## Consequences

- SlopSearX gains a human identity boundary without changing MCP OAuth or the
  SearXNG surface.
- Multi-replica sessions require Valkey; protected routes are unavailable when
  it is absent. Public search remains independent.
- Operators must configure an OIDC provider, identity memberships, external
  origin, secrets, and the TLS/proxy boundary before enabling workflows.
- Grant changes take effect within a short bounded interval and immediately
  when revision lookup succeeds; availability is traded for fail-closed access.
- Server-rendered pages remain the presentation model established by ADR 001.
  The browser never becomes a bearer-token client.

## Threat model summary

| Threat | Required control |
| --- | --- |
| Session fixation | Rotate at login, tenant/privilege change, and suspicious reuse; invalidate old handle atomically. |
| CSRF | Synchronizer token, exact Origin/Referer validation, safe-method discipline, SameSite defense in depth. |
| Confused deputy | Resolve active tenant server-side; authorize action and object immediately before transition. |
| Header spoofing | Ignore by default; explicit narrow proxy trust plus edge stripping; signed assertion or mTLS for any future proxy-identity provider. |
| Open redirect | Exact callback registration and allowlisted local `return_to` paths with single canonical decoding. |
| Token leakage | Opaque HttpOnly cookie; no OIDC tokens in browser, URLs, logs, metrics, or artifacts. |
| Cross-tenant enumeration | Tenant-qualified stores plus uniform 404 response/timing/cache behavior for object-level denial. |
| Stale grants | Current revision/membership resolution with short bounded cache; deny on unknown or unavailable state. |

## Rollout and rollback

Implementation is gated by `SLOPSEARX_WORKFLOW_PORTAL_ENABLED=false` by default.
OIDC endpoints and `/workflows` are mounted only when configuration validation
passes. Canary read-only routes first; enable each mutation separately only
after its CSRF, authorization, idempotency, concurrency, audit, accessibility,
and browser gates pass.

Rollback disables the workflow portal and stops creation of new sessions. It
does not change `/`, `/search`, MCP, or durable workflow processing. Existing
browser-session and login-transaction records are deleted by bounded indexes or
expire under their short TTLs; workflow artifacts keep their original
retention. Re-enabling after an identity or key incident increments the session
generation so old handles cannot resume.

## Unresolved implementation choices

The implementation issue must select the supported OIDC library, exact
configuration names, bounded rate values, session-index ceiling, and audit sink
from repository-compatible options. Those choices may tighten this contract;
they may not add an identity fallback, accept bearer tokens in the browser,
weaken tenant/grant resolution, or make public search depend on OIDC. A material
change to those semantics requires a superseding ADR.

## Acceptance and supersession

Maintainer merge of this ADR's pull request is the explicit acceptance event.
Issue #358 must link the accepted revision before implementation begins. A
later decision may add a conforming trusted-proxy identity provider, but it must
use the normalized context and all session, tenant, authorization, audit, and
test requirements above. Replacing OIDC as the primary provider or sharing MCP
tokens requires a superseding ADR.
