# Local-fork upstream advisory coverage

## The gap this closes

cargo-deny 0.20.2 converts a local path package's absent Cargo metadata `source`
into "no source" and skips it before advisory matching
(`src/advisories/helpers/db.rs`, `let ksrc = km.krate.source.as_ref()?;`). The
reviewed private forks therefore carried real upstream code with no upstream
advisory identity:

| local package | upstream identity | vendored root |
| --- | --- | --- |
| `ssx-cli-hyper` | `hyper` 1.12.0 | `vendor/ssx-cli-hyper` |
| `process-wrap` | `process-wrap` 10.0.1 | `vendor/ssx-cli-process-wrap` |
| `libxml` | `libxml` 0.3.22 | `vendor/libxml` |

A green `cargo deny check all` on the physical graph cannot show that those
copies were scanned, because the scan never saw them. `ssx-source-xml`
(`vendor/expat`) is project-owned: it wraps Expat 2.9.0 with a hand-written FFI
surface and is not a fork of an upstream Rust crate, so it has no upstream
advisory identity to restore.

## Evidence boundary

The derived view is **upstream advisory identity evidence only**. It is never
resolver, feature, TLS, dependency-source or shipping evidence, and it does not
replace the physical `cargo deny check all`. It also does not prove that the
private patches are free of bugs: source review and behavioural qualification
remain required, exactly as the per-fork maintenance guides already say.

Concretely, `cargo xtask ci-helper local-fork-advisory`:

1. runs `cargo metadata --locked` on the real workspace and retains the exact
   physical document at
   `target/rust-evidence/local-fork-advisory-physical-metadata.json`;
2. rewrites **only** the reviewed local forks to their upstream registry
   identity in a second document at
   `target/rust-evidence/local-fork-advisory-metadata.json`, leaving every
   package `id` and every `resolve` edge byte-identical, so the derived document
   keeps the same graph shape;
3. runs the pinned cargo-deny 0.20.2 with
   `--metadata-path <derived> --config deny.toml check advisories` against a
   freshly fetched RustSec database, and records the tool version, database
   revision, database commit time, exact command, exit code and every
   diagnostic;
4. fails the gate if the run exits non-zero or reports any advisory finding.

Only `check advisories` is run on the derived view. `check licenses`, `bans`
and `sources` stay on the physical graph, where they are meaningful.

## Mapping and fail-closed rules

`docs/rust-rewrite/t08a/local-fork-advisory-mappings.json` (schema
`local-fork-advisory-mappings-v2`) is the single reviewed inventory. Each
`third_party_fork` row is bound to exact bytes:

- the physical `manifest` path, byte count and SHA-256;
- the declared `license`, which must equal the live package `license`;
- the upstream `name`, `version` and `source` (today only the crates.io index is
  accepted, so an unknown or mistyped source fails before the checker runs);
- the pristine upstream `archive` `.crate` path, byte count and SHA-256;
- the reviewed `patch` path, byte count and SHA-256;
- the `provenance` record path, byte count and SHA-256, whose own
  `upstream.name`, `upstream.version` and `upstream.archive_sha256` must agree
  with the row.

`project_owned` rows declare the manifest and license only, and must not carry
upstream identity fields.

The adapter fails closed when:

- any pinned file is missing, changes size or changes digest;
- a mapping row matches zero or more than one live local package;
- two rows claim the same `(local_name, local_version)`;
- a live local package's manifest path or license disagrees with its row;
- a live local package with `source: null` is neither reviewed nor a workspace
  member inside a declared `project_owned_roots` prefix (`crates/`);
- a row declares an upstream source other than the crates.io index.

Adding a vendor directory, renaming a fork, or bumping a fork version therefore
breaks the gate until the mapping and its provenance are updated in the same
review.

## Deterministic fixtures

`cargo xtask ci-helper local-fork-advisory-fixtures` materialises the checked-in
synthetic advisory databases under
`docs/rust-rewrite/t08a/fixtures/` into date-pinned git repositories, serves
them with `git daemon` on a loopback port, and runs the real pinned checker
three times:

| case | database | metadata | expectation |
| --- | --- | --- | --- |
| `raw-physical` | `advisory-db` | physical | `RUSTSEC-2020-9002` `hyper` 1.12.0 once and `RUSTSEC-2020-9004` `process-wrap` 10.0.1 once; the local `libxml` fork is **not** reported |
| `derived-view` | `advisory-db` | derived | `RUSTSEC-2020-9001` `libxml` 0.3.22 once, `RUSTSEC-2020-9002` twice, `RUSTSEC-2020-9004` twice; `RUSTSEC-2020-9003` never |
| `clean-database` | `advisory-db-clean` | derived | exit 0 and no diagnostic |

The fixture advisories pin an exact version (`patched = ["> X", "< X"]`, which
leaves only `X` affected), so the derived case proves the checker matched the
recorded package **and version**, and the absence of `RUSTSEC-2020-9003` (which
only affects 0.3.21) proves the version is actually evaluated. Each case asserts
the advisory id, the advisory package, the matched package and the matched
version parsed from the diagnostic's own lock span, and that the exit code
agrees with the finding count. A non-zero exit alone is never accepted as
detection.

The fixtures use the installed pinned checker (`SLOPSEARX_CARGO_DENY` may point
at an exact binary; otherwise `cargo deny` is used) and the real
`--metadata-path` interface, so a parse or configuration error cannot pass as a
vulnerability.

## CI

The required `Supply-chain policy (cargo-deny 0.20.2)` job runs, in order:
`cargo deny --locked --workspace check all`, the fixture suite, then the real
derived-view gate, and uploads
`target/rust-evidence/local-fork-advisory*.json` as the
`local-fork-advisory-evidence` artifact. The job keeps its existing trigger,
permissions, runner and identity. The workflow source stays mirrored in
`docs/rust-rewrite/ci/gates.json` and bound through
`docs/rust-rewrite/t08a/bindings.json`.

## Known limits

- Where the same upstream version is also a live registry dependency (`hyper`
  via axum/reqwest, `process-wrap` via rmcp), the derived view reports the
  advisory once per physical copy. That is intentional: the private fork is a
  distinct copy of the code, and collapsing the two would hide which one needs
  remediation.
- The `libxml` fork is a materially modified tree (owned-budget feature, path
  dependency on `ssx-owned-budget`, a build script that pins the vendored
  libxml2 2.15.4 C archive, and additional FFI modules). Its
  `libxml-provenance.json` records the exact pristine crates.io archive, the
  complete file map and the zero-context patch that reproduces the vendored
  tree; the C archive is pinned separately by `vendor/libxml/build.rs`. The A05
  adoption packet is not ready for adoption and this gate does not change that.
- Advisory coverage is upstream identity coverage. It is not a statement that
  the reviewed patches are safe, and it never authorises a registry, Reqwest or
  Python fallback.

## Maintenance

When a fork is added, renamed or version-bumped, update the mapping row, the
pristine archive, the patch and the provenance record in the same change, then
re-run both helpers. When upstream provenance is refreshed, keep the previous
archive, patch, evidence and exact command output in history. Do not widen
`project_owned_roots` to make a new unlisted vendor tree pass; classify it as a
reviewed fork with real provenance instead.
