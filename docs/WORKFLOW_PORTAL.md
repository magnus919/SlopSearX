# Authenticated workflow portal

The protected `/workflows` console lets a human supervisor inspect retained
research, staged-search, dossier, and saved-search state in the active tenant.
It is a server-rendered adapter over the same stores, research runner, MCP
policy, capability catalog, and Valkey instance used by agent workflows. The
composition action invokes the authoritative MCP application function in
process with request-local state and tenant scopes; it does not call MCP over
HTTP, scan Valkey keys, or extend artifact retention.
Public `/`, `/search`, and every SearXNG format remain session-independent.

The feature is disabled unless `SLOPSEARX_WORKFLOW_PORTAL_ENABLED=1`. Enabling
it is fail-closed: startup requires connected Valkey, the shared capability
catalog, an HTTPS external origin, valid OIDC configuration, a session secret,
and a valid identity membership file. The implementation uses Authorization
Code flow with PKCE S256, a one-time Valkey transaction, issuer, audience,
time, nonce validation, and RS256 verification from the provider JWKS. OIDC
tokens are held only for callback validation and are never persisted.

## Configuration

| Variable | Required value |
| --- | --- |
| `SLOPSEARX_WORKFLOW_PORTAL_ENABLED` | `1` to enable; unset is disabled |
| `SLOPSEARX_PORTAL_EXTERNAL_ORIGIN` | Public HTTPS origin |
| `SLOPSEARX_PORTAL_OIDC_ISSUER` | Exact HTTPS issuer |
| `SLOPSEARX_PORTAL_OIDC_CLIENT_ID` | Confidential OIDC client ID |
| `SLOPSEARX_PORTAL_OIDC_CLIENT_SECRET` | Confidential OIDC client secret |
| `SLOPSEARX_PORTAL_SESSION_KEY` | Random secret containing at least 32 bytes |
| `SLOPSEARX_PORTAL_PREVIOUS_SESSION_KEY` | Optional immediately previous key used only to verify a bounded drain |
| `SLOPSEARX_PORTAL_PREVIOUS_KEY_DRAIN_SECONDS` | Previous-key drain from 60 to 600 seconds; defaults to 600 |
| `SLOPSEARX_PORTAL_MEMBERSHIPS_FILE` | Absolute path to the operator-managed JSON directory |
| `SLOPSEARX_WORKFLOW_PORTAL_ACTIONS` | Comma-separated actions; unset is read-only |

Register exactly `${SLOPSEARX_PORTAL_EXTERNAL_ORIGIN}/auth/callback` at the
provider. The provider must advertise PKCE S256 and RS256 signing keys.

The membership file maps an already verified `(issuer, sub)` to internal
principal and tenant records. Mount it read-only and do not commit it:

```json
{
  "principals": [
    {
      "principal_id": "internal-opaque-id",
      "issuer": "https://id.example.com",
      "subject": "provider-subject",
      "revision": 1,
      "enabled": true,
      "memberships": [
        {
          "tenant_id": "tenant-opaque-id",
          "display_name": "Research team",
          "revision": 1,
          "enabled": true,
          "grants": ["workflow.read", "workflow.research.cancel"]
        }
      ]
    }
  ]
}
```

Every request reloads the principal and membership through the resolver
boundary. A revision change invalidates existing sessions. Multi-tenant users
choose only from server-derived memberships; forms never supply authority.

## Two release gates

The read gate uses the feature flag and `workflow.read` membership grant. It
provides bounded cursor pagination and tenant-qualified detail reads. Recent
research uses a 200-entry maintained tenant index. Staged and saved searches
use their existing bounded indexes. Requests perform no wildcard key scans.

Mutations require an action in both the membership and
`SLOPSEARX_WORKFLOW_PORTAL_ACTIONS`. Supported actions are:

- `workflow.research.cancel`
- `workflow.research.retry`
- `workflow.saved.control`
- `workflow.saved.ack`
- `workflow.compose`

Each mutation requires a synchronizer CSRF token, exact same-origin check,
form content type, server-resolved tenant, action grant, opaque idempotency
key, and current object revision. Mutation claims live for ten minutes in
Valkey. Duplicate requests do not repeat the action; changed or concurrent
submissions return 409. Research retries recheck current intent and sensitive
engine policy before dispatch. GET routes never dispatch or mutate records.

Saved-search detail shows the whole bounded tenant event inbox without moving
the acknowledgement cursor. Acknowledgement advances that inbox only to the
cursor displayed by the form, so it cannot silently skip an unseen event from
another saved search. Composition currently admits the approved
`staged_search` to `research` transition through the shared resolver and
research start function.

When the retrieval-receipt grant is enabled, retained research detail also
generates the same bounded research manifest used by MCP. Its manifest items
carry the tenant-scoped retrieval receipts and artifact lineage available at
read time; the read performs no fetch and extends no retention horizon.

The current release supports direct HTTPS exposure only. Forwarding headers
are ignored, the request Host must match the configured external origin, and
startup rejects any `SLOPSEARX_PORTAL_TRUSTED_PROXIES` value. A future proxy
mode requires a separately implemented narrow allowlist and edge validation.

## Privacy and failure behavior

The cookie contains only a random opaque handle. Valkey keys use its keyed
digest. Sessions default to 30 minutes idle and eight hours absolute, with
`Secure`, `HttpOnly`, `SameSite=Lax`, `Path=/`, and no `Domain`. Login state
uses a separate `__Secure-` host-only cookie restricted to the callback path
and expires within ten minutes.

Unknown, foreign-tenant, expired, policy-revoked, and malformed handles share
one 404 representation with `Cache-Control: no-store`. HTML navigation without
a session redirects to sign-in. Explicit machine negotiation returns a 401
problem response with `WWW-Authenticate`. Protected pages escape stored
content and use a restrictive CSP. Audit streams use closed vocabularies and
keyed hashes; they omit queries, contents, subjects, cookies, CSRF values,
authorization codes, and OIDC tokens.

Rollback by unsetting `SLOPSEARX_WORKFLOW_PORTAL_ENABLED` and restarting.
Existing workflows continue through MCP while browser records expire under
their bounded TTLs.
