# EXP-074 harness qualification

Qualified source: `46f61dfa07a40f089ed8cb3d02b5fa672dccc6cc`.
The committed integrity manifest binds eight owned files and 19 transitive
source/input files, the sealed protocol and registration, CPython 3.14.7,
httpx 0.28.1, httpcore 1.0.9 and anyio 4.14.1.

Root independently ran the following offline commands:

```sh
python3 docs/experiments/evidence/EXP-074/normal-runner-tests.py.txt
python3 docs/experiments/evidence/EXP-074/analysis-tests.py.txt
python3 docs/experiments/evidence/EXP-074/verifier-tests.py.txt
python3 docs/experiments/evidence/EXP-073/fusion-tests.py.txt
git diff --check
```

All passed: 16 runner, 17 analyzer, 16 verifier and three fusion tests. These
cover frozen request identity, local-only default, redirects, HTTP errors,
decoded overflow, credential-echo suppression, cancellation, partial timeout,
missing usage, predispatch deadlines, interrupted/fallback receipt binding,
tampering, source closure, lease-before-I/O admission and terminal stopping.
Complete synthetic schedules receive no quality credit.

Independent review found two material admission/verification defects; fixes and
the bounded follow-up are recorded in [implementation-review.md](implementation-review.md).
The earlier unqualified offline attempt is retained separately, with its request
count and usage unknown. Passing qualification does not erase that incident.

This qualifies the harness mechanics, not the selector or a production change.
One admitted live comparison remains subject to the frozen 44-call schedule,
one-second whole-phase deadline, unchanged quality gates and no retries.
Development success would still require genuinely untouched confirmation and
the full production-acceptance checklist before implementation and adoption.
