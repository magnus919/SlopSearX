# Alerting

Active contributors: Magnus Hedemark

## Overview

Prometheus Alertmanager rules are defined in `docs/alerting/rules.yml`. These rules consume metrics from `GET /metrics` and fire alerts when service health degrades.

## Alert inventory

| Alert | Severity | Expression | For | Description |
|---|---|---|---|---|
| **SlopSearxDown** | critical | `up{job="slopsearx"} == 0` | 1m | Instance unreachable |
| **EngineDegraded** | warning | `slopsearx_engine_status > 0` | 5m | Engine status non-zero (degraded or down) |
| **HighErrorRatio** | warning | Query growth > 25% in 5m | 5m | Rapid traffic increase; check for anomaly |
| **HighLatency** | warning | `slopsearx_engine_latency_seconds{quantile="0.95"} > 5` | 5m | P95 latency exceeds 5 seconds |
| **RateLimitSaturation** | info | `rate(slopsearx_server_requests_total[5m]) > 100` | 5m | Server handling >100 req/s |
| **ServerErrorSpike** | warning | `rate(slopsearx_server_errors_total[5m]) > 0.1` | 5m | Error rate exceeds 0.1/s |

## Alertmanager configuration

### Docker Compose

Add prometheus + alertmanager services referencing `docs/alerting/rules.yml`:

```yaml
prometheus:
  image: prom/prometheus:latest
  volumes:
    - ./docs/alerting/rules.yml:/etc/prometheus/rules.yml
  command:
    - '--config.file=/etc/prometheus/prometheus.yml'

alertmanager:
  image: prom/alertmanager:latest
```

### Kubernetes

Apply as a PrometheusRule custom resource if using the Prometheus Operator:

```yaml
apiVersion: monitoring.coreos.com/v1
kind: PrometheusRule
metadata:
  name: slopsearx
spec:
  groups:
    - name: slopsearx
      rules:
        - alert: SlopSearxDown
          ...
```

## Runbooks

Incident response procedures are documented in `docs/runbooks.md`. Key scenarios:

### Health check recovery

1. Check Valkey: `docker compose exec valkey redis-cli PING`
2. Restart Valkey if unresponsive
3. Verify engine API keys
4. Full restart: `docker compose down && docker compose up -d`

### Rate limit triage

1. Check `/metrics` for rate limit counter spike
2. Verify Valkey is operational
3. Increase per-engine rate limits via `ENGINE_<NAME>_RATE_LIMIT`
4. Add proxy pool entries for scrape engines

### Engine degradation

1. Check `/health` for per-engine status
2. Circuit breaker auto-resets after 300s
3. Force circuit reset by restarting container
4. Check upstream API status if persistent

### Disk/memory pressure

1. SlopSearX is stateless except Valkey — restarting replicas is safe
2. Valkey data is ephemeral (caches have TTLs, audit streams auto-truncate)
3. Reduce Valkey maxmemory or trim audit retention for persistent issues

## Key source files

| File | Description |
|---|---|
| `docs/alerting/rules.yml` | Prometheus Alertmanager rules |
| `docs/runbooks.md` | Incident response procedures |
| `docs/grafana/per-engine-monitoring.json` | Grafana dashboard |
