# F02 Rust architecture handoff

F02 implements [plan item F02](../plan.json) after the accepted [F01 inventory](../inventory/README.md). Its decisions are [ADR 003: workspace and contract boundaries](../../adr/003-rust-workspace-and-contract-boundaries.md), [ADR 004: Valkey authority and migration](../../adr/004-rust-valkey-authority-and-migration.md), and [ADR 005: protocol, TLS and platforms](../../adr/005-rust-protocol-tls-and-platforms.md). The [state compatibility register](state-compatibility.md) maps those decisions to every F01 state family. [Candidate pins](pins.json) are exact version inputs for F03/T01; they are not a compiled lockfile or a native parity claim.

The proposed F02 design, contingent on maintainer acceptance of the ADRs, requires one shared search/policy application boundary with explicit surface-specific policy inputs; a complete engine descriptor registry; wire and durable DTOs independent of internal Rust types; Valkey fencing for durable commits; one Python/Rust execution owner during cutover; full stdio, streamable HTTP and legacy SSE parity; direct TLS for the protected workflow portal; and Linux amd64/arm64 server images. macOS/Windows native clients require their own release evidence before support is advertised. Python remains only a temporary, pinned comparison oracle during migration and an optional separately packaged wire client, never a final backend or mandatory native gate.

| Downstream owner | Accepted F02 input | First executable proof |
| --- | --- | --- |
| F03/F04/F05 | Crate graph, exact pins, port/injection seam, wire/durable DTO separation | Locked workspace build, byte fixtures and nonzero native mismatch gate |
| F06/F09/F14/F15 | Engine-local descriptors, one application policy decision, normalized service | Registry completeness, all-path denial matrix, service integration |
| F07/F10/F13, W00–W11 | State compatibility and exclusive fenced mutation | Real-Valkey races, interrupted recovery, per-namespace reverse readers |
| T01–T09 | Complete protocol/schema/OAuth/portal/TLS boundaries | Real-client transport matrix, negative security suite, browser/CLI journeys |
| O02/O04, G04–G08 | Target matrix, trust roots, cutover/recovery classes | Exact artifact install, CA/TLS and staging migration/rollback rehearsal |

Every row in F01 remains pending until its implementation owner links executable evidence. A pin change, transport retirement, storage incompatibility or policy tightening requires an ADR amendment and affected contract/intentional-difference update before implementation fan-out. The upstream main revision and F01 inventory baseline must be reconciled under `inventory/DRIFT.md` at every merge.

F05 amends ADR 003 with the pure `ssx-json` serialization crate. Domain and wire
consume its bounded lossless JSON values independently; it has no backend or
network dependencies. The native gate rejects serde_json `arbitrary_precision`
feature unification and qualifies huge integers and ordinary reserved-looking
object keys through storage and public DTO serialization.

F08 adds [ADR 006](../../adr/006-adapter-http-bounds.md), exactly pinned flate2
1.1.9 and reqwest's cookies feature for isolated search jars with bounded,
independently counted encoded/decoded bodies. Decoder ownership and shutdown
are qualified separately from provider parsers in the [F08 handoff](../f08/README.md).
