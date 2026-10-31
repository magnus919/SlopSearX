# W08 local validation

The native-qualified code head is `a67a156100d30f87cb824a52699ae575da8dc0cd`. [LOCAL_ACCEPTANCE.json](LOCAL_ACCEPTANCE.json) records actual original process exits and receipt hashes. This documents W08 service/source semantics, not production, provider quality or the full rewrite. See the [Rust rewrite case study](../CASE_STUDY.md).

| Command/control | Actual result |
| --- | --- |
| `cargo xtask check W08` | 26 completed cases: 13 application and 13 real private-Valkey/loopback infrastructure cases; zero ignored |
| `cargo xtask check W02` | 28 existing worker cases and three completion/lease controls; zero ignored |
| Standalone MCP T06 source-binding target | Five existing cases passed; no added predecessor test identities |
| W08, W07 and shared A13 source controls | 2 + 7 + 1 passed; old/new body, mode, caller, missing/duplicate/relocated-span and filesystem controls retained |
| Scoped strict Clippy | Application, infrastructure and xtask, all targets, `-D warnings`: actual exit 0 |
| `cargo fmt --all --check` | Actual exit 0 |

The gate artifacts are `/Volumes/tank01/magnus/rust-builds/slopsearx/w08-native-composition-20261027/quality-04/W08.json` (SHA256 `82b4cf4c6c56ede5f5797aaffd9336d0a516b0b54f67534023a9337fa8c0a1c5`) and `/Volumes/tank01/magnus/rust-builds/slopsearx/w08-native-composition-20261027/quality-04/W02.json` (SHA256 `be1e60597af1b0a637d6a79c50e51a6d91437c7101c198bb994722dd0d947f4f`). Every execution used pinned Rust 1.99.0 and Valkey 8.1.10, locked/offline Cargo, two jobs, debug/incremental disabled, the existing target/cache and three exclusive leases. The final target used 15,810,449,408 allocated bytes; it stayed below the 16 GiB soft threshold and 18 GiB hard limit. Original processes retired, scratch was removed and leases were released. Raw earlier compiler, fixture, policy and source-control failures remain retained beside the passing runs.

The regression map, qualification registry, all 20 sealed inputs and current five-source proof capsule remain exactly the qualified bytes. Its old pending-status field is historical; the companion acceptance receipt supplies current results. Only W08 narrative/status documentation changes afterward. The final task delivery receipt distinguishes this documentation child from the qualified head and records actual all-files/scoped hook and final AST results. No native gate is presented as having run on a later documentation commit.

W09 must wire final authenticated MCP schemas/registry/transports and a trusted live principal/operator-to-tenant authority source for leased work across restarts and replicas. Stored job tenants and source descriptors grant no authority. W10 owns protected portal composition UI and browser acceptance; existing portal CI remains a delivery requirement. Native fixtures make no live-provider, production, relevance or vulnerability-applicability claim. Acknowledged reservations remain monotonic; a dispatch-admission witness cannot revoke an already in-flight request.
