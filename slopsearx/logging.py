"""Structured logging via structlog.

Provides JSON-formatted structured logging as a drop-in replacement for
stdlib logging.  All log events carry timestamp, logger name, log level,
and any extra keyword arguments as structured fields.

Usage::

    from slopsearx.logging import get_logger

    log = get_logger(__name__)
    log.info("request_processed", query="test", engine_count=5)
"""

from __future__ import annotations

import logging
import sys
from typing import Any

import structlog


def setup_logging(*, level: int = logging.INFO, json_output: bool = True) -> None:
    """Configure structlog for the entire application.

    Args:
        level: Minimum log level (default INFO).
        json_output: If True, emit JSON; otherwise human-readable console output.
    """
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
