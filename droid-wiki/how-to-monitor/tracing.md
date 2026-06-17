# Tracing

Active contributors: Magnus Hedemark

## Overview

SlopSearX uses **X-Request-ID** headers for distributed tracing across services. Every request gets a unique UUIDv4 ID that propagates through the entire request lifecycle.

## How it works

`RequestIDMiddleware` (`slopsearx/middleware.py`) is applied to the FastAPI app:

```python
app.add_middleware(RequestIDMiddleware)
```

### Behavior

1. **Incoming requests:** If the request already has an `X-Request-ID` header, it is **preserved** (supports upstream tracing)
2. **New requests:** If no `X-Request-ID` is present, a new UUIDv4 is generated
3. **Request state:** The ID is stored in `request.state.request_id` for downstream use by handlers and loggers
4. **Response:** The ID is echoed in the response `X-Request-ID` header

### Trace format

```
X-Request-ID: 550e8400-e29b-41d4-a716-446655440000
```

SlopSearX also generates shorter query IDs for response metadata:

```
query_id: "ssx-abc12345"
```

These query IDs are included in the `meta.query_id` field of every response for traceability.

## Use cases

- **Distributed tracing:** When SlopSearX is behind a gateway or load balancer, propagate the incoming X-Request-ID through all services
- **Log correlation:** Combine X-Request-ID with structured logging to trace a request through all log events
- **Error tracking:** Sentry automatically tags errors with the request ID when configured
- **Audit trail:** Query IDs in the audit stream can be matched to specific response `meta.query_id` values

## Integration with Sentry

When `SENTRY_DSN` is configured, the request ID is automatically added as a Sentry tag, enabling:
- Correlation of errors with specific requests
- Tracing errors back to request context and browser logs
- Grouping related errors from the same request chain

## Key source files

| File | Description |
|---|---|
| `slopsearx/middleware.py` | RequestIDMiddleware implementation |
| `slopsearx/server.py` | `_generate_query_id()` for response metadata |
