"""Structured logging via structlog.  Optional Sentry error tracking.

Provides JSON-formatted structured logging as a drop-in replacement for
stdlib logging.  All log events carry timestamp, logger name, log level,
and any extra keyword arguments as structured fields.

If the ``SENTRY_DSN`` environment variable is set, Sentry error tracking
is activated automatically with breadcrumbs, request-id tagging, and
full stack traces.  Otherwise Sentry remains dormant — zero overhead.

Usage::

    from slopsearx.logging import get_logger, setup_logging

    setup_logging()
    log = get_logger(__name__)
    log.info("request_processed", query="test", engine_count=5)
"""

from __future__ import annotations

import logging
import os
import sys
from typing import Any

import structlog


def _init_sentry(dsn: str, sample_rate: float = 1.0) -> None:
    """Initialize Sentry SDK if a DSN is configured."""
    import sentry_sdk

    sentry_sdk.init(
        dsn=dsn,
        sample_rate=sample_rate,
        send_default_pii=False,
        traces_sample_rate=0.0,
        _experiments={"continuous_profiling_auto_start": False},  # type: ignore[unused-ignore]
    )


def setup_logging(
    *,
    level: int = logging.INFO,
    json_output: bool = True,
    sentry_dsn: str | None = None,
) -> None:
    """Configure structlog for the entire application.

    Args:
        level: Minimum log level (default INFO).
        json_output: If True, emit JSON; otherwise human-readable console output.
        sentry_dsn: Optional Sentry DSN for error tracking.  Also read from
            ``SENTRY_DSN`` env var.  If omitted and env var is unset,
            Sentry is not activated.
    """
    dsn = sentry_dsn or os.getenv("SENTRY_DSN")
    if dsn:
        _init_sentry(dsn)

    processors: list[Any] = [
        structlog.stdlib.filter_by_level,
        structlog.stdlib.add_logger_name,
        structlog.stdlib.add_log_level,
        structlog.stdlib.PositionalArgumentsFormatter(),
        structlog.processors.TimeStamper(fmt="iso"),
        structlog.processors.StackInfoRenderer(),
        structlog.processors.format_exc_info,
    ]

    if json_output:
        processors.append(structlog.processors.JSONRenderer())
    else:
        processors.append(structlog.dev.ConsoleRenderer())

    structlog.configure(
        processors=processors,
        wrapper_class=structlog.stdlib.BoundLogger,
        context_class=dict,
        logger_factory=structlog.stdlib.LoggerFactory(),
        cache_logger_on_first_use=True,
    )

    root_logger = logging.getLogger()
    root_logger.setLevel(level)
    handler = logging.StreamHandler(sys.stderr)
    handler.setLevel(level)
    root_logger.handlers = [handler]


def get_logger(name: str | None = None) -> Any:
    """Return a structured logger bound to *name*."""
    return structlog.get_logger(name or __name__)


def capture_exception(error: BaseException) -> None:
    """Report *error* to Sentry if configured.  No-op otherwise."""
    try:
        import sentry_sdk
    except ImportError:
        return
    sentry_sdk.capture_exception(error)
