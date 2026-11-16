# ADR 009: Upgrade the rmcp security baseline

- **Status:** Proposed for acceptance with PR #1115; implementation is authorized by the maintainer's blocker-remediation request, not a production deployment approval
- **Scope:** Amendment to ADR 005's SDK version; existing transport, policy and platform contracts remain applicable
- **Decision authority:** Maintainer review and merge of [PR #1115](https://github.com/magnus919/SlopSearX/pull/1115)
- **Date:** 2026-11-12
- **Depends on:** [ADR 005](005-rust-protocol-tls-and-platforms.md), F02 historical architecture record, T01/T06 transport qualification

## Context

The live rmcp 3.5.1 dependency fails the supply-chain gate for
RUSTSEC-2026-0457, RUSTSEC-2026-0461 and RUSTSEC-2026-0462. The
[3.9.0 release](https://github.com/modelcontextprotocol/rust-sdk/releases/tag/rmcp-v3.9.0)
and [RustSec's patched floor](https://rustsec.org/advisories/RUSTSEC-2026-0462.html)
provide the upgrade target. The F02 `architecture/pins.json` file is a frozen
proposal-era input, including its original retrieval dates and SDK observations;
it is not the live lockfile. Rewriting it incorrectly invalidated the F02 gate
on candidate `018d0c4f`. Preserve that historical record byte-for-byte.

## Decision

Pin the live workspace manifest and lockfile to rmcp and rmcp-macros 3.9.0.
Their exact versions and checksums are authoritative in `Cargo.toml` and
`Cargo.lock`. The finite [security successor](../rust-rewrite/t08a/rmcp-security-successor.json)
authenticates current and predecessor manifest/lock bytes and reconstructs the
unchanged historical inputs before existing source-custody checks. This ADR
supersedes only the proposal's rmcp version for current builds, not its other
architecture decisions or evidence.

Retain the SDK's new diagnostic text for the two malformed outer-parameter
requests documented in [T06 parity](../rust-rewrite/t06/PARITY.md). Keep the
frozen capture unchanged and explicitly assert both historical and current
text in the transport test. HTTP415, wrong-token401 before parsing and zero
engine dispatch remain required. No advisory exemption or broader diagnostic
parity claim is introduced.

## Alternatives and consequences

Keeping 3.5.1 leaves the reported advisories unresolved. Suppressing the
advisories would weaken the gate. Replacing frozen proposal/capture bytes
would erase the basis of historical qualification. The selected upgrade adds
exact successor checks and requires fresh runtime and platform qualification.
Rollback is a reviewed dependency change with renewed security assessment;
reverting to a vulnerable version is not an acceptable green-build shortcut.

## Confirmation

The PR maintainer owns acceptance and any future amendment. On each candidate,
CI owners run cargo-deny without exemptions, F02, source-custody controls and
the existing MCP/gateway and platform gates. Any failure blocks this PR's
acceptance; it is investigated rather than waived. These checks remain while
the dependency is used and are revised only with a reviewed successor.

Candidate `018d0c4f` passed local cargo-deny, 174 MCP/gateway tests, 200 xtask
tests and strict Clippy, but failed hosted F02 because the frozen proposal was
edited. The corrective candidate restores that proposal and must pass F02 and
all applicable hosted gates on its own head. Local tests establish their named
contracts only; they do not establish production readiness or deployment.
