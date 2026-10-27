# T07 client platform acceptance candidate

This continuation adds a standalone client-only workflow for ADR005's five
native targets. It preserves the existing Rust foundation workflow and does not
change native server scope. Source preparation is not platform acceptance: each
row remains pending until its actual hosted job compiles, installs and executes.
O04 signature, release packaging and artifact promotion remain separate.

| Native target | Explicit hosted runner | Required execution |
| --- | --- | --- |
| x86_64-unknown-linux-gnu | ubuntu-24.04 | installed runtime, TLS/bundle/proxy/redirect and platform store |
| aarch64-unknown-linux-gnu | ubuntu-24.04-arm | same, running natively on ARM |
| x86_64-apple-darwin | macos-15-intel | same, with an owned temporary keychain |
| aarch64-apple-darwin | macos-15 | same, running natively on Apple Silicon |
| x86_64-pc-windows-msvc | windows-2025 | same plus DACL/reparse/digest and owned Job witnesses |

Runner labels and native architectures were verified on 2026-10-27 against
[GitHub's primary runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
Before enabling test features, the lane copies the three exact default Cargo
compiler-artifact executables and records their SHA256/length/role/version/target
in an explicit manifest. Installation/TLS tests require those preserved inputs;
existing runtime tests use them whenever that manifest is supplied. Fixture DNS
and the owner probe remain separate explicitly named component controls. Later
feature builds cannot silently replace the preserved shipping images. Named
compiled inventories require each relevant OS case exactly once, including the
exact ignored hosted trust case, before execution. Inventory is not a runtime pass.
The lane asserts `rustc -vV`'s actual host, uses the pinned toolchain, keeps target
and test roots in the disposable job directory, retains raw command receipts,
and removes its reproducible outputs. It never uses a cross-build as runtime
proof. No Python executable or engine credential is needed by the installed
client. Build prerequisites and the compiler are still required to build it.

## Product changes and deliberate boundaries

`MCP_REMOTE_CA_BUNDLE` is an optional operator path to a PEM certificate bundle.
The existing `IoOwner` reads at most 4 MiB plus one overflow byte during a
30-second startup admission window. The complete maintained PEM parser result
must be nonempty. Missing, empty, malformed, oversized or nonregular inputs fail
startup; certificate merge or client construction failure also fails startup.
The original I/O worker is drained on every startup error. OS filesystem work is
not forcibly interrupted: cancellation revokes admission and preserves the worker
until its actual completion. No path, certificate body, URL or credential is
formatted into public failure diagnostics.

All supplied certificates are passed to `tls_certs_merge`, preserving platform
roots and hostname verification. This is the explicit ADR005 additive-trust
contract, rather than a claim of Python environment-variable parity. The private
fixture verifies that a valid second certificate in a bundle is honored and that
an invalid later certificate cannot silently narrow the collection. The hosted
store case additionally proves that an unrelated additive bundle does not replace
an independently trusted platform root.

OAuth explicit token files and supplied `XDG_CONFIG_HOME` paths do not require an
unrelated home variable. Empty XDG remains the source's relative config base.
On Windows, a nonempty `USERPROFILE` takes precedence over
`HOMEDRIVE` plus `HOMEPATH`; Unix `HOME` does not override a Windows profile.
Empty/missing Windows profile components fail unless that native fallback is
complete. This refusal avoids an accidental current-directory token default.
Unix HOME selection otherwise remains unchanged. Python's eager evaluation of
`Path.home()` for an already supplied XDG base is intentionally unnecessary here.

Windows token files use protected creation and a confidentiality DACL policy for
the current user, SYSTEM and Administrators. Insecure existing token ACLs fail
closed; the client does not repair or adopt them. A held validated parent spans
same-parent atomic replacement. Read handles allow delete sharing for complete
replacement while denying concurrent writers. Reparse points, extra hard links,
untrusted-writer parents and identity changes fail closed. A failed create-new
collision never grants cleanup ownership of the preexisting file. This is local
file custody, not encryption or protection against a malicious trusted principal.

Installed package verification continues to allow public readers while refusing
untrusted writers. Its three fixed Windows roles and exact record/hash/identity
checks remain authoritative. Package record schema and DNS protocol do not change.
The new probe and wrong-Job controls exist only behind a nondefault required
feature; no environment flag or command mode enables them in a production binary.
A fixture-only owned read guard uses the exact production admission/read helper
to interleave a held token read with real atomic replacement. It requires a
complete old document or explicit identity denial, a complete new document at
the current path, confidential ACLs, and kernel parent-rename denial until the
original guard is released. No sleep or global mutation hook supplies this barrier.
The abrupt-parent case captures an original read-only process handle and verifies
creation time before killing the original owning parent Child. It holds no Job
handle, so it cannot accidentally defeat the last-handle kill-on-close witness.
The wrong-Job receipt identifies `CreateProcessW` rejection specifically, and
keeps a valid entered-child control plus never-resume cleanup on unexpected
acceptance. These are native Windows witnesses, not PID-disappearance assertions.

## Exact candidate commands

Production graph/build, on each native runner:

```sh
cargo tree --locked -p ssx-gateway --no-default-features --edges normal
cargo build --locked -p ssx-gateway --no-default-features --bin ssx-gateway --bin ssx-gateway-dns --bin ssx-gateway-input --message-format=json
```

The workflow refuses server application/infra/HTTP/registry/config/worker packages
in that default normal-dependency graph. No `--workspace`, `--all-features`,
server test fixture or xtask bootstrap enters this client lane. The new dev edges
use the already locked rustls/tokio-rustls/rcgen and Windows owner versions;
integration must resolve actual Cargo.lock membership without version drift.

```sh
cargo test --locked -p ssx-gateway --no-default-features --features source-endpoint-test-fixtures,windows-owner-test-fixtures --test t07_platform_install --test t07_platform_tls --test t07_runtime --test t07_windows_owner --test t07_windows_security -- --nocapture
```

The platform store test is deliberately ignored by ordinary Cargo/local runs.
Only the dedicated ephemeral job sets `SLOPSEARX_PLATFORM_TRUST_FIXTURE=1`, requires
`GITHUB_ACTIONS=true`, and selects its exact test with `--exact --ignored`.
It first rejects the private CA, installs only the unique fixture certificate,
proves default platform verification and independent additive trust, rejects a
wrong hostname, removes the exact CA, and proves rejection again. Windows uses
CurrentUser Root; macOS uses an owned temporary keychain and restores the prior
search list; Linux uses an exact fingerprint-named temporary CA file. RAII cleanup,
a shell EXIT trap, and an always-running workflow finalizer retain cleanup
responsibility. None of these commands is authorized on a personal trust store.

The normal private TLS cases use only loopback endpoints and explicit additive
trust. They prove no bearer header enters CONNECT, no proxy authorization reaches
the TLS origin, and cross-origin MCP redirects are refused before a destination
HTTP request. The redirect witness first requires a successful ordinary TLS
initialization with the same installed client and CA, then independently requires
failed initialization, the configured bearer at the redirecting origin and zero
destination requests. This follows retained MCP `next_request_within_origin`
(`source-inputs/mcp__shared___httpx_utils.py.txt:86–124`); it does not relax the
product redirect policy. Existing runtime cases remain the authority for OAuth/session/EOF behavior;
Unix signal controls remain platform-gated. Windows security cases are compiled
and executed only on Windows with the nondefault owner fixture feature. A zero
case count on other platforms is not Windows evidence.

## Integration and remaining evidence

This branch continues the accepted c213 T07 source. The T07 owner integrates the
reviewed patch, refreshes actual lock membership and all affected current source
bindings, and qualifies the resulting exact revision. Historical/frozen capsules
must not be repinned to conceal these additions. The current 215-case qualification
predates this patch and cannot qualify its changed product or tests.

Required remaining evidence includes exact-head native compile/lint/runtime
results for the five rows, current/frozen source proof reconciliation, and hosted
CI before issue acceptance. Windows 10 minimum-version installation and O04 signed
artifact release acceptance remain separate. Native Windows server support is
outside ADR005's client scope. No provider, production gateway, token or signing
secret is used by these tests.

Portal impact: these changes affect the native remote client and its local
packaging/security only. Search/backend wire contracts, shared policy and the web
portal remain unchanged; browser contract changes and a new portal flow are not
introduced.
