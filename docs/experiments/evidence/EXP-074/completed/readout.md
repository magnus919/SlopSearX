# EXP-074 live outcome: incomplete at the combined deadline

The qualified, merged harness from PR #656 ran once against the registered
public cases. Raw verification passed. The outcome is **inconclusive**: the
comparison is incomplete, so no development quality analysis or confirmation
credit is permitted. No runtime selector was implemented or adopted.

The synthetic 80-card capacity operation completed in 770.759 ms. Five research
operations completed: q1 base/repeat/rotate, q2 base and q3 base. Their measured
durable phases ranged from 484.563 to 875.259 ms. The next operation, q4 base,
failed when the purpose call timed out within the remaining combined budget.
Its fresh W0 call took 492.205 ms; the failed purpose call took 502.788 ms.
The durable failure endpoint was 1015.675 ms. Cleanup/receipt time is retained,
not subtracted to convert the failure into a pass.

There were 14 physical attempts: two capacity calls and 12 research calls.
Known usage is 202,205 input and 6,912 output tokens. Usage for the final timeout
is unknown. The failed operation retains the complete valid W0 ordering as
fallback, and all 15 later operations remain uninvoked. There was no retry and
no new search; the Brave allowance remains 8/10 used.

## What this establishes

The in-process path got farther than EXP-073, including the same synthetic
capacity case and five research operations. These separately executed runs are
not a controlled estimate of provider-latency improvement. EXP-073 remains a
terminal historical failure; EXP-074 remains a distinct terminal incomplete
comparison. Neither may be replayed, combined or graded as a completed study.

Removing subprocess overhead was insufficient to establish that two serial
model calls reliably fit the frozen one-second durable phase. There is no
ranking-quality conclusion from the successful prefix. Future work must address
the mechanism and normal service budget explicitly, with a new prospective
registration; simply retrying this study or relaxing its deadline is unsupported.
The complete production-acceptance checklist remains open.

## Integrity and publication

`raw-verification.json` recomputes request/response hashes, exact component
orders and fusion, observed usage, durable candidate/failure sidecars, full W0
fallback and terminal stopping. It reports `verified: true`, `incomplete`,
14 attempts and unknown usage. `analysis.json` performs no quality analysis.
The retained qualification manifest binds the reviewed source and dependencies.

The 25 original JSON files were scanned, including 84 decoded hexadecimal
fields, for the actual credential value and private-location patterns. There
were zero hits; they were copied byte-identically. `publication-scan.json`
records portable hashes and the scan limits. No private launch paths, endpoints
or credential values are included in the public packet.
