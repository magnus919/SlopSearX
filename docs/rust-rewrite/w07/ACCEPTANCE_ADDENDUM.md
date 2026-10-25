# W07 acceptance addendum

W07 now includes the missing receipt workflow telemetry, additional assertions against original receipt/lineage behavior, and a correction to raw artifact argument handling. These source changes have been reviewed; their native qualification and final delivery remain pending. Keep issue #913 open. This addendum records design decisions and the exact evidence still needed; it does not declare whole parity, production activation, performance improvement or Python-backend retirement.

## Source and evidence attribution

[GATE.md](GATE.md) records executed qualification for J `4c271fd759d3507c70fd436dda118a26a826d6b0`: 100 named native completions and 22 individually measured wire comparisons. Physical revision `7424b9c5b552f66e249a79d544940a35c4d52cff` is J's two-document successor, not a separate native run. Those results remain valid evidence for J only.

The current implementation commit is `17e4a0359d188e728929e8446d7113550f6d7f51`, whose parent `e552a72c3ef941ffa17031e46fcc21988cca0f63` includes the reviewed CI integration and finite source-reader reconciliation. The seven-file raw-artifact correction is committed and statically checked; native qualification remains pending. Do not attribute J's execution to this new source state.

The original [qualification.json](qualification.json) remains frozen. The current [qualification-completion-v1.json](qualification-completion-v1.json) preserves every original suite, selector and identity. Five product tests and one inverse/control test produced 106 declared identities; the raw-artifact regression adds one, producing 107 declared identities. None of those new counts is an execution receipt. The current registry binds 75 exact source leaves, including the application `Metrics` port, observability forwarding module and real collector implementation. `crates/xtask/src/w07.rs::preserve_registry` reconstructs the complete current registry from the original using only the reviewed finite delta; missing/weakened pins, dropped tests, altered selectors and unknown additions fail closed.

Both 22-case references, four complete descriptors, original source bindings and exact output assertions remain unchanged. They measure the frozen negative-call subset, not every positive Python journey. New assertions use explicit expectations from original source functions; no new Python capture, field normalization or replacement oracle is implied.

## Product correction and original-source assertions

Receipt telemetry now uses the existing owner-supplied [Metrics port](../../../crates/ssx-application/src/lib.rs), [Telemetry forwarding](../../../crates/ssx-infra/src/observability/mod.rs) and [real collector](../../../crates/ssx-infra/src/observability/metrics.rs). Source branches emit accepted/terminal only after insertion succeeds, preserve failed observations as failed, retain insert-time replay behavior, and emit the existing rejection labels for idempotency/capacity. Early retained replay emits no new receipt workflow event. No second collector, engine acquisition or verification authority is introduced.

The source assertions remain in the existing fixture owners:

| Original contract | Current native assertion |
| --- | --- |
| `tests/test_retrieval_receipts.py`: mixed success/failure, normalization, page limits, manifest attribution, atomic malformed inputs and per-item errors | `crates/ssx-mcp/tests/w07_mcp.rs::authenticated_receipts_emit_source_workflow_counters_on_the_shared_collector` checks exact normalized observations, both receipt identities/statuses, false verification, `DUMP`/`PTTL` preservation, all five malformed inputs, duplicate/per-item behavior and positive artifact edges. Existing service telemetry branch tests distinguish early and insert-time replay. |
| `tests/test_artifact_lineage_valkey.py`: independent readers and nonrenewed retention; original graph bounds, tenant denial, legacy/corrupt edges and root selection | `crates/ssx-mcp/tests/w07_composition.rs::retained_snapshot_original_bounds_foreign_legacy_and_source_expiry_contracts` checks exact parent readback, admitted nodes/edges, no dangling endpoints, foreign missing/null dates, deterministic corrupt-edge filtering, legacy lineage status and deduplicated result selection. |
| Original receipt replay after source expiry, and retained research job/attempt/snapshot lineage | That same composition test joins the logical `1000` → `1011` source-expiry transition and preserves source bytes/`TTL`. It also persists a typed completed research record and checks exact `job_summary` artifact identities, graph relationships and readonly retention. This seeded record proves retained serialization/readback; it does **not** prove a worker ran. Actual research acquisition remains the separate `W03` journey. |
| `tests/test_artifact_lineage.py`: duplicate edges, cycles and expired nodes | `lineage::source_cycle_contract::original_cycle_duplicate_edges_and_expired_nodes_are_exact_and_deterministic` checks the original three identities/statuses and concrete three-edge set through the real walker. It does not substitute a walker output for its own expected graph. |

## Accepted hardening and rejected restriction

The following seven design decisions are accepted, conditional on qualification of the actual changed source. Historical [receipt-repository-parity.json](receipt-repository-parity.json) and [mcp-tools-parity.json](mcp-tools-parity.json) retain their original claims and pending statements; this later addendum does not rewrite them.

| Difference | Design decision |
| --- | --- |
| `W07-RECEIPT-CORRUPT-PREFLIGHT` | Known corrupt preconditions and size/capacity refusal preserve bytes and retention; unexpected completion remains indeterminate. |
| `W07-RECEIPT-ID-COLLISION` | Never overwrite an existing identity; valid replay precedes unrelated index corruption. |
| `W07-RECEIPT-REPLAY-BOUNDARY` | Bound keys/digests and distinguish unavailable from missing. |
| `W07-MCP-KNOWN-CORRUPT-STATE` | Return the declared structured `store_corrupt` objects. |
| `W07-MCP-NATIVE-STORAGE-FAILURES` | Preserve declared `Closed`/`Unavailable`, `InvalidInput` and `Collision` code/message distinctions. |
| `W07-MCP-INDETERMINATE-PERSISTENCE` | Report `completion_unknown` with same-key replay guidance; never automatically retry using a new ID or claim no write occurred. |
| `W07-LINEAGE-POST-READ-AUTHORITY` | Resample grant/engine/sensitive authority after awaits while preserving cancellation, error precedence and owner drain. |

`W07-MCP-FINITE-ARBITRARY-OBJECTS` is **not accepted**. The JSON token `1e999` is syntactically valid; binary64 overflow does not make its lexical spelling an `Infinity` literal. Original `Any` artifact dictionaries reach ordered semantic shape/version/type checks. The earlier recursive finite-number refusal unnecessarily changed tool error shape and manifest grant/conflict/bounds precedence.

The reviewed correction retains raw `Observed` information until those closed artifact checks produce a typed `Result`. Invalid dictionaries are never converted to a fabricated ID, null, zero or string. Manifest errors remain deferred through existing `ManifestDraft` results; lineage retains its original semantic order. Shared `Number`/`arbitrary_precision` behavior, typed receipt strings/strict integers, normal large integers and finite output constraints remain unchanged. The schema audit found `Any` only in manifest source/artifact-list items and lineage artifact; receipt/passage strings have no successful `str(inf)` coercion path.

`evidence::tools::tests::raw_any_artifacts_preserve_source_semantic_precedence_without_storage` adds exact raw-token/error-order and zero-storage assertions. The existing [authenticated loopback fixture](../../../crates/ssx-mcp/tests/w07_mcp.rs) checks raw `1e999` requests with semantic `isError=false` results, unchanged keys/`DUMP`/`TTL` and no additional engine dispatch. These tests are authored/reviewed, not yet executed. Their five implementation/test source pins and one identity append are mandatory current qualification inputs.

## Required acceptance evidence

The current source must pass actual exact-name discovery and execution, including all 107 declared identities and unchanged 22 wire comparisons, plus inherited `T01`/`T06`/`F03` and required exact-head hosted CI/portal checks. Preserve original failures and source attribution. An unexpected count, missing/ignored test, altered reference or changed source pin requires investigation, not a waiver or silent repin.

Only actual receipts for the final source can close the remaining qualification obligations. Reviewed publication/merge, downstream native startup composition, portal release validation and `G08` retirement keep their own acceptance boundaries. Successful focused tests or this design addendum alone establish none of those outcomes.
