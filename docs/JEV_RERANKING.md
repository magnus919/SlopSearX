# Optional semantic result reranking

Providing `TYPESAFE_API_KEY` now enables both the existing specialist routing
and Jev result reranking. Without the key, the configured deterministic
`ranking.strategy` (presence by default, or reciprocal rank fusion) continues
unchanged. A configured key is an opt-in to send the search query and bounded
result-card text to TypeSafe. Removing it and restarting the process restores
the deterministic path. No additional backend selector or persistent credential
configuration is required.

Routing and reranking are separate jobs. Explicit engine/category/media scopes
still bypass specialist *routing*, but their retrieved result cards can be
reranked. Scope previews do not retrieve cards and therefore do not rerank.
Policy and filter enforcement remain authoritative before retrieval. Searches
selecting any sensitive engine (for example HIBP or DeHashed), or returning cards
with sensitive-engine provenance, stay deterministic and send no cards to the reranker.

## Shared service behavior

1. Retrieve, filter and deduplicate through the existing service and configured
   presence/RRF ranker.
2. Pass the first **40** deterministic candidates to the optional asynchronous
   rerank provider, provided there are at least two candidates and the query
   fits the 4096-byte query bound.
3. The Jev backend asks one ordinary, ten-level `Score` question for each card
   in a single request to `https://api.typesafe.ai/v1/systemone`, pinning
   `jev-1.13.0`. It orders by the returned ordinal `score`, descending, with
   stable original-order ties. No custom nonlinear weights or fixed fusion.
4. The host accepts only an exact permutation of the supplied opaque IDs.
   Apply that order **across general and specialist source tiers** within the
   scored shortlist. Source tiers are retrieval-preference metadata, not a
   relevance override. The remaining unscored tail stays behind the scored
   pool in deterministic order. All original content, URLs, filters,
   provenance, source scores and metadata remain host-owned.
5. A configured reranker disables specialist promotion, even if advice fails
   or is skipped. Engine-selection confidence does not establish relevance of
   an engine's first result. Successful enhanced results receive positions
   consistent with the final order. Keyless searches preserve legacy behavior.

The shortlist bound limits work; it does not prove that it covers all relevant
results. The original numeric `score` field remains the source-fusion score,
not Jev relevance or confidence. `ranking_explanation` is
`semantic_shortlist_rerank` only after valid advice; HTTP, MCP, CLI, snapshots
and the portal use the same canonical result. The portal explains the semantic
order and the meaning of the displayed score. Invalid/unavailable advice keeps
the configured deterministic explanation and ordering, without specialist
promotion when the reranker is configured. Sensitive scopes still send no cards
and also receive no promotion in this enabled mode.

The first-40 selection remains deterministic and can miss specialists when
general feeds fill the pool. Global relevance competition is guaranteed only
inside the eligible scored pool, not over the complete retrieval corpus. No
forced source quota or automatic promotion is substituted for model relevance.

## Data, resilience and caching

The provider receives only the query and card-local IDs, title (256 UTF-8
bytes), URL (512 bytes, excluding userinfo, query parameters and fragment), and
snippet (1200 bytes). It receives no full pages, images, arbitrary structured
metadata, engine credentials or private authentication headers. Text limits
truncate UTF-8 without splitting characters. Queries over 4096 bytes skip
reranking instead of silently changing the intent. Total request size is capped
at 128,000 bytes. Candidate text is explicitly framed as untrusted evidence.
These instructions are a relevance defense, not a security classifier or proof
of resistance to every prompt injection.

Each process's startup provider permits at most **two** concurrent reranking
requests. One request per uncached/coalesced search, **zero automatic retries**.
The total advice deadline is **one second**, including semaphore wait and HTTP;
interactive searches use only their remaining dispatch deadline. Timeout,
transport/HTTP/JSON error, wrong model, missing/extra question IDs, malformed
scores, or an invalid permutation cause deterministic fallback. Cancellation
propagates and releases resources. If every search waiter disconnects, the
existing shared search-flight lifecycle cancels pending work. A provider might
still charge an already accepted request; cancellation cannot promise zero cost.

Successful advice is part of the existing full-response cache. Backend/model,
rubric version, host ordering-policy version and request-shaping identity separate enhanced, changed-provider
and keyless cache entries. Cache hits and simultaneous equivalent callers do
not repeat reranking. Prefer-fresh requests deliberately bypass reuse. Reranker
failures do not write the canonical response cache, allowing a later request to
try again. Cache identity contains no credential or candidate content.

Logs contain outcome, candidate count, latency and returned input/output token
usage only, never query, cards, API key, HTTP response body or exception text.
Returned usage is retained in aggregate logs even when answer validation fails.
Token telemetry is **not account billing**; multiplying by a published rate is
only a list-price estimate. The earlier research billing discrepancy remains
unresolved and this implementation makes no account-charge claim.

## Extending the backend

`slopsearx/rerank.py` defines immutable `RerankCandidate` and `RerankDecision`
types and a minimal `RerankProvider` protocol: `async rerank(query, candidates)`
returns a decision or `None`; `cache_identity()` versions all behavior affecting
cached order. `AppContext.rerank_provider` injects it independently of the
Jev-specific routing object. Another backend can implement this contract without
changing HTTP/MCP/portal paths. No other backend is implemented here.

The host enforces membership, pool/tail placement, shared-flight ownership,
cache separation and timeout. Backend implementations own scoring, transport,
credentials and concurrency. They must treat supplied cards as immutable and
return IDs only. This is a replaceable ordering seam, not a general autonomous
decision engine.

This maintainer-requested integration implements key-gated behavior while keeping
future backend replacement small. Earlier bounded research suggests pure Score
is worth exploring but does not establish real-search effectiveness or special
benefit from ten nonlinear weights. This change is not a claim that production
relevance will improve on every query. No deployment is performed.
