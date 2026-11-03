# Foundation CI runtime budget

Issue [#1030](https://github.com/magnus919/SlopSearX/issues/1030) tracks the delivery timeout observed on PR #1014. The source context and rewrite acceptance contract remain in the [Rust rewrite case study](../CASE_STUDY.md). At baseline `24ee06a2a7500892315ad38afbe1d72952f4caec`, [the Foundation workflow](../../../.github/workflows/rust.yml) gave each Linux architecture job a 60-minute timeout. This candidate changes only that Foundation job timeout to 90 minutes; both `ubuntu-24.04` (AMD64) and `ubuntu-24.04-arm` (ARM64) matrix entries and `fail-fast: false` remain unchanged.

## Observation

Run [39257154348](https://github.com/magnus919/SlopSearX/actions/runs/39257154348) used candidate `e74eed7e2d01509905bcc18dbcf4e223905e6813` (test merge `367e5601e0ebdce518279e003cae8d5998992768`; source tree `9dbb4ac420dcbfd2000e869aecdacad30fa93639`). AMD64 completed all Foundation steps successfully in 2,975 seconds (49m35s). ARM64 ran for 3,608 seconds (60m08s) and was cancelled by the 60-minute limit during “Native infrastructure security adapters”; its later readiness smoke was skipped. The completed steps common to the two jobs took 2,949 seconds on AMD64 and 3,584 seconds on ARM64, a 1.215 ARM-to-AMD wall-time ratio.

These are hosted-job and step wall times. Steps combine compilation, test execution, setup and hosted-runner scheduling. The evidence does not isolate CPU, memory, disk throughput or scheduling delay. ARM64 has no completed full-job duration, and cancellation is not a passing result or evidence that the interrupted final gate passed.

## Implemented budget and decision rule

The candidate source implements the bounded timeout change from 60 to 90 minutes. This source change is not yet qualified: no full post-change Foundation run has completed on both architectures. This adds at most 30 hosted-runner wall minutes per architecture job, or 60 across both matrix jobs if both use the full allowance. It is a wall-time budget, not a dollar estimate or aggregate CPU measurement. It keeps the current commands, arguments, matrix, cases, assertions, mutation controls, source bindings, evidence upload, caller/product/test deadlines and expected outcomes unchanged. No gate is skipped, weakened or reclassified.

Accept the runtime change only after a fresh run of the complete Foundation workflow on the exact candidate source finishes successfully on both AMD64 and ARM64 within 90 minutes per job, with all original gate outcomes, cleanup and source-bound receipts present. Require substantive review and the applicable exact-head pull-request checks. If either job fails a gate or reaches 90 minutes, stop: keep #1030 open and inspect the retained source-bound timings and failure before proposing another change or run. Do not count cancellation as success or repeat an unchanged attempt to seek a pass.

This is a CI delivery prerequisite, not product-quality uplift or full rewrite acceptance. Before closing #1030, review the retained gate receipts for build/test accounting and available resource evidence; record unavailable metrics as limitations. No local native build is part of this change.


## Follow-up observation: PR #1090, 2026-11-03

On PR #1090 head `9e6b2af1c9cb2ee2384e85f3780f282c601a8304`, the AMD64 Foundation prerequisites job ran from 08:18:01Z until the 130-minute job limit cancelled it at 10:29:18Z. The job log shows A13 completed; A04 `real_nine_bindings_and_distinct_current_history_failures` was still running when the job ended. On the same run, ARM64 prerequisites completed successfully and both architecture-specific Foundation core jobs completed. The available log establishes an insufficient 130-minute AMD64 prerequisite budget for this candidate run; it does not establish a deadlock, an assertion failure, or the minimum sufficient budget.

The next bounded candidate raises only `foundation-prerequisites` from 130 to 160 minutes. It preserves every test, command, feature, matrix entry, gate, and evidence contract. The additional allowance is at most 30 minutes per architecture job (60 minutes total if both consume it). This is a candidate, not a qualification result. Accept it only after a fresh exact-head Foundation run completes all prerequisites and the required architecture-specific aggregates on AMD64 and ARM64, with existing evidence intact. If either prerequisite job reaches 160 minutes or fails, stop increasing the timeout; profile the slow test and CI resource behavior before another change.
