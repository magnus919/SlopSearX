# Jev bounded function suite results

Date: 2026-09-20

Registration commit: `b7b4ce3`

All five experiments ran once, in the registered order, without retries or
post-result changes. All 77 Jev calls were valid responses from pinned
`jev-1.13.0`. Total usage was 76,556 input tokens and 9,131 output tokens, with
estimated input cost `$0.003215352`.

| Experiment | Outcome | Primary evidence | Interpretation |
| --- | --- | --- | --- |
| EXP-006 hard-query fusion | Not supported | MRR `0.6845 -> 0.6952` (`+0.0107`); 11/12 coverage; zero regressions | Jev improved two low-ranked targets but missed the preregistered effect size. Do not pursue ranking integration from this evidence. |
| EXP-007 robustness | Supported for further research | Balanced accuracy `0.9375`; adversarial FPR `0`; positive FNR `0.125` | Strongest evidence that bounded Jev classification can resist prompt-like result text. Missing snippets remain a weakness. |
| EXP-008 research stopping | Not supported | Accuracy `0.75`; zero false stops; sufficient recall `0.50` | Conservative behavior avoided unsafe stops but discarded too much genuinely sufficient evidence. Do not use as a stopping gate. |
| EXP-009 engine-plan selection | Supported for further research | Accuracy `0.80` vs keyword baseline `0.4667`; gain `+0.3333` | Most promising functional hypothesis, provided Jev only chooses among pre-authorized plans after deterministic policy filtering. |
| EXP-010 annotations | Supported for further research | Coverage, accuracy, macro F1 all `0.8333` | Promising as additive metadata. Standards overlapped with primary documentation, so taxonomy design needs work before any user surface. |

## Detailed interpretation

### Ranking

Hard-query fusion moved the TaskGroup target from rank 7 to 5 and the Fetch API
target from rank 14 to 7, but ten other cases produced no fusion improvement
and one Rust target was absent from Brave's top 20. Jev-only ranking was worse
overall than Brave. The fixed fusion caused no target regressions, but its
`+0.0107` MRR lift is too small to justify ranking, cache, snapshot, latency,
privacy, and compatibility changes.

### Robustness

Jev rejected every frozen prompt-like or misleading negative. The sole error
was a canonical PromQL page represented only by title and URL; probability
`0.34` fell below the fixed `0.5` threshold. A follow-up should measure how
missing snippets affect false negatives across a larger captured corpus. This
result does not establish resistance to arbitrary prompt injection.

### Research stopping

The model made no false-positive stop decisions, which is the safe direction,
but assigned low or borderline sufficiency to four complete fixtures: current
TaskGroup documentation, two authoritative package-license records, the
NVD-plus-CVE set, and a complete structured filter-enforcement report. The
result suggests that one generic sufficiency question is too conservative and
opaque for control flow. Deterministic facet completion remains preferable.

### Engine-plan selection

Jev corrected several queries that the first-match keyword router sent to the
wrong family, especially jobs queries containing code terms and scientific
queries without a registered keyword. Its three misses were semantically
defensible overlaps: developer sentiment (`code` vs `social`), Treaty of
Utrecht background (`historical` vs `reference`), and a POSIX manual (`web` vs
`reference`). This indicates both useful contextual discrimination and label
ambiguity. A larger multi-label or utility-based evaluation is required before
considering an optional planner.

### Additive annotations

Jev correctly classified all primary-doc, repository, tutorial, discussion,
and vendor-marketing fixtures. It abstained on all three standards/regulatory
fixtures because those sources also scored as primary documentation. This is a
useful abstention pattern, but it shows that the six labels are not mutually
exclusive. A future experiment should use multi-label annotations rather than
force a single class.

## Product recommendation

Do not open an implementation issue for reranking or research stopping.

If more research is authorized, the strongest next candidate is an offline,
larger engine-plan experiment using multi-label human judgments and real query
traces. The second candidate is multi-label additive source annotations. Both
must preserve deterministic policy, sensitive-engine, explicit-scope, and
no-key behavior. Robustness should be a prerequisite test suite for either,
not a standalone product feature.

These experiments still do not establish affected-user demand, acceptable
external processing, ordinary-account retention, multilingual behavior, or a
production latency budget. “Supported for further research” means exactly
that; it is not authorization to deliver a feature.

Evidence: [shared harness](evidence/JEV-FUNCTION-SUITE/harness.py.txt),
[EXP-006](evidence/EXP-006/summary.json),
[EXP-007](evidence/EXP-007/summary.json),
[EXP-008](evidence/EXP-008/summary.json),
[EXP-009](evidence/EXP-009/summary.json),
[EXP-010](evidence/EXP-010/summary.json), and
[checksums](evidence/JEV-FUNCTION-SUITE/SHA256SUMS.txt).
