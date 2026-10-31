# Native gateway dependency boundary

This successor composes accepted T07 parent `1c3a3d6cfd9b20334b46f6a73b8fb068fe78d958`
with reviewed main `e9c5911a468b892fe2adef9027374cb867a2535d`. The genuine offline
Cargo resolver added only local `ssx-source-json` and its infra/MCP edges; every external
package version, source and checksum is unchanged. Selected native and Windows gateway
dependency trees exclude the server closure, while direct default MCP and xtask trees
retain it. Those are metadata results, not compilation, Windows execution or installation
acceptance. Current combined-source qualification remains pending.

The gateway uses the genuine `ssx-mcp` library without its default `server` feature.
Direct default builds of `ssx-mcp` still enable `server`. Existing xtask and infra-test
consumers explicitly request server support. Server modules, including main's evidence module, and the four optional
application/infra/config/registry dependencies remain behind that feature; shared
opaque framing, registry, discovery and lifecycle code have one implementation.

`ssx-source-json` owns the existing parser, accounting, future ownership and diagnostic
files. `ssx_infra::adapters::source_json` re-exports the same concrete types and APIs.
The parser still uses the existing domain, finite JSON and budget primitives. Its
limits, source fixtures, number/depth/Unicode behavior and external dependency pins
are unchanged. `Text::from_points` is public so existing adapter callers can construct
the same code-point values across the crate boundary. Relative fixture includes move
one directory level; no fixture data or parser implementation is duplicated.

Gateway tests select `server-test-fixtures` explicitly for the accepted T06 server
comparison. The ordinary client build and portable owner/runtime tests need no
server/engine implementation. T07 enables that feature for its runtime suite so the
existing catalog comparison is retained. A future Windows job must use the portable
client suites and never claim the Linux/macOS public T07 inventory executed there.

## Qualification relocation

All 17 parser test functions move from package `ssx-infra`, prefix
`adapters::source_json::`, to package `ssx-source-json`, prefix `source_json::`.
No test function is removed. A06 preserves its 16 historical parser identities plus
the explicit current decoded-integer control, requires exact compiled lists and completed
names in both physical libraries, and retains the original logical regression bindings.
Its four frozen helper rows authenticate the relocated leaves before recovering their
exact accepted Git bytes. A04 still executes its five grammar/source controls;
F15 executes the complete shared library suite as well as its remaining infra suite.
T07 changes its one decoded-integer selector to the shared package. A13 retains the
frozen logical inventory and checks the union of the two real compiled inventories,
projecting only the exact shared prefix back to its historical identity. Its current
inventory explicitly includes T07's decoded-integer control; historical capture data
is not rewritten.

The finite current-source pin successors cover A04/A06/A13/F15 and T07, including
source maps in the A13 replay fixture. Frozen source/qualification records remain
unchanged. The twelve-path joint capsule proves both accepted Git frames from the same
physical bytes. The existing opaque proof first removes its one exact server-fixture
cfg annotation, then keeps every original owner and archive check. W07 current source
coverage grows from 75 to 78 pins for the three new proof inputs; its 108 identities,
original 100 and measured wire 22 are preserved.

The relocation is a current implementation
successor, not permission to weaken a source hash check. Refresh genuine unit-profile
project bindings through the existing handlers when needed; do not trim manifests or
reuse an old profile as current evidence.

Required feature proofs include client-only build/tree (no infra/libxml), the direct
MCP default and existing server consumers, combined workspace feature unification,
server-enabled gateway fixtures, all moved parser tests and the current T07 inventory.
Windows then needs native MSVC builds of gateway/DNS/input executables, the five owner
controls, portable runtime and real installation/tamper/ACL/TLS cases in hosted CI.

All ADR005 targets remain committed: Linux x86_64/aarch64, macOS x86_64/aarch64 and
Windows x86_64. This change preserves libxml's platform guard and does not enable
native Windows servers. O04 retains release signing/distribution acceptance.
