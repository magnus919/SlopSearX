# Rust 1.99.1 builder successor

PR #1116 updates the digest-pinned healthcheck builder, not the repository's
Rust 1.99.0 compiler contract. The reviewed successor reconstructs the accepted
Dockerfile and two interim Python source operands before the original O02/F01
proofs interpret them. Their historical tuples and F01 inventory stay unchanged.

The capsule's compiled SHA-256 is
`dfec3598f0142f7d6d4839d2779e42a4923ab0e40b5f6c817e82670f7937011b`.
It owns exactly three paths and five ordered edits. Current bytes must pass the
existing descriptor-bound regular0644 reader first. Both the runtime capsule
and each source leaf receive its exact physical size/hash/mode/identity checks.
The inverse verifies the accepted prior size/hash, then reconstructs the full
current bytes forward. Mutable endpoint declarations cannot authorize a change:
any capsule edit fails the compiled outer identity.

`capsule.json` records the exact before/after endpoints and edits against
`011dbfd7eedc47cf60d7d27569d90cee1587da21`. `t08a-bindings-prior.json` preserves
the previous binding document byte-for-byte. The current T08a document changes
only the O02 reader row and adds the successor module and capsule rows; the
binding audit records both document identities and the exact changed row set.
These are reviewed current native bindings, not rewritten historical O02 hashes.
The original snapshot amendment, original F01 inventory, compiler pin, Cargo
lock and historical O02 operands remain authoritative and unchanged.

The regression controls exercise actual three-source reconstruction, rejection
of historical/changed/empty/duplicate operands, forged caller expectations,
missing/duplicate/reordered/extra or modified capsule operations and endpoints,
and the real source reader with non-pin edits, oversized files, wrong physical
mode, source/capsule symlinks, and missing/forged runtime capsules. The existing
reader retains its descriptor identity checks before and after bounded reads.
Real F01 source-consumer and T08a binding tests are required in addition to these
component controls. Container architecture, portal, dependency-policy and
independent exact-head review gates still govern merge. No deployment or
production validation follows from these checks.

Rollback reverts the builder update and its successor together. A future image
or source change requires another reviewed endpoint; it cannot self-rehash this
capsule or silently widen the version allowlists.
