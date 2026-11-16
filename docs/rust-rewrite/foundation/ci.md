# Rust foundation build and smoke

F03 adds an additive Rust workflow. The existing Python checks and service
remain enabled; this workflow validates the new workspace while it is still a
foundation skeleton. Its green result does not establish search, MCP, workflow,
portal, adapter or production readiness.

## Toolchain and support boundary

The repository pins stable Rust 1.99.0 in `rust-toolchain.toml`. Cargo must use
the committed `Cargo.lock`; CI uses `--locked` for builds, tests, Clippy, lane
evidence and dependency policy. Linux x86_64 and aarch64 hosted runners execute
the foundation checks on their native architecture. This is the F03 foundation
matrix, not the shipping artifact qualification promised by O04. Native client
release support for macOS and Windows also remains pending O04 evidence.

`cargo-deny` is installed at 0.20.2. `deny.toml` denies known vulnerabilities,
yanked releases, unmaintained direct workspace dependencies, unapproved
licenses, registry wildcard requirements, and unknown registry or Git sources.
Local path dependencies are allowed because the foundation crates are
`publish = false` members of one workspace; registry wildcard requirements
remain denied. Duplicate versions are warnings while the early dependency
graph is being stabilized. Any new license or source exception must be explicit
in `deny.toml` and reviewed with the dependency change.

## Developer commands

From the repository root, run:

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace
cargo xtask check F03 --output target/rust-evidence/F03.json
sh scripts/rust-foundation-smoke.sh
```

The lane command writes machine-readable, source-bound evidence and exits
nonzero when the F03 case set is incomplete or a required case fails. Workspace
tests are run separately so `xtask`'s own tests are included. The smoke defaults
to `target/debug/ssx-server`; set `SSX_SERVER_BIN` when testing a target-specific
build. It starts that compiled server on loopback with an ephemeral port,
requires its startup line to identify `mode: foundation`, and requires
`GET /healthz` to return HTTP 200 with the exact foundation readiness body.
Unknown HTTP paths remain explicitly unimplemented in this scope.

The server receives a PATH containing only Python-command tripwires. Calling
`python` or `python3` records a marker and fails the smoke. The same check scans
the locked dependency list for common CPython bridge crates and requires
platform inspection tools to check the executable's dynamic libraries and
imported symbols (`readelf` and `nm` on Linux; `otool` and `nm` on macOS). This
evidence covers the F03 server artifact and its normal dependency graph; it
does not claim that the full repository or future feature set has no Python
dependency. The script itself uses ordinary POSIX shell and standard
command-line utilities; it does not invoke Python.

## CI ownership

`.github/workflows/rust.yml` owns the additive Rust matrix and supply-chain
check. It does not edit or disable Python gates. The two Linux jobs build and
test on their matching architecture, run the F03 evidence command, and invoke
the real-server smoke. The separate supply-chain job checks the complete
workspace against the pinned cargo-deny version. Future Rust issues should
extend this workflow with their explicit evidence gates; removing Python gates
belongs to the separately planned cutover work after replacement suites are
accepted.
