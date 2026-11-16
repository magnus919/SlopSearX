# T08A bounded physical source reads

This repair follows the static T08 independent semantic review recorded on
issue [#903](https://github.com/magnus919/SlopSearX/issues/903#issuecomment-6881283260)
on 2026-11-14. The reviewer found that T08A and the A06/T02 source composition
readers checked a path and then used a separate unbounded `fs::read`. The review
did not reproduce a race; hashes rejected unexpected final bytes but did not
bound the read or prove that metadata described the opened object.

The shared T08A custody reader now opens Unix path components relative to held
directory descriptors with `O_NOFOLLOW`, opens the leaf with `O_NOFOLLOW` and
`O_NONBLOCK`, checks regular-file type, size, mode, device and inode on the open
handle, reads at most the configured bound plus one byte, then rechecks the
descriptor and named leaf. This rejects symlinked parent components, symlinked
leaves, oversize inputs, in-place size or mode changes, and leaf replacement
observed during a read. Authentication hashes and existing source projections
remain the authority for accepted bytes.

Unix opens use the already-pinned `nix` safe `openat` API with `O_NOFOLLOW`,
`O_DIRECTORY`, and `O_NONBLOCK`; `nix` supplies target-specific UAPI flag values,
including Linux aarch64. A compile-time assertion keeps the ARM64 directory and
nofollow values pinned to the reviewed ABI. Other non-Windows, non-supported Unix
targets fail closed at read time.

Windows canonicalizes the root, traverses from a complete drive or UNC root
anchor, opens each directory with `OPEN_REPARSE_POINT` and read-only sharing,
rejects reparse points, and retains those handles through the leaf read. The
leaf handle uses the same sharing rule, so the open handle chain denies new write and delete sharing while its handles remain open.
Stable pre/post metadata checks and the bounded read use that same leaf handle;
no unstable file-index API or path-only recheck is needed. Windows controls cover
complete drive/UNC anchors, root and parent rename denial, leaf write/delete/
rename/replacement denial while open, and release after return. Reparse rejection
is exercised when the test host permits symlink creation. Windows behavior is not
runtime-validated in this macOS run; the guarantee is limited to documented
Windows sharing and reparse semantics represented by Rust's `OpenOptionsExt` flags.

The T08A compatibility overlay uses only the configured, existing
`SLOPSEARX_TEST_TEMP_ROOT`, and requires it to resolve to the same nonsymlink
directory as `TMPDIR`. Its unique child directory is created exclusively and
removed after the controls. The active run ledger records the registered short
scratch root; the code does not fall back to the operating system temp root.

Focused controls use an owned temporary fixture and exercise valid exact-bound
bytes, an oversize file, leaf and parent symlinks, leaf and parent replacement
after open, in-place content mutation, and mode mutation. The replacement and mutation
controls use a deterministic hook between descriptor validation and reading;
they are adversarial unit controls, not evidence that a race was observed in a
workspace. F09/T02's existing source-composition control path is also retained and now
uses the same opened-handle reader for its physical operands. Its W07/T02 joint
reader and paired MCP manifest reader use the bounded API too, so composition
cannot reopen either input with unbounded `fs::read` after admission.

The direct Unix `nix` dependency added for safe descriptor-relative opens is
recorded in `issue903-config-edge-successor-v1.json`, including the `ssx-infra`
test-only dependency needed when its integration tests include the shared reader.
That finite successor contains exact prior/current byte lengths, SHA-256
values, Unix modes, Git blobs, and ordered changed spans for `Cargo.lock`
and the `ssx-cli-check`, `ssx-mcp`, and `ssx-infra` manifests, the native-search and
W07/T02 joint readers, and the T03 admission
source. T08 custody reverses the new lock edge before entering the unchanged O02
and historical custody proofs. T03 verifies the physical native-search and joint
bytes at its owning admission seams, reverses the outer source layer, and then
invokes the unchanged historical inverse. The capsule also records the updated
`shared-successor.rs` reader from its exact accepted-main predecessor. T08A's
physical main-join admission projects that bounded reader to the existing
accepted source before either unchanged CLI or W08 inverse runs. The A06/A13
control path physically bounds the `ssx-infra` manifest, projects it to the
sealed 1,254-byte endpoint, and rejects changed bytes or a wrong opened mode.
The capsule checks full reverse and forward reconstruction and rejects wrong
paths, modes, bytes, and prior Git identities. The T08A binding inventory has
been refreshed for these exact source and manifest changes after PR #1136 merged.

The changed T06 dependency-audit source reader has its own finite outer capsule
inside the accepted dependency-audit current-main owner. It authenticates the
bounded physical reader bytes and mode, reconstructs the unchanged 31,817-byte
accepted endpoint (`e66d9152…`, blob `40f336a0…`), then runs the historical
inverse. Pure DNS and dependency-audit frames require caller mode `0644`, then
pass canonical `0644` through byte projections and historical inverses. Windows
mode `0` is accepted only at physical admission alongside the stage-zero index;
wrapper controls reject mode `0` and other wrong modes in pure frames. The
historical dependency-audit and DNS capsules keep their original endpoints.

Changing the Rust reader source changes exact T08A source bindings. PR #1136
has merged; the current binding inventory is refreshed for these source and
manifest changes. The full T08A gate still requires a later authorized scoped
validation run.
