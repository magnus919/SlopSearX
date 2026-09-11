# ADR 001: Web portal delivery architecture

- **Status:** Accepted for the first portal release
- **Scope:** The human browser portal tracked by milestone #9
- **Decision authority:** Maintainer directive in issue #315 and the implementation request for this milestone
- **Date:** 2026-09-11

## Context

SlopSearX already has a stable search core and machine-facing HTTP contract. The
search pipeline is normalized in [`SearchService`](../../slopsearx/service.py),
scope selection is handled by `ScopeResolver`, capability metadata is generated
by [`CapabilityCatalog`](../../slopsearx/capabilities.py), and the HTTP layer is
a thin adapter in [`server.py`](../../slopsearx/server.py). The existing cache
stores the canonical full response and derives request views at the read
boundary. A browser portal must reuse those boundaries instead of creating a
second search implementation.

The current API has four search paths (`GET`/`POST` on `/` and `/search`) and
format negotiation for HTML, JSON, CSV, RSS, and YAML. HTML is the human
default; an explicit machine format must retain its existing error and content
negotiation behavior. The portal's first slice is now served by the same
formatter, with a landing page for a bare browser visit.

The repository's current policy model is strongest on MCP paths. The service
resolver excludes sensitive engines from automatic routing, but an HTTP caller
can currently name an engine explicitly without passing through the MCP policy
grant. That is a real boundary to close in #328, not an assumption of parity.

## Options considered

### A. Server-rendered HTML with progressive enhancement (chosen)

- **Rendering:** FastAPI renders the HTML response from the normalized
  `SearchResponse`; a small amount of dependency-free JavaScript adds focus,
  theme, and navigation conveniences.
- **Framework/build tooling:** Python and the existing formatter only; no Node
  runtime or client bundler is required for the first release.
- **Packaging:** HTML, CSS, and JavaScript ship in the Python package/container
  that already serves `/search`; there is one deployable artifact and one
  versioned rollback point.
- **Testing:** route/formatter contract tests, deterministic fake-engine tests,
  and browser checks can exercise the same output that production serves.
- **Licensing/dependency burden:** no new frontend framework or font/icon
  dependency; browser-native controls and text are preferred.

### B. Embedded client application

A small client app could fetch JSON from `/search` and render results in the
browser. It would improve client-side transitions, but it introduces a build
toolchain, a second serialization boundary, a larger JavaScript payload, and
two places where access/error behavior can diverge. It remains a possible
follow-up once measured interaction needs justify it.

### C. Separate frontend deployment

A separately deployed React/Vue/Svelte application would maximize UI freedom,
but it adds an artifact registry, cross-origin policy, independent release
coordination, and a second operational surface. It is disproportionate for the
first portal and would make a safe rollback harder.

## Decision

Use **server-rendered HTML with progressive enhancement** for the first portal.

1. **One search path.** Browser requests construct `SearchRequest`, invoke
   `SearchService`, and render the resulting normalized response. The browser
   never calls an engine adapter directly.
2. **Stable routes.** A `GET /` with no query and an HTML negotiation returns
   the landing page. `GET /search` and `POST /search` (and the compatible root
   forms) remain search routes. `/health`, `/healthz`, `/metrics`, and `/config`
   remain operator/machine endpoints.
3. **Explicit URL state.** The first release serializes `q`, `categories`,
   `engines`, `language`, `time_range`, `safesearch`, and `pageno` in the URL.
   Changing scope or filters resets `pageno` to `1`; submitting the same URL is
   a reproducible search. MCP cursor snapshots remain an MCP concern and are
   not exposed as a browser-only hidden state.
4. **Truthful capability language.** The portal may show a scope or filter only
   when the live capability catalog exposes it. A consumed parameter is not
   described as enforced unless the audited `enforced_filters` declaration says
   so. Partial, empty, rejected, and unavailable states are rendered as states,
   not silently converted to a successful-looking empty page.
5. **Shared access boundary.** HTTP portal requests must use the same policy
   decision inputs as other search surfaces before dispatch. Sensitive engines
   remain unavailable to unscoped/category routing and require an operator
   grant for explicit browser selection. Browser-visible configuration never
   contains credentials; `auth_configured` is a boolean capability fact only.
6. **Operator-controlled default, user-controlled preference.** The operator
   selects `SLOPSEARX_PORTAL_DEFAULT_THEME=dark|darker` (default `dark`). A
   saved explicit browser choice wins over that default; if storage is
   unavailable, the choice lasts for the current document only. There is no
   automatic system-theme matching in this release.
7. **Same-origin deployment.** The portal is served by the SlopSearX process
   and container. A reverse proxy may provide authentication and TLS, but the
   application does not trust arbitrary forwarding headers and never exposes
   engine credentials to the browser. POST forms and any future mutating
   preference endpoint must carry an explicit CSRF decision before deployment
   (#328).

## Deployment and rollback

The release is one image/package. Deploy the portal behind the same proxy and
rate limit as `/search`, exercise `/` and a deterministic `/search?q=...`
smoke query, then promote traffic. Roll back by restoring the previous image
and configuration; no browser database migration or separate frontend asset
deployment is required. A feature flag is not required for the static first
release, but a proxy route switch may be used for a canary.

## Evidence and fitness functions

- Existing route and formatter tests cover the landing page, HTML result
  rendering, theme configuration, safe links, and preserved machine formats;
  see [`tests/test_server.py`](../../tests/test_server.py) and
  [`tests/test_formatter.py`](../../tests/test_formatter.py).
- PR #333 is the implementation evidence for the initial shell:
  <https://github.com/magnus919/SlopSearX/pull/333>.
- The remaining evidence is tracked by #328, #329, and #332: HTTP policy
  parity, deterministic failure fixtures, keyboard/screen-reader checks, and
  browser journeys must pass before release.

## External observations

The design study is intentionally small and dated. SearXNG exposes configured
categories as tabs and keeps additional engines discoverable through search
syntax ([categories as tabs](https://docs.searxng.org/admin/settings/settings_categories_as_tabs.html),
[search syntax](https://docs.searxng.org/user/search-syntax.html)); its admin
API also demonstrates that the search form is a normal same-origin POST
([administration API](https://docs.searxng.org/admin/api.html)). Brave's current
help page emphasizes obvious vertical filters, region/recency controls, and
pagination ([Brave Search help](https://search.brave.com/help)). Kagi documents
keyboard help, a `/` search focus shortcut, and lenses directly below the
search box ([Kagi operators](https://help.kagi.com/kagi/features/search-operators.html),
[Kagi quick start](https://help.kagi.com/kagi/getting-started/)). These are
observations of those products, not claims that SlopSearX matches their
relevance or privacy guarantees.
