# Native CI and protected review successors (O06)

For the measured AMD64/ARM64 Foundation runtime observation, implemented timeout budget, and qualification rule, see [Foundation CI runtime budget](FOUNDATION_RUNTIME_BUDGET.md). Foundation rerun artifact identity and the same-run reuse contract are documented in [Foundation workflow reruns](FOUNDATION_RERUNS.md).

O06 prepares additive successors. Existing Python checks, release workflows,
protected review analysis and publication remain enabled and unchanged. O09
owns activation after each semantic lane is accepted. A green O06 result means
that the complete executable gate inventory matches its successor ledger,
protected source still has its accepted provenance and authority, and the
native helper/adversarial tests pass. It does not certify the pending backend,
portal, adapter, release, coverage or experiment ports.

## Complete transition ledger

`gates.json` records every complete workflow job and step, all workflow-level
fields outside `jobs`, successor, owner and state. This includes triggers,
permissions, needs, strategy, timeouts, environments, concurrency and both job
and step `continue-on-error`. Native validation compares
sets and complete step values, so removing a job, adding an unmapped job, or
changing a run command cannot pass by preserving a count. The manifest also
contains every configured pre-commit hook and the current PR template. Hook
successor coverage is checked by ID. Legacy Python sources are already owned
in F01's `inventory/python-disposition.json` and inline invocations in
`inventory/python-invocations.json`; this ledger adds the gate-level handoff.

The `pending-owner` entries deliberately require O02/O04 shipping and registry
helpers, O05 behavioral/coverage suites, T07 browser tests, O07 upstream probes,
and O09 architecture/security activation. O06 cannot replace those with a
passing placeholder. Native Rust type/build/test, fmt/Clippy and cargo-deny
already run in the additive F03 workflow. O09 must retain each gate until its
listed owner provides meaningful native evidence, and must explicitly review
metric changes such as Python complexity analysis or native code coverage.
The existing Python CodeQL result is not Rust security coverage.

The native `source-hygiene` successor checks the new Rust/CI sources and this
CI documentation for trailing whitespace, final newline, CR/mixed endings,
conflict markers, 500 KiB size, JSON and YAML validity. Historical evidence is
outside this new-source scope and stays byte-exact. It does not claim all-config
TOML parsing or legacy Python AST analysis; those are explicit O09 transitions.

## Native commands

```sh
cargo test --locked -p xtask 'o06::tests::'
cargo xtask check O06 --output target/rust-evidence/O06.json
cargo xtask ci-helper inventory
cargo xtask ci-helper source-hygiene
cargo xtask ci-helper mirror-config /path/to/daemon.json
cargo xtask ci-helper manifest /path/to/oci-index.json
cargo xtask ci-helper review-bootstrap-plan /absolute/temp/trusted-review-harness
cargo xtask ci-helper review-bootstrap /absolute/temp/trusted-review-harness
```

`mirror-config` emits a merged configuration while preserving unrelated settings
and existing mirrors; repeated runs are idempotent. It performs no privileged
write or daemon restart. O02/O09 must retain the hosted-Linux admission,
running-container guard, atomic file replacement and restart lifecycle when
wiring the emitted configuration. `manifest` rejects mutable/malformed child
digests, non-index documents, missing or ambiguous Linux amd64/arm64 entries.
It validates supplied OCI index contents, not remote tag identity or the digest
of the index bytes; O04 retains responsibility for authenticated registry
resolution, byte/digest binding, promotion writes and portal release smoke.
Both are executable native helpers with independent negative tests, not claims
that the entire release lane has already been ported.

`.github/workflows/rust-ci-successors.yml` has only contents:read and no secrets.
It runs on ordinary PRs and main pushes and retains source-bound O06 evidence.
Its separately scoped manual-only Python-client job installs no backend package:
it checks the standalone `ssx` syntax/help. This optional compatibility job is
not a prerequisite of native backend checks. O05 owns additional client wire
contracts when the real native transport exists.

## Protected review decision and authority

The external integration decision is to retain the separately versioned,
immutable `groktopus/codereview` analysis workflow and protected publication
action as external services/tools. Their implementation language is outside
the SlopSearX backend/runtime retirement boundary. This is not permission for a
project-owned Python review shim, remote Python backend fallback, or execution
of PR-head code in a writer job. The current external harness installation
remains an external dependency; it must never install the target project.

The native `review-bootstrap` command is the concrete successor to the
project-owned inline Python clone/checkout bootstrap. It accepts only the
accepted repository, exact harness SHA and protected policy identity. It
requires a nonexistent absolute `trusted-review-harness` directory outside the
target source, clears inherited environment and Git global/system config,
disables credentials/prompts/hooks, bounds subprocess deadlines, clones without
checkout, checks out only the accepted detached revision, and verifies HEAD.
It neither imports nor executes the harness or target. Failures are nonzero and
may leave a disposable partial external checkout; reruns refuse that existing
destination instead of trusting or mutating it. No review is sent by O06.

O09 wires this native bootstrap into the protected-source publisher while
retaining the exact external publication action, official lockfile-pinned
artifact bridge and external harness installation. O09 must update publisher
policy/source provenance through the protected review process: changing the
workflow invalidates its current published SHA-256 identity. It must not simply
edit `pr-publish.yml` and self-assert a new policy hash. The separately disabled
source-context probe remains disabled; its distinct historical harness pin is
not silently upgraded to the publisher pin. O09 may retire that disabled local
wrapper rather than enable a new diagnostic.

O06 checks the complete protected publisher, analysis, disabled probe and
policy against accepted **compiled Rust blob constants**, as well as their
source manifest. It separately parses publisher trigger, protected revision,
credential persistence, environment and exact permission scope. Changing the
workflow and refreshing the editable manifest is insufficient: independent
adversarial tests do exactly that for weaker event conditions, untrusted head
checkout, widened permissions, mutable harness pin and persisted credentials.
Other tests reject wrong repository/revision/credential/artifact-trust mode,
preexisting/in-source destinations, failed clone and mismatched observed HEAD.
The pinned external runtime continues to own authenticated artifact download,
bounded admission, artifact digest/provenance, current-head reconciliation and
review publication; this preparation check does not replace its security gate.

## Parser dependency

`serde-saphyr` is pinned exactly at 2.0.0, deserialize-only. The workspace had
serde_json but no YAML parser; workflow authority cannot be safely analyzed by
substring matching or a homegrown indentation parser. Package-owner metadata
at [crates.io 2.0.0](https://crates.io/crates/serde-saphyr/2.0.0) and the
[upstream source](https://github.com/bourumir-wyngs/serde-saphyr) reports
MIT OR Apache-2.0 and MSRV 1.89, compatible with accepted Rust 1.99 and existing
license policy. The lockfile records exact transitive identities. Includes,
properties, serialization and comments features are disabled. O06 reads at most
4 MiB per CI input, caps parser events/nesting, and rejects duplicate keys,
anchors and aliases. Tests exercise malformed and ambiguous documents. Existing
F02 architecture dependency pins are unchanged; this is a tooling-only addition.
