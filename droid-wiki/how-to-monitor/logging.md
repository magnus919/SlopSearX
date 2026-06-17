# Logging

Active contributors: Magnus Hedemark

## Overview

SlopSearX uses **structlog** for structured JSON logging. All log events carry timestamp, logger name, log level, and any extra keyword arguments as structured fields. Stdlib logging is configured as the backend, so existing libraries (httpx, valkey) log through the same pipeline.

## Setup

```python
from slopsearx.logging import setup_logging, get_logger

setup_logging()           # called at server startup
log = get_logger(__name__)
log.info("request_processed", query="test", engine_count=5)
```

Output (JSON mode):
```json
{"event": "request_processed", "logger": "slopsearx.server", "level": "info", "timestamp": "2026-06-15T12:34:56.789Z", "query": "test", "engine_count": 5}
```

## Configuration

`setup_logging()` accepts:

| Parameter | Default | Description |
|---|---|---|
| `level` | `logging.INFO` | Minimum log level |
| `json_output` | `True` | JSON (production) vs console (dev) |
| `sentry_dsn` | `None` | Sentry DSN for error tracking (also read from `SENTRY_DSN` env var) |

## Sentry error tracking

When `SENTRY_DSN` is set (env var or parameter), Sentry SDK initializes automatically:

- **Exception capture:** All unhandled exceptions in engine dispatch are reported
- **Breadcrumbs:** Request context and log events automatically attached
- **Request ID tagging:** `X-Request-ID` added as Sentry tag for trace correlation
- **Zero overhead:** When DSN is not set, Sentry is never initialized

### Sentry → GitHub integration

For self-hosted Sentry, enable GitHub integration to auto-create issues from errors:

1. Sentry → Settings → Integrations → GitHub → Add Installation
2. Select `magnus919/SlopSearX` repository
3. Configure issue creation rules (e.g., new unresolved error → GitHub issue)
4. Errors appear as GitHub issues with stack traces and context

## Log output

### JSON (production)

```json
{"event": "engine_dispatched", "engine": "brave", "latency_ms": 340, "results": 10, "timestamp": "2026-06-15T12:34:56.789Z", "logger": "slopsearx.adapter", "level": "info"}
```

### Console (development)

```
2026-06-15T12:34:56.789Z [info     ] engine_dispatched              engine=brave latency_ms=340 results=10
```

## Integration

- **Server startup:** `setup_logging()` in `_startup()`
- **Engine dispatch:** `capture_exception(exc)` for unhandled exceptions
- **Middleware:** `RequestIDMiddleware` stores request ID in `request.state.request_id`
- **Valkey clients:** `SearchCache`, `ValkeySlidingWindow` use stdlib logging → structlog pipeline

## Key source files

| File | Description |
|---|---|
| `slopsearx/logging.py` | structlog setup, Sentry integration, `capture_exception()` |
| `slopsearx/middleware.py` | X-Request-ID middleware |
