# F06 implementation evidence

F06 owns the static config/catalog seam; later adapter batches provide engine-local factory registrations and metadata changes. New provider ports must update only their engine fragment and provider crate files. The catalog, runtime lookup, and documentation aggregate are generated or validated centrally.

`cargo xtask check F06` is the focused acceptance gate. It checks pinned Python source blobs, fragment-to-catalog equality, exact per-engine Jev roles and routing-card text from the separately frozen source-derived `routing-expectations.json`, a same-count card-swap negative case, descriptor completeness, the 51-engine set, generated Markdown, source-case coverage, and native config/registry tests. The frozen fixture records the `slopsearx/jev.py` Git blob from `source-pins.json`; aggregate role counts alone are not considered parity evidence. It does not contact providers, use live credentials, or claim that any provider has been ported.

Engine factories are registered against descriptors in the registry crate. The application/engine wiring layer owns an immutable application context and engine-local resolved configuration, which a factory can capture through `Arc`; the registry does not depend on configuration or application crates. Adding a provider therefore remains an engine-local registration and does not require an orchestrator edit.

The source-backed compatibility record and explicit native type-validation boundaries are in [PARITY.md](PARITY.md). The current aggregate is [catalog.json](catalog.json), with one JSON descriptor per engine under `fragments/` and [generated catalog documentation](engine-catalog.md).

`crates/xtask/fixtures/f06/scalars.json` contains 21 actual Python 3.14.7
`load_config` observations against the exact pinned config source: raw scalar
kinds/values, numeric grammar, None-default text, YAML-origin coercion, ordered
alias resolution, list stripping and expected errors. The native config tests
replay those inputs; F06 independently pins the exact fixture SHA256 and rejects
changed answers. No Python helper is invoked by the native gate. The capture
uses only noncredential settings. Raw `ConfigScalar` values require `effective_i64`/`effective_f64` plus
consumer-specific bounds before use by later native adapters.

`crates/xtask/fixtures/f06/credentials.json` separately captures ten synthetic
file/environment credential cases from Python 3.14.7 against the same pinned
config source. Native replay checks nullable strings, independent overrides,
empty values, custom engines, alias precedence and redaction. Its independently
pinned digest prevents fixture answers from redefining their own identity.
Malformed nontext YAML credentials fail with a redacted type error, as documented
in the parity record. No live credentials or providers are used.

The separate `yaml.json` corpus covers 79 parser cases and seven actual config
origin cases against Python 3.14.7 / PyYAML 6.0.3. Native parsing uses public
style/tag-aware events from the existing pinned parser, not YAML text rewriting.
Alias/merge behavior, resource limits and explicit JSON-profile differences are
recorded in the parity document. The original scalar and credential corpora are
not changed by this correction.

The native foundation workflow runs F06 on both Linux x86_64 and aarch64 jobs
and retains `F06.json` with the other source-bound reports. This is additive to
the legacy CI gates; the O06 workflow ledger checks the exact step and upload.
