# Native application-image promotion writer

This additive O02 (#919) transition starts at
`88314a23079333e185d96cdcdb973554a9500182`. It preserves the deployment oracle's
literal equality semantics and every historical F01/W03 fixture, inventory,
Python source identities, builder-v1 capsule and builder-v2 capsule. It does not retire
Python source, resolve overall O02 acceptance, publish an image, or deploy.

## Digest-slot custody

The current builder-v2 deployment-test source is exactly 13,732 bytes with
SHA-256 `e9d53620f211325296cea8f3723c11a013621a8eb7fc642e077f06bf235035b8`.
The new source is 13,810 bytes with normalized-digest SHA-256
`ecdf9ce063dc0086287e7d9bc22dfc85e5b4f1d523c6914b799191041d71c8dd`.
Its sole fixed edit requires the native workflow `--write` invocation and
rejects the Python invocation. The unique prior/current assertion spans and
their byte offset are recorded in `source-transition.json`. Only its 64
lowercase hexadecimal digest bytes at range `[3032,3096)` may subsequently vary. The entire source, after normalizing that slot to the original digest,
must retain the new exact size/hash, then the unique assertion edit is inverted
to reconstruct the prior exact size/hash. The assignment spelling, quotation marks,
imports, executable code and all other bytes remain authenticated. No Python
is interpreted by this proof.

`o02_image_custody.rs` defines this parameterized current-source successor.
`o02_health_retirement.rs` normalizes only the exact builder-v2 current endpoint
and inverts the single fixed assertion edit before its existing descriptor-bound
material check and unchanged historical inverse. Native extraction separately authenticates that inverse and its
original fixed literal, then returns the actual current slot. Synthetic
literal-only inputs remain usable by read-only parser controls; the writer
requires the real complete source template and authenticated runtime capsule.

The additive `t08a-bindings-prior.json` preserves baseline binding bytes.
`t08a-binding-audit.json` lists the finite changed/new current inputs, their
sizes and hashes. These current bindings are re-authored on the actual source;
they are not copied from closed #1085. Only #1085's descriptor-bound snapshot,
staging and identity-check ideas are adapted. Its broad reference scanner and
changed oracle design are not used.

## Filesystem transaction and recovery

The writer validates the current named Compose service image and Kubernetes
container image using duplicate-key-rejecting bounded YAML parsing. A text
replacement must change exactly that active semantic field and preserve all
other parsed values and original formatting. Comment/alias/escaped-scalar or
unrelated reference ambiguities fail closed. All operands must be regular
0644 files, opened with no-follow directory-relative descriptors and read caps.

Native operations hold an exclusive advisory lock on the selected root's
opened directory. This serializes cooperating native checks and writers;
external tools that ignore the lock can still observe individual replacements.
A four-file filesystem rename is not atomic as a set. Before creating output
stages or replacing files, the writer prepares a complete journal in a unique
private initialization directory, synchronizes it, then publishes it with
no-replace rename as a private 0700
`.ssx-image-promotion/` directory with a bounded 0600 journal containing exact
originals, validated outputs and the owned staging names. It synchronizes the
journal and directories. Reserved journal, active-directory, commit-marker and cleanup-receipt publication
also use no-replace renames, preserving foreign artifacts inserted after
preflight. Outputs are written and synchronized as drafts inside the private transaction
directory before an exclusive rename publishes them beside the destinations.
Recovery can retire a zero-byte draft at its journal-derived name after a
create-before-write interruption. A zero-byte file beside a destination still
requires inspection. Each staged output and each source identity is checked
again before the writer atomically exchanges the two names on Linux or macOS.
The former source remains at the stage name. The writer verifies both entries
after exchange and attempts to swap them back if either differs. An unknown
entry is preserved for recovery when the exchange cannot be safely reversed.

Before commit-marker publication, a failure makes the writer compensate in reverse order and verifies
byte-identical originals before retiring the recovery record. The same transaction owns the two active YAML examples in `spec.md`: exactly
two unambiguous references are required, their active image fields must agree
with deployment pins, and all bytes outside those two references are preserved.
A failed rollback
retains the journal and reports recovery required. Unknown external edits,
symlinks, wrong modes, foreign hard links and malformed journal data block
recovery without overwriting them. Repair the conflicting external state to one
of the journal's exact original/proposed operands, preserving the journal, then
retry explicit recovery. Never delete the only recovery record to clear an
error.

`image-pin-promote --recover [--root DIR]` is an explicit offline operation;
it returns 0 when clean/recovered and 2 on an error. An interrupted uncommitted
transaction restores originals. An unknown/empty reserved directory, malformed pending
record, unknown entry, symlink, hard link or wrong mode is preserved and requires
repair; the reserved name and same owner alone never authorize cleanup. An atomically published, fully written/fsynced commit marker means the complete
outputs were synchronized: recovery preserves those outputs and finishes owned
cleanup. Errors after marker publication are committed/uncertain, return an
explicit recovery diagnostic, and never enter rollback. Cleanup publishes an
atomic bounded receipt outside the transaction directory before retiring its
last record; that receipt binds the complete validated plan, final operand
bytes and directory device/inode. It safely resumes cleanup if the bound
directory is empty or already removed, while an unrelated empty directory still
has no cleanup authority. Commit cleanup failure emits a retention message while the promotion
remains successful; a later check still refuses the pending journal until
recovery completes. Before rollback, the live outputs must have their exact
displaced originals at the recorded stage names. Rollback exchanges each pair
in reverse order and verifies both entries. When a live original is present,
recovery retires a published forward stage only when its contents match the
complete expected output. A private draft may contain an incomplete prefix. After commitment, recovery retires only displaced stages
that match the complete original. An empty or unrelated file at a recorded
staging name remains in place and recovery reports the conflict.

`--check` remains comparison-only: a pending transaction causes an error with
no source/journal mutation. `--write` recovers under its lease before preparing
its next transaction. Clean same-digest writes are idempotent. Unpublished initialization directories cannot have changed target files. A
complete authenticated initialization can be cleaned; partial/unknown inert
initializations remain untouched with diagnostics. Reaching the bounded
initialization scan limit emits a diagnostic. Recovery of the active journal
uses its exact names and can complete independently of that scan. A root stream descriptor
uses CLOEXEC so child processes cannot retain the advisory lease; controlled
negative and positive subprocess cases cover this boundary.

The existing
workflow restricts generated PR contents to the four operands and refuses pending
transaction artifacts before publication. It now installs/confirms the repository-pinned Rust toolchain and runs
native recover/check/write/recover in order. It accepts only check exits 0/1 and write
exits 0/10; every other exit fails. Its schedule and publication integration are
retained. The workflow gives the repository token contents and pull-request
write permissions for its existing publication step. The action authors and
signs off the generated commit as the Actions bot. GitHub may require a
maintainer to approve checks on a bot-created PR. This change does not dispatch
or otherwise run that workflow.

## Validation boundaries

Offline controls cover successful writes/idempotence, exact custody projection,
active-field ambiguity and invalid target/source/capsule refusal, no-follow and
mode controls, foreign-source mutation, staging failures, compensation after
0/1/2/3/4 completed renames, rollback failure retention and explicit retry,
interrupted transactions, partial recovery stages, committed cleanup, and
comparison-only checks preserving source/journal bytes. Static workflow/catalog
comparison and fake `cargo` shell invocations exercise success/drift/updated,
error, unexpected-exit and interrupted-exit handling without registry access or
workflow execution. The registered run root holds commands, exits, raw outputs,
toolchain failures, target sizes and storage measurements.

Portal impact review: the writer changes local deployment data and release
helper wiring. It does not change browser request/result contracts, policy,
cache, runtime configuration, image build contents or the deployed digest in
this PR. No portal behavior acceptance is inferred from offline writer tests.

## Catalog custody composition

The promotion workflow source is mirrored exactly in the F03/O06 gate catalog.
The current Python dependency-audit successor reads that catalog before the earlier upstream-contracts and T06 inverses.
`catalog-successor.json` and `o02_image_gate_successor.rs` define one finite
replacement of only the `image-promotion.yml:promote` row, preserving every other
byte and reconstructing the exact current-main endpoint before the unchanged catalog inverses. The physical reader returns actual current bytes; supplied buffers
are compared with those descriptor-read bytes before projection. Controls use
`fs::read` to prove true current buffers pass and normalized/stale buffers fail.
Neither the prior security capsule nor its historical Git identities change.
