# ADR 010: Support Apple Silicon on macOS

- **Status:** Accepted by the maintainer; implementation pending reviewed merge
- **Scope:** Native macOS client distribution and migration qualification
- **Decision authority:** Maintainer direction on 2026-11-27 to remove Intel Mac support and take trust-store changes off the release critical path
- **Date:** 2026-11-27
- **Amends:** [ADR 005](005-rust-protocol-tls-and-platforms.md), macOS client targets and gateway TLS qualification

## Context

ADR 005 proposed both Intel and Apple Silicon macOS client targets. Supporting
both requires separate builds and install evidence. The original gateway
qualification also required adding and removing a test CA in the macOS user
trust store. Typical deployments use certificates issued by a public CA. Apple's [macOS release and
security history](https://support.apple.com/en-us/100100) lists Apple Silicon
for macOS 27 and records updates to older macOS versions. This project sets
its own support boundary.

## Decision

Qualify and distribute native macOS clients for Apple Silicon
(`aarch64-apple-darwin`). Remove Intel macOS from the supported platform
matrix, required CI jobs, release package and install qualification, and
gateway qualification. Preserve historical Intel evidence.
Keep the published x86-64 and ARM64 Linux targets and the existing
best-effort Windows policy.

Qualify the installed Apple Silicon gateway on a disposable GitHub-hosted
arm64 macOS runner. Require an end-to-end MCP session over publicly trusted
HTTPS using the gateway's default certificate verification. Private CA bundle
scenarios and platform trust-store changes are outside release qualification.
Track the optional platform trust investigation and its historical evidence in
[#1073](https://github.com/magnus919/SlopSearX/issues/1073).

## Consequences and confirmation

The project will stop producing or qualifying new Intel macOS client
artifacts. The decision applies to future releases and qualification.

The [CLI workflow](../../.github/workflows/rust-cli.yml) verifies supported
Linux and macOS targets. [T07](https://github.com/magnus919/SlopSearX/issues/902)
owns the installed gateway HTTPS checks. [O04](https://github.com/magnus919/SlopSearX/issues/921)
owns the signed release matrix and clean installation evidence.
