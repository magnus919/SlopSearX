# SlopSearX MCP Server

The Model Context Protocol (MCP) server exposes SlopSearX to AI agents as
intent-level tools. Agents can search across 51 engines without knowing URL
query strings, discover what can be searched, preview routing before spending
rate limits, page through stable result snapshots, and run bounded
multi-query research jobs.

The MCP server runs the **same pipeline** as the HTTP API
(`slopsearx.service.SearchService`): identical scope resolution, ranking,
deduplication, caching, and failure semantics. The HTTP API remains the
SearXNG compatibility boundary; MCP is a second, agent-native surface.
SlopSearX is a **standalone search service** — it never delegates to SearXNG
and never fetches or verifies linked pages. Search results carry a
machine-readable `retrieval` handoff record (see `docs/RETRIEVAL_HANDOFF.md`)
so a downstream reader such as GroktoCrawl can capture pages and link them
back to the originating result and snapshot.

- **Tools (15):** intent search, targeted search, jobs, security, science,
  capability listing, scope explanation, service status, snapshot reads,
  research jobs (start/get/cancel/retry/extend).
- **Resources:** `slopsearx://capabilities`, `slopsearx://capabilities/{engine}`,
  `slopsearx://routing-profiles`, `slopsearx://health/summary`.
- **Prompts (4):** repeatable agent workflows that compose the tools.

---

## 1. Requirements

| Requirement | Notes |
|---|---|
| Python ≥ 3.12 | Same as the rest of SlopSearX |
| `slopsearx` installed | MCP support ships with the package (`fastmcp` dependency) |
| Valkey (recommended) | Caching, rate limiting, snapshots, and research jobs persist here. Without Valkey the server still runs and searches work, but pagination cursors and research jobs are unavailable |
| Engine API keys | Same `ENGINE_*_API_KEY` environment variables the HTTP service uses (e.g. `ENGINE_BRAVE_API_KEY`) |

## 2. Installation

```bash
pip install -e ".[dev]"      # or: pip install slopsearx
```

Verify the server starts and lists its tools:

```bash
slopsearx-mcp --help 2>&1 | head -5   # shows the CLI flags (--remote, --token, --oauth, ...)
```

## 3. Configuration

Configuration is layered: defaults → `config.yaml` `mcp:` section → `MCP_*`
environment variables (env always wins). There is **no MCP-specific config
file**; the `mcp:` section lives in the same `config.yaml` the HTTP service
uses (`/etc/slopsearx/config.yaml` or the repo-root `config.yaml`).

### 3.1 `config.yaml`

```yaml
mcp:
  # Specialist tool grants — all disabled by default (secure default).
  enabled_tools:
    jobs: false
    security: false
    science: false
    research: false
  # Engines that generic routing must never reach accidentally. Only an
  # explicit engines list (with the targeted grant) or the security tool
  # (with its grant) can query them.
  sensitive_engines: [hibp, dehashed]
  # Engines whose adapters fail closed without credentials.
  required_key_engines: [abuseipdb, brave, censys, dehashed, fred, hibp,
                         intelx, otx, shodan, tmdb, virustotal, vulncheck]
  # Bounds
  max_query_length: 500
  max_results: 50
  snapshot_ttl_seconds: 3600        # pagination cursors expire after 1h
  job_max_queries: 20
  job_max_engines_per_query: 10
  job_max_results: 500
  job_default_deadline_seconds: 600
  # Durable research execution (Valkey-backed lease model).
  job_lease_ttl_seconds: 60
  job_poll_interval_seconds: 1.0
  job_max_concurrent_jobs: 1
  # Auth (HTTP transport only). Empty = authentication disabled; stdio is
  # trusted by its process-launch boundary.
  auth_token: ""
  # Policy boundary (deliberate): allow slopsearx_search_targeted to reach
  # sensitive engines (hibp, dehashed). Default false — agents get
  # tool_disabled until the operator opts in.
  targeted_sensitive_allowed: false
  # OAuth 2.1 mode — the alternative to auth_token, required by OAuth-only
  # clients such as Claude Web / ChatGPT connectors. When enabled, the
  # server speaks standard MCP OAuth with dynamic client registration
  # (PKCE + RFC 7591); auth_token is then ignored on the HTTP transport.
  # issuer_url must be externally reachable by the client's browser.
  oauth:
    enabled: false
    issuer_url: "https://mcp.example.com"
    # service_documentation_url: "https://example.com/docs"
    access_token_ttl_seconds: 3600
    refresh_token_ttl_seconds: 2592000
```

### 3.2 Environment variables

| Variable | Default | Purpose |
|---|---|---|
| `MCP_TRANSPORT` | `stdio` | `stdio` or `http` |
| `MCP_HOST` / `MCP_PORT` | `127.0.0.1` / `8000` | HTTP transport bind address |
| `MCP_AUTH_TOKEN` | empty | Bearer token required by the HTTP transport (static-token mode) |
| `MCP_OAUTH_ENABLED` | unset (false) | `1`/`true` enables OAuth 2.1 mode (alternative to the static token) |
| `MCP_OAUTH_ISSUER_URL` | empty | externally reachable server URL — required when OAuth is enabled |
| `MCP_OAUTH_SERVICE_DOCUMENTATION_URL` | empty | advertised docs URL in OAuth metadata |
| `MCP_OAUTH_ACCESS_TOKEN_TTL_SECONDS` | `3600` | access-token lifetime |
| `MCP_OAUTH_REFRESH_TOKEN_TTL_SECONDS` | `2592000` | refresh-token lifetime (30 days) |
| `MCP_GRANT_JOBS` | unset (false) | `1`/`true` enables `slopsearx_search_jobs` |
| `MCP_GRANT_SECURITY` | unset (false) | enables `slopsearx_search_security` and `intent=security` |
| `MCP_GRANT_SCIENCE` | unset (false) | enables `slopsearx_search_science` |
| `MCP_GRANT_RESEARCH` | unset (false) | enables research jobs |
| `MCP_TARGETED_SENSITIVE_ALLOWED` | unset (false) | lets `slopsearx_search_targeted` query sensitive engines (`hibp`, `dehashed`); otherwise they are rejected with `tool_disabled` |
| `MCP_MAX_QUERY_LENGTH` | `500` | max query characters |
| `MCP_MAX_RESULTS` | `50` | presentation bound on result pages |
| `MCP_SNAPSHOT_TTL_SECONDS` | `3600` | cursor TTL |
| `MCP_JOB_MAX_QUERIES` | `20` | per-job query budget |
| `MCP_JOB_MAX_ENGINES_PER_QUERY` | `10` | per-query engine budget |
| `MCP_JOB_MAX_RESULTS` | `500` | job result budget |
| `MCP_JOB_DEFAULT_DEADLINE_SECONDS` | `600` | default job deadline |
| `MCP_JOB_LEASE_TTL_SECONDS` | `60` | research-job lease visibility timeout (how long a replica may own a running job before another replica can reclaim it) |
| `MCP_JOB_POLL_INTERVAL_SECONDS` | `1.0` | how often an idle research worker polls Valkey for claimable jobs |
| `MCP_JOB_MAX_CONCURRENT_JOBS` | `1` | bounded per-replica research-job concurrency |
| `MCP_SENSITIVE_ENGINES` | `hibp,dehashed` | comma-separated override |
| `MCP_LOG_LEVEL` | `info` | uvicorn log level for HTTP transport |
| `MCP_REMOTE_URL` | empty | gateway mode: remote server URL (`--remote`) |
| `MCP_REMOTE_TOKEN` | empty | gateway client credential for a static-token remote (`--token`) |
| `MCP_REMOTE_OAUTH` | false | gateway connects via the OAuth client flow (`--oauth`) |
| `MCP_REMOTE_OAUTH_CALLBACK_PORT` | `8765` | gateway loopback redirect port (`--oauth-callback-port`) |
| `MCP_REMOTE_OAUTH_NO_BROWSER` | false | gateway prints the authorize URL instead of opening a browser (`--oauth-no-browser`) |
| `MCP_REMOTE_TOKEN_FILE` | empty | gateway token file path override (`--oauth-token-file`); defaults to `~/.config/slopsearx/oauth/<hash>.json` |

Engine credentials, Valkey URL, and the HTTP service settings (`VALKEY_URL`,
`MAX_CONCURRENT_ENGINES`, `PER_CLIENT_REQUESTS`, `FAIL_CLOSED`, …) are shared
with the HTTP service and configure the same runtime.

## 4. Running

### 4.1 stdio (default — recommended for local agents)

```bash
slopsearx-mcp
# or
python -m slopsearx.mcp
```

The server speaks MCP over stdin/stdout. Point your MCP client at it (see
§5). No auth: the launch boundary (whoever can spawn the process) is the
trust boundary.

### 4.2 HTTP (streamable)

Local-only (default bind):

```bash
export MCP_TRANSPORT=http
export MCP_AUTH_TOKEN=$(openssl rand -hex 24)   # strongly recommended
slopsearx-mcp
# → streamable HTTP on 127.0.0.1:8000, endpoint path /mcp
```

Remote clients (SlopSearX on a different host than the MCP client):

```bash
export MCP_TRANSPORT=http
export MCP_HOST=0.0.0.0          # bind all interfaces so remote hosts can connect
export MCP_PORT=8000
export MCP_AUTH_TOKEN=$(openssl rand -hex 24)
slopsearx-mcp
# → http://<this-host>:8000/mcp
```

With a token set, every request must carry
`Authorization: Bearer <token>`; requests without it get `401`. The token
is the only boundary between the network and your engine credentials —
when exposing the port beyond loopback, terminate TLS in front of the
server (reverse proxy) so the token is not sent in plaintext, and restrict
the port with a firewall to the expected clients where possible.

### 4.3 Deployment options — the server host is your choice

The MCP server is an ordinary SlopSearX process. Deploy it on the
SlopSearX host however you run the rest of the service; the MCP client
never cares, because it only needs the URL and token from §4.2.

- **Native install** (shown in §4.1/§4.2): `pip install slopsearx`, then
  run `slopsearx-mcp` or `python -m slopsearx.mcp`. Wrap it in a venv,
  a systemd unit, or your process supervisor of choice.
- **Docker**: the server runs in the same image as the HTTP service. For
  remote clients, publish the port and bind all interfaces inside the
  container:

  ```bash
  docker run -d --name slopsearx-mcp -e VALKEY_URL=redis://valkey:6379/0 \
    -e MCP_TRANSPORT=http -e MCP_HOST=0.0.0.0 -e MCP_AUTH_TOKEN=change-me \
    -p 8000:8000 \
    ghcr.io/magnus919/slopsearx:latest slopsearx-mcp
  # → http://<docker-host>:8000/mcp
  ```

The client-side MCP configuration (§4.2, §5) is identical regardless of
which deployment you pick — it never depends on how the server host runs
the process.

### 4.4 Remote gateway (agent-host CLI)

When the agent and the SlopSearX server are on different hosts, run
`slopsearx-mcp` **on the agent's host** as a stdio gateway that holds a
streamable-HTTP connection to the remote server:

```text
agent (Hermes) ──stdio──▶ slopsearx-mcp --remote …   (agent host)
                              │ streamable HTTP + Bearer token
                              ▼
                    slopsearx-mcp (serve mode)        (SlopSearX host)
```

```bash
# On the SlopSearX host (see §4.2):
MCP_TRANSPORT=http MCP_HOST=0.0.0.0 MCP_AUTH_TOKEN=<token> slopsearx-mcp

# On the agent's host (spawned by the MCP client over stdio):
slopsearx-mcp --remote http://<slopsearx-host>:8000/mcp --token <token>
# or: MCP_REMOTE_URL=… MCP_REMOTE_TOKEN=… slopsearx-mcp
```

The gateway re-exposes the remote server's tools, resources, and prompts
(registered from its live capability list), so the agent sees the exact
same surface as connecting directly. The agent host needs only this
package — no Valkey, no engine API keys, no search wiring. If the remote
server is unreachable or the credentials are wrong, the gateway fails at
startup with a clear message.

**Gateway authentication** matches the remote's mode:

- **Static token** (remote runs in default serve mode): pass
  `--token <token>` or `MCP_REMOTE_TOKEN` (prefer the env var so the token
  does not appear in process listings). The token is a client credential
  for the gateway process only.
- **OAuth 2.1** (remote runs in OAuth mode, §4.5): pass `--oauth`:

  ```bash
  slopsearx-mcp --remote http://<slopsearx-host>:8000/mcp --oauth
  # optional flags:
  #   --oauth-callback-port 8765   loopback port for the redirect (default 8765)
  #   --oauth-no-browser           print the authorize URL instead of opening a browser (headless hosts)
  #   --oauth-token-file FILE      persist tokens here (default: ~/.config/slopsearx/oauth/<hash>.json)
  ```

  The gateway runs the standard MCP OAuth flow from the **agent's host**:
  dynamic client registration (RFC 7591, public client + PKCE), then a
  local loopback callback on `http://127.0.0.1:<port>/callback` that
  receives the browser redirect. The authorize URL is printed to
  **stderr** (stdout carries the MCP protocol) and a browser is opened
  unless `--oauth-no-browser` is set. Tokens are persisted in a 0600 JSON
  file, so later runs reuse them without re-authorizing.
  `--oauth` and `--token` are mutually exclusive; the same flow is
  available via `MCP_REMOTE_OAUTH`, `MCP_REMOTE_OAUTH_CALLBACK_PORT`,
  `MCP_REMOTE_OAUTH_NO_BROWSER`, and `MCP_REMOTE_TOKEN_FILE` (§3.2).

### 4.5 OAuth 2.1 mode (Claude Web / ChatGPT connectors)

Remote MCP servers that OAuth-only clients connect to (Claude Web
connectors, ChatGPT plugins, and Plaud-style connectors generally) require
standard MCP OAuth 2.1 instead of a static bearer token. Enable it:

```bash
MCP_TRANSPORT=http MCP_HOST=0.0.0.0 \
MCP_OAUTH_ENABLED=1 \
MCP_OAUTH_ISSUER_URL=https://mcp.example.com \
slopsearx-mcp
```

- `issuer_url` must be the URL the client's browser can reach (the
  authorization redirects there); terminate TLS in front of the server for
  anything beyond loopback.
- The server then exposes the standard endpoints:
  `/.well-known/oauth-authorization-server`, `/authorize`, `/token`,
  `/register` (dynamic client registration, RFC 7591), and `/revoke`, and
  protects `/mcp` with OAuth-issued bearer tokens.
- SlopSearX has no user accounts, so authorization is **auto-approved** for
  registered clients (PKCE + redirect-uri checks are enforced by the MCP
  SDK) — possession of a registered client is the equivalent of possessing
  the static token. Treat the network boundary accordingly.
- OAuth state (clients, codes, tokens) lives in Valkey when available, so
  tokens survive across replicas.
- OAuth mode and `auth_token` are mutually exclusive: when OAuth is
  enabled, the static token is ignored on the HTTP transport.

## 5. Client setup

### Claude (Desktop, Web, and mobile)

Claude Desktop and Claude mobile accept a local stdio server —
`claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "slopsearx": {
      "command": "/path/to/slopsearx/.venv/bin/slopsearx-mcp"
    }
  }
}
```

Claude **Web** (and web-based connectors) require OAuth for remote MCP
servers — connect to a server running in OAuth mode (§4.5) via
Customize → Connectors → add server with the metadata URL
(`https://mcp.example.com/.well-known/oauth-authorization-server`); the
connector runs the standard authorize flow in your browser.

### Cursor

Settings → MCP → Add new MCP server:

```
Command: /path/to/slopsearx/.venv/bin/slopsearx-mcp
Transport: stdio
```

### Hermes Agent (Nous Research)

Hermes reads MCP servers from `~/.hermes/config.yaml` under `mcp_servers`
(see the [Hermes MCP docs](https://hermes-agent.nousresearch.com/docs/user-guide/features/mcp)).
SlopSearX typically runs on a **separate host** from Hermes. Two remote
setups work; the stdio gateway (option A) is the recommended one because
Hermes only ever talks stdio to a local process.

In both cases, the server side is identical — **1. Run the MCP server on
the SlopSearX host** (see §4.2; deploy it however you run SlopSearX —
native or Docker, §4.3):

```bash
MCP_TRANSPORT=http \
MCP_HOST=0.0.0.0 \                 # bind all interfaces so the remote host can connect
MCP_PORT=8000 \
MCP_AUTH_TOKEN=<strong-random-token> \
slopsearx-mcp
```

- **Server-side settings stay on the server.** `VALKEY_URL`, engine API
  keys (`ENGINE_*_API_KEY`), MCP grants (`MCP_GRANT_*`), and bounds are
  read by the server process from its own environment (or `config.yaml`)
  on the SlopSearX host — exactly like the HTTP service. Hermes never
  sees or needs any of them.
- The token is the only boundary between the network and your engine
  credentials. Terminate TLS in front of the server (reverse proxy) so
  the bearer token is not sent in plaintext, and restrict the port with
  a firewall to Hermes' host if possible.

#### Option A (recommended): stdio gateway on the Hermes host

Run `slopsearx-mcp` on the Hermes host as a gateway that connects to the
remote server (§4.4). Hermes spawns it over stdio; the agent host needs
only the package — no Valkey, no engine keys.

`~/.hermes/config.yaml`:

```yaml
mcp_servers:
  slopsearx:
    command: "/path/to/slopsearx/.venv/bin/slopsearx-mcp"
    args: ["--remote", "http://<slopsearx-host>:8000/mcp"]
    env:
      MCP_REMOTE_TOKEN: "<same-token-as-the-server>"   # or use a launcher wrapper
```

If the remote server runs in **OAuth mode** (§4.5), drop the token and let
the gateway run the OAuth client flow instead:

```yaml
mcp_servers:
  slopsearx:
    command: "/path/to/slopsearx/.venv/bin/slopsearx-mcp"
    args: ["--remote", "http://<slopsearx-host>:8000/mcp", "--oauth"]
    env:
      MCP_REMOTE_OAUTH_NO_BROWSER: "true"   # headless hosts: print the authorize URL
```

- The token is the gateway's client credential; keep it out of process
  listings by using `MCP_REMOTE_TOKEN` (env) rather than `--token`, and
  out of `config.yaml` if you prefer by launching via a wrapper script
  or `${env:MCP_REMOTE_TOKEN}` from `~/.hermes/.env`.
- In OAuth mode the first connection prints the authorize URL to the
  gateway's stderr (visible in Hermes' gateway logs) and opens a browser
  on the Hermes host; afterwards tokens persist in the gateway's token
  file and later runs skip re-authorization.
- The gateway registers the remote server's tools at startup; if the
  server is unreachable or the credentials are wrong, the gateway fails
  with a clear message and Hermes reports the connection error.

#### Option B: direct HTTP

Hermes connects straight to the remote MCP server (§4.2):

```yaml
mcp_servers:
  slopsearx:
    url: "http://<slopsearx-host>:8000/mcp"
    headers:
      Authorization: "Bearer <same-token-as-the-server>"
```

- The MCP endpoint is `/mcp` (e.g. `http://10.0.0.5:8000/mcp`).
- **Static-token mode** (shown above): send the bearer token in
  `headers`. **OAuth mode** (§4.5): omit the header and set `auth: oauth`
  on the server entry — Hermes runs the standard MCP OAuth flow against
  the server.
- If Hermes' preflight content-type probe fails against a proxied
  endpoint, add `skip_preflight: true` to the server entry.

**Verify (either option):** start Hermes, then reload MCP config with
`/reload-mcp` after any edit. Tools register as
`mcp__slopsearx__slopsearx_search`, `mcp__slopsearx__slopsearx_read_results`,
etc. (Hermes prefixes server tools with `mcp__<server>__`); resources and
prompts auto-register as `mcp__slopsearx__read_resource` / `...__get_prompt`
when supported.

#### Local stdio (only when SlopSearX runs on the same host as Hermes)

Use this instead when both run on one machine:

```yaml
# ~/.hermes/config.yaml
mcp_servers:
  slopsearx:
    command: "/path/to/slopsearx/.venv/bin/slopsearx-mcp"
    args: []
```

- Use the **absolute path** to the `slopsearx-mcp` executable in your
  SlopSearX virtualenv; don't rely on PATH from Hermes' shell.
- Server-side settings still stay on the server: configure them where
  the process is launched (shell env, a wrapper script, systemd, or
  Docker), not in Hermes' config. Example wrapper:

  ```bash
  #!/usr/bin/env bash
  # /usr/local/bin/slopsearx-mcp-launch
  export VALKEY_URL="redis://127.0.0.1:6379/0"   # server-side infrastructure
  export ENGINE_BRAVE_API_KEY="..."              # engine credentials
  export MCP_GRANT_JOBS="1"                      # opt-in specialist grants
  exec /path/to/slopsearx/.venv/bin/slopsearx-mcp
  ```

  Point `command` at the wrapper. Without Valkey the server still runs
  but degrades gracefully (no cache, rate-limit enforcement, snapshots,
  or research jobs — see §10).
- **Optional passthrough:** if you prefer to keep server settings in the
  Hermes block anyway, the `env:` map is passed through to the server
  subprocess as its environment (`${VAR}` is resolved from
  `~/.hermes/.env`). Treat that as server configuration living in the
  Hermes file — not as something Hermes needs to know.

#### Narrow the surface with tool filters

Only register the tools you want Hermes to see (original tool names;
globs allowed — see the
[Hermes config reference](https://hermes-agent.nousresearch.com/docs/reference/mcp-config-reference)).
This works with both option A (gateway) and option B (direct HTTP):

```yaml
mcp_servers:
  slopsearx:
    # Option A (gateway): command + args
    command: "/path/to/slopsearx/.venv/bin/slopsearx-mcp"
    args: ["--remote", "http://<slopsearx-host>:8000/mcp"]
    env:
      MCP_REMOTE_TOKEN: "<same-token-as-the-server>"
    # Option B (direct HTTP): url + headers instead of command/args/env
    # url: "http://<slopsearx-host>:8000/mcp"
    # headers:
    #   Authorization: "Bearer <same-token-as-the-server>"
    tools:
      include:
        - slopsearx_search
        - slopsearx_search_targeted
        - slopsearx_list_capabilities
        - slopsearx_explain_search_scope
        - slopsearx_read_results
        - slopsearx_read_result
```

### Generic MCP client (MCP Inspector, custom)

```bash
npx @modelcontextprotocol/inspector -- /path/to/slopsearx/.venv/bin/slopsearx-mcp
```

## 6. Tools

All tools return JSON. A search tool returns an envelope with `results`,
`scope` (what was selected and why), `engine_outcomes` (which sources
answered and which failed), a structured `enforcement` report (per-filter
`enforced`/`partially_enforced`/`unsupported`/`rejected` with reasons), and
`meta` (query id, cache status, partial flag, rank explanation, pagination
cursor, warnings).

### 6.1 `slopsearx_search`

Intent-based search — the primary entry point.

| Parameter | Type | Notes |
|---|---|---|
| `query` | string, required | ≤ `max_query_length` chars |
| `intent` | string | `auto` (default), `web`, `news`, `science`, `reference`, `code`, `social`, `historical`, `jobs`, `security`, `medical`, `finance`, `packages`, `media`, `legal`, `geography` |
| `categories` | string[] | OR filter; overridden by `engines` |
| `engines` | string[] | explicit override; must be known engines |
| `language` | string | `en` default — **not enforced by adapters** (warning returned) |
| `time_range` | string | `day`/`month`/`year` — **not enforced by adapters** (warning returned) |
| `safesearch` | string | `off` (default), `moderate`, `strict`. **Strict fails closed**: no adapter enforces it |
| `max_results` | int | presentation bound, capped at `MCP_MAX_RESULTS` |
| `include` | string[] | subset of `results`, `suggestions`, `engine_status`, `diagnostics`, `payload` |
| `freshness` | string | `no_preference` (default), `prefer_cache`, `prefer_fresh` |

### 6.2 `slopsearx_search_targeted`

Deliberate, auditable selection of named engines. Unknown or inactive
engines are rejected with a list of valid alternatives.

> **Sensitive engines are gated by one uniform operator grant.** The engines in
> `mcp.sensitive_engines` (default: `hibp`, `dehashed`) are **never** reachable
> through generic routing, categories, or intent profiles — that is deliberate.
> They are also unreachable through any explicit-engine path unless the operator
> set the single sensitive-engine grant `MCP_TARGETED_SENSITIVE_ALLOWED=1` (or
> `targeted_sensitive_allowed: true`). One shared policy gate applies this grant
> uniformly across **every** search path — generic `slopsearx_search` with an
> explicit `engines` list, `slopsearx_search_targeted`, jobs, security, and
> science. Naming a sensitive engine without the grant returns `tool_disabled`
> listing the rejected engines. This is a policy boundary, not a bug: the
> operator must explicitly opt in before an agent can query breach and
> credential-exposure sources through any tool. The specialist grants
> (`MCP_GRANT_JOBS/SECURITY/SCIENCE`) enable their tools; they do **not** by
> themselves grant sensitive-engine access.

### 6.3 `slopsearx_search_jobs` (grant: `MCP_GRANT_JOBS`)

Company-first ATS search. `company` is required; `keywords`, `location`,
`employment_type`, and `sources` (default `greenhouse,ashby,lever`) are
supported. The tool builds the internal `"… at <company>"` query the ATS
adapters understand. Current adapters return title, URL, and location/salary
where available — **no full job descriptions and no cross-ATS global
search** (stated in the response warnings).

### 6.4 `slopsearx_search_security` (grant: `MCP_GRANT_SECURITY`)

Threat-intelligence search. `evidence_types` (`vulnerability`, `exposure`,
`reputation`, `malware`, `threat_intel`, `exploit`) resolve to engine
profiles; `engines` overrides. Results are **search findings, not a
complete security assessment** — absence from the selected engines does not
mean absence of a vulnerability.

### 6.5 `slopsearx_search_science` (grant: `MCP_GRANT_SCIENCE`)

Research search. `source_types` (`papers`, `scholarly_index`, `biomedical`,
`chemistry`, `datasets`, `general_reference`) resolve to engine profiles.
Provenance and coverage are reported, but peer-review status and study
quality are never inferred from search results.

### 6.6 `slopsearx_list_capabilities`

Generated from the live registry — never from prose. Filters: `family`,
`category`, `include_disabled`, `include_auth_requirements` (auth class and
whether credentials are configured; **never the key values**).

### 6.7 `slopsearx_explain_search_scope`

Dry-run routing preview: which engines would run, which were excluded and
why, the routing rule, and the matched topic. Executes nothing and spends
no rate limits — call it before dispatching a costly search.

### 6.8 `slopsearx_get_service_status`

Liveness, Valkey connectivity (and fail-closed state), engine inventory,
snapshot/job store availability. `/health` does **not** probe external
APIs — engine health is observed passively through search outcomes.

### 6.9 `slopsearx_read_results` / 6.10 `slopsearx_read_result`

Stable pagination over a captured snapshot. `cursor` comes from a previous
search's `meta.cursor`; `read_results(cursor, page, max_results)` pages the
captured evidence and never re-runs the query. `read_result(result_id)`
expands one server-issued ID (`<cursor>:<index>`) with provenance and rank
explanation. Arbitrary URLs are rejected — the server is not an SSRF page
fetcher.

Every expanded record also carries a machine-readable `retrieval` handoff
record (contract `slopsearx.retrieval_handoff` v1, defined in
`docs/RETRIEVAL_HANDOFF.md`): the result identity, the canonical URL when it is
eligible for downstream retrieval, a closed `url_status` token
(`ok`/`missing`/`non_http`/`unsafe_scheme`/`ambiguous`) with a stable reason,
snippet-only/non-verification status, and the snapshot/query provenance a
downstream retriever (e.g. GroktoCrawl) uses to associate a captured page back
to this result — without parsing prose. Ineligible URLs (missing, unsafe
schemes such as `file:`/`data:`/`javascript:`, non-HTTP, or
canonicalization-ambiguous) are **never** handed off as fetch targets.
Result cards carry the same eligibility summary in compact form (`retrieval`),
so a card-only consumer can decide whether to fetch without expanding.

### 6.11–6.13 Research jobs (grant: `MCP_GRANT_RESEARCH`)

- `slopsearx_start_research(question, strategy, max_queries, max_engines_per_query, deadline, idempotency_key)` — strategies:
  - `triangulate` — same question across independent source families
  - `broad` — several source families
  - `fresh` — recent material (`time_range` day/month)
  - `counterevidence` — limitations, criticism, counterexamples
  Returns a job handle immediately.
- `slopsearx_get_job(job_id)` — state (`queued`, `running`, `partial`,
  `succeeded`, `failed`, `cancelled`, `expired`), per-query progress,
  query ids, and snapshot cursors for completed evidence.
- `slopsearx_cancel_job(job_id)` — best-effort: stops undispatched
  queries; in-flight upstream calls complete and their evidence stays
  readable.
- `slopsearx_retry_research(job_id)` — re-runs only the failed/empty
  subqueries of a completed job, reusing successful subqueries'
  byte-for-byte-unchanged snapshot cursors. Each retried subquery is
  linked to the same job under a NEW attempt/cursor; the original attempt's
  evidence stays readable. Returns a structured `no_retryable_work` error
  when the job has no failed/empty work.
- `slopsearx_extend_research(job_id, query)` — a bounded follow-up query
  within the job's remaining budget, persisted as part of the same job.

Jobs are idempotent (caller-supplied `idempotency_key`), budget-bounded,
and expire after 24h. Completed queries are immutable — their cursors remain
readable across retry and cancel.

### 6.13.1 Durable execution across replicas

When Valkey is connected, research jobs execute durably across replicas using
a lease-based claim model:

- A `queued` job is claimed atomically by exactly one replica (Valkey `SET NX`),
  which marks it `running` under an exclusive lease token and a visibility
  timeout (`job_lease_ttl_seconds`).
- The owning replica renews its lease while it executes and releases it on
  completion. If a replica dies mid-job, its lease expires and another replica
  claims the job, resets any `running` subquery to `pending`, and resumes the
  remaining work. Completed subqueries and their snapshot cursors are
  preserved byte-for-byte.
- Duplicate delivery cannot duplicate evidence: the atomic claim admits
  exactly one owner, and completed subqueries are never re-executed.
- Cancellation is durable and race-free: `slopsearx_cancel_job` records a
  cancellation flag that the owning worker observes on its next reload, then
  stops undispatched queries and preserves completed evidence. If no worker
  holds the lease, cancellation finalizes the job immediately.
- Each replica runs a bounded number of concurrent jobs
  (`job_max_concurrent_jobs`); an idle worker polls Valkey every
  `job_poll_interval_seconds` for claimable jobs.

The topology is machine-discoverable via `slopsearx_get_service_status` /
`slopsearx://health/summary` under `research_execution`:
`mode` is `durable_leased` (Valkey connected) or `degraded` (no Valkey).

Without Valkey, research jobs are **not executed**. The job store is a no-op
and the worker cannot claim the locally enqueued job, so it is dropped rather
than run. `slopsearx_start_research` still returns a handle, but it is flagged
`degraded`/`ephemeral` and the job is never persisted or executed.

### 6.14 Why there is no separate "advanced search" tool

Earlier design work (the original PRD) floated a dedicated, typed
"advanced search" operation with explicit detail selection and
`requires_answers` / `requires_media` / `requires_source_type`
requirements. That separate tool was **not** added. The richer `include`,
detail (record vs. card), and capability-`requires_*` semantics on the
existing tools cover the same needs with no extra surface area:

- **Field / detail selection.** `slopsearx_search(include=[...])` selects
  which envelope sections to return (`results`, `suggestions`,
  `engine_status`, `diagnostics`, `payload`). Detail is progressive: cards are compact
  and `slopsearx_read_result` expands a card into a full record (complete
  `content`, media, every contributing engine, provenance) without a new
  tool. `max_results` bounds the presented page.
- **`requires_*` evaluation.** `slopsearx_list_capabilities` exposes each
  engine's `supported_result_types` (text/answers/corrections/infoboxes/
  media/structured) and `supported_filters`, generated from the live
  registry. An agent that "requires answers" or "requires media" reads the
  catalog to select engines that declare the result type, then dispatches
  with `engines`/`intent`/`categories` — the "require" check is delegated to
  the catalog, not a new tool.
- **Explicit source boundaries.** `slopsearx_search_targeted` (and the
  specialist tools) already provide deliberate, auditable engine selection
  when a precise source set is required.

Because the catalog already evaluates capability requirements and the
search/read tools already expose explicit detail control, a separate typed
tool would only duplicate surface area, re-implement the policy gate, and
add a second way to express the same request. Keeping one search surface
with progressive disclosure is the deliberate, documented decision; the
capability catalog is the canonical way to inspect what a source supports.

### 6.15 Structured domain payloads

Some adapters return structured fields that do not fit the common result
envelope (e.g. a CVE's CVSS vector, a paper's authors, a package's license, a
job's salary, a movie's release date, a FRED series' units, or an FDA drug
label's active substance). SlopSearX preserves these as an **optional,
versioned payload** attached to a result, rather than flattening them into a
single generic snippet.

A payload is self-describing:

```json
{
  "domain": "security",
  "type": "vulnerability",
  "schema_version": 1,
  "data": { "cve_id": "CVE-2024-12345", "cvss": { "score": 9.8 } },
  "provenance": {
    "engine": "nvd",
    "adapter_fields": ["cve_id", "cvss"],
    "normalized_fields": [],
    "inferred_fields": []
  }
}
```

- `domain` is one of `security`, `science`, `packages`, `jobs`, `media`,
  `financial`, `biomedical`; `type` narrows it within the family (e.g.
  `vulnerability`, `publication`, `package`, `job`, `media_item`,
  `economic_series`, `drug_label`). `schema_version` versions the envelope.
- `provenance` distinguishes `adapter_fields` (reported by the source) from
  `normalized_fields` (mapped from the common envelope) and `inferred_fields`
  (derived by the pipeline). `data` never invents fields the adapter did not
  return — an absent field is absent, not `null`/`false`/empty.
- Results **without** a payload remain valid and backward-compatible; the
  common envelope is their complete representation.

Disclosure follows the same progressive model as content:

- **Compact cards** inline a payload only when the caller passed
  `include=["payload"]` or the serialized payload is small enough to inline.
- **`slopsearx_read_result`** returns the complete available payload
  (`payload`), or `null` when the result has none (or an unserializable
  payload), so an agent never has to rediscover the source to reason over
  structured fields.

The canonical cache/snapshot form also bounds what is persisted: a payload is
stored only when its serialized size is ≤ `PAYLOAD_MAX_PERSIST_BYTES`
(default 16384 = 16 KiB, overridable via the `PAYLOAD_MAX_PERSIST_BYTES`
environment variable). This is distinct from the 512-byte compact-disclosure
cap — the smaller cap keeps triage cards small, while the persistence bound
prevents the shared Valkey cache and snapshots from absorbing unbounded
payloads. A payload above the persistence bound is dropped from the persisted
form and therefore reads back as `null` on `slopsearx_read_result`.

**Payloads are source-derived evidence, not verification.** They are exactly
what the adapter reported; SlopSearX did not fetch or verify the linked page,
did not fill in missing fields, and draws no domain-specific conclusions from
them. A CVSS score in a payload is the score the source reported, not an
independent assessment.

## 7. Resources and prompts

Read resources instead of guessing: `slopsearx://capabilities`,
`slopsearx://capabilities/{engine}`, `slopsearx://routing-profiles`,
`slopsearx://health/summary`.

Four prompts are bundled for repeatable workflows: `research_with_source_coverage`,
`investigate_vulnerability`, `find_company_jobs`, `compare_package_or_project`.

## 8. Agent usage guide

- **Prefer intent over explicit engines.** `intent=auto` uses query-topic
  routing with a tier-1 fallback. Preview it with `slopsearx_explain_search_scope`.
- **Check `engine_outcomes`.** Partial results are normal — some sources
  block, time out, or rate-limit. Absence from a source is not proof of
  absence of the thing searched.
- **Treat results as leads, not facts.** SlopSearX returns titles, URLs, and
  snippets. It never fetches or verifies page bodies. The `score` is a
  cross-engine presence signal (`tier_then_cross_engine_presence`), not
  relevance confidence. Structured `payload` fields are source-derived
  evidence — exactly what the adapter reported — not verification or
  analysis.
- **Hand retrieval off through the handoff record.** Every result card and
  expanded record carries a `retrieval` block (contract
  `slopsearx.retrieval_handoff`, `docs/RETRIEVAL_HANDOFF.md`). Read
  `retrieval.url_status` / `retrieval.eligible` to decide whether a URL may
  be fetched — unsafe, non-HTTP, missing, and ambiguous URLs are never handed
  off as fetch targets. When you capture a page downstream (e.g. with
  GroktoCrawl), record `retrieval.result_id` and
  `retrieval.provenance.{snapshot_cursor, query_id}` on the capture so the
  capture links back to the search result without prose parsing. SlopSearX
  itself never fetches the page.
- **Paginate with cursors.** `meta.cursor` → `slopsearx_read_results`.
  Pages come from captured evidence; the query never re-runs.
- **Do not invent engines.** Read `slopsearx://capabilities` for the live
  set. Unknown engine names are rejected with alternatives.
- **Respect filter honesty.** Every search response carries a structured
  `enforcement` report keyed by filter name (`language`, `time_range`,
  `safesearch`, plus specialist params such as jobs `location`/`employment_type`
  and science `date_from`/`date_to`), each entry `{requested, status, reason,
  enforced_by}`. `status` is one of `enforced`/`partially_enforced`/`unsupported`/
  `rejected`. No adapter enforces `language`, `time_range`, or `moderate`
  safesearch, so those report `unsupported`; strict SafeSearch is `rejected`
  (fails closed). Read the `status` field, not the warning strings, to decide
  how a filter was applied.
- **Specialist workflows need grants.** Jobs, security, science, and
  research tools return a clear `tool_disabled` error until the operator
  grants them — do not work around it.
- **Sensitive engines are off-limits by default.** `hibp` and `dehashed`
  are rejected by **every** search path (generic explicit engines, targeted,
  jobs, security, science) unless the operator set `MCP_TARGETED_SENSITIVE_ALLOWED=1`.
  A `tool_disabled` error naming those engines means the operator has not
  opted in — do not try alternate spellings or workarounds; report the grant
  requirement instead.

## 9. Safety boundaries

- **Sensitive engines (`hibp`, `dehashed`) require one explicit operator
  grant.** They are unreachable from generic, category, and intent routing,
  and from every explicit-engine path, unless `MCP_TARGETED_SENSITIVE_ALLOWED=1`
  is set. The single shared policy gate applies the grant uniformly across
  generic, targeted, jobs, security, and science — the specialist grants
  enable their tools but do not grant sensitive access. Operators should
  treat granting `MCP_TARGETED_SENSITIVE_ALLOWED` as authorizing queries
  against breach and credential-exposure data.
- API keys are never exposed: the catalog reports only `auth.class` and
  `auth.configured`.
- `read_results`/`read_result` accept only server-issued cursors/IDs —
  never caller-supplied URLs.
- **The retrieval handoff never mints unsafe fetch targets.** The `retrieval`
  handoff record classifies every result URL (`ok`/`missing`/`non_http`/
  `unsafe_scheme`/`ambiguous`) and only hands off an absolute `http`/`https`
  URL with a host. `file:`, `data:`, `javascript:`, and similar schemes are
  marked ineligible (`retrieval.url` is `null`), so a downstream retriever
  cannot be pointed at them. SlopSearX performs no fetch anywhere — the MCP
  server is not an SSRF-capable page fetcher, and the handoff record is
  advisory composition metadata (see `docs/RETRIEVAL_HANDOFF.md`).
- Queries, engine fan-out, result pages, and job budgets are bounded.
- **OAuth mode trusts the network boundary.** Authorization is
  auto-approved for dynamically registered clients (no user accounts on
  SlopSearX), so any client that can complete registration + PKCE gets
  tokens — the same exposure as a published static token. Expose OAuth
  mode only behind TLS and access controls, and treat the issuer URL as
  public.
- Audits of MCP searches use the tenant identifier (`mcp:<tenant>`) rather
  than a client IP; raw queries are still retained per the shared audit
  policy (90 days in Valkey).

## 10. Operations

- **Valkey is the only shared state.** Without it, searches degrade
  gracefully (no cache, no rate-limit enforcement, no snapshots, no jobs).
- **Research execution topology.** With Valkey connected, research jobs are
  durable across replicas via the lease-based claim model (see §6.13.1); the
  `research_execution.mode` field in `slopsearx_get_service_status` reports
  the effective mode (`durable_leased` or `degraded`). Without Valkey,
  research jobs are not executed (the enqueued job is dropped).
- **Versioning.** `slopsearx_get_service_status` and `slopsearx://health/summary`
  report two versions: the package version (`importlib.metadata`, `0.2.0`)
  and the MCP contract version (`contract_version`), not the FastAPI app's
  `/health` version field.
- **Metrics.** Per-tool call/error/latency counters appear on the HTTP
  service's `/metrics` endpoint (`slopsearx_mcp_tool_*`) — operators only;
  agents see `slopsearx_get_service_status` instead.
- **Cache scoping & representation identity.** Cached responses are keyed by
  query, language, safesearch, categories, engines, page, and time range, so a
  cached response always matches the requested search scope. The cache stores
  the **canonical full response** — all include-able fields and the unsliced
  result set. `include` filtering and the `max_results` presentation slice are
  derived at the MCP read boundary from the current request, never from the
  request that populated the cache, so a cached response's `include`/`max_results`
  view always agrees with the current request. `max_results` bounds the presented
  page; it does not truncate the captured snapshot (pagination still reaches the
  full set).

## 11. Troubleshooting

| Symptom | Likely cause / fix |
|---|---|
| Remote client (e.g. Hermes) cannot connect | Server not bound for remote access (`MCP_HOST=0.0.0.0`), wrong port, token mismatch, or firewall; endpoint path is `/mcp` |
| OAuth connector (Claude Web) cannot authorize | Server not in OAuth mode (`MCP_OAUTH_ENABLED=1` + `MCP_OAUTH_ISSUER_URL`); issuer URL not reachable from the browser; TLS not terminated in front of the server |
| Gateway `--oauth` never prints an authorize URL | Callback port already in use by another process — pick a free one with `--oauth-callback-port`; verify the remote is in OAuth mode and reachable |
| Gateway `--oauth` prints the URL but authorization times out | The browser never hit the callback (loopback port blocked or URL opened on a different host); use `--oauth-no-browser` and complete the redirect on the agent host |
| Gateway re-authorizes on every run | The token file was not persisted — pass `--oauth-token-file FILE` (or `MCP_REMOTE_TOKEN_FILE`) to a stable path |
| Tool returns `tool_disabled` | Grant missing: set `MCP_GRANT_JOBS/SECURITY/SCIENCE/RESEARCH=1` |
| Any search tool returns `tool_disabled` naming `hibp`/`dehashed` | Sensitive engines need the uniform grant: `MCP_TARGETED_SENSITIVE_ALLOWED=1` (or `mcp.targeted_sensitive_allowed: true`) — deliberate policy boundary, not a bug |
| `invalid_scope` with alternatives | Engine name typo or engine disabled in config |
| `safesearch_unenforced` | No adapter enforces SafeSearch; use `moderate`/`off` |
| `expired_handle` / `store_unavailable` / `invalid_cursor` / `invalid_result_id` | Snapshot read lifecycle: `expired_handle` when a snapshot is present but past its TTL (with `expires_at`); `store_unavailable` when Valkey is unreachable at read time; `invalid_cursor`/`invalid_result_id` when the handle is unknown/malformed |
| `all_engines_failed` | Every selected engine failed; check `engine_outcomes` and retry |
| Research job stuck `running` | Process died; jobs are marked `expired` at next startup |
| No results but engines `ok` | Legitimate empty match (e.g. jobs tool without a company) |
