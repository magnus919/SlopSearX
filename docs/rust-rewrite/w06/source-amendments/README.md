# W06 source amendment

`current-source-amendment-v1.json` records one narrow successor to the frozen
W06 owned-source pin for `crates/ssx-application/src/lib.rs`. The archived
`de04` bytes are the exact source pinned by W06 qualification before W07. The
only permitted suffix adds the two W07 module exports for `lineage` and
`receipts`. The xtask W06 check validates the manifest identity, archive bytes,
parent Git blob, and complete successor bytes. It rejects any other source
change.

The original W06 source captures, packets, qualification, and their source
bindings remain unchanged. This amendment only reconciles a source identity;
it does not claim W06 behavioral parity or broaden W06 coverage.

The O01 source gate reuses this same exact amendment verifier for the
`crates/ssx-application/src/lib.rs` artifact. It still requires the pinned
O01 coverage manifest and validates the frozen W06 source, archived parent,
and complete reviewed W07 successor; no other O01 artifact receives this
amendment.

A09 and A12 each also pin `crates/xtask/src/main.rs`. The separate
`xtask-main-source-amendment-v1.json` and exact `de04` main archive prove only
one insertion: `mod w06_source_amendment;` immediately after the unique
`mod w06;` declaration. The complete parent/current bytes, lengths, SHA-256
and parent/HEAD Git blobs are checked; extra or missing declarations fail.
The original lib amendment and all captured pin manifests remain unchanged.

The shared A13 wrapper intercepts exactly five frozen consumer/source bindings:
O01 coverage for lib; A09 coverage for lib and main; A12 qualification for lib
and main. It checks each hardcoded container digest against both the caller's
pin and actual manifest bytes, and requires the exact historical source digest
and frozen caller field-presence contract. All five rows omit `bytes`, so their
actual caller value must be `None`; the independent archive/material proofs
still enforce the exact historical and current lengths. Other paths retain the original A13 validator. A09/A12 gate source
and A13's V5 proof remain unchanged, avoiding a recursive amendment chain.
The helper retains actual Git identity checks and emits compact
`W07_SOURCE_AMENDMENT_PROOF` JSON to captured gate stderr (command arguments,
exit codes and output digests); W06 retains its existing report command records.
Shared scope/manifest/pin/length controls and each source's mutation controls
run before accepting a source reconciliation. These checks reconcile exact
source identities, not new behavioral coverage or a test-inventory change.

Positive binding controls derive all five caller arguments from the exact frozen
`artifacts` (O01/A09) and `sources` (A12) rows, rather than inventing optional
length fields. Unexpected `Some` lengths, including the otherwise correct
historical number, are rejected as changes to those frozen caller contracts.
