# O02 native container healthcheck increment

Read the [rewrite case study](../CASE_STUDY.md) and [O02 issue #919](https://github.com/magnus919/SlopSearX/issues/919).

The production image runs `/usr/local/bin/ssx-healthcheck` instead of `docker/healthcheck.py`. The std-only `ssx-healthcheck` package sends GET `/health` directly to IPv4 loopback port 8080, accepts only HTTP 200, reads bounded response headers and does not read, decode or validate the body. It emits no output and exits 0 on success or 1 on failure. The existing Docker health interval 30s, timeout 5s, start-period 5s and retries 3 remain unchanged.

Intentional operational hardening: this fixed local readiness probe never consults proxy settings and never follows redirects. One absolute four-second deadline bounds connection, request and header acquisition; slow trickles cannot reset it. The old urllib helper inherited proxy/redirect behavior and had only Docker's outer timeout. The native response contract is bounded to 16 KiB of CRLF-terminated headers and an HTTP/1.0 or HTTP/1.1 status line. Oversized headers, unsupported protocol spellings and missing CRLF termination fail closed; these explicitly bounded parsing rules replace urllib tolerance. No configurable URL, upstream acquisition, JSON-health requirement or backend import is added.

The digest-pinned Rust 1.99.1 Bookworm builder compiles/tests the native helper for the image's architecture; the workspace still selects the Rust 1.99.0 toolchain from `rust-toolchain.toml`. The existing Python runtime base, application source and uvicorn command remain unchanged. Before image construction, CI runs `cargo xtask ci-helper image-pin-verify-base` with the repository-pinned toolchain to strictly parse and check both builder and runtime tags/digests against the primary registry. The interim Python verifier remains tracked for historical source custody.

Run `cargo test --locked -p ssx-healthcheck` and `cargo clippy --locked -p ssx-healthcheck --all-targets -- -D warnings`. The unit owner exercises actual loopback requests, status-only success with absent/invalid bodies, non-200/redirect/malformed/oversized headers, refused connections and a slow-header deadline. Existing amd64/ARM64 Docker CI builds the actual image and executes its shipped helper against the running server with hostile proxy settings, then verifies silent exit 1 in an otherwise empty container. Local Docker/Podman are unavailable on the authoring Mac; hosted image acceptance is required. Source authorship or a native host unit pass cannot substitute for that witness.

This is a partial delivery, not complete O02 or Python-free server/release acceptance. The upstream-probe workflow invocation is transitioned to native Rust in the [O02 upstream-contracts receipt](UPSTREAM_CONTRACTS_TRANSITION.json), with offline replay by default and live dispatch explicitly gated. The digest promotion workflow still has an active Python writer; portal-smoke and mirror configuration have native invocations, while their historical Python files remain frozen for G08 source-retirement review. Other operational Python and full O02 acceptance remain in scope. O04 owns final native image/package assembly; G08 owns backend Python retirement after its hard predecessors. No production activation is included.

## Python-free image verification

The T02 image also runs the shipped `ssx-healthcheck` binary against the
native HTTP fixture on loopback port 8080. The binary must exit silently with
1 before startup, 0 while the service is ready, and 1 after owned shutdown.
Each probe runs with hostile proxy variables and an otherwise empty environment.
The existing image gate supplies the Python-free, network-none, read-only and
nonroot witness. This exercise covers the qualification fixture and healthcheck
binary. The server release packaging remains under O04.
