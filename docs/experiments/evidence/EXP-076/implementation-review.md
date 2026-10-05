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
