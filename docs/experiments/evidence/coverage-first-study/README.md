# Coverage-first study preparation

**Status: proposed design and offline preparation; not registered or admitted.**
The [maintainer-approved amendment](../../coverage-first-release-decision.md)
permits preparing this new candidate study. These artifacts are not an invocation
permit, completed experiment, or product qualification.

The question is whether reserving relevant leads for the caller's research needs
helps finish research tasks compared with the configured query-only Jev ranking.
Both development and confirmation have eight new tasks and five exact repository
navigation targets. Each stage has four evidence-seeking and four source-discovery
tasks, balanced between four natural pools of at most 40 results and four natural
pools of 41–80 results. Confirmation has not been acquired or scored.

| Artifact | Role |
| --- | --- |
| [cohorts.json](cohorts.json) | Proposed tasks, caller facets, queries, explicit scopes and navigation targets |
| [cohort-overlap-check.json](cohort-overlap-check.json) | Input-bound metadata comparison against prior declared cases; not proof of corpus independence |
| [protocol.json](protocol.json) | Draft schedule, unchanged gates and limits, with outstanding execution prerequisites |
| [observed-control-baseline.json](observed-control-baseline.json) | Sanitized read-only source/configuration observations; no inference or quality probe |
| [control-source-parity.json](control-source-parity.json) | Source comparison identifying the deployed V1 parser and unchanged projection helpers |
| [w0-rerank-v1.py.txt](w0-rerank-v1.py.txt) | Exact public V1 source bytes for reproducible offline control replay |
| [execution-preparation.md](execution-preparation.md) | Offline-tested transport, archival and paired-consumer seams; remaining invocation prerequisites |
| [candidate-runtime-identity.json](candidate-runtime-identity.json) | Expected public runtime projection; not a live health receipt or admission |

The observed deployments differ: the stable configuration has no Jev reranker,
while the experimental configuration enables it. The primary control is the
configured experimental Jev V1 path; stable presence ordering is diagnostic.
The original service projection and rubric match current main, but its newer
response parser is not substituted for the deployed parser. The replay helper
uses the pinned original V1 bytes and reports strict-parser compatibility and
unknown usage separately.

The new modules are deliberately offline:

- `scripts/coverage_study_core.py` verifies caller-supplied sealed material and
  ordered accounting. A verified summary grants no execution authority and does
  not itself observe the running source tree or dependencies.
- `scripts/coverage_study_acquire.py` exercises current adapters and the shared
  grouping/ranking pipeline through injected recorded mock responses. It is a
  canonical pool seam before W0 reranking, not a live acquisition command.
- `scripts/coverage_legacy_control.py` replays hash-pinned successful responses
  through the original V1 parser without networking. It does not qualify live
  timing, HTTP-failure behavior or all deployment branches.

The draft permits at most 39 declared Jev calls per stage, within the prior
44-call ceiling: two synthetic capacity probes, 16 base research calls, 16
repeat/rotation calls and up to five navigation calls. Existing token ceilings,
per-call reserves, one-second selector phase, no-retry policy and task-use gates
remain. The available token budget does not guarantee completion; missing usage
or an exhausted reserve stops the stage with retained evidence.

Before any scientific call, finish the guarded live acquisition/capture/provider,
answer and assessment driver; freeze exact source/dependency closure, prompts,
assignments, manifests, subgroup decision rules and complete response inventories;
independently review and merge registration/qualification artifacts; and publish
new source-bound admissions. Fresh blind references must precede task scoring.
Both stages must pass before optional product implementation. Failed or
inconclusive results are retained and published, with no automatic rescue study.

[Paired answer and grading preparation](answer-grading-preparation.md) now
composes the once-only stage through a complete synthetic workflow. Its
[validation receipt](answer-grading-validation.json) records the limited offline
evidence. Live capture remains disabled pending the
[fetch-boundary qualification](capture-fetch-boundary-review.md).

No Brave search, source capture, Jev evaluation, answerer or scientific-assessor
call was made in preparing this packet. Coding and review agents helped prepare
the offline artifacts. Runtime metadata checks were read-only; deployments and credentials were
unchanged. Private endpoints, addresses, hostnames, paths and credential values
are excluded from these artifacts.
