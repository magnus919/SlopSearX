# Native CLI authoring and integration split

This scheduling decision takes effect only when its focused decision PR is reviewed, accepted and normally merged and the plan/index and native issue relationships match. Filing the issue or publishing this proposal does not authorize T08A implementation. The [case study](CASE_STUDY.md) and every existing finish-to-start edge remain in force.

## Decision and reason

Split the existing native CLI scope between [T08A/#1062](https://github.com/magnus919/SlopSearX/issues/1062), which authors and independently qualifies the external HTTP client, and [T08/#903](https://github.com/magnus919/SlopSearX/issues/903), which integrates it with the accepted native HTTP service and completes final qualification/delivery. This adds no product capability and does not relax the finished no-Python destination.

The audited baseline is `c9d49a2a14b18a3cc1fb74a805e568aa897f1d70`. Its `ssx` is 8,927 bytes, SHA-256 `4c45e876fc79dffc0f7da8ccccc9c6ba67d4f4cc9e0bd9e8194f74184b96c65b`. It only calls `/search`, `/health` and `/config`; it does not import server implementation or shared page types. [PR1007](https://github.com/magnus919/SlopSearX/pull/1007) merged the native HTTP foundation at `e12744511013bd05370173e777daaef471bf9527` on 2026-10-26. [T02/#897](https://github.com/magnus919/SlopSearX/issues/897) correctly remains open for [#1049](https://github.com/magnus919/SlopSearX/issues/1049) and full acceptance. That restoration changes server page propagation/cache/HTML behavior; it need not change an independent client's exact outbound decimal text.

## Entry conditions and ownership

T08A is blocked by accepted/merged [F04/#883](https://github.com/magnus919/SlopSearX/issues/883), including its transitive F02/F03 handoffs, and by acceptance/merge of this decision. Its fresh-main baseline must contain merged PR1007. Read the current guides, issue and pinned source; reconcile drift rather than implementing against an assumed server interface.

T08A owns `crates/ssx-cli`, real external-child loopback fixtures, the native `cargo xtask check T08A` inventory/evidence, supported-target source-build/install instructions and coordinated gate/CI registration. Its detailed issue preserves the original argv, URL encoding, arbitrary integers, Unicode/JSON/YAML, failure, redirect/order, proxy/CA and optional-client obligations. Native gate/runtime/install evidence must work without Python or backend/FFI/sidecar/remote fallback. Existing temporary reference observations do not establish native acceptance. Shared Cargo/xtask/CI/install files remain single-owned.

T08A may not edit server/application/infrastructure implementation, page/domain, policy, adapters, caches, snapshots or renderer interfaces. It may not replace deployment defaults, switch required gates, remove Python backend source, call providers or deploy production. Any necessary shared change goes to its existing owner.

T08 keeps its hard T02 predecessor and adds T08A. Its integration cannot start until both accepted/merged handoffs exist. T08 consumes the component corpus and adds real native-client/native-server evidence, including all commands/formats/errors and exact high-page propagation, distinct cache identities and repeat hits. It requalifies final source/binary/install evidence and `cargo xtask check T08`, then hands accepted artifacts to O04/#921. T08A fixture success alone cannot close T08 or T02. O04 distribution/signing/promotion remains downstream.

## Graph and acceptance preservation

The only added plan/native blocked-by edges are `T08A -> F04` (#1062 blocked by #883) and `T08 -> T08A` (#903 blocked by #1062). Existing `T08 -> T02` (#903 blocked by #897) and all other edges remain unchanged. Here the arrow means “depends on”. There are 69 plan items and 174 plan edges after this split, compared with the original 68 and 172. Original publication metadata remains historical; the live graph also contains later follow-up issues such as #1049, whose safeguards are preserved.

After this decision is accepted and merged, #903's earlier repeated start prohibition is superseded only for the separate T08A authoring scope. It remains binding for T08 integration/final qualification. No issue closure, test-count substitution, narrow page-domain approval or generic dependency exception is introduced. T03 and G01–G09 requirements are unchanged; native production cutover still requires the separate G07 authorization packet.

Validate unique IDs/dependency references, acyclicity, preservation of every old edge, all 51 adapter assignments and every item's path to G09; read back the detailed issues, milestone/epic and new native edges. Preserve the original #903 body and decision receipts. Normal review and merge of this decision precede any T08A implementation.

`CASE_STUDY.md` remains byte-identical because its global finish-to-start semantics remain accurate. It is also an exact source input to existing adapter/page qualification; this scheduling split requires no unrelated source-proof amendments. The current plan, this note and revised issues define the new scope allocation without changing historical captures or checks.
