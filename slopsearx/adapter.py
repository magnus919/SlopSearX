"""
SlopSearX — Engine Adapter Interface.

The adapter interface is the primary architectural invariant of SlopSearX.
Every engine is exactly one file, registered via @register_engine.
Adding a new engine requires zero changes to the orchestrator.

Adapters never raise exceptions — all error states are classified and
returned in AdapterResponse.status.
"""

from __future__ import annotations

import enum
import os
import time
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any, Optional
from urllib.parse import parse_qs, urlencode, urlparse, urlunparse

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


_SENSITIVE_QUERY_PARAMS: set[str] = {
    "api_key",
    "key",
    "apiKey",
    "token",
    "access_token",
}


def sanitize_url(url: str) -> str:
    """Strip known sensitive query parameters from a URL.

    Removes query parameters listed in ``_SENSITIVE_QUERY_PARAMS``
    (``api_key``, ``key``, ``apiKey``, ``token``, ``access_token``)
    from **url** to prevent credential leakage in error messages and
    logs.

    Args:
        url: The URL to sanitize.

    Returns:
        The sanitized URL with sensitive parameters removed, or the
        original string if parsing fails.
    """
    try:
        parsed = urlparse(url)
        if not parsed.query:
            # Nothing to remove — return URL as-is (no trailing ?).
            return url.rstrip("?")
        query_params = parse_qs(parsed.query, keep_blank_values=True)
        # Remove only the sensitive keys
        for key in _SENSITIVE_QUERY_PARAMS:
            query_params.pop(key, None)
        if not query_params:
            # All params were sensitive — drop the query entirely
            new_query = ""
        else:
            new_query = urlencode(query_params, doseq=True)
        sanitized = urlunparse(parsed._replace(query=new_query))
        return sanitized
    except Exception:  # noqa: BLE001
        # Malformed URL — return original string unmodified
        return url


# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------


@dataclass
class SearchResult:
    """Internal normalized result dataclass. Decoupled from wire format."""

    url: str
    title: str
    content: str
    engine: str  # primary engine name
    engines: set[str] = field(default_factory=set)
    score: float = 0.0
    position: int = 0
    category: str = "general"
    published_date: Optional[str] = None  # ISO 8601
    thumbnail: Optional[str] = None
    img_src: Optional[str] = None
    tier: int = 1  # 1 = primary (broad), 2 = secondary (specialized)


class EngineStatus(enum.Enum):
    """Standardized engine health / error classification."""

    OK = "ok"
    RATE_LIMITED = "rate_limited"
    BLOCKED = "blocked"
    ERROR = "error"
    TIMEOUT = "timeout"


@dataclass
class AdapterResponse:
    """Canonical response type for every adapter's search() method."""

    results: list[SearchResult]
    status: EngineStatus
    error_message: Optional[str] = None
    latency_ms: float = 0.0
    # SearXNG extended fields — populated by adapters that support them
    answers: list[dict[str, Any]] = field(default_factory=list)
    corrections: list[str] = field(default_factory=list)
    infoboxes: list[dict[str, Any]] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Base classes
# ---------------------------------------------------------------------------


class EngineAdapter(ABC):
    """Base class for all search-engine adapters.

    Each adapter lives in exactly one Python file, registered via the
    ``@register_engine`` decorator.  Subclasses override ``search()``;
    the base class provides sensible defaults for ``health()``,
    ``warmup()``, and ``shutdown()``.
    """

    # -- Engine identity (set on the class, not instance) -----------------
    name: str = ""  # e.g. "brave"
    display_name: str = ""  # e.g. "Brave Search API"
    env_prefix: str = ""  # e.g. "ENGINE_BRAVE"
    engine_type: str = "api"  # "api" | "scrape" | "structured"
    categories: list[str] = ["general"]  # SearXNG-compatible category tags

    # Circuit-breaker defaults (can be overridden per-instance via config or env vars)
    CIRCUIT_BREAKER_THRESHOLD: int = 5  # consecutive errors before circuit opens
    CIRCUIT_BREAKER_TIMEOUT: int = 300  # seconds circuit stays open

    def __init__(self, config: dict[str, Any] | None = None, rate_limiter: Any = None) -> None:
        self.config = config or {}
        self.rate_limiter = rate_limiter  # injected by server at startup
        # Merge categories: self-declared default + config override/add/remove
        self._merge_categories()

        # Circuit breaker state
        self.consecutive_errors: int = 0
        self.circuit_open_until: float = 0.0
        # Resolve threshold/timeout from env (fall back to class defaults)
        try:
            env_threshold = os.environ.get("ENGINE_CIRCUIT_THRESHOLD", str(self.CIRCUIT_BREAKER_THRESHOLD))
            self._circuit_threshold: int = int(env_threshold)
        except (ValueError, TypeError):
            self._circuit_threshold = self.CIRCUIT_BREAKER_THRESHOLD
        try:
            env_timeout = os.environ.get("ENGINE_CIRCUIT_TIMEOUT", str(self.CIRCUIT_BREAKER_TIMEOUT))
            self._circuit_timeout: int = int(env_timeout)
        except (ValueError, TypeError):
            self._circuit_timeout = self.CIRCUIT_BREAKER_TIMEOUT

    async def _check_rate_limit(self) -> AdapterResponse | None:
        """Check rate limiter before dispatching a search request.

        Returns an ``AdapterResponse`` with ``RATE_LIMITED`` status if
        the rate limiter denies the request, or ``None`` if allowed.
        Safe to call when ``self.rate_limiter`` is ``None`` (e.g. tests).
        """
        if self.rate_limiter is None:
            return None
        allowed = await self.rate_limiter.acquire(self.name)
        if not allowed:
            return AdapterResponse(
                results=[],
                status=EngineStatus.RATE_LIMITED,
                error_message="rate limited",
            )
        return None

    @abstractmethod
    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        """Execute a search against this engine.

        Args:
            query: The search query string.
            params: Opaque bag of normalisation hints such as
                ``language``, ``safesearch``, ``pageno``,
                ``categories``, ``time_range``.

        Returns:
            AdapterResponse — the canonical response type.
            **Never raises.**  All errors are classified and returned
            in the ``status`` field.
        """

    async def health(self) -> EngineStatus:
        """Lightweight probe — default sends a single-pager health check."""
        try:
            result = await self.search("healthcheck", {"pageno": 1})
            return result.status
        except Exception:  # noqa: BLE001
            return EngineStatus.ERROR

    async def warmup(self) -> None:
        """Optional lifecycle hook — called at startup."""

    async def shutdown(self) -> None:
        """Optional lifecycle hook — called at graceful shutdown."""

    def _merge_categories(self) -> None:
        """Merge self-declared categories with config override/add/remove.

        Config keys:
            categories: list[str] — full override (replaces self-declared)
            categories_add: list[str] — append to self-declared
            categories_remove: list[str] — suppress from self-declared
        """
        cat_cfg: dict[str, Any] = self.config.get("categories", {})
        if isinstance(cat_cfg, list):
            # Bare list = full override (backward-compat)
            self.categories = list(cat_cfg)
            return
        if not isinstance(cat_cfg, dict):
            return  # no category config
        if "override" in cat_cfg:
            self.categories = list(cat_cfg["override"])
            return
        # Add/remove path
        self.categories = list(type(self).categories)  # copy class attr
        for cat in cat_cfg.get("add", []):
            if cat not in self.categories:
                self.categories.append(cat)
        for cat in cat_cfg.get("remove", []):
            if cat in self.categories:
                self.categories.remove(cat)

    # ------------------------------------------------------------------
    # Circuit breaker
    # ------------------------------------------------------------------

    def circuit_allowed(self) -> bool:
        """Check whether the circuit breaker allows dispatching a query.

        Returns ``True`` if the circuit is closed (not tripped) or a
        half-open probe is due. Returns ``False`` if the circuit is
        open — the caller should skip this engine.
        """
        if self.circuit_open_until <= 0.0:
            return True  # closed
        if time.time() >= self.circuit_open_until:
            return True  # half-open probe due
        return False  # still open

    def record_success(self) -> None:
        """Record a successful response, resetting the circuit breaker."""
        self.consecutive_errors = 0
        self.circuit_open_until = 0.0

    def record_failure(self) -> None:
        """Record a failed response, potentially opening the circuit.

        If ``consecutive_errors`` reaches the threshold the circuit
        opens and remains open for ``circuit_timeout`` seconds.
        """
        self.consecutive_errors += 1
        if self.consecutive_errors >= self._circuit_threshold:
            self.circuit_open_until = time.time() + self._circuit_timeout


class ScrapeAdapter(EngineAdapter, ABC):
    """Base class for scrape-based engines (DDG, Google).

    Scrape adapters send HTTP GET/POST requests with stealth headers
    and parse HTML responses — no headless browser required.

    Supports optional proxy rotation via ``proxy_pool`` or
    ``scrape_proxy_url`` config keys.
    """

    engine_type = "scrape"

    def __init__(self, config: dict[str, Any] | None = None, rate_limiter: Any = None) -> None:
        super().__init__(config, rate_limiter)
        from slopsearx.proxypool import ProxyPool

        self._proxy_pool = ProxyPool.from_config(self.config)

    # Sensible defaults; individual engines can override.
    @property
    def request_headers(self) -> dict[str, str]:
        return {
            "User-Agent": (
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
                "(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
            ),
            "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            "Accept-Language": "en-US,en;q=0.9",
        }

    @property
    def timeout_ms(self) -> int:
        val: int = self.config.get("timeout_ms", 10_000)
        return val

    def _get_proxy(self) -> dict[str, str] | None:
        """Return an httpx-compatible proxy dict, or ``None``.

        Delegates to :class:`slopsearx.proxypool.ProxyPool` if
        the engine has proxy configuration.
        """
        if self._proxy_pool is None:
            return None
        return self._proxy_pool.get_proxy()

    def _report_proxy_success(self, proxy: dict[str, str] | None) -> None:
        """Report a successful request through the given proxy."""
        if self._proxy_pool is not None:
            self._proxy_pool.report_success(proxy)

    def _report_proxy_failure(self, proxy: dict[str, str] | None) -> None:
        """Report a failed request (CAPTCHA, 429, 403) through the given proxy."""
        if self._proxy_pool is not None:
            self._proxy_pool.report_failure(proxy)

    async def health(self) -> EngineStatus:
        """Probe: can we reach the engine's homepage?"""
        import httpx

        base_url = self.config.get("base_url", "")
        if not base_url:
            return EngineStatus.ERROR
        try:
            async with httpx.AsyncClient(timeout=5.0) as client:
                resp = await client.get(base_url, headers=self.request_headers)
                return EngineStatus.OK if resp.status_code == 200 else EngineStatus.ERROR
        except Exception:  # noqa: BLE001
            return EngineStatus.ERROR


# ---------------------------------------------------------------------------
# Registry
# ---------------------------------------------------------------------------


_ENGINE_REGISTRY: dict[str, type[EngineAdapter]] = {}


def register_engine(cls: type[EngineAdapter]) -> type[EngineAdapter]:
    """Decorator that registers an adapter class in the global registry.

    Usage::

        @register_engine
        class MyEngine(EngineAdapter):
            name = "myengine"
            ...
    """
    assert issubclass(cls, EngineAdapter), f"{cls.__name__} must subclass EngineAdapter"
    assert cls.name, f"{cls.__name__} must set a non-empty class-level 'name'"
    _ENGINE_REGISTRY[cls.name] = cls
    return cls


def list_engines() -> dict[str, type[EngineAdapter]]:
    """Return a copy of the current engine registry."""
    return dict(_ENGINE_REGISTRY)


def discover_engines(
    engine_configs: dict[str, dict[str, Any]] | None = None,
) -> dict[str, EngineAdapter]:
    """Instantiate all registered adapters with their per-engine config.

    Args:
        engine_configs: Mapping of engine name → config dict.
            If ``None``, engines are instantiated with empty config.

    Returns:
        Dict of engine name → instantiated (and enabled) adapter.
    """
    engine_configs = engine_configs or {}
    instances: dict[str, EngineAdapter] = {}
    for name, cls in _ENGINE_REGISTRY.items():
        cfg = dict(engine_configs.get(name, {}))
        if cfg.get("enabled", True):
            # Restructure category config for _merge_categories()
            cat_opts: dict[str, list[str]] = {}
            if cfg.get("categories"):
                cat_opts["override"] = cfg.pop("categories")
            if cfg.get("categories_add"):
                cat_opts["add"] = cfg.pop("categories_add")
            if cfg.get("categories_remove"):
                cat_opts["remove"] = cfg.pop("categories_remove")
            if cat_opts:
                cfg["categories"] = cat_opts
            instances[name] = cls(cfg)
    return instances
