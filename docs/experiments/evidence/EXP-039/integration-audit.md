# Integration boundary audit

This is a source audit and future implementation plan, not qualification or a runtime change.

## SlopSearX

At baseline `63735511226e9f06a1fc7a27d24f7d1d05df15e9`, the shared `SearchService` calls `_rerank_results(request.query, ranked, timeout)`. That method projects only `ranked[:MAX_CANDIDATES]`, validates an exact permutation, and appends the unscored tail. `MAX_CANDIDATES` is 40. The provider currently accepts a query and immutable candidate tuple; it has no caller-purpose field.

If qualified, an explicit purpose must reach shared request construction, the HTTP search boundary and generic MCP construction, as well as the provider. It must change cache/singleflight identity, retain ordinary requests unchanged, and preserve sensitive-scope/provenance exclusion, missing-key bypass, and complete original-order fallback for invalid, unavailable, oversized or timed-out results. Search engine selection remains deterministic service policy.

Complete-list qualification must test a single request containing the whole saved pool before considering partitioning. Root independently reconstructed the saved 44-card cardiac body with the actual service projection and incumbent contract: 68,510 bytes with the production JSON encoding, inside the current 128,000-byte bound. See `whole-pool-size-audit.json`; this is offline byte feasibility, not scoring or qualification. Larger/worst-case pools can exceed the bound and must fall back intact. Changing candidate-count limits does not authorize exceeding byte/deadline limits or dropping a tail. Chunked scores are not assumed comparable.

## GroktoCrawl X

Root independently inspected experimental source revision `3612affba3b51e7240a0e60d079499739052c0b1` after the parallel audit initially covered mainline only. There is no graph output in that checkout.

`agent-svc/agent/searxng_client.py` constructs the ordinary SearXNG GET parameters from the retrieval query. It presently has no ranking-purpose parameter. `agent-svc/agent/research/loop.py::_run_research_events` retains the original caller prompt. Discovery helpers in `research/discovery.py` dispatch original and generated queries; multi-query discovery requests `limit=None`, preserving the whole upstream pool before acquisition. These are distinct from synthesis instructions and subsequent page-contribution judgments.

A supported extension should retain the engine query and original research purpose separately, forward purpose only through an explicitly configured compatible SlopSearX integration, and cover single/multi-query plus streaming/nonstreaming paths. Ordinary SearXNG behavior must remain compatible. Do not infer purpose from categories, a generated query, or synthesis `system_prompt`, and do not create another Jev result reranker inside GroktoCrawl X. The existing answer reranking/retrieval modes have distinct downstream semantics and require compatibility coverage rather than replacement by assumption.

Implementation, portal regression coverage, architectural documentation where applicable, normal code review/CI and verified merges remain pending. No Hermes or deployment configuration was changed.
