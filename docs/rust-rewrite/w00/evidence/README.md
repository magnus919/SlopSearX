# Superseded calibration failures

These files are historical failed observations, not current acceptance reports.
`source-before-claim-rehydration.json` is the initial unqualified capture. Its
stale-clock `run_id_after` hashed cjson's raw integer timestamp. The source store
actually calls `definition_from_payload` before the runner, producing a float.
Current `../source.json` fixes that single field using the actual source helper;
all original model/policy/strict/control rows and other five claim rows remain
unchanged. Historical corpus SHA256: `b31b4f5cda7ffb258db8423b7461fdf7cb20ec8a1be55376af75a09a3b9f437c`.

`domain-boundary-red.txt` preserves the three independently reproduced failures
(finite subtraction overflow manufactured slot zero, oversized window macro
panic, oversized identity macro panic). `fingerprint-boundary-red.txt` preserves
the equivalent policy-identity macro panic. These failures were retained before
the local fallible window/direct identity/finite-intermediate corrections.
Final native qualification is separate and binds the corrected source.

`review-correction-red.txt` retains the independent four-counterexample native
reproduction (live claim baseline, exact revision increment and both configured
query-limit cases), before the focused corrections. It is historical failure
evidence, not a current acceptance result.

`final-review-two-red.txt` retains the subsequent independent MAX lifecycle and
nonobject-baseline failures before the final correction. These are historical
negative results, not current qualification receipts.
