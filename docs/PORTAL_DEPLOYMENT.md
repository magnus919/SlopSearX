# Portal deployment and release runbook

The portal is served by the same SlopSearX process and image as the API. There
is no separate frontend build, asset host, browser database, or migration.
This makes the portal and API one release unit with one rollback point.

## Browser URLs

- `GET /` — browser landing page with the configured default theme.
- `GET /search?q=...` — HTML results page.
- `GET /search?q=...&format=json` — SearXNG-compatible JSON for clients.
- `GET /healthz` — readiness probe used by deployment smoke checks.
- `GET /health` — operator health and observed engine state.

The application is same-origin. If a reverse proxy publishes it below a host
name, proxy `/` and `/search` together; do not rewrite only the HTML route.
Keep `/health` and `/healthz` available to the orchestrator, and keep
`/metrics` restricted to the monitoring network.

## Configuration and access modes

The portal defaults to Dark mode. Set `SLOPSEARX_PORTAL_DEFAULT_THEME=darker`
to make the initial document use the Darker near-black palette. A user's
explicit Darker/Dark choice is stored in browser local storage when available;
storage failure only makes the choice last for the current document.

SlopSearX has no browser account or session layer. For a private deployment,
place the service behind the internal network or an authenticated reverse
proxy. For a public deployment, require TLS and proxy authentication before
forwarding to port 8080. The application does not trust arbitrary forwarded
headers, and engine API keys remain server-side.

The portal and MCP surface share the sensitive-engine policy. Explicit browser
selection of the default sensitive engines (`hibp` and `dehashed`) is rejected
unless the operator sets `MCP_TARGETED_SENSITIVE_ALLOWED=true`. This grant is
an operator decision; it does not make sensitive engines eligible for
unscoped or category routing.

Search forms are read-only requests in this release, so there is no browser
mutation or authenticated session that needs a CSRF token. The portal emits no
analytics, third-party scripts, or remote fonts by default. Result thumbnails
are the only remote media and are limited to HTTP(S) URLs; untrusted text is
escaped before it enters the document.

## Verified artifact deployment

Use the digest-pinned image from `docker-compose.yml` or `k8s/deployment.yaml`.
CI builds, scans, and smoke-tests the image before it is published. To select
the artifact for a commit, resolve its short-SHA tag and update the deployment
pin:

```bash
docker buildx imagetools inspect ghcr.io/magnus919/slopsearx:<short-sha> \
  --format '{{json .Manifest.Digest}}'
```

The repository's Docker workflow runs the portable release smoke check:

```bash
python3 scripts/portal_release_smoke.py \
  --base-url http://127.0.0.1:8080 \
  --expected-theme dark
```

The check verifies the landing form, default theme, CSP, readiness response,
and a JSON search envelope. A search returning 503 is accepted when upstream
sources are unavailable, provided it remains a valid response with the
requested query and result list.

## Upgrade, canary, and rollback

1. Deploy the new digest to one replica or an isolated compose service.
2. Run the smoke check for the landing page, HTML search, JSON search, and
   `/healthz`; inspect the `Content-Security-Policy` and `X-Request-ID` headers.
3. Exercise the Dark/Darker toggle, keyboard `/` focus, scope disclosure, and
   a page-two URL in the supported browser matrix. Record the browser, viewport,
   commit, and any unavailable assistive-technology environment.
4. Promote the digest after readiness is healthy and the deterministic browser
   job is green.
5. If landing/search routes, policy boundaries, escaping, or readiness fail,
   restore the previous image digest and configuration. No frontend asset or
   browser data rollback is required.

The release smoke script and CI browser job are local/CI evidence. A
production rollout and rollback rehearsal still require the operator's target
cluster or host; record that live verification separately from these checks.

## Troubleshooting

- Seeing `query_required` at the root means the request negotiated a machine
  format or was sent to `/search`; open `/` with an HTML Accept header for the
  landing page.
- A 403 `engine_restricted` response means an explicit sensitive engine was
  requested without the operator grant. Check the policy setting rather than
  exposing an engine key to the browser.
- A 503 result page can be a truthful all-source or cached failure. Inspect
  `unresponsive_engines`, `/health`, and the configured engine credentials.
- If the portal loads without styles, confirm the proxy forwards `/` and
  `/search` to the same service and does not cache an HTML error response.
