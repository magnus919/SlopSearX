# A05 parser and decoder implementation sequence

Preserve the full A05 search capability and Python-free runtime objective. The observed parser and decoder differences are unresolved work; do not close A05 on the existing158 fixtures. The additional28 actual observations must become mandatory native replay evidence.

Proceed in independently reviewable phases when an implementation slot opens:

1. Establish a native HTML compatibility primitive using a pinned vendored libxml2 build and a reviewed Rust safe binding. Preserve lxml wrapper semantics, not just C tree output. First qualify version2.14.6 against the oracle, inspect upstream security fixes and choose a maintained/security-patched source with explicit regression evidence before shipping. No system-library fallback or build-time network downloads. Build Linux GNU x86_64/aarch64 and macOS from the same source. Existing workspace unsafe forbid stays; any unavoidable local unsafe boundary requires a separate concrete review, not a blanket lint exemption. Dependency internals are reviewed through the selected binding and SBOM.
2. Implement complete native HTTPX text-decoding behavior from the measured standard-codec inventory. Distinguish empty-body short circuit, unknown-label fallback, known rejected decoders, and applicable decoders. Generated alias/mapping/name tables must include provenance, licensing and immutable capture verification. Python is permitted only in temporary offline oracle generators inventoried for G08 retirement. No interpreter, Python C-API, subprocess Python, or remote Python service may remain at runtime.
3. Integrate with all A05 adapters and their actual transport lifecycle, preserving full source responses/request ordering/error classification and accepted exceptions. Validate bounded input/output/memory/CPU and cancellation, including parser resource-loader refusal. If audited incremental parsing cannot provide a hard stop, use a supervised native Rust helper with bounded admission/IPC and kill+reap on deadline; prove packaging and descendant shutdown. This is an implementation requirement, not a reason to weaken deadlines.

Before committing dependency or build policy changes, provide exact package/version/source/checksum/features/license diff, supported-target build evidence, resource-loading tests and lifecycle proposal. Parent will review the concrete result. Neither prototype compilation nor finite fixtures establish complete parser/codec parity. Do not introduce provider calls, deployment, or any new Python runtime path.

Evidence: [measured codec inventory](source-codec-policy.json), [second-review whole-source captures](corpus-review-2.json), and [original comparison contract](README.md). official libxml2 HTMLparser.h describes custom non-HTML5 tree construction and says HTML_PARSE_NONET has no effect: https://gnome.pages.gitlab.gnome.org/libxml2/html/HTMLparser_8h.html . Library version security/upgrades must be assessed from upstream https://raw.githubusercontent.com/GNOME/libxml2/master/NEWS .

## Architecture and acceptance ownership

This is a work plan, not an accepted dependency amendment. F02 requires an ADR
amendment and affected contract/intentional-difference updates for dependency or
policy changes before implementation fan-out. The implementation must prepare
that amendment with exact source/version/build/license evidence. Parent review
owns acceptance; neither A05 nor the milestone may close while these gaps remain.
The current ordinary and first-review corpora remain immutable. The new capture
pack must be integrated into the mandatory gate before any final qualification.

The old Python codec registry is a source observation, not a requirement to ship
Python. Generated mapping data and native decoding algorithms must carry their
licenses and provenance; their generator is temporary migration tooling. Final
release tests must execute without Python or a Python-backed service available.

## Upstream release verification, 2026-10-16

The [official 2.15 release index](https://download.gnome.org/sources/libxml2/2.15/)
now identifies **2.15.4** as its latest release. The previously inspected GitHub
`master/NEWS` stops at 2.15.0 and must not be used to infer current release status.
The [2.15.4 release notes](https://download.gnome.org/sources/libxml2/2.15/libxml2-2.15.4.news)
list additional bounds, overflow and memory-safety fixes. This establishes that
an unmodified 2.14.6 oracle-version pin is not a sufficient shipping decision;
it does not by itself establish exploitability of any configured parser path.

Evaluate 2.15.4 as the initial maintained vendor candidate, alongside the 2.14.6
oracle compatibility comparison. The [official checksum file](https://download.gnome.org/sources/libxml2/2.15/libxml2-2.15.4.sha256sum)
publishes `98087fd181d9070724f3fbc65c7377db03038eb92bd882374daff44940138821`
for `libxml2-2.15.4.tar.xz`. Verify downloaded bytes against this value before any
build; retain provenance and licenses in the eventual vendor manifest. No source
archive has been downloaded or dependency adopted by this note.

Version choice still requires the complete A05 corpus, wrapper semantics,
resource-loader refusal, bounded lifecycle and supported-target evidence above.
Any behavior delta must be fixed or explicitly reviewed with its actual source
observation; it must not be hidden by rewriting the oracle. The F02 dependency
policy amendment remains required before adopting the build into the workspace.
