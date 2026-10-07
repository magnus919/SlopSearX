# Implementation requirements pending qualification

These are implementation plans, not delivered behavior or adoption approval. EXP-039 failed its original guard; EXP-040 and prospective confirmation must qualify the candidate first.

## One ranking boundary

SlopSearX already owns Jev ranking in its shared SearchService. Add optional `research_purpose` at the shared request and HTTP/MCP boundaries. The retrieval `q` and engine dispatch remain unchanged. When explicit purpose is supplied, the one existing Jev provider judges that text in its existing `state.query` field with the exact qualified source-selection rubric. Do not append an ignored field or create a GroktoCrawl-side Jev reranker. Ordinary query-only requests must retain their separately qualified behavior.

Purpose ranking must process the complete deduplicated eligible result pool in one call, subject to the qualified 80-card and 256 KB body limits. No cross-batch Score fusion or scored prefix with an unscored tail is permitted in this mode. Any invalid input, oversized pool/body, unsupported provider, missing or malformed response, model mismatch, timeout or provider failure preserves the complete incumbent order. No arbitrary source removal, fetched-content claim, follow-up-search action, or calibration claim follows from a Score.

## Shared state and compatibility

Validate explicit purpose at every ingress and in the shared service, with the qualified 4,096 UTF-8 byte boundary. Include normalized purpose identity in canonical response cache and singleflight keys; snapshot order must reflect the correct request. Version provider/rubric/body bounds and the host ordering policy. Preserve the existing deterministic sensitive-engine/provenance exclusions, keyless path, one-second deadline, response cap and exact permutation validation.

The SearXNG response remains a compatible superset; the additive request extension is optional. Portal impact review must preserve the purpose through pagination/refinement forms and safely escape it. Describe why selection changed and any complete-pool fallback using existing metadata conventions, without exposing raw secrets or internal operation details. HTTP GET/form POST, MCP generic search, and portal contracts need tests.

## GroktoCrawl X forwarding

Use a configured compatible-SlopSearX capability, disabled for arbitrary SearXNG endpoints. Forward explicit caller research purpose through SearXNGClient; preserve the generated retrieval query. Agent research uses its original caller objective, not generated queries or a synthesis system prompt. Cover sync, streaming, single-query and multi-query discovery paths. Caller text beyond the supported purpose bound must preserve retrieval rather than cause upstream validation failure or silently truncate the experimental representation. Direct generic search may supply its own explicit purpose. Existing answer-reranking semantics remain a separate feature.

## Delivery gates

After supported development and untouched confirmation, author the interface ADR with evidence and limits, then implement focused runtime PRs. Required verification includes real shared provider request projection, complete 44/80 membership, boundary rejection and unchanged full fallback, cache/singleflight purpose isolation, ordinary missing-purpose compatibility, no-key and sensitive-scope tests, and actual API/MCP/portal contracts. GroktoCrawl tests must verify compatible-provider forwarding and ordinary-SearXNG omission. Run repository-required local checks, all applicable CI and substantive review before merging. Verify exact merge SHAs and reconcile issue #516 and experimental roadmap docs. A repository merge is not an authorized deployment change; Hermes and existing services remain unchanged.
