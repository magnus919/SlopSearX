# Deployment

## Docker

SlopSearX ships with a multi-stage Dockerfile based on `python:3.12-slim`.

- Base image: `python:3.12-slim`
- Final image size: approximately 200MB
- Health check: HTTP GET on port 8080 `/health`
- User: non-root
- Exposed port: 8080

### Docker Compose

The `docker-compose.yml` defines two services:

- `slopsearx` - The search application container.
- `valkey` - Valkey (Redis-compatible) server for caching and distributed rate limiting.

```
docker compose up -d
```

## Kubernetes

### Deployment

`k8s/deployment.yaml` defines the application deployment:

- Default replicas: 3
- Configuration via ConfigMap
- Secrets via SecretKeyRef (API keys)
- Liveness probe: HTTP GET `/health` (initial delay 5s, period 10s)
- Readiness probe: HTTP GET `/health` (initial delay 5s, period 5s)

### Horizontal Pod Autoscaler

`k8s/hpa.yaml` defines autoscaling:

- Min replicas: 3
- Max replicas: 100
- Target CPU utilization: 70%

### Service

`k8s/service.yaml` defines a ClusterIP service:

- Type: ClusterIP
- Port: 8080
- Target port: 8080

### Kustomization

`k8s/kustomization.yaml` bundles the Kubernetes manifests for aggregate deployment:

```
kubectl apply -k k8s/
```
