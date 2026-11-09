# PREREQ-1111: establish permission-control inputs

## Frozen plan

Registered 2026-11-09, before measurement. Baseline `bb902651f1247ca33c383d903dc466d5e8916f86`; related #1111. Measurement prerequisite only, no product-benefit hypothesis.

Problem: recorded macOS custody failures may receive accepted 0644 rather than the requested forbidden mode. Issue corrections invalidate the proposed universal 04755 replacement. Before touching sealed sources, establish which inputs this host actually retains.

Priority: resolves an observed contributor qualification obstacle; unlike broader task-quality proposals, this has a reachable inexpensive diagnostic. Not proof of improved search or agent tasks. Recent EXP-123/122/121 delivered gateway corrections; 120 was unmerged, 119/117 not supported, 118 prerequisite only. No supported candidate from this loop was identified in current open PRs.

Primary metric: exact retained permission bits immediately after chmod, before any write, per requested mode. Useful prerequisite criterion: all three repeats agree for 0644 accepted control and ordinary forbidden modes 0600 and 0755; separately report whether 04644 and 04755 retain S_ISUID. Do not collapse inability to create the input into validator acceptance. No inference across hosts.

Fixed sample: three fresh files per mode for [0644,0600,0755,04644,04755,02644,01644], all beneath owned persistent Tank run directory. Write inert text before chmod; record readback immediately, then append inert text and record readback again to distinguish operation ordering. No files executed. Capture platform, uid, filesystem and exact probe code. No root changes, mount/security changes or credentials.

Guardrails: no production/sealed source modifications; no weakened validator; healthy 0644 readback exact; private owned scratch removed; zero API calls, zero builds. Plan limited to one host, 21 files, no retries or additional modes; stop on any unexpected exception and retain output. Default 45-minute ceiling. Uncertainty is exact finite repeatability, not a population estimate.

Command: execute the Python probe retained verbatim in `evidence/PREREQ-1111/probe.txt` using Python3 via stdin. Evidence: JSON readbacks, Python/platform versions, filesystem identifier and SHA256 of probe. This probes filesystem input readiness only; it does not run the custody validator or establish its passing suite. Supported prerequisite requires the controls above; otherwise blocked. No implementation PR from this probe alone.

## Readout

Outcome: supported **measurement prerequisite only**. Registration commit `1fe7d055`. All 21 planned cases completed, exit0; no exclusions, retries, source changes or extra modes. [Raw observations](evidence/PREREQ-1111/observations.json), [exact inert probe](evidence/PREREQ-1111/probe.txt), [provenance](evidence/PREREQ-1111/provenance.txt).

On this macOS27.0 arm64 host, uid501, Tank device16777252: every requested mode was retained immediately after chmod (21/21). In particular 04644 and 04755 retained S_ISUID in all3 repeats each. After appending data, 04644 became0644, 04755 became0755 and02644 became0644, each3/3. Ordinary0644/0600/0755 and sticky01644 remained unchanged. Accepted and ordinary forbidden controls pass9/9; finite repeatability exact, no cross-host confidence estimate.

This establishes the intended setuid inputs are reachable in this execution context, contrary to treating the earlier mismatch as an immutable host blocker. It also demonstrates operation ordering can remove those bits. **It does not explain the previous Rust-suite failures.** Source inspection of `snapshot-fixture-amendment/w07-helpers.rs` lines726 onward and `a06/source-amendments/w07-t02-joint.rs` lines851 onward shows immediate validator calls after chmod; no intervening write is evident in those caller loops. Do not attribute their failure to this append demonstration without tracing their callees and reproducing the actual failing Rust command.

No validator executed, no Rust suite rebuilt, no portable mode replacement established. All guardrails passed; scratch files removed by probe finally block. Zero provider requests/cost; subscription usage unknown. Elapsed diagnostic process below one second; overall cycle timestamps are in the storage ledger. No product quality, availability, deployment or implementation delivery claim.

Next decisive step: instrument requested/read-back mode at the actual failing custody boundary in an isolated current-source run, preserve the original failure and source/cache provenance, then distinguish input establishment from validation logic. Existing sealed amendments remain unchanged. A wholesale 04644→04755 rewrite remains unjustified.
