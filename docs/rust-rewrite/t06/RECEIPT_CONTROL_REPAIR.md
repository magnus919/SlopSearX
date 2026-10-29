# Deterministic receipt publication qualification

The post-merge Rust foundation run [40508238845](https://github.com/magnus919/SlopSearX/actions/runs/40508238845) tested upstream `e9c5911a468b892fe2adef9027374cb867a2535d`. Its ARM runtime job failed T01's `receipt_write_latency_is_outside_the_ready_fake_engine_future` assertion at `search-qualification.rs:206`: “control did not delay post-call publication.” This was an assertion failure, not a job timeout. AMD runtime, both prerequisite/adapter partitions and supply-chain policy passed. Both final assembly jobs then failed their required aggregate runtime-partition check.

The fixture armed one 100ms Valkey write pause before the old provider-timeout control, then assumed the same pause still delayed later publication. Scheduling could consume that window. The affected test and its direct fixture/observer code were unchanged by W07; the assertion originated in `5f3dbe582`.

The repair suspends the fixture's owned `Child` until explicit release. A guard borrows the server, checks STOP/CONT command outcomes and observes its stopped process state before and after polling actual publication. Cleanup resumes the child on early exit before the server owner kills/reaps it. The original one-millisecond old-engine timeout, ready-engine first-poll assertion and exact stored receipt-key assertion remain. No longer sleep or widened test budget replaces the failed control.

## Qualified source and evidence

The qualified code/test/input head is `07414ae79b8972f641ba141bf077e5e4134e4bdb`. A later documentation-only commit may carry this handoff without changing those qualified bytes. Local execution used Rust1.99.0 on `aarch64-apple-darwin`, pinned Valkey8.1.10, private Unix sockets, the existing shared target/cache and the original three exclusive leases. There were no live providers or deployment operations.

The exact component checks passed: receipt qualification binary5/5, standalone T06 source bindings5/5, xtask T06 binding control1/1, and affected W07 controls7/7, with no ignored cases. Formatting and strict scoped Clippy passed. `cargo xtask check T01` passed all10 recorded commands; `cargo xtask check T06` passed all57 recorded commands. These local results do not establish a passing hosted Linux ARM run.

Unique raw output, actual process waits, selected-image dep-info/compiler provenance and source seals are retained under `/Volumes/tank01/magnus/rust-builds/slopsearx/arm-t01-receipt-control-20261027/`. The gate reports are `native-01/T01.json` (SHA256 `9d345d7e50373b3f36c987b823a00e68f24db9c849fa7ed288772d7a69008031`) and `native-01/T06.json` (SHA256 `f4b3b177cbf5158addbddc0b32aad45eb89ba64c17cc85ace4dd86cf8ebf7c59`). `native-02/result.json` records the successful resumed window and final lease/process/TMP retirement. The earlier scratch auditor stopped after a successful compile because it mishandled a hyphenated Cargo binary name; its original failure and the corrected additive image audit remain recorded. No native test failure was hidden or retried through that correction.

## Frozen source maintenance and downstream scope

Changing these two qualification files also changes their exact T06 reader operands. The additive [successor capsule](source-amendments/receipt-control-successor.json) proves four complete current afterimages and ordered inverses to their exact `e9c5911a` Git bodies, modes and blobs. Five historical binding containers/archives retain their original bytes. Both T06 readers and the W07 reader first use the separately reviewed [current closure](source-amendments/receipt-control-closure.rs), whose independent literal hashes bind the executable helper and capsule. Existing test identities remain unchanged; mutated bodies, modes, caller tuples and inverse descriptions fail closed.

This is qualification-only maintenance under the [Rust delivery contract](../CASE_STUDY.md). Backend search, portal requests/results, filters, policy, storage and production startup behavior are unchanged. Final hosted CI and normal review remain required before merge; no push, provider campaign or deployment is authorized by these local results.
