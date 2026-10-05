# EXP-076 harness qualification

The comparison harness is qualified against source revision
`d39910541fa85ac07efd73a885f66803ceecabaa`. This qualifies the integrity of
the registered comparison, not ranking quality or production adoption. No
provider or search call has occurred in EXP-076.

The committed manifest binds seven owned files and 24 reachable source/input
files. Runtime: CPython 3.14.7, httpx 0.28.1, httpcore 1.0.9, anyio 4.14.1.
All eight frozen input pins and 21 prepared operations were independently checked.
The immutable registration remains byte-identical.

Root independently passed 13 runner tests, four analyzer tests and ten raw
verifier tests on integrated committed source. The complete 44-call local
fixture exercises actual source-closure checks and both separate assessor
paths, with fixture-only output receiving no quality credit. Coverage includes
full membership and exact request projection, tie/fusion behavior, native
fallback, known and unknown usage, transport cleanup, cap-plus-one overflow
evidence, build overruns, dispatch denial after client creation, final-fsync
deadlines, call-sidecar mutation and source-pin tampering.

The substantive runner and verifier reviews are retained in
[implementation-review.md](implementation-review.md). Their material findings
were corrected and covered by regressions. The first integrated verifier
checkpoint failed receipt inventory validation; the correction preserves
exact inventory validation and the failed checkpoint remains disclosed.

[admission-smoke.json](admission-smoke.json) records an unmocked committed-source
and manifest check using only a temporary lease and DNS/credential sentinels.
It issued no live capability, read no credential and touched no real study
lease. A duplicate admission was rejected before either sentinel.

The registered one-shot live comparison is now eligible for admission. It must
revalidate the committed qualification, consume the durable study lease once,
retain every failure and unknown usage, and never retry. Only a complete result
can be assessed against the frozen development gates. Genuinely untouched
confirmation, task-use evidence and all production acceptance requirements
remain mandatory before implementation.
