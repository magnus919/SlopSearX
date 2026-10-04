# EXP-038 results

The candidate did not meet the registered ranking-quality gate. It is not adopted for ordinary search. All 32 provider attempts qualified; none were retried or replaced. No Brave searches were made.

| Original query | Root nDCG delta | Luna nDCG delta | Root useful@10 | Luna useful@10 |
| --- | ---: | ---: | --- | --- |
| q1 | +0.0327 | -0.0885 | 10→10 | 10→10 |
| q2 | +0.0148 | -0.0783 | 10→10 | 10→10 |
| q3 | +0.0379 | +0.0475 | 10→10 | 10→10 |
| q4 | +0.0364 | +0.0441 | 10→10 | 10→10 |
| q5 | +0.0431 | +0.0431 | 10→10 | 10→10 |
| q6 | -0.0284 | -0.0575 | 10→10 | 10→9 |
| q7 | -0.2163 | -0.2538 | 10→10 | 10→10 |
| q8 | -0.0712 | +0.0379 | 10→10 | 10→10 |

Paired q1–q8 mean nDCG delta is −0.018892 under the root reference (95% query-bootstrap interval −0.084903 to +0.028840) and −0.038198 under the Luna reference (−0.113822 to +0.026109). Neither clears the registered +0.05 material threshold. These intervals do not establish a statistically certain regression. Both references are exposed assistant assessments, not human gold. Top-ten useful counts are unchanged under root and decline by one under Luna; all useful baseline facets remain represented.

The separately registered climate extension q9 improves +0.141140 root and +0.055170 Luna, with useful@10 rising 9→10 under both. It cannot rescue the primary result. All five exact-navigation targets rank first under both contracts; repeat overlap is 0.9–1.0. All eight guard checks pass.

The provider reports 588,115 input and 15,588 output tokens across 32 requests. Maximum complete remote HTTP evaluation time is 622.94 ms; maximum proxy wall time is 945.98 ms. All calls were serialized with a one-second total remote evaluation alarm, no retry and no model substitution. Monetary cost is unmeasured. Wrapper startup and remote HTTP timings are reported separately; this is not a deployed-search latency benchmark.

## Reproduction and integrity

Registration: `821b1b7`; original qualified tooling: `91916de`; bodies frozen before dispatch: `1076d95`. Prepared manifest SHA-256: `5376c98fc1638e88bbf23d3c23e982488131d53786baf1570b00922f79c59414`. The local Python 3.12 environment passed the replay self-test and all 11 offline fixture tests before calls. Preparation froze 32 bodies; verifier after execution confirmed all 32 bodies and all 32 receipts with no errors. The four repeats are exact payload duplicates. Independent serialized input membership and byte bounds remain unchanged.

Commands: `python replay.py.txt --self-test`; `python fixture_tests.py.txt`; `python replay.py.txt --prepare --root docs/experiments/evidence/EXP-038`; `--verify`; `--run` with a private argv transport configuration; `--analyze`; final `--verify`. Secrets and private configuration are absent from the tracked runner and artifacts. The transport deadline was a remote one-second alarm, with credentials read in memory on the authorized host. Public evidence contains no destination hostname or credential value.

Post-execution portability correction: the exact baseline source bytes are preserved in `baseline-rerank.py.txt`, matching the already frozen source hash. The runner can use that verified snapshot when current main's transport implementation differs. This changes neither request bodies, judgments, output-to-order policy, analysis nor gates; no model calls were repeated. Original tooling remains available at its recorded commit. Raw attempts and responses are retained unchanged.

## Next decision

Keep the ordinary W0 contract. The largest development loss occurs in q7, where the evidence-focused contract promotes adjacent topical cards over more direct verification and benchmark sources. Source-finding versus substantive-evidence intent is a plausible explanation, not causal proof: rubric and context shape changed together. See diagnosis.md for card-level evidence.

Continue issue #516 with an explicit caller-supplied research purpose/question rather than guessing that purpose from engine keywords. Freeze the next candidate and its numerical gates before calls. Preserve all previous outcomes; reserve the ten authorized Brave attempts for independently assessed confirmation after selection. This failed candidate and completed provider hardening do not complete ranking production-readiness.
