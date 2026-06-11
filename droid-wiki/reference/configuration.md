# Configuration reference

## Environment variables

### Global settings

| Variable | Required | Default | Description |
|---|---|---|---|
| `VALKEY_URL` | No | `valkey://localhost:6379` | Valkey or Redis connection string |
| `SEARCH_CACHE_TTL_SECONDS` | No | `300` | Global cache TTL in seconds |
| `SEARCH_LOG_LEVEL` | No | `INFO` | Log level (`DEBUG`, `INFO`, `WARNING`, `ERROR`) |
| `SEARCH_DEFAULT_ENGINES` | No | all | Comma-separated list of default engines |
| `SEARCH_ENABLE_SUGGESTIONS` | No | `false` | Enable Brave Suggest API calls (opt-in to avoid extra API cost) |

### Security / Threat Intelligence engines

| Variable | Required | Default | Description |
|---|---|---|---|
| `ENGINE_ABUSEIPDB_API_KEY` | Yes |  | AbuseIPDB API key |
| `ENGINE_CENSYS_API_KEY` | Yes |  | Censys API ID |
| `ENGINE_CENSYS_API_SECRET` | Yes |  | Censys API secret |
| `ENGINE_DEHASHED_API_KEY` | Yes |  | DeHashed API key (email:api_key format for basic auth) |
| `ENGINE_GREYNOISE_API_KEY` | No |  | GreyNoise API key (optional — community tier works without one) |
| `ENGINE_HIBP_API_KEY` | Yes |  | Have I Been Pwned API key |
| `ENGINE_INTELX_API_KEY` | Yes |  | IntelX API key |
| `ENGINE_NVD_API_KEY` | No |  | NVD API key (optional — without it, rate limit is ~5 req/30s) |
| `ENGINE_OTX_API_KEY` | Yes |  | AlienVault OTX API key |
| `ENGINE_SHODAN_API_KEY` | Yes |  | Shodan API key |
| `ENGINE_VIRUSTOTAL_API_KEY` | Yes |  | VirusTotal API key |
| `ENGINE_VULNCHECK_API_KEY` | Yes |  | VulnCheck Community API key |

### General / Web engines

| Variable | Required | Default | Description |
|---|---|---|---|
| `ENGINE_BRAVE_API_KEY` | Yes |  | Brave Search API key |

### Developer / Package Registry engines

| Variable | Required | Default | Description |
|---|---|---|---|
| `ENGINE_GITHUB_TOKEN` | Yes |  | GitHub personal access token |
| `ENGINE_STACKEXCHANGE_API_KEY` | No |  | Stack Exchange API key (optional — higher rate limits with key) |

### Finance / Economics engines

| Variable | Required | Default | Description |
|---|---|---|---|
| `ENGINE_FRED_API_KEY` | Yes |  | FRED API key (free tier at fred.stlouisfed.org) |

### Media & Entertainment engines

| Variable | Required | Default | Description |
|---|---|---|---|
| `ENGINE_TMDB_API_KEY` | Yes |  | TMDB API key (free tier available) |

### Science & Research engines

| Variable | Required | Default | Description |
|---|---|---|---|
| `ENGINE_HUGGINGFACE_API_KEY` | No |  | HuggingFace API token |
| `ENGINE_SEMANTICSCHOLAR_API_KEY` | No |  | Semantic Scholar API key |

## Config file format

SlopSearX reads configuration from `/etc/slopsearx/config.yaml`. Environment variables override values in this file.

### Example

```yaml
# /etc/slopsearx/config.yaml

cache:
  ttl_seconds: 300

log:
  level: INFO

default_engines:
  - brave
  - duckduckgo
  - wikipedia

valkey:
  url: "valkey://valkey:6379"

engines:
  brave:
    api_key: "${ENGINE_BRAVE_API_KEY}"
  github:
    token: "${ENGINE_GITHUB_TOKEN}"
  huggingface:
    api_key: "${ENGINE_HUGGINGFACE_API_KEY}"
  stackexchange:
    api_key: "${ENGINE_STACKEXCHANGE_API_KEY}"
  semanticscholar:
    api_key: "${ENGINE_SEMANTICSCHOLAR_API_KEY}"
  abuseipdb:
    api_key: "${ENGINE_ABUSEIPDB_API_KEY}"
  censys:
    api_key: "${ENGINE_CENSYS_API_KEY}"
    api_secret: "${ENGINE_CENSYS_API_SECRET}"
  dehashed:
    api_key: "${ENGINE_DEHASHED_API_KEY}"
  fred:
    api_key: "${ENGINE_FRED_API_KEY}"
  greynoise:
    api_key: "${ENGINE_GREYNOISE_API_KEY}"
  hibp:
    api_key: "${ENGINE_HIBP_API_KEY}"
  intelx:
    api_key: "${ENGINE_INTELX_API_KEY}"
  nvd:
    api_key: "${ENGINE_NVD_API_KEY}"
  otx:
    api_key: "${ENGINE_OTX_API_KEY}"
  shodan:
    api_key: "${ENGINE_SHODAN_API_KEY}"
  tmdb:
    api_key: "${ENGINE_TMDB_API_KEY}"
  virustotal:
    api_key: "${ENGINE_VIRUSTOTAL_API_KEY}"
  vulncheck:
    api_key: "${ENGINE_VULNCHECK_API_KEY}"
```
