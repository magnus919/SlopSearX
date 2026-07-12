# SlopSearX — Engine Adapter Reference

Every search engine is exactly one Python file in `engines/`, registered via `@register_engine`. Adding a new engine requires **zero changes** to the orchestrator — the registry auto-discovers modules at import time.

## Quick Start

```python
# engines/myengine.py
from slopsearx.adapter import EngineAdapter, register_engine, AdapterResponse

@register_engine
class MyEngine(EngineAdapter):
    """My search engine adapter."""

    # -- Engine identity (required) --
    name = "myengine"           # registry key, used in ?engines= param
    display_name = "My Engine"  # human-readable label
    env_prefix = "ENGINE_MYENG" # env var prefix for config
    engine_type = "api"         # "api" | "scrape" | "structured"
    categories = ["general"]    # SearXNG-compatible category tags

    async def search(self, query: str, params: dict | None = None) -> AdapterResponse:
        """Execute a search. Never raise — classify errors in AdapterResponse.status."""
        ...
```

## Adapter Contract Rules

1. **Every adapter is exactly one file.** Adding an engine means adding one Python file with a `@register_engine` decorated class. No config file changes, no orchestrator modifications.
2. **Adapters never raise exceptions.** All error states are classified and returned in `AdapterResponse.status`. The orchestrator never sees an unhandled exception from any adapter.
3. **Adapters own their rate limiting.** Call `self.rate_limiter.acquire(engine_name)` before each request.
4. **Adapters own their error classification.** HTTP 429, CAPTCHA, DOM change — each is classified as transient or permanent.
5. **The internal schema is decoupled from wire format.** `SearchResult` is the internal dataclass. SearXNG JSON/YAML are output formatters — not the data model.

## Class Attributes

| Attribute | Required | Default | Description |
|---|---|---|---|
| `name` | **Yes** | `""` | Registry key, matches filename. Used in `?engines=` param. |
| `display_name` | No | `""` | Human-readable label for `/config` and health output. |
| `env_prefix` | No | `""` | Env var prefix: `ENGINE_BRAVE_API_KEY`, `ENGINE_BRAVE_CATEGORIES`. |
| `engine_type` | No | `"api"` | `"api"` (structured JSON API), `"scrape"` (HTML parsing), `"structured"` (e.g. Wikipedia). |
| `categories` | No | `["general"]` | SearXNG-compatible category tags. Determines which `?categories=` queries include this engine. Can use namespace prefixes: `github:code`, `huggingface:datasets`. |

### Category Reclassification

Operators can override categories without modifying adapter code:

```yaml
# config.yaml
engines:
  myengine:
    categories:
      - general
      - news
      - finance
```

Env var equivalents:
```bash
ENGINE_MYENG_CATEGORIES=general,news
```

## Data Types

### SearchResult (internal)

```python
@dataclass
class SearchResult:
    url: str                  # required
    title: str                # required
    content: str              # required — snippet or description
    engine: str               # primary engine name
    engines: set[str]         # all engines that returned this URL (populated by merger)
    score: float = 0.0
    position: int = 0
    category: str = "general"
    published_date: str | None = None  # ISO 8601
    thumbnail: str | None = None
    img_src: str | None = None
```

### AdapterResponse (return type)

```python
@dataclass
class AdapterResponse:
    results: list[SearchResult]
    status: EngineStatus      # OK, RATE_LIMITED, BLOCKED, ERROR, TIMEOUT
    error_message: str | None = None
    latency_ms: float = 0.0
```

## Lifecycle Hooks

Optional — override for setup/teardown:

```python
async def warmup(self) -> None:    # called at server startup
async def shutdown(self) -> None:   # called at graceful shutdown
async def health(self) -> EngineStatus:  # lightweight probe
```

The default `health()` sends a minimal query. Override for engine-specific probes (e.g., checking homepage reachability for scrape engines).

## Adapter Registry

Adapters are auto-discovered at import time. The `engines/` package imports each module, triggering `@register_engine`:

```python
# engines/__init__.py
from engines import brave, wikipedia, duckduckgo, google, ...
```

Adding a new engine file + one import line in `__init__.py` is all that's needed.

**Keep README.md in sync.** The Engines table in `README.md` must reflect every registered adapter. Add a row when adding an engine, remove the row when removing one.

## Engine-Specific Sub-Categories

Engines can declare namespace-prefixed sub-categories for fine-grained routing:

| Engine | Sub-Category | Behavior |
|---|---|---|
| GitHub | `github:code` | Code search endpoint |
| | `github:issues` | Issues + PRs search |
| | `github:prs` | PRs only (alias) |
| HuggingFace | `huggingface:datasets` | Dataset search |
| | `huggingface:papers` | Paper search |
| Reddit | `reddit:subreddit` | Subreddit-scoped search (`params["subreddit"]`) |

Sub-categories appear in `/config` output alongside base categories and are selected with `?categories=github:code`.

## Built-In Adapters (48)

### General / Web

| Adapter | File | Type | Categories | Auth |
|---|---|---|---|---|
| Brave Search | `engines/brave.py` | api | general, news, science, images | `ENGINE_BRAVE_API_KEY` |
| DuckDuckGo | `engines/duckduckgo.py` | scrape | general, news | None |
| Google | `engines/google.py` | scrape | general, news | None |
| Hacker News | `engines/hackernews.py` | api | general, news | None |
| Reddit | `engines/reddit.py` | api | general, social, reddit:subreddit | None |
| Wikipedia | `engines/wikipedia.py` | api | general, science, reference | None |

### Developer / Package Registries

| Adapter | File | Type | Categories | Auth |
|---|---|---|---|---|
| Crates.io | `engines/crates.py` | api | general, it, reference, packages | None |
| Docker Hub | `engines/dockerhub.py` | api | general, it, reference, packages | None |
| GitHub | `engines/github.py` | api | general, reference, github:code, github:issues, github:prs | `GITHUB_TOKEN` |
| npm | `engines/npm.py` | api | general, it, reference, packages | None |
| PyPI | `engines/pypi.py` | api | general, it, reference, packages | None |
| Repology | `engines/repology.py` | api | general, it, reference, packages | None |
| RubyGems | `engines/rubygems.py` | api | general, it, reference, packages | None |
| Stack Exchange | `engines/stackexchange.py` | api | general, reference, science, stackexchange:code, stackexchange:serverfault | Optional app key |

### Science & Research

| Adapter | File | Type | Categories | Auth |
|---|---|---|---|---|
| arXiv | `engines/arxiv.py` | api | general, science, reference | None |
| HuggingFace | `engines/huggingface.py` | api | general, science, huggingface:datasets, huggingface:papers | `HF_TOKEN` (optional) |
| Internet Archive | `engines/internetarchive.py` | api | reference, web:archive, historical | None |
| OpenAlex | `engines/openalex.py` | api | general, science, reference | None |
| Open Library | `engines/openlibrary.py` | api | general, books, reference | None |
| Semantic Scholar | `engines/semanticscholar.py` | api | general, science, reference | Optional API key |
| UniProt | `engines/uniprot.py` | api | general, science, reference, biology, medical | None |

### Medical / Health

| Adapter | File | Type | Categories | Auth |
|---|---|---|---|---|
| ClinicalTrials.gov | `engines/clinicaltrials.py` | api | general, medical, health, science | None |
| openFDA | `engines/openfda.py` | api | general, medical, health, science, government | None |
| PubChem | `engines/pubchem.py` | api | general, science, reference, chemistry, medical | None |
| PubMed | `engines/pubmed.py` | api | general, science, reference, medical, health | None |

### Security / Threat Intelligence

| Adapter | File | Type | Categories | Auth |
|---|---|---|---|---|
| AbuseIPDB | `engines/abuseipdb.py` | api | security, threat-intel | `ENGINE_ABUSEIPDB_API_KEY` |
| AlienVault OTX | `engines/otx.py` | api | security, threat-intel | `ENGINE_OTX_API_KEY` |
| Censys | `engines/censys.py` | api | it, security | `ENGINE_CENSYS_API_KEY` + `_API_SECRET` |
| CRT.sh | `engines/crtsh.py` | api | it, security | None |
| CVE Program (MITRE) | `engines/cve.py` | api | it, security | None |
| DeHashed | `engines/dehashed.py` | api | security, threat-intel | `ENGINE_DEHASHED_API_KEY` |
| Exploit-DB | `engines/exploitdb.py` | scrape | security, exploit | None |
| FIRST EPSS | `engines/epss.py` | api | security, threat-intel | None |
| GreyNoise | `engines/greynoise.py` | api | security, threat-intel | `ENGINE_GREYNOISE_API_KEY` (optional) |
| Have I Been Pwned | `engines/hibp.py` | api | security, reference | `ENGINE_HIBP_API_KEY` |
| IntelX | `engines/intelx.py` | api | security, threat-intel | `ENGINE_INTELX_API_KEY` |
| MITRE ATT&CK | `engines/mitreattack.py` | api | security, reference | None |
| NVD (NIST) | `engines/nvd.py` | api | it, security | `ENGINE_NVD_API_KEY` (optional) |
| Shodan | `engines/shodan.py` | api | it, security | `ENGINE_SHODAN_API_KEY` |
| URLhaus | `engines/urlhaus.py` | api | security, threat-intel | None |
| VirusTotal | `engines/virustotal.py` | api | security, malware | `ENGINE_VIRUSTOTAL_API_KEY` |
| VulnCheck | `engines/vulncheck.py` | api | security, threat-intel | `ENGINE_VULNCHECK_API_KEY` |

### Finance / Economics

| Adapter | File | Type | Categories | Auth |
|---|---|---|---|---|
| FRED | `engines/fred.py` | api | general, finance, reference, economics | `ENGINE_FRED_API_KEY` |
| SEC EDGAR | `engines/edgar.py` | api | general, finance, reference | None |

### Media & Entertainment

| Adapter | File | Type | Categories | Auth |
|---|---|---|---|---|
| MusicBrainz | `engines/musicbrainz.py` | api | general, music, reference | None |
| TMDB | `engines/tmdb.py` | api | general, movies, entertainment | `ENGINE_TMDB_API_KEY` |

### Geography / GIS

| Adapter | File | Type | Categories | Auth |
|---|---|---|---|---|
| Nominatim (OSM) | `engines/nominatim.py` | api | general, geography, reference | None |

### Legal

| Adapter | File | Type | Categories | Auth |
|---|---|---|---|---|
| Oyez (SCOTUS) | `engines/oyez.py` | api | general, reference, legal | None |

See `slopsearx/adapter.py` for the base classes (`EngineAdapter`, `ScrapeAdapter`) and the registry functions (`register_engine`, `discover_engines`).
