# EXP-076 implementation review

Status: in progress; no source qualification or live admission.
The registration-pinned design and historical evidence remain unchanged.

The unchanged EXP-075 HTTP transport was reviewed by GPT-6 Luna against the
new contract. Its fresh-client, no-proxy/redirect/retry behavior, streamed 2 MB
response cap, exact-key echo suppression and cleanup are reusable with an exact
source pin. New orchestration tests still must cover the 160-question shape,
response overflow, streamed cancellation/unknown usage, split-marker echo,
proxy/redirect sentinels and the full construction-through-fsync deadline.

Root's first read of the unfinished request/parser source identified an
undefined `p` in `parse_candidate()`'s successful return (`p['usage']`). The
validated local variable is `usage`. The finding was sent to the owning agent;
successful full candidate parsing must have an offline regression that reaches
the actual return path. This draft observation is not a test failure or a
qualification result. Final review must bind the corrected committed source.

Root then ran a credential-free synthetic build/parse smoke against the draft.
Imports and pinned dependencies loaded, but `build_candidate()` raised
`KeyError: 'purpose_question_prefix'`: it looked for the prefix in EXP-076's
protocol rather than the exact pinned EXP-075 source designated by the sealed
wire contract and capacity preview. Correct the implementation source lookup,
not the immutable registration. The smoke dispatched no request and never
reached parsing. Complete build/parse and preview-byte regression coverage is
required before source qualification.

## Subsequent draft smoke

After both source fixes, root passed 52 natural build/parse checks (thirteen
pools under four permutations), with candidate bodies equal to the sealed
calculator's projection. A 160-answer synthetic response also parsed fully.
These are local structural fixtures, not ranking evidence or final source
qualification.

The synthetic smoke exposed a further mismatch: the new builder's fixture
produced a 267,151-byte request rather than the sealed preview fixture's
270,847 bytes. The maximum shape has 160 questions in both cases, but its exact
inventory/context differs. The owning agent must use the registered preview's
exact synthetic cards and context before neutral admission. Do not revise the
immutable preview or treat a shorter substitute as its capacity test.

## Initial runner inspection

The unfinished runner is explicitly offline-only. Root identified five further
requirements before a usable checkpoint or qualification:

1. Measure baseline duration after its receipt fsync and candidate duration
   after final receipt fsync; the draft measured both earlier.
2. Persist call-record timing changes before binding sidecar hashes so embedded
   calls and durable sidecars remain identical.
3. Avoid appending one physical call twice when a completed baseline later
   fails its whole-phase deadline.
4. A candidate deadline failure must produce native incumbent fallback and a
   failed receipt, never retain valid candidate output with a complete receipt.
5. Remove the unregistered two-second paired-wall stopping rule. The sealed
   design gates each complete durable phase separately and records paired wall
   time as an observation.

These findings were sent to the source owner. Final regression and exact-source
review must establish their disposition before live admission.

## Initial evaluator unit checks

Root independently ran all four drafted metric tests: both reference case
assembly, synthetic quality suppression, identity displacement and pinned-input
tamper rejection passed. This bypasses the not-yet-built raw verifier and is
unit evidence only. Root also checked that source-bound and metric-view lead
grades agree for every frozen ID. The identity regression needs strengthening
to hold useful count and facets constant while displacing more than one useful
source, without a silent early return. The complete receipt/source-closure
integration regression remains mandatory.
