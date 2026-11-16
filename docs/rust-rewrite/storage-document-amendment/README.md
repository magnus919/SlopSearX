# Exact storage-document amendment

This validator permits only the reviewed PR1027 storage-policy revisions of `AGENTS.md` and `CONTRIBUTING.md`. It independently binds each exact historical source SHA and optional byte length to an immutable original archive, then checks the exact current successor SHA. Prefixes, alternate spellings, unknown files, changed archives and changed current bytes fail closed.

Archives are exact Git bytes, not reconstructed Markdown: `AGENTS.before.md` is blob `fa5b75d5407dfb90613d617d4f6c24f236b06aa4` (SHA256 `d710c84ebad4582dc607cc3bdb806a5a5253259e4cabd5c515f51e55c3588af8`,14212bytes); `CONTRIBUTING.before.md` is blob `8950cc84fbf5abebf8c72a1a8fe39b4cd9ef3eb1` (SHA256 `2df6be141a3b06c662a4062af79eb074248ea70f297a7bf4c65432c10feba681`,8223bytes). Source authoring compared each archive to `git show <coverage.baseline>:<path>` for A07, A08 and A09; all three baselines contain those same exact bytes.

A07 and A08 call this module for those source-inventory paths only. A09 reaches the same module through its unchanged A13 source-pin verifier. The latter still verifies the exact A13V5 manifest/predecessor chain and exact calling-container identity before admitting the two document rows. All nine existing code amendments, every other original/artifact check, all coverage containers and their case counts remain unchanged. `a09.rs` and its pinned historical successor are unchanged. This is an explicit source-document amendment, not a blanket historical-docs exemption.

The six amendment controls live in `tests.rs`, included only by the xtask A07 module. The shared validator contains no test module, so importing it through existing A13 fixture source does not add names to the historical A13 native inventories.
