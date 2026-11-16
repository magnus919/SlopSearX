## Observed delivery blocker

PR #1014 candidate `e74eed7e2d01509905bcc18dbcf4e223905e6813` integrated current main `798f43c0`, including #1026's tiny-deadline qualification correction and #1025's session-expiry test correction. Its local T06/F01/O06, locked build, strict Clippy and changed-path hooks passed. Fresh required CI used test merge `367e5601e0ebdce518279e003cae8d5998992768`, with identical source tree `9dbb4ac420dcbfd2000e869aecdacad30fa93639`.

[Run39257154348](https://github.com/magnus919/SlopSearX/actions/runs/39257154348):

- AMD64 foundation job118075207643 passed,13:05:10–13:54:45 UTC on2026-10-19.
- [ARM64 job118075207483](https://github.com/magnus919/SlopSearX/actions/runs/39257154348/job/118075207483) ran13:05:13–14:05:21 UTC and was CANCELLED during `Native infrastructure security adapters`, after the prior component gates including F03/T06 had passed.
- Authoritative check annotation: `The job has exceeded the maximum execution time of 1h0m0s`.
- `.github/workflows/rust.yml` sets the foundation job `timeout-minutes: 60`.

All other current-head PR checks passed or were intentional skips. The cancelled ARM job is incomplete qualification, so #1014 remains unmerged. This does not establish a source regression or successful completion of the interrupted A12 gate. The October18 tiny-deadline failure remains separate retained history; today's source-clock correction cleared that earlier failure gate on both architectures.

## Outcome and acceptance

Provide a measured native-CI runtime/budget plan that reliably completes all required architecture gates. Options may include safely splitting work, reducing proven duplicate execution while retaining evidence, or a justified reviewed job-budget adjustment. Preserve every required case, assertion, negative/mutation control, source binding, architecture and owned-process cleanup. Retain the original timeout observation and complete command accounting.

- [ ] Identify the exact command/gate durations and resource profile from source-bound AMD64/ARM64 receipts; distinguish host scheduling/build work from test semantics.
- [ ] Define a bounded acceptance rule and stopping/resource plan before implementation. Treat this as a delivery/measurement prerequisite, not product-quality uplift.
- [ ] Keep original caller deadlines, captured inputs/outputs and result expectations unchanged. No status override or check removal.
- [ ] Qualify the complete revised workflow on a current candidate on both Linux architectures, including all original gate outcomes and cleanup. Never count cancellation as success or rely on repeated identical attempts to find a pass.
- [ ] Deliver through normal review/CI; refresh #1014 against accepted source as needed, then require all its applicable exact-head CI and substantive review before normal merge.

Milestone: Rust Rewrite. Related #879, #936, #942 and pending warning delivery #1014. No production deployment, alerting or live provider calls are authorized by this issue. Daily cycle records retain the evidence and actual elapsed time.
