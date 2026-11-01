# Native image operations — partial O02

Read the [rewrite case study](../CASE_STUDY.md) and [O02 #919](https://github.com/magnus919/SlopSearX/issues/919). This increment replaces `scripts/verify_docker_base_digest.py` and the Docker publication workflow's inline Python platform parser with `ssx-ops`. It is stacked on health PR #1082 and preserves that prerequisite's product bytes. It does not implement digest promotion, switch the Python server, complete O02 or authorize production activation.

## Commands and preservation

`ssx-ops verify-base-digests [--dockerfile PATH]` defaults to `Dockerfile`. It strictly recognizes either a legacy pinned Python runtime alone or the current pinned Rust 1.99.0 Bookworm health builder followed by the Python runtime. Unknown/additional stages, invalid tags and malformed digests fail closed. It checks every recognized base against Docker Hub: exit 0 means every pin matches, 1 means a stale pin, and 2 means invalid input/lookup/protocol failure. This operational command deliberately performs read-only public registry requests; default tests are entirely offline.

`ssx-ops verify-image-platforms --manifest PATH` reads the actual `imagetools inspect --raw` output and requires exactly the Linux amd64/arm64 platform set. Duplicate entries retain the old Python set semantics; extra platforms including unknown/unknown fail. This application-index rule is not applied to multiarchitecture base images. Invalid manifest input exits 1; invalid command arguments exit 2.

The native tool reuses accepted `AdapterHttp` sessions, bounded decoder/IO/retirement, native `ssx-json`, `sha2` and URL construction. It adds no TLS stack or generic evidence framework. The health helper remains std-only. Source Dockerfile input is capped at 64 KiB and application manifests at 1 MiB.

## Explicit operational hardening

Registry requests use fixed Docker Hub token and manifest endpoints, TLS verification, no proxy or redirects and identity encoding. Each logical request has at most three attempts, a 30-second attempt deadline, retry delays of 1/2 seconds and only the existing verifier's network/timeout or 429/500/502/503/504 retry classes. Token body is capped at 64 KiB; token length at 16 KiB; manifest wire/decoded body at 1 MiB. Compressed responses are rejected. Tokens/Authorization and response bodies never appear in diagnostics. Valid Docker tags are bounded to the registry tag grammar rather than accepting arbitrary non-whitespace text from an invalid Dockerfile.

A single unambiguous lowercase SHA-256 digest header is required. The exact received identity manifest bytes must hash to that header before comparison with the pin; the old verifier checked only the header. These finite transport/content-integrity rules are reviewed differences from urllib, not claimed proxy/environment/redirect parity.

## Actual callers and CI footprint

The existing amd64/ARM64 Docker jobs compile/test one native host tool in an owned runner-temporary target with debug information/incremental cache disabled, then invoke it before image building. The amd64 job uploads only its normal tool and checksum for the same workflow's manifest publisher. The publisher verifies that checksum, executes the native platform validator on every actual manifest, and preserves existing image digests, tags, permissions, push conditions and retention behavior. Both jobs retire their owned tool/build prefix. No third compiler target, arbitrary artifact repository/run, binary download or new privilege domain is introduced.

Offline validation commands are `cargo test --locked --offline -p ssx-ops -- --test-threads=1` and `cargo clippy --locked --offline -p ssx-ops --all-targets -- -D warnings`. Transport replay exercises actual command logic before denied/error cases, including signed manifests, stale pins, bounded retries/deadlines, invalid/oversized tokens, digest/body/encoding/endpoint substitution and every platform-set failure. Actual native subprocess controls cover local manifest success/failure and invalid source/arguments with an empty PATH; valid registry CLI acquisition remains a hosted caller witness, not a local live probe. Local Docker/Podman are unavailable. No native or hosted results are claimed by source authorship.

## Reference regression mapping and remaining scope

The old generic-base, multiple/unrecognized-stage and health-builder verifier tests move to `actual_docker_stages_and_hostile_stage_forms`; transient/permanent lookup tests move to the native retry/token/size/deadline controls. The health builder COPY boundary remains in the deployment-artifact owner. The former Python platform assertion moves to native exact-platform-set and actual-process controls. Unrelated promotion, deployment and image-retention regressions remain intact. Immutable migration captures and historical qualification documents are unchanged.

Current lock admission removes only the new dependency-node stanza to recover the exact accepted health lock, then reuses its existing inverse and immutable original capsule. Full forward reconstruction plus old/missing/duplicate/changed controls preserve fail-closed admission; no historical oracle is repinned.

Image digest promotion remains Python and is not changed here. Its workflow still passes unsupported `--write`, and its exit-10 handling conflicts with Actions' default errexit. The following increment must explicitly repair those interfaces while preserving pin ownership, rollback and PR conditions. Portal smoke, hosted-only mirror configuration and bounded actual-adapter upstream probes also remain O02 work. O04/G08 retain native server assembly and backend retirement. No full O02 gate is claimed.
