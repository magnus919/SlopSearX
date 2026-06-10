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

Operators can override, extend, or suppress categories without modifying adapter code:

```yaml
# config.yaml
engines:
  myengine:
    categories:
      override: ["general", "news", "finance"]  # replace entirely
      # or:
      add: ["legal"]       # append to self-declared
      remove: ["images"]   # suppress from self-declared
```

Env var equivalents:
```bash
ENGINE_MYENG_CATEGORIES=general,news
ENGINE_MYENG_CATEGORIES_ADD=legal
ENGINE_MYENG_CATEGORIES_REMOVE=images
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

## Built-In Adapters

| Adapter | File | Type | Categories | Auth |
|---|---|---|---|---|
| arXiv | `engines/arxiv.py` | api | general, science, reference | None |
| Brave Search | `engines/brave.py` | api | general, news, science, images | `ENGINE_BRAVE_API_KEY` |
| DuckDuckGo | `engines/duckduckgo.py` | scrape | general, news | None |
| GitHub | `engines/github.py` | api | general, reference, github:code/issues/prs | `GITHUB_TOKEN` |
| Google | `engines/google.py` | scrape | general, news | None |
| Hacker News | `engines/hackernews.py` | api | general, news | None |
| HuggingFace | `engines/huggingface.py` | api | general, science, hf:datasets/papers | `HF_TOKEN` (optional) |
| Reddit | `engines/reddit.py` | api | general, social, reddit:subreddit | None |
| Semantic Scholar | `engines/semanticscholar.py` | api | general, science, reference | None (optional key) |
| Wikipedia | `engines/wikipedia.py` | api | general, science, reference | None |

See `slopsearx/adapter.py` for the base classes (`EngineAdapter`, `ScrapeAdapter`) and the registry functions (`register_engine`, `discover_engines`).
