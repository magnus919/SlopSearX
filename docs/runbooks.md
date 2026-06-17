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
