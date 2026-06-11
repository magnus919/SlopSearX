# Merging and ranking

Active contributors: Magnus Hedemark

## Purpose

Merges results from multiple engines, deduplicates by normalized URL, and ranks results. V1 uses presence-weighted ranking. The `Ranker` interface is pluggable for future strategies.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `Ranker` | `slopsearx/merger.py` | Abstract base class for ranking strategies. The `rank()` method receives engine results and returns a ranked, deduplicated list. Designed to be pluggable from V1 so the strategy can be swapped without architecture changes. |
| `PresenceRanker` | `slopsearx/merger.py` | V1 presence-weighted ranking. Normalizes URLs, deduplicates by normalized URL, boosts results that appear in multiple engines, and assigns positions by final score. |
| `_normalise_url` | `slopsearx/merger.py` | Strips tracking parameters (`utm_*`, `fbclid`, `gclid`) from URLs for deduplication purposes. |
| `build_engine_status` | `slopsearx/merger.py` | Builds per-engine status map from adapter responses: result count, latency, and status value. |
| `extract_unresponsive` | `slopsearx/merger.py` | Extracts failing engines as `[engine_name, reason]` pairs for the SearXNG-compatible `unresponsive_engines` field. |
| `build_meta` | `slopsearx/merger.py` | Builds the `meta.*` extension field with response time, cached flag, query ID, and per-engine status. |
| `merge_results` | `slopsearx/merger.py` | Convenience wrapper for backward compatibility. Dispatches to the configured `Ranker`. |

## How it works

### URL normalization

Before deduplication, every result URL passes through `_normalise_url()`. The function:

1. Parses the URL with `urllib.parse.urlparse()`
2. Strips known tracking parameters: `utm_source`, `utm_medium`, `utm_campaign`, `utm_term`, `utm_content`, `fbclid`, `gclid`
3. Reconstructs the URL without those parameters

This ensures that the same article linked with different tracking tags is treated as one result.

### Deduplication

The deduplication strategy is first-occurrence-wins. When two engines return the same normalized URL:

- The first URL encountered keeps its position
- The second engine name is added to the result's `engines` set
- The result's primary `engine` field stays as the original engine

```python
if norm_url in seen:
    existing = seen[norm_url]
    existing.engines.add(engine_name)
    existing.score = 1.0 * len(existing.engines)
```

### Presence-weighted scoring

The V1 ranking algorithm assigns each result a base score of 1.0. When a result appears in multiple engines, its score is boosted by the number of engines that returned it:

```
score = 1.0 * number_of_engines
```

Results are then sorted by score descending. This biases toward consensus: a URL returned by 3 different engines is ranked above a URL returned by only 1 engine, regardless of which engine produced each.

### Per-engine result budget

Each engine can declare a per-request result budget via `PresenceRanker.__init__(per_engine_budget)`. This caps how many results from a single engine contribute to the final ranking. The budget prevents a single engine with many low-quality results from dominating the merged output.

### Documented quality ceiling

Presence-weighted ranking is deliberately simple. It is not better than any individual engine's ranking. It provides breadth (coverage gain from multiple engines) at the cost of precision (less accurate ordering than any single engine). The ranking is presence-weighted, not quality-weighted.

### Metadata helpers

Three helper functions build the SearXNG-compatible metadata returned alongside results:

- `build_engine_status()` assembles per-engine diagnostic data: result count, latency in milliseconds, and status
- `extract_unresponsive()` collects engines that returned non-OK statuses into `[engine_name, reason]` pairs
- `build_meta()` bundles everything into the `meta.*` extension dict: `response_time_ms`, `cached`, `query_id`, `engine_status`

## Integration points

- **Search flow:** After `asyncio.gather()` dispatches to all engines, the server collects `AdapterResponse` objects, groups results by engine, and passes them to `_ranker.rank()`.
- **Response formatting:** The ranked result list feeds into `format_json()` or `format_yaml_markdown()` for serialization.
- **Metadata:** `build_meta()` output is injected into the response as `meta.*` extension fields.

## Entry points for modification

- Adding a new ranking strategy: subclass `Ranker` and implement `rank()`
- Adjusting deduplication behavior: modify `_normalise_url()` to strip or retain additional parameters
- Changing metadata structure: modify `build_engine_status()` or `build_meta()`

## Key source files

| File | Description |
|---|---|
| `slopsearx/merger.py` | All ranking logic: `Ranker`, `PresenceRanker`, URL normalization, metadata helpers |
