# ADR 008: Native runtime roots, engine assembly and server activation

- **Status:** Proposed for maintainer acceptance; no implementation authority is claimed by this document
- **Scope:** `ssx-infra` runtime composition, `ssx-server` activation, and the T08/O04 boundary
- **Decision authority:** Rust rewrite case study, planning issue #903 (T08) and #921 (O04)
- **Date:** 2026-11-05
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

That leaves a real gap in the dependency graph as well as the code: T08 (#903)
must integrate the native client "against an actual separately launched native
HTTP service", while O04 (#921) is listed as blocked by T08 yet its assembly
role is described elsewhere — `docs/rust-rewrite/t03/CORE_DELIVERY.md` calls
`HttpState::with_public_portal` "the composition seam for later O04 server
assembly". The two issues are therefore mutually dependent in substance, so no
one starts.

## Decision (proposed)

**One assembler, fail-closed, shared.** Introduce a single runtime-root
constructor — proposed home `ssx_infra::runtime::RuntimeRoot`:

```text
RuntimeRoot::from_config(&ssx_config::Config, RuntimePorts) -> Result<Runtime, AssemblyError>
```

`Runtime` owns the `SearchService`, the `PolicyService`, telemetry, the Valkey
store and the explicitly owned `AdapterHttp` transports that ADR 007 requires to
be closed only after flights join. `ssx-server` parses argv and environment, then
calls it. The gateway, MCP server and workers obtain the same root or a clone of
its services; none of them rebuilds `SearchDependencies`.

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

1. Assembler skeleton plus a set-parity gate: registry engine names must equal
   assembled engine names, and a test asserts that no `SearchDependencies`
   literal exists outside test modules.
2. Engines that already expose a typed runtime module and need no credential
   (`arxiv`, `openalex`, `semanticscholar`) and the credential-free scrape and
   reference engines.
3. Credential-bearing and sensitive engines, including the F10 admission and
   quota paths.
4. Enable `ssx-server` search mode and switch the T02 image to it.
5. T08 integration against that service, then O04 release composition.

## Consequences

- Until step 4, `ssx-server` continues to declare foundation mode explicitly;
  nothing silently claims search readiness.
- Because assembly is fail-closed, steps 2 and 3 are additive and reviewable per
  engine, rather than one unreviewable drop.
- The set-parity gate turns "all engines assembled" into an executable property
  instead of a checklist claim.
- Ownership becomes unambiguous: `ssx-infra` owns assembly, `ssx-server` owns
  activation, T08 owns client integration, O04 owns release composition. This
  removes the #903/#921 inversion without duplicating a shared interface.

## Alternatives rejected

- **Each binary assembles its own dependencies.** Rejected: ADR 007 requires one
  shared flight registry and one owner per resource; per-binary assembly would
  duplicate safety and cache boundaries.
- **Derive dependencies from environment like `SearchConfig::from_env`.** Not
  sufficient: the existing environment parser covers execution knobs, while the
  engine set, typed engine configuration, policy catalog and reranker need typed
  configuration.
- **Keep the fake runtime until G04 staging.** Rejected: T08 cannot integrate
  against it, so G-series qualification cannot begin, and the remaining
  milestone issues stay transitively blocked.
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
- Milestone assessment `docs/rust-rewrite/MILESTONE_11_ASSESSMENT.md` records the
  corresponding dependency inversion.
