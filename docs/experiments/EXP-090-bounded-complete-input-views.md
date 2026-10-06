# EXP-090 — bounded complete-input views

Status: first setup stopped before grading; corrected source must be qualified before a fresh setup.

EXP-089 received the assessor’s actual final JSON, but its required source array was empty. Its [terminal diagnostic](evidence/EXP-089/completed/readout.md) shows truncated bulk reads and a subsequent lossy preview. This stage tests a narrow delivery change: addressable views of the exact original assessment packet. It does not assume truncation caused the empty response.

The viewer divides original UTF-8 bytes at whole-line boundaries into bounded pages. Concatenating every page reproduces the original input byte-for-byte. Each tool response contains at most two pages within a fixed output-size limit; no source, passage, metadata field, reference label, order or character is selected, summarized, clipped or rewritten. The controller verifies the actual tool results against the expected byte-exact renderer and requires complete page coverage before accepting an assessment. Reader assertions and plausible summaries do not establish delivery.

The actual final JSON carrier and unchanged reference/schema/coverage validators remain. All sixteen validated A/B card references are still carried as immutable development input. All source and answer assessments are fresh; failed source outputs are excluded. The original candidate, source packet grouping, evidence, rubric, numerical gates and wholly fresh untouched confirmation stay fixed. There are no new searches or source fetches.

Historical grader calls total eighteen. Eighty fresh source/answer assessments yield ninety-eight total, within the original 112-call ceiling; adding the eighteen answer API calls yields 116 within 130. No failed call is refunded. A new qualification, manifest, clock and lease are required; earlier authority and elapsed time receive no credit.

The discriminating probe is the first fresh source assessment with verified complete input delivery. If the worker still cannot produce a valid complete assessment after full delivery, stop this non-converging harness diagnosis and report the remaining model, task or context evidence gap. Do not weaken acceptance criteria or claim research improvement from structural tests.

See the [protocol](evidence/EXP-090/protocol.json) and [production acceptance checklist](complete-pool-production-acceptance.md). This remains an experiment, separate from mainline GroktoCrawl and production implementation.

The [first setup receipt](evidence/EXP-090/setup-only/readout.md) retains the controller-to-preparer proof mismatch. Health passed; no grading or quality analysis occurred. Its clock and lease are terminal and receive no credit in the corrected setup.
