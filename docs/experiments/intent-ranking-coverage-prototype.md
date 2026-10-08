# Intent-ranking coverage prototype

**Status: PROTOTYPE / NOT REGISTERED / NOT ADMITTED.** This is an offline, synthetic-testable design prototype. It makes no quality, adoption, factual-coverage, or execution-authority claim and is not wired into search or any provider.

The prototype compiles one canonical shared state containing query, purpose, requested facet definitions, and candidate IDs/title/URL/snippet. For every candidate it asks the existing `slopsearx.rerank` score question with its pinned `MODEL`, `LEVELS`, and `INSTRUCTIONS`, plus a Choice question whose `criteria` map contains every subset of the 2–3 requested facets, the empty profile (`none`), and `unknown`. The choice instructions request one best profile. Criteria descriptions name only bounded facet IDs and refer to their definitions in shared state; candidate text and facet descriptions remain data, never trusted instructions.

The response parser requires the documented Score fields (`type`, `score`, `legend`, `probabilities`, `confidence`) and Choice fields (`type`, `choice`, `probabilities`, `confidence`). It checks the exact score legend, complete probability maps, finite values, probability sums, confidence bounds, chosen maximum, and probability-weighted score consistency. Confidence and distributions are retained for audit but do not affect policy; probabilities are not normalized.

Selection reserves the highest-query-score lead (score at least 5) for each requested facet, using the canonical tie break and deduplicating a candidate that leads multiple facets. The reserved union is sorted by score then canonical tie key; all remaining candidates are also sorted by score then canonical tie key. If any requested facet lacks a usable lead, the complete incumbent order is returned. A tied maximum profile is treated as policy-unknown; the original selected option and full probability map remain in the audit object.

Bounds are 80 cards, 160 questions, 128,000 shared-state bytes, 384,000 request bytes, and 2,000,000 response bytes. Query/purpose are each limited to 4,096 UTF-8 bytes; title, URL, snippet, and facet description limits remain 256, 512, 1,200, and 512 bytes. Inputs are rejected rather than trimmed or split. Invalid input or response returns the full incumbent order when the incumbent order itself is valid; an invalid incumbent order raises `CoverageError` because no valid fallback order exists. Fallbacks use a fixed reason and do not retry.

This design covers facet ranking for research-style contexts only. Contexts without the required facets, including navigation, stay on the incumbent path. A future frozen protocol would still need to define its shared control, and retain the unchanged assessor/task-use, stability, navigation, timing, and resource gates before any execution could be considered.

Run the focused synthetic tests with `pytest tests/test_intent_ranking_coverage.py`.

The HTTP wire contract was checked against TypeSafe's [current API reference](https://docs.typesafe.ai/api) on 2026-10-08. Synthetic serialization does not establish provider acceptance, token fit for every natural pool, classification accuracy, or end-to-end usefulness. Those require separately registered live evidence.
