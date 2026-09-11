# Retrieval provenance, ranking, deduplication, and freshness evaluation

This evaluation closes the discovery scope in issue #307. It describes the
standalone SlopSearX contract as of 2026-09-11. It does not use live engine
traffic, an LLM, GroktoCrawl, or page retrieval, and it makes no claim about
production relevance or recall.

## Current baseline

SlopSearX already has two deterministic ranking strategies. Presence ranking
remains the default. Reciprocal rank fusion (RRF) is an operator opt-in and
uses each engine feed's one-based order. Both strategies preserve tier
governance and deduplicate URLs after removing a small closed set of tracking
parameters.

The repository's three synthetic, author-judged fixtures produce this pinned
nDCG@3 baseline:

| Query family | Presence | RRF | Observation |
| --- | ---: | ---: | --- |
| general | 0.5413 | 0.9828 | RRF improves this constructed feed pair. |
| code | 0.7098 | 1.0000 | RRF improves this constructed feed pair. |
| science | 1.0000 | 0.6443 | RRF regresses because feed order and URL tie-breaking disagree with the judgment. |

These fixtures prove deterministic arithmetic and expose a regression. They do
not justify changing the default. The current test command is:

```console
pytest --no-cov -q -s tests/test_rank_fusion.py tests/test_merger.py \
  tests/test_result_contract.py tests/test_searxng_api_contract.py
```

The test boundary covers the two rankers, tracking-parameter overlap, tier
preservation, stable ordering, MCP provenance and snapshots, and the pinned
SearXNG HTTP contract. It does not measure live recall, freshness, result-body
equivalence, or end-to-end retrieval quality.

The issue audit found that [#244](https://github.com/magnus919/SlopSearX/issues/244)
already delivered evaluated opt-in RRF,
[#254](https://github.com/magnus919/SlopSearX/issues/254) preserved its effective
explanation through caches and snapshots, and
[#309](https://github.com/magnus919/SlopSearX/issues/309) added the first explicit
entity view. The shared artifact-lineage work in
[#352](https://github.com/magnus919/SlopSearX/issues/352) supplies a common
identity for future additive views; it does not itself change ranking or
canonicalize URLs.

## Contract audit

| Discovery item | Current support | Evidence and limit |
| --- | --- | --- |
| Contributing engines | Implemented | MCP cards and records expose a sorted `source_engines` list and `source_count`; merged `SearchResult.engines` survives cache and snapshot serialization. This says where a normalized URL appeared, not that sources are independent. |
| Per-engine rank | Used internally by opt-in RRF; not retained | RRF enumerates each engine feed in order. The merged record retains only its final position and score, so clients cannot inspect individual contributions later. |
| Explainable rank fusion | Partially implemented | `meta.ranking` and snapshot provenance name the effective strategy. The RRF formula is documented, but per-engine component values are not emitted. |
| URL deduplication | Implemented with a narrow identity | Both rankers strip `utm_*`, `fbclid`, and `gclid`, then collapse equal normalized URLs. The winning display record remains and contributing engine names are unioned. Raw duplicate records and their differing fields are not retained. Redirect resolution, AMP collapse, DOI equivalence, and live canonical links are not performed. |
| Domain entity grouping | Implemented as an opt-in MCP read | Version 1 groups explicit CVE, npm, and PyPI identifiers from a snapshot. It preserves references to every post-dedup result, leaves unknowns as singletons, and does not change flat search results. It cannot restore raw results already collapsed by URL deduplication. |
| Published time | Implemented when source-reported | `published_date` is nullable and retained exactly. OpenAlex can enforce relative and absolute publication filters. Missing or malformed dates remain unknown. |
| Modified, discovered, and retrieved time | Not conflated | There is no normalized modified time. Search query IDs and snapshots record observation context, while retrieval receipts carry attributed `captured_at` values. None is relabeled as publication or modification time. |
| Deterministic canonical identity | Partial | Entity IDs are deterministic for supported explicit identifiers. Snapshot result IDs are stable only within one snapshot. URL-deduplicated results have no cross-search canonical identity. |
| Deadlines and failure diagnostics | Implemented at execution boundaries | Interactive searches, staged searches, and research jobs are bounded. Engine outcomes use a closed status vocabulary and expose partial/deadline diagnostics. These are operational outcomes, not judgments about result quality. |
| Replayable quality harness | Partial | Synthetic rank and routing evaluations are replayable and source-free. There is no captured multi-engine corpus measuring recall, overlap, latency, failures, freshness, and duplicate reduction together. |

## Decisions

The following changes are accepted as additive future work. They should first
be proven in a replayable captured-response harness, then exposed through MCP
or another explicit view:

1. Retain bounded per-engine contribution records containing engine, original
   feed rank, normalized identity, and rank component. This would let any
   SlopSearX client explain a merged score and would let a GroktoCrawl-X
   experiment test whether contribution diversity predicts successful page
   acquisition.
2. Add a versioned canonical-result relationship view that references every
   contributing raw result while keeping the existing flat merged response.
   General clients could inspect disagreements and duplicate reduction;
   GroktoCrawl-X could select one fetch target without mistaking duplicates for
   independent evidence.
3. Add explicit nullable timestamps with source attribution only when an
   adapter reports their distinct meaning. General clients could reason about
   publication versus observation age; GroktoCrawl-X could test acquisition
   policies without treating search time as content freshness.
4. Extend the deterministic harness with captured, licensed or redistributable
   response fixtures and independently judged pools. Measure recall at K,
   nDCG, overlap, duplicate reduction, status coverage, and latency separately
   by query family. GroktoCrawl-X can consume those same captured result sets
   in adaptive-acquisition experiments without adding its policy to SlopSearX.

[Issue #356](https://github.com/magnus919/SlopSearX/issues/356) is the accepted
next slice for scholarly and repository entity identities. Portal explanation
work in [issue #355](https://github.com/magnus919/SlopSearX/issues/355) may
render existing ranking, scope, enforcement, and provenance, but must not call
an aggregate score confidence or fabricate missing rank components.

The following changes are rejected at this stage:

- changing the default from presence ranking based on three synthetic cases;
- adding an LLM or GroktoCrawl dependency to ranking, identity, or stopping;
- resolving redirects or fetching pages inside search-result normalization;
- treating repeated URLs, entity grouping, source count, or rank as
  corroboration, truth, source independence, or research sufficiency;
- replacing nullable dates with inferred timestamps;
- replacing SearXNG flat results or altering their default membership, order,
  fields, parameters, methods, or latency policy.

## Compatibility and experiment boundary

Any future contribution or canonical-group contract must be opt-in and
additive. The canonical cache must store enough information to derive that
view without letting presentation choices contaminate cache identity.
Snapshots remain immutable, JSON-safe, tenant-scoped evidence captures.
Ordinary `/` and `/search` clients continue to receive the SearXNG-compatible
contract and default presence ordering.

SlopSearX owns retrieval facts: source attribution, execution outcomes,
rank arithmetic, explicit identifiers, and known timestamps. A consuming
GroktoCrawl-X experiment owns its judgments: source diversity requirements,
page acquisition, publication eligibility, research sufficiency, and stopping.
That boundary keeps every proposed field useful to general clients and keeps
standalone search behavior model-independent. The consuming experiment is
tracked in [GroktoCrawl-X milestone 7](https://github.com/magnus919/groktocrawl-x/milestone/7).
