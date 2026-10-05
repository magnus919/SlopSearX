# EXP-071 numeric admission qualification

Root independently passed all 13 runner and 16 analyzer tests. The actual offline CLI completed the full 21-operation/43-call schedule with synthetic_fixture_only. All 21 W0 and all 21 full-pool Choice bodies are byte-identical to EXP-070; source-pinned request controls record the comparison. No live calls or searches were used for qualification.

One bounded independent Luna pass found no blocking issue. The exact registered schema, doc slug and all three registration pins match the live admission gate. Functional differences from EXP-070 are limited to the registered four-binary64-ULP selected-label numeric check and experiment namespace; raw probabilities, normalization, support/conditional math, sorting, inputs, quality gates and limits remain unchanged.

Fixtures cover the observed one-ULP pair, ties, four versus five ULPs, meaningful wrong argmax, strict malformed keys/type/range/bool/nonfinite handling, unchanged policy outputs, unknown-usage fail-stop/raw receipts, remaining child timeout, candidate-phase overrun and full fresh-W0 fallback. Independent analysis recomputes distributions and ordering, requires all 43 reported usages/resource admissions and full schedule/membership/deadlines, and keeps synthetic output outside quality/adoption.

Root additionally parsed the actual previously failed EXP-070 response under the corrected validator: all 29 IDs and known usage validate. This is prospective validator regression evidence only; EXP-070 remains incomplete and earns no quality credit. The pinned prior-response-regression receipt records the source hash.

Qualification establishes bounded mechanics, not ranking uplift. Untouched confirmation and the full production acceptance checklist remain required. No runtime, ordinary/default path, deployment or Hermes changes were made.
