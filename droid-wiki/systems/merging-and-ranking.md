# Merging and ranking

Active contributors: Magnus Hedemark

## Purpose

Merges raw results from multiple engines into a ranked, deduplicated list using presence-weighted ranking.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `PresenceRanker` | `slopsearx/merger.py` | V1: presence-weighted ranking. Results from more engines score higher |
| `merge_results()` | `slopsearx/merger.py` | Convenience wrapper for backward compatibility |
| `build_meta()` | `slopsearx/merger.py` | Builds `meta.*` extension field with response_time_ms, cached, query_id, engine_status |
| `extract_unresponsive()` | `slopsearx/merger.py` | Builds unresponsive_engines list for SearXNG-compatible response |

## How it works

### V1: PresenceRanker

1. **URL normalization** — strip tracking params (`utm_*`, `fbclid`, `gclid`) before dedup
2. **Per-engine budget** — optional per-engine result cap enforced during merge
3. **Deduplication** — first occurrence of normalized URL wins. Subsequent occurrences add engine name to `engines` set
4. **Scoring** — `score = 1.0 * len(engines)` (presence-weighted: wider is better)
5. **Tier preservation** — lower tier number (higher priority) preserved during dedup
6. **Sorting** — `sorted(results, key=lambda r: (r.tier, -r.score))`
7. **Position assignment** — position field set 1-based after sorting

### Documented quality ceiling

V1 ranking is not better than any individual engine's ranking. It provides breadth (coverage gain) at the cost of precision (less accurate ordering). Presence-weighted, not quality-weighted.

### V2: WeightedFusionRanker (future)

Planned for when traffic generates click-through data:
- Per-engine trust scores from Valkey
- Reciprocal Rank Fusion (RRF) with per-engine weight multipliers
- Extract a shared interface only if a second ranking strategy is implemented

## Integration points

- **Server search handler:** `_ranker.rank(engine_results, q, search_params)` called after all engines respond
- **Formatter:** Ranked results passed to `format_json()` or `format_yaml_markdown()`

## Entry points

- Change ranking: add the new strategy and extract a shared interface if it needs one
- Add per-engine budget: pass `per_engine_budget` dict to `PresenceRanker` constructor

## Key source files

| File | Description |
|---|---|
| `slopsearx/merger.py` | PresenceRanker, URL normalization, metadata helpers |
