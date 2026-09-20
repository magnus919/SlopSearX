# EXP-004: Jev reranking for official-documentation navigation

## Registration (2026-09-20)

- State: registered
- Owner and related issue: Codex research spike for
  [#389](https://github.com/magnus919/SlopSearX/issues/389).
- Baseline: `d4fc9b741795de55216ad49b1ac9d7c8a39d11e0` (`origin/main` at
  registration). EXP-001 through EXP-003 are complete. A pre-registration
  synthetic connectivity smoke test returned HTTP 200 from `jev-1.13.0`; it is
  excluded from every metric in this experiment.
- Problem and observed evidence: SlopSearX presence ranking rewards URL overlap
  rather than semantic relevance. RRF improved two of three synthetic repository
  cases and regressed the third. Those cases do not establish production
  relevance. Issue #389 asks whether Jev may add optional value without
  authorizing a feature.
- Intended beneficiary and project objective: a portal or MCP user issuing a
  navigational query for an official technical reference. The bounded objective
  is to put the known official page earlier in an already-retrieved candidate
  set. This does not measure broad search quality, human demand, answer quality,
  or non-English behavior.
- Hypothesis: for the fixed 12-query official-documentation corpus below, a
  Jev `noul` relevance score applied after deterministic collection will improve
  mean reciprocal rank at 20 (MRR@20) of the canonical official target by at
  least **0.10 absolute** over the stronger aggregate deterministic baseline
  (presence or RRF), without violating the guardrails below.
- Candidate: collect at most 10 results each from the existing DuckDuckGo,
  Google, Wikipedia, and Stack Exchange adapters; merge the union through the
  repository's unmodified presence and RRF rankers; send at most the first 30
  distinct RRF candidates as one bounded Jev request per query. State contains
  only the query plus candidate index, title, URL host/path, and at most 500
  characters of adapter-provided snippet. Ask one independent `noul` relevance
  question per candidate. Sort by probability descending with original RRF rank
  and normalized URL as deterministic tie-breakers. Do not cross tier boundaries,
  fetch pages, or alter source code.
- Model: pin `jev-1.13.0`; do not use the moving `jev-latest` alias for measured
  calls. Record the resolved model from every response.
- Primary metric: MRR@20 over the 12 fixed queries. A target absent from the
  collected union or below rank 20 scores zero. URL matching lowercases the host,
  removes `www.`, removes query/fragment and trailing slash, and accepts only the
  registered host plus path prefix. The comparison baseline is whichever of
  presence or RRF has higher aggregate MRR@20; it is selected only by this fixed
  rule, not after inspecting individual cases.
- Minimum practically useful effect: +0.10 absolute MRR@20. An external model
  must move an official target roughly one or more useful positions across this
  small corpus to justify further investigation; a smaller effect does not
  justify the new dependency from this pilot.
- Guardrails:
  - at least 8 of 12 canonical targets must appear in the collected candidate
    unions, otherwise the experiment is `inconclusive` because reranking lacks
    enough retrieval coverage;
  - at least 10 of 12 measured Jev calls must return HTTP 200, the pinned model,
    all requested typed answers, finite probabilities in `[0, 1]`, and usage;
  - no present target may move down by more than five ranks versus the stronger
    deterministic baseline for that query, and no more than two present targets
    may move down at all;
  - measured Jev p95 end-to-end latency must be <=1.5 seconds and maximum latency
    <=3 seconds; acquisition-engine latency is reported separately;
  - no result membership, adapter response, tier, URL, title, snippet, or
    provenance mutation is allowed; only a derived experimental order is made;
  - raw evidence must contain no API keys, authorization headers, personal
    queries, sensitive-engine output, cookies, or fetched page bodies;
  - no repository source, configuration, dependency, workflow, portal, API, MCP,
    cache, snapshot, policy, or filter behavior may change.
- Fixed corpus and canonical targets:

  | ID | Query | Accepted canonical target |
  | --- | --- | --- |
  | q01 | `Python asyncio TaskGroup documentation` | `docs.python.org/3/library/asyncio-task.html` |
  | q02 | `Python pathlib documentation` | `docs.python.org/3/library/pathlib.html` |
  | q03 | `FastAPI response model documentation` | `fastapi.tiangolo.com/tutorial/response-model` |
  | q04 | `PostgreSQL SELECT command documentation` | `postgresql.org/docs/current/sql-select.html` |
  | q05 | `Rust std Option enum documentation` | `doc.rust-lang.org/std/option/enum.Option.html` |
  | q06 | `npm package.json documentation` | `docs.npmjs.com/cli/` path ending in `/configuring-npm/package-json` |
  | q07 | `GitHub Actions workflow syntax documentation` | `docs.github.com/` path ending in `/workflow-syntax-for-github-actions` |
  | q08 | `Docker Compose file reference` | `docs.docker.com/reference/compose-file` |
  | q09 | `Kubernetes Deployment documentation` | `kubernetes.io/docs/concepts/workloads/controllers/deployment` |
  | q10 | `Prometheus querying basics documentation` | `prometheus.io/docs/prometheus/latest/querying/basics` |
  | q11 | `MDN Fetch API documentation` | `developer.mozilla.org/` path ending in `/Web/API/Fetch_API` |
  | q12 | `Terraform lifecycle meta-argument documentation` | `developer.hashicorp.com/terraform/language/meta-arguments/lifecycle` |

  Path suffix rules for q06, q07, and q11 tolerate documented locale/version
  prefixes but not a different page. Redirects are not resolved.
- Dataset origin and rights: query strings and target identifiers are authored
  for this experiment and contain no personal data. Search-result evidence is
  captured live from existing adapters, retains source URLs and bounded snippets
  solely for reproducibility, and must not be republished beyond what provider
  terms allow. If redistribution is unclear, preserve hashes and derived metrics
  rather than raw snippets in the repository and mark the legal gap.
- Development/evaluation split: there is no tuning set and no prompt iteration.
  The exact question wording, candidate truncation, sort, thresholds, and corpus
  are frozen here before measurement. This is an exhaustive fixed-corpus pilot;
  no population confidence interval or generalization claim is permitted.
- Sampling and repetitions: one acquisition and one Jev request per query,
  processed in the fixed q01-q12 order. Do not retry measured Jev failures.
  Adapter-internal documented fallback is allowed and recorded. No query is
  added, removed, or replaced after seeing results.
- Fixed stopping rule and resources: stop after 12 Jev calls, 500,000 Jev input
  tokens, estimated Jev spend of $0.02, the first suspected secret exposure, a
  correctness/contract guardrail breach, or 45 minutes of measured execution.
  Network/provider failure is evidence, not permission to extend the sample.
- Exact planned command:

  ```console
  set -a; source /Volumes/tank01/magnus/git/SlopSearX/.env; set +a
  .venv/bin/python /private/tmp/exp004_jev_rerank.py \
    --repo "$PWD" \
    --output docs/experiments/evidence/EXP-004 \
    --model jev-1.13.0 \
    --max-candidates 30 \
    --max-snippet-chars 500 \
    --request-timeout 3.0
  unset TYPESAFE_API_KEY
  ```

  The inert reproduction source and exact environment will be copied into the
  evidence directory after execution. The harness must import and use the
  repository adapters and rankers; it may not reimplement their parsing or rank
  arithmetic. Run `pytest --no-cov -q -s tests/test_rank_fusion.py
  tests/test_merger.py` after measurement as a regression check.
- Evidence retention: `docs/experiments/evidence/EXP-004/` will contain the
  sanitized harness, environment, per-query engine outcomes, bounded candidates,
  provider responses with secrets absent, metric summary, command statuses, and
  SHA256 manifest. If raw snippets cannot be redistributed, retain a sanitized
  manifest with hashes and derived fields and explain the omission.
- Portal impact: none during the experiment because no production path changes.
  A later proposal would require explicit external-processing disclosure,
  ranking provenance, fallback diagnostics, accessibility review, and portal
  contract/browser coverage.
- Decision rule:
  - `supported`: completed fixed corpus; primary gain >=0.10; every guardrail
    passes. This supports only a separate, broader confirmation experiment or
    implementation proposal discussion—not shipping.
  - `not-supported`: completed fixed corpus with adequate coverage and valid
    calls, but the primary threshold is missed or a relevance-regression,
    latency, or cost guardrail fails.
  - `inconclusive`: fewer than 8 targets are retrieved, fewer than 10 valid Jev
    calls complete, legal/terms uncertainty prevents necessary evidence, or the
    fixed corpus cannot resolve the effect.
  - `blocked`: the registered harness cannot execute because of infrastructure,
    access, or repository failure unrelated to the candidate.
- Registration commit: recorded after the signed registration commit; do not
  amend the frozen plan.

## Readout

### Inconclusive (2026-09-20)

The frozen run completed with exit zero, but the acquisition layer supplied no
usable evaluation corpus. None of the 12 canonical targets appeared and only
one query produced any candidate at all. DuckDuckGo reported challenge walls on
both HTML frontends for every query, Google reported blocked for every query,
Wikipedia returned HTTP 403 for every query, and Stack Exchange produced one
result across the corpus. This failed the preregistered minimum of eight
retrieved targets before Jev ranking quality could be measured.

The harness made the registered one request per query without retries. Eleven
empty-candidate requests returned HTTP 422 because their question maps were
empty. The one non-empty request returned HTTP 200 from the pinned
`jev-1.13.0`, with a complete typed answer. These 422 responses are a downstream
effect of empty acquisition, not evidence of Jev unreliability. There were only
393 billed input tokens and 23 output tokens, for an estimated Jev cost of
`$0.000016506` at the registered price. Observed request latency was 130.0 ms
mean and 318.4 ms maximum/p95 by nearest rank, but only one request exercised a
real candidate, so this is not a representative latency estimate.

Presence, RRF, and Jev MRR@20 were all zero because target coverage was zero.
The primary `+0.10` effect therefore cannot be evaluated. The exact registered
outcome is **inconclusive**, not `not-supported`: the failed coverage and valid-
call guardrails prevent interpreting the zero metric as evidence for or against
Jev reranking.

No prompt, corpus, engine set, timeout, query, target, or decision rule was
changed after registration, and no measured request was retried. No source,
configuration, dependency, API, MCP, portal, cache, snapshot, policy, or filter
behavior changed. A future attempt is justified only with a frozen,
redistributable captured candidate corpus or an environment already proven to
return adequate candidates; it must be a new linked experiment with a new
registration rather than a rerun of EXP-004.

Evidence: [summary](evidence/EXP-004/summary.json),
[per-query sanitized rows](evidence/EXP-004/rows.json),
[exact harness](evidence/EXP-004/harness.py.txt), and
[checksums](evidence/EXP-004/SHA256SUMS.txt). Snippet bodies were not retained;
the rows contain their lengths and SHA256 hashes. The rotated API key and
authorization headers were never written to evidence.

Decision: do not open an implementation issue from EXP-004. Preserve the
provider/architecture discovery separately and consider a captured-corpus
confirmation experiment only after its provenance, rights, labels, and
candidate coverage are established.
