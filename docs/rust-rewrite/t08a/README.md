# T08A standalone native CLI

This component authors `ssx` against the frozen external HTTP contract. It does not
qualify the native backend, close T08/T02, switch deployment defaults or certify a
finished rewrite. Historical Python captures remain immutable reference evidence.

All four commands use external HTTP only. The binary imports no domain, service,
server, infrastructure, adapter, cache or page crate. `ssx-json` supplies only native
finite float formatting; its default value parser cannot represent the
source's NaN/Infinity, lone escaped surrogates or unlimited-size values. A CLI-local attributed copy retains exact float normalization without the shared
4 MiB constructor cap. A separate
CLI JSON AST therefore preserves object insertion/duplicate-key order, exact
integer text, Unicode codepoints and source JSON indentation/escaping. No shared
codec is weakened. The admitted decimal digit table comes from the immutable
3.13.6/Unicode15.1 source runtime, not a Rust host Unicode approximation.

Native diagnostics intentionally replace Python traceback/exception/path bytes
with sanitized `error: ssx: ...` messages; usage help is native Clap help. Usage
failure remains 2, runtime/shape/transport failure remains 1, failure stdout stays
empty, and engines retains ordered health-before-config behavior. Whitespace-only
query keeps its exact original error. JSON parse/print/clone/drop use explicit stacks: the source accepts nesting through
at least 1000 and values/tokens above 4 MiB. No shared JSON depth/body ceiling is
inherited. The source's 4300-digit
integer admission is retained; values never narrow through i128/u64/f64.
Piped native stdout/stderr are UTF-8 with literal LF on every target. Frozen Unix
bytes remain exact; Windows CPython CRLF/locale output is an explicit portability
difference, not universal byte parity inferred from Mac captures.
Present undecodable URL/proxy routing settings fail closed rather than falling
back to localhost or direct routing. Unix byte cases use exact0xff; Windows
executes the equivalent unpaired-UTF16 configuration cases, without skipping.

## Trust and proxy contract

Trust-directory lookup and X509 chain/hostname validation use Rust openssl
0.10.81 with statically vendored OpenSSL3.6.3; Rustls retains normal TLS12/TLS13 handshake-signature verification.
The context matches observed Python 3.13 flags TRUSTED_FIRST, PARTIAL_CHAIN,
X509_STRICT, authentication security level 2, SSL-server purpose, NO_PARTIAL_WILDCARDS and CN fallback. OpenSSL checks
validity, critical extensions and chain constraints. Frozen CPython3.13.6 uses
OpenSSL3.0.16 (the immutable observations record that exact runtime). Its default
paths register a raw store URI in addition to hash_dir: matching `hash.1` works
without `hash.0`, while a path list containing only `hash.1` fails. OpenSSL3.6.3
no longer adds that URI implicitly. The CLI-private `ssx-cli-openssl-store` leaf
registers it through OpenSSL's vetted store loader without splitting it, scanning
PEM files or changing validation. Its only unsafe allowance is the documented
borrowed-store/CString FFI module; workspace policy remains unchanged. Native
ownership/lazy-loading, interior-NUL and native URI-error controls accompany the
real external-child .1/wrong-hash/empty/path-list corpus. Unix CA paths pass exact OS bytes through the tiny safe wrapper with explicit interior-NUL rejection. Linux owns a real non-UTF8-path control; macOS filesystems rejected that setup before TLS, so macOS uses a Unicode owned path with the failure preserved. Windows rejects undecodable UTF16 paths rather than using a lossy trust path.
There is no accept-any-certificate mode. SSL_CERT_FILE and SSL_CERT_DIR replace
only their respective defaults; empty/absent/unreadable paths cannot silently
restore broader roots. Missing default files may coexist with a working DIR,
matching observed source behavior. The exact synthetic-root controls include
correct/wrong FILE, DIR-only, FILE-only, missing/empty FILE, wrong hash, hash.1,
path-list, both empty, expired, critical-extension and hostname failures.

Default trust discovery is a proposed explicit portability difference: an
independent binary uses the host OS OpenSSL bundle/directory locations listed in
`tls.rs`, never a Python installation's private bundle or a baked Cargo build path.
On Windows, the existing vetted rustls-platform-verifier 0.7.1 enforces native chain/SSL policy only if both FILE/DIR overrides are absent. Explicit overrides
never restore platform roots. This deliberately uses native Windows purpose, disallowed-certificate and intermediate
policy instead of importing CPython-enumerated CA/ROOT entries as unrestricted
OpenSSL anchors; hosted native Windows evidence remains
required. Maintained Rustls cipher/signature algorithms replace the OpenSSL
negotiation set, a disclosed native TLS compatibility disposition. No universal default-root parity is claimed. Proxy handling keeps
urllib lower-case precedence, CGI HTTP_PROXY suppression, no_proxy bypass and
HTTP/HTTPS proxies. When no environment proxy map exists, read-only native macOS SystemConfiguration and Windows Internet Settings discovery preserves the source OS fallback and bypass rules. A truthy unknown *_proxy entry suppresses OS fallback as in urllib; ALL_PROXY is not used as an HTTP(S) route. No OS settings are modified.
URL userinfo and the observed invalid Unicode URL forms fail before any HTTP
request; redirects cannot introduce Basic credentials. Proxy credentials remain
a separately tested source-supported path. Exact redirect limits use four repeats
per target and ten distinct redirect targets, rather than a generic hop limit.

Category sorting rejects a direct or nested unordered NaN comparison with empty
stdout. This is the approved malformed-identifier/order disposition, not Timsort
parity. Single NaN, JSON nonfinite values, Infinity and ordinary typed string
sorting retain their separately observed contracts.

## Build and source installation

Supported source commands use the repository-pinned Rust 1.99 toolchain and require
C compiler, Perl and make (MSVC/nmake on Windows) to build static OpenSSL; no dynamic libssl or Python is
required by the delivered binary. Use the host's registered persistent build and
TMP roots; do not create artifacts in the checkout. Examples (set paths first):

```sh
export CARGO_TARGET_DIR="$SSX_BUILD_ROOT/target"
export TMPDIR="$SSX_BUILD_ROOT/TMP"
export OPENSSL_NO_ASM=1 OPENSSL_NO_VENDOR=0 OPENSSL_STATIC=1
cargo build --locked -p ssx-cli --bin ssx
cargo test --locked -p ssx-cli --test external
cargo xtask check T08A
# Windows (backend-free runner of the identical gate module):
cargo run --locked -p ssx-cli-check -- check T08A
cargo install --debug --locked --path crates/ssx-cli --root "$SSX_INSTALL_ROOT"
"$SSX_INSTALL_ROOT/bin/ssx" --help
```

The backend-free developer checker reuses the exact T08A module and inventory.
Its only execution dependencies are Serde JSON, SHA-256, Tokio, process-wrap and Unix nix for bounded process ownership; it links no production CLI, OpenSSL or backend crate. The production CLI never links or invokes it. Windows uses this runner because the shared legacy xtask links
backend libxml, whose supported targets exclude Windows. This is no backend
build waiver; the full native CLI/TLS/install inventory still runs on Windows.

The target executable is `$CARGO_TARGET_DIR/debug/ssx` (`ssx.exe` on Windows);
installed executables are `$SSX_INSTALL_ROOT/bin/ssx`. Actual install/platform and
runtime inspection results must be attached to the final evidence; these commands
are proposed until executed. O04 still owns release archives/signing/promotion.

## Corpus, custody and limits

`cases.json` is a finite executable inventory derived from retained original and
additional source observations. Native external children run with PATH=/nonexistent,
no Python/backend variables, and owned loopback TCP/TLS fixtures. Each records
exact argv, stdout/stderr/status, ordered requests, image digest, waited child and
joined listener retirement. A negative control removes a case; another changes a real server reply and requires an output mismatch. The only malformed framing exceptions are the five enumerated in intentional-differences.json: wrong data CRLF, absent trailer blank, and chunk-size +2/0x2/2_0. The unchanged upstream HTTP1 suite plus ten named private controls use the upstream full UNIT harness because its Compat tests require http2; the shipping CLI still enables only client/http1. Every TLS handshake and decrypted IO operation keeps the same 15-second source deadline; partial encrypted TCP progress cannot extend it. The gate binds current source,
compiler executable artifacts, runtime case bytes and raw evidence. Historical
captures prove only source observations; native and hosted acceptance are separate.

Portal impact: wire-only client; no browser/backend/shared rendering or request
contract changes. No portal implementation change is needed. The shipping root
`ssx` remains the historical standalone stdlib Python wire client during migration;
it imports no backend and is not a mandatory native dependency. No new optional
Python client is introduced. T08 consumes these source/binary/corpus artifacts and
must separately qualify the accepted native server and final installation.

The checker also exposes `check T08A-TLS-ROLES [--output PATH]` for locked dependency-policy evidence only. This preserves the original backend Rustls sentinel and all three forbidden TLS package names, proves the exact CLI-only statically vendored OpenSSL identities, and uses a real transitive dependency probe to reject a backend that reaches that exception. It does not claim CLI runtime or backend acceptance. The two-path shared source capsule authenticates physical current main/lock bytes before reconstructing exact frozen historical frames; no prior pin is replaced.

A separate closed legacy-input frame admits only the new root workspace member, the two directly consumed proof helpers, and the A13 qualification-only command frame. A06 and A08 authenticate the actual current root manifest before their original fixed tuples; W07 authenticates both current helper files before its unchanged original inverse. Included-module consumers share a self-contained relative-path custody leaf, so MCP integration-test roots do not need an xtask module namespace. Historical manifests, captures, pins and test identities stay immutable.

Workspace build/lint/test owners that compile unit targets explicitly enable only the private Hyper full/source-framing qualification union. This includes the three partition provenance builds, workspace Clippy/tests, F03 and A13 standalone all-targets helper build. Their existing artifact.features and evidence_scope retain qualification roles; no output schema, aggregate identity checks or shipping manifest changes. T08A builds the standalone CLI again with no features after external-test compilation, checks exact CLI/private Cargo JSON feature floors, then launches that actual default image and separately installs default source.

Default-shipping provenance is distinct from the external-test build snapshot even when both use the same target path. After the actual default compiler transcript is positively validated, four isolated metadata mutations require named rejection of private qualification features, CLI diagnostic features, and each test-profile flag. They are metadata guard controls, not runtime/platform proof. The post-floor executable hash must stay stable through its actual external run; installation has its own independent digest.

Formatting uses the original supported `cargo fmt --all --check` everywhere. The private crate alone supplies the stable `.rustfmt.toml` `disable_all_formatting = true`; it is itself an explicitly hash-guarded private addition. Ordinary workspace packages and local path dependencies keep their previous formatting coverage. No upstream Rust, archive or license byte is changed by this policy. A pinned two-crate control proves ordinary mismatch rejection, private-byte preservation, and private mismatch rejection when the config is absent.
