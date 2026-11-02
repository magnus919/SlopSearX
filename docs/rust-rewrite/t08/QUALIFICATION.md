# T08 qualification snapshot


Status: **Local and hosted T08/F03 qualification passed; final independent acceptance, reviewed O04 handoff and measured cleanup PENDING.**


The actual local run used source `6ac559a01ff6ea8bef047c923c23251837e2b27e` on `aarch64-apple-darwin`, Rust `1.99.0` (`b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`, LLVM `23.1.1`). `Cargo.lock` SHA-256: `7f64930ad5267129bcd5f750de6bba4249c8a3bfde3a3de1753a853b5e1db9d9`.


Read the [rewrite case study](../CASE_STUDY.md), [T08 contract](README.md), and [client source-installation guide](../t08a/README.md#build-and-source-installation). This records controlled production-component integration through the real HTTP router, SearchService, npm adapter and private Valkey. The upstream npm response endpoint is deterministic loopback. Production startup, deployment and O04 distribution acceptance require their own evidence.


## Executed integration


T08 passed all 17 ordered cases against the actual default CLI and again against the independently source-installed CLI: **34 runtime cases**. Each receipt then passed three admission counterfactuals (omit a case, duplicate a case, change image identity): **six controls**, counted separately from runtime cases.


The run checked actual output/status boundaries, Unicode queries, exact raw numeric page lexemes `18446744073709551616` and `18446744073709551617`, distinct first cache misses, repeat hits without additional adapter/upstream dispatch, denied scope/page and classified failures. Child waits, upstream thread join and private socket removal completed for both receipts.


| Image | SHA-256 |
| --- | --- |
| Default CLI | `dc512ae02830cd28f7cc60f6dbe6c7a2e7ce543252bcfeb2833d7e29c0b532b9` |
| Source-installed CLI | `dc512ae02830cd28f7cc60f6dbe6c7a2e7ce543252bcfeb2833d7e29c0b532b9` |
| HTTP production-component assembly | `038f1497969856145458e66470798c8b7a2e7f9b752339826dc948b23db8f3ab` |
| Native integration driver | `37f5cf3822ef7d1c00b509a6be712067a07d8d82d4480473433c3a12f16ba135` |
| Private Valkey server | `452c8577bb2e7e621762bd95a434ff8c3b88f3a8f9c80c4fbb4baece93b99990` |


The two CLI digests are equal; installation and both executions were independently observed.


Actual case IDs, each executed on both images:


- `health-text`
- `health-json`
- `config-text`
- `config-json`
- `engines-text`
- `engines-json`
- `search-yaml`
- `search-json`
- `page-0-0`
- `page-0-1`
- `page-1-0`
- `page-1-1`
- `unknown-engine`
- `negative-page`
- `upstream-error`
- `upstream-malformed`
- `unavailable-server`


## Executed standalone compatibility


On the same source and native host, T08A passed **591 compatibility receipts**: the unchanged original 531 (522 frozen, eight supplemental and one negative control), plus **30 native discovery cases on the default image and 30 on the installed image**. All 591 retained receipt file hashes and both new owner summaries were verified. Expected failure cases retain their expected failure statuses; these are contract passes, not 591 successful HTTP responses.


## Executed source acceptance


The current A13 owning reader passed: one selected test, zero failures/ignored, 148 filtered. Current O06 passed: 15 selected tests, zero failures/ignored, 134 filtered. Formatting and current normal/test compiler roles passed. Strict xtask and CLI checks retain their actual `ae7ae63a` origins with unchanged Rust/CLI source; current compiler identities were rebuilt, and the changed driver passed its current strict check.


This verified `6ac559a0` run supersedes the author-time “native/installed acceptance pending” note retained in source-bound [`t08a/intentional-differences.json`](../t08a/intentional-differences.json). It does not change that historical record or imply hosted completion.


## Hosted exact-head acceptance

Merged PR [#1081](https://github.com/magnus919/SlopSearX/pull/1081) passed hosted T08 and F03 qualification at exact head `5a6fd3b41428fd1e9c6317e14f2c131eeb88419f` in [Actions run 41329461494](https://github.com/magnus919/SlopSearX/actions/runs/41329461494). Both the [x86_64](https://github.com/magnus919/SlopSearX/actions/runs/41329461494/job/124916298551) and [ARM64](https://github.com/magnus919/SlopSearX/actions/runs/41329461494/job/124916298440) Foundation runtime jobs passed “Native CLI default and installed backend integration evidence” (`cargo xtask check T08`). Both the [x86_64](https://github.com/magnus919/SlopSearX/actions/runs/41329461494/job/124916298442) and [ARM64](https://github.com/magnus919/SlopSearX/actions/runs/41329461494/job/124916298512) Foundation core jobs passed “Foundation lane evidence” (`cargo xtask check F03`). The run completed with 37 successful checks, 4 skipped, and no failed or pending checks.

Reconciliation against current main `393599e824b35da1dffb58eb5cd45c83b99eb7a1` found the Rust workflow, gate sources, lockfile, toolchain, and F01/F02/F03 plan entries unchanged from the tested revision. Later planning edits changed the full plan digest and the corresponding T08A binding row only. T08 does not run T08A’s source/binding audit, so these documentation-only changes do not invalidate the hosted T08/F03 evidence. The earlier pending-hosted status is stale.

## Remaining acceptance

- **PENDING:** final independent acceptance recorded for the merged T08 integration; PR #1081 currently has no submitted GitHub review.
- **PENDING:** review the retained O04/#921 handoff (SHA-256 `e2bd97996b59fd0c597214b80d35ed9aef4eb22c813b5e8da21b98761655f95a`) and record owner acceptance.
- **PENDING:** reconcile the final T08 acceptance record and measure/clean up owned build artifacts after the applicable PR merge.

The T08 integration PR #1081 is already merged. Hosted/release acceptance beyond the T08/F03 gates, O04 release artifacts, signing/promotion and production install/upgrade/uninstall remain in #921. No production activation was performed.

Hosted and release acceptance cannot be inferred from the successful local checks. O04/#921 owns release archives, signing/promotion and production install/upgrade/uninstall. The optional historical root `ssx` Python script remains a standalone client-side wire client (SHA-256 `4c45e876fc79dffc0f7da8ccccc9c6ba67d4f4cc9e0bd9e8194f74184b96c65b`); the native client/backend paths neither invoke nor require it. Portal review found no browser-visible contract change in this integration slice.

Refs #903 and #921. No production activation was performed.

## Evidence identities


| Record | SHA-256 |
| --- | --- |
| T08 gate report | `64e99cea47f86733bb074ab4b6bcfa3be0ab9016c81bc0feabad81d32911839e` |
| Default completed receipt | `5bcece2a38740917021b0c1ce40c47d3957be19173e3a28a5903f285485e2e60` |
| Installed completed receipt | `07f5f4866f967dc1df8e9c7eebea28c96b375c62bf1628b32267c8b2ca71ecc4` |
| Actual O04 handoff | `e2bd97996b59fd0c597214b80d35ed9aef4eb22c813b5e8da21b98761655f95a` |
| T08A gate report | `6f1a28fb2d53798f8b2c405db93bc986560637af3aa9922d8627a489ca151f1f` |
| Complete 591-receipt inventory | `fd3bc897a4ace37181f2cc9be7baa04e127a1eb293c654805ca314f26387915f` |
| New discovery owner, each image | `5c0c94cfd061e77785a85e9f9964f1a58df6e98ef6402fe29588e0f47d4eadc0` |
| A13 owning reader stdout | `489b24a3dddb195064b4f72e4179a4261c6087cceb1f2ac65472f17edcd735b5` |
| O06 gate report | `b849991ee074e77ba5ecf4d2adc1a48d6a618e851d78696b93efaa9f6c34392c` |
| Final local acceptance handoff | `8cf29a4531c2343aec4f2024992c793c29bf3c61d33458a9dd301401cc652e08` |


The receipt paths are retained in the actual gate report and `O04-HANDOFF.json` below the run's `rust-evidence/t08-native-*` directory. Preserve these origins when later documentation or CI commits change the branch head; they do not become new-head executions.


