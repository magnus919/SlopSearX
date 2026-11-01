# Native application image promotion — partial O02

Read the [rewrite case study](../CASE_STUDY.md), [native image operations](IMAGE_OPERATIONS.md) and [O02 #919](https://github.com/magnus919/SlopSearX/issues/919). This increment is stacked after image-operations PR #1083, which follows #1081 and #1082. Native source, fixtures and workflow are authored here; qualification results are not implied by authorship.

## Explicit command and data boundary

`ssx-ops promote-image-digest (--check | --write) [--root PATH]` requires exactly one explicit mode. Neither, both, repeated or unknown options fail with exit 2 before registry IO. The default root is the current directory. Check exits 0 for a current pin, 1 for drift, 2 for source/registry failure; it never writes. Write exits 0 for a current pin, 10 only after successful replacement of the owned data surfaces, and 2 for validation/lookup/application failure.

Only three fixed data files may be written: `docker-compose.yml`, `k8s/deployment.yaml` and `spec.md`. Each deployment file must have at least one matching full lowercase SHA-256 application reference; spec must have at least two. Repeated references are accepted only when all three sources agree on one digest. Snapshot occurrence counts are retained and every replacement must preserve them and unrelated bytes exactly. Missing, conflicting, malformed, uppercase or overlong references fail closed. Each source is bounded to 1 MiB and valid UTF-8.

This intentionally fixes the previous coupling between release data and source code: deployment tests now derive their expected full validated image pin from Compose and independently check Kubernetes and spec. Future promotion PRs update three data files, without editing test code or its source-proof hashes. The earlier proposed fourth test-code write surface is superseded. Existing default-image, source-build opt-in, environment forwarding, network aliases, scan/smoke ordering, retention and multiarchitecture contracts remain.

## Public registry and integrity policy

The only registry flow is the anonymous public GHCR token endpoint for `repository:magnus919/slopsearx:pull`, followed by `https://ghcr.io/v2/magnus919/slopsearx/manifests/latest`. The handler reuses accepted `AdapterHttp`, native JSON, token bounds and exact response-body digest validation. No arbitrary repository, endpoint, token, resolved-digest option or environment routing override is exposed.

The four previous GHCR Accept types remain: OCI index, Docker manifest list, OCI image manifest and Docker v2 image manifest. Docker Hub's separate Accept policy is unchanged. Token response is at most 64 KiB, token length 16 KiB, and identity manifest body 1 MiB. Endpoints must match exactly, redirects and environment proxies are disabled, compressed bodies are rejected, and exactly one full lowercase SHA-256 header must match the exact received body.

Each attempt has a 30-second deadline; at most three attempts use 1/2-second backoff only for network/deadline or 429/500/502/503/504 failures. This replaces Python's catch-all retries and 2/4-second waits deliberately. Permanent 400/401/403/404 failures are immediate. Tokens, Authorization and response bodies never appear in application diagnostics. Resolving latest does not itself prove scanning, architecture or signing attestations; existing publication and protected PR checks remain authoritative.

## Unix source and application guarantees

The operational workflow is qualified on Linux architectures and local Unix fixtures; Windows write qualification is not claimed. Portable pin planning and registry functions remain available, and existing image-verification commands retain their behavior. The added Unix filesystem API uses already pinned nix 0.31.3 with no new package version or bespoke unsafe FFI.

Before lookup, read and validate all three regular source files through retained directory/file descriptors with nofollow/nonblock opens, explicit byte limits and file identity/content checks. No symlink leaf or parent may substitute a fixed source. Build all replacement buffers in memory and prove inverse reconstruction before mutation. Recheck original identity/content before staging and immediately before each replacement.

Stage all outputs in create-new owned sibling files, preserve original permission bits, flush/synchronize and verify the staged bytes before changing originals. Replace in fixed order with per-file atomic descriptor-relative renames and directory synchronization. Predictable prevalidation or staging errors leave originals unchanged. Three renames are **not one transaction**. A later rename/IO failure returns 2 with the exact replaced paths and staging-cleanup failures, retires remaining identifiable owned staging files, and does not roll back or overwrite concurrent edits. Mixed-pin state must be reviewed/recovered before retry. These checks do not claim a cross-file filesystem compare-and-swap against a concurrent writer racing the final rename.

## Actual caller and acceptance

The existing daily `23 5 * * *` and manual workflow build/test one owned native host tool, safely capture exit 10 under Actions errexit, and retain the existing conditional promotion PR flow. PR staging is limited to the three data paths; branch, labels, conventional title/commit and permissions remain. Owned build/tool/scratch inputs are always retired. Creating a maintenance PR does not authorize merging it or deploying an image.

Offline fixtures cover repeated-pin planning, malformed/missing/conflicting inputs, signed GHCR replay and denied/error paths, actual owned filesystem writes, unchanged modes/counts, create-new collisions, real later rename failure with partial-state accounting, and preserved concurrent edits. Actual native subprocess cases cover explicit-mode/source errors with an empty PATH. A bounded local stub executes the actual workflow run body under Bash errexit for exits 0, 10 and 2. No default test invokes a real registry, edits checkout deployment pins or opens a PR.

Before delivery, run the affected native package's actual inventory/build/lint and existing deployment regressions, then the complete active source-admission closure, including F01/F02, T02, A13, O06 and the shared W03 historical consumer. Frozen historical files remain unchanged. Hosted workflow behavior is separate from offline evidence; no real promotion or workflow dispatch is part of local qualification.

Portal smoke, hosted mirror configuration, upstream probes and Python backend cutover remain open O02/rewrite work. No full O02 completion, native server assembly, release activation or production operation is claimed.
