# Core, state, and adapter contract inventory

Baseline: `20109f9083babd014971951e8c02565ae51c2326`. Source observation only; Rust parity evidence remains pending. This companion indexes 288 machine-readable records in `core-contracts.json`, including all 51 imported and registered adapters.

## Adapter ownership and regression sources

| Adapter | Plan owner | Required regression sources |
| --- | --- | --- |
| `abuseipdb` | A11 | `tests/test_abuseipdb.py`, `tests/test_adapters.py`, `tests/test_greynoise.py`, `tests/test_jev.py`, `tests/test_otx.py`, `tests/test_upstream_contracts.py`, `tests/test_urlhaus.py` |
| `arxiv` | A06 | `tests/test_adapters.py`, `tests/test_arxiv_relevance.py`, `tests/test_jev.py`, `tests/test_openalex_adapter.py`, `tests/test_scholarly.py`, `tests/test_upstream_contracts.py` |
| `ashby` | A14 | `tests/test_adapters.py`, `tests/test_jev.py`, `tests/test_jobs_adapters.py`, `tests/test_upstream_contracts.py` |
| `brave` | A04 | `tests/test_adapters.py`, `tests/test_jev.py`, `tests/test_media_contract.py`, `tests/test_tier1_adapters.py`, `tests/test_upstream_contracts.py` |
| `censys` | A12 | `tests/test_adapters.py`, `tests/test_censys.py`, `tests/test_crtsh.py`, `tests/test_jev.py`, `tests/test_shodan.py`, `tests/test_upstream_contracts.py`, `tests/test_virustotal.py`, `tests/test_vulncheck.py` |
| `clinicaltrials` | A07 | `tests/test_adapters.py`, `tests/test_europepmc.py`, `tests/test_jev.py`, `tests/test_pubmed.py`, `tests/test_uniprot.py`, `tests/test_upstream_contracts.py` |
| `crates` | A01 | `tests/test_adapters.py`, `tests/test_jev.py`, `tests/test_new_engines.py`, `tests/test_upstream_contracts.py` |
| `crtsh` | A12 | `tests/test_adapters.py`, `tests/test_censys.py`, `tests/test_crtsh.py`, `tests/test_jev.py`, `tests/test_shodan.py`, `tests/test_upstream_contracts.py`, `tests/test_virustotal.py`, `tests/test_vulncheck.py` |
| `cve` | A10 | `tests/test_adapters.py`, `tests/test_cve.py`, `tests/test_epss.py`, `tests/test_exploitdb.py`, `tests/test_jev.py`, `tests/test_mitreattack.py`, `tests/test_nvd.py`, `tests/test_upstream_contracts.py` |
| `dehashed` | A13 | `tests/test_adapters.py`, `tests/test_dehashed.py`, `tests/test_hibp.py`, `tests/test_intelx.py`, `tests/test_jev.py`, `tests/test_mcp_security.py`, `tests/test_upstream_contracts.py` |
| `dockerhub` | A02 | `tests/test_adapters.py`, `tests/test_jev.py`, `tests/test_new_engines.py`, `tests/test_upstream_contracts.py` |
| `duckduckgo` | A05 | `tests/test_adapters.py`, `tests/test_duckduckgo_resilience.py`, `tests/test_google_adapter.py`, `tests/test_http_pooling.py`, `tests/test_jev.py`, `tests/test_upstream_contracts.py` |
| `edgar` | A08 | `tests/test_adapters.py`, `tests/test_jev.py`, `tests/test_new_engines.py`, `tests/test_payload.py`, `tests/test_upstream_contracts.py` |
| `epss` | A10 | `tests/test_adapters.py`, `tests/test_cve.py`, `tests/test_epss.py`, `tests/test_exploitdb.py`, `tests/test_jev.py`, `tests/test_mitreattack.py`, `tests/test_nvd.py`, `tests/test_upstream_contracts.py` |
| `europepmc` | A07 | `tests/test_adapters.py`, `tests/test_europepmc.py`, `tests/test_jev.py`, `tests/test_pubmed.py`, `tests/test_uniprot.py`, `tests/test_upstream_contracts.py` |
| `exploitdb` | A10 | `tests/test_adapters.py`, `tests/test_cve.py`, `tests/test_epss.py`, `tests/test_exploitdb.py`, `tests/test_jev.py`, `tests/test_mitreattack.py`, `tests/test_nvd.py`, `tests/test_upstream_contracts.py` |
| `fred` | A08 | `tests/test_adapters.py`, `tests/test_jev.py`, `tests/test_new_engines.py`, `tests/test_payload.py`, `tests/test_upstream_contracts.py` |
| `github` | A03 | `tests/test_adapters.py`, `tests/test_jev.py`, `tests/test_stackexchange_adapter.py`, `tests/test_upstream_contracts.py` |
| `google` | A05 | `tests/test_adapters.py`, `tests/test_duckduckgo_resilience.py`, `tests/test_google_adapter.py`, `tests/test_http_pooling.py`, `tests/test_jev.py`, `tests/test_upstream_contracts.py` |
| `greenhouse` | A14 | `tests/test_adapters.py`, `tests/test_jev.py`, `tests/test_jobs_adapters.py`, `tests/test_upstream_contracts.py` |
| `greynoise` | A11 | `tests/test_abuseipdb.py`, `tests/test_adapters.py`, `tests/test_greynoise.py`, `tests/test_jev.py`, `tests/test_otx.py`, `tests/test_upstream_contracts.py`, `tests/test_urlhaus.py` |
| `hackernews` | A03 | `tests/test_adapters.py`, `tests/test_jev.py`, `tests/test_stackexchange_adapter.py`, `tests/test_upstream_contracts.py` |
| `hibp` | A13 | `tests/test_adapters.py`, `tests/test_dehashed.py`, `tests/test_hibp.py`, `tests/test_intelx.py`, `tests/test_jev.py`, `tests/test_mcp_security.py`, `tests/test_upstream_contracts.py` |
| `huggingface` | A03 | `tests/test_adapters.py`, `tests/test_jev.py`, `tests/test_stackexchange_adapter.py`, `tests/test_upstream_contracts.py` |
| `intelx` | A13 | `tests/test_adapters.py`, `tests/test_dehashed.py`, `tests/test_hibp.py`, `tests/test_intelx.py`, `tests/test_jev.py`, `tests/test_mcp_security.py`, `tests/test_upstream_contracts.py` |
| `internetarchive` | A09 | `tests/test_adapters.py`, `tests/test_internetarchive_adapter.py`, `tests/test_jev.py`, `tests/test_media_contract.py`, `tests/test_openlibrary_url_contract.py`, `tests/test_upstream_contracts.py` |
| `lever` | A14 | `tests/test_adapters.py`, `tests/test_jev.py`, `tests/test_jobs_adapters.py`, `tests/test_upstream_contracts.py` |
| `mitreattack` | A10 | `tests/test_adapters.py`, `tests/test_cve.py`, `tests/test_epss.py`, `tests/test_exploitdb.py`, `tests/test_jev.py`, `tests/test_mitreattack.py`, `tests/test_nvd.py`, `tests/test_upstream_contracts.py` |
| `musicbrainz` | A09 | `tests/test_adapters.py`, `tests/test_internetarchive_adapter.py`, `tests/test_jev.py`, `tests/test_media_contract.py`, `tests/test_openlibrary_url_contract.py`, `tests/test_upstream_contracts.py` |
| `nominatim` | A09 | `tests/test_adapters.py`, `tests/test_internetarchive_adapter.py`, `tests/test_jev.py`, `tests/test_media_contract.py`, `tests/test_openlibrary_url_contract.py`, `tests/test_upstream_contracts.py` |
| `npm` | A01 | `tests/test_adapters.py`, `tests/test_jev.py`, `tests/test_new_engines.py`, `tests/test_upstream_contracts.py` |
| `nvd` | A10 | `tests/test_adapters.py`, `tests/test_cve.py`, `tests/test_epss.py`, `tests/test_exploitdb.py`, `tests/test_jev.py`, `tests/test_mitreattack.py`, `tests/test_nvd.py`, `tests/test_upstream_contracts.py` |
| `openalex` | A06 | `tests/test_adapters.py`, `tests/test_arxiv_relevance.py`, `tests/test_jev.py`, `tests/test_openalex_adapter.py`, `tests/test_scholarly.py`, `tests/test_upstream_contracts.py` |
| `openfda` | A08 | `tests/test_adapters.py`, `tests/test_jev.py`, `tests/test_new_engines.py`, `tests/test_payload.py`, `tests/test_upstream_contracts.py` |
| `openlibrary` | A09 | `tests/test_adapters.py`, `tests/test_internetarchive_adapter.py`, `tests/test_jev.py`, `tests/test_media_contract.py`, `tests/test_openlibrary_url_contract.py`, `tests/test_upstream_contracts.py` |
| `otx` | A11 | `tests/test_abuseipdb.py`, `tests/test_adapters.py`, `tests/test_greynoise.py`, `tests/test_jev.py`, `tests/test_otx.py`, `tests/test_upstream_contracts.py`, `tests/test_urlhaus.py` |
| `oyez` | A08 | `tests/test_adapters.py`, `tests/test_jev.py`, `tests/test_new_engines.py`, `tests/test_payload.py`, `tests/test_upstream_contracts.py` |
| `pubchem` | A07 | `tests/test_adapters.py`, `tests/test_europepmc.py`, `tests/test_jev.py`, `tests/test_pubmed.py`, `tests/test_uniprot.py`, `tests/test_upstream_contracts.py` |
| `pubmed` | A07 | `tests/test_adapters.py`, `tests/test_europepmc.py`, `tests/test_jev.py`, `tests/test_pubmed.py`, `tests/test_uniprot.py`, `tests/test_upstream_contracts.py` |
| `pypi` | A02 | `tests/test_adapters.py`, `tests/test_jev.py`, `tests/test_new_engines.py`, `tests/test_upstream_contracts.py` |
| `reddit` | A04 | `tests/test_adapters.py`, `tests/test_jev.py`, `tests/test_media_contract.py`, `tests/test_tier1_adapters.py`, `tests/test_upstream_contracts.py` |
| `rubygems` | A02 | `tests/test_adapters.py`, `tests/test_jev.py`, `tests/test_new_engines.py`, `tests/test_upstream_contracts.py` |
| `semanticscholar` | A06 | `tests/test_adapters.py`, `tests/test_arxiv_relevance.py`, `tests/test_jev.py`, `tests/test_openalex_adapter.py`, `tests/test_scholarly.py`, `tests/test_upstream_contracts.py` |
| `shodan` | A12 | `tests/test_adapters.py`, `tests/test_censys.py`, `tests/test_crtsh.py`, `tests/test_jev.py`, `tests/test_shodan.py`, `tests/test_upstream_contracts.py`, `tests/test_virustotal.py`, `tests/test_vulncheck.py` |
| `stackexchange` | A03 | `tests/test_adapters.py`, `tests/test_jev.py`, `tests/test_stackexchange_adapter.py`, `tests/test_upstream_contracts.py` |
| `tmdb` | A09 | `tests/test_adapters.py`, `tests/test_internetarchive_adapter.py`, `tests/test_jev.py`, `tests/test_media_contract.py`, `tests/test_openlibrary_url_contract.py`, `tests/test_upstream_contracts.py` |
| `uniprot` | A07 | `tests/test_adapters.py`, `tests/test_europepmc.py`, `tests/test_jev.py`, `tests/test_pubmed.py`, `tests/test_uniprot.py`, `tests/test_upstream_contracts.py` |
| `urlhaus` | A11 | `tests/test_abuseipdb.py`, `tests/test_adapters.py`, `tests/test_greynoise.py`, `tests/test_jev.py`, `tests/test_otx.py`, `tests/test_upstream_contracts.py`, `tests/test_urlhaus.py` |
| `virustotal` | A12 | `tests/test_adapters.py`, `tests/test_censys.py`, `tests/test_crtsh.py`, `tests/test_jev.py`, `tests/test_shodan.py`, `tests/test_upstream_contracts.py`, `tests/test_virustotal.py`, `tests/test_vulncheck.py` |
| `vulncheck` | A12 | `tests/test_adapters.py`, `tests/test_censys.py`, `tests/test_crtsh.py`, `tests/test_jev.py`, `tests/test_shodan.py`, `tests/test_upstream_contracts.py`, `tests/test_virustotal.py`, `tests/test_vulncheck.py` |
| `wikipedia` | A04 | `tests/test_adapters.py`, `tests/test_jev.py`, `tests/test_media_contract.py`, `tests/test_tier1_adapters.py`, `tests/test_upstream_contracts.py` |

## Core contracts and environment reads

The AST census covers 68 direct `os.environ.get`, `os.getenv`, `environ.get`, and `os.environ[]` reads in `slopsearx/**/*.py` and `engines/*.py`. Each `env.read.*` record retains the raw key/default expressions and source location. Separate `config.helper_env.*` records expand routing, gateway and portal secret helper calls. `load_config` also iterates `os.environ.items()` for `ENGINE_*` and `SEARCH_*`; per-key config records specify that mapping.

### `config.layering` (F06)

Built-in dataclasses/default engine map, optional YAML at /etc/slopsearx/config.yaml, then ENGINE_*/SEARCH_* process environment. File engine entries merge fieldwise over listed built-ins; extra named engines may be declared. Env only mutates existing configured engines; unknown ENGINE_* names are ignored.

Source: `slopsearx/config.py:load_config`, `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:_load_env_overrides`, `slopsearx/config.py:_apply_env_overrides`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses defaults; empty YAML leaves defaults; explicit null/empty file fields are not interchangeable. Unknown env engine is ignored; malformed integer fails loading. YAML parse errors are not a missing file.

### `config.engine_fields` (F06)

EngineEntry defaults: enabled=true, base_url="", type=api, timeout_ms=5000, max_results=10, rate_limit=null, weight=1.0, api_key/api_secret/user_agent/categories/proxy_pool/scrape_proxy_url=null. ENGINE_<NAME>_<FIELD> overrides typed existing entry; categories comma split, empty string becomes []. HF_TOKEN supplies Hugging Face key only when explicit ENGINE_HUGGINGFACE_API_KEY absent. Built-in Europe PMC and Internet Archive default disabled.

Source: `slopsearx/config.py:EngineEntry`, `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:_load_env_overrides`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`, `tests/test_capabilities.py`.
Distinctions: Missing credential is not empty credential; disabled is not ineligible because of auth. Empty categories override is not missing categories.

### `config.global_fields` (F06)

CacheConfig ttl_seconds=300/max_result_sets=10000/revalidate_on_hit=false; RankingConfig strategy=presence; RoutingConfig enabled=true, topics/fallback/budget=null; SearchConfig formats=[html,csv,json,rss] with YAML additive; default_engines=[brave,wikipedia], log_level=INFO, enable_suggestions=false. SEARCH_CACHE_TTL_SECONDS positive int, SEARCH_LOG_LEVEL uppercase, SEARCH_DEFAULT_ENGINES comma split, SEARCH_FORMATS lowercased nonempty list.

Source: `slopsearx/config.py:CacheConfig`, `slopsearx/config.py:RankingConfig`, `slopsearx/config.py:RoutingConfig`, `slopsearx/config.py:SearchConfig`, `slopsearx/config.py:Config`, `slopsearx/config.py:_apply_env_overrides`.
Regression: `tests/test_config.py`, `tests/test_server.py`.
Distinctions: YAML null versus absent matters; SEARCH_DEFAULT_ENGINES does not drop empty comma components while SEARCH_FORMATS does. Invalid TTL raises ValueError.

### `config.runtime` (F06)

build_context reads MAX_CONCURRENT_ENGINES=10, PER_CLIENT_REQUESTS=30, PER_CLIENT_WINDOW_SECONDS=60, FAIL_CLOSED=false, VALKEY_URL, FEATURE_EMPTY_SCRAPE_DIAGNOSTICS, FEATURE_BRAVE_CATEGORY_ROUTING. Cache separately reads SEARCH_CACHE_TTL_SECONDS=3600, PARTIAL=30 capped 300, NEGATIVE=60. Routing budget, Jev threshold/timeout/cache, rerank key, health staleness and MCP policy read their own env paths.

Source: `slopsearx/service.py:build_context`, `slopsearx/cache.py:SearchCache.__init__`, `slopsearx/routing.py:load_routing_budget`, `slopsearx/jev.py:JevSpecialistRouter.__init__`, `slopsearx/capabilities.py:load_mcp_policy`.
Regression: `tests/test_service.py`, `tests/test_cache.py`, `tests/test_routing_service.py`, `tests/test_jev.py`, `tests/test_mcp_security.py`.
Distinctions: Config CacheConfig.ttl_seconds=300 differs from SearchCache runtime default 3600; preserve actual caller path. Missing VALKEY_URL is no shared store; invalid runtime settings and empty credentials differ from defaults.

### `config.catalog_policy` (F06)

CapabilityCatalog derives live eligibility/auth/filters/result types/cost/observed health; supported_filters is consumption hint, enforced_filters alone grants truthful enforcement. MCPPolicy file section then MCP_* env; specialist grants do not grant sensitive access, MCP_TARGETED_SENSITIVE_ALLOWED gates hibp/dehashed uniformly.

Source: `slopsearx/capabilities.py:CapabilityCatalog`, `slopsearx/capabilities.py:EngineCapability`, `slopsearx/capabilities.py:load_mcp_policy`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_capability_catalog.py`, `tests/test_mcp_security.py`.
Distinctions: Unknown auth is not none; missing health is not observed healthy; explicit sensitive list with non-sensitive entries fails atomically.

### `core.model_serialization` (F05)

SearchResult typed fields include set[str] engines, tier, position, media/domain payloads. Serialization sorts engine provenance; loader accepts list and legacy stringified set. SearchResponse and EngineOutcome round-trip typed fields; no default=str for durable typed fields.

Source: `slopsearx/adapter.py:SearchResult`, `slopsearx/service.py:search_result_to_dict`, `slopsearx/service.py:search_result_from_dict`, `slopsearx/service.py:search_response_to_payload`.
Regression: `tests/test_result_contract.py`, `tests/test_service.py`, `tests/test_cache.py`.
Distinctions: Absent optional fields, explicit null and empty arrays/sets are distinct; malformed set must not silently stringify.

### `core.scope` (F09)

Explicit engines override categories; categories OR; media intent seeds advertised sources; topic match, tier1 fallback, all-active fallback. Unknown explicit engines produce warnings/exclusions; sensitive engines do not enter automatic routes.

Source: `slopsearx/service.py:ScopeResolver.resolve`, `slopsearx/service.py:ScopeResolver._apply_routing`, `slopsearx/capabilities.py:CapabilityCatalog`.
Regression: `tests/test_service.py`, `tests/test_routing_service.py`, `tests/test_capabilities.py`.
Distinctions: Explicit empty engine list is not unknown named engine; no eligible source and unsupported media coverage remain observable.

### `core.filter_enforcement` (F09)

Per filter report exactly {requested,status,reason,enforced_by}; statuses enforced/partially_enforced/unsupported/rejected. Only audited enforced_filters supplies upstream/local layer. Invalid local time_range does not count. Strict SafeSearch rejects predispatch.

Source: `slopsearx/filters.py:resolve_filter_enforcement`, `slopsearx/filters.py:engine_filter_layer`, `slopsearx/mcp/tools.py:_enforce_policy`.
Regression: `tests/test_mcp_enforcement.py`, `tests/test_filter_semantics.py`, `tests/test_mcp_security.py`.
Distinctions: Unsupported differs from absent request and rejected; partial differs from full; supported_filters does not prove enforcement.

### `core.execution` (F14)

SearchService validates, resolves, rate limits, coalesces same-key flights, queues under semaphore, applies deadline including queue time, dispatches adapters, returns partial outcomes, optional suggestions and cache views.

Source: `slopsearx/service.py:SearchService.search`, `slopsearx/service.py:SearchService._execute_search`, `slopsearx/service.py:SearchService._gather_with_deadline`.
Regression: `tests/test_service.py`, `tests/test_concurrency.py`, `tests/test_interactive_deadline.py`.
Distinctions: Timeout/cancel/error/empty OK/partial response differ; waiter cancellation must not cancel another caller flight.

### `core.presence_rank` (F11)

Default ranker normalizes URL/tracking, deduplicates, scores by number of engines, preserves minimum tier, sorts (tier,-score), assigns 1-based position; per-engine budget supported.

Source: `slopsearx/merger.py:PresenceRanker.rank`, `slopsearx/merger.py:_normalise_url`.
Regression: `tests/test_merger.py`, `tests/test_rank_fusion.py`.
Distinctions: Empty feed differs from failed engine; duplicate URL adds provenance, not another result.

### `core.rrf_rank` (F11)

Opt-in reciprocal_rank_fusion sums 1/(60+feed rank) once per engine/normalized URL, retains min tier, deterministic URL tie break and 1-based positions; unknown strategy falls back to presence.

Source: `slopsearx/merger.py:ReciprocalRankFusionRanker.rank`, `slopsearx/merger.py:create_ranker`.
Regression: `tests/test_rank_fusion.py`, `tests/test_merger.py`.
Distinctions: No overlap differs from duplicate within one feed; missing strategy uses default presence.

### `core.scholarly` (F11)

Scholarly identity/version grouping preserves attribution and work/version boundaries; refuses lossy merges and bounds work groups.

Source: `slopsearx/scholarly.py:group_publications`, `slopsearx/publication_metadata.py`.
Regression: `tests/test_scholarly.py`, `tests/test_scholarly_arxiv_html_identity.py`, `tests/test_europepmc.py`.
Distinctions: Missing DOI/arXiv identity is not equivalent to equal title; preprint notices and attribution survive.

### `core.jev_routing` (F12)

Jev routing roles complete across built-ins; general/broad_support/specialist cards. Optional specialist decision uses threshold 0.65, 1000ms timeout and bounded cache; eligibility/policy/health gates remain authoritative.

Source: `slopsearx/jev.py:ROUTING_ROLES`, `slopsearx/jev.py:validate_routing_cards`, `slopsearx/jev.py:JevSpecialistRouter`.
Regression: `tests/test_jev.py`, `tests/test_routing_eval.py`, `tests/test_routing_service.py`.
Distinctions: Unavailable/timeout/low score falls back deterministically; model advice does not grant authority or prove relevance.

### `core.rerank_advisory` (F15)

Optional semantic rerank accepts permutation of supplied IDs only; bounded text and global shortlist cannot promote outside allowed set. Advisories derive at read boundary without changing canonical cached evidence.

Source: `slopsearx/service.py:SearchService._rerank_results`, `slopsearx/rerank.py`, `slopsearx/advisories.py`.
Regression: `tests/test_rerank.py`, `tests/test_advisories.py`.
Distinctions: No key/unavailable provider means deterministic fallback; omitted advisory differs from empty advisory; sensitive results excluded.

### `core.cache_key` (F13)

search:v2 SHA256 compact JSON key over trimmed query (punctuation and percent preserved), language, safesearch, sorted categories/engines, page, time_range, media_type, date bounds and resolved time window. Cache stores canonical unsliced response, view per request; include/max_results/freshness do not alter key.

Source: `slopsearx/cache.py:cache_key`, `slopsearx/service.py:SearchService._read_cache`, `slopsearx/service.py:SearchService._view_for_request`.
Regression: `tests/test_cache.py`, `tests/test_cache_integration.py`, `tests/test_mcp_snapshots.py`.
Distinctions: Miss/corrupt/unavailable all result in no cache payload; negative _error sentinel differs from empty results.

### `core.cache_ttl` (F13)

General TTL 3600s; news capped 300s; partial capped by SEARCH_CACHE_PARTIAL_TTL_SECONDS default 30 max 300; negative default 60; answer cache query-only answer:v2 key. Connection failures retry after 5s and do not fail search.

Source: `slopsearx/cache.py:_ttl_for_query`, `slopsearx/cache.py:SearchCache.connect`, `slopsearx/cache.py:SearchCache.set_error`.
Regression: `tests/test_cache.py`, `tests/test_cache_integration.py`.
Distinctions: Invalid nonpositive TTL raises; Valkey unavailable best-effort cache miss differs from durable state unavailable.

### `core.rate_health` (F10)

Distributed sliding window and local token bucket with upstream cooldown 30s bounded 1..300, circuit/observed-health handling; request admission and per-engine availability affect scope.

Source: `slopsearx/ratelimit.py:ValkeySlidingWindow`, `slopsearx/ratelimit.py:RateLimiter`, `slopsearx/capabilities.py:build_engine_health`.
Regression: `tests/test_ratelimit.py`, `tests/test_observed_health.py`, `tests/test_interactive_deadline.py`.
Distinctions: Synthetic eligibility is not observed health; missing/stale health differs from healthy; limit rejection differs from upstream rate limit.

### `config.key.engine.enabled` (F06)

Config key `engine.enabled` defaults to true; YAML key is `engines.<name>.enabled`. Process environment key `ENGINE_<NAME>_ENABLED` has higher precedence.

Source: `slopsearx/config.py:EngineEntry`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.engine.base_url` (F06)

Config key `engine.base_url` defaults to ""; YAML key is `engines.<name>.base_url`. Process environment key `ENGINE_<NAME>_BASE_URL` has higher precedence.

Source: `slopsearx/config.py:EngineEntry`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.engine.type` (F06)

Config key `engine.type` defaults to "api"; YAML key is `engines.<name>.type`. Process environment key `ENGINE_<NAME>_TYPE` has higher precedence.

Source: `slopsearx/config.py:EngineEntry`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.engine.timeout_ms` (F06)

Config key `engine.timeout_ms` defaults to 5000; YAML key is `engines.<name>.timeout_ms`. Process environment key `ENGINE_<NAME>_TIMEOUT_MS` has higher precedence.

Source: `slopsearx/config.py:EngineEntry`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.engine.max_results` (F06)

Config key `engine.max_results` defaults to 10; YAML key is `engines.<name>.max_results`. Process environment key `ENGINE_<NAME>_MAX_RESULTS` has higher precedence.

Source: `slopsearx/config.py:EngineEntry`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.engine.rate_limit` (F06)

Config key `engine.rate_limit` defaults to null; YAML key is `engines.<name>.rate_limit`. Process environment key `ENGINE_<NAME>_RATE_LIMIT` has higher precedence.

Source: `slopsearx/config.py:EngineEntry`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.engine.weight` (F06)

Config key `engine.weight` defaults to 1.0; YAML key is `engines.<name>.weight`. Process environment key `ENGINE_<NAME>_WEIGHT` has higher precedence.

Source: `slopsearx/config.py:EngineEntry`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.engine.api_key` (F06)

Config key `engine.api_key` defaults to null; YAML key is `engines.<name>.api_key`. Process environment key `ENGINE_<NAME>_API_KEY` has higher precedence.

Source: `slopsearx/config.py:EngineEntry`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.engine.api_secret` (F06)

Config key `engine.api_secret` defaults to null; YAML key is `engines.<name>.api_secret`. Process environment key `ENGINE_<NAME>_API_SECRET` has higher precedence.

Source: `slopsearx/config.py:EngineEntry`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.engine.user_agent` (F06)

Config key `engine.user_agent` defaults to null; YAML key is `engines.<name>.user_agent`. Process environment key `ENGINE_<NAME>_USER_AGENT` has higher precedence.

Source: `slopsearx/config.py:EngineEntry`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.engine.categories` (F06)

Config key `engine.categories` defaults to null; YAML key is `engines.<name>.categories`. Process environment key `ENGINE_<NAME>_CATEGORIES` has higher precedence.

Source: `slopsearx/config.py:EngineEntry`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.engine.proxy_pool` (F06)

Config key `engine.proxy_pool` defaults to null; YAML key is `engines.<name>.proxy_pool`. Process environment key `ENGINE_<NAME>_PROXY_POOL` has higher precedence.

Source: `slopsearx/config.py:EngineEntry`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.engine.scrape_proxy_url` (F06)

Config key `engine.scrape_proxy_url` defaults to null; YAML key is `engines.<name>.scrape_proxy_url`. Process environment key `ENGINE_<NAME>_SCRAPE_PROXY_URL` has higher precedence.

Source: `slopsearx/config.py:EngineEntry`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.cache.ttl_seconds` (F06)

Config key `cache.ttl_seconds` defaults to 300; YAML key is `cache.ttl_seconds`. Process environment key `SEARCH_CACHE_TTL_SECONDS` has higher precedence.

Source: `slopsearx/config.py:CacheConfig`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.cache.max_result_sets` (F06)

Config key `cache.max_result_sets` defaults to 10000; YAML key is `cache.max_result_sets`. No direct generic env override is implemented for this key.

Source: `slopsearx/config.py:CacheConfig`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.cache.revalidate_on_hit` (F06)

Config key `cache.revalidate_on_hit` defaults to false; YAML key is `cache.revalidate_on_hit`. No direct generic env override is implemented for this key.

Source: `slopsearx/config.py:CacheConfig`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.ranking.strategy` (F06)

Config key `ranking.strategy` defaults to "presence"; YAML key is `ranking.strategy`. No direct generic env override is implemented for this key.

Source: `slopsearx/config.py:RankingConfig`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.routing.enabled` (F06)

Config key `routing.enabled` defaults to true; YAML key is `routing.enabled`. No direct generic env override is implemented for this key.

Source: `slopsearx/config.py:RoutingConfig`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.routing.topics` (F06)

Config key `routing.topics` defaults to null; YAML key is `routing.topics`. No direct generic env override is implemented for this key.

Source: `slopsearx/config.py:RoutingConfig`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.routing.fallback` (F06)

Config key `routing.fallback` defaults to null; YAML key is `routing.fallback`. No direct generic env override is implemented for this key.

Source: `slopsearx/config.py:RoutingConfig`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.routing.budget` (F06)

Config key `routing.budget` defaults to null; YAML key is `routing.budget`. No direct generic env override is implemented for this key.

Source: `slopsearx/config.py:RoutingConfig`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.search.formats` (F06)

Config key `search.formats` defaults to ["html", "csv", "json", "rss"]; YAML key is `search.formats`. Process environment key `SEARCH_FORMATS` has higher precedence.

Source: `slopsearx/config.py:SearchConfig`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.global.default_engines` (F06)

Config key `global.default_engines` defaults to ["brave", "wikipedia"]; YAML key is `default_engines`. Process environment key `SEARCH_DEFAULT_ENGINES` has higher precedence.

Source: `slopsearx/config.py:Config`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.global.log_level` (F06)

Config key `global.log_level` defaults to "INFO"; YAML key is `log_level`. Process environment key `SEARCH_LOG_LEVEL` has higher precedence.

Source: `slopsearx/config.py:Config`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.key.global.enable_suggestions` (F06)

Config key `global.enable_suggestions` defaults to false; YAML key is `enable_suggestions`. No direct generic env override is implemented for this key.

Source: `slopsearx/config.py:Config`, `slopsearx/config.py:load_config`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Missing uses default; explicit null and empty input retain their current loader behavior and require fixture capture. Invalid typed env values may raise during loading.

### `config.engine_default.abuseipdb` (F06)

Built-in engine `abuseipdb` config overlay on EngineEntry defaults: {"base_url": "https://api.abuseipdb.com/api/v2", "max_results": 5, "rate_limit": 1, "timeout_ms": 10000, "type": "api", "weight": 0.6}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/abuseipdb.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.arxiv` (F06)

Built-in engine `arxiv` config overlay on EngineEntry defaults: {"base_url": "https://export.arxiv.org/api/query", "max_results": 5, "rate_limit": 0.33, "timeout_ms": 10000, "type": "api", "weight": 0.8}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/arxiv.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.ashby` (F06)

Built-in engine `ashby` config overlay on EngineEntry defaults: {"base_url": "https://jobs.ashbyhq.com", "max_results": 10, "rate_limit": 1, "timeout_ms": 5000, "type": "api", "weight": 0.5}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/ashby.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.brave` (F06)

Built-in engine `brave` config overlay on EngineEntry defaults: {"base_url": "https://api.search.brave.com/res/v1/web/search", "max_results": 20, "rate_limit": 15, "timeout_ms": 5000, "type": "api", "weight": 1.0}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/brave.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.censys` (F06)

Built-in engine `censys` config overlay on EngineEntry defaults: {"base_url": "https://search.censys.io/api/v2", "max_results": 10, "rate_limit": 0.5, "timeout_ms": 10000, "type": "api", "weight": 0.7}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/censys.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.cve` (F06)

Built-in engine `cve` config overlay on EngineEntry defaults: {"base_url": "https://cveawg.mitre.org/api", "max_results": 5, "rate_limit": 5, "timeout_ms": 10000, "type": "api", "weight": 0.7}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/cve.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.dehashed` (F06)

Built-in engine `dehashed` config overlay on EngineEntry defaults: {"base_url": "https://dehashed.com/api/v1", "max_results": 10, "rate_limit": 1, "timeout_ms": 10000, "type": "api", "weight": 0.5}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/dehashed.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.duckduckgo` (F06)

Built-in engine `duckduckgo` config overlay on EngineEntry defaults: {"base_url": "https://html.duckduckgo.com/html/", "max_results": 10, "timeout_ms": 10000, "type": "scrape", "weight": 0.6}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/duckduckgo.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.europepmc` (F06)

Built-in engine `europepmc` config overlay on EngineEntry defaults: {"base_url": "https://www.ebi.ac.uk/europepmc/webservices/rest", "enabled": false, "max_results": 10, "rate_limit": 0.5, "timeout_ms": 5000, "type": "api", "weight": 0.8}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/europepmc.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.github` (F06)

Built-in engine `github` config overlay on EngineEntry defaults: {"base_url": "https://api.github.com", "max_results": 5, "rate_limit": 0.5, "timeout_ms": 5000, "type": "api", "weight": 0.8}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/github.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.google` (F06)

Built-in engine `google` config overlay on EngineEntry defaults: {"base_url": "https://www.google.com/search", "max_results": 10, "timeout_ms": 10000, "type": "scrape", "weight": 0.5}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/google.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.greenhouse` (F06)

Built-in engine `greenhouse` config overlay on EngineEntry defaults: {"base_url": "https://boards-api.greenhouse.io/v1/boards", "max_results": 10, "rate_limit": 1, "timeout_ms": 5000, "type": "api", "weight": 0.5}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/greenhouse.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.hackernews` (F06)

Built-in engine `hackernews` config overlay on EngineEntry defaults: {"base_url": "https://hn.algolia.com/api/v1/search", "max_results": 5, "rate_limit": 10, "timeout_ms": 3000, "type": "api", "weight": 0.7}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/hackernews.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.hibp` (F06)

Built-in engine `hibp` config overlay on EngineEntry defaults: {"base_url": "https://haveibeenpwned.com/api/v3", "max_results": 10, "rate_limit": 1, "timeout_ms": 10000, "type": "api", "weight": 0.6}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/hibp.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.huggingface` (F06)

Built-in engine `huggingface` config overlay on EngineEntry defaults: {"base_url": "https://huggingface.co/api", "max_results": 5, "rate_limit": 1, "timeout_ms": 5000, "type": "api", "weight": 0.7}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/huggingface.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.intelx` (F06)

Built-in engine `intelx` config overlay on EngineEntry defaults: {"base_url": "https://2.intelx.io", "max_results": 10, "rate_limit": 1, "timeout_ms": 15000, "type": "api", "weight": 0.5}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/intelx.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.internetarchive` (F06)

Built-in engine `internetarchive` config overlay on EngineEntry defaults: {"base_url": "https://archive.org", "enabled": false, "max_results": 10, "rate_limit": 5, "timeout_ms": 10000, "type": "api", "weight": 0.6}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/internetarchive.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.lever` (F06)

Built-in engine `lever` config overlay on EngineEntry defaults: {"base_url": "https://api.lever.co/v0/postings", "max_results": 10, "rate_limit": 1, "timeout_ms": 5000, "type": "api", "weight": 0.5}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/lever.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.nvd` (F06)

Built-in engine `nvd` config overlay on EngineEntry defaults: {"base_url": "https://services.nvd.nist.gov/rest/json/cves/2.0", "max_results": 10, "rate_limit": 0.17, "timeout_ms": 10000, "type": "api", "weight": 0.7}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/nvd.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.openalex` (F06)

Built-in engine `openalex` config overlay on EngineEntry defaults: {"base_url": "https://api.openalex.org", "max_results": 10, "rate_limit": 10, "timeout_ms": 5000, "type": "api", "weight": 0.8}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/openalex.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.otx` (F06)

Built-in engine `otx` config overlay on EngineEntry defaults: {"base_url": "https://otx.alienvault.com/api/v1", "max_results": 10, "rate_limit": 5, "timeout_ms": 10000, "type": "api", "weight": 0.7}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/otx.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.reddit` (F06)

Built-in engine `reddit` config overlay on EngineEntry defaults: {"base_url": "https://www.reddit.com", "max_results": 10, "rate_limit": 1, "timeout_ms": 5000, "type": "api", "weight": 0.7}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/reddit.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.semanticscholar` (F06)

Built-in engine `semanticscholar` config overlay on EngineEntry defaults: {"base_url": "https://api.semanticscholar.org/graph/v1/paper/search", "max_results": 5, "rate_limit": 1, "timeout_ms": 5000, "type": "api", "weight": 0.8}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/semanticscholar.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.shodan` (F06)

Built-in engine `shodan` config overlay on EngineEntry defaults: {"base_url": "https://api.shodan.io", "max_results": 10, "rate_limit": 1, "timeout_ms": 10000, "type": "api", "weight": 0.7}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/shodan.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.stackexchange` (F06)

Built-in engine `stackexchange` config overlay on EngineEntry defaults: {"base_url": "https://api.stackexchange.com/2.3", "max_results": 10, "rate_limit": 30, "timeout_ms": 5000, "type": "api", "weight": 0.9}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/stackexchange.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.virustotal` (F06)

Built-in engine `virustotal` config overlay on EngineEntry defaults: {"base_url": "https://www.virustotal.com/api/v3", "max_results": 10, "rate_limit": 4, "timeout_ms": 10000, "type": "api", "weight": 0.7}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/virustotal.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.vulncheck` (F06)

Built-in engine `vulncheck` config overlay on EngineEntry defaults: {"base_url": "https://api.vulncheck.com/v3", "max_results": 5, "rate_limit": 5, "timeout_ms": 10000, "type": "api", "weight": 0.6}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/vulncheck.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.engine_default.wikipedia` (F06)

Built-in engine `wikipedia` config overlay on EngineEntry defaults: {"base_url": "https://en.wikipedia.org/w/api.php", "max_results": 3, "rate_limit": 200, "timeout_ms": 3000, "type": "api", "weight": 0.9}.

Source: `slopsearx/config.py:_DEFAULT_ENGINES`, `slopsearx/config.py:load_config`, `engines/wikipedia.py`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: Absent YAML uses this overlay; explicit file field overrides it; disabled=false/true and missing enabled are distinct.

### `config.mcp.max_query_length` (F06)

MCPPolicy `max_query_length` defaults to 500; YAML `mcp.max_query_length` then env `MCP_MAX_QUERY_LENGTH`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.max_results` (F06)

MCPPolicy `max_results` defaults to 50; YAML `mcp.max_results` then env `MCP_MAX_RESULTS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.snapshot_ttl_seconds` (F06)

MCPPolicy `snapshot_ttl_seconds` defaults to 3600; YAML `mcp.snapshot_ttl_seconds` then env `MCP_SNAPSHOT_TTL_SECONDS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.job_max_queries` (F06)

MCPPolicy `job_max_queries` defaults to 20; YAML `mcp.job_max_queries` then env `MCP_JOB_MAX_QUERIES`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.job_max_engines_per_query` (F06)

MCPPolicy `job_max_engines_per_query` defaults to 10; YAML `mcp.job_max_engines_per_query` then env `MCP_JOB_MAX_ENGINES_PER_QUERY`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.job_max_results` (F06)

MCPPolicy `job_max_results` defaults to 500; YAML `mcp.job_max_results` then env `MCP_JOB_MAX_RESULTS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.job_default_deadline_seconds` (F06)

MCPPolicy `job_default_deadline_seconds` defaults to 600; YAML `mcp.job_default_deadline_seconds` then env `MCP_JOB_DEFAULT_DEADLINE_SECONDS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.job_lease_ttl_seconds` (F06)

MCPPolicy `job_lease_ttl_seconds` defaults to 60; YAML `mcp.job_lease_ttl_seconds` then env `MCP_JOB_LEASE_TTL_SECONDS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.job_poll_interval_seconds` (F06)

MCPPolicy `job_poll_interval_seconds` defaults to 1.0; YAML `mcp.job_poll_interval_seconds` then env `MCP_JOB_POLL_INTERVAL_SECONDS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.job_max_concurrent_jobs` (F06)

MCPPolicy `job_max_concurrent_jobs` defaults to 1; YAML `mcp.job_max_concurrent_jobs` then env `MCP_JOB_MAX_CONCURRENT_JOBS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.staged_max_deadline_ms` (F06)

MCPPolicy `staged_max_deadline_ms` defaults to 30000; YAML `mcp.staged_max_deadline_ms` then env `MCP_STAGED_MAX_DEADLINE_MS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.staged_max_engine_calls` (F06)

MCPPolicy `staged_max_engine_calls` defaults to 64; YAML `mcp.staged_max_engine_calls` then env `MCP_STAGED_MAX_ENGINE_CALLS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.saved_max_definitions` (F06)

MCPPolicy `saved_max_definitions` defaults to 20; YAML `mcp.saved_max_definitions` then env `MCP_SAVED_MAX_DEFINITIONS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.saved_max_engines` (F06)

MCPPolicy `saved_max_engines` defaults to 5; YAML `mcp.saved_max_engines` then env `MCP_SAVED_MAX_ENGINES`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.saved_max_results` (F06)

MCPPolicy `saved_max_results` defaults to 100; YAML `mcp.saved_max_results` then env `MCP_SAVED_MAX_RESULTS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.saved_default_reports` (F06)

MCPPolicy `saved_default_reports` defaults to 20; YAML `mcp.saved_default_reports` then env `MCP_SAVED_DEFAULT_REPORTS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.saved_max_reports` (F06)

MCPPolicy `saved_max_reports` defaults to 100; YAML `mcp.saved_max_reports` then env `MCP_SAVED_MAX_REPORTS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.saved_default_retention_seconds` (F06)

MCPPolicy `saved_default_retention_seconds` defaults to 604800; YAML `mcp.saved_default_retention_seconds` then env `MCP_SAVED_DEFAULT_RETENTION_SECONDS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.saved_max_retention_seconds` (F06)

MCPPolicy `saved_max_retention_seconds` defaults to 2592000; YAML `mcp.saved_max_retention_seconds` then env `MCP_SAVED_MAX_RETENTION_SECONDS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.saved_min_interval_seconds` (F06)

MCPPolicy `saved_min_interval_seconds` defaults to 60; YAML `mcp.saved_min_interval_seconds` then env `MCP_SAVED_MIN_INTERVAL_SECONDS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.saved_max_interval_seconds` (F06)

MCPPolicy `saved_max_interval_seconds` defaults to 86400; YAML `mcp.saved_max_interval_seconds` then env `MCP_SAVED_MAX_INTERVAL_SECONDS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.saved_max_concurrent_runs` (F06)

MCPPolicy `saved_max_concurrent_runs` defaults to 2; YAML `mcp.saved_max_concurrent_runs` then env `MCP_SAVED_MAX_CONCURRENT_RUNS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.saved_dispatch_timeout_seconds` (F06)

MCPPolicy `saved_dispatch_timeout_seconds` defaults to 30; YAML `mcp.saved_dispatch_timeout_seconds` then env `MCP_SAVED_DISPATCH_TIMEOUT_SECONDS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.saved_event_capacity` (F06)

MCPPolicy `saved_event_capacity` defaults to 1000; YAML `mcp.saved_event_capacity` then env `MCP_SAVED_EVENT_CAPACITY`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.saved_event_retention_seconds` (F06)

MCPPolicy `saved_event_retention_seconds` defaults to 604800; YAML `mcp.saved_event_retention_seconds` then env `MCP_SAVED_EVENT_RETENTION_SECONDS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.saved_event_max_consumers` (F06)

MCPPolicy `saved_event_max_consumers` defaults to 100; YAML `mcp.saved_event_max_consumers` then env `MCP_SAVED_EVENT_MAX_CONSUMERS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.auth_token` (F06)

MCPPolicy `auth_token` defaults to ""; YAML `mcp.auth_token` then env `MCP_AUTH_TOKEN`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid or nonpositive numeric env is ignored; YAML positive-integer bounds apply; explicit empty auth token disables token auth.

### `config.mcp.targeted_sensitive_allowed` (F06)

MCPPolicy `targeted_sensitive_allowed` defaults to false; YAML `mcp.targeted_sensitive_allowed` then env `MCP_TARGETED_SENSITIVE_ALLOWED`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.oauth_enabled` (F06)

MCPPolicy `oauth_enabled` defaults to false; YAML `mcp.oauth.enabled` then env `MCP_OAUTH_ENABLED`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.oauth_issuer_url` (F06)

MCPPolicy `oauth_issuer_url` defaults to ""; YAML `mcp.oauth.issuer_url` then env `MCP_OAUTH_ISSUER_URL`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.oauth_service_documentation_url` (F06)

MCPPolicy `oauth_service_documentation_url` defaults to ""; YAML `mcp.oauth.service_documentation_url` then env `MCP_OAUTH_SERVICE_DOCUMENTATION_URL`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.oauth_access_token_ttl_seconds` (F06)

MCPPolicy `oauth_access_token_ttl_seconds` defaults to 3600; YAML `mcp.oauth.access_token_ttl_seconds` then env `MCP_OAUTH_ACCESS_TOKEN_TTL_SECONDS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.oauth_refresh_token_ttl_seconds` (F06)

MCPPolicy `oauth_refresh_token_ttl_seconds` defaults to 2592000; YAML `mcp.oauth.refresh_token_ttl_seconds` then env `MCP_OAUTH_REFRESH_TOKEN_TTL_SECONDS`. Policy is separate from HTTP config.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: An absent env var leaves YAML/default; invalid/nonpositive numeric env is ignored, while recognized booleans accept true/1/yes and false/0/no. Explicit empty string and absent have field-specific behavior.

### `config.mcp.grant.jobs` (F06)

Specialist `jobs` grant defaults false; YAML `mcp.enabled_tools.jobs` then env `MCP_GRANT_JOBS`. Grant does not authorize sensitive engines.

Source: `slopsearx/capabilities.py:MCPPolicy.enabled_tools`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: Missing grant is false by default; explicit false overrides YAML true; invalid env token leaves YAML/default. Sensitive mixed engine list fails atomically.

### `config.mcp.grant.security` (F06)

Specialist `security` grant defaults false; YAML `mcp.enabled_tools.security` then env `MCP_GRANT_SECURITY`. Grant does not authorize sensitive engines.

Source: `slopsearx/capabilities.py:MCPPolicy.enabled_tools`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: Missing grant is false by default; explicit false overrides YAML true; invalid env token leaves YAML/default. Sensitive mixed engine list fails atomically.

### `config.mcp.grant.science` (F06)

Specialist `science` grant defaults false; YAML `mcp.enabled_tools.science` then env `MCP_GRANT_SCIENCE`. Grant does not authorize sensitive engines.

Source: `slopsearx/capabilities.py:MCPPolicy.enabled_tools`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: Missing grant is false by default; explicit false overrides YAML true; invalid env token leaves YAML/default. Sensitive mixed engine list fails atomically.

### `config.mcp.grant.research` (F06)

Specialist `research` grant defaults false; YAML `mcp.enabled_tools.research` then env `MCP_GRANT_RESEARCH`. Grant does not authorize sensitive engines.

Source: `slopsearx/capabilities.py:MCPPolicy.enabled_tools`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: Missing grant is false by default; explicit false overrides YAML true; invalid env token leaves YAML/default. Sensitive mixed engine list fails atomically.

### `config.mcp.grant.dependency_dossier` (F06)

Specialist `dependency_dossier` grant defaults false; YAML `mcp.enabled_tools.dependency_dossier` then env `MCP_GRANT_DEPENDENCY_DOSSIER`. Grant does not authorize sensitive engines.

Source: `slopsearx/capabilities.py:MCPPolicy.enabled_tools`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: Missing grant is false by default; explicit false overrides YAML true; invalid env token leaves YAML/default. Sensitive mixed engine list fails atomically.

### `config.mcp.grant.staged_search` (F06)

Specialist `staged_search` grant defaults false; YAML `mcp.enabled_tools.staged_search` then env `MCP_GRANT_STAGED_SEARCH`. Grant does not authorize sensitive engines.

Source: `slopsearx/capabilities.py:MCPPolicy.enabled_tools`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: Missing grant is false by default; explicit false overrides YAML true; invalid env token leaves YAML/default. Sensitive mixed engine list fails atomically.

### `config.mcp.grant.retrieval_receipts` (F06)

Specialist `retrieval_receipts` grant defaults false; YAML `mcp.enabled_tools.retrieval_receipts` then env `MCP_GRANT_RETRIEVAL_RECEIPTS`. Grant does not authorize sensitive engines.

Source: `slopsearx/capabilities.py:MCPPolicy.enabled_tools`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: Missing grant is false by default; explicit false overrides YAML true; invalid env token leaves YAML/default. Sensitive mixed engine list fails atomically.

### `config.mcp.grant.saved_searches` (F06)

Specialist `saved_searches` grant defaults false; YAML `mcp.enabled_tools.saved_searches` then env `MCP_GRANT_SAVED_SEARCHES`. Grant does not authorize sensitive engines.

Source: `slopsearx/capabilities.py:MCPPolicy.enabled_tools`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: Missing grant is false by default; explicit false overrides YAML true; invalid env token leaves YAML/default. Sensitive mixed engine list fails atomically.

### `config.mcp.grant.saved_search_events` (F06)

Specialist `saved_search_events` grant defaults false; YAML `mcp.enabled_tools.saved_search_events` then env `MCP_GRANT_SAVED_SEARCH_EVENTS`. Grant does not authorize sensitive engines.

Source: `slopsearx/capabilities.py:MCPPolicy.enabled_tools`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: Missing grant is false by default; explicit false overrides YAML true; invalid env token leaves YAML/default. Sensitive mixed engine list fails atomically.

### `config.mcp.sensitive_engines` (F06)

MCPPolicy sensitive engine set defaults to DEFAULT_SENSITIVE_ENGINES; YAML `mcp.sensitive_engines` list then nonempty comma-separated `MCP_SENSITIVE_ENGINES` env. Uniform targeted_sensitive_allowed grant governs access.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`, `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: Absent env retains YAML/default; empty env does not clear the set; unknown names fail policy validation, unlike an empty valid set.

### `config.mcp.required_key_engines` (F06)

MCPPolicy required-key set defaults to REQUIRED_KEY_ENGINES; YAML `mcp.required_key_engines` list can override; there is no matching environment override in _apply_mcp_env.

Source: `slopsearx/capabilities.py:MCPPolicy`, `slopsearx/capabilities.py:_apply_mcp_section`.
Regression: `tests/test_capabilities.py`, `tests/test_mcp_security.py`.
Distinctions: Missing list uses default; explicit empty YAML list clears it; unknown names fail validation.

### `config.runtime.max_concurrent_engines` (F06)

Runtime environment `MAX_CONCURRENT_ENGINES` default 10 is read directly by slopsearx/service.py:build_context; it is not generically resolved by load_config.

Source: `slopsearx/service.py:build_context`.
Regression: `tests/test_service.py`, `tests/test_config.py`.
Distinctions: Missing env uses runtime default; empty/invalid setting follows this source reader and must be fixture tested. Credential absence differs from explicit secret value.

### `config.runtime.per_client_requests` (F06)

Runtime environment `PER_CLIENT_REQUESTS` default 30 is read directly by slopsearx/service.py:build_context; it is not generically resolved by load_config.

Source: `slopsearx/service.py:build_context`.
Regression: `tests/test_service.py`, `tests/test_config.py`.
Distinctions: Missing env uses runtime default; empty/invalid setting follows this source reader and must be fixture tested. Credential absence differs from explicit secret value.

### `config.runtime.per_client_window_seconds` (F06)

Runtime environment `PER_CLIENT_WINDOW_SECONDS` default 60 is read directly by slopsearx/service.py:build_context; it is not generically resolved by load_config.

Source: `slopsearx/service.py:build_context`.
Regression: `tests/test_service.py`, `tests/test_config.py`.
Distinctions: Missing env uses runtime default; empty/invalid setting follows this source reader and must be fixture tested. Credential absence differs from explicit secret value.

### `config.runtime.fail_closed` (F06)

Runtime environment `FAIL_CLOSED` default false is read directly by slopsearx/service.py:build_context; it is not generically resolved by load_config.

Source: `slopsearx/service.py:build_context`.
Regression: `tests/test_service.py`, `tests/test_config.py`.
Distinctions: Missing env uses runtime default; empty/invalid setting follows this source reader and must be fixture tested. Credential absence differs from explicit secret value.

### `config.runtime.valkey_url` (F06)

Runtime environment `VALKEY_URL` default "" is read directly by slopsearx/service.py:build_context; it is not generically resolved by load_config.

Source: `slopsearx/service.py:build_context`.
Regression: `tests/test_service.py`, `tests/test_config.py`.
Distinctions: Missing env uses runtime default; empty/invalid setting follows this source reader and must be fixture tested. Credential absence differs from explicit secret value.

### `config.runtime.empty_scrape_diagnostics` (F06)

Runtime environment `FEATURE_EMPTY_SCRAPE_DIAGNOSTICS` default false is read directly by slopsearx/service.py:build_context; it is not generically resolved by load_config.

Source: `slopsearx/service.py:build_context`.
Regression: `tests/test_service.py`, `tests/test_config.py`.
Distinctions: Missing env uses runtime default; empty/invalid setting follows this source reader and must be fixture tested. Credential absence differs from explicit secret value.

### `config.runtime.brave_category_routing` (F06)

Runtime environment `FEATURE_BRAVE_CATEGORY_ROUTING` default false is read directly by slopsearx/service.py:build_context; it is not generically resolved by load_config.

Source: `slopsearx/service.py:build_context`.
Regression: `tests/test_service.py`, `tests/test_config.py`.
Distinctions: Missing env uses runtime default; empty/invalid setting follows this source reader and must be fixture tested. Credential absence differs from explicit secret value.

### `config.runtime.cache_ttl_seconds` (F06)

Runtime environment `SEARCH_CACHE_TTL_SECONDS` default 3600 is read directly by slopsearx/cache.py:SearchCache; it is not generically resolved by load_config.

Source: `slopsearx/cache.py:SearchCache`.
Regression: `tests/test_service.py`, `tests/test_config.py`.
Distinctions: Missing env uses runtime default; empty/invalid setting follows this source reader and must be fixture tested. Credential absence differs from explicit secret value.

### `config.runtime.cache_partial_ttl_seconds` (F06)

Runtime environment `SEARCH_CACHE_PARTIAL_TTL_SECONDS` default 30 is read directly by slopsearx/cache.py:SearchCache; it is not generically resolved by load_config.

Source: `slopsearx/cache.py:SearchCache`.
Regression: `tests/test_service.py`, `tests/test_config.py`.
Distinctions: Missing env uses runtime default; empty/invalid setting follows this source reader and must be fixture tested. Credential absence differs from explicit secret value.

### `config.runtime.cache_negative_ttl_seconds` (F06)

Runtime environment `SEARCH_CACHE_NEGATIVE_TTL_SECONDS` default 60 is read directly by slopsearx/cache.py:SearchCache; it is not generically resolved by load_config.

Source: `slopsearx/cache.py:SearchCache`.
Regression: `tests/test_service.py`, `tests/test_config.py`.
Distinctions: Missing env uses runtime default; empty/invalid setting follows this source reader and must be fixture tested. Credential absence differs from explicit secret value.

### `config.runtime.observed_health_stale_seconds` (F06)

Runtime environment `OBSERVED_HEALTH_STALE_SECONDS` default 300 is read directly by slopsearx/capabilities.py:observed_health_stale_seconds; it is not generically resolved by load_config.

Source: `slopsearx/capabilities.py:observed_health_stale_seconds`.
Regression: `tests/test_service.py`, `tests/test_config.py`.
Distinctions: Missing env uses runtime default; empty/invalid setting follows this source reader and must be fixture tested. Credential absence differs from explicit secret value.

### `config.runtime.jev_specialist_threshold` (F06)

Runtime environment `JEV_SPECIALIST_THRESHOLD` default 0.65 is read directly by slopsearx/jev.py:JevSpecialistRouter; it is not generically resolved by load_config.

Source: `slopsearx/jev.py:JevSpecialistRouter`.
Regression: `tests/test_service.py`, `tests/test_config.py`.
Distinctions: Missing env uses runtime default; empty/invalid setting follows this source reader and must be fixture tested. Credential absence differs from explicit secret value.

### `config.runtime.jev_timeout_ms` (F06)

Runtime environment `JEV_TIMEOUT_MS` default 1000 is read directly by slopsearx/jev.py:JevSpecialistRouter; it is not generically resolved by load_config.

Source: `slopsearx/jev.py:JevSpecialistRouter`.
Regression: `tests/test_service.py`, `tests/test_config.py`.
Distinctions: Missing env uses runtime default; empty/invalid setting follows this source reader and must be fixture tested. Credential absence differs from explicit secret value.

### `config.runtime.jev_routing_cache_ttl_seconds` (F06)

Runtime environment `JEV_ROUTING_CACHE_TTL_SECONDS` default 3600 is read directly by slopsearx/jev.py:JevSpecialistRouter; it is not generically resolved by load_config.

Source: `slopsearx/jev.py:JevSpecialistRouter`.
Regression: `tests/test_service.py`, `tests/test_config.py`.
Distinctions: Missing env uses runtime default; empty/invalid setting follows this source reader and must be fixture tested. Credential absence differs from explicit secret value.

### `config.runtime.typesafe_api_key` (F06)

Runtime environment `TYPESAFE_API_KEY` default "" is read directly by slopsearx/jev.py:JevSpecialistRouter; it is not generically resolved by load_config.

Source: `slopsearx/jev.py:JevSpecialistRouter`.
Regression: `tests/test_service.py`, `tests/test_config.py`.
Distinctions: Missing env uses runtime default; empty/invalid setting follows this source reader and must be fixture tested. Credential absence differs from explicit secret value.

### `env.read.engines.brave.BraveAdapter.__init__.76` (A04)

Direct environment read `os.environ.get(f'{self.env_prefix}_API_KEY')` at engines/brave.py:76; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `engines/brave.py:BraveAdapter.__init__`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.engines.brave.BraveAdapter.search.94` (A04)

Direct environment read `os.environ.get(f'{self.env_prefix}_API_KEY')` at engines/brave.py:94; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `engines/brave.py:BraveAdapter.search`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.engines.fred.FredAdapter.search.58` (A08)

Direct environment read `environ.get('ENGINE_FRED_API_KEY')` at engines/fred.py:58; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `engines/fred.py:FredAdapter.search`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.engines.tmdb.TMDBAdapter.search.54` (A09)

Direct environment read `environ.get('ENGINE_TMDB_API_KEY')` at engines/tmdb.py:54; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `engines/tmdb.py:TMDBAdapter.search`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.adapter.EngineAdapter.__init__.377` (F03)

Direct environment read `os.environ.get('ENGINE_CIRCUIT_THRESHOLD')` at slopsearx/adapter.py:377; raw fallback expression `str(self.CIRCUIT_BREAKER_THRESHOLD)`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/adapter.py:EngineAdapter.__init__`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.adapter.EngineAdapter.__init__.382` (F03)

Direct environment read `os.environ.get('ENGINE_CIRCUIT_TIMEOUT')` at slopsearx/adapter.py:382; raw fallback expression `str(self.CIRCUIT_BREAKER_TIMEOUT)`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/adapter.py:EngineAdapter.__init__`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.cache._cache_ttl_setting.89` (F13)

Direct environment read `os.environ.get(name)` at slopsearx/cache.py:89; raw fallback expression `str(default)`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/cache.py:_cache_ttl_setting`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.cache.SearchCache.__init__.116` (F13)

Direct environment read `os.environ.get('VALKEY_URL')` at slopsearx/cache.py:116; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/cache.py:SearchCache.__init__`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.cache.SearchCache.__init__.124` (F13)

Direct environment read `os.environ.get('SEARCH_CACHE_NEGATIVE_TTL_SECONDS')` at slopsearx/cache.py:124; raw fallback expression `'60'`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/cache.py:SearchCache.__init__`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.capabilities.observed_health_stale_seconds.283` (F06)

Direct environment read `os.environ.get('OBSERVED_HEALTH_STALE_SECONDS')` at slopsearx/capabilities.py:283; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/capabilities.py:observed_health_stale_seconds`.
Regression: `tests/test_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.capabilities._apply_mcp_env.782` (F06)

Direct environment read `os.environ.get(env_var)` at slopsearx/capabilities.py:782; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.capabilities._apply_mcp_env.816` (F06)

Direct environment read `os.environ.get(env_var)` at slopsearx/capabilities.py:816; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.capabilities._apply_mcp_env.825` (F06)

Direct environment read `os.environ.get('MCP_JOB_POLL_INTERVAL_SECONDS')` at slopsearx/capabilities.py:825; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.capabilities._apply_mcp_env.834` (F06)

Direct environment read `os.environ.get('MCP_AUTH_TOKEN')` at slopsearx/capabilities.py:834; raw fallback expression `None`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.capabilities._apply_mcp_env.838` (F06)

Direct environment read `os.environ.get('MCP_TARGETED_SENSITIVE_ALLOWED')` at slopsearx/capabilities.py:838; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.capabilities._apply_mcp_env.844` (F06)

Direct environment read `os.environ.get('MCP_SENSITIVE_ENGINES')` at slopsearx/capabilities.py:844; raw fallback expression `None`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.capabilities._apply_mcp_env.849` (F06)

Direct environment read `os.environ.get('MCP_OAUTH_ENABLED')` at slopsearx/capabilities.py:849; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.capabilities._apply_mcp_env.854` (F06)

Direct environment read `os.environ.get('MCP_OAUTH_ISSUER_URL')` at slopsearx/capabilities.py:854; raw fallback expression `None`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.capabilities._apply_mcp_env.857` (F06)

Direct environment read `os.environ.get('MCP_OAUTH_SERVICE_DOCUMENTATION_URL')` at slopsearx/capabilities.py:857; raw fallback expression `None`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.capabilities._apply_mcp_env.864` (F06)

Direct environment read `os.environ.get(env_var)` at slopsearx/capabilities.py:864; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/capabilities.py:_apply_mcp_env`.
Regression: `tests/test_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.config.EngineEntry.__post_init__.75` (F06)

Direct environment read `os.environ.get(f'ENGINE_{self.base_url.upper()}_API_KEY')` at slopsearx/config.py:75; raw fallback expression `None`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/config.py:EngineEntry.__post_init__`.
Regression: `tests/test_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.config._load_env_overrides.354` (F06)

Direct environment read `os.environ[]('HF_TOKEN')` at slopsearx/config.py:354; raw fallback expression `<KeyError if absent>`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/config.py:_load_env_overrides`.
Regression: `tests/test_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.config._dict_to_config.364` (F06)

Direct environment read `os.environ.get(f'ENGINE_{name.upper()}_API_KEY')` at slopsearx/config.py:364; raw fallback expression `None`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/config.py:_dict_to_config`.
Regression: `tests/test_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.jev.JevSpecialistRouter.from_environment.196` (F12)

Direct environment read `os.environ.get('TYPESAFE_API_KEY')` at slopsearx/jev.py:196; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/jev.py:JevSpecialistRouter.from_environment`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.jev.JevSpecialistRouter.from_environment.200` (F12)

Direct environment read `os.environ.get('JEV_SPECIALIST_THRESHOLD')` at slopsearx/jev.py:200; raw fallback expression `'0.65'`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/jev.py:JevSpecialistRouter.from_environment`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.jev.JevSpecialistRouter.from_environment.201` (F12)

Direct environment read `os.environ.get('JEV_TIMEOUT_MS')` at slopsearx/jev.py:201; raw fallback expression `'1000'`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/jev.py:JevSpecialistRouter.from_environment`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.jev.JevSpecialistRouter.route.276` (F12)

Direct environment read `os.environ.get('JEV_ROUTING_CACHE_TTL_SECONDS')` at slopsearx/jev.py:276; raw fallback expression `'3600'`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/jev.py:JevSpecialistRouter.route`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.logging.setup_logging.58` (O01)

Direct environment read `os.getenv('SENTRY_DSN')` at slopsearx/logging.py:58; raw fallback expression `None`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/logging.py:setup_logging`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.mcp.harness.main.329` (G01)

Direct environment read `os.environ.get('MCP_HARNESS_HOST')` at slopsearx/mcp/harness.py:329; raw fallback expression `'127.0.0.1'`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/mcp/harness.py:main`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.mcp.harness.main.333` (G01)

Direct environment read `os.environ.get('MCP_HARNESS_PORT')` at slopsearx/mcp/harness.py:333; raw fallback expression `'8105'`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/mcp/harness.py:main`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.mcp.harness.main.337` (G01)

Direct environment read `os.environ.get('MCP_HARNESS_TOKEN')` at slopsearx/mcp/harness.py:337; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/mcp/harness.py:main`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.mcp.harness.main.342` (G01)

Direct environment read `os.environ.get('MCP_HARNESS_ENGINES')` at slopsearx/mcp/harness.py:342; raw fallback expression `','.join(DEFAULT_FAKE_ENGINES)`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/mcp/harness.py:main`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.mcp.harness.main.348` (G01)

Direct environment read `os.environ.get('MCP_HARNESS_RESULTS')` at slopsearx/mcp/harness.py:348; raw fallback expression `'3'`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/mcp/harness.py:main`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.mcp.harness.main.355` (G01)

Direct environment read `os.environ.get('MCP_LOG_LEVEL')` at slopsearx/mcp/harness.py:355; raw fallback expression `'info'`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/mcp/harness.py:main`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.mcp.oauth_client.default_token_path.99` (T07)

Direct environment read `os.environ.get('XDG_CONFIG_HOME')` at slopsearx/mcp/oauth_client.py:99; raw fallback expression `str(Path.home() / '.config')`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/mcp/oauth_client.py:default_token_path`.
Regression: `tests/test_mcp_oauth_client.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.mcp.server._env_flag.351` (T01)

Direct environment read `os.environ.get(name)` at slopsearx/mcp/server.py:351; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/mcp/server.py:_env_flag`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.mcp.server.main.371` (T07)

Direct environment read `os.environ.get('MCP_REMOTE_URL')` at slopsearx/mcp/server.py:371; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/mcp/server.py:main`.
Regression: `tests/test_mcp_oauth_client.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.mcp.server.main.376` (T07)

Direct environment read `os.environ.get('MCP_REMOTE_TOKEN')` at slopsearx/mcp/server.py:376; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/mcp/server.py:main`.
Regression: `tests/test_mcp_oauth_client.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.mcp.server.main.389` (T07)

Direct environment read `os.environ.get('MCP_REMOTE_OAUTH_CALLBACK_PORT')` at slopsearx/mcp/server.py:389; raw fallback expression `'8765'`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/mcp/server.py:main`.
Regression: `tests/test_mcp_oauth_client.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.mcp.server.main.400` (T07)

Direct environment read `os.environ.get('MCP_REMOTE_TOKEN_FILE')` at slopsearx/mcp/server.py:400; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/mcp/server.py:main`.
Regression: `tests/test_mcp_oauth_client.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.mcp.server.main.405` (T01)

Direct environment read `os.environ.get('MCP_TRANSPORT')` at slopsearx/mcp/server.py:405; raw fallback expression `'stdio'`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/mcp/server.py:main`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.mcp.server.main.431` (T01)

Direct environment read `os.environ.get('MCP_HOST')` at slopsearx/mcp/server.py:431; raw fallback expression `'127.0.0.1'`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/mcp/server.py:main`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.mcp.server.main.433` (T01)

Direct environment read `os.environ.get('MCP_PORT')` at slopsearx/mcp/server.py:433; raw fallback expression `'8000'`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/mcp/server.py:main`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.mcp.server.main.460` (T01)

Direct environment read `os.environ.get('MCP_LOG_LEVEL')` at slopsearx/mcp/server.py:460; raw fallback expression `'info'`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/mcp/server.py:main`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.payload.payload_max_persist_bytes.185` (F05)

Direct environment read `os.environ.get('PAYLOAD_MAX_PERSIST_BYTES')` at slopsearx/payload.py:185; raw fallback expression `None`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/payload.py:payload_max_persist_bytes`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.ratelimit.ValkeySlidingWindow.__init__.126` (F10)

Direct environment read `os.environ.get('VALKEY_URL')` at slopsearx/ratelimit.py:126; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/ratelimit.py:ValkeySlidingWindow.__init__`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.rerank.JevReranker.from_environment.107` (F15)

Direct environment read `os.environ.get('TYPESAFE_API_KEY')` at slopsearx/rerank.py:107; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/rerank.py:JevReranker.from_environment`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.routing._coerce_non_negative_int.363` (F12)

Direct environment read `os.environ.get(env_var)` at slopsearx/routing.py:363; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/routing.py:_coerce_non_negative_int`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.routing._coerce_cost_class.376` (F12)

Direct environment read `os.environ.get(env_var)` at slopsearx/routing.py:376; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/routing.py:_coerce_cost_class`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.server._portal_default_theme.193` (T02)

Direct environment read `os.getenv('SLOPSEARX_PORTAL_DEFAULT_THEME')` at slopsearx/server.py:193; raw fallback expression `'dark'`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/server.py:_portal_default_theme`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.service.build_context.326` (F14)

Direct environment read `os.environ.get('MAX_CONCURRENT_ENGINES')` at slopsearx/service.py:326; raw fallback expression `'10'`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/service.py:build_context`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.service.build_context.337` (F14)

Direct environment read `os.environ.get('PER_CLIENT_REQUESTS')` at slopsearx/service.py:337; raw fallback expression `'30'`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/service.py:build_context`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.service.build_context.341` (F14)

Direct environment read `os.environ.get('PER_CLIENT_WINDOW_SECONDS')` at slopsearx/service.py:341; raw fallback expression `'60'`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/service.py:build_context`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.service.build_context.346` (F14)

Direct environment read `os.environ.get('FAIL_CLOSED')` at slopsearx/service.py:346; raw fallback expression `'false'`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/service.py:build_context`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.service.build_context.350` (F14)

Direct environment read `os.environ.get('VALKEY_URL')` at slopsearx/service.py:350; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/service.py:build_context`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.service.build_context.359` (F14)

Direct environment read `os.environ.get('FEATURE_EMPTY_SCRAPE_DIAGNOSTICS')` at slopsearx/service.py:359; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/service.py:build_context`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.service.build_context.366` (F14)

Direct environment read `os.environ.get('FEATURE_BRAVE_CATEGORY_ROUTING')` at slopsearx/service.py:366; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/service.py:build_context`.
Regression: `tests/test_adapters.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.workflow_portal._required_secret.73` (T09)

Direct environment read `os.getenv(name)` at slopsearx/workflow_portal.py:73; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/workflow_portal.py:_required_secret`.
Regression: `tests/test_workflow_portal_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.workflow_portal._previous_secret.80` (T09)

Direct environment read `os.getenv('SLOPSEARX_PORTAL_PREVIOUS_SESSION_KEY')` at slopsearx/workflow_portal.py:80; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/workflow_portal.py:_previous_secret`.
Regression: `tests/test_workflow_portal_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.workflow_portal._previous_secret.85` (T09)

Direct environment read `os.getenv('SLOPSEARX_PORTAL_PREVIOUS_KEY_DRAIN_SECONDS')` at slopsearx/workflow_portal.py:85; raw fallback expression `'600'`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/workflow_portal.py:_previous_secret`.
Regression: `tests/test_workflow_portal_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.workflow_portal.build_workflow_runtime_from_env.169` (W10)

Direct environment read `os.getenv('SLOPSEARX_WORKFLOW_PORTAL_ENABLED')` at slopsearx/workflow_portal.py:169; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/workflow_portal.py:build_workflow_runtime_from_env`.
Regression: `tests/test_workflow_portal_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.workflow_portal.build_workflow_runtime_from_env.177` (T09)

Direct environment read `os.getenv('SLOPSEARX_PORTAL_EXTERNAL_ORIGIN')` at slopsearx/workflow_portal.py:177; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/workflow_portal.py:build_workflow_runtime_from_env`.
Regression: `tests/test_workflow_portal_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.workflow_portal.build_workflow_runtime_from_env.187` (T09)

Direct environment read `os.getenv('SLOPSEARX_PORTAL_TRUSTED_PROXIES')` at slopsearx/workflow_portal.py:187; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/workflow_portal.py:build_workflow_runtime_from_env`.
Regression: `tests/test_workflow_portal_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.workflow_portal.build_workflow_runtime_from_env.189` (T09)

Direct environment read `os.getenv('SLOPSEARX_PORTAL_OIDC_ISSUER')` at slopsearx/workflow_portal.py:189; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/workflow_portal.py:build_workflow_runtime_from_env`.
Regression: `tests/test_workflow_portal_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.workflow_portal.build_workflow_runtime_from_env.192` (T09)

Direct environment read `os.getenv('SLOPSEARX_PORTAL_OIDC_CLIENT_ID')` at slopsearx/workflow_portal.py:192; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/workflow_portal.py:build_workflow_runtime_from_env`.
Regression: `tests/test_workflow_portal_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.workflow_portal.build_workflow_runtime_from_env.193` (T09)

Direct environment read `os.getenv('SLOPSEARX_PORTAL_OIDC_CLIENT_SECRET')` at slopsearx/workflow_portal.py:193; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/workflow_portal.py:build_workflow_runtime_from_env`.
Regression: `tests/test_workflow_portal_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.workflow_portal.build_workflow_runtime_from_env.198` (T09)

Direct environment read `os.getenv('SLOPSEARX_PORTAL_MEMBERSHIPS_FILE')` at slopsearx/workflow_portal.py:198; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/workflow_portal.py:build_workflow_runtime_from_env`.
Regression: `tests/test_workflow_portal_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `env.read.slopsearx.workflow_portal.build_workflow_runtime_from_env.201` (W10)

Direct environment read `os.getenv('SLOPSEARX_WORKFLOW_PORTAL_ACTIONS')` at slopsearx/workflow_portal.py:201; raw fallback expression `''`. Preserve parser, missing, empty and invalid-value behavior at the named source reader.

Source: `slopsearx/workflow_portal.py:build_workflow_runtime_from_env`.
Regression: `tests/test_workflow_portal_config.py`.
Distinctions: Absent env follows raw fallback expression; present empty string, malformed text and omitted key can produce different outcomes. Resolve computed key expressions from caller during F02.

### `config.helper_env.routing_max_engines_per_intent` (F06)

Helper-expanded `ROUTING_MAX_ENGINES_PER_INTENT` config for routing.budget.max_engines_per_intent; effective default `0 (unbounded)`; positive integer; invalid/zero becomes unbounded.

Source: `slopsearx/routing.py:load_routing_budget`.
Regression: `tests/test_routing_service.py`.
Distinctions: Missing, explicitly empty, invalid and valid env values follow helper parser, not generic Config coercion.

### `config.helper_env.routing_max_cost_class` (F06)

Helper-expanded `ROUTING_MAX_COST_CLASS` config for routing.budget.max_cost_class; effective default `paid`; one of ranked cost classes; invalid becomes paid.

Source: `slopsearx/routing.py:load_routing_budget`.
Regression: `tests/test_routing_service.py`.
Distinctions: Missing, explicitly empty, invalid and valid env values follow helper parser, not generic Config coercion.

### `config.helper_env.routing_coverage_target` (F06)

Helper-expanded `ROUTING_COVERAGE_TARGET` config for routing.budget.coverage_target; effective default `0 (unbounded)`; positive integer; invalid/zero becomes unbounded.

Source: `slopsearx/routing.py:load_routing_budget`.
Regression: `tests/test_routing_service.py`.
Distinctions: Missing, explicitly empty, invalid and valid env values follow helper parser, not generic Config coercion.

### `config.helper_env.mcp_remote_oauth` (T07)

Helper-expanded `MCP_REMOTE_OAUTH` config for gateway oauth flag; effective default `false`; true/1/yes only.

Source: `slopsearx/mcp/server.py:_env_flag`.
Regression: `tests/test_mcp_oauth_client.py`.
Distinctions: Missing, explicitly empty, invalid and valid env values follow helper parser, not generic Config coercion.

### `config.helper_env.mcp_remote_oauth_no_browser` (T07)

Helper-expanded `MCP_REMOTE_OAUTH_NO_BROWSER` config for gateway no-browser flag; effective default `false`; true/1/yes only.

Source: `slopsearx/mcp/server.py:_env_flag`.
Regression: `tests/test_mcp_oauth_client.py`.
Distinctions: Missing, explicitly empty, invalid and valid env values follow helper parser, not generic Config coercion.

### `config.helper_env.slopsearx_portal_session_key` (T09)

Helper-expanded `SLOPSEARX_PORTAL_SESSION_KEY` config for portal session HMAC key; effective default `required`; at least 32 bytes; missing or short raises.

Source: `slopsearx/workflow_portal.py:_required_secret`.
Regression: `tests/test_workflow_portal_config.py`.
Distinctions: Missing, explicitly empty, invalid and valid env values follow helper parser, not generic Config coercion.

### `config.helper_env.hf_token` (F06)

HF_TOKEN convention supplies Hugging Face api_key only when ENGINE_HUGGINGFACE_API_KEY is absent; explicit prefixed key wins even if empty.

Source: `slopsearx/config.py:_load_env_overrides`.
Regression: `tests/test_config.py`, `tests/test_service_engine_config.py`.
Distinctions: HF_TOKEN absent, present empty and explicit ENGINE_HUGGINGFACE_API_KEY are distinct precedence cases.

## Durable record families

### `state.research_job` (W01)

mcp:job:{tenant}:{job_id}; 24h retention. ResearchJob stores queries, attempts, coverage, workflow, budget and lifecycle queued/running/partial/succeeded/failed/cancelled/expired. Retry preserves immutable prior attempts and resets retryable query state.

Source: `slopsearx/research_store.py:ResearchJobStore._key`, `slopsearx/research_models.py:ResearchJob`, `slopsearx/research_models.py:recover_orphan_attempts`.
Regression: `tests/test_research_durability.py`, `tests/test_research_retry_followup.py`.
Distinctions: Unknown job, expired job, cancelled job and partial job are distinct; null attempt outcome is not zero observed calls.

Schema fields from dataclasses: `ResearchJob`: `job_id`, `question`, `strategy`, `state`, `queries`, `warnings`, `created_at`, `deadline`, `tenant`, `idempotency_key`, `workflow`, `cancel_requested`, `owner_id`, `lease_token`, `lease_expires_at`, `subquestions`, `budget_limits`, `budget_used`, `seen_lead_ids`, `caller_completed`, `completion_rationale`, `completion_subquestion_states`, `stop_reason`; `ResearchQuery`: `index`, `query`, `intent`, `engines`, `time_range`, `state`, `query_id`, `result_count`, `cursor`, `error`, `engine_coverage`, `attempts`, `enforcement`, `subquestion_id`, `rationale`, `parent_attempt_id`, `continuation_key`, `continuation_digest`, `requires_intent_grant`, `planning_method`, `evidence_result_ids`, `evidence_engines`, `evidence_intent`; `ResearchQueryAttempt`: `cursor`, `query_id`, `result_count`, `error`, `state`, `attempted_at`, `engine_coverage`, `attempt_id`, `finished_at`, `enforcement`, `admitted_result_ids`, `new_lead_ids`

### `state.research_idempotency` (W01)

mcp:idem:{tenant}:{idempotency_key} maps admission to an existing job. Atomic duplicate admission must replay same request and reject conflicting identity, preserving tenant isolation.

Source: `slopsearx/research_store.py:ResearchJobStore._idem_key`, `slopsearx/research_store.py:ResearchJobStore.create_idempotent`.
Regression: `tests/test_research_durability.py`, `tests/test_research_queue_integration.py`.
Distinctions: Missing idempotency mapping is not storage unavailable; same key/same payload is replay, different payload is conflict.

### `state.research_lease` (W02)

mcp:joblease:{tenant}:{job_id}, mcp:jobcancel:{tenant}:{job_id}; default 60s lease. Claim/renew/save_if_owned use token fencing; cancellation and orphan recovery do not authorize stale commits.

Source: `slopsearx/research_store.py:ResearchJobStore.claim`, `slopsearx/research_store.py:ResearchJobStore.renew`, `slopsearx/research_store.py:ResearchJobStore.save_if_owned`.
Regression: `tests/test_research_queue_integration.py`, `tests/test_research_durability.py`.
Distinctions: Lease absent, lease owned by another token and store unavailable differ; provider outcome after lost lease remains unknown and budget charged.

### `state.research_indexes` (W02)

mcp:ready:v1:tenant:{tenant}, :tenants, :turn, :cursor and :reconcile-lock plus mcp:recent:v1:{tenant} are repairable discovery/index coordination, not job authority. Ready takes reserve candidate non-destructively, rotates tenants, reconciles in batches; recent capped at 200 and 24h retention.

Source: `slopsearx/research_store.py:_READY_REFRESH_SCRIPT`, `slopsearx/research_store.py:_READY_TAKE_SCRIPT`, `slopsearx/research_store.py:ResearchJobStore.claim_next_any_tenant`.
Regression: `tests/test_research_queue_integration.py`, `tests/test_research_durability.py`.
Distinctions: Empty index does not prove no durable jobs; damaged index requires reconciliation, not lost work.

### `state.research_budget` (W01)

reserve_attempt conserves engine-call and elapsed budgets before dispatch; finish_attempt records observed outcomes without refunding ambiguous remote calls. Retry does not erase prior attempt cost.

Source: `slopsearx/research_budget.py:initialize_budget`, `slopsearx/research_budget.py:reserve_attempt`, `slopsearx/research_budget.py:finish_attempt`.
Regression: `tests/test_research_durability.py`, `tests/test_research_coverage.py`.
Distinctions: Null observed count means unknown; zero means observed no calls. Exhausted budget differs from failed query.

### `state.saved_definition` (W00)

mcp:saved:v1:definition:{tenant}:{search_id}, definitions/count/tenants and recent (max 200) indexes; definition revision CAS, quota and policy fingerprint. TTL is logical expires_at plus 300s store margin.

Source: `slopsearx/saved_store.py:SavedSearchStore.create`, `slopsearx/saved_store.py:SavedSearchStore.compare_and_set`, `slopsearx/saved_models.py:SavedDefinition`.
Regression: `tests/test_saved_models.py`, `tests/test_saved_valkey.py`.
Distinctions: Absent versus expired/deleted; stale revision conflict versus storage unavailable; empty index is not authoritative if records exist.

Schema fields from dataclasses: `SavedDefinition`: `search_id`, `tenant`, `query`, `engines`, `interval_seconds`, `retention_seconds`, `max_results`, `max_reports`, `created_at`, `expires_at`, `next_due`, `revision`, `paused`, `policy_fingerprint`, `last_slot`, `latest_run_id`, `baseline`, `lease_token`, `lease_expires_at`, `deleted`; `SavedLimits`: `definitions`, `engines`, `rows`, `reports`, `retention_seconds`, `minimum_interval`, `maximum_interval`, `concurrency`, `dispatch_timeout`

### `state.saved_schedule_lease` (W04)

mcp:saved:v1:due:{tenant} and lease:{tenant}:{search_id}; claim gives 60s token, renew fences worker, commit validates token/revision/policy fingerprint and advances next due.

Source: `slopsearx/saved_store.py:SavedSearchStore.claim`, `slopsearx/saved_store.py:SavedSearchStore.commit`, `slopsearx/saved_store.py:SavedSearchStore.renew`.
Regression: `tests/test_saved_valkey.py`, `tests/test_saved_searches.py`.
Distinctions: Paused/expired/missing are distinct from not-yet-due; stale token cannot publish report.

Schema fields from dataclasses: `SavedDefinition`: `search_id`, `tenant`, `query`, `engines`, `interval_seconds`, `retention_seconds`, `max_results`, `max_reports`, `created_at`, `expires_at`, `next_due`, `revision`, `paused`, `policy_fingerprint`, `last_slot`, `latest_run_id`, `baseline`, `lease_token`, `lease_expires_at`, `deleted`

### `state.saved_report_baseline` (W04)

mcp:saved:v1:report:{tenant}:{search_id}:{run_id}, reports index and baseline:{tenant}:{search_id}; observation identity url-source-window-v1, stable IDs, diff reports capped at 4 MiB. Atomic commit preserves baseline/report and event coupling.

Source: `slopsearx/saved_store.py:SavedSearchStore._report_key`, `slopsearx/saved_store.py:SavedSearchStore._baseline_key`, `slopsearx/saved_models.py:stable_id`, `slopsearx/saved_models.py:compare_observations`.
Regression: `tests/test_saved_valkey.py`, `tests/test_saved_models.py`, `tests/test_saved_searches.py`.
Distinctions: No baseline differs from empty baseline; missing report differs from empty changes; duplicate run ID cannot create second report.

### `state.saved_tombstone` (W00)

mcp:saved:v1:deleted:{tenant}:{search_id}; delete CAS removes definition, due/recent/report/baseline/event records and retains tombstone. Index repair must not resurrect a deleted definition.

Source: `slopsearx/saved_store.py:SavedSearchStore.delete`, `slopsearx/saved_store.py:SavedSearchStore._tombstone_key`.
Regression: `tests/test_saved_valkey.py`, `tests/test_saved_searches.py`.
Distinctions: Deleted ID is not merely unknown; stale expected revision differs from successful delete.

### `state.saved_events` (W05)

mcp:saved:v1:events:{tenant} stream, event-ack:{tenant}:{consumer}, event-consumers:{tenant}, event-expired marker. Retention default 604800s, capacity 1000, consumers 100; cursor is Redis stream major-minor decimal pair. Ack is monotonic and bounded by observed stream.

Source: `slopsearx/saved_store.py:SavedSearchStore.read_events`, `slopsearx/saved_store.py:SavedSearchStore.acknowledge_event`, `slopsearx/saved_events.py:validate_cursor`.
Regression: `tests/test_saved_events.py`, `tests/test_saved_events_valkey.py`, `tests/test_saved_events_transport.py`.
Distinctions: No events differs from cursor too old/invalid; outbox full rejects coupled mutation rather than dropping event; redacted event keeps identity.

### `state.staged_record` (W11)

mcp:staged:v1:record:{sha256(tenant)[:24]}:{operation_id}; logical retention 3600s plus 300s store margin; plan digest SHA256 sorted compact ensure_ascii JSON. Stages hold attempts, scope, enforcement, budget, deadlines, stop reason.

Source: `slopsearx/staged.py:StagedSearchStore._key`, `slopsearx/staged.py:plan_digest`, `slopsearx/staged.py:StagedSearchStore.read`.
Regression: `tests/test_staged_search.py`, `tests/test_staged_valkey_integration.py`.
Distinctions: Expired retained record differs from unknown ID; empty Lua cjson arrays must rehydrate as [] rather than {}.

### `state.staged_admission_index` (W11)

mcp:staged:v1:idem:{tenant_hash}:{sha256(key)}, index:{tenant_hash}, tenant marker, global ready/running queues; max 32 operations per tenant. Atomic admission gives replay/conflict/quota/unavailable; scan repairs lost index before admitting.

Source: `slopsearx/staged.py:StagedSearchStore.admit`, `slopsearx/staged.py:_ADMIT_SCRIPT`, `slopsearx/staged.py:StagedSearchStore._idem_key`.
Regression: `tests/test_staged_valkey_integration.py`, `tests/test_staged_search.py`.
Distinctions: Index missing with tenant marker invokes recovery; incomplete scan is unavailable, never false quota availability.

### `state.staged_lease_budget` (W11)

mcp:staged:v1:lease:{tenant_hash}:{operation_id}; 60s lease. Claim atomically reserves stage engine calls before dispatch; save_owned and renew fence stale tokens; recovery marks interrupted attempt and preserves consumed reservation. W06 consumes this store API in the runner.

Source: `slopsearx/staged.py:_CLAIM_SCRIPT`, `slopsearx/staged.py:StagedSearchStore.save_owned`, `slopsearx/staged.py:StagedSearchStore.recover_orphans`.
Regression: `tests/test_staged_valkey_integration.py`, `tests/test_staged_search.py`.
Distinctions: Budget exhausted/deadline expired differ from busy claim; null observed calls is unknown, not zero.

### `state.snapshot` (F13)

mcp:snapshot:{tenant}:{snap-12hex}; immutable complete results/scope/query_id/lineage, opaque result ID snapshot:index. Store TTL exceeds logical expires_at by 300s.

Source: `slopsearx/snapshot.py:SnapshotStore.create`, `slopsearx/snapshot.py:SnapshotStore.read`, `slopsearx/snapshot.py:SnapshotStore.result_id`.
Regression: `tests/test_mcp_snapshots.py`, `tests/test_cache_integration.py`.
Distinctions: store_unavailable, expired_handle, invalid_cursor distinct; malformed/foreign tenant record surfaces invalid_cursor without leaking content.

Schema fields from dataclasses: `SearchSnapshot`: `snapshot_id`, `query`, `query_id`, `results`, `scope`, `total`, `tenant`, `created_at`, `expires_at`, `ranking_explanation`, `lineage`, `lineage_available`

### `state.retrieval_receipt` (W07)

mcp:retrieval-receipt:v1 horizon/index/receipt/idem records per tenant and tokenized result ID. Fixed 86400s logical retention plus 300s store margin, max 20 receipts/result, 32 KiB receipt. Atomic insert enforces horizon, replay/conflict/capacity.

Source: `slopsearx/retrieval_receipts.py:ReceiptStore.submit`, `slopsearx/retrieval_receipts.py:ReceiptStore.replay`, `slopsearx/retrieval_receipts.py:ReceiptStore._horizon_key`.
Regression: `tests/test_retrieval_receipts.py`, `tests/test_retrieval_receipts_valkey.py`.
Distinctions: Missing/replayed/conflict/expired/unavailable are distinct; receipt records attest retrieval status, not verified document content.

### `state.oauth_client` (T04)

mcp:oauth:client:{client_id}; 30-day registration TTL. OAuthClientInformationFull is serialized as JSON. Valkey read/write/delete errors currently fall back to process memory, which is not cross-replica durable; F02 must decide explicit outage policy.

Source: `slopsearx/mcp/oauth.py:OAuthStateStore._key`, `slopsearx/mcp/oauth.py:SlopSearxOAuthProvider.register_client`.
Regression: `tests/test_mcp_oauth.py`.
Distinctions: Unknown client and storage outage currently both may read memory; invalid JSON is not a valid missing registration.

### `state.oauth_code` (T04)

mcp:oauth:code:{opaque_code}; AuthorizationCode stores client_id, scopes, expires_at, code_challenge, redirect_uri, resource, subject. Constructor code TTL defaults 600s; exchange issues token pair then deletes code. Single-use atomicity across replicas is an intentional hardening gap for F02/T04.

Source: `slopsearx/mcp/oauth.py:SlopSearxOAuthProvider.__init__`, `slopsearx/mcp/oauth.py:SlopSearxOAuthProvider.authorize`, `slopsearx/mcp/oauth.py:SlopSearxOAuthProvider.exchange_authorization_code`.
Regression: `tests/test_mcp_oauth.py`.
Distinctions: Missing/expired/wrong-client/replayed code must differ; current get-then-delete sequence is not atomic.

### `state.oauth_access` (T04)

mcp:oauth:access:{opaque_token}; AccessToken JSON includes client_id, scopes, expires_at, resource, subject plus linked refresh_token. TTL MCP_OAUTH_ACCESS_TOKEN_TTL_SECONDS default 3600. Revocation deletes pair.

Source: `slopsearx/mcp/oauth.py:SlopSearxOAuthProvider._issue_tokens`, `slopsearx/mcp/oauth.py:SlopSearxOAuthProvider.load_access_token`, `slopsearx/mcp/oauth.py:SlopSearxOAuthProvider.revoke_token`.
Regression: `tests/test_mcp_oauth.py`.
Distinctions: Missing, expired and revoked token differ conceptually; remote store outage currently falls back to process memory.

### `state.oauth_refresh` (T04)

mcp:oauth:refresh:{opaque_token}; RefreshToken JSON includes client_id, scopes, expires_at, subject plus linked access_token. TTL default 2592000. Refresh issues new pair and revokes prior pair; current rotation is not atomic across replicas.

Source: `slopsearx/mcp/oauth.py:SlopSearxOAuthProvider._issue_tokens`, `slopsearx/mcp/oauth.py:SlopSearxOAuthProvider.exchange_refresh_token`.
Regression: `tests/test_mcp_oauth.py`.
Distinctions: Wrong client, expired, revoked and simultaneous refresh replay must be separately classified; mixed old/new token state needs migration decision.

### `state.oauth_client_file` (T07)

Gateway client FileTokenStorage persists registered client info and tokens as 0600 JSON under per-server user config path; it is local client state, not Valkey shared authority. `_load` collapses read and JSON errors to {}; `get_client_info`/`get_tokens` return None for absent or falsey entries and do not check expiry themselves.

Source: `slopsearx/mcp/oauth_client.py:FileTokenStorage._load`, `slopsearx/mcp/oauth_client.py:FileTokenStorage.get_tokens`, `slopsearx/mcp/oauth_client.py:default_token_path`.
Regression: `tests/test_mcp_oauth_client.py`.
Distinctions: Missing file and read/malformed JSON errors collapse to empty storage; absent and falsey token entries collapse to None. A truthy invalid token may fail model validation. Token expiry handling belongs to the SDK/client flow, not this storage getter.

### `state.portal_oidc_transaction` (T09)

portal:oidc:v1:{HMAC(handle)}; 600s transaction containing state, nonce, PKCE verifier, return_to and created_at. Begin uses NX; complete validates and consumes atomically where supported, with previous HMAC key drain bounded 60..600s.

Source: `slopsearx/portal_oidc.py:OIDCIdentityProvider._transaction_key`, `slopsearx/portal_oidc.py:OIDCIdentityProvider.begin`, `slopsearx/portal_oidc.py:OIDCIdentityProvider.complete`.
Regression: `tests/test_portal_oidc.py`.
Distinctions: Missing/expired/consumed/mismatched state, nonce and IdP claims differ; unavailable state fails protected login.

### `state.portal_session` (T09)

portal:session:v1:{HMAC(handle)}; PortalSession fields principal_id, issuer, active_tenant, authenticated_at/created_at/last_seen_at, idle/absolute expiry, principal/membership revisions, csrf_secret, generation, refresh_revision. Idle 1800s, absolute 28800s; rotate/replace/revoke/refresh are fenced.

Source: `slopsearx/portal_auth.py:PortalSession`, `slopsearx/portal_auth.py:PortalSessionStore._key`, `slopsearx/portal_auth.py:PortalSessionStore.resolve`.
Regression: `tests/test_portal_auth.py`, `tests/test_workflow_portal.py`.
Distinctions: No cookie, expired, revoked, unavailable and valid session differ; active_tenant null requires selection; membership revision changes invalidate authority.

Schema fields from dataclasses: `PortalSession`: `principal_id`, `issuer`, `active_tenant`, `authenticated_at`, `created_at`, `last_seen_at`, `idle_expires_at`, `absolute_expires_at`, `principal_revision`, `membership_revision`, `csrf_secret`, `generation`, `refresh_revision`

### `state.portal_membership` (T09)

PortalPrincipal/PortalMembership are resolved by MembershipResolver; StaticMembershipResolver is configured in process, not a Valkey record. Session captures revisions and current membership is rechecked on resolve. Tenant choice rotates session and CSRF secret.

Source: `slopsearx/portal_auth.py:PortalMembership`, `slopsearx/portal_auth.py:MembershipResolver`, `slopsearx/portal_auth.py:StaticMembershipResolver`, `slopsearx/portal_auth.py:PortalSessionStore.rotate`.
Regression: `tests/test_portal_auth.py`, `tests/test_workflow_console_policy.py`.
Distinctions: No membership, inactive tenant, changed role/revision, and null active tenant are distinct denials/choices.

### `state.portal_mutation` (W10)

portal:mutation:v1:{sha256(principal_id NUL tenant_id NUL action NUL idem)}; 600s NX pending claim then done result. Claim ID fences finishing. T09 owns identity/session authority consumed by this browser mutation.

Source: `slopsearx/workflow_portal.py:MutationGuard._key`, `slopsearx/workflow_portal.py:MutationGuard.claim`, `slopsearx/workflow_portal.py:MutationGuard.finish`.
Regression: `tests/test_workflow_portal.py`, `tests/test_workflow_console_policy.py`.
Distinctions: Claimed, inflight, replay, conflict and unavailable differ; pending after crash needs explicit retry/expiry behavior.

### `state.engine_rate_window` (F10)

ratelimit:{engine}:{int(monotonic/window)} distributed counter, INCRBY then EXPIRE at 2*window on first increment. This uses process monotonic time in a cross-replica key and is an explicit F10 hardening target; fail_closed controls unavailable behavior.

Source: `slopsearx/ratelimit.py:ValkeySlidingWindow.acquire`.
Regression: `tests/test_ratelimit.py`.
Distinctions: Counter absent, over limit and Valkey unavailable differ; mixed process clock offsets can make windows inconsistent.

### `state.upstream_cooldown` (F10)

ratelimit:upstream-cooldown:{engine}; Lua sets or extends bounded 1..300s cooldown, default 30s. Read is best-effort and unavailable returns 0.

Source: `slopsearx/ratelimit.py:ValkeySlidingWindow._cooldown_key`, `slopsearx/ratelimit.py:ValkeySlidingWindow.set_upstream_cooldown`.
Regression: `tests/test_ratelimit.py`, `tests/test_observed_health.py`.
Distinctions: No cooldown and unavailable both currently return zero but have different evidence; zero must not be reported as observed healthy.

### `state.portal_rate_limit` (T09)

portal:limit:v1:{bucket}:{HMAC(bucket NUL identity NUL wall-clock slot)}; increment counter with TTL window+1. Protected routes fail closed on disconnected store or Lua error.

Source: `slopsearx/portal_rate_limit.py:PortalRateLimiter._storage_key`, `slopsearx/portal_rate_limit.py:PortalRateLimiter.allow`.
Regression: `tests/test_workflow_portal.py`, `tests/test_portal_auth.py`.
Distinctions: Limit exceeded and store unavailable both deny, but are distinct causes; bucket identity remains pseudonymous.

### `state.portal_audit` (T09)

portal:audit:v1:{UTC date} bounded Valkey stream, MAXLEN approximate 10000, TTL 7776000s. Closed action/outcome/reason vocabularies and HMAC-pseudonymized principal/tenant/object fields.

Source: `slopsearx/portal_audit.py:PortalAuditLogger.record`.
Regression: `tests/test_workflow_portal.py`, `tests/test_portal_auth.py`.
Distinctions: Audit skip on unavailable store differs from a recorded denial; unknown vocabulary raises before write.

### `state.query_audit` (O01)

query_audit:{local date} stream, approximate 10000 entries/day, TTL 7776000s; query text, client IP, engine counts/status and latency. Best effort, no-op on unavailable store.

Source: `slopsearx/audit.py:QueryAuditLogger._stream_key`, `slopsearx/audit.py:QueryAuditLogger.record_query`.
Regression: `tests/test_metrics.py`, `tests/test_server.py`.
Distinctions: Missing audit record from unavailable store differs from no search; privacy/redaction review required before Rust parity.

### `state.engine_stats` (O01)

engine_stats:{engine}:{local date} hash; queries/results/errors/rate_limited/total_latency_ms/total_score counters with TTL 7776000s. Best effort.

Source: `slopsearx/stats.py:EngineStatsTracker._daily_key`, `slopsearx/stats.py:EngineStatsTracker.record_query`.
Regression: `tests/test_metrics.py`, `tests/test_observed_health.py`.
Distinctions: No hash is not observed zero queries; write failure must not become search failure.

### `state.suggestion_cache` (F13)

suggest:{sha256(lowercase(trim(query)))} stores {suggestions: list} for 1800s, including empty list after providers fail. Brave primary, DDG fallback.

Source: `slopsearx/suggest.py:_suggest_cache_key`, `slopsearx/suggest.py:SuggestionService.fetch`.
Regression: `tests/test_service.py`, `tests/test_cache.py`.
Distinctions: Cached empty suggestions differ from cache miss; unavailable cache still allows provider fallback.

### `state.canonical_search_cache` (F13)

search:v2:{sha256(scoped compact JSON)} stores full canonical unsliced SearchResponse payload. TTL general 3600s, news at most 300s, partial at most 30s by default; no tenant key segment, so any tenant-sensitive result must be prevented by authority/scope policy.

Source: `slopsearx/cache.py:cache_key`, `slopsearx/service.py:_scope_cache_key`, `slopsearx/service.py:SearchService._write_cache`.
Regression: `tests/test_cache.py`, `tests/test_cache_integration.py`.
Distinctions: Cache miss, corrupt record and unavailable store all trigger recomputation; negative _error sentinel is a separate record shape.

### `state.answer_cache` (F13)

answer:v2:{sha256(trim(query))} stores answer-level payload by query only with general TTL. Language/SafeSearch variants share this legacy answer key; migration must preserve or explicitly retire separate use.

Source: `slopsearx/cache.py:_answer_cache_key`, `slopsearx/cache.py:SearchCache.get_answer`, `slopsearx/cache.py:SearchCache.set_answer`.
Regression: `tests/test_cache.py`.
Distinctions: Absent answer differs from empty answer payload; answer cache is broader than canonical search scope.

## Open parity evidence

Every record requires native fixture execution and exact-artifact result binding. Resolve the following during F02/F05–F15, W00–W11, and A01–A14:

- Confirm cross-language byte compatibility for `stable_id`, staged `plan_digest`, cache fingerprints and durable identifiers; JSON key order, escaping and Unicode casefold can change IDs.
- Validate every namespace and TTL margin on real Valkey, including expired versus unknown and unavailable versus missing outcomes. Do not infer durable semantics from in-memory fakes.
- Capture per-adapter wire request order/count, authentication placement, parser failures and opt-in behavior from deterministic fixtures. Existing test paths in the plan are mandatory migration targets even where their exact fixture census is pending.
- Reconcile runtime config readers with `load_config`; notably `CacheConfig.ttl_seconds=300` and `SearchCache` default `3600` are both current source facts and must not be silently collapsed.
- Add source-specific fixture details for scholarly grouping, Jev advice, reranking and health. A completed Python test is baseline evidence, not a Rust parity pass.
