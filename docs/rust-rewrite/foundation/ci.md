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
# Capture the exact native producer and its image boundary before smoke.
target=$(rustc --version --verbose | sed -n 's/^host: //p')
target_root=${CARGO_TARGET_DIR:-"$PWD/target"}
evidence="$target_root/rust-evidence/foundation-smoke"
mkdir -p "$evidence"
cargo build --locked -p xtask --example foundation_python_boundary --target "$target"
cargo test --locked -p xtask --example foundation_python_boundary --target "$target" -- --test-threads=1
producer_exit=0
cargo build --locked --workspace --target "$target" --message-format=json-render-diagnostics > "$evidence/workspace.jsonl" || producer_exit=$?
printf '%s\n' "$producer_exit" > "$evidence/workspace.exit"
[ "$producer_exit" -eq 0 ]
verifier="$target_root/$target/debug/examples/foundation_python_boundary"
server="$target_root/$target/debug/ssx-server"
"$verifier" "$PWD" "$target_root" "$target" "$evidence/workspace.jsonl" "$evidence/workspace.exit" "$server" --record > "$evidence/boundary.json"
SSX_SERVER_BIN="$server" SSX_FOUNDATION_TARGET="$target" \
SSX_FOUNDATION_TARGET_ROOT="$target_root" SSX_FOUNDATION_BOUNDARY_BIN="$verifier" \
SSX_FOUNDATION_BUILD_JSON="$evidence/workspace.jsonl" \
SSX_FOUNDATION_BUILD_EXIT="$evidence/workspace.exit" \
SSX_FOUNDATION_BUILD_BOUNDARY="$evidence/boundary.json" sh scripts/rust-foundation-smoke.sh
```

The lane command writes machine-readable, source-bound evidence and exits
nonzero when the F03 case set is incomplete or a required case fails. Workspace
tests are run separately so `xtask`'s own tests are included. The smoke requires
the captured actual native workspace build stream, actual producer exit and
post-build image receipt shown above; absent evidence fails closed. Set
`SSX_SERVER_BIN` to that exact target-specific image. It starts that compiled
server on loopback with an ephemeral port,
requires its startup line to identify `mode: foundation`, and requires
`GET /healthz` to return HTTP 200 with the exact foundation readiness body.
Unknown HTTP paths remain explicitly unimplemented in this scope.

The server receives a PATH containing only Python-command tripwires. Calling
`python` or `python3` records a marker and fails the smoke. The native Rust
verifier audits every emitted package identity in the completed
workspace compiler-artifact superset, including cached, custom-build and
proc-macro artifacts. It rejects the same exact CPython bridge package names;
inactive optional lock entries alone are not compiled runtime dependencies.
The unique normal `ssx-server` artifact must match the source manifest, empty
server feature list, native target path and captured image SHA/length, which
are checked again before launch. This conservative superset can reject a
compiled bridge belonging to another workspace member; it never assumes that
a package-only Cargo graph re-resolution represents the built server.
The unchanged platform inspection checks the executable's dynamic libraries
and imported symbols (`readelf` and `nm` on Linux; `otool` and `nm` on macOS). This
evidence covers the F03 server artifact and its normal dependency graph; it
does not claim that the full repository or future feature set has no Python
dependency. The script uses POSIX shell and standard command-line tools plus
the source-built native Rust verifier. Neither invokes Python. Capture success establishes the
local artifact boundary only; hosted smoke execution and readiness remain required.

## CI ownership

`.github/workflows/rust.yml` owns the additive Rust matrix and supply-chain
check. It does not edit or disable Python gates. The two Linux jobs build and
test on their matching architecture, run the F03 evidence command, and invoke
the real-server smoke. The separate supply-chain job checks the complete
workspace against the pinned cargo-deny version. Future Rust issues should
extend this workflow with their explicit evidence gates; removing Python gates
belongs to the separately planned cutover work after replacement suites are
accepted.
