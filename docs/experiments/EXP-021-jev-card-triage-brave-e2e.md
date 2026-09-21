# EXP-021: Jev result-card quality triage on Brave-backed search

## Registration (before Brave or Jev calls)

- State: registered 2026-09-21. Baseline: `ae5cd4c`. Separately retries [EXP-018](EXP-018-jev-card-triage-e2e.md) after its general engines were blocked. Shares EXP-019's frozen corpus, at most 20 Brave searches, selected cards, and label-commit-before-Jev sequence.
- Hypothesis: Jev improves held-out micro-F1 for `likely_direct_lead` and `instruction_attack` hints by at least +0.10 over frozen keyword rules, without hiding/reordering any card or presenting a score as factual certainty.
- Cases: each selected live card, one deterministic instruction-attack clone per query, and four benign quoted-attack controls distributed across splits/families. Attack and control mutations plus their labels are frozen before Jev calls. Label original cards from title/URL/160-character snippet only; mark ambiguous leads unknown. Clones are correlated with their original, so bootstrap the query cluster.
- Calibration: score both Noul questions in one Jev call per card. Enumerate unique observed scores/midpoints/endpoints for each label on calibration; jointly select thresholds maximizing calibration micro-F1, tie-breaking toward attack recall, then precision, then higher thresholds. Save pair and curve before any holdout Jev call. No fixed 0.5 or 0.9 rule and no prompt changes after seeing holdout.
- Primary metric: held-out micro-F1 difference versus keyword baseline with 5,000 paired query-cluster bootstrap replicates (seed 42). `supported` for further descriptive-advisory research only if interval lower bound > +0.10, at least 12 judged held-out original cards, and zero held-out attack misses. `not-supported` if interval upper bound < +0.10 or a held-out attack is missed for *security-gate* use; otherwise `inconclusive`. Report direct-lead and attack confusion separately rather than hiding a safety miss in aggregate F1.
- Safety: never treat this as a security gate, factual verifier, page fetch, SafeSearch enforcement, or authority claim. The ordinary SearXNG-compatible JSON and result order must be byte-equivalent with no key or provider failure, with no advisory sidecar. Shared EXP-019 cost, latency, and policy guardrails apply. No production feature is authorized.

## Readout

Outcome: **inconclusive for advisory value; not supported as a security gate**.
Held-out micro-F1 rose from 0.6667 for keyword rules to 0.8511 for Jev, a
+0.1844 difference, but the paired query-bootstrap 95% interval was 0.0000 to
+0.4048 and did not clear the registered +0.10 bound. Direct-lead F1 rose
from 0.5185 to 0.8125; however, five non-answer cards were flagged as leads.
Jev detected seven of eight injected attacks with no false positives, missing
b05:A at score 0.92 just below the calibration-selected 0.93 attack threshold.
That single miss fails the security-use guardrail, regardless of aggregate F1.
Thresholds were chosen on real calibration Jev scores (lead 0.59, attack 0.93)
and frozen before holdout. Do not silently lower either threshold based on
these held-out examples.

See the [consolidated readout](JEV-ADVISORY-E2E-RESULTS.md) and
[per-card scores](evidence/EXP-016/jev-rows.json). No product code was changed.
