# EXP-071 source-order diagnosis

This is a read-only explanation of the rejected development run, not a new selector, rescoring, or adoption result. The [completed run](completed/run.json) and [analysis](completed/analysis.json) remain authoritative. Two independent GPT-6 Luna reviews and the root review traced the losses to actual card IDs and raw distributions.

## Observed ordering failures

| Case/card | Fresh W0 rank | Candidate rank | Supported mass | Conditional band | Interpretation |
|---|---:|---:|---:|---:|---|
| q2 c9 | 4 | 15 | .98 | 1.581633 | Direct Java 8 to 21 migration lead displaced despite positive support |
| q3 c1 | 9 | 18 | .93 | 1.591398 | FOSSA guardrails lead displaced despite positive support |
| q3 c13 | 7 | 15 | .89 | 1.640449 | License-contamination/provenance lead displaced despite positive support |
| q8 c1 | 4 | 14 | approximately .97 | 1.886598 | Flaky-test reproduction lead displaced despite positive support |
| q2 c17 | 15 | 4 | .58 | 1.965517 | Adjacent Java 25 source promoted despite unsupported being the largest individual option |
| q8 c22 | 17 | 2 | .62 | 2.080645 | GUI reproduction source promoted despite unsupported being the largest individual option |

All 32 displaced useful fresh-W0 top-ten card/case/reference instances remained above the registered .5 support cutoff. Lowering that cutoff cannot restore those ranks. The main observed loss is ordering within the supported class; aggregate support also admits some adjacent sources. These are separate failure modes.

Diagnostic class-first/canonical ordering helps q3 but harms q8 and is mixed on q2. Therefore removing conditional ordering is not a universal repair. Conditional expectations over ordinal bands and sibling-count judgments are plausible design concerns, not established sole causes. Preserve assessor disagreements and unseen-page limitations; do not regrade cards or discard inconvenient cases.

## Implications for the next development decision

- Do not tune a support cutoff to address within-class losses.
- Do not repeat absolute task-fit Score as a novel mechanism: EXP-039 and EXP-065 already tested it; their guard failures remain intact.
- Do not use the diagnosed ordering as qualifying evidence for a new policy.
- Check prior upstream experiments and shipped release behavior before registration. Any new policy needs a frozen independent run, unchanged quality/retention/stability/navigation/resource gates, then untouched confirmation and the full production acceptance checklist.

This diagnosis made zero provider or search calls and changed no runtime, deployment, or default behavior.
