# T08 qualification snapshot

Status: **Historical local T08/T08A and hosted PR #1081 T08/F03 acceptance passed on their recorded source revisions. The full F03 aggregation and T08 runtime also passed on current-main-equivalent source in PR #1090’s final Foundation attempt. The 591 T08A receipts remain historical because T08A source bindings changed afterward. Independent review, O04 owner acceptance, and release/cleanup acceptance remain pending.**

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

## Current-main reconciliation (checked 2026-11-03)

The hosted evidence has two distinct origins; neither is relabeled as a run on a different source SHA:

- PR [#1081](https://github.com/magnus919/SlopSearX/pull/1081) was tested at `5a6fd3b41428fd1e9c6317e14f2c131eeb88419f`, then merged as `cfd4d1e3c99544547074b3ee0494056d30e2d90c`. [Foundation run 41329461494](https://github.com/magnus919/SlopSearX/actions/runs/41329461494) passed its T08 and F03 jobs on x86_64 and ARM64; the T08 jobs are [x86_64](https://github.com/magnus919/SlopSearX/actions/runs/41329461494/job/124916298551) and [ARM64](https://github.com/magnus919/SlopSearX/actions/runs/41329461494/job/124916298440), and the F03 jobs are [x86_64](https://github.com/magnus919/SlopSearX/actions/runs/41329461494/job/124916298442) and [ARM64](https://github.com/magnus919/SlopSearX/actions/runs/41329461494/job/124916298512). The PR aggregate had 37 successful and four skipped checks, with no failures or pending checks.
- Current `origin/main` was `d7fc628d1069ab1f0a5fe8ee114796c235615eca`. Its Git tree is identical to the source tree at `edb7851a84a89d81f33d7f8aab292131e62f3444`, the tested head of PR #1090. [Foundation run 41620922650, attempt 3](https://github.com/magnus919/SlopSearX/actions/runs/41620922650/attempts/3) passed the F03 aggregates on x86_64 ([job](https://github.com/magnus919/SlopSearX/actions/runs/41620922650/job/125923506714)) and ARM64 ([job](https://github.com/magnus919/SlopSearX/actions/runs/41620922650/job/125923504886)), as well as both Foundation runtime jobs. In that run, the `Native CLI default and installed backend integration evidence` step—which executes `cargo xtask check T08`—passed on x86_64 ([job](https://github.com/magnus919/SlopSearX/actions/runs/41620922650/job/125923581560)) and ARM64 ([job](https://github.com/magnus919/SlopSearX/actions/runs/41620922650/job/125923551462)). This is current-main-equivalent T08/F03 hosted evidence; it does not upgrade any earlier local receipt to a current-source result.

The following T08-specific inputs were byte-identical between the #1081 tested head and current `d7fc628`:

| T08 input | SHA-256 at both revisions |
| --- | --- |
| `.github/workflows/rust-cli.yml` | `adbc0ade6742558d9697e49f7069f21f48bcb4f34ea5b8c0c29ee3e6b54bac6f` |
| `crates/xtask/src/t08.rs` | `f5eb362c1fad0a396d957daf81cffc0cde8cf7430f44be289b33fba2b4fd3d79` |
| `crates/ssx-http/examples/t08_cli.rs` | `01532f617e649a5b8092ce354a74dda15fc9ae60f87ffa56acb6f19a40c12283` |
| `crates/ssx-http/examples/t08_fixture.rs` | `6490580899bc84789571c616314beb5ec187bcf912f7c8784abb3766ea960fe7` |
| `crates/ssx-cli/src/main.rs` | `1fef44ecd6eb3c05b977adb5de34287606ab5e20ab45f9c688b03f31db276619` |
| `crates/ssx-cli/tests/external.rs` | `89e50af0396867428d4b50642bebb2ffb5cc34dba513f6d3b443f7bfa178c444` |
| `docs/rust-rewrite/t08/inventory.json` | `37dd2eb7d31c7c303e4850391a243bc2416310085fc52e1c34f61afd6a9a7edf` |

The shared xtask dispatcher file changed from SHA-256 `dbce267dc183e0f092eb880f9600676d217ca6b4e8f2ccc39bf7baf322080ceb` to `7b84b2b3a0adcd82a6a8bf411f9c2d5696f6c24eac258a2d3f2aa732d19ae653` for the F01/O02 healthcheck-retirement proof and related tests. Its T08 and T08A dispatch arms and scope strings are unchanged. The T08 implementation and listed runtime/client/inventory inputs above are byte-identical.

The F03 orchestration did change after #1081. `rust.yml:foundation-runtime` retains the same `cargo xtask check T08` command, with its outer timeout raised from 120 to 130 minutes. The F03 prerequisite/core/runtime/adapter job budgets and evidence aggregation were updated by #1090, including retry-aware partition/artifact aggregation. The workflow SHA changed from `f655e7756697d2168089d2508bd9d45c5e635820c81eef70c562ea50d4aa35db` to `014b1a9e83816712eaaf56aa5607ccb58dd1d25597fb3c17fb7e197f1d555b12`; the gate catalog SHA changed from `69d01fc4bb348b5011e0ffbc7ef6ccee59f74e7fd320832762902cd76d77331f` to `3f402f9e66592c7136e132cb781df0a22523cf449ee8aa080975a18f1a0a1e7d`. The later successful #1090 attempt validates this current aggregation and the T08 runtime against the tree now at `d7fc628`.

The T08A standalone compatibility corpus needs a separate qualification statement. These T08A/F03 support inputs changed after the #1081 run:

| Input | #1081 tested head `5a6fd3b` | Current main `d7fc628` |
| --- | --- | --- |
| `crates/xtask/src/t08a_main_join.rs` | `c5a5226d3ddf9ad7ddb4b1fa6ea7dd696ec18c7de2114fdf21ec0b7e52ccf1a8` | `f5258e7af5eefaedb76e1949692e76ac36d1d82a0d8d9bcce43cbbc619f48bf3` |
| `docs/rust-rewrite/t08a/bindings.json` | `87f044a16fc5a36b0db253bb8474a22de9dcdef94bbafa116f79f3c8d3f844da` | `49fd6cf7758c1c4cb7bc2d51a9c46190d3dfef4c8a3827f6b8f07e660e66356e` |
| `Cargo.lock` | `7f64930ad5267129bcd5f750de6bba4249c8a3bfde3a3de1753a853b5e1db9d9` | `53fb4619c6e4fd12eae7928073c5062929498d2db29b270e456514a855a5bdd9` |
| `docs/rust-rewrite/plan.json` | `7cd195caaac59e1ac27acb25ae4116ed6154ef19bab442fb83db2d8435a98ed7` | `ebe32895fc44eb46b3a1bf4fb088f1dfb06ddd3a35cbbadf90374798faa6c9ec` |

The F03 workflow and gate-catalog changes are listed above. Therefore the 591 receipts and local T08A report above remain valid historical results for `6ac559a0`; they do not prove T08A acceptance for current `d7fc628` inputs. Re-run the current source-bound T08A gate before treating that part as accepted.

PR #1081 has no submitted review and no review request. Its green hosted checks prove the listed execution gates, not independent review. The O04 handoff hash above is historical evidence; issue #921 remains open, and owner acceptance, release-artifact/signing/install validation, production cutover authorization, and measured post-merge cleanup remain separate work. No production activation is implied.

## Remaining acceptance

- **PASSED at current-main-equivalent `edb7851`/`d7fc628` source:** F03 aggregate and T08 runtime acceptance on x86_64 and ARM64 in Foundation run 41620922650, attempt 3.
- **PENDING:** re-run the T08A source-bound compatibility gate against the current accepted source/bindings; the 591 receipts above are historical evidence only.
- **PENDING:** independent review, O04 owner acceptance, release artifact/signing/install/upgrade/uninstall acceptance, final authorized production cutover, and measured post-merge cleanup.

Hosted and release acceptance cannot be inferred from these successful local checks. O04/#921 owns release archives, signing/promotion and production install/upgrade/uninstall. The optional historical root `ssx` Python script remains a standalone client-side wire client (SHA-256 `4c45e876fc79dffc0f7da8ccccc9c6ba67d4f4cc9e0bd9e8194f74184b96c65b`); the native client/backend paths neither invoke nor require it. Portal review found no browser-visible contract change in this integration slice.

Refs #903 and #921. No production activation was performed.
