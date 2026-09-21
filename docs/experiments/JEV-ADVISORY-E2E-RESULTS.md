# Jev advisory functions: end-to-end search-card follow-up

This record follows [EXP-015](EXP-015-jev-advisory-calibration.md). It tests
three **research-only** Jev advisories on actual cards from the shared
`SearchService`, not just hand-written prompt fixtures. Nothing here changes
production routing, result ordering, HTTP/MCP output, policy, or the no-key
default. All queries were synthetic; no real user query was used.

## Acquisition and calibration

The first registered run, [EXP-016–018](EXP-016-jev-source-annotations-e2e.md),
made 40 free DDG/Google adapter attempts. All 40 were classified `blocked` and
returned no general cards. Its 60 free specialist attempts were preserved, not
silently retried or called negative Jev evidence. A separately registered
[EXP-019–021](EXP-019-jev-annotations-brave-e2e.md) retry used **one Brave search
per query, 20 total with no retries**, and reused those specialist responses.
All 20 Brave requests returned five cards. The fixed sample comprised 55
original cards, 20 instruction-attack clones, four benign quote controls, and
20 next-source states.

Card/source gold and its [rubric](evidence/EXP-016/labeling-rubric.md) were
committed before any Jev call. One agent adjudicated from bounded result-card
fields and specialist results; pages were not opened and there was no second
rater. Twelve queries were calibration; eight were held out. The 99 Jev calls
all returned valid scores. Actual calibration scores selected thresholds before
holdout: 0.79 for all source-type labels, 0.86 for source advice, and 0.59
(direct lead) / 0.93 (instruction attack). No fixed score cutoff, count cap, or
holdout retuning was used. The complete calibration curves and scores are
[retained](evidence/EXP-016/jev-calibration.json).

| Function | Frozen baseline → Jev on holdout | Paired query-bootstrap 95% interval for Jev minus baseline | Registered outcome |
| --- | ---: | ---: | --- |
| Multi-label source type (micro-F1) | 0.957 → 0.900 | -0.176 to +0.167 | Inconclusive; observed regression |
| Next-source realized net utility/query | -0.25 → 0.00 | -0.25 to +0.75 | Inconclusive |
| Card triage (micro-F1) | 0.667 → 0.851 | 0.000 to +0.405 | Inconclusive for advisory; security-gate use not supported |

The intervals resample the eight held-out **queries**, keeping their cards and
mutations together (5,000 replicates, seed 42). None clears its preregistered
minimum useful effect. The tiny convenience sample, one adjudicator, and
unbalanced positive labels make this research evidence, not a product-ready
estimate. The deterministic annotation baseline was written after viewing the
acquisition/labels but frozen before Jev scores; its strong 0.957 F1 should not
be treated as an independently developed production baseline.

## What failed and what it means

- Source annotation: Jev missed two held-out overlaps at the calibration-derived
  0.79 cutoff: the official CERN first-site card scored 0.69 for `primary_docs`,
  and a GitHub README guide scored 0.63 for `tutorial` while retaining
  `source_repository`. The six-label taxonomy also has no specific label for
  registry metadata or scholarly papers. No observed reason to add Jev here
  instead of improving deterministic typing and taxonomy.
- Next-source advice: Jev reduced requested specialists from six to two in
  holdout, but omitted one useful npm follow-up (0.84 versus 0.86) and selected
  Internet Archive for a historical task despite that adapter timing out.
  The prompt asked conceptual fit while the primary metric measured realized
  value; health/availability is an independent eligibility gate. The
  [per-case rows](evidence/EXP-016/jev-rows.json) separate conceptual fit from
  actual response yield. This run does not show a reliable product gain.
- Quality triage: direct-lead F1 improved from 0.519 to 0.813, but Jev marked
  five non-answer cards as leads and missed one of eight held-out injected
  instructions: its attack score was 0.92, just below the calibration-derived
  0.93 threshold. That is a direct counterexample to security-gate use. The
  same mutated card also lowered its direct-lead score; card-content attacks
  can affect multiple hints at once. Descriptive advisory use remains
  unproven, not rejected forever.

Jev's 99 calls used 66,088 input tokens (about $0.00278 estimated input charge
at the published rate), median latency 194 ms, p95 280 ms. This excludes Brave
billing, which is not estimated here. The experiment-only sidecar left a
formatter fixture byte-equivalent with no key, simulated provider failure, and
valid advice. This is **not** a production HTTP/MCP integration test for a new
feature. Existing production Jev, MCP harness, and result-contract tests passed
58/58 with local socket permission; the initial sandbox-only test run could not
bind loopback and is not a code failure.

## Decision

Do not ship these three additions from this evidence. Source typing is better
addressed by deterministic metadata/rules in this sample; next-source advice
needs availability-aware eligibility and a cleaner separation between
conceptual fit and realized yield; quality hints need stronger attack recall
and independently judged direct-lead labels. A later product decision would
require a larger, diverse, blinded corpus and a transport-level prototype with
portal/API/MCP contract checks. The existing keyless search path remains the
default, and Jev must never grant policy access or verify a page.

Evidence: [query corpus](evidence/EXP-016/query-set.json),
[free-engine attempt](evidence/EXP-016/acquisition.json),
[Brave attempt](evidence/EXP-016/brave-acquisition.json),
[fixed cases](evidence/EXP-016/selected-cases.json),
[committed labels](evidence/EXP-016/labels.json),
[Jev scores](evidence/EXP-016/jev-rows.json),
[Jev summary](evidence/EXP-016/jev-summary.json), and
[provider attempts](evidence/EXP-016/jev-attempts.json).
