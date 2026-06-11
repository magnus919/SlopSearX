# Glossary

| Term | Definition |
|---|---|
| **Adapter** | A Python class that wraps a single search engine's API or scrape interface. Each adapter lives in one file in `engines/` and is registered via `@register_engine`. |
| **AdapterResponse** | The canonical return type for every adapter's `search()` method. Contains a list of `SearchResult`, a status enum, optional error message, and measured latency. |
| **Backpressure** | A feedback mechanism where the rate limiter signals upstream clients (or the orchestrator) to slow down when the system approaches capacity limits. Implemented via Valkey sliding window counters. |
| **Category routing** | Filtering of engines by SearXNG-compatible category tags. Clients can pass `?categories=science,news` to restrict results to engines declaring those categories. |
| **EngineAdapter** | Abstract base class for API-based engines (Brave, Wikipedia, etc.) |
| **EngineStatsTracker** | Subsystem in `slopsearx/stats.py` that records per-engine quality telemetry (success/failure counts, latency histograms) in Valkey after each dispatch. Enables adaptive engine selection and observability. |
| **PresenceRanker** | V1 ranking strategy that boosts results appearing in multiple engines. Provides breadth at the cost of precision. |
| **ProxyPool** | Subsystem in `slopsearx/proxypool.py` that manages a configurable list of proxy URLs for scrape-based adapters. Provides round-robin proxy rotation to evade IP bans and CAPTCHA walls. |
| **QueryRouter** | Subsystem in `slopsearx/router.py` that analyzes incoming queries and selects only relevant engines based on topic matching, category filters, and explicit engine selections. Reduces unnecessary dispatch to irrelevant engines. |
| **Ranker** | Pluggable interface for ranking strategies. V1 ships with `PresenceRanker`; V2 plans include `WeightedFusionRanker` with per-engine trust scores. |
| **ScrapeAdapter** | Abstract base class for HTML-scrape engines (DuckDuckGo, Google). Sends HTTP requests with stealth headers and parses HTML with lxml. |
| **Scrape engine** | An engine that sends HTTP GET/POST requests to a search engine's HTML page and parses the response. No headless browser required. |
| **SearchResult** | Internal normalized result dataclass decoupled from any output format. Contains url, title, content, engine, score, position, and metadata. |
| **SearXNG** | The open-source meta search engine that SlopSearX is designed to replace. SlopSearX preserves the SearXNG JSON response contract for backward compatibility. |
| **Security engines** | Engines focused on vulnerability research, threat intelligence, and security scanning. Examples: Shodan, Censys, VirusTotal, NVD, Exploit-DB. |
| **SuggestionService** | Subsystem in `slopsearx/suggest.py` that runs as a background task during search queries, fetching suggestions from engine suggest APIs to populate the `suggestions` response field. |
| **Threat-intel engines** | A subset of security engines specializing in threat intelligence data: IP reputation (AbuseIPDB, GreyNoise), credential leaks (HIBP, DeHashed), dark web intelligence (IntelX), and exploit data (Exploit-DB, VulnCheck). |
| **Tier 1 engines** | Reliable API-based engines (Brave, Wikipedia) that form the reliability backbone |
| **Tier 2 engines** | Best-effort scrape-based engines (DuckDuckGo, Google) with no SLA |
| **Topic-based routing** | The QueryRouter's strategy for matching query content to engine categories using keyword heuristics, reducing the set of dispatched engines to those likely to return relevant results. |
| **Valkey** | Redis-compatible in-memory data store used for caching, distributed rate limiting, and engine quality statistics. |
| **Valkey sliding window** | A distributed rate limiting algorithm that uses Valkey sorted sets to track request timestamps within a moving time window. Correct across 50+ replicas. |
| **GroktoCrawl** | The larger AI agent stack that SlopSearX integrates into, replacing its SearXNG component |
| **OpenMetrics** | The standard format for exposing metrics (Prometheus text format). SlopSearX's `/metrics` endpoint emits OpenMetrics without the prometheus-client library. |
| **ConfigMap** | Kubernetes resource for mounting configuration files. The optional `config.yaml` is typically provided via ConfigMap. |
| **HPA** | Horizontal Pod Autoscaler — Kubernetes resource that automatically scales replicas based on CPU utilization. |
