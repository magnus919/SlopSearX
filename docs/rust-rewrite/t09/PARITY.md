# T09 parity and intentional corrections

Reference: accepted main `1f3f5bc8`, Python 3.14.7, 11 source hashes in the immutable
capture. The parent independently reproduced every capture byte. Source claim
validator acceptance is not evidence that a signature can be forged.

| Contract | Source and native evidence | Deliberate difference |
| --- | --- | --- |
| OIDC identity | Exact issuer (trailing slashes removed), nonempty sub, client audience, multi-audience azp, nonce and 30-second skew; 22 source cases replayed after maintained signing | Numeric finite JSON times only: reject bool/string/nonfinite coercions; reject mixed-type audience arrays. Single-audience irrelevant azp remains accepted. |
| Signature verification | Source fixed public valid/tampered vectors replayed; native independently signed malformed/forged cases | AWS-LC RSA 2048–8192 replaces handwritten modular arithmetic; decoded duplicate keys, unsupported crit/b64, ambiguous kid, mismatched use/alg/key_ops and empty key_ops deny. Size/depth/member limits are explicit new refusal boundaries. |
| Settings/discovery | Six constructor cases, exact callback and HTTPS endpoints, existing lifetime cache retained | Reject URL userinfo; native URL builder preserves existing authorization-endpoint query parameters instead of adding a second question mark. No arbitrary algorithm from discovery/token metadata. |
| Membership | Six source directory cases, known mixed grants/defaults/uniqueness/revision/issuer checks; F06 Secret Debug redacts | Unknown action/grant fails startup per ADR002 (source allowed unused arbitrary strings); identify invalid config before rollout. Directory port cannot substitute requested principal or verified tuple. Bounded 1 MiB directory/1024 principals/128 memberships. |
| Sessions | Seven actual source transitions consumed by native TLS/private-Valkey replay; pending chooser, grant intersection, expiry, principal revocation and missing/unavailable distinctions | Server TIME replaces local authority; exact raw CAS replaces generation-only comparison. Observed revocation CAS cannot delete a newer refreshed record. Bounded typed records reject malformed/coercible fields. |
| Key rotation | Current-only issuance and bounded previous verification; actual independent authorities and exclusive-deadline regression | Drain is initialized using Valkey time, 60–600 seconds, not a process clock. Its environment integer grammar preserves source whitespace, Unicode digits and underscores; decimals/exponents remain invalid. Native namespace requires explicit drain/relogin contract. |
| Return path |13 actual source cases replayed | Three named source-accepted paths (encoded backslash, repeated percent decode and dot-segment escape) refuse to `/workflows`; other safe paths preserve bytes. Other equivalent unsafe encodings are refused by the same bounded rule. |
| Mutation admission | Source AST expression supplies five exact semantic digests; defaults, Unicode and replay across CSRF rotation tested | Framed admission key identities prevent concatenation ambiguity. The source semantic digest retains NUL separators; native routes reject decoded NUL in kind/id/action/cursor/question/strategy before admission to prevent distinct tuples colliding. Ordinary source digests and captures remain unchanged. Browser guards remain 600 seconds pending/done; expired guard is not proof no operation happened. W10 owns durable command replay. |
| CSRF/form | Actual TLS middleware enforces same Origin/Referer and form token | Support ADR002 header alternative; duplicate/conflicting fields/headers and credentials deny rather than source last/first-value collapse. No CSRF/raw HTTP fields enter W10 commands/Debug. |
| Quota | Existing bucket limits and charging law retained | Valkey TIME, atomic increment+expiry and framed identity remove replica-clock/concatenation ambiguity; no F10 quota law changed. |
| Audit | Closed source vocabulary/keyed identity,90-day retention; real stream and wrong-type tests | Exact 10,000 entry cap replaces approximate MAXLEN. Audit error maps to generic 503 rather than uncaught source exception; already-completed operations are not rolled back/retried. |
| Public/protected routing | Public stand-ins unchanged under absent/valid/invalid cookies; actual TLS/cookies and two tenants; uniform 404 objects; unsupported action 403 | These prove the identity mounting boundary, not complete unfinished T02/W10 renderers or production policy dispatch. Direct TLS rejects proxy trust, preserving accepted ADR002/005. |

Bounded record/input refusal, new namespace and stronger authority are intentional
hardening, not source wire-equivalence claims. No source Python runtime or existing
oracle bytes were changed. Failed red repro logs and final native reports are
retained in the task handoff; source fixtures remain separate from native results.
