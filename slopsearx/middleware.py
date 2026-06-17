"""FastAPI middleware: X-Request-ID propagation and structured logging.

Adds a unique ``X-Request-ID`` header to every response and attaches it
to the request state for downstream consumers.  Incoming ``X-Request-ID``
headers are preserved to support distributed tracing across services.
"""

from __future__ import annotations

import uuid
from typing import Awaitable, Callable

from starlette.middleware.base import BaseHTTPMiddleware
from starlette.requests import Request
from starlette.responses import Response


class RequestIDMiddleware(BaseHTTPMiddleware):
    """Inject / propagate an X-Request-ID header on every request.

    - If the incoming request already carries ``X-Request-ID``, it is
      preserved and echoed back.
    - Otherwise a new UUIDv4 is generated.
    - The id is stored in ``request.state.request_id`` for use by
      downstream middleware, route handlers, and loggers.
    """

    async def dispatch(
        self,
        request: Request,
        call_next: Callable[[Request], Awaitable[Response]],
    ) -> Response:
        request_id = request.headers.get("X-Request-ID") or str(uuid.uuid4())
        request.state.request_id = request_id

        response = await call_next(request)
        response.headers["X-Request-ID"] = request_id
        return response
