# ADR 008: Native runtime roots, engine assembly and server activation

- **Status:** Proposed for maintainer acceptance; no implementation authority is claimed by this document
- **Scope:** `ssx-infra` runtime composition, `ssx-server` activation, and the T08/O04 boundary
- **Decision authority:** Rust rewrite case study, planning issue #903 (T08) and #921 (O04)
- **Date:** 2026-11-05
- **Proposal updated:** 2026-11-18
- **Depends on:** ADR 003 (workspace boundaries), ADR 004 (Valkey authority), ADR 005 (protocol/TLS), ADR 007 (shared execution ownership), F01 catalog

## Context

The native search service is implemented and qualified, but nothing in the
repository builds it for production. Three measured facts:

1. `crates/ssx-server/src/main.rs` announces itself as a *foundation readiness
   server*: `Native foundation readiness server; search is not implemented`. It
   serves `ssx_infra::foundation_runtime()`, whose engine is `FakeEngine` and
   whose store is `MemoryStore(StoreRead::Found(()))`.
2. `ssx_infra::search::SearchService::new` does build the real catalog
   (`ssx_registry::Registry::builtins()` from
   `docs/rust-rewrite/f06/catalog.json`), but `SearchDependencies` — policy,
   engine bindings, cache, admission, suggestions, metrics, audit, clock,
   reranker, owned transports — is constructed **only in test code**. Every
   `SearchDependencies { .. }` literal in the tree sits under `tests/` or inside
   a `#[cfg(test)]` module.
3. ADR 007 already requires that "runtime roots supply logical adapter
   generations" and that HTTP, MCP and workers "share clones of one service".
   No such runtime root exists.

The original November 5 proposal identified an apparent T08/O04 dependency
inversion. [T08](https://github.com/magnus919/SlopSearX/issues/903) subsequently
merged its CLI integration against a separate process running the production
HTTP router, SearchService, npm adapter and private Valkey. The [T08
handoff](../rust-rewrite/t08/README.md) records that component qualification and
the independent acceptance it still requires.

[O04](https://github.com/magnus919/SlopSearX/issues/921) requires an accepted T08
handoff and owns production-shaped runtime assembly and release packaging. O04
must still qualify the packaged commands in isolation against deterministic
private upstream fixtures and disposable Valkey. Production traffic activation
requires separate [G07 authorization](https://github.com/magnus919/SlopSearX/issues/945)
after staging. The maintainer still needs to decide the runtime assembly design
proposed below.

## Decision (proposed)

**One assembler, fail-closed, shared.** Introduce a single runtime-root
constructor — proposed home `ssx_infra::runtime::RuntimeRoot`:

```text
RuntimeRoot::from_config(&ssx_config::Config, RuntimePorts) -> Result<Runtime, AssemblyError>
```

`Runtime` owns the `SearchService`, the `PolicyService`, telemetry, the Valkey
store and the explicitly owned `AdapterHttp` transports that ADR 007 requires to
be closed only after flights join. `ssx-server` parses argv and environment, then
calls it. HTTP, MCP and worker entry points that share a process use clones of
that process's service. Standalone server and worker processes use the same
assembly rules and Valkey authority, with process-local lifecycle ownership.
The gateway consumes the native MCP endpoint over its existing wire contract.

**Fail-closed engine assembly.** The assembler iterates the engine set the
registry declares. Every engine needs an explicit constructor rule; an engine
with no rule fails construction with `AssemblyError::UnassembledEngine { name }`.
A partially implemented assembler then cannot silently reduce coverage, which is
the property F01 needs and the reason this can be built incrementally.

**Generation identity.** An engine's logical generation is a digest of its
effective configuration plus its implementation identity, so a configuration or
adapter change rotates the value ADR 007 folds into dispatch identity. The
digest never includes process addresses or wall-clock values.

**Secrets and discovery.** Only environment-referenced credentials are read.
They never enter logs, telemetry or the derived metadata views. Sensitive
engines stay excluded from public discovery exactly as today.

**Activation is explicit.** `ssx-server` keeps foundation mode as a named mode
and gains a search mode only when the assembler covers the whole registry set.
Until then, a startup that requests search mode fails rather than degrading to
the fake runtime.

## Proposed migration order

1. Accept the T08 component handoff and meet O04's other predecessors before
   starting O04 implementation.
2. In O04, establish one assembly root and an executable parity check between
   the registry and the engine constructor rules. Keep construction of
   production `SearchDependencies` at that root and exercise it through the
   packaged entry points.
3. Add typed rules for credential-free engines, followed by credential-bearing
   and sensitive engines and their admission and quota configuration. Keep
   incomplete assembly unavailable for search.
4. Qualify the packaged server and workers in native search mode with the
   accepted CLI, deterministic private upstream fixtures, and disposable Valkey.
   Record exact artifacts, readiness, policy and tenant behavior, and joined
   shutdown. Carry the release artifacts forward to G06 staging.

## Consequences

- Foundation readiness remains explicit until assembled search mode is
  available and qualified.
- Because assembly is fail-closed, steps 2 and 3 are additive and reviewable per
  engine, rather than one unreviewable drop.
- The set-parity gate turns "all engines assembled" into an executable property
  instead of a checklist claim.
- `ssx-infra` owns assembly code and `ssx-server` owns entry-point mode
  selection within O04. T08 owns CLI component qualification. O04 owns packaged
  assembly and delivery to staging. G07 owns production traffic activation.

## Alternatives rejected

- **Each binary assembles its own dependencies.** Rejected: ADR 007 requires one
  shared flight registry and one owner per resource; per-binary assembly would
  duplicate safety and cache boundaries.
- **Derive dependencies from environment like `SearchConfig::from_env`.** Not
  sufficient: the existing environment parser covers execution knobs, while the
  engine set, typed engine configuration, policy catalog and reranker need typed
  configuration.
- **Extend the foundation runtime into staging.** Rejected: G06 needs the real
  packaged native commands and state migration evidence from G04, including
  runtime readiness, supervised shutdown and deterministic upstream behavior.
- **Default to a partial engine set when a rule is missing.** Rejected: it would
  silently shrink coverage, which F01 and G01 exist to prevent.

## Evidence

- `crates/ssx-server/src/main.rs`: `Native foundation readiness server; search is
  not implemented`; `ssx_infra::foundation_runtime()`.
- `crates/ssx-infra/src/lib.rs`: `foundation_runtime()` uses
  `FakeEngine(AdapterOutcome::Success)` and `MemoryStore(StoreRead::Found(()))`.
- `grep -rn 'SearchDependencies *{' crates/` returns only `tests/` paths and
  `#[cfg(test)]` modules (for example
  `crates/ssx-infra/src/research_runner/completion_tests.rs`).
- ADR 007: "Runtime roots supply logical adapter generations"; "HTTP, MCP and
  workers must share clones of one service".
- The [November 5 assessment](../rust-rewrite/MILESTONE_11_ASSESSMENT.md)
  retains the original sequencing concern. The live [O04 handoff
  criteria](https://github.com/magnus919/SlopSearX/issues/921) preserve the
  T08 → O04 dependency.
