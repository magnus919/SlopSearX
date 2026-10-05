# EXP-075 harness qualification

Qualified source: `f31a06d39853bb113afbe68846a368d4b51ac34c`.
The integrity manifest binds eight owned files, 21 source/input files, the
sealed registration and protocol, CPython 3.14.7, httpx 0.28.1, httpcore 1.0.9
and anyio 4.14.1.

Root independently ran the offline runner, analyzer and verifier test files
and inherited EXP-073 fusion tests, using that CPython runtime. All 37 tests
passed: 14 runner, six analyzer, 14 verifier and three fusion checks.
`git diff --check` also passed.

Coverage includes frozen request parity, 120-question construction, separate
score ranges, strict JSON/membership/usage parsing, exact rational rank fusion,
complete native fallback, measured build timeout without dispatch, local-only
fixtures, client closure, qualification before lease/DNS/key, one-shot admission
and fixed lease identity, next-call usage reservation, exact receipt/source
binding, required terminal artifacts, receipt tampering, phase-specific
failure analysis, independent A/B gates and synthetic/incomplete no-quality
policy. The independent full local fixture smoke completed 44 exchanges and
closed all clients; it supplies no provider capacity or ranking-quality credit.

Substantive review findings and bounded follow-ups are retained in
[implementation-review.md](implementation-review.md). The earlier offline
builder failure is retained separately; passing tests do not erase it.

This qualifies experiment mechanics only. One live comparison remains subject
to committed-source admission, the fixed 44-call ceiling, one-second candidate
phase including construction and durable receipts, independent control timing,
unchanged quality gates and no retries. A qualifying development result would
still need untouched new tasks and pools, plus the full production acceptance
checklist. No production selector, default change or deployment is delivered.
