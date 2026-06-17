# Runbooks

Incident response procedures for SlopSearX operators.

## Health Check Recovery

**Symptom:** `/health` returns non-200 or reports engine failures.

1. Check Valkey connectivity: `docker compose exec valkey redis-cli PING`
2. Restart Valkey if unresponsive: `docker compose restart valkey`
3. Verify engine API keys are valid (check env vars / K8s secrets)
4. Full restart: `docker compose down && docker compose up -d`

## Rate Limit Triage

**Symptom:** High rate of 429 responses or engines cycling through cooldown.

1. Check `/metrics` for `slopsearx_rate_limited_total` counter spike
2. Verify Valkey is operational (rate limiter falls back to in-process otherwise)
3. Increase per-engine rate limits via `ENGINE_<NAME>_RATE_LIMIT` env vars
4. Add proxy pool entries via `ENGINE_<NAME>_PROXY_POOL` for scrape engines

## Engine Degradation

**Symptom:** Individual engine returning errors or circuit breaker open.

1. Check `/health` response for per-engine status
2. Circuit breaker auto-resets after 300s of cooldown
3. Force circuit reset by restarting the container
4. If persistent, check upstream API status and update engine adapter if API changed

## Disk/Memory Pressure

**Symptom:** Container OOM kills or disk alerts near Valkey data.

1. SlopSearX is stateless except for Valkey — restarting any replica is safe
2. Valkey data is ephemeral (cache keys have TTLs, audit stream auto-truncates)
3. For persistent disk issues, reduce Valkey maxmemory or trim audit retention

## Alerting

Prometheus Alertmanager rules are defined in `docs/alerting/rules.yml`.
These rules cover:

- **Availability**: instance down detection
- **Engine health**: degraded engines, high error rates
- **Latency**: P95 latency exceeding thresholds
- **Rate limiting**: client throttling alerts
- **Cache**: cache miss ratio monitoring
- **Server errors**: overall error rate tracking

See `docs/grafana/per-engine-monitoring.json` for the companion Grafana
dashboard that surfaces these metrics visually.

## Error Tracking (Sentry)

Set `SENTRY_DSN` to enable automatic error reporting via Sentry (self-hosted
or SaaS).  When configured, all unhandled exceptions in engine dispatch are
captured with full stack traces, request metadata, and breadcrumbs.

### Sentry → GitHub Integration

When using self-hosted Sentry, enable the GitHub integration to auto-create
issues from error events:

1. In Sentry: Settings → Integrations → GitHub → Add Installation
2. Select the `magnus919/SlopSearX` repository
3. Configure issue creation rules (e.g., create issue on new unresolved error)
4. Errors will automatically appear as GitHub issues with stack traces and context

This closes the loop from production error → tracked issue → code fix.

