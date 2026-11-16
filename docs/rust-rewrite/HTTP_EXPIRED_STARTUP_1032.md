# HTTP expired startup admission (#1032)

The locally qualified Rust candidate is committed as `c253e604` and normally
merged with the fetched `origin/main` at `ff3a6ede` in merge commit
`ba710dcc`. A subsequent normal merge of current `origin/main` at
`4b7156d3` is recorded in merge commit `714d83fc`. Root acceptance for the
bounded causal and restoration run is
recorded at
`/Volumes/tank01/magnus/rust-builds/slopsearx/storage-ledger/pr1034-wikipedia-reset-causal-root-acceptance-v1.json`
(SHA-256 `c4688030c4d84c089fbee98391edd3869ecf54e43ff1a891efa867c97da3260f`).
This is local macOS qualification evidence. Current-candidate Linux AMD64,
Linux ARM64, and portal CI remain pending; this evidence does not close #1032
or unblock dependent work.

## Observed failure and diagnosis boundary

[PR #1024 ARM job 118105702415](https://github.com/magnus919/SlopSearX/actions/runs/39266027319/job/118105702415)
at exact historical head `84b2c39f76dc5a426f4f455b67ded10be3753e65`
failed the native biomedical step with 262 passed and one failed. The failing
control,
`http::a10_owner_clock_tests::unmocked_system_expiry_and_expired_search_context_dispatch_nothing`,
received `Error::Deadline` but found a retained client at the assertion
`expired unmocked HTTP never checks out/builds a client`. This is a failed
assertion, not a wall-budget timeout. The SHA-256 of
`tests/support/a10_http_clock.rs` at that historical failure head was
`b850833e2cac860cce876858fdff32130640b07fd2192f4f74b34f8729676c72`; that is
a historical hash, not the current file hash.

At baseline `24ee06a2a7500892315ad38afbe1d72952f4caec`, the production HTTP
owner reads `std::time::Instant`, but its sleep delegates to Tokio's timer.
`Session::execute` checked stop and cancellation, then relied on a biased
select's sleep branch before polling `execute_inner`. If that sleep is still
pending after the owner deadline, the inner future can spend request budget
and check out a client. The original failure proves the client-side effect;
it does not expose the precise timer-wheel state on that ARM poll.

The candidate guard compares the same owner clock directly to the unchanged
caller deadline after stop/cancellation checks and before header work or the
select. The select continues to own in-flight expiry. No production timeout
was increased, no public request/response contract changed, and no performance
claim is made.

## Local causal and restoration evidence

The candidate adds four startup controls in
`crates/ssx-infra/tests/support/http_expired_startup.rs`, registered in the
existing HTTP test module:

- System expiry with Tokio deliberately paused: real time crosses a 250us
deadline, the timer is first-polled and must still be pending, and execute
must immediately return Deadline with zero request accounting, transport
calls or clients. Both an injected transport and the real pool are covered.
All connection permits are held to prevent any socket even if the guard
regresses. Pausing belongs only to this test's isolated Tokio runtime.
- Manual expiry exactly at 250us and at 251us, with zero side effects.
- Closed before Cancelled before Deadline precedence with expired startup.
- Successful System dispatch and Manual dispatch at 249us of a 250us owner
budget, despite expired wall time. This rejects a guard using System time in
place of the Manual owner clock.

The root-approved one-line mutant removed the second-stage
`session.set_deadline(...)` call in Wikipedia's two-stage search flow. The
selected A04 control then failed with exit 101 and the expected sole panic at
`crates/ssx-infra/src/../tests/support/a04_broad_api.rs:1275:5`:
`second actual stored deadline was not renewed`. Strict restoration returned
the same source inode to its original bytes. This demonstrates that the second
stage uses a renewed operation deadline; it does not replay the original ARM
startup failure or identify its timer-wheel state. A fresh compile and
complete 291-name library inventory check then passed. The 291 names are an
inventory listing, not evidence that all 291 tests ran.

Separate historical evidence covers removal of the original startup guard.
The earlier root acceptance receipt is
`/Volumes/tank01/magnus/rust-builds/slopsearx/storage-ledger/http-1032-native-v2-root-acceptance.json`
(SHA-256 `bfcaac2daf4a8597684aad5758beecafa53b04eac87f468a6f373b84a111db13`).
It records seven commands with exits `0, 0, 101, 0, 0, 0, 0`, including the
expected mutant failure and source restoration. This is distinct from the
Wikipedia second-stage deadline mutant above; it is historical local macOS
evidence, not a replay of the later Linux ARM failure.

After restoration, all 35 distinct HTTP controls passed: 23 A04 tests, two
endpoint controls, four startup controls, and six A10 controls. The selected
Wikipedia deadline control also passed when run separately; it is one of the
23 A04 tests, so the run had 36 test executions across 35 distinct names.
The earlier source-pinned positive batch also accepted its 112 xtask checks,
63 A13 checks, typed control bridge, and A13 compiled-inventory parser
controls. These counts describe bounded native qualification, not full
workspace or platform CI.

The unchanged 250us System/Manual startup controls remained in the candidate.
The System decoder test stages a 60-second post-exchange wait; that case is
separate from, and is not evidence for, the 250us startup boundary.

## Source bindings and delivery limits

The V6 source amendment preserves V5/V4/V3/V2 and all historical source
containers. The exact baseline HTTP source is retained in
`docs/rust-rewrite/a13/source-amendments/http.rs.24ee.frozen`. The new control
module has an exact path/bytes/mode/SHA binding and negative checks against
modified source and substituted paths. No original provider capture or frozen
qualification JSON was repinned. The four added test names are in the full
compiled inventory; historical names remain required.

The 23-test A04 unit was moved under `tests/support/` without pruning test
names or changing frozen captures. The preserved source amendment records
the relocation and exact source bytes. Typed control bindings distinguish
current source from historical source. `SystemOperationTrace` and its
recording hook are guarded by `#[cfg(test)]`; they provide deterministic test
observations and are omitted from production builds.

Portal impact review: this changes native HTTP admission, so expired work
stops before network resources are used. The accepted timeout result, portal
request state, result fields, access policy, filters, cache/snapshots, and
HTML/API error shape are unchanged. Current-candidate portal contract CI is
still required, alongside the pending Linux AMD64 and ARM64 native gates.
Python backend execution remains outside this Rust qualification.


## Current exact-source qualification snapshot (2026-10-19)

The current qualification is for the candidate working-tree source based at commit `6aa01ed3ee268224ab159311581df3f5173e502a`, against `origin/main` `4b7156d3915fce29f84d39b077101f9d233fc248`. The four dirty source/amendment paths are not contained in that base commit. The sealed packet is `/Volumes/tank01/magnus/rust-builds/slopsearx/pr1034-marker-namespace-qualification-v1/FINAL-MANIFEST.json` (SHA-256 `31c5dbd081f6a62ead5fddf04bb73e4fc5b374d62bbb354cd7e8e03deb331610`). Its 21 source pins include HTTP `crates/ssx-infra/src/http.rs` at `c5116d9a0081a0497a12643440053f001b6d68b48e9b4bf8c5feabf81eed5d10` and helper `docs/rust-rewrite/a04/source-amendments/scoped-successor.rs` at `edb8ac754f54c78da747296bf0a115ef4d04d289e9ba10b8f76a32168c092c2a`. The helper change from the predecessor qualification is only two diagnostic-label renames: `PR1034_A13_MAIN_COMPOSITION_PROOF` and `PR1034_A13_COMPOSITION_CONTROLS` became `PR1034_MAIN_COMPOSITION_PROOF` and `PR1034_COMPOSITION_CONTROLS`, because the strict A13 namespace parser rejected unrelated `A13_` markers. The parser was unchanged. Older helper `0f2d548…` and A13-pending v2 records are stale; the immutable predecessor packet remains historical evidence.

On that changed-source composition, exactly these five checks were rerun and passed: Rustfmt; strict workspace/all-target Clippy; full xtask suite (112 passed); A13 (all 75 selectors; 82 typed commands, zero nonzero); and W06 (73 commands, zero nonzero; 303 completions comprising 248 source and 55 native). The A13 report has 160 total records, including proof/completion rows; those rows are not commands. The W06 report has 74 total records, including its proof/completion row. Earlier 291 full-infrastructure tests, focused HTTP controls, A04/A09/A12, and other common-runtime evidence were not rerun for this two-label source delta; they remain predecessor evidence.

Full exact-new-head Linux AMD64 and ARM64 source inventories and the portal inventory/impact gate remain required before merge. Historical published checks for `191537e2003c51064a1e17739f3300b1e5ac4a8c` are not current-head evidence. This local macOS qualification does not close #1032, authorize merge, establish performance, or claim full milestone closure. The historical ARM assertion failure, diagnosis boundary, earlier causal and restoration controls, and portal contract analysis above are retained from the original document without alteration.
