# Private process-wrap developer owner

`vendor/ssx-cli-process-wrap` retains the crates.io package name and version
`process-wrap 10.0.1`. Only xtask and the backend-free CLI checker use its exact
path dependency; the excluded package is absent from the shipping ssx graph.
There is no global Cargo patch or additional generic process API.

The root lock retains the separate unmodified crates.io process-wrap identity for
rmcp 3.5.1. Only the two developer consumers resolve to the private path package.
The gate selects by canonical manifest plus path source-ID and checks both sets
of actual resolver edges; name/version equality alone is insufficient. The real
resolver positive runs before single-field wrong-source/wrong-manifest controls.
This repair does not claim to change production rmcp containment.

The retained `.crate` is immutable (SHA-256
`1f21b97672d2dc848e7b25701ab4535618b92f4861c13cc3f7f7bed52ad3c8da`).
Its 94 members include the original Apache-2.0 and MIT licenses and Cargo.lock.
The finite provenance map authenticates all 95 fork files: 90 pristine members,
four modified members and one added regression module. Retain original file
modes, including the executable multiprocess helper. Reproduce the fork from an
owned extraction of this archive with `git apply --check --unidiff-zero` and
`git apply --unidiff-zero process-wrap.patch`; compare every path, mode and hash
to process-wrap-provenance.json. The zero-context flag is required. Never format
the pristine archive or the 90 immutable members to refresh the mutable patch.

The Tokio Windows change queries ActiveProcesses on the retained original job
handle. Leader try_wait never consumes job notifications. Final wait caches
success only after both leader exit and original-job zero; asynchronous 5 ms
polling is cancellation safe and leaves no blocking completion-port worker.
The caller keeps kill plus bounded wait and separately requires actual original
job zero. Query errors, wait I/O errors and deadlines remain fail closed.
The Unix implementations and std Windows implementation are unchanged.

T08A authenticates the complete finite fork, archive, patch and original licenses
before execution. It runs strict private LIB Clippy with the gate's feature set
and explicitly shares the root Cargo target. On Windows it checks the exact
30-name all-features tokio_windows inventory, then runs that entire test target
unfiltered: 28 passes and two original ignored subprocess helpers are required.
The three new ordinary tests include the exact helper plus actual whole-job
regressions: leader observation before final wait, and leader exit while a live
descendant remains. Helper output is bounded to 4 KiB, real helper execution is
asserted, all final waits are bounded to five seconds, and original-job zero is
queried explicitly. No new ignored test is allowed.

The excluded package's private unit/LIB commands use its immutable upstream
Cargo.lock. Record that lock separately from the root consumer Cargo.lock; do
not present the private lock's Tokio resolution as the root consumer's graph.
Full Windows T08A additionally executes actual root-resolved checker consumers,
the frozen external corpus, source install and all existing cleanup controls.
Shipping CLI dependency/feature checks and all other T08A controls remain in
force. Private-unit success does not replace root-consumer or platform evidence.

When updating, review original-job ownership, cancellation, Windows API types,
archive/license/advisory identity and the complete fork map; refresh only mutable
provenance/bindings and the existing finite root manifest/lock frames after real
Cargo resolution. Keep frozen predecessor capsules unchanged. Preserve the old
archive, patches, actual failures and exact command/image evidence in history.
