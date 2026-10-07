# EXP-093 — fixed-width citation codes

Status: terminal; assessment inconclusive.

[EXP-092](EXP-092-explicit-citation-aliases.md) completed byte-exact delivery and copied every source/header identity correctly, but one of 85 citation occurrences omitted the last character of a hash-bearing alias. The strict decoder rejected it. This continuation changes citation-reference representation; it does not complete, repair, reuse or score that failed response.

Build one complete immutable codebook per research task before dispatch from all six source-job bindings. Assign short fixed-width codes in the canonical order of the existing aliases, preserving catalog order across every role and subset. Source graders, the answerer and answer assessors use the same table. A code maps strictly to its existing alias and original evidence identity. Unknown, malformed, truncated and cross-scope codes fail; no prefix, fuzzy, case or inferred-ID repair is permitted.

Only declared citation-reference fields and catalog keys change on the model-facing wire. Source text, URLs, task/identity fields, other metadata, arrays and catalog relative order remain unchanged. The inverse must reproduce the original canonical input exactly. Model packet bytes consequently differ; the original frozen 513-file bundle remains unchanged. Codebook hashes bind the prepared inputs and paired decoder receipts before any model call. Cryptographic evidence identity remains in the validator, not in the copied reference code.

All sixteen validated card assignments remain immutable and use their existing labels. All 48 source and 32 answer assessments are fresh. Candidate, rubric, requested GPT-6 Luna model, numerical/resource gates and wholly fresh untouched confirmation stay fixed. Twenty-one historical grader calls plus eighty fresh assessments total 101 within 112; eighteen answer API calls yield 119 within 130. Failed calls are never refunded. No new search or source fetch is allowed.

The frozen-input codebook construction, all 48 input round trips, source/answerer/answer-assessor integration, strict failure cases, exact slot bindings, and controller/dispatch/viewer path were exercised offline before admission. Those checks establish harness integrity, not assessment quality. Research usefulness and production readiness still require the [acceptance checklist](complete-pool-production-acceptance.md).

## Readout

[Terminal readout](evidence/EXP-093/completed/readout.md) records the one fresh source-assessment attempt. The full input-view transcript passed its byte-exact coverage check, but strict citation validation rejected the response because one citation crossed source scope. Of 127 citation occurrences, 126 were valid for the expected source and one was not. No source assessment was accepted, no grading was repaired, and no quality result was produced. The run is terminal; there is no follow-on study proposed here.
