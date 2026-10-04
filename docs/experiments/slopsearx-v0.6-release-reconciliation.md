# SlopSearX v0.6.0 reconciliation before selector implementation

Inspected on 2026-10-04. [v0.6.0](https://github.com/magnus919/SlopSearX/releases/tag/v0.6.0) was published at 16:56:32 UTC from `ae52a386ee709351e14b373e6f4ddbb34e7b2d64`. Current main at inspection was `701ec0d8a2611e329bae7a4414ed0555966ce6fa`. The release-to-main diff contains no changes under `slopsearx/`, `tests/`, or `pyproject.toml`; GPT-6 Luna independently compared ten relevant runtime/package blobs. This is source inspection, not confirmation of any deployed image.

## Already shipped; reuse it

- Optional, key-gated Jev result reranking with pinned `jev-1.13.0`, strict response validation, deterministic stable ordering and atomic incumbent fallback.
- Jev specialist routing and routing advisories across existing HTTP/MCP/portal/agent surfaces. Engine-routing scores are not evidence of source usefulness or facet coverage.
- Scholarly grouping before merge/ranking. Use the complete post-grouping merged list as the candidate pool; grouping metadata is not proof of source quality.
- Shared SearchService, sensitive-scope and grouped-provenance guards, provider-aware cache identity, tenant-scoped full-result snapshot capture, and presentation slicing after capture.
- Research jobs retaining their parent question and bounded subquery execution. Their existence does not mean the parent question reaches the ranker.

Do not build another Jev transport, ranking pipeline, grouping system, snapshot mechanism or Jev client inside GroktoCrawl. Extend the shipped service/provider seams and preserve their compatibility/security behavior.

## Still missing from the release

The [shipped request](https://github.com/magnus919/SlopSearX/blob/v0.6.0/slopsearx/service.py#L122) has no explicit caller-purpose or facet fields. The [shipped rerank seam](https://github.com/magnus919/SlopSearX/blob/v0.6.0/slopsearx/service.py#L1145) sends only the first forty cards with the query to the ordinary Score provider, then appends the unconsidered tail. A full captured snapshot does not imply full-pool model ranking.

The proposed extension therefore remains opt-in original-purpose/facet propagation, a separate purpose-capable provider contract, and qualified complete-pool ranking/selection up to its frozen bound. It must reuse the existing grouping and service seam; preserve legacy `rerank(query, candidates)`; isolate context in cache/singleflight/research replay identity; carry explicit context across HTTP/MCP/portal and original X research paths; and preserve whole-incumbent fallback and sensitive-provenance exclusion. Research currently dispatches [generated query text](https://github.com/magnus919/SlopSearX/blob/v0.6.0/slopsearx/research.py#L706), so job-level question storage does not substitute for that propagation.

## Compatibility delta and evidence boundary

Relative to the original implementation-checklist inspection revision `3d5e8bac38b619aec1bd7a679f1a7667f1fc1abf`, the relevant runtime change is stricter snapshot decoding: malformed/non-finite persisted records return invalid-cursor state without exposing content or mutating the record. Preserve this behavior and its regression tests. The version bump does not introduce a competing purpose-aware selector or require replacing the ordinary ranking contract.

The [original acceptance checklist](complete-pool-production-acceptance.md) is retained unchanged because historical experiment integrity manifests pin it. This reconciliation supersedes its source-inspection baseline, not its acceptance gates. EXP-059 passed exposed development only: all primary outputs retained D, with incremental coverage confined to the extended research case. No production-quality claim follows from the release or that result.

Before adopting runtime code, confirm the frozen candidate on untouched inputs against this release's actual ordinary W0 on the identical pool, with D retained as a comparator. Trace real provider requests rather than assuming a full response was fully reranked. Brave remains at 0/10 attempts; failures/retries consume the allowance. No deployment, Hermes, mainline GroktoCrawl or ordinary/default behavior changes are authorized by this reconciliation.
