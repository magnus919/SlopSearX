# EXP-050 runner/projection integrity review

Read-only review of the administratively renamed EXP-050 staging artifacts. No request, provider, search, credential, grading, or source-edit action was performed.

Reviewed hashes:

- `runner.py.txt`: `a4a42b8ae41c550f860f3747c0a40d5309dbe6f6e735c7113555f6cea46f20ed`
- `projection.py.txt`: `6429170f56a630c584e8169c6d06c150f0ebb3f5270bf11d97b96fd47ad69548`
- `integration-qualification.py.txt`: `108cb2f9b19aafe5e1697ea934657bb41ae0661af1bf8659872d5a2ee168e9db`
- `plan.json`: `27da79250c05e6293b44aa25e7764deaae6108ae144732ec5681169f84c2608e`
- `batch-body.json`: `0c328f18ef4e5cfbad7319f28db64be2bf8b6c2f28f04ebd88cca9dbfbce0513`

The earlier integration defect is closed. The runner now has explicit raw-input mode; the combined fixture selects it and feeds the exact response bytes to `projection.project`, which performs strict parsing and returns raw response and per-record byte receipts. The outer runner keeps response hash/length in its private attempt record, while the public projected result excludes that raw receipt. This path preserves the strict 8 MiB total response, 256 KB raw-record, JSON depth/list, duplicate-key, non-finite-number, UTF-8, exact response count, and schema checks. The fixture rejects an oversized escaped record, confirming the bound is based on original response bytes rather than reserialization.

The projection fixes also close the previously identified data-integrity issues: `paperId` must be forty hex characters; unknown external-ID namespaces such as numeric `CorpusId` are ignored while recognized key values remain typed; conflicting normalized aliases fail closed; URL-derived identifiers and wire namespace prefixes are validated; the arXiv request scope is explicit as work-base with version/content unverified; and original engine provenance remains explicitly unknown. Author/type per-item clipping is reflected in the clipped flag, the abstract excerpt retains original whitespace and marks clipping only when bounded, and the information screen now counts nonempty author/type values while rejecting empty journal shells and whitespace-only values.

The pinned runner fixture reports 21 checks, the projection fixture 11, and the combined raw-callback fixture 13; the combined receipt binds the current runner, projection, plan, batch body and fixture hashes. Its synthetic shuffled 217-query response preserves all 423 original cards, including all 204 `no_identifier` rows. These are offline protocol/integrity checks only, not evidence that metadata is accurate or useful.

No remaining component-level protocol or integration blocker found in the reviewed bytes. The acquisition remains unperformed; this review does not establish live provider behavior or complete the broader production goal.
