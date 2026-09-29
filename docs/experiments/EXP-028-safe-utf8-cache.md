# EXP-028: UTF-8 cache values with ASCII fallback

## Registration — 2026-09-29

- State: registered. Owner: Codex. Start: 2026-09-29T13:01:41Z.
- Baseline: a8a70f580474c79c23d63a61409ea09048f41489.
- New evidence: EXP-027 identified a lost cache write on lone surrogates;
  UTF-8 savings were never measured. This candidate adds a specific fallback
  before storage and retains the original useful-effect threshold.
- Candidate: in SearchCache.set serialize with ensure_ascii=False, attempt
  serialized.encode("utf-8"), and on UnicodeEncodeError reserialize using the
  baseline json.dumps(value). Keep default separators, keys, TTLs and readers.
  Do not combine with EXP-026 compact separators or tune after seeing results.
- Hypothesis: this preserves baseline cache behavior while saving >=10% of
  total UTF-8 cache-value bytes across the fixed 20 captured query rows.
- Primary metric: 1 - total candidate bytes / total baseline bytes, two passes
  of EXP-016/brave-acquisition.json with hash
  bda41e595a347ec51a993ea5a3b94512f120f6a4202f2e72cf6c0608389366b1.
  Prior baseline was 67,310 bytes/pass in EXP-026; remeasure here. Ten percent
  remains the practical threshold for advancing storage-efficiency work.
- First run EXP-027's five service compatibility cases, adding a lone low
  surrogate U+DC00. Both arms must write once, hit cache, dispatch once,
  preserve decoded payloads, keys and TTLs. Zero tolerated violations.
- Then use EXP-026's captured-card SearchService/SearchCache replay, with the
  installed Valkey Encoder(utf-8, strict, False) in the in-memory recorder.
  Preserve all card fields. Same volatile normalization and alternating arm
  order; 20 rows twice, 80 write/hit pairs, no exclusions. Its supplementary
  nested/empty/control/Unicode/surrogate round trips must also pass.
- Deterministic fixed-input bytes: no population confidence interval or claim
  about real traffic distribution, human benefit, Valkey allocation or latency.
  Extra serialization/encoding CPU is unmeasured; supported bytes would require
  normal performance and portal regression checks before an implementation PR.
- Stop on first guard failure; otherwise complete both passes, no retuning.
  Supported iff >=10% saving and all guards pass. Below bar/guard breach:
  not-supported. Unavailable runtime: blocked. Incomplete data: inconclusive.
- Budget: 45 minutes, offline, zero paid/upstream calls. No production changes.
- Commands: PYTHONPATH=. /Volumes/tank01/magnus/git/SlopSearX/.venv/bin/python
  /private/tmp/exp028-screen.py; only after passing, same interpreter runs
  /private/tmp/exp028-replay.py. Freeze full scripts as inert evidence under
  docs/experiments/evidence/EXP-028, alongside outputs, hashes and environment.
- Portal impact: no proposed presentation changes; preserve canonical cache
  semantics. Candidate injected in memory only and restored after measurement.
- EXP-025 has no new inputs; no eligible unfinished work precedes this cycle.

## Readout — not-supported

Registration b9794df preceded implementation. All six compatibility cases
passed, including both high and low lone surrogates: cache writes succeeded,
second requests hit cache, and adapters dispatched once. Then all 80 captured
write/hit pairs passed payload, key, TTL and redispatch guards; supplementary
round trips passed. Total service calls: 24 screen + 160 corpus = 184.

Primary bytes across both passes: **134,620 baseline → 134,196 candidate**,
a saving of **424 bytes / 0.31496%**, below the registered 10% minimum.
Per single corpus pass: 67,310 → 67,098 bytes, saving 212 bytes. Repetitions
agreed exactly. No statistical interval applies to this fixed deterministic
corpus; effect on unseen workloads is unknown. CPU cost of the extra encoding
check, real Valkey allocation, latency and human/agent benefit are unmeasured.

[Summary](evidence/EXP-028/summary.json), [compatibility](evidence/EXP-028/screen.json),
[raw rows](evidence/EXP-028/rows.json), [families](evidence/EXP-028/families.json)
and [reproduction](evidence/EXP-028/reproduce.md) preserve all evidence.
There were no deviations, retries, paid/upstream calls or production edits.
The in-memory candidate was restored after each stage. No implementation PR;
retain only documentation and inert evidence in the accompanying auto-merged PR.

This resolves EXP-027's specific failure but does not meet the advancement bar.
Do not repeat separator/Unicode cache experiments on this same corpus. Revisit
only with a justified different workload or a distinct observed bottleneck;
do not lower the threshold or combine rejected candidates after seeing results.
