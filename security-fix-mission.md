# SlopSearX Security Fix Mission

Fix the 5 highest-impact security issues identified by the deep audit. These are all real exploit paths or production stability risks — no config-access preconditions needed.

## Milestone 1: Stop API key leaks in error messages

**Files:** `engines/nvd.py`, `engines/shodan.py`, `engines/stackexchange.py`

When these adapters get an HTTP error, `str(exc)` on `httpx.HTTPStatusError` includes the full request URL with the API key in query parameters. This error message is returned to the client via the unauthenticated `/search` endpoint.

**Task 1.1:** Add a helper function that strips query parameters (or at least known sensitive params like `api_key`) from URLs before they go into error messages. Place it in `slopsearx/adapter.py` so all adapters can use it.

**Task 1.2:** Update `nvd.py`, `shodan.py`, and `stackexchange.py` to sanitize the URL passed to `str(exc)` — either by using the helper, or by catching `HTTPStatusError` separately and building a sanitized error string before the broad `except Exception` handler.

**Success criteria:** Triggering a 403/429 on these engines no longer returns the API key in `unresponsive_engines` error messages.

---

## Milestone 2: Move API keys from URL query params to HTTP headers

**Files:** `engines/fred.py`, `engines/nvd.py`, `engines/shodan.py`, `engines/stackexchange.py`, `engines/tmdb.py`

These adapters pass API keys as URL query parameters (`?api_key=...` or `&api_key=...`), exposing them in access logs, proxy logs, Referer headers, and error messages.

**Task 2.1:** For each engine, replace the URL query-param auth with the appropriate header-based auth:
- **Shodan**: `httpx.Client(headers={"Authorization": f"Bearer {key}"})` — Shodan accepts `Authorization: Bearer <token>` in headers
- **NVD**: `httpx.Client(headers={"apiKey": key})` — NVD accepts `apiKey` header
- **StackExchange**: `httpx.Client(headers={"X-API-Key": key})` — StackExchange accepts `X-API-Key` header (or `Authorization: Bearer`)
- **TMDB**: `httpx.Client(headers={"Authorization": f"Bearer {key}"})` — TMDB prefers header-based auth
- **FRED**: St. Louis FRED API only supports query-param auth — for this one, add a `_sanitize_url` helper that redacts the key from error messages and logs

**Task 2.2:** After migrating, remove `api_key` from the URL construction so the key is never in the query string.

**Success criteria:** No API key appears in the URL path or query string of any outbound request.

---

## Milestone 3: Replace sync Valkey calls with async

**Files:** `slopsearx/cache.py`, `slopsearx/ratelimit.py`, `slopsearx/stats.py`

All three modules use the synchronous `valkey.Valkey` class in an async FastAPI context. Under any real load, blocking calls stall the event loop and tank throughput.

**Task 3.1:** Switch to `valkey.asyncio.Valkey` everywhere. The async API is nearly identical — `await client.get(key)` instead of `client.get(key)`. The connection pool management should use `async with` context managers.

**Task 3.2:** Update all callers in `server.py` to `await` the async operations.

**Task 3.3:** Update the Valkey connection initialization to create an async client instance instead of sync.

**Task 3.4:** Update the `SearchCache`, `ValkeySlidingWindow`, and `EngineStatsTracker` constructors and method signatures to be async where they perform I/O.

**Success criteria:** No synchronous `valkey.Valkey` calls remain. All Valkey operations are properly awaited.

---

## Milestone 4: Add concurrency limit to engine dispatch

**Files:** `slopsearx/server.py`, `slopsearx/merger.py`

One `/search` request fans out to up to 48 concurrent HTTP requests with no semaphore or backpressure. This exhausts connection pools, hits upstream rate limits, and enables amplification attacks.

**Task 4.1:** Add an `asyncio.Semaphore` in the search handler or merger with a configurable max concurrency (default 10). Wrap each engine dispatch in `async with semaphore`.

**Task 4.2:** Make the concurrency limit configurable via an env var or config setting (e.g., `MAX_CONCURRENT_ENGINES`).

**Task 4.3:** Add a per-client/IP rate limiter using the existing `ValkeySlidingWindow` infrastructure — keyed on client IP from `request.client.host`, with configurable requests-per-window.

**Success criteria:** A single search request cannot create more than N concurrent outbound HTTP connections. Repeated requests from the same client are rate-limited.

---

## Milestone 5: Make rate limiter fail-closed

**Files:** `slopsearx/ratelimit.py`

When Valkey is unavailable, `ValkeySlidingWindow.check()` catches the exception and returns `no_delay=True` (allow through). This means a Valkey outage disables all rate limiting.

**Task 5.1:** Add a `fail_closed` parameter to `ValkeySlidingWindow.__init__` (default `False` to preserve current behavior for rollback safety).

**Task 5.2:** When `fail_closed=True`, the exception handler returns `no_delay=False` (block request) instead of `True`.

**Task 5.3:** Add a local Redis-unavailable flag as a safety valve: if Valkey has been down for more than N seconds and `fail_closed=True`, fall back to a simple in-process `LocalTokenBucket` so the service doesn't fully lock up.

**Task 5.4:** Wire the `fail_closed` setting through the config loading in `server.py`.

**Success criteria:** Rate limit enforcement survives a Valkey restart or network blip.
