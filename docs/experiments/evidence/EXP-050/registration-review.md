# Prospective registration review

Luna reviewer production_surface_audit independently verified all 17 source pins and reported that exact ExternalIds namespaces, case-insensitive paperId matching and leading-zero handling resolve the request/response mapping ambiguity.

## Finding retained

The earlier wording set a twenty-second whole-operation timeout while allowing up to three HTTP attempts and thirty-second retry delays, leaving the elapsed-time bound ambiguous. The reviewer also required that the published transport fingerprint exclude API-key material.

## Root resolution before registration

The protocol now sets an absolute monotonic twenty-second deadline per attempt, including streamed response reads, and a 120-second absolute deadline for the full run including backoff and parsing. A retry can begin only if its delay and complete next-attempt allowance fit. The contract records those limits. The fingerprint covers credential-free transport source bytes and excludes key values, credential-bearing configuration and derived key digests.

This records the independent findings already delivered and root's resolution. Final independent review is retained in registration-review-final.md. It found no acquisition-protocol blocker and requested a wording correction to avoid overstating incomplete EXP-048 outputs. Root applied that correction before freezing registration. No lookup, implementation or grading occurred during registration preparation.
