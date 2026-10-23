# Locally modified Rust MCP SDK

rmcp3.5.1, https://github.com/modelcontextprotocol/rust-sdk/, is Apache-2.0 according to its checksum-verified upstream package manifest. The actual upstream archive contains no LICENSE or NOTICE file. LICENSE-APACHE is locally supplied standard Apache2.0 text from the already verified reqwest0.13.5 LICENSE-APACHE, not a file claimed to have been retained from rmcp. Original source/manifests/notices remain unchanged apart from the declared diagnostic patch.

SlopSearX modifies only src/transport/auth.rs:23logging invocations retain their level/condition with constant messages instead of opaque credential/provider/URI/scope/error fields;6unused diagnostic error bindings are renamed. No OAuth protocol/state/wire/persistence behavior or SDK error API is changed. Version3.5.1 alone does not identify this fork. ../PROVENANCE.json records upstream archive checksum, every local file hash, full tree hash and retained complete ../patches/rmcp.patch identity.

This is an integration proposal, not repository activation or production qualification. Before accepting an update, verify the newly pinned upstream archive, inspect all auth diagnostics and public error boundaries, rebase the smallest patch, regenerate exact patch/tree/file provenance, preserve license/attribution, rerun actual private-loopback patched/unpatched SDK sentinel and ordinary tracing/log-only transparency controls plus every affected consumer regression, and obtain independent maintainer review. Release SBOMs include upstream and local identities. The gateway still classifies returned SDK errors privately and never formats arbitrary SDK credential/config/callback Debug values.

## Explicit native gateway OAuth redirect profile

Additive source-only successor to auth SHA5a158137f05fe766bbc79be29b7130d5041ba74bdb539c3200ad923b1fecf99b. SourceWithinOrigin is selected only by explicit manager opt-in before HTTP. Original Follow and Stop defaults and all23 sanitized diagnostic sites remain. Discovery, registration, authorization-code exchange and refresh use the same method-preserving same-origin/default HTTPS-upgrade five-hop adapter profile. Default SDK HTTP adapter refuses the new variant. No qualification or production activation is claimed.


## Native-only development-fixture packaging

Before first SlopSearX publication, omit the upstream test_with_python Rust
integration target and its unused Python client/server development package.
These are third-party development fixtures, not the existing SlopSearX backend
or rollback implementation. Remove only that target declaration from generated
Cargo.toml and Cargo.toml.orig; retain every SDK feature, dependency, source API,
license and production transport/auth byte. rmcp-native-dev-fixtures.patch
records the exact local omission against the preceding composed SDK tree.

The live FastMCP development smoke is explicitly retired from this vendor
package. Native T01 retains all frozen Python-wire observations and existing
tests. Its additive SDK builder test captures and bounds actual child stderr,
checks the complete one-page tool/resource catalogs through list_all helpers,
then awaits SDK shutdown and the stderr reader. It does not claim live Python
interoperability, multi-page list_all equivalence, or fresh qualification of the
separate OAuth SourceWithinOrigin/profile/privacy controls. Qualification of
this combined source remains pending. Existing backend retirement remains G08.
