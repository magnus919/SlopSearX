# T05 source parity and intentional hardening

Source: accepted main 403dfd37, pinned Python3.14.7/MCP2.3.0/FastMCP4.0.11.
Native replay compares full semantic JSON/text, status and selected headers.

| Contract | Native behavior and evidence |
| --- | --- |
| Discovery/DCR/client authentication | Source metadata, nullable registration fields, Basic/post/public clients, malformed forms and CORS wire observations replayed. No invented OIDC discovery alias. |
| Authorization | Source exact redirect selection, explicit-vs-implicit distinction, scope checks, state and redirect errors. Empty challenge is intentionally rejected before code creation (source minted an unusable code). |
| Resource | Foreign resource rejected before authorization; resource-bound grants remain bound at exchange and bearer admission. Source accepted foreign resource then ignored it at bearer admission. Intentional hardening preserves T04's resource retention. |
| Token/refresh/revoke | Source verifier/redirect/client authentication/scope/replay/error/cache headers, refresh pairing and revocation. Malformed explicit redirect must reject 400 and preserve implicit code before valid redemption. |
| Network auth | Every supported method/path/session request authenticates. Source returned 405 before auth for GET/HEAD/PUT and its SSE CLI branch omitted auth. Native 401-before-dispatch and legacy SSE auth are intentional corrections; trusted stdio remains unchanged. |
| Ambiguous/bounded input | Duplicate credentials/content types, excessive bodies/fields/header lengths and malformed multipart refuse. These are explicit new refusal boundaries, not claims of source equivalence for hostile inputs. Ordinary source multipart forms remain supported through maintained Axum/multer. |
| Durable authority | T04 exact CAS/TTL/paired-token invariants retained. Valkey clock port is additive; no local fallback or automatic unknown-write retry. Client metadata authentication read is distinct from atomic durable grant/client TTL commit. |

First wire replay exposed native404/CORS mismatch and exporter multipart opaque-ID
normalization bug; both were corrected with source observations unchanged. Parent
review then exposed IPv6 loopback refusal and malformed optional redirect collapse;
red evidence and the additional actual source observation remain retained.

Persisted client authority is checked narrowly at authentication: missing/null
secret expiry preserves source optional behavior; present expiry requires a
nonnegative integer. Missing/null/unknown/wrong-typed auth method denies, matching
the source absence refusal (it never defaults to post during stored-client reads).
Malformed persisted authority maps to generic 503 without echoing stored fields,
rather than coercing corruption into no-expiry or leaking an unknown method.
This is explicit corruption hardening, not a full unrelated metadata schema.
Actual private-Valkey tests preserve the original code and client bytes, then
restore the exact valid record and redeem successfully. Fixed source helper
observations are retained in the review packet; the original 70 wire observations remain unchanged.

Independent full review found four additional source-wire differences. The source
full-client model replaces empty optional text and `jwks` placeholders with their
null defaults before persistence; response serialization omits those null fields.
Native registration now follows both projections, with actual-store assertions.
Authorization shape validation aggregates field errors in model order and selects
`unsupported_response_type` whenever that literal fails. Token authentication
remains first, then selected-grant shape validation precedes registered-grant
permission checks and all code/token lookup. Unsupported-grant diagnostics retain
the source supported-grant list using the accepted Python value formatter.

Ten actual loopback source observations were appended after the original 70,
preserving every original request, response and opaque identifier alias. The
retained native red replay reproduces all four review findings; the corrected
80-case replay and an actual-Valkey no-code-consumption/recovery test qualify the
correction. These are source-compatibility fixes, not new intentional differences.

Correction recheck exposed an unchecked diagnostic index when persisted grant
metadata was absent. Native token reads now share one typed effective-grant
projection for permission and diagnostics: missing/null/empty-string placeholders
use the SDK default authorization-code plus refresh grants; an explicit empty list
denies all grants, and arbitrary string grant names remain unchanged. No durable
record is rewritten. Wrong typed persisted grant metadata produces generic503
rather than the source model-validation500, an intentional corruption refusal.
Actual-Valkey regression covers nine stored variants, structured responses,
unchanged code/record after rejection, and successful redemption after exact
record restoration. The prior80 source observations and exporter remain unchanged.

Stored authorization-code integrity: native read and redemption repeat the creation-time text bounds for redirect URI and PKCE challenge. Corrupt empty, control-bearing or oversized values return a generic unavailable response without consuming the code or issuing tokens. The real endpoint/private-Valkey regression includes an implicit redirect whose corrupt stored URI previously bypassed the request redirect comparison; restoring the original record permits redemption. This is deliberate fail-closed durable-state hardening.
