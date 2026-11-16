# A05 candidate adoption packet — parent review

**Not ready for adoption.** This packet records exact completed evidence and remaining qualification. No workspace dependency/build policy was changed. Current branch01876639 is preserved; fresh origin/main544c8307. No commit/push.

| Evidence | Exact artifact/status |
| --- | --- |
| Maintained C candidate | Official2.15.4 archive verified SHA25698087fd181d9070724f3fbc65c7377db03038eb92bd882374daff44940138821 |
| Binding baseline | crates.io libxml0.3.22 matches registry SHA2561157d8bc7a14d07445541827e7a9c80fc5939718b43a973cf7e666d3ab5035fb |
| Final safe API + tests patch | libxml-safe-api-proposal.patch SHA2566873ac9c49ed893930a2ef14636ce2808dfd1db9bdb6e919f22d510f141f737c; supersedes d9/e6 draft patches |
| Isolated dependency graph | binding-proposed-lock-diff.json; runtime/build graph resolved but license/advisory qualification pending; no workspace lock adoption |
| Native macOS C+binding link | Completed; /private/tmp/a05-libxml2-candidate-build.log, CMakeCache.txt, compile_commands.json and /private/tmp/a05-safe-binding-candidate-run.log |
| Callback ABI + actual dispatch |2tests passed/0failed; /private/tmp/a05-safe-binding-loader-tests.log |
| Full mandatory replay |186required;175full comparisons pass/11expected failures; /private/tmp/a05-186-required-replay.log |
| Source codec provenance | codec-source-notice-manifest.json SHA2569d2eb28e8db0068e4775bb047baa96666aec23e69952a42020678c5711604bb4;176source files/108applicable codec module byte comparisons, zero mismatches |

## Exact binding review observations

Safe API accepts &str only, creates HTML context itself, fixes encoding/options, installs deny-all context loader before memory parsing, returns owned document/diagnostics. Loader callback does not inspect URL/publicID/userdata, never allocates/panics, nulls non-null output pointer and returns XML_IO_LOAD_ERROR. Tests directly exercise output/null-output ABI and prove actual libxml2 dispatch by callback counters through XML context file loading for fileURL,existing localfile,HTTP,FTP; all return no document and a real loopback listener receives no connection. XML context is used deliberately because HTML DTD input alone may never invoke any loader. Source parserInternals.c xmlLoadResource returns immediately after installed callback, with no fallback on rejection; CATALOG is disabled so catalog resolution cannot precede callback.

ErrorCollector stores boxed Vec at stable address, registers thread-local callback and restores prior handler/data before dropping errors by field order. ParserContext is RAII-owned, returned Document separately owns its allocation. Controlled catch_unwind test confirms restoration of calling-thread handler/data. C diagnostics allocation still can abort on OOM; helper process containment remains needed. No async suspension/C objects cross worker threads. Further concurrent handler-isolation/drop stress, callback invocation during HTML paths where applicable and safety review remain pending.

Test harness detail: initial attempt to test libxml as a non-member dependency was rejected by Cargo, then temporary candidate workspace membership was used. Upstream criterion/rayon dev-dependency declarations were removed only in the isolated test harness because no lib source unit tests reference them; no upstream complete-suite qualification is claimed. The proposed released dependency still has its original package manifest.2newtests executed with12unrelated source unit tests filtered; this is explicit focused API qualification, not the A05 fail-closed complete gate.

C feature alignment: actual2.15.4 configuration retains OUTPUT/C14N/SCHEMAS/READER/REGEXPS/etc needed by broad wrapper, and bindgen forced OUTPUT/C14N macros therefore agree with generated xmlversion.h. Disabled HTTP,modules,catalog,Python,iconv,ICU,zlib,XInclude. Candidate parse code links on macOS; this is not an all-target/all-exported-function symbol proof. Need all-target symbol/layout gate before adoption. Static archive has no undefined iconv symbols; executable otool still includes platform libiconv, origin not yet classified. No dynamic libxml/libpython/ICU linkage observed. Do not claim a stronger dependency absence.

Smoke found NUL-only candidate has no root, whereas original source whole-search is valid empty. This remains wrapper/version qualification work; no corpus values were changed. Orphan table rows retain X in C candidate. Neither smoke establishes complete lxml semantics.

## Codec provenance decision requested

Official Python3.14.7 source downloaded only for inspection: SHA2563b48dac8fb59f62eaa67ac83c1eb12bda1b7a08406dd286e252c11a66be27f81. HTTPS source and local hash retained; publisher signature not independently verified. Manifest hashes complete encodings package, CJK implementation/mappings, Unicode/name generators and relevant core codec files; root LICENSE and per-file notice match lines retained. All108applicable runtime Python codec module .py files match archive byteexact. No table/state machine/algorithm has been copied into native runtime.

Review this exact manifest before approving any PSF-derived tables/ports. License review must retain root historical license text and file-specific notices; do not infer all files share one SPDX label or add blanket deny allowlist. Generator remains temporary source-evidence tooling at /private/tmp/a05-codec-source-manifest.py; if retained in repository it must be inventoried for G08. Current manifest is a source/notice proposal, not a legal determination or completed codec implementation.

## Remaining blocking evidence

1. Review final exact patch, source/notice manifest and resolved package licenses/advisories; enforce minimum/exact vendored version in build, remove all system/default-binding fallback.
2. Qualify2.14.6 versus2.15.4 through full lxml wrapper/source observations and every current186case; repair mismatches without rewriting expected output.
3. Linux GNUx86_64/aarch64 and remaining macOS build/symbol/layout qualification, release SBOM and security source review.
4. Implement supervised native helper protocol/admission/deadlinekill+wait/shutdown tests; process resource confinement claims need actual evidence. Configurable concurrency; derive byte bounds from accepted32MiBF08body and codec expansion. No new16MiBcap.
5. Complete native108codec families with approved provenance, alias/error/empty/malformed matrix, generated-table mutation failures and independent review. No Python runtime.
6. Refresh only parent-approved F01 pins, full checks and exact-tree independent review. A05 remains unaccepted.

No Cargo/helper build is running at handoff; all terminal processes were polled to exit. ASTgraph refreshed after mandatory gate source changes. Last disk check6.9GiB available.

## Requested package/policy and complete host symbol receipt

Per-new-package declared license/MSRV retained in binding-new-package-policy.json; absent upstream MSRV remains null, not inferred. Isolated cargo-deny with existing repository deny.toml, --offline --locked, exits0: advisories/bans/licenses/sources all OK; unmatched existing allowlist warnings only. This uses cached advisory state and is not a fresh remote security assurance. Initial attempts used an obsolete --disable-fetch flag and were rejected; final successful command uses current global --offline. Temporary harness explicitly declares MIT/publish=false, no policy relaxation.

Exact enabled/disabled C feature configuration is in libxml2-feature-config.txt. Host symbol audit compares every xml/html function declared by generated bindings against the actual static archive:1276declared functions,1421defined xml/html symbols, **zero missing declarations**. This closes current-host forced C14N/OUTPUT mismatch with both actually enabled. Other target layouts/symbols remain future CI evidence.

Aggregate evidence manifest is adoption-evidence-manifest.json; its SHA256 is reported in parent handoff. It pins patch, graph/package policy, deny log, exact C options, symbol audit, actual loader tests, codec source manifest and expected-red186replay log. No build or test process remains active at this receipt.
