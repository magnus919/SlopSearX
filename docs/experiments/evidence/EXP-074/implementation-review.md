# EXP-074 implementation qualification review

Reviewed integration: `b280cc2ce4c76161863edf059f773694d1c7675c`.
This is unfinished qualification, not a live comparison or selector acceptance.

Independent review checked the offline default, immutable source closure,
one-shot admission, deadlines through durable receipts, terminal stopping,
usage uncertainty, interrupted responses, exact fusion and unchanged quality
gates. Root independently ran 12 runner, 17 analyzer, 15 raw-verifier and three
inherited fusion tests successfully with CPython 3.14.7, httpx 0.28.1,
httpcore 1.0.9 and anyio 4.14.1. These are fixture results with no quality credit.

Two material findings remain open before qualification:

1. Independent review found that live admission resolves DNS and reads the
   credential before acquiring the exclusive study lease. Lease rejection must
   precede both actions. The correction must retain the lease after any later
   setup failure and include tests prohibiting DNS and credential access on
   rejection.
2. Root found that raw verification passes symbolic `HEAD` to a helper requiring
   a literal commit SHA. Use the resolving helper and add coverage of the
   non-synthetic qualification path; synthetic receipt tests cannot prove it.

The durable candidate-body receipt is distinct from the final outcome receipt.
Review confirmed that the phase endpoint follows candidate-body fsync, while a
missed deadline produces complete incumbent fallback and a separately bound
failure receipt. An over-deadline candidate receives no valid result credit.

The earlier unqualified offline network escape remains recorded in
`offline-qualification-incident.json`; its unknown request count is not replaced
by the later fixture test results. No qualification manifest or admitted live
comparison exists at this review checkpoint. Fixes and their verification will
be recorded below before admission.

## Findings addressed

Integrated corrections at `46f61dfa07a40f089ed8cb3d02b5fa672dccc6cc`:
the runner creates and fsyncs its exclusive lease before DNS or credential
lookup; an existing lease rejects before either action, and subsequent setup
failure keeps the lease. Raw verification resolves current HEAD before passing
a literal revision to the committed-byte helper. Independent bounded follow-up
verified both corrections and found no remaining material blocker.

Root reran 16 runner, 17 analyzer and 16 verifier tests after integration;
all passed. The three unchanged fusion tests also passed (52 total). The added
admission tests use temporary leases and DNS/environment sentinels; the verifier
regression covers the non-synthetic qualification closure with mocked committed
fixture bytes. They did not perform real DNS, read a credential or call a provider.

The private launch wrapper was also inspected without execution. It resolves the
credential lazily within `live_gate`, after exact source validation, durable
lease acquisition and DNS; it does not load the key before admission checks.
