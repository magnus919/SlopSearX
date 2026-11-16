# Generated process-pool fixtures

The private Unix cleanup fixtures execute their generated script as data passed to the stable /bin/sh executable. This avoids asking execve to execute a writable temporary inode. The private cfg(test, unix) opt-in applies only to six fixture pools; production and default pools continue to launch their configured native helper directly.

The success/protocol fixture deliberately holds a writable descriptor without writing through it. It retains exact success and truncated-protocol results, payload/accounting release, child PID disappearance and joined shutdown. A mismatched result now drains and removes the fixture before its primary assertion.

The prior ARM CI receipt observed Spawn("Text file busy (os error 26)"); its descriptor-level cause is unknown. Darwin qualification passed the selected control, all six fixtures, the unchanged 207-test library inventory, workspace/all-target build, formatting and strict Clippy. The first full-library attempt lacked the required native HTML helper: its 200 passes/seven failures are preserved. Building the required executable before the unchanged suite produced 207 passes; no test or timer changed. Evidence is in evidence/process-fixtures/manifest.json. Linux qualification and current-head review remain required before merge.

Context: [Rust rewrite case study](../CASE_STUDY.md).

Receipts are Base64-encoded gzip bytes in text files. Decode Base64 and then gzip to recover the exact raw streams; the manifest binds encoded, compressed and raw hashes. This satisfies native source hygiene without binary-asset exemptions. The initial hosted O06 failure on unlisted gzip files is retained in PR history; fixture source is unchanged.
