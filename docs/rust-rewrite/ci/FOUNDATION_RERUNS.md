# Rust Foundation workflow reruns

A GitHub Actions rerun keeps the original workflow run ID and source revision but increments `GITHUB_RUN_ATTEMPT`. A failed-job-only rerun therefore may combine successful partition artifacts from an earlier attempt with a regenerated artifact from a retried partition.

The Foundation aggregator accepts a partition only when its identity names the exact partition, source revision, target triple, runner architecture, and workflow run ID. Its attempt must be a positive integer no greater than the current workflow attempt. Each partition upload uses `overwrite: true`, so a retried partition replaces its earlier artifact. The semantic-failure control carries a separate identity receipt and must match the same source, target, architecture, run ID, and the prerequisites partition attempt. Exactly one intact expected control marker is required.

This policy permits reuse only inside the same workflow run and exact source/target identity. It rejects evidence from another run, commit, target, partition, future attempt, or altered/missing/duplicate control. `scripts/tests/foundation-aggregation.test.mjs` exercises first-attempt success, a mixed-attempt failed-job-only rerun, and those negative cases; `scripts/foundation-aggregation.mjs` is the same validator used by the hosted aggregate job.
