# Changelog

## Unreleased

### Features

* expose a machine-readable search-to-retrieval handoff record (`retrieval`,
  contract `slopsearx.retrieval_handoff` v1) on every MCP result card and
  expanded record: result identity, the raw result URL handed off verbatim
  when eligible, a closed
  `url_status` classification (`ok`/`missing`/`non_http`/`unsafe_scheme`/
  `ambiguous`) with stable reasons, snippet-only/non-verification status, and
  snapshot/query provenance so a downstream retriever (e.g. GroktoCrawl) can
  associate a capture with the originating result without parsing prose;
  ineligible URLs are never handed off as fetch targets
* make result URL normalization robust to canonicalization-ambiguous URLs so a
  malformed result URL is classified `ambiguous` at the handoff boundary
  instead of failing the whole search
* derive live engine health from observed search outcomes and
  circuit-breaker/auth state: `/health`, the MCP status surface, and the
  capability catalog now share one status vocabulary and freshness timestamp,
  keep never-observed engines explicitly `unknown`, mark stale observations
  stale, and expose circuit/auth signals separately (no active probing)
* make research job execution durable across replicas: Valkey-backed lease
  claim with ownership, visibility timeout, orphan recovery, cancellation
  flags, idempotent duplicate-delivery, bounded per-replica concurrency, and
  machine-discoverable `research_execution.mode` in `slopsearx_get_service_status`
* derive MCP tenant identity per-request from the authenticated OAuth client
  id (falling back to a single default tenant) so jobs, snapshots, and
  rate-limit/audit identifiers never bleed across concurrent requests
* add MCP server (`slopsearx-mcp`): 15 intent-level tools, capability
  discovery, scope explanation, snapshot pagination, research jobs (including
  selective retry and bounded follow-up), and
  bearer-token HTTP transport — see `docs/MCP_SERVER.md`
* add remote gateway mode (`slopsearx-mcp --remote <url>`): a stdio MCP
  server that proxies tools, resources, and prompts to a remote SlopSearX
  MCP server over streamable HTTP, so the agent host needs no server wiring
* add MCP OAuth 2.1 authorization-server mode (dynamic client registration,
  PKCE, revocation) so OAuth-requiring clients such as Claude Web
  connectors can connect; static bearer-token auth remains an alternative
* add the matching OAuth 2.1 *client* flow to the gateway
  (`slopsearx-mcp --remote <url> --oauth`): loopback-callback
  authorization, PKCE, and token persistence in a 0600 file so later runs
  reuse tokens without re-authorizing; `--oauth`/`--token` are mutually
  exclusive
* extract shared `SearchService`/`ScopeResolver` pipeline used by both the
  HTTP API and the MCP server
* scope cache keys to include categories, engines, page, and time range so
  cached responses can never cross search scopes; cache the canonical full
  response and derive the requested `include`/`max_results` view at the MCP
  read boundary so representation never depends on the request that populated
  the cache
* add runtime capability catalog (`slopsearx/capabilities.py`) generated
  from the live engine registry, with intent profiles and an operator
  policy model for MCP grants, bounds, and sensitive engines
* unify sensitive-engine and specialist-grant enforcement behind one shared
  policy gate applied to every search path (generic explicit engines,
  targeted, jobs, security, science); `MCP_TARGETED_SENSITIVE_ALLOWED` is the
  single grant that permits sensitive engines, applied uniformly
* add a structured per-filter enforcement report (`enforcement`) to every
  search tool with statuses `enforced`/`partially_enforced`/`unsupported`/
  `rejected`, replacing prose-only filter warnings
* make `SearchResult` serialization JSON-safe (set → sorted list) so engine
  provenance survives cache and snapshot round-trips
* harden snapshot/cursor lifecycle with explicit `expired_handle` and
  `store_unavailable` semantics and JSON-safe provenance through snapshots
* persist per-query and per-engine research coverage with structured failure
  classes; add selective retry (`slopsearx_retry_research`) and bounded
  follow-up (`slopsearx_extend_research`)
* expose the full engine capability matrix and curated non-secret operational
  diagnostics (service + MCP contract version, Valkey state, engine count,
  grants by name, health by status class, policy bounds, degradation summary)

### Documentation

* add the search-to-retrieval handoff contract (`docs/RETRIEVAL_HANDOFF.md`):
  the search-only boundary, the `retrieval` handoff record schema and URL
  eligibility vocabulary, capture-association semantics, and GroktoCrawl as a
  composition option without an undocumented runtime integration claim;
  document the `retrieval` mapping in `docs/MCP_CONTRACT.md`, `docs/MCP_SERVER.md`,
  `docs/MCP_SERVER_DESIGN.md`, and `spec.md`, with contract fixtures under
  `tests/fixtures/retrieval_handoff/`
* add end-user and agent install/configuration guide for the MCP server
  (`docs/MCP_SERVER.md`); reconcile the README engine table with the live
  51-adapter registry
* fix the stale 48-engine count in `docs/ENGINE_ADAPTERS.md` and `AGENTS.md`
  (51 registered adapters, including the jobs adapters `ashby`, `greenhouse`,
  `lever`); correct the cache-scoping, sensitive-engine-grant, and snapshot
  error-mapping claims to match the implemented behavior

## [0.3.0](https://github.com/magnus919/SlopSearX/compare/v0.2.0...v0.3.0) (2026-08-22)


### Features

* add curated non-secret operational diagnostics to status and health ([28c39f3](https://github.com/magnus919/SlopSearX/commit/28c39f3d7d3da6c068677afab6a2824235f060b0))
* add deterministic MCP fixture harness over streamable HTTP ([def7497](https://github.com/magnus919/SlopSearX/commit/def749765d53b6bd421d3c3c0925fee16a912e77))
* add explainable cost- and coverage-aware routing ([b8103fc](https://github.com/magnus919/SlopSearX/commit/b8103fc3406afc5025c058d62ce449d3fbed13ab))
* add explainable cost- and coverage-aware routing ([d11d105](https://github.com/magnus919/SlopSearX/commit/d11d105e193fe876eb757616bb9e0b6a57a5e89f))
* add explicit image/video media search and result contract ([2dceac1](https://github.com/magnus919/SlopSearX/commit/2dceac137867e50dbc7c9be7f8c6cbd70bd548be))
* add explicit media search and result contract ([85e710b](https://github.com/magnus919/SlopSearX/commit/85e710bc4775a9005aca4714ec86bc1fe5a6bbf9))
* add jobs search topic and ATS engine adapters (Greenhouse, Ashby, Lever) ([#132](https://github.com/magnus919/SlopSearX/issues/132)) ([dfd517c](https://github.com/magnus919/SlopSearX/commit/dfd517c545a1d84c2a8f4be04a651694e6c18cc1))
* add MCP server for AI agents ([af763ab](https://github.com/magnus919/SlopSearX/commit/af763ab9c6a35f8635a48ea47e0f49fac3e00a4c))
* add MCP server for AI agents ([c1ae16f](https://github.com/magnus919/SlopSearX/commit/c1ae16f21c3be6013b2bd8987c836113aac07c27))
* add selective research retry and bounded follow-up ([2d5e8c6](https://github.com/magnus919/SlopSearX/commit/2d5e8c6977cdcb9573c8619f9b247953d4e42baf))
* add structured filter-enforcement report to MCP search tools ([9797ac7](https://github.com/magnus919/SlopSearX/commit/9797ac7c9280888088fb4967697f5dea7e3cdea1))
* audit and declare engine capability metadata ([2e32f3c](https://github.com/magnus919/SlopSearX/commit/2e32f3c27407c27ca7aae8ff91c6de75abae7fbe))
* audit and declare engine capability metadata ([18e9736](https://github.com/magnus919/SlopSearX/commit/18e97367c4fe0a9ab0dc467acb10e9d7545be5cb))
* derive live engine health from observed outcomes ([0fd7bde](https://github.com/magnus919/SlopSearX/commit/0fd7bde997303a323eb6bca044dd662e755d61b8))
* derive live engine health from observed outcomes ([4ff5000](https://github.com/magnus919/SlopSearX/commit/4ff5000080fcc483f0af4687d64bc8d17da42c5e))
* diagnose empty scrape responses ([51ee882](https://github.com/magnus919/SlopSearX/commit/51ee882c96f220a404aa87f11df12459ecf87086))
* diagnose empty scrape responses ([6004d81](https://github.com/magnus919/SlopSearX/commit/6004d8149c9cbdf845649a48f0283048071cc998))
* enforce and report search filter semantics ([f3ae26a](https://github.com/magnus919/SlopSearX/commit/f3ae26a712bf1454dc05f60f2bab1567dd1eb03e))
* enforce and report search filter semantics ([b26d1f3](https://github.com/magnus919/SlopSearX/commit/b26d1f38c4aa3e22613e11f9ecca459bc29e7c84))
* expose full engine capability matrix from the live catalog ([6fe1c5c](https://github.com/magnus919/SlopSearX/commit/6fe1c5c2a4e8d1646124ac87207ec178a9bd9c5e))
* expose machine-readable search-to-retrieval handoff boundary ([c1c510a](https://github.com/magnus919/SlopSearX/commit/c1c510a862f67761fc1bd97f114bdcd2e4ecb4ec))
* full-strength MCP access for SlopSearX ([f922728](https://github.com/magnus919/SlopSearX/commit/f9227280e6c2a3ef95364a954e9b75a5eb25a5fd))
* harden snapshot/cursor lifecycle with expiry and unavailability semantics ([87e2781](https://github.com/magnus919/SlopSearX/commit/87e27819e79c0dbd321cea03f811f150c9a26d46))
* implement progressive-disclosure result card/record contract ([c788662](https://github.com/magnus919/SlopSearX/commit/c788662027bc720f65cfa72ca36dddfa415b4a7e))
* make research job execution durable across replicas ([7c9993e](https://github.com/magnus919/SlopSearX/commit/7c9993e50e0a03fe571479712bd4874386ac19de))
* make research job execution durable across replicas ([0b370be](https://github.com/magnus919/SlopSearX/commit/0b370be0821cbe6de0a794257be12e778228e698))
* persist research per-query and per-engine coverage with failure classes ([5365ba1](https://github.com/magnus919/SlopSearX/commit/5365ba180a3da6e4113853015002043c8723a5b8))
* preserve typed domain payloads in normalized results ([9b020c8](https://github.com/magnus919/SlopSearX/commit/9b020c88380736a56a198fd7ae3880a4189eb234))
* preserve typed domain payloads in normalized results ([6c8f4ef](https://github.com/magnus919/SlopSearX/commit/6c8f4ef2fa51bd2297b4b475c881b811f5e33ca8))
* raise Brave default result cap ([#207](https://github.com/magnus919/SlopSearX/issues/207)) ([03b294f](https://github.com/magnus919/SlopSearX/commit/03b294f0a7a4b7652e1f6430f0d5cf3722ad2874))
* recover discarded evidence in the MCP search envelope ([192b8a9](https://github.com/magnus919/SlopSearX/commit/192b8a916594703eee55f489477a7986fab7e9fa))
* unify sensitive-engine and specialist-grant policy gate ([61cc3f3](https://github.com/magnus919/SlopSearX/commit/61cc3f3b572a9209232e2061fae254f7691f8065))


### Bug Fixes

* add type args to bare dict in __init__ for mypy 2.2.0 ([9bc5b1a](https://github.com/magnus919/SlopSearX/commit/9bc5b1a684a9f53c8d48cc9e808ddbd16488066f))
* align payload/media disclosure gates with persistence boundary ([cb423d4](https://github.com/magnus919/SlopSearX/commit/cb423d476dec514fde915f702eabc8381cc94a86))
* align payload/media disclosure gates with persistence boundary ([ef7b065](https://github.com/magnus919/SlopSearX/commit/ef7b065e0be6e93bf6992e8b3eb0989f738a2d45))
* apply base-image security updates ([#209](https://github.com/magnus919/SlopSearX/issues/209)) ([851090f](https://github.com/magnus919/SlopSearX/commit/851090f246802097bc6f460923351c325bd688f3))
* apply persistence bound to HTTP payload output gate ([e57963d](https://github.com/magnus919/SlopSearX/commit/e57963da9e29a844018f4e8829c8325cf4f32b37))
* bound the aggregate dispatch deadline ([0f78d0d](https://github.com/magnus919/SlopSearX/commit/0f78d0da3ebfb64a78f306312b022eb2244d6168))
* **brave:** load API key from environment variable as fallback ([9096879](https://github.com/magnus919/SlopSearX/commit/90968797d73953fb448a7b9506d70e4cca228e3f))
* **brave:** load API key from environment variable as fallback ([9096879](https://github.com/magnus919/SlopSearX/commit/90968797d73953fb448a7b9506d70e4cca228e3f))
* **brave:** load API key from environment variable as fallback ([8f7f8b7](https://github.com/magnus919/SlopSearX/commit/8f7f8b77837f0b44581ace9107aa40053a2a2efc))
* **brave:** load API key in __init__ so health check passes at startup ([9becad0](https://github.com/magnus919/SlopSearX/commit/9becad0d7b9ee954c76d7c90e514278fe4f1e416))
* **brave:** load API key in __init__ so health check passes at startup ([9becad0](https://github.com/magnus919/SlopSearX/commit/9becad0d7b9ee954c76d7c90e514278fe4f1e416))
* **brave:** load API key in __init__ so health check passes at startup ([f833807](https://github.com/magnus919/SlopSearX/commit/f8338079db3b8abec1d94cbe483a5f5d247d5b11))
* cache canonical full response and derive per-request view ([9c64dbc](https://github.com/magnus919/SlopSearX/commit/9c64dbcb96dcf00749a500959d807a8d34652caf))
* centralize feature env overrides ([6031f62](https://github.com/magnus919/SlopSearX/commit/6031f625a97283acccd9113066b90a7a3535de5b))
* centralize feature env overrides ([bebdaa3](https://github.com/magnus919/SlopSearX/commit/bebdaa34043989e9e3121285e37e28e8f2169c78))
* **ci:** skip droid-review for dependabot PRs ([a14910b](https://github.com/magnus919/SlopSearX/commit/a14910b33bdac1300cbae957ff326f1c5e5dfa6f))
* clear stale research leases, guard saves, use SCAN ([a2a697d](https://github.com/magnus919/SlopSearX/commit/a2a697d8af35019062f9c9bbf360bd4157c26a65))
* close media routing gaps from issue-188 code review ([e088544](https://github.com/magnus919/SlopSearX/commit/e088544c992a77563fe1a1d2149f91032bd2e20e))
* close SSRF gaps in retrieval URL handoff guard ([3112c20](https://github.com/magnus919/SlopSearX/commit/3112c20c098404d7553cf4663a453fce12d08ed8))
* complete timeout status follow-up contract ([f11e15f](https://github.com/magnus919/SlopSearX/commit/f11e15fd8c937c19545943e6eb07deb884fda761))
* expire deadline-passed claims and guard direct runs ([fc2b789](https://github.com/magnus919/SlopSearX/commit/fc2b789e9cd229294e5846a44ec543496da348bf))
* finalize lapsed-deadline runs before mutation, scan colon-safe tenants ([40afa0c](https://github.com/magnus919/SlopSearX/commit/40afa0cee77de02e422974b7b12b587fb9f0b57a))
* fold observed health into routing cache digest ([dc19d08](https://github.com/magnus919/SlopSearX/commit/dc19d088531e2f0a45581cc6374417d7962048a2))
* fold sensitive/tier1 sets into routing digest; freeze HTTP budget ([f15a73b](https://github.com/magnus919/SlopSearX/commit/f15a73b06d8978634ed114381c7c7f7332e1a7e8))
* fold sensitive/tier1 sets into routing digest; freeze HTTP budget ([f3e084f](https://github.com/magnus919/SlopSearX/commit/f3e084f686167402060688f36981cbe6875e61fc))
* gate cancel/retry/extend and keep research leases alive ([78ac88a](https://github.com/magnus919/SlopSearX/commit/78ac88a6fe2bcc7eb92d9db6b42f8e57a83b59b1))
* gate health folding in routing digest on engine-count cap ([c8739ac](https://github.com/magnus919/SlopSearX/commit/c8739acaba5597f3ebb265a3c0ccd61c5e14c0d7))
* gate payload serialization the renderer way and cap requested inline ([a298d5a](https://github.com/magnus919/SlopSearX/commit/a298d5acbc6bab03381e59c963f55930325c9d4b))
* handle arXiv HTTPS redirects safely ([8985874](https://github.com/magnus919/SlopSearX/commit/8985874718c0afe50e9e2233bd32ff10f913f4d7))
* harden payload disclosure and CVSS parity ([0957a1e](https://github.com/magnus919/SlopSearX/commit/0957a1ecffc362c9edeac40071bd0ae42d834fce))
* harden payload serialization, persistence bound, and CVSS parity ([f780274](https://github.com/magnus919/SlopSearX/commit/f7802747e99982e56f7ce275efea7cf512f11fbe))
* harden retrieval handoff URL classification and docs ([45fd16c](https://github.com/magnus919/SlopSearX/commit/45fd16c1c7ca6165074ae35c7a2d94f80391d0c9))
* harden snapshot TTL and research retry/extend edge cases ([e3a04f9](https://github.com/magnus919/SlopSearX/commit/e3a04f9737ddb5b8720d11b3dffdb9f2fe536db1))
* harden URL handoff guard and dedup against malformed hosts ([4c99744](https://github.com/magnus919/SlopSearX/commit/4c9974431d75e26ead8c47e3dc857de1bacb9896))
* honor per-engine timeout_ms at search dispatch ([#184](https://github.com/magnus919/SlopSearX/issues/184)) ([23dc38c](https://github.com/magnus919/SlopSearX/commit/23dc38c92b217c78c70c4c4b7f02bf0feb9b2a36))
* install cssselect at runtime ([2433336](https://github.com/magnus919/SlopSearX/commit/243333698ce2f0f7d7550f090af262f35e5afc5e))
* install cssselect at runtime ([e859bc2](https://github.com/magnus919/SlopSearX/commit/e859bc273e7e30089c9b1e73b60ee0f08441cd4c))
* keep cached scope live and pin routing test config ([d5cd05a](https://github.com/magnus919/SlopSearX/commit/d5cd05ac90a944551ab0ace391c481b81dc9dfa6))
* keep intent media type across explicit scope in _resolve_scope ([f97f578](https://github.com/magnus919/SlopSearX/commit/f97f578300eb47d9267c4b2cc41059416ecd3bb7))
* lease direct research runs and check lease liveness ([3999081](https://github.com/magnus919/SlopSearX/commit/3999081f9eb0547eb03ca382f47ae06e1cb25ed9))
* make filter enforcement value-aware and honest in warnings ([0924989](https://github.com/magnus919/SlopSearX/commit/0924989f9056632db2ae6d595ebf7f0c9652e0ab))
* make research lease primitives atomic and claim terminal retries ([f1da60c](https://github.com/magnus919/SlopSearX/commit/f1da60c6cf08efb259858e4e6456be285de5e775))
* make SearchResult serialization JSON-safe ([6b7447a](https://github.com/magnus919/SlopSearX/commit/6b7447a35be5657eb610512c1f3c552c22ef7d4f))
* memoize /health config+catalog and never fabricate observed latency ([8d77e38](https://github.com/magnus919/SlopSearX/commit/8d77e3830f7e60cc24596b8685cec67ad6a7e019))
* never fabricate observed health latency or auth state ([0005392](https://github.com/magnus919/SlopSearX/commit/00053923eef11a8782873bb83e42cda22e0425b1))
* never hand off SSRF-prone IP hosts or userinfo URLs ([9f1730c](https://github.com/magnus919/SlopSearX/commit/9f1730c750b3a3ae4404c6d9dcc40eee4e47a71d))
* only count the local layer for filters with a local post-filter ([b44b20b](https://github.com/magnus919/SlopSearX/commit/b44b20b3acee3f40d7b3d4f10788387428c42472))
* pin Docker image provenance ([#211](https://github.com/magnus919/SlopSearX/issues/211)) ([77d1448](https://github.com/magnus919/SlopSearX/commit/77d1448727c35c830af04acde0893d20fc160ef2))
* pin sanitized error-message assertion to exact output ([32c47fd](https://github.com/magnus919/SlopSearX/commit/32c47fda739af1213e795f7cf689b3fcba58ad76))
* preserve arXiv rate-limit classification ([28d4670](https://github.com/magnus919/SlopSearX/commit/28d4670f729ccc97a9cf538467fb3e5b92e4923c))
* preserve configured timeout in fan-out deadline ([9ab5c37](https://github.com/magnus919/SlopSearX/commit/9ab5c37df02ffd98d3b855d092b4d12ab4c1ac29))
* **ratelimit:** remove unused sidecar stub ([9b22616](https://github.com/magnus919/SlopSearX/commit/9b22616ae6c718b7b64673ac0978354f6cab5084))
* **ratelimit:** remove unused sidecar stub ([9ae0dad](https://github.com/magnus919/SlopSearX/commit/9ae0dadaa05e4059f9b33533b625ec4ecce2b03d))
* reconcile direct runs and surface durable cancellation ([de1f754](https://github.com/magnus919/SlopSearX/commit/de1f75414db358e425f5ca61b4425205bec3f33d))
* reject IPv6 6to4 literals and distinguish reserved-prefix reasons ([635bc6c](https://github.com/magnus919/SlopSearX/commit/635bc6c5aa8d4fecaaa48dd093ea3a85f9a9f41a))
* reject IPv6 translation-prefix and site-local retrieval targets ([a18d889](https://github.com/magnus919/SlopSearX/commit/a18d88955330b2d721d29c5acd0d5a5285b8b61b))
* reject IPv6 translation-prefix and site-local retrieval targets ([427ade1](https://github.com/magnus919/SlopSearX/commit/427ade12ce3ad1f5f750efb4937ba4f293d05291))
* reject percent-encoded and non-ASCII hosts in retrieval handoff ([7c52d2a](https://github.com/magnus919/SlopSearX/commit/7c52d2a4217f60e55964c54200be7b14211fb860))
* remove dead payload catalog and gate read-result payload ([f495e58](https://github.com/magnus919/SlopSearX/commit/f495e586b840bc527a7e7abbaacb3d4c70b51324))
* route strict-safesearch preview through the real query scope ([373b52e](https://github.com/magnus919/SlopSearX/commit/373b52ead486e50f8d7af784cf0af032df2eb718))
* run routing pass on media path and pin harness config ([7e3c0df](https://github.com/magnus919/SlopSearX/commit/7e3c0dff883a73713035534fbb611a4d631198d6))
* satisfy both mypy gate versions for MCP SDK redirect helper ([de3d939](https://github.com/magnus919/SlopSearX/commit/de3d93979021933a836345c0646c49120cd8fa53))
* scope IPv6 reserved check so IPv4-mapped literals embed-safe ([776c431](https://github.com/magnus919/SlopSearX/commit/776c4318c5fd1d00ee610304f900df52f5d0412b))
* strict payload size gating and CVSS/FRED fixes ([64b2007](https://github.com/magnus919/SlopSearX/commit/64b200725450aaabaf96c2bb39d645c2eb45a14c))
* sync ctx sensitive set from policy; harden routing eval bounds ([965e1f3](https://github.com/magnus919/SlopSearX/commit/965e1f359ca39616c41e3fe0f70c68e7c0b32630))
* treat empty max_cost_class as permissive; fix R5 eval baseline ([f615341](https://github.com/magnus919/SlopSearX/commit/f615341229a40fc4b1ce3b8001bc54c23a3dfad4))


### Documentation

* add engine troubleshooting guidance ([f350d49](https://github.com/magnus919/SlopSearX/commit/f350d496fcf93bbf02cf2920849e8f713e06b2dc))
* add engine troubleshooting guidance ([59e796c](https://github.com/magnus919/SlopSearX/commit/59e796ca1262f829f51c666479c54c552f136af4))
* add field-level MCP contract mapping ([07186a7](https://github.com/magnus919/SlopSearX/commit/07186a7cc26eb2823c1c97a14f5c7b2bb836f30f))
* correct stale claims and document MCP invariants ([0f4f725](https://github.com/magnus919/SlopSearX/commit/0f4f725d7a5853a442b3aafa67063839c2045a5d))
* define the search-to-retrieval handoff boundary ([4dd4e8a](https://github.com/magnus919/SlopSearX/commit/4dd4e8a9495c75220a2f286c8bf53310c4251d5b))
* document advanced-search decision and finalize CI parity ([7da23a6](https://github.com/magnus919/SlopSearX/commit/7da23a6228cb63fa3c21f7f294308dd4a116bcf2))
* expose unavailable health status ([95d9a1b](https://github.com/magnus919/SlopSearX/commit/95d9a1b2f69e878cbb6c8fd3eab9e178e6e61ca9))

## [0.2.0](https://github.com/magnus919/SlopSearX/compare/v0.1.1...v0.2.0) (2026-07-02)


### Features

* add error tracking, alerting, product analytics, and error-to-insight pipeline ([77caf77](https://github.com/magnus919/SlopSearX/commit/77caf77c044bc61b3a5705368d386d5b66f66cbc))
* add error tracking, alerting, product analytics, and error-to-insight pipeline ([7a7eeea](https://github.com/magnus919/SlopSearX/commit/7a7eeea71a875dbd39f07e55ea8c9bcdc7dd3f5e))
* add feature flag infrastructure and regenerate wiki ([5864cdf](https://github.com/magnus919/SlopSearX/commit/5864cdf86c0c82c848a94d9f0be9e24f62a67fd4))
* add image search support to DuckDuckGo adapter ([#124](https://github.com/magnus919/SlopSearX/issues/124)) ([5bc615e](https://github.com/magnus919/SlopSearX/commit/5bc615e47b1e7eca9c1c370940c0b9d58f7a0a12))
* add pre-commit hooks, complexity, dead-code, duplicate detection, and import-linter ([b2ff3a3](https://github.com/magnus919/SlopSearX/commit/b2ff3a38c12b14dffa5a8fb11d0728c6714491e1))
* add pre-commit hooks, complexity/dead-code/duplicate detection, and import-linter ([8d26d55](https://github.com/magnus919/SlopSearX/commit/8d26d552d4fb284d39e6ff1acc275fffdc34a22f))
* aggressive caching, circuit breaker, query audit trail ([#92](https://github.com/magnus919/SlopSearX/issues/92)) ([#93](https://github.com/magnus919/SlopSearX/issues/93)) ([54bd5be](https://github.com/magnus919/SlopSearX/commit/54bd5becfa99c9924ea17a9f6efca35f52f5e9f6))
* fix 12 remaining Agent Readiness signals ([f4a230a](https://github.com/magnus919/SlopSearX/commit/f4a230acf2081762abc97c3618e157d516c0a27b))
* fix 12 remaining Agent Readiness signals ([615849d](https://github.com/magnus919/SlopSearX/commit/615849d70aa38073ad0d4200f225070bfb107715))


### Bug Fixes

* correct CI regressions from pipeline hardening ([ca3e52b](https://github.com/magnus919/SlopSearX/commit/ca3e52b542bfa32132f8906ff3d45d7f48864943))
* integration tests for CI without Valkey ([1e1e89d](https://github.com/magnus919/SlopSearX/commit/1e1e89d2d1955472d83e2e4eaf26a762bf39c249))
* route Brave adapter by category instead of always hitting /web ([13133e7](https://github.com/magnus919/SlopSearX/commit/13133e79b663a8ef7b2d392d7cd77948e96f3e5c)), closes [#123](https://github.com/magnus919/SlopSearX/issues/123)
* stop hitting Brave API in health checks ([393a746](https://github.com/magnus919/SlopSearX/commit/393a7464d9e5ecffa2f6457c661a2aed883cf50d))


### Documentation

* add feature flag workflow and pre-commit guidance ([1c7a2cf](https://github.com/magnus919/SlopSearX/commit/1c7a2cf7ff3005f8a34e06342651fb25e63e98d2))
